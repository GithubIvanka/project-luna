//! Kernel-backed block-volume discovery.
//!
//! Discovery starts from sysfs/kernel block devices. udev's database is used
//! only as optional metadata enrichment (filesystem type, filesystem label,
//! partition GUID/type/name). The public Volume model never exposes a Linux
//! pathname as its identity.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::{VolumeId, VolumeInfo, VolumeRole, VolumeState};

const EFI_SYSTEM_PARTITION_GUID: &str = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b";
const LINUX_SWAP_PARTITION_GUID: &str = "0657fd6d-a4ab-43c4-84e5-0933c84b4f4f";
const LINUX_FILESYSTEM_PARTITION_GUID: &str = "0fc63daf-8483-4772-8e79-3d69d8477de4";

#[derive(Clone, Debug)]
pub struct BlockVolumeDiscovery {
    sysfs_block_root: PathBuf,
    dev_disk_root: PathBuf,
    udev_data_root: PathBuf,
}

impl Default for BlockVolumeDiscovery {
    fn default() -> Self {
        Self {
            sysfs_block_root: PathBuf::from("/sys/class/block"),
            dev_disk_root: PathBuf::from("/dev/disk"),
            udev_data_root: PathBuf::from("/run/udev/data"),
        }
    }
}

impl BlockVolumeDiscovery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scan(&self) -> io::Result<Vec<VolumeInfo>> {
        let partuuids = self.read_symlink_map("by-partuuid")?;
        let fsuuids = self.read_symlink_map("by-uuid")?;
        let labels = self.read_symlink_map("by-label")?;
        let mut volumes = Vec::new();

        for entry in fs::read_dir(&self.sysfs_block_root)? {
            let entry = entry?;
            let sysfs_path = entry.path();
            if !sysfs_path.join("partition").exists() {
                continue;
            }

            let devname = entry.file_name().to_string_lossy().into_owned();
            let uevent = parse_kv_file(&sysfs_path.join("uevent"))?;
            let major = uevent.get("MAJOR").cloned().unwrap_or_default();
            let minor = uevent.get("MINOR").cloned().unwrap_or_default();

            let udev = if !major.is_empty() && !minor.is_empty() {
                parse_udev_database(&self.udev_data_root.join(format!("b{major}:{minor}")))?
            } else {
                BTreeMap::new()
            };

            let partuuid = udev
                .get("ID_PART_ENTRY_UUID")
                .cloned()
                .or_else(|| partuuids.get(&devname).cloned());
            let fs_uuid = udev
                .get("ID_FS_UUID")
                .cloned()
                .or_else(|| fsuuids.get(&devname).cloned());
            let fs_label = udev
                .get("ID_FS_LABEL")
                .cloned()
                .or_else(|| labels.get(&devname).cloned());
            let partname = udev
                .get("ID_PART_ENTRY_NAME")
                .cloned()
                .or_else(|| uevent.get("PARTNAME").cloned());
            let parttype = udev.get("ID_PART_ENTRY_TYPE").cloned();
            let filesystem = udev.get("ID_FS_TYPE").cloned();

            let id = if let Some(partuuid) = partuuid {
                VolumeId::from(format!("partition-guid:{partuuid}"))
            } else if let Some(fs_uuid) = fs_uuid {
                VolumeId::from(format!("filesystem-uuid:{fs_uuid}"))
            } else {
                VolumeId::from(format!("block-device:{devname}"))
            };

            let role = classify_role(
                fs_label.as_deref(),
                partname.as_deref(),
                parttype.as_deref(),
                filesystem.as_deref(),
            );
            let native_name = default_native_name(role, fs_label.as_deref(), &devname);

            volumes.push(VolumeInfo::discovered(
                id,
                fs_label,
                native_name,
                filesystem,
                role,
                VolumeState::Available,
                format!("/dev/{devname}"),
            ));
        }

        volumes.sort_by(|left, right| left.backend_node().cmp(right.backend_node()));
        Ok(volumes)
    }

    fn read_symlink_map(&self, directory: &str) -> io::Result<BTreeMap<String, String>> {
        let mut result = BTreeMap::new();
        let root = self.dev_disk_root.join(directory);
        if !root.is_dir() {
            return Ok(result);
        }

        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let target = fs::canonicalize(entry.path()).ok();
            if let Some(target) =
                target.and_then(|path| path.file_name().map(|name| name.to_owned()))
            {
                result.insert(target.to_string_lossy().into_owned(), name);
            }
        }
        Ok(result)
    }
}

fn parse_kv_file(path: &Path) -> io::Result<BTreeMap<String, String>> {
    parse_lines(fs::read_to_string(path)?.lines(), false)
}

fn parse_udev_database(path: &Path) -> io::Result<BTreeMap<String, String>> {
    if !path.is_file() {
        return Ok(BTreeMap::new());
    }
    parse_lines(fs::read_to_string(path)?.lines(), true)
}

