//! UserSession domain boundary for Project Luna.
//!
//! Luna models a user's interactive session as one conceptual instance rather
//! than exposing Linux-style TTY sessions to the architecture.

/// Marker for a user-session instance.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct UserSession;
