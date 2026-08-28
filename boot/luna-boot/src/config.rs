//! Boot configuration management
//!
//! TEMPORARY IMPLEMENTATION FORMAT
//! This is a minimal prototype configuration format for initial boot path testing.
//! The final Luna boot configuration format will be specified separately.

use alloc::string::String;
use alloc::vec::Vec;
use crate::error::{BootError, BootResult};
use crate::target::BootTarget;

/// Boot configuration (temporary format)
#[derive(Debug, Clone)]
pub struct BootConfig {
    /// Default boot target index
    pub default_target: usize,
    /// Available boot targets
    pub targets: Vec<BootTarget>,
    /// Boot timeout in milliseconds (0 = no timeout, immediate boot)
    pub boot_timeout_ms: u32,
    /// Whether to show boot messages
    pub verbose: bool,
}

impl BootConfig {
    /// Create a minimal default configuration
    pub fn default_config() -> Self {
        let mut targets = Vec::new();

        // Add a default system target
        targets.push(
            BootTarget::new("Luna System", "1.0.0", "\\kernels\\bzImage.efi")
                .with_cmdline("root=/dev/sda2 rw quiet")
        );

        // Add recovery target
        targets.push(
            BootTarget::new("Recovery", "1.0.0", "\\kernels\\bzImage-recovery.efi")
                .with_cmdline("root=/dev/sda2 rw recovery quiet")
                .recovery()
        );

        Self {
            default_target: 0,
            targets,
            boot_timeout_ms: 2000, // 2 second window for B key
            verbose: false,
        }
    }

    /// Parse configuration from a simple key-value format
    ///
    /// This is a temporary implementation. The real format will be specified
    /// in a future RFC.
    pub fn parse_simple(_content: &str) -> BootResult<Self> {
        // For now, return default config
        // Real implementation would parse actual config file
        Ok(Self::default_config())
    }

    /// Get the default boot target
    pub fn default_target(&self) -> BootResult<&BootTarget> {
        self.targets
            .get(self.default_target)
            .ok_or(BootError::NoBootTargets)
    }

    /// Get all available targets
    pub fn targets(&self) -> &[BootTarget] {
        &self.targets
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = BootConfig::default_config();
        assert!(!config.targets.is_empty());
        assert!(config.default_target() .is_ok());
    }

    #[test]
    fn test_config_has_recovery() {
        let config = BootConfig::default_config();
        let has_recovery = config.targets.iter().any(|t| t.is_recovery);
        assert!(has_recovery, "Config should have recovery target");
    }
}
