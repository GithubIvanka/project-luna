//! Low-level filesystem primitives for Project Luna.
//!
//! This crate deliberately does not contain authorization policy, logical-root
//! mapping, application sandbox policy, configuration policy, or bundle logic.
//! It is the lowest-level filesystem abstraction in the Luna architecture.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Error returned by a low-level filesystem operation.
#[derive(Debug)]
pub enum FsError {
    /// The underlying operating-system filesystem operation failed.
    Io(io::Error),
}

impl From<io::Error> for FsError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Opaque handle to an opened file.
///
/// Policy and logical-path decisions are intentionally outside this crate.
pub struct FileHandle(fs::File);

impl FileHandle {
    /// Creates a handle from an already opened standard-library file.
    pub fn from_std(file: fs::File) -> Self {
        Self(file)
    }

    /// Borrows the underlying standard-library file.
    pub fn as_std(&self) -> &fs::File {
        &self.0
    }

    /// Returns the underlying standard-library file.
    pub fn into_std(self) -> fs::File {
        self.0
    }
}

/// Basic filesystem metadata exposed without adding policy semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileMetadata {
    pub is_file: bool,
    pub is_directory: bool,
    pub len: u64,
}

/// Minimal filesystem interface used by higher Luna layers.
pub trait FileSystem {
    fn open(&self, path: &Path) -> Result<FileHandle, FsError>;
    fn create(&self, path: &Path) -> Result<FileHandle, FsError>;
    fn remove(&self, path: &Path) -> Result<(), FsError>;
    fn metadata(&self, path: &Path) -> Result<FileMetadata, FsError>;
}

/// Host filesystem implementation backed by `std::fs`.
#[derive(Debug, Default, Clone, Copy)]
pub struct HostFileSystem;

impl FileSystem for HostFileSystem {
    fn open(&self, path: &Path) -> Result<FileHandle, FsError> {
        Ok(FileHandle(fs::File::open(path)?))
    }

    fn create(&self, path: &Path) -> Result<FileHandle, FsError> {
        Ok(FileHandle(fs::File::create(path)?))
    }

    fn remove(&self, path: &Path) -> Result<(), FsError> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.is_dir() {
            fs::remove_dir(path)?;
        } else {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    fn metadata(&self, path: &Path) -> Result<FileMetadata, FsError> {
        let metadata = fs::metadata(path)?;
        Ok(FileMetadata {
            is_file: metadata.is_file(),
            is_directory: metadata.is_dir(),
            len: metadata.len(),
        })
    }
}

/// Returns a normalized owned path without resolving it through the host OS.
///
/// This is intentionally only a value helper. It does not implement Luna's
/// logical-root mapping or authorization rules.
pub fn owned_path(path: impl AsRef<Path>) -> PathBuf {
    path.as_ref().to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::{FileSystem, HostFileSystem};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("luna-fs-test-{unique}"))
    }

    #[test]
    fn host_filesystem_creates_reads_metadata_and_removes_file() {
        let path = temp_file();
        let fs = HostFileSystem;

        {
            let handle = fs.create(&path).expect("create file");
            assert!(handle.as_std().metadata().expect("metadata").is_file());
        }

        let metadata = fs.metadata(&path).expect("read metadata");
        assert!(metadata.is_file);
        assert_eq!(metadata.len, 0);

        let _ = fs.open(&path).expect("open file");
        fs.remove(&path).expect("remove file");
        assert!(!path.exists());
    }
}
