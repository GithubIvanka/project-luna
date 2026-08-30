//! Bundle Format v1 (`.lbp`) candidate codec.
//!
//! This module implements the currently proposed LBP1 container boundary. It is
//! intentionally independent from application lifecycle management: the bundle
//! manager decides when to install/import/update a bundle, while this module
//! only reads, validates and writes the transport representation.

use std::fs::{self, File};
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};

use blake3::Hasher;
use serde::{Deserialize, Serialize};
use tar::{Archive, Builder, EntryType, Header};

use crate::{validate_manifest, BundleError, BundleKind, BundleManifest, BundleMetadata, BundleResource};
use luna_common::{BundleId, Version};

pub const MAGIC: [u8; 4] = *b"LBP1";
pub const FORMAT_VERSION: u16 = 1;
pub const HEADER_SIZE: usize = 52;
pub const SECTION_ENTRY_SIZE: usize = 64;

const SECTION_MANIFEST: u32 = 1;
const SECTION_PAYLOAD: u32 = 2;
const SECTION_SIGNATURE: u32 = 4;
const COMPRESSION_NONE: u32 = 0;
const COMPRESSION_ZSTD: u32 = 1;

#[derive(Debug)]
pub enum LbpError {
    Io(io::Error),
    InvalidHeader,
    UnsupportedVersion(u16),
    InvalidSectionTable,
    InvalidSection,
    UnsupportedCompression(u32),
    HashMismatch,
    Manifest(BundleError),
    ManifestFormat(String),
    PayloadFormat(String),
    PayloadPath(PathBuf),
    UnsupportedEntry(String),
    NumericOverflow,
}

impl std::fmt::Display for LbpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::InvalidHeader => f.write_str("invalid LBP1 header"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported LBP format version: {v}"),
            Self::InvalidSectionTable => f.write_str("invalid LBP section table"),
            Self::InvalidSection => f.write_str("invalid LBP section"),
            Self::UnsupportedCompression(c) => write!(f, "unsupported LBP compression: {c}"),
            Self::HashMismatch => f.write_str("LBP content hash mismatch"),
            Self::Manifest(e) => write!(f, "invalid bundle manifest: {e}"),
            Self::ManifestFormat(e) => write!(f, "invalid manifest TOML: {e}"),
            Self::PayloadFormat(e) => write!(f, "invalid payload archive: {e}"),
            Self::PayloadPath(p) => write!(f, "unsafe payload path: {}", p.display()),
            Self::UnsupportedEntry(p) => write!(f, "unsupported payload entry: {p}"),
            Self::NumericOverflow => f.write_str("numeric overflow in LBP structure"),
        }
    }
}

impl std::error::Error for LbpError {}
impl From<io::Error> for LbpError { fn from(value: io::Error) -> Self { Self::Io(value) } }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SectionKind { Manifest, Payload, Signature }

