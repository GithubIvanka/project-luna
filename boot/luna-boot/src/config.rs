//! Boot target policy.
//!
//! The normal target is graphical and intentionally quiet. Diagnostics are a
//! boot-mode concern, not a second OS/serial target.

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
                "Luna PC System",
                "pc",
                "",
                "/kernels/default/bzImage",
            )
            .with_initrd("/kernels/default/initramfs.img")
            .with_cmdline(
                "console=tty0 quiet loglevel=3 root=/dev/ram0 ro rdinit=/init luna.system_image=/images/luna-0.1.0.squashfs luna.system_device=LABEL=LUNA-SYSTEM luna.data_device=LABEL=LUNA-DATA",
            ),
        ];
        Self {
            default_target: 0,
            targets,
        }
    }

    pub fn default_target(&self) -> BootResult<&BootTarget> {
        self.targets
            .get(self.default_target)
            .ok_or(BootError::NoBootTargets)
    }

    pub fn targets(&self) -> &[BootTarget] {
        &self.targets
    }
}
