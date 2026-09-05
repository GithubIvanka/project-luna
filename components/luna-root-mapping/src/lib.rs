//! Logical filesystem mapping primitives for Project Luna.
//!
//! A mapping table belongs to one logical filesystem namespace. Rules map
//! logical resources to backing paths and carry the declarative access mode
//! that the execution layer must enforce. Authorization is deliberately outside
//! this crate and belongs to `luna-security`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Component, Path, PathBuf};

use luna_common::{ResourceAccess, RuntimeKind};

#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct LogicalPath(String);

impl LogicalPath {
    /// Creates a canonical Linux-style logical path.
    ///
    /// This performs lexical normalization only. It never follows symlinks or
    /// consults the host filesystem.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, MappingError> {
        let path = path.as_ref();
        if !path.is_absolute() {
            return Err(MappingError::NotAbsolute);
        }

        let mut normalized = String::from("/");
        let mut first = true;

        for component in path.components() {
            match component {
                Component::RootDir => {}
                Component::CurDir => {}
                Component::ParentDir => return Err(MappingError::ParentTraversal),
                Component::Normal(value) => {
                    let value = value.to_str().ok_or(MappingError::NonUtf8Path)?;
                    if !first {
                        normalized.push('/');
                    }
                    normalized.push_str(value);
                    first = false;
                }
                Component::Prefix(_) => return Err(MappingError::InvalidPrefix),
            }
        }

        if first {
            normalized.clear();
            normalized.push('/');
        }

        Ok(Self(normalized))
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum MappingKind {
    File,
    Subtree,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MappingRule {
    logical: LogicalPath,
    physical: PhysicalPath,
    kind: MappingKind,
    access: BTreeSet<ResourceAccess>,
}

impl MappingRule {
    pub fn new(logical: LogicalPath, physical: PhysicalPath) -> Self {
        Self::file(logical, physical)
    }

    pub fn file(logical: LogicalPath, physical: PhysicalPath) -> Self {
        Self {
            logical,
            physical,
            kind: MappingKind::File,
            access: BTreeSet::new(),
        }
    }

    pub fn subtree(logical: LogicalPath, physical: PhysicalPath) -> Self {
        Self {
            logical,
            physical,
            kind: MappingKind::Subtree,
            access: BTreeSet::new(),
        }
    }

    pub fn with_access<I>(mut self, access: I) -> Self
    where
        I: IntoIterator<Item = ResourceAccess>,
    {
        self.access.extend(access);
        self
    }

    pub fn logical(&self) -> &LogicalPath {
        &self.logical
    }

    pub fn physical(&self) -> &PhysicalPath {
        &self.physical
    }

    pub fn kind(&self) -> MappingKind {
        self.kind
    }

    pub fn access(&self) -> &BTreeSet<ResourceAccess> {
        &self.access
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MappingError {
    NotAbsolute,
    TrailingSlash,
    ParentTraversal,
    NonUtf8Path,
    InvalidPrefix,
    DuplicateLogicalPath,
    NotMapped,
    ConflictingPhysicalPath,
    RuntimeConflict {
        existing: RuntimeKind,
        requested: RuntimeKind,
    },
}

impl fmt::Display for MappingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NotAbsolute => "logical path must be absolute",
            Self::TrailingSlash => "file mapping must not end with a slash",
            Self::ParentTraversal => "parent traversal is not allowed",
            Self::NonUtf8Path => "logical path must be valid UTF-8",
            Self::InvalidPrefix => "logical path must not contain an OS-specific prefix",
            Self::DuplicateLogicalPath => "logical path is already mapped",
            Self::NotMapped => "logical path is not mapped",
            Self::ConflictingPhysicalPath => "logical path has conflicting physical mappings",
            Self::RuntimeConflict {
                existing,
                requested,
            } => return write!(f, "mapping runtime conflict: {existing} vs {requested}"),
        };
        f.write_str(message)
    }
}

impl std::error::Error for MappingError {}

#[derive(Clone, Debug, Default)]
pub struct MappingTable {
    rules: Vec<MappingRule>,
    runtime: Option<RuntimeKind>,
}

impl MappingTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_runtime(runtime: RuntimeKind) -> Self {
        Self {
            rules: Vec::new(),
            runtime: Some(runtime),
        }
    }

