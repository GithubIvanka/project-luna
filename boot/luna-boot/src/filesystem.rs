//! EFI System Partition filesystem access.
//!
//! This wrapper is intentionally limited to the filesystem containing the
//! bootloader itself. The production `system` partition is ext4 and is handled
//! by `ext4.rs` over Block I/O; we must never pick an arbitrary filesystem.

use alloc::vec::Vec;

use uefi::prelude::*;
use uefi::proto::loaded_image::LoadedImage;
use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::table::boot::ScopedProtocol;

use crate::error::{BootError, BootResult};

pub struct UefiFilesystem<'a> {
    fs: ScopedProtocol<'a, SimpleFileSystem>,
}

impl<'a> UefiFilesystem<'a> {
    /// Open the Simple File System belonging to the image that launched us.
    pub fn open(boot_services: &'a BootServices, image_handle: Handle) -> BootResult<Self> {
        let loaded_image = boot_services
            .open_protocol_exclusive::<LoadedImage>(image_handle)
            .map_err(|_| BootError::FilesystemError)?;

        let device_handle = loaded_image
            .device()
            .ok_or(BootError::FilesystemError)?;

        let fs = boot_services
            .open_protocol_exclusive::<SimpleFileSystem>(device_handle)
            .map_err(|_| BootError::FilesystemError)?;

        Ok(Self { fs })
    }

    pub fn read_file(&mut self, path: &str) -> BootResult<Vec<u8>> {
        let mut root = self.fs.open_volume().map_err(|_| BootError::FilesystemError)?;
        let handle = root
            .open(path, FileMode::Read, FileAttribute::empty())
            .map_err(|_| BootError::FilesystemError)?;

        let mut file = match handle.into_type().map_err(|_| BootError::FilesystemError)? {
            File::Regular(file) => file,
            File::Dir(_) => return Err(BootError::FilesystemError),
        };

        let mut info_buffer = vec![0u8; 512];
        let info = file
            .get_info::<FileInfo>(&mut info_buffer)
            .map_err(|_| BootError::FilesystemError)?;
        let file_size = usize::try_from(info.file_size()).map_err(|_| BootError::FilesystemError)?;

        let mut buffer = vec![0u8; file_size];
        let read = file.read(&mut buffer).map_err(|_| BootError::FilesystemError)?;
        if read != file_size {
            return Err(BootError::FilesystemError);
        }
        Ok(buffer)
    }

    pub fn file_exists(&mut self, path: &str) -> bool {
        let mut root = match self.fs.open_volume() {
            Ok(root) => root,
            Err(_) => return false,
        };
        root.open(path, FileMode::Read, FileAttribute::empty()).is_ok()
    }
}
