//! Device and volume lifecycle boundary for Project Luna.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub use luna_common::{DriveLetter, DriveLetterError, VolumeId, VolumeRef, VolumeRole};

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
    role: VolumeRole,
}

impl VolumeInfo {
    pub fn new(id: VolumeId, label: Option<String>, state: VolumeState) -> Self {
        Self::with_role(id, label, state, VolumeRole::External)
    }

    pub fn with_role(
        id: VolumeId,
        label: Option<String>,
        state: VolumeState,
        role: VolumeRole,
    ) -> Self {
        Self {
            id,
            label,
            state,
            role,
        }
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

    pub fn role(&self) -> VolumeRole {
        self.role
    }
}

#[derive(Debug, Default)]
pub struct VolumeRegistry {
    active: BTreeMap<VolumeId, DriveLetter>,
    by_letter: BTreeMap<DriveLetter, VolumeId>,
    labels: BTreeMap<VolumeId, String>,
    preferred: BTreeMap<VolumeId, DriveLetter>,
    roles: BTreeMap<VolumeId, VolumeRole>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum VolumeAssignmentError {
    NoExternalLettersAvailable,
    InvalidPreferredLetter,
    LetterAlreadyAssigned,
    CannotReassignReservedVolume,
    VolumeNotActive,
}

impl std::fmt::Display for VolumeAssignmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::NoExternalLettersAvailable => "no external drive letters are available",
            Self::InvalidPreferredLetter => "preferred drive letter must be D through Z",
            Self::LetterAlreadyAssigned => "drive letter is already assigned",
            Self::CannotReassignReservedVolume => "reserved volume cannot be assigned a drive letter",
            Self::VolumeNotActive => "volume is not actively assigned",
        };
        f.write_str(message)
    }
}

impl std::error::Error for VolumeAssignmentError {}

