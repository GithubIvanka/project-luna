//! Identity page tables for the Linux 64-bit boot protocol.
//!
//! Linux requires the kernel runtime range, zero page and command line to be
//! identity mapped at entry. We build a loader-owned 2 MiB identity map before
//! ExitBootServices and extend it far enough to keep the final transition code
//! and stack accessible after CR3 is switched.

use core::ptr;
use uefi::boot::{self, AllocateType, MemoryType, PAGE_SIZE};

use crate::error::{BootError, BootResult};

const BASE_PD_COUNT: usize = 64; // Keep the normal mapping at 64 GiB.
const ENTRIES_PER_PD: usize = 512;
const PAGE_2M: u64 = 0x20_0000;
const PAGE_1G: u64 = 0x4000_0000;
const MAX_PML4_ZERO_REGION: u64 = 0x80_0000_0000; // 512 GiB.
const TRANSITION_CUSHION: u64 = PAGE_2M;

/// Build a low-address identity map large enough for the normal 64 GiB range
/// plus the final loader transition code and a 2 MiB stack cushion.
///
/// `entry_address` is the address of the assembly transition stub. `stack`
/// is the current RSP immediately before ExitBootServices. The returned page
/// count must be reserved in the final E820 map because the table allocation
/// remains live after Boot Services exit.
pub fn prepare_identity_map(entry_address: u64, stack: u64) -> BootResult<(u64, usize)> {
    let transition_end = entry_address
        .checked_add(TRANSITION_CUSHION)
        .ok_or(BootError::MemoryAllocationFailed)?;
    let stack_end = stack
        .checked_add(TRANSITION_CUSHION)
        .ok_or(BootError::MemoryAllocationFailed)?;
    let required_end = BASE_PD_COUNT as u64 * PAGE_1G.max(PAGE_2M * ENTRIES_PER_PD as u64)
        .max(transition_end)
        .max(stack_end);

    if required_end > MAX_PML4_ZERO_REGION {
        return Err(BootError::Unsupported(
            "loader transition address exceeds the low identity-map region",
        ));
    }

    let pd_count = BASE_PD_COUNT
        .max(required_end.div_ceil(PAGE_1G) as usize)
        .min(ENTRIES_PER_PD);
    let table_pages = 1 + 1 + pd_count;
    let allocation = boot::allocate_pages(
        AllocateType::MaxAddress(0xffff_ffff),
        MemoryType::LOADER_DATA,
        table_pages,
    )
    .map_err(|_| BootError::MemoryAllocationFailed)?;
    let base = allocation.as_ptr() as u64;

    unsafe { ptr::write_bytes(base as *mut u8, 0, table_pages * PAGE_SIZE); }

    let pml4 = base as *mut u64;
    let pdpt = (base + PAGE_SIZE as u64) as *mut u64;
    unsafe { pml4.add(0).write((base + PAGE_SIZE as u64) | 0x3); }

    for pd_index in 0..pd_count {
        let pd = base + (2 + pd_index) as u64 * PAGE_SIZE as u64;
        unsafe { pdpt.add(pd_index).write(pd | 0x3); }
        let pd_ptr = pd as *mut u64;
        for entry in 0..ENTRIES_PER_PD {
            let physical = (pd_index as u64 * ENTRIES_PER_PD as u64 + entry as u64) * PAGE_2M;
            unsafe { pd_ptr.add(entry).write(physical | 0x83); }
        }
    }

    Ok((base, table_pages))
}
