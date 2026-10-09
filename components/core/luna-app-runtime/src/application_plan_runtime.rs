//! Process-launch integration for [`ApplicationPlan`](crate::ApplicationPlan).
//!
//! Authorization is intentionally completed before this module is entered.
//! The launcher only consumes an `AuthorizedApplicationPlan`, validates that
//! its mapping/capability declarations still match execution policy, builds
//! a profile-driven logical root in a private mount namespace, installs
//! filesystem enforcement, and registers the supervised process.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use luna_common::{ResourceAccess, RuntimeProfile, VolumeId, VolumeRef};
use luna_namespace::{
    LinuxMountNamespace, MountInstance, NamespaceError, ResolvedVolume, VolumeMountManager,
    VolumeMountResolver, materialize_profiled_logical_root,
};
use luna_root_mapping::{LogicalPath, MappingKind, MappingSource, MappingTable};
use luna_security::{CapabilityName, CapabilityRegistry};
use luna_system_runtime::SystemRuntimeService;
use luna_user_session::UserSession;
use luna_volume_manager::VolumeManager;

use crate::application_plan::AuthorizedApplicationPlan;
use crate::{
    ApplicationInstance, ApplicationInstanceId, FailureStage, InMemoryApplicationRuntime,
    InstanceState, LinuxApplicationRuntime, RuntimeError,
};

/// Adapts the system-owned Volume Manager backend into the namespace mount
/// resolver. The application-facing API still uses only VolumeRef and paths.
pub struct VolumeManagerResolver<'a> {
    manager: &'a VolumeManager,
}

impl<'a> VolumeManagerResolver<'a> {
    pub fn new(manager: &'a VolumeManager) -> Self {
        Self { manager }
    }

    pub fn snapshot_for_mapping(
        &self,
        mapping: &MappingTable,
    ) -> Result<StaticVolumeResolver, NamespaceError> {
        StaticVolumeResolver::from_mapping(self.manager, mapping)
    }
}

impl VolumeMountResolver for VolumeManagerResolver<'_> {
    fn resolve_volume(
        &self,
        volume: &luna_common::VolumeRef,
    ) -> Result<ResolvedVolume, NamespaceError> {
        let backend = self
            .manager
            .volume_backend(volume)
            .map_err(|error| NamespaceError::FilesystemAccess(error.to_string()))?;
        Ok(ResolvedVolume::new(
            backend.volume().clone(),
            backend.backend_node().to_path_buf(),
            backend.filesystem().to_owned(),
            backend.read_only(),
        ))
    }
}

/// An owned resolver snapshot safe to move into a forked child before exec.
///
/// The snapshot deliberately contains only backend details for VolumeRefs that
/// are actually present in one mapping table; it carries no drive-letter state.
#[derive(Clone, Debug, Default)]
pub struct StaticVolumeResolver {
    volumes: BTreeMap<VolumeId, ResolvedVolume>,
}

impl StaticVolumeResolver {
    pub fn from_mapping(
        manager: &VolumeManager,
        mapping: &MappingTable,
    ) -> Result<Self, NamespaceError> {
        let mut volumes = BTreeMap::new();
        for rule in mapping.iter() {
            if rule.access().is_empty() {
                continue;
            }
            let MappingSource::Volume(path) = rule.source() else {
                continue;
            };
            let volume = path.volume();
            if volumes.contains_key(volume.id()) {
                continue;
            }
            let backend = manager
                .volume_backend(volume)
                .map_err(|error| NamespaceError::FilesystemAccess(error.to_string()))?;
            volumes.insert(
                volume.id().clone(),
                ResolvedVolume::new(
                    backend.volume().clone(),
                    backend.backend_node().to_path_buf(),
                    backend.filesystem().to_owned(),
                    backend.read_only(),
                ),
            );
        }
        Ok(Self { volumes })
    }

    pub fn len(&self) -> usize {
        self.volumes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.volumes.is_empty()
    }
}

impl VolumeMountResolver for StaticVolumeResolver {
    fn resolve_volume(&self, volume: &VolumeRef) -> Result<ResolvedVolume, NamespaceError> {
        self.volumes.get(volume.id()).cloned().ok_or_else(|| {
            NamespaceError::FilesystemAccess(format!(
                "volume {} is missing from resolver snapshot",
                volume.id()
            ))
        })
    }
}

