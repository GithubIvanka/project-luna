//! Network service boundary for Project Luna.
//!
//! Network configuration is implemented by NetworkManager in the desktop
//! payload. Luna keeps the device/connection model independent from that
//! implementation.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkState {
    Offline,
    Connected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkDevice {
    name: String,
    address: Option<String>,
    state: NetworkState,
}

impl NetworkDevice {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            address: None,
            state: NetworkState::Offline,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn address(&self) -> Option<&str> {
        self.address.as_deref()
    }
    pub const fn state(&self) -> NetworkState {
        self.state
    }
    pub fn connect(&mut self, address: impl Into<String>) {
        self.address = Some(address.into());
        self.state = NetworkState::Connected;
    }
    pub fn disconnect(&mut self) {
        self.address = None;
        self.state = NetworkState::Offline;
    }
}

pub trait NetworkBackend {
    type Error;
    fn devices(&self) -> Result<Vec<NetworkDevice>, Self::Error>;
}
