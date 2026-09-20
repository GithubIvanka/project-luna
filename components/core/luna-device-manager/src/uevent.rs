//! Minimal kernel device event monitor.
//!
//! Linux emits kobject uevents through NETLINK_KOBJECT_UEVENT. Luna only needs
//! a small parser and monitor for hotplug/reprobe; it does not need a full udev
//! database or rules engine in the native path.

use std::io;
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::time::Duration;

const BUFFER_SIZE: usize = 16 * 1024;
const UEVENT_GROUP: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Uevent {
    pub action: String,
    pub devpath: String,
    pub properties: Vec<(String, String)>,
}

impl Uevent {
    pub fn property(&self, name: &str) -> Option<&str> {
        self.properties
            .iter()
            .find_map(|(key, value)| (key == name).then_some(value.as_str()))
    }

    pub fn subsystem(&self) -> Option<&str> {
        self.property("SUBSYSTEM")
    }

    pub fn devname(&self) -> Option<&str> {
        self.property("DEVNAME")
    }
}

#[derive(Debug)]
pub struct UeventMonitor {
    fd: OwnedFd,
}

impl UeventMonitor {
    pub fn open() -> io::Result<Self> {
        let fd = unsafe {
            libc::socket(
                libc::AF_NETLINK,
                libc::SOCK_DGRAM | libc::SOCK_NONBLOCK,
                libc::NETLINK_KOBJECT_UEVENT,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }

        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        let mut address = MaybeUninit::<libc::sockaddr_nl>::zeroed();
        let address_ptr = address.as_mut_ptr();
        unsafe {
            (*address_ptr).nl_family = libc::AF_NETLINK as u16;
            (*address_ptr).nl_pid = 0;
            (*address_ptr).nl_groups = UEVENT_GROUP;
        }

        let result = unsafe {
            libc::bind(
                fd.as_raw_fd(),
                address_ptr.cast::<libc::sockaddr>(),
                std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
            )
        };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }

        // SAFETY: the structure was zero-initialized and fully populated above.
        let _address = unsafe { address.assume_init() };

        Ok(Self { fd })
    }

    pub fn receive(&self) -> io::Result<Option<Uevent>> {
        let mut buffer = [0u8; BUFFER_SIZE];
        let result = unsafe {
            libc::recv(
                self.fd.as_raw_fd(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                libc::MSG_DONTWAIT,
            )
        };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::WouldBlock {
                return Ok(None);
            }
            return Err(error);
        }
        if result == 0 {
            return Ok(None);
        }

        parse_message(&buffer[..result as usize]).map(Some)
    }

    pub fn wait(&self, timeout: Duration) -> io::Result<bool> {
        let mut descriptor = libc::pollfd {
            fd: self.fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let timeout_ms = timeout.as_millis().min(i32::MAX as u128) as i32;
        let result = unsafe { libc::poll(&mut descriptor, 1, timeout_ms) };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(result > 0 && descriptor.revents & libc::POLLIN != 0)
    }
}

fn parse_message(bytes: &[u8]) -> io::Result<Uevent> {
    let mut fields = bytes
        .split(|value| *value == 0)
        .filter(|field| !field.is_empty());

    let header = fields
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "uevent header missing"))?;
    let header = std::str::from_utf8(header)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "uevent header is not UTF-8"))?;
    let (action, devpath) = header
        .split_once('@')
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "uevent header is malformed"))?;

    let mut properties = Vec::new();
    for field in fields {
        let field = std::str::from_utf8(field).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "uevent property is not UTF-8")
        })?;
        if let Some((key, value)) = field.split_once('=') {
            properties.push((key.to_owned(), value.to_owned()));
        }
    }

    Ok(Uevent {
        action: action.to_owned(),
        devpath: devpath.to_owned(),
        properties,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_message;

    #[test]
    fn parses_kernel_uevent() {
        let message = b"add@/devices/virtual/input/event0\0ACTION=add\0SUBSYSTEM=input\0DEVNAME=input/event0\0\0";
        let event = parse_message(message).expect("parse");
        assert_eq!(event.action, "add");
        assert_eq!(event.devpath, "/devices/virtual/input/event0");
        assert_eq!(event.subsystem(), Some("input"));
        assert_eq!(event.devname(), Some("input/event0"));
    }
}
