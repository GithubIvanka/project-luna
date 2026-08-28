//! Domain model for Luna bundles.
//!
//! This crate does not define the `.lbp` wire/archive representation. That
//! remains an RFC-0002 concern. The runtime/domain representation is separate
//! from its transport format.

use std::fmt;

use luna_common::{BundleId, Version};

/// Kind of bundle represented by the domain model.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum BundleKind {
    Application,
    Component,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleMetadata {
    id: BundleId,
    version: Version,
    kind: BundleKind,
}

impl BundleMetadata {
    pub fn new(id: BundleId, version: Version, kind: BundleKind) -> Self {
        Self { id, version, kind }
    }

    pub fn id(&self) -> &BundleId {
        &self.id
    }

    pub fn version(&self) -> Version {
        self.version
    }

    pub fn kind(&self) -> BundleKind {
        self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleResource {
    logical_path: String,
    source_path: String,
}

impl BundleResource {
    pub fn new(
        logical_path: impl Into<String>,
        source_path: impl Into<String>,
    ) -> Self {
        Self {
            logical_path: logical_path.into(),
            source_path: source_path.into(),
        }
    }

    pub fn logical_path(&self) -> &str {
        &self.logical_path
    }

    pub fn source_path(&self) -> &str {
        &self.source_path
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleManifest {
    metadata: BundleMetadata,
    resources: Vec<BundleResource>,
}

impl BundleManifest {
    pub fn new(metadata: BundleMetadata) -> Self {
        Self {
            metadata,
            resources: Vec::new(),
        }
    }

    pub fn metadata(&self) -> &BundleMetadata {
        &self.metadata
    }

    pub fn resources(&self) -> &[BundleResource] {
        &self.resources
    }

    pub fn add_resource(&mut self, resource: BundleResource) {
        self.resources.push(resource);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BundleError {
    EmptyIdentifier,
    EmptyResourcePath,
    DuplicateResource(String),
}

impl fmt::Display for BundleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIdentifier => f.write_str("bundle identifier is empty"),
            Self::EmptyResourcePath => f.write_str("bundle resource path is empty"),
            Self::DuplicateResource(path) => write!(f, "duplicate bundle resource: {path}"),
        }
    }
}

impl std::error::Error for BundleError {}

/// Validates the structural invariants that are independent of the transport
/// format. Format-specific validation remains in RFC-0002 implementation code.
pub fn validate_manifest(manifest: &BundleManifest) -> Result<(), BundleError> {
    if manifest.metadata.id().as_str().trim().is_empty() {
        return Err(BundleError::EmptyIdentifier);
    }

    let mut paths = std::collections::BTreeSet::new();
    for resource in manifest.resources() {
        if resource.logical_path().trim().is_empty()
            || resource.source_path().trim().is_empty()
        {
            return Err(BundleError::EmptyResourcePath);
        }

        if !paths.insert(resource.logical_path().to_owned()) {
            return Err(BundleError::DuplicateResource(
                resource.logical_path().to_owned(),
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        validate_manifest, BundleKind, BundleManifest, BundleMetadata, BundleResource,
    };
    use luna_common::{BundleId, Version};

    #[test]
    fn manifest_validates_unique_resources() {
        let metadata = BundleMetadata::new(
            BundleId::from("example.app"),
            Version::new(1, 0, 0),
            BundleKind::Application,
        );
        let mut manifest = BundleManifest::new(metadata);
        manifest.add_resource(BundleResource::new("/bin/app", "resources/bin/app"));

        assert!(validate_manifest(&manifest).is_ok());
    }

    #[test]
    fn manifest_rejects_duplicate_logical_resources() {
        let metadata = BundleMetadata::new(
            BundleId::from("example.app"),
            Version::new(1, 0, 0),
            BundleKind::Application,
        );
        let mut manifest = BundleManifest::new(metadata);
        manifest.add_resource(BundleResource::new("/bin/app", "one"));
        manifest.add_resource(BundleResource::new("/bin/app", "two"));

        assert!(validate_manifest(&manifest).is_err());
    }
}
