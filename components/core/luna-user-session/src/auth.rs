use std::ffi::CString;
use std::io;

use crate::{SessionError, UserCredentials};

#[derive(Debug)]
pub enum AuthenticationError {
    Io(io::Error),
    UnknownUser(String),
    LockedUser(String),
    InvalidIdentity(String),
    InvalidPassword,
    CryptUnavailable,
    Session(SessionError),
}

impl std::fmt::Display for AuthenticationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "authentication I/O failed: {error}"),
            Self::UnknownUser(user) => write!(f, "authentication user is unknown: {user}"),
            Self::LockedUser(user) => write!(f, "authentication user is locked: {user}"),
            Self::InvalidIdentity(detail) => {
                write!(f, "authentication identity is invalid: {detail}")
            }
            Self::InvalidPassword => write!(f, "authentication password is invalid"),
            Self::CryptUnavailable => write!(f, "password verification primitive is unavailable"),
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
pub struct AuthenticatedIdentity {
    pub uid: u32,
    pub gid: u32,
    pub username: String,
}

pub(crate) fn authenticate_preselected_identity(
    username: &str,
) -> Result<AuthenticatedIdentity, AuthenticationError> {
    let passwd = std::fs::read_to_string("/etc/passwd")?;
    passwd
        .lines()
        .find_map(|line| {
            let mut fields = line.splitn(7, ":");
            let name = fields.next()?;
            let _ = fields.next()?;
            let uid = fields.next()?.parse::<u32>().ok()?;
            let gid = fields.next()?.parse::<u32>().ok()?;
            let _ = fields.next()?;
            let _ = fields.next()?;
            let shell = fields.next().unwrap_or_default();
            (name == username && uid >= 1000 && shell != "/sbin/nologin" && shell != "/bin/false")
                .then(|| AuthenticatedIdentity {
                    uid,
                    gid,
                    username: name.to_owned(),
                })
        })
        .ok_or_else(|| AuthenticationError::UnknownUser(username.to_owned()))
}

pub(crate) fn authenticate(
    username: &str,
    password: &str,
) -> Result<UserCredentials, AuthenticationError> {
    if username.is_empty() || username.contains(':') || username.contains('\n') {
        return Err(AuthenticationError::InvalidIdentity(username.to_owned()));
    }
    if password.len() > 1024 {
        return Err(AuthenticationError::InvalidPassword);
    }

    let passwd = std::fs::read_to_string("/etc/passwd")?;
    let Some((uid, gid, allowed)) = passwd.lines().find_map(|line| {
        let mut fields = line.splitn(7, ':');
        let name = fields.next()?;
        let _password = fields.next()?;
        let uid = fields.next()?.parse::<u32>().ok()?;
        let gid = fields.next()?.parse::<u32>().ok()?;
        let _gecos = fields.next()?;
        let _home = fields.next()?;
        let shell = fields.next().unwrap_or_default();
        (name == username).then(|| {
            (
                uid,
                gid,
                shell != "/sbin/nologin" && shell != "/bin/false" && uid >= 1000,
            )
        })
    }) else {
        return Err(AuthenticationError::UnknownUser(username.to_owned()));
    };
    if !allowed {
        return Err(AuthenticationError::UnknownUser(username.to_owned()));
    }

    let shadow = std::fs::read_to_string("/etc/shadow")?;
    let Some(encoded) = shadow.lines().find_map(|line| {
        let mut fields = line.splitn(9, ':');
        let name = fields.next()?;
        let hash = fields.next()?;
        (name == username).then(|| hash.to_owned())
    }) else {
        return Err(AuthenticationError::InvalidPassword);
    };

    if encoded.is_empty() || encoded.starts_with('!') || encoded.starts_with('*') {
        return Err(AuthenticationError::LockedUser(username.to_owned()));
    }

    verify_password(password, &encoded)?;
    Ok(UserCredentials::new(uid, gid, username))
}

fn verify_password(password: &str, encoded: &str) -> Result<(), AuthenticationError> {
    let password = CString::new(password).map_err(|_| AuthenticationError::InvalidPassword)?;
    let encoded = CString::new(encoded).map_err(|_| AuthenticationError::InvalidPassword)?;

    unsafe extern "C" {
        fn crypt(key: *const libc::c_char, salt: *const libc::c_char) -> *mut libc::c_char;
    }

    let result = unsafe { crypt(password.as_ptr(), encoded.as_ptr()) };
    if result.is_null() {
        return Err(AuthenticationError::CryptUnavailable);
    }

    let result = unsafe { std::ffi::CStr::from_ptr(result) }.to_bytes();
    if constant_time_equal(result, encoded.as_bytes()) {
        Ok(())
    } else {
        Err(AuthenticationError::InvalidPassword)
    }
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    let max = left.len().max(right.len());
    let mut diff = (left.len() ^ right.len()) as u8;
    for index in 0..max {
        let l = left.get(index).copied().unwrap_or(0);
        let r = right.get(index).copied().unwrap_or(0);
        diff |= l ^ r;
    }
    diff == 0
}

pub fn handoff_current_identity() -> io::Result<()> {
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "authenticated user environment is missing",
            )
        })?;
    let uid = unsafe { libc::getuid() };
    let expected = username_for_uid(uid)?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "authenticated uid has no passwd entry",
        )
    })?;
    if expected != username {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "authenticated identity mismatch",
        ));
    }

    const HANDOFF_RUN_DIR: &str = "/run/luna-session";
    const HANDOFF_RESULT: &str = "/run/luna-session/result";
    std::fs::create_dir_all(HANDOFF_RUN_DIR)?;
    let cpath = CString::new(HANDOFF_RUN_DIR)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))?;
    if unsafe { libc::chmod(cpath.as_ptr(), 0o733) } != 0 {
        return Err(io::Error::last_os_error());
    }

    let temp = format!("{HANDOFF_RUN_DIR}/result.{uid}.{}", std::process::id());
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o644)
        .open(&temp)?;
    use std::io::Write;
    writeln!(file, "{uid}")?;
    writeln!(file, "{username}")?;
    file.sync_all()?;
    std::fs::rename(temp, HANDOFF_RESULT)?;
    Ok(())
}

fn username_for_uid(uid: u32) -> io::Result<Option<String>> {
    let passwd = std::fs::read_to_string("/etc/passwd")?;
    Ok(passwd.lines().find_map(|line| {
        let mut fields = line.split(':');
        let name = fields.next()?;
        let _password = fields.next()?;
        let parsed_uid = fields.next()?.parse::<u32>().ok()?;
        (parsed_uid == uid).then(|| name.to_owned())
    }))
}

use std::os::unix::fs::OpenOptionsExt;

#[cfg(test)]
mod tests {
    use super::constant_time_equal;

    #[test]
    fn constant_time_compare_checks_length() {
        assert!(constant_time_equal(b"abc", b"abc"));
        assert!(!constant_time_equal(b"abc", b"abd"));
        assert!(!constant_time_equal(b"abc", b"ab"));
    }
}
