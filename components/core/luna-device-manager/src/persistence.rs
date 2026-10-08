//! Durable user preferences for the Luna Volume Namespace.
//!
//! Only user-controlled metadata is persisted here. Runtime assignments,
//! including the currently active Windows drive letters, stay in memory.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{DriveLetter, VolumeAssignmentError, VolumeId, VolumeRegistry};

pub const VOLUME_REGISTRY_VERSION: u32 = 1;

#[derive(Debug)]
pub enum VolumeRegistryError {
    Io(std::io::Error),
    TomlDecode(toml::de::Error),
    TomlEncode(toml::ser::Error),
    UnsupportedVersion(u32),
    DuplicateVolumeId,
    EmptyVolumeId,
    InvalidPreferredLetter,
    Assignment(VolumeAssignmentError),
}

impl std::fmt::Display for VolumeRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "volume registry I/O error: {error}"),
            Self::TomlDecode(error) => write!(f, "volume registry parse error: {error}"),
            Self::TomlEncode(error) => write!(f, "volume registry serialization error: {error}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported volume registry version: {version}")
            }
            Self::DuplicateVolumeId => f.write_str("volume registry contains duplicate volume id"),
            Self::EmptyVolumeId => f.write_str("volume registry contains an empty volume id"),
            Self::InvalidPreferredLetter => {
                f.write_str("volume registry contains an invalid preferred drive letter")
            }
            Self::Assignment(error) => write!(f, "restore active volume assignment: {error}"),
        }
    }
}

impl std::error::Error for VolumeRegistryError {}

impl From<std::io::Error> for VolumeRegistryError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<toml::de::Error> for VolumeRegistryError {
    fn from(error: toml::de::Error) -> Self {
        Self::TomlDecode(error)
    }
}

impl From<toml::ser::Error> for VolumeRegistryError {
    fn from(error: toml::ser::Error) -> Self {
        Self::TomlEncode(error)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct RegistryDocument {
    version: u32,
    #[serde(default)]
    volumes: Vec<RegistryEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RegistryEntry {
    id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    native_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    preferred_letter: Option<String>,
}

impl VolumeRegistry {
    /// Loads only durable preferences. Active assignments are intentionally not
    /// restored because drive letters are runtime state and are re-evaluated
    /// when volumes are attached.
    pub fn load_preferences(path: impl AsRef<Path>) -> Result<Self, VolumeRegistryError> {
        let input = fs::read_to_string(path)?;
        let document: RegistryDocument = toml::from_str(&input)?;
        if document.version != VOLUME_REGISTRY_VERSION {
            return Err(VolumeRegistryError::UnsupportedVersion(document.version));
        }

        let mut registry = Self::new();
        let mut seen = std::collections::BTreeSet::new();

        for entry in document.volumes {
            if entry.id.is_empty() {
                return Err(VolumeRegistryError::EmptyVolumeId);
            }
            let id = VolumeId::from(entry.id);
            if !seen.insert(id.clone()) {
                return Err(VolumeRegistryError::DuplicateVolumeId);
            }

            if let Some(label) = entry.label {
                registry.set_label(&id, label);
            }
            if let Some(native_name) = entry.native_name {
                registry.set_native_name(&id, native_name);
            }
            if let Some(preferred_letter) = entry.preferred_letter {
                let mut chars = preferred_letter.chars();
                let value = match (chars.next(), chars.next()) {
                    (Some(value), None) => value,
                    _ => return Err(VolumeRegistryError::InvalidPreferredLetter),
                };
                let letter = DriveLetter::new(value)
                    .map_err(|_| VolumeRegistryError::InvalidPreferredLetter)?;
                registry
                    .set_preferred(&id, letter)
                    .map_err(|_| VolumeRegistryError::InvalidPreferredLetter)?;
            }
        }

        Ok(registry)
    }

    /// Persists only durable metadata and preferences. Runtime drive-letter
    /// assignments and active volume roles are deliberately excluded.
    pub fn save_preferences(&self, path: impl AsRef<Path>) -> Result<(), VolumeRegistryError> {
        let path = path.as_ref();
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;

        let mut ids = std::collections::BTreeSet::new();
        ids.extend(self.labels.keys().cloned());
        ids.extend(self.native_names.keys().cloned());
        ids.extend(self.preferred.keys().cloned());

        let volumes = ids
            .into_iter()
            .map(|id| RegistryEntry {
                label: self.labels.get(&id).cloned(),
                native_name: self.native_names.get(&id).cloned(),
                preferred_letter: self
                    .preferred
                    .get(&id)
                    .copied()
                    .map(|letter| letter.as_char().to_string()),
                id: id.into_string(),
            })
            .collect();

        let document = RegistryDocument {
            version: VOLUME_REGISTRY_VERSION,
            volumes,
        };
        let content = toml::to_string_pretty(&document)?;

        let temp_path = temporary_path(path);
        fs::write(&temp_path, content.as_bytes())?;
        if let Err(error) = fs::rename(&temp_path, path) {
            let _ = fs::remove_file(&temp_path);
            return Err(error.into());
        }
        Ok(())
    }
}

fn temporary_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("volume-registry.toml");
    path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::{VOLUME_REGISTRY_VERSION, VolumeRegistry, VolumeRegistryError};
    use crate::{DriveLetter, VolumeId};
    use std::fs;

    fn temp_registry_path(suffix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "luna-volume-registry-{}-{suffix}.toml",
            std::process::id()
        ))
    }

    #[test]
    fn saves_and_loads_only_durable_preferences() {
        let path = temp_registry_path("roundtrip");
        let _ = fs::remove_file(&path);

        let mut registry = VolumeRegistry::new();
        let id = VolumeId::from("partition-guid:backup");
        registry.set_label(&id, "Backup");
        registry.set_native_name(&id, "backup");
        registry
            .set_preferred(&id, DriveLetter::new('H').unwrap())
            .unwrap();

        registry.save_preferences(&path).unwrap();
        let loaded = VolumeRegistry::load_preferences(&path).unwrap();
        assert_eq!(loaded.label(&id), Some("Backup"));
        assert_eq!(loaded.native_name(&id), Some("backup"));
        assert_eq!(loaded.preferred(&id), Some(DriveLetter::new('H').unwrap()));
        assert_eq!(loaded.current_letter(&id), None);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains(&format!("version = {VOLUME_REGISTRY_VERSION}")));
        assert!(content.contains("partition-guid:backup"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_unsupported_version() {
        let path = temp_registry_path("version");
        let _ = fs::remove_file(&path);
        fs::write(&path, "version = 99\n").unwrap();
        assert!(matches!(
            VolumeRegistry::load_preferences(&path),
            Err(VolumeRegistryError::UnsupportedVersion(99))
        ));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_invalid_preferred_letter() {
        let path = temp_registry_path("letter");
        let _ = fs::remove_file(&path);
        fs::write(
            &path,
            "version = 1\n\n[[volumes]]\nid = \"volume-1\"\npreferred_letter = \"C\"\n",
        )
        .unwrap();
        assert!(matches!(
            VolumeRegistry::load_preferences(&path),
            Err(VolumeRegistryError::InvalidPreferredLetter)
        ));
        let _ = fs::remove_file(path);
    }
}
