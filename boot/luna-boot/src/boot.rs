//! Main luna-boot orchestration.

use alloc::vec::Vec;
use uefi::boot::{self, open_protocol_exclusive};
use uefi::proto::console::text::Input;

use crate::boot_key::boot_menu_requested;
use crate::discovery::BootCatalog;
use crate::error::{BootError, BootResult};
use crate::external::boot_first_external;
use crate::filesystem::SystemFilesystem;
use crate::handoff::{BootMode, BootState, KernelHandoff, LunaHandoff, PreparedIdentity};
use crate::kernel::KernelLoader;
use crate::menu::{BootMenu, BootMenuAction, BootSelection};
use crate::paging::prepare_identity_map;
use crate::splash;

const PAGE_TABLE_PAGES: usize = 66;

pub fn boot_flow() -> BootResult<()> {
    let input_handle = boot::get_handle_for_protocol::<Input>()?;
    let mut input = open_protocol_exclusive::<Input>(input_handle)?;
    let menu_requested = boot_menu_requested(&mut input);
    drop(input);

    let mut filesystem = match SystemFilesystem::open() {
        Ok(value) => Some(value),
        Err(_) if menu_requested => None,
        Err(error) => return Err(error),
    };

    let catalog = match filesystem.as_mut() {
        Some(fs) => BootCatalog::discover(fs).unwrap_or_default(),
        None => BootCatalog::default(),
    };

    let selection = if menu_requested {
        let stdout_handle = boot::get_handle_for_protocol::<uefi::proto::console::text::Output>()?;
        let stdin_handle = boot::get_handle_for_protocol::<Input>()?;
        let stdout = open_protocol_exclusive(stdout_handle)?;
        let stdin = open_protocol_exclusive(stdin_handle)?;
        let mut menu = BootMenu::new(stdout, stdin);
        menu.show(&catalog.targets, catalog.default_target)
            .unwrap_or(BootSelection {
                action: BootMenuAction::Continue,
                target_index: catalog.default_target,
            })
    } else {
        splash::show();
        BootSelection {
            action: BootMenuAction::Continue,
            target_index: catalog.default_target,
        }
    };

    if selection.action == BootMenuAction::ExternalBoot {
        return boot_first_external();
    }

    let filesystem = filesystem.as_mut().ok_or(BootError::FilesystemError)?;

    let selected = match selection.action {
        BootMenuAction::Recovery => catalog
            .recovery
            .clone()
            .ok_or(BootError::RecoveryUnavailable),
        BootMenuAction::Factory => catalog
            .factory
            .clone()
            .ok_or(BootError::Unsupported("factory environment is unavailable on this installation")),
        BootMenuAction::Continue
        | BootMenuAction::SystemImage
        | BootMenuAction::VerboseBoot => catalog
            .targets
            .get(selection.target_index)
            .cloned()
            .ok_or(BootError::TargetNotFound),
        BootMenuAction::ExternalBoot => unreachable!(),
    }?;

    let mode = match selection.action {
        BootMenuAction::VerboseBoot => BootMode::Detailed,
        BootMenuAction::Recovery => BootMode::Recovery,
        BootMenuAction::Factory => BootMode::Factory,
        _ => BootMode::Normal,
    };

    // A fallback is an atomic image+init+kernel tuple. Never pair a failed
    // target's manifest/image with another target's prepared kernel.
    let mut candidates = Vec::new();
    candidates.push(selected.clone());
    if matches!(
        selection.action,
        BootMenuAction::Continue
            | BootMenuAction::SystemImage
            | BootMenuAction::VerboseBoot
    ) {
        candidates.extend(
            catalog
                .targets
                .iter()
                .skip(selection.target_index + 1)
                .cloned(),
        );
    }

    let mut prepared = None;
    let mut target = None;
    for candidate in candidates {
        match KernelLoader::new(filesystem).prepare(&candidate) {
            Ok(value) => {
                target = Some(candidate);
                prepared = Some(value);
                break;
            }
            Err(error) if mode == BootMode::Normal => {
                log::warn!(
                    "Luna: target {} / kernel {} preparation failed: {error:?}",
                    candidate.system_version,
                    candidate.kernel_id
                );
            }
            Err(error) => return Err(error),
        }
    }
    let (mut target, mut prepared) = match (target, prepared) {
        (Some(target), Some(prepared)) => (target, prepared),
        _ => return Err(BootError::TargetNotFound),
    };

    if selection.action == BootMenuAction::VerboseBoot {
        target.kernel_cmdline = target
            .kernel_cmdline
            .split_whitespace()
            .filter(|part| *part != "quiet" && !part.starts_with("loglevel="))
            .collect::<Vec<_>>()
            .join(" ");
        target.kernel_cmdline.push_str(" loglevel=7 ignore_loglevel");
    }

    let manifest_bytes = filesystem.read_file(&target.manifest_path)?;
    let image_bytes = filesystem.read_file(&target.system_image_path)?;
    let kernel_identity = PreparedIdentity {
        kernel_digest: prepared.kernel_digest,
    };
    let luna_handoff = LunaHandoff::build(
        &target,
        mode,
        BootState::default(),
        filesystem.system_partition(),
        filesystem.data_partition(),
        &manifest_bytes,
        &image_bytes,
        &kernel_identity,
        prepared.init_address,
        prepared.init_size,
        prepared.init_digest,
    )?;

    prepared.boot_params.set_setup_data(luna_handoff.address)?;
    let page_table = prepare_identity_map()?;

    let mut reserved = prepared.allocations.clone();
    reserved.push((luna_handoff.address, luna_handoff.allocation_pages));
    reserved.push((page_table, PAGE_TABLE_PAGES));

    let final_map = unsafe { boot::exit_boot_services(None) };
    prepared
        .boot_params
        .set_e820_from_map_reserved(&final_map, &reserved)?;

    let handoff = KernelHandoff {
        kernel_load_address: prepared.kernel_address,
        kernel_entry: prepared.kernel_entry,
        init_address: prepared.init_address,
        init_size: prepared.init_size,
        boot_params_address: prepared.boot_params_address,
        command_line_address: prepared.command_line_address,
        setup: prepared.setup,
        boot_params: prepared.boot_params,
        page_table,
        luna_handoff_address: luna_handoff.address,
        luna_handoff_size: luna_handoff.size,
        luna_handoff_pages: luna_handoff.allocation_pages,
    };

    if !handoff.is_ready() {
        return Err(BootError::InvalidKernel);
    }

    unsafe {
        core::ptr::copy_nonoverlapping(
            handoff.boot_params.as_bytes().as_ptr(),
            handoff.boot_params_address as *mut u8,
            handoff.boot_params.as_bytes().len(),
        );
        handoff.enter();
    }
}
