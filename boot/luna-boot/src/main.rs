//! Luna UEFI bootloader entry point

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use log::{info, error};
use uefi::prelude::*;

mod boot;
mod config;
mod error;
mod filesystem;
mod kernel;
mod keyboard;
mod memory;
mod menu;
mod recovery;
mod target;

#[entry]
fn efi_main(image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    // Initialize logging
    uefi_services::init(&mut system_table).expect("Failed to initialize UEFI services");

    info!("Luna bootloader starting");
    info!("UEFI version: {:?}", system_table.uefi_revision());

    // Run boot flow
    let boot_services = system_table.boot_services();
    match boot::boot_flow(boot_services) {
        Ok(()) => {
            // Should never reach here (kernel.boot() never returns)
            error!("Boot flow returned unexpectedly");
            Status::ABORTED
        }
        Err(err) => {
            error!("Boot failed: {}", err);

            // Try to show error to user
            // In real implementation, would show error screen

            Status::ABORTED
        }
    }
}
