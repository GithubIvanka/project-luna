//! Linux x86_64 bzImage loading and validation.
//!
//! This module owns the format-level part of the Linux boot protocol. It does
//! not call UEFI after the handoff boundary and does not pretend that reading a
//! bzImage is equivalent to executing it.

use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{BootError, BootResult};
use crate::linux::LinuxSetupHeader;
use crate::target::BootTarget;
use crate::filesystem::UefiFilesystem;

pub struct KernelLoader<'a> {
    filesystem: &'a mut UefiFilesystem<'a>,
}

impl<'a> KernelLoader<'a> {
    pub fn new(filesystem: &'a mut UefiFilesystem<'a>) -> Self {
        Self { filesystem }
    }

    pub fn load_kernel(&mut self, target: &BootTarget) -> BootResult<LoadedKernel> {
        let data = self.filesystem.read_file(&target.kernel_path)?;
        let header = LinuxSetupHeader::parse(&data)?;

        let setup_size = header.setup_size();
        if data.len() <= setup_size {
            return Err(BootError::InvalidKernel);
        }

        let payload_end = header
            .payload_offset
            .checked_add(header.payload_length)
            .ok_or(BootError::InvalidKernel)? as usize;
        if payload_end > data.len() || header.payload_offset as usize < setup_size {
            return Err(BootError::InvalidKernel);
        }

        Ok(LoadedKernel {
            image: data,
            header,
            cmdline: target.kernel_cmdline.clone(),
        })
    }
}

pub struct LoadedKernel {
    pub image: Vec<u8>,
    pub header: LinuxSetupHeader,
    pub cmdline: String,
}

impl LoadedKernel {
    pub fn setup_size(&self) -> usize { self.header.setup_size() }

    pub fn protected_mode_image(&self) -> BootResult<&[u8]> {
        let start = self.header.payload_offset as usize;
        let end = start
            .checked_add(self.header.payload_length as usize)
            .ok_or(BootError::InvalidKernel)?;
        self.image.get(start..end).ok_or(BootError::InvalidKernel)
    }
}
