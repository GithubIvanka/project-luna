//! UserSession domain boundary for Project Luna.
//!
//! A UserSession represents one interactive user context. It owns session
//! identity/lifecycle, while application execution belongs to app-runtime and
//! system-wide supervision belongs to system-runtime.

use std::fmt;

use luna_common::UserId;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct SessionId(u128);

impl SessionId {
    pub const fn new(value: u128) -> Self { Self(value) }
    pub const fn get(self) -> u128 { self.0 }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionState {
    Starting,
    Authenticating,
    Active,
    Restricted,
    Ending,
    Ended,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserSession {
    id: SessionId,
    user: UserId,
    state: SessionState,
}

impl UserSession {
    pub fn new(id: SessionId, user: UserId) -> Self {
        Self { id, user, state: SessionState::Starting }
    }

    pub const fn id(&self) -> SessionId { self.id }
    pub fn user(&self) -> &UserId { &self.user }
    pub const fn state(&self) -> SessionState { self.state }

    pub fn transition(&mut self, next: SessionState) -> Result<(), SessionError> {
        let valid = matches!(
            (self.state, next),
            (SessionState::Starting, SessionState::Authenticating)
                | (SessionState::Authenticating, SessionState::Active)
                | (SessionState::Authenticating, SessionState::Ending)
                | (SessionState::Active, SessionState::Restricted)
                | (SessionState::Active, SessionState::Ending)
                | (SessionState::Restricted, SessionState::Active)
                | (SessionState::Restricted, SessionState::Ending)
                | (SessionState::Ending, SessionState::Ended)
        );
        if !valid {
            return Err(SessionError::InvalidTransition { from: self.state, to: next });
        }
        self.state = next;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionError {
    InvalidTransition { from: SessionState, to: SessionState },
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTransition { from, to } => {
                write!(f, "invalid session transition from {from:?} to {to:?}")
            }
        }
    }
}

impl std::error::Error for SessionError {}

#[cfg(test)]
mod tests {
    use super::{SessionId, SessionState, UserSession};
    use luna_common::UserId;

    #[test]
    fn session_requires_authentication_before_activation() {
        let mut session = UserSession::new(SessionId::new(1), UserId::from("alice"));
        assert_eq!(session.state(), SessionState::Starting);
        session.transition(SessionState::Authenticating).expect("enter authentication");
        session.transition(SessionState::Active).expect("activate session");
        session.transition(SessionState::Restricted).expect("restrict session");
        session.transition(SessionState::Active).expect("restore session");
        session.transition(SessionState::Ending).expect("end session");
        session.transition(SessionState::Ended).expect("finish session");
        assert_eq!(session.state(), SessionState::Ended);
    }

    #[test]
    fn authentication_can_cancel() {
        let mut session = UserSession::new(SessionId::new(2), UserId::from("alice"));
        session.transition(SessionState::Authenticating).expect("enter authentication");
        session.transition(SessionState::Ending).expect("cancel authentication");
        session.transition(SessionState::Ended).expect("finish session");
    }
}
