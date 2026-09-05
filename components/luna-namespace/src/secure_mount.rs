//! File-descriptor-based secure bind mounting for trusted runtime sources.
//!
//! Source resolution is performed with `openat2(2)` using `RESOLVE_BENEATH`,
//! `RESOLVE_NO_SYMLINKS`, and `RESOLVE_NO_MAGICLINKS`. The resulting O_PATH
//! descriptor is converted into a detached mount object with `open_tree(2)`
//! and attached with `move_mount(2)`. This closes the pathname-check versus
//! mount time-of-check/time-of-use window for source resources.

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBun};

use crate::NamespaceError;

#[cfg(target_os = "linux")]
pub(crate) fn secure_bind_mount(
    source: &Path,
    target: &Path,
    read_only: bool,
) -> Result<(), NamespaceError> {
    if !source.is_absolute() || source == Path::new("/") {
        return Err(NamespaceError::InvalidPath);
    }

    let root_fd = open_path(Path::new("/"))?;
    let relative = source
        .strip_prefix("/")
        .map_err(|_| NamespaceError::InvalidPath)?;
    let source_fd = open_beneath(root_fd.as_raw_fd(), relative)?;
    let mount_fd = clone_mount(source_fd.as_raw_fd())?;
    attach_mount(mount_fd.as_raw_fd(), target)?;

    if read_only {
        let target = path_cstring(target)?;
        let status = unsafe {
            libc::mount(
                std::ptr::null(),
                target.as_ptr(),
                std::ptr::null(),
                libc::MS_BIND | libc::MS_REMOUNT | libc::MS_RDONLY,
                std::ptr::null(),
            )
        };
        if status == -1 {
            return Err(io::Error::last_os_error().into());
        }
    }

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
fn attach_mount(mount_fd: libc::c_int, target: &Path) -> Result<(), NamespaceError> {
    let empty = [0_u8];
    let target = path_cstring(target)?;
    let status = unsafe {
        libc::syscall(
            libc::SYS_move_mount,
            mount_fd,
            empty.as_ptr(),
            libc::AT_FDCWD,
            target.as_ptr(),
            MOVE_MOUNT_F_EMPTY_PATH,
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
const AT_EMPTY_PATH: u32 = 0x1000;
#[cfg(target_os = "linux")]
const OPEN_TREE_CLONE: u32 = 0x0000_0001;
#[cfg(target_os = "linux")]
const OPEN_TREE_CLOEXEC: u32 = libc::O_CLOEXEC as u32;
#[cfg(target_os = "linux")]
const MOVE_MOUNT_F_EMPTY_PATH: u32 = 0x0000_0004;
#[cfg(target_os = "linux")]
const RESOLVE_NO_MAGICLINKS: u64 = 0x02;
#[cfg(target_os = "linux")]
const RESOLVE_NO_SYMLINKS: u64 = 0x04;
#[cfg(target_os = "linux")]
const RESOLVE_BENEATH: u64 = 0x08;
