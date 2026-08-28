//! Persistent state contracts for Project Luna.
//!
//! This crate describes durable key/value state boundaries. It does not own
//! configuration, boot state, security policy, or subsystem-specific lifecycle
//! semantics.

use std::fmt;

/// Logical key used by a durable state store.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct StateKey(String);

impl StateKey {
    /// Creates a state key from its textual representation.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the borrowed textual representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque value stored in a durable state store.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateValue(Vec<u8>);

impl StateValue {
    /// Creates a state value from owned bytes.
    pub fn new(value: impl Into<Vec<u8>>) -> Self {
        Self(value.into())
    }

    /// Returns the stored bytes without transferring ownership.
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    /// Consumes the value and returns its bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

/// Monotonic revision of a state store.
///
/// A revision identifies the complete store snapshot observed by a caller.
/// The first committed snapshot has revision `1`; a newly created store starts
/// at revision `0`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Revision(u64);

impl Revision {
    /// Returns the initial revision of an empty store.
    pub const fn initial() -> Self {
        Self(0)
    }

    /// Returns the numeric revision value.
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// A single mutation inside an atomic state transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateMutation {
    /// Insert or replace a value.
    Set(StateKey, StateValue),
    /// Remove a key if it exists.
    Remove(StateKey),
}

/// Collection of state mutations that must be committed atomically.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StateTransaction {
    mutations: Vec<StateMutation>,
}

impl StateTransaction {
    /// Creates an empty transaction.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a set/replace mutation.
    pub fn set(&mut self, key: StateKey, value: StateValue) -> &mut Self {
        self.mutations.push(StateMutation::Set(key, value));
        self
    }

    /// Adds a remove mutation.
    pub fn remove(&mut self, key: StateKey) -> &mut Self {
        self.mutations.push(StateMutation::Remove(key));
        self
    }

    /// Returns whether the transaction contains no mutations.
    pub fn is_empty(&self) -> bool {
        self.mutations.is_empty()
    }

    /// Returns the number of mutations in the transaction.
    pub fn len(&self) -> usize {
        self.mutations.len()
    }

    /// Returns the transaction mutations for a store implementation.
    pub fn mutations(&self) -> &[StateMutation] {
        &self.mutations
    }
}

/// Errors returned by the persistent state boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateError {
    /// The supplied key does not satisfy the store's key contract.
    InvalidKey,
    /// The store cannot be modified in its current mode.
    ReadOnly,
    /// The caller attempted to commit against an obsolete store revision.
    RevisionConflict { expected: Revision, actual: Revision },
    /// Backend-specific storage failure.
    Storage(String),
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKey => f.write_str("invalid state key"),
            Self::ReadOnly => f.write_str("state store is read-only"),
            Self::RevisionConflict { expected, actual } => write!(
                f,
                "state revision conflict: expected {}, actual {}",
                expected.value(),
                actual.value()
            ),
            Self::Storage(message) => write!(f, "state storage error: {message}"),
        }
    }
}

impl std::error::Error for StateError {}

/// Persistence boundary for durable state.
///
/// The abstraction is deliberately synchronous. Implementations own atomic
/// persistence; asynchronous orchestration belongs to higher layers.
pub trait StateStore {
    /// Reads a value from the current snapshot.
    fn get(&self, key: &StateKey) -> Result<Option<StateValue>, StateError>;

    /// Writes one value outside a transaction.
    fn set(&mut self, key: StateKey, value: StateValue) -> Result<(), StateError>;

    /// Removes one value outside a transaction.
    fn remove(&mut self, key: &StateKey) -> Result<(), StateError>;

    /// Returns the current store revision.
    fn revision(&self) -> Revision;

    /// Commits all mutations atomically if `expected_revision` still matches.
    ///
    /// A successful non-empty commit advances the revision exactly once.
    /// Implementations must not partially apply a transaction when a mutation
    /// fails or when the revision check fails.
    fn transaction(
        &mut self,
        expected_revision: Revision,
        transaction: StateTransaction,
    ) -> Result<Revision, StateError>;
}

