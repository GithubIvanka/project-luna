//! Central security-policy boundary for Project Luna.
//!
//! Low-level filesystem and kernel primitives enforce restrictions; this crate
//! owns Luna's policy model and authorization decisions.

/// Identifier for a security policy subject.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SubjectId(pub String);
