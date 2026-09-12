//! UEFI Block I/O adapter used by the read-only ext4 layer.

use alloc::vec;
use alloc::vec::Vec;
use core::ops::Deref;
use core::ptr::NonNull;

use uefi::Handle;
use uefi::boot::{self, OpenProtocolAttributes, OpenProtocolParams, ScopedProtocol, open_protocol};
use uefi::proto::media::block::BlockIO;
use uefi::proto::{Protocol, ProtocolPointer};

use crate::error::{BootError, BootResult};
use crate::ext4::BlockDevice;

const IO_CHUNK: usize = 4096;

struct BorrowedProtocol<P: Protocol + ?Sized>(NonNull<P>);

impl<P: Protocol + ?Sized> BorrowedProtocol<P> {
    fn from_scoped(protocol: ScopedProtocol<P>) -> Self {
        let ptr = protocol
            .get()
            .map(NonNull::from)
            .expect("GET_PROTOCOL returned a null interface");
        core::mem::forget(protocol);
        Self(ptr)
    }
}

impl<P: Protocol + ?Sized> Deref for BorrowedProtocol<P> {
    type Target = P;

    fn deref(&self) -> &Self::Target {
        // SAFETY: the firmware protocol is valid for the lifetime of the
        // loader and remains installed until ExitBootServices().
        unsafe { self.0.as_ref() }
    }
}

fn open_shared<P: ProtocolPointer>(handle: Handle) -> BootResult<BorrowedProtocol<P>> {
    let protocol = unsafe {
        open_protocol::<P>(
            OpenProtocolParams {
                handle,
                agent: boot::image_handle(),
                controller: None,
            },
            OpenProtocolAttributes::GetProtocol,
        )
        .map_err(BootError::from)?
    };

    Ok(BorrowedProtocol::from_scoped(protocol))
}

fn open_device_path(handle: Handle) -> BootResult<&'static uefi::proto::device_path::DevicePath> {
    use uefi::proto::device_path::DevicePath;

    let protocol = unsafe {
        open_protocol::<DevicePath>(
            OpenProtocolParams {
                handle,
                agent: boot::image_handle(),
                controller: None,
            },
            OpenProtocolAttributes::GetProtocol,
        )
        .map_err(BootError::from)?
    };

    let path = protocol.get().ok_or(BootError::FilesystemError)?;
    let path = unsafe { core::mem::transmute::<&DevicePath, &'static DevicePath>(path) };
    core::mem::forget(protocol);
    Ok(path)
}

pub struct UefiBlockDevice {
    io: BorrowedProtocol<BlockIO>,
    start_lba: u64,
    block_count: u64,
    block_size: u64,
}

impl UefiBlockDevice {
    /// Create a strict view over `block_count` blocks starting at `start_lba`.
    pub fn new(handle: Handle, start_lba: u64, block_count: u64) -> BootResult<Self> {
        let io = open_shared::<BlockIO>(handle)?;
        let media = io.media();
        let block_size = media.block_size() as u64;
        if block_size == 0 || !(IO_CHUNK as u64).is_multiple_of(block_size) {
            return Err(BootError::Unsupported(
                "UEFI block size is not supported by the loader I/O buffer",
            ));
        }
        if block_count == 0 || start_lba.checked_add(block_count - 1).is_none() {
            return Err(BootError::FilesystemError);
        }
        let last_lba = start_lba + block_count - 1;
        if last_lba > media.last_block() {
            return Err(BootError::FilesystemError);
        }
        Ok(Self {
            io,
            start_lba,
            block_count,
            block_size,
        })
    }

