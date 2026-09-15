//! Small, dependency-light foundational value types shared across Luna crates.
//!
//! This crate intentionally contains only values whose meaning is genuinely
//! shared across subsystem boundaries. Subsystem-specific errors, policy,
//! persistence, runtime state, filesystem operations, and serialization belong
//! to their owning crates.

mod access;
mod id;
mod profile;
mod runtime;
mod user;
mod version;

pub use access::ResourceAccess;
pub use id::{BundleId, ComponentId};
pub use profile::{ProfileError, RuntimeProfile};
pub use runtime::{ParseRuntimeKindError, RuntimeKind, RuntimeSpec};
pub use user::UserId;
pub use version::Version;
