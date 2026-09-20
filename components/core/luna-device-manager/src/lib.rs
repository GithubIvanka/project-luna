//! Device and volume lifecycle boundary for Project Luna.

use std::fs;
use std::path::Path;

mod uevent;
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

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct VolumeId(String);

impl VolumeId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VolumeState {
    Detected,
    Mounted,
    Unavailable,
    Removing,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolumeInfo {
    id: VolumeId,
    label: Option<String>,
    state: VolumeState,
}

impl VolumeInfo {
    pub fn new(id: VolumeId, label: Option<String>, state: VolumeState) -> Self {
        Self { id, label, state }
    }
    pub fn id(&self) -> &VolumeId {
        &self.id
    }
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
    pub fn state(&self) -> VolumeState {
        self.state
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

pub trait DeviceQuery {
    type Error;
    fn volumes(&self) -> Result<Vec<VolumeInfo>, Self::Error>;
}
#[cfg(test)]
mod tests {
    use super::{DeviceId, DeviceManager, InputDeviceInfo, VolumeId, VolumeInfo, VolumeState};

    #[test]
    fn volume_identity_and_user_label_are_separate() {
        let volume = VolumeInfo::new(
            VolumeId::new("volume-1"),
            Some("fleshka".to_owned()),
            VolumeState::Mounted,
        );
        assert_eq!(volume.id().as_str(), "volume-1");
        assert_eq!(volume.label(), Some("fleshka"));
    }

    #[test]
    fn device_manager_starts_with_empty_registry() {
        let manager = DeviceManager::new();
        assert!(manager.input_devices().is_empty());
    }

    #[test]
    fn input_device_identity_keeps_kernel_node_separate() {
        let id = DeviceId::new("input:event3");
        let info = InputDeviceInfo {
            id,
            node: "/dev/input/event3".into(),
            sysfs_path: "/sys/class/input/event3".into(),
            name: Some("keyboard".into()),
        };
        assert_eq!(info.id().as_str(), "input:event3");
        assert_eq!(info.node(), "/dev/input/event3");
        assert_eq!(info.name(), Some("keyboard"));
    }
}
