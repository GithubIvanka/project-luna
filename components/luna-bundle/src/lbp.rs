//! Bundle Format v1 (`.lbp`) transport codec candidate.
//!
//! The transport format is deliberately kept separate from the Bundle domain
//! model and from application lifecycle management. This module can read and
//! write an LBP1 container, while `luna-app-manager` owns installation and
//! lifecycle decisions.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};

use blake3::Hasher;
use serde::{Deserialize, Serialize};
use tar::{Archive, Builder, EntryType, Header};

pub const MAGIC: [u8; 4] = *b"LBP1";
pub const FORMAT_VERSION: u16 = 1;
pub const HEADER_SIZE: usize = 52;
pub const SECTION_ENTRY_SIZE: usize = 64;

const SECTION_MANIFEST: u32 = 1;
const SECTION_PAYLOAD: u32 = 2;
const SECTION_RESOURCES: u32 = 3;
const SECTION_SIGNATURE: u32 = 4;
const COMPRESSION_NONE: u32 = 0;
const COMPRESSION_ZSTD: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SectionKind { Manifest, Payload, Resources, Signature }

impl SectionKind {
    fn code(self) -> u32 { match self { Self::Manifest => SECTION_MANIFEST, Self::Payload => SECTION_PAYLOAD, Self::Resources => SECTION_RESOURCES, Self::Signature => SECTION_SIGNATURE } }
    fn from_code(code: u32) -> Result<Self, LbpError> { match code { SECTION_MANIFEST => Ok(Self::Manifest), SECTION_PAYLOAD => Ok(Self::Payload), SECTION_RESOURCES => Ok(Self::Resources), SECTION_SIGNATURE => Ok(Self::Signature), _ => Err(LbpError::UnknownSectionType(code)) } }
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LbpArchive {
    pub manifest: LbpManifest,
    pub sections: Vec<SectionInfo>,
    bytes: Vec<u8>,
}

impl LbpArchive {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, LbpError> {
        if bytes.len() < HEADER_SIZE || bytes[0..4] != MAGIC { return Err(LbpError::InvalidHeader); }
        let version = u16::from_le_bytes(bytes[4..6].try_into().map_err(|_| LbpError::InvalidHeader)?);
        if version != FORMAT_VERSION { return Err(LbpError::UnsupportedVersion(version)); }
        let flags = u16::from_le_bytes(bytes[6..8].try_into().map_err(|_| LbpError::InvalidHeader)?);
        if flags != 0 { return Err(LbpError::UnsupportedFlags(flags)); }
        let section_count = u32::from_le_bytes(bytes[8..12].try_into().map_err(|_| LbpError::InvalidHeader)?) as usize;
        let table_offset = usize::try_from(u64::from_le_bytes(bytes[12..20].try_into().map_err(|_| LbpError::InvalidHeader)?)).map_err(|_| LbpError::NumericOverflow)?;

        let mut normalized_header = [0u8; HEADER_SIZE];
        normalized_header.copy_from_slice(&bytes[..HEADER_SIZE]);
        let expected_hash = normalized_header[20..52].to_owned();
        normalized_header[20..52].fill(0);
        if hash32(&normalized_header) != expected_hash.as_slice() { return Err(LbpError::HashMismatch); }

        let table_len = section_count.checked_mul(SECTION_ENTRY_SIZE).ok_or(LbpError::NumericOverflow)?;
        let table_end = table_offset.checked_add(table_len).ok_or(LbpError::NumericOverflow)?;
        if table_offset < HEADER_SIZE || table_end > bytes.len() { return Err(LbpError::InvalidSectionTable); }

        let mut sections = Vec::with_capacity(section_count);
        for index in 0..section_count {
            let start = table_offset + index * SECTION_ENTRY_SIZE;
            let kind = SectionKind::from_code(u32::from_le_bytes(bytes[start..start + 4].try_into().unwrap()))?;
            let compression = u32::from_le_bytes(bytes[start + 4..start + 8].try_into().unwrap());
            let offset = u64::from_le_bytes(bytes[start + 8..start + 16].try_into().unwrap());
            let compressed_length = u64::from_le_bytes(bytes[start + 16..start + 24].try_into().unwrap());
            let uncompressed_length = u64::from_le_bytes(bytes[start + 24..start + 32].try_into().unwrap());
            let mut content_hash = [0u8; 32];
            content_hash.copy_from_slice(&bytes[start + 32..start + 64]);
            match compression { COMPRESSION_NONE | COMPRESSION_ZSTD => {}, other => return Err(LbpError::UnsupportedCompression(other)) }
            validate_range(offset, compressed_length, bytes.len())?;
            if offset < table_end as u64 { return Err(LbpError::InvalidSectionTable); }
            sections.push(SectionInfo { kind, compression, offset, compressed_length, uncompressed_length, content_hash });
        }

