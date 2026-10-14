//! Файловый движок самостоятельного графического Luna Files на закреплённом Yazi.
//!
//! Чтение каталогов и создание папок используют `yazi-fs` независимо от
//! терминального интерфейса Yazi. Его планировщик и глобальные настройки
//! не запускаются. Изменяющие операции закрепляют объекты дескрипторами Linux
//! и публикуют результат без перезаписи; удаление означает перенос в корзину HOME.

mod operations;
pub use operations::{OperationControl, OperationProgress};

use std::ffi::OsStr;
use std::io;
use std::path::{Component, Path, PathBuf};
use yazi_fs::engine::{DirReader, Engine, FileHolder, local::Local};
use yazi_shared::path::PathLike;

pub const YAZI_REVISION: &str = "8dd895c695a5950330c2623eb43debf323b60654";
const MAX_DIRECTORY_ENTRIES: usize = 10_000;

#[derive(Clone, Debug, Eq, PartialEq)]
/// Запись каталога с исходным путём; отображение имени не меняет его байты.
pub struct FileEntry {
    path: PathBuf,
    directory: bool,
    size: u64,
    symlink: bool,
}

impl FileEntry {
    pub fn new(path: impl Into<PathBuf>, directory: bool, size: u64) -> Self {
        Self {
            path: path.into(),
            directory,
            size,
            symlink: false,
        }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub const fn is_directory(&self) -> bool {
        self.directory
    }
    pub const fn size(&self) -> u64 {
        self.size
    }
    pub const fn is_symlink(&self) -> bool {
        self.symlink
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileOperation {
    Copy,
    Move,
    /// Перенос в корзину домашнего каталога, без безвозвратного удаления.
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

/// Граница между интерфейсом приложения и асинхронными файловыми операциями.
/// Все пути проверяются движком; GTK не выполняет файловый ввод-вывод напрямую.
pub trait FileBackend {
    type Error;
    fn read_directory(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<Vec<FileEntry>, Self::Error>>;
    fn submit(
        &self,
        operation: FileOperation,
        source: &Path,
        target: Option<&Path>,
    ) -> impl Future<Output = Result<(), Self::Error>>;
}

#[derive(Clone, Copy, Debug, Default)]
/// Реализация через Yazi Local с дополнительной политикой безопасных изменений.
pub struct YaziBackend;

pub(crate) fn require_absolute(path: &Path) -> io::Result<()> {
    if !path.is_absolute() || path.components().any(|p| matches!(p, Component::ParentDir)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Expected an absolute path without '..'",
        ));
    }
    Ok(())
}

impl YaziBackend {
    /// Создаёт одну папку в закреплённом родителе; имя не может содержать обход пути.
    pub async fn create_directory(&self, parent: &Path, name: &OsStr) -> io::Result<()> {
        require_absolute(parent)?;
        let mut parts = Path::new(name).components();
        if !matches!(parts.next(), Some(Component::Normal(part)) if part == name)
            || parts.next().is_some()
            || name.as_encoded_bytes().contains(&0)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Enter one folder name, without '/' or '..'",
            ));
        }
        operations::create_directory(parent, name).await
    }
}

impl FileBackend for YaziBackend {
    type Error = io::Error;

    async fn read_directory(&self, path: &Path) -> io::Result<Vec<FileEntry>> {
        require_absolute(path)?;
        // Чтение имеет предел размера, чтобы огромный каталог не заполнил память GUI.
        let mut reader = Local::regular(path).read_dir().await?;
        let mut entries = Vec::new();
        while let Some(entry) = reader.next().await? {
            if entries.len() == MAX_DIRECTORY_ENTRIES {
                return Err(io::Error::other(
                    "Directory exceeds the current 10000 entry limit",
                ));
            }
            let item_path = entry.path().to_os_owned()?;
            let file = match entry.file().await {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            let mut item = FileEntry::new(item_path, file.cha.is_dir(), file.cha.len);
            item.symlink = file.cha.is_link();
            entries.push(item);
        }
        entries.sort_by(|a, b| {
            (!a.directory, a.path.file_name()).cmp(&(!b.directory, b.path.file_name()))
        });
        Ok(entries)
    }

    async fn submit(
        &self,
        operation: FileOperation,
        source: &Path,
        target: Option<&Path>,
    ) -> io::Result<()> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        self.operate(
            operation,
            source,
            target,
            &home,
            &OperationControl::default(),
        )
        .await
        .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "luna-files-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn run(future: impl Future<Output = ()>) {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future);
    }

