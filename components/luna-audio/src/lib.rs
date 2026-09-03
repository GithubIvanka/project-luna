//! Audio service boundary for Project Luna.
//!
//! The immutable desktop payload provides PipeWire + WirePlumber as the audio
//! implementation. This crate keeps Luna-facing policy independent of that
//! implementation so a different backend can be introduced later.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Volume(u8);

impl Volume {
    pub fn new(value: u8) -> Self { Self(value.min(100)) }
    pub const fn get(self) -> u8 { self.0 }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioState {
    Unavailable,
    Ready,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioEndpoint {
    id: String,
    name: String,
    volume: Volume,
    muted: bool,
}

impl AudioEndpoint {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self { id: id.into(), name: name.into(), volume: Volume::new(100), muted: false }
    }
    pub fn id(&self) -> &str { &self.id }
    pub fn name(&self) -> &str { &self.name }
    pub const fn volume(&self) -> Volume { self.volume }
    pub const fn muted(&self) -> bool { self.muted }
    pub fn set_volume(&mut self, volume: Volume) { self.volume = volume; }
    pub fn set_muted(&mut self, muted: bool) { self.muted = muted; }
}

pub trait AudioBackend {
    type Error;
    fn state(&self) -> Result<AudioState, Self::Error>;
    fn endpoints(&self) -> Result<Vec<AudioEndpoint>, Self::Error>;
}
