//! Profile-driven System Image materialization.
//!
//! The legacy `materialize_logical_root` API creates a full System Image
//! lower layer. The production launch path must not expose that entire tree;
//! this module overlays an empty runtime root and explicitly binds only the
//! resources named by `RuntimeProfile` plus already-authorized application
//! mappings.

use std::ffi::CString;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use luna_common::{ResourceAccess, RuntimeProfile};
use luna_root_mapping::{LogicalPath, MappingKind, MappingRule, MappingTable, PhysicalPath};

use crate::{LinuxMountNamespace, LogicalRoot, NamespaceError};

pub fn materialize_profiled_logical_root(
    namespace: &LinuxMountNamespace,
    root: &Path,
    base_root: &Path,
    mappings: &MappingTable,
    profile: &RuntimeProfile,
) -> Result<LogicalRoot, NamespaceError> {
    let logical = namespace.materialize_logical_root(root, base_root, mappings)?;

    // Hide the full legacy lower tree behind a fresh empty tmpfs root. The
    // resulting root is populated only by explicit profile and application
    // mounts below.
    mount_tmpfs(root)?;

    for (logical_path, access) in profile.resources() {
        let source = base_root.join(logical_path.trim_start_matches('/'));
        let target = root.join(logical_path.trim_start_matches('/'));
        if !source.exists() || !source.is_dir() {
            return Err(NamespaceError::FilesystemAccess(format!(
                "runtime profile source is not a directory: {}",
                source.display()
            )));
        }
        fs::create_dir_all(&target)?;
        let read_only = !access.contains(&ResourceAccess::Write);
        bind_mount(&source, &target, read_only)?;
    }

    for rule in mappings.iter().filter(|rule| !rule.access().is_empty()) {
        let logical_path = root.join(rule.logical().as_str().trim_start_matches('/'));
        let adjusted = match rule.kind() {
            MappingKind::File => MappingRule::file(
                LogicalPath::new(logical_path).map_err(|_| NamespaceError::InvalidPath)?,
                rule.physical().clone(),
            )
            .with_access(rule.access().iter().copied()),
            MappingKind::Subtree => MappingRule::subtree(
                LogicalPath::new(logical_path).map_err(|_| NamespaceError::InvalidPath)?,
                rule.physical().clone(),
            )
            .with_access(rule.access().iter().copied()),
        };
        namespace.apply_mapping(
            &adjusted,
            !rule.access().contains(&ResourceAccess::Write),
        )?;
    }

    fs::create_dir_all(root.join("proc"))?;
    fs::create_dir_all(root.join("sys"))?;
    fs::create_dir_all(root.join("dev"))?;
    fs::create_dir_all(root.join("tmp"))?;
    mount_filesystem(
        "proc",
        &root.join("proc"),
        libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
        None,
    )?;
    mount_filesystem(
        "sysfs",
        &root.join("sys"),
        libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC | libc::MS_RDONLY,
        None,
    )?;
    mount_filesystem(
        "tmpfs",
        &root.join("dev"),
        libc::MS_NOSUID | libc::MS_NODEV,
        Some("mode=0755"),
    )?;
    mount_filesystem(
        "tmpfs",
        &root.join("tmp"),
        libc::MS_NOSUID | libc::MS_NODEV,
        Some("mode=1777"),
    )?;

    Ok(logical)
}

fn bind_mount(source: &Path, target: &Path, read_only: bool) -> Result<(), NamespaceError> {
    mount_filesystem_path(Some(source), target, libc::MS_BIND, None)?;
    if read_only {
        mount_filesystem_path(
            None,
            target,
            libc::MS_BIND | libc::MS_REMOUNT | libc::MS_RDONLY,
            None,
        )?;
    }
    Ok(())
}

fn mount_tmpfs(target: &Path) -> Result<(), NamespaceError> {
    mount_filesystem(
        "tmpfs",
        target,
        libc::MS_NOSUID | libc::MS_NODEV,
        Some("mode=0755"),
    )
}

fn mount_filesystem(
    filesystem: &str,
    target: &Path,
    flags: libc::c_ulong,
    data: Option<&str>,
) -> Result<(), NamespaceError> {
    let source = CString::new(filesystem).map_err(|_| NamespaceError::InvalidPath)?;
    mount_raw(Some(&source), target, Some(&source), flags, data)
}

fn mount_filesystem_path(
    source: Option<&Path>,
    target: &Path,
    flags: libc::c_ulong,
    data: Option<&str>,
) -> Result<(), NamespaceError> {
    let source = source.map(path_cstring).transpose()?;
    mount_raw(source.as_ref(), target, None, flags, data)
}

fn mount_raw(
    source: Option<&CString>,
    target: &Path,
    filesystem: Option<&CString>,
    flags: libc::c_ulong,
    data: Option<&str>,
) -> Result<(), NamespaceError> {
    let target = path_cstring(target)?;
    let data = data
        .map(|value| CString::new(value).map_err(|_| NamespaceError::InvalidPath))
        .transpose()?;
    let status = unsafe {
        libc::mount(
            source.map_or(std::ptr::null(), CString::as_ptr),
            target.as_ptr(),
            filesystem.map_or(std::ptr::null(), CString::as_ptr),
            flags,
            data.as_ref()
                .map_or(std::ptr::null(), |value| value.as_ptr().cast()),
        )
    };
    if status == -1 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(())
}

fn path_cstring(path: &Path) -> Result<CString, NamespaceError> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| NamespaceError::InvalidPath)
}

// Keep the `PhysicalPath` type in this module's contract explicit.
const _: fn(PhysicalPath) = |_| {};
