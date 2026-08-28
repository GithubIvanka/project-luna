//! Linux x86_64 bzImage boot-protocol definitions and validation.
//!
//! This module intentionally stops at the handoff boundary. The actual CPU
//! transition is kept in a tiny architecture-specific unsafe routine so that
//! parsing and validation remain testable.

use crate::error::{BootError, BootResult};

const SETUP_HEADER_OFFSET: usize = 0x1f1;
const HEADER_MAGIC_OFFSET: usize = 0x202;
const MIN_HEADER: usize = 0x268;
const HDRS: u32 = 0x5372_6448;
const PROTOCOL_64BIT_MIN: u16 = 0x020c;

#[derive(Clone, Copy, Debug)]
pub struct LinuxSetupHeader {
    pub setup_sects: u8,
    pub boot_flag: u16,
    pub header: u32,
    pub version: u16,
    pub loadflags: u8,
    pub kernel_alignment: u32,
    pub relocatable_kernel: u8,
    pub min_alignment: u8,
    pub xloadflags: u16,
    pub cmdline_size: u32,
    pub payload_offset: u32,
    pub payload_length: u32,
    pub initrd_addr_max: u32,
    pub pref_address: u64,
    pub init_size: u32,
    pub handover_offset: u32,
}

fn le16(image: &[u8], off: usize) -> BootResult<u16> {
    image.get(off..off + 2)
        .ok_or(BootError::InvalidKernel)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
}
fn le32(image: &[u8], off: usize) -> BootResult<u32> {
    image.get(off..off + 4)
        .ok_or(BootError::InvalidKernel)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn le64(image: &[u8], off: usize) -> BootResult<u64> {
    image.get(off..off + 8)
        .ok_or(BootError::InvalidKernel)
        .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
}

impl LinuxSetupHeader {
    pub fn parse(image: &[u8]) -> BootResult<Self> {
        if image.len() < MIN_HEADER {
            return Err(BootError::InvalidKernel);
        }
        if le32(image, HEADER_MAGIC_OFFSET)? != HDRS {
            return Err(BootError::InvalidKernel);
        }

        let version = le16(image, SETUP_HEADER_OFFSET + 0x12)?;
        if version < PROTOCOL_64BIT_MIN {
            return Err(BootError::Unsupported("Linux boot protocol < 2.12"));
        }

        let setup_sects = image[SETUP_HEADER_OFFSET];
        let boot_flag = le16(image, SETUP_HEADER_OFFSET + 1)?;
        let loadflags = image[SETUP_HEADER_OFFSET + 0x11];
        let kernel_alignment = le32(image, SETUP_HEADER_OFFSET + 0x3f)?;
        let relocatable_kernel = image[SETUP_HEADER_OFFSET + 0x44];
        let min_alignment = image[SETUP_HEADER_OFFSET + 0x45];
        let xloadflags = le16(image, SETUP_HEADER_OFFSET + 0x46)?;
        let cmdline_size = le32(image, SETUP_HEADER_OFFSET + 0x47)?;
        let initrd_addr_max = le32(image, SETUP_HEADER_OFFSET + 0x3b)?;
        let payload_offset = le32(image, SETUP_HEADER_OFFSET + 0x57)?;
        let payload_length = le32(image, SETUP_HEADER_OFFSET + 0x5b)?;
        let pref_address = le64(image, SETUP_HEADER_OFFSET + 0x67)?;
        let init_size = le32(image, SETUP_HEADER_OFFSET + 0x6f)?;
        let handover_offset = le32(image, SETUP_HEADER_OFFSET + 0x73)?;

        if boot_flag != 0xaa55 || relocatable_kernel == 0 || init_size == 0 {
            return Err(BootError::InvalidKernel);
        }
        if kernel_alignment == 0 || !kernel_alignment.is_power_of_two() {
            return Err(BootError::InvalidKernel);
        }
        if payload_length == 0 {
            return Err(BootError::InvalidKernel);
        }

        Ok(Self {
            setup_sects,
            boot_flag,
            header: HDRS,
            version,
            loadflags,
            kernel_alignment,
            relocatable_kernel,
            min_alignment,
            xloadflags,
            cmdline_size,
            payload_offset,
            payload_length,
            initrd_addr_max,
            pref_address,
            init_size,
            handover_offset,
        })
    }

    pub fn setup_size(&self) -> usize {
        let sectors = if self.setup_sects == 0 { 4 } else { self.setup_sects };
        (sectors as usize + 1) * 512
    }

    pub fn protected_mode_offset(&self) -> usize {
        self.setup_size()
    }
}
