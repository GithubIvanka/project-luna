//! Main luna-boot orchestration.

use uefi::boot::{self, open_protocol_exclusive};
use uefi::proto::console::text::{Input, Output};

use crate::boot_key::boot_menu_requested;
use crate::discovery::BootCatalog;
use crate::error::{BootError, BootResult};
use crate::filesystem::SystemFilesystem;
use crate::handoff::KernelHandoff;
use crate::kernel::KernelLoader;
use crate::menu::{BootMenu, BootMenuAction, BootSelection};
use crate::paging::prepare_identity_map;
use crate::splash;
use crate::target::BootTarget;

pub fn boot_flow() -> BootResult<()> {
    let mut filesystem = SystemFilesystem::open()?;
    let catalog = BootCatalog::discover(&mut filesystem)?;

    let input_handle = boot::get_handle_for_protocol::<Input>()?;
    let mut input = open_protocol_exclusive::<Input>(input_handle)?;
    let menu_requested = boot_menu_requested(&mut input);
    drop(input);

    let selection = if menu_requested {
        let stdout_handle = boot::get_handle_for_protocol::<Output>()?;
        let stdin_handle = boot::get_handle_for_protocol::<Input>()?;
        let stdout = open_protocol_exclusive::<Output>(stdout_handle)?;
        let stdin = open_protocol_exclusive::<Input>(stdin_handle)?;
        let mut menu = BootMenu::new(stdout, stdin);
        menu.show(&catalog.targets, catalog.default_target)
            .unwrap_or(BootSelection { action: BootMenuAction::Continue, target_index: catalog.default_target })
    } else {
        splash::show();
        BootSelection { action: BootMenuAction::Continue, target_index: catalog.default_target }
    };

    match selection.action {
        BootMenuAction::Recovery => {
            return Err(BootError::RecoveryUnavailable);
        }
        BootMenuAction::Factory => {
            return Err(BootError::Unsupported("factory selection requires persisted factory boot state"));
        }
        BootMenuAction::ExternalBoot => {
            return Err(BootError::Unsupported("external-device boot backend is not enabled in this build"));
        }
        BootMenuAction::Continue | BootMenuAction::SystemImage | BootMenuAction::VerboseBoot => {}
    }

    let selected_index = selection.target_index;
    let mut prepared = None;
    let mut target_for_handoff = None;
    for (index, candidate) in catalog.targets.iter().enumerate().skip(selected_index) {
        let mut target = candidate.clone();
        if selection.action == BootMenuAction::VerboseBoot && index == selected_index {
            target.kernel_cmdline = target
                .kernel_cmdline
                .split_whitespace()
                .filter(|part| *part != "quiet" && !part.starts_with("loglevel="))
                .collect::<alloc::vec::Vec<_>>()
                .join(" ");
            target.kernel_cmdline.push_str(" loglevel=7 ignore_loglevel");
        }
        match KernelLoader::new(&mut filesystem).prepare(&target) {
            Ok(kernel) => {
                prepared = Some(kernel);
                target_for_handoff = Some(target);
                if index != selected_index {
                    if selection.action != BootMenuAction::VerboseBoot {
                        // Soft fallback to the next older discovered System Image.
                        // No state is rewritten during fallback.
                    }
                }
                break;
            }
            Err(_) => continue,
        }
    }

    let prepared = prepared.ok_or(BootError::KernelLoadFailed)?;
    let _target = target_for_handoff.ok_or(BootError::TargetNotFound)?;

    let page_table = prepare_identity_map()?;
    let mut handoff = KernelHandoff {
        kernel_load_address: prepared.kernel_address,
        kernel_entry: prepared.kernel_entry,
        boot_params_address: prepared.boot_params_address,
        command_line_address: prepared.command_line_address,
        initrd_address: prepared.initrd_address,
        initrd_size: prepared.initrd_size,
        setup: prepared.setup,
        boot_params: prepared.boot_params,
        page_table,
    };

    if !handoff.is_ready() { return Err(BootError::InvalidKernel); }

    let final_map = unsafe { boot::exit_boot_services(None) };
    handoff.boot_params.set_e820_from_map(&final_map)?;
    unsafe {
        core::ptr::copy_nonoverlapping(
            handoff.boot_params.as_bytes().as_ptr(),
            handoff.boot_params_address as *mut u8,
            handoff.boot_params.as_bytes().len(),
        );
        handoff.enter();
    }
}

pub fn handle_boot_error(_error: BootError, _target: &BootTarget) -> BootResult<()> {
    Err(BootError::Unsupported("boot error display must occur before ExitBootServices"))
}
