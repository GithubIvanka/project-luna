//! Identity page tables for the Linux 64-bit boot protocol.
//!
//! Linux requires the kernel runtime range, zero page and command line to be
//! identity mapped at entry. We build loader-owned 2 MiB identity mappings
//! before ExitBootServices and select a 4-level or 5-level root matching the
//! paging mode already active on the CPU.

use core::arch::asm;
use core::ptr;
use uefi::boot::{self, AllocateType, MemoryType, PAGE_SIZE};

use crate::error::{BootError, BootResult};

const BASE_PD_COUNT: usize = 64; // Keep the normal mapping at 64 GiB.
const ENTRIES_PER_TABLE: usize = 512;
const PAGE_2M: u64 = 0x20_0000;
const PAGE_1G: u64 = 0x4000_0000;
const MAX_PML4_ZERO_REGION: u64 = 0x80_0000_0000; // 512 GiB.
const TRANSITION_CUSHION: u64 = PAGE_2M;
const CR4_LA57: u64 = 1 << 12;

/// Build a low-address identity map large enough for the normal 64 GiB range
/// plus the final transition code and a 2 MiB stack cushion.
///
/// If the firmware entered us with CR4.LA57 set, the returned CR3 value is a
/// PML5 root whose first entry points at the same PML4 used by the 4-level
/// mapping. Otherwise the PML4 itself is returned. This keeps the CPU's current
/// paging mode unchanged while ensuring the new CR3 has the correct shape.
///
/// `entry_address` is the address of the assembly transition stub. `stack` is
/// the current RSP immediately before ExitBootServices. The returned page
/// count must be reserved in the final E820 map because all allocated table
/// pages remain live after Boot Services exit.
pub fn prepare_identity_map(entry_address: u64, stack: u64) -> BootResult<(u64, usize)> {
    let transition_end = entry_address
        .checked_add(TRANSITION_CUSHION)
        .ok_or(BootError::MemoryAllocationFailed)?;
    let stack_end = stack
        .checked_add(TRANSITION_CUSHION)
        .ok_or(BootError::MemoryAllocationFailed)?;
    let base_end = BASE_PD_COUNT as u64 * PAGE_1G;
    let required_end = base_end.max(transition_end).max(stack_end);

    if required_end > MAX_PML4_ZERO_REGION {
        return Err(BootError::Unsupported(
            "loader transition address exceeds the low identity-map region",
        ));
    }

    let pd_count = BASE_PD_COUNT
        .max(required_end.div_ceil(PAGE_1G) as usize)
        .min(ENTRIES_PER_TABLE);
    let use_five_level = current_cr4() & CR4_LA57 != 0;
    let pml5_pages = usize::from(use_five_level);
    let table_pages = 1 + pml5_pages + 1 + pd_count;

    let allocation = boot::allocate_pages(
        AllocateType::MaxAddress(0xffff_ffff),
        MemoryType::LOADER_DATA,
        table_pages,
    )
    .map_err(|_| BootError::MemoryAllocationFailed)?;
    let base = allocation.as_ptr() as u64;

    unsafe {
        ptr::write_bytes(base as *mut u8, 0, table_pages * PAGE_SIZE);
    }

    let (pml4, pdpt) = if use_five_level {
        let pml5 = base as *mut u64;
        let pml4 = (base + PAGE_SIZE as u64) as *mut u64;
        let pdpt = (base + 2 * PAGE_SIZE as u64) as *mut u64;
        unsafe {
            pml5.add(0).write((pml4 as u64) | 0x3);
        }
        (pml4, pdpt)
    } else {
        let pml4 = base as *mut u64;
        let pdpt = (base + PAGE_SIZE as u64) as *mut u64;
        (pml4, pdpt)
    };

    unsafe {
        pml4.add(0).write((pdpt as u64) | 0x3);
    }

    let pd_base = if use_five_level { 3 } else { 2 };
    for pd_index in 0..pd_count {
        let pd = base + (pd_base + pd_index) as u64 * PAGE_SIZE as u64;
        unsafe {
            pdpt.add(pd_index).write(pd | 0x3);
        }
        let pd_ptr = pd as *mut u64;
        for entry in 0..ENTRIES_PER_TABLE {
            let physical = (pd_index as u64 * ENTRIES_PER_TABLE as u64 + entry as u64) * PAGE_2M;
            unsafe {
                pd_ptr.add(entry).write(physical | 0x83);
            }
        }
    }

    Ok((base, table_pages))
}

#[inline]
fn current_cr4() -> u64 {
    let value: u64;
    unsafe {
        asm!("mov {}, cr4", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}
