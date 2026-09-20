//! Project Luna direct PID1.
//!
//! `luna-init` is executed directly by the Linux kernel as PID 1. The kernel
//! supplies LunaBootHandoffV1 through file descriptor 3. Direct PID 1
//! execution owns bootstrap and lifecycle without a second init process.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::fd::FromRawFd;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

const HANDOFF_MAGIC: &[u8; 8] = b"LUNAHD01";
const HANDOFF_MAJOR: u16 = 1;
const HANDOFF_HEADER_SIZE: usize = 84;
const HANDOFF_ALIGNED_HEADER_SIZE: usize = 88;
const HANDOFF_MAX_SIZE: usize = 64 * 1024;
const RECORD_HEADER_SIZE: usize = 8;
const RECORD_ALIGN: usize = 8;
const RECORD_SYSTEM_PARTITION: u16 = 1;
const RECORD_DATA_PARTITION: u16 = 2;
const RECORD_SYSTEM_IMAGE: u16 = 3;
const RECORD_KERNEL_IDENTITY: u16 = 4;
const RECORD_LUNA_INIT_IMAGE: u16 = 5;
const RECORD_BOOT_MODE: u16 = 6;
const RECORD_BOOT_STATE: u16 = 7;
const RECORD_RECOVERY_DATA_IMAGE: u16 = 8;
const CHECKSUM_OFFSET: usize = 52;
const CHECKSUM_SIZE: usize = 32;

fn main() -> ! {
    match run() {
        Ok(()) => reap_forever(),
        Err(error) => panic!("luna-init: {error}"),
    }
}

fn run() -> Result<(), String> {
    let handoff = read_handoff_fd3()?;
    let context = BootContext::parse(&handoff)?;

    write_stderr("Luna: luna-init is running as PID 1\\n");
    write_stderr(&format!(
        "Luna: target image={} mode={}\\n",
        context.image_path, context.boot_mode,
    ));

    prepare_boot_devices()?;
    prepare_drm_device_access()?;
    let mounted = SystemMounts::mount(&context)?;
    mounted.prepare_logical_root(&context)?;
    mounted.enter_logical_root()?;

    write_stderr("Luna: logical root ready; starting luna-system-runtime\\n");
    let session_user = if context.boot_mode == 2 {
        "recovery"
    } else {
        "luna"
    };
    let mut runtime = std::process::Command::new("/sbin/luna-system-runtime")
        .env("LUNA_SYSTEM_RUNTIME_ROOT", "1")
        .env("LUNA_SESSION_USER", session_user)
        .env("LUNA_BOOT_MODE", context.boot_mode.to_string())
        .spawn()
        .map_err(|error| format!("start luna-system-runtime: {error}"))?;

    let status = runtime
        .wait()
        .map_err(|error| format!("wait for luna-system-runtime: {error}"))?;
    write_stderr(&format!(
        "Luna: luna-system-runtime exited with status {status}\\n"
    ));

    Err(format!("system runtime terminated: {status}"))
}

#[derive(Debug, Clone)]
struct BootContext {
    system_guid: [u8; 16],
    data_guid: Option<[u8; 16]>,
    image_path: String,
    image_digest: [u8; 32],
    recovery_data_path: Option<String>,
    recovery_data_digest: Option<[u8; 32]>,
    boot_mode: u8,
}

impl BootContext {
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        let mut checked = bytes.to_vec();
        validate_handoff(&mut checked)?;
        let records_offset = read_u64(bytes, 36)? as usize;
        let records_size = read_u64(bytes, 44)? as usize;
        let records_end = records_offset
            .checked_add(records_size)
            .ok_or_else(|| "handoff record range overflow".to_owned())?;
        let mut offset = records_offset;
        let mut system_guid = None;
        let mut data_guid = None;
        let mut image_path = None;
        let mut image_digest = None;
        let mut recovery_data_path = None;
        let mut recovery_data_digest = None;
        let mut boot_mode = None;

        while offset < records_end {
            let record_type = read_u16(bytes, offset)?;
            let record_size = read_u32(bytes, offset + 4)? as usize;
            let payload = offset
                .checked_add(RECORD_HEADER_SIZE)
                .ok_or_else(|| "handoff record offset overflow".to_owned())?;
            if !range_ok(payload, record_size, records_end) {
                return Err(format!("record {record_type} exceeds handoff"));
            }

            match record_type {
                RECORD_SYSTEM_PARTITION => {
                    if record_size < 36 {
                        return Err("system partition record is too small".to_owned());
                    }
                    system_guid = Some(read_guid(bytes, payload + 16)?);
                }
                RECORD_DATA_PARTITION => {
                    if record_size < 36 {
                        return Err("data partition record is too small".to_owned());
                    }
                    let flags = read_u16(bytes, offset + 2)?;
                    if flags & 1 == 0 {
                        data_guid = Some(read_guid(bytes, payload + 16)?);
                    }
                }
                RECORD_SYSTEM_IMAGE => {
                    if record_size < 72 {
                        return Err("system image record is too small".to_owned());
                    }
                    let family_len = read_u16(bytes, payload)? as usize;
                    let version_len = read_u16(bytes, payload + 2)? as usize;
                    let filename_len = read_u16(bytes, payload + 4)? as usize;
                    let strings = payload
                        .checked_add(72)
                        .ok_or_else(|| "system image strings overflow".to_owned())?;
                    let strings_len = family_len
                        .checked_add(version_len)
                        .and_then(|value| value.checked_add(filename_len))
                        .ok_or_else(|| "system image strings length overflow".to_owned())?;
                    if strings_len > record_size - 72 {
                        return Err("system image record strings exceed payload".to_owned());
                    }
                    let filename_start = strings
                        .checked_add(family_len)
                        .and_then(|value| value.checked_add(version_len))
                        .ok_or_else(|| "system image filename offset overflow".to_owned())?;
                    let filename = bytes
                        .get(filename_start..filename_start + filename_len)
                        .ok_or_else(|| "system image filename is outside handoff".to_owned())?;
                    let filename = std::str::from_utf8(filename)
                        .map_err(|_| "system image filename is not UTF-8".to_owned())?;
                    validate_image_path(filename)?;
                    image_path = Some(if filename.starts_with("recovery/") {
                        format!("/{filename}")
                    } else {
                        format!("/images/{filename}")
                    });
                    image_digest = Some(
                        bytes[payload + 40..payload + 72]
                            .try_into()
                            .map_err(|_| "invalid image digest".to_owned())?,
                    );
                }
                RECORD_BOOT_MODE => {
                    if record_size != 1 {
                        return Err("invalid boot mode record".to_owned());
                    }
                    boot_mode = Some(bytes[payload]);
                }
                RECORD_RECOVERY_DATA_IMAGE => {
                    if record_size < 40 {
                        return Err("recovery DATA record is too small".to_owned());
                    }
                    let version_len = read_u16(bytes, payload)? as usize;
                    let filename_len = read_u16(bytes, payload + 2)? as usize;
                    let strings = payload
                        .checked_add(40)
                        .ok_or_else(|| "recovery DATA strings overflow".to_owned())?;
                    let strings_len = version_len
                        .checked_add(filename_len)
                        .ok_or_else(|| "recovery DATA strings length overflow".to_owned())?;
                    if strings_len > record_size - 40 {
                        return Err("recovery DATA strings exceed payload".to_owned());
                    }
                    let filename_start = strings
                        .checked_add(version_len)
                        .ok_or_else(|| "recovery DATA filename offset overflow".to_owned())?;
                    let filename = bytes
                        .get(filename_start..filename_start + filename_len)
                        .ok_or_else(|| "recovery DATA filename is outside handoff".to_owned())?;
                    let filename = std::str::from_utf8(filename)
                        .map_err(|_| "recovery DATA filename is not UTF-8".to_owned())?;
                    validate_image_path(filename)?;
                    recovery_data_path = Some(format!("/recovery/{filename}"));
                    recovery_data_digest = Some(
                        bytes[payload + 8..payload + 40]
                            .try_into()
                            .map_err(|_| "invalid recovery DATA digest".to_owned())?,
                    );
                }
                _ => {}
            }

            let next = payload
                .checked_add(record_size)
                .and_then(|value| value.checked_add(RECORD_ALIGN - 1))
                .ok_or_else(|| "handoff record alignment overflow".to_owned())?
                & !(RECORD_ALIGN - 1);
            if next <= offset || next > records_end {
                return Err("invalid handoff record alignment".to_owned());
            }
            offset = next;
        }

