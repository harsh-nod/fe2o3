//! Complete call-effect coverage over one borrowed canonical inventory.
//! This is not a footprint, provenance, termination or refinement proof.

use std::{error::Error as StdError, fmt, mem::size_of};

use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirFunctionCoordinateV1 as Function, OperationKind,
};

use crate::CanonicalKirInventoryV1 as Inventory;

#[path = "canonical_kir_call_effects_v1/visit.rs"]
mod visit;
pub use visit::*;

#[path = "canonical_kir_call_effects_v1/gfx942_inline_v30.rs"]
mod gfx942_inline_v30;

/// Completeness covers physical memory and compiler ordering, including all
/// syntactic calls and blocks. It says nothing about traps or convergence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirCallEffectDecisionV1 {
    CompleteEmpty,
    CompleteNonempty,
    /// A declaration, recursive dependency or opaque assembly is not summarized.
    Incomplete,
}
use CanonicalKirCallEffectDecisionV1 as Decision;

impl Decision {
    fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Incomplete, _) | (_, Self::Incomplete) => Self::Incomplete,
            (Self::CompleteNonempty, _) | (_, Self::CompleteNonempty) => Self::CompleteNonempty,
            _ => Self::CompleteEmpty,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirCallEffectErrorV1 {
    Resource(Resource),
    InvalidFunction(Function),
    Incomplete(Function),
    InconsistentInventory,
}
use CanonicalKirCallEffectErrorV1 as Error;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::InvalidFunction(function) => {
                write!(f, "invalid call-effect function {}", function.0)
            }
            Self::Incomplete(function) => {
                write!(f, "incomplete call effects for function {}", function.0)
            }
            Self::InconsistentInventory => f.write_str("canonical call-effect inventory mismatch"),
        }
    }
}
impl StdError for Error {}
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug)]
enum State {
    Unseen,
    Active,
    Done(Decision),
}

#[derive(Debug)]
struct Frame {
    function: Function,
    operation: usize,
    call: usize,
    decision: Decision,
}

/// Exact-owner classification, with no copied or independently editable graph.
/// The nonempty decision deliberately does not grant read, bounds or alias facts:
/// consumers must inspect the qualified occurrences and establish those facts.
#[derive(Debug)]
pub struct CanonicalKirCallEffectsV1<'i, 'g> {
    inventory: &'i Inventory<'g>,
    states: Vec<State>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirCallEffectStorageV1(usize);
impl CanonicalKirCallEffectStorageV1 {
    /// Logical report payload, excluding caller-owned graph/inventory and RSS.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

impl<'i, 'g> CanonicalKirCallEffectsV1<'i, 'g> {
    /// O(functions + operations + calls), plus at most two type lookups per
    /// bounded assembly operation, each logarithmic in inventory definitions.
    /// No recursive host calls or expanded callee bodies. Every declaration,
    /// cycle dependency and opaque assembly remains incomplete, even when its
    /// local effect list is empty.
    ///
    /// Caller reserves its live graph/inventory. Work, vector initialization and
    /// requested storage are charged before use. Scratch drops before restoring
    /// the entry floor on Result return; reserve the returned receipt while the
    /// report lives. Allocator overhead and physical OOM recovery are not modeled.
    pub fn derive(
        inventory: &'i Inventory<'g>,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, CanonicalKirCallEffectStorageV1)> {
        let retained = size_of::<Self>()
            .checked_add(payload::<State>(inventory.functions().len())?)
            .ok_or(Resource::Arithmetic)?;
        let floor = budget.storage();
        let result = Self::build(inventory, budget);
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?,
        )?;
        result.map(|report| (report, CanonicalKirCallEffectStorageV1(retained)))
    }

    pub const fn inventory(&self) -> &'i Inventory<'g> {
        self.inventory
    }

    pub fn belongs_to(&self, inventory: &Inventory<'_>) -> bool {
        std::ptr::eq(self.inventory, inventory)
    }

