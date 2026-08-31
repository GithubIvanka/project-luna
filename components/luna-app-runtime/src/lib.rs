//! Application execution runtime boundary for Project Luna.
//!
//! Application lifecycle belongs here; low-level Linux namespace mechanics
//! remain in `luna-namespace`, mapping semantics in `luna-root-mapping`, and
//! policy decisions in `luna-security`.

use std::collections::BTreeMap;
use std::path::Path;

use luna_bundle::{validate_manifest, BundleManifest};
use luna_common::{BundleId, Version};
use luna_namespace::{LinuxMountNamespace, LogicalRoot, NamespaceError};
use luna_root_mapping::{LogicalPath, MappingError, MappingTable};
use luna_security::{AuthorizationRequest, Decision, PolicyAuthority};
use luna_user_session::{SessionId, SessionState, UserSession};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ApplicationInstanceId(u128);
impl ApplicationInstanceId { pub const fn new(value: u128) -> Self { Self(value) } pub const fn get(self) -> u128 { self.0 } }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstanceState { Starting, Running, Stopping, Stopped, Failed }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationInstance { id: ApplicationInstanceId, application: BundleId, version: Version, session: SessionId, state: InstanceState }
impl ApplicationInstance {
    pub fn new(id: ApplicationInstanceId, application: BundleId, version: Version, session: SessionId) -> Self { Self { id, application, version, session, state: InstanceState::Starting } }
    pub const fn id(&self) -> ApplicationInstanceId { self.id }
    pub fn application(&self) -> &BundleId { &self.application }
    pub const fn version(&self) -> Version { self.version }
    pub fn session(&self) -> SessionId { self.session }
    pub const fn state(&self) -> InstanceState { self.state }
    pub fn transition(&mut self, next: InstanceState) -> Result<(), RuntimeError> {
        let valid = matches!((self.state, next),
            (InstanceState::Starting, InstanceState::Running) |
            (InstanceState::Starting, InstanceState::Failed) |
            (InstanceState::Running, InstanceState::Stopping) |
            (InstanceState::Running, InstanceState::Failed) |
            (InstanceState::Stopping, InstanceState::Stopped));
        if !valid { return Err(RuntimeError::InvalidTransition); }
        self.state = next;
        Ok(())
    }
}

#[derive(Debug)]
pub enum RuntimeError { InvalidTransition, InvalidBundle(String), Mapping(MappingError), Namespace(NamespaceError), Security(String), SessionNotActive, InstanceNotFound }
impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTransition => f.write_str("invalid application instance transition"),
            Self::InvalidBundle(message) => write!(f, "invalid bundle: {message}"),
            Self::Mapping(error) => write!(f, "mapping error: {error}"),
            Self::Namespace(error) => write!(f, "namespace error: {error}"),
            Self::Security(message) => write!(f, "security authorization failed: {message}"),
            Self::SessionNotActive => f.write_str("session is not active"),
            Self::InstanceNotFound => f.write_str("application instance not found"),
        }
    }
}
impl std::error::Error for RuntimeError {}

pub trait ApplicationRuntime {
    type Error;
    fn launch(&mut self, manifest: &BundleManifest, mapping: &MappingTable) -> Result<ApplicationInstance, Self::Error>;
    fn authorize(&self, policy: &dyn PolicyAuthority, request: &AuthorizationRequest) -> Result<Decision, Self::Error>;
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

    fn validate_resources(manifest: &BundleManifest, mapping: &MappingTable) -> Result<(), RuntimeError> {
        validate_manifest(manifest).map_err(|e| RuntimeError::InvalidBundle(e.to_string()))?;
        for resource in manifest.resources() {
            let logical = LogicalPath::new(resource.logical_path()).map_err(RuntimeError::Mapping)?;
            mapping.resolve(&logical).map_err(|_| RuntimeError::Mapping(MappingError::NotMapped))?;
        }
        mapping.materialize().map_err(RuntimeError::Mapping)?;
        Ok(())
    }

