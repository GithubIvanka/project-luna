//! Boot target policy.
//!
//! The bring-up target now follows the real Luna handoff contract: the
//! bootloader selects a kernel and early userspace from the SYSTEM partition,
//! then `luna-init` constructs the selected SquashFS root and attaches DATA.

use alloc::vec;
use alloc::vec::Vec;

use crate::error::{BootError, BootResult};
use crate::target::BootTarget;

#[derive(Debug, Clone)]
pub struct BootConfig { pub default_target: usize, pub targets: Vec<BootTarget> }

impl BootConfig {
    pub fn default_config() -> Self {
        let targets = vec![BootTarget::new("Luna Test System", "test", "", "/kernels/test/bzImage")
            .with_initrd("/kernels/test/initramfs.img")
            .with_cmdline("console=ttyS0 root=/dev/ram0 rw rdinit=/init luna.system_image=/images/luna-test.squashfs")];
        Self { default_target: 0, targets }
    }
    pub fn default_target(&self) -> BootResult<&BootTarget> { self.targets.get(self.default_target).ok_or(BootError::NoBootTargets) }
    pub fn targets(&self) -> &[BootTarget] { &self.targets }
}
