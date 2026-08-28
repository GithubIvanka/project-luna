//! Error types for luna-boot

use core::fmt;
use uefi::Status;

#[derive(Debug, Clone, Copy)]
pub enum BootError {
    /// UEFI operation failed
    UefiError(Status),
    /// Boot configuration not found
    ConfigNotFound,
    /// Invalid boot configuration
    InvalidConfig,
    /// No boot targets available
    NoBootTargets,
    /// Selected boot target not found
    TargetNotFound,
    /// Kernel load failed
    KernelLoadFailed,
    /// Memory allocation failed
    MemoryAllocationFailed,
    /// Filesystem access failed
    FilesystemError,
    /// ExitBootServices failed
    ExitBootServicesFailed,
    /// Invalid kernel format
    InvalidKernelFormat,
}

impl fmt::Display for BootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UefiError(status) => write!(f, "UEFI error: {:?}", status),
            Self::ConfigNotFound => write!(f, "Boot configuration not found"),
            Self::InvalidConfig => write!(f, "Invalid boot configuration"),
            Self::NoBootTargets => write!(f, "No boot targets available"),
            Self::TargetNotFound => write!(f, "Selected boot target not found"),
            Self::KernelLoadFailed => write!(f, "Failed to load kernel"),
            Self::MemoryAllocationFailed => write!(f, "Memory allocation failed"),
            Self::FilesystemError => write!(f, "Filesystem access error"),
            Self::ExitBootServicesFailed => write!(f, "Failed to exit boot services"),
            Self::InvalidKernelFormat => write!(f, "Invalid kernel format"),
        }
    }
}

impl From<uefi::Error> for BootError {
    fn from(err: uefi::Error) -> Self {
        Self::UefiError(err.status())
    }
}

pub type BootResult<T> = Result<T, BootError>;
