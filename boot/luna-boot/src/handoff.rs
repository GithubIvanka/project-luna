//! Final post-ExitBootServices CPU transition.

use core::arch::global_asm;

use crate::boot_params::BootParams;
use crate::error::{BootError, BootResult};
use crate::linux::LinuxSetupHeader;

// Keep this deliberately small and independent of UEFI. The Linux x86-64
// protocol enters with interrupts disabled, a flat 64-bit GDT, CR3 pointing
// at the page tables prepared by the loader, and RSI pointing at boot_params.
//
// Rust/LLVM's integrated assembler uses Intel syntax for this target. Do not
// use ELF-only directives such as `.type ...,@function`: the UEFI target is
// PE/COFF, not ELF.
global_asm!(r#"
    .section .text.luna_handoff,"ax"
    .global luna_linux_entry
luna_linux_entry:
    cli
    mov cr3, rdx
    lgdt [rip + luna_boot_gdt_ptr]

    // GDT: 0x08 = 64-bit code, 0x10 = data.
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax
    xor eax, eax
    mov fs, ax
    mov gs, ax

    // Enter the Linux 64-bit entry point with CS=0x08.
    // RDI = kernel entry, RSI = boot_params, RDX = CR3.
    mov rax, rdi
    push 0x08
    push rax
    retfq

    .align 8
luna_boot_gdt:
    .quad 0x0000000000000000
    // 64-bit code: present, ring 0, executable/readable, long mode.
    .quad 0x00af9a000000ffff
    // Data: present, ring 0, writable.
    .quad 0x00cf92000000ffff
luna_boot_gdt_ptr:
    .word 0x17
    .quad luna_boot_gdt
"#);

unsafe extern "sysv64" {
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

    /// Transfer control directly to the Linux 64-bit entry point. No UEFI
    /// service, allocator or logger may be touched after this call.
    pub unsafe fn enter(self) -> ! {
        unsafe { luna_linux_entry(self.kernel_entry, self.boot_params_address, self.page_table) }
    }
}

pub fn validate(handoff: &KernelHandoff) -> BootResult<()> {
    if handoff.is_ready() {
        Ok(())
    } else {
        Err(BootError::InvalidKernel)
    }
}
