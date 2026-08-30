//! Linux namespace backend for Project Luna.
//!
//! This crate contains operating-system-specific namespace mechanics. Luna
//! policy and logical path resolution remain owned by `luna-security` and
//! `luna-root-mapping` respectively.

use std::ffi::CString;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use luna_root_mapping::{MappingRule, MappingTable};

#[derive(Debug)]
pub enum NamespaceError {
    Io(io::Error),
    InvalidPath,
    MissingBaseRoot,
}

impl std::fmt::Display for NamespaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "namespace operation failed: {error}"),
            Self::InvalidPath => f.write_str("path contains an interior NUL byte"),
            Self::MissingBaseRoot => f.write_str("logical root base directory does not exist"),
        }
    }
}

impl std::error::Error for NamespaceError {}

impl From<io::Error> for NamespaceError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// A process-local Linux mount namespace.
///
/// This object represents an already-created namespace. It deliberately does
/// not own application policy, process lifecycle, or logical path resolution.
#[derive(Debug, Clone, Copy, Default)]
pub struct LinuxMountNamespace;

/// Conventional directories that make the application root look like a normal
/// Linux filesystem even when Luna only exposes selected physical resources.
const STANDARD_ROOT_DIRS: &[&str] = &[
    "bin", "boot", "dev", "etc", "home", "lib", "lib64", "media", "mnt", "opt", "proc",
    "root", "run", "sbin", "srv", "sys", "tmp", "usr", "var",
];

impl LinuxMountNamespace {
    /// Create a private mount namespace for the calling process.
    ///
    /// Call this in the dedicated child that will become the application's
    /// process tree. The mount namespace is intentionally not restored.
    pub fn enter_private() -> Result<Self, NamespaceError> {
        // SAFETY: unshare(2) affects only the calling process.
        if unsafe { libc::unshare(libc::CLONE_NEWNS) } == -1 {
            return Err(io::Error::last_os_error().into());
        }

        // Make the copied root mount tree private so later mount changes do
        // not propagate back into the parent namespace.
        if mount(None, Path::new("/"), None, libc::MS_REC | libc::MS_PRIVATE)? == -1 {
            return Err(io::Error::last_os_error().into());
        }

        Ok(Self)
    }

    /// Prepare a complete Linux-compatible logical root below `root`.
    ///
    /// `base_root` is normally the mounted Luna System Image. It is mounted as
    /// the base filesystem and then the validated mapping table overlays the
    /// selected DATA-backed resources. The caller must perform security
    /// authorization before invoking this method.
    pub fn materialize_logical_root(
        &self,
        root: &Path,
        base_root: &Path,
        mappings: &MappingTable,
    ) -> Result<LogicalRoot, NamespaceError> {
        if !base_root.is_dir() {
            return Err(NamespaceError::MissingBaseRoot);
        }

        fs::create_dir_all(root)?;
        for directory in STANDARD_ROOT_DIRS {
            fs::create_dir_all(root.join(directory))?;
        }

        // The System Image becomes the conventional Linux-compatible base /
        // while DATA resources are layered on top through explicit mappings.
        bind_mount(base_root, root, true)?;

        // The base image may not contain all mount targets expected by the
        // mapping table, so create their parents before applying the mounts.
        for rule in mappings.iter() {
            prepare_mount_target(root, rule)?;
        }
        self.apply_read_only_mappings(mappings)?;

        // proc/sys are standard kernel views rather than DATA mappings. They
        // are mounted inside the private namespace and therefore do not alter
        // the host namespace.
        mount_proc(&root.join("proc"))?;
        mount_sysfs(&root.join("sys"))?;

        // /dev is intentionally an empty private tmpfs. Device-manager policy
        // can later bind only explicitly authorized device nodes into it.
        mount_tmpfs(&root.join("dev"), "mode=0755")?;

        Ok(LogicalRoot { path: root.to_path_buf() })
    }

    /// Materialize the supplied validated mapping table as read-only bind
    /// mounts. This is the conservative primitive used when the caller has
    /// not authorized write access to the backing resource.
    pub fn apply_read_only_mappings(&self, mappings: &MappingTable) -> Result<(), NamespaceError> {
        for rule in mappings.iter() {
            self.apply_mapping(rule, true)?;
        }
        Ok(())
    }

    /// Apply one already-authorized mapping.
    ///
    /// `read_only=false` is intentionally an explicit low-level choice: the
    /// security layer must make the corresponding authorization decision before
    /// the runtime asks this backend to create a writable mapping.
    pub fn apply_mapping(&self, rule: &MappingRule, read_only: bool) -> Result<(), NamespaceError> {
        let target = rule.logical().as_path();
        let source = rule.physical().as_path();

        match rule.kind() {
            luna_root_mapping::MappingKind::File => {
                if !source.is_file() {
                    return Err(io::Error::new(io::ErrorKind::NotFound, "file mapping source is not a file").into());
                }
                if !target.exists() {
                    if let Some(parent) = target.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::File::create(target)?;
                }
            }
            luna_root_mapping::MappingKind::Subtree => {
                if !source.is_dir() {
                    return Err(io::Error::new(io::ErrorKind::NotFound, "subtree mapping source is not a directory").into());
                }
                fs::create_dir_all(target)?;
            }
        }

        bind_mount(source, target, read_only)
    }

