//! Политика безопасных изменений файлов для Linux.
//! Родительские каталоги открываются по одному, без перехода по ссылкам.
//! Yazi работает через закреплённые пути /proc/self/fd. Системный renameat2
//! запрещает перезапись, которую обычное переименование Yazi может допустить.
use crate::{FileOperation, YaziBackend, require_absolute};
use std::{
    collections::HashSet,
    ffi::{CString, OsStr, OsString},
    fs::{self, File, Metadata},
    io,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, PermissionsExt},
        },
    },
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use yazi_fs::engine::{DirReader, Engine, FileBuilder, FileHolder, local::Local};
use yazi_shared::path::PathLike;

const MAX_NODES: u64 = 100_000;
const MAX_DEPTH: usize = 128;
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
struct Control {
    cancelled: AtomicBool,
    bytes: AtomicU64,
    items: AtomicU64,
}
#[derive(Clone, Default)]
/// Общий для GUI и рабочего потока сигнал отмены и счётчики прогресса.
/// Отмена кооперативная: она проверяется между порциями записи и перед переносом.
pub struct OperationControl(Arc<Control>);
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
/// Снимок счётчиков для интерфейса; число объектов учитывает начатые узлы дерева.
pub struct OperationProgress {
    pub bytes: u64,
    pub items: u64,
}
impl OperationControl {
    /// Запрашивает остановку; не прерывает принудительно уже выполняющийся вызов ядра.
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Release);
    }
    pub fn progress(&self) -> OperationProgress {
        OperationProgress {
            bytes: self.0.bytes.load(Ordering::Relaxed),
            items: self.0.items.load(Ordering::Relaxed),
        }
    }
    fn check(&self) -> io::Result<()> {
        if self.0.cancelled.load(Ordering::Acquire) {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Operation cancelled",
            ))
        } else {
            Ok(())
        }
    }
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
fn cstring(name: &OsStr) -> io::Result<CString> {
    CString::new(name.as_bytes()).map_err(|_| invalid("NUL in path"))
}
fn name_of(path: &Path) -> io::Result<&OsStr> {
    require_absolute(path)?;
    path.file_name()
        .ok_or_else(|| invalid("A filesystem root cannot be modified"))
}
fn open_at(parent: &File, name: &OsStr, flags: i32) -> io::Result<File> {
    let name = cstring(name)?;
    // SAFETY: дескриптор родителя действителен, имя завершено NUL; успешный fd передаётся File.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: fd получен успешным openat; передаём владение ровно одному File.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}
// Путь /proc/self/fd обращается к уже открытому объекту, пока его File жив.
// Это связывает операции Yazi с закреплённым каталогом или проверенным файлом.
fn alias(file: &File) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()))
}
fn directory(path: &Path) -> io::Result<File> {
    require_absolute(path)?;
    // std::File::open в закреплённом musl SDK удаляет O_PATH из custom_flags.
    // Корень нужно закрепить без чтения: иначе изоляция отклонит запрос ReadDir.
    // SAFETY: постоянный путь завершён NUL; openat не требует параметра mode
    // без O_CREAT/O_TMPFILE. Возвращённый дескриптор проверяется до использования.
    let raw = unsafe {
        libc::syscall(
            libc::SYS_openat,
            libc::AT_FDCWD,
            c"/".as_ptr(),
            libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: успешный SYS_openat вернул новый fd; File становится его владельцем.
    let mut fd = unsafe { File::from_raw_fd(raw as i32) };
    for part in path.components() {
        if let Component::Normal(name) = part {
            fd = open_at(&fd, name, libc::O_PATH | libc::O_DIRECTORY)?;
        }
    }
    Ok(fd)
}
fn parent(path: &Path) -> io::Result<File> {
    name_of(path)?;
    directory(path.parent().ok_or_else(|| invalid("Missing parent"))?)
}
fn same(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino()
}
fn validate_parent(path: &Path, pinned: &File) -> io::Result<()> {
    if !same(&parent(path)?.metadata()?, &pinned.metadata()?) {
        return Err(io::Error::other("Selected parent directory changed"));
    }
    Ok(())
}
fn validate_directory(path: &Path, pinned: &File) -> io::Result<()> {
    if !same(&directory(path)?.metadata()?, &pinned.metadata()?) {
        return Err(io::Error::other("Selected directory changed"));
    }
    Ok(())
}
fn sync_directory(fd: &File) -> io::Result<()> {
    File::open(alias(fd))?.sync_all()
}
// Сверяем номера объектов при подъёме от назначения: строковое сравнение путей
// само по себе не обнаруживает все способы попадания внутрь исходной папки.
fn reject_descendant(source: &File, name: &OsStr, destination: &File) -> io::Result<()> {
    let path = alias(source).join(name);
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() {
        return Ok(());
    }
    let mut current = destination.try_clone()?;
    for _ in 0..MAX_DEPTH {
        let here = current.metadata()?;
        if same(&metadata, &here) {
            return Err(invalid("Cannot copy a directory into itself"));
        }
        let above = open_at(&current, OsStr::new(".."), libc::O_PATH | libc::O_DIRECTORY)?;
        if same(&here, &above.metadata()?) {
            return Ok(());
        }
        current = above;
    }
    Err(invalid("Destination ancestor limit exceeded"))
}
fn unchanged(a: &Metadata, b: &Metadata) -> bool {
    same(a, b)
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
// Окончательное перемещение атомарно отказывает, если имя назначения уже занято.
// Ошибка EXDEV возвращается вызывающему коду без копирования и удаления источника.
fn rename_no_replace(from: &File, name: &OsStr, to: &File, new: &OsStr) -> io::Result<()> {
    let name = cstring(name)?;
    let new = cstring(new)?;
    // SAFETY: дескрипторы живы; оба имени состоят из одного компонента и завершены NUL.
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            from.as_raw_fd(),
            name.as_ptr(),
            to.as_raw_fd(),
            new.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
fn mkdir_private(parent: &File, name: &OsStr) -> io::Result<()> {
    let name = cstring(name)?;
    // SAFETY: родитель жив, CString хранит имя до конца вызова; mode задаётся явно.
    if unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
fn token() -> String {
    format!(
        "luna-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

// Очистка обходит только закреплённые каталоги, не цели символических ссылок.
// Неизвестные объекты и подменённое внешнее имя временного каталога сохраняются.
fn clean(fd: &File, owned: &HashSet<(u64, u64)>, depth: usize) -> io::Result<()> {
    if depth > MAX_DEPTH + 2 {
        return Err(invalid("Staging cleanup depth exceeded"));
    }
    for entry in fs::read_dir(alias(fd))? {
        let name = entry?.file_name();
        let metadata = fs::symlink_metadata(alias(fd).join(&name))?;
        if !owned.contains(&(metadata.dev(), metadata.ino())) {
            return Err(io::Error::other("Unknown object in staging; preserved"));
        }
        let flags = match open_at(fd, &name, libc::O_RDONLY | libc::O_DIRECTORY) {
            Ok(child) => {
                if !same(&metadata, &child.metadata()?) {
                    return Err(io::Error::other("Staging directory replaced; preserved"));
                }
                clean(&child, owned, depth + 1)?;
                libc::AT_REMOVEDIR
            }
            Err(error) if matches!(error.raw_os_error(), Some(libc::ENOTDIR | libc::ELOOP)) => 0,
            Err(error) => return Err(error),
        };
        if !same(&metadata, &fs::symlink_metadata(alias(fd).join(&name))?) {
            return Err(io::Error::other("Staging entry replaced; preserved"));
        }
        let name = cstring(&name)?;
        // SAFETY: fd жив, имя принадлежит CString; flags согласован с типом объекта.
        if unsafe { libc::unlinkat(fd.as_raw_fd(), name.as_ptr(), flags) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}
// Приватная подготовка внутри папки назначения: незавершённая копия не видна
// под конечным именем. owned содержит только созданные этой операцией объекты.
// Drop пытается убрать их при ошибке/отмене, сохраняя неизвестные подмены.
struct Stage {
    parent: File,
    name: OsString,
    fd: File,
    owned: HashSet<(u64, u64)>,
}
impl Stage {
    async fn new(parent: File) -> io::Result<Self> {
        let name = OsString::from(format!(".{}-copy", token()));
        mkdir_private(&parent, &name)?;
        let fd = open_at(&parent, &name, libc::O_RDONLY | libc::O_DIRECTORY)?;
        fd.set_permissions(fs::Permissions::from_mode(0o700))?;
        Ok(Self {
            parent,
            name,
            fd,
            owned: HashSet::new(),
        })
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        if let Ok(current) = fs::symlink_metadata(alias(&self.parent).join(&self.name))
            && self.fd.metadata().is_ok_and(|owned| same(&owned, &current))
        {
            let _ = clean(&self.fd, &self.owned, 0);
            if let Ok(name) = cstring(&self.name) {
                // SAFETY: родитель и CString живы; AT_REMOVEDIR удаляет только пустую папку.
                unsafe {
                    libc::unlinkat(self.parent.as_raw_fd(), name.as_ptr(), libc::AT_REMOVEDIR);
                }
            }
        }
    }
}

pub(crate) async fn create_directory(path: &Path, name: &OsStr) -> io::Result<()> {
    let fd = directory(path)?;
    Local::regular(&alias(&fd).join(name)).create_dir().await
}

fn copy_node<'a>(
    source: &'a File,
    name: &'a OsStr,
    dest: &'a File,
    new: &'a OsStr,
    control: &'a OperationControl,
    owned: &'a mut HashSet<(u64, u64)>,
    depth: usize,
) -> std::pin::Pin<Box<dyn Future<Output = io::Result<()>> + 'a>> {
    Box::pin(async move {
        control.check()?;
        if depth > MAX_DEPTH || control.0.items.fetch_add(1, Ordering::Relaxed) >= MAX_NODES {
            return Err(invalid(
                "Copy exceeds the 100000 objects / 128 levels limit",
            ));
        }
        let src_path = alias(source).join(name);
        let dst_path = alias(dest).join(new);
        let before = fs::symlink_metadata(&src_path)?;
        if before.file_type().is_symlink() {
            let link = Local::regular(&src_path).read_link().await?.to_os_owned()?;
            if !unchanged(&before, &fs::symlink_metadata(&src_path)?) {
                return Err(io::Error::other("Source changed during copy"));
            }
            Local::regular(&dst_path)
                .symlink_file(link.as_os_str())
                .await?;
            let created = fs::symlink_metadata(&dst_path)?;
            if !created.file_type().is_symlink() {
                return Err(io::Error::other("Staging link replaced"));
            }
            owned.insert((created.dev(), created.ino()));
        } else if before.is_dir() {
            let src = open_at(source, name, libc::O_RDONLY | libc::O_DIRECTORY)?;
            if !same(&before, &src.metadata()?) {
                return Err(io::Error::other("Source changed during copy"));
            }
            Local::regular(&dst_path).create_dir().await?;
            let dst = open_at(dest, new, libc::O_RDONLY | libc::O_DIRECTORY)?;
            let created = dst.metadata()?;
            owned.insert((created.dev(), created.ino()));
            // Временные папки остаются доступными для очистки после ошибки.
            dst.set_permissions(fs::Permissions::from_mode(0o700))?;
            let mut reader = Local::regular(&alias(&src)).read_dir().await?;
            while let Some(entry) = reader.next().await? {
                let path = entry.path().to_os_owned()?;
                let name = path
                    .file_name()
                    .ok_or_else(|| invalid("Missing entry name"))?;
                copy_node(&src, name, &dst, name, control, owned, depth + 1).await?;
            }
            if !unchanged(&before, &src.metadata()?) {
                return Err(io::Error::other("Directory changed during copy"));
            }
            dst.sync_all()?;
        } else if before.is_file() {
            // Сначала закрепляем объект без открытия для чтения и сверяем его тип:
            // подменённое устройство/FIFO не должно открыться вместо обычного файла.
            let pinned = open_at(source, name, libc::O_PATH)?;
            if !same(&before, &pinned.metadata()?) || !pinned.metadata()?.is_file() {
                return Err(io::Error::other("Source changed during copy"));
            }
            let mut reader = Local::regular(&alias(&pinned))
                .demand()
                .read(true)
                .open(&alias(&pinned))
                .await?;
            let mut writer = Local::regular(&dst_path)
                .demand()
                .write(true)
                .create_new(true)
                .open(&dst_path)
                .await?;
            let created = writer.metadata().await?;
            owned.insert((created.dev(), created.ino()));
            let mut buffer = vec![0; 256 * 1024];
            let mut bytes = 0;
            loop {
                control.check()?;
                let count = reader.read(&mut buffer).await?;
                if count == 0 {
                    break;
                }
                if bytes + count as u64 > before.len() {
                    return Err(io::Error::other("Source grew during copy"));
                }
                writer.write_all(&buffer[..count]).await?;
                // Tokio fs::File может вернуть управление до окончания фоновой записи.
                // Завершаем запись порции перед проверкой отмены и очисткой файла.
                writer.flush().await?;
                bytes += count as u64;
                control.0.bytes.fetch_add(count as u64, Ordering::Relaxed);
            }
            if bytes != before.len() || !unchanged(&before, &pinned.metadata()?) {
                return Err(io::Error::other("File changed during copy"));
            }
            // Сохраняем обычные права, но не переносим setuid/setgid/sticky.
            writer
                .set_permissions(fs::Permissions::from_mode(before.mode() & 0o777))
                .await?;
            writer.sync_all().await?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Sockets, devices and FIFOs cannot be copied",
            ));
        }
        Ok(())
    })
}

// Время удаления записывается в локальном формате внешнего стандарта корзины.
fn date() -> io::Result<String> {
    // SAFETY: NULL допустим для time — запись во внешний указатель не требуется.
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: now жив, local предоставляет выровненное место для tm;
    // содержимое используется только после успешной инициализации.
    if unsafe { libc::localtime_r(&now, local.as_mut_ptr()) }.is_null() {
        return Err(io::Error::other("Cannot determine Trash timestamp"));
    }
    let mut buf = [0u8; 32];
    // SAFETY: буфер имеет указанную длину, формат завершён NUL, tm инициализирован.
    let count = unsafe {
        libc::strftime(
            buf.as_mut_ptr().cast(),
            buf.len(),
            c"%Y-%m-%dT%H:%M:%S".as_ptr(),
            local.as_ptr(),
        )
    };
    if count == 0 {
        return Err(io::Error::other("Cannot format Trash timestamp"));
    }
    Ok(String::from_utf8_lossy(&buf[..count]).into_owned())
}
// Кодируем исходные байты пути, а не отображаемую UTF-8 строку:
// так метаданные корзины не теряют имена с произвольными байтами.
fn uri(path: &Path) -> String {
    let mut value = String::new();
    for &byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
            value.push(char::from(byte));
        } else {
            use std::fmt::Write;
            let _ = write!(value, "%{byte:02X}");
        }
    }
    value
}
async fn trash_dir(home: &File, parts: &[&str]) -> io::Result<File> {
    let mut fd = home.try_clone()?;
    for name in parts {
        let created = match mkdir_private(&fd, OsStr::new(name)) {
            Ok(()) => true,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
            Err(error) => return Err(error),
        };
        fd = open_at(&fd, OsStr::new(name), libc::O_RDONLY | libc::O_DIRECTORY)?;
        let metadata = fd.metadata()?;
        if created {
            fd.set_permissions(fs::Permissions::from_mode(0o700))?;
        }
        // SAFETY: geteuid не принимает указателей и не меняет владение ресурсами.
        if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o022 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Trash path must be owned by the user and not writable by others",
            ));
        }
    }
    Ok(fd)
}

