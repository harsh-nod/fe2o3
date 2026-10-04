//! Original-source conservative CFG continuation of prepared nominal effects.
//! Rows are source-indexed and retain unknown branch alternatives. This does
//! not prove subgroup participation, emit a ranked recipe, or authorize access.
use super::super::bf16_nominal_final_candidate_v1::{
    NominalPreparedTensorEffectsV1, with_nominal_prepared_tensor_effects_v1,
};
use super::super::bf16_nominal_source_preparation_v1::{
    NominalRootAssertionSourceV1, with_nominal_root_assertion_preparation_v1,
};
use super::super::*;
use super::*;
use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1;
use fe2o3_lower_mir_kernel::Bf16NominalCallQueryErrorV1 as QueryError;
use std::mem::{size_of, size_of_val};
use std::panic::{AssertUnwindSafe, catch_unwind};

type Result<T> = std::result::Result<T, QueryError>;

/// Private Copy projection derived only while the original lowerer loan lives.
/// Source and canonical argument coordinates are deliberately separate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) struct NominalCheckedViewV1 {
    source_block: u32,
    parameter: u32,
    source_local: u32,
    source_argument: u32,
    success: u32,
    failure: u32,
    required: u64,
}
impl NominalCheckedViewV1 {
    // Component-test datum only; this cannot construct an authenticated source
    // owner, prepared-flow loan, executable or normal-compilation authority.
    #[cfg(test)]
    pub(in crate::production_ranked_projection_v1) fn synthetic(
        source_block: u32,
        parameter: u32,
        source_local: u32,
        source_argument: u32,
        success: u32,
        failure: u32,
        required: u64,
    ) -> Self {
        Self {
            source_block,
            parameter,
            source_local,
            source_argument,
            success,
            failure,
            required,
        }
    }
    pub(in crate::production_ranked_projection_v1) fn source_block(self) -> usize {
        self.source_block as usize
    }
    pub(in crate::production_ranked_projection_v1) fn parameter(self) -> u32 {
        self.parameter
    }
    pub(in crate::production_ranked_projection_v1) fn source_local(self) -> u32 {
        self.source_local
    }
    pub(in crate::production_ranked_projection_v1) fn source_argument(self) -> u32 {
        self.source_argument
    }
    pub(in crate::production_ranked_projection_v1) fn success(self) -> usize {
        self.success as usize
    }
    pub(in crate::production_ranked_projection_v1) fn failure(self) -> usize {
        self.failure as usize
    }
    pub(in crate::production_ranked_projection_v1) fn required(self) -> u64 {
        self.required
    }
}

/// Only the factory below constructs this source-bound lexical loan. The
/// original tensor/effect/Return association stays alive; a copied row or a
/// successful callback does not certify participation or normal admission.
pub(in crate::production_ranked_projection_v1) struct NominalPreparedControlFlowV1<'a, 'g> {
    effects: &'a NominalPreparedTensorEffectsV1<'a, 'g>,
    assertions: &'a NominalRootAssertionSourceV1<'a>,
    terminators: &'a [ProjectedCfgTerminatorV1],
    checked_views: &'a [Option<NominalCheckedViewV1>; 32],
}
impl<'a, 'g> NominalPreparedControlFlowV1<'a, 'g> {
    pub(in crate::production_ranked_projection_v1) fn effects(
        &self,
    ) -> &NominalPreparedTensorEffectsV1<'a, 'g> {
        self.effects
    }
    pub(in crate::production_ranked_projection_v1) fn assertions(
        &self,
    ) -> &NominalRootAssertionSourceV1<'a> {
        self.assertions
    }
    pub(in crate::production_ranked_projection_v1) fn checked_views(
        &self,
    ) -> &[Option<NominalCheckedViewV1>; 32] {
        self.checked_views
    }
    pub(in crate::production_ranked_projection_v1) fn terminators(
        &self,
    ) -> &[ProjectedCfgTerminatorV1] {
        self.terminators
    }
}

