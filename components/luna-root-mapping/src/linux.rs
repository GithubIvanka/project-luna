//! Linux namespace materialization primitives.
//!
//! This backend uses the Linux mount namespace and bind-mount mechanisms as
//! implementation primitives. It deliberately does not contain Luna policy:
//! callers must provide an already validated `MappingTable` and any security
//! authorization must have happened before materialization.

use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::{MappingError, MappingRule, MappingTable, PhysicalPath};

#[derive(Debug)]
pub enum LinuxNamespaceError {
    Io(io::Error),
    Mapping(MappingError),
    InvalidCString,
    Unsupported,
}

impl std::fmt::Display for LinuxNamespaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "Linux namespace operation failed: {error}"),
            Self::Mapping(error) => write!(f, "mapping error: {error}"),
            Self::InvalidCString => f.write_str("path contains an interior NUL byte"),
            Self::Unsupported => f.write_str("Linux namespace backend is unavailable"),
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

/// A materialized Linux mount namespace prepared for a subsequent exec.
///
/// The type does not attempt to restore the caller's original namespace. It is
/// intended to be used in a dedicated child/process context before launching
/// an application.
#[derive(Debug, Clone, Copy, Default)]
pub struct LinuxMountNamespace;

impl LinuxMountNamespace {
    /// Unshares the caller into a private mount namespace and prevents mount
    /// propagation back into the parent namespace.
    pub fn unshare_private() -> Result<Self, LinuxNamespaceError> {
        if unshare(libc::CLONE_NEWNS) != 0 {
            return Err(io::Error::last_os_error().into());
        }

        if mount(
            None,
            Path::new("/"),
            libc::MS_REC | libc::MS_PRIVATE,
        ) != 0
        {
            return Err(io::Error::last_os_error().into());
        }

        Ok(Self)
    }

    /// Applies validated mappings as read-only bind mounts by default.
    ///
    /// Destination paths must already exist. Creating the logical root tree is
    /// deliberately outside this low-level operation so a future materializer
    /// can decide which portions originate from the system image, user data,
    /// application bundle, or cache before mounts are applied.
    pub fn apply_read_only_mappings(
        &self,
        table: &MappingTable,
    ) -> Result<(), LinuxNamespaceError> {
        for rule in table.iter() {
            if rule.mapping_kind().is_subtree_or_file() {
                bind_mount_read_only(rule.logical().as_path(), rule.physical())?;
            }
        }
        Ok(())
    }
}

fn bind_mount_read_only(
    logical: &Path,
    physical: &PhysicalPath,
) -> Result<(), LinuxNamespaceError> {
    if mount(Some(physical.as_path()), logical, libc::MS_BIND) != 0 {
        return Err(io::Error::last_os_error().into());
    }

    // MS_REMOUNT is used after bind-mounting because MS_BIND alone does not
    // make the underlying bind mount read-only.
    if mount(
        None,
        logical,
        libc::MS_BIND | libc::MS_REMOUNT | libc::MS_RDONLY,
    ) != 0
    {
        return Err(io::Error::last_os_error().into());
    }

    Ok(())
}

fn to_cstring(path: &Path) -> Result<CString, LinuxNamespaceError> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| LinuxNamespaceError::InvalidCString)
}

fn unshare(flags: libc::c_int) -> libc::c_int {
    // SAFETY: libc forwards directly to the Linux unshare(2) system call.
    unsafe { libc::unshare(flags) }
}

fn mount(
    source: Option<&Path>,
    target: &Path,
    flags: libc::c_ulong,
) -> io::Result<libc::c_int> {
    let source = match source {
        Some(path) => Some(to_cstring(path).map_err(|error| match error {
            LinuxNamespaceError::InvalidCString => {
                io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL")
            }
            LinuxNamespaceError::Io(error) => error,
            LinuxNamespaceError::Mapping(error) => {
                io::Error::new(io::ErrorKind::InvalidInput, error)
            }
            LinuxNamespaceError::Unsupported => {
                io::Error::new(io::ErrorKind::Unsupported, "unsupported")
            }
        })?),
        None => None,
    };
    let target = to_cstring(target).map_err(|error| match error {
        LinuxNamespaceError::InvalidCString => io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"),
        LinuxNamespaceError::Io(error) => error,
        LinuxNamespaceError::Mapping(error) => io::Error::new(io::ErrorKind::InvalidInput, error),
        LinuxNamespaceError::Unsupported => io::Error::new(io::ErrorKind::Unsupported, "unsupported"),
    })?;

    // SAFETY: pointers are valid NUL-terminated strings for the duration of
    // the syscall; fstype/data are null because bind/remount/private operations
    // don't require them here.
    let result = unsafe {
        libc::mount(
            source.as_ref().map_or(std::ptr::null(), |value| value.as_ptr()),
            target.as_ptr(),
            std::ptr::null(),
            flags,
            std::ptr::null(),
        )
    };

    Ok(result)
}

trait MappingRuleExt {
    fn mapping_kind(&self) -> crate::MappingKind;
}

impl MappingRuleExt for MappingRule {
    fn mapping_kind(&self) -> crate::MappingKind {
        self.kind()
    }
}

#[cfg(test)]
mod tests {
    use super::LinuxNamespaceError;

    #[test]
    fn error_is_debuggable() {
        let error = LinuxNamespaceError::Unsupported;
        assert_eq!(error.to_string(), "Linux namespace backend is unavailable");
    }
}
