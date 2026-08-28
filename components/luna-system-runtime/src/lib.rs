//! System-wide runtime supervision boundary for Project Luna.
//!
//! One system runtime supervises multiple UserSession instances. It owns system
//! runtime orchestration, not security policy or application installation.

use luna_event::{EventPublisher, EventType};
use luna_user_session::UserSession;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeState {
    Starting,
    Running,
    Degraded,
    Stopping,
    Stopped,
}

pub trait SystemRuntime {
    type Error;

    fn state(&self) -> RuntimeState;
    fn sessions(&self) -> Vec<&UserSession>;
    fn supervise(&mut self) -> Result<(), Self::Error>;
}

/// Marker for the component that owns the runtime event source.
pub fn session_event_type() -> EventType {
    EventType::new("session.state.changed")
}

/// Documents the dependency without choosing a concrete event transport.
pub fn accepts_event_publisher<P: EventPublisher>(_publisher: &P) {}

#[cfg(test)]
mod tests {
    use super::session_event_type;

    #[test]
    fn runtime_has_stable_session_event_name() {
        assert_eq!(session_event_type().as_str(), "session.state.changed");
    }
}
