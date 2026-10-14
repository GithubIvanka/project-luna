//! AppRuntime delegates child ownership to the existing system supervisor.
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Write};
use std::os::fd::FromRawFd;
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitStatus, Stdio};

use luna_app_runtime::{ApplicationProcessHost, ProcessId, ProcessState};

use crate::process_cgroup::ProcessCgroup;
use crate::{SupervisedProcess, SystemRuntimeService, close_non_stdio_on_exec};

impl ApplicationProcessHost for SystemRuntimeService {
    fn spawn_application(
        &mut self,
        program: &str,
        args: &[OsString],
        env: &[(OsString, OsString)],
        mut setup: Box<dyn FnMut() -> io::Result<()> + Send + Sync>,
    ) -> io::Result<ProcessId> {
        let cgroup = ProcessCgroup::new()?;
        let membership = cgroup.child_membership_fd()?;
        // Command maps application stderr to /dev/null. Retain a CLOEXEC copy
        // solely to report child setup errors to the supervisor's diagnostics.
        let diagnostic_fd = unsafe { libc::fcntl(2, libc::F_DUPFD_CLOEXEC, 3) };
        if diagnostic_fd == -1 {
            return Err(io::Error::last_os_error());
        }
        let mut diagnostic = unsafe { File::from_raw_fd(diagnostic_fd) };
        let mut command = Command::new(program);
        command.args(args).env_clear().envs(env.iter().cloned());
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // SAFETY: the child creates its own process group, applies the authorized
        // namespace/credentials, and closes inherited descriptors before exec.
        unsafe {
            command.pre_exec(move || {
                let result = (|| {
                    // Detach any supervisor controlling TTY. A terminal application may
                    // later acquire only a slave from its own private devpts instance.
                    if libc::setsid() < 0 {
                        return Err(io::Error::last_os_error());
                    }
                    ProcessCgroup::enter(&membership)?;
                    setup()?;
                    close_non_stdio_on_exec()
                })();
                if let Err(error) = &result {
                    let _ = writeln!(
                        diagnostic,
                        "luna-system-runtime: application child setup failed: {error}"
                    );
                }
                result
            });
        }
        let child = command.spawn()?;
        let id = crate::ProcessId::new(child.id());
        self.supervisor.processes.insert(
            id,
            SupervisedProcess {
                child,
                cgroup: Some(cgroup),
            },
        );
        Ok(ProcessId::new(id.get()))
    }

    fn poll_process(&mut self, process: ProcessId) -> io::Result<ProcessState> {
        let id = crate::ProcessId::new(process.get());
        if !self.supervisor.contains(id) {
            return Err(io::Error::other("unknown application process"));
        }
        match self
            .supervisor
            .poll(id)
            .map_err(|e| io::Error::other(e.to_string()))?
        {
            crate::ProcessState::Running => Ok(ProcessState::Running),
            crate::ProcessState::Exited(status) => Ok(ProcessState::Exited(status)),
        }
    }

    fn terminate_supervised_process(&mut self, process: ProcessId) -> io::Result<ExitStatus> {
        let id = crate::ProcessId::new(process.get());
        if !self.supervisor.contains(id) {
            return Err(io::Error::other("unknown application process"));
        }
        self.supervisor
            .terminate(id)
            .map_err(|e| io::Error::other(e.to_string()))
    }
}

impl SystemRuntimeService {
    /// Только чтение границы уже известного процессу runtime экземпляра.
    /// PID клиента сам по себе никогда не выбирает приложение или новую cgroup.
    pub fn application_contains_peer(&self, leader: ProcessId, peer: i32) -> io::Result<bool> {
        let process = self.supervisor.processes.get(&crate::ProcessId::new(leader.get()))
            .ok_or_else(|| io::Error::other("unknown application process"))?;
        match &process.cgroup { Some(cgroup) => cgroup.contains(peer), None => Ok(false) }
    }
}
