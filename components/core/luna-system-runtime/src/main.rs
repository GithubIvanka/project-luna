use std::ffi::OsString;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
use std::time::Duration;

use luna_common::{UserId, Version};
use luna_system_manager::{
    InitRef, KernelRef, PersistentSystemManager, RecoveryDataImageRef, RecoveryTarget,
    SystemImageRef, SystemState, SystemTarget,
};
use luna_system_runtime::{ProcessId, SystemRuntime, SystemRuntimeService};
use luna_user_session::SessionState;
use luna_user_session::UserCredentials;

mod boot_success;

const SYSTEM_SOURCE_FD_ENV: &str = "LUNA_SYSTEM_SOURCE_FD";
const IMAGE_SOURCE_FD_ENV: &str = "LUNA_IMAGE_SOURCE_FD";
const SESSION_USER_ENV: &str = "LUNA_SESSION_USER";
const BOOT_MODE_ENV: &str = "LUNA_BOOT_MODE";
const DEVICE_MANAGER_STATUS: &str = "/run/luna/device-manager.status";

fn wait_for_path(path: &str, timeout: Duration) -> Result<(), String> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match fs::metadata(path) {
            Ok(_) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("inspect {path}: {error}")),
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!("timed out waiting for {path}"));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn wait_for_socket(path: &str, timeout: Duration) -> Result<(), String> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match fs::metadata(path) {
            Ok(metadata) if metadata.file_type().is_socket() => return Ok(()),
            Ok(_) => return Err(format!("{path} exists but is not a socket")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("inspect {path}: {error}")),
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!("timed out waiting for {path}"));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn secure_source_handoff_fds() -> Result<(), String> {
    for variable in [SYSTEM_SOURCE_FD_ENV, IMAGE_SOURCE_FD_ENV] {
        let Some(value) = std::env::var_os(variable) else {
            continue;
        };
        let fd: i32 = value
            .to_string_lossy()
            .parse()
            .map_err(|_| format!("invalid {variable} value"))?;
        set_cloexec(fd)?;
        // SAFETY: this runs synchronously at PID 1 startup, before the runtime
        // creates any worker threads or child processes. Removing these
        // internal handoff variables therefore cannot race another env reader.
        unsafe { std::env::remove_var(variable) };
    }
    Ok(())
}

fn set_cloexec(fd: i32) -> Result<(), String> {
    const F_GETFD: i32 = 1;
    const F_SETFD: i32 = 2;
    const FD_CLOEXEC: i32 = 1;

    unsafe extern "C" {
        fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    }

    let flags = unsafe { fcntl(fd, F_GETFD) };
    if flags < 0 {
        return Err(format!("get source fd flags for {fd} failed"));
    }
    if unsafe { fcntl(fd, F_SETFD, flags | FD_CLOEXEC) } < 0 {
        return Err(format!("set close-on-exec for source fd {fd} failed"));
    }
    Ok(())
}

fn target(image: Version, init: Version, kernel: Version) -> SystemTarget {
    SystemTarget::new(
        SystemImageRef::new(image),
        InitRef::new(init),
        KernelRef::new(kernel),
    )
}

fn default_development_system_state() -> SystemState {
    SystemState::new(
        target(
            Version::new(0, 1, 0),
            Version::new(0, 1, 0),
            Version::new(0, 1, 0),
        ),
        target(
            Version::new(0, 1, 0),
            Version::new(0, 1, 0),
            Version::new(0, 1, 0),
        ),
        RecoveryTarget::new(
            target(
                Version::new(0, 1, 0),
                Version::new(0, 1, 0),
                Version::new(0, 1, 0),
            ),
            RecoveryDataImageRef::new(Version::new(0, 1, 0)),
        ),
    )
}

fn graphical_session_command() -> &'static str {
    // Keep the proven Alpine/Luna desktop launcher intact while the canonical
    // disk transition is being completed. The launcher creates the user D-Bus
    // session and starts PipeWire/WirePlumber before Niri.
    "/usr/bin/luna-graphical-session"
}

fn start_openrc() -> Result<(), luna_system_runtime::RuntimeError> {
    for runlevel in ["sysinit", "boot", "default"] {
        eprintln!("luna-system-runtime: OpenRC starting {runlevel} runlevel");
        let status = std::process::Command::new("/usr/bin/openrc")
            .arg(runlevel)
            .status()
            .map_err(|error| {
                luna_system_runtime::RuntimeError::Session(format!(
                    "start OpenRC {runlevel}: {error}"
                ))
            })?;
        if !status.success() {
            return Err(luna_system_runtime::RuntimeError::Session(format!(
                "OpenRC {runlevel} returned {status}"
            )));
        }
        eprintln!("luna-system-runtime: OpenRC {runlevel} runlevel ready");
    }
    Ok(())
}