        let boot_mode = boot_mode.ok_or_else(|| "handoff lacks BOOT_MODE".to_owned())?;
        if boot_mode == 2 && (recovery_data_path.is_none() || recovery_data_digest.is_none()) {
            return Err("Recovery boot handoff lacks Recovery DATA Image".to_owned());
        }
        Ok(Self {
            system_guid: system_guid.ok_or_else(|| "handoff lacks SYSTEM_PARTITION".to_owned())?,
            data_guid,
            image_path: image_path.ok_or_else(|| "handoff lacks SYSTEM_IMAGE".to_owned())?,
            image_digest: image_digest
                .ok_or_else(|| "handoff lacks System Image digest".to_owned())?,
            recovery_data_path,
            recovery_data_digest,
            boot_mode,
        })
    }
}

#[derive(Debug)]
struct SystemMounts {
    root: std::path::PathBuf,

    data: std::path::PathBuf,
    image: std::path::PathBuf,
    _loop_device: std::os::fd::OwnedFd,
    _recovery_data_loop: Option<std::os::fd::OwnedFd>,
}

impl SystemMounts {
    fn mount(context: &BootContext) -> Result<Self, String> {
        let root = std::path::PathBuf::from("/luna-root");
        let system = root.join(".luna/system");
        let data = root.join(".luna/data");
        let recovery_data_image = root.join(".luna/recovery-data");
        let image = root.join(".luna/image");
        if root.exists() {
            if std::fs::read_dir(&root)
                .map_err(|error| format!("inspect stale logical root: {error}"))?
                .next()
                .is_some()
            {
                return Err("/luna-root is not empty".to_owned());
            }
        } else {
            std::fs::create_dir_all(&root)
                .map_err(|error| format!("create logical root: {error}"))?;
        }
        mount_fs(
            "tmpfs",
            "tmpfs",
            &root,
            libc::MS_NOSUID | libc::MS_NODEV,
            Some("mode=0755"),
        )?;
        std::fs::create_dir_all(&system)
            .map_err(|error| format!("create system mountpoint: {error}"))?;
        std::fs::create_dir_all(&image)
            .map_err(|error| format!("create image mountpoint: {error}"))?;
        std::fs::create_dir_all(&data)
            .map_err(|error| format!("create data mountpoint: {error}"))?;

        let system_device = find_partition_device(context.system_guid)?;
        mount_fs(
            "ext4",
            &system_device,
            &system,
            libc::MS_RDONLY | libc::MS_NOSUID,
            None,
        )?;

        let image_source = root
            .join(".luna/system")
            .join(context.image_path.trim_start_matches('/'));
        if !image_source.is_file() {
            return Err(format!(
                "selected System Image is missing: {}",
                image_source.display()
            ));
        }
        let digest = hash_file(&image_source)?;
        if digest != context.image_digest {
            return Err(format!(
                "System Image digest mismatch: {}",
                image_source.display()
            ));
        }
        let (loop_device, loop_path) = attach_loop(&image_source)?;
        mount_fs(
            "squashfs",
            &loop_path,
            &image,
            libc::MS_RDONLY | libc::MS_NOSUID | libc::MS_NODEV,
            None,
        )?;

        let mut recovery_data_loop = None;
        if context.boot_mode == 2 {
            let source = context
                .recovery_data_path
                .as_deref()
                .ok_or_else(|| "Recovery boot lacks Recovery DATA Image".to_owned())?;
            let digest = context
                .recovery_data_digest
                .ok_or_else(|| "Recovery boot lacks Recovery DATA digest".to_owned())?;
            let source = system.join(source.trim_start_matches('/'));
            if !source.is_file() {
                return Err(format!(
                    "Recovery DATA Image is missing: {}",
                    source.display()
                ));
            }
            if hash_file(&source)? != digest {
                return Err("Recovery DATA Image digest mismatch".to_owned());
            }
            std::fs::create_dir_all(&recovery_data_image)
                .map_err(|error| format!("create Recovery DATA source mountpoint: {error}"))?;
            let (recovery_loop, loop_path) = attach_loop(&source)?;
            mount_fs(
                "squashfs",
                &loop_path,
                &recovery_data_image,
                libc::MS_RDONLY | libc::MS_NOSUID | libc::MS_NODEV,
                None,
            )?;
            mount_fs(
                "tmpfs",
                "tmpfs",
                &data,
                libc::MS_NOSUID | libc::MS_NODEV,
                Some("mode=0755"),
            )?;
            copy_tree(&recovery_data_image, &data)?;
            recovery_data_loop = Some(recovery_loop);
        } else if let Some(guid) = context.data_guid {
            let device = find_partition_device(guid)?;
            mount_fs(
                "btrfs",
                &device,
                &data,
                libc::MS_NOSUID | libc::MS_NODEV,
                None,
            )?;
        } else {
            return Err("normal boot reached luna-init without physical LUNA-DATA".to_owned());
        }

        Ok(Self {
            root,
            data,
            image,
            _loop_device: loop_device,
            _recovery_data_loop: recovery_data_loop,
        })
    }

