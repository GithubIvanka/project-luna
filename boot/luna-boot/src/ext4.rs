//! Minimal read-only ext4 reader used by luna-boot.
//!
//! Supported operations are intentionally limited to bootloader needs:
//! regular-file lookup, directory enumeration and reads. Journaling, writes,
//! checksums and extended attributes are not interpreted. The implementation
//! supports the common ext4 extent format and legacy direct block maps.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::error::{BootError, BootResult};

pub const EXT4_SUPER_MAGIC: u16 = 0xEF53;
const SUPERBLOCK_OFFSET: u64 = 1024;
const SUPERBLOCK_SIZE: usize = 1024;
const EXT4_EXTENTS_FL: u32 = 0x0008_0000;
const EXT4_FT_DIR: u8 = 2;
const EXT4_FT_REG_FILE: u8 = 1;
const EXT4_ROOT_INO: u32 = 2;

pub trait BlockDevice {
    fn block_size(&self) -> u64;
    fn read_at(&mut self, offset: u64, dst: &mut [u8]) -> BootResult<()>;
}

#[derive(Clone, Copy, Debug)]
pub struct Ext4Geometry {
    pub block_size: u32,
    pub inode_size: u16,
    pub blocks_per_group: u32,
    pub inodes_per_group: u32,
    pub inode_count: u32,
    pub first_data_block: u32,
    pub descriptor_size: u16,
    pub groups: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirEntry {
    pub name: String,
    pub inode: u32,
    pub file_type: u8,
}
impl DirEntry {
    pub fn is_dir(&self) -> bool { self.file_type == EXT4_FT_DIR }
    pub fn is_file(&self) -> bool { self.file_type == EXT4_FT_REG_FILE || self.file_type == 0 }
}

pub struct Ext4<D> {
    device: D,
    geometry: Ext4Geometry,
    has_64bit: bool,
}

#[derive(Clone, Copy)]
struct Inode {
    mode: u16,
    size: u64,
    flags: u32,
    blocks: [u8; 60],
}

impl<D: BlockDevice> Ext4<D> {
    pub fn open(mut device: D) -> BootResult<Self> {
        if device.block_size() == 0 || !device.block_size().is_power_of_two() { return Err(BootError::InvalidFilesystem); }
        let mut sb = [0u8; SUPERBLOCK_SIZE];
        device.read_at(SUPERBLOCK_OFFSET, &mut sb)?;
        if u16_at(&sb, 0x38) != EXT4_SUPER_MAGIC { return Err(BootError::InvalidFilesystem); }
        let log_block_size = u32_at(&sb, 0x18);
        let block_size = 1024u32.checked_shl(log_block_size).ok_or(BootError::InvalidFilesystem)?;
        if !(1024..=65536).contains(&block_size) || !block_size.is_power_of_two() { return Err(BootError::InvalidFilesystem); }
        let inode_count = u32_at(&sb, 0x00);
        let blocks_lo = u32_at(&sb, 0x04);
        let first_data_block = u32_at(&sb, 0x14);
        let blocks_per_group = u32_at(&sb, 0x20);
        let inodes_per_group = u32_at(&sb, 0x28);
        let inode_size = u16_at(&sb, 0x58);
        let feature_incompat = u32_at(&sb, 0x60);
        let has_64bit = feature_incompat & 0x80 != 0;
        let descriptor_size = if has_64bit { u16_at(&sb, 0xfe) } else { 32 };
        if inode_count == 0 || blocks_per_group == 0 || inodes_per_group == 0 { return Err(BootError::InvalidFilesystem); }
        if inode_size < 128 || inode_size as u32 > block_size || !inode_size.is_power_of_two() { return Err(BootError::InvalidFilesystem); }
        if descriptor_size < 32 || descriptor_size as u32 > block_size { return Err(BootError::InvalidFilesystem); }
        let blocks = blocks_lo as u64;
        let groups = (blocks.saturating_sub(first_data_block as u64) + blocks_per_group as u64 - 1) / blocks_per_group as u64;
        Ok(Self {
            device,
            geometry: Ext4Geometry { block_size, inode_size, blocks_per_group, inodes_per_group, inode_count, first_data_block, descriptor_size, groups: groups.min(u32::MAX as u64) as u32 },
            has_64bit,
        })
    }

    pub fn geometry(&self) -> Ext4Geometry { self.geometry }

