use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use luna_common::{ResourceAccess, RuntimeKind};

#[derive(Debug, Clone, PartialEq, Eq)]
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
            } => {
                return write!(f, "mapping runtime conflict: {existing} vs {requested}");
            }
        };
        f.write_str(message)
    }
}

impl std::error::Error for MappingError {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LogicalPath(PathBuf);

impl LogicalPath {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, MappingError> {
        let path = path.into();
        if !path.is_absolute() {
            return Err(MappingError::NotAbsolute);
        }
        if path.to_string_lossy().ends_with('/') && path != Path::new("/") {
            return Err(MappingError::TrailingSlash);
        }
        if path.components().any(|component| matches!(component, Component::ParentDir)) {
            return Err(MappingError::ParentTraversal);
        }
        if path
            .components()
            .any(|component| matches!(component, Component::Prefix(_)))
        {
            return Err(MappingError::InvalidPrefix);
        }
        if path.to_str().is_none() {
            return Err(MappingError::NonUtf8Path);
        }
        Ok(Self(path))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl fmt::Display for LogicalPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.display().fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhysicalPath(PathBuf);

impl PhysicalPath {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl fmt::Display for PhysicalPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.display().fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingKind {
    File,
    Subtree,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingRule {
    pub logical: LogicalPath,
    pub physical: PhysicalPath,
    pub kind: MappingKind,
    pub access: std::collections::BTreeSet<ResourceAccess>,
}

impl MappingRule {
    pub fn new(
        logical: LogicalPath,
        physical: PhysicalPath,
        kind: MappingKind,
    ) -> Self {
        Self {
            logical,
            physical,
            kind,
            access: std::collections::BTreeSet::new(),
        }
    }

    pub fn with_access(mut self, access: impl IntoIterator<Item = ResourceAccess>) -> Self {
        self.access.extend(access);
        self
    }

    pub fn access(&self) -> &std::collections::BTreeSet<ResourceAccess> {
        &self.access
    }
}

#[derive(Debug, Clone, Default)]
pub struct MappingTable {
    rules: BTreeMap<LogicalPath, MappingRule>,
    runtime: Option<RuntimeKind>,
}

impl MappingTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_runtime(mut self, runtime: RuntimeKind) -> Self {
        self.runtime = Some(runtime);
        self
    }

    pub fn insert(&mut self, rule: MappingRule) -> Result<(), MappingError> {
        if self.rules.contains_key(&rule.logical) {
            return Err(MappingError::DuplicateLogicalPath);
        }
        if let Some(existing_runtime) = self.runtime {
            let requested_runtime = self.runtime_for_rule(&rule);
            if requested_runtime != existing_runtime {
                return Err(MappingError::RuntimeConflict {
                    existing: existing_runtime,
                    requested: requested_runtime,
                });
            }
        }
        self.rules.insert(rule.logical.clone(), rule);
        Ok(())
    }

    fn runtime_for_rule(&self, _rule: &MappingRule) -> RuntimeKind {
        self.runtime.unwrap_or(RuntimeKind::Luna)
    }

    pub fn resolve_rule(&self, logical: &LogicalPath) -> Result<&MappingRule, MappingError> {
        if let Some(rule) = self.rules.get(logical) {
            return Ok(rule);
        }

        self.rules
            .values()
            .filter(|rule| {
                matches!(rule.kind, MappingKind::Subtree)
                    && logical.as_path().starts_with(rule.logical.as_path())
            })
            .max_by_key(|rule| rule.logical.as_path().components().count())
            .ok_or(MappingError::NotMapped)
    }

    /// Resolves a logical resource to its physical backing path.
    pub fn resolve(&self, logical: &LogicalPath) -> Result<PhysicalPath, MappingError> {
        let rule = self.resolve_rule(logical)?;
        match rule.kind {
            MappingKind::File => Ok(rule.physical.clone()),
            MappingKind::Subtree => {
                let relative = logical
                    .as_path()
                    .strip_prefix(rule.logical.as_path())
                    .map_err(|_| MappingError::NotMapped)?;
                Ok(PhysicalPath::new(rule.physical.as_path().join(relative)))
            }
        }
    }

    pub fn rules(&self) -> impl Iterator<Item = &MappingRule> {
        self.rules.values()
    }

    pub fn materialize(&self) -> Result<(), MappingError> {
        for rule in self.rules.values() {
            if rule.logical.as_path() == Path::new("/") && matches!(rule.kind, MappingKind::File) {
                return Err(MappingError::ConflictingPhysicalPath);
            }
        }
        Ok(())
    }

    pub fn accepts_runtime(&self, runtime: RuntimeKind) -> bool {
        self.runtime.is_none_or(|expected| expected == runtime)
    }
}

#[cfg(test)]
mod tests {
    use super::{LogicalPath, MappingKind, MappingRule, MappingTable, PhysicalPath};
    use luna_common::{ResourceAccess, RuntimeKind};

    fn logical(path: &str) -> LogicalPath {
        LogicalPath::new(path).unwrap()
    }

    #[test]
    fn explicit_file_mapping_resolves() {
        let mut table = MappingTable::new().with_runtime(RuntimeKind::Luna);
        table
            .insert(
                MappingRule::new(
                    logical("/bin/app"),
                    PhysicalPath::new("/data/apps/app/bin"),
                    MappingKind::File,
                )
                .with_access([ResourceAccess::Execute]),
            )
            .unwrap();

        assert_eq!(
            table.resolve(&logical("/bin/app")).unwrap().as_path(),
            std::path::Path::new("/data/apps/app/bin")
        );
    }

    #[test]
    fn explicit_subtree_mapping_resolves_descendants() {
        let mut table = MappingTable::new();
        table
            .insert(MappingRule::new(
                logical("/lib/gtk"),
                PhysicalPath::new("/data/system/libs/gtk/4"),
                MappingKind::Subtree,
            ))
            .unwrap();

        assert_eq!(
            table.resolve(&logical("/lib/gtk/libgtk.so")).unwrap().as_path(),
            std::path::Path::new("/data/system/libs/gtk/4/libgtk.so")
        );
    }

    #[test]
    fn narrower_subtree_wins() {
        let mut table = MappingTable::new();
        table
            .insert(MappingRule::new(
                logical("/lib"),
                PhysicalPath::new("/data/lib"),
                MappingKind::Subtree,
            ))
            .unwrap();
        table
            .insert(MappingRule::new(
                logical("/lib/gtk"),
                PhysicalPath::new("/data/gtk"),
                MappingKind::Subtree,
            ))
            .unwrap();

        assert_eq!(
            table.resolve(&logical("/lib/gtk/libgtk.so")).unwrap().as_path(),
            std::path::Path::new("/data/gtk/libgtk.so")
        );
    }
}
