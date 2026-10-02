fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if std::env::args().any(|arg| arg == "--probe-resources") {
        match luna_user_session::SeatController::discover_default() {
            Ok(seat) => {
                println!("seat={}", seat.resources().name);
                println!("input_devices={}", seat.input_devices().len());
                for device in seat.input_devices() {
                    println!(
                        "input node={} kind={:?} name={}",
                        device.event_node.display(),
                        device.kind,
                        device.name
                    );
                }
                println!("graphics_devices={}", seat.graphics_devices().len());
                for device in seat.graphics_devices() {
                    println!("graphics node={}", device.node.display());
                }
            }
            Err(error) => {
                eprintln!("luna-user-session: resource probe failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }

    if std::env::args().any(|arg| arg == "--probe-graphics") {
        match luna_user_session::SeatController::discover_default() {
            Ok(seat) => {
                for device in seat.graphics_devices() {
                    let drm = luna_user_session::DrmDevice::new(&device.node);
                    match drm.probe() {
                        Ok(probe) => {
                            let r = &probe.resources;
                            println!(
                                "drm node={} dumb={} crtcs={} connectors={} encoders={} max={}x{}",
                                device.node.display(),
                                probe.dumb_buffer,
                                r.crtc_count,
                                r.connector_count,
                                r.encoder_count,
                                r.max_width,
                                r.max_height
                            );
                            match drm.open().and_then(|handle| handle.select_display_target()) {
                                Ok(Some(target)) => println!(
                                    "  target connector={} crtc={} mode={} {}x{}@{}Hz",
                                    target.connector_id,
                                    target.crtc_id,
                                    target.mode.name,
                                    target.mode.width,
                                    target.mode.height,
                                    target.mode.refresh_hz
                                ),
                                Ok(None) => println!("  target=none"),
                                Err(error) => eprintln!(
                                    "  target selection failed for {}: {error}",
                                    device.node.display()
                                ),
                            }
                        }
                        Err(error) => {
                            eprintln!(
                                "luna-user-session: DRM probe {} failed: {error}",
                                device.node.display()
                            );
                            std::process::exit(1);
                        }
                    }
                }
            }
            Err(error) => {
                eprintln!("luna-user-session: graphics resource probe failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }

    if std::env::args().any(|arg| arg == "--probe-buffer") {
        match luna_user_session::SeatController::discover_default() {
            Ok(seat) => {
                let mut succeeded = false;
                for device in seat.graphics_devices() {
                    let drm = luna_user_session::DrmDevice::new(&device.node);
                    let result = drm.open().and_then(|handle| {
                        let target = handle
                            .select_display_target()?
                            .ok_or_else(|| std::io::Error::new(
                                std::io::ErrorKind::NotFound,
                                "no connected display target",
                            ))?;
                        let buffer = handle.create_dumb(target.mode.width, target.mode.height, 32)?;
                        let framebuffer = handle.add_framebuffer(&buffer, 24)?;
                        let mut mapping = buffer.map()?;
                        mapping.fill_xrgb(0x00182028);
                        println!(
                            "buffer node={} connector={} crtc={} fb={} handle={} {}x{} pitch={} size={}",
                            device.node.display(),
                            target.connector_id,
                            target.crtc_id,
                            framebuffer.id(),
                            buffer.handle(),
                            buffer.width,
                            buffer.height,
                            buffer.pitch,
                            buffer.size
                        );
                        Ok::<(), std::io::Error>(())
                    });
                    match result {
                        Ok(()) => succeeded = true,
                        Err(error) => eprintln!(
                            "luna-user-session: buffer probe {} failed: {error}",
                            device.node.display()
                        ),
                    }
                }
                if !succeeded {
                    std::process::exit(1);
                }
            }
            Err(error) => {
                eprintln!("luna-user-session: graphics resource probe failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }

    if args.iter().any(|arg| arg == "--login") {
        match luna_user_session::SessionUi::new(None).authenticate() {
            Ok(credentials) => {
                println!(
                    "authenticated username={} uid={} gid={}",
                    credentials.username, credentials.uid, credentials.gid
                );
            }
            Err(error) => {
                eprintln!("luna-user-session: native login failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }

    if let Some(position) = args.iter().position(|arg| arg == "--login-launch") {
        let program = args.get(position + 1).cloned().unwrap_or_else(|| {
            eprintln!("luna-user-session: --login-launch requires a program");
            std::process::exit(2);
        });
        let command_args = args[position + 2..].to_vec();

        let login_ui = luna_user_session::SessionUi::new(None);
        loop {
            let credentials = loop {
                match login_ui.authenticate() {
                    Ok(credentials) => break credentials,
                    Err(luna_user_session::SessionUiError::Cancelled) => {
                        eprintln!("luna-user-session: login cancelled; returning to login UI");
                    }
                    Err(error) => {
                        eprintln!("luna-user-session: native login failed: {error}");
                        std::process::exit(1);
                    }
                }
            };

            let runtime_dir = format!("/run/user/{}", credentials.uid);
            if let Err(error) = std::fs::create_dir_all(&runtime_dir) {
                eprintln!("luna-user-session: cannot create {runtime_dir}: {error}");
                std::process::exit(1);
            }
            use std::ffi::CString;
            let cpath = CString::new(runtime_dir.as_str()).expect("runtime path has no NUL");
            if unsafe { libc::chown(cpath.as_ptr(), credentials.uid, credentials.gid) } != 0 {
                eprintln!(
                    "luna-user-session: cannot own {runtime_dir}: {}",
                    std::io::Error::last_os_error()
                );
                std::process::exit(1);
            }
            if unsafe { libc::chmod(cpath.as_ptr(), 0o700) } != 0 {
                eprintln!(
                    "luna-user-session: cannot protect {runtime_dir}: {}",
                    std::io::Error::last_os_error()
                );
                std::process::exit(1);
            }

            use std::os::unix::process::CommandExt;
            let home = format!("/home/{}", credentials.username);
            let child_credentials = credentials.clone();
            let mut command = std::process::Command::new(&program);
            command
                .args(&command_args)
                .env("USER", &credentials.username)
                .env("LOGNAME", &credentials.username)
                .env("HOME", &home)
                .env("SHELL", "/bin/ash")
                .env("XDG_RUNTIME_DIR", &runtime_dir)
                .env("XDG_SESSION_TYPE", "wayland")
                .env("XDG_SEAT", "seat0")
                .env("XDG_VTNR", "1")
                .env("LIBSEAT_BACKEND", "seatd")
                .current_dir(&home);

            // Keep this process privileged so logout can return to the native
            // login screen. Only the desktop child receives the user identity.
            unsafe {
                command.pre_exec(move || {
                    if libc::setpgid(0, 0) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    eprintln!("luna-user-session: applying user credentials");
                    child_credentials.apply()?;
                    eprintln!("luna-user-session: user credentials applied");
                    Ok(())
                });
            }

            eprintln!(
                "luna-user-session: launching graphical desktop for uid={}",
                credentials.uid
            );
            match command.spawn() {
                Ok(mut child) => {
                    let child_pid = child.id();
                    let wait_result = child.wait();
                    terminate_session_process_group(child_pid);
                    match wait_result {
                        Ok(status) if status.success() => {
                            eprintln!(
                                "luna-user-session: graphical desktop exited cleanly; returning to login UI"
                            );
                        }
                        Ok(status) => {
                            eprintln!(
                                "luna-user-session: graphical desktop exited: {status}; returning to login UI"
                            );
                        }
                        Err(error) => {
                            eprintln!(
                                "luna-user-session: session child wait failed: {error}; returning to login UI"
                            );
                        }
                    }
                }
                Err(error) => {
                    eprintln!(
                        "luna-user-session: session child start failed: {error}; returning to login UI"
                    );
                }
            }

            if let Err(error) = std::fs::remove_dir_all(&runtime_dir) {
                eprintln!(
                    "luna-user-session: could not remove {} after logout: {}",
                    runtime_dir, error
                );
            }
        }
    }

    if let Err(error) = luna_user_session::handoff_current_identity() {
        eprintln!("luna-user-session: authentication handoff failed: {error}");
        std::process::exit(1);
    }
}

fn terminate_session_process_group(pid: u32) {
    let pgid = -(pid as libc::pid_t);
    unsafe {
        libc::kill(pgid, libc::SIGTERM);
    }
    std::thread::sleep(std::time::Duration::from_millis(100));
    unsafe {
        libc::kill(pgid, libc::SIGKILL);
    }
}
