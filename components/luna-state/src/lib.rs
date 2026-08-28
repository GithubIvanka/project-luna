//! Persistent state contracts for Project Luna.
//!
//! This crate describes durable key/value state boundaries. It does not own
//! configuration, boot state, security policy, or subsystem-specific lifecycle
//! semantics.

use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct StateKey(String);

impl StateKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateValue(Vec<u8>);

impl StateValue {
    pub fn new(value: impl Into<Vec<u8>>) -> Self {
        Self(value.into())
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateError {
    InvalidKey,
    ReadOnly,
    Storage(String),
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKey => f.write_str("invalid state key"),
            Self::ReadOnly => f.write_str("state store is read-only"),
            Self::Storage(message) => write!(f, "state storage error: {message}"),
        }
    }
}

impl std::error::Error for StateError {}

/// Persistence boundary for durable state.
pub trait StateStore {
    fn get(&self, key: &StateKey) -> Result<Option<StateValue>, StateError>;
    fn set(&mut self, key: StateKey, value: StateValue) -> Result<(), StateError>;
    fn remove(&mut self, key: &StateKey) -> Result<(), StateError>;
}

#[cfg(test)]
mod tests {
    use super::{StateKey, StateStore, StateValue};

    struct MemoryStore(std::collections::BTreeMap<StateKey, StateValue>);

    impl StateStore for MemoryStore {
        fn get(&self, key: &StateKey) -> Result<Option<StateValue>, super::StateError> {
            Ok(self.0.get(key).cloned())
        }

        fn set(
            &mut self,
            key: StateKey,
            value: StateValue,
        ) -> Result<(), super::StateError> {
            self.0.insert(key, value);
            Ok(())
        }

        fn remove(&mut self, key: &StateKey) -> Result<(), super::StateError> {
            self.0.remove(key);
            Ok(())
        }
    }

    #[test]
    fn state_contract_supports_round_trip() {
        let key = StateKey::new("example/state");
        let mut store = MemoryStore(std::collections::BTreeMap::new());
        store
            .set(key.clone(), StateValue::new(b"value".to_vec()))
            .expect("set state");
        assert_eq!(
            store.get(&key).expect("get state").unwrap().as_slice(),
            b"value"
        );
    }
}
