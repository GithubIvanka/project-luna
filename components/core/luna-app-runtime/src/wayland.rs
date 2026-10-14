//! One system-selected Wayland endpoint, bound to an authorized UserSession.
use std::fs;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use luna_common::ResourceAccess;
use luna_root_mapping::MappingKind;
use luna_security::{CapabilityName, CapabilityRegistry};
use luna_user_session::{SessionId, UserCredentials};

use crate::AuthorizedApplicationPlan;

pub const WAYLAND_CAPABILITY: &str = "desktop.wayland";
pub const WAYLAND_LOGICAL: &str = "/run/wayland-0";

#[derive(Clone, Debug)]
pub struct WaylandEndpoint {
    source: PathBuf,
    session: SessionId,
    uid: u32,
    dev: u64,
    ino: u64,
    peer_pid: i32,
}

impl WaylandEndpoint {
    /// Caller checks this peer PID belongs to the active desktop cgroup before
    /// using the proof. The child rechecks the mounted inode and peer identity.
    pub fn inspect(source: PathBuf, session: SessionId, uid: u32) -> io::Result<Self> {
        if uid == 0
            || !source.is_absolute()
            || source.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
        {
            return Err(io::Error::other("invalid Wayland endpoint context"));
        }
        let metadata = socket_metadata(&source, uid)?;
        let peer = connect_peer(&source)?;
        if peer.uid != uid || peer.pid <= 0 {
            return Err(io::Error::other("Wayland peer is not the session user"));
        }
        Ok(Self {
            source,
            session,
            uid,
            dev: metadata.dev(),
            ino: metadata.ino(),
            peer_pid: peer.pid,
        })
    }

    pub fn source(&self) -> &Path {
        &self.source
    }
    pub fn peer_pid(&self) -> i32 {
        self.peer_pid
    }
    pub fn session(&self) -> SessionId {
        self.session
    }

    pub fn validate_plan(
        &self,
        plan: &AuthorizedApplicationPlan,
        credentials: &UserCredentials,
    ) -> io::Result<()> {
        let value = plan.value();
        let name = CapabilityName::new(WAYLAND_CAPABILITY).map_err(io::Error::other)?;
        let provider = CapabilityRegistry::with_default_providers();
        let mapping = value
            .mapping()
            .iter()
            .find(|rule| rule.logical().as_path() == Path::new(WAYLAND_LOGICAL));
        if value.session() != self.session
            || credentials.uid != self.uid
            || provider.provider_for(&name) != Some("desktop")
            || !value
                .manifest()
                .capabilities()
                .any(|cap| cap == WAYLAND_CAPABILITY)
            || !mapping.is_some_and(|rule| {
                rule.kind() == MappingKind::File
                    && rule
                        .physical()
                        .is_some_and(|physical| physical.as_path() == self.source)
                    && rule.access().len() == 1
                    && rule.access().contains(&ResourceAccess::Read)
            })
        {
            return Err(io::Error::other(
                "Wayland proof does not match authorized session/resource",
            ));
        }
        Ok(())
    }

    /// The secure bind pins the socket. Replacing its source path before bind
    /// must fail here; a replacement after bind cannot redirect this instance.
    pub fn verify_mounted(&self, root: &Path) -> io::Result<()> {
        let mounted = root.join(WAYLAND_LOGICAL.trim_start_matches('/'));
        let metadata = socket_metadata(&mounted, self.uid)?;
        let peer = connect_peer(&mounted)?;
        if metadata.dev() != self.dev
            || metadata.ino() != self.ino
            || peer.pid != self.peer_pid
            || peer.uid != self.uid
        {
            return Err(io::Error::other(
                "Wayland endpoint changed during materialization",
            ));
        }
        Ok(())
    }
}

fn socket_metadata(path: &Path, uid: u32) -> io::Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_socket() || metadata.uid() != uid {
        return Err(io::Error::other("unsafe Wayland socket type/owner"));
    }
    Ok(metadata)
}

