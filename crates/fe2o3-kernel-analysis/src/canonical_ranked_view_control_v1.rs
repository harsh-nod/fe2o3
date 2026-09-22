//! Occurrence-preserving control queries and stationary scoped accounting.
use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkLedgerIdentityV1, Terminator};

pub(super) fn terminator(term: &Terminator) -> (TerminatorClass, Obligations) {
    let control = Obligations::NONE.with(Obligation::Control);
    match term {
        Terminator::Branch { .. } => (TerminatorClass::Branch, control),
        Terminator::ConditionalBranch { .. } => (TerminatorClass::ConditionalBranch, control),
        Terminator::Switch { .. } => (TerminatorClass::Switch, control),
        Terminator::IntegerSwitch { .. } => (TerminatorClass::IntegerSwitch, control),
        Terminator::Return { .. } => (TerminatorClass::Return, control),
        Terminator::Unreachable => (
            TerminatorClass::Unreachable,
            control.with(Obligation::TrapBehavior),
        ),
    }
}

pub(super) fn edge(
    inventory: &Inventory<'_>,
    ordinal: usize,
    budget: &mut Budget<'_>,
) -> Result<EdgeClass> {
    budget.charge_work(3)?;
    let edge = inventory
        .edges()
        .get(ordinal)
        .ok_or(Error::InconsistentInventory)?;
    let source = edge.coordinate.source;
    let function = inventory
        .functions()
        .get(source.function.0 as usize)
        .ok_or(Error::InconsistentInventory)?;
    let block = add(function.blocks.start, source.block as usize)?;
    if block >= function.blocks.end {
        return Err(Error::InconsistentInventory);
    }
    let term = inventory
        .blocks()
        .get(block)
        .ok_or(Error::InconsistentInventory)?
        .terminator;
    let successor = edge.coordinate.successor as usize;
    match term {
        Terminator::Branch { .. } if successor == 0 => Ok(EdgeClass::Branch),
        Terminator::ConditionalBranch { .. } if successor == 0 => Ok(EdgeClass::True),
        Terminator::ConditionalBranch { .. } if successor == 1 => Ok(EdgeClass::False),
        Terminator::Switch { cases, .. } if successor < cases.len() => {
            Ok(EdgeClass::SwitchCase(successor))
        }
        Terminator::Switch { cases, .. } if successor == cases.len() => {
            Ok(EdgeClass::SwitchDefault)
        }
        Terminator::IntegerSwitch { cases, .. } if successor < cases.len() => {
            Ok(EdgeClass::IntegerCase(successor))
        }
        Terminator::IntegerSwitch { cases, .. } if successor == cases.len() => {
            Ok(EdgeClass::IntegerDefault)
        }
        Terminator::Branch { .. }
        | Terminator::ConditionalBranch { .. }
        | Terminator::Switch { .. }
        | Terminator::IntegerSwitch { .. }
        | Terminator::Return { .. }
        | Terminator::Unreachable => Err(Error::InconsistentInventory),
    }
}

pub(super) struct Accounting {
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    retained: usize,
    failure: Option<Error>,
    cleanup: bool,
}
impl Accounting {
    pub(super) fn new(budget: &Budget<'_>, floor: usize, retained: usize) -> Self {
        Self {
            slot: budget as *const Budget<'_> as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor,
            retained,
            failure: None,
            cleanup: true,
        }
    }
    fn same(&self, budget: &Budget<'_>) -> bool {
        self.slot == budget as *const Budget<'_> as usize
            && self.ledger == budget.work_ledger_identity_v1()
    }
    pub(super) fn fail(&mut self, error: Error) -> Error {
        self.failure.get_or_insert(error).clone()
    }
    pub(super) fn charge(&mut self, budget: &mut Budget<'_>, work: usize) -> Result<()> {
        let expected = self
            .floor
            .checked_add(self.retained)
            .ok_or(Resource::Arithmetic)?;
        if !self.same(budget) {
            return Err(self.fail(Resource::Accounting.into()));
        }
        if budget.storage() < expected {
            self.cleanup = false;
            return Err(self.fail(Resource::Accounting.into()));
        }
        if budget.storage() != expected {
            return Err(self.fail(Resource::Accounting.into()));
        }
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        budget
            .charge_work(work)
            .map_err(|error| self.fail(error.into()))
    }
    pub(super) fn postcheck(&mut self, budget: &Budget<'_>) -> Result<()> {
        let expected = self
            .floor
            .checked_add(self.retained)
            .ok_or(Resource::Arithmetic)?;
        if !self.same(budget) || budget.storage() < expected {
            self.cleanup = false;
            self.fail(Resource::Accounting.into());
        } else if budget.storage() != expected {
            self.fail(Resource::Accounting.into());
        }
        self.failure.clone().map_or(Ok(()), Err)
    }
    pub(super) fn release(&self, budget: &mut Budget<'_>) -> Result<()> {
        if self.cleanup && self.same(budget) {
            budget.release_storage(self.retained)?;
        }
        Ok(())
    }
}

/// No external callback runs during candidate construction. Rollback may therefore
/// release precisely its accepted delta after all owned candidates have dropped.
pub(super) fn transaction<T>(
    budget: &mut Budget<'_>,
    run: impl FnOnce(&mut Budget<'_>) -> Result<T>,
) -> Result<T> {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let floor = budget.storage();
    let result = catch_unwind(AssertUnwindSafe(|| run(budget)));
    let delta = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)?;
    budget.release_storage(delta)?;
    match result {
        Ok(value) => value,
        Err(payload) => {
            // The caller's floor is restored before a hostile panic payload drops.
            drop(payload);
            Err(Error::Panicked)
        }
    }
}
