use std::fs;
use std::io;

use crate::SessionError;

#[derive(Debug)]
pub enum AuthenticationError {
    Io(io::Error),
    UnknownUser(String),
    InvalidIdentity(String),
    Session(SessionError),
}

impl std::fmt::Display for AuthenticationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "authentication I/O failed: {error}"),
            Self::UnknownUser(user) => write!(f, "authentication user is unknown: {user}"),
            Self::InvalidIdentity(detail) => {
                write!(f, "authentication identity is invalid: {detail}")
            }
            Self::Session(error) => write!(f, "session transition failed: {error}"),
        }
    }
}

impl std::error::Error for AuthenticationError {}

impl From<io::Error> for AuthenticationError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuthenticatedIdentity {
    pub(crate) uid: u32,
    pub(crate) gid: u32,
    pub(crate) username: String,
}

/// Authenticate the identity selected by the boot/session policy without
/// spawning an external greeter or login daemon.
///
/// Interactive credential collection is intentionally not synthesized here:
/// Alpha currently treats the selected Luna account as the trusted boot identity.
/// The UserSession boundary owns this transition and the future native SessionUI
/// can replace only the credential-collection part without changing the boundary.
pub(crate) fn authenticate_preselected_identity(
    username: &str,
) -> Result<AuthenticatedIdentity, AuthenticationError> {
    if username.is_empty() || username.contains(':') || username.contains('\n') {
        return Err(AuthenticationError::InvalidIdentity(username.to_owned()));
    }

    let passwd = fs::read_to_string("/etc/passwd")?;
    for line in passwd.lines() {
        let mut fields = line.split(':');
        let name = fields.next().unwrap_or_default();
        let _password = fields.next();
        let uid = fields.next().and_then(|value| value.parse::<u32>().ok());
        let gid = fields.next().and_then(|value| value.parse::<u32>().ok());
        if name == username {
            let uid = uid.ok_or_else(|| {
                AuthenticationError::InvalidIdentity(format!("invalid uid for {username}"))
            })?;
            let gid = gid.ok_or_else(|| {
                AuthenticationError::InvalidIdentity(format!("invalid gid for {username}"))
            })?;
            return Ok(AuthenticatedIdentity {
                uid,
                gid,
                username: name.to_owned(),
            });
        }
    }

    Err(AuthenticationError::UnknownUser(username.to_owned()))
}
#[cfg(test)]
mod tests {
    use super::{AuthenticationError, authenticate_preselected_identity};

    #[test]
    fn unknown_user_is_rejected() {
        let error = authenticate_preselected_identity("__luna_missing_user__")
            .expect_err("missing account must not authenticate");
        assert!(matches!(error, AuthenticationError::UnknownUser(_)));
    }

    #[test]
    fn invalid_identity_is_rejected() {
        let error = authenticate_preselected_identity("bad:user")
            .expect_err("malformed account name must not authenticate");
        assert!(matches!(error, AuthenticationError::InvalidIdentity(_)));
    }
}
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;

const HANDOFF_RUN_DIR: &str = "/run/luna-session";
const HANDOFF_RESULT: &str = "/run/luna-session/result";

pub fn handoff_current_identity() -> std::io::Result<()> {
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "authenticated user environment is missing",
            )
        })?;
    let uid = unsafe { libc::getuid() };
    let expected = username_for_uid(uid)?.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "authenticated uid has no passwd entry",
        )
    })?;
    if expected != username {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "authenticated identity mismatch",
        ));
    }

    std::fs::create_dir_all(HANDOFF_RUN_DIR)?;
    let cpath = std::ffi::CString::new(HANDOFF_RUN_DIR)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "path contains NUL"))?;
    if unsafe { libc::chmod(cpath.as_ptr(), 0o733) } != 0 {
        return Err(std::io::Error::last_os_error());
    }

    let temp = format!("{HANDOFF_RUN_DIR}/result.{uid}.{}", std::process::id());
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o644)
        .open(&temp)?;
    writeln!(file, "{uid}")?;
    writeln!(file, "{username}")?;
    file.sync_all()?;
    std::fs::rename(temp, HANDOFF_RESULT)?;
    Ok(())
}

fn username_for_uid(uid: u32) -> std::io::Result<Option<String>> {
    let passwd = std::fs::read_to_string("/etc/passwd")?;
    Ok(passwd.lines().find_map(|line| {
        let mut fields = line.split(':');
        let name = fields.next()?;
        let _password = fields.next()?;
        let parsed_uid = fields.next()?.parse::<u32>().ok()?;
        (parsed_uid == uid).then(|| name.to_owned())
    }))
}
