//! Application lifecycle management backend for Project Luna.
//!
//! Installation, update, removal, verification, migration, and package import
//! belong here. Normal application execution belongs to app-runtime.

use luna_common::{BundleId, Version};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ApplicationRef {
    id: BundleId,
    version: Version,
}

impl ApplicationRef {
    pub const fn new(id: BundleId, version: Version) -> Self { Self { id, version } }
    pub const fn id(&self) -> &BundleId { &self.id }
    pub const fn version(&self) -> Version { self.version }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplicationOperation {
    Install(ApplicationRef),
    Update { from: ApplicationRef, to: ApplicationRef },
    Remove(ApplicationRef),
    Verify(ApplicationRef),
    Migrate { from: Version, to: Version, id: BundleId },
}

#[derive(Debug)]
pub struct AppManagerError(String);

impl AppManagerError {
    pub fn new(message: impl Into<String>) -> Self { Self(message.into()) }
}

impl std::fmt::Display for AppManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.0) }
}
impl std::error::Error for AppManagerError {}

pub trait ApplicationManager {
    fn execute(&mut self, operation: &ApplicationOperation) -> Result<(), AppManagerError>;
}

#[cfg(test)]
mod tests {
    use super::{ApplicationOperation, ApplicationRef};
    use luna_common::{BundleId, Version};

    #[test]
    fn application_operation_keeps_version_identity() {
        let app = ApplicationRef::new(BundleId::from("example.app"), Version::new(2, 0, 0));
        let op = ApplicationOperation::Install(app.clone());
        assert_eq!(op, ApplicationOperation::Install(app));
    }
}
