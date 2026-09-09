// SPDX-License-Identifier: GPL-2.0
#![allow(unsafe_op_in_unsafe_fn)]

//! Project Luna kernel-side LunaBootHandoffV1 parser.
//!
//! This file is copied into the pinned Linux source tree by the Luna kernel
//! build tool. Parsing stays in Rust; the x86 C side only provides the Linux
//! setup_data head pointer and later consumes the exported state.

use core::ffi::c_void;
use core::ptr;

use kernel::bindings;

// Kbuild compiles this file as a standalone Rust crate/object. The kernel
// print macros expect this crate-root prefix to exist.
const __LOG_PREFIX: &[u8] = b"luna_boot\0";

const SETUP_DATA_TYPE: u32 = 0x4c55_4e41; // "LUNA"
const HANDOFF_MAGIC: u64 = ux64_from_bytes(*b"LUNAHD01");
const HANDOFF_MAJOR: u16 = 1;
const HANDOFF_HEADER_SIZE: usize = 84;
const HANDOFF_ALIGNED_HEADER_SIZE: usize = 88;
const HANDOFF_MAX_SIZE: usize = 64 * 1024;
const SETUP_DATA_NODE_SIZE: usize = 16;
const RECORD_HEADER_SIZE: usize = 8;
const RECORD_ALIGN: usize = 8;
const RECORD_SYSTEM_PARTITION: u16 = 1;
const RECORD_DATA_PARTITION: u16 = 2;
const RECORD_SYSTEM_IMAGE: u16 = 3;
const RECORD_KERNEL_IDENTITY: u16 = 4;
const RECORD_LUNA_INIT_IMAGE: u16 = 5;
const RECORD_BOOT_MODE: u16 = 6;
const RECORD_BOOT_STATE: u16 = 7;

static mut HANDOFF_PHYS: u64 = 0;
static mut HANDOFF_SIZE: u32 = 0;
static mut INIT_PHYS: u64 = 0;
static mut INIT_SIZE: u64 = 0;
static mut HANDOFF_VALID: bool = false;
static mut INIT_VALID: bool = false;

const fn ux64_from_bytes(bytes: [u8; 8]) -> u64 {
    u64::from_le_bytes(bytes)
}

#[inline]
unsafe fn read_u16(base: *const u8, offset: usize) -> u16 {
    u16::from_le(ptr::read_unaligned(base.add(offset).cast()))
}

#[inline]
unsafe fn read_u32(base: *const u8, offset: usize) -> u32 {
    u32::from_le(ptr::read_unaligned(base.add(offset).cast()))
}

#[inline]
unsafe fn read_u64(base: *const u8, offset: usize) -> u64 {
    u64::from_le(ptr::read_unaligned(base.add(offset).cast()))
}

#[inline]
fn range_ok(offset: usize, size: usize, total: usize) -> bool {
    offset <= total && size <= total - offset
}

#[inline]
unsafe fn clear_state() {
    HANDOFF_PHYS = 0;
    HANDOFF_SIZE = 0;
    INIT_PHYS = 0;
    INIT_SIZE = 0;
    HANDOFF_VALID = false;
    INIT_VALID = false;
}

#[inline]
unsafe fn unmap(base: *mut u8, len: usize) {
    bindings::early_memunmap(base.cast::<c_void>(), len);
}

unsafe fn reject(base: *mut u8, len: usize) {
    clear_state();
    unmap(base, len);
}

