//! Physical device discovery and hardware-facing event boundary.
use std::fs;
use std::path::Path;

pub use luna_common::{VolumeId, VolumeRole};

mod block;
mod uevent;

pub use block::BlockVolumeDiscovery;
pub use uevent::{Uevent, UeventMonitor};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct DeviceId(String);

impl DeviceId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredBlockVolume {
    id: VolumeId,
    label: Option<String>,
    native_name: String,
    filesystem: Option<String>,
    role: VolumeRole,
    backend_node: String,
    removable: bool,
}

impl DiscoveredBlockVolume {
    pub fn new(
        id: VolumeId,
        label: Option<String>,
        native_name: String,
        filesystem: Option<String>,
        role: VolumeRole,
        backend_node: String,
        removable: bool,
    ) -> Self {
        Self {
            id,
            label,
            native_name,
            filesystem,
            role,
            backend_node,
            removable,
        }
    }
    pub fn id(&self) -> &VolumeId {
        &self.id
    }
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
    pub fn native_name(&self) -> &str {
        &self.native_name
    }
    pub fn filesystem(&self) -> Option<&str> {
        self.filesystem.as_deref()
    }
    pub fn role(&self) -> VolumeRole {
        self.role
    }
    pub fn backend_node(&self) -> &str {
        &self.backend_node
    }
    pub fn is_removable(&self) -> bool {
        self.removable
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputDeviceInfo {
    id: DeviceId,
    node: String,
    sysfs_path: String,
    name: Option<String>,
}

impl InputDeviceInfo {
    fn from_sysfs(entry: &std::fs::DirEntry) -> std::io::Result<Option<Self>> {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("event") {
            return Ok(None);
        }
        let path = entry.path();
        let device_name = fs::read_to_string(path.join("device/name"))
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        Ok(Some(Self {
            id: DeviceId::new(format!("input:{name}")),
            node: format!("/dev/input/{name}"),
            sysfs_path: path.to_string_lossy().into_owned(),
            name: device_name,
        }))
    }
    pub fn id(&self) -> &DeviceId {
        &self.id
    }
    pub fn node(&self) -> &str {
        &self.node
    }
    pub fn sysfs_path(&self) -> &str {
        &self.sysfs_path
    }
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

#[derive(Default)]
pub struct DeviceManager {
    input: Vec<InputDeviceInfo>,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn refresh_input_devices(&mut self) -> std::io::Result<()> {
        let root = Path::new("/sys/class/input");
        let mut devices = Vec::new();
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            if let Some(device) = InputDeviceInfo::from_sysfs(&entry)? {
                devices.push(device);
            }
        }
        devices.sort_by(|a, b| a.node.cmp(&b.node));
        self.input = devices;
        Ok(())
    }

    pub fn input_devices(&self) -> &[InputDeviceInfo] {
        &self.input
    }
}

#[cfg(test)]
mod tests {
    use super::{DeviceId, DeviceManager, DiscoveredBlockVolume, InputDeviceInfo};
    use luna_common::{VolumeId, VolumeRole};

    #[test]
    fn device_manager_starts_with_empty_input_registry() {
        assert!(DeviceManager::new().input_devices().is_empty());
    }

    #[test]
    fn input_device_identity_keeps_kernel_node_separate() {
        let info = InputDeviceInfo {
            id: DeviceId::new("input:event3"),
            node: "/dev/input/event3".into(),
            sysfs_path: "/sys/class/input/event3".into(),
            name: Some("keyboard".into()),
        };
        assert_eq!(info.id().as_str(), "input:event3");
        assert_eq!(info.node(), "/dev/input/event3");
        assert_eq!(info.name(), Some("keyboard"));
    }

    #[test]
    fn discovered_volume_exposes_identity_separately_from_backend_node() {
        let discovered = DiscoveredBlockVolume::new(
            VolumeId::from("partition-guid:usb"),
            Some("USB".into()),
            "usb".into(),
            Some("exfat".into()),
            VolumeRole::External,
            "/dev/sdb1".into(),
            true,
        );
        assert_eq!(discovered.id().as_str(), "partition-guid:usb");
        assert_eq!(discovered.backend_node(), "/dev/sdb1");
        assert!(discovered.is_removable());
        assert_eq!(discovered.role(), VolumeRole::External);
    }
}
