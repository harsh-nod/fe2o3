//! Reservation-only recipe context borrowing one real canonical-facts owner.
//! There is no owning recipe or refund here. The caller's physical owner keeps
//! every accepted credit until all associated values are dropped, including
//! values parked in outer captures by resource callbacks. No raw Budget escapes.
use super::super::{
    ProductionRankedProjectionErrorV1 as Error, SemanticFunctionDeclV1,
    bf16_nominal_preparation_resources_v1::{PreparationResourcesV1, resource},
    bf16_nominal_source_preparation_v1::RichNominalSourceTablesV1,
    root_checked_references_v1::require_same_source_v1,
};
use super::{CanonicalSourceAssertionFactsV1, ProjectedAssertionFactsV1};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1,
};
use std::mem::size_of;

type Result<T> = std::result::Result<T, Error>;

#[path = "bf16_nominal_initial_graph_v1.rs"]
mod initial_graph_v1;
#[path = "bf16_nominal_root_prefix_indices_v1.rs"]
mod root_prefix_indices_v1;
#[allow(unused_imports)]
pub(in crate::production_ranked_projection_v1) use initial_graph_v1::{
    NominalCompleteForProfileGraphV1, NominalInitialGraphV1, PendingNominalInitialGraphV1,
};
#[allow(unused_imports)]
pub(in crate::production_ranked_projection_v1) use root_prefix_indices_v1::{
    ActualRootGuardedAccessesV1, ActualRootPrefixIndicesV1, PendingActualRootPrefixIndicesV1,
};

#[derive(Clone, Copy)]
struct ReservationState {
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    storage: usize,
    owned: usize,
    work: usize,
    peak: usize,
}
impl ReservationState {
    fn new(budget: &Budget<'_>, owned: usize) -> Result<Self> {
        if owned > budget.storage()
            || budget.failed_work().is_some()
            || budget.failed_storage().is_some()
        {
            return Err(resource(Resource::Accounting));
        }
        Ok(Self {
            slot: budget as *const Budget<'_> as usize,
            ledger: budget.work_ledger_identity_v1(),
            storage: budget.storage(),
            owned,
            work: budget.work(),
            peak: budget.peak_storage(),
        })
    }

    fn check(self, budget: &Budget<'_>, owned: usize) -> Result<()> {
        let growth = owned
            .checked_sub(self.owned)
            .ok_or_else(|| resource(Resource::Accounting))?;
        let floor = self
            .storage
            .checked_add(growth)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        if self.slot != budget as *const Budget<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < floor
            || budget.work() < self.work
            || budget.peak_storage() < self.peak
            || budget.failed_work().is_some()
            || budget.failed_storage().is_some()
        {
            return Err(resource(Resource::Accounting));
        }
        Ok(())
    }
}

fn with_resource_borrow<R>(
    budget: &mut Budget<'_>,
    owned: &mut usize,
    state: ReservationState,
    inspect: impl FnOnce(&mut PreparationResourcesV1<'_, '_>) -> Result<R>,
) -> Result<R> {
    state.check(budget, *owned)?;
    let result = {
        let mut resources = PreparationResourcesV1::new(budget, owned);
        inspect(&mut resources)
    };
    // A denied operation cannot be hidden by a successful callback result.
    state.check(budget, *owned)?;
    result
}

fn frame<R, F>() -> Result<usize> {
    let mut bytes = 4096usize;
    for amount in [
        size_of::<ReservationState>(),
        size_of::<NominalRecipeResourcesV1<'static, 'static, 'static, 'static, 'static, 'static>>(),
        size_of::<PreparationResourcesV1<'static, 'static>>(),
        size_of::<F>()
            .checked_mul(2)
            .ok_or_else(|| resource(Resource::Arithmetic))?,
        size_of::<Result<R>>()
            .checked_mul(2)
            .ok_or_else(|| resource(Resource::Arithmetic))?,
    ] {
        bytes = bytes
            .checked_add(amount)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
    }
    Ok(bytes)
}

