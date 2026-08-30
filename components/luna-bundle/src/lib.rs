//! Domain model for Luna bundles.
//!
//! This crate also exposes the RFC-0002 transport codec as `lbp`. The domain
//! model remains independent from the on-disk `.lbp` representation.

use std::collections::BTreeSet;
use std::fmt;
use luna_common::{BundleId, Version};

#[allow(clippy::collapsible_if)]
#[path = "lbp_v1.rs"]
pub mod lbp;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum BundleKind { Application, Component }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleMetadata { id: BundleId, version: Version, kind: BundleKind }
impl BundleMetadata {
    pub fn new(id: BundleId, version: Version, kind: BundleKind) -> Self { Self { id, version, kind } }
    pub fn id(&self) -> &BundleId { &self.id }
    pub fn version(&self) -> Version { self.version }
    pub fn kind(&self) -> BundleKind { self.kind }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleResource { logical_path: String, source_path: String }
impl BundleResource {
    pub fn new(logical_path: impl Into<String>, source_path: impl Into<String>) -> Self { Self { logical_path: logical_path.into(), source_path: source_path.into() } }
    pub fn logical_path(&self) -> &str { &self.logical_path }
    pub fn source_path(&self) -> &str { &self.source_path }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleManifest { metadata: BundleMetadata, resources: Vec<BundleResource> }
impl BundleManifest {
    pub fn new(metadata: BundleMetadata) -> Self { Self { metadata, resources: Vec::new() } }
    pub fn metadata(&self) -> &BundleMetadata { &self.metadata }
    pub fn resources(&self) -> &[BundleResource] { &self.resources }
    pub fn add_resource(&mut self, resource: BundleResource) { self.resources.push(resource); }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BundleError { EmptyIdentifier, EmptyResourcePath, DuplicateResource(String), InvalidLogicalPath(String), InvalidSourcePath(String) }
impl fmt::Display for BundleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self { Self::EmptyIdentifier => f.write_str("bundle identifier is empty"), Self::EmptyResourcePath => f.write_str("bundle resource path is empty"), Self::DuplicateResource(path) => write!(f, "duplicate bundle resource: {path}"), Self::InvalidLogicalPath(path) => write!(f, "invalid logical bundle path: {path}"), Self::InvalidSourcePath(path) => write!(f, "invalid bundle source path: {path}") }
    }
}
impl std::error::Error for BundleError {}

pub fn validate_manifest(manifest: &BundleManifest) -> Result<(), BundleError> {
    if manifest.metadata.id().as_str().trim().is_empty() { return Err(BundleError::EmptyIdentifier); }
    let mut paths = BTreeSet::new();
    for resource in manifest.resources() {
        let logical = resource.logical_path();
        let source = resource.source_path();
        if logical.trim().is_empty() || source.trim().is_empty() { return Err(BundleError::EmptyResourcePath); }
        if !logical.starts_with('/') || logical.ends_with('/') || logical.split('/').any(|part| part == "..") { return Err(BundleError::InvalidLogicalPath(logical.to_owned())); }
        if source.starts_with('/') || source.split('/').any(|part| part == "..") { return Err(BundleError::InvalidSourcePath(source.to_owned())); }
        if !paths.insert(logical.to_owned()) { return Err(BundleError::DuplicateResource(logical.to_owned())); }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_manifest, BundleError, BundleKind, BundleManifest, BundleMetadata, BundleResource};
    use luna_common::{BundleId, Version};

    fn manifest() -> BundleManifest {
        BundleManifest::new(BundleMetadata::new(BundleId::from("example.app"), Version::new(1, 0, 0), BundleKind::Application))
    }

    #[test]
    fn manifest_validates_unique_resources() {
        let mut manifest = manifest();
        manifest.add_resource(BundleResource::new("/bin/app", "resources/bin/app"));
        assert!(validate_manifest(&manifest).is_ok());
    }

    #[test]
    fn manifest_rejects_duplicate_logical_resources() {
        let mut manifest = manifest();
        manifest.add_resource(BundleResource::new("/bin/app", "one"));
        manifest.add_resource(BundleResource::new("/bin/app", "two"));
        assert!(matches!(validate_manifest(&manifest), Err(BundleError::DuplicateResource(_))));
    }

    #[test]
    fn manifest_rejects_path_traversal() {
        let mut manifest = manifest();
        manifest.add_resource(BundleResource::new("/bin/../secret", "secret"));
        assert!(matches!(validate_manifest(&manifest), Err(BundleError::InvalidLogicalPath(_))));
    }
}