fn query_error(error: ProductionRankedProjectionErrorV1) -> QueryError {
    match error {
        ProductionRankedProjectionErrorV1::CanonicalAssertions(
            CanonicalAssertionErrorV1::Resource(e),
        ) => QueryError::Resource(e),
        ProductionRankedProjectionErrorV1::Unsupported(reason)
        | ProductionRankedProjectionErrorV1::Incomplete(reason) => QueryError::Unavailable(reason),
        ProductionRankedProjectionErrorV1::UnprovenAssert { .. } => {
            QueryError::Unavailable("nominal prepared CFG retains an unproved source assertion")
        }
        _ => QueryError::Unavailable("nominal prepared CFG canonical source query refused"),
    }
}
fn require(ok: bool, reason: &'static str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(QueryError::Unavailable(reason))
    }
}
fn plus(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or(Resource::Arithmetic.into())
}
fn times(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b).ok_or(Resource::Arithmetic.into())
}
fn frame<R>(callback: usize) -> Result<usize> {
    let mut bytes = plus(
        8192usize,
        times(size_of::<[Option<NominalCheckedViewV1>; 32]>(), 4)?,
    )?;
    for amount in [
        times(size_of::<Vec<ProjectedCfgTerminatorV1>>(), 4)?,
        times(
            size_of::<NominalPreparedControlFlowV1<'static, 'static>>(),
            4,
        )?,
        times(size_of::<ProjectedCfgTerminatorV1>(), 4)?,
        times(size_of::<Checkpoint>(), 4)?,
        times(size_of::<Result<R>>(), 4)?,
        times(size_of::<Result<()>>(), 4)?,
        times(callback, 4)?,
    ] {
        bytes = plus(bytes, amount)?;
    }
    Ok(bytes)
}
#[derive(Clone, Copy)]
struct Checkpoint {
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    storage: usize,
    work: usize,
    peak: usize,
}
impl Checkpoint {
    fn take(budget: &Budget<'_>) -> Result<Self> {
        if budget.failed_work().is_some() || budget.failed_storage().is_some() {
            return Err(Resource::Accounting.into());
        }
        Ok(Self {
            slot: budget as *const Budget<'_> as usize,
            ledger: budget.work_ledger_identity_v1(),
            storage: budget.storage(),
            work: budget.work(),
            peak: budget.peak_storage(),
        })
    }
    fn require(self, budget: &Budget<'_>, owned: usize) -> Result<()> {
        if self.slot != budget as *const Budget<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < plus(self.storage, owned)?
            || budget.work() < self.work
            || budget.peak_storage() < self.peak
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }
}

fn require_source(
    effects: &NominalPreparedTensorEffectsV1<'_, '_>,
    assertions: &NominalRootAssertionSourceV1<'_>,
    owner: &ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'_>,
    root: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    call: &SemanticDirectCallV1,
) -> Result<()> {
    let original = effects.original();
    let candidate = original.candidate();
    let semantic = owner.semantic_ssa().source_semantic();
    let function = semantic
        .functions()
        .get(root.index() as usize)
        .ok_or(QueryError::Unavailable("nominal prepared CFG root absent"))?;
    require(
        std::ptr::eq(candidate.owner(), owner)
            && std::ptr::eq(candidate.inventory(), inventory)
            && std::ptr::eq(candidate.source_call(), call)
            && inventory.belongs_to(owner.executable())
            && std::ptr::eq(original.function(), function)
            && std::ptr::eq(assertions.cfg().function(), function)
            && std::ptr::eq(assertions.cfg().types(), semantic.types())
            && effects.source_block() == block
            && (1..=32).contains(&function.blocks().len())
            && effects.effects().len() == function.blocks().len()
            && assertions.decisions().len() == function.blocks().len(),
        "nominal prepared CFG original owner/effects/assertion source differs",
    )?;
    let actual = function
        .blocks()
        .get(block.index() as usize)
        .ok_or(QueryError::Unavailable(
            "nominal prepared CFG call block absent",
        ))?;
    require(
        matches!(actual.terminator().kind(),
        SemanticTerminatorKindV1::Call(actual) if std::ptr::eq(actual, call)),
        "nominal prepared CFG exact original Call differs",
    )
}

