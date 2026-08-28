//! Change-execution boundary for Project Luna.
use luna_common::{BundleId, Version};
use luna_state::{Revision, StateKey, StateStore, StateTransaction, StateValue};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateOperation { InstallSystemImage(Version), RemoveSystemImage(Version), InstallKernel(Version), RemoveKernel(Version), InstallApplication(BundleId,Version), RemoveApplication(BundleId,Version) }
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdatePlan{operations:Vec<UpdateOperation>}
impl UpdatePlan{pub fn new()->Self{Self{operations:Vec::new()}} pub fn push(&mut self,o:UpdateOperation){self.operations.push(o)} pub fn operations(&self)->&[UpdateOperation]{&self.operations} pub fn is_empty(&self)->bool{self.operations.is_empty()} pub fn validate(&self)->Result<(),UpdateError>{for window in self.operations.windows(2){if matches!((&window[0],&window[1]),(UpdateOperation::RemoveSystemImage(_),UpdateOperation::InstallSystemImage(_))){return Err(UpdateError::new("system image removal cannot precede installation"));}}Ok(())}}
impl Default for UpdatePlan{fn default()->Self{Self::new()}}
#[derive(Debug)] pub struct UpdateError(String);
impl UpdateError{pub fn new(m:impl Into<String>)->Self{Self(m.into())}}
impl std::fmt::Display for UpdateError{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.write_str(&self.0)}}
impl std::error::Error for UpdateError{}
pub trait UpdateExecutor{fn execute(&mut self,plan:&UpdatePlan)->Result<(),UpdateError>;}

/// Prototype executor that records an update plan as one atomic state transaction.
pub struct TransactionalUpdateExecutor<'a,S:StateStore>{store:&'a mut S}
impl<'a,S:StateStore> TransactionalUpdateExecutor<'a,S>{pub fn new(store:&'a mut S)->Self{Self{store}} pub fn execute_at_revision(&mut self,expected:Revision,plan:&UpdatePlan)->Result<Revision,UpdateError>{plan.validate()?;let mut tx=StateTransaction::new();for (index,op) in plan.operations().iter().enumerate(){tx.set(StateKey::new(format!("update/plan/{index}")),StateValue::new(format!("{op:?}").into_bytes()));}self.store.transaction(expected,tx).map_err(|e|UpdateError::new(e.to_string()))}}
impl<'a,S:StateStore> UpdateExecutor for TransactionalUpdateExecutor<'a,S>{fn execute(&mut self,plan:&UpdatePlan)->Result<(),UpdateError>{let revision=self.store.revision();self.execute_at_revision(revision,plan).map(|_|())}}

#[cfg(test)]
mod tests{use super::*;use luna_state::{MemoryStateStore,StateStore};#[test]fn plan_preserves_operations(){let mut p=UpdatePlan::new();p.push(UpdateOperation::InstallKernel(Version::new(9,0,0)));p.push(UpdateOperation::InstallApplication(BundleId::from("example.app"),Version::new(2,0,0)));assert_eq!(p.operations().len(),2)}#[test]fn execution_is_one_atomic_revision(){let mut s=MemoryStateStore::new();let mut p=UpdatePlan::new();p.push(UpdateOperation::InstallKernel(Version::new(9,0,0)));p.push(UpdateOperation::InstallSystemImage(Version::new(2,0,0)));let r=s.revision();TransactionalUpdateExecutor::new(&mut s).execute_at_revision(r,&p).unwrap();assert_eq!(s.revision(),r.next());assert!(s.get(&StateKey::new("update/plan/0")).unwrap().is_some())}}