    /// Create a strict view spanning the whole physical disk.
    pub fn whole_disk(handle: Handle) -> BootResult<Self> {
        let io = open_shared::<BlockIO>(handle)?;
        let last_block = io.media().last_block();
        let block_count = last_block
            .checked_add(1)
            .ok_or(BootError::FilesystemError)?;
        let block_size = io.media().block_size() as u64;
        if block_size == 0 || !(IO_CHUNK as u64).is_multiple_of(block_size) {
            return Err(BootError::Unsupported(
                "UEFI block size is not supported by the loader I/O buffer",
            ));
        }
        Ok(Self {
            io,
            start_lba: 0,
            block_count,
            block_size,
        })
    }

    fn read_chunk(&mut self, lba: u64, dst: &mut [u8]) -> BootResult<()> {
        #[repr(align(4096))]
        struct Aligned([u8; IO_CHUNK]);

        let mut aligned = Aligned([0; IO_CHUNK]);
        self.io
            .read_blocks(self.io.media().media_id(), lba, &mut aligned.0[..dst.len()])
            .map_err(|_| BootError::FilesystemError)?;
        dst.copy_from_slice(&aligned.0[..dst.len()]);
        Ok(())
    }
}

impl BlockDevice for UefiBlockDevice {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn read_at(&mut self, offset: u64, dst: &mut [u8]) -> BootResult<()> {
        if dst.is_empty() {
            return Ok(());
        }

        let capacity = self
            .block_count
            .checked_mul(self.block_size)
            .ok_or(BootError::FilesystemError)?;
        let end = offset
            .checked_add(dst.len() as u64)
            .ok_or(BootError::FilesystemError)?;
        if end > capacity {
            return Err(BootError::FilesystemError);
        }

        let absolute = self
            .start_lba
            .checked_mul(self.block_size)
            .and_then(|base| base.checked_add(offset))
            .ok_or(BootError::FilesystemError)?;
        let first_lba = absolute / self.block_size;
        let in_block = (absolute % self.block_size) as usize;

        let total = in_block
            .checked_add(dst.len())
            .ok_or(BootError::FilesystemError)?;
        let blocks = total.div_ceil(self.block_size as usize);
        let bytes = blocks
            .checked_mul(self.block_size as usize)
            .ok_or(BootError::FilesystemError)?;

        let mut temp = vec![0; bytes];
        let mut copied = 0usize;
        let mut remaining = bytes;
        let mut lba = first_lba;
        while remaining != 0 {
            let n = remaining.min(IO_CHUNK);
            self.read_chunk(lba, &mut temp[copied..copied + n])?;
            copied += n;
            remaining -= n;
            lba = lba
                .checked_add((n / self.block_size as usize) as u64)
                .ok_or(BootError::FilesystemError)?;
        }
        dst.copy_from_slice(&temp[in_block..in_block + dst.len()]);
        Ok(())
    }
}

pub fn parent_disk_handle(image_handle: Handle) -> BootResult<Handle> {
    use uefi::proto::device_path::DevicePath;

    let loaded = open_shared::<uefi::proto::loaded_image::LoadedImage>(image_handle)?;
    let device = loaded.device().ok_or(BootError::FilesystemError)?;
    let path = open_device_path(device)?;
    let bytes = path.as_bytes();

    let mut cut = None;
    for node in path.node_iter() {
        if node.device_type().0 == 0x04 && node.sub_type().0 == 0x01 {
            cut = Some(node.as_ffi_ptr() as usize - bytes.as_ptr() as usize);
            break;
        }
    }
    let cut = cut.ok_or(BootError::FilesystemError)?;
    if cut + 4 > bytes.len() {
        return Err(BootError::FilesystemError);
    }

    let mut parent = Vec::with_capacity(cut + 4);
    parent.extend_from_slice(&bytes[..cut]);
    parent.extend_from_slice(&[0x7f, 0xff, 0x04, 0x00]);
    let parent_path =
        <&DevicePath>::try_from(parent.as_slice()).map_err(|_| BootError::FilesystemError)?;
    let mut remaining = parent_path;
    boot::locate_device_path::<BlockIO>(&mut remaining).map_err(|_| BootError::FilesystemError)
}