    fn prepare_logical_root(&self, context: &BootContext) -> Result<(), String> {
        let root = &self.root;
        let runtime_source = self
            .image
            .join("apps/luna-system-runtime/luna-system-runtime");
        if !runtime_source.is_file() {
            return Err("System Image is missing bootstrap runtime".to_owned());
        }
        std::fs::create_dir_all(root.join("sbin"))
            .map_err(|error| format!("create /sbin: {error}"))?;
        let runtime_target = root.join("sbin/luna-system-runtime");
        std::fs::copy(&runtime_source, &runtime_target)
            .map_err(|error| format!("materialize luna-system-runtime: {error}"))?;
        std::fs::set_permissions(&runtime_target, std::fs::Permissions::from_mode(0o755))
            .map_err(|error| format!("set luna-system-runtime permissions: {error}"))?;
        std::fs::create_dir_all(root.join("usr/bin"))
            .map_err(|error| format!("create /usr/bin: {error}"))?;
        std::fs::create_dir_all(root.join("usr/lib"))
            .map_err(|error| format!("create /usr/lib: {error}"))?;
        std::fs::create_dir_all(root.join("usr/share"))
            .map_err(|error| format!("create /usr/share: {error}"))?;
        std::fs::create_dir_all(root.join("lib64"))
            .map_err(|error| format!("create /lib64: {error}"))?;
        std::fs::create_dir_all(root.join("lib"))
            .map_err(|error| format!("create /lib: {error}"))?;
        std::fs::create_dir_all(root.join("lib/luna"))
            .map_err(|error| format!("create /lib/luna: {error}"))?;
        bind_mount(&self.image.join("libs"), &root.join("lib/luna"))?;
        let shadow_gid = bootstrap_group_id(&self.image.join("config/group"), "shadow")?;

        // Materialize application executables into the logical /usr/bin view.
        // Their physical System Image location remains apps/<name>/<file>.
        let apps = self.image.join("apps");
        for entry in
            std::fs::read_dir(&apps).map_err(|error| format!("read System Image apps: {error}"))?
        {
            let entry = entry.map_err(|error| format!("read app entry: {error}"))?;
            let name = entry.file_name();
            let skip_app_executable = name == "luna-system-runtime";
            let app = entry.path();
            if !app.is_dir() {
                return Err(format!("invalid application resource: {}", app.display()));
            }
            for file in std::fs::read_dir(&app)
                .map_err(|error| format!("read application resource: {error}"))?
            {
                let file = file.map_err(|error| format!("read application file: {error}"))?;
                let source = file.path();
                let metadata = std::fs::symlink_metadata(&source)
                    .map_err(|error| format!("inspect application file: {error}"))?;
                if metadata.is_dir() {
                    if file.file_name() != "resources" {
                        return Err(format!(
                            "unsupported nested application directory: {}",
                            source.display()
                        ));
                    }
                    for resource in std::fs::read_dir(&source)
                        .map_err(|error| format!("read application resources: {error}"))?
                    {
                        let resource = resource
                            .map_err(|error| format!("read application resource entry: {error}"))?;
                        let resource_source = resource.path();
                        let resource_name = resource.file_name();
                        let resource_target = root.join("usr/share").join(&resource_name);
                        let resource_meta = std::fs::symlink_metadata(&resource_source)
                            .map_err(|error| format!("inspect application resource: {error}"))?;
                        if resource_target.exists() || resource_target.is_symlink() {
                            if resource_target.is_dir() {
                                return Err(format!(
                                    "application resource target already exists: {}",
                                    resource_target.display()
                                ));
                            }
                            std::fs::remove_file(&resource_target).map_err(|error| {
                                format!("replace application resource target: {error}")
                            })?;
                        }
                        if resource_meta.is_dir() {
                            std::fs::create_dir_all(&resource_target).map_err(|error| {
                                format!("create application resource target: {error}")
                            })?;
                            bind_mount(&resource_source, &resource_target)?;
                        } else {
                            std::fs::create_dir_all(
                                resource_target.parent().expect("/usr/share has a parent"),
                            )
                            .map_err(|error| {
                                format!("create application resource parent: {error}")
                            })?;
                            std::fs::copy(&resource_source, &resource_target).map_err(|error| {
                                format!("materialize application resource: {error}")
                            })?;
                        }
                    }
                    continue;
                }
                if skip_app_executable {
                    continue;
                }
                let target = root.join("usr/bin").join(file.file_name());
                if target.exists() || target.is_symlink() {
                    std::fs::remove_file(&target)
                        .map_err(|error| format!("replace application file: {error}"))?;
                }
                if metadata.file_type().is_symlink() {
                    let link = std::fs::read_link(&source)
                        .map_err(|error| format!("read application link: {error}"))?;
                    std::os::unix::fs::symlink(link, &target)
                        .map_err(|error| format!("materialize application link: {error}"))?;
                } else {
                    std::fs::copy(&source, &target)
                        .map_err(|error| format!("materialize application: {error}"))?;
                    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))
                        .map_err(|error| format!("set application permissions: {error}"))?;
                }
            }
        }

        // DATA owns the full compatibility/runtime provider closure. Native
        // Luna core binaries remain in the System Image, while DATA libraries
        // and applications are exposed through the logical runtime namespace.
        Self::materialize_data_apps(&self.data, root, shadow_gid)?;
        bind_mount(&self.data.join("system/libs"), &root.join("usr/lib"))?;
        let compat_lib = root.join("lib/x86_64-linux-gnu");
        if compat_lib.is_symlink() || compat_lib.exists() {
            std::fs::remove_file(&compat_lib)
                .map_err(|error| format!("replace /lib/x86_64-linux-gnu: {error}"))?;
        }
        std::os::unix::fs::symlink("../usr/lib/x86_64-linux-gnu", &compat_lib)
            .map_err(|error| format!("create /lib/x86_64-linux-gnu compatibility link: {error}"))?;
        for resource_class in [
            "fonts",
            "icons",
            "themes",
            "cursors",
            "sounds",
            "locales",
            "translations",
        ] {
            let source = self.data.join("system/resources").join(resource_class);
            if !source.is_dir() {
                return Err(format!(
                    "LUNA-DATA is missing resource class: {resource_class}"
                ));
            }
            let target = root.join("usr/share").join(resource_class);
            std::fs::create_dir_all(&target)
                .map_err(|error| format!("create /usr/share/{resource_class}: {error}"))?;
            bind_mount(&source, &target)?;
        }
        // Preserve the source hierarchy of both providers in logical runtime
        // namespaces. System Image/config is the immutable default; physical
        // or VirtualData/system/config remains the selected mutable provider.
        copy_tree(&self.image.join("config"), &root.join("config"))?;
        copy_tree(&self.image.join("config"), &root.join("etc"))?;
        copy_tree(&self.data.join("system/config"), &root.join("etc"))?;
        let nsswitch = self.image.join("config/nsswitch.conf");
        if nsswitch.is_file() {
            std::fs::copy(&nsswitch, root.join("etc/nsswitch.conf"))
                .map_err(|error| format!("restore Luna nsswitch.conf: {error}"))?;
        }
        let machine_id = root.join("etc/machine-id");
        if !machine_id.is_file()
            || std::fs::metadata(&machine_id)
                .map(|value| value.len())
                .unwrap_or(0)
                == 0
        {
            let mut value = String::with_capacity(33);
            for byte in context.system_guid {
                use std::fmt::Write as _;
                let _ = write!(&mut value, "{byte:02x}");
            }
            value.push('\n');
            std::fs::write(&machine_id, value)
                .map_err(|error| format!("materialize machine-id: {error}"))?;
            std::fs::set_permissions(&machine_id, std::fs::Permissions::from_mode(0o444))
                .map_err(|error| format!("set machine-id permissions: {error}"))?;
        }
        let loader = self
            .data
            .join("system/libs/loader/ld-linux-x86-64.so.2");
        if loader.is_file() {
            std::fs::copy(&loader, root.join("lib64/ld-linux-x86-64.so.2"))
                .map_err(|error| format!("materialize DATA ELF loader: {error}"))?;
        }
        if root.join("bin").exists() || root.join("bin").is_symlink() {
            std::fs::remove_file(root.join("bin"))
                .map_err(|error| format!("replace /bin: {error}"))?;
        }
        std::os::unix::fs::symlink("/usr/bin", root.join("bin"))
            .map_err(|error| format!("create /bin compatibility link: {error}"))?;

        std::fs::create_dir_all(root.join("etc"))
            .map_err(|error| format!("create /etc: {error}"))?;
        std::fs::create_dir_all(root.join("home"))
            .map_err(|error| format!("create /home: {error}"))?;
        std::fs::create_dir_all(root.join("run"))
            .map_err(|error| format!("create /run: {error}"))?;
        std::fs::create_dir_all(root.join("tmp"))
            .map_err(|error| format!("create /tmp: {error}"))?;
        std::fs::create_dir_all(root.join("dev"))
            .map_err(|error| format!("create /dev: {error}"))?;
        std::fs::create_dir_all(root.join("proc"))
            .map_err(|error| format!("create /proc: {error}"))?;
        std::fs::create_dir_all(root.join("sys"))
            .map_err(|error| format!("create /sys: {error}"))?;
        std::fs::create_dir_all(root.join("data"))
            .map_err(|error| format!("create /data: {error}"))?;

        mount_fs(
            "devtmpfs",
            "devtmpfs",
            &root.join("dev"),
            libc::MS_NOSUID | libc::MS_NOEXEC,
            Some("mode=0755"),
        )?;
        std::fs::create_dir_all(root.join("dev/pts"))
            .map_err(|error| format!("create /dev/pts: {error}"))?;
        std::fs::create_dir_all(root.join("dev/shm"))
            .map_err(|error| format!("create /dev/shm: {error}"))?;
        mount_fs(
            "tmpfs",
            "tmpfs",
            &root.join("dev/shm"),
            libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
            Some("mode=1777"),
        )?;
        mount_fs(
            "proc",
            "proc",
            &root.join("proc"),
            libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
            None,
        )?;
        mount_fs(
            "devpts",
            "devpts",
            &root.join("dev/pts"),
            libc::MS_NOSUID | libc::MS_NOEXEC,
            Some("mode=0620,ptmxmode=0666"),
        )?;
        // Keep logical-root sysfs writable while the transitional uevent
        // provider is active. The final Luna-native device manager will not
        // require udev trigger writes.
        mount_fs(
            "sysfs",
            "sysfs",
            &root.join("sys"),
            libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
            None,
        )?;
        mount_fs(
            "tmpfs",
            "tmpfs",
            &root.join("run"),
            libc::MS_NOSUID | libc::MS_NODEV,
            Some("mode=0755"),
        )?;
        mount_fs(
            "tmpfs",
            "tmpfs",
            &root.join("tmp"),
            libc::MS_NOSUID | libc::MS_NODEV,
            Some("mode=1777"),
        )?;

        bind_mount(&self.data, &root.join("data"))?;
        let username = if context.boot_mode == 2 {
            "recovery"
        } else {
            "luna"
        };
        std::fs::create_dir_all(self.data.join("users").join(username).join("home"))
            .map_err(|error| format!("create persistent {username} home: {error}"))?;
        std::fs::create_dir_all(self.data.join("users").join(username).join("data"))
            .map_err(|error| format!("create persistent {username} data: {error}"))?;
        std::fs::create_dir_all(self.data.join("users").join(username).join("config"))
            .map_err(|error| format!("create persistent {username} config: {error}"))?;
        std::fs::create_dir_all(self.data.join("system/state/auth"))
            .map_err(|error| format!("create persistent auth state: {error}"))?;

        Self::materialize_data_apps(&self.data, root, shadow_gid)?;

        replace_with_symlink(
            &root.join(format!("home/{username}")),
            std::path::Path::new(&format!("/data/users/{username}/home")),
        )?;
        replace_with_symlink(
            &root.join("etc/passwd"),
            std::path::Path::new("/data/system/state/auth/passwd"),
        )?;
        replace_with_symlink(
            &root.join("etc/group"),
            std::path::Path::new("/data/system/state/auth/group"),
        )?;
        replace_with_symlink(
            &root.join("etc/shadow"),
            std::path::Path::new("/data/system/state/auth/shadow"),
        )?;
        for name in ["passwd", "group", "shadow"] {
            let target = self.data.join("system/state/auth").join(name);
            if !target.exists() {
                let source = self.image.join("config").join(name);
                std::fs::copy(&source, &target)
                    .map_err(|error| format!("initialize {name}: {error}"))?;
            }
            if name == "shadow" {
                chown_group(&target, shadow_gid)?;
            }
        }
        Ok(())
    }

    fn materialize_data_apps(
        data: &std::path::Path,
        root: &std::path::Path,
        _shadow_gid: u32,
    ) -> Result<(), String> {
        let apps = data.join("system/apps");
        if !apps.is_dir() {
            return Ok(());
        }

        for entry in
            std::fs::read_dir(&apps).map_err(|error| format!("read DATA apps: {error}"))?
        {
            let entry = entry.map_err(|error| format!("read DATA app entry: {error}"))?;
            let app = entry.path();
            if !app.is_dir() {
                continue;
            }

            for file in
                std::fs::read_dir(&app).map_err(|error| format!("read DATA app: {error}"))?
            {
                let file = file.map_err(|error| format!("read DATA app file: {error}"))?;
                let source = file.path();
                let name = file.file_name();
                let meta = std::fs::symlink_metadata(&source)
                    .map_err(|error| format!("inspect DATA app file: {error}"))?;

                if meta.is_dir() {
                    if name != "resources" {
                        return Err(format!(
                            "unsupported nested DATA application directory: {}",
                            source.display()
                        ));
                    }

                    for resource in std::fs::read_dir(&source)
                        .map_err(|error| format!("read DATA application resources: {error}"))?
                    {
                        let resource =
                            resource.map_err(|error| format!("read DATA resource entry: {error}"))?;
                        let resource_source = resource.path();
                        let resource_name = resource.file_name();
                        let resource_target = root.join("usr/share").join(&resource_name);
                        let resource_meta = std::fs::symlink_metadata(&resource_source)
                            .map_err(|error| format!("inspect DATA resource: {error}"))?;

                        if resource_target.exists() || resource_target.is_symlink() {
                            if resource_target.is_dir() && resource_meta.is_dir() {
                                // Overlay a provider-owned resource directory on the
                                // logical runtime namespace without copying it to tmpfs.
                            } else {
                                std::fs::remove_file(&resource_target).map_err(|error| {
                                    format!("replace DATA resource target: {error}")
                                })?;
                            }
                        }

                        if resource_meta.is_dir() {
                            std::fs::create_dir_all(&resource_target).map_err(|error| {
                                format!("create DATA resource target: {error}")
                            })?;
                            bind_mount(&resource_source, &resource_target)?;
                        } else {
                            if let Some(parent) = resource_target.parent() {
                                std::fs::create_dir_all(parent).map_err(|error| {
                                    format!("create DATA resource parent: {error}")
                                })?;
                            }
                            std::fs::copy(&resource_source, &resource_target).map_err(|error| {
                                format!("materialize DATA resource: {error}")
                            })?;
                        }
                    }
                    continue;
                }

                let target = root.join("usr/bin").join(&name);
                if target.exists() || target.is_symlink() {
                    std::fs::remove_file(&target)
                        .map_err(|error| format!("replace DATA application: {error}"))?;
                }

                if meta.file_type().is_symlink() {
                    let link = std::fs::read_link(&source)
                        .map_err(|error| format!("read DATA application link: {error}"))?;
                    std::os::unix::fs::symlink(link, &target)
                        .map_err(|error| format!("materialize DATA application link: {error}"))?;
                } else {
                    std::fs::copy(&source, &target)
                        .map_err(|error| format!("materialize DATA application: {error}"))?;
                    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))
                        .map_err(|error| format!("set DATA application permissions: {error}"))?;
                }

            }
        }

        Ok(())
    }

    fn enter_logical_root(&self) -> Result<(), String> {
        let path = std::ffi::CString::new(self.root.as_os_str().as_encoded_bytes())
            .map_err(|_| "logical root contains NUL".to_owned())?;
        if unsafe { libc::chroot(path.as_ptr()) } == -1 {
            return Err(format!(
                "chroot logical root: {}",
                std::io::Error::last_os_error()
            ));
        }
        std::env::set_current_dir("/").map_err(|error| format!("enter logical root: {error}"))?;
        Ok(())
    }
}

