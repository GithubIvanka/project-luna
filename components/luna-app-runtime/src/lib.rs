//! Application execution runtime boundary for Project Luna.
//! Application process ownership belongs to `luna-system-runtime`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use luna_bundle::{validate_manifest, BundleManifest};
use luna_common::{BundleId, RuntimeKind, RuntimeSpec, Version};
use luna_namespace::{LinuxMountNamespace, LogicalRoot, NamespaceError};
use luna_root_mapping::{LogicalPath, MappingError, MappingTable};
use luna_security::{AuthorizationRequest, Decision, Permission, PolicyAuthority, Principal, Resource};
use luna_system_runtime::{ProcessError, ProcessId, ProcessState, SystemRuntimeService};
use luna_user_session::{SessionId, SessionState, UserSession};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ApplicationInstanceId(u128);
impl ApplicationInstanceId {
    pub const fn new(value: u128) -> Self { Self(value) }
    pub const fn get(self) -> u128 { self.0 }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstanceState { Starting, Running, Stopping, Stopped, Failed }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationInstance {
    id: ApplicationInstanceId,
    application: BundleId,
    version: Version,
    session: SessionId,
    runtime: RuntimeSpec,
    state: InstanceState,
    process: Option<ProcessId>,
}

impl ApplicationInstance {
    pub fn new(id: ApplicationInstanceId, application: BundleId, version: Version, session: SessionId) -> Self {
        Self::new_with_runtime(id, application, version, session, RuntimeSpec::default())
    }
    pub fn new_with_runtime(id: ApplicationInstanceId, application: BundleId, version: Version, session: SessionId, runtime: RuntimeSpec) -> Self {
        Self { id, application, version, session, runtime, state: InstanceState::Starting, process: None }
    }
    pub const fn id(&self) -> ApplicationInstanceId { self.id }
    pub fn application(&self) -> &BundleId { &self.application }
    pub const fn version(&self) -> Version { self.version }
    pub const fn session(&self) -> SessionId { self.session }
    pub const fn runtime(&self) -> RuntimeSpec { self.runtime }
    pub const fn state(&self) -> InstanceState { self.state }
    pub const fn process(&self) -> Option<ProcessId> { self.process }
    pub fn attach_process(&mut self, process: ProcessId) -> Result<(), RuntimeError> {
        if self.process.is_some() { return Err(RuntimeError::ProcessAlreadyAttached); }
        self.process = Some(process);
        Ok(())
    }
    pub fn detach_process(&mut self) -> Option<ProcessId> { self.process.take() }
    pub fn transition(&mut self, next: InstanceState) -> Result<(), RuntimeError> {
        let valid = matches!((self.state, next),
            (InstanceState::Starting, InstanceState::Running) |
            (InstanceState::Starting, InstanceState::Failed) |
            (InstanceState::Running, InstanceState::Stopping) |
            (InstanceState::Running, InstanceState::Stopped) |
            (InstanceState::Running, InstanceState::Failed) |
            (InstanceState::Stopping, InstanceState::Stopped) |
            (InstanceState::Stopping, InstanceState::Failed));
        if !valid { return Err(RuntimeError::InvalidTransition); }
        self.state = next;
        Ok(())
    }
}

#[derive(Debug)]
pub enum RuntimeError {
    InvalidTransition,
    InvalidBundle(String),
    Mapping(MappingError),
    Namespace(NamespaceError),
    Security(String),
    SessionNotActive,
    InstanceNotFound,
    Process(ProcessError),
    Supervisor(luna_system_runtime::RuntimeError),
    ProcessAlreadyAttached,
    NoProcess,
    InvalidExecutable(String),
    Staging(String),
    RuntimeMismatch { mapping: Option<RuntimeKind>, requested: RuntimeKind },
}
impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTransition => f.write_str("invalid application instance transition"),
            Self::InvalidBundle(e) => write!(f, "invalid bundle: {e}"),
            Self::Mapping(e) => write!(f, "mapping error: {e}"),
            Self::Namespace(e) => write!(f, "namespace error: {e}"),
            Self::Security(e) => write!(f, "security authorization failed: {e}"),
            Self::SessionNotActive => f.write_str("session is not active"),
            Self::InstanceNotFound => f.write_str("application instance not found"),
            Self::Process(e) => write!(f, "process supervision failed: {e}"),
            Self::Supervisor(e) => write!(f, "system runtime operation failed: {e}"),
            Self::ProcessAlreadyAttached => f.write_str("application instance already has a process"),
            Self::NoProcess => f.write_str("application instance has no process"),
            Self::InvalidExecutable(e) => write!(f, "invalid application executable: {e}"),
            Self::Staging(e) => write!(f, "namespace staging failed: {e}"),
            Self::RuntimeMismatch { mapping, requested } => write!(f, "runtime mismatch: mapping={mapping:?}, requested={requested}"),
        }
    }
}
impl std::error::Error for RuntimeError {}
impl From<ProcessError> for RuntimeError { fn from(value: ProcessError) -> Self { Self::Process(value) } }
impl From<luna_system_runtime::RuntimeError> for RuntimeError { fn from(value: luna_system_runtime::RuntimeError) -> Self { Self::Supervisor(value) } }

