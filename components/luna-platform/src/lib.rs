//! Public platform contract for Project Luna.
//!
//! This crate defines platform-neutral concepts shared by native Luna clients
//! and compatibility layers. It deliberately does not depend on a compositor,
//! toolkit, audio server, or Linux-specific IPC implementation.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformVersion {
    pub major: u16,
    pub minor: u16,
}

impl PlatformVersion {
    pub const V1: Self = Self { major: 1, minor: 0 };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlatformCapability {
    Audio,
    Ui,
    Settings,
    Clipboard,
    FileDialog,
    Permissions,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformInfo {
    version: PlatformVersion,
    name: String,
}

impl PlatformInfo {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            version: PlatformVersion::V1,
            name: name.into(),
        }
    }

    pub const fn version(&self) -> PlatformVersion {
        self.version
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Minimal discovery boundary for a Luna platform implementation.
///
/// IPC, process lifetime, permissions, and backend selection belong to the
/// service implementation. Clients should depend on this contract rather than
/// on a concrete toolkit or daemon.
pub trait PlatformBackend {
    type Error;

    fn info(&self) -> Result<PlatformInfo, Self::Error>;

    fn supports(&self, capability: PlatformCapability) -> Result<bool, Self::Error>;
}
