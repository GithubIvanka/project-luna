//! Передача одного выбранного файла и ограниченный клиент открытия приложений.
//! Доказательство закрепляет объект, но не выдаёт разрешений: их проверяет policy.
use std::ffi::{CString, OsString};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use luna_common::ResourceAccess;
use luna_root_mapping::MappingKind;
use luna_user_session::{SessionId, UserCredentials};
use serde::{Deserialize, Serialize};

use crate::AuthorizedApplicationPlan;
use crate::protocol::{APPLICATION_SOCKET, CatalogRequest, DesktopApplication, MAX_CATALOG_RESPONSE, MAX_DESKTOP_APPLICATIONS, MAX_REQUEST, MAX_RESPONSE, RunResponse, valid_content_identity};

pub const OPEN_CAPABILITY: &str = "application.open";
pub const SELECTED_CAPABILITY: &str = "user.selected-file";
pub const SELECTED_LOGICAL: &str = "/open/selected";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileHandler {
    pub application: DesktopApplication,
    pub mime_types: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileHandlersResponse {
    pub format: u32,
    pub handlers: Vec<FileHandler>,
    pub error: Option<String>,
}
impl FileHandlersResponse {
    pub fn to_toml(&self) -> io::Result<String> {
        let mut unique = std::collections::BTreeSet::new();
        if self.format != 1 || self.handlers.len() > MAX_DESKTOP_APPLICATIONS {
            return Err(io::Error::other("invalid file-handler catalog"));
        }
        for handler in &self.handlers {
            handler.application.validate()?;
            luna_bundle::lbp::FileOpenEntry { mime_types: handler.mime_types.clone(), args: vec![] }
                .validate().map_err(io::Error::other)?;
            if !unique.insert((&handler.application.application, &handler.application.version)) {
                return Err(io::Error::other("duplicate file handler"));
            }
        }
        let text = toml::to_string(self).map_err(io::Error::other)?;
        if text.len() > MAX_CATALOG_RESPONSE { return Err(io::Error::other("handler catalog too large")); }
        Ok(text)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenFileRequest {
    pub format: u32,
    pub operation: String,
    pub application: String,
    pub version: String,
    pub content_identity: String,
    /// Пользователь выбирает заявленный тип обработчика; это не доказательство формата файла.
    pub mime_type: String,
    /// Байты логического пути /home/... в hex, без потери имён не в UTF-8.
    pub path_hex: String,
}
impl OpenFileRequest {
    pub fn validate(&self) -> io::Result<()> {
        DesktopApplication { application: self.application.clone(), version: self.version.clone(),
            content_identity: self.content_identity.clone(), name: "handler".into(), terminal: false }.validate()?;
        if self.format != 1 || self.operation != "open-file"
            || !valid_content_identity(&self.content_identity)
            || !luna_bundle::lbp::valid_mime_type(&self.mime_type) {
            return Err(io::Error::other("invalid file-open request"));
        }
        self.path().map(|_| ())
    }
    pub fn path(&self) -> io::Result<PathBuf> {
        if self.path_hex.len() > 8192 || !self.path_hex.len().is_multiple_of(2)
            || self.path_hex.bytes().any(|b| !b.is_ascii_digit() && !(b'a'..=b'f').contains(&b)) {
            return Err(io::Error::other("invalid encoded file path"));
        }
        let bytes = self.path_hex.as_bytes().chunks_exact(2).map(|pair| {
            let digit = |byte| if byte <= b'9' { byte - b'0' } else { byte - b'a' + 10 };
            (digit(pair[0]) << 4) | digit(pair[1])
        }).collect::<Vec<u8>>();
        let path = PathBuf::from(OsString::from_vec(bytes));
        validate_selected_path(&path)?;
        Ok(path)
    }
}

fn validate_selected_path(path: &Path) -> io::Result<()> {
    let bytes = path.as_os_str().as_bytes();
    if bytes.len() > 4096 || !bytes.starts_with(b"/home/") || bytes.contains(&0)
        || bytes[6..].split(|b| *b == b'/').any(|part| part.is_empty() || part == b"." || part == b"..") {
        return Err(io::Error::other("selected file must be below session HOME"));
    }
    Ok(())
}

pub fn encode_file_path(path: &Path) -> String {
    use std::fmt::Write as _;
    let mut text = String::new();
    for byte in path.as_os_str().as_bytes() { let _ = write!(text, "{byte:02x}"); }
    text
}

fn exchange<T: Serialize, R: serde::de::DeserializeOwned>(request: &T, limit: usize) -> io::Result<R> {
    let text = toml::to_string(request).map_err(io::Error::other)?;
    if text.len() > MAX_REQUEST { return Err(io::Error::other("file-open request too large")); }
    let mut socket = UnixStream::connect(APPLICATION_SOCKET)?;
    socket.set_write_timeout(Some(std::time::Duration::from_secs(5)))?;
    socket.set_read_timeout(Some(std::time::Duration::from_secs(60)))?;
    socket.write_all(text.as_bytes())?;
    socket.shutdown(Shutdown::Write)?;
    let mut response = String::new();
    socket.take((limit + 1) as u64).read_to_string(&mut response)?;
    if response.len() > limit { return Err(io::Error::other("file-open response too large")); }
    toml::from_str(&response).or_else(|error| {
        if let Ok(result) = toml::from_str::<RunResponse>(&response)
            && let Some(message) = result.error { return Err(io::Error::other(message)); }
        Err(io::Error::other(error))
    })
}

pub fn file_handlers() -> io::Result<FileHandlersResponse> {
    let response: FileHandlersResponse = exchange(&CatalogRequest { format: 1, operation: "file-handlers".into() }, MAX_CATALOG_RESPONSE)?;
    response.to_toml()?;
    if let Some(error) = &response.error { return Err(io::Error::other(error.clone())); }
    Ok(response)
}

/// Ответ подтверждает запуск, а не завершение обработчика; им продолжает владеть runtime.
pub fn open_file(request: &OpenFileRequest) -> io::Result<RunResponse> {
    request.validate()?;
    let response: RunResponse = exchange(request, MAX_RESPONSE)?;
    if response.format != 1 { return Err(io::Error::other("unsupported file-open response")); }
    if let Some(error) = &response.error { return Err(io::Error::other(error.clone())); }
    if !response.instance.as_ref().is_some_and(|value| value.parse::<u64>().is_ok_and(|id| id > 0)) {
        return Err(io::Error::other("missing launched handler instance"));
    }
    Ok(response)
}

/// O_PATH закрепляет inode без чтения файла от root и живёт до подготовки дочернего процесса.
#[derive(Clone, Debug)]
pub struct SelectedFile {
    source: PathBuf,
    session: SessionId,
    uid: u32,
    pinned: Arc<File>,
}
impl SelectedFile {
    pub fn inspect(home: &Path, logical: &Path, session: SessionId, uid: u32) -> io::Result<Self> {
        if uid == 0 || !home.is_absolute() { return Err(io::Error::other("invalid selected-file user")); }
        validate_selected_path(logical)?;
        let relative = logical.strip_prefix("/home").map_err(io::Error::other)?;
        let mut directory = path_fd(Path::new("/"), true)?;
        for part in home.components() {
            match part {
                Component::RootDir => {},
                Component::Normal(name) => directory = open_at(&directory, name, true)?,
                _ => return Err(io::Error::other("unsafe HOME path")),
            }
        }
        if directory.metadata()?.uid() != uid { return Err(io::Error::other("foreign HOME owner")); }
        let parts = relative.components().collect::<Vec<_>>();
        if parts.len() > 256 { return Err(io::Error::other("selected file path too deep")); }
        let mut file = directory;
        for (index, part) in parts.iter().enumerate() {
            let Component::Normal(name) = part else { return Err(io::Error::other("unsafe selected path")); };
            file = open_at(&file, name, index + 1 != parts.len())?;
        }
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.uid() != uid { return Err(io::Error::other("selected object must be a user-owned regular file")); }
        Ok(Self { source: home.join(relative), session, uid, pinned: Arc::new(file) })
    }
    pub fn source(&self) -> &Path { &self.source }
    pub fn validate_plan(&self, plan: &AuthorizedApplicationPlan, user: &UserCredentials) -> io::Result<()> {
        validate_proof(plan, user, self.session, self.uid, SELECTED_CAPABILITY, SELECTED_LOGICAL, &self.source)?;
        let metadata = self.pinned.metadata()?;
        if !metadata.is_file() || metadata.uid() != self.uid { return Err(io::Error::other("selected file changed type/owner")); }
        Ok(())
    }
    pub fn verify_mounted(&self, root: &Path) -> io::Result<()> {
        let mounted = fs::symlink_metadata(root.join(SELECTED_LOGICAL.trim_start_matches('/')))?;
        let pinned = self.pinned.metadata()?;
        if !mounted.is_file() || mounted.uid() != self.uid || mounted.dev() != pinned.dev() || mounted.ino() != pinned.ino() {
            return Err(io::Error::other("selected file replaced during materialization"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ApplicationEndpoint {
    session: SessionId,
    uid: u32,
    pinned: Arc<File>,
}
impl ApplicationEndpoint {
    pub fn inspect(session: SessionId, uid: u32) -> io::Result<Self> {
        let parent = fs::symlink_metadata("/run/luna")?;
        let pinned = path_fd(Path::new(APPLICATION_SOCKET), false)?;
        let metadata = pinned.metadata()?;
        if uid == 0 || !parent.is_dir() || parent.uid() != 0 || parent.mode() & 0o022 != 0
            || !metadata.file_type().is_socket() || metadata.uid() != 0 {
            return Err(io::Error::other("unsafe application endpoint"));
        }
        Ok(Self { session, uid, pinned: Arc::new(pinned) })
    }
    pub fn validate_plan(&self, plan: &AuthorizedApplicationPlan, user: &UserCredentials) -> io::Result<()> {
        validate_proof(plan, user, self.session, self.uid, OPEN_CAPABILITY, APPLICATION_SOCKET, Path::new(APPLICATION_SOCKET))
    }
    pub fn verify_mounted(&self, root: &Path) -> io::Result<()> {
        let metadata = fs::symlink_metadata(root.join(APPLICATION_SOCKET.trim_start_matches('/')))?;
        let pinned = self.pinned.metadata()?;
        if !metadata.file_type().is_socket() || metadata.uid() != 0 || metadata.dev() != pinned.dev() || metadata.ino() != pinned.ino() {
            return Err(io::Error::other("application endpoint replaced during materialization"));
        }
        Ok(())
    }
}

fn validate_proof(plan: &AuthorizedApplicationPlan, user: &UserCredentials, session: SessionId, uid: u32,
    capability: &str, logical: &str, source: &Path) -> io::Result<()> {
    let value = plan.value();
    if value.session() != session || user.uid != uid || uid == 0
        || !value.manifest().capabilities().any(|cap| cap == capability)
        || !value.mapping().iter().any(|rule| rule.logical().as_path() == Path::new(logical)
            && rule.kind() == MappingKind::File && rule.physical().is_some_and(|path| path.as_path() == source)
            && rule.access().len() == 1 && rule.access().contains(&ResourceAccess::Read)) {
        return Err(io::Error::other("file handoff proof does not match authorized plan"));
    }
    Ok(())
}

fn path_fd(path: &Path, directory: bool) -> io::Result<File> {
    let path = CString::new(path.as_os_str().as_bytes()).map_err(io::Error::other)?;
    // SAFETY: CString жив до конца вызова; O_PATH не требует аргумента mode.
    let fd = unsafe { libc::openat(libc::AT_FDCWD, path.as_ptr(), libc::O_PATH | libc::O_NOFOLLOW | libc::O_CLOEXEC | if directory { libc::O_DIRECTORY } else { 0 }) };
    own_fd(fd)
}
fn open_at(parent: &File, name: &std::ffi::OsStr, directory: bool) -> io::Result<File> {
    let name = CString::new(name.as_bytes()).map_err(io::Error::other)?;
    // SAFETY: родитель и строка живы; в SDK сохраняем O_PATH через прямой openat.
    let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), libc::O_PATH | libc::O_NOFOLLOW | libc::O_CLOEXEC | if directory { libc::O_DIRECTORY } else { 0 }) };
    own_fd(fd)
}
fn own_fd(fd: i32) -> io::Result<File> {
    if fd < 0 { Err(io::Error::last_os_error()) }
    // SAFETY: успешный openat создал новый дескриптор; File получает единственное владение.
    else { Ok(unsafe { File::from_raw_fd(fd) }) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!("luna-handoff-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir(&path).unwrap(); Self(path)
        }
    }
    impl Drop for Directory { fn drop(&mut self) { fs::remove_dir_all(&self.0).unwrap(); } }
    fn request(path: &Path) -> OpenFileRequest {
        OpenFileRequest { format: 1, operation: "open-file".into(), application: "org.luna.terminal".into(),
            version: "1.3.1".into(), content_identity: "a".repeat(64), mime_type: "text/plain".into(), path_hex: encode_file_path(path) }
    }
    fn uid() -> u32 {
        // SAFETY: geteuid не принимает указателей и не меняет ресурсы процесса.
        unsafe { libc::geteuid() }
    }
    #[test]
    fn encoded_paths_preserve_bytes_and_reject_navigation() {
        let path = PathBuf::from(OsString::from_vec(b"/home/file-\xff".to_vec()));
        assert_eq!(request(&path).path().unwrap(), path);
        for value in ["/home", "/home/", "/home//a", "/home/../a", "/home/./a", "/home/a/..", "/homeish/a", "/etc/passwd", "/home/a\0b"] {
            assert!(request(Path::new(value)).validate().is_err(), "{value:?}");
        }
        let mut bad = request(Path::new("/home/a")); bad.path_hex.push('f');
        assert!(bad.validate().is_err());
        bad.path_hex = "FF".into(); assert!(bad.validate().is_err());
    }
    #[test]
    fn protocol_rejects_unpinned_content_unknown_fields_and_caller_arguments() {
        let mut value = request(Path::new("/home/a")); value.content_identity = "A".repeat(64);
        assert!(value.validate().is_err());
        value.content_identity = "a".repeat(64); value.mime_type = "text/plain; charset=utf-8".into();
        assert!(value.validate().is_err());
        let text = toml::to_string(&request(Path::new("/home/a"))).unwrap();
        assert!(toml::from_str::<OpenFileRequest>(&format!("{text}\nargs=['--escape']")).is_err());
    }
    #[test]
    fn selected_file_pins_regular_inode_and_rejects_links_and_directories() {
        let root = Directory::new(); fs::write(root.0.join("file"), b"selected").unwrap();
        fs::create_dir(root.0.join("folder")).unwrap();
        symlink("file", root.0.join("link")).unwrap();
        symlink("folder", root.0.join("parent-link")).unwrap();
        fs::write(root.0.join("folder/file"), b"inside").unwrap();
        for path in ["/home/link", "/home/folder", "/home/parent-link/file"] {
            assert!(SelectedFile::inspect(&root.0, Path::new(path), SessionId::new(1), uid()).is_err());
        }
        let proof = SelectedFile::inspect(&root.0, Path::new("/home/file"), SessionId::new(1), uid()).unwrap();
        // SAFETY: fd жив; F_GETFL не требует третьего аргумента.
        let flags = unsafe { libc::fcntl(proof.pinned.as_raw_fd(), libc::F_GETFL) };
        assert_ne!(flags & libc::O_PATH, 0);
        assert!(SelectedFile::inspect(&root.0, Path::new("/home/file"), SessionId::new(1), uid() + 1).is_err());
    }
    #[test]
    fn proof_detects_replacement_after_materialization() {
        let home = Directory::new(); let mounted = Directory::new();
        fs::write(home.0.join("file"), b"chosen").unwrap();
        let proof = SelectedFile::inspect(&home.0, Path::new("/home/file"), SessionId::new(1), uid()).unwrap();
        fs::create_dir(mounted.0.join("open")).unwrap();
        fs::hard_link(home.0.join("file"), mounted.0.join("open/selected")).unwrap();
        proof.verify_mounted(&mounted.0).unwrap();
        fs::remove_file(mounted.0.join("open/selected")).unwrap();
        fs::write(mounted.0.join("open/selected"), b"replacement").unwrap();
        assert!(proof.verify_mounted(&mounted.0).is_err());
        fs::remove_file(mounted.0.join("open/selected")).unwrap();
        symlink(home.0.join("file"), mounted.0.join("open/selected")).unwrap();
        assert!(proof.verify_mounted(&mounted.0).is_err());
    }
    #[test]
    fn catalog_requires_unique_valid_declared_handlers() {
        let handler = FileHandler { application: DesktopApplication { application: "org.luna.terminal".into(), version: "1.3.1".into(), content_identity: "a".repeat(64), name: "Terminal".into(), terminal: true }, mime_types: vec!["text/plain".into()] };
        let mut response = FileHandlersResponse { format: 1, handlers: vec![handler.clone()], error: None };
        assert!(response.to_toml().is_ok()); response.handlers.push(handler);
        assert!(response.to_toml().is_err()); response.handlers.pop();
        response.handlers[0].mime_types.push("text/plain".into()); assert!(response.to_toml().is_err());
    }
}
