use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const RUN_DIR: &str = "/run/luna-login";
const RESULT: &str = "/run/luna-login/result";
const GREETD_CONFIG: &str = "/run/luna-login/greetd.toml";
const GREETD: &str = "/usr/bin/greetd";
const GREETER_SESSION: &str = "/usr/bin/noctalia-greeter-session";

fn main() {
    if env::args().nth(1).as_deref() == Some("--handoff") {
        if let Err(error) = write_handoff() {
            eprintln!("luna-login-handoff: {error}");
            std::process::exit(1);
        }
        return;
    }

    if let Err(error) = run_login() {
        eprintln!("luna-login: {error}");
        std::process::exit(1);
    }
}

fn run_login() -> io::Result<()> {
    if !Path::new(GREETD).is_file() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "embedded greetd backend is missing"));
    }
    if !Path::new(GREETER_SESSION).is_file() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "Noctalia Greeter session is missing"));
    }
    fs::create_dir_all(RUN_DIR)?;
    set_mode(RUN_DIR, 0o733)?;
    let _ = fs::remove_file(RESULT);
    write_greetd_config()?;

    let mut greetd = Command::new(GREETD)
        .arg("--config")
        .arg(GREETD_CONFIG)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()?;

    let deadline = Instant::now() + Duration::from_secs(600);
    let result = loop {
        if let Some(status) = greetd.try_wait()? {
            return Err(io::Error::other(format!("embedded greetd exited before authentication: {status}")));
        }
        if let Some(value) = read_authenticated_result()? {
            break value;
        }
        if Instant::now() >= deadline {
            let _ = greetd.kill();
            return Err(io::Error::new(io::ErrorKind::TimedOut, "graphical login timed out"));
        }
        thread::sleep(Duration::from_millis(50));
    };

    let _ = greetd.kill();
    let _ = greetd.wait();
    println!("uid={} user={}", result.uid, result.username);
    Ok(())
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
struct AuthenticatedUser {
    uid: u32,
    username: String,
}

fn read_authenticated_result() -> io::Result<Option<AuthenticatedUser>> {
    let path = PathBuf::from(RESULT);
    let metadata = match fs::metadata(&path) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let contents = fs::read_to_string(&path)?;
    let mut parts = contents.lines();
    let uid = parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid authenticated uid"))?;
    let username = parts
        .next()
        .filter(|value| !value.is_empty() && !value.contains('\0') && !value.contains(':'))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid authenticated username"))?
        .to_owned();

    if metadata.uid() != uid {
        return Ok(None);
    }
    let passwd_name = username_for_uid(uid)?;
    if passwd_name.as_deref() != Some(username.as_str()) {
        return Ok(None);
    }
    Ok(Some(AuthenticatedUser { uid, username }))
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

fn write_handoff() -> io::Result<()> {
    let username = env::var("USER")
        .or_else(|_| env::var("LOGNAME"))
        .map_err(|_| io::Error::new(io::ErrorKind::PermissionDenied, "authenticated user environment is missing"))?;
    let uid = unsafe { libc::getuid() };
    let expected = username_for_uid(uid)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::PermissionDenied, "authenticated uid has no passwd entry"))?;
    if expected != username {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "authenticated identity mismatch"));
    }
    let temp = format!("{RUN_DIR}/result.{uid}.{}", std::process::id());
    let mut file = OpenOptions::new().create_new(true).write(true).mode(0o644).open(&temp)?;
    writeln!(file, "{uid}")?;
    writeln!(file, "{username}")?;
    file.sync_all()?;
    fs::rename(temp, RESULT)?;
    Ok(())
}

fn set_mode(path: &str, mode: u32) -> io::Result<()> {
    let status = Command::new("/bin/chmod")
        .arg(format!("{mode:o}"))
        .arg(path)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other("failed to set Luna login runtime permissions"))
    }
}