fn start_system_services(runtime: &mut SystemRuntimeService) -> Vec<(ProcessId, &'static str)> {
    if std::env::var_os("LUNA_SKIP_SYSTEM_SERVICES").is_some() || !nix_like_root() {
        return Vec::new();
    }

    // System services use RAM-backed transient IPC state. These directories
    // are part of the runtime environment, not persistent DATA.
    if let Err(error) = fs::create_dir_all("/run/dbus") {
        eprintln!("luna-system-runtime: cannot prepare /run/dbus: {error}");
    }
    if let Err(error) = fs::create_dir_all("/run/luna") {
        eprintln!("luna-system-runtime: cannot prepare /run/luna: {error}");
    }

    // Bootstrap ordering is intentional: udev must enumerate devices before
    // libinput/udisks, and the system D-Bus bus must exist before services that
    // register objects on it. Luna has no external service manager providing
    // these dependency edges for us.
    let ordered: [(&str, &[&str], Option<&str>); 7] = [
        ("/usr/bin/udevd", &[], None),
        (
            "/usr/bin/luna-device-manager",
            &[],
            Some("/run/luna/device-manager.ready"),
        ),
        (
            "/usr/bin/seatd",
            &["-g", "seat", "-l", "info"],
            Some("/run/seatd.sock"),
        ),
        (
            "/usr/bin/dbus-daemon",
            &["--system", "--nofork"],
            Some("/run/dbus/system_bus_socket"),
        ),
        ("/usr/bin/NetworkManager", &["--no-daemon"], None),
        ("/usr/bin/bluetoothd", &["--nodetach"], None),
        ("/usr/bin/udisksd", &[], None),
    ];

    let mut services = Vec::new();
    for (program, args, ready_socket) in ordered {
        let spawn = if program == "/usr/bin/luna-device-manager"
            || program == "/usr/bin/udisksd"
            || program == "/usr/bin/seatd"
        {
            runtime.spawn_process_inherited_stdio(program, args.iter().copied())
        } else {
            runtime.spawn_process(program, args.iter().copied())
        };
        match spawn {
            Ok(id) => {
                eprintln!(
                    "luna-system-runtime: started system service {program} (pid {})",
                    id.get()
                );
                services.push((id, program));
                if program == "/usr/bin/udevd" {
                    std::thread::sleep(Duration::from_millis(150));
                    match std::process::Command::new("/usr/bin/udevadm")
                        .args(["trigger", "--subsystem-match=input", "--action=add"])
                        .status()
                    {
                        Ok(status) if status.success() => {
                            eprintln!("luna-system-runtime: udev input coldplug triggered");
                        }
                        Ok(status) => {
                            eprintln!(
                                "luna-system-runtime: udev input coldplug failed with {status}"
                            );
                        }
                        Err(error) => {
                            eprintln!("luna-system-runtime: udev input coldplug failed: {error}");
                        }
                    }
                    match std::process::Command::new("/usr/bin/udevadm")
                        .arg("settle")
                        .status()
                    {
                        Ok(status) if status.success() => {}
                        Ok(status) => {
                            eprintln!("luna-system-runtime: udev settle returned {status}");
                        }
                        Err(error) => {
                            eprintln!("luna-system-runtime: udev settle failed: {error}");
                        }
                    }
                    for event in ["event0", "event1", "event2"] {
                        match std::process::Command::new("/usr/bin/udevadm")
                            .args([
                                "info",
                                "--query=property",
                                "--name",
                                &format!("/dev/input/{event}"),
                            ])
                            .output()
                        {
                            Ok(output) if output.status.success() => {
                                for line in String::from_utf8_lossy(&output.stdout).lines() {
                                    if line.starts_with("ID_INPUT_")
                                        || line.starts_with("ID_SEAT=")
                                        || line.starts_with("TAGS=")
                                        || line.starts_with("NAME=")
                                        || line.starts_with("SUBSYSTEM=")
                                    {
                                        eprintln!("luna-system-runtime: udev {event} {line}");
                                    }
                                }
                            }
                            Ok(output) => {
                                eprintln!(
                                    "luna-system-runtime: udev {event} info returned {}",
                                    output.status
                                );
                            }
                            Err(error) => {
                                eprintln!("luna-system-runtime: udev {event} info failed: {error}");
                            }
                        }
                    }
                }
                if let Some(path) = ready_socket {
                    let readiness = if program == "/usr/bin/luna-device-manager" {
                        wait_for_path(path, Duration::from_secs(5))
                    } else {
                        wait_for_socket(path, Duration::from_secs(3))
                    };
                    if readiness.is_err() {
                        eprintln!(
                            "luna-system-runtime: system service {program} did not become ready at {path}"
                        );
                        if program == "/usr/bin/luna-device-manager"
                            || std::env::var_os("LUNA_STRICT_SYSTEM_SERVICES").is_some()
                        {
                            std::process::exit(1);
                        }
                    } else if program == "/usr/bin/luna-device-manager" {
                        match fs::read_to_string(DEVICE_MANAGER_STATUS) {
                            Ok(status) => {
                                for line in status.lines() {
                                    eprintln!("luna-system-runtime: device-manager {line}");
                                }
                            }
                            Err(error) => eprintln!(
                                "luna-system-runtime: cannot read device-manager status: {error}"
                            ),
                        }
                    }
                }
            }
            Err(error) if std::env::var_os("LUNA_STRICT_SYSTEM_SERVICES").is_none() => {
                eprintln!(
                    "luna-system-runtime: optional system service {program} unavailable: {error}"
                );
            }
            Err(error) => {
                eprintln!("luna-system-runtime: required system service {program} failed: {error}");
                std::process::exit(1);
            }
        }
    }
    services
}