pub trait ApplicationRuntime {
    type Error;
    fn launch(&mut self, manifest: &BundleManifest, mapping: &MappingTable) -> Result<ApplicationInstance, Self::Error>;
    fn authorize(&self, policy: &dyn PolicyAuthority, request: &AuthorizationRequest) -> Result<Decision, Self::Error>;
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
pub struct PreparedApplicationNamespace { instance: ApplicationInstanceId, root: LogicalRoot }
impl PreparedApplicationNamespace {
    pub fn instance(&self) -> ApplicationInstanceId { self.instance }
    pub fn root(&self) -> &LogicalRoot { &self.root }
}

#[derive(Default)]
pub struct InMemoryApplicationRuntime { next_id: u128, instances: BTreeMap<ApplicationInstanceId, ApplicationInstance> }
impl InMemoryApplicationRuntime {
    pub fn new() -> Self { Self { next_id: 1, instances: BTreeMap::new() } }
    fn validate_resources(manifest: &BundleManifest, mapping: &MappingTable, runtime: RuntimeSpec) -> Result<(), RuntimeError> {
        validate_manifest(manifest).map_err(|e| RuntimeError::InvalidBundle(e.to_string()))?;
        if !mapping.accepts_runtime(runtime.kind()) { return Err(RuntimeError::RuntimeMismatch { mapping: mapping.runtime(), requested: runtime.kind() }); }
        for resource in manifest.resources() {
            let logical = LogicalPath::new(resource.logical_path()).map_err(RuntimeError::Mapping)?;
            mapping.resolve(&logical).map_err(|_| RuntimeError::Mapping(MappingError::NotMapped))?;
        }
        mapping.materialize().map_err(RuntimeError::Mapping)?;
        Ok(())
    }
    pub fn launch_for_session(&mut self, manifest: &BundleManifest, mapping: &MappingTable, session: &UserSession) -> Result<ApplicationInstanceId, RuntimeError> {
        self.launch_for_session_with_runtime(manifest, mapping, session, RuntimeSpec::default())
    }
    pub fn launch_for_session_with_runtime(&mut self, manifest: &BundleManifest, mapping: &MappingTable, session: &UserSession, runtime: RuntimeSpec) -> Result<ApplicationInstanceId, RuntimeError> {
        if session.state() != SessionState::Active { return Err(RuntimeError::SessionNotActive); }
        Self::validate_resources(manifest, mapping, runtime)?;
        let id = ApplicationInstanceId::new(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        let mut instance = ApplicationInstance::new_with_runtime(id, manifest.metadata().id().clone(), manifest.metadata().version(), session.id(), runtime);
        instance.transition(InstanceState::Running)?;
        self.instances.insert(id, instance);
        Ok(id)
    }
    pub fn prepare_authorized_namespace_for_session(&self, instance: ApplicationInstanceId, preparation: NamespacePreparation<'_>) -> Result<PreparedApplicationNamespace, RuntimeError> {
        let stored = self.instances.get(&instance).ok_or(RuntimeError::InstanceNotFound)?;
        if stored.runtime() != preparation.runtime { return Err(RuntimeError::RuntimeMismatch { mapping: preparation.mapping.runtime(), requested: preparation.runtime.kind() }); }
        Self::validate_mapping_only(preparation.mapping, preparation.runtime)?;
        let runtime_request = AuthorizationRequest { principal: Principal::Application(stored.application().clone()), resource: Resource::Runtime(preparation.runtime.kind()), permission: Permission::Use };
        Self::require_allow(preparation.policy, &runtime_request)?;
        for request in preparation.requests { Self::require_allow(preparation.policy, request)?; }
        let root = preparation.namespace.materialize_logical_root(preparation.root, preparation.base_root, preparation.mapping).map_err(RuntimeError::Namespace)?;
        Ok(PreparedApplicationNamespace { instance, root })
    }
    pub fn prepare_namespace_for_session(&self, instance: ApplicationInstanceId, namespace: &LinuxMountNamespace, root: &Path, base_root: &Path, mapping: &MappingTable) -> Result<PreparedApplicationNamespace, RuntimeError> {
        let stored = self.instances.get(&instance).ok_or(RuntimeError::InstanceNotFound)?;
        if !mapping.accepts_runtime(stored.runtime().kind()) { return Err(RuntimeError::RuntimeMismatch { mapping: mapping.runtime(), requested: stored.runtime().kind() }); }
        Self::validate_mapping_only(mapping, stored.runtime())?;
        let logical_root = namespace.materialize_logical_root(root, base_root, mapping).map_err(RuntimeError::Namespace)?;
        Ok(PreparedApplicationNamespace { instance, root: logical_root })
    }
    fn require_allow(policy: &dyn PolicyAuthority, request: &AuthorizationRequest) -> Result<(), RuntimeError> {
        match policy.authorize(request).map_err(|e| RuntimeError::Security(e.to_string()))? {
            Decision::Allow => Ok(()),
            Decision::Deny => Err(RuntimeError::Security(format!("denied: {request:?}"))),
            Decision::Ask => Err(RuntimeError::Security("authorization requires user confirmation".into())),
            Decision::Constrained { constraints } => Err(RuntimeError::Security(format!("constraint enforcement not supplied: {constraints:?}"))),
        }
    }
    fn validate_mapping_only(mapping: &MappingTable, runtime: RuntimeSpec) -> Result<(), RuntimeError> {
        if !mapping.accepts_runtime(runtime.kind()) { return Err(RuntimeError::RuntimeMismatch { mapping: mapping.runtime(), requested: runtime.kind() }); }
        mapping.materialize().map_err(RuntimeError::Mapping)?;
        Ok(())
    }
    pub fn instance(&self, id: ApplicationInstanceId) -> Result<&ApplicationInstance, RuntimeError> { self.instances.get(&id).ok_or(RuntimeError::InstanceNotFound) }
    pub fn instance_mut(&mut self, id: ApplicationInstanceId) -> Result<&mut ApplicationInstance, RuntimeError> { self.instances.get_mut(&id).ok_or(RuntimeError::InstanceNotFound) }
    pub fn stop(&mut self, id: ApplicationInstanceId) -> Result<(), RuntimeError> { let instance = self.instances.get_mut(&id).ok_or(RuntimeError::InstanceNotFound)?; instance.transition(InstanceState::Stopping)?; instance.transition(InstanceState::Stopped) }
    pub fn fail(&mut self, id: ApplicationInstanceId) -> Result<(), RuntimeError> { self.instances.get_mut(&id).ok_or(RuntimeError::InstanceNotFound)?.transition(InstanceState::Failed) }
}
impl ApplicationRuntime for InMemoryApplicationRuntime {
    type Error = RuntimeError;
    fn launch(&mut self, manifest: &BundleManifest, mapping: &MappingTable) -> Result<ApplicationInstance, Self::Error> {
        Self::validate_resources(manifest, mapping, RuntimeSpec::default())?;
        let id = ApplicationInstanceId::new(self.next_id); self.next_id = self.next_id.saturating_add(1);
        let mut instance = ApplicationInstance::new(id, manifest.metadata().id().clone(), manifest.metadata().version(), SessionId::new(0));
        instance.transition(InstanceState::Running)?; self.instances.insert(id, instance.clone()); Ok(instance)
    }
    fn authorize(&self, policy: &dyn PolicyAuthority, request: &AuthorizationRequest) -> Result<Decision, Self::Error> { policy.authorize(request).map_err(|e| RuntimeError::Security(e.to_string())) }
}

/// Real Linux application launcher. Process ownership remains in system-runtime.
pub struct LinuxApplicationRuntime { model: InMemoryApplicationRuntime, processes: BTreeMap<ProcessId, ApplicationInstanceId>, roots: BTreeMap<ProcessId, PathBuf> }
impl Default for LinuxApplicationRuntime { fn default() -> Self { Self::new() } }
impl LinuxApplicationRuntime {
    pub fn new() -> Self { Self { model: InMemoryApplicationRuntime::new(), processes: BTreeMap::new(), roots: BTreeMap::new() } }
    pub fn instance(&self, id: ApplicationInstanceId) -> Result<&ApplicationInstance, RuntimeError> { self.model.instance(id) }
    #[cfg(unix)]
    pub fn launch_authorized<I, S>(&mut self, manifest: &BundleManifest, mapping: &MappingTable, session: &UserSession, policy: &dyn PolicyAuthority, requests: &[AuthorizationRequest], runtime_spec: RuntimeSpec, runtime: &mut SystemRuntimeService, namespace: LinuxMountNamespace, base_root: &Path, staging_parent: &Path, program: &str, args: I) -> Result<ApplicationInstanceId, RuntimeError>
    where I: IntoIterator<Item = S>, S: AsRef<std::ffi::OsStr>
    {
        if !program.starts_with('/') || program.contains("..") || program.contains('\0') { return Err(RuntimeError::InvalidExecutable(program.to_owned())); }
        if session.state() != SessionState::Active { return Err(RuntimeError::SessionNotActive); }
        InMemoryApplicationRuntime::validate_resources(manifest, mapping, runtime_spec)?;
        let runtime_request = AuthorizationRequest { principal: Principal::Application(manifest.metadata().id().clone()), resource: Resource::Runtime(runtime_spec.kind()), permission: Permission::Use };
        InMemoryApplicationRuntime::require_allow(policy, &runtime_request)?;
        for request in requests { InMemoryApplicationRuntime::require_allow(policy, request)?; }
        fs::create_dir_all(staging_parent).map_err(|e| RuntimeError::Staging(e.to_string()))?;
        let id = ApplicationInstanceId::new(self.model.next_id); self.model.next_id = self.model.next_id.saturating_add(1);
        let root = staging_parent.join(format!("instance-{}", id.get()));
        if root.exists() { return Err(RuntimeError::Staging(format!("staging root already exists: {}", root.display()))); }
        fs::create_dir(&root).map_err(|e| RuntimeError::Staging(e.to_string()))?;
        let mapping = mapping.clone(); let base_root = base_root.to_path_buf(); let root_for_child = root.clone();
        let process = runtime.spawn_process_with_pre_exec(program, args, move || {
            let logical = namespace.materialize_logical_root(&root_for_child, &base_root, &mapping).map_err(|e| std::io::Error::other(e.to_string()))?;
            namespace.enter_logical_root(&logical).map_err(|e| std::io::Error::other(e.to_string()))?; Ok(())
        });
        let process = match process { Ok(value) => value, Err(error) => { let _ = fs::remove_dir_all(&root); return Err(error.into()); } };
        let mut instance = ApplicationInstance::new_with_runtime(id, manifest.metadata().id().clone(), manifest.metadata().version(), session.id(), runtime_spec);
        instance.attach_process(process)?; instance.transition(InstanceState::Running)?;
        self.model.instances.insert(id, instance); self.processes.insert(process, id); self.roots.insert(process, root); Ok(id)
    }
    pub fn poll(&mut self, id: ApplicationInstanceId, runtime: &mut SystemRuntimeService) -> Result<InstanceState, RuntimeError> {
        let process = self.model.instance(id)?.process().ok_or(RuntimeError::NoProcess)?;
        match runtime.poll_process(process)? {
            ProcessState::Running => Ok(InstanceState::Running),
            ProcessState::Exited(status) => { self.processes.remove(&process); self.cleanup_root(process); let instance = self.model.instance_mut(id)?; instance.detach_process(); if status.success() { if instance.state() == InstanceState::Running { instance.transition(InstanceState::Stopped)?; } } else if matches!(instance.state(), InstanceState::Running | InstanceState::Stopping) { instance.transition(InstanceState::Failed)?; } Ok(instance.state()) }
        }
    }
    pub fn terminate(&mut self, id: ApplicationInstanceId, runtime: &mut SystemRuntimeService) -> Result<(), RuntimeError> {
        let process = self.model.instance(id)?.process().ok_or(RuntimeError::NoProcess)?; let _status = runtime.terminate_supervised_process(process)?; self.processes.remove(&process); self.cleanup_root(process);
        let instance = self.model.instance_mut(id)?; instance.detach_process(); if instance.state() == InstanceState::Running { instance.transition(InstanceState::Stopping)?; } if instance.state() == InstanceState::Stopping { instance.transition(InstanceState::Stopped)?; } Ok(())
    }
    pub fn reconcile(&mut self, runtime: &mut SystemRuntimeService) -> Result<Vec<(ApplicationInstanceId, InstanceState)>, RuntimeError> {
        let ids: Vec<_> = self.processes.keys().copied().collect(); let mut changes = Vec::new();
        for process in ids { match runtime.poll_process(process)? { ProcessState::Running => {}, ProcessState::Exited(status) => { if let Some(id) = self.processes.remove(&process) { self.cleanup_root(process); let instance = self.model.instance_mut(id)?; instance.detach_process(); if status.success() { if instance.state() == InstanceState::Running { instance.transition(InstanceState::Stopped)?; } } else if matches!(instance.state(), InstanceState::Running | InstanceState::Stopping) { instance.transition(InstanceState::Failed)?; } changes.push((id, instance.state())); } } } }
        Ok(changes)
    }
    fn cleanup_root(&mut self, process: ProcessId) { if let Some(root) = self.roots.remove(&process) { let parent = root.parent().unwrap_or(Path::new("/tmp")); let name = root.file_name().and_then(|v| v.to_str()).unwrap_or("root"); let support = parent.join(format!(".luna-namespace-{}-{}", process.get(), name)); let _ = fs::remove_dir_all(root); let _ = fs::remove_dir_all(support); } }
}

#[cfg(test)]
mod tests {
    use super::*;
    use luna_bundle::{BundleKind, BundleManifest, BundleMetadata, BundleResource};
    use luna_common::{BundleId, RuntimeKind, UserId, Version};
    use luna_root_mapping::{LogicalPath, MappingRule, PhysicalPath};

