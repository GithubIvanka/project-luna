//! Native Luna early-userspace initializer.
//!
//! `luna-init` prepares the final root and replaces itself with the final
//! `/sbin/init`. The final init is `luna-system-runtime`.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitStatus};

const BUSYBOX: &str = "/bin/busybox";
const SYSTEM_MOUNT: &str = "/run/luna-system";
const NEWROOT: &str = "/newroot";
const DATA_MOUNT: &str = "/newroot/data";

fn main() -> ! {
    if let Err(error) = run() {
        eprintln!("luna-init: {error}");
        emergency_shell();
    }
    unreachable!("luna-init emergency shell returned")
}

fn run() -> Result<(), String> {
    mount("proc", "/proc", "proc", "nosuid,nodev,noexec")?;
    mount("sysfs", "/sys", "sysfs", "ro,nosuid,nodev,noexec")?;
    mount("devtmpfs", "/dev", "devtmpfs", "mode=0755,nosuid")
        .or_else(|_| mount("devtmpfs", "/dev", "tmpfs", "mode=0755,nosuid"))?;

    mkdir(SYSTEM_MOUNT)?;
    mkdir(DATA_MOUNT)?;
    for path in [
        &format!("{NEWROOT}/proc"),
        &format!("{NEWROOT}/sys"),
        &format!("{NEWROOT}/dev"),
        &format!("{NEWROOT}/run"),
    ] {
        mkdir(path)?;
    }

    mount_by_label("LUNA-SYSTEM", SYSTEM_MOUNT, "ro")?;
    mount_by_label("LUNA-DATA", DATA_MOUNT, "rw")?;

    let image = system_image_from_cmdline();
    let image_path = format!("{SYSTEM_MOUNT}{image}");
    if !is_regular_file(&image_path) {
        return Err(format!("selected System Image not found: {image_path}"));
    }

    mount_loop_squashfs(&image_path, NEWROOT)?;

    move_mount("/dev", &format!("{NEWROOT}/dev"))?;
    move_mount("/proc", &format!("{NEWROOT}/proc"))?;
    move_mount("/sys", &format!("{NEWROOT}/sys"))?;
    move_mount(SYSTEM_MOUNT, &format!("{NEWROOT}/run/luna-system"))?;

    let init = format!("{NEWROOT}/sbin/init");
    if !is_executable(&init) {
        return Err("final System Image has no executable /sbin/init".to_owned());
    }

    exec_switch_root(NEWROOT, "/sbin/init")
}

fn mkdir(path: &str) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| format!("mkdir {path}: {e}"))
}

fn is_regular_file(path: &str) -> bool {
    fs::metadata(path).map(|m| m.is_file()).unwrap_or(false)
}

fn is_executable(path: &str) -> bool {
    fs::metadata(path)
        .map(|m| m.is_file() && (m.permissions().mode() & 0o111 != 0))
        .unwrap_or(false)
}

fn system_image_from_cmdline() -> String {
    let content = fs::read_to_string("/proc/cmdline").unwrap_or_default();
    content
        .split_whitespace()
        .find_map(|arg| arg.strip_prefix("luna.system_image=").map(str::to_owned))
        .unwrap_or_else(|| "/images/luna-0.1.0.squashfs".to_owned())
}

fn mount(source: &str, target: &str, fs_type: &str, options: &str) -> Result<(), String> {
    let status = Command::new(BUSYBOX)
        .args(["mount", "-t", fs_type, "-o", options, source, target])
        .status()
        .map_err(|e| format!("mount {source} on {target}: {e}"))?;
    require_success(status, &format!("mount {source} on {target}"))
}

fn mount_by_label(label: &str, target: &str, options: &str) -> Result<(), String> {
    let source = format!("/dev/disk/by-label/{label}");
    if fs::metadata(&source).is_err() {
        return Err(format!("block device label is unavailable: {label}"));
    }
    mount(&source, target, "ext4", options)
}

fn mount_loop_squashfs(image: &str, target: &str) -> Result<(), String> {
    mount(image, target, "squashfs", "ro,loop")
}

fn move_mount(source: &str, target: &str) -> Result<(), String> {
    let status = Command::new(BUSYBOX)
        .args(["mount", "--move", source, target])
        .status()
        .map_err(|e| format!("move mount {source} -> {target}: {e}"))?;
    require_success(status, &format!("move mount {source} -> {target}"))
}

fn exec_switch_root(newroot: &str, init: &str) -> Result<(), String> {
    let error = Command::new(BUSYBOX)
        .args(["switch_root", "-c", "/dev/console", newroot, init])
        .exec();
    Err(format!("exec switch_root failed: {error}"))
}

fn require_success(status: ExitStatus, operation: &str) -> Result<(), String> {
    if status.success() {
        Ok(())
    } else {
        Err(format!("{operation} failed with {status}"))
    }
}

fn emergency_shell() -> ! {
    let _ = Command::new(BUSYBOX).arg("sh").status();
    std::process::exit(1)
}

#[cfg(test)]
mod tests {
    use super::system_image_from_cmdline;

    #[test]
    fn default_system_image_is_absolute_squashfs() {
        let value = system_image_from_cmdline();
        assert!(value.starts_with('/'));
        assert!(value.ends_with(".squashfs"));
    }
}
