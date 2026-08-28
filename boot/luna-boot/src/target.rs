//! Resolved boot target: System Image + compatible Linux kernel.

use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone)]
pub struct BootTarget {
    pub name: String,
    pub system_version: String,
    pub kernel_path: String,
    pub kernel_cmdline: String,
    pub is_recovery: bool,
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

pub struct TargetManager {
    targets: Vec<BootTarget>,
    default_index: Option<usize>,
}

impl TargetManager {
    pub fn new() -> Self {
        Self { targets: Vec::new(), default_index: None }
    }

    pub fn add_target(&mut self, target: BootTarget) {
        self.targets.push(target);
    }

    pub fn set_default(&mut self, index: usize) {
        if index < self.targets.len() {
            self.default_index = Some(index);
        }
    }

    pub fn targets(&self) -> &[BootTarget] { &self.targets }

    pub fn default_target(&self) -> Option<&BootTarget> {
        self.default_index.and_then(|index| self.targets.get(index))
    }

    pub fn select_target(&self, index: usize) -> Option<&BootTarget> {
        self.targets.get(index)
    }

    /// The architecture permits only the immutable Factory pair as automatic
    /// fallback. We must never silently boot an arbitrary older system image.
    pub fn fallback_target(&self, failed_index: usize) -> Option<&BootTarget> {
        let failed = self.targets.get(failed_index)?;
        if failed.is_factory {
            return None;
        }
        self.targets.iter().find(|target| target.is_factory)
    }

    pub fn is_empty(&self) -> bool { self.targets.is_empty() }
    pub fn len(&self) -> usize { self.targets.len() }
}

impl Default for TargetManager {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_is_only_automatic_fallback() {
        let mut manager = TargetManager::new();
        manager.add_target(BootTarget::new("Current", "1.0.0", "/kernels/bzImage"));
        manager.add_target(BootTarget::new("Factory", "factory", "/kernels/factory/bzImage").factory());

        assert!(manager.fallback_target(0).unwrap().is_factory);
        assert!(manager.fallback_target(1).is_none());
    }
}
