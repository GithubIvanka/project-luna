// Application execution runtime boundary for Project Luna.
// Application process ownership belongs to `luna-system-runtime`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

use luna_bundle::validate_manifest;
use luna_common::{BundleId, RuntimeKind, RuntimeSpec, Version};
use luna_namespace::{LinuxMountNamespace, LogicalRoot, NamespaceError};
use luna_root_mapping::{LogicalPath, MappingError, MappingTable};
use luna_security::{
    AuthorizationRequest, Decision, Permission, PolicyAuthority, Principal, Resource,
};
use luna_system_runtime::{ProcessError, ProcessId, ProcessState, SystemRuntimeService};
use luna_user_session::{SessionId, SessionState, UserSession};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ApplicationInstanceId(u128);
impl ApplicationInstanceId {
    pub const fn new(value: u128) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u128 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstanceState {
    Created,
    Starting,
    Running,
    Stopping,
    Stopped,
    Crashed,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessExit {
    Exited { code: i32 },
    Signaled { signal: i32 },
    UnknownFailure,
}

impl ProcessExit {
    fn from_status(status: &ExitStatus) -> Self {
        if let Some(code) = status.code() {
            return Self::Exited { code };
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            if let Some(signal) = status.signal() {
                return Self::Signaled { signal };
            }
        }
        Self::UnknownFailure
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApplicationProcess {
    id: ProcessId,
    exit: Option<ProcessExit>,
}

impl ApplicationProcess {
    pub const fn id(&self) -> ProcessId {
        self.id
    }

    pub const fn exit(&self) -> Option<ProcessExit> {
        self.exit
    }

    pub const fn is_running(&self) -> bool {
        self.exit.is_none()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureStage {
    Starting,
    Stopping,
    Supervision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstanceFailure {
    stage: FailureStage,
    message: String,
}

impl InstanceFailure {
    pub fn new(stage: FailureStage, message: impl Into<String>) -> Self {
        Self {
            stage,
            message: message.into(),
        }
    }

    pub const fn stage(&self) -> FailureStage {
        self.stage
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

/// One concrete application execution lifecycle.
///
/// State mutation is intentionally private to `luna-app-runtime`; callers can
/// observe lifecycle state but cannot bypass the validated transition methods.
///
/// ```compile_fail
/// use luna_app_runtime::{ApplicationInstance, ApplicationInstanceId, InstanceState};
/// use luna_common::{BundleId, Version};
/// use luna_user_session::SessionId;
///
/// let mut instance = ApplicationInstance::new(
///     ApplicationInstanceId::new(1),
///     BundleId::from("example.app"),
///     Version::new(1, 0, 0),
///     SessionId::new(1),
/// );
/// instance.transition(InstanceState::Running);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationInstance {
    id: ApplicationInstanceId,
    application: BundleId,
    version: Version,
    session: SessionId,
    runtime: RuntimeSpec,
    state: InstanceState,
    process: Option<ApplicationProcess>,
    failure: Option<InstanceFailure>,
}
impl ApplicationInstance {
    pub fn new(
        id: ApplicationInstanceId,
        application: BundleId,
        version: Version,
        session: SessionId,
    ) -> Self {
        Self::new_with_runtime(id, application, version, session, RuntimeSpec::default())
    }
    pub fn new_with_runtime(
        id: ApplicationInstanceId,
        application: BundleId,
        version: Version,
        session: SessionId,
        runtime: RuntimeSpec,
    ) -> Self {
        Self {
            id,
            application,
            version,
            session,
            runtime,
            state: InstanceState::Created,
            process: None,
            failure: None,
        }
    }
    pub const fn id(&self) -> ApplicationInstanceId {
        self.id
    }
    pub fn application(&self) -> &BundleId {
        &self.application
    }
    pub const fn version(&self) -> Version {
        self.version
    }
    pub const fn session(&self) -> SessionId {
        self.session
    }
    pub const fn runtime(&self) -> RuntimeSpec {
        self.runtime
    }
    pub const fn state(&self) -> InstanceState {
        self.state
    }
    pub const fn process_info(&self) -> Option<&ApplicationProcess> {
        self.process.as_ref()
    }
    pub const fn process(&self) -> Option<ProcessId> {
        match self.process {
            Some(process) => Some(process.id),
            None => None,
        }
    }
    pub const fn exit(&self) -> Option<ProcessExit> {
        match self.process {
            Some(process) => process.exit,
            None => None,
        }
    }
    pub fn failure(&self) -> Option<&InstanceFailure> {
        self.failure.as_ref()
    }
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            InstanceState::Stopped | InstanceState::Crashed | InstanceState::Failed
        )
    }
    fn active_process(&self) -> Option<ProcessId> {
        self.process
            .filter(ApplicationProcess::is_running)
            .map(|process| process.id)
    }
    pub(crate) fn attach_process(&mut self, process: ProcessId) -> Result<(), RuntimeError> {
        if self.state != InstanceState::Starting {
            return Err(RuntimeError::InvalidTransition {
                from: self.state,
                to: InstanceState::Running,
            });
        }
        if self.process.is_some() {
            return Err(RuntimeError::ProcessAlreadyAttached);
        }
        self.process = Some(ApplicationProcess {
            id: process,
            exit: None,
        });
        Ok(())
    }
    pub(crate) fn transition(&mut self, next: InstanceState) -> Result<(), RuntimeError> {
        let valid = matches!(
            (self.state, next),
            (InstanceState::Created, InstanceState::Starting)
                | (InstanceState::Starting, InstanceState::Running)
                | (InstanceState::Starting, InstanceState::Failed)
                | (InstanceState::Running, InstanceState::Stopping)
                | (InstanceState::Running, InstanceState::Stopped)
                | (InstanceState::Running, InstanceState::Crashed)
                | (InstanceState::Running, InstanceState::Failed)
                | (InstanceState::Stopping, InstanceState::Stopped)
                | (InstanceState::Stopping, InstanceState::Crashed)
                | (InstanceState::Stopping, InstanceState::Failed)
        );
        if !valid {
            return Err(RuntimeError::InvalidTransition {
                from: self.state,
                to: next,
            });
        }
        self.state = next;
        Ok(())
    }
    pub(crate) fn record_failure(
        &mut self,
        stage: FailureStage,
        message: impl Into<String>,
    ) -> Result<(), RuntimeError> {
        self.transition(InstanceState::Failed)?;
        self.failure = Some(InstanceFailure::new(stage, message));
        Ok(())
    }
    pub(crate) fn record_process_exit(&mut self, status: ExitStatus) -> Result<(), RuntimeError> {
        match self.process {
            None => return Err(RuntimeError::NoProcess),
            Some(process) if process.exit.is_some() => {
                return Err(RuntimeError::ProcessAlreadyExited);
            }
            Some(_) => {}
        }
        let outcome = ProcessExit::from_status(&status);
        let next = if status.success() {
            InstanceState::Stopped
        } else {
            InstanceState::Crashed
        };
        self.transition(next)?;
        self.process.as_mut().expect("process checked above").exit = Some(outcome);
        Ok(())
    }
    pub(crate) fn record_requested_stop(&mut self, status: ExitStatus) -> Result<(), RuntimeError> {
        match self.process {
            None => return Err(RuntimeError::NoProcess),
            Some(process) if process.exit.is_some() => {
                return Err(RuntimeError::ProcessAlreadyExited);
            }
            Some(_) => {}
        }
        self.transition(InstanceState::Stopped)?;
        self.process.as_mut().expect("process checked above").exit =
            Some(ProcessExit::from_status(&status));
        Ok(())
    }
}

#[derive(Debug)]
pub enum RuntimeError {
    InvalidTransition {
        from: InstanceState,
        to: InstanceState,
    },
    InvalidBundle(String),
    Mapping(MappingError),
    Namespace(NamespaceError),
    Security(String),
    SessionNotActive,
    SessionMismatch {
        expected: SessionId,
        actual: SessionId,
    },
    InstanceNotFound,
    Process(ProcessError),
    Supervisor(luna_system_runtime::RuntimeError),
    ProcessAlreadyAttached,
    ProcessAlreadyExited,
    NoProcess,
    InvalidExecutable(String),
    Staging(String),
    RuntimeMismatch {
        mapping: Option<RuntimeKind>,
        requested: RuntimeKind,
    },
}
impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTransition { from, to } => {
                write!(
                    f,
                    "invalid application instance transition from {from:?} to {to:?}"
                )
            }
            Self::InvalidBundle(e) => write!(f, "invalid bundle: {e}"),
            Self::Mapping(e) => write!(f, "mapping error: {e}"),
            Self::Namespace(e) => write!(f, "namespace error: {e}"),
            Self::Security(e) => write!(f, "security authorization failed: {e}"),
            Self::SessionNotActive => f.write_str("session is not active"),
            Self::SessionMismatch { expected, actual } => write!(
                f,
                "application plan session {} does not match launch session {}",
                expected.get(),
                actual.get()
            ),
            Self::InstanceNotFound => f.write_str("application instance not found"),
            Self::Process(e) => write!(f, "process supervision failed: {e}"),
            Self::Supervisor(e) => write!(f, "system runtime operation failed: {e}"),
            Self::ProcessAlreadyAttached => {
                f.write_str("application instance already has a process")
            }
            Self::ProcessAlreadyExited => {
                f.write_str("application instance process exit was already recorded")
            }
            Self::NoProcess => f.write_str("application instance has no running process"),
            Self::InvalidExecutable(e) => write!(f, "invalid application executable: {e}"),
            Self::Staging(e) => write!(f, "namespace staging failed: {e}"),
            Self::RuntimeMismatch { mapping, requested } => write!(
                f,
                "runtime mismatch: mapping={mapping:?}, requested={requested:?}"
            ),
        }
    }
}
impl std::error::Error for RuntimeError {}
impl From<ProcessError> for RuntimeError {
    fn from(value: ProcessError) -> Self {
        Self::Process(value)
    }
}
impl From<luna_system_runtime::RuntimeError> for RuntimeError {
    fn from(value: luna_system_runtime::RuntimeError) -> Self {
        Self::Supervisor(value)
    }
}

/// Testable runtime boundary. Launch accepts only the capability-bearing
/// authorized plan type; a plain `ApplicationPlan` cannot cross this API.
pub trait ApplicationRuntime {
    type Error;
    fn launch_authorized(
        &mut self,
        plan: application_plan::AuthorizedApplicationPlan,
        session: &UserSession,
    ) -> Result<ApplicationInstance, Self::Error>;
    fn authorize(
        &self,
        policy: &dyn PolicyAuthority,
        request: &AuthorizationRequest,
    ) -> Result<Decision, Self::Error>;
}

pub struct NamespacePreparation<'a> {
    pub namespace: &'a LinuxMountNamespace,
    pub root: &'a Path,
    pub base_root: &'a Path,
    pub mapping: &'a MappingTable,
    pub policy: &'a dyn PolicyAuthority,
    pub requests: &'a [AuthorizationRequest],
    pub runtime: RuntimeSpec,
}
#[derive(Debug)]
pub struct PreparedApplicationNamespace {
    instance: ApplicationInstanceId,
    root: LogicalRoot,
}
impl PreparedApplicationNamespace {
    pub fn instance(&self) -> ApplicationInstanceId {
        self.instance
    }
    pub fn root(&self) -> &LogicalRoot {
        &self.root
    }
}

#[derive(Default)]
pub struct InMemoryApplicationRuntime {
    pub(crate) next_id: u128,
    pub(crate) instances: BTreeMap<ApplicationInstanceId, ApplicationInstance>,
}
impl InMemoryApplicationRuntime {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            instances: BTreeMap::new(),
        }
    }
    fn validate_mapping_only(
        mapping: &MappingTable,
        runtime: RuntimeSpec,
    ) -> Result<(), RuntimeError> {
        if !mapping.accepts_runtime(runtime.kind()) {
            return Err(RuntimeError::RuntimeMismatch {
                mapping: mapping.runtime(),
                requested: runtime.kind(),
            });
        }
        mapping.materialize().map_err(RuntimeError::Mapping)?;
        Ok(())
    }
    pub(crate) fn validate_session(
        expected: SessionId,
        session: &UserSession,
    ) -> Result<(), RuntimeError> {
        if session.id() != expected {
            return Err(RuntimeError::SessionMismatch {
                expected,
                actual: session.id(),
            });
        }
        if session.state() != SessionState::Active {
            return Err(RuntimeError::SessionNotActive);
        }
        Ok(())
    }
    pub(crate) fn allocate_instance_id(&mut self) -> ApplicationInstanceId {
        let id = ApplicationInstanceId::new(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        id
    }
    pub(crate) fn insert(&mut self, instance: ApplicationInstance) {
        self.instances.insert(instance.id(), instance);
    }
    pub fn prepare_authorized_namespace_for_session(
        &self,
        instance: ApplicationInstanceId,
        preparation: NamespacePreparation<'_>,
    ) -> Result<PreparedApplicationNamespace, RuntimeError> {
        let stored = self
            .instances
            .get(&instance)
            .ok_or(RuntimeError::InstanceNotFound)?;
        if stored.runtime() != preparation.runtime {
            return Err(RuntimeError::RuntimeMismatch {
                mapping: preparation.mapping.runtime(),
                requested: preparation.runtime.kind(),
            });
        }
        Self::validate_mapping_only(preparation.mapping, preparation.runtime)?;
        let runtime_request = AuthorizationRequest {
            principal: Principal::Application(stored.application().clone()),
            resource: Resource::Runtime(preparation.runtime.kind()),
            permission: Permission::Use,
        };
        Self::require_allow(preparation.policy, &runtime_request)?;
        for request in preparation.requests {
            Self::require_allow(preparation.policy, request)?;
        }
        let root = preparation
            .namespace
            .materialize_logical_root(preparation.root, preparation.base_root, preparation.mapping)
            .map_err(RuntimeError::Namespace)?;
        Ok(PreparedApplicationNamespace { instance, root })
    }
    fn require_allow(
        policy: &dyn PolicyAuthority,
        request: &AuthorizationRequest,
    ) -> Result<(), RuntimeError> {
        match policy
            .authorize(request)
            .map_err(|e| RuntimeError::Security(e.to_string()))?
        {
            Decision::Allow => Ok(()),
            Decision::Deny => Err(RuntimeError::Security(format!("denied: {request:?}"))),
            Decision::Ask => Err(RuntimeError::Security(
                "authorization requires user confirmation".into(),
            )),
            Decision::Constrained { constraints } => Err(RuntimeError::Security(format!(
                "constraint enforcement not supplied: {constraints:?}"
            ))),
        }
    }
    pub fn instance(
        &self,
        id: ApplicationInstanceId,
    ) -> Result<&ApplicationInstance, RuntimeError> {
        self.instances
            .get(&id)
            .ok_or(RuntimeError::InstanceNotFound)
    }
    pub(crate) fn instance_mut(
        &mut self,
        id: ApplicationInstanceId,
    ) -> Result<&mut ApplicationInstance, RuntimeError> {
        self.instances
            .get_mut(&id)
            .ok_or(RuntimeError::InstanceNotFound)
    }
    pub fn stop(&mut self, id: ApplicationInstanceId) -> Result<(), RuntimeError> {
        let instance = self.instance_mut(id)?;
        instance.transition(InstanceState::Stopping)?;
        instance.transition(InstanceState::Stopped)
    }
    pub fn fail(&mut self, id: ApplicationInstanceId) -> Result<(), RuntimeError> {
        let instance = self.instance_mut(id)?;
        let stage = match instance.state() {
            InstanceState::Starting => FailureStage::Starting,
            InstanceState::Stopping => FailureStage::Stopping,
            _ => FailureStage::Supervision,
        };
        instance.record_failure(stage, "runtime reported application failure")
    }
}
impl ApplicationRuntime for InMemoryApplicationRuntime {
    type Error = RuntimeError;
    fn launch_authorized(
        &mut self,
        plan: application_plan::AuthorizedApplicationPlan,
        session: &UserSession,
    ) -> Result<ApplicationInstance, Self::Error> {
        Self::validate_session(plan.session(), session)?;
        Self::validate_mapping_only(plan.mapping(), plan.runtime())?;
        validate_manifest(plan.manifest())
            .map_err(|e| RuntimeError::InvalidBundle(e.to_string()))?;
        for resource in plan.manifest().resources() {
            let logical =
                LogicalPath::new(resource.logical_path()).map_err(RuntimeError::Mapping)?;
            plan.mapping()
                .resolve(&logical)
                .map_err(RuntimeError::Mapping)?;
        }
        let id = self.allocate_instance_id();
        let mut instance = ApplicationInstance::new_with_runtime(
            id,
            plan.application().clone(),
            plan.version(),
            plan.session(),
            plan.runtime(),
        );
        instance.transition(InstanceState::Starting)?;
        instance.transition(InstanceState::Running)?;
        self.insert(instance.clone());
        Ok(instance)
    }
    fn authorize(
        &self,
        policy: &dyn PolicyAuthority,
        request: &AuthorizationRequest,
    ) -> Result<Decision, Self::Error> {
        policy
            .authorize(request)
            .map_err(|e| RuntimeError::Security(e.to_string()))
    }
}

pub struct LinuxApplicationRuntime {
    pub(crate) model: InMemoryApplicationRuntime,
    pub(crate) processes: BTreeMap<ProcessId, ApplicationInstanceId>,
    pub(crate) roots: BTreeMap<ProcessId, PathBuf>,
}
impl Default for LinuxApplicationRuntime {
    fn default() -> Self {
        Self::new()
    }
}
impl LinuxApplicationRuntime {
    pub fn new() -> Self {
        Self {
            model: InMemoryApplicationRuntime::new(),
            processes: BTreeMap::new(),
            roots: BTreeMap::new(),
        }
    }
    pub fn instance(
        &self,
        id: ApplicationInstanceId,
    ) -> Result<&ApplicationInstance, RuntimeError> {
        self.model.instance(id)
    }
    pub fn poll(
        &mut self,
        id: ApplicationInstanceId,
        runtime: &mut SystemRuntimeService,
    ) -> Result<InstanceState, RuntimeError> {
        let process = self
            .model
            .instance(id)?
            .active_process()
            .ok_or(RuntimeError::NoProcess)?;
        match runtime.poll_process(process)? {
            ProcessState::Running => Ok(InstanceState::Running),
            ProcessState::Exited(status) => {
                self.processes.remove(&process);
                self.cleanup_root(process);
                let instance = self.model.instance_mut(id)?;
                instance.record_process_exit(status)?;
                Ok(instance.state())
            }
        }
    }
    pub fn terminate(
        &mut self,
        id: ApplicationInstanceId,
        runtime: &mut SystemRuntimeService,
    ) -> Result<(), RuntimeError> {
        let process = self
            .model
            .instance(id)?
            .active_process()
            .ok_or(RuntimeError::NoProcess)?;
        {
            let instance = self.model.instance_mut(id)?;
            if instance.state() == InstanceState::Running {
                instance.transition(InstanceState::Stopping)?;
            }
        }
        let status = match runtime.terminate_supervised_process(process) {
            Ok(status) => status,
            Err(error) => {
                self.model
                    .instance_mut(id)?
                    .record_failure(FailureStage::Stopping, error.to_string())?;
                return Err(error.into());
            }
        };
        self.processes.remove(&process);
        self.cleanup_root(process);
        self.model.instance_mut(id)?.record_requested_stop(status)
    }
    pub fn reconcile(
        &mut self,
        runtime: &mut SystemRuntimeService,
    ) -> Result<Vec<(ApplicationInstanceId, InstanceState)>, RuntimeError> {
        let ids: Vec<_> = self.processes.keys().copied().collect();
        let mut changes = Vec::new();
        for process in ids {
            match runtime.poll_process(process)? {
                ProcessState::Running => {}
                ProcessState::Exited(status) => {
                    if let Some(id) = self.processes.remove(&process) {
                        self.cleanup_root(process);
                        let instance = self.model.instance_mut(id)?;
                        instance.record_process_exit(status)?;
                        changes.push((id, instance.state()));
                    }
                }
            }
        }
        Ok(changes)
    }
    pub(crate) fn cleanup_root(&mut self, process: ProcessId) {
        if let Some(root) = self.roots.remove(&process) {
            let parent = root.parent().unwrap_or(Path::new("/tmp"));
            let name = root.file_name().and_then(|v| v.to_str()).unwrap_or("root");
            let support = parent.join(format!(".luna-namespace-{}-{}", process.get(), name));
            let _ = fs::remove_dir_all(root);
            let _ = fs::remove_dir_all(support);
        }
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::{
        ApplicationInstance, ApplicationInstanceId, FailureStage, InstanceState, ProcessExit,
        RuntimeError,
    };
    use luna_common::{BundleId, Version};
    use luna_system_runtime::ProcessId;
    use luna_user_session::SessionId;
    use std::process::Command;

    fn instance() -> ApplicationInstance {
        ApplicationInstance::new(
            ApplicationInstanceId::new(1),
            BundleId::from("example.app"),
            Version::new(1, 0, 0),
            SessionId::new(7),
        )
    }

    #[test]
    fn normal_lifecycle_transitions_are_allowed() {
        let mut instance = instance();
        assert_eq!(instance.state(), InstanceState::Created);
        instance.transition(InstanceState::Starting).unwrap();
        instance.transition(InstanceState::Running).unwrap();
        instance.transition(InstanceState::Stopping).unwrap();
        instance.transition(InstanceState::Stopped).unwrap();
        assert!(instance.is_terminal());
    }

    #[test]
    fn starting_can_fail() {
        let mut instance = instance();
        instance.transition(InstanceState::Starting).unwrap();
        instance
            .record_failure(FailureStage::Starting, "exec failed")
            .unwrap();
        assert_eq!(instance.state(), InstanceState::Failed);
        assert_eq!(instance.failure().unwrap().stage(), FailureStage::Starting);
    }

    #[test]
    fn stopping_can_fail() {
        let mut instance = instance();
        instance.transition(InstanceState::Starting).unwrap();
        instance.transition(InstanceState::Running).unwrap();
        instance.transition(InstanceState::Stopping).unwrap();
        instance
            .record_failure(FailureStage::Stopping, "termination failed")
            .unwrap();
        assert_eq!(instance.state(), InstanceState::Failed);
    }

    #[test]
    fn running_can_crash_and_records_exit() {
        let mut instance = instance();
        instance.transition(InstanceState::Starting).unwrap();
        instance.attach_process(ProcessId::new(42)).unwrap();
        instance.transition(InstanceState::Running).unwrap();
        let status = Command::new("sh").args(["-c", "exit 17"]).status().unwrap();
        instance.record_process_exit(status).unwrap();
        assert_eq!(instance.state(), InstanceState::Crashed);
        assert_eq!(instance.exit(), Some(ProcessExit::Exited { code: 17 }));
    }

    #[test]
    fn normal_exit_becomes_stopped() {
        let mut instance = instance();
        instance.transition(InstanceState::Starting).unwrap();
        instance.attach_process(ProcessId::new(42)).unwrap();
        instance.transition(InstanceState::Running).unwrap();
        let status = Command::new("sh").args(["-c", "exit 0"]).status().unwrap();
        instance.record_process_exit(status).unwrap();
        assert_eq!(instance.state(), InstanceState::Stopped);
        assert_eq!(instance.exit(), Some(ProcessExit::Exited { code: 0 }));
    }

    #[test]
    fn terminal_states_cannot_return_to_running() {
        let mut stopped = instance();
        stopped.transition(InstanceState::Starting).unwrap();
        stopped.transition(InstanceState::Running).unwrap();
        stopped.transition(InstanceState::Stopping).unwrap();
        stopped.transition(InstanceState::Stopped).unwrap();
        assert!(matches!(
            stopped.transition(InstanceState::Running),
            Err(RuntimeError::InvalidTransition {
                from: InstanceState::Stopped,
                to: InstanceState::Running
            })
        ));

        let mut failed = instance();
        failed.transition(InstanceState::Starting).unwrap();
        failed
            .record_failure(FailureStage::Starting, "setup failed")
            .unwrap();
        assert!(matches!(
            failed.transition(InstanceState::Running),
            Err(RuntimeError::InvalidTransition {
                from: InstanceState::Failed,
                to: InstanceState::Running
            })
        ));
    }

    #[test]
    fn created_cannot_skip_starting() {
        let mut instance = instance();
        assert!(matches!(
            instance.transition(InstanceState::Stopped),
            Err(RuntimeError::InvalidTransition {
                from: InstanceState::Created,
                to: InstanceState::Stopped
            })
        ));
    }
}
