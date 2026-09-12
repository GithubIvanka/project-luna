//! Linux namespace backend for Project Luna.
//!
//! This crate contains operating-system-specific namespace mechanics. Luna
//! policy and logical path resolution remain owned by `luna-security` and
//! `luna-root-mapping` respectively.

use std::ffi::CString;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

use landlock::{
    ABI, Access, AccessFs, CompatLevel, Compatible, PathBeneath, PathFd, Ruleset, RulesetAttr,
    RulesetCreatedAttr, RulesetStatus,
};
use luna_common::{ResourceAccess, RuntimeProfile};
use luna_root_mapping::{MappingKind, MappingRule, MappingTable};

mod profile;
mod secure_mount;
pub use profile::materialize_profiled_logical_root;

#[derive(Debug)]
pub enum NamespaceError {
    Io(io::Error),
    InvalidPath,
    MissingBaseRoot,
    RootNotEmpty,
    FilesystemAccess(String),
    Landlock(String),
}

impl std::fmt::Display for NamespaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "namespace operation failed: {error}"),
            Self::InvalidPath => {
                f.write_str("path contains an interior NUL byte or unsafe component")
            }
            Self::MissingBaseRoot => f.write_str("logical root base directory does not exist"),
            Self::RootNotEmpty => f.write_str("logical root staging directory must be empty"),
            Self::FilesystemAccess(error) => {
                write!(f, "filesystem access policy failed: {error}")
            }
            Self::Landlock(error) => write!(f, "Landlock enforcement failed: {error}"),
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
#[derive(Debug, Clone, Copy, Default)]
pub struct LinuxMountNamespace;

impl LinuxMountNamespace {
    /// Create a private Linux mount namespace for the calling process.
    pub fn enter_private() -> Result<Self, NamespaceError> {
        if unsafe { libc::unshare(libc::CLONE_NEWNS) } == -1 {
            return Err(io::Error::last_os_error().into());
        }
        if mount(None, Path::new("/"), None, libc::MS_REC | libc::MS_PRIVATE)? == -1 {
            return Err(io::Error::last_os_error().into());
        }
        Ok(Self)
    }

    /// Apply mappings at the caller's current root using their declared access.
    pub fn apply_mappings(&self, mappings: &MappingTable) -> Result<(), NamespaceError> {
        for rule in mappings.iter().filter(|rule| !rule.access().is_empty()) {
            let read_only = !rule.access().contains(&ResourceAccess::Write);
            self.apply_mapping(rule, read_only)?;
        }
        Ok(())
    }

