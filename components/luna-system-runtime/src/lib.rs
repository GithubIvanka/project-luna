//! System-wide runtime supervision boundary for Project Luna.
//! `system-runtime` is the sole owner of supervised processes.

use std::collections::BTreeMap;
use std::fmt;
use std::process::{Child, Command, ExitStatus, Stdio};

use luna_event::{EventPublisher, EventType};
use luna_state::RedbStateStore;
use luna_system_manager::{PersistentSystemManager, SystemState};
use luna_user_session::{SessionId, SessionState, UserSession};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeState { Starting, Running, Degraded, Stopping, Stopped }

pub trait SystemRuntime { type Error; fn state(&self)->RuntimeState; fn sessions(&self)->Vec<&UserSession>; fn supervise(&mut self)->Result<(),Self::Error>; }
pub fn session_event_type()->EventType{EventType::new("session.state.changed")}
pub fn accepts_event_publisher<P:EventPublisher>(_publisher:&P){}

#[derive(Clone,Copy,Debug,Eq,PartialEq,Ord,PartialOrd,Hash)]
pub struct ProcessId(u32);
impl ProcessId{pub const fn new(value:u32)->Self{Self(value)}pub const fn get(self)->u32{self.0}}
#[derive(Debug)]
pub enum ProcessError{Spawn(std::io::Error),Kill(std::io::Error),Wait(std::io::Error),Unknown(ProcessId)}
impl fmt::Display for ProcessError{fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{match self{Self::Spawn(e)=>write!(f,"failed to spawn child process: {e}"),Self::Kill(e)=>write!(f,"failed to terminate child process: {e}"),Self::Wait(e)=>write!(f,"failed to reap child process: {e}"),Self::Unknown(id)=>write!(f,"unknown supervised process {}",id.get())}}}
impl std::error::Error for ProcessError{}
#[derive(Debug)]pub enum ProcessState{Running,Exited(ExitStatus)}
#[derive(Debug)]struct SupervisedProcess{child:Child}
#[derive(Default)]pub struct ProcessSupervisor{processes:BTreeMap<ProcessId,SupervisedProcess>}
impl ProcessSupervisor{
 pub fn new()->Self{Self::default()}
 pub fn spawn<I,S>(&mut self,program:&str,args:I)->Result<ProcessId,ProcessError> where I:IntoIterator<Item=S>,S:AsRef<std::ffi::OsStr>{self.spawn_command(program,args,false,None::<fn()->std::io::Result<()>>)}
 pub fn spawn_inherited<I,S>(&mut self,program:&str,args:I)->Result<ProcessId,ProcessError> where I:IntoIterator<Item=S>,S:AsRef<std::ffi::OsStr>{self.spawn_command(program,args,true,None::<fn()->std::io::Result<()>>)}
 #[cfg(unix)]pub fn spawn_with_pre_exec<I,S,F>(&mut self,program:&str,args:I,setup:F)->Result<ProcessId,ProcessError> where I:IntoIterator<Item=S>,S:AsRef<std::ffi::OsStr>,F:FnMut()->std::io::Result<()>+Send+Sync+'static{self.spawn_command(program,args,false,Some(setup))}
 fn spawn_command<I,S,F>(&mut self,program:&str,args:I,inherited_stdio:bool,setup:Option<F>)->Result<ProcessId,ProcessError> where I:IntoIterator<Item=S>,S:AsRef<std::ffi::OsStr>,F:FnMut()->std::io::Result<()>+Send+Sync+'static{let mut command=Command::new(program);command.args(args);if inherited_stdio{command.stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit());}else{command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());}#[cfg(unix)]if let Some(setup)=setup{unsafe{std::os::unix::process::CommandExt::pre_exec(&mut command,setup);}}let child=command.spawn().map_err(ProcessError::Spawn)?;let id=ProcessId::new(child.id());self.processes.insert(id,SupervisedProcess{child});Ok(id)}
 pub fn poll(&mut self,id:ProcessId)->Result<ProcessState,ProcessError>{let process=self.processes.get_mut(&id).ok_or(ProcessError::Unknown(id))?;match process.child.try_wait().map_err(ProcessError::Wait)?{Some(status)=>{self.processes.remove(&id);Ok(ProcessState::Exited(status))},None=>Ok(ProcessState::Running)}}
 pub fn terminate(&mut self,id:ProcessId)->Result<ExitStatus,ProcessError>{let mut process=self.processes.remove(&id).ok_or(ProcessError::Unknown(id))?;process.child.kill().map_err(ProcessError::Kill)?;process.child.wait().map_err(ProcessError::Wait)}
 pub fn reap_finished(&mut self)->Result<Vec<(ProcessId,ExitStatus)>,ProcessError>{let ids:Vec<_>=self.processes.keys().copied().collect();let mut finished=Vec::new();for id in ids{if let ProcessState::Exited(status)=self.poll(id)?{finished.push((id,status));}}Ok(finished)}
 pub fn contains(&self,id:ProcessId)->bool{self.processes.contains_key(&id)} pub fn len(&self)->usize{self.processes.len()} pub fn is_empty(&self)->bool{self.processes.is_empty()}
}
impl Drop for ProcessSupervisor{fn drop(&mut self){let ids:Vec<_>=self.processes.keys().copied().collect();for id in ids{let _=self.terminate(id);}}}

