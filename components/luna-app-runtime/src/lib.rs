//! Application execution runtime boundary for Project Luna.
//!
//! This crate owns ApplicationInstance lifecycle and consumes the logical root,
//! security policy, bundle model, and user-session context. Installation and
//! update policy remain outside the runtime.

use luna_bundle::BundleManifest;
use luna_common::{BundleId, Version};
use luna_root_mapping::MappingTable;
use luna_security::{AuthorizationRequest, Decision, PolicyAuthority};
use luna_user_session::SessionId;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ApplicationInstanceId(u128);

impl ApplicationInstanceId {
    pub const fn new(value: u128) -> Self { Self(value) }
    pub const fn get(self) -> u128 { self.0 }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstanceState {
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationInstance {
    id: ApplicationInstanceId,
    application: BundleId,
    version: Version,
    session: SessionId,
    state: InstanceState,
}

impl ApplicationInstance {
    pub fn new(
        id: ApplicationInstanceId,
        application: BundleId,
        version: Version,
        session: SessionId,
    ) -> Self {
        Self { id, application, version, session, state: InstanceState::Starting }
    }

    pub const fn id(&self) -> ApplicationInstanceId { self.id }
    pub fn application(&self) -> &BundleId { &self.application }
    pub const fn version(&self) -> Version { self.version }
    pub fn session(&self) -> SessionId { self.session }
    pub const fn state(&self) -> InstanceState { self.state }

    pub fn transition(&mut self, next: InstanceState) -> Result<(), RuntimeError> {
        let valid = matches!(
            (self.state, next),
            (InstanceState::Starting, InstanceState::Running)
                | (InstanceState::Starting, InstanceState::Failed)
                | (InstanceState::Running, InstanceState::Stopping)
                | (InstanceState::Running, InstanceState::Failed)
                | (InstanceState::Stopping, InstanceState::Stopped)
        );
        if !valid { return Err(RuntimeError::InvalidTransition); }
        self.state = next;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeError { InvalidTransition }

pub trait ApplicationRuntime {
    type Error;

    fn launch(
        &mut self,
        manifest: &BundleManifest,
        mapping: &MappingTable,
    ) -> Result<ApplicationInstance, Self::Error>;

    fn authorize(
        &self,
        policy: &dyn PolicyAuthority,
        request: &AuthorizationRequest,
    ) -> Result<Decision, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::{ApplicationInstance, ApplicationInstanceId, InstanceState};
    use luna_common::{BundleId, Version};
    use luna_user_session::SessionId;

    #[test]
    fn instance_has_explicit_lifecycle() {
        let mut instance = ApplicationInstance::new(
            ApplicationInstanceId::new(1),
            BundleId::from("example.app"),
            Version::new(1, 0, 0),
            SessionId::new(7),
        );
        assert_eq!(instance.state(), InstanceState::Starting);
        instance.transition(InstanceState::Running).expect("start instance");
        instance.transition(InstanceState::Stopping).expect("stop instance");
        instance.transition(InstanceState::Stopped).expect("finish instance");
    }
}
