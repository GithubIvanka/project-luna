//! Linux x86_64 zero-page helpers.

use crate::e820::E820Entry;
use crate::error::{BootError, BootResult};

pub const BOOT_PARAMS_SIZE: usize = 4096;
pub const E820_MAX_ENTRIES: usize = 128;

#[derive(Clone)]
pub struct BootParams {
    bytes: [u8; BOOT_PARAMS_SIZE],
}

impl BootParams {
    pub const fn zeroed() -> Self { Self { bytes: [0; BOOT_PARAMS_SIZE] } }
    pub fn as_bytes(&self) -> &[u8] { &self.bytes }
    pub fn as_mut_bytes(&mut self) -> &mut [u8] { &mut self.bytes }

    pub fn copy_setup_header(&mut self, kernel: &[u8]) -> BootResult<()> {
        let start = 0x1f1;
        let header_len = *kernel.get(0x201).ok_or(BootError::InvalidKernel)? as usize;
        let end = 0x202usize.checked_add(header_len).ok_or(BootError::InvalidKernel)?;
        if end > kernel.len() || end > self.bytes.len() { return Err(BootError::InvalidKernel); }
        self.bytes[start..end].copy_from_slice(&kernel[start..end]);
        Ok(())
    }

    pub fn set_loader_type(&mut self, value: u8) { self.bytes[0x210] = value; }
    pub fn set_loadflags(&mut self, value: u8) { self.bytes[0x211] = value; }

    pub fn set_cmdline(&mut self, address: u64) -> BootResult<()> {
        if address > u32::MAX {
            self.bytes[0x0c8..0x0cc].copy_from_slice(&((address >> 32) as u32).to_le_bytes());
            self.bytes[0x228..0x22c].copy_from_slice(&(address as u32).to_le_bytes());
        } else {
            self.bytes[0x228..0x22c].copy_from_slice(&(address as u32).to_le_bytes());
        }
        Ok(())
    }

    pub fn set_ramdisk(&mut self, address: u64, size: u64) -> BootResult<()> {
        if address > u32::MAX || size > u32::MAX {
            self.bytes[0xc0..0xc4].copy_from_slice(&((address >> 32) as u32).to_le_bytes());
            self.bytes[0xc4..0xc8].copy_from_slice(&((size >> 32) as u32).to_le_bytes());
        }
        self.bytes[0x218..0x21c].copy_from_slice(&(address as u32).to_le_bytes());
        self.bytes[0x21c..0x220].copy_from_slice(&(size as u32).to_le_bytes());
        Ok(())
    }

    pub fn set_e820(&mut self, entries: &[E820Entry]) -> BootResult<()> {
        if entries.len() > E820_MAX_ENTRIES { return Err(BootError::Unsupported("too many E820 entries")); }
        self.bytes[0x1e8] = entries.len() as u8;
        const BASE: usize = 0x2d0;
        for (i, entry) in entries.iter().enumerate() {
            let p = BASE + i * 20;
            self.bytes[p..p + 8].copy_from_slice(&entry.addr.to_le_bytes());
            self.bytes[p + 8..p + 16].copy_from_slice(&entry.size.to_le_bytes());
            self.bytes[p + 16..p + 20].copy_from_slice(&entry.typ.to_le_bytes());
        }
        Ok(())
    }
}
