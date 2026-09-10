//! Linux x86_64 zero-page helpers.

use crate::e820::E820Entry;
use crate::error::{BootError, BootResult};

pub const BOOT_PARAMS_SIZE: usize = 4096;
pub const E820_MAX_ENTRIES: usize = 128;
pub const LOW_MEMORY_CMDLINE_MAX: u64 = 0x9f_fff;

const SETUP_HEADER_START: usize = 0x1f1;
const SETUP_HEADER_END: usize = 0x290;
const SETUP_DATA_OFFSET: usize = 0x250;
const LOADFLAGS_OFFSET: usize = 0x211;
const HEAP_END_PTR_OFFSET: usize = 0x224;
const CMDLINE_PTR_OFFSET: usize = 0x228;
const EXT_CMDLINE_PTR_OFFSET: usize = 0x0c8;
const CAN_USE_HEAP: u8 = 1 << 7;

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

    pub fn set_loadflags(&mut self, value: u8) { self.bytes[LOADFLAGS_OFFSET] = value; }

    /// Advertise the conventional setup heap required by modern Linux boot
    /// protocol loaders. The kernel setup code uses this flag when it needs
    /// temporary low-memory storage during early initialization.
    pub fn enable_setup_heap(&mut self) {
        self.bytes[LOADFLAGS_OFFSET] |= CAN_USE_HEAP;
        self.bytes[HEAP_END_PTR_OFFSET..HEAP_END_PTR_OFFSET + 2]
            .copy_from_slice(&0xde00u16.to_le_bytes());
    }

    pub fn set_cmdline(&mut self, address: u64) -> BootResult<()> {
        if address > LOW_MEMORY_CMDLINE_MAX {
            return Err(BootError::Unsupported("Linux x86_64 command line must reside below 0xA0000"));
        }
        self.bytes[EXT_CMDLINE_PTR_OFFSET..EXT_CMDLINE_PTR_OFFSET + 4].fill(0);
        self.bytes[CMDLINE_PTR_OFFSET..CMDLINE_PTR_OFFSET + 4]
            .copy_from_slice(&(address as u32).to_le_bytes());
        Ok(())
    }

    pub fn set_setup_data(&mut self, address: u64) -> BootResult<()> {
        if address > u32::MAX as u64 {
            return Err(BootError::Unsupported("luna setup_data must be below 4 GiB for Linux boot protocol compatibility"));
        }
        self.bytes[SETUP_DATA_OFFSET..SETUP_DATA_OFFSET + 8]
            .copy_from_slice(&address.to_le_bytes());
        Ok(())
    }

    pub fn set_e820(&mut self, entries: &[E820Entry]) -> BootResult<()> {
        if entries.len() > E820_MAX_ENTRIES { return Err(BootError::Unsupported("too many E820 entries")); }
        self.bytes[0x1e8] = entries.len() as u8;
        for (i, entry) in entries.iter().enumerate() { self.write_e820(i, entry); }
        Ok(())
    }

    pub fn set_e820_from_map_reserved(
        &mut self,
        map: &impl uefi::mem::memory_map::MemoryMap,
        reserved: &[(u64, usize)],
    ) -> BootResult<()> {
        let mut entries = [E820Entry { addr: 0, size: 0, typ: 0, reserved: 0 }; E820_MAX_ENTRIES];
        let mut count = 0usize;

        for d in map.entries() {
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

            let start = d.phys_start;
            let end = start.checked_add(size).ok_or(BootError::InvalidKernel)?;
            let mut segments = [(start, end); 16];
            let mut segment_count = 1usize;
            for &(rstart, rpages) in reserved {
                if rpages == 0 { continue; }
                let rend = rstart.checked_add((rpages as u64).saturating_mul(4096)).ok_or(BootError::InvalidKernel)?;
                let mut next = [(0u64, 0u64); 16];
                let mut next_count = 0usize;
                for &(s, e) in &segments[..segment_count] {
                    if next_count + 2 > next.len() { return Err(BootError::Unsupported("too many E820 reservation splits")); }
                    if rend <= s || rstart >= e {
                        next[next_count] = (s, e);
                        next_count += 1;
                    } else {
                        if s < rstart { next[next_count] = (s, rstart.min(e)); next_count += 1; }
                        let mid_start = s.max(rstart);
                        let mid_end = e.min(rend);
                        if mid_start < mid_end && typ != 2 {
                            next[next_count] = (mid_start, mid_end | (1u64 << 63));
                            next_count += 1;
                        } else if mid_start < mid_end {
                            next[next_count] = (mid_start, mid_end);
                            next_count += 1;
                        }
                        if rend < e { next[next_count] = (rend.max(s), e); next_count += 1; }
                    }
                }
                segment_count = next_count;
                for segment in segments.iter_mut().take(segment_count) {
                    segment.1 &= !(1u64 << 63);
                }
                for i in 0..segment_count {
                    if segments[i].0 < segments[i].1 && segments[i].0 >= rstart && segments[i].1 <= rend && typ != 2 {
                        segments[i].1 |= 1u64 << 63;
                    }
                }
            }

            for &(s, tagged_e) in &segments[..segment_count] {
                if s == tagged_e & !(1u64 << 63) { continue; }
                let is_reserved = tagged_e & (1u64 << 63) != 0;
                let e = tagged_e & !(1u64 << 63);
                if e <= s { continue; }
                if count == E820_MAX_ENTRIES { return Err(BootError::Unsupported("too many E820 entries")); }
                entries[count] = E820Entry { addr: s, size: e - s, typ: if is_reserved { 2 } else { typ }, reserved: 0 };
                count += 1;
            }
        }
        self.set_e820(&entries[..count])
    }

    fn write_e820(&mut self, index: usize, entry: &E820Entry) {
        let p = 0x2d0 + index * 20;
        self.bytes[p..p + 8].copy_from_slice(&entry.addr.to_le_bytes());
        self.bytes[p + 8..p + 16].copy_from_slice(&entry.size.to_le_bytes());
        self.bytes[p + 16..p + 20].copy_from_slice(&entry.typ.to_le_bytes());
    }
}