    #[test]
    fn yazi_lists_sorted_entries_and_metadata_without_tui() {
        let root = Directory::new();
        fs::write(root.0.join("a-file"), b"hello").unwrap();
        fs::create_dir(root.0.join("z-folder")).unwrap();
        run(async {
            let items = YaziBackend.read_directory(&root.0).await.unwrap();
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].path(), root.0.join("z-folder"));
            assert!(items[0].is_directory());
            assert_eq!(items[1].size(), 5);
        });
    }

    #[test]
    fn yazi_preserves_non_utf8_paths() {
        let root = Directory::new();
        let name = std::ffi::OsString::from_vec(vec![b'x', 0xff]);
        fs::write(root.0.join(&name), b"x").unwrap();
        run(async {
            assert_eq!(
                YaziBackend.read_directory(&root.0).await.unwrap()[0]
                    .path()
                    .file_name(),
                Some(name.as_os_str())
            );
        });
    }

    #[test]
    fn yazi_handles_directory_and_dangling_links() {
        let root = Directory::new();
        fs::create_dir(root.0.join("directory")).unwrap();
        symlink("directory", root.0.join("link")).unwrap();
        symlink("missing", root.0.join("broken")).unwrap();
        run(async {
            let items = YaziBackend.read_directory(&root.0).await.unwrap();
            assert_eq!(items.len(), 3);
            let link = items
                .iter()
                .find(|item| item.path().ends_with("link"))
                .unwrap();
            assert!(link.is_directory());
            assert!(link.is_symlink());
            let broken = items
                .iter()
                .find(|item| item.path().ends_with("broken"))
                .unwrap();
            assert!(!broken.is_directory());
            assert!(broken.is_symlink());
        });
    }

    #[test]
    fn yazi_creates_folder_and_refuses_existing_target() {
        let root = Directory::new();
        run(async {
            YaziBackend
                .create_directory(&root.0, OsStr::new("New Folder"))
                .await
                .unwrap();
            let error = YaziBackend
                .create_directory(&root.0, OsStr::new("New Folder"))
                .await
                .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        });
        assert!(root.0.join("New Folder").is_dir());
    }

    #[test]
    fn names_cannot_escape_selected_parent() {
        let root = Directory::new();
        run(async {
            for name in [
                "",
                ".",
                "..",
                "../escape",
                "/tmp/escape",
                "a/b",
                "a/",
                "a\0b",
            ] {
                assert_eq!(
                    YaziBackend
                        .create_directory(&root.0, OsStr::new(name))
                        .await
                        .unwrap_err()
                        .kind(),
                    io::ErrorKind::InvalidInput,
                    "{name:?}"
                );
            }
        });
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
    }

    #[test]
    fn relative_and_parent_paths_are_explicit_errors() {
        run(async {
            for path in ["relative", "/tmp/../tmp"] {
                assert_eq!(
                    YaziBackend
                        .read_directory(Path::new(path))
                        .await
                        .unwrap_err()
                        .kind(),
                    io::ErrorKind::InvalidInput
                );
            }
        });
    }

    #[test]
    fn unavailable_directory_is_reported() {
        let root = Directory::new();
        run(async {
            assert_eq!(
                YaziBackend
                    .read_directory(&root.0.join("missing"))
                    .await
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::NotFound
            );
        });
    }

    #[test]
    fn conflicting_operations_never_modify_files() {
        let root = Directory::new();
        let source = root.0.join("source");
        let target = root.0.join("target");
        fs::write(&source, b"source").unwrap();
        fs::write(&target, b"target").unwrap();
        run(async {
            for operation in [
                FileOperation::Copy,
                FileOperation::Move,
                FileOperation::Rename,
            ] {
                assert_eq!(
                    YaziBackend
                        .submit(operation, &source, Some(&target))
                        .await
                        .unwrap_err()
                        .kind(),
                    io::ErrorKind::AlreadyExists
                );
            }
        });
        assert_eq!(fs::read(source).unwrap(), b"source");
        assert_eq!(fs::read(target).unwrap(), b"target");
    }
}
