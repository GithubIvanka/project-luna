//! Explicit trusted system-resource profiles for application execution.
//!
//! A profile describes the logical System Image paths and access modes that
//! Luna intentionally makes available to an application runtime. It is a
//! value-level contract; physical mounting and kernel enforcement remain owned
//! by `luna-namespace`.

use crate::ResourceAccess;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProfile {
    name: String,
    resources: BTreeMap<String, BTreeSet<ResourceAccess>>,
}

impl RuntimeProfile {
    pub fn new(name: impl Into<String>) -> Result<Self, ProfileError> {
        let name = name.into();
        if name.is_empty() || name.chars().any(char::is_whitespace) {
            return Err(ProfileError::InvalidName);
        }
        Ok(Self {
            name,
            resources: BTreeMap::new(),
        })
    }

    /// Minimal trusted System Image view shared by native application runtimes.
    /// Application DATA, devices and named capabilities are not included here.
    pub fn minimal() -> Self {
        let mut profile = Self::new("minimal").expect("built-in profile name is valid");
        let readable_executable = [ResourceAccess::Read, ResourceAccess::Execute];
        profile
            .add_resource("/usr", readable_executable)
            .expect("built-in profile resource is valid");
        profile
            .add_resource("/lib", readable_executable)
            .expect("built-in profile resource is valid");
        profile
            .add_resource("/lib64", readable_executable)
            .expect("built-in profile resource is valid");
        profile
            .add_resource("/etc", [ResourceAccess::Read])
            .expect("built-in profile resource is valid");
        profile
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn resources(&self) -> impl Iterator<Item = (&str, &BTreeSet<ResourceAccess>)> {
        self.resources
            .iter()
            .map(|(path, access)| (path.as_str(), access))
    }

    pub fn add_resource<I>(
        &mut self,
        logical_path: impl Into<String>,
        access: I,
    ) -> Result<(), ProfileError>
    where
        I: IntoIterator<Item = ResourceAccess>,
    {
        let logical_path = logical_path.into();
        if !logical_path.starts_with('/')
            || logical_path == "/"
            || logical_path.contains("..")
            || logical_path.contains('\0')
            || logical_path.chars().any(char::is_whitespace)
        {
            return Err(ProfileError::InvalidResource(logical_path));
        }
        self.resources
            .insert(logical_path, access.into_iter().collect());
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
            Self::InvalidResource(path) => {
                write!(f, "runtime profile resource is invalid: {path}")
            }
        }
    }
}

impl std::error::Error for ProfileError {}

#[cfg(test)]
mod tests {
    use super::RuntimeProfile;
    use crate::ResourceAccess;

    #[test]
    fn minimal_profile_is_deterministic() {
        let profile = RuntimeProfile::minimal();
        let resources = profile.resources().collect::<Vec<_>>();
        assert_eq!(resources.len(), 4);
        assert_eq!(resources[0].0, "/etc");
        assert_eq!(resources[1].0, "/lib");
        assert_eq!(resources[2].0, "/lib64");
        assert_eq!(resources[3].0, "/usr");
        assert_eq!(
            resources[0].1.iter().copied().collect::<Vec<_>>(),
            vec![ResourceAccess::Read]
        );
    }

    #[test]
    fn profile_rejects_traversal_and_root() {
        let mut profile = RuntimeProfile::new("test").unwrap();
        assert!(
            profile
                .add_resource("usr/bin", [ResourceAccess::Read])
                .is_err()
        );
        assert!(
            profile
                .add_resource("/usr/../etc", [ResourceAccess::Read])
                .is_err()
        );
        assert!(profile.add_resource("/", [ResourceAccess::Read]).is_err());
    }

    #[test]
    fn duplicate_resources_replace_with_explicit_access() {
        let mut profile = RuntimeProfile::new("test").unwrap();
        profile
            .add_resource("/usr", [ResourceAccess::Read])
            .unwrap();
        profile
            .add_resource("/usr", [ResourceAccess::Read, ResourceAccess::Execute])
            .unwrap();
        let (_, access) = profile.resources().next().unwrap();
        assert!(access.contains(&ResourceAccess::Execute));
    }
}
