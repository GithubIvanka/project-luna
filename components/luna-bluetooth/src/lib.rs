//! Bluetooth service boundary for Project Luna.
//!
//! BlueZ provides the Linux Bluetooth implementation. Luna owns the user-facing
//! model and policy so the desktop shell does not depend on bluetoothctl.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BluetoothState {
    Disabled,
    Powered,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BluetoothDevice {
    address: String,
    name: Option<String>,
    connected: bool,
}

impl BluetoothDevice {
    pub fn new(address: impl Into<String>, name: Option<String>) -> Self {
        Self { address: address.into(), name, connected: false }
    }
    pub fn address(&self) -> &str { &self.address }
    pub fn name(&self) -> Option<&str> { self.name.as_deref() }
    pub const fn connected(&self) -> bool { self.connected }
    pub fn set_connected(&mut self, connected: bool) { self.connected = connected; }
}

pub trait BluetoothBackend {
    type Error;
    fn state(&self) -> Result<BluetoothState, Self::Error>;
    fn devices(&self) -> Result<Vec<BluetoothDevice>, Self::Error>;
}