    pub fn read_file(&mut self, path: &str) -> BootResult<Vec<u8>> {
        let inode = self.resolve_path(path)?;
        if inode.mode & 0xf000 != 0x8000 { return Err(BootError::FilesystemError); }
        let size = usize::try_from(inode.size).map_err(|_| BootError::FilesystemError)?;
        let mut out = vec![0u8; size];
        self.read_inode_data(&inode, &mut out)?;
        Ok(out)
    }

    pub fn file_exists(&mut self, path: &str) -> BootResult<bool> {
        match self.resolve_path(path) {
            Ok(inode) => Ok(inode.mode & 0xf000 == 0x8000),
            Err(BootError::TargetNotFound) => Ok(false),
            Err(error) => Err(error),
        }
    }

    pub fn read_dir(&mut self, path: &str) -> BootResult<Vec<DirEntry>> {
        let inode = self.resolve_path(path)?;
        if inode.mode & 0xf000 != 0x4000 { return Err(BootError::FilesystemError); }
        let size = usize::try_from(inode.size).map_err(|_| BootError::FilesystemError)?;
        let mut data = vec![0u8; size];
        self.read_inode_data(&inode, &mut data)?;
        let mut entries = Vec::new();
        let mut off = 0usize;
        while off + 8 <= data.len() {
            let ino = u32_at(&data, off);
            let rec_len = u16_at(&data, off + 4) as usize;
            let name_len = data[off + 6] as usize;
            let file_type = data[off + 7];
            if rec_len < 8 || off + rec_len > data.len() || name_len > rec_len - 8 { return Err(BootError::InvalidFilesystem); }
            if ino != 0 && name_len != 0 {
                let name_bytes = &data[off + 8..off + 8 + name_len];
                let name = String::from_utf8(name_bytes.to_vec()).map_err(|_| BootError::FilesystemError)?;
                if name != "." && name != ".." { entries.push(DirEntry { name, inode: ino, file_type }); }
            }
            off += rec_len;
        }
        Ok(entries)
    }

    fn resolve_path(&mut self, path: &str) -> BootResult<Inode> {
        if !path.starts_with('/') { return Err(BootError::FilesystemError); }
        let mut inode = self.read_inode(EXT4_ROOT_INO)?;
        for component in path.split('/').filter(|p| !p.is_empty()) {
            if inode.mode & 0xf000 != 0x4000 { return Err(BootError::FilesystemError); }
            let next = self.find_in_directory(&inode, component)?;
            inode = self.read_inode(next)?;
        }
        Ok(inode)
    }

    fn group_descriptor(&mut self, group: u32) -> BootResult<Vec<u8>> {
        if group >= self.geometry.groups { return Err(BootError::FilesystemError); }
        let table_block = if self.geometry.block_size == 1024 { 2 } else { 1 };
        let offset = table_block as u64 * self.geometry.block_size as u64 + group as u64 * self.geometry.descriptor_size as u64;
        let mut desc = vec![0u8; self.geometry.descriptor_size as usize];
        self.device.read_at(offset, &mut desc)?;
        Ok(desc)
    }

    fn inode_table_block(&mut self, group: u32) -> BootResult<u64> {
        let desc = self.group_descriptor(group)?;
        let lo = u32_at(&desc, 8) as u64;
        let hi = if self.has_64bit && desc.len() >= 44 { u32_at(&desc, 40) as u64 } else { 0 };
        Ok(lo | (hi << 32))
    }

    fn read_inode(&mut self, ino: u32) -> BootResult<Inode> {
        if ino == 0 || ino > self.geometry.inode_count { return Err(BootError::FilesystemError); }
        let index = ino - 1;
        let group = index / self.geometry.inodes_per_group;
        let local = index % self.geometry.inodes_per_group;
        let table = self.inode_table_block(group)?;
        let offset = table * self.geometry.block_size as u64 + local as u64 * self.geometry.inode_size as u64;
        let mut raw = vec![0u8; self.geometry.inode_size as usize];
        self.device.read_at(offset, &mut raw)?;
        let mut blocks = [0u8; 60];
        blocks.copy_from_slice(&raw[40..100]);
        let size_lo = u32_at(&raw, 4) as u64;
        let size_hi = if raw.len() >= 112 { u32_at(&raw, 108) as u64 } else { 0 };
        Ok(Inode { mode: u16_at(&raw, 0), size: size_lo | (size_hi << 32), flags: u32_at(&raw, 32), blocks })
    }