fn populate(
    effects: &NominalPreparedTensorEffectsV1<'_, '_>,
    assertions: &NominalRootAssertionSourceV1<'_>,
    facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
    owned: &mut usize,
    rows: &mut Vec<ProjectedCfgTerminatorV1>,
    checked_views: &mut [Option<NominalCheckedViewV1>; 32],
) -> Result<()> {
    facts.charge_private_array_work(128).map_err(query_error)?;
    let candidate = effects.original().candidate();
    let owner = candidate.owner();
    let function = effects.original().function();
    require(
        facts.masked.is_none()
            && std::ptr::eq(facts.owner, owner)
            && std::ptr::eq(facts.report.inventory(), candidate.inventory())
            && facts.correspondence_owner == facts.semantic_function
            && owner
                .semantic_ssa()
                .source_semantic()
                .functions()
                .get(facts.semantic_function.index() as usize)
                .is_some_and(|actual| std::ptr::eq(actual, function))
            && rows.is_empty()
            && rows.capacity() == 0
            && checked_views.iter().all(Option::is_none),
        "nominal prepared CFG actual canonical facts or empty output differs",
    )?;
    let count = function.blocks().len();
    facts
        .charge_private_array_work(count)
        .map_err(query_error)?;
    root_cfg_terminator_resources_v1::reserve_owned(
        facts,
        owned,
        times(count, size_of::<ProjectedCfgTerminatorV1>())?,
    )
    .map_err(query_error)?;
    rows.try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    require(
        rows.capacity() == count,
        "nominal prepared CFG row capacity differs",
    )?;
    let entry = function.entry().index() as usize;
    require(
        entry < count,
        "nominal prepared CFG entry outside original source",
    )?;
    require(
        facts.is_materialized_block(entry).map_err(query_error)?,
        "nominal prepared CFG original entry is not materialized",
    )?;
    for index in 0..count {
        facts.charge_private_array_work(16).map_err(query_error)?;
        let materialized = facts.is_materialized_block(index).map_err(query_error)?;
        if !materialized {
            require(
                effects.effects()[index] == ProjectedCapabilityTerminatorEffectsV1::default(),
                "nominal prepared CFG absent block retains a source effect",
            )?;
            require(
                index != effects.source_block().index() as usize,
                "nominal prepared CFG actual tensor Call is not materialized",
            )?;
            rows.push(ProjectedCfgTerminatorV1::AbsentMaterialized);
            continue;
        }
        // Narrow same-owner query, separate from the ordinary predicate tables.
        // Neither an unsupported view switch nor a denial becomes uniform.
        let checked = if matches!(
            function.blocks()[index].terminator().kind(),
            SemanticTerminatorKindV1::SwitchInt { .. }
        ) {
            owner.with_checked_bf16_view_switch_v1(
                candidate.inventory(),
                facts.report,
                facts.semantic_function,
                effects.source_block(),
                candidate.source_call(),
                SemanticBlockIdV1::from_index(index as u32),
                facts.budget,
                |view, budget| {
                    budget.charge_work(32)?;
                    require(
                        std::ptr::eq(view.source(), function)
                            && view.source_block().index() as usize == index,
                        "nominal checked-view source loan differs",
                    )?;
                    Ok(NominalCheckedViewV1 {
                        source_block: view.source_block().index(),
                        parameter: view.parameter(),
                        source_local: view.source_local().index(),
                        source_argument: view.source_argument(),
                        success: view.success().index(),
                        failure: view.failure().index(),
                        required: view.required(),
                    })
                },
            )?
        } else {
            None
        };
        checked_views[index] = checked;
        rows.push(
            root_cfg_terminator_resources_v1::project(
                function,
                index,
                owner.semantic_ssa().source_semantic().callables(),
                assertions.decisions()[index],
                facts,
                &[],
                &[],
                Some(&mut *owned),
            )
            .map_err(query_error)?,
        );
    }
    // This is not a graph-reachability or convergence assertion. Source block
    // coordinates cannot be relabeled as ranked block/operation coordinates.
    require(
        rows.len() == count,
        "nominal prepared CFG row census differs",
    )
}

