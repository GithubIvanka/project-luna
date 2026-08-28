//! Linux kernel loading (x86_64 prototype)
//!
//! This is a minimal implementation for the first boot path.
//! Full Linux boot protocol support will be added later.

use alloc::vec::Vec;
use uefi::prelude::*;
use crate::error::{BootError, BootResult};
use crate::target::BootTarget;
use crate::filesystem::UefiFilesystem;
use crate::memory::MemoryMap;

/// Linux kernel loader
pub struct KernelLoader<'a> {
    boot_services: &'a BootServices,
    filesystem: &'a mut UefiFilesystem<'a>,
}

impl<'a> KernelLoader<'a> {
    pub fn new(
        boot_services: &'a BootServices,
        filesystem: &'a mut UefiFilesystem<'a>,
    ) -> Self {
        Self {
            boot_services,
            filesystem,
        }
    }

    /// Load kernel from filesystem
    pub fn load_kernel(&mut self, target: &BootTarget) -> BootResult<LoadedKernel> {
        log::info!("Loading kernel from: {}", target.kernel_path);

        // Read kernel file
        let kernel_data = self.filesystem.read_file(&target.kernel_path)?;

        if kernel_data.is_empty() {
            return Err(BootError::KernelLoadFailed);
        }

        // For prototype, we'll just store the data
        // Real implementation would parse bzImage format and set up boot_params
        log::info!("Kernel loaded: {} bytes", kernel_data.len());

        Ok(LoadedKernel {
            data: kernel_data,
            cmdline: target.kernel_cmdline.clone(),
        })
    }
}

/// Loaded kernel ready for execution
pub struct LoadedKernel {
    pub data: Vec<u8>,
    pub cmdline: alloc::string::String,
}

impl LoadedKernel {
    /// Jump to kernel entry point
    ///
    /// This is a placeholder. Real implementation would:
    /// 1. Set up boot_params structure
    /// 2. Set up command line
    /// 3. Pass memory map
    /// 4. Jump to kernel entry point
    pub fn boot(self, _memory_map: MemoryMap) -> ! {
        // In real implementation, this would jump to kernel
        // For now, we'll panic to indicate we reached this point
        log::info!("Kernel boot requested (not implemented)");

        // Infinite loop to prevent return
        loop {
            core::hint::spin_loop();
        }
    }
}
