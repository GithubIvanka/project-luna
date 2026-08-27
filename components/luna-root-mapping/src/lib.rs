//! Logical filesystem mapping primitives for Project Luna.
//!
//! This crate defines mapping concepts only. Security policy, application
//! lifecycle, and runtime ownership belong to higher layers.

/// A logical path exposed to an application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalPath(String);

impl LogicalPath {
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A mapping from a logical path to a backing filesystem path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathMapping {
    pub logical: LogicalPath,
    pub backing: String,
}
