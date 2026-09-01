use std::time::Duration;

use luna_common::UserId;
use luna_system_runtime::{SystemRuntime, SystemRuntimeService};

fn main() {
    let shell = std::env::args().nth(1).unwrap_or_else(|| "/bin/sh".to_owned());
    let respawn = std::env::var_os("LUNA_NO_RESPAWN").is_none();
    let mut runtime = SystemRuntimeService::new();
    runtime.start();

    loop {
        let session = match runtime.create_session(UserId::from("luna")) {
            Ok(id) => id,
            Err(error) => {
                eprintln!("luna-system-runtime: failed to create initial session: {error}");
                std::process::exit(1);
            }
        };

        if let Err(error) = runtime.launch_session_shell(session, &shell) {
            eprintln!("luna-system-runtime: failed to launch shell {shell}: {error}");
            std::process::exit(1);
        }

        loop {
            if let Err(error) = runtime.supervise() {
                eprintln!("luna-system-runtime: supervision error: {error}");
                std::process::exit(1);
            }
            if runtime.session(session).map(|value| matches!(value.state(), luna_user_session::SessionState::Ended)).unwrap_or(true) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        if !respawn {
            break;
        }
    }
}
