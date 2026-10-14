//! RFC-0002 Bundle Format v1 (`.lbp`) transport codec.
//!
//! The transport representation is separate from the Bundle domain model and
//! application lifecycle. The codec keeps the container deterministic,
//! validates every structural boundary before exposing decoded sections, and
//! rejects unsafe payload entries.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};

use blake3::Hasher;
use serde::{Deserialize, Serialize};
use tar::{Archive, Builder, EntryType, Header};

pub const MAGIC: [u8; 4] = *b"LBP1";
pub const FORMAT_VERSION: u16 = 1;
pub const HEADER_SIZE: usize = 64;
pub const SECTION_ENTRY_SIZE: usize = 64;

const HEADER_HASH_OFFSET: usize = 32;
const SECTION_MANIFEST: u32 = 1;
const SECTION_PAYLOAD: u32 = 2;
const SECTION_RESOURCES: u32 = 3;
const SECTION_SIGNATURE: u32 = 4;
const COMPRESSION_NONE: u32 = 0;
const COMPRESSION_ZSTD: u32 = 1;
const MAX_SECTION_SIZE: u64 = 1 << 30;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SectionKind {
    Manifest,
    Payload,
    Resources,
    Signature,
}

impl SectionKind {
    fn code(self) -> u32 {
        match self {
            Self::Manifest => SECTION_MANIFEST,
            Self::Payload => SECTION_PAYLOAD,
            Self::Resources => SECTION_RESOURCES,
            Self::Signature => SECTION_SIGNATURE,
        }
    }

