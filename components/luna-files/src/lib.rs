//! Project Luna file manager boundary.
//!
//! Yazi is the initial filesystem engine. Its current release (v26.9.1) already
//! provides asynchronous I/O, task scheduling, a virtual filesystem and file
//! operations; Luna will progressively replace its terminal UI with a native
//! Wayland GUI while retaining the proven filesystem/task core.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileEntry {
    path: PathBuf,
    directory: bool,
    size: u64,
}

impl FileEntry {
    pub fn new(path: impl Into<PathBuf>, directory: bool, size: u64) -> Self {
        Self { path: path.into(), directory, size }
    }
    pub fn path(&self) -> &Path { &self.path }
    pub const fn is_directory(&self) -> bool { self.directory }
    pub const fn size(&self) -> u64 { self.size }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileOperation {
    Copy,
    Move,
    Delete,
    Rename,
    CreateDirectory,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileOperationState {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

pub trait FileBackend {
    type Error;
    fn read_directory(&self, path: &Path) -> Result<Vec<FileEntry>, Self::Error>;
    fn submit(&self, operation: FileOperation, source: &Path, target: Option<&Path>) -> Result<(), Self::Error>;
}
