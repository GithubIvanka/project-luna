//! UEFI memory-map helpers.

use alloc::vec::Vec;

use uefi::boot;
use uefi::mem::memory_map::{MemoryMapOwned, MemoryType};

use crate::error::{BootError, BootResult};

/// Obtain a snapshot using uefi-rs' allocation-safe memory-map API.
///
/// The snapshot is only valid while boot services remain active. For the final
/// handoff use `boot::exit_boot_services`, whose returned map is the final map.
pub fn current_memory_map() -> BootResult<MemoryMapOwned> {
    boot::memory_map(MemoryType::LOADER_DATA).map_err(|_| BootError::UefiError(uefi::Status::ABORTED))
}

/// Copy only the scalar information needed by code that must not retain UEFI
/// references across the ExitBootServices boundary.
#[derive(Clone, Copy, Debug)]
pub struct MemoryRegion {
    pub start: u64,
    pub pages: u64,
    pub ty: MemoryType,
}

pub fn snapshot_regions(map: &MemoryMapOwned) -> Vec<MemoryRegion> {
    map.entries()
        .map(|d| MemoryRegion {
            start: d.phys_start,
            pages: d.page_count,
            ty: d.ty,
        })
        .collect()
}
