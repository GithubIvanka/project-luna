//! Storage discovery for Luna's ext4 system partition.

use crate::block::{parent_disk_handle, UefiBlockDevice};
use crate::error::BootResult;
use crate::ext4::Ext4;
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
}

/// Kept as a named boundary for callers that need to distinguish storage
/// discovery failures from a missing UEFI ESP. The system partition is not a
/// SimpleFileSystem volume and is never accessed through UEFI's FAT API.
pub fn validate_system_filesystem() -> BootResult<()> {
    let _ = SystemFilesystem::open()?;
    Ok(())
}
