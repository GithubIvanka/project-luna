use std::fmt;

/// Stable identifier of a Luna user.
///
/// The canonical user-id syntax and persistence rules belong to the user/session
/// subsystem. `luna-common` only provides the shared value type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UserId(String);

impl UserId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl From<String> for UserId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for UserId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::UserId;

    #[test]
    fn user_id_round_trips_text() {
        let id = UserId::from("alice");
        assert_eq!(id.as_str(), "alice");
        assert_eq!(id.to_string(), "alice");
        assert_eq!(id.into_string(), "alice");
    }
}
