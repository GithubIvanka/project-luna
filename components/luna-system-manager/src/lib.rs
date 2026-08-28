//! System-state model and query boundary for Project Luna.
//!
//! The system manager owns the model of installed/selected system images. It
//! does not execute updates; mutation is performed through update-manager.

use luna_common::Version;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SystemImageRef {
    version: Version,
}

impl SystemImageRef {
    pub const fn new(version: Version) -> Self {
        Self { version }
    }

    pub const fn version(&self) -> Version {
        self.version
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemState {
    current: SystemImageRef,
    factory: SystemImageRef,
}

impl SystemState {
    pub const fn new(current: SystemImageRef, factory: SystemImageRef) -> Self {
        Self { current, factory }
    }

    pub const fn current(&self) -> &SystemImageRef {
        &self.current
    }

    pub const fn factory(&self) -> &SystemImageRef {
        &self.factory
    }
}

pub trait SystemQuery {
    type Error;

    fn state(&self) -> Result<SystemState, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::{SystemImageRef, SystemState};
    use luna_common::Version;

    #[test]
    fn system_state_keeps_current_and_factory_separate() {
        let current = SystemImageRef::new(Version::new(3, 0, 0));
        let factory = SystemImageRef::new(Version::new(1, 0, 0));
        let state = SystemState::new(current.clone(), factory.clone());

        assert_eq!(state.current(), &current);
        assert_eq!(state.factory(), &factory);
    }
}
