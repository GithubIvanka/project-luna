//! Device and volume lifecycle boundary for Project Luna.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub use luna_common::{DriveLetter, DriveLetterError, VolumeId, VolumeRef, VolumeRole};

mod block;
mod persistence;
mod uevent;

pub use block::BlockVolumeDiscovery;
pub use persistence::{VOLUME_REGISTRY_VERSION, VolumeRegistryError};
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
    Available,
    Mounted,
    Unavailable,
    Removing,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolumeInfo {
    id: VolumeId,
    label: Option<String>,
    native_name: Option<String>,
    filesystem: Option<String>,
    state: VolumeState,
    role: VolumeRole,
    backend_node: String,
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
            native_name: None,
            filesystem: None,
            state,
            role,
            backend_node: String::new(),
        }
    }

    pub fn discovered(
        id: VolumeId,
        label: Option<String>,
        native_name: String,
        filesystem: Option<String>,
        role: VolumeRole,
        state: VolumeState,
        backend_node: String,
    ) -> Self {
        Self {
            id,
            label,
            native_name: Some(native_name),
            filesystem,
            state,
            role,
            backend_node,
        }
    }

    pub fn id(&self) -> &VolumeId {
        &self.id
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn native_name(&self) -> Option<&str> {
        self.native_name.as_deref()
    }

    pub fn filesystem(&self) -> Option<&str> {
        self.filesystem.as_deref()
    }

    pub fn state(&self) -> VolumeState {
        self.state
    }

    pub fn role(&self) -> VolumeRole {
        self.role
    }

    pub(crate) fn backend_node(&self) -> &str {
        &self.backend_node
    }

    fn with_runtime_metadata(
        &self,
        other: &Self,
        label: Option<String>,
        native_name: Option<String>,
    ) -> Self {
        Self {
            id: self.id.clone(),
            label,
            native_name,
            filesystem: other.filesystem.clone(),
            state: other.state,
            role: other.role,
            backend_node: other.backend_node.clone(),
        }
    }
}

#[derive(Debug, Default)]
pub struct VolumeRegistry {
    active: BTreeMap<VolumeId, DriveLetter>,
    by_letter: BTreeMap<DriveLetter, VolumeId>,
    labels: BTreeMap<VolumeId, String>,
    native_names: BTreeMap<VolumeId, String>,
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
            Self::CannotReassignReservedVolume => {
                "reserved volume cannot be assigned a drive letter"
            }
            Self::VolumeNotActive => "volume is not actively assigned",
        };
        f.write_str(message)
    }
}

impl std::error::Error for VolumeAssignmentError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VolumeSyncError {
    Discovery(String),
    Assignment(VolumeAssignmentError),
}

impl std::fmt::Display for VolumeSyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Discovery(error) => write!(f, "volume discovery failed: {error}"),
            Self::Assignment(error) => write!(f, "volume assignment failed: {error}"),
        }
    }
}

impl std::error::Error for VolumeSyncError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VolumeEvent {
    Attached {
        volume: VolumeInfo,
        drive_letter: Option<DriveLetter>,
    },
    Updated {
        volume: VolumeInfo,
        drive_letter: Option<DriveLetter>,
    },
    Detached {
        volume: VolumeInfo,
        drive_letter: Option<DriveLetter>,
    },
}

impl VolumeEvent {
    pub fn volume(&self) -> &VolumeInfo {
        match self {
            Self::Attached { volume, .. }
            | Self::Updated { volume, .. }
            | Self::Detached { volume, .. } => volume,
        }
    }

    pub fn drive_letter(&self) -> Option<DriveLetter> {
        match self {
            Self::Attached { drive_letter, .. }
            | Self::Updated { drive_letter, .. }
            | Self::Detached { drive_letter, .. } => *drive_letter,
        }
    }
}

