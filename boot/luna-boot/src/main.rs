#![no_main]
#![no_std]

extern crate alloc;

use uefi::prelude::*;
use uefi::boot::open_protocol_exclusive;
use uefi::proto::console::text::Output;

mod block;
mod boot;
mod boot_key;
mod boot_params;
mod discovery;
mod e820;
mod error;
mod ext4;
mod external;
mod filesystem;
mod gpt;
mod handoff;
mod kernel;
mod linux;
mod menu;
mod paging;
mod splash;
mod target;

#[entry]
fn efi_main() -> Status {
    uefi::helpers::init().expect("failed to initialize UEFI services");
    match boot::boot_flow() {
        Ok(()) => Status::SUCCESS,
        Err(error) => {
            if let Ok(handle) = uefi::boot::get_handle_for_protocol::<Output>() {
                if let Ok(mut stdout) = open_protocol_exclusive::<Output>(handle) {
                    let message = alloc::format!("{error}\r\n\r\nPress any key to return to firmware.\r\n");
                    menu::show_error(&mut stdout, &message);
                }
            }
            log::error!("Luna boot failed: {error}");
            Status::ABORTED
        }
    }
}
