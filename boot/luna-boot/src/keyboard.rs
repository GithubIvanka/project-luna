//! Non-blocking boot-key handling.
//!
//! The boot path must never introduce a fixed delay. Firmware input is sampled
//! once when luna-boot starts; if B is already pending, the menu is entered.

use uefi::prelude::*;
use uefi::proto::console::text::{Input, Key};
use uefi::table::boot::ScopedProtocol;

pub fn check_for_boot_key(boot_services: &BootServices) -> bool {
    let stdin_handle = match boot_services.get_handle_for_protocol::<Input>() {
        Ok(handle) => handle,
        Err(_) => return false,
    };

    let mut stdin: ScopedProtocol<Input> = match boot_services.open_protocol_exclusive(stdin_handle) {
        Ok(protocol) => protocol,
        Err(_) => return false,
    };

    // Do not reset the console here: reset would discard a B key that firmware
    // already placed in the input queue. read_key() is non-blocking.
    match stdin.read_key() {
        Ok(Some(Key::Printable(ch))) => {
            let ch = ch.as_char();
            ch == 'B' || ch == 'b'
        }
        _ => false,
    }
}