#[cfg(test)]
mod tests {
    use super::{
        Revision, StateError, StateKey, StateMutation, StateStore, StateTransaction, StateValue,
    };
    use std::collections::BTreeMap;

    struct MemoryStore {
        values: BTreeMap<StateKey, StateValue>,
        revision: Revision,
    }

    impl MemoryStore {
        fn new() -> Self {
            Self {
                values: BTreeMap::new(),
                revision: Revision::initial(),
            }
        }

        fn apply(
            values: &mut BTreeMap<StateKey, StateValue>,
            transaction: &StateTransaction,
        ) {
            for mutation in transaction.mutations() {
                match mutation {
                    StateMutation::Set(key, value) => {
                        values.insert(key.clone(), value.clone());
                    }
                    StateMutation::Remove(key) => {
                        values.remove(key);
                    }
                }
            }
        }
    }

    impl StateStore for MemoryStore {
        fn get(&self, key: &StateKey) -> Result<Option<StateValue>, StateError> {
            Ok(self.values.get(key).cloned())
        }

        fn set(&mut self, key: StateKey, value: StateValue) -> Result<(), StateError> {
            self.values.insert(key, value);
            self.revision = Revision::from(self.revision.value() + 1);
            Ok(())
        }

        fn remove(&mut self, key: &StateKey) -> Result<(), StateError> {
            self.values.remove(key);
            self.revision = Revision::from(self.revision.value() + 1);
            Ok(())
        }

        fn revision(&self) -> Revision {
            self.revision
        }

        fn transaction(
            &mut self,
            expected_revision: Revision,
            transaction: StateTransaction,
        ) -> Result<Revision, StateError> {
            if self.revision != expected_revision {
                return Err(StateError::RevisionConflict {
                    expected: expected_revision,
                    actual: self.revision,
                });
            }

            if transaction.is_empty() {
                return Ok(self.revision);
            }

            let mut next = self.values.clone();
            Self::apply(&mut next, &transaction);
            self.values = next;
            self.revision = Revision::from(self.revision.value() + 1);
            Ok(self.revision)
        }
    }

    impl From<u64> for Revision {
        fn from(value: u64) -> Self {
            Self(value)
        }
    }

    #[test]
    fn state_contract_supports_round_trip() {
        let key = StateKey::new("example/state");
        let mut store = MemoryStore::new();
        store
            .set(key.clone(), StateValue::new(b"value".to_vec()))
            .expect("set state");
        assert_eq!(
            store.get(&key).expect("get state").unwrap().as_slice(),
            b"value"
        );
    }

    #[test]
    fn transaction_commits_atomically_and_advances_once() {
        let key_a = StateKey::new("a");
        let key_b = StateKey::new("b");
        let mut store = MemoryStore::new();
        let base = store.revision();

        let mut transaction = StateTransaction::new();
        transaction
            .set(key_a.clone(), StateValue::new(b"one".to_vec()))
            .set(key_b.clone(), StateValue::new(b"two".to_vec()));

        let next = store
            .transaction(base, transaction)
            .expect("transaction should commit");

        assert_eq!(next.value(), base.value() + 1);
        assert_eq!(store.get(&key_a).unwrap().unwrap().as_slice(), b"one");
        assert_eq!(store.get(&key_b).unwrap().unwrap().as_slice(), b"two");
    }

    #[test]
    fn stale_revision_is_rejected_without_partial_write() {
        let key = StateKey::new("example/state");
        let mut store = MemoryStore::new();
        let stale = store.revision();
        store
            .set(key.clone(), StateValue::new(b"current".to_vec()))
            .expect("initial write");

        let mut transaction = StateTransaction::new();
        transaction.set(key.clone(), StateValue::new(b"stale".to_vec()));

        let error = store.transaction(stale, transaction).unwrap_err();
        assert!(matches!(error, StateError::RevisionConflict { .. }));
        assert_eq!(store.get(&key).unwrap().unwrap().as_slice(), b"current");
    }

    #[test]
    fn empty_transaction_does_not_advance_revision() {
        let mut store = MemoryStore::new();
        let revision = store.revision();
        let next = store
            .transaction(revision, StateTransaction::new())
            .expect("empty transaction");
        assert_eq!(next, revision);
    }
}