#[derive(Debug)]pub enum RuntimeError{Process(ProcessError),UnknownSession(SessionId),Session(String),State(String)}
impl fmt::Display for RuntimeError{fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{match self{Self::Process(e)=>write!(f,"process supervision failed: {e}"),Self::UnknownSession(id)=>write!(f,"unknown session {}",id.get()),Self::Session(e)=>write!(f,"session transition failed: {e}"),Self::State(e)=>write!(f,"system state failed: {e}")}}}
impl std::error::Error for RuntimeError{} impl From<ProcessError> for RuntimeError{fn from(value:ProcessError)->Self{Self::Process(value)}}

pub struct SystemRuntimeService{state:RuntimeState,next_session_id:u128,sessions:BTreeMap<SessionId,UserSession>,session_processes:BTreeMap<ProcessId,SessionId>,supervisor:ProcessSupervisor,system_manager:Option<PersistentSystemManager<RedbStateStore>>}
impl Default for SystemRuntimeService{fn default()->Self{Self::new()}}
impl SystemRuntimeService{
 pub fn new()->Self{Self{state:RuntimeState::Starting,next_session_id:1,sessions:BTreeMap::new(),session_processes:BTreeMap::new(),supervisor:ProcessSupervisor::new(),system_manager:None}}
 pub fn start(&mut self){self.state=RuntimeState::Running}
 pub fn attach_system_manager(&mut self,manager:PersistentSystemManager<RedbStateStore>){self.system_manager=Some(manager)}
 pub fn system_state(&self)->Option<&SystemState>{self.system_manager.as_ref().map(PersistentSystemManager::state)}
 pub fn system_state_revision(&self)->Option<luna_state::Revision>{self.system_manager.as_ref().map(PersistentSystemManager::revision)}
 pub fn create_session(&mut self,user:luna_common::UserId)->Result<SessionId,RuntimeError>{let id=SessionId::new(self.next_session_id);self.next_session_id=self.next_session_id.saturating_add(1);let mut session=UserSession::new(id,user);session.transition(SessionState::Active).map_err(|e|RuntimeError::Session(e.to_string()))?;self.sessions.insert(id,session);Ok(id)}
 pub fn session(&self,id:SessionId)->Result<&UserSession,RuntimeError>{self.sessions.get(&id).ok_or(RuntimeError::UnknownSession(id))}
 pub fn spawn_process<I,S>(&mut self,program:&str,args:I)->Result<ProcessId,RuntimeError> where I:IntoIterator<Item=S>,S:AsRef<std::ffi::OsStr>{Ok(self.supervisor.spawn(program,args)?)}
 #[cfg(unix)]pub fn spawn_process_with_pre_exec<I,S,F>(&mut self,program:&str,args:I,setup:F)->Result<ProcessId,RuntimeError> where I:IntoIterator<Item=S>,S:AsRef<std::ffi::OsStr>,F:FnMut()->std::io::Result<()>+Send+Sync+'static{Ok(self.supervisor.spawn_with_pre_exec(program,args,setup)?)}
 pub fn poll_process(&mut self,id:ProcessId)->Result<ProcessState,RuntimeError>{Ok(self.supervisor.poll(id)?)}
 pub fn terminate_supervised_process(&mut self,id:ProcessId)->Result<ExitStatus,RuntimeError>{Ok(self.supervisor.terminate(id)?)}
 pub fn launch_session_shell(&mut self,id:SessionId,shell:&str)->Result<ProcessId,RuntimeError>{self.session(id)?;let process=self.supervisor.spawn_inherited(shell,std::iter::empty::<&str>())?;self.session_processes.insert(process,id);Ok(process)}
 pub fn terminate_process(&mut self,process:ProcessId)->Result<(),RuntimeError>{self.supervisor.terminate(process)?;if let Some(session_id)=self.session_processes.remove(&process){self.end_session(session_id)?;}Ok(())}
 fn end_session(&mut self,id:SessionId)->Result<(),RuntimeError>{let session=self.sessions.get_mut(&id).ok_or(RuntimeError::UnknownSession(id))?;if matches!(session.state(),SessionState::Active|SessionState::Restricted){session.transition(SessionState::Ending).map_err(|e|RuntimeError::Session(e.to_string()))?;}if session.state()==SessionState::Ending{session.transition(SessionState::Ended).map_err(|e|RuntimeError::Session(e.to_string()))?;}Ok(())}
}
impl SystemRuntime for SystemRuntimeService{type Error=RuntimeError;fn state(&self)->RuntimeState{self.state}fn sessions(&self)->Vec<&UserSession>{self.sessions.values().collect()}fn supervise(&mut self)->Result<(),Self::Error>{let ids:Vec<_>=self.session_processes.keys().copied().collect();for process in ids{match self.supervisor.poll(process)?{ProcessState::Running=>{},ProcessState::Exited(_)=>{if let Some(session_id)=self.session_processes.remove(&process){self.end_session(session_id)?;}}}}Ok(())}}

