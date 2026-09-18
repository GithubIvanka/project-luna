//! UserSession domain boundary for Project Luna.
//!
//! A UserSession represents one interactive user context. It owns user/session
//! identity and lifecycle, including the authentication boundary.

use std::fmt;

use luna_common::UserId;

mod login;
pub use login::{LoginError, handoff_current_identity};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct SessionId(u128);

impl SessionId {
    pub const fn new(value: u128) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u128 {
        self.0
    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginState {
    Visible,
    Authenticating,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserSession {
    id: SessionId,
    user: UserId,
    state: SessionState,
    login_state: LoginState,
}

impl UserSession {
    pub fn new(id: SessionId, user: UserId) -> Self {
        Self {
            id,
            user,
            state: SessionState::Starting,
            login_state: LoginState::Visible,
        }
    }

    pub const fn id(&self) -> SessionId {
        self.id
    }
    pub fn user(&self) -> &UserId {
        &self.user
    }
    pub const fn state(&self) -> SessionState {
        self.state
    }
    pub const fn login_state(&self) -> LoginState {
        self.login_state
    }

    pub fn authenticate(&mut self) -> Result<(), LoginError> {
        if self.state != SessionState::Starting {
            return Err(LoginError::Session(SessionError::InvalidTransition {
                from: self.state,
                to: SessionState::Authenticating,
            }));
        }

        self.transition(SessionState::Authenticating)
            .map_err(LoginError::Session)?;

        match login::authenticate() {
            Ok(user) => {
                self.user = user.username.into();
                self.login_state = LoginState::Succeeded;
                self.transition(SessionState::Active)
                    .map_err(LoginError::Session)
            }
            Err(error) => {
                self.login_state = LoginState::Failed;
                Err(error)
            }
        }
    }

    pub fn login_succeeded(&mut self) -> Result<(), SessionError> {
        if self.state != SessionState::Authenticating {
            return Err(SessionError::InvalidTransition {
                from: self.state,
                to: SessionState::Active,
            });
        }
        self.login_state = LoginState::Succeeded;
        self.transition(SessionState::Active)
    }

    pub fn login_failed(&mut self) {
        self.login_state = LoginState::Failed;
    }

    pub fn login_cancelled(&mut self) {
        self.login_state = LoginState::Cancelled;
    }

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
            return Err(SessionError::InvalidTransition {
                from: self.state,
                to: next,
            });
        }
        self.state = next;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionError {
    InvalidTransition {
        from: SessionState,
        to: SessionState,
    },
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
    use super::{LoginState, SessionId, SessionState, UserSession};
    use luna_common::UserId;

    #[test]
    fn graphical_login_precedes_active_session() {
        let mut session = UserSession::new(SessionId::new(1), UserId::from("alice"));
        assert_eq!(session.login_state(), LoginState::Visible);
        session
            .transition(SessionState::Authenticating)
            .expect("enter login");
        session.login_succeeded().expect("authenticate session");
        assert_eq!(session.state(), SessionState::Active);
        assert_eq!(session.login_state(), LoginState::Succeeded);
    }

    #[test]
    fn login_failure_does_not_activate_session() {
        let mut session = UserSession::new(SessionId::new(2), UserId::from("alice"));
        session
            .transition(SessionState::Authenticating)
            .expect("enter login");
        session.login_failed();
        assert_eq!(session.state(), SessionState::Authenticating);
        assert_eq!(session.login_state(), LoginState::Failed);
    }
}