const LUNA_SEAT_GID: u32 = 997;

fn prepare_drm_device_access() -> Result<(), String> {
    let drm_dir = std::path::Path::new("/dev/dri");
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        let mut found = 0usize;
        if let Ok(entries) = std::fs::read_dir(drm_dir) {
            for entry in entries {
                let entry = entry.map_err(|error| format!("read DRM device entry: {error}"))?;
                let path = entry.path();
                let metadata = std::fs::metadata(&path)
                    .map_err(|error| format!("inspect DRM device {}: {error}", path.display()))?;
                let mode = metadata.mode();
                if (mode & libc::S_IFMT) != libc::S_IFCHR as u32 {
                    continue;
                }
                found += 1;
                let cpath = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())
                    .map_err(|_| format!("DRM device path contains NUL: {}", path.display()))?;
                if unsafe { libc::chown(cpath.as_ptr(), 0, LUNA_SEAT_GID) } == -1 {
                    return Err(format!(
                        "set DRM device group {}: {}",
                        path.display(),
                        std::io::Error::last_os_error()
                    ));
                }
                if unsafe { libc::chmod(cpath.as_ptr(), 0o660) } == -1 {
                    return Err(format!(
                        "set DRM device permissions {}: {}",
                        path.display(),
                        std::io::Error::last_os_error()
                    ));
                }
            }
        }
        if found > 0 {
            write_stderr(&format!(
                "Luna: prepared {found} DRM device(s) for seat group\\n"
            ));
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn prepare_boot_devices() -> Result<(), String> {
    std::fs::create_dir_all("/dev").map_err(|error| format!("create early /dev: {error}"))?;
    std::fs::create_dir_all("/sys").map_err(|error| format!("create early /sys: {error}"))?;
    mount_fs(
        "devtmpfs",
        "devtmpfs",
        std::path::Path::new("/dev"),
        libc::MS_NOSUID | libc::MS_NOEXEC,
        Some("mode=0755"),
    )?;
    std::fs::create_dir_all("/dev/shm")
        .map_err(|error| format!("create early /dev/shm: {error}"))?;
    mount_fs(
        "tmpfs",
        "tmpfs",
        std::path::Path::new("/dev/shm"),
        libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
        Some("mode=1777"),
    )?;
    // sysfs must remain writable for the transitional uevent backend:
    // udevadm trigger writes "add" to device uevent attributes. The future
    // Luna-native device manager will consume the kernel uevent stream directly
    // and will not depend on these trigger writes.
    mount_fs(
        "sysfs",
        "sysfs",
        std::path::Path::new("/sys"),
        libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
        None,
    )?;
    std::fs::create_dir_all("/proc").map_err(|error| format!("create early /proc: {error}"))?;
    mount_fs(
        "proc",
        "proc",
        std::path::Path::new("/proc"),
        libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
        None,
    )?;
    Ok(())
}

fn validate_image_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path.contains("//")
    {
        return Err("invalid System Image filename".to_owned());
    }
    if path
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err("System Image filename contains unsafe component".to_owned());
    }
    Ok(())
}

