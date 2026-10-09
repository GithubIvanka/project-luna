//! Logical volume registry, automatic attachment policy, and backend resolution.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub use luna_common::{DriveLetter, DriveLetterError, VolumeId, VolumeRef, VolumeRole};
use luna_device_manager::DiscoveredBlockVolume;

mod persistence;
pub use persistence::{VOLUME_REGISTRY_VERSION, VolumeRegistryError};

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
    removable: bool,
}

/// Backend-only information needed when a VolumeRef is materialized into a
/// Linux mount. Applications never receive this object directly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolumeBackend {
    volume: VolumeRef,
    backend_node: PathBuf,
    filesystem: String,
    read_only: bool,
}

impl VolumeBackend {
    pub fn volume(&self) -> &VolumeRef {
        &self.volume
    }

    pub fn backend_node(&self) -> &Path {
        &self.backend_node
    }

    pub fn filesystem(&self) -> &str {
        &self.filesystem
    }

    pub fn read_only(&self) -> bool {
        self.read_only
    }
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
            removable: false,
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
            removable: false,
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

    pub fn is_removable(&self) -> bool {
        self.removable
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
            removable: other.removable,
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
pub enum VolumeBackendError {
    UnknownVolume(VolumeId),
    Unavailable {
        volume: VolumeId,
        state: VolumeState,
    },
    NonMountableRole {
        volume: VolumeId,
        role: VolumeRole,
    },
    MissingFilesystem(VolumeId),
    UnsupportedFilesystem {
        volume: VolumeId,
        filesystem: String,
    },
    MissingBackendNode(VolumeId),
}

impl std::fmt::Display for VolumeBackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownVolume(id) => write!(f, "unknown volume {id}"),
            Self::Unavailable { volume, state } => {
                write!(
                    f,
                    "volume {volume} is not available for mounting: {state:?}"
                )
            }
            Self::NonMountableRole { volume, role } => {
                write!(f, "volume {volume} has non-mountable role {role:?}")
            }
            Self::MissingFilesystem(id) => write!(f, "volume {id} has no filesystem type"),
            Self::UnsupportedFilesystem { volume, filesystem } => write!(
                f,
                "volume {volume} uses unsupported filesystem {filesystem}"
            ),
            Self::MissingBackendNode(id) => write!(f, "volume {id} has no backend node"),
        }
    }
}

impl std::error::Error for VolumeBackendError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VolumeSyncError {
    Assignment(VolumeAssignmentError),
}

impl std::fmt::Display for VolumeSyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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

/// Auto-attach policy controls logical availability, not a global Linux mount.
/// Applications materialize a private MountInstance only when they open a volume.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutoAttachPolicy {
    enabled: bool,
    include_luna_data: bool,
    supported_filesystems: BTreeSet<String>,
}

impl Default for AutoAttachPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            include_luna_data: true,
            supported_filesystems: [
                "btrfs", "ext2", "ext3", "ext4", "exfat", "f2fs", "ntfs", "ntfs3", "udf", "vfat",
                "xfs",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        }
    }
}

impl AutoAttachPolicy {
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn supports_filesystem(&self, filesystem: &str) -> bool {
        self.supported_filesystems
            .contains(&filesystem.to_ascii_lowercase())
    }

    pub fn should_auto_attach(&self, volume: &DiscoveredBlockVolume) -> bool {
        if !self.enabled {
            return false;
        }
        let role_allowed = match volume.role() {
            VolumeRole::External => true,
            VolumeRole::LunaData => self.include_luna_data,
            VolumeRole::Efi
            | VolumeRole::System
            | VolumeRole::Swap
            | VolumeRole::RecoveryVirtual => false,
        };
        role_allowed
            && volume
                .filesystem()
                .is_some_and(|fs| self.supports_filesystem(fs))
    }
}

#[derive(Default)]
pub struct VolumeManager {
    volumes: BTreeMap<VolumeId, VolumeInfo>,
    volume_registry: VolumeRegistry,
    auto_attach: AutoAttachPolicy,
}

