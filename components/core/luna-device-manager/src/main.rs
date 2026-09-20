use std::fs;
use std::path::Path;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

use luna_device_manager::DeviceManager;

const UDEVD: &str = "/usr/bin/systemd-udevd";
const UDEVADM: &str = "/usr/bin/udevadm";
const UDEV_CONTROL: &str = "/run/udev/control";
const READY_PATH: &str = "/run/luna/device-manager.ready";
const STATUS_PATH: &str = "/run/luna/device-manager.status";
const WAIT_TIMEOUT: Duration = Duration::from_secs(3);

fn wait_for_udev(child: &mut Child) -> Result<(), String> {
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        if Path::new(UDEV_CONTROL).exists() {
            return Ok(());
        }
        if let Some(status) = child.try_wait().map_err(|e| format!("poll udevd: {e}"))? {
            return Err(format!("udevd exited before readiness: {status}"));
        }
        if Instant::now() >= deadline {
            return Err("timed out waiting for udev control socket".into());
        }
        thread::sleep(Duration::from_millis(25));
    }
}
fn run() -> Result<(), String> {
    std::fs::create_dir_all("/run/luna").map_err(|e| format!("create /run/luna: {e}"))?;
    let _ = std::fs::remove_file(READY_PATH);
    let _ = std::fs::remove_file(STATUS_PATH);
    let mut manager = DeviceManager::new();
    let uevent_monitor = luna_device_manager::UeventMonitor::open()
        .map_err(|error| format!("open native kernel uevent monitor: {error}"))?;
    eprintln!("luna-device-manager: native kernel uevent monitor ready");
    // The native kernel uevent monitor is sufficient for Luna's own device
    // registry. The udev daemon/udevadm pair remains a transitional provider
    // for richer desktop metadata when present, but Recovery must not depend
    // on shipping the whole udev userspace just to reach the graphical session.
    let mut udevd = match Command::new(UDEVD)
        .args(["--children-max=8", "--resolve-names=late"])
        .spawn()
    {
        Ok(mut child) => {
            if let Err(error) = wait_for_udev(&mut child) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
            eprintln!("luna-device-manager: udev compatibility backend ready");
            Some(child)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!(
                "luna-device-manager: udev compatibility backend unavailable; using native mode"
            );
            None
        }
        Err(error) => {
            return Err(format!("start udev compatibility backend: {error}"));
        }
    };

    if udevd.is_some() {
        for subsystem in ["input", "drm", "sound", "block", "usb"] {
            let status = Command::new(UDEVADM)
                .args(["trigger", "--action=add", "--subsystem-match", subsystem])
                .status()
                .map_err(|e| format!("trigger udev {subsystem}: {e}"))?;
            if !status.success() {
                eprintln!("luna-device-manager: udev trigger for {subsystem} returned {status}");
            }
        }

        let settle = Command::new(UDEVADM)
            .args(["settle", "--timeout=3"])
            .status()
            .map_err(|e| format!("settle udev events: {e}"))?;
        if !settle.success() {
            eprintln!("luna-device-manager: udev settle returned {settle}");
        }
    }

    manager
        .refresh_input_devices()
        .map_err(|error| format!("input sysfs scan unavailable: {error}"))?;
    let input_count = manager.input_devices().len();
    let mut status = format!("input_devices={input_count}\n");
    for device in manager.input_devices() {
        status.push_str("input=");
        status.push_str(device.node());
        status.push('\n');
        status.push_str("name=");
        status.push_str(device.name().unwrap_or_default());
        status.push('\n');
        if udevd.is_some() {
            let output = Command::new(UDEVADM)
                .args(["info", "--query=property", "--name", device.node()])
                .output()
                .map_err(|error| format!("query udev properties for {}: {error}", device.node()))?;
            status.push_str("udev_info_status=");
            status.push_str(&output.status.code().unwrap_or(-1).to_string());
            status.push('\n');
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                if line.starts_with("DEVNAME=")
                    || line.starts_with("ID_INPUT=")
                    || line.starts_with("ID_INPUT_KEYBOARD=")
                    || line.starts_with("ID_INPUT_MOUSE=")
                    || line.starts_with("ID_INPUT_TOUCHPAD=")
                    || line.starts_with("ID_SEAT=")
                    || line.starts_with("TAGS=")
                {
                    status.push_str(line);
                    status.push('\n');
                }
            }
        } else {
            status.push_str("udev_info_status=unavailable\n");
        }
    }
    let udev_db_entries = fs::read_dir("/run/udev/data")
        .map(|entries| entries.filter_map(Result::ok).count())
        .unwrap_or(0);
    status.push_str("udev_compat_backend=");
    status.push_str(if udevd.is_some() { "1\n" } else { "0\n" });
    status.push_str("native_uevent_monitor=1\n");
    status.push_str("udev_db_entries=");
    status.push_str(&udev_db_entries.to_string());
    status.push('\n');
    std::fs::write(STATUS_PATH, status)
        .map_err(|error| format!("write device-manager status: {error}"))?;
    std::fs::write(READY_PATH, b"ready\n")
        .map_err(|error| format!("write device-manager readiness: {error}"))?;
    eprintln!("luna-device-manager: ready; discovered {input_count} input device(s)");
    for device in manager.input_devices() {
        eprintln!(
            "luna-device-manager: input {} {:?}",
            device.node(),
            device.name()
        );
    }

    loop {
        if uevent_monitor
            .wait(Duration::from_millis(250))
            .map_err(|error| format!("wait for kernel uevent: {error}"))?
        {
            while let Some(event) = uevent_monitor
                .receive()
                .map_err(|error| format!("receive kernel uevent: {error}"))?
            {
                let subsystem = event.subsystem().unwrap_or("unknown");
                let devname = event.devname().unwrap_or("");
                eprintln!(
                    "luna-device-manager: uevent action={} subsystem={} devname={} devpath={}",
                    event.action, subsystem, devname, event.devpath
                );
                if subsystem == "input" {
                    manager
                        .refresh_input_devices()
                        .map_err(|error| format!("refresh input registry after uevent: {error}"))?;
                }
            }
        }

        if let Some(child) = udevd.as_mut() {
            match child
                .try_wait()
                .map_err(|e| format!("poll udev compatibility backend: {e}"))?
            {
                Some(status) => return Err(format!("udev compatibility backend exited: {status}")),
                None => {}
            }
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("luna-device-manager: {error}");
        std::process::exit(1);
    }
}