/// A source-bound *borrower*, not a storage owner, readiness token, or strict
/// checked-origin constructor. A closed-profile graph loan is available, but
/// actual guarded access construction and its joined origins remain absent.
/// The only mutable Budget borrow is already inside these real canonical facts.
pub(in crate::production_ranked_projection_v1) struct NominalRecipeResourcesV1<
    'f,
    'r,
    'i,
    'g,
    'b,
    'w,
> {
    facts: &'f mut CanonicalSourceAssertionFactsV1<'r, 'i, 'g, 'b, 'w>,
    owned: &'f mut usize,
    state: ReservationState,
    function: &'g SemanticFunctionDeclV1,
}

impl NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_> {
    pub(in crate::production_ranked_projection_v1) fn function(&self) -> &SemanticFunctionDeclV1 {
        self.function
    }

    /// Each accepted reservation is recorded in the OUTER owner's counter.
    /// Results and captured values may outlive this borrow; nothing is refunded.
    pub(in crate::production_ranked_projection_v1) fn with_resources<R>(
        &mut self,
        inspect: impl FnOnce(&mut PreparationResourcesV1<'_, '_>) -> Result<R>,
    ) -> Result<R> {
        with_resource_borrow(self.facts.budget, self.owned, self.state, inspect)
    }

    /// Resource and fact borrows cannot overlap: both require exclusive self.
    /// This forwards to the same real facts, including their existing refusals.
    pub(in crate::production_ranked_projection_v1) fn with_facts<R>(
        &mut self,
        inspect: impl FnOnce(&mut dyn ProjectedAssertionFactsV1) -> Result<R>,
    ) -> Result<R> {
        self.state.check(self.facts.budget, *self.owned)?;
        let result = inspect(self.facts);
        self.state.check(self.facts.budget, *self.owned)?;
        result
    }
}

/// The caller supplies a counter owned by its physical pending-recipe owner.
/// Accepted credits (including this generic frame) remain there on success,
/// error, and unwind. This function NEVER refunds or exposes a replacement
/// ledger. The outer owner must catch unwind, drop its payloads, perform its own
/// postflight, then refund only its accepted credits. A Copy result would not
/// prevent an allocation from escaping through a captured slot, so it is not
/// used as a false ownership guarantee here.
pub(in crate::production_ranked_projection_v1) fn with_nominal_recipe_resources_v1<'w, R, F>(
    facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, 'w>,
    rich: &RichNominalSourceTablesV1<'_>,
    owned: &mut usize,
    inspect: F,
) -> Result<R>
where
    F: FnOnce(&mut NominalRecipeResourcesV1<'_, '_, '_, '_, '_, 'w>) -> Result<R>,
{
    if facts.masked.is_some() {
        return Err(Error::Incomplete(
            "nominal recipe resources require unmasked real facts",
        ));
    }
    let function = facts
        .owner
        .semantic_ssa()
        .source_semantic()
        .functions()
        .get(facts.semantic_function.index() as usize)
        .ok_or(Error::Incomplete("nominal recipe source function absent"))?;
    require_same_source_v1(function, rich.function())?;
    let state = ReservationState::new(facts.budget, *owned)?;
    let bytes = frame::<R, F>()?;
    with_resource_borrow(facts.budget, owned, state, |resources| {
        resources.work(32)?;
        resources.reserve_storage(bytes)
    })?;
    let mut context = NominalRecipeResourcesV1 {
        facts,
        owned,
        state,
        function,
    };
    // Real canonical correspondence must recognize the actual source entry;
    // equal content or a caller-provided source index is insufficient.
    let entry = function.entry().index() as usize;
    if !context.with_facts(|facts| facts.is_materialized_block(entry))? {
        return Err(Error::Incomplete(
            "nominal recipe source entry is not materialized",
        ));
    }
    let result = inspect(&mut context);
    context.state.check(context.facts.budget, *context.owned)?;
    result
}

#[cfg(test)]
#[path = "bf16_nominal_recipe_resources_v1_tests.rs"]
mod tests;

#[cfg(test)]
pub(in crate::production_ranked_projection_v1) use initial_graph_v1::{
    nominal_initial_graph_controls_for_test_v1, observe_nominal_initial_graph_for_test_v1,
};

