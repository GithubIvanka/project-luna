//! Conversion of UEFI memory descriptors into the Linux E820 zero-page map.

use crate::boot_params::{E820Entry, E820_MAX_ENTRIES};
use uefi::table::boot::{MemoryDescriptor, MemoryType};

pub const E820_RAM: u32 = 1;
pub const E820_RESERVED: u32 = 2;
pub const E820_ACPI: u32 = 3;
pub const E820_NVS: u32 = 4;
pub const E820_UNUSABLE: u32 = 5;

fn kind(memory_type: MemoryType) -> u32 {
    match memory_type {
        MemoryType::CONVENTIONAL
        | MemoryType::LOADER_CODE
        | MemoryType::LOADER_DATA
        | MemoryType::BOOT_SERVICES_CODE
        | MemoryType::BOOT_SERVICES_DATA => E820_RAM,
        MemoryType::ACPI_RECLAIM => E820_ACPI,
        MemoryType::ACPI_NON_VOLATILE => E820_NVS,
        MemoryType::UNUSABLE => E820_UNUSABLE,
        _ => E820_RESERVED,
    }
}

/// Convert descriptors without borrowing UEFI state beyond the call.
pub fn from_uefi<'a, I>(descriptors: I) -> ([E820Entry; E820_MAX_ENTRIES], usize)
where
    I: IntoIterator<Item = &'a MemoryDescriptor>,
{
    let mut out = [E820Entry { addr: 0, size: 0, type_: 0 }; E820_MAX_ENTRIES];
    let mut count = 0usize;

    for d in descriptors {
        if count == E820_MAX_ENTRIES {
            break;
        }
        let size = d.page_count.saturating_mul(4096);
        if size == 0 {
            continue;
        }
        out[count] = E820Entry {
            addr: d.phys_start,
            size,
            type_: kind(d.ty),
        };
        count += 1;
    }
    (out, count)
}
