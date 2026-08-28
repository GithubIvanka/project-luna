//! Main boot-flow orchestration.
//!
//! The function intentionally stops before the destructive Linux handoff until
//! the final physical-memory allocator and x86_64 entry stub are present.
//! Everything before that boundary is ordinary UEFI code and is testable.

use uefi::prelude::*;
use uefi::proto::console::text::{Input, Output};

use crate::config::BootConfig;
use crate::error::{BootError, BootResult};
use crate::filesystem::UefiFilesystem;
use crate::kernel::KernelLoader;
use crate::keyboard::check_for_boot_key;
use crate::menu::{show_error, BootMenu};
use crate::target::BootTarget;

pub fn boot_flow(image_handle: Handle, boot_services: &BootServices) -> BootResult<()> {
    let config = load_config(boot_services)?;

    // This is deliberately a single non-blocking sample. No two-second delay.
    let target = if check_for_boot_key(boot_services) {
        show_boot_menu(boot_services, &config)?
    } else {
        config.default_target()?.clone()
    };

    // The ESP is opened only through the LoadedImage.device() handle belonging
    // to luna-boot itself. The system partition is not an EFI filesystem.
    let mut filesystem = UefiFilesystem::open(boot_services, image_handle)?;
    let mut loader = KernelLoader::new(&mut filesystem);

    match loader.load_kernel(&target) {
        Ok(kernel) => {
            // At this point the bzImage has passed the Linux boot-protocol
            // header validation. Physical placement, initrd allocation,
            // final boot_params population and the assembly transition are the
            // only remaining hardware-facing steps.
            let _ = kernel;
            Err(BootError::Unsupported("final Linux x86_64 handoff is not yet enabled"))
        }
        Err(primary_error) => {
            if let Some(factory) = config.targets().iter().find(|t| t.is_factory) {
                if factory.kernel_path != target.kernel_path {
                    let mut fallback_loader = KernelLoader::new(&mut filesystem);
                    if fallback_loader.load_kernel(factory).is_ok() {
                        return Err(BootError::Unsupported("Factory kernel validated; final handoff is not yet enabled"));
                    }
                }
            }
            Err(primary_error)
        }
    }
}

fn load_config(_boot_services: &BootServices) -> BootResult<BootConfig> {
    Ok(BootConfig::default_config())
}

fn show_boot_menu(
    boot_services: &BootServices,
    config: &BootConfig,
) -> BootResult<BootTarget> {
    let stdout_handle = boot_services
        .get_handle_for_protocol::<Output>()
        .map_err(|_| BootError::UefiError(uefi::Status::NOT_FOUND))?;
    let stdin_handle = boot_services
        .get_handle_for_protocol::<Input>()
        .map_err(|_| BootError::UefiError(uefi::Status::NOT_FOUND))?;

    let stdout = boot_services
        .open_protocol_exclusive::<Output>(stdout_handle)
        .map_err(|_| BootError::UefiError(uefi::Status::NOT_FOUND))?;
    let stdin = boot_services
        .open_protocol_exclusive::<Input>(stdin_handle)
        .map_err(|_| BootError::UefiError(uefi::Status::NOT_FOUND))?;

    let mut menu = BootMenu::new(stdout, stdin);
    match menu.show(config.targets()) {
        Some(index) => config.targets().get(index).cloned().ok_or(BootError::TargetNotFound),
        None => config.default_target().cloned(),
    }
}

pub fn handle_boot_error(
    boot_services: &BootServices,
    error: BootError,
    _config: &BootConfig,
    _failed_target: &BootTarget,
) -> BootResult<()> {
    let stdout_handle = boot_services
        .get_handle_for_protocol::<Output>()
        .map_err(|_| BootError::UefiError(uefi::Status::NOT_FOUND))?;
    let mut stdout = boot_services
        .open_protocol_exclusive::<Output>(stdout_handle)
        .map_err(|_| BootError::UefiError(uefi::Status::NOT_FOUND))?;

    let message = alloc::format!("Boot failed: {}", error);
    show_error(&mut stdout, &message);
    Err(error)
}
