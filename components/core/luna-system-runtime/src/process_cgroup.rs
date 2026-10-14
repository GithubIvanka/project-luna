//! Kernel-owned descendant boundary; a process group alone cannot contain setsid.
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const ROOT: &str = "/run/luna/cgroups";
const CGROUP2_MAGIC: u64 = 0x63677270;
static NEXT: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub struct ProcessCgroup {
    path: PathBuf,
    membership: String,
    procs: File,
    kill: File,
}

impl ProcessCgroup {
    pub fn new() -> io::Result<Self> {
        let runtime = fs::symlink_metadata("/run/luna")?;
        if !runtime.is_dir() || runtime.uid() != 0 || runtime.mode() & 0o022 != 0 {
            return Err(io::Error::other("unsafe Luna runtime directory"));
        }
        let root = Path::new(ROOT);
        match fs::create_dir(root) {
            Ok(()) => fs::set_permissions(root, fs::Permissions::from_mode(0o700))?,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        let directory = File::options()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(root)?;
        let metadata = directory.metadata()?;
        if metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
            return Err(io::Error::other("unsafe cgroup mount directory"));
        }
        let mut info: libc::statfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstatfs(directory.as_raw_fd(), &mut info) } != 0 {
            return Err(io::Error::last_os_error());
        }
        if info.f_type as u64 != CGROUP2_MAGIC {
            if fs::read_dir(root)?.next().is_some() {
                return Err(io::Error::other("cgroup mountpoint is not empty"));
            }
            // This private runtime mount exposes the same kernel unified hierarchy.
            if unsafe {
                libc::mount(
                    c"none".as_ptr(),
                    c"/run/luna/cgroups".as_ptr(),
                    c"cgroup2".as_ptr(),
                    libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
                    std::ptr::null(),
                )
            } != 0
            {
                return Err(io::Error::last_os_error());
            }
        }
        let name = format!(
            "luna-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = root.join(&name);
        fs::create_dir(&path)?;
        let open = |name: &str| {
            OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
                .open(path.join(name))
        };
        let result = (|| {
            Ok(Self {
                procs: open("cgroup.procs")?,
                kill: open("cgroup.kill")?,
                membership: format!("/{name}"),
                path: path.clone(),
            })
        })();
        if result.is_err() {
            let _ = fs::remove_dir(path);
        }
        result
    }

    /// Called in the forked child before namespace setup and credential drop.
    pub fn enter(fd: &File) -> io::Result<()> {
        if unsafe { libc::write(fd.as_raw_fd(), b"0\n".as_ptr().cast(), 2) } != 2 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    pub fn child_membership_fd(&self) -> io::Result<File> {
        self.procs.try_clone()
    }
    pub fn membership(&self) -> &str {
        &self.membership
    }
    pub fn contains(&self, pid: i32) -> io::Result<bool> {
        if pid <= 0 {
            return Ok(false);
        }
        let text = fs::read_to_string(format!("/proc/{pid}/cgroup"))?;
        Ok(member_of(&text, &self.membership))
    }
    pub fn kill_and_wait(&self) -> io::Result<()> {
        if unsafe { libc::write(self.kill.as_raw_fd(), b"1\n".as_ptr().cast(), 2) } != 2 {
            return Err(io::Error::last_os_error());
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        while !fs::read_to_string(self.path.join("cgroup.events"))?
            .lines()
            .any(|l| l == "populated 0")
        {
            if Instant::now() >= deadline {
                return Err(io::Error::other("application descendants did not exit"));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }
}
impl Drop for ProcessCgroup {
    fn drop(&mut self) {
        if let Err(error) = self
            .kill_and_wait()
            .and_then(|_| fs::remove_dir(&self.path))
        {
            eprintln!("luna-system-runtime: cgroup cleanup failed: {error}");
        }
    }
}
pub fn member_of(text: &str, membership: &str) -> bool {
    text.lines()
        .any(|line| line.strip_prefix("0::") == Some(membership))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn membership_is_exact_and_unified() {
        assert!(member_of("0::/luna-2-1\n", "/luna-2-1"));
        assert!(!member_of("0::/luna-2-10\n", "/luna-2-1"));
        assert!(!member_of("0::/luna-2-1/foreign\n", "/luna-2-1"));
        assert!(!member_of("2:cpu:/luna-2-1\n", "/luna-2-1"));
    }
}
