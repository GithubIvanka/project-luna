// SPDX-License-Identifier: GPL-2.0
#![allow(unsafe_op_in_unsafe_fn)]

//! Project Luna direct-PID1 execution path.
//!
//! The bootloader supplies a validated `luna-init` ELF as a boot-reserved
//! physical memory object. This module keeps the Luna-specific policy in Rust:
//! it creates an anonymous kernel memory-backed executable object, copies the
//! exact bytes into it, and enters the Linux ELF/binfmt machinery through the
//! small kernel-internal file execution adapter.

use core::ffi::{c_char, c_void};

use kernel::bindings;

const __LOG_PREFIX: &[u8] = b"luna_exec\0";

const EINVAL: i32 = 22;
const EFAULT: i32 = 14;
const EIO: i32 = 5;
const ENOENT: i32 = 2;
const MAX_ERRNO: isize = 4095;

type KernelFile = c_void;

unsafe extern "C" {
    fn shmem_kernel_file_setup(
        name: *const c_char,
        size: bindings::loff_t,
        vma_flags: u64,
    ) -> *mut KernelFile;
    fn kernel_write(
        file: *mut KernelFile,
        buf: *const c_void,
        count: usize,
        pos: *mut bindings::loff_t,
    ) -> isize;
    fn fput(file: *mut KernelFile);
    fn kernel_execve_file(
        file: *mut KernelFile,
        argv: *const *const c_char,
        envp: *const *const c_char,
    ) -> i32;

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
    file: *mut KernelFile,
    phys: u64,
    size: usize,
) -> Result<(), i32> {
    if size == 0 {
        return Err(-EINVAL);
    }

    let mapped = bindings::early_memremap(phys as bindings::phys_addr_t, size) as *mut u8;
    if mapped.is_null() {
        return Err(-EFAULT);
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
            break Err(-EIO);
        }
        offset += chunk;
    };

    bindings::early_memunmap(mapped.cast::<c_void>(), size);
    result
}

unsafe fn build_executable_object() -> Result<*mut KernelFile, i32> {
    let phys = x86_luna_init_phys();
    let size = x86_luna_init_size();

    if phys == 0 || size == 0 || size > isize::MAX as u64 {
        return Err(-EINVAL);
    }

    let name = b"luna-init\0";
    let file = shmem_kernel_file_setup(
        name.as_ptr().cast::<c_char>(),
        size as bindings::loff_t,
        0,
    );
    if file.is_null() || is_err_ptr(file) {
        return Err(ptr_err(file));
    }

    if let Err(error) = copy_phys_to_file(file, phys, size as usize) {
        fput(file);
        return Err(error);
    }

    Ok(file)
}

/// Launch the bootloader-selected `luna-init` as the first userspace task.
///
/// The ELF is never staged into a filesystem pathname. The kernel execution
/// adapter consumes the anonymous memory-backed file and reuses Linux's normal
/// binfmt/ELF process construction.
#[unsafe(link_section = ".init.text")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn x86_luna_exec_init() -> i32 {
    if !x86_luna_boot_available() {
        return -ENOENT;
    }

    kernel::pr_info!("Luna: preparing memory-resident luna-init for direct PID 1\n");

    let file = match build_executable_object() {
        Ok(file) => file,
        Err(error) => {
            kernel::pr_err!("Luna: failed to prepare luna-init executable object: error {}\n", error);
            return error;
        }
    };

    static ARG0: &[u8] = b"luna-init\0";
    static PATH_ENV: &[u8] = b"PATH=/bin:/sbin:/usr/bin:/usr/sbin\0";
    static DIRECT_ENV: &[u8] = b"LUNA_DIRECT_INIT=1\0";

    let argv: [*const c_char; 2] = [ARG0.as_ptr().cast::<c_char>(), core::ptr::null()];
    let envp: [*const c_char; 3] = [
        PATH_ENV.as_ptr().cast::<c_char>(),
        DIRECT_ENV.as_ptr().cast::<c_char>(),
        core::ptr::null(),
    ];

    kernel::pr_info!("Luna: executing memory-resident luna-init as PID 1\n");
    let ret = kernel_execve_file(file, argv.as_ptr(), envp.as_ptr());

    fput(file);
    kernel::pr_err!("Luna: direct luna-init execution failed: error {}\n", ret);
    ret
}
