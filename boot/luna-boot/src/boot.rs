//! Main luna-boot orchestration.

use uefi::boot::{self, open_protocol_exclusive};
use uefi::proto::console::text::Input;

use crate::boot_key::boot_menu_requested;
use crate::discovery::BootCatalog;
use crate::error::{BootError, BootResult};
use crate::external::boot_first_external;
use crate::filesystem::SystemFilesystem;
use crate::handoff::KernelHandoff;
use crate::kernel::KernelLoader;
use crate::menu::{BootMenu, BootMenuAction, BootSelection};
use crate::paging::prepare_identity_map;
use crate::splash;
use crate::target::BootTarget;

pub fn boot_flow() -> BootResult<()> {
    let input_handle = boot::get_handle_for_protocol::<Input>()?;
    let mut input = open_protocol_exclusive::<Input>(input_handle)?;
    let menu_requested = boot_menu_requested(&mut input);
    drop(input);

    // An external-media request must remain usable even when the internal
    // SYSTEM filesystem is damaged or absent. Normal boot still requires it.
    let mut filesystem = match SystemFilesystem::open() {
        Ok(value) => Some(value),
        Err(error) if menu_requested => None,
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
    let mut target = match selection.action {
        BootMenuAction::Recovery => catalog
            .recovery
            .clone()
            .ok_or(BootError::RecoveryUnavailable)?,
        BootMenuAction::Factory => catalog
            .factory
            .clone()
            .ok_or(BootError::Unsupported(
                "factory environment is unavailable on this installation",
            ))?,
        BootMenuAction::Continue | BootMenuAction::SystemImage | BootMenuAction::VerboseBoot => catalog
            .targets
            .get(selection.target_index)
            .cloned()
            .ok_or(BootError::TargetNotFound)?,
        BootMenuAction::ExternalBoot => unreachable!(),
    };

    if selection.action == BootMenuAction::VerboseBoot {
        target.kernel_cmdline = target
            .kernel_cmdline
            .split_whitespace()
            .filter(|part| *part != "quiet" && !part.starts_with("loglevel="))
            .collect::<alloc::vec::Vec<_>>()
            .join(" ");
        target.kernel_cmdline.push_str(" loglevel=7 ignore_loglevel");
    }

    let prepared = if matches!(selection.action, BootMenuAction::Recovery | BootMenuAction::Factory) {
        KernelLoader::new(filesystem).prepare(&target)
    } else {
        match KernelLoader::new(filesystem).prepare(&target) {
            Ok(value) => Ok(value),
            Err(primary) => {
                let mut fallback = None;
                for candidate in catalog.targets.iter().skip(selection.target_index + 1) {
                    if let Ok(value) = KernelLoader::new(filesystem).prepare(candidate) {
                        fallback = Some(value);
                        break;
                    }
                }
                fallback.ok_or(primary)
            }
        }
    }?;

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