    pub fn apply_mapping(&self, rule: &MappingRule, read_only: bool) -> Result<(), NamespaceError> {
        let target = rule.logical().as_path();
        let source = rule.physical().as_path();
        match rule.kind() {
            MappingKind::File => {
                if !source.is_file() {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "file mapping source is not a file",
                    )
                    .into());
                }
                if !target.exists() {
                    if let Some(parent) = target.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::File::create(target)?;
                }
            }
            MappingKind::Subtree => {
                if !source.is_dir() {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "subtree mapping source is not a directory",
                    )
                    .into());
                }
                fs::create_dir_all(target)?;
            }
        }
        bind_mount(source, target, read_only)
    }

    /// Enforce every mapping's Read/Write/Execute permissions with Landlock.
    pub fn enforce_filesystem_access(
        &self,
        profile: &RuntimeProfile,
        mappings: &MappingTable,
    ) -> Result<(), NamespaceError> {
        let handled = AccessFs::from_all(ABI::V3);
        let mut ruleset = Ruleset::default()
            .set_compatibility(CompatLevel::HardRequirement)
            .handle_access(handled)
            .map_err(|error| NamespaceError::Landlock(error.to_string()))?
            .create()
            .map_err(|error| NamespaceError::Landlock(error.to_string()))?;

        // Trusted runtime resources are distinct from application mappings,
        // but both must be represented in the final Landlock ruleset.
        for (logical_path, declared_access) in profile.resources() {
            let rule = MappingRule::subtree(
                luna_root_mapping::LogicalPath::new(logical_path)
                    .map_err(|_| NamespaceError::InvalidPath)?,
                luna_root_mapping::PhysicalPath::new(logical_path),
            )
            .with_access(declared_access.iter().copied());
            let access = landlock_access(&rule)?;
            let fd = PathFd::new(Path::new(logical_path))
                .map_err(|error| NamespaceError::Landlock(error.to_string()))?;
            ruleset = ruleset
                .add_rule(PathBeneath::new(fd, access))
                .map_err(|error| NamespaceError::Landlock(error.to_string()))?;
        }

        for rule in mappings.iter() {
            if rule.access().is_empty() {
                continue;
            }
            let access = landlock_access(rule)?;
            let fd = PathFd::new(rule.logical().as_path())
                .map_err(|error| NamespaceError::Landlock(error.to_string()))?;
            ruleset = ruleset
                .add_rule(PathBeneath::new(fd, access))
                .map_err(|error| NamespaceError::Landlock(error.to_string()))?;
        }

        let status = ruleset
            .restrict_self()
            .map_err(|error| NamespaceError::Landlock(error.to_string()))?;
        if status.ruleset != RulesetStatus::FullyEnforced || !status.no_new_privs {
            return Err(NamespaceError::Landlock(format!(
                "filesystem restrictions were not fully enforced: {:?}",
                status.ruleset
            )));
        }
        Ok(())
    }

    pub fn enter_logical_root(&self, root: &LogicalRoot) -> Result<(), NamespaceError> {
        if !root.path.is_dir() {
            return Err(NamespaceError::MissingBaseRoot);
        }
        let root = CString::new(root.path.as_os_str().as_bytes())
            .map_err(|_| NamespaceError::InvalidPath)?;
        if unsafe { libc::chroot(root.as_ptr()) } == -1 {
            return Err(io::Error::last_os_error().into());
        }
        std::env::set_current_dir("/")?;
        Ok(())
    }

    pub fn expose_device(
        &self,
        root: &LogicalRoot,
        source: &Path,
        logical_name: &Path,
        read_only: bool,
    ) -> Result<(), NamespaceError> {
        if !source.exists() {
            return Err(
                io::Error::new(io::ErrorKind::NotFound, "device source does not exist").into(),
            );
        }
        validate_relative_path(logical_name)?;
        let target = root.path.join("dev").join(logical_name);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        if !target.exists() {
            fs::File::create(&target)?;
        }
        bind_mount(source, &target, read_only)
    }
}

#[derive(Debug, Clone)]
pub struct LogicalRoot {
    path: PathBuf,
    support: PathBuf,
}

impl LogicalRoot {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn support_path(&self) -> &Path {
        &self.support
    }
}

fn landlock_access(rule: &MappingRule) -> Result<landlock::BitFlags<AccessFs>, NamespaceError> {
    let mut access: Option<landlock::BitFlags<AccessFs>> = None;
    let mut add = |right: AccessFs| {
        let flags: landlock::BitFlags<AccessFs> = right.into();
        access = Some(access.take().unwrap_or_default() | flags);
    };

    if rule.access().contains(&ResourceAccess::Read) {
        add(AccessFs::ReadFile);
        if rule.kind() == MappingKind::Subtree {
            add(AccessFs::ReadDir);
        }
    }
    if rule.access().contains(&ResourceAccess::Write) {
        add(AccessFs::WriteFile);
        add(AccessFs::Truncate);
        if rule.kind() == MappingKind::Subtree {
            add(AccessFs::RemoveDir);
            add(AccessFs::RemoveFile);
            add(AccessFs::MakeDir);
            add(AccessFs::MakeReg);
            add(AccessFs::Refer);
        }
    }
    if rule.access().contains(&ResourceAccess::Execute) {
        add(AccessFs::Execute);
    }

    access.ok_or_else(|| {
        NamespaceError::FilesystemAccess("mapping access produced an empty Landlock rule".into())
    })
}

fn bind_mount(source: &Path, target: &Path, read_only: bool) -> Result<(), NamespaceError> {
    secure_mount::secure_bind_mount(source, target, read_only)
}