impl VolumeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_label(
        &mut self,
        volume: &VolumeId,
        label: impl Into<String>,
    ) {
        self.labels.insert(volume.clone(), label.into());
    }

    pub fn label(&self, volume: &VolumeId) -> Option<&str> {
        self.labels.get(volume).map(String::as_str)
    }

    pub fn set_preferred(
        &mut self,
        volume: &VolumeId,
        letter: DriveLetter,
    ) -> Result<(), VolumeAssignmentError> {
        if !letter.is_external_range() {
            return Err(VolumeAssignmentError::InvalidPreferredLetter);
        }
        self.preferred.insert(volume.clone(), letter);
        Ok(())
    }

    pub fn clear_preferred(&mut self, volume: &VolumeId) {
        self.preferred.remove(volume);
    }

    pub fn preferred(&self, volume: &VolumeId) -> Option<DriveLetter> {
        self.preferred.get(volume).copied()
    }

    pub fn assign(&mut self, volume: &VolumeInfo) -> Result<Option<DriveLetter>, VolumeAssignmentError> {
        if let Some(letter) = self.active.get(volume.id()).copied() {
            return Ok(Some(letter));
        }

        self.roles.insert(volume.id().clone(), volume.role());
        match volume.role() {
            VolumeRole::LunaData => {
                if self.by_letter.contains_key(&DriveLetter::C) {
                    return Err(VolumeAssignmentError::LetterAlreadyAssigned);
                }
                self.active.insert(volume.id().clone(), DriveLetter::C);
                self.by_letter.insert(DriveLetter::C, volume.id().clone());
                Ok(Some(DriveLetter::C))
            }
            VolumeRole::Efi
            | VolumeRole::System
            | VolumeRole::Swap
            | VolumeRole::RecoveryVirtual => Ok(None),
            VolumeRole::External => {
                let letter = self.preferred(volume.id()).and_then(|preferred| {
                    if !self.by_letter.contains_key(&preferred) {
                        Some(preferred)
                    } else {
                        None
                    }
                }).or_else(|| {
                    ('D'..='Z')
                        .map(|value| DriveLetter::new(value).expect("ASCII letter"))
                        .find(|candidate| !self.by_letter.contains_key(candidate))
                }).ok_or(VolumeAssignmentError::NoExternalLettersAvailable)?;

                self.active.insert(volume.id().clone(), letter);
                self.by_letter.insert(letter, volume.id().clone());
                Ok(Some(letter))
            }
        }
    }

    pub fn reassign(
        &mut self,
        volume: &VolumeId,
        letter: DriveLetter,
    ) -> Result<(), VolumeAssignmentError> {
        if !letter.is_external_range() {
            return Err(VolumeAssignmentError::CannotReassignReservedVolume);
        }
        let role = self
            .roles
            .get(volume)
            .copied()
            .ok_or(VolumeAssignmentError::VolumeNotActive)?;
        if role != VolumeRole::External {
            return Err(VolumeAssignmentError::CannotReassignReservedVolume);
        }
        if let Some(owner) = self.by_letter.get(&letter) {
            if owner != volume {
                return Err(VolumeAssignmentError::LetterAlreadyAssigned);
            }
        }
        let old = self.active.insert(volume.clone(), letter);
        if let Some(old) = old {
            self.by_letter.remove(&old);
        }
        self.by_letter.insert(letter, volume.clone());
        Ok(())
    }

    pub fn release(&mut self, volume: &VolumeId) -> Option<DriveLetter> {
        let letter = self.active.remove(volume)?;
        self.by_letter.remove(&letter);
        self.roles.remove(volume);
        Some(letter)
    }

    pub fn current_letter(&self, volume: &VolumeId) -> Option<DriveLetter> {
        self.active.get(volume).copied()
    }

    pub fn volume_for_letter(&self, letter: DriveLetter) -> Option<&VolumeId> {
        self.by_letter.get(&letter)
    }

    pub fn active_assignments(&self) -> impl Iterator<Item = (&VolumeId, DriveLetter)> {
        self.active.iter().map(|(id, letter)| (id, *letter))
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
    volumes: BTreeMap<VolumeId, VolumeInfo>,
    volume_registry: VolumeRegistry,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_volume(
        &mut self,
        volume: VolumeInfo,
    ) -> Result<Option<DriveLetter>, VolumeAssignmentError> {
        if let Some(label) = volume.label() {
            self.volume_registry.set_label(volume.id(), label);
        }
        let letter = self.volume_registry.assign(&volume)?;
        self.volumes.insert(volume.id().clone(), volume);
        Ok(letter)
    }

    pub fn remove_volume(&mut self, volume: &VolumeId) -> Option<VolumeInfo> {
        self.volume_registry.release(volume);
        self.volumes.remove(volume)
    }

    pub fn volumes(&self) -> impl Iterator<Item = &VolumeInfo> {
        self.volumes.values()
    }

    pub fn volume_registry(&self) -> &VolumeRegistry {
        &self.volume_registry
    }

    pub fn volume_registry_mut(&mut self) -> &mut VolumeRegistry {
        &mut self.volume_registry
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
    use super::{
        DeviceId, DeviceManager, DriveLetter, InputDeviceInfo, VolumeId, VolumeInfo, VolumeRegistry,
        VolumeRole, VolumeState,
    };

    #[test]
    fn volume_identity_and_user_label_are_separate() {
        let volume = VolumeInfo::new(
            VolumeId::new("volume-1"),
            Some("fleshka".to_owned()),
            VolumeState::Mounted,
        );
        assert_eq!(volume.id().as_str(), "volume-1");
        assert_eq!(volume.label(), Some("fleshka"));
        assert_eq!(volume.role(), VolumeRole::External);
    }

    #[test]
    fn user_label_can_change_without_changing_volume_identity() {
        let mut registry = VolumeRegistry::new();
        let id = VolumeId::from("usb-1");
        registry.set_label(&id, "Archive");
        assert_eq!(registry.label(&id), Some("Archive"));
        registry.set_label(&id, "Projects");
        assert_eq!(registry.label(&id), Some("Projects"));
        assert_eq!(id.as_str(), "usb-1");
    }

    #[test]
    fn luna_data_always_gets_c_and_system_volumes_stay_hidden() {
        let mut registry = VolumeRegistry::new();
        let data = VolumeInfo::with_role(
            VolumeId::from("data-1"),
            Some("Luna Data".into()),
            VolumeState::Mounted,
            VolumeRole::LunaData,
        );
        let sys = VolumeInfo::with_role(
            VolumeId::from("sys-1"),
            Some("LUNA-SYS".into()),
            VolumeState::Mounted,
            VolumeRole::System,
        );
        assert_eq!(registry.assign(&data).unwrap(), Some(DriveLetter::C));
        assert_eq!(registry.assign(&sys).unwrap(), None);
    }

    #[test]
    fn external_volumes_are_assigned_sequentially_and_reuse_released_letters() {
        let mut registry = VolumeRegistry::new();
        let a = VolumeInfo::new(VolumeId::from("a"), Some("A".into()), VolumeState::Mounted);
        let b = VolumeInfo::new(VolumeId::from("b"), Some("B".into()), VolumeState::Mounted);
        let c = VolumeInfo::new(VolumeId::from("c"), Some("C".into()), VolumeState::Mounted);
        assert_eq!(registry.assign(&a).unwrap().unwrap().as_char(), 'D');
        assert_eq!(registry.assign(&b).unwrap().unwrap().as_char(), 'E');
        assert_eq!(registry.release(a.id()).unwrap().as_char(), 'D');
        assert_eq!(registry.assign(&c).unwrap().unwrap().as_char(), 'D');
    }

    #[test]
    fn preferred_letter_is_optional_and_explicitly_user_controlled() {
        let mut registry = VolumeRegistry::new();
        let volume = VolumeId::from("backup");
        registry.set_preferred(&volume, DriveLetter::new('H').unwrap()).unwrap();
        let info = VolumeInfo::new(volume.clone(), Some("Backup".into()), VolumeState::Mounted);
        assert_eq!(registry.assign(&info).unwrap(), Some(DriveLetter::new('H').unwrap()));
        assert_eq!(registry.preferred(&volume), Some(DriveLetter::new('H').unwrap()));
        registry.clear_preferred(&volume);
        assert_eq!(registry.preferred(&volume), None);
    }

    #[test]
    fn drive_letter_reassignment_never_moves_the_volume_identity() {
        let mut registry = VolumeRegistry::new();
        let id = VolumeId::from("usb-1");
        let info = VolumeInfo::new(id.clone(), Some("USB".into()), VolumeState::Mounted);
        assert_eq!(registry.assign(&info).unwrap(), Some(DriveLetter::D));
        let new_letter = DriveLetter::new('F').unwrap();
        registry.reassign(&id, new_letter).unwrap();
        assert_eq!(registry.current_letter(&id), Some(new_letter));
        assert_eq!(registry.volume_for_letter(new_letter), Some(&id));
        assert_eq!(registry.volume_for_letter(DriveLetter::D), None);
    }

    #[test]
    fn c_cannot_be_reassigned_to_an_external_volume() {
        let mut registry = VolumeRegistry::new();
        let info = VolumeInfo::new(VolumeId::from("usb-1"), None, VolumeState::Mounted);
        registry.assign(&info).unwrap();
        assert_eq!(
            registry.reassign(info.id(), DriveLetter::C),
            Err(super::VolumeAssignmentError::CannotReassignReservedVolume)
        );
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