/// Immutable execution environment selected by the system runtime for one
/// application launch.
///
/// Keeping these inputs together makes the namespace/root handoff explicit and
/// prevents callers from accidentally mixing the roots of different launches.
#[derive(Clone, Debug)]
pub struct ApplicationLaunchContext {
    namespace: LinuxMountNamespace,
    base_root: PathBuf,
    staging_parent: PathBuf,
    trusted_source_roots: Vec<PathBuf>,
    volume_resolver: Option<StaticVolumeResolver>,
}

impl ApplicationLaunchContext {
    pub fn new(
        namespace: LinuxMountNamespace,
        base_root: impl Into<PathBuf>,
        staging_parent: impl Into<PathBuf>,
    ) -> Self {
        Self {
            namespace,
            base_root: base_root.into(),
            staging_parent: staging_parent.into(),
            trusted_source_roots: Vec::new(),
            volume_resolver: None,
        }
    }

    /// Attach an owned Volume Manager snapshot for Volume mapping materialization.
    pub fn with_volume_resolver(mut self, resolver: StaticVolumeResolver) -> Self {
        self.volume_resolver = Some(resolver);
        self
    }

    pub fn volume_resolver(&self) -> Option<&StaticVolumeResolver> {
        self.volume_resolver.as_ref()
    }

    /// Add a system-selected physical source root from which application
    /// mappings may be mounted. The root is validated before launch.
    pub fn with_trusted_source_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.trusted_source_roots.push(root.into());
        self
    }

    pub fn namespace(&self) -> LinuxMountNamespace {
        self.namespace
    }

    pub fn base_root(&self) -> &Path {
        &self.base_root
    }

    pub fn staging_parent(&self) -> &Path {
        &self.staging_parent
    }

    pub fn trusted_source_roots(&self) -> &[PathBuf] {
        &self.trusted_source_roots
    }

    /// Validate filesystem roots before any staging directory or child process
    /// is created.
    pub fn validate(&self) -> Result<(), RuntimeError> {
        if !self.base_root.is_absolute() || !self.staging_parent.is_absolute() {
            return Err(RuntimeError::Staging(
                "launch context roots must be absolute paths".into(),
            ));
        }
        if self.has_navigation_components() {
            return Err(RuntimeError::Staging(
                "launch context roots must not contain '.' or '..' components".into(),
            ));
        }
        if self.base_root == Path::new("/") {
            return Err(RuntimeError::Staging(
                "base root must not be the host root".into(),
            ));
        }
        if self.staging_parent == Path::new("/") {
            return Err(RuntimeError::Staging(
                "staging parent must not be the host root".into(),
            ));
        }
        if self.staging_parent == self.base_root || self.staging_parent.starts_with(&self.base_root)
        {
            return Err(RuntimeError::Staging(
                "staging parent must be outside base root".into(),
            ));
        }
        for root in &self.trusted_source_roots {
            if root == Path::new("/") || root.starts_with(&self.staging_parent) {
                return Err(RuntimeError::Staging(
                    "trusted source root must not be host root or staging content".into(),
                ));
            }
            if !root.is_absolute() || has_navigation_syntax(root) {
                return Err(RuntimeError::Staging(
                    "trusted source roots must be absolute and normalized".into(),
                ));
            }
        }
        Ok(())
    }

    fn has_navigation_components(&self) -> bool {
        has_navigation_syntax(&self.base_root)
            || has_navigation_syntax(&self.staging_parent)
            || self
                .trusted_source_roots
                .iter()
                .any(|root| has_navigation_syntax(root))
    }
}

fn has_navigation_syntax(path: &Path) -> bool {
    path.to_string_lossy()
        .split('/')
        .any(|component| component == "." || component == "..")
}

