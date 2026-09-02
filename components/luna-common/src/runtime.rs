//! Shared runtime identity used across mapping, security and execution layers.
//!
//! This is intentionally a small value-level contract. It does not own runtime
//! storage, lifecycle, policy, namespace materialization, or ABI probing.

use std::fmt;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum RuntimeKind {
    /// Native Luna userspace. The system libc is musl.
    #[default]
    Luna,
    /// GNU libc compatibility runtime for applications that require glibc.
    Glibc,
    /// Runtime supplied privately by the Bundle itself.
    Bundle,
}

impl RuntimeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Luna => "luna",
            Self::Glibc => "glibc",
            Self::Bundle => "bundle",
        }
    }

    pub const fn is_native(self) -> bool {
        matches!(self, Self::Luna)
    }
}

impl fmt::Display for RuntimeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseRuntimeKindError;

impl fmt::Display for ParseRuntimeKindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown Luna runtime kind")
    }
}

impl std::error::Error for ParseRuntimeKindError {}

impl FromStr for RuntimeKind {
    type Err = ParseRuntimeKindError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "luna" => Ok(Self::Luna),
            "glibc" => Ok(Self::Glibc),
            "bundle" => Ok(Self::Bundle),
            _ => Err(ParseRuntimeKindError),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct RuntimeSpec {
    kind: RuntimeKind,
}

impl RuntimeSpec {
    pub const fn new(kind: RuntimeKind) -> Self {
        Self { kind }
    }

    pub const fn luna() -> Self {
        Self::new(RuntimeKind::Luna)
    }

    pub const fn glibc() -> Self {
        Self::new(RuntimeKind::Glibc)
    }

    pub const fn bundle() -> Self {
        Self::new(RuntimeKind::Bundle)
    }

    pub const fn kind(self) -> RuntimeKind {
        self.kind
    }
}

impl Default for RuntimeSpec {
    fn default() -> Self {
        Self::luna()
    }
}

impl From<RuntimeKind> for RuntimeSpec {
    fn from(kind: RuntimeKind) -> Self {
        Self::new(kind)
    }
}

#[cfg(test)]
mod tests {
    use super::{RuntimeKind, RuntimeSpec};
    use std::str::FromStr;

    #[test]
    fn runtime_kind_has_stable_wire_names() {
        assert_eq!(RuntimeKind::Luna.as_str(), "luna");
        assert_eq!(RuntimeKind::Glibc.as_str(), "glibc");
        assert_eq!(RuntimeKind::Bundle.as_str(), "bundle");
    }

    #[test]
    fn runtime_kind_round_trips_from_str() {
        for kind in [RuntimeKind::Luna, RuntimeKind::Glibc, RuntimeKind::Bundle] {
            assert_eq!(RuntimeKind::from_str(kind.as_str()).unwrap(), kind);
        }
        assert!(RuntimeKind::from_str("systemd").is_err());
    }

    #[test]
    fn runtime_spec_defaults_to_native_luna() {
        assert_eq!(RuntimeSpec::default().kind(), RuntimeKind::Luna);
        assert!(RuntimeKind::Luna.is_native());
        assert!(!RuntimeKind::Glibc.is_native());
    }
}
