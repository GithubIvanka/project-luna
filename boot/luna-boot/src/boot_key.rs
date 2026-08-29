//! Non-blocking boot-menu key detection.

use uefi::proto::console::text::{Input, Key};

/// Sample the firmware input queue once. There is deliberately no timeout or
/// stall: normal boot must not pay a boot-menu delay.
pub fn boot_menu_requested(input: &mut Input) -> bool {
    matches!(
        input.read_key(),
        Ok(Some(Key::Printable(c))) if c == 'B' || c == 'b'
    )
}
