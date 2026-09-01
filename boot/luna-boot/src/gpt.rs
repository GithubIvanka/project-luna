//! Small GPT reader. Luna identifies its system partition by GPT partition
//! name `SYSTEM`, keeping the bootloader independent of host OS mount code.

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
        device.read_at(entries_lba * bs + index as u64 * entry_size as u64, &mut raw)?;

        for n in 0..count as usize {
            let e = &raw[n * entry_size as usize..(n + 1) * entry_size as usize];
            if e[..16].iter().all(|b| *b == 0) {
                continue;
            }
            let first = u64_at(e, 32);
            let last = u64_at(e, 40);
            if first > last {
                continue;
            }
            if partition_name_is_system(e) {
                return Ok(Partition { first_lba: first, last_lba: last });
            }
        }
        index += count;
    }
    Err(BootError::TargetNotFound)
}

fn partition_name_is_system(entry: &[u8]) -> bool {
    if entry.len() < 68 {
        return false;
    }
    const NAME: &[u8] = b"system";
    for (i, expected) in NAME.iter().enumerate() {
        let p = 56 + i * 2;
        let mut actual = entry[p];
        if actual.is_ascii_uppercase() {
            actual = actual.to_ascii_lowercase();
        }
        if actual != *expected || entry[p + 1] != 0 {
            return false;
        }
    }
    true
}

fn u32_at(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}
fn u64_at(b: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(b[off..off + 8].try_into().unwrap())
}