    /// Coordinates are local locators, never transferable evidence.
    pub fn decision(&self, function: Function, budget: &mut Budget<'_>) -> Result<Decision> {
        budget.charge_work(1)?;
        match self.states.get(function.0 as usize) {
            Some(State::Done(decision)) => Ok(*decision),
            Some(_) => Err(Error::InconsistentInventory),
            None => Err(Error::InvalidFunction(function)),
        }
    }

    fn build(inventory: &'i Inventory<'g>, budget: &mut Budget<'_>) -> Result<Self> {
        let count = inventory.functions().len();
        budget.reserve_storage(size_of::<Self>())?;
        let mut states = vector::<State>(count, budget)?;
        states.resize(count, State::Unseen);
        let mut stack = vector::<Frame>(count, budget)?;
        for ordinal in 0..count {
            budget.charge_work(1)?;
            if !matches!(states[ordinal], State::Unseen) {
                continue;
            }
            let function = inventory.functions()[ordinal].coordinate;
            stack.push(frame(inventory, function)?);
            states[ordinal] = State::Active;
            while let Some(current) = stack.last_mut() {
                budget.charge_work(1)?;
                let row = &inventory.functions()[current.function.0 as usize];
                if current.operation == row.operations.end {
                    let completed = stack.pop().ok_or(Error::InconsistentInventory)?;
                    states[completed.function.0 as usize] = State::Done(completed.decision);
                    if let Some(parent) = stack.last_mut() {
                        parent.decision = parent.decision.join(completed.decision);
                    }
                    continue;
                }
                let operation = inventory
                    .operations()
                    .get(current.operation)
                    .ok_or(Error::InconsistentInventory)?;
                current.operation += 1;
                if !operation.effects.is_empty() || !operation.compiler_ordering().is_empty() {
                    current.decision = current.decision.join(Decision::CompleteNonempty);
                }
                if matches!(operation.operation.kind, OperationKind::InlineAssembly(_))
                    && !gfx942_inline_v30::has_closed_effects(
                        inventory,
                        current.function,
                        operation.operation,
                        budget,
                    )?
                {
                    current.decision = Decision::Incomplete;
                }
                if !matches!(operation.operation.kind, OperationKind::Call { .. }) {
                    continue;
                }
                let call = inventory
                    .calls()
                    .get(current.call)
                    .ok_or(Error::InconsistentInventory)?;
                current.call += 1;
                if call.coordinate != operation.coordinate || current.call > row.calls.end {
                    return Err(Error::InconsistentInventory);
                }
                if operation
                    .operation
                    .has_complete_effect_summary_with_budget_v1(budget)?
                {
                    continue;
                }
                let Some(target) = call.target else {
                    current.decision = Decision::Incomplete;
                    continue;
                };
                match states
                    .get(target.0 as usize)
                    .ok_or(Error::InconsistentInventory)?
                {
                    State::Active => current.decision = Decision::Incomplete,
                    State::Done(decision) => current.decision = current.decision.join(*decision),
                    State::Unseen => {
                        if stack.len() >= count {
                            return Err(Error::InconsistentInventory);
                        }
                        stack.push(frame(inventory, target)?);
                        states[target.0 as usize] = State::Active;
                    }
                }
            }
        }
        Ok(Self { inventory, states })
    }
}

fn frame(inventory: &Inventory<'_>, function: Function) -> Result<Frame> {
    let row = inventory
        .functions()
        .get(function.0 as usize)
        .ok_or(Error::InvalidFunction(function))?;
    Ok(Frame {
        function,
        operation: row.operations.start,
        call: row.calls.start,
        decision: if row.function.body.is_some() {
            Decision::CompleteEmpty
        } else {
            Decision::Incomplete
        },
    })
}

fn payload<T>(count: usize) -> Result<usize> {
    count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic.into())
}

fn vector<T>(count: usize, budget: &mut Budget<'_>) -> Result<Vec<T>> {
    budget.charge_work(count)?;
    budget.reserve_storage(payload::<T>(count)?)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    Ok(values)
}

#[cfg(test)]
#[path = "canonical_kir_call_effects_v1_tests.rs"]
mod tests;