    /// Bind this mapping plan to exactly one execution runtime.
    ///
    /// A mapping plan may be runtime-neutral while it is being assembled. Once
    /// accepted for execution, changing its runtime is rejected.
    pub fn bind_runtime(&mut self, runtime: RuntimeKind) -> Result<(), MappingError> {
        match self.runtime {
            None => {
                self.runtime = Some(runtime);
                Ok(())
            }
            Some(existing) if existing == runtime => Ok(()),
            Some(existing) => Err(MappingError::RuntimeConflict {
                existing,
                requested: runtime,
            }),
        }
    }

    pub fn runtime(&self) -> Option<RuntimeKind> {
        self.runtime
    }

    /// Returns whether this plan is compatible with the requested runtime.
    /// Runtime-neutral plans are accepted so mapping construction can remain
    /// reusable until an execution plan binds the runtime.
    pub fn accepts_runtime(&self, runtime: RuntimeKind) -> bool {
        self.runtime.is_none() || self.runtime == Some(runtime)
    }

    pub fn insert(&mut self, rule: MappingRule) -> Result<(), MappingError> {
        if self.rules.iter().any(|item| item.logical == rule.logical) {
            return Err(MappingError::DuplicateLogicalPath);
        }
        self.rules.push(rule);
        Ok(())
    }

    /// Resolves a logical resource to its physical backing path.
    pub fn resolve(&self, logical: &LogicalPath) -> Result<PhysicalPath, MappingError> {
        Ok(self.resolve_rule(logical)?.physical.clone())
    }

    /// Resolves a logical resource to the mapping rule responsible for it.
    pub fn resolve_rule(&self, logical: &LogicalPath) -> Result<&MappingRule, MappingError> {
        if let Some(rule) = self.rules.iter().find(|rule| rule.logical == *logical) {
            return Ok(rule);
        }

        self.rules
            .iter()
            .filter(|rule| rule.kind == MappingKind::Subtree)
            .filter(|rule| is_descendant(logical.as_path(), rule.logical.as_path()))
            .max_by_key(|rule| path_depth(rule.logical.as_path()))
            .ok_or(MappingError::NotMapped)
    }