        validate_non_overlapping(&sections)?;
        require_exactly_one(&sections, SectionKind::Manifest)?;
        require_exactly_one(&sections, SectionKind::Payload)?;
        require_at_most_one(&sections, SectionKind::Resources)?;
        require_at_most_one(&sections, SectionKind::Signature)?;

        let manifest_section = find_section(&sections, SectionKind::Manifest).unwrap();
        let payload_section = find_section(&sections, SectionKind::Payload).unwrap();
        let manifest_bytes = decode_section(&bytes, manifest_section)?;
        let payload_bytes = decode_section(&bytes, payload_section)?;
        let manifest_text = std::str::from_utf8(&manifest_bytes).map_err(|_| LbpError::ManifestFormat("manifest is not UTF-8".into()))?;
        let manifest = LbpManifest::from_toml(manifest_text)?;
        validate_payload(&payload_bytes)?;
        Ok(Self { manifest, sections, bytes })
    }

    pub fn read_from_path(path: impl AsRef<Path>) -> Result<Self, LbpError> { Self::from_bytes(fs::read(path)?) }

    pub fn manifest_bytes(&self) -> Result<Vec<u8>, LbpError> { decode_section(&self.bytes, find_section(&self.sections, SectionKind::Manifest).ok_or(LbpError::InvalidSectionTable)?) }
    pub fn payload_bytes(&self) -> Result<Vec<u8>, LbpError> { decode_section(&self.bytes, find_section(&self.sections, SectionKind::Payload).ok_or(LbpError::InvalidSectionTable)?) }

