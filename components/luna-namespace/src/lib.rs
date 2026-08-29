//! Linux namespace backend for Project Luna.
//!
//! This crate contains operating-system-specific namespace mechanics. Luna
//! policy and logical path resolution remain owned by `luna-security` and
//! `luna-root-mapping` respectively.

use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use luna_root_mapping::MappingTable;

#[derive(Debug)]
pub enum NamespaceError {
    Io(io::Error),
    InvalidPath,
}

impl std::fmt::Display for NamespaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "namespace operation failed: {error}"),
            Self::InvalidPath => f.write_str("path contains an interior NUL byte"),
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
        if mount(None, Path::new("/"), libc::MS_REC | libc::MS_PRIVATE)? == -1 {
            return Err(io::Error::last_os_error().into());
        }

        Ok(Self)
    }

    /// Materialize the supplied validated mapping table as read-only bind
    /// mounts.
    ///
    /// The logical destination paths must already exist in the runtime's
    /// prepared Linux-compatible root. Creating that tree and enforcing Luna
    /// authorization are higher-level responsibilities.
    pub fn apply_read_only_mappings(
        &self,
        mappings: &MappingTable,
    ) -> Result<(), NamespaceError> {
        for rule in mappings.iter() {
            if mount(
                Some(rule.physical().as_path()),
                rule.logical().as_path(),
                libc::MS_BIND,
            )? == -1
            {
                return Err(io::Error::last_os_error().into());
            }

            if mount(
                None,
                rule.logical().as_path(),
                libc::MS_BIND | libc::MS_REMOUNT | libc::MS_RDONLY,
            )? == -1
            {
                return Err(io::Error::last_os_error().into());
            }
        }

        Ok(())
    }
}

fn cstring(path: &Path) -> Result<CString, NamespaceError> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| NamespaceError::InvalidPath)
}

fn mount(
    source: Option<&Path>,
    target: &Path,
    flags: libc::c_ulong,
) -> Result<libc::c_int, NamespaceError> {
    let source = source.map(cstring).transpose()?;
    let target = cstring(target)?;

    // SAFETY: the C strings live until the syscall returns; null fstype/data
    // pointers are valid for bind/remount/private mount operations.
    Ok(unsafe {
        libc::mount(
            source.as_ref().map_or(std::ptr::null(), |value| value.as_ptr()),
            target.as_ptr(),
            std::ptr::null(),
            flags,
            std::ptr::null(),
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
}
