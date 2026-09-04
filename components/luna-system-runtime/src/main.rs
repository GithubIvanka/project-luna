use std::ffi::OsString;
use std::fs;
use std::process::ExitStatus;
use std::time::Duration;

use luna_common::{UserId, Version};
use luna_system_manager::{KernelRef, PersistentSystemManager, SystemImageRef, SystemState};
use luna_system_runtime::{ProcessId, ProcessState, SystemRuntime, SystemRuntimeService};
use luna_user_session::SessionState;

fn default_development_system_state() -> SystemState {
    SystemState::new(
        SystemImageRef::new(Version::new(0, 1, 0)),
        SystemImageRef::new(Version::new(0, 1, 0)),
        KernelRef::new(Version::new(0, 1, 0)),
        KernelRef::new(Version::new(0, 1, 0)),
    )
}

fn graphical_login_command() -> String {
    if let Some(command) = std::env::var_os("LUNA_GRAPHICAL_LOGIN_COMMAND")
        && !command.is_empty()
    {
        return command.to_string_lossy().into_owned();
    }

    fs::read_to_string("/etc/luna/graphical-login")
        .ok()
        .and_then(|value| {
            value
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty() && !line.starts_with('#'))
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "/usr/bin/luna-login".to_owned())
}

fn graphical_session_command() -> String {
    if let Some(command) = std::env::var_os("LUNA_GRAPHICAL_SESSION_COMMAND")
        && !command.is_empty()
    {
        return command.to_string_lossy().into_owned();
    }

    fs::read_to_string("/etc/luna/graphical-session")
        .ok()
        .and_then(|value| {
            value
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty() && !line.starts_with('#'))
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "/usr/bin/niri-session".to_owned())
}

fn login_succeeded(status: &ExitStatus) -> bool {
    status.success()
}

fn start_system_services(runtime: &mut SystemRuntimeService) -> Vec<ProcessId> {
    if std::env::var_os("LUNA_SKIP_SYSTEM_SERVICES").is_some() || !nix_like_root() {
        return Vec::new();
    }

    let mut services = Vec::new();
    let definitions: [(&str, &[&str]); 4] = [
        ("/usr/bin/dbus-daemon", &["--system", "--nofork"]),
        ("/usr/sbin/NetworkManager", &["--no-daemon"]),
        ("/usr/libexec/bluetooth/bluetoothd", &["--nodetach"]),
        ("/usr/libexec/udisks2/udisksd", &[]),
    ];

    for (program, args) in definitions {
        match runtime.spawn_process(program, args.iter().copied()) {
            Ok(id) => {
                eprintln!(
                    "luna-system-runtime: started system service {program} (pid {})",
                    id.get()
                );
                services.push(id);
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
    let args = [
        OsString::from("--reuid"),
        OsString::from(username.as_str()),
        OsString::from("--regid"),
        OsString::from(username.as_str()),
        OsString::from("--init-groups"),
        OsString::from("--"),
        OsString::from(program),
    ];
    runtime.spawn_process("/usr/bin/setpriv", args)
}

fn main() {
    let login_command = graphical_login_command();
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
                "luna-system-runtime: System Image {}, kernel {}",
                state.current().version(),
                state.current_kernel().version()
            );
        }
        Err(error) => {
            eprintln!("luna-system-runtime: persistent system state unavailable: {error}");
            std::process::exit(1);
        }
    }

    runtime.start();
    let system_services = start_system_services(&mut runtime);

    loop {
        let session = match runtime.create_login_session(UserId::from("luna")) {
            Ok(id) => id,
            Err(error) => {
                eprintln!("luna-system-runtime: failed to create graphical UserSession: {error}");
                std::process::exit(1);
            }
        };

        let login_process = match runtime.spawn_process(&login_command, std::iter::empty::<&str>())
        {
            Ok(id) => id,
            Err(error) => {
                eprintln!(
                    "luna-system-runtime: failed to launch graphical login {login_command}: {error}"
                );
                std::process::exit(1);
            }
        };

        let login_status = loop {
            match runtime.poll_process(login_process) {
                Ok(ProcessState::Running) => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Ok(ProcessState::Exited(status)) => break status,
                Err(error) => {
                    eprintln!("luna-system-runtime: graphical login supervision failed: {error}");
                    std::process::exit(1);
                }
            }
        };

        if !login_succeeded(&login_status) {
            eprintln!(
                "luna-system-runtime: graphical authentication failed; returning to login screen"
            );
            let _ = runtime.cancel_login(session);
            if !respawn {
                break;
            }
            continue;
        }

        if let Err(error) = runtime.authenticate_session(session) {
            eprintln!("luna-system-runtime: authentication transition failed: {error}");
            std::process::exit(1);
        }

        if let Err(error) = launch_graphical_user_session(&mut runtime, session, &session_command) {
            eprintln!(
                "luna-system-runtime: failed to launch graphical UserSession {session_command}: {error}"
            );
            std::process::exit(1);
        }

        loop {
            if let Err(error) = runtime.supervise() {
                eprintln!("luna-system-runtime: supervision error: {error}");
                std::process::exit(1);
            }

            for service in &system_services {
                if matches!(runtime.poll_process(*service), Ok(ProcessState::Exited(_))) {
                    eprintln!(
                        "luna-system-runtime: a supervised host service exited; continuing in degraded mode"
                    );
                }
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
