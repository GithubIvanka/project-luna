//! UEFI external-media boot support.
//!
//! This is intentionally a UEFI-only operation. It looks for a standard
//! `EFI/BOOT/BOOTX64.EFI` loader on other SimpleFileSystem devices and
//! chainloads the first device whose loader can be loaded and started.

use alloc::vec::Vec;

use uefi::boot::{self, open_protocol_exclusive, LoadImageSource, SearchType};
use uefi::proto::device_path::build::{self, DevicePathBuilder};
use uefi::proto::device_path::{DevicePath, DeviceSubType, DeviceType};
use uefi::proto::loaded_image::LoadedImage;
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::proto::BootPolicy;
use uefi::{cstr16, Handle, Identify};

use crate::error::{BootError, BootResult};

pub fn boot_first_external() -> BootResult<()> {
    let current_device = current_device_handle()?;
    let handles = boot::locate_handle_buffer(SearchType::ByProtocol(&SimpleFileSystem::GUID))
        .map_err(|_| BootError::RecoveryUnavailable)?;

    for handle in handles.iter().copied() {
        if Some(handle) == current_device {
            continue;
        }
        if try_boot_device(handle).is_ok() {
            return Ok(());
        }
    }

    Err(BootError::Unsupported("no external EFI/BOOT/BOOTX64.EFI was found"))
}

fn current_device_handle() -> BootResult<Option<Handle>> {
    let image = open_protocol_exclusive::<LoadedImage>(boot::image_handle())
        .map_err(|_| BootError::UefiError(uefi::Status::DEVICE_ERROR))?;
    Ok(image.device())
}

fn try_boot_device(handle: Handle) -> BootResult<()> {
    let device_path = open_protocol_exclusive::<DevicePath>(handle)
        .map_err(|_| BootError::UefiError(uefi::Status::DEVICE_ERROR))?;

    let mut storage = Vec::new();
    let path = {
        let mut builder = DevicePathBuilder::with_vec(&mut storage);
        for node in device_path.node_iter() {
            if node.full_type() == (DeviceType::MEDIA, DeviceSubType::MEDIA_FILE_PATH) {
                break;
            }
            builder = builder
                .push(&node)
                .map_err(|_| BootError::Unsupported("invalid external device path"))?;
        }
        builder = builder
            .push(&build::media::FilePath {
                path_name: cstr16!(r"EFI\BOOT\BOOTX64.EFI"),
            })
            .map_err(|_| BootError::Unsupported("failed to build external boot path"))?;
        builder
            .finalize()
            .map_err(|_| BootError::Unsupported("failed to finalize external boot path"))?
    };

    let image = boot::load_image(
        boot::image_handle(),
        LoadImageSource::FromDevicePath {
            device_path: path,
            boot_policy: BootPolicy::ExactMatch,
        },
    )
    .map_err(|_| BootError::Unsupported("external bootloader could not be loaded"))?;

    boot::start_image(image)
        .map(|_| ())
        .map_err(|_| BootError::Unsupported("external bootloader returned failure"))
}
