#![no_main]
#![no_std]

extern crate alloc;

use uefi::prelude::*;

mod block;
mod boot;
mod boot_key;
mod boot_params;
mod config;
mod e820;
mod error;
mod ext4;
mod filesystem;
mod gpt;
mod handoff;
mod kernel;
mod linux;
mod menu;
mod paging;
mod target;

#[entry]
fn efi_main(_image_handle: Handle, _system_table: SystemTable<Boot>) -> Status {
    uefi::helpers::init().expect("failed to initialize UEFI services");
    match boot::boot_flow() {
        Ok(()) => Status::SUCCESS,
        Err(error) => {
            log::error!("Luna boot failed: {error}");
            Status::ABORTED
        }
    }
}