    fn from_code(value: u32) -> Result<Self, LbpError> {
        match value {
            SECTION_MANIFEST => Ok(Self::Manifest),
            SECTION_PAYLOAD => Ok(Self::Payload),
            SECTION_RESOURCES => Ok(Self::Resources),
            SECTION_SIGNATURE => Ok(Self::Signature),
            other => Err(LbpError::UnknownSectionType(other)),
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LbpArchive {
    pub manifest: LbpManifest,
    pub sections: Vec<SectionInfo>,
    bytes: Vec<u8>,
}

impl LbpArchive {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, LbpError> {
        if bytes.len() < HEADER_SIZE || bytes[..4] != MAGIC {
            return Err(LbpError::InvalidHeader);
        }

        let version = read_u16(&bytes, 4)?;
        if version != FORMAT_VERSION {
            return Err(LbpError::UnsupportedVersion(version));
        }

        let flags = read_u16(&bytes, 6)?;
        if flags != 0 {
            return Err(LbpError::UnsupportedFlags(flags));
        }

        let count = usize::try_from(read_u32(&bytes, 8)?).map_err(|_| LbpError::NumericOverflow)?;
        let table_offset =
            usize::try_from(read_u64(&bytes, 12)?).map_err(|_| LbpError::NumericOverflow)?;
        let table_length =
            usize::try_from(read_u64(&bytes, 20)?).map_err(|_| LbpError::NumericOverflow)?;
        let header_length =
            usize::try_from(read_u32(&bytes, 28)?).map_err(|_| LbpError::NumericOverflow)?;

        let expected_table_length = count
            .checked_mul(SECTION_ENTRY_SIZE)
            .ok_or(LbpError::NumericOverflow)?;
        if header_length != HEADER_SIZE
            || table_length != expected_table_length
            || table_offset < HEADER_SIZE
        {
            return Err(LbpError::InvalidSectionTable);
        }

        let mut expected_hash = [0u8; 32];
        expected_hash.copy_from_slice(&bytes[HEADER_HASH_OFFSET..HEADER_SIZE]);
        let mut normalized = [0u8; HEADER_SIZE];
        normalized.copy_from_slice(&bytes[..HEADER_SIZE]);
        normalized[HEADER_HASH_OFFSET..HEADER_SIZE].fill(0);
        if hash32(&normalized) != expected_hash {
            return Err(LbpError::HashMismatch);
        }

        let table_end = table_offset
            .checked_add(table_length)
            .ok_or(LbpError::NumericOverflow)?;
        if table_end > bytes.len() {
            return Err(LbpError::InvalidSectionTable);
        }

        let mut sections = Vec::with_capacity(count);
        for index in 0..count {
            let start = table_offset
                .checked_add(
                    index
                        .checked_mul(SECTION_ENTRY_SIZE)
                        .ok_or(LbpError::NumericOverflow)?,
                )
                .ok_or(LbpError::NumericOverflow)?;
            let kind = SectionKind::from_code(read_u32(&bytes, start)?)?;
            let compression = read_u32(&bytes, start + 4)?;
            let offset = read_u64(&bytes, start + 8)?;
            let compressed_length = read_u64(&bytes, start + 16)?;
            let uncompressed_length = read_u64(&bytes, start + 24)?;
            let mut content_hash = [0u8; 32];
            content_hash.copy_from_slice(&bytes[start + 32..start + 64]);

            if !matches!(compression, COMPRESSION_NONE | COMPRESSION_ZSTD) {
                return Err(LbpError::UnsupportedCompression(compression));
            }
            if matches!(kind, SectionKind::Manifest | SectionKind::Signature)
                && compression != COMPRESSION_NONE
            {
                return Err(LbpError::UnsupportedCompression(compression));
            }
            validate_range(offset, compressed_length, bytes.len())?;
            if offset < table_end as u64 {
                return Err(LbpError::InvalidSectionTable);
            }
            if uncompressed_length > MAX_SECTION_SIZE {
                return Err(LbpError::ResourceLimit);
            }

            sections.push(SectionInfo {
                kind,
                compression,
                offset,
                compressed_length,
                uncompressed_length,
                content_hash,
            });
        }

        validate_non_overlapping(&sections)?;
        require_exactly_one(&sections, SectionKind::Manifest)?;
        require_exactly_one(&sections, SectionKind::Payload)?;
        require_at_most_one(&sections, SectionKind::Resources)?;
        require_at_most_one(&sections, SectionKind::Signature)?;

        let manifest_section =
            find_section(&sections, SectionKind::Manifest).ok_or(LbpError::InvalidSectionTable)?;
        let payload_section =
            find_section(&sections, SectionKind::Payload).ok_or(LbpError::InvalidSectionTable)?;

        let manifest_bytes = decode_section(&bytes, manifest_section)?;
        let payload_bytes = decode_section(&bytes, payload_section)?;
        let manifest_text = std::str::from_utf8(&manifest_bytes)
            .map_err(|_| LbpError::ManifestFormat("manifest is not UTF-8".into()))?;
        let manifest = LbpManifest::from_toml(manifest_text)?;

        validate_payload(&payload_bytes)?;
        validate_manifest_payload(&manifest, &payload_bytes)?;

        Ok(Self {
            manifest,
            sections,
            bytes,
        })
    }

    pub fn read_from_path(path: impl AsRef<Path>) -> Result<Self, LbpError> {
        Self::from_bytes(fs::read(path)?)
    }

    pub fn manifest_bytes(&self) -> Result<Vec<u8>, LbpError> {
        let section = find_section(&self.sections, SectionKind::Manifest)
            .ok_or(LbpError::InvalidSectionTable)?;
        decode_section(&self.bytes, section)
    }

    pub fn payload_bytes(&self) -> Result<Vec<u8>, LbpError> {
        let section = find_section(&self.sections, SectionKind::Payload)
            .ok_or(LbpError::InvalidSectionTable)?;
        decode_section(&self.bytes, section)
    }

    pub fn signature_bytes(&self) -> Result<Option<Vec<u8>>, LbpError> {
        match find_section(&self.sections, SectionKind::Signature) {
            Some(section) => decode_section(&self.bytes, section).map(Some),
            None => Ok(None),
        }
    }

    pub fn content_identity(&self) -> Result<[u8; 32], LbpError> {
        let manifest = self.manifest.to_toml()?.into_bytes();
        let payload = self.payload_bytes()?;
        let resources = match find_section(&self.sections, SectionKind::Resources) {
            Some(section) => decode_section(&self.bytes, section)?,
            None => Vec::new(),
        };

        let mut hasher = Hasher::new();
        hasher.update(&manifest);
        hasher.update(&payload);
        hasher.update(&resources);
        Ok(*hasher.finalize().as_bytes())
    }

    pub fn extract_payload(&self, destination: impl AsRef<Path>) -> Result<(), LbpError> {
        use std::os::unix::fs::PermissionsExt;
        let destination = destination.as_ref();
        fs::create_dir_all(destination)?;
        if !fs::symlink_metadata(destination)?.is_dir()
            || fs::read_dir(destination)?.next().is_some()
        {
            return Err(LbpError::PayloadFormat(
                "extraction requires an empty real directory".into(),
            ));
        }
        let mut archive = Archive::new(Cursor::new(self.payload_bytes()?));
        let mut seen = BTreeSet::new();

        for entry in archive.entries().map_err(payload_error)? {
            let mut entry = entry.map_err(payload_error)?;
            let path = entry.path().map_err(payload_error)?.into_owned();
            validate_payload_path(&path)?;
            if !seen.insert(path.clone()) {
                return Err(LbpError::DuplicatePayloadPath(path));
            }
            match entry.header().entry_type() {
                EntryType::Regular | EntryType::Directory => {}
                _ => {
                    return Err(LbpError::UnsupportedEntry(path.display().to_string()));
                }
            }
            // TAR's implicit directories inherit the caller's umask. Create
            // these ourselves with deterministic, safe modes instead.
            let mut parent = destination.to_path_buf();
            for component in path.parent().into_iter().flat_map(Path::components) {
                parent.push(component);
                match fs::create_dir(&parent) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error.into()),
                }
                if !fs::symlink_metadata(&parent)?.is_dir() {
                    return Err(LbpError::UnsupportedEntry(parent.display().to_string()));
                }
                fs::set_permissions(&parent, fs::Permissions::from_mode(0o755))?;
            }
            if !entry.unpack_in(destination).map_err(payload_error)? {
                return Err(LbpError::PayloadPath(path));
            }
            if entry.header().entry_type() == EntryType::Directory {
                fs::set_permissions(destination.join(path), fs::Permissions::from_mode(0o755))?;
            }
        }
        Ok(())
    }

    /// Compare the complete extracted tree with the verified transport payload.
    /// Implicit parent directories are allowed; extra files, links, special
    /// entries, changed bytes and changed modes are rejected.
    pub fn verify_extracted_payload(
        &self,
        destination: impl AsRef<Path>,
        read_only: bool,
    ) -> Result<(), LbpError> {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let destination = destination.as_ref();
        let root_metadata = fs::symlink_metadata(destination)?;
        if !root_metadata.is_dir() {
            return Err(LbpError::UnsupportedEntry(
                destination.display().to_string(),
            ));
        }
        let mut allowed = BTreeSet::new();
        let mut archive = Archive::new(Cursor::new(self.payload_bytes()?));
        for entry in archive.entries().map_err(payload_error)? {
            let mut entry = entry.map_err(payload_error)?;
            let relative = entry.path().map_err(payload_error)?.into_owned();
            for parent in relative.ancestors().filter(|p| !p.as_os_str().is_empty()) {
                allowed.insert(parent.to_path_buf());
                let metadata = fs::symlink_metadata(destination.join(parent))?;
                if parent != relative && !metadata.is_dir() {
                    return Err(LbpError::UnsupportedEntry(parent.display().to_string()));
                }
            }
            let path = destination.join(&relative);
            let metadata = fs::symlink_metadata(&path)?;
            let expected_mode = entry.header().mode().map_err(payload_error)?;
            let expected_mode = if read_only {
                expected_mode & !0o222
            } else {
                expected_mode
            };
            if metadata.permissions().mode() & 0o7777 != expected_mode {
                return Err(LbpError::PayloadFormat(format!(
                    "changed payload mode: {}",
                    relative.display()
                )));
            }
            if entry.header().entry_type() == EntryType::Directory {
                if !metadata.is_dir() {
                    return Err(LbpError::UnsupportedEntry(relative.display().to_string()));
                }
            } else {
                if !metadata.is_file() || metadata.nlink() != 1 || metadata.len() != entry.size() {
                    return Err(LbpError::PayloadFormat(format!(
                        "changed payload file: {}",
                        relative.display()
                    )));
                }
                let mut expected = Hasher::new();
                let mut actual = Hasher::new();
                io::copy(&mut entry, &mut expected)?;
                io::copy(&mut File::open(path)?, &mut actual)?;
                if expected.finalize() != actual.finalize() {
                    return Err(LbpError::HashMismatch);
                }
            }
        }
        fn walk(
            root: &Path,
            relative: &Path,
            allowed: &BTreeSet<PathBuf>,
            read_only: bool,
            owner: u32,
        ) -> Result<(), LbpError> {
            let metadata = fs::symlink_metadata(root.join(relative))?;
            let mode = metadata.permissions().mode() & 0o7777;
            let safe_mode = if read_only {
                mode == 0o555
            } else {
                matches!(mode, 0o700 | 0o750 | 0o755)
            };
            if !safe_mode || metadata.uid() != owner {
                return Err(LbpError::PayloadFormat(
                    "changed payload directory mode or owner".into(),
                ));
            }
            for entry in fs::read_dir(root.join(relative))? {
                let entry = entry?;
                let path = relative.join(entry.file_name());
                let kind = entry.file_type()?;
                if !allowed.contains(&path) || (!kind.is_file() && !kind.is_dir()) {
                    return Err(LbpError::UnsupportedEntry(path.display().to_string()));
                }
                if kind.is_dir() {
                    walk(root, &path, allowed, read_only, owner)?;
                } else if entry.metadata()?.uid() != owner {
                    return Err(LbpError::PayloadFormat("changed payload file owner".into()));
                }
            }
            Ok(())
        }
        walk(
            destination,
            Path::new(""),
            &allowed,
            read_only,
            root_metadata.uid(),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LbpManifest {
    pub format: u16,
    pub bundle: BundleManifestInfo,
    pub platform: PlatformInfo,
    pub entry: Option<EntryPoint>,
    #[serde(default, alias = "dependency")]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub capabilities: Capabilities,
    #[serde(default, alias = "mapping")]
    pub mappings: Vec<MappingDeclaration>,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleManifestInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlatformInfo {
    pub arch: String,
    pub min_system: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntryPoint {
    pub exec: String,
    pub logical: Option<String>,
    /// Неизменяемые аргументы по умолчанию перед аргументами вызова.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// Отдельный режим открытия одного выбранного файла только для чтения.
    /// Объявление запрашивает user.selected-file; разрешение выдаёт политика ОС.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_open: Option<FileOpenEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileOpenEntry {
    pub mime_types: Vec<String>,
    #[serde(default)]
    pub args: Vec<String>,
}

impl FileOpenEntry {
    pub fn validate(&self) -> Result<(), LbpError> {
        let mut unique = std::collections::BTreeSet::new();
        if self.mime_types.is_empty()
            || self.mime_types.len() > 16
            || self.mime_types.iter().any(|value| !valid_mime_type(value) || !unique.insert(value))
            || self.args.len() > 64
            || self.args.iter().any(|arg| arg.len() > 4096 || arg.contains('\0'))
        {
            return Err(LbpError::ManifestFormat("invalid entry.file_open".into()));
        }
        Ok(())
    }
}

/// Ограниченная грамматика точного MIME без параметров, шаблонов и управляющих символов.
pub fn valid_mime_type(value: &str) -> bool {
    if value.len() > 127 { return false; }
    let parts = value.split('/').collect::<Vec<_>>();
    parts.len() == 2 && parts.iter().all(|part| part.bytes().next().is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && part.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b".+-".contains(&byte)))
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Dependency {
    pub id: String,
    pub version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(default)]
    pub requested: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MappingDeclaration {
    pub logical: String,
    pub source: String,
    #[serde(default)]
    pub access: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct Metadata {
    pub author: Option<String>,
    pub license: Option<String>,
    pub homepage: Option<String>,
}

impl LbpManifest {
    pub fn from_toml(input: &str) -> Result<Self, LbpError> {
        let manifest: Self =
            toml::from_str(input).map_err(|error| LbpError::ManifestFormat(error.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn to_toml(&self) -> Result<String, LbpError> {
        self.validate()?;
        toml::to_string(self).map_err(|error| LbpError::ManifestFormat(error.to_string()))
    }

    pub fn validate(&self) -> Result<(), LbpError> {
        if let Some(file_open) = self.entry.as_ref().and_then(|entry| entry.file_open.as_ref()) {
            file_open.validate()?;
            if self.entry.as_ref().expect("file_open belongs to entry").args.len() + file_open.args.len() + 1 > 64 {
                return Err(LbpError::ManifestFormat("combined file-open arguments exceed limit".into()));
            }
        }
        if self.entry.as_ref().is_some_and(|entry| {
            entry.args.len() > 64
                || entry
                    .args
                    .iter()
                    .any(|arg| arg.len() > 4096 || arg.contains('\0'))
        }) {
            return Err(LbpError::ManifestFormat("invalid entry.args".into()));
        }
        if self.format != FORMAT_VERSION {
            return Err(LbpError::UnsupportedVersion(self.format));
        }
        if self.bundle.id.trim().is_empty() || self.bundle.name.trim().is_empty() {
            return Err(LbpError::ManifestFormat(
                "bundle.id and bundle.name must not be empty".into(),
            ));
        }
        if parse_version(&self.bundle.version).is_none() {
            return Err(LbpError::ManifestFormat(
                "bundle.version must be MAJOR.MINOR.PATCH".into(),
            ));
        }
        if !matches!(self.bundle.kind.as_str(), "application" | "component") {
            return Err(LbpError::ManifestFormat("unsupported bundle.type".into()));
        }
        if self.platform.arch != "x86_64" {
            return Err(LbpError::ManifestFormat(
                "unsupported platform.arch for LBP1".into(),
            ));
        }
        if let Some(version) = &self.platform.min_system {
            if parse_version(version).is_none() {
                return Err(LbpError::ManifestFormat(
                    "platform.min_system must be MAJOR.MINOR.PATCH".into(),
                ));
            }
        }

        if let Some(entry) = &self.entry {
            validate_bundle_relative_path(Path::new(&entry.exec))?;
            if let Some(logical) = &entry.logical {
                validate_logical_path(logical)?;
            }
        } else if self.bundle.kind == "application" {
            return Err(LbpError::ManifestFormat(
                "application bundle requires entry".into(),
            ));
        }

        let mut logicals = BTreeSet::new();
        for mapping in &self.mappings {
            if mapping
                .access
                .iter()
                .any(|access| !matches!(access.as_str(), "read" | "write" | "execute"))
            {
                return Err(LbpError::ManifestFormat(
                    "unsupported mapping access".into(),
                ));
            }
            validate_logical_path(&mapping.logical)?;
            if !mapping.source.starts_with("@dep:") {
                validate_bundle_relative_path(Path::new(&mapping.source))?;
            } else if mapping.source.len() == "@dep:".len() {
                return Err(LbpError::ManifestFormat(
                    "dependency mapping source must name a dependency".into(),
                ));
            }
            if !logicals.insert(mapping.logical.clone()) {
                return Err(LbpError::ManifestFormat(format!(
                    "duplicate mapping: {}",
                    mapping.logical
                )));
            }
        }

        for dependency in &self.dependencies {
            if dependency.id.trim().is_empty() || dependency.version.trim().is_empty() {
                return Err(LbpError::ManifestFormat(
                    "dependency id/version must not be empty".into(),
                ));
            }
            validate_dependency_constraint(&dependency.version)?;
        }

        if self
            .capabilities
            .requested
            .iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(LbpError::ManifestFormat(
                "capability name must not be empty".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum LbpError {
    Io(io::Error),
    InvalidHeader,
    UnsupportedVersion(u16),
    UnsupportedFlags(u16),
    InvalidSectionTable,
    InvalidSection,
    UnknownSectionType(u32),
    UnsupportedCompression(u32),
    HashMismatch,
    ManifestFormat(String),
    PayloadFormat(String),
    PayloadPath(PathBuf),
    DuplicatePayloadPath(PathBuf),
    UnsupportedEntry(String),
    MissingPayloadFile(PathBuf),
    NumericOverflow,
    ResourceLimit,
}

impl std::fmt::Display for LbpError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::InvalidHeader => formatter.write_str("invalid LBP1 header"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported LBP version: {version}")
            }
            Self::UnsupportedFlags(flags) => write!(formatter, "unsupported LBP flags: {flags:#x}"),
            Self::InvalidSectionTable => formatter.write_str("invalid LBP section table"),
            Self::InvalidSection => formatter.write_str("invalid LBP section"),
            Self::UnknownSectionType(kind) => write!(formatter, "unknown LBP section type: {kind}"),
            Self::UnsupportedCompression(value) => {
                write!(formatter, "unsupported LBP compression: {value}")
            }
            Self::HashMismatch => formatter.write_str("LBP content hash mismatch"),
            Self::ManifestFormat(error) => write!(formatter, "invalid manifest: {error}"),
            Self::PayloadFormat(error) => write!(formatter, "invalid payload archive: {error}"),
            Self::PayloadPath(path) => write!(formatter, "unsafe payload path: {}", path.display()),
            Self::DuplicatePayloadPath(path) => {
                write!(formatter, "duplicate payload path: {}", path.display())
            }
            Self::UnsupportedEntry(entry) => {
                write!(formatter, "unsupported payload entry: {entry}")
            }
            Self::MissingPayloadFile(path) => write!(
                formatter,
                "manifest references missing payload file: {}",
                path.display()
            ),
            Self::NumericOverflow => formatter.write_str("numeric overflow"),
            Self::ResourceLimit => formatter.write_str("resource limit exceeded"),
        }
    }
}

impl std::error::Error for LbpError {}

impl From<io::Error> for LbpError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn write_from_directory(
    path: impl AsRef<Path>,
    manifest: &LbpManifest,
    source_root: impl AsRef<Path>,
) -> Result<(), LbpError> {
    fs::write(path, build_from_directory(manifest, source_root)?)?;
    Ok(())
}

pub fn build_from_directory(
    manifest: &LbpManifest,
    source_root: impl AsRef<Path>,
) -> Result<Vec<u8>, LbpError> {
    manifest.validate()?;
    let manifest_bytes = manifest.to_toml()?.into_bytes();
    let payload = build_deterministic_tar(manifest, source_root.as_ref())?;
    let compressed = zstd::stream::encode_all(Cursor::new(&payload), 3)
        .map_err(|error| LbpError::PayloadFormat(error.to_string()))?;
    let sections = [
        (
            SectionKind::Manifest,
            COMPRESSION_NONE,
            manifest_bytes,
            None,
        ),
        (
            SectionKind::Payload,
            COMPRESSION_ZSTD,
            compressed,
            Some(payload.len() as u64),
        ),
    ];

    let count = sections.len();
    let table_length = count
        .checked_mul(SECTION_ENTRY_SIZE)
        .ok_or(LbpError::NumericOverflow)?;
    let table_end = HEADER_SIZE
        .checked_add(table_length)
        .ok_or(LbpError::NumericOverflow)?;
    let mut output = vec![0u8; table_end];
    output[..4].copy_from_slice(&MAGIC);
    output[4..6].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
    output[6..8].copy_from_slice(&0u16.to_le_bytes());
    output[8..12].copy_from_slice(&(count as u32).to_le_bytes());
    output[12..20].copy_from_slice(&(HEADER_SIZE as u64).to_le_bytes());
    output[20..28].copy_from_slice(&(table_length as u64).to_le_bytes());
    output[28..32].copy_from_slice(&(HEADER_SIZE as u32).to_le_bytes());

    let mut infos = Vec::with_capacity(count);
    let mut offset = table_end as u64;
    for (kind, compression, data, uncompressed_length) in sections {
        let info = SectionInfo {
            kind,
            compression,
            offset,
            compressed_length: data.len() as u64,
            uncompressed_length: uncompressed_length.unwrap_or(data.len() as u64),
            content_hash: hash32(&data),
        };
        output.extend_from_slice(&data);
        offset = offset
            .checked_add(data.len() as u64)
            .ok_or(LbpError::NumericOverflow)?;
        infos.push(info);
    }

    for (index, info) in infos.iter().enumerate() {
        let start = HEADER_SIZE + index * SECTION_ENTRY_SIZE;
        output[start..start + 4].copy_from_slice(&info.kind.code().to_le_bytes());
        output[start + 4..start + 8].copy_from_slice(&info.compression.to_le_bytes());
        output[start + 8..start + 16].copy_from_slice(&info.offset.to_le_bytes());
        output[start + 16..start + 24].copy_from_slice(&info.compressed_length.to_le_bytes());
        output[start + 24..start + 32].copy_from_slice(&info.uncompressed_length.to_le_bytes());
        output[start + 32..start + 64].copy_from_slice(&info.content_hash);
    }

    let mut normalized = [0u8; HEADER_SIZE];
    normalized.copy_from_slice(&output[..HEADER_SIZE]);
    normalized[HEADER_HASH_OFFSET..HEADER_SIZE].fill(0);
    let header_hash = hash32(&normalized);
    output[HEADER_HASH_OFFSET..HEADER_SIZE].copy_from_slice(&header_hash);
    Ok(output)
}

pub(crate) fn build_deterministic_tar(
    manifest: &LbpManifest,
    source_root: &Path,
) -> Result<Vec<u8>, LbpError> {
    let mut sources = BTreeSet::new();
    if let Some(entry) = &manifest.entry {
        sources.insert(PathBuf::from(&entry.exec));
    }
    for mapping in &manifest.mappings {
        if mapping.source.starts_with("@dep:") {
            continue;
        }
        collect_source_files(source_root, Path::new(&mapping.source), &mut sources)?;
    }

    let mut builder = Builder::new(Vec::new());
    builder.follow_symlinks(false);
    for relative in sources {
        validate_bundle_relative_path(&relative)?;
        let full = source_root.join(&relative);
        validate_source_ancestors(source_root, &relative)?;
        let metadata = fs::symlink_metadata(&full)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(LbpError::UnsupportedEntry(relative.display().to_string()));
        }

        let mut data = Vec::new();
        File::open(&full)?.read_to_end(&mut data)?;
        if data.len() as u64 != metadata.len() {
            return Err(LbpError::PayloadFormat(
                "source size changed while packaging".into(),
            ));
        }

        append_canonical_file(&mut builder, &relative, &metadata, Cursor::new(data))?;
    }
    builder.finish().map_err(payload_error)?;
    builder.into_inner().map_err(payload_error)
}

/// Shared encoding keeps transport and installed directory identities equal.
pub(crate) fn append_canonical_file<W: io::Write>(
    builder: &mut Builder<W>,
    relative: &Path,
    metadata: &fs::Metadata,
    data: impl Read,
) -> Result<(), LbpError> {
    validate_bundle_relative_path(relative)?;
    let mut header = Header::new_gnu();
    header.set_path(relative).map_err(payload_error)?;
    header.set_size(metadata.len());
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    #[cfg(unix)]
    header.set_mode(
        if std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o111 != 0 {
            0o755
        } else {
            0o644
        },
    );
    #[cfg(not(unix))]
    header.set_mode(0o644);
    header.set_username("").map_err(payload_error)?;
    header.set_groupname("").map_err(payload_error)?;
    header.set_cksum();
    // tar::Builder trusts the header and does not enforce the byte count.
    // Limit actual reads and reject short streams, including truncate/regrow
    // races which a second stat alone would not detect.
    let mut bounded = data.take(metadata.len());
    builder
        .append(&header, &mut bounded)
        .map_err(payload_error)?;
    if bounded.limit() != 0 {
        return Err(LbpError::PayloadFormat(
            "payload size changed while reading".into(),
        ));
    }
    Ok(())
}

fn collect_source_files(
    root: &Path,
    relative: &Path,
    output: &mut BTreeSet<PathBuf>,
) -> Result<(), LbpError> {
    validate_bundle_relative_path(relative)?;
    let full = root.join(relative);
    validate_source_ancestors(root, relative)?;
    let metadata = fs::symlink_metadata(&full)?;
    if metadata.file_type().is_symlink() {
        return Err(LbpError::UnsupportedEntry(relative.display().to_string()));
    }
    if metadata.is_file() {
        output.insert(relative.to_path_buf());
        return Ok(());
    }
    if metadata.is_dir() {
        let mut entries = fs::read_dir(&full)?.collect::<Result<Vec<_>, io::Error>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            collect_source_files(root, &relative.join(entry.file_name()), output)?;
        }
        return Ok(());
    }
    Err(LbpError::UnsupportedEntry(relative.display().to_string()))
}

fn validate_manifest_payload(manifest: &LbpManifest, payload: &[u8]) -> Result<(), LbpError> {
    let mut paths = BTreeSet::new();
    let mut archive = Archive::new(Cursor::new(payload));
    for entry in archive.entries().map_err(payload_error)? {
        let entry = entry.map_err(payload_error)?;
        paths.insert(entry.path().map_err(payload_error)?.into_owned());
    }

    if let Some(entry) = &manifest.entry {
        if !paths.contains(Path::new(&entry.exec)) {
            return Err(LbpError::MissingPayloadFile(PathBuf::from(&entry.exec)));
        }
    }

    for mapping in &manifest.mappings {
        if mapping.source.starts_with("@dep:") {
            continue;
        }
        let source = Path::new(&mapping.source);
        if !paths.contains(source) && !paths.iter().any(|path| path.starts_with(source)) {
            return Err(LbpError::MissingPayloadFile(PathBuf::from(&mapping.source)));
        }
    }
    Ok(())
}

fn validate_payload(bytes: &[u8]) -> Result<(), LbpError> {
    let mut archive = Archive::new(Cursor::new(bytes));
    let mut seen = BTreeSet::new();
    for entry in archive.entries().map_err(payload_error)? {
        let entry = entry.map_err(payload_error)?;
        let path = entry.path().map_err(payload_error)?.into_owned();
        validate_payload_path(&path)?;
        if !seen.insert(path.clone()) {
            return Err(LbpError::DuplicatePayloadPath(path));
        }
        match entry.header().entry_type() {
            EntryType::Regular | EntryType::Directory => {}
            _ => return Err(LbpError::UnsupportedEntry(path.display().to_string())),
        }
        let mode = entry.header().mode().map_err(payload_error)?;
        if !matches!(mode, 0o644 | 0o755)
            || (entry.header().entry_type() == EntryType::Directory && mode != 0o755)
        {
            return Err(LbpError::PayloadFormat(
                "unsafe or non-canonical payload mode".into(),
            ));
        }
    }
    Ok(())
}

fn validate_payload_path(path: &Path) -> Result<(), LbpError> {
    if path.is_absolute() || path.as_os_str().is_empty() || has_navigation(path) {
        return Err(LbpError::PayloadPath(path.to_path_buf()));
    }
    for component in path.components() {
        if matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
                | std::path::Component::CurDir
        ) {
            return Err(LbpError::PayloadPath(path.to_path_buf()));
        }
    }
    Ok(())
}

fn validate_logical_path(value: &str) -> Result<(), LbpError> {
    let path = Path::new(value);
    if !path.is_absolute() || value.ends_with('/') || has_navigation(path) {
        return Err(LbpError::ManifestFormat(
            "logical path must be absolute and must not end with /".into(),
        ));
    }
    for component in path.components() {
        if matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::CurDir
                | std::path::Component::Prefix(_)
        ) {
            return Err(LbpError::ManifestFormat(
                "logical path contains forbidden traversal".into(),
            ));
        }
    }
    Ok(())
}

fn validate_bundle_relative_path(path: &Path) -> Result<(), LbpError> {
    if path.as_os_str().is_empty() || path.is_absolute() || has_navigation(path) {
        return Err(LbpError::ManifestFormat(format!(
            "invalid bundle-relative path: {}",
            path.display()
        )));
    }
    for component in path.components() {
        if matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
                | std::path::Component::CurDir
        ) {
            return Err(LbpError::ManifestFormat(format!(
                "unsafe bundle-relative path: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn has_navigation(path: &Path) -> bool {
    path.to_string_lossy()
        .split('/')
        .any(|part| matches!(part, "." | ".."))
}

fn validate_source_ancestors(root: &Path, relative: &Path) -> Result<(), LbpError> {
    if !fs::symlink_metadata(root)?.is_dir() {
        return Err(LbpError::UnsupportedEntry(root.display().to_string()));
    }
    let mut path = root.to_path_buf();
    for component in relative.components() {
        path.push(component);
        if fs::symlink_metadata(&path)?.file_type().is_symlink() {
            return Err(LbpError::UnsupportedEntry(path.display().to_string()));
        }
    }
    Ok(())
}

fn validate_dependency_constraint(value: &str) -> Result<(), LbpError> {
    let value = value.trim();
    let stripped = value
        .strip_prefix('^')
        .or_else(|| value.strip_prefix('~'))
        .or_else(|| value.strip_prefix(">="))
        .or_else(|| value.strip_prefix("<="))
        .or_else(|| value.strip_prefix('>'))
        .or_else(|| value.strip_prefix('<'))
        .unwrap_or(value);
    if parse_version(stripped).is_none() {
        return Err(LbpError::ManifestFormat(
            "unsupported dependency version constraint".into(),
        ));
    }
    Ok(())
}

fn parse_version(value: &str) -> Option<(u32, u32, u32)> {
    let mut parts = value.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        None
    } else {
        Some((major, minor, patch))
    }
}

fn find_section(sections: &[SectionInfo], kind: SectionKind) -> Option<&SectionInfo> {
    sections.iter().find(|section| section.kind == kind)
}

fn require_exactly_one(sections: &[SectionInfo], kind: SectionKind) -> Result<(), LbpError> {
    if sections
        .iter()
        .filter(|section| section.kind == kind)
        .count()
        == 1
    {
        Ok(())
    } else {
        Err(LbpError::InvalidSectionTable)
    }
}

fn require_at_most_one(sections: &[SectionInfo], kind: SectionKind) -> Result<(), LbpError> {
    if sections
        .iter()
        .filter(|section| section.kind == kind)
        .count()
        <= 1
    {
        Ok(())
    } else {
        Err(LbpError::InvalidSectionTable)
    }
}

fn validate_range(offset: u64, length: u64, total: usize) -> Result<(), LbpError> {
    let end = offset
        .checked_add(length)
        .ok_or(LbpError::NumericOverflow)?;
    if end > total as u64 {
        Err(LbpError::InvalidSection)
    } else {
        Ok(())
    }
}

fn validate_non_overlapping(sections: &[SectionInfo]) -> Result<(), LbpError> {
    let mut ranges = Vec::with_capacity(sections.len());
    for section in sections {
        let end = section
            .offset
            .checked_add(section.compressed_length)
            .ok_or(LbpError::NumericOverflow)?;
        ranges.push((section.offset, end));
    }
    ranges.sort_unstable();
    for pair in ranges.windows(2) {
        if pair[0].1 > pair[1].0 {
            return Err(LbpError::InvalidSectionTable);
        }
    }
    Ok(())
}

fn decode_section(bytes: &[u8], section: &SectionInfo) -> Result<Vec<u8>, LbpError> {
    let start = usize::try_from(section.offset).map_err(|_| LbpError::NumericOverflow)?;
    let len = usize::try_from(section.compressed_length).map_err(|_| LbpError::NumericOverflow)?;
    let end = start.checked_add(len).ok_or(LbpError::NumericOverflow)?;
    if end > bytes.len() {
        return Err(LbpError::InvalidSection);
    }

    let stored = &bytes[start..end];
    if hash32(stored) != section.content_hash {
        return Err(LbpError::HashMismatch);
    }

    let decoded = match section.compression {
        COMPRESSION_NONE => stored.to_vec(),
        COMPRESSION_ZSTD => {
            let decoder =
                zstd::stream::read::Decoder::new(Cursor::new(stored)).map_err(payload_error)?;
            let mut decoded = Vec::new();
            decoder
                .take(section.uncompressed_length + 1)
                .read_to_end(&mut decoded)
                .map_err(payload_error)?;
            decoded
        }
        other => return Err(LbpError::UnsupportedCompression(other)),
    };
    if decoded.len() as u64 != section.uncompressed_length {
        return Err(LbpError::InvalidSection);
    }
    if decoded.len() as u64 > MAX_SECTION_SIZE {
        return Err(LbpError::ResourceLimit);
    }
    Ok(decoded)
}

fn read_exact<const N: usize>(bytes: &[u8], start: usize) -> Result<[u8; N], LbpError> {
    let end = start.checked_add(N).ok_or(LbpError::NumericOverflow)?;
    if end > bytes.len() {
        return Err(LbpError::InvalidHeader);
    }
    bytes[start..end]
        .try_into()
        .map_err(|_| LbpError::InvalidHeader)
}

fn read_u16(bytes: &[u8], start: usize) -> Result<u16, LbpError> {
    Ok(u16::from_le_bytes(read_exact::<2>(bytes, start)?))
}

fn read_u32(bytes: &[u8], start: usize) -> Result<u32, LbpError> {
    Ok(u32::from_le_bytes(read_exact::<4>(bytes, start)?))
}

fn read_u64(bytes: &[u8], start: usize) -> Result<u64, LbpError> {
    Ok(u64::from_le_bytes(read_exact::<8>(bytes, start)?))
}

fn payload_error<E: std::fmt::Display>(error: E) -> LbpError {
    LbpError::PayloadFormat(error.to_string())
}

fn hash32(data: &[u8]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(data);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before UNIX epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("luna-lbp-v1-{stamp}"));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn manifest() -> LbpManifest {
        LbpManifest {
            format: FORMAT_VERSION,
            bundle: BundleManifestInfo {
                id: "org.example.app".into(),
                name: "Example App".into(),
                version: "1.2.3".into(),
                kind: "application".into(),
            },
            platform: PlatformInfo {
                arch: "x86_64".into(),
                min_system: Some("1.0.0".into()),
            },
            entry: Some(EntryPoint {
                exec: "bin/app".into(),
                logical: Some("/usr/bin/app".into()),
                args: Vec::new(),
                file_open: None,
            }),
            dependencies: Vec::new(),
            capabilities: Capabilities::default(),
            mappings: Vec::new(),
            metadata: Metadata::default(),
        }
    }

    fn fixture() -> PathBuf {
        let root = temp_dir();
        fs::create_dir_all(root.join("bin")).expect("create bin");
        fs::write(root.join("bin/app"), b"#!/bin/sh\necho luna\n").expect("write fixture");
        root
    }

    #[test]
    fn canonical_tar_rejects_short_stream_and_bounds_extra_bytes() {
        let root = fixture();
        let metadata = fs::metadata(root.join("bin/app")).unwrap();
        let mut builder = Builder::new(Vec::new());
        assert!(
            append_canonical_file(
                &mut builder,
                Path::new("bin/app"),
                &metadata,
                Cursor::new(b"short")
            )
            .is_err()
        );
        let bytes = vec![b'x'; metadata.len() as usize + 100];
        let mut input = Cursor::new(bytes);
        let mut builder = Builder::new(Vec::new());
        append_canonical_file(&mut builder, Path::new("bin/app"), &metadata, &mut input).unwrap();
        assert_eq!(input.position(), metadata.len());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn header_is_exactly_64_bytes_and_uses_little_endian_fields() {
        let root = fixture();
        let bytes = build_from_directory(&manifest(), &root).expect("build bundle");
        assert_eq!(&bytes[..4], b"LBP1");
        assert_eq!(
            u16::from_le_bytes(bytes[4..6].try_into().expect("header version")),
            1
        );
        assert_eq!(
            u16::from_le_bytes(bytes[6..8].try_into().expect("flags")),
            0
        );
        assert_eq!(
            u32::from_le_bytes(bytes[8..12].try_into().expect("section count")),
            2
        );
        assert_eq!(
            u64::from_le_bytes(bytes[12..20].try_into().expect("table offset")),
            64
        );
        assert_eq!(
            u64::from_le_bytes(bytes[20..28].try_into().expect("table length")),
            128
        );
        assert_eq!(
            u32::from_le_bytes(bytes[28..32].try_into().expect("header length")),
            64
        );
        assert_ne!(&bytes[32..64], &[0u8; 32]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn manifest_roundtrips_and_content_identity_is_stable() {
        let root = fixture();
        let manifest = manifest();
        let first = build_from_directory(&manifest, &root).expect("build first");
        let second = build_from_directory(&manifest, &root).expect("build second");
        assert_eq!(first, second);
        let parsed = LbpArchive::from_bytes(first).expect("parse bundle");
        assert_eq!(parsed.manifest, manifest);
        assert_eq!(
            parsed
                .content_identity()
                .expect("stable content identity")
                .len(),
            32
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn corrupt_header_hash_is_rejected() {
        let root = fixture();
        let mut bytes = build_from_directory(&manifest(), &root).expect("build bundle");
        bytes[32] ^= 1;
        assert!(matches!(
            LbpArchive::from_bytes(bytes),
            Err(LbpError::HashMismatch)
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unsupported_architecture_is_rejected() {
        let mut manifest = manifest();
        manifest.platform.arch = "aarch64".into();
        assert!(matches!(
            manifest.validate(),
            Err(LbpError::ManifestFormat(message)) if message.contains("platform.arch")
        ));
    }

    #[test]
    fn unsupported_dependency_syntax_is_rejected() {
        let mut manifest = manifest();
        manifest.dependencies.push(Dependency {
            id: "org.example.dep".into(),
            version: "^1.2.0 || ^2.0.0".into(),
        });
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn extraction_preserves_payload_bytes() {
        let root = fixture();
        let destination = temp_dir();
        let archive =
            LbpArchive::from_bytes(build_from_directory(&manifest(), &root).expect("build bundle"))
                .expect("parse bundle");
        archive
            .extract_payload(&destination)
            .expect("extract payload");
        assert_eq!(
            fs::read(destination.join("bin/app")).expect("read extracted payload"),
            b"#!/bin/sh\necho luna\n"
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(destination);
    }
}
