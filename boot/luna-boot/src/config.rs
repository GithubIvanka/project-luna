//! Boot target policy.
//!
//! The physical `current`/manifest format remains an architecture item to be
//! formalized. Until that contract is finalized this module provides a small
//! deterministic test configuration with the final path layout.

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
                "Luna Test System",
                "test",
                "/images/luna-test.squashfs",
                "/kernels/test/bzImage",
            )
            .with_initrd("/kernels/test/initramfs.img")
            .with_cmdline("console=ttyS0 quiet luna.image=/images/luna-test.squashfs"),
            BootTarget::new(
                "Luna Factory",
                "factory",
                "/images/luna-factory.squashfs",
                "/kernels/factory/bzImage",
            )
            .with_initrd("/kernels/factory/initramfs.img")
            .with_cmdline("console=ttyS0 quiet luna.factory=1")
            .factory(),
        ];
        Self { default_target: 0, targets }
    }

    pub fn default_target(&self) -> BootResult<&BootTarget> {
        self.targets.get(self.default_target).ok_or(BootError::NoBootTargets)
    }
    pub fn targets(&self) -> &[BootTarget] { &self.targets }
}
