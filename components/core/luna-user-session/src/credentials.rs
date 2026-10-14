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

    /// Resolve supplementary groups in the parent, before fork and chroot.
    pub fn supplementary_groups(&self) -> io::Result<Vec<u32>> {
        let name = CString::new(self.username.as_str())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "username contains NUL"))?;
        let mut count = 16;
        let mut groups = vec![0; count as usize];
        // SAFETY: buffers have count elements; libc reports the required size.
        if unsafe { libc::getgrouplist(name.as_ptr(), self.gid, groups.as_mut_ptr(), &mut count) } < 0 {
            if !(1..=65536).contains(&count) {
                return Err(io::Error::other("invalid supplementary group count"));
            }
            groups.resize(count as usize, 0);
            if unsafe { libc::getgrouplist(name.as_ptr(), self.gid, groups.as_mut_ptr(), &mut count) } < 0 {
                return Err(io::Error::other("cannot resolve supplementary groups"));
            }
        }
        groups.truncate(count as usize);
        Ok(groups)
    }
    /// Only credential syscalls in the child; no NSS lookup inside the app root.
    pub fn apply_with_groups(&self, groups: &[u32]) -> io::Result<()> {
        // SAFETY: group slice is valid and the privileged child owns its credentials.
        if unsafe { libc::setgroups(groups.len(), groups.as_ptr()) } != 0
            || unsafe { libc::setresgid(self.gid, self.gid, self.gid) } != 0
            || unsafe { libc::setresuid(self.uid, self.uid, self.uid) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
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
