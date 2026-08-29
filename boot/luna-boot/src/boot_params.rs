//! Linux x86_64 zero-page helpers.

use crate::e820::E820Entry;
use crate::error::{BootError, BootResult};

pub const BOOT_PARAMS_SIZE: usize = 4096;
pub const E820_MAX_ENTRIES: usize = 128;

// setup_header begins at 0x1f1. There is no one-byte header length at 0x201;
// deriving a copy length from that byte corrupts/truncates the zero page.
const SETUP_HEADER_START: usize = 0x1f1;
const SETUP_HEADER_END: usize = 0x290;

#[derive(Clone)]
pub struct BootParams { bytes: [u8; BOOT_PARAMS_SIZE] }

impl BootParams {
    pub const fn zeroed() -> Self { Self { bytes: [0; BOOT_PARAMS_SIZE] } }
    pub fn as_bytes(&self) -> &[u8] { &self.bytes }

    pub fn copy_setup_header(&mut self, kernel: &[u8]) -> BootResult<()> {
        if kernel.len() < SETUP_HEADER_END {
            return Err(BootError::InvalidKernel);
        }
        self.bytes[SETUP_HEADER_START..SETUP_HEADER_END]
            .copy_from_slice(&kernel[SETUP_HEADER_START..SETUP_HEADER_END]);
        Ok(())
    }

    pub fn set_loader_type(&mut self, value: u8) { self.bytes[0x210] = value; }
    pub fn set_loadflags(&mut self, value: u8) { self.bytes[0x211] = value; }

    pub fn set_cmdline(&mut self, address: u64) -> BootResult<()> {
        if address > u32::MAX as u64 {
            self.bytes[0x0c8..0x0cc].copy_from_slice(&((address >> 32) as u32).to_le_bytes());
        } else {
            self.bytes[0x0c8..0x0cc].fill(0);
        }
        self.bytes[0x228..0x22c].copy_from_slice(&(address as u32).to_le_bytes());
        Ok(())
    }

    pub fn set_ramdisk(&mut self, address: u64, size: u64) -> BootResult<()> {
        if address > u32::MAX as u64 || size > u32::MAX as u64 {
            self.bytes[0xc0..0xc4].copy_from_slice(&((address >> 32) as u32).to_le_bytes());
            self.bytes[0xc4..0xc8].copy_from_slice(&((size >> 32) as u32).to_le_bytes());
        } else {
            self.bytes[0xc0..0xc8].fill(0);
        }
        self.bytes[0x218..0x21c].copy_from_slice(&(address as u32).to_le_bytes());
        self.bytes[0x21c..0x220].copy_from_slice(&(size as u32).to_le_bytes());
        Ok(())
    }

    pub fn set_e820(&mut self, entries: &[E820Entry]) -> BootResult<()> {
        if entries.len() > E820_MAX_ENTRIES { return Err(BootError::Unsupported("too many E820 entries")); }
        self.bytes[0x1e8] = entries.len() as u8;
        for (i, entry) in entries.iter().enumerate() { self.write_e820(i, entry); }
        Ok(())
    }

    pub fn set_e820_from_map(&mut self, map: &impl uefi::mem::memory_map::MemoryMap) -> BootResult<()> {
        let mut count = 0usize;
        for d in map.entries() {
            if count == E820_MAX_ENTRIES { return Err(BootError::Unsupported("too many E820 entries")); }
            let size = d.page_count.saturating_mul(4096);
            if size == 0 { continue; }
            let typ = match d.ty {
                uefi::mem::memory_map::MemoryType::CONVENTIONAL
                | uefi::mem::memory_map::MemoryType::BOOT_SERVICES_CODE
                | uefi::mem::memory_map::MemoryType::BOOT_SERVICES_DATA
                | uefi::mem::memory_map::MemoryType::LOADER_CODE
                | uefi::mem::memory_map::MemoryType::LOADER_DATA => 1,
                _ => 2,
            };
            self.write_e820(count, &E820Entry { addr: d.phys_start, size, typ, reserved: 0 });
            count += 1;
        }
        self.bytes[0x1e8] = count as u8;
        Ok(())
    }

    fn write_e820(&mut self, index: usize, entry: &E820Entry) {
        let p = 0x2d0 + index * 20;
        self.bytes[p..p + 8].copy_from_slice(&entry.addr.to_le_bytes());
        self.bytes[p + 8..p + 16].copy_from_slice(&entry.size.to_le_bytes());
        self.bytes[p + 16..p + 20].copy_from_slice(&entry.typ.to_le_bytes());
    }
}
