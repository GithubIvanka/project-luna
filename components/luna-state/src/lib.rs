//! Persistent state contracts and the first durable backend for Project Luna.
//!
//! The public abstraction stays synchronous and revision-checked. The durable
//! implementation uses redb, a small embedded ACID key/value database that is
//! crash-safe by default. No Luna-specific WAL is layered on top of it.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use redb::{Database, ReadableTable, TableDefinition};

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Revision(u64);

impl Revision {
    pub const fn initial() -> Self {
        Self(0)
    }

    pub const fn value(self) -> u64 {
        self.0
    }

    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateMutation {
    Set(StateKey, StateValue),
    Remove(StateKey),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StateTransaction {
    mutations: Vec<StateMutation>,
}

impl StateTransaction {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, key: StateKey, value: StateValue) -> &mut Self {
        self.mutations.push(StateMutation::Set(key, value));
        self
    }

    pub fn remove(&mut self, key: StateKey) -> &mut Self {
        self.mutations.push(StateMutation::Remove(key));
        self
    }

    pub fn is_empty(&self) -> bool {
        self.mutations.is_empty()
    }

    pub fn mutations(&self) -> &[StateMutation] {
        &self.mutations
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateError {
    InvalidKey,
    ReadOnly,
    RevisionConflict { expected: Revision, actual: Revision },
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

pub trait StateStore {
    fn get(&self, key: &StateKey) -> Result<Option<StateValue>, StateError>;
    fn set(&mut self, key: StateKey, value: StateValue) -> Result<(), StateError>;
    fn remove(&mut self, key: &StateKey) -> Result<(), StateError>;
    fn revision(&self) -> Revision;
    fn transaction(
        &mut self,
        expected_revision: Revision,
        transaction: StateTransaction,
    ) -> Result<Revision, StateError>;
}

#[derive(Clone, Debug, Default)]
pub struct MemoryStateStore {
    values: BTreeMap<StateKey, StateValue>,
    revision: Revision,
}

impl MemoryStateStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn valid(key: &StateKey) -> Result<(), StateError> {
        if key.as_str().trim().is_empty()
            || key.as_str().contains("..")
            || key.as_str().starts_with('/')
        {
            Err(StateError::InvalidKey)
        } else {
            Ok(())
        }
    }

    fn apply(
        values: &mut BTreeMap<StateKey, StateValue>,
        transaction: &StateTransaction,
    ) -> Result<(), StateError> {
        for mutation in transaction.mutations() {
            match mutation {
                StateMutation::Set(key, value) => {
                    Self::valid(key)?;
                    values.insert(key.clone(), value.clone());
                }
                StateMutation::Remove(key) => {
                    Self::valid(key)?;
                    values.remove(key);
                }
            }
        }
        Ok(())
    }
}

impl StateStore for MemoryStateStore {
    fn get(&self, key: &StateKey) -> Result<Option<StateValue>, StateError> {
        Self::valid(key)?;
        Ok(self.values.get(key).cloned())
    }

    fn set(&mut self, key: StateKey, value: StateValue) -> Result<(), StateError> {
        let revision = self.revision;
        let mut transaction = StateTransaction::new();
        transaction.set(key, value);
        self.transaction(revision, transaction).map(|_| ())
    }

    fn remove(&mut self, key: &StateKey) -> Result<(), StateError> {
        let revision = self.revision;
        let mut transaction = StateTransaction::new();
        transaction.remove(key.clone());
        self.transaction(revision, transaction).map(|_| ())
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
        Self::apply(&mut next, &transaction)?;
        self.values = next;
        self.revision = self.revision.next();
        Ok(self.revision)
    }
}

const VALUES: TableDefinition<&str, &[u8]> = TableDefinition::new("values");
const META: TableDefinition<&str, u64> = TableDefinition::new("meta");
const REVISION_KEY: &str = "revision";

/// Durable state store backed by redb.
///
/// The database lives in the DATA state area. A single committed redb
/// transaction updates all mutations and the global revision together, so a
/// crash cannot expose a partially applied Luna state transaction.
pub struct RedbStateStore {
    db: Database,
    path: PathBuf,
    revision: Revision,
}

impl fmt::Debug for RedbStateStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedbStateStore")
            .field("path", &self.path)
            .field("revision", &self.revision)
            .finish()
    }
}

impl RedbStateStore {
    /// Open or create a durable state database at an explicit path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StateError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(storage_error)?;
        }

