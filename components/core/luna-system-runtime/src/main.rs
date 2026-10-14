use std::ffi::OsString;
use std::fs;
use std::time::Duration;

use luna_common::{UserId, Version};
use luna_system_manager::{
    InitRef, KernelRef, PersistentSystemManager, RecoveryDataImageRef, RecoveryTarget,
    SystemImageRef, SystemState, SystemTarget,
};
use luna_system_runtime::{ProcessId, ProcessState, SystemRuntime, SystemRuntimeService};
use luna_user_session::SessionState;
use luna_user_session::UserCredentials;

mod application_broker;
mod application_catalog;
mod boot_success;

const SYSTEM_SOURCE_FD_ENV: &str = "LUNA_SYSTEM_SOURCE_FD";
const IMAGE_SOURCE_FD_ENV: &str = "LUNA_IMAGE_SOURCE_FD";
const SESSION_USER_ENV: &str = "LUNA_SESSION_USER";
const BOOT_MODE_ENV: &str = "LUNA_BOOT_MODE";
const DEVICE_MANAGER_READY: &str = "/run/luna/device-manager.ready";
const DEVICE_MANAGER_STATUS: &str = "/run/luna/device-manager.status";
const DEVICE_MANAGER_INVENTORY: &str = "/run/luna/device-manager.volumes";
const VOLUME_MANAGER_READY: &str = "/run/luna/volume-manager.ready";
const VOLUME_MANAGER_STATUS: &str = "/run/luna/volume-manager.status";

fn boot_diagnostic(message: std::fmt::Arguments<'_>) {
    use std::io::Write;
    // Запись kmsg сохраняет целостность при одновременном выводе ядра в serial.
    // Даже один write в stderr может быть разорван kernel console посреди UART.
    let line = format!("{message}\n");
    if let Ok(mut log) = fs::OpenOptions::new().write(true).open("/dev/kmsg") {
        if log.write_all(format!("<5>{line}").as_bytes()).is_ok() {
            return;
        }
    }
    // В среде без kmsg остаётся один заранее сформированный write в stderr.
    let _ = std::io::stderr().lock().write_all(line.as_bytes());
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

fn status_value<'a>(status: &'a str, key: &str) -> Option<&'a str> {
    status.lines().find_map(|line| {
        line.strip_prefix(key)
            .and_then(|value| value.strip_prefix('='))
    })
}

fn validate_volume_service_status(
    device_status: &str,
    volume_status: &str,
) -> Result<(usize, usize), String> {
    if status_value(device_status, "native_uevent_monitor") != Some("1") {
        return Err("luna-device-manager did not confirm its native kernel uevent monitor".into());
    }

    let block_volumes = status_value(device_status, "block_volumes")
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| "luna-device-manager status has no valid block_volumes count".to_owned())?;
    if block_volumes == 0 {
        return Err("luna-device-manager discovered no block partitions".into());
    }

    if status_value(volume_status, "auto_attach") != Some("true") {
        return Err("luna-volume-manager auto-attach policy is not enabled".into());
    }

    let volumes = status_value(volume_status, "volume_count")
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| "luna-volume-manager status has no valid volume_count".to_owned())?;
    let available = status_value(volume_status, "available_count")
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| "luna-volume-manager status has no valid available_count".to_owned())?;
    if volumes == 0 || available == 0 {
        return Err(format!(
            "luna-volume-manager did not make any volumes available (detected={volumes}, available={available})"
        ));
    }

    Ok((block_volumes, available))
}