    fn find_in_directory(&mut self, dir: &Inode, wanted: &str) -> BootResult<u32> {
        let mut data = vec![0u8; usize::try_from(dir.size).map_err(|_| BootError::FilesystemError)?];
        self.read_inode_data(dir, &mut data)?;
        let wanted = wanted.as_bytes();
        let mut off = 0usize;
        while off + 8 <= data.len() {
            let ino = u32_at(&data, off);
            let rec_len = u16_at(&data, off + 4) as usize;
            let name_len = data[off + 6] as usize;
            let file_type = data[off + 7];
            if rec_len < 8 || off + rec_len > data.len() || name_len > rec_len - 8 { return Err(BootError::InvalidFilesystem); }
            let name = &data[off + 8..off + 8 + name_len];
            if ino != 0 && name == wanted && (file_type == EXT4_FT_DIR || file_type == EXT4_FT_REG_FILE || file_type == 0) { return Ok(ino); }
            off += rec_len;
        }
        Err(BootError::TargetNotFound)
    }

    fn read_inode_data(&mut self, inode: &Inode, out: &mut [u8]) -> BootResult<()> {
        if inode.flags & EXT4_EXTENTS_FL != 0 { self.read_extent_node(&inode.blocks, out, 0)?; }
        else { self.read_legacy_blocks(&inode.blocks, out)?; }
        Ok(())
    }

    fn read_extent_node(&mut self, node: &[u8], out: &mut [u8], file_block_base: u64) -> BootResult<()> {
        if u16_at(node, 0) != 0xf30a { return Err(BootError::InvalidFilesystem); }
        let entries = u16_at(node, 2) as usize;
        let depth = u16_at(node, 6);
        if entries > 4 || 12 + entries * 12 > node.len() { return Err(BootError::InvalidFilesystem); }
        if depth == 0 {
            for i in 0..entries {
                let p = 12 + i * 12;
                let logical = u32_at(node, p) as u64;
                let raw_len = u16_at(node, p + 4);
                let len = (raw_len & 0x7fff) as u64;
                let phys_lo = u32_at(node, p + 8) as u64;
                let phys_hi = u16_at(node, p + 6) as u64;
                let phys = phys_lo | (phys_hi << 32);
                self.copy_extent(logical, len, phys, file_block_base, out)?;
            }
        } else {
            for i in 0..entries {
                let p = 12 + i * 12;
                let logical = u32_at(node, p) as u64;
                let child_lo = u32_at(node, p + 4) as u64;
                let child_hi = u16_at(node, p + 8) as u64;
                let child = child_lo | (child_hi << 32);
                let mut child_data = vec![0u8; self.geometry.block_size as usize];
                self.device.read_at(child * self.geometry.block_size as u64, &mut child_data)?;
                self.read_extent_node(&child_data, out, logical)?;
            }
        }
        Ok(())
    }

    fn copy_extent(&mut self, logical: u64, len: u64, physical: u64, _base: u64, out: &mut [u8]) -> BootResult<()> {
        let bs = self.geometry.block_size as usize;
        let start = logical as usize * bs;
        if start >= out.len() { return Ok(()); }
        let bytes = (len as usize * bs).min(out.len() - start);
        self.device.read_at(physical * self.geometry.block_size as u64, &mut out[start..start + bytes])?;
        Ok(())
    }

    fn read_legacy_blocks(&mut self, blocks: &[u8; 60], out: &mut [u8]) -> BootResult<()> {
        let bs = self.geometry.block_size as usize;
        let direct = 12usize;
        for i in 0..direct {
            let p = i * 4;
            let block = u32_at(blocks, p) as u64;
            if block == 0 { break; }
            let start = i * bs;
            if start >= out.len() { break; }
            let len = bs.min(out.len() - start);
            self.device.read_at(block * bs as u64, &mut out[start..start + len])?;
        }
        if out.len() > direct * bs { return Err(BootError::Unsupported("ext4 indirect block maps")); }
        Ok(())
    }
}

fn u16_at(data: &[u8], off: usize) -> u16 { u16::from_le_bytes([data[off], data[off + 1]]) }
fn u32_at(data: &[u8], off: usize) -> u32 { u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) }
