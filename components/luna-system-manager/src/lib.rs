//! System-state model and query boundary for Project Luna.
//!
//! The system manager owns the logical model of selected System Images,
//! `luna-init` versions, kernels and the Recovery DATA Image. It does not
//! execute updates; mutation is performed through `luna-update-manager`.

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
pub struct InitRef {
    version: Version,
}
impl InitRef {
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

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct RecoveryDataImageRef {
    version: Version,
}
impl RecoveryDataImageRef {
    pub const fn new(version: Version) -> Self {
        Self { version }
    }
    pub const fn version(&self) -> Version {
        self.version
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemTarget {
    image: SystemImageRef,
    init: InitRef,
    kernel: KernelRef,
}
impl SystemTarget {
    pub const fn new(image: SystemImageRef, init: InitRef, kernel: KernelRef) -> Self {
        Self {
            image,
            init,
            kernel,
        }
    }
    pub const fn image(&self) -> &SystemImageRef {
        &self.image
    }
    pub const fn init(&self) -> &InitRef {
        &self.init
    }
    pub const fn kernel(&self) -> &KernelRef {
        &self.kernel
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryTarget {
    target: SystemTarget,
    data: RecoveryDataImageRef,
}
impl RecoveryTarget {
    pub const fn new(target: SystemTarget, data: RecoveryDataImageRef) -> Self {
        Self { target, data }
    }
    pub const fn target(&self) -> &SystemTarget {
        &self.target
    }
    pub const fn data(&self) -> &RecoveryDataImageRef {
        &self.data
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemState {
    current: SystemTarget,
    factory: SystemTarget,
    recovery: RecoveryTarget,
}
impl SystemState {
    pub const fn new(
        current: SystemTarget,
        factory: SystemTarget,
        recovery: RecoveryTarget,
    ) -> Self {
        Self {
            current,
            factory,
            recovery,
        }
    }
    pub const fn current(&self) -> &SystemTarget {
        &self.current
    }
    pub const fn factory(&self) -> &SystemTarget {
        &self.factory
    }
    pub const fn recovery(&self) -> &RecoveryTarget {
        &self.recovery
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
const CURRENT_INIT: &str = "system/current/init";
const CURRENT_KERNEL: &str = "system/current/kernel";
const FACTORY_IMAGE: &str = "system/factory/image";
const FACTORY_INIT: &str = "system/factory/init";
const FACTORY_KERNEL: &str = "system/factory/kernel";
const RECOVERY_IMAGE: &str = "system/recovery/image";
const RECOVERY_INIT: &str = "system/recovery/init";
const RECOVERY_KERNEL: &str = "system/recovery/kernel";
const RECOVERY_DATA: &str = "system/recovery/data";

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
        init: Version,
        kernel: Version,
        expected: Revision,
    ) -> Result<Revision, SystemManagerError> {
        let next = SystemState::new(
            SystemTarget::new(
                SystemImageRef::new(image),
                InitRef::new(init),
                KernelRef::new(kernel),
            ),
            self.state.factory().clone(),
            self.state.recovery().clone(),
        );
        let tx = target_transaction(
            CURRENT_IMAGE,
            CURRENT_INIT,
            CURRENT_KERNEL,
            next.current(),
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
    let mut tx = target_transaction(
        CURRENT_IMAGE,
        CURRENT_INIT,
        CURRENT_KERNEL,
        state.current(),
    );
    tx.set(
        StateKey::new(FACTORY_IMAGE),
        StateValue::new(state.factory().image().version().to_string().into_bytes()),
    )
    .set(
        StateKey::new(FACTORY_INIT),
        StateValue::new(state.factory().init().version().to_string().into_bytes()),
    )
    .set(
        StateKey::new(FACTORY_KERNEL),
        StateValue::new(state.factory().kernel().version().to_string().into_bytes()),
    )
    .set(
        StateKey::new(RECOVERY_IMAGE),
        StateValue::new(state.recovery().target().image().version().to_string().into_bytes()),
    )
    .set(
        StateKey::new(RECOVERY_INIT),
        StateValue::new(state.recovery().target().init().version().to_string().into_bytes()),
    )
    .set(
        StateKey::new(RECOVERY_KERNEL),
        StateValue::new(state.recovery().target().kernel().version().to_string().into_bytes()),
    )
    .set(
        StateKey::new(RECOVERY_DATA),
        StateValue::new(state.recovery().data().version().to_string().into_bytes()),
    );
    tx
}

fn target_transaction(
    image_key: &'static str,
    init_key: &'static str,
    kernel_key: &'static str,
    target: &SystemTarget,
) -> StateTransaction {
    let mut tx = StateTransaction::new();
    tx.set(
        StateKey::new(image_key),
        StateValue::new(target.image().version().to_string().into_bytes()),
    )
    .set(
        StateKey::new(init_key),
        StateValue::new(target.init().version().to_string().into_bytes()),
    )
    .set(
        StateKey::new(kernel_key),
        StateValue::new(target.kernel().version().to_string().into_bytes()),
    );
    tx
}

fn read_state<S: StateStore>(store: &S) -> Result<SystemState, SystemManagerError> {
    Ok(SystemState::new(
        read_target(store, CURRENT_IMAGE, CURRENT_INIT, CURRENT_KERNEL)?,
        read_target(store, FACTORY_IMAGE, FACTORY_INIT, FACTORY_KERNEL)?,
        RecoveryTarget::new(
            read_target(store, RECOVERY_IMAGE, RECOVERY_INIT, RECOVERY_KERNEL)?,
            RecoveryDataImageRef::new(read_version(store, RECOVERY_DATA)?),
        ),
    ))
}

fn read_target<S: StateStore>(
    store: &S,
    image_key: &'static str,
    init_key: &'static str,
    kernel_key: &'static str,
) -> Result<SystemTarget, SystemManagerError> {
    Ok(SystemTarget::new(
        SystemImageRef::new(read_version(store, image_key)?),
        InitRef::new(read_version(store, init_key)?),
        KernelRef::new(read_version(store, kernel_key)?),
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

    fn target(image: (u64, u64, u64), init: (u64, u64, u64), kernel: (u64, u64, u64)) -> SystemTarget {
        SystemTarget::new(
            SystemImageRef::new(Version::new(image.0, image.1, image.2)),
            InitRef::new(Version::new(init.0, init.1, init.2)),
            KernelRef::new(Version::new(kernel.0, kernel.1, kernel.2)),
        )
    }

    fn state() -> SystemState {
        SystemState::new(
            target((3, 0, 0), (3, 1, 0), (8, 2, 0)),
            target((1, 0, 0), (1, 0, 1), (7, 0, 0)),
            RecoveryTarget::new(
                target((2, 0, 0), (2, 1, 0), (8, 1, 0)),
                RecoveryDataImageRef::new(Version::new(1, 4, 0)),
            ),
        )
    }

    #[test]
    fn system_state_keeps_atomic_current_factory_and_recovery_targets() {
        let state = state();
        assert_eq!(state.current().image().version(), Version::new(3, 0, 0));
        assert_eq!(state.current().init().version(), Version::new(3, 1, 0));
        assert_eq!(state.current().kernel().version(), Version::new(8, 2, 0));
        assert_eq!(state.factory().image().version(), Version::new(1, 0, 0));
        assert_eq!(state.factory().init().version(), Version::new(1, 0, 1));
        assert_eq!(state.factory().kernel().version(), Version::new(7, 0, 0));
        assert_eq!(state.recovery().target().image().version(), Version::new(2, 0, 0));
        assert_eq!(state.recovery().target().init().version(), Version::new(2, 1, 0));
        assert_eq!(state.recovery().target().kernel().version(), Version::new(8, 1, 0));
        assert_eq!(state.recovery().data().version(), Version::new(1, 4, 0));
    }

    #[test]
    fn durable_system_state_round_trips() {
        let initial = state();
        let store = MemoryStateStore::new();
        let manager = PersistentSystemManager::initialize(store.clone(), initial.clone()).unwrap();
        let restored = PersistentSystemManager::load(store).unwrap();
        assert_eq!(restored.state(), &initial);
        assert_eq!(manager.state(), &initial);
    }

    #[test]
    fn current_update_preserves_factory_and_recovery() {
        let initial = state();
        let mut manager =
            PersistentSystemManager::initialize(MemoryStateStore::new(), initial.clone()).unwrap();
        let revision = manager.revision();
        let next = manager
            .set_current(
                Version::new(4, 0, 0),
                Version::new(4, 1, 0),
                Version::new(9, 0, 0),
                revision,
            )
            .unwrap();
        assert_eq!(next, revision.next());
        assert_eq!(manager.state().current().image().version(), Version::new(4, 0, 0));
        assert_eq!(manager.state().current().init().version(), Version::new(4, 1, 0));
        assert_eq!(manager.state().current().kernel().version(), Version::new(9, 0, 0));
        assert_eq!(manager.state().factory(), initial.factory());
        assert_eq!(manager.state().recovery(), initial.recovery());
    }

    #[test]
    fn stale_revision_does_not_change_state() {
        let initial = state();
        let mut manager =
            PersistentSystemManager::initialize(MemoryStateStore::new(), initial.clone()).unwrap();
        let stale = Revision::initial();
        let current = manager.revision();
        assert_ne!(current, stale);
        assert!(manager
            .set_current(
                Version::new(5, 0, 0),
                Version::new(5, 1, 0),
                Version::new(10, 0, 0),
                stale,
            )
            .is_err());
        assert_eq!(manager.state(), &initial);
    }

    #[test]
    fn missing_redb_state_is_initialized() {
        let root =
            std::env::temp_dir().join(format!("luna-system-manager-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let manager = PersistentSystemManager::open_or_initialize_redb(&root, state()).unwrap();
        assert_eq!(manager.state().current().image().version(), Version::new(3, 0, 0));
        let _ = std::fs::remove_dir_all(root);
    }
}
