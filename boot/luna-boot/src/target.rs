//! Boot target abstraction

use alloc::string::String;
use alloc::vec::Vec;

/// Represents a boot target (system image + kernel combination)
#[derive(Debug, Clone)]
pub struct BootTarget {
    /// Display name for menu
    pub name: String,
    /// System image version
    pub system_version: String,
    /// Kernel path on boot filesystem
    pub kernel_path: String,
    /// Optional kernel command line parameters
    pub kernel_cmdline: String,
    /// Whether this is a recovery target
    pub is_recovery: bool,
    /// Whether this is the factory/default target
    pub is_factory: bool,
}

impl BootTarget {
    pub fn new(
        name: impl Into<String>,
        system_version: impl Into<String>,
        kernel_path: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            system_version: system_version.into(),
            kernel_path: kernel_path.into(),
            kernel_cmdline: String::new(),
            is_recovery: false,
            is_factory: false,
        }
    }

    pub fn with_cmdline(mut self, cmdline: impl Into<String>) -> Self {
        self.kernel_cmdline = cmdline.into();
        self
    }

    pub fn recovery(mut self) -> Self {
        self.is_recovery = true;
        self
    }

    pub fn factory(mut self) -> Self {
        self.is_factory = true;
        self
    }
}

/// Manages available boot targets and selection logic
pub struct TargetManager {
    targets: Vec<BootTarget>,
    default_index: Option<usize>,
}

impl TargetManager {
    pub fn new() -> Self {
        Self {
            targets: Vec::new(),
            default_index: None,
        }
    }

    pub fn add_target(&mut self, target: BootTarget) {
        self.targets.push(target);
    }

    pub fn set_default(&mut self, index: usize) {
        if index < self.targets.len() {
            self.default_index = Some(index);
        }
    }

    pub fn targets(&self) -> &[BootTarget] {
        &self.targets
    }

    pub fn default_target(&self) -> Option<&BootTarget> {
        self.default_index.and_then(|i| self.targets.get(i))
    }

    pub fn select_target(&self, index: usize) -> Option<&BootTarget> {
        self.targets.get(index)
    }

    /// Get next fallback target if primary fails
    pub fn fallback_target(&self, failed_index: usize) -> Option<&BootTarget> {
        // Try previous target, then factory
        if failed_index > 0 {
            self.targets.get(failed_index - 1)
        } else {
            // Look for factory target
            self.targets.iter().find(|t| t.is_factory)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    pub fn len(&self) -> usize {
        self.targets.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_target_creation() {
        let target = BootTarget::new("Luna 1.0", "1.0.0", "/kernels/8.2.0.efi")
            .with_cmdline("console=ttyS0");

        assert_eq!(target.name, "Luna 1.0");
        assert_eq!(target.system_version, "1.0.0");
        assert_eq!(target.kernel_path, "/kernels/8.2.0.efi");
        assert_eq!(target.kernel_cmdline, "console=ttyS0");
        assert!(!target.is_recovery);
        assert!(!target.is_factory);
    }

    #[test]
    fn test_target_manager_default() {
        let mut manager = TargetManager::new();
        manager.add_target(BootTarget::new("Luna 1.0", "1.0.0", "/kernels/8.0.0.efi"));
        manager.add_target(BootTarget::new("Luna 2.0", "2.0.0", "/kernels/8.2.0.efi"));

        assert!(manager.default_target().is_none());

        manager.set_default(1);
        assert_eq!(manager.default_target().unwrap().name, "Luna 2.0");
    }

    #[test]
    fn test_fallback_selection() {
        let mut manager = TargetManager::new();
        manager.add_target(BootTarget::new("Luna 1.0", "1.0.0", "/kernels/8.0.0.efi").factory());
        manager.add_target(BootTarget::new("Luna 2.0", "2.0.0", "/kernels/8.1.0.efi"));
        manager.add_target(BootTarget::new("Luna 3.0", "3.0.0", "/kernels/8.2.0.efi"));

        // Fallback from index 2 should be index 1
        let fallback = manager.fallback_target(2).unwrap();
        assert_eq!(fallback.name, "Luna 2.0");

        // Fallback from index 0 should be factory
        let fallback = manager.fallback_target(0).unwrap();
        assert!(fallback.is_factory);
    }
}
