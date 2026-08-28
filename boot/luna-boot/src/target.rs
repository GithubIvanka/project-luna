//! A resolved Luna boot target: System Image + compatible Linux kernel.

use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone)]
pub struct BootTarget {
    pub name: String,
    pub system_version: String,
    pub system_image_path: String,
    pub kernel_path: String,
    pub initrd_path: String,
    pub kernel_cmdline: String,
    pub is_recovery: bool,
    pub is_factory: bool,
}

impl BootTarget {
    pub fn new(name: impl Into<String>, system_version: impl Into<String>, system_image_path: impl Into<String>, kernel_path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            system_version: system_version.into(),
            system_image_path: system_image_path.into(),
            kernel_path: kernel_path.into(),
            initrd_path: String::new(),
            kernel_cmdline: String::new(),
            is_recovery: false,
            is_factory: false,
        }
    }
    pub fn with_initrd(mut self, path: impl Into<String>) -> Self { self.initrd_path = path.into(); self }
    pub fn with_cmdline(mut self, cmdline: impl Into<String>) -> Self { self.kernel_cmdline = cmdline.into(); self }
    pub fn recovery(mut self) -> Self { self.is_recovery = true; self }
    pub fn factory(mut self) -> Self { self.is_factory = true; self }
}

pub struct TargetManager { targets: Vec<BootTarget>, default_index: Option<usize> }
impl TargetManager {
    pub fn new() -> Self { Self { targets: Vec::new(), default_index: None } }
    pub fn add_target(&mut self, target: BootTarget) { self.targets.push(target); }
    pub fn set_default(&mut self, index: usize) { if index < self.targets.len() { self.default_index = Some(index); } }
    pub fn targets(&self) -> &[BootTarget] { &self.targets }
    pub fn default_target(&self) -> Option<&BootTarget> { self.default_index.and_then(|i| self.targets.get(i)) }
    pub fn select_target(&self, index: usize) -> Option<&BootTarget> { self.targets.get(index) }
    pub fn fallback_target(&self, failed_index: usize) -> Option<&BootTarget> {
        let failed = self.targets.get(failed_index)?;
        if failed.is_factory { return None; }
        self.targets.iter().find(|target| target.is_factory)
    }
    pub fn is_empty(&self) -> bool { self.targets.is_empty() }
    pub fn len(&self) -> usize { self.targets.len() }
}
impl Default for TargetManager { fn default() -> Self { Self::new() } }
