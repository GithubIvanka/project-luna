//! Configuration model boundary for Project Luna.

/// Configuration scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigScope {
    System,
    User,
    Application,
}
