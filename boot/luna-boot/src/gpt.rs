//! Small GPT reader. Luna identifies its system partition by GPT partition
//! name `system`, keeping the bootloader independent of host OS mount code.

use alloc::vec::Vec;

use crate::error::{BootError, BootResult};
use crate::ext4::BlockDevice;

const GPT_HEADER_LBA: u64 = 1;
const GPT_SIGNATURE: &[u8; 8] = b"EFI PART";

#[derive(Clone, Copy, Debug)]
pub struct Partition {
    pub first_lba: u64,
    pub last_lba: u64,
}

pub fn find_system_partition<D: BlockDevice>(device: &mut D) -> BootResult<Partition> {
    let bs = device.block_size();
    if bs < 512 || bs > 4096 || bs % 512 != 0 {
        return Err(BootError::Unsupported("unsupported GPT block size"));
    }

    let mut header = Vec::new();
    header.resize(bs as usize, 0);
    device.read_at(GPT_HEADER_LBA * bs, &mut header)?;
    if &header[..8] != GPT_SIGNATURE {
        return Err(BootError::InvalidFilesystem);
    }

    let entries_lba = u64_at(&header, 0x48);
    let entry_count = u32_at(&header, 0x50);
    let entry_size = u32_at(&header, 0x54);
    if entry_count == 0 || entry_count > 4096 || !(128..=4096).contains(&entry_size) || !entry_size.is_power_of_two() {
        return Err(BootError::InvalidFilesystem);
    }

    let entries_per_read = (bs as usize / entry_size as usize).max(1);
    let mut index = 0u32;
    while index < entry_count {
        let count = (entry_count - index).min(entries_per_read as u32);
        let mut raw = Vec::new();
        raw.resize(count as usize * entry_size as usize, 0);
        device.read_at(
            entries_lba * bs + index as u64 * entry_size as u64,
            &mut raw,
        )?;

        for n in 0..count as usize {
            let e = &raw[n * entry_size as usize..(n + 1) * entry_size as usize];
            if e[..16].iter().all(|b| *b == 0) {
                continue;
            }
            let first = u64_at(e, 32);
            let last = u64_at(e, 40);
            if first > last { continue; }
            if partition_name(e) == "system" {
                return Ok(Partition { first_lba: first, last_lba: last });
            }
        }
        index += count;
    }
    Err(BootError::TargetNotFound)
}

fn partition_name(entry: &[u8]) -> &str {
    // GPT names are UTF-16LE at offset 56. For the boot partition name used by
    // Luna we only need ASCII BMP characters, so decode without allocation.
    // A fixed static cannot safely expose decoded text, therefore this helper
    // recognizes exactly the canonical ASCII name.
    const NAME: [u16; 6] = [b's' as u16, b'y' as u16, b's' as u16, b't' as u16, b'e' as u16, b'm' as u16];
    if entry.len() < 68 { return ""; }
    for i in 0..NAME.len() {
        let p = 56 + i * 2;
        if u16::from_le_bytes([entry[p], entry[p + 1]]) != NAME[i] { return ""; }
    }
    "system"
}

fn u32_at(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}
fn u64_at(b: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(b[off..off + 8].try_into().unwrap())
}