#[cfg(test)]
pub(crate) use root_prefix_indices_v1::observe_actual_root_argument_initialization_for_test_v1;
#[cfg(test)]
pub(crate) use root_prefix_indices_v1::observe_actual_root_guarded_accesses_for_test_v1;
#[cfg(test)]
pub(crate) use root_prefix_indices_v1::observe_actual_root_prefix_indices_for_test_v1;
#[cfg(test)]
pub(crate) use root_prefix_indices_v1::observe_actual_root_retired_fixed_proof_for_test_v1;

#[cfg(test)]
pub(crate) use root_prefix_indices_v1::observe_actual_root_fixed_prefix_comparison_for_test_v1;

pub(in crate::production_ranked_projection_v1) fn with_nominal_source_preparation_v1<'g, 'w, R, F>(
    facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, 'g, '_, 'w>,
    expected_function: &SemanticFunctionDeclV1,
    expected_ledger: (usize, CanonicalKernelIrWorkLedgerIdentityV1),
    owned: &mut usize,
    inspect: F,
) -> Result<R>
where
    F: FnOnce(&mut PreparationResourcesV1<'_, '_>) -> Result<R>,
{
    // Sequential narrow borrower only. It neither constructs Rich tables nor
    // releases callback storage. The outer pending outlives this whole loan.
    if facts.masked.is_some() {
        return Err(Error::Incomplete(
            "source-ordered preparation requires unmasked real facts",
        ));
    }
    let function = facts
        .owner
        .semantic_ssa()
        .source_semantic()
        .functions()
        .get(facts.semantic_function.index() as usize)
        .ok_or(Error::Incomplete(
            "source-ordered preparation function absent",
        ))?;
    require_same_source_v1(function, expected_function)?;
    let state = ReservationState::new(facts.budget, *owned)?;
    if (state.slot, state.ledger) != expected_ledger {
        return Err(resource(Resource::Accounting));
    }
    let bytes = source_preparation_frame::<R, F>()?;
    with_resource_borrow(facts.budget, owned, state, |resources| {
        resources.work(32)?;
        resources.reserve_storage(bytes)
    })?;
    let materialized = facts.is_materialized_block(function.entry().index() as usize);
    state.check(facts.budget, *owned)?;
    if !materialized? {
        return Err(Error::Incomplete(
            "source-ordered preparation entry is not materialized",
        ));
    }
    with_resource_borrow(facts.budget, owned, state, inspect)
}
fn source_preparation_frame<R, F>() -> Result<usize> {
    let rows = [
        size_of::<(
            &mut CanonicalSourceAssertionFactsV1<'static, 'static, 'static, 'static, 'static>,
            &SemanticFunctionDeclV1,
            (usize, CanonicalKernelIrWorkLedgerIdentityV1),
            &mut usize,
            F,
        )>(),
        size_of::<(
            ReservationState,
            &SemanticFunctionDeclV1,
            Option<&SemanticFunctionDeclV1>,
            &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
            &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
        )>(),
        size_of::<(
            usize,
            Result<usize>,
            Result<bool>,
            bool,
            fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1,
            u32,
            usize,
        )>(),
        size_of::<(
            &mut PreparationResourcesV1<'static, 'static>,
            usize,
            PreparationResourcesV1<'static, 'static>,
            &mut Budget<'static>,
            &mut usize,
            ReservationState,
            Result<()>,
            Result<R>,
            R,
            F,
        )>(),
        size_of::<(usize, usize, Option<usize>, Resource, Error, Result<usize>)>(),
        size_of::<(
            [usize; 6],
            std::array::IntoIter<usize, 6>,
            usize,
            usize,
            Option<usize>,
        )>(),
    ];
    rows.into_iter().try_fold(0usize, |sum, row| {
        sum.checked_add(row)
            .ok_or_else(|| resource(Resource::Arithmetic))
    })
}

#[allow(unused_imports)]
pub(in crate::production_ranked_projection_v1) use root_prefix_indices_v1::{
    ActualSelectedInputsV1, select_actual_capability_prefix_inputs_v1,
};
