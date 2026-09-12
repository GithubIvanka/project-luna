//! Linux E820 data types used by luna-boot.

use uefi::boot::{self, AllocateType, MemoryType, PAGE_SIZE};

use crate::error::{BootError, BootResult};

const SETUP_E820_EXT: u32 = 1;
const SETUP_DATA_NODE_SIZE: usize = 16;
pub const MAX_E820_EXT_ENTRIES: usize = 4096;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct E820Entry {
    pub addr: u64,
    pub size: u64,
    pub typ: u32,
    pub reserved: u32,
}

/// Preallocated Linux `SETUP_E820_EXT` setup_data node.
///
/// The allocation must exist before ExitBootServices because the final EFI
/// memory map is only available after ExitBootServices. The node is filled
/// after the final map is obtained, then linked in front of Luna's own
/// setup_data node.
pub struct E820Extension {
    pub address: u64,
    pub allocation_pages: usize,
    capacity_entries: usize,
}

impl E820Extension {
    pub fn allocate() -> BootResult<Self> {
        let data_bytes = MAX_E820_EXT_ENTRIES
            .checked_mul(core::mem::size_of::<E820Entry>() - core::mem::size_of::<u32>())
            .ok_or(BootError::InvalidKernel)?;
        let total_bytes = SETUP_DATA_NODE_SIZE
            .checked_add(data_bytes)
            .ok_or(BootError::InvalidKernel)?;
        let pages = total_bytes
            .div_ceil(PAGE_SIZE)
            .max(1);
        let allocation = boot::allocate_pages(
            AllocateType::MaxAddress(0xffff_ffff),
            MemoryType::LOADER_DATA,
            pages,
        )
        .map_err(|_| BootError::MemoryAllocationFailed)?;
        let address = allocation.as_ptr() as u64;
        unsafe {
            core::ptr::write_bytes(address as *mut u8, 0, pages * PAGE_SIZE);
        }
        Ok(Self {
            address,
            allocation_pages: pages,
            capacity_entries: MAX_E820_EXT_ENTRIES,
        })
    }

    pub fn write_entry(&mut self, index: usize, entry: &E820Entry) -> BootResult<()> {
        if index >= self.capacity_entries {
            return Err(BootError::Unsupported("too many extended E820 entries"));
        }
        let offset = SETUP_DATA_NODE_SIZE
            .checked_add(index.checked_mul(20).ok_or(BootError::InvalidKernel)?)
            .ok_or(BootError::InvalidKernel)?;
        let node = self.address as *mut u8;
        unsafe {
            core::ptr::copy_nonoverlapping(
                entry.addr.to_le_bytes().as_ptr(),
                node.add(offset),
                8,
            );
            core::ptr::copy_nonoverlapping(
                entry.size.to_le_bytes().as_ptr(),
                node.add(offset + 8),
                8,
            );
            core::ptr::copy_nonoverlapping(
                entry.typ.to_le_bytes().as_ptr(),
                node.add(offset + 16),
                4,
            );
        }
        Ok(())
    }

    pub fn finalize(&mut self, entry_count: usize, next: u64) -> BootResult<()> {
        if entry_count > self.capacity_entries {
            return Err(BootError::Unsupported("too many extended E820 entries"));
        }
        let data_len = entry_count
            .checked_mul(20)
            .ok_or(BootError::InvalidKernel)?;
        if data_len > u32::MAX as usize {
            return Err(BootError::Unsupported("extended E820 data is too large"));
        }

        unsafe {
            let node = self.address as *mut u8;
            (node.add(0) as *mut u64).write(next);
            (node.add(8) as *mut u32).write(SETUP_E820_EXT);
            (node.add(12) as *mut u32).write(data_len as u32);
        }
        Ok(())
    }
}
