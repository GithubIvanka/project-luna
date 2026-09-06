//! Native Luna early-userspace initializer.
//!
//! `luna-init` owns only early bootstrap. It constructs a RAM-backed logical
//! root from a selected immutable System Image, mounts runtime pseudo-filesystems
//! and then execs `luna-system-runtime` from that RAM root.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

const BUSYBOX: &str = "/bin/busybox";
const NEWROOT: &str = "/newroot";
const DATA_MOUNT: &str = "/newroot/data";
const SOURCE_ROOT: &str = "/newroot/run/luna-source";
const SYSTEM_MOUNT: &str = "/newroot/run/luna-source/system";
const IMAGE_MOUNT: &str = "/newroot/run/luna-source/image";

/// The first userspace base is deliberately explicit rather than a copy of the
/// complete System Image. Later resources can be hydrated by the runtime layer.
const BOOTSTRAP_PATHS: &[&str] = &[
    "/bin/busybox",
    "/sbin/luna-system-runtime",
    "/sbin/init",
    "/etc/os-release",
    "/etc/hostname",
    "/etc/passwd",
    "/etc/group",
    "/etc/shadow",
    "/etc/profile",
    "/etc/luna",
    "/usr/bin/luna-login",
    "/usr/bin/niri-session",
    "/usr/bin/setpriv",
];

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

    mkdir(NEWROOT)?;
    mount(
        "tmpfs",
        NEWROOT,
        "tmpfs",
        "mode=0755,nosuid,nodev",
    )?;
    mkdir(DATA_MOUNT)?;
    mkdir(SOURCE_ROOT)?;
    mkdir(SYSTEM_MOUNT)?;
    mkdir(IMAGE_MOUNT)?;

    let content = fs::read_to_string("/proc/cmdline").unwrap_or_default();
    let system_device = cmdline_value(&content, "luna.system_device")
        .unwrap_or_else(|| "LABEL=LUNA-SYSTEM".to_owned());
    let data_device = cmdline_value(&content, "luna.data_device")
        .unwrap_or_else(|| "LABEL=LUNA-DATA".to_owned());

    mount_device_spec(&system_device, SYSTEM_MOUNT, "ro")?;
    mount_device_spec(&data_device, DATA_MOUNT, "rw")?;

    let image = system_image_from_cmdline(&content)?;
    let image_path = format!("{SYSTEM_MOUNT}{image}");
    if !is_regular_file(&image_path) {
        return Err(format!("selected System Image not found: {image_path}"));
    }

    mount_loop_squashfs(&image_path, IMAGE_MOUNT)?;

    prepare_root()?;
    materialize_bootstrap(IMAGE_MOUNT, NEWROOT)?;

    mount("proc", &format!("{NEWROOT}/proc"), "proc", "nosuid,nodev,noexec")?;
    mount(
        "sysfs",
        &format!("{NEWROOT}/sys"),
        "sysfs",
        "ro,nosuid,nodev,noexec",
    )?;
    mount(
        "devtmpfs",
        &format!("{NEWROOT}/dev"),
        "devtmpfs",
        "mode=0755,nosuid",
    )
    .or_else(|_| {
        mount(
            "devtmpfs",
            &format!("{NEWROOT}/dev"),
            "tmpfs",
            "mode=0755,nosuid",
        )
    })?;
    mount(
        "tmpfs",
        &format!("{NEWROOT}/run"),
        "tmpfs",
        "mode=0755,nosuid,nodev",
    )?;
    mount(
        "tmpfs",
        &format!("{NEWROOT}/tmp"),
        "tmpfs",
        "mode=1777,nosuid,nodev",
    )?;

    // The SYSTEM and selected image mounts intentionally survive bootstrap and
    // the final chroot. They are internal immutable sources for later runtime
    // hydration. Their lifetime is transferred to the runtime/hydration layer.
    let init = format!("{NEWROOT}/sbin/init");
    if !is_executable(&init) {
        return Err("RAM-backed root has no executable /sbin/init".to_owned());
    }

    unmount("/proc")?;
    unmount("/sys")?;
    unmount("/dev")?;

    exec_chroot(NEWROOT, "/sbin/init")
}

fn prepare_root() -> Result<(), String> {
    for path in BOOTSTRAP_PATHS {
        let relative = path
            .strip_prefix('/')
            .ok_or_else(|| format!("invalid bootstrap path: {path}"))?;
        let destination = Path::new(NEWROOT).join(relative);
        let parent = destination
            .parent()
            .ok_or_else(|| format!("bootstrap destination has no parent: {destination:?}"))?;
        mkdir(&parent.to_string_lossy())?;
    }

    for directory in ["proc", "sys", "dev", "run", "tmp"] {
        mkdir(&format!("{NEWROOT}/{directory}"))?;
    }
    Ok(())
}

