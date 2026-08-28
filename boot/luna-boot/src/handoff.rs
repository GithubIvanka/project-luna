//! Final handoff planning.
//!
//! This module deliberately contains no UEFI calls after `ExitBootServices`.
//! It is the data-only boundary between the UEFI loader and the Linux entry
//! routine.

use crate::boot_params::BootParams;
use crate::linux::LinuxSetupHeader;

pub struct KernelHandoff {
    pub kernel_load_address: u64,
    pub kernel_entry: u64,
    pub boot_params_address: u64,
    pub command_line_address: u64,
    pub command_line_size: usize,
    pub initrd_address: u64,
    pub initrd_size: usize,
    pub setup: LinuxSetupHeader,
    pub boot_params: BootParams,
}

impl KernelHandoff {
    pub fn new(setup: LinuxSetupHeader, boot_params: BootParams) -> Self {
        Self {
            kernel_load_address: 0,
            kernel_entry: 0,
            boot_params_address: 0,
            command_line_address: 0,
            command_line_size: 0,
            initrd_address: 0,
            initrd_size: 0,
            setup,
            boot_params,
        }
    }

    /// The actual assembly transition is intentionally not exposed until all
    /// memory allocations and protocol validation have completed.
    ///
    /// Linux requires 64-bit mode with paging enabled, disabled interrupts,
    /// flat boot GDT selectors, and RSI pointing at boot_params. A Rust call
    /// into an arbitrary physical address cannot provide those invariants.
    pub fn is_ready(&self) -> bool {
        self.kernel_load_address != 0
            && self.kernel_entry != 0
            && self.boot_params_address != 0
            && self.command_line_address != 0
    }
}