    fn setup(runtime: RuntimeKind) -> (BundleManifest, MappingTable, UserSession) {
        let mut manifest = BundleManifest::new(BundleMetadata::new(BundleId::from("example.app"), Version::new(1, 0, 0), BundleKind::Application));
        manifest.add_resource(BundleResource::new("/bin/app", "bin/app"));
        let mut mapping = MappingTable::with_runtime(runtime);
        mapping.insert(MappingRule::new(LogicalPath::new("/bin/app").unwrap(), PhysicalPath::new("/data/system/apps/example/bin/app"))).unwrap();
        let mut session = UserSession::new(SessionId::new(7), UserId::from("alice")); session.transition(SessionState::Authenticating).unwrap(); session.transition(SessionState::Active).unwrap();
        (manifest, mapping, session)
    }

    #[test]
    fn instance_has_typed_runtime() {
        let (manifest, mapping, session) = setup(RuntimeKind::Glibc); let mut runtime = InMemoryApplicationRuntime::new();
        let id = runtime.launch_for_session_with_runtime(&manifest, &mapping, &session, RuntimeSpec::glibc()).unwrap();
        assert_eq!(runtime.instance(id).unwrap().runtime(), RuntimeSpec::glibc());
    }

    #[test]
    fn mapping_and_runtime_must_agree() {
        let (manifest, mapping, session) = setup(RuntimeKind::Glibc); let mut runtime = InMemoryApplicationRuntime::new();
        assert!(matches!(runtime.launch_for_session_with_runtime(&manifest, &mapping, &session, RuntimeSpec::luna()), Err(RuntimeError::RuntimeMismatch { .. })));
    }

    #[test]
    fn runtime_classes_are_available_to_the_execution_contract() {
        let (manifest, mapping, session) = setup(RuntimeKind::Bundle); let mut runtime = InMemoryApplicationRuntime::new();
        let id = runtime.launch_for_session_with_runtime(&manifest, &mapping, &session, RuntimeSpec::bundle()).unwrap();
        assert_eq!(runtime.instance(id).unwrap().runtime().kind(), RuntimeKind::Bundle);
    }
}