unsafe fn parse_handoff(phys: u64, node_len: u32) {
    clear_state();

    let len = node_len as usize;
    if !(HANDOFF_HEADER_SIZE..=HANDOFF_MAX_SIZE).contains(&len) {
        return;
    }
    if phys.checked_add(len as u64).is_none() {
        return;
    }

    let base = bindings::early_memremap(phys as bindings::phys_addr_t, len) as *mut u8;
    if base.is_null() {
        return;
    }

    let magic = read_u64(base, 0);
    let major = read_u16(base, 8);
    let header_size = read_u32(base, 12) as usize;
    let total_size = read_u32(base, 16) as usize;
    let records_offset = read_u64(base, 36) as usize;
    let records_size = read_u64(base, 44) as usize;

    if magic != HANDOFF_MAGIC
        || major != HANDOFF_MAJOR
        || header_size != HANDOFF_HEADER_SIZE
        || total_size != len
        || records_offset != HANDOFF_ALIGNED_HEADER_SIZE
        || records_offset % RECORD_ALIGN != 0
        || !range_ok(records_offset, records_size, len)
        || records_offset.checked_add(records_size).is_none()
    {
        reject(base, len);
        return;
    }

    let records_end = records_offset + records_size;
    let mut offset = records_offset;
    let mut init_seen = false;
    let mut system_seen = false;
    let mut data_seen = false;
    let mut image_seen = false;
    let mut kernel_seen = false;
    let mut mode_seen = false;
    let mut state_seen = false;

    while offset < records_end {
        if records_end - offset < RECORD_HEADER_SIZE {
            reject(base, len);
            return;
        }

        let record_type = read_u16(base, offset);
        let record_size = read_u32(base, offset + 4) as usize;
        let payload = match offset.checked_add(RECORD_HEADER_SIZE) {
            Some(value) => value,
            None => {
                reject(base, len);
                return;
            }
        };
        if !range_ok(payload, record_size, records_end) {
            reject(base, len);
            return;
        }

        match record_type {
            RECORD_SYSTEM_PARTITION => system_seen = true,
            RECORD_DATA_PARTITION => data_seen = true,
            RECORD_SYSTEM_IMAGE => image_seen = true,
            RECORD_KERNEL_IDENTITY => kernel_seen = true,
            RECORD_BOOT_MODE => {
                if record_size != 1 {
                    reject(base, len);
                    return;
                }
                mode_seen = true;
            }
            RECORD_BOOT_STATE => {
                if record_size != 24 {
                    reject(base, len);
                    return;
                }
                state_seen = true;
            }
            RECORD_LUNA_INIT_IMAGE => {
                if init_seen || record_size != 56 {
                    reject(base, len);
                    return;
                }
                let address = read_u64(base, payload);
                let size = read_u64(base, payload + 8);
                if address == 0 || size == 0 || address.checked_add(size).is_none() {
                    reject(base, len);
                    return;
                }
                INIT_PHYS = address;
                INIT_SIZE = size;
                init_seen = true;
            }
            _ => {}
        }

        let next = match payload
            .checked_add(record_size)
            .and_then(|value| value.checked_add(RECORD_ALIGN - 1))
        {
            Some(value) => value & !(RECORD_ALIGN - 1),
            None => {
                reject(base, len);
                return;
            }
        };
        if next <= offset || next > records_end {
            reject(base, len);
            return;
        }
        offset = next;
    }

    if !(system_seen && data_seen && image_seen && kernel_seen && init_seen && mode_seen && state_seen) {
        reject(base, len);
        return;
    }

    HANDOFF_PHYS = phys;
    HANDOFF_SIZE = len as u32;
    HANDOFF_VALID = true;
    INIT_VALID = true;

    kernel::pr_info!(
        "Luna: handoff v1 accepted: {} bytes, init {:#x}+{}\n",
        total_size,
        INIT_PHYS,
        INIT_SIZE
    );

    unmap(base, len);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn x86_luna_boot_parse(setup_data_phys: u64) {
    clear_state();
    let mut phys = setup_data_phys;

    for _ in 0..1024 {
        if phys == 0 {
            return;
        }

        let node = bindings::early_memremap(
            phys as bindings::phys_addr_t,
            SETUP_DATA_NODE_SIZE,
        ) as *mut u8;
        if node.is_null() {
            clear_state();
            return;
        }

        let next = read_u64(node, 0);
        let node_type = read_u32(node, 8);
        let node_len = read_u32(node, 12);
        unmap(node, SETUP_DATA_NODE_SIZE);

        if node_type == SETUP_DATA_TYPE {
            let Some(payload_phys) = phys.checked_add(SETUP_DATA_NODE_SIZE as u64) else {
                clear_state();
                return;
            };
            parse_handoff(payload_phys, node_len);
            return;
        }

        phys = next;
    }

    clear_state();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn x86_luna_boot_available() -> bool {
    HANDOFF_VALID && INIT_VALID
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn x86_luna_init_phys() -> u64 {
    INIT_PHYS
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn x86_luna_init_size() -> u64 {
    INIT_SIZE
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn x86_luna_handoff_phys() -> u64 {
    HANDOFF_PHYS
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn x86_luna_handoff_size() -> u32 {
    HANDOFF_SIZE
}
