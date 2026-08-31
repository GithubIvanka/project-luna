//! Change-execution boundary for Project Luna.
//!
//! `luna-update-manager` owns mutation execution. Domain managers remain owners
//! of their domain state; this crate coordinates a transaction across those
//! managers through an explicit backend and a durable state journal.

use luna_common::{BundleId, Version};
use luna_state::{Revision, StateKey, StateStore, StateTransaction, StateValue};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateOperation {
    InstallSystemImage(Version),
    RemoveSystemImage(Version),
    InstallKernel(Version),
    RemoveKernel(Version),
    InstallApplication(BundleId, Version),
    RemoveApplication(BundleId, Version),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdatePlan {
    operations: Vec<UpdateOperation>,
}

impl UpdatePlan {
    pub fn new() -> Self {
        Self { operations: Vec::new() }
    }
    pub fn push(&mut self, operation: UpdateOperation) { self.operations.push(operation); }
    pub fn operations(&self) -> &[UpdateOperation] { &self.operations }
    pub fn is_empty(&self) -> bool { self.operations.is_empty() }
    pub fn validate(&self) -> Result<(), UpdateError> {
        if self.is_empty() { return Err(UpdateError::new("update plan is empty")); }
        for window in self.operations.windows(2) {
            if matches!((&window[0], &window[1]), (UpdateOperation::RemoveSystemImage(_), UpdateOperation::InstallSystemImage(_))) {
                return Err(UpdateError::new("system image removal cannot precede installation"));
            }
        }
        Ok(())
    }
}
impl Default for UpdatePlan { fn default() -> Self { Self::new() } }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateId(String);
impl UpdateId {
    pub fn new(value: impl Into<String>) -> Result<Self, UpdateError> {
        let value = value.into();
        if value.trim().is_empty() || value.contains("..") || value.contains('/') { return Err(UpdateError::new("invalid update identifier")); }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str { &self.0 }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdatePhase { Prepared, Checkpointed, Applying, Verifying, Committed, RolledBack, Failed }
impl UpdatePhase {
    fn as_str(self) -> &'static str {
        match self { Self::Prepared=>"prepared", Self::Checkpointed=>"checkpointed", Self::Applying=>"applying", Self::Verifying=>"verifying", Self::Committed=>"committed", Self::RolledBack=>"rolled-back", Self::Failed=>"failed" }
    }
}

#[derive(Debug)]
pub struct UpdateError(String);
impl UpdateError { pub fn new(message: impl Into<String>) -> Self { Self(message.into()) } }
impl std::fmt::Display for UpdateError { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.0) } }
impl std::error::Error for UpdateError {}

/// Domain managers provide the physical mutation backend. `prepare` must not
/// perform an irreversible mutation; `apply` is the mutation boundary.
pub trait UpdateBackend {
    fn prepare(&mut self, plan: &UpdatePlan) -> Result<(), UpdateError>;
    fn apply(&mut self, operation: &UpdateOperation) -> Result<(), UpdateError>;
    fn verify(&mut self, plan: &UpdatePlan) -> Result<(), UpdateError>;
    fn rollback(&mut self, operation: &UpdateOperation) -> Result<(), UpdateError>;
}

#[derive(Default)]
pub struct RecordingUpdateBackend { applied: Vec<UpdateOperation> }
impl RecordingUpdateBackend { pub fn applied(&self) -> &[UpdateOperation] { &self.applied } }
impl UpdateBackend for RecordingUpdateBackend {
    fn prepare(&mut self, _plan: &UpdatePlan) -> Result<(), UpdateError> { Ok(()) }
    fn apply(&mut self, operation: &UpdateOperation) -> Result<(), UpdateError> { self.applied.push(operation.clone()); Ok(()) }
    fn verify(&mut self, _plan: &UpdatePlan) -> Result<(), UpdateError> { Ok(()) }
    fn rollback(&mut self, operation: &UpdateOperation) -> Result<(), UpdateError> {
        if let Some(index) = self.applied.iter().rposition(|item| item == operation) { self.applied.remove(index); }
        Ok(())
    }
}

