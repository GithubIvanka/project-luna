//! Linux namespace backend for Project Luna.
//!
//! This crate contains operating-system-specific namespace mechanics. Luna
//! policy and logical path resolution remain owned by `luna-security` and
//! `luna-root-mapping` respectively.

use std::ffi::CString;
use std::fs;
use std::io;
use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::FileTypeExt;
use std::path::{Component, Path, PathBuf};

use landlock::{
    ABI, Access, AccessFs, CompatLevel, Compatible, PathBeneath, PathFd, Ruleset, RulesetAttr,
    RulesetCreatedAttr, RulesetStatus, Scope,
};
use luna_common::{ResourceAccess, RuntimeProfile};
use luna_root_mapping::{MappingKind, MappingRule, MappingTable};

mod profile;
mod secure_mount;
mod volume_mount;

pub use profile::{materialize_profiled_logical_root, materialize_private_terminal};
pub use volume_mount::{MountInstance, ResolvedVolume, VolumeMountManager, VolumeMountResolver};

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

    /// Enforce every mapping's Read/Write/Execute permissions with Landlock.
    pub fn enforce_filesystem_access(
        &self,
        profile: &RuntimeProfile,
        mappings: &MappingTable,
    ) -> Result<(), NamespaceError> {
        self.enforce_filesystem_access_with_terminal(profile, mappings, false)
    }

    /// The caller must validate the sealed terminal.pty capability first and
    /// materialize its private device filesystem before entering this root.
    pub fn enforce_filesystem_access_with_terminal(
        &self,
        profile: &RuntimeProfile,
        mappings: &MappingTable,
        terminal: bool,
    ) -> Result<(), NamespaceError> {
        // Named and abstract UNIX endpoints outside this instance are denied
        // unless an exact socket mapping has been authorized. ABI9 is required;
        // dropping the socket restriction on older kernels is not permitted.
        let handled = AccessFs::from_all(ABI::V3) | AccessFs::ResolveUnix;
        let mut ruleset = Ruleset::default()
            .set_compatibility(CompatLevel::HardRequirement)
            .handle_access(handled)
            .map_err(|error| NamespaceError::Landlock(error.to_string()))?
            .scope(Scope::AbstractUnixSocket)
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
            let fd = open_mapping_path(rule.logical().as_path())?;
            let metadata = fs::File::from(fd.try_clone()?).metadata()?;
            let access = if metadata.file_type().is_socket() {
                socket_access(rule)?
            } else {
                landlock_access(rule)?
            };
            ruleset = ruleset
                .add_rule(PathBeneath::new(fd, access))
                .map_err(|error| NamespaceError::Landlock(error.to_string()))?;
        }

        if terminal {
            let fd = PathFd::new("/dev")
                .map_err(|error| NamespaceError::Landlock(error.to_string()))?;
            // Only this instance's synthetic devices and new devpts instance;
            // no host TTY, GPU, disk or input device is exposed.
            ruleset = ruleset
                .add_rule(PathBeneath::new(fd, AccessFs::ReadFile | AccessFs::WriteFile | AccessFs::ReadDir))
                .map_err(|error| NamespaceError::Landlock(error.to_string()))?;
            // Yazi's scratch files and DDS socket remain inside this instance's
            // private tmpfs. Neither host runtime sockets nor device creation
            // are permitted by this rule.
            let temporary = MappingRule::subtree(
                luna_root_mapping::LogicalPath::new("/tmp").map_err(|_| NamespaceError::InvalidPath)?,
                luna_root_mapping::PhysicalPath::new("/tmp"),
            ).with_access([ResourceAccess::Read, ResourceAccess::Write]);
            let fd = PathFd::new("/tmp").map_err(|error| NamespaceError::Landlock(error.to_string()))?;
            ruleset = ruleset
                .add_rule(PathBeneath::new(fd, landlock_access(&temporary)? | AccessFs::MakeSock | AccessFs::ResolveUnix))
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

// Open the already materialized object as O_PATH, never as a socket/device
// stream. PathBeneath accepts the owned descriptor directly.
fn open_mapping_path(path: &Path) -> Result<OwnedFd, NamespaceError> {
    let path = cstring(path)?;
    let fd = unsafe { libc::syscall(libc::SYS_openat, libc::AT_FDCWD,
        path.as_ptr(), libc::O_PATH | libc::O_CLOEXEC | libc::O_NOFOLLOW, 0) } as i32;
    if fd < 0 { return Err(io::Error::last_os_error().into()); }
    // open returned one new owned descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn socket_access(rule: &MappingRule) -> Result<landlock::BitFlags<AccessFs>, NamespaceError> {
    if rule.kind() != MappingKind::File || rule.access().len() != 1
        || !rule.access().contains(&ResourceAccess::Read) {
        return Err(NamespaceError::FilesystemAccess("socket requires an exact read-only mapping".into()));
    }
    Ok(AccessFs::ResolveUnix.into())
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
            add(AccessFs::MakeSym);
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
    use luna_common::ResourceAccess;
    use luna_root_mapping::{LogicalPath, MappingRule, PhysicalPath};

    #[test]
    fn socket_rules_use_pinned_opath_and_only_resolve_unix() {
        use std::os::fd::AsRawFd;
        use std::os::unix::net::UnixListener;
        let path = std::env::temp_dir().join(format!("luna-ns-socket-{}", std::process::id()));
        let listener = UnixListener::bind(&path).unwrap();
        let fd = super::open_mapping_path(&path).unwrap();
        assert_ne!(unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) } & libc::O_PATH, 0);
        let rule = MappingRule::file(LogicalPath::new("/run/wayland-0").unwrap(), PhysicalPath::new(&path))
            .with_access([ResourceAccess::Read]);
        let access = super::socket_access(&rule).unwrap();
        assert_eq!(access, landlock::AccessFs::ResolveUnix);
        assert!(super::socket_access(&rule.clone().with_access([ResourceAccess::Read, ResourceAccess::Write])).is_err());
        // Diagnostic comparison only: the direct O_PATH operation above is mandatory.
        println!("standard PathFd socket result: {:?}", landlock::PathFd::new(&path));
        drop((fd, listener));
        std::fs::remove_file(path).unwrap();
    }

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
    fn writable_subtree_allows_file_manager_links_but_no_special_nodes() {
        let rule = MappingRule::subtree(LogicalPath::new("/home").unwrap(), PhysicalPath::new("/data/home"))
            .with_access([ResourceAccess::Read, ResourceAccess::Write]);
        let access = landlock_access(&rule).unwrap();
        assert!(access.contains(landlock::AccessFs::MakeSym));
        assert!(access.contains(landlock::AccessFs::Refer));
        assert!(!access.contains(landlock::AccessFs::MakeFifo));
        assert!(!access.contains(landlock::AccessFs::MakeSock));
        assert!(!access.contains(landlock::AccessFs::MakeChar));
        assert!(!access.contains(landlock::AccessFs::MakeBlock));
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
