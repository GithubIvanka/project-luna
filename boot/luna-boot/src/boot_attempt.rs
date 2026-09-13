//! Persistent crash marker and volatile progress tracking for one boot attempt.
//!
//! The marker is intentionally tiny. It answers one question that must survive
//! a kernel panic, power loss, or reset: did the previous boot reach success?
//! Detailed progress remains in memory and is not persisted at every stage.

use alloc::boxed::Box;
use blake3::Hasher;
use uefi::cstr16;
use uefi::runtime::{self, VariableAttributes, VariableVendor};
use uefi::Status;

use luna_common::{BootAttemptProgress, BootStage};

use crate::error::{BootError, BootResult};

const VARIABLE_NAME: &uefi::CStr16 = cstr16!("LunaBootAttempt");
const MAGIC: &[u8; 8] = b"LUNABT01";
const FORMAT: u8 = 1;
const STATUS_IN_PROGRESS: u8 = 1;
const PAYLOAD_SIZE: usize = 56;

const ATTRIBUTES: VariableAttributes = VariableAttributes::NON_VOLATILE
    .union(VariableAttributes::BOOTSERVICE_ACCESS)
    .union(VariableAttributes::RUNTIME_ACCESS);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootAttemptMarker {
    pub attempt_id: u64,
}

impl BootAttemptMarker {
    pub fn read() -> BootResult<Option<Self>> {
        let vendor = VariableVendor::GLOBAL_VARIABLE;
        let value = match runtime::get_variable_boxed(VARIABLE_NAME, &vendor) {
            Ok((value, _)) => value,
            Err(error) if error.status() == Status::NOT_FOUND => return Ok(None),
            Err(error) => return Err(BootError::from(error)),
        };

        if value.len() != PAYLOAD_SIZE || &value[..8] != MAGIC {
            log::warn!("Luna: ignoring invalid persistent boot-attempt marker");
            return Ok(None);
        }
        if value[8] != FORMAT || value[9] != STATUS_IN_PROGRESS {
            log::warn!("Luna: ignoring unknown persistent boot-attempt marker state");
            return Ok(None);
        }

        let attempt_id = u64::from_le_bytes(value[12..20].try_into().unwrap());
        let stored_checksum: [u8; 32] = value[24..56].try_into().unwrap();
        let mut checked = value.to_vec();
        checked[24..56].fill(0);
        let checksum = *blake3::hash(&checked).as_bytes();
        if checksum != stored_checksum {
            log::warn!("Luna: ignoring corrupted persistent boot-attempt marker");
            return Ok(None);
        }

        Ok(Some(Self { attempt_id }))
    }

    pub fn begin(attempt_id: u64) -> BootResult<Self> {
        let mut payload = [0u8; PAYLOAD_SIZE];
        payload[..8].copy_from_slice(MAGIC);
        payload[8] = FORMAT;
        payload[9] = STATUS_IN_PROGRESS;
        payload[12..20].copy_from_slice(&attempt_id.to_le_bytes());
        let checksum = *blake3::hash(&payload).as_bytes();
        payload[24..56].copy_from_slice(&checksum);

        runtime::set_variable(VARIABLE_NAME, &VariableVendor::GLOBAL_VARIABLE, ATTRIBUTES, &payload)
            .map_err(BootError::from)?;
        Ok(Self { attempt_id })
    }

    pub fn clear() -> BootResult<()> {
        runtime::delete_variable(VARIABLE_NAME, &VariableVendor::GLOBAL_VARIABLE)
            .map_err(BootError::from)
    }

    pub fn next_attempt_id(
        previous: Option<Self>,
        init_address: u64,
        init_size: usize,
        kernel_digest: &[u8; 32],
    ) -> u64 {
        if let Some(previous) = previous {
            return previous.attempt_id.saturating_add(1).max(1);
        }

        let mut hasher = Hasher::new();
        hasher.update(b"Project Luna boot attempt v1");
        hasher.update(&init_address.to_le_bytes());
        hasher.update(&(init_size as u64).to_le_bytes());
        hasher.update(kernel_digest);
        let digest = *hasher.finalize().as_bytes();
        u64::from_le_bytes(digest[..8].try_into().unwrap()).max(1)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootAttempt {
    progress: BootAttemptProgress,
}

impl BootAttempt {
    pub fn new(attempt_id: u64) -> Self {
        Self {
            progress: BootAttemptProgress::new(attempt_id),
        }
    }

    pub const fn attempt_id(self) -> u64 {
        self.progress.attempt_id()
    }

    pub const fn stage(self) -> BootStage {
        self.progress.stage()
    }

    pub fn advance(&mut self, stage: BootStage) -> bool {
        self.progress.advance(stage)
    }

    pub const fn succeeded(self) -> bool {
        self.progress.succeeded()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_attempt_id_is_stable_for_the_same_inputs() {
        let digest = [7u8; 32];
        let left = BootAttemptMarker::next_attempt_id(None, 0x1000, 0x2000, &digest);
        let right = BootAttemptMarker::next_attempt_id(None, 0x1000, 0x2000, &digest);
        assert_eq!(left, right);
        assert_ne!(left, 0);
    }

    #[test]
    fn previous_attempt_id_is_incremented() {
        let digest = [7u8; 32];
        let id = BootAttemptMarker::next_attempt_id(
            Some(BootAttemptMarker { attempt_id: 41 }),
            0x1000,
            0x2000,
            &digest,
        );
        assert_eq!(id, 42);
    }

    #[test]
    fn volatile_progress_is_monotonic() {
        let mut attempt = BootAttempt::new(123);
        assert_eq!(attempt.stage(), BootStage::BootloaderLoaded);
        assert!(attempt.advance(BootStage::BootloaderCompleted));
        assert!(attempt.advance(BootStage::KernelHandoff));
        assert!(!attempt.advance(BootStage::KernelStarted.min(BootStage::KernelHandoff)));
    }
}
