//! Simple text-mode boot menu

use alloc::string::String;
use alloc::vec::Vec;
use uefi::prelude::*;
use uefi::proto::console::text::{Input, Output};
use uefi::table::boot::ScopedProtocol;
use crate::target::BootTarget;

/// Simple text menu for boot target selection
pub struct BootMenu<'a> {
    stdout: ScopedProtocol<'a, Output>,
    stdin: ScopedProtocol<'a, Input>,
}

impl<'a> BootMenu<'a> {
    pub fn new(
        stdout: ScopedProtocol<'a, Output>,
        stdin: ScopedProtocol<'a, Input>,
    ) -> Self {
        Self { stdout, stdin }
    }

    /// Display menu and get user selection
    pub fn show(&mut self, targets: &[BootTarget]) -> Option<usize> {
        let mut selected = 0;
        let _ = self.stdout.clear();

        loop {
            // Draw menu
            let _ = self.stdout.set_cursor_position(0, 0);
            let _ = self.stdout.output_string("Project Luna Boot Menu\n\n");

            for (i, target) in targets.iter().enumerate() {
                if i == selected {
                    let _ = self.stdout.output_string("> ");
                } else {
                    let _ = self.stdout.output_string("  ");
                }

                let mut label = String::new();
                label.push_str(&target.name);
                if target.is_recovery {
                    label.push_str(" [Recovery]");
                }
                label.push_str("\n");
                let _ = self.stdout.output_string(&label);
            }

            let _ = self.stdout.output_string("\nArrow keys: Navigate\n");
            let _ = self.stdout.output_string("Enter: Select\n");
            let _ = self.stdout.output_string("Esc: Boot default\n");

            // Read input
            let _ = self.stdin.reset(false);

            // Wait for key
            match self.stdin.read_key() {
                Ok(Some(key)) => {
                    use uefi::proto::console::text::Key;
                    match key {
                        Key::Printable(char) => {
                            let ch = char.as_char();
                            if ch == '\r' || ch == '\n' {
                                // Enter
                                return Some(selected);
                            } else if ch == '\x1b' {
                                // Escape
                                return None;
                            }
                        }
                        Key::Special(special) => {
                            use uefi::proto::console::text::Key::Special::*;
                            match special {
                                Up => {
                                    if selected > 0 {
                                        selected -= 1;
                                    }
                                }
                                Down => {
                                    if selected < targets.len() - 1 {
                                        selected += 1;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Ok(None) => {
                    // No key, continue loop
                }
                Err(_) => {
                    // Error reading key
                    return None;
                }
            }
        }
    }
}

/// Simple error display
pub fn show_error(
    stdout: &mut ScopedProtocol<Output>,
    message: &str,
) {
    let _ = stdout.clear();
    let _ = stdout.set_cursor_position(0, 0);
    let _ = stdout.output_string("Project Luna - Boot Error\n\n");
    let _ = stdout.output_string(message);
    let _ = stdout.output_string("\n\nPress any key to continue...");

    // In real implementation, wait for key
}
