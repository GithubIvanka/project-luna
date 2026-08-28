//! Keyboard input handling for boot key detection

use uefi::prelude::*;
use uefi::proto::console::text::Input;
use uefi::table::boot::{ScopedProtocol, SearchType};

/// Check if the B key was pressed during boot window
pub fn check_for_boot_key(boot_services: &BootServices) -> bool {
    // Try to get the console input protocol
    let stdin_handle = boot_services.get_handle_for_protocol::<Input>();

    let stdin_handle = match stdin_handle {
        Ok(handle) => handle,
        Err(_) => return false,
    };

    let mut stdin: ScopedProtocol<Input> = match boot_services.open_protocol_exclusive(stdin_handle) {
        Ok(protocol) => protocol,
        Err(_) => return false,
    };

    // Reset input buffer
    let _ = stdin.reset(false);

    // Check for key press (non-blocking)
    if let Ok(Some(key)) = stdin.read_key() {
        // Check if 'B' or 'b' was pressed
        match key {
            uefi::proto::console::text::Key::Printable(char) => {
                let ch = char.as_char();
                return ch == 'B' || ch == 'b';
            }
            _ => {}
        }
    }

    false
}

/// Wait for any key press with timeout
pub fn wait_for_key(boot_services: &BootServices, timeout_us: u64) -> bool {
    let stdin_handle = match boot_services.get_handle_for_protocol::<Input>() {
        Ok(handle) => handle,
        Err(_) => return false,
    };

    let mut stdin: ScopedProtocol<Input> = match boot_services.open_protocol_exclusive(stdin_handle) {
        Ok(protocol) => protocol,
        Err(_) => return false,
    };

    let _ = stdin.reset(false);

    // Simple busy-wait with timeout
    let start = boot_services.current_time();
    let start = match start {
        Ok(time) => time,
        Err(_) => return false,
    };

    loop {
        if let Ok(Some(_)) = stdin.read_key() {
            return true;
        }

        let current = match boot_services.current_time() {
            Ok(time) => time,
            Err(_) => return false,
        };

        // Check timeout (simplified - real implementation would calculate elapsed time)
        // For now, just return false after checking once
        return false;
    }
}
