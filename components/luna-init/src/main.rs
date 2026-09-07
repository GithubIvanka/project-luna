//! Native Luna early-userspace initializer.
//!
//! `luna-init` owns only early bootstrap. It constructs the RAM-backed logical
//! root directly, attaches DATA at logical `/`, and keeps SYSTEM/SquashFS as
//! hidden immutable source mounts outside the future logical root.

use std::ffi::CString;
use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

const BUSYBOX: &str = "/bin/busybox";
const NEWROOT: &str = "/newroot";
const OLDROOT: &str = "/newroot/.luna-oldroot";
const DATA_MOUNT: &str = "/newroot/data";
const SYSTEM_MOUNT: &str = "/luna-source/system";
const IMAGE_MOUNT: &str = "/luna-source/image";
const SYSTEM_SOURCE_FD_ENV: &str = "LUNA_SYSTEM_SOURCE_FD";
const IMAGE_SOURCE_FD_ENV: &str = "LUNA_IMAGE_SOURCE_FD";
const SYS_PIVOT_ROOT: usize = 155;
const SYS_UMOUNT2: usize = 166;
const MNT_DETACH: usize = 2;
const DEFAULT_SYSTEM_IMAGE: &str = "/images/luna-0.1.0.squashfs";

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
    mkdir("/luna-source")?;
    mkdir(SYSTEM_MOUNT)?;
    mkdir(IMAGE_MOUNT)?;

    // `/run` is normal volatile runtime state. It is not used as a container for
    // SYSTEM or System Image sources; those sources remain outside the future `/`.
    mkdir(&format!("{NEWROOT}/run"))?;
    mount(
        "tmpfs",
        &format!("{NEWROOT}/run"),
        "tmpfs",
        "mode=0755,nosuid,nodev",
    )?;

    let content = fs::read_to_string("/proc/cmdline").unwrap_or_default();
    let system_device = cmdline_value(&content, "luna.system_device")
        .unwrap_or_else(|| "LABEL=LUNA-SYSTEM".to_owned());
    let data_device = cmdline_value(&content, "luna.data_device")
        .unwrap_or_else(|| "LABEL=LUNA-DATA".to_owned());

    // SYSTEM is physical immutable storage, never the logical root. DATA is
    // independently attached to the RAM root and becomes logical `/data`.
    mount_device_spec(&system_device, SYSTEM_MOUNT, "ro")?;
    mount_device_spec(&data_device, DATA_MOUNT, "rw")?;

    let image = system_image_from_cmdline(&content)?;
    let image_path = format!("{SYSTEM_MOUNT}{image}");
    if !is_regular_file(&image_path) {
        return Err(format!("selected System Image not found: {image_path}"));
    }

    let manifest_path = manifest_path_for_image(&image)?;
    let manifest = fs::read_to_string(format!("{SYSTEM_MOUNT}{manifest_path}"))
        .map_err(|e| format!("read System Image manifest {manifest_path}: {e}"))?;
    let bootstrap_paths = parse_bootstrap_paths(&manifest)?;
    validate_bootstrap_contract(&bootstrap_paths)?;

    mount_loop_squashfs(&image_path, IMAGE_MOUNT)?;

    prepare_root(&bootstrap_paths)?;
    materialize_bootstrap(IMAGE_MOUNT, NEWROOT, &bootstrap_paths)?;

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
        &format!("{NEWROOT}/tmp"),
        "tmpfs",
        "mode=1777,nosuid,nodev",
    )?;

    // Keep directory FDs for the immutable physical sources. After pivot_root
    // the old initramfs tree is detached and inaccessible by path, while these
    // FDs remain available only to the trusted system runtime for later hydration.
    let system_source = File::open(SYSTEM_MOUNT)
        .map_err(|e| format!("open SYSTEM source for runtime handoff: {e}"))?;
    let image_source = File::open(IMAGE_MOUNT)
        .map_err(|e| format!("open System Image source for runtime handoff: {e}"))?;
    clear_cloexec(system_source.as_raw_fd())?;
    clear_cloexec(image_source.as_raw_fd())?;

    let init = format!("{NEWROOT}/sbin/init");
    if !is_executable(&init) {
        return Err("RAM-backed root has no executable /sbin/init".to_owned());
    }

    unmount("/proc")?;
    unmount("/sys")?;
    unmount("/dev")?;
    pivot_root()?

    // Source mounts are intentionally no longer reachable through a path from
    // the logical root. The open FDs above keep the trusted source objects alive.
    exec_init(
        "/sbin/init",
        system_source.as_raw_fd(),
        image_source.as_raw_fd(),
    )
}

