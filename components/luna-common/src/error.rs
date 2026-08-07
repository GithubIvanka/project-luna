use std::fmt;

#[derive(Debug)]
pub enum LunaError {
    NotFound(String),
    InvalidFormat(String),
    InvalidVersion(String),
    PermissionDenied(String),
    Io(std::io::Error),
    Other(String),
}

impl fmt::Display for LunaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(msg) =>
                write!(f, "Not found: {}", msg),
            Self::InvalidFormat(msg) =>
                write!(f, "Invalid format: {}", msg),
            Self::InvalidVersion(msg) =>
                write!(f, "Invalid version: {}", msg),
            Self::PermissionDenied(msg) =>
                write!(f, "Permission denied: {}", msg),
            Self::Io(err) =>
                write!(f, "IO error: {}", err),
            Self::Other(msg) =>
                write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for LunaError {}
impl From<std::io::Error> for LunaError {
    fn from(
        value: std::io::Error
    ) -> Self {
        Self::Io(value)
    }
}
