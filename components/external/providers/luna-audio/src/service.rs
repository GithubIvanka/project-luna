use crate::{AudioBackend, AudioEndpoint, AudioState, Volume};

/// Policy-neutral facade over a concrete Luna audio backend.
///
/// The facade is intentionally small: backend-specific discovery, routing,
/// device access, and IPC remain outside this type. This lets the same service
/// model be reused by native backends and compatibility adapters.
pub struct AudioService<B> {
    backend: B,
}

impl<B> AudioService<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn into_backend(self) -> B {
        self.backend
    }
}

impl<B: AudioBackend> AudioService<B> {
    pub fn state(&self) -> Result<AudioState, B::Error> {
        self.backend.state()
    }

    pub fn endpoints(&self) -> Result<Vec<AudioEndpoint>, B::Error> {
        self.backend.endpoints()
    }

    pub fn set_volume(
        &mut self,
        endpoint_id: &str,
        volume: Volume,
    ) -> Result<(), B::Error> {
        self.backend.set_volume(endpoint_id, volume)
    }

    pub fn set_muted(&mut self, endpoint_id: &str, muted: bool) -> Result<(), B::Error> {
        self.backend.set_muted(endpoint_id, muted)
    }
}
