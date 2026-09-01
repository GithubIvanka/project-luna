//! Exceptional pre-OS control surface for Project Luna.
//!
//! Normal boot never enters this menu. It is shown only after the user
//! explicitly requests it during early `luna-boot.efi` startup.

use alloc::string::String;
use alloc::vec::Vec;

use uefi::boot::ScopedProtocol;
use uefi::proto::console::text::{Input, Key, Output, ScanCode};
use uefi::CString16;

use crate::target::BootTarget;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BootMenuAction {
    Continue,
    SystemImage,
    Recovery,
    Factory,
    ExternalBoot,
    VerboseBoot,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BootSelection {
    pub action: BootMenuAction,
    pub target_index: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum MenuEntry {
    Continue,
    SystemImage(usize),
    Recovery,
    Factory,
    ExternalBoot,
    VerboseBoot,
}

pub struct BootMenu {
    stdout: ScopedProtocol<Output>,
    stdin: ScopedProtocol<Input>,
}

impl BootMenu {
    pub fn new(stdout: ScopedProtocol<Output>, stdin: ScopedProtocol<Input>) -> Self {
        Self { stdout, stdin }
    }

    pub fn show(&mut self, targets: &[BootTarget], default_target: usize) -> Option<BootSelection> {
        let mut entries = Vec::new();
        entries.push(MenuEntry::Continue);
        for (index, _) in targets.iter().enumerate() {
            entries.push(MenuEntry::SystemImage(index));
        }
        entries.push(MenuEntry::Recovery);
        entries.push(MenuEntry::Factory);
        entries.push(MenuEntry::ExternalBoot);
        entries.push(MenuEntry::VerboseBoot);

        let mut selected = 0usize;
        loop {
            let _ = self.stdout.clear();
            let _ = self.stdout.set_cursor_position(0, 0);
            self.print("Project Luna\r\n\r\n");
            self.print("Boot Menu\r\n\r\n");

            for (index, entry) in entries.iter().enumerate() {
                self.print(if selected == index { "> " } else { "  " });
                let label = self.entry_label(*entry, targets, default_target);
                self.print(&label);
                self.print("\r\n");
            }

            self.print("\r\nArrow keys: Navigate    Enter: Select    Esc: Continue\r\n");

            match self.stdin.read_key() {
                Ok(Some(Key::Special(ScanCode::UP))) => {
                    selected = selected.saturating_sub(1);
                }
                Ok(Some(Key::Special(ScanCode::DOWN))) => {
                    if selected + 1 < entries.len() {
                        selected += 1;
                    }
                }
                Ok(Some(Key::Printable(c))) if c == '\r' || c == '\n' => {
                    return Some(self.selection_for(entries[selected], default_target));
                }
                Ok(Some(Key::Special(ScanCode::ESCAPE))) => {
                    return Some(BootSelection {
                        action: BootMenuAction::Continue,
                        target_index: default_target,
                    });
                }
                Ok(_) => {}
                Err(_) => return None,
            }
        }
    }

    fn entry_label(&self, entry: MenuEntry, targets: &[BootTarget], default_target: usize) -> String {
        let mut label = String::new();
        match entry {
            MenuEntry::Continue => label.push_str("Continue to Luna"),
            MenuEntry::SystemImage(index) => {
                label.push_str("System Image: ");
                if let Some(target) = targets.get(index) {
                    label.push_str(&target.name);
                    label.push_str(" ");
                    label.push_str("[");
                    label.push_str(&target.system_version);
                    label.push_str("]");
                    if index == default_target {
                        label.push_str(" [Current]");
                    }
                } else {
                    label.push_str("<invalid target>");
                }
            }
            MenuEntry::Recovery => label.push_str("Recovery Environment"),
            MenuEntry::Factory => label.push_str("Factory Environment"),
            MenuEntry::ExternalBoot => label.push_str("Boot from USB / External Device"),
            MenuEntry::VerboseBoot => label.push_str("Verbose Boot"),
        }
        label
    }

    fn selection_for(&self, entry: MenuEntry, default_target: usize) -> BootSelection {
        match entry {
            MenuEntry::Continue => BootSelection {
                action: BootMenuAction::Continue,
                target_index: default_target,
            },
            MenuEntry::SystemImage(index) => BootSelection {
                action: BootMenuAction::SystemImage,
                target_index: index,
            },
            MenuEntry::Recovery => BootSelection {
                action: BootMenuAction::Recovery,
                target_index: default_target,
            },
            MenuEntry::Factory => BootSelection {
                action: BootMenuAction::Factory,
                target_index: default_target,
            },
            MenuEntry::ExternalBoot => BootSelection {
                action: BootMenuAction::ExternalBoot,
                target_index: default_target,
            },
            MenuEntry::VerboseBoot => BootSelection {
                action: BootMenuAction::VerboseBoot,
                target_index: default_target,
            },
        }
    }

    fn print(&mut self, text: &str) {
        if let Ok(value) = CString16::try_from(text) {
            let _ = self.stdout.output_string(&value);
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