        let db = Database::create(&path).map_err(storage_error)?;
        let mut store = Self {
            db,
            path,
            revision: Revision::initial(),
        };
        store.initialize_tables()?;
        store.revision = store.read_revision()?;
        Ok(store)
    }

    /// Open the canonical persistent state location below a DATA partition.
    pub fn open_system_state(data_root: impl AsRef<Path>) -> Result<Self, StateError> {
        Self::open(data_root.as_ref().join("system/state/luna-state.redb"))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Check redb integrity explicitly. Normal operation does not need this;
    /// redb performs crash recovery when reopening the database.
    pub fn check_integrity(&self) -> Result<bool, StateError> {
        self.db.check_integrity().map_err(storage_error)
    }

    fn initialize_tables(&mut self) -> Result<(), StateError> {
        let transaction = self.db.begin_write().map_err(storage_error)?;
        {
            transaction.open_table(VALUES).map_err(storage_error)?;
            transaction.open_table(META).map_err(storage_error)?;
        }
        transaction.commit().map_err(storage_error)
    }

    fn read_revision(&self) -> Result<Revision, StateError> {
        let transaction = self.db.begin_read().map_err(storage_error)?;
        let table = transaction.open_table(META).map_err(storage_error)?;
        let value = table
            .get(REVISION_KEY)
            .map_err(storage_error)?
            .map(|guard| guard.value());
        Ok(Revision(value.unwrap_or(0)))
    }

    fn apply_mutations(
        table: &mut redb::Table<'_, &str, &[u8]>,
        transaction: &StateTransaction,
    ) -> Result<(), StateError> {
        for mutation in transaction.mutations() {
            match mutation {
                StateMutation::Set(key, value) => {
                    validate_key(key)?;
                    table.insert(key.as_str(), value.as_slice()).map_err(storage_error)?;
                }
                StateMutation::Remove(key) => {
                    validate_key(key)?;
                    table.remove(key.as_str()).map_err(storage_error)?;
                }
            }
        }
        Ok(())
    }
}

impl StateStore for RedbStateStore {
    fn get(&self, key: &StateKey) -> Result<Option<StateValue>, StateError> {
        validate_key(key)?;
        let transaction = self.db.begin_read().map_err(storage_error)?;
        let table = transaction.open_table(VALUES).map_err(storage_error)?;
        Ok(table
            .get(key.as_str())
            .map_err(storage_error)?
            .map(|guard| StateValue::new(guard.value().to_vec())))
    }

    fn set(&mut self, key: StateKey, value: StateValue) -> Result<(), StateError> {
        let revision = self.revision;
        let mut transaction = StateTransaction::new();
        transaction.set(key, value);
        self.transaction(revision, transaction).map(|_| ())
    }

    fn remove(&mut self, key: &StateKey) -> Result<(), StateError> {
        let revision = self.revision;
        let mut transaction = StateTransaction::new();
        transaction.remove(key.clone());
        self.transaction(revision, transaction).map(|_| ())
    }

    fn revision(&self) -> Revision {
        self.revision
    }

    fn transaction(
        &mut self,
        expected_revision: Revision,
        transaction: StateTransaction,
    ) -> Result<Revision, StateError> {
        for mutation in transaction.mutations() {
            match mutation {
                StateMutation::Set(key, _) | StateMutation::Remove(key) => validate_key(key)?,
            }
        }
        if transaction.is_empty() {
            return Ok(self.revision);
        }

        let write = self.db.begin_write().map_err(storage_error)?;
        let actual_revision = {
            let meta = write.open_table(META).map_err(storage_error)?;
            meta.get(REVISION_KEY)
                .map_err(storage_error)?
                .map(|guard| guard.value())
                .unwrap_or(0)
        };
        let actual = Revision(actual_revision);
        if actual != expected_revision {
            return Err(StateError::RevisionConflict {
                expected: expected_revision,
                actual,
            });
        }

        {
            let mut values = write.open_table(VALUES).map_err(storage_error)?;
            Self::apply_mutations(&mut values, &transaction)?;
        }
        {
            let mut meta = write.open_table(META).map_err(storage_error)?;
            meta.insert(REVISION_KEY, actual.next().value())
                .map_err(storage_error)?;
        }

        write.commit().map_err(storage_error)?;
        self.revision = actual.next();
        Ok(self.revision)
    }
}

fn validate_key(key: &StateKey) -> Result<(), StateError> {
    if key.as_str().trim().is_empty()
        || key.as_str().contains("..")
        || key.as_str().starts_with('/')
    {
        Err(StateError::InvalidKey)
    } else {
        Ok(())
    }
}

fn storage_error(error: impl fmt::Display) -> StateError {
    StateError::Storage(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let key = StateKey::new("a");
        let mut store = MemoryStateStore::new();
        store.set(key.clone(), StateValue::new(b"v".to_vec())).unwrap();
        assert_eq!(store.get(&key).unwrap().unwrap().as_slice(), b"v");
    }

    #[test]
    fn atomic_revision() {
        let mut store = MemoryStateStore::new();
        let revision = store.revision();
        let mut transaction = StateTransaction::new();
        transaction
            .set(StateKey::new("a"), StateValue::new(b"1".to_vec()))
            .set(StateKey::new("b"), StateValue::new(b"2".to_vec()));
        assert_eq!(store.transaction(revision, transaction).unwrap(), revision.next());
    }

    #[test]
    fn stale_is_rejected() {
        let mut store = MemoryStateStore::new();
        let stale = store.revision();
        store
            .set(StateKey::new("a"), StateValue::new(b"current".to_vec()))
            .unwrap();
        let mut transaction = StateTransaction::new();
        transaction.set(StateKey::new("a"), StateValue::new(b"stale".to_vec()));
        assert!(matches!(
            store.transaction(stale, transaction),
            Err(StateError::RevisionConflict { .. })
        ));
        assert_eq!(
            store.get(&StateKey::new("a")).unwrap().unwrap().as_slice(),
            b"current"
        );
    }

    #[test]
    fn invalid_key_does_not_mutate() {
        let mut store = MemoryStateStore::new();
        assert!(matches!(
            store.set(StateKey::new("../bad"), StateValue::new(vec![1])),
            Err(StateError::InvalidKey)
        ));
        assert_eq!(store.revision(), Revision::initial());
    }
}