#[cfg(unix)]
fn restrict_staging_root(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn restrict_staging_root(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

/// A newly-created staging root is removed unless ownership is explicitly
/// committed to the running process maps. This keeps every early-return path
/// fail-closed without duplicating cleanup branches.
struct PendingStagingRoot {
    path: PathBuf,
    committed: bool,
}

impl PendingStagingRoot {
    fn create(parent: &Path, instance: ApplicationInstanceId) -> Result<Self, RuntimeError> {
        fs::create_dir_all(parent).map_err(|error| RuntimeError::Staging(error.to_string()))?;
        let path = parent.join(format!("instance-{}", instance.get()));
        if path.exists() {
            return Err(RuntimeError::Staging(format!(
                "staging root already exists: {}",
                path.display()
            )));
        }
        fs::create_dir(&path).map_err(|error| RuntimeError::Staging(error.to_string()))?;
        if let Err(error) = restrict_staging_root(&path) {
            let _ = fs::remove_dir(&path);
            return Err(RuntimeError::Staging(format!(
                "restrict staging root permissions {}: {error}",
                path.display()
            )));
        }
        Ok(Self {
            path,
            committed: false,
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn commit(mut self) -> PathBuf {
        self.committed = true;
        self.path.clone()
    }
}

impl Drop for PendingStagingRoot {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_dir_all(&self.path);
            if let (Some(parent), Some(name)) = (self.path.parent(), self.path.file_name()) {
                let support =
                    parent.join(format!(".luna-volume-mounts-{}", name.to_string_lossy()));
                let _ = fs::remove_dir_all(support);
            }
        }
    }
}

#[cfg(unix)]
pub trait ApplicationPlanLauncher {
    /// Launch an already-authorized plan for the same currently active session
    /// against which the plan was validated.
    ///
    /// No policy evaluation occurs here. The caller must obtain the
    /// `AuthorizedApplicationPlan` from `luna-security` first.
    ///
    /// A plain `ApplicationPlan` cannot cross this boundary:
    ///
    /// ```compile_fail
    /// use luna_app_runtime::{
    ///     ApplicationLaunchContext, ApplicationPlan, ApplicationPlanLauncher,
    ///     LinuxApplicationRuntime,
    /// };
    /// use luna_system_runtime::SystemRuntimeService;
    /// use luna_user_session::UserSession;
    ///
    /// fn invalid_launch(
    ///     launcher: &mut LinuxApplicationRuntime,
    ///     plan: ApplicationPlan,
    ///     session: &UserSession,
    ///     runtime: &mut SystemRuntimeService,
    ///     context: &ApplicationLaunchContext,
    /// ) {
    ///     launcher.launch_authorized_plan(plan, session, runtime, context);
    /// }
    /// ```
    fn launch_authorized_plan(
        &mut self,
        plan: AuthorizedApplicationPlan,
        session: &UserSession,
        runtime: &mut SystemRuntimeService,
        context: &ApplicationLaunchContext,
    ) -> Result<ApplicationInstanceId, RuntimeError>;
}

#[cfg(unix)]
impl ApplicationPlanLauncher for LinuxApplicationRuntime {
    fn launch_authorized_plan(
        &mut self,
        plan: AuthorizedApplicationPlan,
        session: &UserSession,
        runtime: &mut SystemRuntimeService,
        context: &ApplicationLaunchContext,
    ) -> Result<ApplicationInstanceId, RuntimeError> {
        InMemoryApplicationRuntime::validate_session(plan.value().session(), session)?;
        context.validate()?;
        validate_mapping_access(&plan)?;
        validate_capabilities(&plan)?;

        let program = plan.value().executable().path().to_str().ok_or_else(|| {
            RuntimeError::InvalidExecutable(plan.value().executable().path().display().to_string())
        })?;

        let mapping = plan.value().mapping().clone();
        let volume_resolver = context.volume_resolver().cloned();
        if mapping
            .iter()
            .any(|rule| matches!(rule.source(), MappingSource::Volume(_)))
            && volume_resolver.is_none()
        {
            return Err(RuntimeError::Security(
                "Volume mapping requires a Device Manager resolver snapshot".into(),
            ));
        }
        validate_volume_mapping_layout(&mapping)
            .map_err(|error| RuntimeError::Security(error.to_string()))?;

        let id = self.model.allocate_instance_id();
        let mut instance = ApplicationInstance::new_with_runtime(
            id,
            plan.value().application().clone(),
            plan.value().version(),
            plan.value().session(),
            plan.value().runtime(),
        );
        instance.transition(InstanceState::Starting)?;
        self.model.insert(instance);

        let staging = match PendingStagingRoot::create(context.staging_parent(), id) {
            Ok(staging) => staging,
            Err(error) => {
                self.model
                    .instance_mut(id)?
                    .record_failure(FailureStage::Starting, error.to_string())?;
                return Err(error);
            }
        };

        let base_root = context.base_root().to_path_buf();
        let trusted_source_roots = context.trusted_source_roots().to_vec();
        let root_for_child = staging.path().to_path_buf();
        let args = plan.value().executable().args().to_vec();
        let namespace = context.namespace();
        let profile = RuntimeProfile::minimal();
        let process = runtime.spawn_process_with_pre_exec(program, args, move || {
            LinuxMountNamespace::enter_private()
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            let logical = materialize_profiled_logical_root(
                &root_for_child,
                &base_root,
                &trusted_source_roots,
                &mapping,
                &profile,
            )
            .map_err(|error| std::io::Error::other(error.to_string()))?;

            if let Some(resolver) = volume_resolver.as_ref() {
                let mut volume_mounts = VolumeMountManager::new();
                materialize_volume_mappings(
                    &mut volume_mounts,
                    &namespace,
                    resolver,
                    logical.path(),
                    &mapping,
                )
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            }

            namespace
                .enter_logical_root(&logical)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            namespace
                .enforce_filesystem_access(&profile, &mapping)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            Ok(())
        });

        let process = match process {
            Ok(process) => process,
            Err(error) => {
                self.model
                    .instance_mut(id)?
                    .record_failure(FailureStage::Starting, error.to_string())?;
                return Err(error.into());
            }
        };

        let attach_result = self.model.instance_mut(id)?.attach_process(process);
        if let Err(error) = attach_result {
            let _ = runtime.terminate_supervised_process(process);
            self.model
                .instance_mut(id)?
                .record_failure(FailureStage::Starting, error.to_string())?;
            return Err(error);
        }

        let transition_result = self
            .model
            .instance_mut(id)?
            .transition(InstanceState::Running);
        if let Err(error) = transition_result {
            let _ = runtime.terminate_supervised_process(process);
            self.model
                .instance_mut(id)?
                .record_failure(FailureStage::Starting, error.to_string())?;
            return Err(error);
        }

        let root = staging.commit();
        self.processes.insert(process, id);
        self.roots.insert(process, root);
        Ok(id)
    }
}

#[cfg(unix)]
fn validate_volume_mapping_layout(mapping: &MappingTable) -> Result<(), NamespaceError> {
    for volume_rule in mapping.iter().filter(|rule| {
        !rule.access().is_empty() && matches!(rule.source(), MappingSource::Volume(_))
    }) {
        for other in mapping.iter().filter(|rule| !rule.access().is_empty()) {
            if volume_rule.logical() == other.logical() {
                continue;
            }
            let volume_path = volume_rule.logical().as_path();
            let other_path = other.logical().as_path();
            if volume_path.starts_with(other_path) || other_path.starts_with(volume_path) {
                return Err(NamespaceError::FilesystemAccess(format!(
                    "overlapping mapping targets cannot be materialized with Volume mounts: {} and {}",
                    volume_rule.logical(),
                    other.logical()
                )));
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn materialize_volume_mappings(
    mounts: &mut VolumeMountManager,
    namespace: &LinuxMountNamespace,
    resolver: &impl VolumeMountResolver,
    root: &Path,
    mapping: &MappingTable,
) -> Result<Vec<MountInstance>, NamespaceError> {
    validate_volume_mapping_layout(mapping)?;
    let mut instances = Vec::new();
    for rule in mapping.iter() {
        if rule.access().is_empty() {
            continue;
        }
        let MappingSource::Volume(path) = rule.source() else {
            continue;
        };
        instances.push(mounts.mount_volume(
            namespace,
            resolver,
            root,
            rule.logical().as_path(),
            path,
            rule.kind(),
            !rule.access().contains(&ResourceAccess::Write),
        )?);
    }
    Ok(instances)
}

#[cfg(unix)]
fn validate_mapping_access(plan: &AuthorizedApplicationPlan) -> Result<(), RuntimeError> {
    for resource in plan.value().manifest().resources() {
        let logical = LogicalPath::new(resource.logical_path()).map_err(RuntimeError::Mapping)?;
        let rule = plan
            .value()
            .mapping()
            .resolve_rule(&logical)
            .map_err(RuntimeError::Mapping)?;
        if rule.access() != resource.access() {
            return Err(RuntimeError::Security(format!(
                "mapping access does not match bundle declaration: {}",
                resource.logical_path()
            )));
        }
    }

    for rule in plan.value().mapping().iter() {
        let declared = plan.value().manifest().resources().iter().any(|resource| {
            resource.logical_path() == rule.logical().as_str()
                || (rule.kind() == MappingKind::Subtree
                    && resource.logical_path().starts_with(rule.logical().as_str())
                    && resource.logical_path()[rule.logical().as_str().len()..].starts_with('/'))
        });
        if !declared {
            return Err(RuntimeError::Security(format!(
                "mapping is not declared by the bundle: {}",
                rule.logical()
            )));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn validate_capabilities(plan: &AuthorizedApplicationPlan) -> Result<(), RuntimeError> {
    let registry = CapabilityRegistry::with_default_providers();
    for capability in plan.value().manifest().capabilities() {
        let name = CapabilityName::new(capability.to_owned())
            .map_err(|error| RuntimeError::Security(error.to_string()))?;
        // The plan already carries policy authorization. Registry lookup only
        // verifies that the approved capability has a provider.
        if registry.provider_for(&name).is_none() {
            return Err(RuntimeError::Security(format!(
                "unknown capability provider: {name}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ApplicationLaunchContext, PendingStagingRoot, VolumeManagerResolver,
        validate_volume_mapping_layout,
    };
    use crate::{ApplicationInstanceId, RuntimeError};
    use luna_namespace::LinuxMountNamespace;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn launch_context_accepts_distinct_absolute_roots() {
        let context = ApplicationLaunchContext::new(
            LinuxMountNamespace,
            Path::new("/luna/system"),
            Path::new("/luna/data/runtime"),
        );
        assert!(context.validate().is_ok());
    }

    #[test]
    fn launch_context_rejects_relative_roots() {
        let context = ApplicationLaunchContext::new(
            LinuxMountNamespace,
            Path::new("system"),
            Path::new("/luna/data/runtime"),
        );
        assert!(matches!(context.validate(), Err(RuntimeError::Staging(_))));
    }

    #[test]
    fn launch_context_rejects_navigation_components() {
        let context = ApplicationLaunchContext::new(
            LinuxMountNamespace,
            Path::new("/luna/./system"),
            Path::new("/luna/data/runtime"),
        );
        assert!(matches!(context.validate(), Err(RuntimeError::Staging(_))));

        let context = ApplicationLaunchContext::new(
            LinuxMountNamespace,
            Path::new("/luna/system"),
            Path::new("/luna/data/../runtime"),
        );
        assert!(matches!(context.validate(), Err(RuntimeError::Staging(_))));
    }

    #[test]
    fn launch_context_rejects_staging_inside_base_root_boundary() {
        let context = ApplicationLaunchContext::new(
            LinuxMountNamespace,
            Path::new("/luna/system"),
            Path::new("/luna/system/runtime"),
        );
        assert!(matches!(context.validate(), Err(RuntimeError::Staging(_))));
    }

    #[test]
    fn launch_context_rejects_staging_equal_to_base_root() {
        let context = ApplicationLaunchContext::new(
            LinuxMountNamespace,
            Path::new("/luna/system"),
            Path::new("/luna/system"),
        );
        assert!(matches!(context.validate(), Err(RuntimeError::Staging(_))));
    }

    #[test]
    fn launch_context_rejects_host_root_as_base_root() {
        let context = ApplicationLaunchContext::new(
            LinuxMountNamespace,
            Path::new("/"),
            Path::new("/luna/data/runtime"),
        );
        assert!(matches!(context.validate(), Err(RuntimeError::Staging(_))));
    }

    #[test]
    fn launch_context_rejects_host_root_as_staging_parent() {
        let context = ApplicationLaunchContext::new(
            LinuxMountNamespace,
            Path::new("/luna/system"),
            Path::new("/"),
        );
        assert!(matches!(context.validate(), Err(RuntimeError::Staging(_))));
    }

    #[test]
    fn launch_context_rejects_implicit_source_roots() {
        let context = ApplicationLaunchContext::new(
            LinuxMountNamespace,
            Path::new("/luna/system"),
            Path::new("/luna/data/runtime"),
        )
        .with_trusted_source_root(Path::new("/"));
        assert!(matches!(context.validate(), Err(RuntimeError::Staging(_))));

        let context = ApplicationLaunchContext::new(
            LinuxMountNamespace,
            Path::new("/luna/system"),
            Path::new("/luna/data/runtime"),
        )
        .with_trusted_source_root(Path::new("/luna/data/../apps"));
        assert!(matches!(context.validate(), Err(RuntimeError::Staging(_))));
    }

    #[test]
    fn launch_context_accepts_explicit_trusted_source_root() {
        let context = ApplicationLaunchContext::new(
            LinuxMountNamespace,
            Path::new("/luna/system"),
            Path::new("/luna/data/runtime"),
        )
        .with_trusted_source_root(Path::new("/luna/data/apps"));
        assert!(context.validate().is_ok());
    }

    #[test]
    fn overlapping_volume_and_physical_mapping_targets_fail_closed() {
        use luna_common::{ResourceAccess, VolumeId, VolumePath, VolumeRef};
        use luna_root_mapping::{LogicalPath, MappingRule, MappingTable, PhysicalPath};

        let volume = VolumeRef::new(VolumeId::from("partition-guid:data"));
        let mut overlapping = MappingTable::new();
        overlapping
            .insert(
                MappingRule::volume_subtree(
                    LogicalPath::new("/home/docs").unwrap(),
                    VolumePath::new(volume.clone(), "/Documents").unwrap(),
                )
                .with_access([ResourceAccess::Read]),
            )
            .unwrap();
        overlapping
            .insert(
                MappingRule::file(
                    LogicalPath::new("/home/docs/readme.txt").unwrap(),
                    PhysicalPath::new("/data/apps/example/readme.txt"),
                )
                .with_access([ResourceAccess::Read]),
            )
            .unwrap();
        assert!(validate_volume_mapping_layout(&overlapping).is_err());

        let mut disjoint = MappingTable::new();
        disjoint
            .insert(
                MappingRule::volume_subtree(
                    LogicalPath::new("/home/docs").unwrap(),
                    VolumePath::new(volume, "/Documents").unwrap(),
                )
                .with_access([ResourceAccess::Read]),
            )
            .unwrap();
        disjoint
            .insert(
                MappingRule::file(
                    LogicalPath::new("/home/photos").unwrap(),
                    PhysicalPath::new("/data/apps/example/photos"),
                )
                .with_access([ResourceAccess::Read]),
            )
            .unwrap();
        assert!(validate_volume_mapping_layout(&disjoint).is_ok());
    }

    #[test]
    fn volume_manager_resolver_adapts_volume_manager_backend() {
        let volume = luna_volume_manager::VolumeInfo::discovered(
            luna_common::VolumeId::from("partition-guid:usb"),
            Some("USB".into()),
            "usb".into(),
            Some("ext4".into()),
            luna_common::VolumeRole::External,
            luna_volume_manager::VolumeState::Available,
            "/dev/null".into(),
        );
        let id = volume.id().clone();
        let mut manager = luna_volume_manager::VolumeManager::new();
        manager.register_volume(volume).unwrap();

        let resolver = VolumeManagerResolver::new(&manager);
        let resolved = luna_namespace::VolumeMountResolver::resolve_volume(
            &resolver,
            &luna_common::VolumeRef::new(id.clone()),
        )
        .unwrap();
        assert_eq!(resolved.volume().id(), &id);
        assert_eq!(resolved.backend_node(), Path::new("/dev/null"));
        assert_eq!(resolved.filesystem(), "ext4");

        use luna_common::{ResourceAccess, VolumePath};
        use luna_root_mapping::{LogicalPath, MappingRule, MappingTable};
        let mut mapping = MappingTable::new();
        mapping
            .insert(
                MappingRule::volume_subtree(
                    LogicalPath::new("/media/docs").unwrap(),
                    VolumePath::new(luna_common::VolumeRef::new(id.clone()), "/Documents").unwrap(),
                )
                .with_access([ResourceAccess::Read]),
            )
            .unwrap();
        mapping
            .insert(MappingRule::volume(
                LogicalPath::new("/media/ignored").unwrap(),
                VolumePath::new(
                    luna_common::VolumeRef::new(luna_common::VolumeId::from("not-registered")),
                    "/ignored.txt",
                )
                .unwrap(),
            ))
            .unwrap();
        let snapshot = resolver.snapshot_for_mapping(&mapping).unwrap();
        assert_eq!(snapshot.len(), 1);
        assert!(
            luna_namespace::VolumeMountResolver::resolve_volume(
                &snapshot,
                &luna_common::VolumeRef::new(luna_common::VolumeId::from("not-registered")),
            )
            .is_err()
        );
    }

    #[test]
    fn uncommitted_staging_root_is_cleaned_up() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let parent = std::env::temp_dir().join(format!(
            "luna-app-runtime-staging-{}-{nonce}",
            std::process::id()
        ));
        let path;
        {
            let staging = PendingStagingRoot::create(&parent, ApplicationInstanceId::new(11))
                .expect("create staging root");
            path = staging.path().to_path_buf();
            assert!(path.exists());
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
                assert_eq!(mode, 0o700);
            }
        }
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(parent);
    }
}