    /// Produces a deterministic namespace description without making Linux
    /// mount calls. The OS-specific backend lives in `luna-namespace`.
    pub fn materialize(&self) -> Result<MaterializedNamespace, MappingError> {
        let mut entries = BTreeMap::new();
        for rule in &self.rules {
            if entries
                .insert(rule.logical.clone(), rule.physical.clone())
                .is_some()
            {
                return Err(MappingError::ConflictingPhysicalPath);
            }
        }
        Ok(MaterializedNamespace {
            entries,
            runtime: self.runtime,
        })
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializedNamespace {
    entries: BTreeMap<LogicalPath, PhysicalPath>,
    runtime: Option<RuntimeKind>,
}

impl MaterializedNamespace {
    pub fn resolve(&self, logical: &LogicalPath) -> Result<&PhysicalPath, MappingError> {
        self.entries.get(logical).ok_or(MappingError::NotMapped)
    }

    pub fn runtime(&self) -> Option<RuntimeKind> {
        self.runtime
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> impl Iterator<Item = (&LogicalPath, &PhysicalPath)> {
        self.entries.iter()
    }
}

fn is_descendant(candidate: &Path, parent: &Path) -> bool {
    candidate != parent && candidate.starts_with(parent)
}

fn path_depth(path: &Path) -> usize {
    path.components()
        .filter(|component| matches!(component, Component::Normal(_)))
        .count()
}

#[cfg(test)]
mod tests {
    use super::{LogicalPath, MappingError, MappingKind, MappingRule, MappingTable, PhysicalPath};
    use luna_common::{ResourceAccess, RuntimeKind};
    use std::path::Path;

    #[test]
    fn logical_path_is_lexically_normalized() {
        let path = LogicalPath::new("/etc//./app/../config");
        assert_eq!(path, Err(MappingError::ParentTraversal));

        let path = LogicalPath::new("/etc//./app").unwrap();
        assert_eq!(path.as_str(), "/etc/app");
    }

    #[test]
    fn exact_file_mapping_resolves() {
        let logical = LogicalPath::new("/bin/app").unwrap();
        let physical = PhysicalPath::new("/data/system/apps/example/resources/bin/app");
        let mut table = MappingTable::new();
        table
            .insert(MappingRule::new(logical.clone(), physical.clone()))
            .unwrap();
        assert_eq!(table.resolve(&logical).unwrap(), physical);
    }

    #[test]
    fn mapping_access_is_explicit_and_deterministic() {
        let logical = LogicalPath::new("/data/file").unwrap();
        let rule = MappingRule::file(logical, PhysicalPath::new("/data/file")).with_access([
            ResourceAccess::Write,
            ResourceAccess::Read,
            ResourceAccess::Read,
        ]);
        assert_eq!(
            rule.access().iter().copied().collect::<Vec<_>>(),
            vec![ResourceAccess::Read, ResourceAccess::Write]
        );
    }

    #[test]
    fn resolve_rule_returns_most_specific_subtree() {
        let mut table = MappingTable::new();
        table
            .insert(MappingRule::subtree(
                LogicalPath::new("/data").unwrap(),
                PhysicalPath::new("/one"),
            ))
            .unwrap();
        table
            .insert(MappingRule::subtree(
                LogicalPath::new("/data/app").unwrap(),
                PhysicalPath::new("/two"),
            ))
            .unwrap();
        let rule = table
            .resolve_rule(&LogicalPath::new("/data/app/file").unwrap())
            .unwrap();
        assert_eq!(rule.physical().as_path(), Path::new("/two"));
    }

    #[test]
    fn explicit_subtree_mapping_resolves_descendants() {
        let logical = LogicalPath::new("/lib/gtk").unwrap();
        let child = LogicalPath::new("/lib/gtk/libgtk.so").unwrap();
        let nested = LogicalPath::new("/lib/gtk/themes/default.ini").unwrap();
        let outside = LogicalPath::new("/lib/gtk4.so").unwrap();
        let physical = PhysicalPath::new("/data/system/libs/gtk/4");
        let mut table = MappingTable::new();
        table
            .insert(MappingRule::subtree(logical, physical))
            .unwrap();
        assert_eq!(table.iter().next().unwrap().kind(), MappingKind::Subtree);
        assert_eq!(
            table.resolve(&child).unwrap().as_path(),
            Path::new("/data/system/libs/gtk/4/libgtk.so")
        );
        assert_eq!(
            table.resolve(&nested).unwrap().as_path(),
            Path::new("/data/system/libs/gtk/4/themes/default.ini")
        );
        assert_eq!(table.resolve(&outside), Err(MappingError::NotMapped));
    }

    #[test]
    fn runtime_binding_is_immutable_after_first_choice() {
        let mut table = MappingTable::new();
        assert_eq!(table.runtime(), None);
        table.bind_runtime(RuntimeKind::Glibc).unwrap();
        assert!(table.accepts_runtime(RuntimeKind::Glibc));
        assert!(!table.accepts_runtime(RuntimeKind::Luna));
        table.bind_runtime(RuntimeKind::Glibc).unwrap();
        assert_eq!(
            table.bind_runtime(RuntimeKind::Bundle),
            Err(MappingError::RuntimeConflict {
                existing: RuntimeKind::Glibc,
                requested: RuntimeKind::Bundle
            })
        );
        let materialized = table.materialize().unwrap();
        assert_eq!(materialized.runtime(), Some(RuntimeKind::Glibc));
    }

    #[test]
    fn unsafe_logical_paths_are_rejected() {
        assert_eq!(LogicalPath::new("etc/app"), Err(MappingError::NotAbsolute));
        assert_eq!(
            LogicalPath::new("/etc/../secret"),
            Err(MappingError::ParentTraversal)
        );
    }

    #[test]
    fn namespace_materialization_is_deterministic() {
        let mut table = MappingTable::new();
        let a = LogicalPath::new("/a").unwrap();
        let b = LogicalPath::new("/b").unwrap();
        table
            .insert(MappingRule::new(a.clone(), PhysicalPath::new("/data/a")))
            .unwrap();
        table
            .insert(MappingRule::new(b, PhysicalPath::new("/data/b")))
            .unwrap();
        let namespace = table.materialize().unwrap();
        assert_eq!(namespace.len(), 2);
        assert_eq!(
            namespace.resolve(&a).unwrap().as_path(),
            Path::new("/data/a")
        );
    }
}
