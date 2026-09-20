fn main() {
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

    if let Err(error) = luna_user_session::handoff_current_identity() {
        eprintln!("luna-user-session: authentication handoff failed: {error}");
        std::process::exit(1);
    }
}
