//! Change-execution boundary for Project Luna.
//!
//! This crate executes update plans. Desired state remains owned by the
//! corresponding system/application/kernel manager.

use luna_common::{BundleId, Version};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateOperation {
    InstallSystemImage(Version),
    RemoveSystemImage(Version),
    InstallKernel(Version),
    RemoveKernel(Version),
    InstallApplication(BundleId, Version),
    RemoveApplication(BundleId, Version),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdatePlan {
    operations: Vec<UpdateOperation>,
}

impl UpdatePlan {
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }

    pub fn push(&mut self, operation: UpdateOperation) {
        self.operations.push(operation);
    }

    pub fn operations(&self) -> &[UpdateOperation] {
        &self.operations
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
}

impl Default for UpdatePlan {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct UpdateError(String);

impl UpdateError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for UpdateError {}

pub trait UpdateExecutor {
    fn execute(&mut self, plan: &UpdatePlan) -> Result<(), UpdateError>;
}

#[cfg(test)]
mod tests {
    use super::{UpdateOperation, UpdatePlan};
    use luna_common::{BundleId, Version};

    #[test]
    fn plan_preserves_requested_operations() {
        let mut plan = UpdatePlan::new();
        plan.push(UpdateOperation::InstallKernel(Version::new(9, 0, 0)));
        plan.push(UpdateOperation::InstallApplication(
            BundleId::from("example.app"),
            Version::new(2, 0, 0),
        ));

        assert_eq!(plan.operations().len(), 2);
        assert!(!plan.is_empty());
    }
}