#[cfg(test)]mod tests{use super::{session_event_type,ProcessState,ProcessSupervisor,RuntimeState,SystemRuntime,SystemRuntimeService};use luna_common::UserId;use luna_user_session::SessionState;#[test]fn runtime_has_stable_session_event_name(){assert_eq!(session_event_type().as_str(),"session.state.changed");}#[test]fn supervisor_spawns_and_reaps_real_child(){let mut s=ProcessSupervisor::new();let id=s.spawn("true",std::iter::empty::<&str>()).unwrap();let mut exited=false;for _ in 0..50{match s.poll(id).unwrap(){ProcessState::Running=>std::thread::sleep(std::time::Duration::from_millis(5)),ProcessState::Exited(status)=>{assert!(status.success());exited=true;break;}}}assert!(exited);assert!(s.is_empty());}#[test]fn supervisor_terminates_long_lived_child(){let mut s=ProcessSupervisor::new();let id=s.spawn("sleep",["60"]).unwrap();let status=s.terminate(id).unwrap();assert!(!status.success());assert!(s.is_empty());}#[test]fn system_runtime_owns_session_and_process_lifecycle(){let mut rt=SystemRuntimeService::new();rt.start();let session=rt.create_session(UserId::from("luna")).unwrap();let process=rt.launch_session_shell(session,"true").unwrap();for _ in 0..50{rt.supervise().unwrap();if rt.session(session).unwrap().state()==SessionState::Ended{break;}std::thread::sleep(std::time::Duration::from_millis(5));}assert_eq!(rt.session(session).unwrap().state(),SessionState::Ended);assert!(!rt.supervisor.contains(process));}#[test]fn generic_process_survives_session_supervision(){let mut rt=SystemRuntimeService::new();rt.start();let process=rt.spawn_process("true",std::iter::empty::<&str>()).unwrap();rt.supervise().unwrap();assert!(matches!(rt.poll_process(process).unwrap(),ProcessState::Exited(status)if status.success()));}}
