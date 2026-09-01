//! Minimal Boot Menu for Project Luna.
//!
//! The menu is text-only because it is an exceptional pre-OS control surface.
//! Normal boot never enters it. Verbose Boot is an explicit diagnostic choice
//! and disables the graphical splash for that boot.

use alloc::string::String;
use uefi::proto::console::text::{Input, Key, Output, ScanCode};
use uefi::CString16;
use uefi::boot::ScopedProtocol;

use crate::target::BootTarget;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BootSelection {
    pub target_index: usize,
    pub verbose: bool,
}

pub struct BootMenu {
    stdout: ScopedProtocol<Output>,
    stdin: ScopedProtocol<Input>,
}

impl BootMenu {
    pub fn new(stdout: ScopedProtocol<Output>, stdin: ScopedProtocol<Input>) -> Self {
        Self { stdout, stdin }
    }

    pub fn show(&mut self, targets: &[BootTarget]) -> Option<BootSelection> {
        if targets.is_empty() {
            return None;
        }

        let mut selected = 0usize;
        let item_count = targets.len() + 1;

        loop {
            let _ = self.stdout.clear();
            let _ = self.stdout.set_cursor_position(0, 0);
            self.print("Project Luna\r\n\r\n");

            for (i, target) in targets.iter().enumerate() {
                self.print(if selected == i { "> " } else { "  " });
                let mut label = String::new();
                label.push_str(&target.name);
                if target.is_factory {
                    label.push_str(" [Factory]");
                }
                if target.is_recovery {
                    label.push_str(" [Recovery]");
                }
                label.push_str("\r\n");
                self.print(&label);
            }

            self.print(if selected == targets.len() { "> " } else { "  " });
            self.print("Verbose Boot\r\n\r\n");
            self.print("Arrow keys: Navigate    Enter: Select    Esc: Normal Boot\r\n");

            match self.stdin.read_key() {
                Ok(Some(Key::Special(ScanCode::UP))) => {
                    if selected > 0 {
                        selected -= 1;
                    }
                }
                Ok(Some(Key::Special(ScanCode::DOWN))) => {
                    if selected + 1 < item_count {
                        selected += 1;
                    }
                }
                Ok(Some(Key::Printable(c))) if c == '\r' || c == '\n' => {
                    if selected == targets.len() {
                        return Some(BootSelection {
                            target_index: 0,
                            verbose: true,
                        });
                    }
                    return Some(BootSelection {
                        target_index: selected,
                        verbose: false,
                    });
                }
                Ok(Some(Key::Special(ScanCode::ESCAPE))) => return None,
                Ok(_) => {}
                Err(_) => return None,
            }
        }
    }

    fn print(&mut self, text: &str) {
        if let Ok(s) = CString16::try_from(text) {
            let _ = self.stdout.output_string(&s);
        }
    }
}

pub fn show_error(stdout: &mut ScopedProtocol<Output>, message: &str) {
    let _ = stdout.clear();
    let _ = stdout.set_cursor_position(0, 0);
    if let Ok(s) = CString16::try_from("Project Luna - Boot Error\r\n\r\n") {
        let _ = stdout.output_string(&s);
    }
    if let Ok(s) = CString16::try_from(message) {
        let _ = stdout.output_string(&s);
    }
    if let Ok(s) = CString16::try_from("\r\n") {
        let _ = stdout.output_string(&s);
    }
}
