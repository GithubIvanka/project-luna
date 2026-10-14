//! Directory Bundle v1. A suffix is a discovery hint; this reader validates the
//! actual format and RFC-0002 ContentIdentity. It grants neither installation
//! status nor execution permission. No transport archive is stored here.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{CString, OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tar::Builder;

use crate::lbp::{
    LbpArchive, LbpError, LbpManifest, SectionKind, append_canonical_file, build_deterministic_tar,
};

pub const APPLICATION_SUFFIX: &str = ".lunaapp";
const FORMAT: &str = "luna-directory-bundle";
const MAX_METADATA: u64 = 1024 * 1024;
const MAX_PAYLOAD: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct DirectoryBundle {
    pub manifest: LbpManifest,
    pub content_identity: [u8; 32],
    pub payload_root: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Integrity {
    format: String,
    layout_version: u16,
    payload_encoding: String,
    content_identity: String,
}

/// Opaque private storage handle, never an application filename or identity.
pub fn storage_slot(id: &str, version: &str) -> String {
    let mut hash = blake3::Hasher::new();
    hash.update(b"luna-installed-slot-v1\0");
    hash.update(id.as_bytes());
    hash.update(b"\0");
    hash.update(version.as_bytes());
    hash.finalize().to_hex().to_string()
}

/// Ordinary basename derived from the friendly name; ID/version stay in TOML.
/// Explicit inspection also accepts renamed directories with valid contents.
pub fn application_filename(manifest: &LbpManifest) -> String {
    let mut name = String::new();
    for character in manifest.bundle.name.chars().take(80) {
        if character.is_ascii_alphanumeric() {
            name.push(character.to_ascii_lowercase());
        } else if !name.is_empty() && !name.ends_with('-') {
            name.push('-');
        }
    }
    let name = name.trim_end_matches('-');
    format!(
        "{}{APPLICATION_SUFFIX}",
        if name.is_empty() { "application" } else { name }
    )
}

fn problem(message: impl Into<String>) -> LbpError {
    LbpError::PayloadFormat(message.into())
}

fn read_metadata(root: &File, name: &str, owner: u32) -> Result<String, LbpError> {
    let file = open_at(root, OsStr::new(name), false)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != owner
        || metadata.mode() & 0o7777 != 0o444
    {
        return Err(problem("unsafe or changed directory Bundle metadata"));
    }
    let mut text = String::new();
    file.take(MAX_METADATA + 1).read_to_string(&mut text)?;
    if text.len() as u64 > MAX_METADATA {
        return Err(LbpError::ResourceLimit);
    }
    Ok(text)
}

/// Create in an empty operator-owned staging directory. The caller handles
/// publication, permissions, fsync and transaction registration.
pub fn materialize_archive(archive: &LbpArchive, root: &Path) -> Result<(), LbpError> {
    if archive.manifest.bundle.kind != "application" {
        return Err(problem(
            "directory application v1 requires bundle.type=application",
        ));
    }
    if archive
        .sections
        .iter()
        .any(|s| matches!(s.kind, SectionKind::Resources | SectionKind::Signature))
    {
        return Err(problem(
            "directory v1 does not support separate RESOURCES or SIGNATURE sections",
        ));
    }
    if !fs::symlink_metadata(root)?.is_dir() || fs::read_dir(root)?.next().is_some() {
        return Err(problem(
            "directory Bundle staging must be an empty real directory",
        ));
    }
    let payload = root.join("payload");
    fs::create_dir(&payload)?;
    fs::set_permissions(&payload, fs::Permissions::from_mode(0o755))?;
    archive.extract_payload(&payload)?;
    archive.verify_extracted_payload(&payload, false)?;
    // Preserve the exact RFC identity, including canonical TAR metadata/order
    // and padding. Never silently turn a valid but different TAR into this v1.
    if build_deterministic_tar(&archive.manifest, &payload)? != archive.payload_bytes()? {
        return Err(problem(
            "directory v1 requires the Luna canonical TAR payload; source identity was not changed",
        ));
    }
    let integrity = Integrity {
        format: FORMAT.into(),
        layout_version: 1,
        payload_encoding: "luna-canonical-tar-v1".into(),
        content_identity: hex(&archive.content_identity()?),
    };
    for (name, bytes) in [
        ("manifest.toml", archive.manifest.to_toml()?),
        (
            "integrity.toml",
            toml::to_string(&integrity).map_err(|e| problem(e.to_string()))?,
        ),
    ] {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(root.join(name))?;
        file.write_all(bytes.as_bytes())?;
        file.sync_all()?;
    }
    Ok(())
}

impl DirectoryBundle {
    /// Inspect an immutable real directory. Recognition validates the complete
    /// inventory, owners, modes, links, metadata and payload, without any LBP.
    pub fn read(root: impl AsRef<Path>) -> Result<Self, LbpError> {
        let root = root.as_ref();
        // Pin every path component: neither parent nor leaf symlink races may
        // redirect inspection. Native Luna provides /proc for fd enumeration.
        let mut pinned = File::open(if root.is_absolute() { "/" } else { "." })?;
        for component in root.components() {
            match component {
                std::path::Component::Normal(name) => pinned = open_at(&pinned, name, true)?,
                std::path::Component::RootDir | std::path::Component::CurDir => {}
                _ => return Err(problem("directory Bundle path contains navigation")),
            }
        }
        let metadata = pinned.metadata()?;
        if !metadata.is_dir() || metadata.mode() & 0o7777 != 0o555 {
            return Err(problem(
                "directory Bundle must be a read-only real directory",
            ));
        }
        let owner = metadata.uid();
        let manifest_text = read_metadata(&pinned, "manifest.toml", owner)?;
        let manifest = LbpManifest::from_toml(&manifest_text)?;
        if manifest.bundle.kind != "application" || manifest.to_toml()? != manifest_text {
            return Err(problem(
                "directory v1 requires a canonical application manifest",
            ));
        }
        let integrity: Integrity =
            toml::from_str(&read_metadata(&pinned, "integrity.toml", owner)?)
                .map_err(|e| problem(e.to_string()))?;
        if integrity.format != FORMAT
            || integrity.layout_version != 1
            || integrity.payload_encoding != "luna-canonical-tar-v1"
        {
            return Err(problem("unsupported directory Bundle format"));
        }
        let mut names = directory_names(&pinned)?
            .into_iter()
            .take(4)
            .collect::<Vec<_>>();
        names.sort();
        if names != ["integrity.toml", "manifest.toml", "payload"].map(std::ffi::OsString::from) {
            return Err(problem("unexpected directory Bundle entries"));
        }
        let payload_root = root.join("payload");
        let payload = open_at(&pinned, OsStr::new("payload"), true)?;
        let snapshot = snapshot_tree(&payload, owner)?;
        let mut selected = BTreeSet::new();
        if let Some(entry) = &manifest.entry {
            selected.insert(PathBuf::from(&entry.exec));
        }
        for mapping in &manifest.mappings {
            if mapping.source.starts_with("@dep:") {
                continue;
            }
            let source = Path::new(&mapping.source);
            let paths = snapshot
                .files
                .keys()
                .filter(|p| *p == source || p.starts_with(source))
                .cloned()
                .collect::<Vec<_>>();
            if paths.is_empty() {
                return Err(LbpError::MissingPayloadFile(source.to_path_buf()));
            }
            selected.extend(paths);
        }
        let mut allowed = BTreeSet::new();
        for path in &selected {
            for parent in path.ancestors().filter(|p| !p.as_os_str().is_empty()) {
                allowed.insert(parent.to_path_buf());
            }
        }
        if snapshot
            .files
            .keys()
            .chain(snapshot.directories.iter())
            .any(|p| !allowed.contains(p))
        {
            return Err(problem("unmapped directory Bundle entry"));
        }
        let mut hash = blake3::Hasher::new();
        hash.update(manifest_text.as_bytes());
        let mut builder = Builder::new(hash);
        for relative in selected {
            let expected = snapshot
                .files
                .get(&relative)
                .ok_or_else(|| problem("missing entry executable"))?;
            let file = open_payload_file(&payload, &relative, owner)?;
            check_snapshot(&file.metadata()?, expected)?;
            // TAR consumes exactly this bounded size and streams into BLAKE3;
            // no payload-sized Vec is allocated and growth cannot extend read.
            append_canonical_file(
                &mut builder,
                &relative,
                expected,
                (&file).take(expected.len()),
            )?;
            check_snapshot(&file.metadata()?, expected)?;
        }
        builder.finish()?;
        let hash = builder.into_inner()?;
        let content_identity = *hash.finalize().as_bytes();
        if integrity.content_identity != hex(&content_identity) {
            return Err(LbpError::HashMismatch);
        }
        Ok(Self {
            manifest,
            content_identity,
            payload_root,
        })
    }
}

fn open_at(parent: &File, name: &OsStr, directory: bool) -> Result<File, LbpError> {
    let name = CString::new(name.as_bytes()).map_err(|_| problem("invalid Bundle filename"))?;
    let flags = libc::O_CLOEXEC
        | libc::O_NOFOLLOW
        | if directory {
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NONBLOCK
        } else {
            libc::O_PATH
        };
    // SAFETY: valid live parent fd and NUL-terminated CString. Successful fd
    // ownership transfers to File exactly once; error descriptors are not used.
    let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let pinned = unsafe { File::from_raw_fd(fd) };
    if directory {
        return Ok(pinned);
    }
    let metadata = pinned.metadata()?;
    if !metadata.is_file() && !metadata.is_dir() {
        return Err(problem("links and special Bundle entries are forbidden"));
    }
    // O_PATH pins the inode without opening a device/FIFO first. Only a known
    // regular file/directory is reopened through the kernel-owned fd link.
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(format!("/proc/self/fd/{}", pinned.as_raw_fd()))?;
    let opened = file.metadata()?;
    if opened.dev() != metadata.dev() || opened.ino() != metadata.ino() {
        return Err(problem("pinned Bundle inode changed"));
    }
    Ok(file)
}

fn directory_names(directory: &File) -> Result<Vec<OsString>, LbpError> {
    let mut names = Vec::new();
    // Kernel-owned proc fd link denotes the pinned inode, not a user path.
    for entry in fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))? {
        if names.len() >= 100_000 {
            return Err(LbpError::ResourceLimit);
        }
        names.push(entry?.file_name());
    }
    Ok(names)
}

