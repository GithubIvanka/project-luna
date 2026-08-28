//! UEFI memory management

use alloc::vec::Vec;
use uefi::prelude::*;
use uefi::table::boot::{MemoryDescriptor, MemoryType};
use crate::error::{BootError, BootResult};

/// Memory map information for kernel
#[derive(Debug)]
pub struct MemoryMap {
    pub descriptors: Vec<MemoryDescriptor>,
    pub map_key: usize,
    pub descriptor_size: usize,
    pub descriptor_version: u32,
}

/// Get UEFI memory map
pub fn get_memory_map(boot_services: &BootServices) -> BootResult<MemoryMap> {
    let mut map_size = 0;
    let mut map_key = 0;
    let mut descriptor_size = 0;
    let mut descriptor_version = 0;

    // First call to get required size
    let _ = boot_services.memory_map_size();

    // Allocate buffer (over-allocate to be safe)
    let buffer_size = map_size + 2 * descriptor_size;
    let mut buffer = vec![0u8; buffer_size];

    // Get actual memory map
    let status = unsafe {
        boot_services.get_memory_map(
            &mut buffer,
            &mut map_size,
            &mut map_key,
            &mut descriptor_size,
            &mut descriptor_version,
        )
    };

    if status.is_err() {
        return Err(BootError::MemoryAllocationFailed);
    }

    // Parse memory descriptors
    let num_descriptors = map_size / descriptor_size;
    let mut descriptors = Vec::with_capacity(num_descriptors);

    for i in 0..num_descriptors {
        let offset = i * descriptor_size;
        let descriptor = unsafe {
            &*(buffer.as_ptr().add(offset) as *const MemoryDescriptor)
        };
        descriptors.push(*descriptor);
    }

    Ok(MemoryMap {
        descriptors,
        map_key,
        descriptor_size,
        descriptor_version,
    })
}

/// Exit UEFI boot services and return memory map
pub fn exit_boot_services(
    boot_services: BootServices,
    memory_map: &MemoryMap,
) -> BootResult<()> {
    // This is a destructive operation - after this, UEFI boot services are no longer available
    let _ = unsafe { boot_services.exit_boot_services(memory_map.map_key) };
    Ok(())
}
