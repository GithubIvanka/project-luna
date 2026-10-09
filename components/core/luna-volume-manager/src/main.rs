use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use luna_device_manager::DiscoveredBlockVolume;
use luna_volume_manager::{VolumeEvent, VolumeId, VolumeManager, VolumeRegistryError, VolumeRole};

const READY_PATH: &str = "/run/luna/volume-manager.ready";
const STATUS_PATH: &str = "/run/luna/volume-manager.status";
const DEVICE_READY_PATH: &str = "/run/luna/device-manager.ready";
const INVENTORY_PATH: &str = "/run/luna/device-manager.volumes";

fn data_root() -> PathBuf {
    std::env::var_os("LUNA_DATA_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/data"))
}

fn preferences_path() -> PathBuf {
    data_root().join("system/volumes/volume-registry.toml")
}

fn wait_for_device_manager() -> Result<(), String> {
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        if Path::new(DEVICE_READY_PATH).is_file() && Path::new(INVENTORY_PATH).is_file() {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err("timed out waiting for luna-device-manager inventory".into());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn load_preferences(manager: &mut VolumeManager, path: &Path) -> Result<(), String> {
    match manager.load_volume_preferences(path) {
        Ok(()) => Ok(()),
        Err(VolumeRegistryError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(())
        }
        Err(error) => Err(format!("load volume preferences: {error}")),
    }
}

fn decode_hex(value: &str) -> Result<String, String> {
    if value.len() % 2 != 0 {
        return Err("invalid odd-length hex field in device inventory".into());
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let high = (pair[0] as char)
            .to_digit(16)
            .ok_or("invalid hex field in device inventory")?;
        let low = (pair[1] as char)
            .to_digit(16)
            .ok_or("invalid hex field in device inventory")?;
        bytes.push(((high << 4) | low) as u8);
    }
    String::from_utf8(bytes).map_err(|_| "device inventory field is not UTF-8".into())
}

fn decode_optional(value: &str) -> Result<Option<String>, String> {
    if value == "-" {
        Ok(None)
    } else {
        decode_hex(value).map(Some)
    }
}

fn parse_role(value: &str) -> Result<VolumeRole, String> {
    match value {
        "Efi" => Ok(VolumeRole::Efi),
        "System" => Ok(VolumeRole::System),
        "LunaData" => Ok(VolumeRole::LunaData),
        "Swap" => Ok(VolumeRole::Swap),
        "External" => Ok(VolumeRole::External),
        "RecoveryVirtual" => Ok(VolumeRole::RecoveryVirtual),
        _ => Err(format!("unknown VolumeRole in device inventory: {value}")),
    }
}

/// Parse the atomically published Device Manager snapshot. Delimited fields
/// containing user-controlled strings are hex-encoded by the producer.
fn parse_inventory(contents: &str) -> Result<Vec<DiscoveredBlockVolume>, String> {
    let mut lines = contents.lines();
    if lines.next() != Some("format=1") {
        return Err("unsupported or missing device inventory format".into());
    }

    let mut volumes = Vec::new();
    for (index, line) in lines.enumerate() {
        if line.is_empty() {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 7 {
            return Err(format!(
                "device inventory line {} has {} fields",
                index + 2,
                fields.len()
            ));
        }
        let id = decode_hex(fields[0])?;
        if id.is_empty() {
            return Err(format!(
                "device inventory line {} has an empty VolumeId",
                index + 2
            ));
        }
        let label = decode_optional(fields[1])?;
        let native_name = decode_hex(fields[2])?;
        let filesystem = decode_optional(fields[3])?;
        let role = parse_role(fields[4])?;
        let backend_node = decode_hex(fields[5])?;
        let removable = match fields[6] {
            "0" => false,
            "1" => true,
            _ => {
                return Err(format!(
                    "device inventory line {} has invalid removable flag",
                    index + 2
                ));
            }
        };
        volumes.push(DiscoveredBlockVolume::new(
            VolumeId::new(id),
            label,
            native_name,
            filesystem,
            role,
            backend_node,
            removable,
        ));
    }
    Ok(volumes)
}

fn save_preferences(manager: &VolumeManager, path: &Path) {
    if let Err(error) = manager.save_volume_preferences(path) {
        eprintln!("luna-volume-manager: cannot persist volume preferences: {error}");
    }
}

fn render_status(manager: &VolumeManager) -> String {
    let volumes = manager.volumes().collect::<Vec<_>>();
    let available_count = volumes
        .iter()
        .filter(|volume| {
            matches!(
                volume.state(),
                luna_volume_manager::VolumeState::Available
                    | luna_volume_manager::VolumeState::Mounted
            )
        })
        .count();
    let mut text = format!(
        "volume_count={}\navailable_count={}\nauto_attach={}\n",
        volumes.len(),
        available_count,
        manager.auto_attach_policy().is_enabled()
    );
    for volume in volumes {
        text.push_str(&format!(
            "volume_id={}\nvolume_label={}\nvolume_native_name={}\nvolume_filesystem={}\nvolume_role={:?}\nvolume_state={:?}\nvolume_removable={}\nvolume_drive_letter={}\n",
            volume.id().as_str(),
            volume.label().unwrap_or_default(),
            volume.native_name().unwrap_or_default(),
            volume.filesystem().unwrap_or_default(),
            volume.role(),
            volume.state(),
            volume.is_removable(),
            manager.volume_registry().current_letter(volume.id()).map(|letter| letter.to_string()).unwrap_or_default(),
        ));
    }
    text
}

fn log_events(events: &[VolumeEvent]) {
    for event in events {
        eprintln!(
            "luna-volume-manager: {:?} {:?} state={:?} alias={:?}",
            event,
            event.volume().id(),
            event.volume().state(),
            event.drive_letter()
        );
    }
}

fn reconcile(manager: &mut VolumeManager, snapshot: &str, prefs: &Path) -> Result<(), String> {
    let discovered = parse_inventory(snapshot)?;
    let events = manager
        .reconcile_discovered_volumes(discovered)
        .map_err(|error| format!("reconcile volume inventory: {error}"))?;
    log_events(&events);
    save_preferences(manager, prefs);
    fs::write(STATUS_PATH, render_status(manager))
        .map_err(|error| format!("write volume-manager status: {error}"))?;
    Ok(())
}

fn run() -> Result<(), String> {
    fs::create_dir_all("/run/luna").map_err(|e| format!("create /run/luna: {e}"))?;
    let _ = fs::remove_file(READY_PATH);
    let _ = fs::remove_file(STATUS_PATH);

    let mut manager = VolumeManager::new();
    let prefs = preferences_path();
    wait_for_device_manager()?;
    load_preferences(&mut manager, &prefs)?;

    let mut last_snapshot = fs::read_to_string(INVENTORY_PATH)
        .map_err(|error| format!("read device inventory: {error}"))?;
    reconcile(&mut manager, &last_snapshot, &prefs)?;
    fs::write(READY_PATH, b"ready\n")
        .map_err(|error| format!("write volume-manager readiness: {error}"))?;
    eprintln!(
        "luna-volume-manager: ready; auto-attach policy enabled={}",
        manager.auto_attach_policy().is_enabled()
    );

    loop {
        thread::sleep(Duration::from_millis(150));
        let snapshot = fs::read_to_string(INVENTORY_PATH)
            .map_err(|error| format!("read device inventory: {error}"))?;
        if snapshot != last_snapshot {
            last_snapshot = snapshot;
            reconcile(&mut manager, &last_snapshot, &prefs)?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_inventory, render_status};
    use luna_common::{VolumeId, VolumeRole};
    use luna_device_manager::DiscoveredBlockVolume;
    use luna_volume_manager::{VolumeManager, VolumeState};

    #[test]
    fn parses_device_manager_inventory_into_volume_identity() {
        let snapshot = concat!(
            "format=1\n",
            "757362\t555342206469736b\t7573622d6469736b\t6578666174\tExternal\t2f6465762f73646231\t1\n"
        );
        let volumes = parse_inventory(snapshot).unwrap();
        assert_eq!(volumes.len(), 1);
        assert_eq!(volumes[0].id(), &VolumeId::from("usb"));
        assert_eq!(volumes[0].label(), Some("USB disk"));
        assert_eq!(volumes[0].native_name(), "usb-disk");
        assert_eq!(volumes[0].filesystem(), Some("exfat"));
        assert_eq!(volumes[0].role(), VolumeRole::External);
        assert_eq!(volumes[0].backend_node(), "/dev/sdb1");
        assert!(volumes[0].is_removable());
    }

    #[test]
    fn status_reports_attached_and_available_volumes_for_boot_gate() {
        let mut manager = VolumeManager::new();
        manager
            .reconcile_discovered_volumes(vec![DiscoveredBlockVolume::new(
                VolumeId::from("usb"),
                Some("USB".into()),
                "USB drive".into(),
                Some("exfat".into()),
                VolumeRole::External,
                "/dev/sdb1".into(),
                true,
            )])
            .unwrap();

        assert_eq!(
            manager.volume(&VolumeId::from("usb")).unwrap().state(),
            VolumeState::Available
        );
        let status = render_status(&manager);
        assert!(status.contains("volume_count=1\n"));
        assert!(status.contains("available_count=1\n"));
        assert!(status.contains("auto_attach=true\n"));
    }

    #[test]
    fn rejects_malformed_inventory_before_reconciling() {
        assert!(parse_inventory("format=2\n").is_err());
        assert!(
            parse_inventory("format=1\n757\t-\t\t-\tExternal\t2f6465762f73646231\t1\n").is_err()
        );
        assert!(
            parse_inventory("format=1\n757362\t-\t757362\t-\tUnknown\t2f6465762f73646231\t1\n")
                .is_err()
        );
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("luna-volume-manager: {error}");
        std::process::exit(1);
    }
}
