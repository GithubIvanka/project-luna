//! Process-launch integration for [`ApplicationPlan`].
//!
//! Authorization is intentionally completed before this module is entered.
//! The launcher only consumes an `AuthorizedApplicationPlan`, validates that
//! its mapping/capability declarations still match execution policy, builds a
//! profile-driven logical root in a private mount namespace, installs
//! filesystem enforcement, and registers the supervised process.

use std::fs;
use std::path::{Component, Path, PathBuf};

use luna_common::RuntimeProfile;
use luna_namespace::{LinuxMountNamespace, materialize_profiled_logical_root};
use luna_root_mapping::{LogicalPath, MappingKind};
use luna_security::{CapabilityName, CapabilityRegistry, Principal};
use luna_system_runtime::SystemRuntimeService;

use crate::application_plan::AuthorizedApplicationPlan;
use crate::{
    ApplicationInstance, ApplicationInstanceId, InstanceState, LinuxApplicationRuntime,
    RuntimeError,
};

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
        }
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

    /// Validate filesystem roots before any staging directory or child process
    /// is created.
    ///
    /// Both roots must be absolute and lexically normalized. Runtime staging
    /// must live outside the immutable System Image tree and must never target
    /// the host root.
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
        Ok(())
    }

    fn has_navigation_components(&self) -> bool {
        self.base_root
            .components()
            .chain(self.staging_parent.components())
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    }
}

#[cfg(unix)]
pub trait ApplicationPlanLauncher {
    /// Launch an already-authorized plan in an explicit execution context.
    ///
    /// No policy evaluation occurs here. The caller must obtain the
    /// `AuthorizedApplicationPlan` from `ApplicationPlan::authorize` first.
    fn launch_authorized_plan(
        &mut self,
        plan: AuthorizedApplicationPlan,
        runtime: &mut SystemRuntimeService,
        context: &ApplicationLaunchContext,
    ) -> Result<ApplicationInstanceId, RuntimeError>;
}

#[cfg(unix)]
impl ApplicationPlanLauncher for LinuxApplicationRuntime {
    fn launch_authorized_plan(
        &mut self,
        plan: AuthorizedApplicationPlan,
        runtime: &mut SystemRuntimeService,
        context: &ApplicationLaunchContext,
    ) -> Result<ApplicationInstanceId, RuntimeError> {
        context.validate()?;
        validate_mapping_access(&plan)?;
        validate_capabilities(&plan)?;

        let program = plan.executable().path().to_str().ok_or_else(|| {
            RuntimeError::InvalidExecutable(plan.executable().path().display().to_string())
        })?;

        fs::create_dir_all(context.staging_parent())
            .map_err(|error| RuntimeError::Staging(error.to_string()))?;

        let id = ApplicationInstanceId::new(self.model.next_id);
        let root = context
            .staging_parent()
            .join(format!("instance-{}", id.get()));
        if root.exists() {
            return Err(RuntimeError::Staging(format!(
                "staging root already exists: {}",
                root.display()
            )));
        }
        fs::create_dir(&root).map_err(|error| RuntimeError::Staging(error.to_string()))?;

        let mapping = plan.mapping().clone();
        let base_root = context.base_root().to_path_buf();
        let root_for_child = root.clone();
        let args = plan.executable().args().to_vec();
        let namespace = context.namespace();
        let profile = RuntimeProfile::minimal();
        let process = runtime.spawn_process_with_pre_exec(program, args, move || {
            LinuxMountNamespace::enter_private()
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            let logical = materialize_profiled_logical_root(
                &namespace,
                &root_for_child,
                &base_root,
                &mapping,
                &profile,
            )
            .map_err(|error| std::io::Error::other(error.to_string()))?;
            namespace
                .enter_logical_root(&logical)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            namespace
                .enforce_filesystem_access(&mapping)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            Ok(())
        });

        let process = match process {
            Ok(process) => process,
            Err(error) => {
                let _ = fs::remove_dir_all(&root);
                return Err(error.into());
            }
        };

        self.model.next_id = self.model.next_id.saturating_add(1);
        let mut instance = ApplicationInstance::new_with_runtime(
            id,
            plan.application().clone(),
            plan.version(),
            plan.session(),
            plan.runtime(),
        );

        if let Err(error) = instance.attach_process(process) {
            let _ = runtime.terminate_supervised_process(process);
            let _ = fs::remove_dir_all(&root);
            return Err(error);
        }

        if let Err(error) = instance.transition(InstanceState::Running) {
            let _ = runtime.terminate_supervised_process(process);
            let _ = fs::remove_dir_all(&root);
            return Err(error);
        }

        self.model.instances.insert(id, instance);
        self.processes.insert(process, id);
        self.roots.insert(process, root);
        Ok(id)
    }
}

#[cfg(unix)]
fn validate_mapping_access(plan: &AuthorizedApplicationPlan) -> Result<(), RuntimeError> {
    for resource in plan.manifest().resources() {
        let logical = LogicalPath::new(resource.logical_path()).map_err(RuntimeError::Mapping)?;
        let rule = plan
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

    for rule in plan.mapping().iter() {
        let declared = plan.manifest().resources().iter().any(|resource| {
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
    let principal = Principal::Application(plan.application().clone());
    for capability in plan.manifest().capabilities() {
        let name = CapabilityName::new(capability.to_owned())
            .map_err(|error| RuntimeError::Security(error.to_string()))?;
        // The plan has already passed policy authorization. This second step
        // only verifies that the approved capability has a registered provider.
        registry
            .grant(principal.clone(), name)
            .map_err(|error| RuntimeError::Security(error.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ApplicationLaunchContext;
    use crate::RuntimeError;
    use luna_namespace::LinuxMountNamespace;
    use std::path::Path;

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
}