fn wait_for_volume_services(
    runtime: &mut SystemRuntimeService,
    services: &[ProcessId],
    timeout: Duration,
) -> Result<(), String> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        for process in services {
            match runtime.poll_process(*process).map_err(|error| {
                format!("poll required system service {}: {error}", process.get())
            })? {
                ProcessState::Running => {}
                ProcessState::Exited(status) => {
                    return Err(format!(
                        "required system service {} exited before boot readiness: {status}",
                        process.get()
                    ));
                }
            }
        }

        let ready = [
            DEVICE_MANAGER_READY,
            DEVICE_MANAGER_INVENTORY,
            VOLUME_MANAGER_READY,
        ]
        .iter()
        .all(|path| std::path::Path::new(path).is_file());
        if ready {
            let device_status = fs::read_to_string(DEVICE_MANAGER_STATUS)
                .map_err(|error| format!("read Device Manager status: {error}"))?;
            let volume_status = fs::read_to_string(VOLUME_MANAGER_STATUS)
                .map_err(|error| format!("read Volume Manager status: {error}"))?;
            let (detected, available) =
                validate_volume_service_status(&device_status, &volume_status)?;
            boot_diagnostic(format_args!(
                "luna-system-runtime: device and volume managers ready (detected={detected}, available={available}, auto_attach=true)"
            ));
            return Ok(());
        }

        if std::time::Instant::now() >= deadline {
            let missing = [
                DEVICE_MANAGER_READY,
                DEVICE_MANAGER_INVENTORY,
                VOLUME_MANAGER_READY,
            ]
            .iter()
            .filter(|path| !std::path::Path::new(path).is_file())
            .copied()
            .collect::<Vec<_>>();
            return Err(format!(
                "timed out waiting for Device Manager / Volume Manager readiness: {}",
                missing.join(", ")
            ));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Start the minimum system services as Luna-owned supervised processes.
/// The runtime deliberately does not delegate critical boot readiness to an
/// external init/service manager. Volume Manager coldplug waits for the
/// Device Manager inventory and both must pass the gate before boot success.
fn start_system_services(
    runtime: &mut SystemRuntimeService,
) -> Result<Vec<ProcessId>, luna_system_runtime::RuntimeError> {
    fs::create_dir_all("/run/luna").map_err(|error| {
        luna_system_runtime::RuntimeError::Session(format!(
            "prepare native system service state: {error}"
        ))
    })?;

    let device_manager = runtime.spawn_process_inherited_stdio(
        "/usr/bin/luna-device-manager",
        std::iter::empty::<&str>(),
    )?;
    eprintln!(
        "luna-system-runtime: started required system service /usr/bin/luna-device-manager (pid {})",
        device_manager.get()
    );

    let volume_manager = runtime.spawn_process_inherited_stdio(
        "/usr/bin/luna-volume-manager",
        std::iter::empty::<&str>(),
    )?;
    eprintln!(
        "luna-system-runtime: started required system service /usr/bin/luna-volume-manager (pid {})",
        volume_manager.get()
    );

    let services = vec![device_manager, volume_manager];
    wait_for_volume_services(runtime, &services, Duration::from_secs(20))
        .map_err(luna_system_runtime::RuntimeError::Session)?;
    Ok(services)
}

/// Start non-critical desktop compatibility services when the provider exists.
/// This path is not a boot-success prerequisite: core device and volume services
/// are already Luna-owned and validated before this function is reached.
fn start_optional_external_services() {
    if !std::path::Path::new("/usr/bin/openrc").is_file() {
        eprintln!(
            "luna-system-runtime: no external service supervisor; continuing with Luna-native system services"
        );
        return;
    }

    for runlevel in ["sysinit", "boot", "default"] {
        boot_diagnostic(format_args!(
            "luna-system-runtime: starting optional compatibility runlevel {runlevel}"
        ));
        match std::process::Command::new("/usr/bin/openrc")
            .arg(runlevel)
            .status()
        {
            Ok(status) if status.success() => {}
            Ok(status) => {
                eprintln!(
                    "luna-system-runtime: optional compatibility runlevel {runlevel} returned {status}; continuing"
                );
                return;
            }
            Err(error) => {
                eprintln!(
                    "luna-system-runtime: optional compatibility runlevel {runlevel} unavailable: {error}"
                );
                return;
            }
        }
    }
}

/// Start the optional seat broker when no external service manager exists.
/// Device Manager owns the udev compatibility daemon and coldplug lifecycle;
/// starting a second udevd here would conflict with that single owner.
fn start_optional_graphics_compatibility_services(
    runtime: &mut SystemRuntimeService,
) -> Vec<(&'static str, ProcessId)> {
    if std::path::Path::new("/usr/bin/openrc").is_file() {
        return Vec::new();
    }

    let mut services = Vec::new();
    if std::path::Path::new("/usr/bin/seatd").is_file() {
        match runtime.spawn_process_inherited_stdio("/usr/bin/seatd", ["-g", "seat", "-l", "info"])
        {
            Ok(process) => {
                eprintln!(
                    "luna-system-runtime: started optional seat compatibility provider /usr/bin/seatd (pid {})",
                    process.get()
                );
                services.push(("/usr/bin/seatd", process));
            }
            Err(error) => eprintln!(
                "luna-system-runtime: optional seat compatibility provider unavailable; continuing: {error}"
            ),
        }
    }

    services
}

fn supervise_optional_graphics_compatibility_services(
    runtime: &mut SystemRuntimeService,
    services: &mut Vec<(&'static str, ProcessId)>,
) {
    let mut index = 0;
    while index < services.len() {
        let (name, process) = services[index];
        match runtime.poll_process(process) {
            Ok(ProcessState::Running) => index += 1,
            Ok(ProcessState::Exited(status)) => {
                eprintln!(
                    "luna-system-runtime: optional graphics compatibility provider {name} exited ({status}); continuing"
                );
                services.remove(index);
            }
            Err(error) => {
                eprintln!(
                    "luna-system-runtime: optional graphics compatibility provider {name} supervision lost: {error}; continuing"
                );
                services.remove(index);
            }
        }
    }
}

fn supervise_required_system_services(
    runtime: &mut SystemRuntimeService,
    services: &[ProcessId],
) -> Result<(), luna_system_runtime::RuntimeError> {
    for process in services {
        match runtime.poll_process(*process)? {
            ProcessState::Running => {}
            ProcessState::Exited(status) => {
                return Err(luna_system_runtime::RuntimeError::Session(format!(
                    "required system service pid {} exited: {status}",
                    process.get()
                )));
            }
        }
    }
    Ok(())
}

fn launch_graphical_user_session(
    runtime: &mut SystemRuntimeService,
    session_id: luna_user_session::SessionId,
    program: &str,
    cgroup: Option<&luna_system_runtime::process_cgroup::ProcessCgroup>,
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
            OsString::from(format!("{}:/usr/local/share:/usr/share", luna_app_runtime::protocol::DESKTOP_SHARE)),
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
    let membership = cgroup
        .map(|group| group.child_membership_fd())
        .transpose()
        .map_err(|error| luna_system_runtime::RuntimeError::Session(error.to_string()))?;
    runtime.launch_graphical_session_with_env_and_pre_exec(
        session_id,
        executable,
        args,
        env,
        move || {
            if let Some(fd) = &membership {
                luna_system_runtime::process_cgroup::ProcessCgroup::enter(fd)?;
            }
            // Give the desktop and its providers one cleanup boundary on logout.
            // SAFETY: this runs only in the forked child before dropping its UID.
            if unsafe { libc::setpgid(0, 0) } != 0 {
                return Err(std::io::Error::last_os_error());
            }
            credentials.apply()
        },
    )
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

    // Презентация принадлежит графической границе UserSession. Системный runtime
    // сообщает реальные этапы и прекращает отрисовку до аутентификации.
    let boot_presentation = if std::env::var(BOOT_MODE_ENV).is_ok_and(|v| v == "0") {
        luna_user_session::BootPresentation::start().ok()
    } else { None };
    if let Some(screen) = &boot_presentation { screen.set_stage("Подготовка состояния системы"); }

    let state_start = std::time::Instant::now();
    eprintln!("luna-system-runtime: opening persistent system state");
    match PersistentSystemManager::open_or_initialize_redb(
        "/data",
        default_development_system_state(),
    ) {
        Ok(manager) => {
            boot_diagnostic(format_args!(
                "luna-system-runtime: persistent system state ready in {} ms (revision {})",
                state_start.elapsed().as_millis(),
                manager.revision().value(),
            ));
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
    if let Some(screen) = &boot_presentation { screen.set_stage("Подключение устройств и томов"); }
    let recovery_mode = std::env::var(BOOT_MODE_ENV).is_ok_and(|value| value == "2");
    let system_services = if recovery_mode {
        // Recovery uses VirtualData and its dedicated toolset; normal physical
        // device/volume startup is intentionally not a Recovery prerequisite.
        Vec::new()
    } else {
        match start_system_services(&mut runtime) {
            Ok(services) => services,
            Err(error) => {
                eprintln!("luna-system-runtime: native system service startup failed: {error}");
                std::process::exit(1);
            }
        }
    };

    if let Err(error) = boot_success::confirm_boot_success() {
        eprintln!("luna-system-runtime: boot success confirmation failed: {error}");
        std::process::exit(1);
    }
    boot_diagnostic(format_args!("luna-system-runtime: boot success confirmed"));

    let mut optional_graphics_compatibility_services = Vec::new();
    if let Some(screen) = &boot_presentation { screen.set_stage("Подготовка графической сессии"); }
    if !recovery_mode {
        start_optional_external_services();
        optional_graphics_compatibility_services =
            start_optional_graphics_compatibility_services(&mut runtime);
    }

    let login_user = std::env::var(SESSION_USER_ENV).unwrap_or_else(|_| "luna".to_owned());

    drop(boot_presentation);

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

        let session_cgroup = match luna_system_runtime::process_cgroup::ProcessCgroup::new() {
            Ok(group) => Some(group),
            Err(error) => {
                eprintln!("luna-system-runtime: session descendant boundary unavailable: {error}");
                None
            }
        };
        let mut application_broker =
            if let Some(group) = session_cgroup.as_ref().filter(|_| !recovery_mode) {
                let user = runtime
                    .session(session)
                    .expect("owned session")
                    .user()
                    .to_string();
                let (uid, gid, _, _) = user_environment(&user).expect("authenticated account");
                match application_broker::ApplicationBroker::new(
                    runtime.session(session).expect("owned session"),
                    UserCredentials::new(uid, gid, user),
                    group,
                ) {
                    Ok(broker) => Some(broker),
                    Err(error) => {
                        eprintln!("luna-system-runtime: application broker unavailable: {error}");
                        None
                    }
                }
            } else {
                None
            };

        let graphical_process = match launch_graphical_user_session(
            &mut runtime,
            session,
            session_command,
            session_cgroup.as_ref(),
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
            if let Err(error) = supervise_required_system_services(&mut runtime, &system_services) {
                eprintln!("luna-system-runtime: required system service failed: {error}");
                std::process::exit(1);
            }
            supervise_optional_graphics_compatibility_services(
                &mut runtime,
                &mut optional_graphics_compatibility_services,
            );
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
            if let Some(broker) = application_broker.as_mut()
                && let Err(error) = broker.poll(&mut runtime)
            {
                eprintln!("luna-system-runtime: application broker failed: {error}");
                if let Err(error) = broker.end_session(&mut runtime) {
                    eprintln!("luna-system-runtime: application cleanup failed: {error}");
                    std::process::exit(1);
                }
                application_broker = None;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        if let Some(mut broker) = application_broker.take()
            && let Err(error) = broker.end_session(&mut runtime)
        {
            eprintln!("luna-system-runtime: application logout cleanup failed: {error}");
            std::process::exit(1);
        }
        if let Some(group) = session_cgroup {
            if let Err(error) = group.kill_and_wait() {
                eprintln!("luna-system-runtime: session descendant cleanup failed: {error}");
                std::process::exit(1);
            }
        } else {
            // Compatibility desktop without cgroup support has no Bundle broker.
            unsafe {
                libc::kill(-(graphical_process.get() as libc::pid_t), libc::SIGTERM);
            }
            std::thread::sleep(Duration::from_millis(100));
            unsafe {
                libc::kill(-(graphical_process.get() as libc::pid_t), libc::SIGKILL);
            }
        }
        let ended_user = runtime
            .session(session)
            .map(|value| value.user().to_string());
        if let Ok((uid, _, _, _)) = ended_user.and_then(|user| user_environment(&user)) {
            let path = format!("/run/user/{uid}");
            if let Err(error) = fs::remove_dir_all(&path)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                eprintln!("luna-system-runtime: could not clean {path} after logout: {error}");
            }
        }
        eprintln!("luna-system-runtime: graphical UserSession ended; returning to login UI");

        if !respawn {
            break;
        }
    }
}

#[cfg(test)]
mod volume_service_tests {
    use super::validate_volume_service_status;

    #[test]
    fn volume_services_require_kernel_monitor_and_auto_attached_volume() {
        let device =
            "input_devices=4\nblock_volumes=4\nnative_uevent_monitor=1\nudev_compat_backend=1\n";
        let volume = "volume_count=4\navailable_count=1\nauto_attach=true\n";
        assert_eq!(
            validate_volume_service_status(device, volume).unwrap(),
            (4, 1)
        );
    }

    #[test]
    fn boot_gate_rejects_missing_block_discovery() {
        let device = "input_devices=4\nblock_volumes=0\nnative_uevent_monitor=1\n";
        let volume = "volume_count=0\navailable_count=0\nauto_attach=true\n";
        assert!(validate_volume_service_status(device, volume).is_err());
    }

    #[test]
    fn boot_gate_rejects_volume_manager_without_auto_attach() {
        let device = "block_volumes=4\nnative_uevent_monitor=1\n";
        let volume = "volume_count=4\navailable_count=4\nauto_attach=false\n";
        assert!(validate_volume_service_status(device, volume).is_err());
    }

    #[test]
    fn boot_gate_rejects_no_available_volumes() {
        let device = "block_volumes=4\nnative_uevent_monitor=1\n";
        let volume = "volume_count=4\navailable_count=0\nauto_attach=true\n";
        assert!(validate_volume_service_status(device, volume).is_err());
    }
}