#[allow(clippy::too_many_arguments)]
fn with_scope<'g, 'w, R, F>(
    effects: &NominalPreparedTensorEffectsV1<'_, 'g>,
    assertions: &NominalRootAssertionSourceV1<'_>,
    owner: &'g ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'g>,
    root: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    call: &SemanticDirectCallV1,
    budget: &mut Budget<'w>,
    inspect: F,
) -> Result<R>
where
    R: Copy + 'static,
    F: for<'a> FnOnce(&NominalPreparedControlFlowV1<'a, 'g>, &mut Budget<'w>) -> Result<R>,
{
    let before = Checkpoint::take(budget)?;
    let mut owned = 0usize;
    // The real physical owner is outside the facts callback, so all partial
    // row backing survives its postflight and drops before our own-only refund.
    let mut rows = Vec::new();
    let mut checked_views = [None; 32];
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        budget.charge_work(128)?;
        let bytes = frame::<R>(size_of_val(&inspect))?;
        budget.reserve_storage(bytes)?;
        owned = bytes;
        require_source(effects, assertions, owner, inventory, root, block, call)?;
        with_nominal_canonical_facts_observation_v1(
            owner,
            inventory,
            root,
            root,
            block,
            call,
            budget,
            |facts| {
                populate(
                    effects,
                    assertions,
                    facts,
                    &mut owned,
                    &mut rows,
                    &mut checked_views,
                )
            },
        )?;
        // The canonical facts and N1 postflights completed before this loan.
        // Enclosing original assertion/effects scopes still retain their own
        // required postflights; no result can bypass them or escape their loan.
        before.require(budget, owned)?;
        if budget.failed_work().is_some() || budget.failed_storage().is_some() {
            return Err(Resource::Accounting.into());
        }
        let view = NominalPreparedControlFlowV1 {
            effects,
            assertions,
            terminators: &rows,
            checked_views: &checked_views,
        };
        let result = inspect(&view, budget);
        drop(view);
        result
    }));
    let result = match outcome {
        Ok(Ok(_)) if budget.failed_work().is_some() || budget.failed_storage().is_some() => {
            Err(Resource::Accounting.into())
        }
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(QueryError::CallbackPanicked)
        }
    };
    drop(rows);
    before.require(budget, owned)?;
    budget.release_storage(owned)?;
    result
}

/// Genuine original-source continuation; no caller-authored predicate mask,
/// detached effects table, replacement ledger, or test-only facts constructor.
/// Full-wave/guarded-memory/recipe verification and normal admission remain
/// required. Unknown switches are deliberately not labeled lane-uniform.
#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn with_nominal_prepared_control_flow_v1<
    'g,
    'w,
    R,
    F,
>(
    owner: &'g ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'g>,
    root: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    call: &SemanticDirectCallV1,
    budget: &mut Budget<'w>,
    inspect: F,
) -> Result<R>
where
    R: Copy + 'static,
    F: for<'a> FnOnce(&NominalPreparedControlFlowV1<'a, 'g>, &mut Budget<'w>) -> Result<R>,
{
    with_nominal_prepared_tensor_effects_v1(
        owner,
        inventory,
        root,
        block,
        call,
        budget,
        move |effects, budget| {
            with_nominal_root_assertion_preparation_v1(
                owner,
                inventory,
                root,
                root,
                block,
                call,
                budget,
                move |assertions, budget| {
                    with_scope(
                        effects, assertions, owner, inventory, root, block, call, budget, inspect,
                    )
                },
            )
        },
    )
}

#[cfg(test)]
#[path = "bf16_nominal_prepared_control_flow_v1_tests.rs"]
mod tests;