impl SectionKind {
    fn to_u32(self) -> u32 {
        match self { Self::Manifest => SECTION_MANIFEST, Self::Payload => SECTION_PAYLOAD, Self::Signature => SECTION_SIGNATURE }
    }
    fn from_u32(value: u32) -> Result<Self, LbpError> {
        match value {
            SECTION_MANIFEST => Ok(Self::Manifest),
            SECTION_PAYLOAD => Ok(Self::Payload),
            SECTION_SIGNATURE => Ok(Self::Signature),
            _ => Err(LbpError::InvalidSection),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SectionInfo {
    pub kind: SectionKind,
    pub compression: u32,
    pub offset: u64,
    pub compressed_length: u64,
    pub uncompressed_length: u64,
    pub content_hash: [u8; 32],
}

#[derive(Clone, Debug)]
pub struct LbpArchive {
    pub manifest: BundleManifest,
    pub sections: Vec<SectionInfo>,
    pub bytes: Vec<u8>,
}

impl LbpArchive {
    pub fn read_from_path(path: impl AsRef<Path>) -> Result<Self, LbpError> {
        let bytes = fs::read(path)?;
        Self::from_bytes(bytes)
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, LbpError> {
        if bytes.len() < HEADER_SIZE { return Err(LbpError::InvalidHeader); }
        if bytes[0..4] != MAGIC { return Err(LbpError::InvalidHeader); }
        let version = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
        if version != FORMAT_VERSION { return Err(LbpError::UnsupportedVersion(version)); }
        let section_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let table_offset = u64::from_le_bytes(bytes[12..20].try_into().unwrap());
        let expected_hash = &bytes[20..52];
        let mut header = bytes[0..HEADER_SIZE].to_vec();
        header[20..52].fill(0);
        if hash32(&header) != expected_hash { return Err(LbpError::HashMismatch); }

        let table_len = section_count.checked_mul(SECTION_ENTRY_SIZE).ok_or(LbpError::NumericOverflow)?;
        let table_end = usize::try_from(table_offset).map_err(|_| LbpError::NumericOverflow)?.checked_add(table_len).ok_or(LbpError::NumericOverflow)?;
        if table_end > bytes.len() || table_offset < HEADER_SIZE as u64 { return Err(LbpError::InvalidSectionTable); }

        let mut sections = Vec::with_capacity(section_count);
        for i in 0..section_count {
            let start = usize::try_from(table_offset).unwrap() + i * SECTION_ENTRY_SIZE;
            let kind = SectionKind::from_u32(u32::from_le_bytes(bytes[start..start + 4].try_into().unwrap()))?;
            let compression = u32::from_le_bytes(bytes[start + 4..start + 8].try_into().unwrap());
            let offset = u64::from_le_bytes(bytes[start + 8..start + 16].try_into().unwrap());
            let compressed_length = u64::from_le_bytes(bytes[start + 16..start + 24].try_into().unwrap());
            let uncompressed_length = u64::from_le_bytes(bytes[start + 24..start + 32].try_into().unwrap());
            let mut content_hash = [0u8; 32];
            content_hash.copy_from_slice(&bytes[start + 32..start + 64]);
            validate_range(offset, compressed_length, bytes.len())?;
            match compression {
                COMPRESSION_NONE | COMPRESSION_ZSTD => {}
                other => return Err(LbpError::UnsupportedCompression(other)),
            }
            sections.push(SectionInfo { kind, compression, offset, compressed_length, uncompressed_length, content_hash });
        }

        validate_non_overlapping(&sections)?;
        let manifest_section = sections.iter().find(|s| s.kind == SectionKind::Manifest).ok_or(LbpError::InvalidSectionTable)?;
        let payload_section = sections.iter().find(|s| s.kind == SectionKind::Payload).ok_or(LbpError::InvalidSectionTable)?;
        let manifest_bytes = decode_section(&bytes, manifest_section)?;
        let payload_bytes = decode_section(&bytes, payload_section)?;
        let manifest = parse_manifest(&manifest_bytes)?;
        validate_manifest(&manifest).map_err(LbpError::Manifest)?;
        validate_payload(&payload_bytes)?;
        Ok(Self { manifest, sections, bytes })
    }

    pub fn manifest_bytes(&self) -> Result<Vec<u8>, LbpError> {
        let section = self.sections.iter().find(|s| s.kind == SectionKind::Manifest).ok_or(LbpError::InvalidSectionTable)?;
        decode_section(&self.bytes, section)
    }

    pub fn payload_bytes(&self) -> Result<Vec<u8>, LbpError> {
        let section = self.sections.iter().find(|s| s.kind == SectionKind::Payload).ok_or(LbpError::InvalidSectionTable)?;
        decode_section(&self.bytes, section)
    }

    /// Extract the payload after structural validation. Existing files are never
    /// overwritten and all entry paths remain relative to `destination`.
    pub fn extract_payload(&self, destination: impl AsRef<Path>) -> Result<(), LbpError> {
        let destination = destination.as_ref();
        fs::create_dir_all(destination)?;
        let payload = self.payload_bytes()?;
        let mut archive = Archive::new(Cursor::new(payload));
        for entry in archive.entries().map_err(|e| LbpError::PayloadFormat(e.to_string()))? {
            let mut entry = entry.map_err(|e| LbpError::PayloadFormat(e.to_string()))?;
            let path = entry.path().map_err(|e| LbpError::PayloadFormat(e.to_string()))?.into_owned();
            validate_payload_path(&path)?;
            match entry.header().entry_type() {
                EntryType::Regular | EntryType::Directory => {}
                _ => return Err(LbpError::UnsupportedEntry(path.display().to_string())),
            }
            entry.unpack_in(destination).map_err(|e| LbpError::PayloadFormat(e.to_string()))?;
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct ManifestWire {
    format: u16,
    bundle: BundleWire,
    #[serde(default)]
    resources: Vec<ResourceWire>,
}

#[derive(Serialize, Deserialize)]
struct BundleWire {
    id: String,
    name: Option<String>,
    version: String,
    kind: String,
}

#[derive(Serialize, Deserialize)]
struct ResourceWire {
    logical: String,
    source: String,
}

fn parse_manifest(bytes: &[u8]) -> Result<BundleManifest, LbpError> {
    let wire: ManifestWire = toml::from_slice(bytes).map_err(|e| LbpError::ManifestFormat(e.to_string()))?;
    if wire.format != FORMAT_VERSION { return Err(LbpError::UnsupportedVersion(wire.format)); }
    let kind = match wire.bundle.kind.as_str() {
        "application" => BundleKind::Application,
        "component" => BundleKind::Component,
        _ => return Err(LbpError::ManifestFormat("unsupported bundle.kind".into())),
    };
    let version = parse_version(&wire.bundle.version).ok_or_else(|| LbpError::ManifestFormat("invalid semver".into()))?;
    let mut manifest = BundleManifest::new(BundleMetadata::new(BundleId::from(wire.bundle.id), version, kind));
    for resource in wire.resources {
        manifest.add_resource(BundleResource::new(resource.logical, resource.source));
    }
    Ok(manifest)
}

fn manifest_to_bytes(manifest: &BundleManifest) -> Result<Vec<u8>, LbpError> {
    validate_manifest(manifest).map_err(LbpError::Manifest)?;
    let kind = match manifest.metadata().kind() {
        BundleKind::Application => "application",
        BundleKind::Component => "component",
    };
    let wire = ManifestWire {
        format: FORMAT_VERSION,
        bundle: BundleWire {
            id: manifest.metadata().id().as_str().to_owned(),
            name: None,
            version: manifest.metadata().version().to_string(),
            kind: kind.to_owned(),
        },
        resources: manifest.resources().iter().map(|r| ResourceWire {
            logical: r.logical_path().to_owned(),
            source: r.source_path().to_owned(),
        }).collect(),
    };
    toml::to_string(&wire)
        .map(|s| s.into_bytes())
        .map_err(|e| LbpError::ManifestFormat(e.to_string()))
}

fn parse_version(value: &str) -> Option<Version> {
    let mut parts = value.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() { return None; }
    Some(Version::new(major, minor, patch))
}

/// Build a deterministic LBP1 archive from files referenced by a bundle manifest.
pub fn write_from_directory(path: impl AsRef<Path>, manifest: &BundleManifest, source_root: impl AsRef<Path>) -> Result<(), LbpError> {
    let bytes = build_from_directory(manifest, source_root)?;
    fs::write(path, bytes)?;
    Ok(())
}

pub fn build_from_directory(manifest: &BundleManifest, source_root: impl AsRef<Path>) -> Result<Vec<u8>, LbpError> {
    validate_manifest(manifest).map_err(LbpError::Manifest)?;
    let manifest_bytes = manifest_to_bytes(manifest)?;
    let payload = build_deterministic_tar(manifest, source_root.as_ref())?;
    let payload_compressed = zstd::stream::encode_all(Cursor::new(&payload), 3).map_err(|e| LbpError::PayloadFormat(e.to_string()))?;

    let sections = [
        (SectionKind::Manifest, COMPRESSION_NONE, manifest_bytes.clone()),
        (SectionKind::Payload, COMPRESSION_ZSTD, payload_compressed.clone()),
    ];

    let section_count = sections.len();
    let table_offset = HEADER_SIZE + 0usize;
    let data_offset = HEADER_SIZE.checked_add(section_count * SECTION_ENTRY_SIZE).ok_or(LbpError::NumericOverflow)?;
    let mut output = vec![0u8; data_offset];
    output[0..4].copy_from_slice(&MAGIC);
    output[4..6].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
    output[6..8].copy_from_slice(&0u16.to_le_bytes());
    output[8..12].copy_from_slice(&(section_count as u32).to_le_bytes());
    output[12..20].copy_from_slice(&(table_offset as u64).to_le_bytes());

    let mut section_infos = Vec::with_capacity(section_count);
    let mut cursor = data_offset as u64;
    for (kind, compression, data) in &sections {
        let hash = hash32(data);
        let info = SectionInfo {
            kind: *kind,
            compression: *compression,
            offset: cursor,
            compressed_length: data.len() as u64,
            uncompressed_length: if *compression == COMPRESSION_ZSTD { if *kind == SectionKind::Payload { payload.len() as u64 } else { data.len() as u64 } } else { data.len() as u64 },
            content_hash: hash,
        };
        section_infos.push(info);
        output.extend_from_slice(data);
        cursor = cursor.checked_add(data.len() as u64).ok_or(LbpError::NumericOverflow)?;
    }

    for (index, info) in section_infos.iter().enumerate() {
        let start = HEADER_SIZE + index * SECTION_ENTRY_SIZE;
        output[start..start + 4].copy_from_slice(&info.kind.to_u32().to_le_bytes());
        output[start + 4..start + 8].copy_from_slice(&info.compression.to_le_bytes());
        output[start + 8..start + 16].copy_from_slice(&info.offset.to_le_bytes());
        output[start + 16..start + 24].copy_from_slice(&info.compressed_length.to_le_bytes());
        output[start + 24..start + 32].copy_from_slice(&info.uncompressed_length.to_le_bytes());
        output[start + 32..start + 64].copy_from_slice(&info.content_hash);
    }

    let header_hash = hash32(&output[0..20]);
    output[20..52].copy_from_slice(&header_hash);
    Ok(output)
}

fn build_deterministic_tar(manifest: &BundleManifest, source_root: &Path) -> Result<Vec<u8>, LbpError> {
    let mut items: Vec<_> = manifest.resources().iter().collect();
    items.sort_by(|a, b| a.source_path().as_bytes().cmp(b.source_path().as_bytes()));
    let mut builder = Builder::new(Vec::new());
    builder.follow_symlinks(false);
    for resource in items {
        let relative = Path::new(resource.source_path());
        validate_payload_path(relative)?;
        let full = source_root.join(relative);
        let metadata = fs::symlink_metadata(&full)?;
        if !metadata.file_type().is_file() { return Err(LbpError::UnsupportedEntry(relative.display().to_string())); }
        let mut file = File::open(&full)?;
        let mut data = Vec::with_capacity(metadata.len() as usize);
        file.read_to_end(&mut data)?;
        let mut header = Header::new_gnu();
        header.set_path(relative).map_err(|e| LbpError::PayloadFormat(e.to_string()))?;
        header.set_size(data.len() as u64);
        let mode = if metadata.permissions().mode() & 0o111 != 0 { 0o755 } else { 0o644 };
        header.set_mode(mode);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_username("").map_err(|e| LbpError::PayloadFormat(e.to_string()))?;
        header.set_groupname("").map_err(|e| LbpError::PayloadFormat(e.to_string()))?;
        header.set_cksum();
        builder.append(&header, Cursor::new(data)).map_err(|e| LbpError::PayloadFormat(e.to_string()))?;
    }
    builder.finish().map_err(|e| LbpError::PayloadFormat(e.to_string()))?;
    builder.into_inner().map_err(|e| LbpError::PayloadFormat(e.to_string()))
}

fn validate_payload(bytes: &[u8]) -> Result<(), LbpError> {
    let mut archive = Archive::new(Cursor::new(bytes));
    for entry in archive.entries().map_err(|e| LbpError::PayloadFormat(e.to_string()))? {
        let entry = entry.map_err(|e| LbpError::PayloadFormat(e.to_string()))?;
        let path = entry.path().map_err(|e| LbpError::PayloadFormat(e.to_string()))?.into_owned();
        validate_payload_path(&path)?;
        match entry.header().entry_type() {
            EntryType::Regular | EntryType::Directory => {}
            _ => return Err(LbpError::UnsupportedEntry(path.display().to_string())),
        }
    }
    Ok(())
}

fn validate_payload_path(path: &Path) -> Result<(), LbpError> {
    if path.is_absolute() { return Err(LbpError::PayloadPath(path.to_path_buf())); }
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir | std::path::Component::RootDir | std::path::Component::Prefix(_)) {
            return Err(LbpError::PayloadPath(path.to_path_buf()));
        }
    }
    Ok(())
}

fn decode_section(bytes: &[u8], section: &SectionInfo) -> Result<Vec<u8>, LbpError> {
    let start = usize::try_from(section.offset).map_err(|_| LbpError::NumericOverflow)?;
    let end = start.checked_add(section.compressed_length as usize).ok_or(LbpError::NumericOverflow)?;
    if end > bytes.len() { return Err(LbpError::InvalidSection); }
    let data = &bytes[start..end];
    if hash32(data) != section.content_hash { return Err(LbpError::HashMismatch); }
    let decoded = match section.compression {
        COMPRESSION_NONE => data.to_vec(),
        COMPRESSION_ZSTD => zstd::stream::decode_all(Cursor::new(data)).map_err(|e| LbpError::PayloadFormat(e.to_string()))?,
        other => return Err(LbpError::UnsupportedCompression(other)),
    };
    if decoded.len() as u64 != section.uncompressed_length { return Err(LbpError::InvalidSection); }
    Ok(decoded)
}

fn validate_range(offset: u64, length: u64, total: usize) -> Result<(), LbpError> {
    let end = offset.checked_add(length).ok_or(LbpError::NumericOverflow)?;
    if end > total as u64 || offset < HEADER_SIZE as u64 { return Err(LbpError::InvalidSection); }
    Ok(())
}

fn validate_non_overlapping(sections: &[SectionInfo]) -> Result<(), LbpError> {
    let mut ranges: Vec<(u64, u64)> = sections.iter().map(|s| (s.offset, s.offset + s.compressed_length)).collect();
    ranges.sort_unstable();
    for pair in ranges.windows(2) {
        if pair[0].1 > pair[1].0 { return Err(LbpError::InvalidSectionTable); }
    }
    Ok(())
}

fn hash32(data: &[u8]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(data);
    *hasher.finalize().as_bytes()
}

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn sample_manifest() -> BundleManifest {
        let mut manifest = BundleManifest::new(BundleMetadata::new(BundleId::from("example.app"), Version::new(1, 2, 3), BundleKind::Application));
        manifest.add_resource(BundleResource::new("/bin/app", "bin/app"));
        manifest.add_resource(BundleResource::new("/share/readme.txt", "share/readme.txt"));
        manifest
    }

    #[test]
    fn roundtrip_is_valid_and_deterministic() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("bin")).unwrap();
        fs::create_dir_all(dir.path().join("share")).unwrap();
        fs::write(dir.path().join("bin/app"), b"#!/bin/sh\necho luna\n").unwrap();
        fs::write(dir.path().join("share/readme.txt"), b"hello\n").unwrap();
        let manifest = sample_manifest();
        let first = build_from_directory(&manifest, dir.path()).unwrap();
        let second = build_from_directory(&manifest, dir.path()).unwrap();
        assert_eq!(first, second);
        let archive = LbpArchive::from_bytes(first).unwrap();
        assert_eq!(archive.manifest.metadata().version(), Version::new(1, 2, 3));
        assert_eq!(archive.manifest.resources().len(), 2);
    }

    #[test]
    fn rejects_payload_traversal() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("file"), b"x").unwrap();
        let mut manifest = sample_manifest();
        manifest.add_resource(BundleResource::new("/evil", "../evil"));
        assert!(matches!(build_from_directory(&manifest, dir.path()), Err(LbpError::Manifest(BundleError::InvalidSourcePath(_)))));
    }

    #[test]
    fn rejects_corrupt_header() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("bin")).unwrap();
        fs::create_dir_all(dir.path().join("share")).unwrap();
        fs::write(dir.path().join("bin/app"), b"x").unwrap();
        fs::write(dir.path().join("share/readme.txt"), b"x").unwrap();
        let mut bytes = build_from_directory(&sample_manifest(), dir.path()).unwrap();
        bytes[0] = b'X';
        assert!(matches!(LbpArchive::from_bytes(bytes), Err(LbpError::InvalidHeader)));
    }
}