fn prepare_root(bootstrap_paths: &[String]) -> Result<(), String> {
    for path in bootstrap_paths {
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
    mkdir(OLDROOT)?;
    Ok(())
}

fn materialize_bootstrap(
    source_root: &str,
    destination_root: &str,
    bootstrap_paths: &[String],
) -> Result<(), String> {
    for path in bootstrap_paths {
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

fn parse_bootstrap_paths(manifest: &str) -> Result<Vec<String>, String> {
    let mut section = "";
    let mut value = None;
    for raw in manifest.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len() - 1];
            continue;
        }
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        if section == "bootstrap" && key.trim() == "paths" {
            value = Some(parse_string_array(raw_value.trim())?);
        }
    }

    value.ok_or_else(|| "System Image manifest has no [bootstrap].paths".to_owned())
}

fn parse_string_array(value: &str) -> Result<Vec<String>, String> {
    let value = value.trim();
    if !value.starts_with('[') || !value.ends_with(']') {
        return Err("bootstrap.paths must be a TOML string array".to_owned());
    }

    let body = &value[1..value.len() - 1];
    let mut values = Vec::new();
    let mut rest = body.trim();
    while !rest.is_empty() {
        if !rest.starts_with('"') {
            return Err("bootstrap.paths entries must use double-quoted strings".to_owned());
        }
        let escaped = rest[1..]
            .find('"')
            .ok_or_else(|| "unterminated bootstrap.paths string".to_owned())?;
        let end = escaped + 1;
        let item = &rest[1..end];
        if item.contains('\\') {
            return Err("bootstrap.paths does not support escaped strings".to_owned());
        }
        values.push(item.to_owned());
        rest = rest[end + 1..].trim_start();
        if rest.is_empty() {
            break;
        }
        let Some(after_comma) = rest.strip_prefix(',') else {
            return Err("bootstrap.paths entries must be comma-separated".to_owned());
        };
        rest = after_comma.trim_start();
    }
    if values.is_empty() {
        return Err("bootstrap.paths must not be empty".to_owned());
    }
    Ok(values)
}

fn validate_bootstrap_contract(paths: &[String]) -> Result<(), String> {
    let required = ["/bin/busybox", "/sbin/luna-system-runtime", "/sbin/init"];
    for path in required {
        if !paths.iter().any(|value| value == path) {
            return Err(format!("bootstrap manifest is missing required path: {path}"));
        }
    }

    for path in paths {
        if !path.starts_with('/')
            || path.contains("//")
            || path.contains("../")
            || path.ends_with("/..")
            || path.contains('\0')
        {
            return Err(format!("invalid bootstrap path: {path}"));
        }
    }
    Ok(())
}