    pub fn launch_for_session(&mut self, manifest: &BundleManifest, mapping: &MappingTable, session: &UserSession) -> Result<ApplicationInstanceId, RuntimeError> {
        if session.state() != SessionState::Active { return Err(RuntimeError::SessionNotActive); }
        Self::validate_resources(manifest, mapping)?;
        let id = ApplicationInstanceId::new(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        let mut instance = ApplicationInstance::new(id, manifest.metadata().id().clone(), manifest.metadata().version(), session.id());
        instance.transition(InstanceState::Running)?;
        self.instances.insert(id, instance);
        Ok(id)
    }

    /// Prepare a namespace only after the caller has obtained explicit security
    /// authorization for every operation represented by `requests`.
    ///
    /// A decision of `Deny`, `Ask`, or an unconstrained interpretation of a
    /// `Constrained` policy is never treated as permission to mount. Typed
    /// constrained policies must be resolved by a future constraint-aware
    /// materializer before they can reach this low-level preparation boundary.
    pub fn prepare_authorized_namespace_for_session(
        &self,
        instance: ApplicationInstanceId,
        namespace: &LinuxMountNamespace,
        root: &Path,
        base_root: &Path,
        mapping: &MappingTable,
        policy: &dyn PolicyAuthority,
        requests: &[AuthorizationRequest],
    ) -> Result<PreparedApplicationNamespace, RuntimeError> {
        self.instances.get(&instance).ok_or(RuntimeError::InstanceNotFound)?;
        Self::validate_mapping_only(mapping)?;
        for request in requests {
            match policy.authorize(request).map_err(|error| RuntimeError::Security(error.to_string()))? {
                Decision::Allow => {}
                Decision::Deny => return Err(RuntimeError::Security(format!("denied: {:?}", request))),
                Decision::Ask => return Err(RuntimeError::Security("authorization requires user confirmation".to_owned())),
                Decision::Constrained { scope } => return Err(RuntimeError::Security(format!("constrained authorization requires constraint enforcement: {scope}"))),
            }
        }
        let logical_root = namespace.materialize_logical_root(root, base_root, mapping).map_err(RuntimeError::Namespace)?;
        Ok(PreparedApplicationNamespace { instance, root: logical_root })
    }

    /// Legacy contract-only preparation. Production callers should use
    /// `prepare_authorized_namespace_for_session` so Security is mandatory.
    pub fn prepare_namespace_for_session(&self, instance: ApplicationInstanceId, namespace: &LinuxMountNamespace, root: &Path, base_root: &Path, mapping: &MappingTable) -> Result<PreparedApplicationNamespace, RuntimeError> {
        self.instances.get(&instance).ok_or(RuntimeError::InstanceNotFound)?;
        Self::validate_mapping_only(mapping)?;
        let logical_root = namespace.materialize_logical_root(root, base_root, mapping).map_err(RuntimeError::Namespace)?;
        Ok(PreparedApplicationNamespace { instance, root: logical_root })
    }

    fn validate_mapping_only(mapping: &MappingTable) -> Result<(), RuntimeError> { mapping.materialize().map_err(RuntimeError::Mapping)?; Ok(()) }
    pub fn instance(&self, id: ApplicationInstanceId) -> Result<&ApplicationInstance, RuntimeError> { self.instances.get(&id).ok_or(RuntimeError::InstanceNotFound) }
    pub fn stop(&mut self, id: ApplicationInstanceId) -> Result<(), RuntimeError> { let instance=self.instances.get_mut(&id).ok_or(RuntimeError::InstanceNotFound)?; instance.transition(InstanceState::Stopping)?; instance.transition(InstanceState::Stopped) }
    pub fn fail(&mut self, id: ApplicationInstanceId) -> Result<(), RuntimeError> { self.instances.get_mut(&id).ok_or(RuntimeError::InstanceNotFound)?.transition(InstanceState::Failed) }
}

impl ApplicationRuntime for InMemoryApplicationRuntime {
    type Error = RuntimeError;
    fn launch(&mut self, manifest: &BundleManifest, mapping: &MappingTable) -> Result<ApplicationInstance, Self::Error> {
        Self::validate_resources(manifest, mapping)?;
        let id=ApplicationInstanceId::new(self.next_id); self.next_id=self.next_id.saturating_add(1);
        let mut instance=ApplicationInstance::new(id, manifest.metadata().id().clone(), manifest.metadata().version(), SessionId::new(0));
        instance.transition(InstanceState::Running)?; self.instances.insert(id, instance.clone()); Ok(instance)
    }
    fn authorize(&self, policy: &dyn PolicyAuthority, request: &AuthorizationRequest) -> Result<Decision, Self::Error> {
        policy.authorize(request).map_err(|e| RuntimeError::Security(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::{ApplicationInstance, ApplicationInstanceId, InMemoryApplicationRuntime, InstanceState};
    use luna_bundle::{BundleKind, BundleManifest, BundleMetadata, BundleResource};
    use luna_common::{BundleId, UserId, Version};
    use luna_root_mapping::{LogicalPath, MappingRule, MappingTable, PhysicalPath};
    use luna_user_session::{SessionId, SessionState, UserSession};

    fn setup() -> (BundleManifest, MappingTable, UserSession) {
        let mut manifest=BundleManifest::new(BundleMetadata::new(BundleId::from("example.app"),Version::new(1,0,0),BundleKind::Application));
        manifest.add_resource(BundleResource::new("/bin/app","bin/app"));
        let mut mapping=MappingTable::new();
        mapping.insert(MappingRule::new(LogicalPath::new("/bin/app").unwrap(),PhysicalPath::new("/data/system/apps/example/bin/app"))).unwrap();
        let mut session=UserSession::new(SessionId::new(7),UserId::from("alice")); session.transition(SessionState::Active).unwrap();
        (manifest,mapping,session)
    }

    #[test]
    fn instance_has_explicit_lifecycle() {
        let mut instance=ApplicationInstance::new(ApplicationInstanceId::new(1),BundleId::from("example.app"),Version::new(1,0,0),SessionId::new(7));
        assert_eq!(instance.state(),InstanceState::Starting); instance.transition(InstanceState::Running).unwrap(); instance.transition(InstanceState::Stopping).unwrap(); instance.transition(InstanceState::Stopped).unwrap();
    }

    #[test]
    fn runtime_requires_active_session_and_valid_mapping() {
        let (manifest,mapping,session)=setup(); let mut runtime=InMemoryApplicationRuntime::new(); let id=runtime.launch_for_session(&manifest,&mapping,&session).unwrap();
        assert_eq!(runtime.instance(id).unwrap().state(),InstanceState::Running); runtime.stop(id).unwrap(); assert_eq!(runtime.instance(id).unwrap().state(),InstanceState::Stopped);
    }
}
