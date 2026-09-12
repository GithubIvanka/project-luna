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
const HASH_CHUNK_SIZE: usize = 64 * 1024;

pub trait BlockDevice {
    fn block_size(&self) -> u64;
    fn read_at(&mut self, offset: u64, dst: &mut [u8]) -> BootResult<()>;
}

#[derive(Clone, Copy, Debug)]
pub struct Ext4Geometry {
    pub block_size: u32,
    pub inode_size: u16,
    pub inodes_per_group: u32,
    pub inode_count: u32,
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
        let groups = blocks
            .saturating_sub(first_data_block as u64)
            .div_ceil(blocks_per_group as u64);
        Ok(Self {
            device,
            geometry: Ext4Geometry { block_size, inode_size, inodes_per_group, inode_count, descriptor_size, groups: groups.min(u32::MAX as u64) as u32 },
            has_64bit,
        })
    }

    pub fn read_file(&mut self, path: &str) -> BootResult<Vec<u8>> {
        let inode = self.resolve_path(path)?;
        if inode.mode & 0xf000 != 0x8000 { return Err(BootError::FilesystemError); }
        let size = usize::try_from(inode.size).map_err(|_| BootError::FilesystemError)?;
        let mut out = vec![0u8; size];
        self.read_inode_data(&inode, &mut out)?;
        Ok(out)
    }

    /// Hash a regular file without materializing the complete file in memory.
    pub fn hash_file(&mut self, path: &str) -> BootResult<[u8; 32]> {
        let inode = self.resolve_path(path)?;
        if inode.mode & 0xf000 != 0x8000 { return Err(BootError::FilesystemError); }

        let mut hasher = blake3::Hasher::new();
        let mut file_offset = 0u64;
        let mut buffer = vec![0u8; HASH_CHUNK_SIZE];
        while file_offset < inode.size {
            let remaining = inode.size - file_offset;
            let chunk_len = remaining.min(HASH_CHUNK_SIZE as u64) as usize;
            self.read_inode_range(&inode, file_offset, &mut buffer[..chunk_len])?;
            hasher.update(&buffer[..chunk_len]);
            file_offset = file_offset
                .checked_add(chunk_len as u64)
                .ok_or(BootError::FilesystemError)?;
        }
        Ok(*hasher.finalize().as_bytes())
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
        self.read_inode_range(inode, 0, out)
    }

    fn read_inode_range(&mut self, inode: &Inode, offset: u64, out: &mut [u8]) -> BootResult<()> {
        let end = offset.checked_add(out.len() as u64).ok_or(BootError::FilesystemError)?;
        if end > inode.size { return Err(BootError::FilesystemError); }
        if out.is_empty() { return Ok(()); }

        let bs = self.geometry.block_size as u64;
        let mut written = 0usize;
        while written < out.len() {
            let file_offset = offset + written as u64;
            let block_index = file_offset / bs;
            let in_block = (file_offset % bs) as usize;
            let chunk = (out.len() - written).min(self.geometry.block_size as usize - in_block);
            self.read_inode_block_range(inode, block_index, in_block, &mut out[written..written + chunk])?;
            written += chunk;
        }
        Ok(())
    }

    fn read_inode_block_range(&mut self, inode: &Inode, block_index: u64, in_block: usize, out: &mut [u8]) -> BootResult<()> {
        if inode.flags & EXT4_EXTENTS_FL != 0 {
            self.read_extent_block_range(&inode.blocks, block_index, in_block, out)?;
        } else {
            self.read_legacy_block_range(&inode.blocks, block_index, in_block, out)?;
        }
        Ok(())
    }

    fn read_extent_block_range(&mut self, node: &[u8], wanted_block: u64, in_block: usize, out: &mut [u8]) -> BootResult<()> {
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
                let extent_end = logical.checked_add(len).ok_or(BootError::InvalidFilesystem)?;
                if wanted_block < logical || wanted_block >= extent_end { continue; }
                if raw_len & 0x8000 != 0 { out.fill(0); return Ok(()); }
                let phys_lo = u32_at(node, p + 8) as u64;
                let phys_hi = u16_at(node, p + 6) as u64;
                let physical = phys_lo | (phys_hi << 32);
                let delta = wanted_block - logical;
                let block = physical.checked_add(delta).ok_or(BootError::FilesystemError)?;
                let offset = block
                    .checked_mul(bs(self))
                    .and_then(|base| base.checked_add(in_block as u64))
                    .ok_or(BootError::FilesystemError)?;
                self.device.read_at(offset, out)?;
                return Ok(());
            }
            out.fill(0);
            Ok(())
        } else {
            let mut selected = None;
            for i in 0..entries {
                let p = 12 + i * 12;
                let logical = u32_at(node, p) as u64;
                if logical <= wanted_block {
                    selected = Some(p);
                } else {
                    break;
                }
            }
            let p = selected.ok_or(BootError::FilesystemError)?;
            let child_lo = u32_at(node, p + 4) as u64;
            let child_hi = u16_at(node, p + 8) as u64;
            let child = child_lo | (child_hi << 32);
            let mut child_data = vec![0u8; self.geometry.block_size as usize];
            self.device.read_at(
                child.checked_mul(self.geometry.block_size as u64).ok_or(BootError::FilesystemError)?,
                &mut child_data,
            )?;
            self.read_extent_block_range(&child_data, wanted_block, in_block, out)
        }
    }

    fn read_legacy_block_range(&mut self, blocks: &[u8; 60], block_index: u64, in_block: usize, out: &mut [u8]) -> BootResult<()> {
        if block_index >= 12 { return Err(BootError::Unsupported("ext4 indirect block maps")); }
        let block = u32_at(blocks, block_index as usize * 4) as u64;
        if block == 0 { out.fill(0); return Ok(()); }
        let offset = block
            .checked_mul(self.geometry.block_size as u64)
            .and_then(|base| base.checked_add(in_block as u64))
            .ok_or(BootError::FilesystemError)?;
        self.device.read_at(offset, out)?;
        Ok(())
    }
}

fn bs<D: BlockDevice>(fs: &Ext4<D>) -> u64 { fs.geometry.block_size as u64 }
fn u16_at(data: &[u8], off: usize) -> u16 { u16::from_le_bytes([data[off], data[off + 1]]) }
fn u32_at(data: &[u8], off: usize) -> u32 { u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) }