fn nix_like_root() -> bool {
    std::env::var_os("LUNA_SYSTEM_RUNTIME_ROOT").is_some_and(|value| value == "1")
        || std::fs::metadata("/etc/luna/services/network.toml").is_ok()
}

fn launch_graphical_user_session(
    runtime: &mut SystemRuntimeService,
    session_id: luna_user_session::SessionId,
    program: &str,
) -> Result<ProcessId, luna_system_runtime::RuntimeError> {
    let session = runtime.session(session_id)?;
    if session.state() != SessionState::Active {
        return Err(luna_system_runtime::RuntimeError::Session(
            "graphical session requires an authenticated active UserSession".into(),
        ));
    }

    let username = session.user().to_string();
    let (uid, gid, home, shell) = user_environment(&username)?;
    let runtime_dir = format!("/run/user/{uid}");
    prepare_user_runtime_dir(&runtime_dir, uid, gid)?;

    let mut command_parts = program.split_whitespace();
    let executable = command_parts.next().ok_or_else(|| {
        luna_system_runtime::RuntimeError::Session("graphical session command is empty".into())
    })?;
    if executable == "/usr/bin/niri" || executable == "/usr/bin/luna-graphical-session" {
        prepare_niri_config(&home, &username)?;
    }
    let args: Vec<OsString> = command_parts.map(OsString::from).collect();
    let env = [
        (OsString::from("HOME"), OsString::from(home)),
        (OsString::from("USER"), OsString::from(username.as_str())),
        (OsString::from("LOGNAME"), OsString::from(username.as_str())),
        (OsString::from("SHELL"), OsString::from(shell)),
        (
            OsString::from("XDG_RUNTIME_DIR"),
            OsString::from(runtime_dir),
        ),
        (
            OsString::from("XDG_SESSION_TYPE"),
            OsString::from("wayland"),
        ),
        (OsString::from("XDG_SEAT"), OsString::from("seat0")),
        (OsString::from("XDG_VTNR"), OsString::from("1")),
        (
            OsString::from("XDG_CURRENT_DESKTOP"),
            OsString::from("niri"),
        ),
        (
            OsString::from("XDG_SESSION_DESKTOP"),
            OsString::from("niri"),
        ),
        (OsString::from("MOZ_ENABLE_WAYLAND"), OsString::from("1")),
        (OsString::from("QT_QPA_PLATFORM"), OsString::from("wayland")),
        (OsString::from("SDL_VIDEODRIVER"), OsString::from("wayland")),
        (
            OsString::from("XDG_DATA_DIRS"),
            OsString::from("/usr/local/share:/usr/share"),
        ),
        (
            OsString::from("FONTCONFIG_FILE"),
            OsString::from("/etc/fonts/fonts.conf"),
        ),
        (
            OsString::from("FONTCONFIG_PATH"),
            OsString::from("/etc/fonts"),
        ),
        (
            OsString::from("XCURSOR_PATH"),
            OsString::from("/usr/share/icons:/usr/share/cursors:/usr/share/pixmaps"),
        ),
        (OsString::from("XCURSOR_THEME"), OsString::from("Adwaita")),
        (OsString::from("XCURSOR_SIZE"), OsString::from("24")),
        (
            OsString::from("NOCTALIA_LOG_LEVEL"),
            OsString::from("debug"),
        ),
        (
            OsString::from("LD_LIBRARY_PATH"),
            OsString::from("/usr/lib/niri-compat:/usr/lib:/usr/local/lib"),
        ),
        (OsString::from("LIBSEAT_BACKEND"), OsString::from("seatd")),
        (
            OsString::from("RUST_LOG"),
            OsString::from(
                "smithay::backend::session::libseat=debug,smithay::backend::udev=debug,smithay::backend::libinput=debug,backend_session=debug,backend_udev=debug,backend_libinput=debug,input_seat=debug,niri::input=debug,niri::backend::tty=debug",
            ),
        ),
    ];
    let credentials = UserCredentials::new(uid, gid, username);
    runtime.spawn_process_with_env_and_pre_exec_inherited_stdio(executable, args, env, move || {
        credentials.apply()
    })
}

