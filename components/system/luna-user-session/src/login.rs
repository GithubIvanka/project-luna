use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::SessionError;

const RUN_DIR: &str = "/run/luna-session";
const RESULT: &str = "/run/luna-session/result";
const GREETD_CONFIG: &str = "/run/luna-session/greetd.toml";
const GREETD: &str = "/usr/bin/greetd";
const GREETER_SESSION: &str = "/usr/bin/noctalia-greeter-session";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug)]
pub enum LoginError {
    Io(io::Error),
    Authentication(String),
    Timeout,
    Session(SessionError),
}

impl std::fmt::Display for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "login I/O failed: {error}"),
            Self::Authentication(error) => write!(f, "authentication failed: {error}"),
            Self::Timeout => write!(f, "graphical login timed out"),
            Self::Session(error) => write!(f, "session transition failed: {error}"),
        }
    }
}

impl std::error::Error for LoginError {}

impl From<io::Error> for LoginError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedUser {
    pub uid: u32,
    pub username: String,
}

pub(crate) fn authenticate() -> Result<AuthenticatedUser, LoginError> {
    if !Path::new(GREETD).is_file() {
        return Err(LoginError::Authentication(
            "greetd backend is missing".to_owned(),
        ));
    }
    if !Path::new(GREETER_SESSION).is_file() {
        return Err(LoginError::Authentication(
            "Noctalia Greeter session is missing".to_owned(),
        ));
    }

    fs::create_dir_all(RUN_DIR)?;
    set_mode(RUN_DIR, 0o733)?;
    let _ = fs::remove_file(RESULT);
    write_greetd_config()?;

    let mut greetd = LoginRunner::spawn()?;
    let deadline = Instant::now() + DEFAULT_TIMEOUT;
    let result = loop {
        if let Some(status) = greetd.child.try_wait()? {
            return Err(LoginError::Authentication(format!(
                "greetd exited before authentication: {status}"
            )));
        }
        if let Some(value) = read_authenticated_result()? {
            break value;
        }
        if Instant::now() >= deadline {
            greetd.stop();
            return Err(LoginError::Timeout);
        }
        thread::sleep(Duration::from_millis(50));
    };

    greetd.stop();
    Ok(result)
}

struct LoginRunner {
    child: Child,
}

impl LoginRunner {
    fn spawn() -> Result<Self, io::Error> {
        let child = Command::new(GREETD)
            .arg("--config")
            .arg(GREETD_CONFIG)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()?;
        Ok(Self { child })
    }

    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for LoginRunner {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            self.stop();
        }
    }
}

fn write_greetd_config() -> io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(GREETD_CONFIG)?;
    writeln!(file, "[terminal]")?;
    writeln!(file, "vt = 1")?;
    writeln!(file)?;
    writeln!(file, "[default_session]")?;
    writeln!(file, "command = \"{GREETER_SESSION}\"")?;
    writeln!(file, "user = \"greeter\"")?;
    Ok(())
}

#[derive(Debug)]
struct RawAuthenticatedUser {
    uid: u32,
    username: String,
}

fn read_authenticated_result() -> io::Result<Option<AuthenticatedUser>> {
    let path = Path::new(RESULT);
    let metadata = match fs::metadata(path) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };

    let contents = fs::read_to_string(path)?;
    let mut parts = contents.lines();
    let uid = parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid authenticated uid"))?;
    let username = parts
        .next()
        .filter(|value| !value.is_empty() && !value.contains('\0') && !value.contains(':'))
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "invalid authenticated username")
        })?
        .to_owned();

    if metadata.uid() != uid || username_for_uid(uid)?.as_deref() != Some(username.as_str()) {
        return Ok(None);
    }

    let user = RawAuthenticatedUser { uid, username };
    Ok(Some(AuthenticatedUser {
        uid: user.uid,
        username: user.username,
    }))
}

fn username_for_uid(uid: u32) -> io::Result<Option<String>> {
    let passwd = fs::read_to_string("/etc/passwd")?;
    Ok(passwd.lines().find_map(|line| {
        let mut fields = line.split(':');
        let name = fields.next()?;
        let _password = fields.next()?;
        let parsed_uid = fields.next()?.parse::<u32>().ok()?;
        (parsed_uid == uid).then(|| name.to_owned())
    }))
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

    fs::create_dir_all(RUN_DIR)?;
    set_mode(RUN_DIR, 0o733)?;
    let temp = format!("{RUN_DIR}/result.{uid}.{}", std::process::id());
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o644)
        .open(&temp)?;
    writeln!(file, "{uid}")?;
    writeln!(file, "{username}")?;
    file.sync_all()?;
    fs::rename(temp, RESULT)?;
    Ok(())
}

fn set_mode(path: &str, mode: u32) -> io::Result<()> {
    let path = std::ffi::CString::new(path)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))?;
    let status = unsafe { libc::chmod(path.as_ptr(), mode) };
    if status == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
