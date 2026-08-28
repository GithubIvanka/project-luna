//! Internal bundle model boundary.
//!
//! Bundle Format v1 and `.lbp` transport details remain deferred to RFC-0002.
//! This crate is therefore only a boundary marker until that specification is
//! accepted.

/// Minimal bundle-domain marker.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Bundle;
