//! Process-launch integration for [`ApplicationPlan`].
//!
//! Authorization is intentionally completed before this module is entered.
//! The launcher only consumes an `AuthorizedApplicationPlan`, materializes the
//! logical root in the child, and registers the supervised process.

use std::fs;
use std::path::Path;

use luna_namespace::LinuxMountNamespace;
use luna_system_runtime::SystemRuntimeService;

use crate::application_plan::AuthorizedApplicationPlan;
use crate::{
    ApplicationInstance, ApplicationInstanceId, InstanceState, LinuxApplicationRuntime,
    RuntimeError,
};

#[cfg(unix)]
pub trait ApplicationPlanLauncher {
    /// Launch an already-authorized plan.
    ///
    /// No policy evaluation occurs here. The caller must obtain the
    /// `AuthorizedApplicationPlan` from `ApplicationPlan::authorize` first.
    fn launch_authorized_plan(
        &mut self,
        plan: AuthorizedApplicationPlan,
        runtime: &mut SystemRuntimeService,
        namespace: LinuxMountNamespace,
        base_root: &Path,
        staging_parent: &Path,
    ) -> Result<ApplicationInstanceId, RuntimeError>;
}

#[cfg(unix)]
impl ApplicationPlanLauncher for LinuxApplicationRuntime {
    fn launch_authorized_plan(
        &mut self,
        plan: AuthorizedApplicationPlan,
        runtime: &mut SystemRuntimeService,
        namespace: LinuxMountNamespace,
        base_root: &Path,
        staging_parent: &Path,
    ) -> Result<ApplicationInstanceId, RuntimeError> {
        let program = plan
            .executable()
            .path()
            .to_str()
            .ok_or_else(|| {
                RuntimeError::InvalidExecutable(plan.executable().path().display().to_string())
            })?;

        fs::create_dir_all(staging_parent)
            .map_err(|error| RuntimeError::Staging(error.to_string()))?;

        let id = ApplicationInstanceId::new(self.model.next_id);
        let root = staging_parent.join(format!("instance-{}", id.get()));
        if root.exists() {
            return Err(RuntimeError::Staging(format!(
                "staging root already exists: {}",
                root.display()
            )));
        }
        fs::create_dir(&root).map_err(|error| RuntimeError::Staging(error.to_string()))?;

        let mapping = plan.mapping().clone();
        let base_root = base_root.to_path_buf();
        let root_for_child = root.clone();
        let args = plan.executable().args().to_vec();
        let process = runtime.spawn_process_with_pre_exec(program, args, move || {
            let logical = namespace
                .materialize_logical_root(&root_for_child, &base_root, &mapping)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            namespace
                .enter_logical_root(&logical)
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
