//! Minimal read-only ext4 reader for the Luna system partition.
//!
//! The bootloader deliberately implements only the filesystem operations it
//! needs: locating regular files by absolute path and reading them into RAM.
//! It does not journal, write, or replay ext4 metadata.

use crate::error::{BootError, BootResult};

pub const EXT4_SUPER_MAGIC: u16 = 0xEF53;
const SUPERBLOCK_OFFSET: usize = 1024;
const SUPERBLOCK_SIZE: usize = 1024;

#[derive(Clone, Copy, Debug)]
pub struct Ext4Geometry {
    pub block_size: u32,
    pub inode_size: u16,
    pub blocks_per_group: u32,
    pub inodes_per_group: u32,
    pub inode_count: u32,
    pub first_data_block: u32,
}

/// A block-device abstraction intentionally independent from UEFI protocols.
pub trait BlockDevice {
    fn block_size(&self) -> u64;
    fn read_at(&mut self, offset: u64, dst: &mut [u8]) -> BootResult<()>;
}

/// Read-only ext4 filesystem.
///
/// Directory traversal and extent mapping are kept behind small helpers so
/// that the UEFI layer never needs to understand on-disk ext4 structures.
pub struct Ext4<D> {
    device: D,
    geometry: Ext4Geometry,
}

impl<D: BlockDevice> Ext4<D> {
    pub fn open(mut device: D) -> BootResult<Self> {
        let mut sb = [0u8; SUPERBLOCK_SIZE];
        device.read_at(SUPERBLOCK_OFFSET as u64, &mut sb)?;

        let magic = u16::from_le_bytes([sb[0x38], sb[0x39]]);
        if magic != EXT4_SUPER_MAGIC {
            return Err(BootError::InvalidFilesystem);
        }

        let log_block_size = u32::from_le_bytes(sb[0x18..0x1c].try_into().unwrap());
        let block_size = 1024u32
            .checked_shl(log_block_size)
            .ok_or(BootError::InvalidFilesystem)?;
        if !(1024..=65536).contains(&block_size) || !block_size.is_power_of_two() {
            return Err(BootError::InvalidFilesystem);
        }

        let inode_count = u32::from_le_bytes(sb[0x00..0x04].try_into().unwrap());
        let blocks_per_group = u32::from_le_bytes(sb[0x20..0x24].try_into().unwrap());
        let inodes_per_group = u32::from_le_bytes(sb[0x28..0x2c].try_into().unwrap());
        let first_data_block = u32::from_le_bytes(sb[0x14..0x18].try_into().unwrap());
        let inode_size = u16::from_le_bytes([sb[0x58], sb[0x59]]);

        if inode_count == 0 || blocks_per_group == 0 || inodes_per_group == 0 {
            return Err(BootError::InvalidFilesystem);
        }
        if inode_size < 128 || !inode_size.is_power_of_two() {
            return Err(BootError::InvalidFilesystem);
        }

        Ok(Self {
            device,
            geometry: Ext4Geometry {
                block_size,
                inode_size,
                blocks_per_group,
                inodes_per_group,
                inode_count,
                first_data_block,
            },
        })
    }

    pub fn geometry(&self) -> Ext4Geometry {
        self.geometry
    }

    /// Placeholder boundary for the full directory/extent reader.
    ///
    /// Keeping this API in place lets boot selection be developed without
    /// coupling it to a future UEFI block protocol implementation.
    pub fn read_file(&mut self, _path: &str) -> BootResult<Vec<u8>> {
        Err(BootError::Unsupported("ext4 directory/extent reader not yet wired"))
    }
}
