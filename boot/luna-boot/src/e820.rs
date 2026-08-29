//! Linux E820 conversion from the final UEFI memory map.

use alloc::vec::Vec;

use uefi::mem::memory_map::{MemoryMap, MemoryType};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct E820Entry {
    pub addr: u64,
    pub size: u64,
    pub typ: u32,
    pub reserved: u32,
}

pub const E820_RAM: u32 = 1;
pub const E820_RESERVED: u32 = 2;

pub fn from_uefi(map: &impl MemoryMap) -> Vec<E820Entry> {
    let mut entries = Vec::new();
    for d in map.entries() {
        let size = d.page_count.saturating_mul(4096);
        if size == 0 { continue; }
        let typ = match d.ty {
            MemoryType::CONVENTIONAL
            | MemoryType::BOOT_SERVICES_CODE
            | MemoryType::BOOT_SERVICES_DATA
            | MemoryType::LOADER_CODE
            | MemoryType::LOADER_DATA => E820_RAM,
            _ => E820_RESERVED,
        };
        entries.push(E820Entry { addr: d.phys_start, size, typ, reserved: 0 });
    }
    entries
}
