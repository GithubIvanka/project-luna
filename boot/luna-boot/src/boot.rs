//! Main luna-boot orchestration.

use alloc::vec::Vec;
use uefi::boot::{self, open_protocol_exclusive};
use uefi::mem::memory_map::MemoryMapOwned;
use uefi::proto::console::text::Input;
use uefi::runtime::{self, ResetType};
use uefi::Status;

use crate::boot_key::boot_menu_requested;
use crate::discovery::BootCatalog;
use crate::error::{BootError, BootResult};
use crate::e820::E820Extension;
use crate::external::boot_first_external;
use crate::filesystem::SystemFilesystem;
use crate::handoff::{
    current_stack_pointer, transition_entry_address, BootMode, BootState, KernelHandoff, LunaHandoff,
    PreparedIdentity,
};
use crate::kernel::{KernelLoader, PreparedKernel};
use crate::menu::{BootMenu, BootMenuAction, BootSelection};
use crate::paging::prepare_identity_map;
use crate::splash;

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
            Err(error) if matches!(mode, BootMode::Normal) => {
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
    let image_digest = filesystem.hash_file(&target.system_image_path)?;
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
        &image_digest,
        &kernel_identity,
        prepared.init_address,
        prepared.init_size,
        prepared.init_digest,
    )?;

    let mut e820_ext = E820Extension::allocate()?;
    prepared.boot_params.set_setup_data(e820_ext.address)?;
    let transition_entry = transition_entry_address();
    let stack_pointer = current_stack_pointer();
    let (page_table, page_table_pages) = prepare_identity_map(transition_entry, stack_pointer)?;

    let mut reserved = prepared.allocations.clone();
    reserved.push((luna_handoff.address, luna_handoff.allocation_pages));
    reserved.push((e820_ext.address, e820_ext.allocation_pages));
    reserved.push((page_table, page_table_pages));

    // From this point onward Boot Services are gone. The post-EBS path is
    // deliberately non-returning so failures can never reach efi_main().
    let final_map = unsafe { boot::exit_boot_services(None) };
    enter_kernel_after_exit_boot_services(
        final_map,
        prepared,
        page_table,
        luna_handoff,
        e820_ext,
        reserved,
    )
}

fn enter_kernel_after_exit_boot_services(
    final_map: MemoryMapOwned,
    mut prepared: PreparedKernel,
    page_table: u64,
    luna_handoff: LunaHandoff,
    mut e820_ext: E820Extension,
    reserved: Vec<(u64, usize)>,
) -> ! {
    if prepared
        .boot_params
        .set_e820_from_map_reserved(
            &final_map,
            &reserved,
            &mut e820_ext,
            luna_handoff.address,
        )
        .is_err()
    {
        runtime::reset(ResetType::COLD, Status::ABORTED, None);
    }

    let handoff = KernelHandoff {
        kernel_load_address: prepared.kernel_address,
        kernel_entry: prepared.kernel_entry,
        init_address: prepared.init_address,
        init_size: prepared.init_size,
        boot_params_address: prepared.boot_params_address,
        boot_params: prepared.boot_params,
        page_table,
        luna_handoff_address: luna_handoff.address,
        luna_handoff_size: luna_handoff.size,
    };

    if !handoff.is_ready() {
        runtime::reset(ResetType::COLD, Status::ABORTED, None);
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
