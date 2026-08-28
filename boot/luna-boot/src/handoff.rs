//! Final post-ExitBootServices CPU transition.

use core::arch::global_asm;

use crate::boot_params::BootParams;
use crate::error::{BootError, BootResult};
use crate::linux::LinuxSetupHeader;

global_asm!(r#"
    .section .text.luna_handoff,"ax"
    .global luna_linux_entry
    .type luna_linux_entry,@function
luna_linux_entry:
    cli
    mov %rdx, %cr3
    lgdt luna_boot_gdt_ptr(%rip)
    mov $0x18, %ax
    mov %ax, %ds
    mov %ax, %es
    mov %ax, %ss
    xor %eax, %eax
    mov %ax, %fs
    mov %ax, %gs
    mov %rdi, %rax
    /* Linux 64-bit boot protocol: RSI = boot_params. */
    pushq $0x10
    pushq %rax
    lretq

    .align 8
luna_boot_gdt:
    .quad 0x0000000000000000
    .quad 0x00af9a000000ffff
    .quad 0x00cf92000000ffff
luna_boot_gdt_ptr:
    .word 0x17
    .quad luna_boot_gdt

    .size luna_linux_entry, .-luna_linux_entry
"#);

extern "sysv64" {
    fn luna_linux_entry(kernel_entry: u64, boot_params: u64, page_table: u64) -> !;
}

pub struct KernelHandoff {
    pub kernel_load_address: u64,
    pub kernel_entry: u64,
    pub boot_params_address: u64,
    pub command_line_address: u64,
    pub initrd_address: u64,
    pub initrd_size: usize,
    pub setup: LinuxSetupHeader,
    pub boot_params: BootParams,
    pub page_table: u64,
}

impl KernelHandoff {
    pub fn is_ready(&self) -> bool {
        self.kernel_load_address != 0
            && self.kernel_entry != 0
            && self.boot_params_address != 0
            && self.page_table != 0
    }

    /// No Rust/UEFI code may execute after this call returns: the assembly
    /// routine transfers control directly to the Linux 64-bit entry point.
    pub unsafe fn enter(self) -> ! {
        luna_linux_entry(self.kernel_entry, self.boot_params_address, self.page_table)
    }
}

pub fn validate(handoff: &KernelHandoff) -> BootResult<()> {
    if handoff.is_ready() { Ok(()) } else { Err(BootError::InvalidKernel) }
}
