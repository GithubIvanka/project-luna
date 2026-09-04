//! System-state model and query boundary for Project Luna.
//!
//! The system manager owns the logical model of selected System Images and
//! kernels. It does not execute updates; mutation is performed through
//! `luna-update-manager`.

use luna_common::Version;
use luna_state::{RedbStateStore, Revision, StateKey, StateStore, StateTransaction, StateValue};
use std::fmt;

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

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct KernelRef {
    version: Version,
}
impl KernelRef {
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
    current_kernel: KernelRef,
    factory_kernel: KernelRef,
}
impl SystemState {
    pub const fn new(
        current: SystemImageRef,
        factory: SystemImageRef,
        current_kernel: KernelRef,
        factory_kernel: KernelRef,
    ) -> Self {
        Self {
            current,
            factory,
            current_kernel,
            factory_kernel,
        }
    }
    pub const fn current(&self) -> &SystemImageRef {
        &self.current
    }
    pub const fn factory(&self) -> &SystemImageRef {
        &self.factory
    }
    pub const fn current_kernel(&self) -> &KernelRef {
        &self.current_kernel
    }
    pub const fn factory_kernel(&self) -> &KernelRef {
        &self.factory_kernel
    }
}

pub trait SystemQuery {
    type Error;
    fn state(&self) -> Result<SystemState, Self::Error>;
}

#[derive(Debug)]
pub enum SystemManagerError {
    State(String),
    MissingState(&'static str),
    InvalidVersion(String),
}
impl fmt::Display for SystemManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::State(e) => write!(f, "system state error: {e}"),
            Self::MissingState(k) => write!(f, "missing system state key: {k}"),
            Self::InvalidVersion(v) => write!(f, "invalid version in system state: {v}"),
        }
    }
}
impl std::error::Error for SystemManagerError {}

const CURRENT_IMAGE: &str = "system/current/image";
const CURRENT_KERNEL: &str = "system/current/kernel";
const FACTORY_IMAGE: &str = "system/factory/image";
const FACTORY_KERNEL: &str = "system/factory/kernel";

pub struct PersistentSystemManager<S: StateStore> {
    store: S,
    state: SystemState,
}
impl<S: StateStore> PersistentSystemManager<S> {
    pub fn load(store: S) -> Result<Self, SystemManagerError> {
        let state = read_state(&store)?;
        Ok(Self { store, state })
    }
    pub fn initialize(mut store: S, state: SystemState) -> Result<Self, SystemManagerError> {
        let tx = encode_state(&state);
        store
            .transaction(store.revision(), tx)
            .map_err(state_error)?;
        Ok(Self { store, state })
    }
    pub fn state(&self) -> &SystemState {
        &self.state
    }
    pub fn revision(&self) -> Revision {
        self.store.revision()
    }
    pub fn store(&self) -> &S {
        &self.store
    }
    pub fn set_current(
        &mut self,
        image: Version,
        kernel: Version,
        expected: Revision,
    ) -> Result<Revision, SystemManagerError> {
        let next = SystemState::new(
            SystemImageRef::new(image),
            self.state.factory().clone(),
            KernelRef::new(kernel),
            self.state.factory_kernel().clone(),
        );
        let mut tx = StateTransaction::new();
        tx.set(
            StateKey::new(CURRENT_IMAGE),
            StateValue::new(image.to_string().into_bytes()),
        )
        .set(
            StateKey::new(CURRENT_KERNEL),
            StateValue::new(kernel.to_string().into_bytes()),
        );
        let revision = self.store.transaction(expected, tx).map_err(state_error)?;
        self.state = next;
        Ok(revision)
    }
}
impl PersistentSystemManager<RedbStateStore> {
    pub fn open_redb(data_root: impl AsRef<std::path::Path>) -> Result<Self, SystemManagerError> {
        Self::load(RedbStateStore::open_system_state(data_root).map_err(state_error)?)
    }
    pub fn open_or_initialize_redb(
        data_root: impl AsRef<std::path::Path>,
        default_state: SystemState,
    ) -> Result<Self, SystemManagerError> {
        let store = RedbStateStore::open_system_state(data_root).map_err(state_error)?;
        match read_state(&store) {
            Ok(state) => Ok(Self { store, state }),
            Err(SystemManagerError::MissingState(_)) => Self::initialize(store, default_state),
            Err(error) => Err(error),
        }
    }
}
impl<S: StateStore> SystemQuery for PersistentSystemManager<S> {
    type Error = SystemManagerError;
    fn state(&self) -> Result<SystemState, Self::Error> {
        Ok(self.state.clone())
    }
}

