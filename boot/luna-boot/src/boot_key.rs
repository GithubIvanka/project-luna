//! Non-blocking boot-menu key detection.

use uefi::proto::console::text::Input;
use uefi::table::boot::BootServices;

/// Read the current console input buffer once. There is deliberately no delay.
pub fn boot_menu_requested(_boot_services: &BootServices, input: &mut Input) -> bool {
    match input.read_key() {
        Ok(Some(key)) => matches!(key, uefi::proto::console::text::Key::Printable(c) if c == 'B' || c == 'b'),
        _ => false,
    }
}
