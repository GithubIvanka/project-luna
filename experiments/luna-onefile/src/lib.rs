#![no_std]

use core::arch::asm;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        unsafe { asm!("pause", options(nomem, nostack, preserves_flags)); }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn luna_main() -> ! {
    static MESSAGE: &[u8] = b"LUNA-INIT: PID 1\nLUNA-INIT: Stage 0 one-file experiment\nLUNA-READY\n";

    unsafe {
        asm!(
            "syscall",
            in("rax") 1usize,
            in("rdi") 1usize,
            in("rsi") MESSAGE.as_ptr(),
            in("rdx") MESSAGE.len(),
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
    }

    loop {
        unsafe { asm!("pause", options(nomem, nostack, preserves_flags)); }
    }
}
