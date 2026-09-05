//! Explicit trusted system-resource profiles for application execution.
//!
//! A profile describes the logical System Image paths that Luna intentionally
//! makes available to an application runtime. It is a value-level contract;
//! physical mounting and kernel enforcement remain owned by `luna-namespace`.

use std::collections::BTreeSet;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProfile {
    name: String,
    resources: BTreeSet<String>,
}

impl RuntimeProfile {
    pub fn new(name: impl Into<String>) -> Result<Self, ProfileError> {
        let name = name.into();
        if name.is_empty() || name.chars().any(char::is_whitespace) {
            return Err(ProfileError::InvalidName);
        }
        Ok(Self {
            name,
            resources: BTreeSet::new(),
        })
    }

    /// Minimal trusted System Image view shared by native application runtimes.
    /// Application data, devices and capabilities are not included here.
    pub fn minimal() -> Self {
        let mut profile = Self::new("minimal").expect("built-in profile name is valid");
        for path in ["/usr", "/lib", "/lib64", "/etc"] {
            profile
                .add_resource(path)
                .expect("built-in profile resource is valid");
        }
        profile
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn resources(&self) -> impl Iterator<Item = &str> {
        self.resources.iter().map(String::as_str)
    }

    pub fn add_resource(&mut self, logical_path: impl Into<String>) -> Result<(), ProfileError> {
        let logical_path = logical_path.into();
        if !logical_path.starts_with('/')
            || logical_path.contains("..")
            || logical_path.contains('\0')
            || logical_path.chars().any(char::is_whitespace)
        {
            return Err(ProfileError::InvalidResource(logical_path));
        }
        self.resources.insert(logical_path);
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileError {
    InvalidName,
    InvalidResource(String),
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName => f.write_str("runtime profile name is invalid"),
            Self::InvalidResource(path) => write!(f, "runtime profile resource is invalid: {path}"),
        }
    }
}

impl std::error::Error for ProfileError {}

#[cfg(test)]
mod tests {
    use super::RuntimeProfile;

    #[test]
    fn minimal_profile_is_deterministic() {
        let profile = RuntimeProfile::minimal();
        let resources = profile.resources().collect::<Vec<_>>();
        assert_eq!(resources, vec!["/etc", "/lib", "/lib64", "/usr"]);
    }

    #[test]
    fn profile_rejects_traversal_and_relative_paths() {
        let mut profile = RuntimeProfile::new("test").unwrap();
        assert!(profile.add_resource("usr/bin").is_err());
        assert!(profile.add_resource("/usr/../etc").is_err());
    }

    #[test]
    fn duplicate_resources_collapse() {
        let mut profile = RuntimeProfile::new("test").unwrap();
        profile.add_resource("/usr").unwrap();
        profile.add_resource("/usr").unwrap();
        assert_eq!(profile.resources().count(), 1);
    }
}
