//! UEFI Block I/O adapter used by the read-only ext4 layer.

use alloc::vec::Vec;
use core::ops::Deref;
use core::ptr::NonNull;

use uefi::boot::{self, open_protocol, OpenProtocolAttributes, OpenProtocolParams, ScopedProtocol};
use uefi::proto::media::block::BlockIO;
use uefi::proto::{Protocol, ProtocolPointer};
use uefi::Handle;

use crate::error::{BootError, BootResult};
use crate::ext4::BlockDevice;

const IO_CHUNK: usize = 4096;

/// A firmware protocol obtained with UEFI `GET_PROTOCOL` semantics.
///
/// UEFI explicitly does not require `CloseProtocol()` for `GET_PROTOCOL`.
/// Some firmware implementations, including OVMF, therefore return
/// `EFI_NOT_FOUND` if a caller tries to close such an access through the
/// `ScopedProtocol` RAII path. The protocol is only needed until
/// `ExitBootServices()`, so Luna deliberately keeps the pointer borrowed and
/// does not issue a matching close.
struct BorrowedProtocol<P: Protocol + ?Sized>(NonNull<P>);

impl<P: Protocol + ?Sized> BorrowedProtocol<P> {
    fn from_scoped(protocol: ScopedProtocol<P>) -> Self {
        let ptr = protocol.get().map(NonNull::from).expect("GET_PROTOCOL returned a null interface");
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

/// Open a sized firmware protocol with `GET_PROTOCOL` semantics.
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

/// Open an unsized `DevicePath` protocol with `GET_PROTOCOL` semantics.
///
/// `DevicePath` is a dynamically-sized UEFI protocol type, so it cannot use
/// `open_shared`, whose `ProtocolPointer` bound requires a sized type. Keep the
/// returned `ScopedProtocol` intentionally leaked for the same reason described
/// by `BorrowedProtocol`: GET_PROTOCOL access does not require CloseProtocol.
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
    block_size: u64,
}

impl UefiBlockDevice {
    pub fn new(handle: Handle, start_lba: u64) -> BootResult<Self> {
        let io = open_shared::<BlockIO>(handle)?;
        let media = io.media();
        let block_size = media.block_size() as u64;
        if block_size == 0 || !(IO_CHUNK as u64).is_multiple_of(block_size) {
            return Err(BootError::Unsupported("UEFI block size is not supported by the loader I/O buffer"));
        }
        Ok(Self { io, start_lba, block_size })
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
    fn block_size(&self) -> u64 { self.block_size }

    fn read_at(&mut self, offset: u64, dst: &mut [u8]) -> BootResult<()> {
        if dst.is_empty() { return Ok(()); }
        let absolute = self.start_lba * self.block_size + offset;
        let first_lba = absolute / self.block_size;
        let in_block = (absolute % self.block_size) as usize;

        let total = in_block + dst.len();
        let blocks = total.div_ceil(self.block_size as usize);
        let bytes = blocks * self.block_size as usize;

        let mut temp = vec![0; bytes];
        let mut copied = 0usize;
        let mut remaining = bytes;
        let mut lba = first_lba;
        while remaining != 0 {
            let n = remaining.min(IO_CHUNK);
            self.read_chunk(lba, &mut temp[copied..copied + n])?;
            copied += n;
            remaining -= n;
            lba += (n / self.block_size as usize) as u64;
        }
        dst.copy_from_slice(&temp[in_block..in_block + dst.len()]);
        Ok(())
    }
}

/// Return a non-partition Block I/O handle on the same physical device as the
/// boot image. The firmware's device path for the ESP contains a media hard
/// drive node; its prefix identifies the parent disk device.
pub fn parent_disk_handle(image_handle: Handle) -> BootResult<Handle> {
    use uefi::proto::device_path::DevicePath;

    let loaded = open_shared::<uefi::proto::loaded_image::LoadedImage>(image_handle)?;
    let device = loaded.device().ok_or(BootError::FilesystemError)?;
    let path = open_device_path(device)?;
    let bytes = path.as_bytes();

    let mut cut = None;
    for node in path.node_iter() {
        // MEDIA_DEVICE_PATH / HARD_DRIVE_DP.
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
    let parent_path = <&DevicePath>::try_from(parent.as_slice())
        .map_err(|_| BootError::FilesystemError)?;
    let mut remaining = parent_path;
    boot::locate_device_path::<BlockIO>(&mut remaining).map_err(|_| BootError::FilesystemError)
}
