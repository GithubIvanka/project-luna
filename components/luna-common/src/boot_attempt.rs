//! Shared in-memory boot progress model.
//!
//! Boot progress is intentionally volatile. Persistent storage only needs a
//! minimal marker that a boot attempt was started and has not reached success.

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(u8)]
pub enum BootStage {
    BootloaderLoaded = 1,
    BootloaderCompleted = 2,
    KernelHandoff = 3,
    KernelStarted = 4,
    InitStarted = 5,
    InitReady = 6,
    SystemRuntimeStarted = 7,
    Success = 8,
}

impl BootStage {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Success)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootAttemptProgress {
    attempt_id: u64,
    stage: BootStage,
}

impl BootAttemptProgress {
    pub const fn new(attempt_id: u64) -> Self {
        Self {
            attempt_id,
            stage: BootStage::BootloaderLoaded,
        }
    }

    pub const fn attempt_id(self) -> u64 {
        self.attempt_id
    }

    pub const fn stage(self) -> BootStage {
        self.stage
    }

    pub fn advance(&mut self, stage: BootStage) -> bool {
        if stage >= self.stage {
            self.stage = stage;
            true
        } else {
            false
        }
    }

    pub const fn succeeded(self) -> bool {
        self.stage.is_terminal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_is_monotonic() {
        let mut progress = BootAttemptProgress::new(42);
        assert_eq!(progress.stage(), BootStage::BootloaderLoaded);
        assert!(progress.advance(BootStage::KernelStarted));
        assert_eq!(progress.stage(), BootStage::KernelStarted);
        assert!(!progress.advance(BootStage::InitStarted));
        assert_eq!(progress.stage(), BootStage::KernelStarted);
        assert!(progress.advance(BootStage::Success));
        assert!(progress.succeeded());
    }
}
