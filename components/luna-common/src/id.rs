use std::fmt;

/// Stable identifier of a Luna bundle.
///
/// Validation of the canonical bundle-id syntax belongs to `luna-bundle`;
/// this foundational type intentionally stores an opaque identifier value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BundleId(String);

impl BundleId {
    /// Creates an identifier from its canonical textual representation.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the identifier as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the identifier and returns its text.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl From<String> for BundleId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for BundleId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for BundleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Stable identifier of a Luna component.
///
/// Validation of the canonical component-id syntax belongs to the owning
/// subsystem; this foundational type intentionally stores an opaque value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ComponentId(String);

impl ComponentId {
    /// Creates an identifier from its canonical textual representation.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the identifier as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the identifier and returns its text.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl From<String> for ComponentId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for ComponentId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for ComponentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{BundleId, ComponentId};

    #[test]
    fn bundle_id_round_trips_text() {
        let id = BundleId::from("example.app");
        assert_eq!(id.as_str(), "example.app");
        assert_eq!(id.to_string(), "example.app");
        assert_eq!(id.into_string(), "example.app");
    }

    #[test]
    fn component_id_round_trips_text() {
        let id = ComponentId::from("runtime");
        assert_eq!(id.as_str(), "runtime");
        assert_eq!(id.to_string(), "runtime");
        assert_eq!(id.into_string(), "runtime");
    }
}