fn manifest_path_for_image(image: &str) -> Result<String, String> {
    let stem = image
        .strip_suffix(".squashfs")
        .ok_or_else(|| format!("invalid System Image filename: {image}"))?;
    Ok(format!("{stem}.toml"))
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
        .unwrap_or_else(|| DEFAULT_SYSTEM_IMAGE.to_owned());

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

fn clear_cloexec(fd: i32) -> Result<(), String> {
    const F_GETFD: i32 = 1;
    const F_SETFD: i32 = 2;
    const FD_CLOEXEC: i32 = 1;

    unsafe extern "C" {
        fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    }

    let flags = unsafe { fcntl(fd, F_GETFD) };
    if flags < 0 {
        return Err(format!("get fd flags for {fd} failed"));
    }
    if unsafe { fcntl(fd, F_SETFD, flags & !FD_CLOEXEC) } < 0 {
        return Err(format!("clear close-on-exec for fd {fd} failed"));
    }
    Ok(())
}

fn pivot_root() -> Result<(), String> {
    let new_root = CString::new(NEWROOT).map_err(|_| "invalid new root".to_owned())?;
    let put_old = CString::new(OLDROOT)
        .map_err(|_| "invalid old root path".to_owned())?;

    unsafe extern "C" {
        fn syscall(number: usize, ...) -> isize;
    }

    let result = unsafe { syscall(SYS_PIVOT_ROOT, new_root.as_ptr(), put_old.as_ptr()) };
    if result != 0 {
        return Err(format!("pivot_root failed: errno {}", -result));
    }

    std::env::set_current_dir("/").map_err(|e| format!("set logical root cwd: {e}"))?;

    let old_root = CString::new("/.luna-oldroot")
        .map_err(|_| "invalid old root path".to_owned())?;
    let result = unsafe { syscall(SYS_UMOUNT2, old_root.as_ptr(), MNT_DETACH) };
    if result != 0 {
        return Err(format!("detach old initramfs root failed: errno {}", -result));
    }

    Ok(())
}

fn exec_init(init: &str, system_fd: i32, image_fd: i32) -> Result<(), String> {
    let error = Command::new(init)
        .env(SYSTEM_SOURCE_FD_ENV, system_fd.to_string())
        .env(IMAGE_SOURCE_FD_ENV, image_fd.to_string())
        .exec();
    Err(format!("exec {init} failed: {error}"))
}

fn require_success(status: ExitStatus, operation: &str) -> Result<(), String> {
    if status.success() {
        Ok(())
    } else {
        Err(format!("{operation} failed with {status}"))
    }
}

fn unmount(target: &str) -> Result<(), String> {
    let status = Command::new(BUSYBOX)
        .args(["umount", "-l", target])
        .status()
        .map_err(|e| format!("unmount {target}: {e}"))?;
    require_success(status, &format!("unmount {target}"))
}

fn emergency_shell() -> ! {
    let _ = Command::new(BUSYBOX).arg("sh").status();
    std::process::exit(1)
}

#[cfg(test)]
mod tests {
    use super::{
        manifest_path_for_image, parse_bootstrap_paths, system_image_from_cmdline,
        validate_bootstrap_contract,
    };

    #[test]
    fn parses_boot_device_from_cmdline() {
        let value = super::cmdline_value(
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
    fn derives_adjacent_manifest_path() {
        assert_eq!(
            manifest_path_for_image("/images/luna-1.2.3.squashfs").unwrap(),
            "/images/luna-1.2.3.toml"
        );
    }

    #[test]
    fn parses_manifest_bootstrap_paths() {
        let manifest = r#"
            [image]
            version = "1.2.3"

            [bootstrap]
            paths = ["/bin/busybox", "/sbin/luna-system-runtime", "/sbin/init"]
        "#;
        let paths = parse_bootstrap_paths(manifest).unwrap();
        assert_eq!(
            paths,
            vec![
                "/bin/busybox".to_owned(),
                "/sbin/luna-system-runtime".to_owned(),
                "/sbin/init".to_owned(),
            ]
        );
    }

    #[test]
    fn rejects_incomplete_bootstrap_contract() {
        let paths = vec!["/bin/busybox".to_owned()];
        assert!(validate_bootstrap_contract(&paths).is_err());
    }

    #[test]
    fn rejects_traversal_in_bootstrap_contract() {
        let paths = vec![
            "/bin/busybox".to_owned(),
            "/sbin/luna-system-runtime".to_owned(),
            "/sbin/init".to_owned(),
            "/etc/../shadow".to_owned(),
        ];
        assert!(validate_bootstrap_contract(&paths).is_err());
    }
}
