//! UEFI filesystem access

use alloc::vec::Vec;
use uefi::prelude::*;
use uefi::proto::media::file::{
    Directory, File, FileAttribute, FileInfo, FileMode, FileSystemVolumeLabel,
};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::table::boot::{ScopedProtocol, SearchType};
use crate::error::{BootError, BootResult};

/// UEFI filesystem wrapper
pub struct UefiFilesystem<'a> {
    fs: ScopedProtocol<'a, SimpleFileSystem>,
}

impl<'a> UefiFilesystem<'a> {
    /// Open the first available filesystem
    pub fn open(boot_services: &'a BootServices) -> BootResult<Self> {
        let handle = boot_services
            .locate_handle_buffer(SearchType::ByProtocol(&SimpleFileSystem::GUID))
            .map_err(|_| BootError::FilesystemError)?;

        let first_handle = handle.first().ok_or(BootError::FilesystemError)?;

        let fs = boot_services
            .open_protocol_exclusive::<SimpleFileSystem>(*first_handle)
            .map_err(|_| BootError::FilesystemError)?;

        Ok(Self { fs })
    }

    /// Read a file into memory
    pub fn read_file(&mut self, path: &str) -> BootResult<Vec<u8>> {
        let mut root = self.fs.open_volume().map_err(|_| BootError::FilesystemError)?;

        let handle = root
            .open(path, FileMode::Read, FileAttribute::empty())
            .map_err(|_| BootError::FilesystemError)?;

        let mut file = match handle.into_type().map_err(|_| BootError::FilesystemError)? {
            File::Regular(f) => f,
            File::Dir(_) => return Err(BootError::FilesystemError),
        };

        // Get file info to determine size
        let mut info_buffer = vec![0; 128];
        let info = file
            .get_info::<FileInfo>(&mut info_buffer)
            .map_err(|_| BootError::FilesystemError)?;

        let file_size = info.file_size() as usize;

        // Read file content
        let mut buffer = vec![0; file_size];
        file.read(&mut buffer).map_err(|_| BootError::FilesystemError)?;

        Ok(buffer)
    }

    /// Check if a file exists
    pub fn file_exists(&mut self, path: &str) -> bool {
        let mut root = match self.fs.open_volume() {
            Ok(r) => r,
            Err(_) => return false,
        };

        root.open(path, FileMode::Read, FileAttribute::empty()).is_ok()
    }
}