impl VolumeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_label(&mut self, volume: &VolumeId, label: impl Into<String>) {
        self.labels.insert(volume.clone(), label.into());
    }

    pub fn label(&self, volume: &VolumeId) -> Option<&str> {
        self.labels.get(volume).map(String::as_str)
    }

    pub fn set_native_name(&mut self, volume: &VolumeId, native_name: impl Into<String>) {
        self.native_names.insert(volume.clone(), native_name.into());
    }

    pub fn native_name(&self, volume: &VolumeId) -> Option<&str> {
        self.native_names.get(volume).map(String::as_str)
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

    pub fn assign(
        &mut self,
        volume: &VolumeInfo,
    ) -> Result<Option<DriveLetter>, VolumeAssignmentError> {
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
                let letter = self
                    .preferred(volume.id())
                    .and_then(|preferred| {
                        if !self.by_letter.contains_key(&preferred) {
                            Some(preferred)
                        } else {
                            None
                        }
                    })
                    .or_else(|| {
                        ('D'..='Z')
                            .map(|value| DriveLetter::new(value).expect("ASCII letter"))
                            .find(|candidate| !self.by_letter.contains_key(candidate))
                    })
                    .ok_or(VolumeAssignmentError::NoExternalLettersAvailable)?;

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

    pub fn volume_ref_for_letter(&self, letter: DriveLetter) -> Option<VolumeRef> {
        self.volume_for_letter(letter).cloned().map(VolumeRef::new)
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
        mut volume: VolumeInfo,
    ) -> Result<Option<DriveLetter>, VolumeAssignmentError> {
        if let Some(label) = self.volume_registry.label(volume.id()) {
            volume.label = Some(label.to_owned());
        } else if let Some(label) = volume.label() {
            self.volume_registry.set_label(volume.id(), label);
        }

        if let Some(native_name) = self.volume_registry.native_name(volume.id()) {
            volume.native_name = Some(native_name.to_owned());
        }

        let letter = self.volume_registry.assign(&volume)?;
        self.volumes.insert(volume.id().clone(), volume);
        Ok(letter)
    }

    pub fn remove_volume(&mut self, volume: &VolumeId) -> Option<VolumeInfo> {
        self.volume_registry.release(volume);
        self.volumes.remove(volume)
    }

    pub fn volume(&self, volume: &VolumeId) -> Option<&VolumeInfo> {
        self.volumes.get(volume)
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

    pub fn load_volume_preferences(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<(), VolumeRegistryError> {
        let mut registry = VolumeRegistry::load_preferences(path)?;
        for volume in self.volumes.values() {
            if registry.label(volume.id()).is_none() {
                if let Some(label) = volume.label() {
                    registry.set_label(volume.id(), label);
                }
            }
            if registry.native_name(volume.id()).is_none() {
                if let Some(native_name) = volume.native_name() {
                    registry.set_native_name(volume.id(), native_name);
                }
            }
            registry
                .assign(volume)
                .map_err(VolumeRegistryError::Assignment)?;
        }
        self.volume_registry = registry;
        Ok(())
    }

    pub fn save_volume_preferences(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<(), VolumeRegistryError> {
        self.volume_registry.save_preferences(path)
    }

    pub fn set_volume_label(&mut self, volume: &VolumeId, label: impl Into<String>) -> bool {
        if let Some(info) = self.volumes.get_mut(volume) {
            let label = label.into();
            self.volume_registry.set_label(volume, &label);
            info.label = Some(label);
            true
        } else {
            false
        }
    }

    pub fn set_volume_native_name(
        &mut self,
        volume: &VolumeId,
        native_name: impl Into<String>,
    ) -> bool {
        if let Some(info) = self.volumes.get_mut(volume) {
            let native_name = native_name.into();
            self.volume_registry.set_native_name(volume, &native_name);
            info.native_name = Some(native_name);
            true
        } else {
            false
        }
    }

    pub fn reconcile_block_volumes(
        &mut self,
        discovery: &BlockVolumeDiscovery,
    ) -> Result<Vec<VolumeEvent>, VolumeSyncError> {
        let discovered = discovery
            .scan()
            .map_err(|error| VolumeSyncError::Discovery(error.to_string()))?;
        let mut seen = std::collections::BTreeSet::new();
        let mut events = Vec::new();

        for incoming in discovered {
            let id = incoming.id().clone();
            seen.insert(id.clone());

            if let Some(existing) = self.volumes.get(&id).cloned() {
                let label = self
                    .volume_registry
                    .label(&id)
                    .map(str::to_owned)
                    .or_else(|| incoming.label.clone());
                let native_name = self
                    .volume_registry
                    .native_name(&id)
                    .map(str::to_owned)
                    .or_else(|| incoming.native_name.clone())
                    .or_else(|| existing.native_name.clone());
                let updated = existing.with_runtime_metadata(&incoming, label, native_name);
                let drive_letter = self.volume_registry.current_letter(&id);

                if updated != existing {
                    self.volumes.insert(id, updated.clone());
                    events.push(VolumeEvent::Updated {
                        volume: updated,
                        drive_letter,
                    });
                }
            } else {
                let drive_letter = self
                    .register_volume(incoming.clone())
                    .map_err(VolumeSyncError::Assignment)?;
                let volume = self
                    .volumes
                    .get(&id)
                    .cloned()
                    .expect("registered volume must be present");
                events.push(VolumeEvent::Attached {
                    volume,
                    drive_letter,
                });
            }
        }

        let removed = self
            .volumes
            .keys()
            .filter(|id| !seen.contains(*id))
            .cloned()
            .collect::<Vec<_>>();

        for id in removed {
            let drive_letter = self.volume_registry.current_letter(&id);
            if let Some(volume) = self.remove_volume(&id) {
                events.push(VolumeEvent::Detached {
                    volume,
                    drive_letter,
                });
            }
        }

        Ok(events)
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
        DeviceId, DeviceManager, DriveLetter, InputDeviceInfo, VolumeId, VolumeInfo, VolumeRef,
        VolumeRegistry, VolumeRole, VolumeState,
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
        assert_eq!(
            registry.volume_ref_for_letter(DriveLetter::D),
            Some(VolumeRef::new(a.id().clone()))
        );
        assert_eq!(registry.release(a.id()).unwrap().as_char(), 'D');
        assert_eq!(registry.assign(&c).unwrap().unwrap().as_char(), 'D');
    }

    #[test]
    fn preferred_letter_is_optional_and_explicitly_user_controlled() {
        let mut registry = VolumeRegistry::new();
        let volume = VolumeId::from("backup");
        registry
            .set_preferred(&volume, DriveLetter::new('H').unwrap())
            .unwrap();
        let info = VolumeInfo::new(volume.clone(), Some("Backup".into()), VolumeState::Mounted);
        assert_eq!(
            registry.assign(&info).unwrap(),
            Some(DriveLetter::new('H').unwrap())
        );
        assert_eq!(
            registry.preferred(&volume),
            Some(DriveLetter::new('H').unwrap())
        );
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
    fn device_manager_restores_persisted_preference_before_attach() {
        let path = std::env::temp_dir().join(format!(
            "luna-device-manager-{}-preferences.toml",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        let volume = VolumeId::from("partition-guid:work");
        let mut preferences = VolumeRegistry::new();
        preferences.set_label(&volume, "Work");
        preferences.set_native_name(&volume, "work");
        preferences
            .set_preferred(&volume, DriveLetter::new('H').unwrap())
            .unwrap();
        preferences.save_preferences(&path).unwrap();

        let mut manager = DeviceManager::new();
        manager.load_volume_preferences(&path).unwrap();
        let info = VolumeInfo::new(volume.clone(), Some("Work".into()), VolumeState::Detected);
        assert_eq!(
            manager.register_volume(info).unwrap(),
            Some(DriveLetter::new('H').unwrap())
        );
        assert_eq!(
            manager
                .volume_registry()
                .volume_ref_for_letter(DriveLetter::new('H').unwrap()),
            Some(VolumeRef::new(volume))
        );

        let _ = std::fs::remove_file(path);
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