fn encode_state(state: &SystemState) -> StateTransaction {
    let mut tx = StateTransaction::new();
    tx.set(
        StateKey::new(CURRENT_IMAGE),
        StateValue::new(state.current().version().to_string().into_bytes()),
    )
    .set(
        StateKey::new(CURRENT_KERNEL),
        StateValue::new(state.current_kernel().version().to_string().into_bytes()),
    )
    .set(
        StateKey::new(FACTORY_IMAGE),
        StateValue::new(state.factory().version().to_string().into_bytes()),
    )
    .set(
        StateKey::new(FACTORY_KERNEL),
        StateValue::new(state.factory_kernel().version().to_string().into_bytes()),
    );
    tx
}
fn read_state<S: StateStore>(store: &S) -> Result<SystemState, SystemManagerError> {
    Ok(SystemState::new(
        SystemImageRef::new(read_version(store, CURRENT_IMAGE)?),
        SystemImageRef::new(read_version(store, FACTORY_IMAGE)?),
        KernelRef::new(read_version(store, CURRENT_KERNEL)?),
        KernelRef::new(read_version(store, FACTORY_KERNEL)?),
    ))
}
fn read_version<S: StateStore>(
    store: &S,
    key: &'static str,
) -> Result<Version, SystemManagerError> {
    let value = store
        .get(&StateKey::new(key))
        .map_err(state_error)?
        .ok_or(SystemManagerError::MissingState(key))?;
    let text = std::str::from_utf8(value.as_slice())
        .map_err(|_| SystemManagerError::InvalidVersion("non-utf8".into()))?;
    parse_version(text).ok_or_else(|| SystemManagerError::InvalidVersion(text.into()))
}
fn parse_version(value: &str) -> Option<Version> {
    let mut parts = value.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        None
    } else {
        Some(Version::new(major, minor, patch))
    }
}
fn state_error(error: luna_state::StateError) -> SystemManagerError {
    SystemManagerError::State(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use luna_state::MemoryStateStore;
    fn state() -> SystemState {
        SystemState::new(
            SystemImageRef::new(Version::new(3, 0, 0)),
            SystemImageRef::new(Version::new(1, 0, 0)),
            KernelRef::new(Version::new(8, 2, 0)),
            KernelRef::new(Version::new(7, 0, 0)),
        )
    }
    #[test]
    fn system_state_keeps_current_factory_image_and_kernel_separate() {
        let state = state();
        assert_eq!(state.current().version(), Version::new(3, 0, 0));
        assert_eq!(state.factory().version(), Version::new(1, 0, 0));
        assert_eq!(state.current_kernel().version(), Version::new(8, 2, 0));
        assert_eq!(state.factory_kernel().version(), Version::new(7, 0, 0));
    }
    #[test]
    fn durable_system_state_round_trips() {
        let initial = state();
        let mut manager =
            PersistentSystemManager::initialize(MemoryStateStore::new(), initial.clone()).unwrap();
        let revision = manager.revision();
        let next = manager
            .set_current(Version::new(4, 0, 0), Version::new(9, 0, 0), revision)
            .unwrap();
        assert_eq!(next, revision.next());
        assert_eq!(manager.state().current().version(), Version::new(4, 0, 0));
        assert_eq!(
            manager.state().current_kernel().version(),
            Version::new(9, 0, 0)
        );
        assert_eq!(manager.state().factory(), initial.factory());
        assert_eq!(manager.state().factory_kernel(), initial.factory_kernel());
    }
    #[test]
    fn stale_revision_does_not_change_state() {
        let initial = state();
        let mut manager =
            PersistentSystemManager::initialize(MemoryStateStore::new(), initial.clone()).unwrap();
        let stale = Revision::initial();
        let current = manager.revision();
        assert_ne!(current, stale);
        assert!(
            manager
                .set_current(Version::new(5, 0, 0), Version::new(10, 0, 0), stale)
                .is_err()
        );
        assert_eq!(manager.state(), &initial);
    }
    #[test]
    fn missing_redb_state_is_initialized() {
        let root =
            std::env::temp_dir().join(format!("luna-system-manager-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let manager = PersistentSystemManager::open_or_initialize_redb(&root, state()).unwrap();
        assert_eq!(manager.state().current().version(), Version::new(3, 0, 0));
        let _ = std::fs::remove_dir_all(root);
    }
}
