//! Luna UEFI bootloader entry point

#![no_std]
#![no_main]

extern crate alloc;

use log::{error, info};
use uefi::prelude::*;

mod boot;
mod boot_params;
mod config;
mod e820;
mod error;
mod ext4;
mod filesystem;
mod handoff;
mod kernel;
mod keyboard;
mod linux;
mod memory;
mod menu;
mod recovery;
mod target;

#[entry]
fn efi_main(image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).expect("failed to initialize UEFI services");

    info!("Luna bootloader starting");
    info!("UEFI revision: {:?}", system_table.uefi_revision());

    let boot_services = system_table.boot_services();

    match boot::boot_flow(image_handle, boot_services) {
        Ok(()) => {
            error!("boot flow returned unexpectedly");
            Status::ABORTED
        }
        Err(err) => {
            error!("boot failed: {}", err);
            Status::ABORTED
        }
    }
}
