//! Main luna-boot orchestration.

use uefi::boot::{self, open_protocol_exclusive};
use uefi::proto::console::text::{Input, Output};

use crate::boot_key::boot_menu_requested;
use crate::config::BootConfig;
use crate::error::{BootError, BootResult};
use crate::filesystem::SystemFilesystem;
use crate::handoff::KernelHandoff;
use crate::kernel::KernelLoader;
use crate::menu::BootMenu;
use crate::paging::prepare_identity_map;
use crate::target::BootTarget;

pub fn boot_flow() -> BootResult<()> {
    let config = BootConfig::default_config();

    let input_handle = boot::get_handle_for_protocol::<Input>()?;
    let mut input = open_protocol_exclusive::<Input>(input_handle)?;
    let menu_requested = boot_menu_requested(&mut input);
    drop(input);

    let target = if menu_requested {
        let stdout_handle = boot::get_handle_for_protocol::<Output>()?;
        let stdin_handle = boot::get_handle_for_protocol::<Input>()?;
        let stdout = open_protocol_exclusive::<Output>(stdout_handle)?;
        let stdin = open_protocol_exclusive::<Input>(stdin_handle)?;
        let mut menu = BootMenu::new(stdout, stdin);
        match menu.show(config.targets()) {
            Some(index) => config.targets().get(index).ok_or(BootError::TargetNotFound)?,
            None => config.default_target()?,
        }
    } else {
        config.default_target()?
    };

    let mut filesystem = SystemFilesystem::open()?;
    let prepared = match KernelLoader::new(&mut filesystem).prepare(target) {
        Ok(kernel) => kernel,
        Err(primary) => {
            let factory = config.targets().iter().find(|t| t.is_factory && !t.is_recovery)
                .ok_or(primary)?;
            if core::ptr::eq(target, factory) {
                return Err(primary);
            }
            KernelLoader::new(&mut filesystem).prepare(factory)?
        }
    };

    // All allocations that can affect the final memory map happen before this
    // point. The page tables are also allocated before ExitBootServices.
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

    // This is the only UEFI call after the final allocation. The returned map
    // is the map whose key was actually accepted by ExitBootServices.
    let final_map = unsafe { boot::exit_boot_services(None) };

    // From here on there are no UEFI calls, allocations, protocol drops or
    // logging. We only touch loader-owned memory and the final map returned by
    // uefi-rs.
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
