//! Asynchronous event contracts shared by higher-level Luna services.
//!
//! This crate defines event-domain types and contracts only. It is not a Kafka
//! dependency or a message-broker implementation.

/// Marker for the event contract boundary.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EventBus;
