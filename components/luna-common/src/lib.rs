//! Small, dependency-light foundational value types shared by Luna crates.
//!
//! This crate intentionally contains only values whose meaning is genuinely
//! shared across subsystem boundaries. Subsystem-specific errors, policy,
//! persistence, runtime state, filesystem operations, and serialization belong
//! to their owning crates.

mod id;
mod runtime;
mod user;
mod version;

pub use id::{BundleId, ComponentId};
pub use runtime::{ParseRuntimeKindError, RuntimeKind, RuntimeSpec};
pub use user::UserId;
pub use version::Version;
