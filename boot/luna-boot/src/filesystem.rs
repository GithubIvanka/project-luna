//! Storage discovery for Luna's ext4 SYSTEM partition.

use crate::block::{parent_disk_handle, UefiBlockDevice};
use crate::error::BootResult;
use crate::ext4::{DirEntry, Ext4};
use crate::gpt::{find_data_partition, find_system_partition, Partition};

pub struct SystemFilesystem {
    fs: Ext4<UefiBlockDevice>,
    system_partition: Partition,
    data_partition: Partition,
}

impl SystemFilesystem {
    pub fn open() -> BootResult<Self> {
        let disk = parent_disk_handle(uefi::boot::image_handle())?;
        let mut probe = UefiBlockDevice::new(disk, 0, uefi::boot::get_image_file_system(disk).is_err() as u64)?;
        let system_partition = find_system_partition(&mut probe)?;
        let data_partition = find_data_partition(&mut probe)?;
        if system_partition.disk_guid != data_partition.disk_guid {
            return Err(crate::error::BootError::InvalidFilesystem);
        }
        let system_blocks = system_partition
            .last_lba
            .checked_sub(system_partition.first_lba)
            .and_then(|count| count.checked_add(1))
            .ok_or(crate::error::BootError::InvalidFilesystem)?;
        let device = UefiBlockDevice::new(disk, system_partition.first_lba, system_blocks)?;
        let fs = Ext4::open(device)?;
        Ok(Self { fs, system_partition, data_partition })
    }

    pub fn read_file(&mut self, path: &str) -> BootResult<alloc::vec::Vec<u8>> {
        self.fs.read_file(path)
    }

    pub fn read_dir(&mut self, path: &str) -> BootResult<alloc::vec::Vec<DirEntry>> {
        self.fs.read_dir(path)
    }

    pub fn file_exists(&mut self, path: &str) -> BootResult<bool> {
        self.fs.file_exists(path)
    }

    pub fn system_partition(&self) -> &Partition { &self.system_partition }
    pub fn data_partition(&self) -> &Partition { &self.data_partition }
}
