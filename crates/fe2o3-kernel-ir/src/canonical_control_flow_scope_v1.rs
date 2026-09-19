//! Owner-bound scoped reuse of the existing live-metered CFG implementation.

use super::{
    ControlFlowError, ControlFlowLimits, MeteredControlFlowErrorV1, MeteredIndexedControlFlowV1,
    analyze_control_flow_with_verification_budget_v1,
};
use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1, CanonicalKirBlockCoordinateV1 as Block,
    CanonicalKirFunctionCoordinateV1 as Function, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};

/// Errors preserve the existing CFG and resource domains without a boolean fallback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalKirControlFlowScopeErrorV1 {
    Resource(Resource),
    ControlFlow(ControlFlowError),
    InvalidFunction(Function),
    InvalidBlock(Block),
    Panicked,
}
use CanonicalKirControlFlowScopeErrorV1 as Error;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "canonical CFG scope: {self:?}")
    }
}
impl std::error::Error for Error {}

struct Accounting {
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    retained: usize,
    failure: Option<Error>,
    cleanup: bool,
}
impl Accounting {
    fn check(&mut self, budget: &Budget<'_>) -> Result<(), Error> {
        let expected = self.floor.checked_add(self.retained);
        if self.slot != budget as *const Budget<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || expected.is_none_or(|expected| budget.storage() < expected)
        {
            self.cleanup = false;
            self.failure = Some(Resource::Accounting.into());
        } else if expected != Some(budget.storage()) {
            self.failure = Some(Resource::Accounting.into());
        }
        self.failure.clone().map_or(Ok(()), Err)
    }
    fn fail(&mut self, error: Error) -> Error {
        self.failure.get_or_insert(error).clone()
    }
    fn charge(&mut self, budget: &mut Budget<'_>, work: usize) -> Result<(), Error> {
        self.check(budget)?;
        budget
            .charge_work(work)
            .map_err(|error| self.fail(error.into()))
    }
}

/// Read-only CFG queries for exactly one function of an actual verified owner.
/// No CFG, allocation-owning facade, or fabricated owner/graph pair can escape.
/// Coordinates are stored ordinals, not raw block IDs. Queries debit before use.
pub struct CanonicalKirControlFlowViewV1<'scope, 'graph> {
    owner: &'graph Owner,
    function: Function,
    flow: &'scope MeteredIndexedControlFlowV1,
    accounting: &'scope mut Accounting,
}
impl<'graph> CanonicalKirControlFlowViewV1<'_, 'graph> {
    pub fn owner(&self) -> &'graph Owner {
        self.owner
    }
    pub fn function(&self) -> Function {
        self.function
    }

    fn block(&mut self, block: Block, budget: &mut Budget<'_>) -> Result<usize, Error> {
        self.accounting.charge(budget, 3)?;
        let position = block.block as usize;
        if block.function != self.function || position >= self.flow.flow.block_ids.len() {
            return Err(self.accounting.fail(Error::InvalidBlock(block)));
        }
        Ok(position)
    }
    pub fn is_reachable(&mut self, block: Block, budget: &mut Budget<'_>) -> Result<bool, Error> {
        let position = self.block(block, budget)?;
        self.accounting.charge(budget, 1)?;
        Ok(self.flow.flow.reachable[position])
    }
    /// Disconnected self-dominance is not returned as entry-reachable dominance.
    pub fn dominates(
        &mut self,
        definition: Block,
        use_block: Block,
        budget: &mut Budget<'_>,
    ) -> Result<bool, Error> {
        let definition = self.block(definition, budget)?;
        let use_block = self.block(use_block, budget)?;
        self.accounting.charge(budget, 5)?;
        Ok(self.flow.flow.reachable[use_block]
            && self.flow.flow.dominates_positions(definition, use_block))
    }
    pub fn is_reducible(&mut self, budget: &mut Budget<'_>) -> Result<bool, Error> {
        self.accounting.charge(budget, 1)?;
        Ok(self.flow.flow.is_reducible())
    }
}

