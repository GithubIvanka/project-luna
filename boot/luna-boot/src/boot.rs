//! Main luna-boot orchestration.

use alloc::vec::Vec;
use uefi::Status;
use uefi::boot::{
    self, OpenProtocolAttributes, OpenProtocolParams, open_protocol, open_protocol_exclusive,
};
use uefi::mem::memory_map::MemoryMapOwned;
use uefi::proto::console::text::Input;
use uefi::runtime::{self, ResetType};

use crate::boot_attempt::{BootAttempt, BootAttemptMarker, BootStage};
use crate::boot_key::boot_menu_requested;
use crate::discovery::BootCatalog;
use crate::e820::E820Extension;
use crate::error::{BootError, BootResult};
use crate::external::boot_first_external;
use crate::filesystem::{DataStatus, SystemFilesystem};
use crate::handoff::{
    BootMode, BootState, KernelHandoff, LunaBootProgress, LunaHandoff, PreparedIdentity,
    current_stack_pointer, transition_entry_address,
};
use crate::kernel::{KernelLoader, PreparedKernel};
use crate::menu::{BootMenu, BootMenuAction, BootSelection};
use crate::paging::prepare_identity_map;
use crate::splash;

pub fn boot_flow() -> BootResult<()> {
    debug_stage("Luna: flow 1 input\r\n");
    let input_handle = boot::get_handle_for_protocol::<Input>()?;
    let mut input_protocol = unsafe {
        open_protocol::<Input>(
            OpenProtocolParams {
                handle: input_handle,
                agent: boot::image_handle(),
                controller: None,
            },
            OpenProtocolAttributes::GetProtocol,
        )
    }?;
    let input = input_protocol.get_mut().ok_or(BootError::FilesystemError)?;
    let menu_requested = boot_menu_requested(input);
    core::mem::forget(input_protocol);

    debug_stage("Luna: flow 2 input ready\r\n");
    let previous_attempt = BootAttemptMarker::read()?;
    debug_stage("Luna: flow 3 marker ready\r\n");

    let mut filesystem = match SystemFilesystem::open() {
        Ok(value) => Some(value),
        Err(_) if menu_requested => None,
        Err(error) => return Err(error),
    };

    debug_stage("Luna: flow 4 filesystem ready\r\n");
    let catalog = match filesystem.as_mut() {
        Some(fs) => BootCatalog::discover(fs).unwrap_or_default(),
        None => BootCatalog::default(),
    };
    debug_stage("Luna: flow 5 catalog ready\r\n");

    debug_stage("Luna: flow 6 before selection\r\n");
    let mut selection = if menu_requested {
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
        debug_stage("Luna: flow 6a before splash\r\n");
        splash::show();
        debug_stage("Luna: flow 6b after splash\r\n");
        BootSelection {
            action: BootMenuAction::Continue,
            target_index: catalog.default_target,
        }
    };
    debug_stage("Luna: flow 7 selection ready\r\n");

    if selection.action == BootMenuAction::ExternalBoot {
        return boot_first_external();
    }

    let filesystem = filesystem.as_mut().ok_or(BootError::FilesystemError)?;
    debug_stage("Luna: flow 8 filesystem mut ready\r\n");

    let data_missing = !matches!(filesystem.data_status(), DataStatus::Found);
    debug_stage("Luna: flow 9 data status ready\r\n");
    if data_missing
        && matches!(
            selection.action,
            BootMenuAction::Continue | BootMenuAction::SystemImage | BootMenuAction::VerboseBoot
        )
    {
        log::warn!("Luna: LUNA-DATA is unavailable; entering Recovery Environment");
        selection = BootSelection {
            action: BootMenuAction::Recovery,
            target_index: catalog.default_target,
        };
    }

    let mut selected = match selection.action {
        BootMenuAction::Recovery => catalog
            .recovery
            .clone()
            .ok_or(BootError::RecoveryUnavailable),
        BootMenuAction::Factory => catalog.factory.clone().ok_or(BootError::Unsupported(
            "factory environment is unavailable on this installation",
        )),
        BootMenuAction::Continue | BootMenuAction::SystemImage | BootMenuAction::VerboseBoot => {
            catalog
                .targets
                .get(selection.target_index)
                .cloned()
                .ok_or(BootError::TargetNotFound)
        }
        BootMenuAction::ExternalBoot => unreachable!(),
    }?;
    debug_stage("Luna: flow 10 target selected\r\n");

    let mode = match selection.action {
        BootMenuAction::VerboseBoot => BootMode::Detailed,
        BootMenuAction::Recovery => BootMode::Recovery,
        BootMenuAction::Factory => BootMode::Factory,
        _ => BootMode::Normal,
    };

    debug_stage("Luna: flow 11 mode start\r\n");
    if selection.action == BootMenuAction::VerboseBoot {
        selected.kernel_cmdline = selected
            .kernel_cmdline
            .split_whitespace()
            .filter(|part| *part != "quiet" && !part.starts_with("loglevel="))
            .collect::<Vec<_>>()
            .join(" ");
        selected.kernel_cmdline.push_str(
            " console=tty0 console=ttyS0,115200n8 loglevel=7 ignore_loglevel initcall_debug",
        );
    }
    debug_stage("Luna: flow 12 candidates start\r\n");

    // A fallback is an atomic image+init+kernel tuple. Never pair a failed
    // target's manifest/image with another target's prepared kernel.
    let mut candidates = Vec::new();
    candidates.push(selected.clone());

    if selection.action == BootMenuAction::Recovery {
        if let Some(factory) = catalog.factory.as_ref()
            && !same_target(&selected, factory)
        {
            candidates.push(factory.clone());
        }
    } else if matches!(selection.action, BootMenuAction::Continue) {
        if let Some(reference) = catalog.boot_state.fallback.as_ref()
            && let Some(fallback) = catalog.target_for_ref(reference)
            && !same_target(&selected, &fallback)
        {
            candidates.push(fallback);
        }
        candidates.extend(
            catalog
                .targets
                .iter()
                .skip(selection.target_index + 1)
                .cloned(),
        );
    } else if matches!(
        selection.action,
        BootMenuAction::SystemImage | BootMenuAction::VerboseBoot
    ) {
        candidates.extend(
            catalog
                .targets
                .iter()
                .skip(selection.target_index + 1)
                .cloned(),
        );
    }
    debug_stage("Luna: flow 13 candidates ready\r\n");

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

    debug_stage("Luna: flow 14 kernel prepared\r\n");
    let attempt_id = BootAttemptMarker::next_attempt_id(
        previous_attempt,
        prepared.init_address,
        prepared.init_size,
        &prepared.kernel_digest,
    );
    let mut attempt = BootAttempt::new(attempt_id);
    attempt.advance(BootStage::BootloaderCompleted);

    debug_stage("Luna: flow 15 attempt ready\r\n");
    let manifest_bytes = filesystem.read_file(&target.manifest_path)?;
    debug_stage("Luna: flow 16 manifest read\r\n");
    let image_digest = filesystem.hash_file(&target.system_image_path)?;
    debug_stage("Luna: flow 17 image hash\r\n");
    if let Some(recovery_path) = target.recovery_data_path.as_deref() {
        let recovery_digest = filesystem.hash_file(recovery_path)?;
        target.recovery_data_digest = Some(recovery_digest);
        debug_stage("Luna: flow 17a recovery DATA hash\r\n");
    }
    let kernel_identity = PreparedIdentity {
        kernel_digest: prepared.kernel_digest,
    };
    let boot_state = BootState {
        fallback_depth: catalog.boot_state.fallback_depth,
        previous_attempt_failed: previous_attempt.is_some()
            || catalog.boot_state.previous_attempt_failed,
        previous_attempt_id: previous_attempt
            .map(|value| value.attempt_id)
            .unwrap_or(catalog.boot_state.attempt_id),
        failure_code: catalog.boot_state.failure_code,
    };
    debug_stage("Luna: flow 18 handoff build start\r\n");
    let luna_handoff = LunaHandoff::build(
        &target,
        mode,
        boot_state,
        attempt_id,
        filesystem.system_partition(),
        filesystem.data_partition(),
        &manifest_bytes,
        &image_digest,
        &kernel_identity,
        prepared.init_address,
        prepared.init_size,
        prepared.init_digest,
    )?;

    debug_stage("Luna: flow 19 handoff built\r\n");
    let progress = LunaBootProgress::allocate(attempt.attempt_id())?;
    debug_stage("Luna: flow 20 progress allocated\r\n");
    let e820_ext = E820Extension::allocate()?;
    debug_stage("Luna: flow 21 e820 allocated\r\n");
    let mut luna_handoff = luna_handoff;
    luna_handoff.link_next(progress.address);
    debug_stage("Luna: flow 22 chain linked\r\n");
    prepared.boot_params.set_setup_data(e820_ext.address)?;
    let transition_entry = transition_entry_address();
    let stack_pointer = current_stack_pointer();
    debug_stage("Luna: flow 23 e820 params set\r\n");
    let (page_table, page_table_pages) = prepare_identity_map(transition_entry, stack_pointer)?;
    debug_stage("Luna: flow 24 page table ready\r\n");

    let mut reserved = prepared.allocations.clone();
    reserved.push((luna_handoff.address, luna_handoff.allocation_pages));
    reserved.push((progress.address, progress.allocation_pages));
    reserved.push((e820_ext.address, e820_ext.allocation_pages));
    reserved.push((page_table, page_table_pages));
    debug_stage("Luna: flow 25 reserved ready\r\n");

    // Persist exactly one minimal checkpoint before handing control to the
    // kernel. Detailed progress remains volatile in `BootAttempt`.
    debug_stage("Luna: flow 26 marker begin\r\n");
    BootAttemptMarker::begin(attempt.attempt_id())?;
    debug_stage("Luna: flow 27 marker begun\r\n");
    attempt.advance(BootStage::KernelHandoff);

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

fn debug_stage(message: &str) {
    use uefi::proto::console::text::Output;
    if let Ok(handle) = boot::get_handle_for_protocol::<Output>()
        && let Ok(mut stdout) = open_protocol_exclusive::<Output>(handle)
    {
        let encoded = uefi::CString16::try_from(message).ok();
        if let Some(encoded) = encoded {
            let _ = stdout.output_string(&encoded);
        }
    }
}

fn same_target(left: &crate::target::BootTarget, right: &crate::target::BootTarget) -> bool {
    left.system_image_path == right.system_image_path
        && left.init_path == right.init_path
        && left.kernel_path == right.kernel_path
        && left.recovery_data_path == right.recovery_data_path
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
        .set_e820_from_map_reserved(&final_map, &reserved, &mut e820_ext, luna_handoff.address)
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
