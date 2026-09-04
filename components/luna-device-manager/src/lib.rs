//! Device and volume lifecycle boundary for Project Luna.

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

pub trait DeviceQuery {
    type Error;
    fn volumes(&self) -> Result<Vec<VolumeInfo>, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::{VolumeId, VolumeInfo, VolumeState};

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
}