impl VolumeManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn auto_attach_policy(&self) -> &AutoAttachPolicy {
        &self.auto_attach
    }
    pub fn auto_attach_policy_mut(&mut self) -> &mut AutoAttachPolicy {
        &mut self.auto_attach
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
        let letter = match volume.role() {
            // LUNA-DATA owns C: even while its backend is temporarily unavailable.
            VolumeRole::LunaData => self.volume_registry.assign(&volume)?,
            VolumeRole::External
                if matches!(
                    volume.state(),
                    VolumeState::Available | VolumeState::Mounted
                ) =>
            {
                self.volume_registry.assign(&volume)?
            }
            _ => {
                self.volume_registry.release(volume.id());
                None
            }
        };
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

    /// Resolve an object-level reference to backend details for namespace-local materialization.
    pub fn volume_backend(&self, volume: &VolumeRef) -> Result<VolumeBackend, VolumeBackendError> {
        let info = self
            .volume(volume.id())
            .ok_or_else(|| VolumeBackendError::UnknownVolume(volume.id().clone()))?;
        if !matches!(info.state(), VolumeState::Available | VolumeState::Mounted) {
            return Err(VolumeBackendError::Unavailable {
                volume: volume.id().clone(),
                state: info.state(),
            });
        }
        if !matches!(info.role(), VolumeRole::External | VolumeRole::LunaData) {
            return Err(VolumeBackendError::NonMountableRole {
                volume: volume.id().clone(),
                role: info.role(),
            });
        }
        let filesystem = info
            .filesystem()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| VolumeBackendError::MissingFilesystem(volume.id().clone()))?;
        if !self.auto_attach.supports_filesystem(filesystem) {
            return Err(VolumeBackendError::UnsupportedFilesystem {
                volume: volume.id().clone(),
                filesystem: filesystem.to_owned(),
            });
        }
        if info.backend_node().is_empty() {
            return Err(VolumeBackendError::MissingBackendNode(volume.id().clone()));
        }
        Ok(VolumeBackend {
            volume: volume.clone(),
            backend_node: PathBuf::from(info.backend_node()),
            filesystem: filesystem.to_owned(),
            read_only: false,
        })
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
                if let Some(name) = volume.native_name() {
                    registry.set_native_name(volume.id(), name);
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

    pub fn reconcile_discovered_volumes(
        &mut self,
        discovered: Vec<DiscoveredBlockVolume>,
    ) -> Result<Vec<VolumeEvent>, VolumeSyncError> {
        let mut seen = BTreeSet::new();
        let mut events = Vec::new();

        for discovered_volume in discovered {
            let id = discovered_volume.id().clone();
            seen.insert(id.clone());
            let state = if self.auto_attach.should_auto_attach(&discovered_volume) {
                VolumeState::Available
            } else {
                VolumeState::Detected
            };
            let mut incoming = VolumeInfo::discovered(
                id.clone(),
                discovered_volume.label().map(str::to_owned),
                discovered_volume.native_name().to_owned(),
                discovered_volume.filesystem().map(str::to_owned),
                discovered_volume.role(),
                state,
                discovered_volume.backend_node().to_owned(),
            );
            incoming.removable = discovered_volume.is_removable();

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
                let previous_letter = self.volume_registry.current_letter(&id);
                let letter = match updated.role() {
                    VolumeRole::LunaData => self
                        .volume_registry
                        .assign(&updated)
                        .map_err(VolumeSyncError::Assignment)?,
                    VolumeRole::External
                        if matches!(
                            updated.state(),
                            VolumeState::Available | VolumeState::Mounted
                        ) =>
                    {
                        self.volume_registry
                            .assign(&updated)
                            .map_err(VolumeSyncError::Assignment)?
                    }
                    _ => {
                        self.volume_registry.release(&id);
                        None
                    }
                };
                if updated != existing || previous_letter != letter {
                    self.volumes.insert(id, updated.clone());
                    events.push(VolumeEvent::Updated {
                        volume: updated,
                        drive_letter: letter,
                    });
                }
            } else {
                let letter = self
                    .register_volume(incoming.clone())
                    .map_err(VolumeSyncError::Assignment)?;
                let volume = self
                    .volumes
                    .get(&id)
                    .cloned()
                    .expect("registered volume must be present");
                events.push(VolumeEvent::Attached {
                    volume,
                    drive_letter: letter,
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
            let letter = self.volume_registry.current_letter(&id);
            if let Some(volume) = self.remove_volume(&id) {
                events.push(VolumeEvent::Detached {
                    volume,
                    drive_letter: letter,
                });
            }
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::{AutoAttachPolicy, VolumeInfo, VolumeManager, VolumeRegistry, VolumeState};
    use crate::VolumeBackendError;
    use luna_common::{DriveLetter, VolumeId, VolumeRef, VolumeRole};
    use luna_device_manager::DiscoveredBlockVolume;
    use std::path::Path;

    fn discovered(
        id: &str,
        fs: Option<&str>,
        role: VolumeRole,
        removable: bool,
    ) -> DiscoveredBlockVolume {
        DiscoveredBlockVolume::new(
            VolumeId::from(id),
            Some("USB".into()),
            "usb".into(),
            fs.map(str::to_owned),
            role,
            "/dev/sdb1".into(),
            removable,
        )
    }

    #[test]
    fn external_filesystem_is_attached_automatically_without_host_mount_path() {
        let mut manager = VolumeManager::new();
        let events = manager
            .reconcile_discovered_volumes(vec![discovered(
                "partition-guid:usb",
                Some("exfat"),
                VolumeRole::External,
                true,
            )])
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].volume().state(), VolumeState::Available);
        assert_eq!(events[0].drive_letter(), Some(DriveLetter::D));
        let backend = manager
            .volume_backend(&VolumeRef::new(VolumeId::from("partition-guid:usb")))
            .unwrap();
        assert_eq!(backend.backend_node(), Path::new("/dev/sdb1"));
        assert!(events[0].volume().is_removable());
    }

    #[test]
    fn internal_user_disk_is_auto_attached_without_removable_flag() {
        let mut manager = VolumeManager::new();
        let events = manager
            .reconcile_discovered_volumes(vec![discovered(
                "partition-guid:internal",
                Some("ext4"),
                VolumeRole::External,
                false,
            )])
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].volume().state(), VolumeState::Available);
        assert_eq!(events[0].drive_letter(), Some(DriveLetter::D));
        assert!(!events[0].volume().is_removable());
    }

    #[test]
    fn detach_releases_drive_alias_for_next_inserted_volume() {
        let mut manager = VolumeManager::new();
        let attached = manager
            .reconcile_discovered_volumes(vec![discovered(
                "usb-one",
                Some("exfat"),
                VolumeRole::External,
                true,
            )])
            .unwrap();
        assert_eq!(attached[0].drive_letter(), Some(DriveLetter::D));

        let detached = manager.reconcile_discovered_volumes(Vec::new()).unwrap();
        assert_eq!(detached.len(), 1);
        assert_eq!(detached[0].drive_letter(), Some(DriveLetter::D));
        assert_eq!(detached[0].volume().state(), VolumeState::Available);

        let next = manager
            .reconcile_discovered_volumes(vec![discovered(
                "usb-two",
                Some("vfat"),
                VolumeRole::External,
                true,
            )])
            .unwrap();
        assert_eq!(next[0].drive_letter(), Some(DriveLetter::D));
    }

    #[test]
    fn volume_that_loses_supported_filesystem_becomes_detected_and_releases_alias() {
        let mut manager = VolumeManager::new();
        manager
            .reconcile_discovered_volumes(vec![discovered(
                "usb",
                Some("ext4"),
                VolumeRole::External,
                true,
            )])
            .unwrap();

        let updated = manager
            .reconcile_discovered_volumes(vec![discovered(
                "usb",
                Some("unknownfs"),
                VolumeRole::External,
                true,
            )])
            .unwrap();
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].volume().state(), VolumeState::Detected);
        assert_eq!(updated[0].drive_letter(), None);

        let next = manager
            .reconcile_discovered_volumes(vec![
                discovered("usb", Some("unknownfs"), VolumeRole::External, true),
                discovered("another-usb", Some("exfat"), VolumeRole::External, true),
            ])
            .unwrap();
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].volume().id().as_str(), "another-usb");
        assert_eq!(next[0].drive_letter(), Some(DriveLetter::D));
    }

    #[test]
    fn unsupported_and_protected_partitions_are_not_auto_attached() {
        let mut manager = VolumeManager::new();
        let events = manager
            .reconcile_discovered_volumes(vec![
                discovered("unknown", Some("mysteryfs"), VolumeRole::External, true),
                discovered("system", Some("ext4"), VolumeRole::System, false),
            ])
            .unwrap();
        assert_eq!(events[0].volume().state(), VolumeState::Detected);
        assert_eq!(events[1].volume().state(), VolumeState::Detected);
        assert_eq!(events[0].drive_letter(), None);
        assert_eq!(events[1].drive_letter(), None);
        assert!(matches!(
            manager.volume_backend(&VolumeRef::new(VolumeId::from("unknown"))),
            Err(VolumeBackendError::Unavailable { .. })
        ));
    }

    #[test]
    fn autoscan_preserves_volume_identity_and_drive_letter_on_update() {
        let mut manager = VolumeManager::new();
        manager
            .reconcile_discovered_volumes(vec![discovered(
                "usb",
                Some("ext4"),
                VolumeRole::External,
                true,
            )])
            .unwrap();
        let events = manager
            .reconcile_discovered_volumes(vec![DiscoveredBlockVolume::new(
                VolumeId::from("usb"),
                Some("New label".into()),
                "usb".into(),
                Some("ext4".into()),
                VolumeRole::External,
                "/dev/sdc1".into(),
                true,
            )])
            .unwrap();
        assert_eq!(events[0].drive_letter(), Some(DriveLetter::D));
        assert_eq!(
            manager
                .volume(&VolumeId::from("usb"))
                .unwrap()
                .id()
                .as_str(),
            "usb"
        );
    }

    #[test]
    fn luna_data_reserves_c_and_system_volumes_do_not_get_drive_letters() {
        let mut registry = VolumeRegistry::new();
        let data = VolumeInfo::with_role(
            VolumeId::from("data"),
            Some("LUNA-DATA".into()),
            VolumeState::Available,
            VolumeRole::LunaData,
        );
        let sys = VolumeInfo::with_role(
            VolumeId::from("sys"),
            Some("LUNA-SYS".into()),
            VolumeState::Available,
            VolumeRole::System,
        );
        assert_eq!(registry.assign(&data).unwrap(), Some(DriveLetter::C));
        assert_eq!(registry.assign(&sys).unwrap(), None);
    }

    #[test]
    fn auto_attach_can_be_disabled_without_losing_discovery() {
        let mut policy = AutoAttachPolicy::default();
        policy.set_enabled(false);
        assert!(!policy.is_enabled());
        assert!(!policy.should_auto_attach(&discovered(
            "usb",
            Some("ext4"),
            VolumeRole::External,
            true
        )));
    }

    #[test]
    fn backend_resolution_rejects_non_mountable_roles() {
        let mut manager = VolumeManager::new();
        manager
            .register_volume(VolumeInfo::discovered(
                VolumeId::from("system"),
                Some("LUNA-SYS".into()),
                "system".into(),
                Some("ext4".into()),
                VolumeRole::System,
                VolumeState::Available,
                "/dev/sda2".into(),
            ))
            .unwrap();
        assert!(matches!(
            manager.volume_backend(&VolumeRef::new(VolumeId::from("system"))),
            Err(VolumeBackendError::NonMountableRole { .. })
        ));
    }
}
