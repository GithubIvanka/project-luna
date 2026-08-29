//! Boot target policy.
//!
//! The physical `current`/manifest format remains an architecture item to be
//! formalized. Until that contract is finalized this module provides the
//! smallest deterministic test configuration: load the kernel directly from
//! `/boot/bzImage` on the ext4 `system` partition. System Image and initramfs
//! loading are intentionally disabled for this bring-up stage.

use alloc::vec;
use alloc::vec::Vec;

use crate::error::{BootError, BootResult};
use crate::target::BootTarget;

#[derive(Debug, Clone)]
pub struct BootConfig {
    pub default_target: usize,
    pub targets: Vec<BootTarget>,
}

impl BootConfig {
    pub fn default_config() -> Self {
        let targets = vec![
            BootTarget::new(
                "Luna Kernel Test",
                "test",
                "",
                "/boot/bzImage",
            )
            .with_cmdline("console=ttyS0 quiet"),
        ];
        Self { default_target: 0, targets }
    }

    pub fn default_target(&self) -> BootResult<&BootTarget> {
        self.targets.get(self.default_target).ok_or(BootError::NoBootTargets)
    }

    pub fn targets(&self) -> &[BootTarget] { &self.targets }
}
