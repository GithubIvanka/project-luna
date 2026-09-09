// SPDX-License-Identifier: GPL-2.0
#![allow(unsafe_op_in_unsafe_fn)]

//! Project Luna direct-PID1 launcher.
//!
//! The Linux init task calls this Rust entry point after the normal early
//! kernel setup is complete. The launcher copies the boot-reserved `luna-init`
//! ELF bytes into the kernel's initial RAM-backed root and then delegates ELF
//! loading to Linux's existing `kernel_execve()` machinery.

use core::ffi::{c_char, c_void};

use kernel::bindings;

// Kbuild compiles this file as a standalone Rust crate/object.
const __LOG_PREFIX: &[u8] = b"luna_exec\0";

const O_WRONLY: i32 = 0x0001;
const O_CREAT: i32 = 0x0040;
const O_TRUNC: i32 = 0x0200;
const INIT_MODE: u32 = 0o700;
const MAX_ERRNO: isize = 4095;

unsafe extern "C" {
    fn filp_open(
        filename: *const c_char,
        flags: i32,
        mode: bindings::umode_t,
    ) -> *mut bindings::file;
    fn kernel_write(
        file: *mut bindings::file,
        buf: *const c_void,
        count: usize,
        pos: *mut bindings::loff_t,
    ) -> isize;
    fn fput(file: *mut bindings::file);
    fn kernel_execve(
        filename: *const c_char,
        argv: *const *const c_char,
        envp: *const *const c_char,
    ) -> i32;
}

unsafe extern "C" {
    fn x86_luna_boot_available() -> bool;
    fn x86_luna_init_phys() -> u64;
    fn x86_luna_init_size() -> u64;
}

#[inline]
fn is_err_ptr<T>(ptr: *mut T) -> bool {
    let value = ptr as isize;
    (-MAX_ERRNO..0).contains(&value)
}

#[inline]
fn ptr_err<T>(ptr: *mut T) -> i32 {
    ptr as isize as i32
}

unsafe fn copy_phys_to_file(
    file: *mut bindings::file,
    phys: u64,
    size: usize,
) -> Result<(), i32> {
    if size == 0 {
        return Err(-22); // -EINVAL
    }

    let mapped = bindings::early_memremap(
        phys as bindings::phys_addr_t,
        size,
    ) as *mut u8;
    if mapped.is_null() {
        return Err(-14); // -EFAULT
    }

    let mut offset = 0usize;
    let mut position: bindings::loff_t = 0;
    let result = loop {
        if offset == size {
            break Ok(());
        }

        let chunk = core::cmp::min(size - offset, 1024 * 1024);
        let written = kernel_write(
            file,
            mapped.add(offset).cast::<c_void>(),
            chunk,
            &mut position,
        );

        if written < 0 {
            break Err(written as i32);
        }
        if written as usize != chunk {
            break Err(-5); // -EIO
        }
        offset += chunk;
    };

    bindings::early_memunmap(mapped.cast::<c_void>(), size);
    result
}

unsafe fn stage_init_image() -> Result<(), i32> {
    let phys = x86_luna_init_phys();
    let size = x86_luna_init_size();

    if phys == 0 || size == 0 || size > isize::MAX as u64 {
        return Err(-22); // -EINVAL
    }

    let path = b"/luna-init\0";
    let file = filp_open(
        path.as_ptr().cast::<c_char>(),
        O_WRONLY | O_CREAT | O_TRUNC,
        INIT_MODE as bindings::umode_t,
    );
    if file.is_null() || is_err_ptr(file) {
        return Err(ptr_err(file));
    }

    let result = copy_phys_to_file(file, phys, size as usize);
    fput(file);
    result
}

/// Launch the bootloader-selected `luna-init` as the initial userspace task.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn x86_luna_exec_init() -> i32 {
    if !x86_luna_boot_available() {
        return -2; // -ENOENT
    }

    kernel::pr_info!("Luna: staging memory-resident luna-init for direct PID 1\n");

    if let Err(error) = stage_init_image() {
        kernel::pr_err!("Luna: failed to stage luna-init: error {}\n", error);
        return error;
    }

    static INIT_PATH: &[u8] = b"/luna-init\0";
    static ARG0: &[u8] = b"luna-init\0";
    static PATH_ENV: &[u8] = b"PATH=/bin:/sbin:/usr/bin:/usr/sbin\0";
    static DIRECT_ENV: &[u8] = b"LUNA_DIRECT_INIT=1\0";

    let argv: [*const c_char; 2] = [ARG0.as_ptr().cast::<c_char>(), core::ptr::null()];
    let envp: [*const c_char; 3] = [
        PATH_ENV.as_ptr().cast::<c_char>(),
        DIRECT_ENV.as_ptr().cast::<c_char>(),
        core::ptr::null(),
    ];

    kernel::pr_info!("Luna: executing luna-init as PID 1\n");
    let ret = kernel_execve(
        INIT_PATH.as_ptr().cast::<c_char>(),
        argv.as_ptr(),
        envp.as_ptr(),
    );

    kernel::pr_err!("Luna: kernel_execve(/luna-init) failed: error {}\n", ret);
    ret
}
