//! Backend-neutral audio contract for Project Luna.
//!
//! The first system integration may continue to use PipeWire + WirePlumber,
//! but applications and Luna services must depend on this contract rather than
//! on a concrete audio server.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Volume(u8);

impl Volume {
    pub fn new(value: u8) -> Self {
        Self(value.min(100))
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioState {
    Unavailable,
    Ready,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioDirection {
    Input,
    Output,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioEndpoint {
    id: String,
    name: String,
    direction: AudioDirection,
    volume: Volume,
    muted: bool,
}

impl AudioEndpoint {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        direction: AudioDirection,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            direction,
            volume: Volume::new(100),
            muted: false,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn direction(&self) -> AudioDirection {
        self.direction
    }

    pub const fn volume(&self) -> Volume {
        self.volume
    }

    pub const fn muted(&self) -> bool {
        self.muted
    }

    pub fn set_volume(&mut self, volume: Volume) {
        self.volume = volume;
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }
}

/// Backend interface used by Luna's audio service.
///
/// Implementations may talk to ALSA directly, use a compatibility server such
/// as PulseAudio, or temporarily bridge through PipeWire. This choice is not
/// visible to callers of the Luna contract.
pub trait AudioBackend {
    type Error;

    fn state(&self) -> Result<AudioState, Self::Error>;
    fn endpoints(&self) -> Result<Vec<AudioEndpoint>, Self::Error>;
    fn set_volume(&mut self, endpoint_id: &str, volume: Volume) -> Result<(), Self::Error>;
    fn set_muted(&mut self, endpoint_id: &str, muted: bool) -> Result<(), Self::Error>;
}