pub struct UpdateEngine<'a, S: StateStore, B: UpdateBackend> { store: &'a mut S, backend: &'a mut B }
impl<'a, S: StateStore, B: UpdateBackend> UpdateEngine<'a, S, B> {
    pub fn new(store: &'a mut S, backend: &'a mut B) -> Self { Self { store, backend } }

    pub fn prepare(&mut self, id: &UpdateId, plan: &UpdatePlan) -> Result<Revision, UpdateError> {
        plan.validate()?;
        let mut transaction = StateTransaction::new();
        transaction
            .set(phase_key(id), StateValue::new(UpdatePhase::Prepared.as_str().as_bytes().to_vec()))
            .set(plan_key(id), StateValue::new(format_plan(plan).into_bytes()))
            .set(applied_key(id), StateValue::new(Vec::new()))
            .set(inflight_key(id), StateValue::new(Vec::new()));
        self.store.transaction(self.store.revision(), transaction).map_err(state_error)?;
        self.backend.prepare(plan)?;
        Ok(self.store.revision())
    }

    pub fn checkpoint(&mut self, id: &UpdateId) -> Result<Revision, UpdateError> { self.set_phase(id, UpdatePhase::Checkpointed) }

    pub fn execute(&mut self, id: &UpdateId, plan: &UpdatePlan) -> Result<Revision, UpdateError> {
        self.prepare(id, plan)?;
        self.checkpoint(id)?;
        self.set_phase(id, UpdatePhase::Applying)?;

        let mut applied = Vec::new();
        for (index, operation) in plan.operations().iter().enumerate() {
            self.set_progress(id, &applied, Some(index))?;
            if let Err(error) = self.backend.apply(operation) {
                for completed in applied.iter().rev() {
                    if let Err(rollback_error) = self.backend.rollback(&plan.operations()[*completed]) {
                        let _ = self.set_phase(id, UpdatePhase::Failed);
                        return Err(UpdateError::new(format!("update failed: {error}; rollback failed: {rollback_error}")));
                    }
                }
                let _ = self.set_progress(id, &[], None);
                let _ = self.set_phase(id, UpdatePhase::RolledBack);
                return Err(error);
            }
            applied.push(index);
            self.set_progress(id, &applied, None)?;
        }

        self.set_phase(id, UpdatePhase::Verifying)?;
        if let Err(error) = self.backend.verify(plan) {
            for index in applied.iter().rev() {
                if let Err(rollback_error) = self.backend.rollback(&plan.operations()[*index]) {
                    let _ = self.set_phase(id, UpdatePhase::Failed);
                    return Err(UpdateError::new(format!("verification failed: {error}; rollback failed: {rollback_error}")));
                }
            }
            let _ = self.set_progress(id, &[], None);
            let _ = self.set_phase(id, UpdatePhase::RolledBack);
            return Err(error);
        }
        self.set_phase(id, UpdatePhase::Committed)
    }

    pub fn reconcile_interrupted(&mut self, id: &UpdateId, plan: &UpdatePlan) -> Result<Revision, UpdateError> {
        plan.validate()?;
        let phase = self.store.get(&phase_key(id)).map_err(state_error)?.ok_or_else(|| UpdateError::new("update journal entry not found"))?;
        let phase = std::str::from_utf8(phase.as_slice()).map_err(|_| UpdateError::new("update journal phase is not valid UTF-8"))?;
        if matches!(phase, "committed" | "rolled-back" | "failed") { return Err(UpdateError::new("update is already in a terminal state")); }
        let mut indices = self.read_applied_indexes(id)?;
        if let Some(index) = self.read_inflight_index(id)? { indices.push(index); }
        indices.sort_unstable();
        indices.dedup();
        for index in indices.iter().rev() {
            let operation = plan.operations().get(*index).ok_or_else(|| UpdateError::new("update journal references an invalid operation index"))?;
            self.backend.rollback(operation)?;
        }
        self.set_progress(id, &[], None)?;
        self.set_phase(id, UpdatePhase::RolledBack)
    }