fn prepare_niri_config(
    home: &str,
    username: &str,
) -> Result<(), luna_system_runtime::RuntimeError> {
    let config_dir = std::path::Path::new(home).join(".config");
    if config_dir.is_symlink() || config_dir.is_file() {
        fs::remove_file(&config_dir).map_err(|error| {
            luna_system_runtime::RuntimeError::Session(format!(
                "replace user config link {}: {error}",
                config_dir.display()
            ))
        })?;
    } else if config_dir.is_dir() {
        fs::remove_dir_all(&config_dir).map_err(|error| {
            luna_system_runtime::RuntimeError::Session(format!(
                "replace user config directory {}: {error}",
                config_dir.display()
            ))
        })?;
    }

    let data_config = format!("/data/users/{username}/config");
    std::os::unix::fs::symlink(&data_config, &config_dir).map_err(|error| {
        luna_system_runtime::RuntimeError::Session(format!(
            "link user config {} -> {}: {error}",
            config_dir.display(),
            data_config
        ))
    })?;

    let niri_dir = config_dir.join("niri");
    fs::create_dir_all(&niri_dir).map_err(|error| {
        luna_system_runtime::RuntimeError::Session(format!(
            "create Niri config directory {}: {error}",
            niri_dir.display()
        ))
    })?;
    let target = niri_dir.join("config.kdl");
    if !target.exists() {
        fs::copy("/etc/luna/niri-config.kdl", &target).map_err(|error| {
            luna_system_runtime::RuntimeError::Session(format!(
                "initialize DATA Niri config {}: {error}",
                target.display()
            ))
        })?;
    }
    Ok(())
}

fn user_environment(
    username: &str,
) -> Result<(u32, u32, String, String), luna_system_runtime::RuntimeError> {
    let passwd = fs::read_to_string("/etc/passwd").map_err(|error| {
        luna_system_runtime::RuntimeError::Session(format!("read passwd for session user: {error}"))
    })?;
    for line in passwd.lines() {
        let mut fields = line.split(':');
        let name = fields.next().unwrap_or_default();
        let _password = fields.next();
        let uid = fields.next().and_then(|value| value.parse::<u32>().ok());
        let gid = fields.next().and_then(|value| value.parse::<u32>().ok());
        let _gecos = fields.next();
        let home = fields.next();
        let shell = fields.next();
        if name == username {
            let uid = uid.ok_or_else(|| {
                luna_system_runtime::RuntimeError::Session(format!(
                    "invalid uid for session user {username}"
                ))
            })?;
            let gid = gid.ok_or_else(|| {
                luna_system_runtime::RuntimeError::Session(format!(
                    "invalid gid for session user {username}"
                ))
            })?;
            let home = home
                .filter(|value| !value.is_empty())
                .unwrap_or("/")
                .to_owned();
            let shell = shell
                .filter(|value| !value.is_empty())
                .unwrap_or("/usr/bin/sh")
                .to_owned();
            return Ok((uid, gid, home, shell));
        }
    }
    Err(luna_system_runtime::RuntimeError::Session(format!(
        "session user {username} has no passwd entry"
    )))
}