fn materialize_bootstrap(source_root: &str, destination_root: &str) -> Result<(), String> {
    for path in BOOTSTRAP_PATHS {
        let source = format!("{source_root}{path}");
        let destination = PathBuf::from(destination_root).join(
            path.strip_prefix('/')
                .ok_or_else(|| format!("invalid bootstrap path: {path}"))?,
        );

        if fs::symlink_metadata(&source).is_err() {
            return Err(format!("bootstrap resource missing from System Image: {path}"));
        }

        let destination_parent = destination
            .parent()
            .ok_or_else(|| format!("bootstrap destination has no parent: {destination:?}"))?;
        mkdir(&destination_parent.to_string_lossy())?;
        copy_recursive(&source, &destination)?;
    }
    Ok(())
}

fn copy_recursive(source: &str, destination: &Path) -> Result<(), String> {
    let status = Command::new(BUSYBOX)
        .args(["cp", "-a", source])
        .arg(destination)
        .status()
        .map_err(|e| format!("copy {source} -> {}: {e}", destination.display()))?;
    require_success(
        status,
        &format!("copy {source} -> {}", destination.display()),
    )
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

fn cmdline_value(content: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    content
        .split_whitespace()
        .find_map(|arg| arg.strip_prefix(&prefix).map(str::to_owned))
}

fn system_image_from_cmdline(content: &str) -> Result<String, String> {
    let value = cmdline_value(content, "luna.system_image")
        .unwrap_or_else(|| "/images/luna-0.1.0.squashfs".to_owned());

    if !value.starts_with("/images/")
        || !value.ends_with(".squashfs")
        || value.contains("../")
        || value.contains("//")
    {
        return Err(format!("invalid System Image path: {value}"));
    }

    Ok(value)
}

fn mount(source: &str, target: &str, fs_type: &str, options: &str) -> Result<(), String> {
    let status = Command::new(BUSYBOX)
        .args(["mount", "-t", fs_type, "-o", options, source, target])
        .status()
        .map_err(|e| format!("mount {source} on {target}: {e}"))?;
    require_success(status, &format!("mount {source} on {target}"))
}

fn mount_device_spec(spec: &str, target: &str, options: &str) -> Result<(), String> {
    if spec.starts_with("/dev/") {
        return mount(spec, target, "ext4", options);
    }

    if let Some(label) = spec.strip_prefix("LABEL=") {
        let status = Command::new(BUSYBOX)
            .args(["blkid", "-L", label])
            .output()
            .map_err(|e| format!("resolve filesystem label {label}: {e}"))?;
        if status.status.success() {
            let source = String::from_utf8_lossy(&status.stdout).trim().to_owned();
            if source.starts_with("/dev/") {
                return mount(&source, target, "ext4", options);
            }
        }
        return mount(spec, target, "ext4", options);
    }

    Err(format!("unsupported block-device specification: {spec}"))
}

fn mount_loop_squashfs(image: &str, target: &str) -> Result<(), String> {
    mount(image, target, "squashfs", "ro,loop")
}

fn unmount(target: &str) -> Result<(), String> {
    let status = Command::new(BUSYBOX)
        .args(["umount", "-l", target])
        .status()
        .map_err(|e| format!("unmount {target}: {e}"))?;
    require_success(status, &format!("unmount {target}"))
}

fn exec_chroot(newroot: &str, init: &str) -> Result<(), String> {
    let error = Command::new(BUSYBOX)
        .args(["chroot", newroot, init])
        .exec();
    Err(format!("exec chroot failed: {error}"))
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
    use super::{cmdline_value, system_image_from_cmdline, BOOTSTRAP_PATHS, IMAGE_MOUNT, SOURCE_ROOT, SYSTEM_MOUNT};

    #[test]
    fn parses_boot_device_from_cmdline() {
        let value = cmdline_value(
            "quiet luna.system_device=/dev/vda2 luna.data_device=/dev/vda3",
            "luna.system_device",
        );
        assert_eq!(value.as_deref(), Some("/dev/vda2"));
    }

    #[test]
    fn accepts_default_system_image() {
        let value = system_image_from_cmdline("").unwrap();
        assert_eq!(value, "/images/luna-0.1.0.squashfs");
    }

    #[test]
    fn rejects_path_traversal_in_system_image() {
        let error = system_image_from_cmdline("luna.system_image=/images/../data/x.squashfs");
        assert!(error.is_err());
    }

    #[test]
    fn bootstrap_is_an_explicit_subset() {
        assert!(BOOTSTRAP_PATHS.contains(&"/sbin/luna-system-runtime"));
        assert!(!BOOTSTRAP_PATHS.contains(&"/usr/share"));
    }

    #[test]
    fn immutable_sources_live_inside_runtime_run() {
        assert!(SOURCE_ROOT.starts_with("/newroot/run/"));
        assert!(SYSTEM_MOUNT.starts_with(SOURCE_ROOT));
        assert!(IMAGE_MOUNT.starts_with(SOURCE_ROOT));
    }
}