    fn set_phase(&mut self, id: &UpdateId, phase: UpdatePhase) -> Result<Revision, UpdateError> {
        let mut transaction = StateTransaction::new();
        transaction.set(phase_key(id), StateValue::new(phase.as_str().as_bytes().to_vec()));
        self.store.transaction(self.store.revision(), transaction).map_err(state_error)
    }

    fn set_progress(&mut self, id: &UpdateId, applied: &[usize], inflight: Option<usize>) -> Result<Revision, UpdateError> {
        let mut transaction = StateTransaction::new();
        let applied = applied.iter().map(|index| index.to_string()).collect::<Vec<_>>().join(",");
        let inflight = inflight.map_or_else(String::new, |index| index.to_string());
        transaction
            .set(applied_key(id), StateValue::new(applied.into_bytes()))
            .set(inflight_key(id), StateValue::new(inflight.into_bytes()));
        self.store.transaction(self.store.revision(), transaction).map_err(state_error)
    }

    fn read_applied_indexes(&self, id: &UpdateId) -> Result<Vec<usize>, UpdateError> {
        let value = self.store.get(&applied_key(id)).map_err(state_error)?.unwrap_or_else(|| StateValue::new(Vec::new()));
        parse_indexes(value.as_slice())
    }

    fn read_inflight_index(&self, id: &UpdateId) -> Result<Option<usize>, UpdateError> {
        let value = self.store.get(&inflight_key(id)).map_err(state_error)?.unwrap_or_else(|| StateValue::new(Vec::new()));
        if value.as_slice().is_empty() { return Ok(None); }
        std::str::from_utf8(value.as_slice()).map_err(|_| UpdateError::new("update journal inflight index is not UTF-8"))?.parse::<usize>().map(Some).map_err(|_| UpdateError::new("update journal inflight index is invalid"))
    }
}