#[derive(Default)]
struct Snapshot {
    files: BTreeMap<PathBuf, fs::Metadata>,
    directories: BTreeSet<PathBuf>,
    bytes: u64,
}

fn check_directory(file: &File, owner: u32) -> Result<(), LbpError> {
    let metadata = file.metadata()?;
    if !metadata.is_dir() || metadata.uid() != owner || metadata.mode() & 0o7777 != 0o555 {
        return Err(problem("unsafe Bundle directory or changed owner/mode"));
    }
    Ok(())
}

fn snapshot_tree(root: &File, owner: u32) -> Result<Snapshot, LbpError> {
    fn walk(
        root: &File,
        relative: &Path,
        owner: u32,
        snapshot: &mut Snapshot,
        depth: usize,
    ) -> Result<(), LbpError> {
        check_directory(root, owner)?;
        if depth > 128 || snapshot.files.len() + snapshot.directories.len() > 100_000 {
            return Err(LbpError::ResourceLimit);
        }
        for name in directory_names(root)? {
            let file = open_at(root, &name, false)?;
            let metadata = file.metadata()?;
            let path = relative.join(name);
            if metadata.is_dir() {
                snapshot.directories.insert(path.clone());
                walk(&file, &path, owner, snapshot, depth + 1)?;
            } else if metadata.is_file()
                && metadata.nlink() == 1
                && metadata.uid() == owner
                && matches!(metadata.mode() & 0o7777, 0o444 | 0o555)
            {
                snapshot.bytes = snapshot
                    .bytes
                    .checked_add(metadata.len())
                    .ok_or(LbpError::ResourceLimit)?;
                if snapshot.bytes > MAX_PAYLOAD {
                    return Err(LbpError::ResourceLimit);
                }
                snapshot.files.insert(path, metadata);
            } else {
                return Err(problem("unsafe Bundle entry or changed mode/owner"));
            }
            if snapshot.files.len() + snapshot.directories.len() > 100_000 {
                return Err(LbpError::ResourceLimit);
            }
        }
        Ok(())
    }
    let mut snapshot = Snapshot::default();
    walk(root, Path::new(""), owner, &mut snapshot, 0)?;
    Ok(snapshot)
}

fn open_payload_file(root: &File, relative: &Path, owner: u32) -> Result<File, LbpError> {
    let mut directory = root.try_clone()?;
    for name in relative.parent().into_iter().flat_map(Path::components) {
        directory = open_at(&directory, name.as_os_str(), true)?;
        check_directory(&directory, owner)?;
    }
    open_at(
        &directory,
        relative
            .file_name()
            .ok_or_else(|| problem("empty payload path"))?,
        false,
    )
}

fn check_snapshot(actual: &fs::Metadata, expected: &fs::Metadata) -> Result<(), LbpError> {
    if !actual.is_file()
        || actual.nlink() != 1
        || actual.dev() != expected.dev()
        || actual.ino() != expected.ino()
        || actual.uid() != expected.uid()
        || actual.mode() != expected.mode()
        || actual.len() != expected.len()
    {
        return Err(problem("Bundle file changed during inspection"));
    }
    Ok(())
}

fn hex(identity: &[u8; 32]) -> String {
    identity.iter().map(|byte| format!("{byte:02x}")).collect()
}