fn parse_lines<'a, I>(lines: I, udev_format: bool) -> io::Result<BTreeMap<String, String>>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut values = BTreeMap::new();
    for line in lines {
        let line = line.trim();
        let line = if udev_format {
            line.strip_prefix("E:")
        } else {
            Some(line)
        };
        let Some(line) = line else {
            continue;
        };
        if let Some((key, value)) = line.split_once('=') {
            values.insert(key.to_owned(), decode_udev_value(value)?);
        }
    }
    Ok(values)
}

fn decode_udev_value(value: &str) -> io::Result<String> {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            output.push(ch);
            continue;
        }

        let marker = chars
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "truncated udev escape"))?;
        if marker != 'x' {
            output.push(marker);
            continue;
        }

        let mut hex = String::new();
        hex.push(
            chars.next().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "truncated udev escape")
            })?,
        );
        hex.push(
            chars.next().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "truncated udev escape")
            })?,
        );
        let byte = u8::from_str_radix(&hex, 16)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid udev escape"))?;
        output.push(byte as char);
    }

    Ok(output)
}

fn classify_role(
    label: Option<&str>,
    partname: Option<&str>,
    parttype: Option<&str>,
    filesystem: Option<&str>,
) -> VolumeRole {
    let label = label.unwrap_or_default();
    let partname = partname.unwrap_or_default();

    if eq_name(label, "LUNA-DATA") || eq_name(partname, "LUNA-DATA") {
        return VolumeRole::LunaData;
    }
    if eq_name(label, "LUNA-SYS") || eq_name(partname, "LUNA-SYS") {
        return VolumeRole::System;
    }
    if eq_name(label, "SWAP") || eq_name(partname, "SWAP") {
        return VolumeRole::Swap;
    }
    if parttype.is_some_and(|value| value.eq_ignore_ascii_case(EFI_SYSTEM_PARTITION_GUID)) {
        return VolumeRole::Efi;
    }
    if parttype.is_some_and(|value| value.eq_ignore_ascii_case(LINUX_SWAP_PARTITION_GUID))
        || filesystem.is_some_and(|value| value.eq_ignore_ascii_case("swap"))
    {
        return VolumeRole::Swap;
    }
    if parttype.is_some_and(|value| value.eq_ignore_ascii_case(LINUX_FILESYSTEM_PARTITION_GUID))
        && eq_name(label, "LUNA-SYS")
    {
        return VolumeRole::System;
    }

    VolumeRole::External
}

fn eq_name(left: &str, right: &str) -> bool {
    left.trim().eq_ignore_ascii_case(right)
}

fn default_native_name(role: VolumeRole, label: Option<&str>, devname: &str) -> String {
    if role == VolumeRole::LunaData {
        return "data".to_owned();
    }
    if role == VolumeRole::System {
        return "system".to_owned();
    }
    if let Some(label) = label.filter(|value| !value.trim().is_empty()) {
        let mut name = String::new();
        for ch in label.chars() {
            if ch.is_ascii_alphanumeric() {
                name.push(ch.to_ascii_lowercase());
            } else if !name.ends_with('-') {
                name.push('-');
            }
        }
        let name = name.trim_matches('-');
        if !name.is_empty() {
            return name.to_owned();
        }
    }
    format!("volume-{devname}")
}

#[cfg(test)]
mod tests {
    use super::{BlockVolumeDiscovery, classify_role, default_native_name};
    use crate::VolumeRole;

    #[test]
    fn classifies_luna_roles_from_partition_metadata() {
        assert_eq!(
            classify_role(Some("LUNA-DATA"), None, None, Some("btrfs")),
            VolumeRole::LunaData
        );
        assert_eq!(
            classify_role(None, Some("LUNA-SYS"), None, Some("ext4")),
            VolumeRole::System
        );
        assert_eq!(
            classify_role(
                None,
                None,
                Some("c12a7328-f81f-11d2-ba4b-00a0c93ec93b"),
                Some("vfat")
            ),
            VolumeRole::Efi
        );
        assert_eq!(
            classify_role(
                None,
                None,
                Some("0657fd6d-a4ab-43c4-84e5-0933c84b4f4f"),
                Some("swap")
            ),
            VolumeRole::Swap
        );
    }

    #[test]
    fn derives_native_names_without_using_identity() {
        assert_eq!(
            default_native_name(VolumeRole::LunaData, Some("anything"), "sda1"),
            "data"
        );
        assert_eq!(
            default_native_name(VolumeRole::External, Some("My Backup"), "sdb1"),
            "my-backup"
        );
        assert_eq!(
            default_native_name(VolumeRole::External, None, "sdb1"),
            "volume-sdb1"
        );
    }

    #[test]
    fn discovery_has_linux_paths_only_as_backend_details() {
        let discovery = BlockVolumeDiscovery::new();
        assert_eq!(
            discovery.sysfs_block_root,
            std::path::PathBuf::from("/sys/class/block")
        );
        assert_eq!(
            discovery.dev_disk_root,
            std::path::PathBuf::from("/dev/disk")
        );
    }
}
