use std::time::Duration;

use luna_common::UserId;
use luna_system_runtime::{SystemRuntime, SystemRuntimeService};

fn main() {
    let shell = std::env::args().nth(1).unwrap_or_else(|| "/bin/sh".to_owned());
    let mut runtime = SystemRuntimeService::new();
    runtime.start();

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
        if runtime.sessions().iter().all(|session| matches!(session.state(), luna_user_session::SessionState::Ended)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
