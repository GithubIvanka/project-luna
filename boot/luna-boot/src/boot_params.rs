//! Linux x86_64 zero-page structures used by the Luna kernel handoff.
//!
//! The layout is intentionally represented as byte arrays for fields that are
//! not needed yet. This avoids inventing a partial `struct boot_params` layout
//! that could silently drift from the Linux ABI.

use crate::error::{BootError, BootResult};

pub const BOOT_PARAMS_SIZE: usize = 4096;
pub const E820_MAX_ENTRIES: usize = 128;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct E820Entry {
    pub addr: u64,
    pub size: u64,
    pub type_: u32,
}

#[derive(Clone)]
pub struct BootParams {
    bytes: [u8; BOOT_PARAMS_SIZE],
}

impl BootParams {
    pub const fn zeroed() -> Self {
        Self { bytes: [0; BOOT_PARAMS_SIZE] }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    /// Copy the setup header from the kernel into the zero page.
    pub fn copy_setup_header(&mut self, kernel: &[u8]) -> BootResult<()> {
        let start = 0x1f1usize;
        let end_marker = *kernel.get(0x201).ok_or(BootError::InvalidKernel)? as usize;
        let end = 0x202usize.checked_add(end_marker).ok_or(BootError::InvalidKernel)?;
        if end > kernel.len() || end - start > self.bytes.len() - 0x1f1 {
            return Err(BootError::InvalidKernel);
        }
        self.bytes[0x1f1..end].copy_from_slice(&kernel[0x1f1..end]);
        Ok(())
    }

    /// Set the command-line pointer and size fields in the Linux header.
    pub fn set_cmdline(&mut self, physical_address: u32) {
        self.bytes[0x228..0x22c].copy_from_slice(&physical_address.to_le_bytes());
    }

    pub fn set_loader_type(&mut self, loader_type: u8) {
        self.bytes[0x210] = loader_type;
    }

    pub fn set_loadflags(&mut self, flags: u8) {
        self.bytes[0x211] = flags;
    }

    pub fn set_ramdisk(&mut self, address: u64, size: u64) -> BootResult<()> {
        if address > u32::MAX || size > u32::MAX {
            return Err(BootError::Unsupported("initrd above 4 GiB requires xloadflags support"));
        }
        self.bytes[0x218..0x21c].copy_from_slice(&(address as u32).to_le_bytes());
        self.bytes[0x21c..0x220].copy_from_slice(&(size as u32).to_le_bytes());
        Ok(())
    }
}
