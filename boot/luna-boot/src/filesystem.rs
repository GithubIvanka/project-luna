//! Storage discovery for Luna's ext4 LUNA-SYS partition.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use uefi::boot;
use uefi::proto::media::block::BlockIO;

use crate::block::{UefiBlockDevice, parent_disk_handle};
use crate::error::{BootError, BootResult};
use crate::ext4::{DirEntry, Ext4};
use crate::gpt::{Partition, find_data_partitions, find_system_partition};

const LUNA_DATA_CONFIG: &str = "/config/luna-data.toml";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataStatus {
    Found,
    Missing,
    Ambiguous,
}

#[derive(Clone, Debug)]
pub struct DataConfig {
    pub preferred_disk_guid: Option<[u8; 16]>,
    pub preferred_partition_guid: Option<[u8; 16]>,
}

impl DataConfig {
    pub fn parse(bytes: &[u8]) -> BootResult<Self> {
        let text = core::str::from_utf8(bytes).map_err(|_| BootError::InvalidConfig)?;
        let mut section = "";
        let mut disk = None;
        let mut partition = None;

        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = &line[1..line.len() - 1];
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match (section, key.trim()) {
                ("data", "preferred_disk_guid") => {
                    let value = parse_string(value).ok_or(BootError::InvalidConfig)?;
                    disk = parse_optional_guid(&value)?;
                }
                ("data", "preferred_partition_guid") => {
                    let value = parse_string(value).ok_or(BootError::InvalidConfig)?;
                    partition = parse_optional_guid(&value)?;
                }
                _ => {}
            }
        }

        match (disk, partition) {
            (Some(_), Some(_)) | (None, None) => Ok(Self {
                preferred_disk_guid: disk,
                preferred_partition_guid: partition,
            }),
            _ => Err(BootError::InvalidConfig),
        }
    }
}

pub struct SystemFilesystem {
    fs: Ext4<UefiBlockDevice>,
    system_partition: Partition,
    data_partition: Option<Partition>,
    data_status: DataStatus,
}

impl SystemFilesystem {
    pub fn open() -> BootResult<Self> {
        let disk = parent_disk_handle(boot::image_handle())?;
        let mut probe = UefiBlockDevice::whole_disk(disk)?;
        let system_partition = find_system_partition(&mut probe)?;
        let system_blocks = system_partition
            .last_lba
            .checked_sub(system_partition.first_lba)
            .and_then(|count| count.checked_add(1))
            .ok_or(BootError::InvalidFilesystem)?;
        let device = UefiBlockDevice::new(disk, system_partition.first_lba, system_blocks)?;
        let mut fs = Ext4::open(device)?;

        let data_config = match fs.read_file(LUNA_DATA_CONFIG) {
            Ok(bytes) => Some(DataConfig::parse(&bytes)?),
            Err(_) => None,
        };
        let (data_partition, data_status) = discover_data_partition(data_config.as_ref())?;

        Ok(Self {
            fs,
            system_partition,
            data_partition,
            data_status,
        })
    }

    pub fn read_file(&mut self, path: &str) -> BootResult<alloc::vec::Vec<u8>> {
        self.fs.read_file(path)
    }

    pub fn hash_file(&mut self, path: &str) -> BootResult<[u8; 32]> {
        self.fs.hash_file(path)
    }

    pub fn read_dir(&mut self, path: &str) -> BootResult<alloc::vec::Vec<DirEntry>> {
        self.fs.read_dir(path)
    }

    pub fn file_exists(&mut self, path: &str) -> BootResult<bool> {
        self.fs.file_exists(path)
    }

    pub fn system_partition(&self) -> &Partition {
        &self.system_partition
    }

    pub fn data_partition(&self) -> Option<&Partition> {
        self.data_partition.as_ref()
    }

    pub fn data_status(&self) -> DataStatus {
        self.data_status
    }
}

fn discover_data_partition(
    config: Option<&DataConfig>,
) -> BootResult<(Option<Partition>, DataStatus)> {
    let handles = boot::find_handles::<BlockIO>().map_err(BootError::from)?;
    let mut candidates = Vec::new();

    for handle in handles {
        let Ok(mut disk) = UefiBlockDevice::whole_disk(handle) else {
            continue;
        };
        let Ok(partitions) = find_data_partitions(&mut disk) else {
            continue;
        };
        for partition in partitions {
            if !candidates.contains(&partition) {
                candidates.push(partition);
            }
        }
    }

    if let Some(config) = config
        && let (Some(disk_guid), Some(partition_guid)) =
            (config.preferred_disk_guid, config.preferred_partition_guid)
    {
        let matches: Vec<_> = candidates
            .iter()
            .filter(|partition| {
                partition.disk_guid == disk_guid && partition.partition_guid == partition_guid
            })
            .cloned()
            .collect();

        match matches.as_slice() {
            [partition] => {
                return Ok((Some(partition.clone()), DataStatus::Found));
            }
            [] => {}
            _ => {
                return Ok((None, DataStatus::Ambiguous));
            }
        }
    }

    match candidates.as_slice() {
        [] => Ok((None, DataStatus::Missing)),
        [partition] => Ok((Some(partition.clone()), DataStatus::Found)),
        _ => Ok((None, DataStatus::Ambiguous)),
    }
}

fn parse_string(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        Some(value[1..value.len() - 1].to_string())
    } else {
        None
    }
}

fn parse_optional_guid(value: &str) -> BootResult<Option<[u8; 16]>> {
    if value.is_empty() {
        return Ok(None);
    }
    parse_guid(value).map(Some).ok_or(BootError::InvalidConfig)
}

fn parse_guid(value: &str) -> Option<[u8; 16]> {
    let bytes = value.as_bytes();
    if bytes.len() != 36
        || bytes[8] != b'-'
        || bytes[13] != b'-'
        || bytes[18] != b'-'
        || bytes[23] != b'-'
    {
        return None;
    }

    let mut canonical = [0u8; 16];
    let mut out = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if matches!(i, 8 | 13 | 18 | 23) {
            i += 1;
            continue;
        }
        let hi = hex(bytes[i])?;
        let lo = hex(bytes[i + 1])?;
        canonical[out] = (hi << 4) | lo;
        out += 1;
        i += 2;
    }

    let mut raw = canonical;
    raw[0..4].reverse();
    raw[4..6].reverse();
    raw[6..8].reverse();
    Some(raw)
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
