//! Small GPT reader. Luna identifies partitions by GPT partition name while
//! carrying stable raw GPT GUID bytes in the boot handoff.

use alloc::string::String;
use alloc::vec;
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
    const GPT_MIN_HEADER_SIZE: usize = 92;
    const GPT_HEADER_SIZE_OFFSET: usize = 0x0c;
    const GPT_HEADER_CRC_OFFSET: usize = 0x10;
    const GPT_ENTRIES_LBA_OFFSET: usize = 0x48;
    const GPT_ENTRY_COUNT_OFFSET: usize = 0x50;
    const GPT_ENTRY_SIZE_OFFSET: usize = 0x54;
    const GPT_ENTRIES_CRC_OFFSET: usize = 0x58;

    let bs = device.block_size();
    if !(512..=4096).contains(&bs) || !bs.is_multiple_of(512) {
        return Err(BootError::Unsupported("unsupported GPT block size"));
    }

    let mut header = vec![0; bs as usize];
    device.read_at(
        GPT_HEADER_LBA
            .checked_mul(bs)
            .ok_or(BootError::FilesystemError)?,
        &mut header,
    )?;

    if &header[..8] != GPT_SIGNATURE {
        return Err(BootError::InvalidFilesystem);
    }

    let header_size = u32_at(&header, GPT_HEADER_SIZE_OFFSET) as usize;
    if !(GPT_MIN_HEADER_SIZE..=bs as usize).contains(&header_size) {
        return Err(BootError::InvalidFilesystem);
    }

    let stored_header_crc = u32_at(&header, GPT_HEADER_CRC_OFFSET);

    let mut header_for_crc = header[..header_size].to_vec();
    header_for_crc[GPT_HEADER_CRC_OFFSET..GPT_HEADER_CRC_OFFSET + 4].fill(0);

    if crc32_ieee(&header_for_crc) != stored_header_crc {
        return Err(BootError::InvalidFilesystem);
    }

    let mut disk_guid = [0u8; 16];
    disk_guid.copy_from_slice(&header[0x38..0x48]);

    let entries_lba = u64_at(&header, GPT_ENTRIES_LBA_OFFSET);
    let entry_count = u32_at(&header, GPT_ENTRY_COUNT_OFFSET);
    let entry_size = u32_at(&header, GPT_ENTRY_SIZE_OFFSET);

    if entry_count == 0
        || entry_count > 4096
        || !(128..=4096).contains(&entry_size)
        || !entry_size.is_power_of_two()
    {
        return Err(BootError::InvalidFilesystem);
    }

    let entry_count_usize = entry_count as usize;
    let entry_size_usize = entry_size as usize;

    let entries_bytes = entry_count_usize
        .checked_mul(entry_size_usize)
        .ok_or(BootError::FilesystemError)?;

    let entries_offset = entries_lba
        .checked_mul(bs)
        .ok_or(BootError::FilesystemError)?;

    entries_offset
        .checked_add(entries_bytes as u64)
        .ok_or(BootError::FilesystemError)?;

    let expected_entries_crc = u32_at(&header, GPT_ENTRIES_CRC_OFFSET);
    let entries_per_read = (bs as usize / entry_size_usize).max(1);

    let mut entries_crc = Crc32::new();
    let mut index = 0u32;
    let mut found = None;

    while index < entry_count {
        let count = (entry_count - index).min(entries_per_read as u32);

        let read_size = (count as usize)
            .checked_mul(entry_size_usize)
            .ok_or(BootError::FilesystemError)?;

        let relative_offset = (index as u64)
            .checked_mul(entry_size as u64)
            .ok_or(BootError::FilesystemError)?;

        let offset = entries_offset
            .checked_add(relative_offset)
            .ok_or(BootError::FilesystemError)?;

        let mut raw = vec![0; read_size];
        device.read_at(offset, &mut raw)?;

        entries_crc.update(&raw);

        if found.is_none() {
            for n in 0..count as usize {
                let start = n
                    .checked_mul(entry_size_usize)
                    .ok_or(BootError::FilesystemError)?;
                let end = start
                    .checked_add(entry_size_usize)
                    .ok_or(BootError::FilesystemError)?;
                let e = &raw[start..end];

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

                    found = Some(Partition {
                        first_lba: first,
                        last_lba: last,
                        partition_guid,
                        disk_guid,
                        label: partition_name(e),
                    });
                }
            }
        }

        index += count;
    }

    if entries_crc.finalize() != expected_entries_crc {
        return Err(BootError::InvalidFilesystem);
    }

    found.ok_or(BootError::TargetNotFound)
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

struct Crc32 {
    value: u32,
}

impl Crc32 {
    fn new() -> Self {
        Self { value: 0xffff_ffff }
    }

    fn update(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.value ^= byte as u32;

            for _ in 0..8 {
                if self.value & 1 != 0 {
                    self.value = (self.value >> 1) ^ 0xedb8_8320;
                } else {
                    self.value >>= 1;
                }
            }
        }
    }

    fn finalize(self) -> u32 {
        !self.value
    }
}

fn crc32_ieee(bytes: &[u8]) -> u32 {
    let mut crc = Crc32::new();
    crc.update(bytes);
    crc.finalize()
}

fn u32_at(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}
fn u64_at(b: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(b[off..off + 8].try_into().unwrap())
}
