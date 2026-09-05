//! Profile-driven logical-root materialization.
//!
//! The production launch path creates the Linux-compatible `/` as a fresh
//! tmpfs root. System Image content remains physically on the hidden SYSTEM
//! partition and is exposed only through explicit read-only bind mappings
//! selected by `RuntimeProfile`. No System Image tree is copied or overlaid
//! into a persistent filesystem tree.

use std::ffi::CString;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use luna_common::{ResourceAccess, RuntimeProfile};
use luna_root_mapping::{LogicalPath, MappingKind, MappingRule, MappingTable};

use crate::{LogicalRoot, NamespaceError};

pub fn materialize_profiled_logical_root(
    root: &Path,
    base_root: &Path,
    trusted_source_roots: &[PathBuf],
    mappings: &MappingTable,
    profile: &RuntimeProfile,
) -> Result<LogicalRoot, NamespaceError> {
    if !base_root.is_dir() {
        return Err(NamespaceError::MissingBaseRoot);
    }
    validate_trusted_roots(trusted_source_roots)?;

    fs::create_dir_all(root)?;
    if fs::read_dir(root)?.next().is_some() {
        return Err(NamespaceError::RootNotEmpty);
    }

    let mut transaction = MountTransaction::new();

    // The logical Linux `/` is RAM-backed. `root` is only a mountpoint on the
    // host filesystem; all visible root contents live in this tmpfs mount.
    mount_tmpfs(root)?;
    transaction.record(root);

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
        crate::secure_mount::secure_bind_mount_from_roots(
            base_root, &source, root, &target, read_only,
        )?;
        transaction.record(&target);
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
        let source = adjusted.physical().as_path();
        let trusted_root = trusted_source_roots
            .iter()
            .find(|root| source == root.as_path() || source.starts_with(root))
            .ok_or_else(|| {
                NamespaceError::FilesystemAccess(format!(
                    "mapping source is outside trusted source roots: {}",
                    source.display()
                ))
            })?;
        prepare_mount_target(root, &adjusted)?;
        crate::secure_mount::secure_bind_mount_from_roots(
            trusted_root,
            source,
            root,
            adjusted.logical().as_path(),
            !adjusted.access().contains(&ResourceAccess::Write),
        )?;
        transaction.record(adjusted.logical().as_path());
    }

    let proc_target = root.join("proc");
    let sys_target = root.join("sys");
    let dev_target = root.join("dev");
    let tmp_target = root.join("tmp");
    fs::create_dir_all(&proc_target)?;
    fs::create_dir_all(&sys_target)?;
    fs::create_dir_all(&dev_target)?;
    fs::create_dir_all(&tmp_target)?;

    mount_filesystem(
        "proc",
        &proc_target,
        libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
        None,
    )?;
    transaction.record(&proc_target);
    mount_filesystem(
        "sysfs",
        &sys_target,
        libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC | libc::MS_RDONLY,
        None,
    )?;
    transaction.record(&sys_target);
    mount_filesystem(
        "tmpfs",
        &dev_target,
        libc::MS_NOSUID | libc::MS_NODEV,
        Some("mode=0755"),
    )?;
    transaction.record(&dev_target);
    mount_filesystem(
        "tmpfs",
        &tmp_target,
        libc::MS_NOSUID | libc::MS_NODEV,
        Some("mode=1777"),
    )?;
    transaction.record(&tmp_target);

    transaction.commit();
    Ok(LogicalRoot {
        path: root.to_path_buf(),
        // There is deliberately no persistent OverlayFS upper/work tree for
        // this root. `support` is the mountpoint parent for lifecycle/cleanup.
        support: root
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| Path::new("/").to_path_buf()),
    })
}

struct MountTransaction {
    mounted: Vec<PathBuf>,
    committed: bool,
}

impl MountTransaction {
    fn new() -> Self {
        Self {
            mounted: Vec::new(),
            committed: false,
        }
    }

    fn record(&mut self, target: &Path) {
        self.mounted.push(target.to_path_buf());
    }

    fn commit(&mut self) {
        self.committed = true;
    }
}

impl Drop for MountTransaction {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        for target in self.mounted.iter().rev() {
            let target = match CString::new(target.as_os_str().as_bytes()) {
                Ok(target) => target,
                Err(_) => continue,
            };
            unsafe {
                libc::umount2(target.as_ptr(), libc::MNT_DETACH);
            }
        }
    }
}

fn validate_trusted_roots(roots: &[PathBuf]) -> Result<(), NamespaceError> {
    for root in roots {
        if !root.is_absolute() || root == Path::new("/") || has_navigation_syntax(root) {
            return Err(NamespaceError::InvalidPath);
        }
        if !root.is_dir() {
            return Err(NamespaceError::FilesystemAccess(format!(
                "trusted source root does not exist: {}",
                root.display()
            )));
        }
    }
    Ok(())
}

fn has_navigation_syntax(path: &Path) -> bool {
    path.to_string_lossy()
        .split('/')
        .any(|component| component == "." || component == "..")
}

fn prepare_mount_target(root: &Path, rule: &MappingRule) -> Result<(), NamespaceError> {
    let target = root.join(rule.logical().as_str().trim_start_matches('/'));
    match rule.kind() {
        MappingKind::File => {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            if !target.exists() {
                fs::File::create(target)?;
            }
        }
        MappingKind::Subtree => fs::create_dir_all(target)?,
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
            source.map_or(std::ptr::null(), |value| value.as_ptr()),
            target.as_ptr(),
            filesystem.map_or(std::ptr::null(), |value| value.as_ptr()),
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