fn read_guid(bytes: &[u8], offset: usize) -> Result<[u8; 16], String> {
    bytes
        .get(offset..offset + 16)
        .ok_or_else(|| "GUID is outside handoff".to_owned())?
        .try_into()
        .map_err(|_| "invalid GUID record".to_owned())
}

fn hash_file(path: &std::path::Path) -> Result<[u8; 32], String> {
    let mut file =
        std::fs::File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let count = std::io::Read::read(&mut file, &mut buffer)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(*hasher.finalize().as_bytes())
}

fn replace_with_symlink(path: &std::path::Path, target: &std::path::Path) -> Result<(), String> {
    if path.is_symlink() || path.is_file() {
        std::fs::remove_file(path)
            .map_err(|error| format!("replace {}: {error}", path.display()))?;
    } else if path.is_dir() {
        std::fs::remove_dir_all(path)
            .map_err(|error| format!("replace directory {}: {error}", path.display()))?;
    } else if path.is_dir() {
        std::fs::remove_dir_all(path)
            .map_err(|error| format!("replace directory {}: {error}", path.display()))?;
    }
    std::os::unix::fs::symlink(target, path)
        .map_err(|error| format!("link {}: {error}", path.display()))
}

fn bootstrap_group_id(path: &std::path::Path, wanted: &str) -> Result<u32, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("read bootstrap group database: {error}"))?;
    for line in text.lines() {
        let mut fields = line.split(':');
        let name = fields.next().unwrap_or_default();
        let _password = fields.next().unwrap_or_default();
        let gid = fields.next().unwrap_or_default();
        if name == wanted {
            return gid
                .parse::<u32>()
                .map_err(|error| format!("invalid GID for bootstrap group {wanted}: {error}"));
        }
    }
    Err(format!("bootstrap group {wanted} is missing"))
}

