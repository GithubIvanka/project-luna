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
    VerboseBoot,
    SystemImage,
    Recovery,
    Factory,
    ExternalBoot,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BootSelection {
    pub action: BootMenuAction,
    pub target_index: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum MenuEntry {
    Continue,
    VerboseBoot,
    SystemImageSelection,
    Recovery,
    Factory,
    ExternalBoot,
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
        let entries = [
            MenuEntry::Continue,
            MenuEntry::VerboseBoot,
            MenuEntry::SystemImageSelection,
            MenuEntry::Recovery,
            MenuEntry::Factory,
            MenuEntry::ExternalBoot,
        ];
        let mut selected = 0usize;

        loop {
            let _ = self.stdout.clear();
            let _ = self.stdout.set_cursor_position(0, 0);
            self.print("Project Luna\r\n\r\nBoot Menu\r\n\r\n");
            for (index, entry) in entries.iter().enumerate() {
                self.print(if selected == index { "> " } else { "  " });
                self.print(&self.entry_label(*entry, targets, default_target));
                self.print("\r\n");
            }
            self.print("\r\nArrow keys: Navigate    Enter: Select    Esc: Continue\r\n");

            match self.stdin.read_key() {
                Ok(Some(Key::Special(ScanCode::UP))) => selected = selected.saturating_sub(1),
                Ok(Some(Key::Special(ScanCode::DOWN))) => {
                    if selected + 1 < entries.len() { selected += 1; }
                }
                Ok(Some(Key::Printable(c))) if c == '\r' || c == '\n' => {
                    match entries[selected] {
                        MenuEntry::SystemImageSelection => {
                            if let Some(result) = self.select_system_image(targets, default_target) { return Some(result); }
                        }
                        entry => return Some(Self::selection_for(entry, default_target)),
                    }
                }
                Ok(Some(Key::Special(ScanCode::ESCAPE))) => {
                    return Some(BootSelection { action: BootMenuAction::Continue, target_index: default_target });
                }
                Ok(_) => {}
                Err(_) => return None,
            }
        }
    }

    fn select_system_image(&mut self, targets: &[BootTarget], default_target: usize) -> Option<BootSelection> {
        if targets.is_empty() {
            return None;
        }
        let mut selected = default_target.min(targets.len() - 1);
        loop {
            let _ = self.stdout.clear();
            let _ = self.stdout.set_cursor_position(0, 0);
            self.print("Project Luna\r\n\r\nSelect System Image\r\n\r\n");
            for (index, target) in targets.iter().enumerate() {
                self.print(if selected == index { "> " } else { "  " });
                let mut label = String::new();
                label.push_str(&target.name);
                label.push_str(" [");
                label.push_str(&target.system_version);
                label.push(']');
                if index == default_target { label.push_str(" [Current]"); }
                label.push_str("\r\n");
                self.print(&label);
            }
            self.print("\r\nArrow keys: Navigate    Enter: Select    Esc: Back\r\n");
            match self.stdin.read_key() {
                Ok(Some(Key::Special(ScanCode::UP))) => selected = selected.saturating_sub(1),
                Ok(Some(Key::Special(ScanCode::DOWN))) => {
                    if selected + 1 < targets.len() { selected += 1; }
                }
                Ok(Some(Key::Printable(c))) if c == '\r' || c == '\n' => {
                    return Some(BootSelection { action: BootMenuAction::SystemImage, target_index: selected });
                }
                Ok(Some(Key::Special(ScanCode::ESCAPE))) => return None,
                Ok(_) => {}
                Err(_) => return None,
            }
        }
    }

    fn entry_label(&self, entry: MenuEntry, targets: &[BootTarget], default_target: usize) -> String {
        let mut label = String::new();
        match entry {
            MenuEntry::Continue => label.push_str("Continue to Luna"),
            MenuEntry::VerboseBoot => label.push_str("Verbose Boot"),
            MenuEntry::SystemImageSelection => {
                label.push_str("System Image selection");
                if targets.is_empty() { label.push_str(" [Unavailable]"); }
                else if let Some(target) = targets.get(default_target) {
                    label.push_str(" ["); label.push_str(&target.system_version); label.push(']');
                }
            }
            MenuEntry::Recovery => label.push_str("Recovery Environment"),
            MenuEntry::Factory => label.push_str("Factory Environment"),
            MenuEntry::ExternalBoot => label.push_str("Boot from USB / External Device"),
        }
        label
    }

    fn selection_for(entry: MenuEntry, default_target: usize) -> BootSelection {
        match entry {
            MenuEntry::Continue | MenuEntry::SystemImageSelection => BootSelection { action: BootMenuAction::Continue, target_index: default_target },
            MenuEntry::VerboseBoot => BootSelection { action: BootMenuAction::VerboseBoot, target_index: default_target },
            MenuEntry::Recovery => BootSelection { action: BootMenuAction::Recovery, target_index: default_target },
            MenuEntry::Factory => BootSelection { action: BootMenuAction::Factory, target_index: default_target },
            MenuEntry::ExternalBoot => BootSelection { action: BootMenuAction::ExternalBoot, target_index: default_target },
        }
    }

    fn print(&mut self, text: &str) {
        if let Ok(value) = CString16::try_from(text) { let _ = self.stdout.output_string(&value); }
    }
}

pub fn show_error(stdout: &mut ScopedProtocol<Output>, message: &str) {
    let _ = stdout.clear();
    let _ = stdout.set_cursor_position(0, 0);
    if let Ok(s) = CString16::try_from("Project Luna - Boot Error\r\n\r\n") { let _ = stdout.output_string(&s); }
    if let Ok(s) = CString16::try_from(message) { let _ = stdout.output_string(&s); }
    if let Ok(s) = CString16::try_from("\r\n") { let _ = stdout.output_string(&s); }
}
