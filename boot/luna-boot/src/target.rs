//! A resolved Luna boot target: System Image + luna-init + compatible Linux kernel.

use alloc::string::String;

#[derive(Debug, Clone)]
pub struct BootTarget {
    pub name: String,
    pub image_family: String,
    pub system_version: String,
    pub system_image_path: String,
    pub manifest_path: String,
    pub init_path: String,
    pub kernel_path: String,
    pub kernel_id: String,
    pub kernel_cmdline: String,
    pub is_recovery: bool,
    pub is_factory: bool,
}

impl BootTarget {
    pub fn new(
        name: impl Into<String>,
        image_family: impl Into<String>,
        system_version: impl Into<String>,
        system_image_path: impl Into<String>,
        manifest_path: impl Into<String>,
        init_path: impl Into<String>,
        kernel_path: impl Into<String>,
        kernel_id: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            image_family: image_family.into(),
            system_version: system_version.into(),
            system_image_path: system_image_path.into(),
            manifest_path: manifest_path.into(),
            init_path: init_path.into(),
            kernel_path: kernel_path.into(),
            kernel_id: kernel_id.into(),
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
