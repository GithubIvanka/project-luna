//! Error types for luna-boot

use core::fmt;
use uefi::Status;

#[derive(Debug, Clone, Copy)]
pub enum BootError {
    UefiError(Status),
    ConfigNotFound,
    InvalidConfig,
    NoBootTargets,
    TargetNotFound,
    KernelLoadFailed,
    MemoryAllocationFailed,
    FilesystemError,
    InvalidFilesystem,
    InvalidKernel,
    InvalidKernelFormat,
    ExitBootServicesFailed,
    RecoveryUnavailable,
    Unsupported(&'static str),
}

impl fmt::Display for BootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UefiError(status) => write!(f, "UEFI error: {:?}", status),
            Self::ConfigNotFound => write!(f, "boot configuration not found"),
            Self::InvalidConfig => write!(f, "invalid boot configuration"),
            Self::NoBootTargets => write!(f, "no boot targets available"),
            Self::TargetNotFound => write!(f, "selected boot target not found"),
            Self::KernelLoadFailed => write!(f, "failed to load kernel"),
            Self::MemoryAllocationFailed => write!(f, "memory allocation failed"),
            Self::FilesystemError => write!(f, "filesystem access error"),
            Self::InvalidFilesystem => write!(f, "invalid or unsupported filesystem"),
            Self::InvalidKernel => write!(f, "invalid Linux kernel"),
            Self::InvalidKernelFormat => write!(f, "invalid kernel format"),
            Self::ExitBootServicesFailed => write!(f, "failed to exit boot services"),
            Self::RecoveryUnavailable => write!(f, "recovery environment unavailable"),
            Self::Unsupported(feature) => write!(f, "unsupported: {}", feature),
        }
    }
}

impl From<uefi::Error> for BootError {
    fn from(err: uefi::Error) -> Self {
        Self::UefiError(err.status())
    }
}

pub type BootResult<T> = Result<T, BootError>;
