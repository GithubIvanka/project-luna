//! Persistent state contracts for Project Luna.
//!
//! This crate intentionally contains state-domain abstractions only. Boot-state,
//! configuration, security policy, and subsystem-specific lifecycle rules remain
//! owned by their respective crates.

/// Marker for the durable state boundary.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StateStore;
