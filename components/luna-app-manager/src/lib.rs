//! Application lifecycle management backend for Project Luna.
//!
//! This crate plans and validates application lifecycle operations. Actual
//! mutation execution is delegated to `luna-update-manager`.

use luna_common::{BundleId, Version};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ApplicationRef {
    id: BundleId,
    version: Version,
}

impl ApplicationRef {
    pub const fn new(id: BundleId, version: Version) -> Self {
        Self { id, version }
    }
    pub const fn id(&self) -> &BundleId {
        &self.id
    }
    pub const fn version(&self) -> Version {
        self.version
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplicationOperation {
    Install(ApplicationRef),
    Update {
        from: ApplicationRef,
        to: ApplicationRef,
    },
    Remove(ApplicationRef),
    Verify(ApplicationRef),
    Migrate {
        from: Version,
        to: Version,
        id: BundleId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationPlan {
    operation: ApplicationOperation,
}

impl ApplicationPlan {
    pub const fn new(operation: ApplicationOperation) -> Self {
        Self { operation }
    }
    pub const fn operation(&self) -> &ApplicationOperation {
        &self.operation
    }
}

#[derive(Debug)]
pub struct AppManagerError(String);

impl AppManagerError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for AppManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for AppManagerError {}

pub trait ApplicationManager {
    fn plan(&self, operation: &ApplicationOperation) -> Result<ApplicationPlan, AppManagerError>;
    fn verify(&self, application: &ApplicationRef) -> Result<(), AppManagerError>;
}

#[cfg(test)]
mod tests {
    use super::{ApplicationOperation, ApplicationPlan, ApplicationRef};
    use luna_common::{BundleId, Version};

    #[test]
    fn plan_preserves_requested_operation_without_executing_it() {
        let app = ApplicationRef::new(BundleId::from("example.app"), Version::new(2, 0, 0));
        let operation = ApplicationOperation::Install(app.clone());
        let plan = ApplicationPlan::new(operation.clone());
        assert_eq!(plan.operation(), &operation);
    }
}