    /// Switch the calling process into the prepared logical root.
    ///
    /// The caller should invoke this after all namespace mounts have been
    /// materialized and immediately before launching the application process.
    pub fn enter_logical_root(&self, root: &LogicalRoot) -> Result<(), NamespaceError> {
        if !root.path.is_dir() {
            return Err(NamespaceError::MissingBaseRoot);
        }

        let root = CString::new(root.path.as_os_str().as_bytes()).map_err(|_| NamespaceError::InvalidPath)?;
        // SAFETY: the path is NUL-free and points to the prepared root.
        if unsafe { libc::chroot(root.as_ptr()) } == -1 {
            return Err(io::Error::last_os_error().into());
        }
        std::env::set_current_dir("/")?;
        Ok(())
    }

    /// Bind one authorized device node into the private /dev view.
    pub fn expose_device(&self, source: &Path, logical_name: &Path, read_only: bool) -> Result<(), NamespaceError> {
        if !source.exists() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "device source does not exist").into());
        }
        let target = Path::new("/dev").join(logical_name);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        if !target.exists() {
            fs::File::create(&target)?;
        }
        bind_mount(source, &target, read_only)
    }
}

/// Prepared logical application root.
#[derive(Debug, Clone)]
pub struct LogicalRoot {
    path: PathBuf,
}

impl LogicalRoot {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn prepare_mount_target(root: &Path, rule: &MappingRule) -> Result<(), NamespaceError> {
    let target = root.join(rule.logical().as_str().trim_start_matches('/'));
    match rule.kind() {
        luna_root_mapping::MappingKind::File => {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            if !target.exists() {
                fs::File::create(target)?;
            }
        }
        luna_root_mapping::MappingKind::Subtree => fs::create_dir_all(target)?,
    }
    Ok(())
}

fn bind_mount(source: &Path, target: &Path, read_only: bool) -> Result<(), NamespaceError> {
    if mount(Some(source), target, None, libc::MS_BIND)? == -1 {
        return Err(io::Error::last_os_error().into());
    }

    if read_only
        && mount(
            None,
            target,
            None,
            libc::MS_BIND | libc::MS_REMOUNT | libc::MS_RDONLY,
        )? == -1
    {
        return Err(io::Error::last_os_error().into());
    }

    Ok(())
}

fn mount_proc(target: &Path) -> Result<(), NamespaceError> {
    mount(Some(Path::new("proc")), target, Some(Path::new("proc")), libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC)
        .map(|status| if status == -1 { Err(io::Error::last_os_error()) } else { Ok(()) })??;
    Ok(())
}

fn mount_sysfs(target: &Path) -> Result<(), NamespaceError> {
    mount(Some(Path::new("sysfs")), target, Some(Path::new("sysfs")), libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC | libc::MS_RDONLY)
        .map(|status| if status == -1 { Err(io::Error::last_os_error()) } else { Ok(()) })??;
    Ok(())
}

fn mount_tmpfs(target: &Path, options: &str) -> Result<(), NamespaceError> {
    mount(Some(Path::new("tmpfs")), target, Some(Path::new("tmpfs")), libc::MS_NOSUID | libc::MS_NODEV, Some(options))
        .map(|status| if status == -1 { Err(io::Error::last_os_error()) } else { Ok(()) })??;
    Ok(())
}

fn cstring(path: &Path) -> Result<CString, NamespaceError> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| NamespaceError::InvalidPath)
}

fn mount(
    source: Option<&Path>,
    target: &Path,
    filesystem: Option<&Path>,
    flags: libc::c_ulong,
) -> Result<libc::c_int, NamespaceError> {
    mount_with_data(source, target, filesystem, flags, None)
}

fn mount_with_data(
    source: Option<&Path>,
    target: &Path,
    filesystem: Option<&Path>,
    flags: libc::c_ulong,
    data: Option<&str>,
) -> Result<libc::c_int, NamespaceError> {
    let source = source.map(cstring).transpose()?;
    let target = cstring(target)?;
    let filesystem = filesystem.map(cstring).transpose()?;
    let data = data.map(|value| CString::new(value).map_err(|_| NamespaceError::InvalidPath)).transpose()?;

    // SAFETY: all C strings remain alive until the syscall returns; null
    // pointers are valid for bind/remount operations.
    Ok(unsafe {
        libc::mount(
            source.as_ref().map_or(std::ptr::null(), |value| value.as_ptr()),
            target.as_ptr(),
            filesystem.as_ref().map_or(std::ptr::null(), |value| value.as_ptr()),
            flags,
            data.as_ref().map_or(std::ptr::null(), |value| value.as_ptr().cast()),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::NamespaceError;

    #[test]
    fn invalid_path_has_stable_message() {
        assert_eq!(
            NamespaceError::InvalidPath.to_string(),
            "path contains an interior NUL byte"
        );
    }

    #[test]
    fn missing_base_root_has_stable_message() {
        assert_eq!(
            NamespaceError::MissingBaseRoot.to_string(),
            "logical root base directory does not exist"
        );
    }
}