impl YaziBackend {
    /// Выполняет выбранную операцию; GUI передаёт HOME своей активной сессии.
    /// Delete переносит объект в корзину, не удаляя безвозвратно.
    /// Перенос между разными монтированиями отказывает без обхода через copy/delete.
    /// Возвращает путь результата; ошибки до окончательного переноса сохраняют источник.
    pub async fn operate(
        &self,
        operation: FileOperation,
        source: &Path,
        target: Option<&Path>,
        home: &Path,
        control: &OperationControl,
    ) -> io::Result<PathBuf> {
        control.check()?;
        let name = name_of(source)?;
        let src = parent(source)?;
        if operation == FileOperation::CreateDirectory {
            if target.is_some() {
                return Err(invalid("Folder creation has no destination"));
            }
            self.create_directory(source.parent().unwrap(), name)
                .await?;
            return Ok(source.to_owned());
        }
        if operation == FileOperation::Delete {
            require_absolute(home)?;
            if target.is_some()
                || source == home
                || !source.starts_with(home)
                || source.starts_with(home.join(".local/share/Trash"))
            {
                return Err(invalid(
                    "Trash is restricted to objects inside HOME, outside Trash",
                ));
            }
            let h = directory(home)?;
            let files = trash_dir(&h, &[".local", "share", "Trash", "files"]).await?;
            let info = trash_dir(&h, &[".local", "share", "Trash", "info"]).await?;
            let key = token();
            let info_name = format!("{key}.trashinfo");
            let info_path = alias(&info).join(&info_name);
            let mut writer = Local::regular(&info_path)
                .demand()
                .write(true)
                .create_new(true)
                .open(&info_path)
                .await?;
            let owned_info = writer.metadata().await?;
            // Запись исходного пути должна попасть на диск раньше переноса объекта.
            // Проверяем каталоги и принадлежность записи повторно перед завершением.
            let preparation = async {
                writer
                    .set_permissions(fs::Permissions::from_mode(0o600))
                    .await?;
                writer
                    .write_all(
                        format!(
                            "[Trash Info]\nPath={}\nDeletionDate={}\n",
                            uri(source),
                            date()?
                        )
                        .as_bytes(),
                    )
                    .await?;
                writer.sync_all().await?;
                info.sync_all()?;
                control.check()?;
                validate_directory(home, &h)?;
                validate_directory(&home.join(".local/share/Trash/files"), &files)?;
                validate_directory(&home.join(".local/share/Trash/info"), &info)?;
                let current_info = fs::symlink_metadata(&info_path)?;
                if !current_info.is_file() || !same(&owned_info, &current_info) {
                    return Err(io::Error::other(
                        "Trash recovery metadata changed; source preserved",
                    ));
                }
                validate_parent(source, &src)?;
                rename_no_replace(&src, name, &files, OsStr::new(&key))
            }
            .await;
            if let Err(error) = preparation {
                if fs::symlink_metadata(&info_path)
                    .is_ok_and(|current| current.is_file() && same(&owned_info, &current))
                {
                    let _ = Local::regular(&info_path).remove_file().await;
                }
                return Err(error);
            }
            // Успешное переименование уже завершило перенос. Ошибка синхронизации
            // не позволяет безопасный откат: сохраняем объект и метаданные,
            // возвращаем его расположение в корзине.
            let _ = sync_directory(&files);
            let _ = sync_directory(&src);
            return Ok(home.join(".local/share/Trash/files").join(key));
        }
        let target = target.ok_or_else(|| invalid("Choose a destination"))?;
        let new = name_of(target)?;
        let dst = parent(target)?;
        if fs::symlink_metadata(alias(&dst).join(new)).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Destination exists; nothing was replaced",
            ));
        }
        match operation {
            FileOperation::Copy => {
                if target.starts_with(source) {
                    return Err(invalid("Cannot copy an object into itself"));
                }
                reject_descendant(&src, name, &dst)?;
                let mut stage = Stage::new(dst).await?;
                copy_node(
                    &src,
                    name,
                    &stage.fd,
                    OsStr::new("payload"),
                    control,
                    &mut stage.owned,
                    0,
                )
                .await?;
                control.check()?;
                validate_parent(target, &stage.parent)?;
                // Только полностью подготовленный результат получает конечное имя.
                // RENAME_NOREPLACE защищает и от конфликта, возникшего во время копирования.
                rename_no_replace(&stage.fd, OsStr::new("payload"), &stage.parent, new)?;
                let _ = sync_directory(&stage.parent);
            }
            FileOperation::Move | FileOperation::Rename => {
                control.check()?;
                validate_parent(source, &src)?;
                validate_parent(target, &dst)?;
                rename_no_replace(&src, name, &dst, new)?;
                let _ = sync_directory(&dst);
                let _ = sync_directory(&src);
            }
            _ => return Err(invalid("Invalid operation")),
        }
        Ok(target.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(future: impl Future<Output = ()>) {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future);
    }
    #[test]
    fn directory_descriptor_retains_o_path_in_native_sdk() {
        let fd = directory(Path::new("/")).unwrap();
        // SAFETY: fd жив; F_GETFL не требует третьего аргумента.
        let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
        assert!(flags >= 0);
        assert_ne!(flags & libc::O_PATH, 0);
    }
    #[test]
    fn cleanup_preserves_injected_real_directory_and_unknown_file() {
        let root = std::env::temp_dir().join(token());
        fs::create_dir(&root).unwrap();
        run(async {
            let stage = Stage::new(directory(&root).unwrap()).await.unwrap();
            let private_path = alias(&stage.fd).join("external");
            fs::create_dir(root.join("external")).unwrap();
            fs::write(root.join("external/sentinel"), b"keep").unwrap();
            fs::rename(root.join("external"), &private_path).unwrap();
            fs::write(alias(&stage.fd).join("unknown"), b"keep").unwrap();
            let saved = root.join(&stage.name);
            drop(stage);
            assert_eq!(fs::read(saved.join("external/sentinel")).unwrap(), b"keep");
            assert_eq!(fs::read(saved.join("unknown")).unwrap(), b"keep");
        });
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cleanup_preserves_outer_name_replacement() {
        let root = std::env::temp_dir().join(token());
        fs::create_dir(&root).unwrap();
        run(async {
            let stage = Stage::new(directory(&root).unwrap()).await.unwrap();
            let name = root.join(&stage.name);
            fs::rename(&name, root.join("old")).unwrap();
            fs::create_dir(&name).unwrap();
            fs::write(name.join("sentinel"), b"keep").unwrap();
            drop(stage);
            assert_eq!(fs::read(name.join("sentinel")).unwrap(), b"keep");
            assert!(root.join("old").is_dir());
        });
        fs::remove_dir_all(root).unwrap();
    }
}
