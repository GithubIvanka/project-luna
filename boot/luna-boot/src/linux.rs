//! Linux x86_64 bzImage boot-protocol definitions.
//!
//! The offsets below follow the Linux x86 boot protocol. The loader uses the
//! 64-bit protocol directly; it does not rely on the EFI handover stub.

use crate::error::{BootError, BootResult};

const SETUP_SECTS: usize = 0x1f1;
const BOOT_FLAG: usize = 0x1fe;
const HEADER_MAGIC: usize = 0x202;
const VERSION: usize = 0x206;
const LOADFLAGS: usize = 0x211;
const INITRD_ADDR_MAX: usize = 0x22c;
const KERNEL_ALIGNMENT: usize = 0x230;
const RELOCATABLE: usize = 0x234;
const MIN_ALIGNMENT: usize = 0x235;
const XLOADFLAGS: usize = 0x236;
const CMDLINE_SIZE: usize = 0x238;
const PAYLOAD_OFFSET: usize = 0x248;
const PAYLOAD_LENGTH: usize = 0x24c;
const PREF_ADDRESS: usize = 0x258;
const INIT_SIZE: usize = 0x260;
const HANDOVER_OFFSET: usize = 0x264;
const MIN_HEADER: usize = 0x268;

const HDRS: u32 = 0x5372_6448;
const PROTOCOL_64BIT_MIN: u16 = 0x020c;

#[derive(Clone, Copy, Debug)]
pub struct LinuxSetupHeader {
    pub setup_sects: u8,
    pub boot_flag: u16,
    pub version: u16,
    pub loadflags: u8,
    pub kernel_alignment: u32,
    pub relocatable_kernel: bool,
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

fn u16_at(image: &[u8], off: usize) -> BootResult<u16> {
    let bytes = image.get(off..off + 2).ok_or(BootError::InvalidKernel)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn u32_at(image: &[u8], off: usize) -> BootResult<u32> {
    let bytes = image.get(off..off + 4).ok_or(BootError::InvalidKernel)?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn u64_at(image: &[u8], off: usize) -> BootResult<u64> {
    let bytes = image.get(off..off + 8).ok_or(BootError::InvalidKernel)?;
    Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
}

impl LinuxSetupHeader {
    pub fn parse(image: &[u8]) -> BootResult<Self> {
        if image.len() < MIN_HEADER {
            return Err(BootError::InvalidKernel);
        }
        if u32_at(image, HEADER_MAGIC)? != HDRS {
            return Err(BootError::InvalidKernel);
        }

        let version = u16_at(image, VERSION)?;
        if version < PROTOCOL_64BIT_MIN {
            return Err(BootError::Unsupported("Linux boot protocol < 2.12"));
        }

        let setup_sects = image[SETUP_SECTS];
        let boot_flag = u16_at(image, BOOT_FLAG)?;
        let loadflags = image[LOADFLAGS];
        let kernel_alignment = u32_at(image, KERNEL_ALIGNMENT)?;
        let relocatable_kernel = image[RELOCATABLE] != 0;
        let min_alignment = image[MIN_ALIGNMENT];
        let xloadflags = u16_at(image, XLOADFLAGS)?;
        let cmdline_size = u32_at(image, CMDLINE_SIZE)?;
        let initrd_addr_max = u32_at(image, INITRD_ADDR_MAX)?;
        let payload_offset = u32_at(image, PAYLOAD_OFFSET)?;
        let payload_length = u32_at(image, PAYLOAD_LENGTH)?;
        let pref_address = u64_at(image, PREF_ADDRESS)?;
        let init_size = u32_at(image, INIT_SIZE)?;
        let handover_offset = u32_at(image, HANDOVER_OFFSET)?;

        if boot_flag != 0xaa55 || !relocatable_kernel || init_size == 0 {
            return Err(BootError::InvalidKernel);
        }
        if kernel_alignment == 0 || !kernel_alignment.is_power_of_two() {
            return Err(BootError::InvalidKernel);
        }
        if payload_length == 0 {
            return Err(BootError::InvalidKernel);
        }
        let setup_size = Self::setup_size_from(setup_sects);
        let end = setup_size
            .checked_add(payload_offset as usize)
            .and_then(|v| v.checked_add(payload_length as usize))
            .ok_or(BootError::InvalidKernel)?;
        if end > image.len() {
            return Err(BootError::InvalidKernel);
        }

        Ok(Self {
            setup_sects,
            boot_flag,
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

    pub const fn setup_size_from(setup_sects: u8) -> usize {
        let sectors = if setup_sects == 0 { 4 } else { setup_sects as usize };
        (sectors + 1) * 512
    }

    pub const fn setup_size(&self) -> usize {
        Self::setup_size_from(self.setup_sects)
    }

    /// File offset of the protected-mode portion of a bzImage.
    pub const fn protected_mode_offset(&self) -> usize {
        self.setup_size()
    }

    /// The 64-bit kernel entry point is runtime_start + 0x200.
    pub const fn entry_offset(&self) -> usize {
        0x200
    }

    pub const fn supports_above_4g(&self) -> bool {
        self.xloadflags & (1 << 1) != 0
    }
}
