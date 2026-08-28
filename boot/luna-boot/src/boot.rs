//! Main boot flow orchestration

use uefi::prelude::*;
use uefi::table::boot::SearchType;
use crate::config::BootConfig;
use crate::error::{BootError, BootResult};
use crate::filesystem::UefiFilesystem;
use crate::kernel::KernelLoader;
use crate::keyboard::check_for_boot_key;
use crate::memory::{exit_boot_services, get_memory_map};
use crate::menu::{show_error, BootMenu};
use crate::target::BootTarget;

/// Main boot flow
pub fn boot_flow(boot_services: &BootServices) -> BootResult<()> {
    log::info!("Starting Luna boot flow");

    // Load configuration
    let config = load_config(boot_services)?;

    // Check for B key press during boot window
    let show_menu = check_for_boot_key(boot_services);

    // Select boot target
    let target = if show_menu {
        show_boot_menu(boot_services, &config)?
    } else {
        config.default_target()?.clone()
    };

    log::info!("Selected boot target: {}", target.name);

    // Open filesystem
    let mut filesystem = UefiFilesystem::open(boot_services)?;

    // Load kernel
    let mut loader = KernelLoader::new(boot_services, &mut filesystem);
    let kernel = loader.load_kernel(&target)?;

    // Get memory map
    let memory_map = get_memory_map(boot_services)?;

    // Exit boot services
    log::info!("Exiting UEFI boot services");
    exit_boot_services(boot_services.clone(), &memory_map)?;

    // Boot kernel (this never returns)
    kernel.boot(memory_map);
}

/// Load boot configuration
fn load_config(_boot_services: &BootServices) -> BootResult<BootConfig> {
    // For prototype, use default config
    // Real implementation would read from boot filesystem
    Ok(BootConfig::default_config())
}

/// Show boot menu and get user selection
fn show_boot_menu(
    boot_services: &BootServices,
    config: &BootConfig,
) -> BootResult<BootTarget> {
    // Get console protocols
    let stdout_handle = boot_services
        .get_handle_for_protocol::<uefi::proto::console::text::Output>()
        .map_err(|_| BootError::UefiError(uefi::Status::NOT_FOUND))?;

    let stdin_handle = boot_services
        .get_handle_for_protocol::<uefi::proto::console::text::Input>()
        .map_err(|_| BootError::UefiError(uefi::Status::NOT_FOUND))?;

    let stdout = boot_services
        .open_protocol_exclusive::<uefi::proto::console::text::Output>(stdout_handle)
        .map_err(|_| BootError::UefiError(uefi::Status::NOT_FOUND))?;

    let stdin = boot_services
        .open_protocol_exclusive::<uefi::proto::console::text::Input>(stdin_handle)
        .map_err(|_| BootError::UefiError(uefi::Status::NOT_FOUND))?;

    // Show menu
    let mut menu = BootMenu::new(stdout, stdin);
    let selection = menu.show(config.targets());

    match selection {
        Some(index) => {
            config
                .targets()
                .get(index)
                .cloned()
                .ok_or(BootError::TargetNotFound)
        }
        None => {
            // User pressed Esc, boot default
            config.default_target().cloned()
        }
    }
}

/// Handle boot error with fallback
pub fn handle_boot_error(
    boot_services: &BootServices,
    error: BootError,
    config: &BootConfig,
    failed_target: &BootTarget,
) -> BootResult<()> {
    log::error!("Boot error: {}", error);

    // Try fallback
    if let Some(fallback) = find_fallback_target(config, failed_target) {
        log::info!("Trying fallback target: {}", fallback.name);
        // Would retry boot with fallback target
        // For now, just report error
    }

    // Show error to user
    let stdout_handle = boot_services
        .get_handle_for_protocol::<uefi::proto::console::text::Output>()
        .map_err(|_| BootError::UefiError(uefi::Status::NOT_FOUND))?;

    let mut stdout = boot_services
        .open_protocol_exclusive::<uefi::proto::console::text::Output>(stdout_handle)
        .map_err(|_| BootError::UefiError(uefi::Status::NOT_FOUND))?;

    let mut error_msg = alloc::string::String::from("Boot failed: ");
    error_msg.push_str(&alloc::format!("{}", error));
    show_error(&mut stdout, &error_msg);

    Err(error)
}

/// Find a fallback target
fn find_fallback_target(config: &BootConfig, failed: &BootTarget) -> Option<BootTarget> {
    // Look for factory target
    config.targets().iter().find(|t| t.is_factory).cloned()
}
