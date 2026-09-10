//! Small GPT reader. Luna identifies partitions by GPT partition name while
//! carrying stable raw GPT GUID bytes in the boot handoff.

use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{BootError, BootResult};
use crate::ext4::BlockDevice;

const GPT_HEADER_LBA: u64 = 1;
const GPT_SIGNATURE: &[u8; 8] = b"EFI PART";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Partition {
    pub first_lba: u64,
    pub last_lba: u64,
    /// Raw 16-byte GPT partition GUID encoding as stored on disk.
    pub partition_guid: [u8; 16],
    pub disk_guid: [u8; 16],
    /// UTF-8 form of the GPT partition name. This is carried only as a
    /// human/discovery label; GUIDs are the stable identity.
    pub label: String,
}

pub fn find_system_partition<D: BlockDevice>(device: &mut D) -> BootResult<Partition> {
    find_named_partition(device, "system")
}

pub fn find_data_partition<D: BlockDevice>(device: &mut D) -> BootResult<Partition> {
    find_named_partition(device, "data")
}

fn find_named_partition<D: BlockDevice>(device: &mut D, wanted: &str) -> BootResult<Partition> {
    let bs = device.block_size();
    if !(512..=4096).contains(&bs) || !bs.is_multiple_of(512) {
        return Err(BootError::Unsupported("unsupported GPT block size"));
    }

    let mut header = vec![0; bs as usize];
    device.read_at(GPT_HEADER_LBA * bs, &mut header)?;
    if &header[..8] != GPT_SIGNATURE {
        return Err(BootError::InvalidFilesystem);
    }

    let mut disk_guid = [0u8; 16];
    disk_guid.copy_from_slice(&header[0x38..0x48]);

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
        let mut raw = vec![0; count as usize * entry_size as usize];
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
            if partition_name_is(e, wanted) {
                let mut partition_guid = [0u8; 16];
                partition_guid.copy_from_slice(&e[16..32]);
                return Ok(Partition {
                    first_lba: first,
                    last_lba: last,
                    partition_guid,
                    disk_guid,
                    label: partition_name(e),
                });
            }
        }
        index += count;
    }
    Err(BootError::TargetNotFound)
}

fn partition_name_is(entry: &[u8], wanted: &str) -> bool {
    partition_name(entry).eq_ignore_ascii_case(wanted)
}

fn partition_name(entry: &[u8]) -> String {
    if entry.len() < 56 + 72 {
        return String::new();
    }
    let mut bytes = Vec::new();
    for i in 0..36usize {
        let p = 56 + i * 2;
        let code = u16::from_le_bytes([entry[p], entry[p + 1]]);
        if code == 0 {
            break;
        }
        if code <= 0x7f {
            bytes.push(code as u8);
        } else {
            bytes.extend_from_slice("?".as_bytes());
        }
    }
    String::from_utf8(bytes).unwrap_or_default()
}

fn u32_at(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}
fn u64_at(b: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(b[off..off + 8].try_into().unwrap())
}
