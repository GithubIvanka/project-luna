//! Main luna-boot orchestration.

use uefi::boot::{self, open_protocol_exclusive};
use uefi::proto::console::text::{Input, Output};

use crate::boot_key::boot_menu_requested;
use crate::config::BootConfig;
use crate::error::{BootError, BootResult};
use crate::filesystem::SystemFilesystem;
use crate::handoff::KernelHandoff;
use crate::kernel::KernelLoader;
use crate::menu::{BootMenu, BootSelection};
use crate::paging::prepare_identity_map;
use crate::splash;
use crate::target::BootTarget;

pub fn boot_flow() -> BootResult<()> {
    let config = BootConfig::default_config();

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
        menu.show(config.targets())
            .unwrap_or(BootSelection {
                target_index: config.default_target,
                verbose: false,
            })
    } else {
        // The splash belongs exclusively to the normal path. Verbose mode is
        // reachable only through Boot Menu and intentionally leaves the text
        // console visible for diagnostics.
        splash::show();
        BootSelection {
            target_index: config.default_target,
            verbose: false,
        }
    };

    let mut target = config
        .targets()
        .get(selection.target_index)
        .ok_or(BootError::TargetNotFound)?
        .clone();

    if selection.verbose {
        target.kernel_cmdline = target
            .kernel_cmdline
            .split_whitespace()
            .filter(|part| *part != "quiet")
            .collect::<alloc::vec::Vec<_>>()
            .join(" ");
        target.kernel_cmdline.push_str(" console=tty0 loglevel=7 ignore_loglevel");
    }

    let mut filesystem = SystemFilesystem::open()?;
    let prepared = match KernelLoader::new(&mut filesystem).prepare(&target) {
        Ok(kernel) => kernel,
        Err(primary) => {
            let factory = config
                .targets()
                .iter()
                .find(|candidate| candidate.is_factory && !candidate.is_recovery)
                .ok_or(primary)?;
            if core::ptr::eq(&target, factory) {
                return Err(primary);
            }
            KernelLoader::new(&mut filesystem).prepare(factory)?
        }
    };

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

    if !handoff.is_ready() {
        return Err(BootError::InvalidKernel);
    }

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