fn prepare_user_runtime_dir(
    path: &str,
    uid: u32,
    gid: u32,
) -> Result<(), luna_system_runtime::RuntimeError> {
    fs::create_dir_all(path).map_err(|error| {
        luna_system_runtime::RuntimeError::Session(format!("create {path}: {error}"))
    })?;
    let cpath = std::ffi::CString::new(path).map_err(|_| {
        luna_system_runtime::RuntimeError::Session(format!("runtime path contains NUL: {path}"))
    })?;
    let status = unsafe { libc::chown(cpath.as_ptr(), uid, gid) };
    if status != 0 {
        return Err(luna_system_runtime::RuntimeError::Session(format!(
            "set ownership on {path}: {}",
            std::io::Error::last_os_error()
        )));
    }
    let status = unsafe { libc::chmod(cpath.as_ptr(), 0o700) };
    if status != 0 {
        return Err(luna_system_runtime::RuntimeError::Session(format!(
            "set permissions on {path}: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}

fn main() {
    if let Err(error) = secure_source_handoff_fds() {
        eprintln!("luna-system-runtime: invalid immutable source handoff: {error}");
        std::process::exit(1);
    }

    let session_command = graphical_session_command();
    let respawn = std::env::var_os("LUNA_NO_RESPAWN").is_none();
    let mut runtime = SystemRuntimeService::new();

    match PersistentSystemManager::open_or_initialize_redb(
        "/data",
        default_development_system_state(),
    ) {
        Ok(manager) => {
            let state = manager.state().clone();
            runtime.attach_system_manager(manager);
            eprintln!(
                "luna-system-runtime: System Image {}, init {}, kernel {}",
                state.current().image().version(),
                state.current().init().version(),
                state.current().kernel().version()
            );
        }
        Err(error) => {
            eprintln!("luna-system-runtime: persistent system state unavailable: {error}");
            std::process::exit(1);
        }
    }

    runtime.start();
    let recovery_mode = std::env::var(BOOT_MODE_ENV).is_ok_and(|value| value == "2");
    if !recovery_mode {
        if let Err(error) = start_openrc() {
            eprintln!("luna-system-runtime: OpenRC bootstrap failed: {error}");
            std::process::exit(1);
        }
    }

    if let Err(error) = boot_success::confirm_boot_success() {
        eprintln!("luna-system-runtime: boot success confirmation failed: {error}");
        std::process::exit(1);
    }
    eprintln!("luna-system-runtime: boot success confirmed");

    let login_user = std::env::var(SESSION_USER_ENV).unwrap_or_else(|_| "luna".to_owned());

    loop {
        let session = if recovery_mode {
            // Recovery has a virtual DATA provider and a dedicated recovery
            // user. There is no interactive login service in Recovery DATA;
            // the environment itself is the authenticated recovery session.
            match runtime.create_session(UserId::from(login_user.as_str())) {
                Ok(id) => id,
                Err(error) => {
                    eprintln!(
                        "luna-system-runtime: failed to create recovery UserSession: {error}"
                    );
                    std::process::exit(1);
                }
            }
        } else {
            let session = match runtime.create_login_session(UserId::from(login_user.as_str())) {
                Ok(id) => id,
                Err(error) => {
                    eprintln!(
                        "luna-system-runtime: failed to create graphical UserSession: {error}"
                    );
                    std::process::exit(1);
                }
            };

            if let Err(error) = runtime.authenticate_session_interactive(session) {
                eprintln!("luna-system-runtime: graphical authentication failed: {error}");
                let _ = runtime.cancel_login(session);
                if !respawn {
                    break;
                }
                continue;
            }
            session
        };

        let _graphical_process = match launch_graphical_user_session(
            &mut runtime,
            session,
            &session_command,
        ) {
            Ok(process) => {
                eprintln!(
                    "luna-system-runtime: graphical UserSession launched {session_command} (pid {})",
                    process.get()
                );
                process
            }
            Err(error) => {
                eprintln!(
                    "luna-system-runtime: failed to launch graphical UserSession {session_command}: {error}"
                );
                std::process::exit(1);
            }
        };

        loop {
            if let Err(error) = runtime.supervise() {
                eprintln!("luna-system-runtime: supervision error: {error}");
                std::process::exit(1);
            }

            if runtime
                .session(session)
                .map(|value| value.state() == SessionState::Ended)
                .unwrap_or(true)
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        if !respawn {
            break;
        }
    }
}
