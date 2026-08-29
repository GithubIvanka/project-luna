//! Linux namespace materialization primitives.
//!
//! This backend uses Linux mount namespaces and bind mounts as implementation
//! primitives. It contains no Luna authorization policy: callers must provide
//! a mapping table that has already passed the appropriate policy checks.

use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::{MappingError, MappingTable, PhysicalPath};

#[derive(Debug)]
pub enum LinuxNamespaceError {
    Io(io::Error),
    Mapping(MappingError),
    InvalidCString,
}

impl std::fmt::Display for LinuxNamespaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "Linux namespace operation failed: {error}"),
            Self::Mapping(error) => write!(f, "mapping error: {error}"),
            Self::InvalidCString => f.write_str("path contains an interior NUL byte"),
        }
    }
}

impl std::error::Error for LinuxNamespaceError {}

impl From<io::Error> for LinuxNamespaceError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<MappingError> for LinuxNamespaceError {
    fn from(error: MappingError) -> Self {
        Self::Mapping(error)
    }
}

/// A private Linux mount namespace entered by the current process.
///
/// The caller should create this in a dedicated child/process context shortly
/// before materializing an application and executing it. The original mount
/// namespace is not restored by this type.
#[derive(Debug, Clone, Copy, Default)]
pub struct LinuxMountNamespace;

impl LinuxMountNamespace {
    /// Enter a new private mount namespace for the calling process.
    pub fn enter_private() -> Result<Self, LinuxNamespaceError> {
        // SAFETY: `unshare` affects only the calling process.
        if unsafe { libc::unshare(libc::CLONE_NEWNS) } == -1 {
            return Err(io::Error::last_os_error().into());
        }

        // Prevent subsequent mount operations from propagating back to the
        // parent namespace through shared mount propagation.
        if mount(None, Path::new("/"), libc::MS_REC | libc::MS_PRIVATE)? == -1 {
            return Err(io::Error::last_os_error().into());
        }

        Ok(Self)
    }

    /// Apply validated mappings as read-only bind mounts.
    ///
    /// This function expects destination paths to already exist. Construction
    /// of the complete logical `/` tree remains a higher-level runtime concern.
    pub fn apply_read_only_mappings(
        &self,
        table: &MappingTable,
    ) -> Result<(), LinuxNamespaceError> {
        for rule in table.iter() {
            if mount(Some(rule.physical().as_path()), rule.logical().as_path(), libc::MS_BIND)? == -1 {
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

fn to_cstring(path: &Path) -> Result<CString, LinuxNamespaceError> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| LinuxNamespaceError::InvalidCString)
}

fn mount(
    source: Option<&Path>,
    target: &Path,
    flags: libc::c_ulong,
) -> Result<libc::c_int, LinuxNamespaceError> {
    let source = source.map(to_cstring).transpose()?;
    let target = to_cstring(target)?;

    // SAFETY: all string pointers remain valid for the duration of the syscall
    // and are NUL-terminated. `fstype` and `data` are intentionally null for
    // bind/remount/private mount operations.
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
    use super::LinuxNamespaceError;

    #[test]
    fn error_is_descriptive() {
        assert_eq!(
            LinuxNamespaceError::InvalidCString.to_string(),
            "path contains an interior NUL byte"
        );
    }
}
