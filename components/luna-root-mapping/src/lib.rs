//! Logical filesystem mapping primitives for Project Luna.
//!
//! A mapping table belongs to one logical filesystem namespace. Rules map
//! individual logical files to backing paths; authorization is deliberately
//! outside this crate and belongs to `luna-security`.

use std::fmt;
use std::path::{Path, PathBuf};

/// A normalized logical path visible inside a Luna process namespace.
#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct LogicalPath(String);

impl LogicalPath {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, MappingError> {
        let path = path.as_ref();
        let value = path.to_str().ok_or(MappingError::NonUtf8Path)?;

        if !value.starts_with('/') {
            return Err(MappingError::NotAbsolute);
        }

        if value.len() > 1 && value.ends_with('/') {
            return Err(MappingError::TrailingSlash);
        }

        if value.split('/').any(|part| part == "..") {
            return Err(MappingError::ParentTraversal);
        }

        Ok(Self(value.to_owned()))
    }

    pub fn root() -> Self {
        Self("/".to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }
}

impl fmt::Display for LogicalPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A backing path on the physical filesystem or another low-level resource.
#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct PhysicalPath(PathBuf);

impl PhysicalPath {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn into_path(self) -> PathBuf {
        self.0
    }
}

/// One exact-file mapping inside a namespace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MappingRule {
    logical: LogicalPath,
    physical: PhysicalPath,
}

impl MappingRule {
    pub fn new(logical: LogicalPath, physical: PhysicalPath) -> Self {
        Self { logical, physical }
    }

    pub fn logical(&self) -> &LogicalPath {
        &self.logical
    }

    pub fn physical(&self) -> &PhysicalPath {
        &self.physical
    }
}

/// Errors produced while constructing or resolving mapping rules.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MappingError {
    NotAbsolute,
    TrailingSlash,
    ParentTraversal,
    NonUtf8Path,
    DuplicateLogicalPath,
    NotMapped,
}

impl fmt::Display for MappingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NotAbsolute => "logical path must be absolute",
            Self::TrailingSlash => "file mapping must not end with a slash",
            Self::ParentTraversal => "parent traversal is not allowed",
            Self::NonUtf8Path => "logical path must be valid UTF-8",
            Self::DuplicateLogicalPath => "logical path is already mapped",
            Self::NotMapped => "logical path is not mapped",
        };
        f.write_str(message)
    }
}

impl std::error::Error for MappingError {}

/// Mapping table for one logical filesystem namespace.
#[derive(Clone, Debug, Default)]
pub struct MappingTable {
    rules: Vec<MappingRule>,
}

impl MappingTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an exact-file mapping. Directory-wide mappings are intentionally
    /// not represented by this API.
    pub fn insert(&mut self, rule: MappingRule) -> Result<(), MappingError> {
        if self.rules.iter().any(|item| item.logical == rule.logical) {
            return Err(MappingError::DuplicateLogicalPath);
        }
        self.rules.push(rule);
        Ok(())
    }

    /// Resolves one logical path to its backing resource.
    pub fn resolve(&self, logical: &LogicalPath) -> Result<&PhysicalPath, MappingError> {
        self.rules
            .iter()
            .find(|rule| rule.logical == *logical)
            .map(|rule| &rule.physical)
            .ok_or(MappingError::NotMapped)
    }

    pub fn remove(&mut self, logical: &LogicalPath) -> Result<MappingRule, MappingError> {
        let index = self
            .rules
            .iter()
            .position(|rule| rule.logical == *logical)
            .ok_or(MappingError::NotMapped)?;
        Ok(self.rules.remove(index))
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &MappingRule> {
        self.rules.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::{LogicalPath, MappingError, MappingRule, MappingTable, PhysicalPath};

    #[test]
    fn exact_file_mapping_resolves() {
        let logical = LogicalPath::new("/bin/app").expect("logical path");
        let physical = PhysicalPath::new("/data/system/apps/example/resources/bin/app");
        let mut table = MappingTable::new();

        table
            .insert(MappingRule::new(logical.clone(), physical.clone()))
            .expect("insert mapping");

        assert_eq!(table.resolve(&logical).expect("resolve mapping"), &physical);
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn directory_mapping_is_not_implicitly_created() {
        let logical = LogicalPath::new("/lib/gtk4.so").expect("logical path");
        let other = LogicalPath::new("/lib/gtk3.so").expect("logical path");
        let physical = PhysicalPath::new("/data/system/libs/gtk/4/gtk4.so");
        let mut table = MappingTable::new();

        table
            .insert(MappingRule::new(logical.clone(), physical))
            .expect("insert mapping");

        assert_eq!(table.resolve(&other), Err(MappingError::NotMapped));
    }

    #[test]
    fn unsafe_logical_paths_are_rejected() {
        assert_eq!(
            LogicalPath::new("etc/app"),
            Err(MappingError::NotAbsolute)
        );
        assert_eq!(
            LogicalPath::new("/etc/../secret"),
            Err(MappingError::ParentTraversal)
        );
        assert_eq!(
            LogicalPath::new("/etc/app/"),
            Err(MappingError::TrailingSlash)
        );
    }
}