fn chown_group(path: &std::path::Path, gid: u32) -> Result<(), String> {
    let path_c = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())
        .map_err(|_| format!("path contains NUL: {}", path.display()))?;
    if unsafe { libc::chown(path_c.as_ptr(), 0, gid) } == -1 {
        return Err(format!(
            "set group ownership on {}: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn mount_fs(
    filesystem: &str,
    source: impl AsRef<std::path::Path>,
    target: &std::path::Path,
    flags: libc::c_ulong,
    data: Option<&str>,
) -> Result<(), String> {
    let source = std::ffi::CString::new(source.as_ref().as_os_str().as_encoded_bytes())
        .map_err(|_| "mount source contains NUL".to_owned())?;
    let target = std::ffi::CString::new(target.as_os_str().as_encoded_bytes())
        .map_err(|_| "mount target contains NUL".to_owned())?;
    let filesystem = std::ffi::CString::new(filesystem)
        .map_err(|_| "mount filesystem contains NUL".to_owned())?;
    let data = data
        .map(|value| {
            std::ffi::CString::new(value).map_err(|_| "mount data contains NUL".to_owned())
        })
        .transpose()?;
    let status = unsafe {
        libc::mount(
            source.as_ptr(),
            target.as_ptr(),
            filesystem.as_ptr(),
            flags,
            data.as_ref()
                .map_or(std::ptr::null(), |value| value.as_ptr().cast()),
        )
    };
    if status == -1 {
        return Err(format!(
            "mount {} on {}: {}",
            filesystem.to_string_lossy(),
            target.to_string_lossy(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn bind_mount(source: &std::path::Path, target: &std::path::Path) -> Result<(), String> {
    let source_c = std::ffi::CString::new(source.as_os_str().as_encoded_bytes())
        .map_err(|_| "bind source contains NUL".to_owned())?;
    let target_c = std::ffi::CString::new(target.as_os_str().as_encoded_bytes())
        .map_err(|_| "bind target contains NUL".to_owned())?;
    let status = unsafe {
        libc::mount(
            source_c.as_ptr(),
            target_c.as_ptr(),
            std::ptr::null(),
            libc::MS_BIND | libc::MS_REC,
            std::ptr::null(),
        )
    };
    if status == -1 {
        return Err(format!(
            "bind mount {} on {}: {}",
            source.display(),
            target.display(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn copy_tree(source: &std::path::Path, target: &std::path::Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(source)
        .map_err(|error| format!("inspect {}: {error}", source.display()))?;
    if metadata.is_dir() {
        std::fs::create_dir_all(target)
            .map_err(|error| format!("create {}: {error}", target.display()))?;
        for entry in std::fs::read_dir(source)
            .map_err(|error| format!("read {}: {error}", source.display()))?
        {
            let entry = entry.map_err(|error| format!("read directory entry: {error}"))?;
            copy_tree(&entry.path(), &target.join(entry.file_name()))?;
        }
    } else if metadata.file_type().is_symlink() {
        let link = std::fs::read_link(source)
            .map_err(|error| format!("read link {}: {error}", source.display()))?;
        if target.exists() || target.is_symlink() {
            std::fs::remove_file(target)
                .map_err(|error| format!("replace {}: {error}", target.display()))?;
        }
        std::os::unix::fs::symlink(link, target)
            .map_err(|error| format!("create link {}: {error}", target.display()))?;
    } else if metadata.is_file() {
        std::fs::copy(source, target)
            .map_err(|error| format!("copy {}: {error}", source.display()))?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(target, std::fs::Permissions::from_mode(metadata.mode()))
            .map_err(|error| format!("set permissions {}: {error}", target.display()))?;
    }
    Ok(())
}

fn find_partition_device(guid: [u8; 16]) -> Result<std::path::PathBuf, String> {
    let entries =
        std::fs::read_dir("/sys/block").map_err(|error| format!("scan /sys/block: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("read /sys/block entry: {error}"))?;
        let disk_name = entry.file_name();
        let disk_name = disk_name.to_string_lossy();
        if disk_name.contains('/')
            || !std::path::Path::new("/dev")
                .join(disk_name.as_ref())
                .exists()
        {
            continue;
        }
        let device = std::path::Path::new("/dev").join(disk_name.as_ref());
        let mut disk = match std::fs::File::open(&device) {
            Ok(file) => file,
            Err(_) => continue,
        };
        let mut header = [0u8; 512];
        if std::io::Read::read_exact(&mut disk, &mut header).is_err() {
            continue;
        }
        if std::io::Seek::seek(&mut disk, std::io::SeekFrom::Start(512)).is_err()
            || std::io::Read::read_exact(&mut disk, &mut header).is_err()
        {
            continue;
        }
        if &header[0..8] != b"EFI PART" {
            continue;
        }
        let entries_lba = u64::from_le_bytes(header[72..80].try_into().unwrap());
        let count = u32::from_le_bytes(header[80..84].try_into().unwrap()) as usize;
        let size = u32::from_le_bytes(header[84..88].try_into().unwrap()) as usize;
        if size < 128 || size > 4096 || count == 0 || count > 4096 {
            continue;
        }
        let mut entry_bytes = vec![0u8; size];
        for index in 0..count {
            let offset = entries_lba
                .checked_mul(512)
                .and_then(|value| value.checked_add(index as u64 * size as u64))
                .ok_or_else(|| "GPT entry offset overflow".to_owned())?;
            if disk.seek(std::io::SeekFrom::Start(offset)).is_err()
                || disk.read_exact(&mut entry_bytes).is_err()
            {
                break;
            }
            if entry_bytes[16..32] != guid || entry_bytes[0..16].iter().all(|byte| *byte == 0) {
                continue;
            }
            let first_lba = u64::from_le_bytes(entry_bytes[32..40].try_into().unwrap());
            let last_lba = u64::from_le_bytes(entry_bytes[40..48].try_into().unwrap());
            if first_lba == 0 || last_lba < first_lba {
                return Err("GPT partition has invalid LBA range".to_owned());
            }
            let partition_number = index + 1;
            let name = if disk_name.starts_with("nvme") || disk_name.starts_with("mmcblk") {
                format!("{}p{}", disk_name, partition_number)
            } else {
                format!("{}{}", disk_name, partition_number)
            };
            let partition = std::path::Path::new("/dev").join(name);
            if partition.exists() {
                return Ok(partition);
            }
            return Err(format!(
                "GPT matched DATA/SYS GUID but device is missing: {}",
                partition.display()
            ));
        }
    }
    Err(format!(
        "partition with GUID {} not found",
        guid_string(guid)
    ))
}

fn guid_string(bytes: [u8; 16]) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
        u16::from_le_bytes(bytes[4..6].try_into().unwrap()),
        u16::from_le_bytes(bytes[6..8].try_into().unwrap()),
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15],
    )
}

fn attach_loop(
    image: &std::path::Path,
) -> Result<(std::os::fd::OwnedFd, std::path::PathBuf), String> {
    use std::os::fd::{FromRawFd, IntoRawFd};
    let control = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/loop-control")
        .map_err(|error| format!("open loop-control: {error}"))?;
    const LOOP_CTL_GET_FREE: libc::Ioctl = 0x4c82 as libc::Ioctl;
    const LOOP_SET_FD: libc::Ioctl = 0x4c00 as libc::Ioctl;
    let number =
        unsafe { libc::ioctl(std::os::fd::AsRawFd::as_raw_fd(&control), LOOP_CTL_GET_FREE) };
    if number < 0 {
        return Err(format!(
            "allocate loop device: {}",
            std::io::Error::last_os_error()
        ));
    }
    let path = format!("/dev/loop{number}");
    let loop_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|error| format!("open {path}: {error}"))?;
    let image_file = std::fs::File::open(image)
        .map_err(|error| format!("open System Image loop source: {error}"))?;
    let status = unsafe {
        libc::ioctl(
            std::os::fd::AsRawFd::as_raw_fd(&loop_file),
            LOOP_SET_FD,
            std::os::fd::AsRawFd::as_raw_fd(&image_file),
        )
    };
    if status == -1 {
        return Err(format!(
            "attach {} to {}: {}",
            image.display(),
            path,
            std::io::Error::last_os_error()
        ));
    }
    std::mem::forget(image_file);
    Ok((
        unsafe { std::os::fd::OwnedFd::from_raw_fd(loop_file.into_raw_fd()) },
        std::path::PathBuf::from(path),
    ))
}

fn read_handoff_fd3() -> Result<Vec<u8>, String> {
    // FD 3 is deliberately transferred without close-on-exec by the kernel
    // launcher. Ownership is taken here so it is closed before children are
    // ever started.
    let mut file = unsafe { File::from_raw_fd(3) };
    file.seek(SeekFrom::Start(0))
        .map_err(|e| format!("seek handoff fd 3: {e}"))?;

    let mut bytes = Vec::with_capacity(HANDOFF_MAX_SIZE.min(4096));
    file.read_to_end(&mut bytes)
        .map_err(|e| format!("read handoff fd 3: {e}"))?;

    if bytes.len() < HANDOFF_HEADER_SIZE {
        return Err(format!("handoff is too small: {} bytes", bytes.len()));
    }
    if bytes.len() > HANDOFF_MAX_SIZE {
        return Err(format!("handoff is too large: {} bytes", bytes.len()));
    }
    Ok(bytes)
}

fn validate_handoff(bytes: &mut [u8]) -> Result<(), String> {
    if &bytes[0..8] != HANDOFF_MAGIC {
        return Err("handoff magic mismatch".to_owned());
    }

    let major = read_u16(bytes, 8)?;
    let header_size = read_u32(bytes, 12)? as usize;
    let total_size = read_u32(bytes, 16)? as usize;
    let records_offset = read_u64(bytes, 36)? as usize;
    let records_size = read_u64(bytes, 44)? as usize;

    if major != HANDOFF_MAJOR {
        return Err(format!("unsupported handoff ABI major: {major}"));
    }
    if header_size != HANDOFF_HEADER_SIZE {
        return Err(format!("unexpected handoff header size: {header_size}"));
    }
    if total_size != bytes.len() {
        return Err(format!(
            "handoff total_size {} does not match fd size {}",
            total_size,
            bytes.len()
        ));
    }
    if records_offset != HANDOFF_ALIGNED_HEADER_SIZE
        || records_offset % RECORD_ALIGN != 0
        || !range_ok(records_offset, records_size, total_size)
    {
        return Err("invalid handoff record area".to_owned());
    }

    validate_checksum(bytes)?;

    let records_end = records_offset + records_size;
    let mut offset = records_offset;
    let mut seen = [false; 8];
    let mut init_size = 0u64;
    let mut recovery_seen = false;
    let mut boot_mode = 0u8;

    while offset < records_end {
        if records_end - offset < RECORD_HEADER_SIZE {
            return Err("truncated handoff record header".to_owned());
        }

        let record_type = read_u16(bytes, offset)?;
        let record_size = read_u32(bytes, offset + 4)? as usize;
        let payload = offset
            .checked_add(RECORD_HEADER_SIZE)
            .ok_or_else(|| "handoff record offset overflow".to_owned())?;
        if !range_ok(payload, record_size, records_end) {
            return Err(format!("record {record_type} exceeds handoff"));
        }

        match record_type {
            RECORD_SYSTEM_PARTITION => seen[0] = true,
            RECORD_DATA_PARTITION => seen[1] = true,
            RECORD_SYSTEM_IMAGE => seen[2] = true,
            RECORD_KERNEL_IDENTITY => seen[3] = true,
            RECORD_LUNA_INIT_IMAGE => {
                if seen[4] || record_size != 56 {
                    return Err("invalid Luna init image record".to_owned());
                }
                let address = read_u64(bytes, payload)?;
                init_size = read_u64(bytes, payload + 8)?;
                if address == 0 || init_size == 0 {
                    return Err("invalid Luna init image range".to_owned());
                }
                seen[4] = true;
            }
            RECORD_BOOT_MODE => {
                if record_size != 1 {
                    return Err("invalid boot mode record".to_owned());
                }
                boot_mode = bytes[payload];
                if boot_mode > 4 {
                    return Err("unsupported boot mode".to_owned());
                }
                seen[5] = true;
            }
            RECORD_BOOT_STATE => {
                if record_size != 24 {
                    return Err("invalid boot state record".to_owned());
                }
                seen[6] = true;
            }
            RECORD_RECOVERY_DATA_IMAGE => {
                if record_size < 40 || recovery_seen {
                    return Err("invalid Recovery DATA Image record".to_owned());
                }
                let version_len = read_u16(bytes, payload)? as usize;
                let filename_len = read_u16(bytes, payload + 2)? as usize;
                let strings = payload
                    .checked_add(40)
                    .ok_or_else(|| "Recovery DATA strings overflow".to_owned())?;
                let strings_len = version_len
                    .checked_add(filename_len)
                    .ok_or_else(|| "Recovery DATA strings length overflow".to_owned())?;
                if strings_len > record_size - 40 {
                    return Err("Recovery DATA strings exceed payload".to_owned());
                }
                let filename_start = strings
                    .checked_add(version_len)
                    .ok_or_else(|| "Recovery DATA filename offset overflow".to_owned())?;
                let filename = bytes
                    .get(filename_start..filename_start + filename_len)
                    .ok_or_else(|| "Recovery DATA filename is outside handoff".to_owned())?;
                let filename = std::str::from_utf8(filename)
                    .map_err(|_| "Recovery DATA filename is not UTF-8".to_owned())?;
                validate_image_path(filename)?;
                recovery_seen = true;
                seen[7] = true;
            }
            _ => {}
        }

        let next = payload
            .checked_add(record_size)
            .and_then(|value| value.checked_add(RECORD_ALIGN - 1))
            .ok_or_else(|| "handoff record alignment overflow".to_owned())?
            & !(RECORD_ALIGN - 1);
        if next <= offset || next > records_end {
            return Err("invalid handoff record alignment".to_owned());
        }
        offset = next;
    }

    if seen[..7].iter().any(|present| !present) {
        return Err("handoff is missing a required record".to_owned());
    }
    if boot_mode == 2 && !recovery_seen {
        return Err("Recovery boot is missing Recovery DATA Image record".to_owned());
    }
    if init_size == 0 {
        return Err("handoff has no luna-init image".to_owned());
    }

    Ok(())
}

fn validate_checksum(bytes: &mut [u8]) -> Result<(), String> {
    if CHECKSUM_OFFSET + CHECKSUM_SIZE > bytes.len() {
        return Err("handoff checksum field is out of range".to_owned());
    }

    let expected = bytes[CHECKSUM_OFFSET..CHECKSUM_OFFSET + CHECKSUM_SIZE].to_vec();
    bytes[CHECKSUM_OFFSET..CHECKSUM_OFFSET + CHECKSUM_SIZE].fill(0);
    let digest = blake3::hash(bytes);
    bytes[CHECKSUM_OFFSET..CHECKSUM_OFFSET + CHECKSUM_SIZE].copy_from_slice(&expected);

    if digest.as_bytes() != expected.as_slice() {
        return Err("handoff BLAKE3 checksum mismatch".to_owned());
    }
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let end = offset
        .checked_add(2)
        .ok_or_else(|| "u16 read overflow".to_owned())?;
    let slice = bytes
        .get(offset..end)
        .ok_or_else(|| "u16 read outside handoff".to_owned())?;
    Ok(u16::from_le_bytes(
        slice
            .try_into()
            .map_err(|_| "invalid u16 slice".to_owned())?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| "u32 read overflow".to_owned())?;
    let slice = bytes
        .get(offset..end)
        .ok_or_else(|| "u32 read outside handoff".to_owned())?;
    Ok(u32::from_le_bytes(
        slice
            .try_into()
            .map_err(|_| "invalid u32 slice".to_owned())?,
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let end = offset
        .checked_add(8)
        .ok_or_else(|| "u64 read overflow".to_owned())?;
    let slice = bytes
        .get(offset..end)
        .ok_or_else(|| "u64 read outside handoff".to_owned())?;
    Ok(u64::from_le_bytes(
        slice
            .try_into()
            .map_err(|_| "invalid u64 slice".to_owned())?,
    ))
}

fn range_ok(offset: usize, size: usize, total: usize) -> bool {
    offset <= total && size <= total - offset
}

fn write_stderr(message: &str) {
    use std::io::Write;
    let mut stderr = std::io::stderr();
    let _ = stderr.write_all(message.as_bytes());
    let _ = stderr.flush();
}

fn reap_forever() -> ! {
    loop {
        let mut status = 0;
        let pid = unsafe { libc::waitpid(-1, &mut status, 0) };
        if pid < 0 {
            let errno = std::io::Error::last_os_error().raw_os_error();
            if errno != Some(libc::ECHILD) && errno != Some(libc::EINTR) {
                write_stderr(&format!("Luna: waitpid failed: errno {:?}\n", errno));
            }
            if errno == Some(libc::ECHILD) {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}
