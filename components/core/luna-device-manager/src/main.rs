use std::path::Path;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

use luna_device_manager::{BlockVolumeDiscovery, DeviceManager, UeventMonitor};

const UDEVD: &str = "/usr/bin/udevd";
const UDEVADM: &str = "/usr/bin/udevadm";
const UDEV_CONTROL: &str = "/run/udev/control";
const READY_PATH: &str = "/run/luna/device-manager.ready";
const STATUS_PATH: &str = "/run/luna/device-manager.status";
const VOLUME_SNAPSHOT_PATH: &str = "/run/luna/device-manager.volumes";
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

fn settle_udev() {
    match Command::new(UDEVADM)
        .args(["settle", "--timeout=3"])
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => eprintln!("luna-device-manager: udev settle returned {status}"),
        Err(error) => eprintln!("luna-device-manager: cannot settle udev events: {error}"),
    }
}

fn encode_hex(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn encode_optional(value: Option<&str>) -> String {
    value.map(encode_hex).unwrap_or_else(|| "-".to_owned())
}

/// Publish a complete, atomically replaced inventory snapshot. Volume Manager
/// consumes this domain boundary; it does not scan sysfs or listen to kernel events.
fn publish_volume_snapshot(discovery: &BlockVolumeDiscovery) -> Result<usize, String> {
    let mut volumes = discovery
        .scan()
        .map_err(|error| format!("scan block devices: {error}"))?;
    volumes.sort_by(|a, b| a.id().cmp(b.id()));

    let mut output = String::from("format=1\n");
    for volume in &volumes {
        output.push_str(&format!(
            "{}\t{}\t{}\t{}\t{:?}\t{}\t{}\n",
            encode_hex(volume.id().as_str()),
            encode_optional(volume.label()),
            encode_hex(volume.native_name()),
            encode_optional(volume.filesystem()),
            volume.role(),
            encode_hex(volume.backend_node()),
            u8::from(volume.is_removable()),
        ));
    }

    let temporary = format!("{VOLUME_SNAPSHOT_PATH}.tmp");
    std::fs::write(&temporary, output)
        .map_err(|error| format!("write volume inventory snapshot: {error}"))?;
    std::fs::rename(&temporary, VOLUME_SNAPSHOT_PATH)
        .map_err(|error| format!("publish volume inventory snapshot: {error}"))?;
    Ok(volumes.len())
}

fn write_status(
    input_count: usize,
    volume_count: usize,
    udev_available: bool,
) -> Result<(), String> {
    let status = format!(
        "input_devices={input_count}\nblock_volumes={volume_count}\nnative_uevent_monitor=1\nudev_compat_backend={}\n",
        u8::from(udev_available)
    );
    std::fs::write(STATUS_PATH, status)
        .map_err(|error| format!("write device-manager status: {error}"))
}

fn run() -> Result<(), String> {
    std::fs::create_dir_all("/run/luna").map_err(|e| format!("create /run/luna: {e}"))?;
    std::fs::create_dir_all("/run/udev").map_err(|e| format!("create /run/udev: {e}"))?;
    let _ = std::fs::remove_file(READY_PATH);
    let _ = std::fs::remove_file(STATUS_PATH);
    let _ = std::fs::remove_file(VOLUME_SNAPSHOT_PATH);

    let mut manager = DeviceManager::new();
    let discovery = BlockVolumeDiscovery::new();
    let monitor = UeventMonitor::open()
        .map_err(|error| format!("open native kernel uevent monitor: {error}"))?;
    eprintln!("luna-device-manager: native kernel uevent monitor ready");

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
            eprintln!("luna-device-manager: udev unavailable; using native device mode");
            None
        }
        Err(error) => return Err(format!("start udev compatibility backend: {error}")),
    };

    if udevd.is_some() {
        for subsystem in ["input", "drm", "sound", "usb", "block"] {
            let status = Command::new(UDEVADM)
                .args(["trigger", "--action=add", "--subsystem-match", subsystem])
                .status()
                .map_err(|e| format!("trigger udev {subsystem}: {e}"))?;
            if !status.success() {
                eprintln!("luna-device-manager: udev trigger for {subsystem}: {status}");
            }
        }
        settle_udev();
    }

    manager
        .refresh_input_devices()
        .map_err(|error| format!("input sysfs scan unavailable: {error}"))?;
    let mut volume_count = publish_volume_snapshot(&discovery)?;
    write_status(manager.input_devices().len(), volume_count, udevd.is_some())?;
    std::fs::write(READY_PATH, b"ready\n")
        .map_err(|error| format!("write device-manager readiness: {error}"))?;
    eprintln!(
        "luna-device-manager: ready; discovered {} input device(s) and {volume_count} block volume(s)",
        manager.input_devices().len()
    );

    loop {
        if monitor
            .wait(Duration::from_millis(250))
            .map_err(|error| format!("wait for kernel uevent: {error}"))?
        {
            let mut refresh_input = false;
            let mut refresh_blocks = false;
            while let Some(event) = monitor
                .receive()
                .map_err(|error| format!("receive kernel uevent: {error}"))?
            {
                let subsystem = event.subsystem().unwrap_or("unknown");
                eprintln!(
                    "luna-device-manager: uevent action={} subsystem={} devname={} devpath={}",
                    event.action,
                    subsystem,
                    event.devname().unwrap_or(""),
                    event.devpath
                );
                refresh_input |= subsystem == "input";
                refresh_blocks |= subsystem == "block";
            }

            if refresh_input {
                manager
                    .refresh_input_devices()
                    .map_err(|error| format!("refresh input registry: {error}"))?;
            }
            if refresh_blocks {
                settle_udev();
                volume_count = publish_volume_snapshot(&discovery)?;
                write_status(manager.input_devices().len(), volume_count, udevd.is_some())?;
                eprintln!(
                    "luna-device-manager: published block inventory ({volume_count} volume(s))"
                );
            } else if refresh_input {
                write_status(manager.input_devices().len(), volume_count, udevd.is_some())?;
            }
        }

        if let Some(child) = udevd.as_mut() {
            if let Some(status) = child
                .try_wait()
                .map_err(|error| format!("poll udev compatibility backend: {error}"))?
            {
                return Err(format!("udev compatibility backend exited: {status}"));
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
