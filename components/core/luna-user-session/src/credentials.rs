use std::ffi::CString;
use std::io;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserCredentials {
    pub uid: u32,
    pub gid: u32,
    pub username: String,
}

impl UserCredentials {
    pub fn new(uid: u32, gid: u32, username: impl Into<String>) -> Self {
        Self {
            uid,
            gid,
            username: username.into(),
        }
    }

    /// Drop the current privileged process into this user's real/effective/saved
    /// identity and initialize the user's supplementary groups.
    ///
    /// It intentionally uses only the Linux credential syscalls exposed by
    /// libc; no external privilege-management utility is required.
    #[cfg(unix)]
    pub fn apply(&self) -> io::Result<()> {
        let username = CString::new(self.username.as_str())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "username contains NUL"))?;

        // SAFETY: username is a valid NUL-terminated string and this function
        // runs in the child immediately before exec, while the process is still
        // privileged. `initgroups` replaces the supplementary group set.
        if unsafe { libc::initgroups(username.as_ptr(), self.gid) } != 0 {
            return Err(io::Error::last_os_error());
        }

        // Use setresgid/setresuid so the child cannot retain a privileged saved
        // identity after the transition.
        if unsafe { libc::setresgid(self.gid, self.gid, self.gid) } != 0 {
            return Err(io::Error::last_os_error());
        }
        if unsafe { libc::setresuid(self.uid, self.uid, self.uid) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}
