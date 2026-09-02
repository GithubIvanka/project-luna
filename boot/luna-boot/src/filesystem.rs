//! Storage discovery for Luna's ext4 SYSTEM partition.

use crate::block::{parent_disk_handle, UefiBlockDevice};
use crate::error::BootResult;
use crate::ext4::{DirEntry, Ext4};
use crate::gpt::find_system_partition;

pub struct SystemFilesystem {
    fs: Ext4<UefiBlockDevice>,
}

impl SystemFilesystem {
    pub fn open() -> BootResult<Self> {
        let disk = parent_disk_handle(uefi::boot::image_handle())?;
        let mut probe = UefiBlockDevice::new(disk, 0)?;
        let partition = find_system_partition(&mut probe)?;
        let device = UefiBlockDevice::new(disk, partition.first_lba)?;
        let fs = Ext4::open(device)?;
        Ok(Self { fs })
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
}

pub fn validate_system_filesystem() -> BootResult<()> {
    let _ = SystemFilesystem::open()?;
    Ok(())
}
