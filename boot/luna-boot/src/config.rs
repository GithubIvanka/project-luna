//! Boot configuration and target discovery policy.
//!
//! The production source of truth is the System Image manifest. This module
//! keeps only the small amount of policy needed before the manifest reader is
//! available: current target first, immutable factory target second.

use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{BootError, BootResult};
use crate::target::BootTarget;

#[derive(Debug, Clone)]
pub struct BootConfig {
    pub default_target: usize,
    pub targets: Vec<BootTarget>,
    pub verbose: bool,
}

impl BootConfig {
    /// Temporary in-memory fallback used until manifest discovery is wired to
    /// the ext4 reader. Paths deliberately use the final Luna naming scheme:
    /// Linux is a plain `bzImage`, not an EFI executable.
    pub fn default_config() -> Self {
        let targets = vec![
            BootTarget::new("Luna System", "1.0.0", "/kernels/bzImage")
                .with_cmdline("quiet luna.image=/images/luna-1.0.0.squashfs"),
            BootTarget::new("Factory", "factory", "/kernels/factory/bzImage")
                .with_cmdline("quiet luna.factory=1")
                .factory(),
        ];

        Self {
            default_target: 0,
            targets,
            verbose: false,
        }
    }

    pub fn parse_simple(_content: &str) -> BootResult<Self> {
        Err(BootError::Unsupported("legacy simple boot configuration"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_factory_fallback() {
        let config = BootConfig::default_config();
        assert!(config.default_target().is_ok());
        assert!(config.targets.iter().any(|target| target.is_factory));
        assert!(config.targets[0].kernel_path.ends_with("bzImage"));
    }
}
