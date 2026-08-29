//! Boot target policy.
//!
//! The physical `current`/manifest format remains an architecture item to be
//! formalized. Until that contract is finalized this module provides the
//! smallest deterministic test configuration: load the kernel and a temporary
//! test initramfs directly from the ext4 `system` partition. The initramfs is
//! only a bring-up aid; the real Luna System Image will be added later.

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
            .with_initrd("/boot/initramfs-test.img")
            .with_cmdline("console=ttyS0 quiet root=/dev/sda2 rootfstype=ext4 break=premount"),
        ];
        Self { default_target: 0, targets }
    }

    pub fn default_target(&self) -> BootResult<&BootTarget> {
        self.targets.get(self.default_target).ok_or(BootError::NoBootTargets)
    }

    pub fn targets(&self) -> &[BootTarget] { &self.targets }
}
