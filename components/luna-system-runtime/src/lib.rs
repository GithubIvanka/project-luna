//! System-wide runtime supervision boundary for Project Luna.
//!
//! One system runtime supervises multiple `UserSession` instances. It owns
//! system-runtime orchestration and process supervision; security policy,
//! application installation, and Linux namespace mechanics remain outside this
//! crate.

use std::collections::BTreeMap;
use std::fmt;
use std::process::{Child, Command, ExitStatus, Stdio};

use luna_event::{EventPublisher, EventType};
use luna_user_session::UserSession;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeState {
    Starting,
    Running,
    Degraded,
    Stopping,
    Stopped,
}

pub trait SystemRuntime {
    type Error;

    fn state(&self) -> RuntimeState;
    fn sessions(&self) -> Vec<&UserSession>;
    fn supervise(&mut self) -> Result<(), Self::Error>;
}

/// Marker for the component that owns the runtime event source.
pub fn session_event_type() -> EventType {
    EventType::new("session.state.changed")
}

/// Documents the dependency without choosing a concrete event transport.
pub fn accepts_event_publisher<P: EventPublisher>(_publisher: &P) {}

/// Opaque identifier for a child process supervised by system-runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ProcessId(u32);

impl ProcessId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Debug)]
pub enum ProcessError {
    Spawn(std::io::Error),
    Kill(std::io::Error),
    Wait(std::io::Error),
    Unknown(ProcessId),
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(error) => write!(f, "failed to spawn child process: {error}"),
            Self::Kill(error) => write!(f, "failed to terminate child process: {error}"),
            Self::Wait(error) => write!(f, "failed to reap child process: {error}"),
            Self::Unknown(id) => write!(f, "unknown supervised process {}", id.get()),
        }
    }
}

impl std::error::Error for ProcessError {}

#[derive(Debug)]
pub enum ProcessState {
    Running,
    Exited(ExitStatus),
}

#[derive(Debug)]
struct SupervisedProcess {
    child: Child,
}

/// Minimal real process supervisor for the system-runtime boundary.
///
/// This intentionally does not construct application namespaces or make
/// security decisions. Callers must provide an already-authorized executable
/// and argument vector. Namespace materialization stays in `luna-namespace`.
#[derive(Default)]
pub struct ProcessSupervisor {
    processes: BTreeMap<ProcessId, SupervisedProcess>,
}

impl ProcessSupervisor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawn a real child process and register it for supervision.
    pub fn spawn<I, S>(&mut self, program: &str, args: I) -> Result<ProcessId, ProcessError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(ProcessError::Spawn)?;
        let id = ProcessId::new(child.id());
        self.processes.insert(id, SupervisedProcess { child });
        Ok(id)
    }

    /// Poll one process without blocking.
    pub fn poll(&mut self, id: ProcessId) -> Result<ProcessState, ProcessError> {
        let process = self.processes.get_mut(&id).ok_or(ProcessError::Unknown(id))?;
        match process.child.try_wait().map_err(ProcessError::Wait)? {
            Some(status) => {
                self.processes.remove(&id);
                Ok(ProcessState::Exited(status))
            }
            None => Ok(ProcessState::Running),
        }
    }

    /// Terminate a supervised process and reap it.
    pub fn terminate(&mut self, id: ProcessId) -> Result<ExitStatus, ProcessError> {
        let mut process = self.processes.remove(&id).ok_or(ProcessError::Unknown(id))?;
        process.child.kill().map_err(ProcessError::Kill)?;
        process.child.wait().map_err(ProcessError::Wait)
    }

    /// Poll all registered children and return the ones that have exited.
    pub fn reap_finished(&mut self) -> Result<Vec<(ProcessId, ExitStatus)>, ProcessError> {
        let ids: Vec<_> = self.processes.keys().copied().collect();
        let mut finished = Vec::new();
        for id in ids {
            if let ProcessState::Exited(status) = self.poll(id)? {
                finished.push((id, status));
            }
        }
        Ok(finished)
    }

    pub fn contains(&self, id: ProcessId) -> bool {
        self.processes.contains_key(&id)
    }

    pub fn len(&self) -> usize {
        self.processes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.processes.is_empty()
    }
}

impl Drop for ProcessSupervisor {
    fn drop(&mut self) {
        let ids: Vec<_> = self.processes.keys().copied().collect();
        for id in ids {
            let _ = self.terminate(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{session_event_type, ProcessState, ProcessSupervisor};

    #[test]
    fn runtime_has_stable_session_event_name() {
        assert_eq!(session_event_type().as_str(), "session.state.changed");
    }

    #[test]
    fn supervisor_spawns_and_reaps_real_child() {
        let mut supervisor = ProcessSupervisor::new();
        let id = supervisor
            .spawn("true", std::iter::empty::<&str>())
            .expect("spawn true");
        assert!(supervisor.contains(id));

        let mut exited = false;
        for _ in 0..50 {
            match supervisor.poll(id).expect("poll child") {
                ProcessState::Running => std::thread::sleep(std::time::Duration::from_millis(5)),
                ProcessState::Exited(status) => {
                    assert!(status.success());
                    exited = true;
                    break;
                }
            }
        }
        assert!(exited, "child did not exit during bounded polling");
        assert!(supervisor.is_empty());
    }

    #[test]
    fn supervisor_terminates_long_lived_child() {
        let mut supervisor = ProcessSupervisor::new();
        let id = supervisor
            .spawn("sleep", ["60"])
            .expect("spawn sleep");
        assert!(supervisor.contains(id));
        let status = supervisor.terminate(id).expect("terminate child");
        assert!(!status.success());
        assert!(supervisor.is_empty());
    }
}
