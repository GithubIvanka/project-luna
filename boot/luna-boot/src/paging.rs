//! Minimal identity page tables for the Linux 64-bit boot protocol.
//!
//! Linux requires the kernel runtime range, zero page and command line to be
//! identity mapped at entry. We build a loader-owned 2 MiB identity map before
//! ExitBootServices and switch to it in the final assembly stub.

use core::ptr;
use uefi::boot::{self, AllocateType, MemoryType, PAGE_SIZE};

use crate::error::{BootError, BootResult};

const PD_COUNT: usize = 64; // 64 GiB of identity mapped RAM/MMIO address space.
const TABLE_PAGES: usize = 1 + 1 + PD_COUNT;

pub fn prepare_identity_map() -> BootResult<u64> {
    let allocation = boot::allocate_pages(
        AllocateType::MaxAddress(0xffff_ffff),
        MemoryType::LOADER_DATA,
        TABLE_PAGES,
    ).map_err(|_| BootError::MemoryAllocationFailed)?;
    let base = allocation.as_ptr() as u64;

    unsafe { ptr::write_bytes(base as *mut u8, 0, TABLE_PAGES * PAGE_SIZE); }

    let pml4 = base as *mut u64;
    let pdpt = (base + PAGE_SIZE as u64) as *mut u64;
    unsafe { pml4.add(0).write(base + PAGE_SIZE as u64 | 0x3); }

    for pd_index in 0..PD_COUNT {
        let pd = base + (2 + pd_index) as u64 * PAGE_SIZE as u64;
        unsafe { pdpt.add(pd_index).write(pd | 0x3); }
        let pd_ptr = pd as *mut u64;
        for entry in 0..512usize {
            let physical = (pd_index as u64 * 512 + entry as u64) * 0x20_0000;
            unsafe { pd_ptr.add(entry).write(physical | 0x83); }
        }
    }
    Ok(base)
}
