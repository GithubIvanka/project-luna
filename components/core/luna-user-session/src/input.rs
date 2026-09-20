//! Minimal Linux evdev input backend for UserSession.
//!
//! This module intentionally implements only the contracts needed by the
//! Luna graphical session. It does not reproduce libinput's full policy,
//! device database, gesture stack, calibration or desktop integration.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

const INPUT_CLASS: &str = "/sys/class/input";
const INPUT_DEV: &str = "/dev/input";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputDeviceKind {
    Keyboard,
    Pointer,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputDevice {
    pub event_node: PathBuf,
    pub sysfs_path: PathBuf,
    pub name: String,
    pub kind: InputDeviceKind,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct InputEvent {
    pub sec: i64,
    pub usec: i64,
    pub event_type: u16,
    pub code: u16,
    pub value: i32,
}

impl InputEvent {
    pub const KEY: u16 = 0x01;
    pub const REL: u16 = 0x02;
    pub const ABS: u16 = 0x03;
    pub const SYN: u16 = 0x00;

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != std::mem::size_of::<Self>() {
            return None;
        }
        let sec = i64::from_ne_bytes(bytes[0..8].try_into().ok()?);
        let usec = i64::from_ne_bytes(bytes[8..16].try_into().ok()?);
        let event_type = u16::from_ne_bytes(bytes[16..18].try_into().ok()?);
        let code = u16::from_ne_bytes(bytes[18..20].try_into().ok()?);
        let value = i32::from_ne_bytes(bytes[20..24].try_into().ok()?);
        Some(Self {
            sec,
            usec,
            event_type,
            code,
            value,
        })
    }
}

pub struct InputDeviceHandle {
    device: InputDevice,
    file: File,
}

impl std::fmt::Debug for InputDeviceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputDeviceHandle")
            .field("device", &self.device)
            .finish_non_exhaustive()
    }
}

impl InputDeviceHandle {
    pub fn device(&self) -> &InputDevice {
        &self.device
    }

    pub fn set_nonblocking(&self, enabled: bool) -> io::Result<()> {
        let fd = self.file.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        let next = if enabled {
            flags | libc::O_NONBLOCK
        } else {
            flags & !libc::O_NONBLOCK
        };
        if unsafe { libc::fcntl(fd, libc::F_SETFL, next) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub fn read_event(&mut self) -> io::Result<Option<InputEvent>> {
        let mut bytes = [0u8; std::mem::size_of::<InputEvent>()];
        match self.file.read(&mut bytes) {
            Ok(0) => Ok(None),
            Ok(len) if len == bytes.len() => Ok(InputEvent::decode(&bytes)),
            Ok(_) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "short evdev input_event read",
            )),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(error),
        }
    }
}

#[derive(Debug, Default)]
pub struct InputBackend {
    devices: Vec<InputDevice>,
}

impl InputBackend {
    pub fn discover() -> io::Result<Self> {
        let mut devices = Vec::new();
        let directory = fs::read_dir(INPUT_CLASS)?;
        for entry in directory {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("event") {
                continue;
            }

            let event_node = Path::new(INPUT_DEV).join(name.as_ref());
            if !event_node.exists() {
                continue;
            }

            let sysfs_path = fs::canonicalize(entry.path())?;
            let device_name = read_sysfs_name(&sysfs_path).unwrap_or_else(|| name.to_string());
            let kind = classify_device(&sysfs_path)?;

            devices.push(InputDevice {
                event_node,
                sysfs_path,
                name: device_name,
                kind,
            });
        }

        devices.sort_by(|left, right| left.event_node.cmp(&right.event_node));
        Ok(Self { devices })
    }

    pub fn devices(&self) -> &[InputDevice] {
        &self.devices
    }

    pub fn open(&self, index: usize) -> io::Result<InputDeviceHandle> {
        let device = self.devices.get(index).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "input device index out of range")
        })?;
        let file = OpenOptions::new().read(true).open(&device.event_node)?;
        Ok(InputDeviceHandle {
            device: device.clone(),
            file,
        })
    }
}

fn read_sysfs_name(sysfs_path: &Path) -> Option<String> {
    let device_name = sysfs_path.join("device/name");
    fs::read_to_string(device_name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn classify_device(sysfs_path: &Path) -> io::Result<InputDeviceKind> {
    let capabilities = sysfs_path.join("device/capabilities");
    let key = capability_words(&capabilities.join("key"))?;
    let relative = capability_words(&capabilities.join("rel"))?;
    let absolute = capability_words(&capabilities.join("abs"))?;

    let has_key = bit_is_set(&key, 30); // KEY_A
    let has_button = bit_is_set(&key, 272); // BTN_MOUSE
    let has_rel = bit_is_set(&relative, 0) || bit_is_set(&relative, 1);
    let has_abs = absolute.iter().any(|&word| word != 0);

    // Some composite USB keyboard interfaces expose extra capability bits
    // (buttons/ABS) even though their primary role is keyboard input. Prefer
    // the presence of alpha-key capability when there is no relative motion.
    if has_key && !has_rel {
        Ok(InputDeviceKind::Keyboard)
    } else if has_button || has_rel || has_abs {
        Ok(InputDeviceKind::Pointer)
    } else {
        Ok(InputDeviceKind::Other)
    }
}

fn capability_words(path: &Path) -> io::Result<Vec<u64>> {
    let text = fs::read_to_string(path)?;
    text.split_whitespace()
        .map(|word| {
            u64::from_str_radix(word, 16)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        })
        .collect()
}

fn bit_is_set(words: &[u64], bit: usize) -> bool {
    let word = bit / 64;
    let offset = bit % 64;
    words
        .get(words.len().saturating_sub(word + 1))
        .is_some_and(|value| (value & (1u64 << offset)) != 0)
}

#[cfg(test)]
mod tests {
    use super::InputEvent;

    #[test]
    fn decodes_evdev_event() {
        let mut bytes = [0u8; 24];
        bytes[16..18].copy_from_slice(&InputEvent::KEY.to_ne_bytes());
        bytes[18..20].copy_from_slice(&30u16.to_ne_bytes());
        bytes[20..24].copy_from_slice(&1i32.to_ne_bytes());
        let event = InputEvent::decode(&bytes).expect("decode");
        assert_eq!(event.event_type, InputEvent::KEY);
        assert_eq!(event.code, 30);
        assert_eq!(event.value, 1);
    }
}