/// Reuses the existing CFG builder, including its **logical requested row-cell**
/// storage contract. This inherited amount is not byte, capacity, or RSS accounting.
/// No new heap table is allocated by the facade. Work/peak/failure history persists.
///
/// Output buffers must be prepaid outside the scope. Callback scratch must be
/// released before a query and on return. An extra exit reservation rejects the
/// result, but only the CFG's exact accepted reservation is released; unrelated
/// excess remains caller-owned. A foreign slot/ledger or undercut reservation
/// disables cleanup, even if a later callback operation restores it.
///
/// Panics are caught, owned CFG/result objects drop before cleanup, and panic
/// payload destruction and caller-defined error conversion are deferred until
/// afterward. Original and rejected-result panic payloads have separate fixed
/// slots; neither replaces and prematurely destroys the other. No fact grants mutation,
/// progress, initialization, source-custody or optimizer authority.
///
/// Returning the original graph borrow is permitted; returning the facade is not:
/// ```
/// use fe2o3_kernel_ir::{with_canonical_kir_control_flow_v1, CanonicalKirFunctionCoordinateV1, CanonicalKirControlFlowScopeErrorV1, CanonicalKernelIrVerificationResourceBudgetV1, VerifiedCanonicalKernelIrModuleV12};
/// fn owner_can_escape<'g>(owner: &'g VerifiedCanonicalKernelIrModuleV12, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) -> Result<&'g VerifiedCanonicalKernelIrModuleV12, CanonicalKirControlFlowScopeErrorV1> {
///     with_canonical_kir_control_flow_v1(owner, CanonicalKirFunctionCoordinateV1(0), Default::default(), budget, |view, _| Ok(view.owner()))
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_ir::{with_canonical_kir_control_flow_v1, CanonicalKirFunctionCoordinateV1, CanonicalKirControlFlowScopeErrorV1, CanonicalKernelIrVerificationResourceBudgetV1, VerifiedCanonicalKernelIrModuleV12};
/// fn cannot_escape(owner: &VerifiedCanonicalKernelIrModuleV12, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _escaped = with_canonical_kir_control_flow_v1(owner, CanonicalKirFunctionCoordinateV1(0), Default::default(), budget,
///         |view, _| Ok::<_, CanonicalKirControlFlowScopeErrorV1>(view));
/// }
/// ```
pub fn with_canonical_kir_control_flow_v1<'graph, 'work, T, E>(
    owner: &'graph Owner,
    function: Function,
    limits: ControlFlowLimits,
    budget: &mut Budget<'work>,
    run: impl for<'scope> FnOnce(
        &mut CanonicalKirControlFlowViewV1<'scope, 'graph>,
        &mut Budget<'work>,
    ) -> Result<T, E>,
) -> Result<T, E>
where
    E: From<Error>,
{
    let mut accounting = Accounting {
        slot: budget as *const Budget<'_> as usize,
        ledger: budget.work_ledger_identity_v1(),
        floor: budget.storage(),
        retained: 0,
        failure: None,
        cleanup: true,
    };
    let mut flow = None;
    let mut deferred_panics = [None, None];
    let mut constructing = true;
    let protected = catch_unwind(AssertUnwindSafe(|| -> Result<Result<T, E>, Error> {
        accounting.charge(budget, 3)?;
        let actual = owner
            .module()
            .functions
            .get(function.0 as usize)
            .filter(|f| f.body.is_some())
            .ok_or(Error::InvalidFunction(function))?;
        flow = Some(
            analyze_control_flow_with_verification_budget_v1(actual, limits, budget).map_err(
                |error| match error {
                    MeteredControlFlowErrorV1::Resource(error) => Error::Resource(error),
                    MeteredControlFlowErrorV1::ControlFlow(error) => Error::ControlFlow(error),
                },
            )?,
        );
        accounting.retained = flow.as_ref().expect("constructed CFG").retained_storage;
        constructing = false;
        let mut view = CanonicalKirControlFlowViewV1 {
            owner,
            function,
            flow: flow.as_ref().expect("constructed CFG"),
            accounting: &mut accounting,
        };
        Ok(run(&mut view, budget))
    }));
    let mut returned = None;
    let mut failure = None;
    match protected {
        Ok(Ok(result)) => returned = Some(result),
        Ok(Err(error)) => failure = Some(error),
        Err(payload) => {
            deferred_panics[0] = Some(payload);
            failure = Some(Error::Panicked);
        }
    }
    // Only construction can reserve without the scoped accounting object. No
    // external callback ran in that phase, so the accepted delta is owned here.
    if constructing {
        accounting.retained = budget.storage().saturating_sub(accounting.floor);
    }
    if let Err(error) = accounting.check(budget) {
        if let Some(result) = returned.take()
            && let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(result)))
        {
            deferred_panics[1] = Some(payload);
        }
        failure = Some(error);
    }
    drop(flow);
    if accounting.cleanup
        && let Err(error) = budget.release_storage(accounting.retained)
    {
        // At most one rejected returned value exists. If the earlier guard
        // already dropped it, take() is None and cannot replace a payload.
        if let Some(result) = returned.take()
            && let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(result)))
        {
            deferred_panics[1] = Some(payload);
        }
        failure = Some(Error::Resource(error));
    }
    drop(deferred_panics);
    match failure {
        Some(error) => Err(E::from(error)),
        None => returned.expect("successful CFG scope retains its callback result"),
    }
}

#[cfg(test)]
#[path = "canonical_control_flow_scope_v1_tests.rs"]
mod tests;