    pub fn extract_payload(&self, destination: impl AsRef<Path>) -> Result<(), LbpError> {
        let destination = destination.as_ref();
        fs::create_dir_all(destination)?;
        let payload = self.payload_bytes()?;
        let mut archive = Archive::new(Cursor::new(payload));
        let mut seen = BTreeSet::new();
        for entry in archive.entries().map_err(|e| LbpError::PayloadFormat(e.to_string()))? {
            let mut entry = entry.map_err(|e| LbpError::PayloadFormat(e.to_string()))?;
            let path = entry.path().map_err(|e| LbpError::PayloadFormat(e.to_string()))?.into_owned();
            validate_payload_path(&path)?;
            if !seen.insert(path.clone()) { return Err(LbpError::DuplicatePayloadPath(path)); }
            match entry.header().entry_type() { EntryType::Regular | EntryType::Directory => {}, _ => return Err(LbpError::UnsupportedEntry(path.display().to_string())) }
            entry.unpack_in(destination).map_err(|e| LbpError::PayloadFormat(e.to_string()))?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LbpManifest {
    pub format: u16,
    pub bundle: BundleManifestInfo,
    pub platform: PlatformInfo,
    pub entry: Option<EntryPoint>,
    #[serde(default)] pub dependencies: Vec<Dependency>,
    #[serde(default)] pub capabilities: Capabilities,
    #[serde(default)] pub mappings: Vec<MappingDeclaration>,
    #[serde(default)] pub metadata: Metadata,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleManifestInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(rename = "type")] pub kind: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlatformInfo { pub arch: String, pub min_system: Option<String> }

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntryPoint { pub exec: String, pub logical: Option<String> }

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Dependency { pub id: String, pub version: String }

#[derive(Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct Capabilities { #[serde(default)] pub requested: Vec<String> }

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MappingDeclaration { pub logical: String, pub source: String, #[serde(default)] pub access: Vec<String> }

#[derive(Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct Metadata { pub author: Option<String>, pub license: Option<String>, pub homepage: Option<String> }

impl LbpManifest {
    pub fn from_toml(input: &str) -> Result<Self, LbpError> {
        let manifest: Self = toml::from_str(input).map_err(|e| LbpError::ManifestFormat(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn to_toml(&self) -> Result<String, LbpError> {
        self.validate()?;
        toml::to_string(self).map_err(|e| LbpError::ManifestFormat(e.to_string()))
    }

    pub fn validate(&self) -> Result<(), LbpError> {
        if self.format != FORMAT_VERSION { return Err(LbpError::UnsupportedVersion(self.format)); }
        if self.bundle.id.trim().is_empty() || self.bundle.name.trim().is_empty() { return Err(LbpError::ManifestFormat("bundle id/name must not be empty".into())); }
        if parse_version(&self.bundle.version).is_none() { return Err(LbpError::ManifestFormat("bundle.version must be MAJOR.MINOR.PATCH".into())); }
        if !matches!(self.bundle.kind.as_str(), "application" | "component") { return Err(LbpError::ManifestFormat("unsupported bundle.type".into())); }
        if self.platform.arch.trim().is_empty() { return Err(LbpError::ManifestFormat("platform.arch must not be empty".into())); }
        if let Some(version) = &self.platform.min_system { if parse_version(version).is_none() { return Err(LbpError::ManifestFormat("platform.min_system must be MAJOR.MINOR.PATCH".into())); } }
        if let Some(entry) = &self.entry { validate_bundle_relative_path(Path::new(&entry.exec))?; if let Some(logical) = &entry.logical { validate_logical_path(logical)?; } } else if self.bundle.kind == "application" { return Err(LbpError::ManifestFormat("application bundle requires entry".into())); }
        let mut logicals = BTreeSet::new();
        for mapping in &self.mappings {
            validate_logical_path(&mapping.logical)?;
            validate_bundle_relative_path(Path::new(&mapping.source))?;
            if !logicals.insert(mapping.logical.clone()) { return Err(LbpError::ManifestFormat(format!("duplicate mapping: {}", mapping.logical))); }
        }
        for dependency in &self.dependencies {
            if dependency.id.trim().is_empty() || dependency.version.trim().is_empty() { return Err(LbpError::ManifestFormat("dependency id/version must not be empty".into())); }
            if !valid_constraint_chars(&dependency.version) { return Err(LbpError::ManifestFormat("unsupported dependency constraint".into())); }
        }
        if self.capabilities.requested.iter().any(|value| value.trim().is_empty()) { return Err(LbpError::ManifestFormat("capability name must not be empty".into())); }
        Ok(())
    }
}

#[derive(Debug)]
pub enum LbpError {
    Io(io::Error), InvalidHeader, UnsupportedVersion(u16), UnsupportedFlags(u16), InvalidSectionTable,
    InvalidSection, UnknownSectionType(u32), UnsupportedCompression(u32), HashMismatch, ManifestFormat(String),
    PayloadFormat(String), PayloadPath(PathBuf), DuplicatePayloadPath(PathBuf), UnsupportedEntry(String), NumericOverflow,
}

impl std::fmt::Display for LbpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"), Self::InvalidHeader => f.write_str("invalid LBP1 header"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported LBP format version: {v}"), Self::UnsupportedFlags(v) => write!(f, "unsupported LBP flags: {v:#x}"),
            Self::InvalidSectionTable => f.write_str("invalid LBP section table"), Self::InvalidSection => f.write_str("invalid LBP section"),
            Self::UnknownSectionType(v) => write!(f, "unknown LBP section type: {v}"), Self::UnsupportedCompression(v) => write!(f, "unsupported LBP compression: {v}"),
            Self::HashMismatch => f.write_str("LBP content hash mismatch"), Self::ManifestFormat(e) => write!(f, "invalid manifest: {e}"),
            Self::PayloadFormat(e) => write!(f, "invalid payload archive: {e}"), Self::PayloadPath(p) => write!(f, "unsafe payload path: {}", p.display()),
            Self::DuplicatePayloadPath(p) => write!(f, "duplicate payload path: {}", p.display()), Self::UnsupportedEntry(p) => write!(f, "unsupported payload entry: {p}"),
            Self::NumericOverflow => f.write_str("numeric overflow in LBP structure"),
        }
    }
}
impl std::error::Error for LbpError {}
impl From<io::Error> for LbpError { fn from(value: io::Error) -> Self { Self::Io(value) } }

pub fn write_from_directory(path: impl AsRef<Path>, manifest: &LbpManifest, source_root: impl AsRef<Path>) -> Result<(), LbpError> { fs::write(path, build_from_directory(manifest, source_root)?)?; Ok(()) }

pub fn build_from_directory(manifest: &LbpManifest, source_root: impl AsRef<Path>) -> Result<Vec<u8>, LbpError> {
    manifest.validate()?;
    let manifest_bytes = manifest.to_toml()?.into_bytes();
    let payload = build_deterministic_tar(manifest, source_root.as_ref())?;
    let compressed = zstd::stream::encode_all(Cursor::new(&payload), 3).map_err(|e| LbpError::PayloadFormat(e.to_string()))?;
    let sections = [(SectionKind::Manifest, COMPRESSION_NONE, manifest_bytes, None), (SectionKind::Payload, COMPRESSION_ZSTD, compressed, Some(payload.len() as u64))];
    let count = sections.len();
    let data_offset = HEADER_SIZE.checked_add(count.checked_mul(SECTION_ENTRY_SIZE).ok_or(LbpError::NumericOverflow)?).ok_or(LbpError::NumericOverflow)?;
    let mut output = vec![0u8; data_offset];
    output[0..4].copy_from_slice(&MAGIC); output[4..6].copy_from_slice(&FORMAT_VERSION.to_le_bytes()); output[6..8].copy_from_slice(&0u16.to_le_bytes());
    output[8..12].copy_from_slice(&(count as u32).to_le_bytes()); output[12..20].copy_from_slice(&(HEADER_SIZE as u64).to_le_bytes());
    let mut infos = Vec::with_capacity(count); let mut cursor = data_offset as u64;
    for (kind, compression, data, uncompressed) in sections {
        let info = SectionInfo { kind, compression, offset: cursor, compressed_length: data.len() as u64, uncompressed_length: uncompressed.unwrap_or(data.len() as u64), content_hash: hash32(&data) };
        cursor = cursor.checked_add(data.len() as u64).ok_or(LbpError::NumericOverflow)?; infos.push(info); output.extend_from_slice(&data);
    }
    for (index, info) in infos.iter().enumerate() {
        let start = HEADER_SIZE + index * SECTION_ENTRY_SIZE; output[start..start + 4].copy_from_slice(&info.kind.code().to_le_bytes());
        output[start + 4..start + 8].copy_from_slice(&info.compression.to_le_bytes()); output[start + 8..start + 16].copy_from_slice(&info.offset.to_le_bytes());
        output[start + 16..start + 24].copy_from_slice(&info.compressed_length.to_le_bytes()); output[start + 24..start + 32].copy_from_slice(&info.uncompressed_length.to_le_bytes());
        output[start + 32..start + 64].copy_from_slice(&info.content_hash);
    }
    let header_hash = hash32(&output[..20]); output[20..52].copy_from_slice(&header_hash); Ok(output)
}

fn build_deterministic_tar(manifest: &LbpManifest, source_root: &Path) -> Result<Vec<u8>, LbpError> {
    let mut mappings = manifest.mappings.clone();
    if let Some(entry) = &manifest.entry { if let Some(logical) = &entry.logical { if !mappings.iter().any(|m| m.logical == *logical && m.source == entry.exec) { mappings.push(MappingDeclaration { logical: logical.clone(), source: entry.exec.clone(), access: vec!["execute".into()] }); } } }
    mappings.sort_by(|a, b| a.source.as_bytes().cmp(b.source.as_bytes()));
    let mut seen = BTreeSet::new(); let mut builder = Builder::new(Vec::new()); builder.follow_symlinks(false);
    for mapping in mappings {
        let relative = Path::new(&mapping.source); validate_bundle_relative_path(relative)?;
        if !seen.insert(mapping.source.clone()) { return Err(LbpError::DuplicatePayloadPath(relative.to_path_buf())); }
        let full = source_root.join(relative); let metadata = fs::symlink_metadata(&full)?;
        if !metadata.file_type().is_file() { return Err(LbpError::UnsupportedEntry(relative.display().to_string())); }
        let mut data = Vec::new(); File::open(&full)?.read_to_end(&mut data)?;
        let mut header = Header::new_gnu(); header.set_path(relative).map_err(|e| LbpError::PayloadFormat(e.to_string()))?; header.set_size(data.len() as u64);
        #[cfg(unix)] let mode = if std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o111 != 0 { 0o755 } else { 0o644 };
        #[cfg(not(unix))] let mode = 0o644;
        header.set_mode(mode); header.set_uid(0); header.set_gid(0); header.set_mtime(0); header.set_username("").map_err(|e| LbpError::PayloadFormat(e.to_string()))?; header.set_groupname("").map_err(|e| LbpError::PayloadFormat(e.to_string()))?; header.set_cksum();
        builder.append(&header, Cursor::new(data)).map_err(|e| LbpError::PayloadFormat(e.to_string()))?;
    }
    builder.finish().map_err(|e| LbpError::PayloadFormat(e.to_string()))?; builder.into_inner().map_err(|e| LbpError::PayloadFormat(e.to_string()))
}

fn validate_payload(bytes: &[u8]) -> Result<(), LbpError> {
    let mut archive = Archive::new(Cursor::new(bytes)); let mut seen = BTreeSet::new();
    for entry in archive.entries().map_err(|e| LbpError::PayloadFormat(e.to_string()))? {
        let entry = entry.map_err(|e| LbpError::PayloadFormat(e.to_string()))?; let path = entry.path().map_err(|e| LbpError::PayloadFormat(e.to_string()))?.into_owned();
        validate_payload_path(&path)?; if !seen.insert(path.clone()) { return Err(LbpError::DuplicatePayloadPath(path)); }
        match entry.header().entry_type() { EntryType::Regular | EntryType::Directory => {}, _ => return Err(LbpError::UnsupportedEntry(path.display().to_string())) }
    }
    Ok(())
}

fn validate_payload_path(path: &Path) -> Result<(), LbpError> {
    if path.is_absolute() { return Err(LbpError::PayloadPath(path.to_path_buf())); }
    for component in path.components() { if matches!(component, std::path::Component::ParentDir | std::path::Component::RootDir | std::path::Component::Prefix(_)) { return Err(LbpError::PayloadPath(path.to_path_buf())); } }
    Ok(())
}

fn validate_logical_path(value: &str) -> Result<(), LbpError> {
    let path = Path::new(value); if !path.is_absolute() || value.ends_with('/') { return Err(LbpError::ManifestFormat("logical path must be absolute and must not end with /".into())); }
    for component in path.components() { if matches!(component, std::path::Component::ParentDir | std::path::Component::Prefix(_)) { return Err(LbpError::ManifestFormat("logical path contains forbidden traversal".into())); } }
    Ok(())
}

fn validate_bundle_relative_path(path: &Path) -> Result<(), LbpError> { if path.is_absolute() { return Err(LbpError::ManifestFormat(format!("bundle-relative path is absolute: {}", path.display()))); } for component in path.components() { if matches!(component, std::path::Component::ParentDir | std::path::Component::RootDir | std::path::Component::Prefix(_)) { return Err(LbpError::ManifestFormat(format!("unsafe bundle-relative path: {}", path.display()))); } } Ok(()) }
fn valid_constraint_chars(value: &str) -> bool { value.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '^' | '~' | '>' | '<' | '=' | '*' | '|' | ',' | '-' | '+' | ' ')) }
fn parse_version(value: &str) -> Option<(u32, u32, u32)> { let mut parts = value.split('.'); let major = parts.next()?.parse().ok()?; let minor = parts.next()?.parse().ok()?; let patch = parts.next()?.parse().ok()?; if parts.next().is_some() { None } else { Some((major, minor, patch)) } }
fn find_section(sections: &[SectionInfo], kind: SectionKind) -> Option<&SectionInfo> { sections.iter().find(|section| section.kind == kind) }
fn require_exactly_one(sections: &[SectionInfo], kind: SectionKind) -> Result<(), LbpError> { if sections.iter().filter(|section| section.kind == kind).count() == 1 { Ok(()) } else { Err(LbpError::InvalidSectionTable) } }
fn require_at_most_one(sections: &[SectionInfo], kind: SectionKind) -> Result<(), LbpError> { if sections.iter().filter(|section| section.kind == kind).count() <= 1 { Ok(()) } else { Err(LbpError::InvalidSectionTable) } }
fn validate_range(offset: u64, length: u64, total: usize) -> Result<(), LbpError> { let end = offset.checked_add(length).ok_or(LbpError::NumericOverflow)?; if end > total as u64 { Err(LbpError::InvalidSection) } else { Ok(()) } }
fn validate_non_overlapping(sections: &[SectionInfo]) -> Result<(), LbpError> { let mut ranges = Vec::with_capacity(sections.len()); for section in sections { let end = section.offset.checked_add(section.compressed_length).ok_or(LbpError::NumericOverflow)?; ranges.push((section.offset, end)); } ranges.sort_unstable(); for pair in ranges.windows(2) { if pair[0].1 > pair[1].0 { return Err(LbpError::InvalidSectionTable); } } Ok(()) }
fn decode_section(bytes: &[u8], section: &SectionInfo) -> Result<Vec<u8>, LbpError> { let start = usize::try_from(section.offset).map_err(|_| LbpError::NumericOverflow)?; let len = usize::try_from(section.compressed_length).map_err(|_| LbpError::NumericOverflow)?; let end = start.checked_add(len).ok_or(LbpError::NumericOverflow)?; if end > bytes.len() { return Err(LbpError::InvalidSection); } let compressed = &bytes[start..end]; if hash32(compressed) != section.content_hash { return Err(LbpError::HashMismatch); } let decoded = match section.compression { COMPRESSION_NONE => compressed.to_vec(), COMPRESSION_ZSTD => zstd::stream::decode_all(Cursor::new(compressed)).map_err(|e| LbpError::PayloadFormat(e.to_string()))?, value => return Err(LbpError::UnsupportedCompression(value)) }; if decoded.len() as u64 != section.uncompressed_length { return Err(LbpError::InvalidSection); } Ok(decoded) }
fn hash32(data: &[u8]) -> [u8; 32] { let mut hasher = Hasher::new(); hasher.update(data); *hasher.finalize().as_bytes() }

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    fn temp_dir() -> PathBuf { let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(); let path = std::env::temp_dir().join(format!("luna-lbp-{stamp}-{}", std::process::id())); fs::create_dir_all(&path).unwrap(); path }
    fn sample_manifest() -> LbpManifest { LbpManifest { format: FORMAT_VERSION, bundle: BundleManifestInfo { id: "org.example.app".into(), name: "Example App".into(), version: "1.2.3".into(), kind: "application".into() }, platform: PlatformInfo { arch: "x86_64".into(), min_system: Some("1.0.0".into()) }, entry: Some(EntryPoint { exec: "bin/app".into(), logical: Some("/usr/bin/app".into()) }), dependencies: vec![], capabilities: Capabilities { requested: vec!["network".into()] }, mappings: vec![], metadata: Metadata { author: Some("Luna".into()), license: Some("Apache-2.0".into()), homepage: None } } }
    #[test] fn manifest_roundtrips_toml() { let m = sample_manifest(); let text = m.to_toml().unwrap(); assert_eq!(m, LbpManifest::from_toml(&text).unwrap()); }
    #[test] fn archive_is_deterministic() { let dir = temp_dir(); fs::create_dir_all(dir.join("bin")).unwrap(); fs::write(dir.join("bin/app"), b"#!/bin/sh\necho luna\n").unwrap(); let m = sample_manifest(); let a = build_from_directory(&m, &dir).unwrap(); let b = build_from_directory(&m, &dir).unwrap(); assert_eq!(a, b); assert_eq!(LbpArchive::from_bytes(a).unwrap().manifest, m); let _ = fs::remove_dir_all(dir); }
    #[test] fn rejects_corrupt_header() { let dir = temp_dir(); fs::create_dir_all(dir.join("bin")).unwrap(); fs::write(dir.join("bin/app"), b"x").unwrap(); let mut bytes = build_from_directory(&sample_manifest(), &dir).unwrap(); bytes[0] = b'X'; assert!(matches!(LbpArchive::from_bytes(bytes), Err(LbpError::InvalidHeader))); let _ = fs::remove_dir_all(dir); }
    #[test] fn rejects_manifest_traversal() { let mut m = sample_manifest(); m.entry = Some(EntryPoint { exec: "../app".into(), logical: Some("/usr/bin/app".into()) }); assert!(m.validate().is_err()); }
    #[test] fn rejects_unknown_format_version() { let mut m = sample_manifest(); m.format = 2; assert!(matches!(m.validate(), Err(LbpError::UnsupportedVersion(2)))); }
}