pub struct TransactionalUpdateExecutor<'a, S: StateStore> { store: &'a mut S }
impl<'a, S: StateStore> TransactionalUpdateExecutor<'a, S> {
    pub fn new(store: &'a mut S) -> Self { Self { store } }
    pub fn execute_at_revision(&mut self, expected: Revision, plan: &UpdatePlan) -> Result<Revision, UpdateError> {
        plan.validate()?;
        let mut transaction = StateTransaction::new();
        for (index, operation) in plan.operations().iter().enumerate() {
            transaction.set(StateKey::new(format!("update/plan/{index}")), StateValue::new(format!("{operation:?}").into_bytes()));
        }
        self.store.transaction(expected, transaction).map_err(state_error)
    }
}
impl<'a, S: StateStore> UpdateExecutor for TransactionalUpdateExecutor<'a, S> {
    fn execute(&mut self, plan: &UpdatePlan) -> Result<(), UpdateError> { let revision = self.store.revision(); self.execute_at_revision(revision, plan).map(|_| ()) }
}
pub trait UpdateExecutor { fn execute(&mut self, plan: &UpdatePlan) -> Result<(), UpdateError>; }

fn phase_key(id: &UpdateId) -> StateKey { StateKey::new(format!("updates/{}/phase", id.as_str())) }
fn plan_key(id: &UpdateId) -> StateKey { StateKey::new(format!("updates/{}/plan", id.as_str())) }
fn applied_key(id: &UpdateId) -> StateKey { StateKey::new(format!("updates/{}/applied", id.as_str())) }
fn inflight_key(id: &UpdateId) -> StateKey { StateKey::new(format!("updates/{}/inflight", id.as_str())) }
fn format_plan(plan: &UpdatePlan) -> String { plan.operations().iter().enumerate().map(|(index, operation)| format!("{index}:{operation:?}")).collect::<Vec<_>>().join("\n") }
fn parse_indexes(value: &[u8]) -> Result<Vec<usize>, UpdateError> {
    if value.is_empty() { return Ok(Vec::new()); }
    std::str::from_utf8(value).map_err(|_| UpdateError::new("update journal applied indexes are not UTF-8"))?.split(',').filter(|entry| !entry.is_empty()).map(|entry| entry.parse::<usize>().map_err(|_| UpdateError::new("update journal applied index is invalid"))).collect()
}
fn state_error(error: luna_state::StateError) -> UpdateError { UpdateError::new(error.to_string()) }

#[cfg(test)]
mod tests {
    use super::*;
    use luna_state::{MemoryStateStore, StateStore};
    #[test]
    fn plan_preserves_operations() { let mut plan=UpdatePlan::new(); plan.push(UpdateOperation::InstallKernel(Version::new(9,0,0))); plan.push(UpdateOperation::InstallApplication(BundleId::from("example.app"),Version::new(2,0,0))); assert_eq!(plan.operations().len(),2); }
    #[test]
    fn execution_is_one_atomic_revision_for_legacy_executor() { let mut store=MemoryStateStore::new(); let mut plan=UpdatePlan::new(); plan.push(UpdateOperation::InstallKernel(Version::new(9,0,0))); plan.push(UpdateOperation::InstallSystemImage(Version::new(2,0,0))); let revision=store.revision(); TransactionalUpdateExecutor::new(&mut store).execute_at_revision(revision,&plan).unwrap(); assert_eq!(store.revision(),revision.next()); assert!(store.get(&StateKey::new("update/plan/0")).unwrap().is_some()); }
    #[test]
    fn engine_records_commit_and_clears_inflight() { let mut store=MemoryStateStore::new(); let mut backend=RecordingUpdateBackend::default(); let id=UpdateId::new("test-1").unwrap(); let mut plan=UpdatePlan::new(); plan.push(UpdateOperation::InstallKernel(Version::new(9,0,0))); UpdateEngine::new(&mut store,&mut backend).execute(&id,&plan).unwrap(); assert_eq!(backend.applied().len(),1); assert_eq!(store.get(&phase_key(&id)).unwrap().unwrap().as_slice(),b"committed"); assert_eq!(store.get(&inflight_key(&id)).unwrap().unwrap().as_slice(),b""); }
    #[test]
    fn failing_backend_is_rolled_back() { struct FailingBackend { applied: Vec<UpdateOperation> } impl UpdateBackend for FailingBackend { fn prepare(&mut self,_:&UpdatePlan)->Result<(),UpdateError>{Ok(())} fn apply(&mut self,operation:&UpdateOperation)->Result<(),UpdateError>{ if self.applied.is_empty(){self.applied.push(operation.clone());Ok(())}else{Err(UpdateError::new("injected failure"))}} fn verify(&mut self,_:&UpdatePlan)->Result<(),UpdateError>{Ok(())} fn rollback(&mut self,operation:&UpdateOperation)->Result<(),UpdateError>{self.applied.retain(|item|item!=operation);Ok(())} } let mut store=MemoryStateStore::new(); let mut backend=FailingBackend{applied:Vec::new()}; let id=UpdateId::new("test-failure").unwrap(); let mut plan=UpdatePlan::new(); plan.push(UpdateOperation::InstallKernel(Version::new(9,0,0))); plan.push(UpdateOperation::InstallSystemImage(Version::new(2,0,0))); assert!(UpdateEngine::new(&mut store,&mut backend).execute(&id,&plan).is_err()); assert!(backend.applied.is_empty()); assert_eq!(store.get(&phase_key(&id)).unwrap().unwrap().as_slice(),b"rolled-back"); }
    #[test]
    fn interrupted_operation_rolls_back_inflight_and_applied_steps() { let mut store=MemoryStateStore::new(); let mut backend=RecordingUpdateBackend::default(); let id=UpdateId::new("test-interrupted").unwrap(); let mut plan=UpdatePlan::new(); plan.push(UpdateOperation::InstallKernel(Version::new(9,0,0))); plan.push(UpdateOperation::InstallSystemImage(Version::new(2,0,0))); { let mut engine=UpdateEngine::new(&mut store,&mut backend); engine.prepare(&id,&plan).unwrap(); engine.checkpoint(&id).unwrap(); engine.set_phase(&id,UpdatePhase::Applying).unwrap(); backend.apply(&plan.operations()[0]).unwrap(); engine.set_progress(&id,&[0],Some(1)).unwrap(); } assert!(UpdateEngine::new(&mut store,&mut backend).reconcile_interrupted(&id,&plan).is_ok()); assert!(backend.applied().is_empty()); }
}
