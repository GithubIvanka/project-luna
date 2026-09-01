use std::fs;
use std::time::Duration;

use luna_common::{UserId, Version};
use luna_system_manager::{KernelRef, PersistentSystemManager, SystemImageRef, SystemState};
use luna_system_runtime::{SystemRuntime, SystemRuntimeService};
use luna_user_session::SessionState;

fn default_development_system_state() -> SystemState {
    SystemState::new(
        SystemImageRef::new(Version::new(0, 1, 0)),
        SystemImageRef::new(Version::new(0, 1, 0)),
        KernelRef::new(Version::new(0, 1, 0)),
        KernelRef::new(Version::new(0, 1, 0)),
    )
}

fn session_command() -> String {
    if let Some(command) = std::env::var_os("LUNA_SESSION_COMMAND") {
        if !command.is_empty() {
            return command.to_string_lossy().into_owned();
        }
    }

    fs::read_to_string("/etc/luna/session")
        .ok()
        .and_then(|value| {
            value
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty() && !line.starts_with('#'))
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "/bin/sh".to_owned())
}

fn session_mode() -> String {
    fs::read_to_string("/etc/luna/mode")
        .ok()
        .and_then(|value| {
            value
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty() && !line.starts_with('#'))
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "console".to_owned())
}

fn main() {
    let command = session_command();
    let mode = session_mode();
    let respawn = std::env::var_os("LUNA_NO_RESPAWN").is_none();
    let mut runtime = SystemRuntimeService::new();

    match PersistentSystemManager::open_or_initialize_redb("/data", default_development_system_state()) {
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

    loop {
        let session = if mode == "graphical" {
            let id = match runtime.create_login_session(UserId::from("luna")) {
                Ok(id) => id,
                Err(error) => {
                    eprintln!("luna-system-runtime: failed to create login session: {error}");
                    std::process::exit(1);
                }
            };
            if let Err(error) = runtime.authenticate_session(id) {
                eprintln!("luna-system-runtime: authentication failed: {error}");
                std::process::exit(1);
            }
            if let Err(error) = runtime.launch_graphical_session(
                id,
                &command,
                std::iter::empty::<&str>(),
            ) {
                eprintln!("luna-system-runtime: failed to launch graphical session {command}: {error}");
                std::process::exit(1);
            }
            id
        } else {
            let id = match runtime.create_session(UserId::from("luna")) {
                Ok(id) => id,
                Err(error) => {
                    eprintln!("luna-system-runtime: failed to create session: {error}");
                    std::process::exit(1);
                }
            };
            if let Err(error) = runtime.launch_session_shell(id, &command) {
                eprintln!("luna-system-runtime: failed to launch session command {command}: {error}");
                std::process::exit(1);
            }
            id
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
