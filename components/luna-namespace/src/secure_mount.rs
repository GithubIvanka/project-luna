//! File-descriptor-based secure bind mounting for trusted runtime sources.
//!
//! Source and target resolution are performed relative to O_PATH directory
//! descriptors. `openat2(2)` is used with `RESOLVE_BENEATH`,
//! `RESOLVE_NO_SYMLINKS`, and `RESOLVE_NO_MAGICLINKS`; the selected source is
//! cloned with `open_tree(2)` and attached with `move_mount(2)`. Read-only is
//! applied to the detached mount object with `mount_setattr(2)` before attach.
//! This keeps both sides out of pathname-based TOCTOU windows.

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use crate::NamespaceError;

#[cfg(target_os = "linux")]
pub(crate) fn secure_bind_mount(
    source: &Path,
    target: &Path,
    read_only: bool,
) -> Result<(), NamespaceError> {
    secure_bind_mount_from_root(Path::new("/"), source, target, read_only)
}

/// Bind mount `source` relative to an explicit physical trust root.
///
/// Callers that pass `/` deliberately request the lower-level legacy mode and
/// must establish trust themselves. Production launch code passes an explicit
/// non-root trust domain selected by the system runtime.
#[cfg(target_os = "linux")]
pub(crate) fn secure_bind_mount_from_root(
    trusted_root: &Path,
    source: &Path,
    target: &Path,
    read_only: bool,
) -> Result<(), NamespaceError> {
    if !trusted_root.is_absolute()
        || !source.is_absolute()
        || source == Path::new("/")
        || !target.is_absolute()
        || target == Path::new("/")
    {
        return Err(NamespaceError::InvalidPath);
    }

    let root_fd = open_path(trusted_root)?;
    let relative = source
        .strip_prefix(trusted_root)
        .map_err(|_| NamespaceError::FilesystemAccess("source is outside trusted root".into()))?;
    let source_fd = if relative.as_os_str().is_empty() {
        root_fd.try_clone().map_err(NamespaceError::Io)?
    } else {
        open_beneath(root_fd.as_raw_fd(), relative)?
    };
    let mount_fd = clone_mount(source_fd.as_raw_fd())?;

    if read_only {
        set_mount_read_only(mount_fd.as_raw_fd())?;
    }

    let target_root_fd = open_path(Path::new("/"))?;
    let target_relative = target
        .strip_prefix("/")
        .map_err(|_| NamespaceError::InvalidPath)?;
    let target_fd = if target_relative.as_os_str().is_empty() {
        return Err(NamespaceError::InvalidPath);
    } else {
        open_beneath(target_root_fd.as_raw_fd(), target_relative)?
    };

    attach_mount(mount_fd.as_raw_fd(), target_fd.as_raw_fd())?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn secure_bind_mount(
    _source: &Path,
    _target: &Path,
    _read_only: bool,
) -> Result<(), NamespaceError> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "secure bind mounting requires Linux openat2/open_tree/move_mount",
    )
    .into())
}

#[cfg(target_os = "linux")]
fn open_path(path: &Path) -> Result<OwnedFd, NamespaceError> {
    let path = path_cstring(path)?;
    let fd = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    if fd == -1 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

#[cfg(target_os = "linux")]
fn open_beneath(dirfd: libc::c_int, relative: &Path) -> Result<OwnedFd, NamespaceError> {
    let path = relative.as_os_str();
    if path.is_empty() || PathBuf::from(path).is_absolute() {
        return Err(NamespaceError::InvalidPath);
    }
    let path = path_cstring(Path::new(path))?;
    let how = OpenHow {
        flags: (libc::O_PATH | libc::O_CLOEXEC) as u64,
        mode: 0,
        resolve: RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS | RESOLVE_NO_SYMLINKS,
    };
    let fd = unsafe {
        libc::syscall(
            libc::SYS_openat2,
            dirfd,
            path.as_ptr(),
            &how as *const OpenHow,
            std::mem::size_of::<OpenHow>(),
        ) as libc::c_int
    };
    if fd == -1 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

#[cfg(target_os = "linux")]
fn clone_mount(source_fd: libc::c_int) -> Result<OwnedFd, NamespaceError> {
    let empty = [0_u8];
    let fd = unsafe {
        libc::syscall(
            libc::SYS_open_tree,
            source_fd,
            empty.as_ptr(),
            (OPEN_TREE_CLONE | OPEN_TREE_CLOEXEC | AT_EMPTY_PATH) as libc::c_uint,
        ) as libc::c_int
    };
    if fd == -1 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

#[cfg(target_os = "linux")]
fn set_mount_read_only(mount_fd: libc::c_int) -> Result<(), NamespaceError> {
    let mut attr = MountAttr {
        attr_set: MOUNT_ATTR_RDONLY,
        attr_clr: 0,
        propagation: 0,
        userns_fd: 0,
    };
    let status = unsafe {
        libc::syscall(
            libc::SYS_mount_setattr,
            mount_fd,
            [0_u8].as_ptr(),
            AT_EMPTY_PATH,
            &mut attr as *mut MountAttr,
            std::mem::size_of::<MountAttr>(),
        ) as libc::c_int
    };
    if status == -1 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn attach_mount(mount_fd: libc::c_int, target_fd: libc::c_int) -> Result<(), NamespaceError> {
    let empty = [0_u8];
    let status = unsafe {
        libc::syscall(
            libc::SYS_move_mount,
            mount_fd,
            empty.as_ptr(),
            target_fd,
            empty.as_ptr(),
            (MOVE_MOUNT_F_EMPTY_PATH | MOVE_MOUNT_T_EMPTY_PATH) as libc::c_uint,
        ) as libc::c_int
    };
    if status == -1 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn path_cstring(path: &Path) -> Result<std::ffi::CString, NamespaceError> {
    std::ffi::CString::new(path.as_os_str().as_bytes()).map_err(|_| NamespaceError::InvalidPath)
}

#[cfg(target_os = "linux")]
#[repr(C)]
struct OpenHow {
    flags: u64,
    mode: u64,
    resolve: u64,
}

#[cfg(target_os = "linux")]
#[repr(C)]
struct MountAttr {
    attr_set: u64,
    attr_clr: u64,
    propagation: u64,
    userns_fd: u64,
}

#[cfg(target_os = "linux")]
const AT_EMPTY_PATH: u32 = 0x1000;
#[cfg(target_os = "linux")]
const OPEN_TREE_CLONE: u32 = 0x0000_0001;
#[cfg(target_os = "linux")]
const OPEN_TREE_CLOEXEC: u32 = libc::O_CLOEXEC as u32;
#[cfg(target_os = "linux")]
const MOVE_MOUNT_F_EMPTY_PATH: u32 = 0x0000_0004;
#[cfg(target_os = "linux")]
const MOVE_MOUNT_T_EMPTY_PATH: u32 = 0x0000_0040;
#[cfg(target_os = "linux")]
const MOUNT_ATTR_RDONLY: u64 = 0x0000_0001;
#[cfg(target_os = "linux")]
const RESOLVE_NO_MAGICLINKS: u64 = 0x02;
#[cfg(target_os = "linux")]
const RESOLVE_NO_SYMLINKS: u64 = 0x04;
#[cfg(target_os = "linux")]
const RESOLVE_BENEATH: u64 = 0x08;
