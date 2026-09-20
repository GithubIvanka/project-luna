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
    secure_bind_mount_from_roots(Path::new("/"), source, Path::new("/"), target, read_only)
}

/// Bind mount `source` below an explicit source root and `target` below an
/// explicit destination root.
///
/// Production launch code supplies non-root source and target trust domains.
/// The legacy wrapper above intentionally retains host-root behavior for
/// internal compatibility and is not used by the profile-driven launcher.
#[cfg(target_os = "linux")]
pub(crate) fn secure_bind_mount_from_roots(
    trusted_source_root: &Path,
    source: &Path,
    trusted_target_root: &Path,
    target: &Path,
    read_only: bool,
) -> Result<(), NamespaceError> {
    let legacy_host_binding =
        trusted_source_root == Path::new("/") && trusted_target_root == Path::new("/");
    if !trusted_source_root.is_absolute()
        || (!legacy_host_binding && trusted_source_root == Path::new("/"))
        || !source.is_absolute()
        || source == Path::new("/")
        || !trusted_target_root.is_absolute()
        || (!legacy_host_binding && trusted_target_root == Path::new("/"))
        || !target.is_absolute()
        || target == Path::new("/")
    {
        return Err(NamespaceError::InvalidPath);
    }

    let host_root_fd = open_path(Path::new("/"))?;
    let source_root_fd = if trusted_source_root == Path::new("/") {
        host_root_fd.try_clone().map_err(NamespaceError::Io)?
    } else {
        let trusted_relative = trusted_source_root
            .strip_prefix("/")
            .map_err(|_| NamespaceError::InvalidPath)?;
        open_beneath(host_root_fd.as_raw_fd(), trusted_relative)?
    };

    let source_relative = source
        .strip_prefix(trusted_source_root)
        .map_err(|_| NamespaceError::FilesystemAccess("source is outside trusted root".into()))?;
    let source_fd = open_beneath(source_root_fd.as_raw_fd(), source_relative)?;
    let mount_fd = clone_mount(source_fd.as_raw_fd())?;

    if read_only {
        set_mount_read_only(mount_fd.as_raw_fd())?;
    }

    let target_root_fd = if trusted_target_root == Path::new("/") {
        host_root_fd
    } else {
        open_path(trusted_target_root)?
    };
    let target_relative = target
        .strip_prefix(trusted_target_root)
        .map_err(|_| NamespaceError::FilesystemAccess("target is outside target root".into()))?;
    let target_fd = open_beneath(target_root_fd.as_raw_fd(), target_relative)?;

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
            libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
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
