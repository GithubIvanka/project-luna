//! Small, dependency-light foundational value types shared by Luna crates.
//!
//! This crate is intentionally narrow. Subsystem-specific errors, runtime state,
//! security policy, filesystem operations, bundle semantics, and service APIs
//! belong to their owning crates.

mod id;
mod version;

pub use id::{BundleId, ComponentId};
pub use version::Version;