fn validate_relative_path(path: &Path) -> Result<(), NamespaceError> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(NamespaceError::InvalidPath);
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(NamespaceError::InvalidPath);
    }
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
    let data = data
        .map(|value| CString::new(value).map_err(|_| NamespaceError::InvalidPath))
        .transpose()?;
    Ok(unsafe {
        libc::mount(
            source
                .as_ref()
                .map_or(std::ptr::null(), |value| value.as_ptr()),
            target.as_ptr(),
            filesystem
                .as_ref()
                .map_or(std::ptr::null(), |value| value.as_ptr()),
            flags,
            data.as_ref()
                .map_or(std::ptr::null(), |value| value.as_ptr().cast()),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{LinuxMountNamespace, NamespaceError, landlock_access};
    use luna_common::{ResourceAccess, RuntimeProfile};
    use luna_root_mapping::{LogicalPath, MappingRule, PhysicalPath};

    #[test]
    fn invalid_path_has_stable_message() {
        assert_eq!(
            NamespaceError::InvalidPath.to_string(),
            "path contains an interior NUL byte or unsafe component"
        );
    }

    #[test]
    fn missing_base_root_has_stable_message() {
        assert_eq!(
            NamespaceError::MissingBaseRoot.to_string(),
            "logical root base directory does not exist"
        );
    }

    #[test]
    fn root_not_empty_has_stable_message() {
        assert_eq!(
            NamespaceError::RootNotEmpty.to_string(),
            "logical root staging directory must be empty"
        );
    }

    #[test]
    fn read_access_does_not_grant_write_or_execute() {
        let rule = MappingRule::file(
            LogicalPath::new("/data/file").unwrap(),
            PhysicalPath::new("/data/file"),
        )
        .with_access([ResourceAccess::Read]);
        let access = landlock_access(&rule).unwrap();
        assert!(access.contains(landlock::AccessFs::ReadFile));
        assert!(!access.contains(landlock::AccessFs::WriteFile));
        assert!(!access.contains(landlock::AccessFs::Execute));
    }

    #[test]
    fn write_only_access_does_not_grant_read() {
        let rule = MappingRule::file(
            LogicalPath::new("/data/file").unwrap(),
            PhysicalPath::new("/data/file"),
        )
        .with_access([ResourceAccess::Write]);
        let access = landlock_access(&rule).unwrap();
        assert!(!access.contains(landlock::AccessFs::ReadFile));
        assert!(access.contains(landlock::AccessFs::WriteFile));
        assert!(access.contains(landlock::AccessFs::Truncate));
        assert!(!access.contains(landlock::AccessFs::Execute));
    }

    #[test]
    fn execute_only_access_does_not_grant_read_or_write() {
        let rule = MappingRule::file(
            LogicalPath::new("/bin/app").unwrap(),
            PhysicalPath::new("/bin/app"),
        )
        .with_access([ResourceAccess::Execute]);
        let access = landlock_access(&rule).unwrap();
        assert!(access.contains(landlock::AccessFs::Execute));
        assert!(!access.contains(landlock::AccessFs::ReadFile));
        assert!(!access.contains(landlock::AccessFs::WriteFile));
    }

    #[test]
    fn trusted_runtime_subtree_grants_read_and_execute_without_write() {
        let rule = MappingRule::subtree(
            LogicalPath::new("/lib64").unwrap(),
            PhysicalPath::new("/system/lib64"),
        )
        .with_access([ResourceAccess::Read, ResourceAccess::Execute]);
        let access = landlock_access(&rule).unwrap();
        assert!(access.contains(landlock::AccessFs::ReadDir));
        assert!(access.contains(landlock::AccessFs::ReadFile));
        assert!(access.contains(landlock::AccessFs::Execute));
        assert!(!access.contains(landlock::AccessFs::WriteFile));
    }

    #[test]
    fn namespace_type_is_zero_sized() {
        assert_eq!(std::mem::size_of::<LinuxMountNamespace>(), 0);
    }
}