fn connect_peer(path: &Path) -> io::Result<libc::ucred> {
    let bytes = path.as_os_str().as_bytes();
    // Fully initialized address; no truncation, embedded NUL or borrowed FD.
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    if bytes.contains(&0) || bytes.len() >= address.sun_path.len() {
        return Err(io::Error::other(
            "Wayland socket path exceeds AF_UNIX limit",
        ));
    }
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (target, source) in address.sun_path.iter_mut().zip(bytes) {
        *target = *source as libc::c_char;
    }
    let raw = unsafe {
        libc::socket(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            0,
        )
    };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    // socket returned a new, uniquely owned descriptor.
    let socket = unsafe { OwnedFd::from_raw_fd(raw) };
    let result = unsafe {
        libc::connect(
            socket.as_raw_fd(),
            (&address as *const libc::sockaddr_un).cast(),
            std::mem::size_of_val(&address) as libc::socklen_t,
        )
    };
    if result != 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::EINPROGRESS) {
            return Err(error); // EAGAIN is a full backlog, not a pending connection.
        }
        let deadline = Instant::now() + Duration::from_millis(200);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Wayland connect timed out",
                ));
            }
            let mut event = libc::pollfd {
                fd: socket.as_raw_fd(),
                events: libc::POLLOUT,
                revents: 0,
            };
            let result = unsafe { libc::poll(&mut event, 1, remaining.as_millis().max(1) as i32) };
            if result > 0 {
                break;
            }
            if result < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                return Err(io::Error::last_os_error());
            }
        }
        let mut error: libc::c_int = 0;
        let mut size = std::mem::size_of_val(&error) as libc::socklen_t;
        if unsafe {
            libc::getsockopt(
                socket.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_ERROR,
                (&mut error as *mut libc::c_int).cast(),
                &mut size,
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        if error != 0 {
            return Err(io::Error::from_raw_os_error(error));
        }
    }
    let mut peer: libc::ucred = unsafe { std::mem::zeroed() };
    let mut size = std::mem::size_of_val(&peer) as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            socket.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut peer as *mut libc::ucred).cast(),
            &mut size,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    if size != std::mem::size_of_val(&peer) as libc::socklen_t || peer.pid <= 0 {
        return Err(io::Error::other("invalid Wayland peer credentials"));
    }
    Ok(peer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn endpoint_rejects_file_symlink_foreign_uid_and_changed_inode() {
        let uid = unsafe { libc::geteuid() };
        assert_ne!(uid, 0, "this unprivileged test requires a non-root runner");
        let root = std::env::temp_dir().join(format!(
            "luna-wl-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("run")).unwrap();
        let source = root.join("source");
        fs::write(&source, "not a socket").unwrap();
        assert!(WaylandEndpoint::inspect(source.clone(), SessionId::new(9), uid).is_err());
        fs::remove_file(&source).unwrap();
        let listener = UnixListener::bind(&source).unwrap();
        assert!(WaylandEndpoint::inspect(source.clone(), SessionId::new(9), uid + 1).is_err());
        std::os::unix::fs::symlink(&source, root.join("link")).unwrap();
        assert!(WaylandEndpoint::inspect(root.join("link"), SessionId::new(9), uid).is_err());
        let proof = WaylandEndpoint::inspect(source.clone(), SessionId::new(9), uid).unwrap();
        assert_eq!(proof.peer_pid(), std::process::id() as i32);
        let mounted = root.join("run/wayland-0");
        fs::hard_link(&source, &mounted).unwrap();
        proof.verify_mounted(&root).unwrap();
        fs::remove_file(&mounted).unwrap();
        let replacement = UnixListener::bind(&mounted).unwrap();
        assert!(proof.verify_mounted(&root).is_err());
        drop((listener, replacement));
        fs::remove_dir_all(root).unwrap();
    }
}
