//! Shared storage identity types for the Luna Volume Namespace.
//!
//! These values describe Luna storage objects and user-facing aliases. They do
//! not own device discovery, persistence, mounting, authorization, or Linux VFS
//! operations.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VolumeId(String);

impl VolumeId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl From<&str> for VolumeId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for VolumeId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for VolumeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DriveLetter(char);

impl DriveLetter {
    pub const C: Self = Self('C');
    pub const D: Self = Self('D');
    pub const Z: Self = Self('Z');

    pub fn new(value: char) -> Result<Self, DriveLetterError> {
        let value = value.to_ascii_uppercase();
        if !value.is_ascii_alphabetic() {
            return Err(DriveLetterError);
        }
        Ok(Self(value))
    }

    pub const fn as_char(self) -> char {
        self.0
    }

    pub const fn is_c(self) -> bool {
        self.0 == 'C'
    }

    pub const fn is_external_range(self) -> bool {
        self.0 >= 'D' && self.0 <= 'Z'
    }
}

impl fmt::Display for DriveLetter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriveLetterError;

impl fmt::Display for DriveLetterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid drive letter")
    }
}

impl std::error::Error for DriveLetterError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VolumeRef {
    id: VolumeId,
}

impl VolumeRef {
    pub fn new(id: VolumeId) -> Self {
        Self { id }
    }

    pub fn id(&self) -> &VolumeId {
        &self.id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VolumePath {
    volume: VolumeRef,
    relative_path: String,
}

impl VolumePath {
    pub fn new(
        volume: VolumeRef,
        relative_path: impl Into<String>,
    ) -> Result<Self, VolumePathError> {
        let relative_path = relative_path.into();
        if !relative_path.starts_with('/') {
            return Err(VolumePathError::NotAbsolute);
        }
        if relative_path.contains('\0') {
            return Err(VolumePathError::Nul);
        }

        let mut normalized = String::from("/");
        for component in relative_path.split('/').filter(|value| !value.is_empty()) {
            if component == ".." || component == "." {
                return Err(VolumePathError::Traversal);
            }
            if normalized.len() > 1 {
                normalized.push('/');
            }
            normalized.push_str(component);
        }

        Ok(Self {
            volume,
            relative_path: normalized,
        })
    }

    pub fn volume(&self) -> &VolumeRef {
        &self.volume
    }

    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    pub fn to_native_display_string(&self, native_name: &str) -> String {
        format!("{}:{}", native_name, self.relative_path)
    }

    pub fn to_windows_display_string(&self, letter: DriveLetter) -> String {
        format!("{}{}", letter, self.relative_path)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumePathError {
    NotAbsolute,
    Nul,
    Traversal,
}

impl fmt::Display for VolumePathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NotAbsolute => "volume-relative path must start with '/'",
            Self::Nul => "volume path contains NUL",
            Self::Traversal => "volume path contains traversal component",
        };
        f.write_str(message)
    }
}

impl std::error::Error for VolumePathError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VolumeRole {
    Efi,
    System,
    LunaData,
    Swap,
    External,
    RecoveryVirtual,
}

#[cfg(test)]
mod tests {
    use super::{DriveLetter, VolumeId, VolumePath, VolumeRef, VolumeRole};

    #[test]
    fn drive_letters_are_case_insensitive_but_canonical_uppercase() {
        assert_eq!(DriveLetter::new('d').unwrap().as_char(), 'D');
        assert!(DriveLetter::new('1').is_err());
    }

    #[test]
    fn c_is_reserved_while_d_to_z_are_external_aliases() {
        assert!(DriveLetter::C.is_c());
        assert!(!DriveLetter::C.is_external_range());
        assert!(DriveLetter::D.is_external_range());
        assert!(DriveLetter::Z.is_external_range());
    }

    #[test]
    fn volume_path_keeps_identity_separate_from_alias() {
        let volume = VolumeRef::new(VolumeId::from("partition-guid:123"));
        let path = VolumePath::new(volume.clone(), "/projects/luna").unwrap();
        assert_eq!(path.volume(), &volume);
        assert_eq!(path.relative_path(), "/projects/luna");
        assert_eq!(path.to_native_display_string("data"), "data:/projects/luna");
        assert_eq!(
            path.to_windows_display_string(DriveLetter::D),
            "D:/projects/luna"
        );
    }

    #[test]
    fn volume_path_normalizes_repeated_and_trailing_slashes() {
        let volume = VolumeRef::new(VolumeId::from("volume-1"));
        let path = VolumePath::new(volume, "//Documents///Projects/").unwrap();
        assert_eq!(path.relative_path(), "/Documents/Projects");
    }

    #[test]
    fn volume_path_rejects_traversal() {
        let volume = VolumeRef::new(VolumeId::from("volume-1"));
        assert!(VolumePath::new(volume, "/../secret").is_err());
    }

    #[test]
    fn volume_roles_are_distinct() {
        assert_ne!(VolumeRole::System, VolumeRole::LunaData);
        assert_ne!(VolumeRole::Swap, VolumeRole::External);
    }
}
