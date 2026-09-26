//! Original source-instance readers for the same-owner pending ranked stage.

use super::*;
use canonical_assertion_facts_v1::{
    MaskedSourceAssertionTableV1, ProjectedAssertionConditionV1, checked_slice_projection_v18,
    condition_from_checked_binding_v18,
};
use fe2o3_kernel_analysis::{
    CanonicalKirCallEffectDecisionV1, CanonicalKirCallEffectsV18, CanonicalKirSparseV18,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::{
    ProductionSliceAccessSiteV1, ProductionSourceCorrespondenceV18,
    ProductionSourceOwnedViewErrorV18,
};

pub(super) fn source_error(
    error: ProductionSourceOwnedViewErrorV18,
) -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::CanonicalAssertions(CanonicalAssertionErrorV1::SourceOwned(
        error,
    ))
}

pub(super) fn retained_resource(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    error: Resource,
) -> ProductionRankedProjectionErrorV1 {
    source_error(relation.retain_query_resource_error_v18(error))
}

pub(super) fn retain_resource_error(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    error: ProductionRankedProjectionErrorV1,
) -> ProductionRankedProjectionErrorV1 {
    use CanonicalAssertionErrorV1 as Assertion;
    use ProductionRankedProjectionErrorV1::CanonicalAssertions;
    match error {
        CanonicalAssertions(Assertion::Resource(error))
        | CanonicalAssertions(Assertion::SourceOwned(
            ProductionSourceOwnedViewErrorV18::Resource(error),
        ))
        | CanonicalAssertions(Assertion::Sparse(
            fe2o3_kernel_analysis::CanonicalKirSparseErrorV1::Resource(error),
        ))
        | CanonicalAssertions(Assertion::MaskedAssertion(
            fe2o3_lower_mir_kernel::ProductionSemanticMaskedShiftQueryErrorV1::Resource(error),
        )) => retained_resource(relation, error),
        other => other,
    }
}

fn binding(detail: &'static str) -> ProductionRankedProjectionErrorV1 {
    source_error(ProductionSourceOwnedViewErrorV18::Binding(detail))
}

// These retained locators are consumed by the full correspondence census, not
// exported as a bounds-neutral, initializedness or executable certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SourceCallObligationV18 {
    pub(super) root: usize,
    pub(super) caller_instance: usize,
    pub(super) caller_function: SemanticFunctionIdV1,
    pub(super) source_block: SemanticBlockIdV1,
    pub(super) callee_instance: usize,
    pub(super) callee_function: SemanticFunctionIdV1,
}

pub(super) struct SourceAssertionFactsV18<'r, 'i, 'g, 'b, 'w> {
    pub(super) relation: &'r ProductionSourceCorrespondenceV18<'g>,
    pub(super) sparse: &'r CanonicalKirSparseV18<'i, 'g>,
    pub(super) effects: &'r CanonicalKirCallEffectsV18<'i, 'g>,
    pub(super) root: usize,
    pub(super) instance: usize,
    pub(super) original_root: SemanticFunctionIdV1,
    pub(super) semantic_function: SemanticFunctionIdV1,
    pub(super) pending: &'b mut [Option<SourceCallObligationV18>],
    pub(super) budget: &'b mut Budget<'w>,
    pub(super) masked: &'r MaskedSourceAssertionTableV1<'r, 'r>,
    pub(super) cleanup: &'r fe2o3_pliron::CanonicalAnalysisCleanupV1<'r>,
    pub(super) slot: usize,
    pub(super) ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    pub(super) floor: usize,
}

// This lexical locator retains the actual source relation, not merely a callee
// function number. Repeated invocations of one function are distinct inputs.
pub(super) struct SourceInvocationV18<'a, 'g> {
    relation: &'a ProductionSourceCorrespondenceV18<'g>,
    pub(super) root: usize,
    pub(super) instance: usize,
    pub(super) original_root: SemanticFunctionIdV1,
    pub(super) function: SemanticFunctionIdV1,
    pub(super) caller: Option<(usize, SemanticBlockIdV1)>,
    pub(super) active: bool,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
}

impl<'a, 'g> SourceInvocationV18<'a, 'g> {
    pub(super) fn new(
        relation: &'a ProductionSourceCorrespondenceV18<'g>,
        root: usize,
        instance: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        let source = relation.source(budget).map_err(source_error)?;
        let original_root = source.root(root, budget).map_err(source_error)?.0;
        let (function, caller) = source.instance(root, instance, budget).map_err(source_error)?;
        let active = source.instance_active(root, instance, budget).map_err(source_error)?;
        Ok(Self { relation, root, instance, original_root, function, caller, active,
            slot: budget as *const Budget<'_> as usize,
            ledger: budget.work_ledger_identity_v1(), floor: budget.storage() })
    }

    pub(super) fn check(
        &self,
        relation: &ProductionSourceCorrespondenceV18<'_>,
        root: usize,
        budget: &mut Budget<'_>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        relation.check_query_v18(budget).map_err(source_error)?;
        budget.charge_work(6).map_err(|error| retained_resource(relation, error))?;
        if self.slot != budget as *const Budget<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            return Err(retained_resource(relation, Resource::Accounting));
        }
        if !std::ptr::eq(self.relation, relation) || self.root != root {
            return Err(binding("invocation projection substituted its source relation or root"));
        }
        let source = relation.source(budget).map_err(source_error)?;
        if source.root(root, budget).map_err(source_error)?.0 != self.original_root
            || source.instance(root, self.instance, budget).map_err(source_error)?
                != (self.function, self.caller)
            || source.instance_active(root, self.instance, budget).map_err(source_error)? != self.active
        {
            return Err(binding("invocation projection substituted its exact caller or function"));
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) fn refuse_invocation_substitution_for_test_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    budget: &mut Budget<'_>,
    mode: u8,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let source = relation.source(budget).map_err(source_error)?;
    let mut active = None;
    for instance in 0..source.instance_count(0, budget).map_err(source_error)? {
        if source.instance_active(0, instance, budget).map_err(source_error)? {
            active = Some(instance);
            break;
        }
    }
    let mut key = SourceInvocationV18::new(relation, 0, active.expect("fixture has no active invocation"), budget)?;
    key.check(relation, 0, budget)?;
    match mode {
        0 => key.instance = usize::MAX,
        1 => key.caller = Some((usize::MAX, SemanticBlockIdV1::from_index(u32::MAX))),
        2 => key.function = SemanticFunctionIdV1::from_index(u32::MAX),
        3 => key.slot ^= 1,
        4 => {
            let mut other_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1);
            let other = Budget::new(&mut other_work, 1);
            key.ledger = other.work_ledger_identity_v1();
        }
        5 => {
            let inventory = relation.inventory(budget).map_err(source_error)?;
            // Even an independently constructed relation over the same source
            // bytes is not the exact lexical relation used to make this key.
            return source.with_ranked_correspondence_v18(inventory, budget,
                |other, budget| key.check(other, 0, budget));
        }
        6 => key.root = usize::MAX,
        _ => panic!("unknown invocation hostile case"),
    }
    key.check(relation, 0, budget)
}

#[cfg(test)]
pub(crate) fn refuse_foreign_invocation_owner_for_test_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    original_budget: &mut Budget<'_>,
    foreign: &ProductionSourceCorrespondenceV18<'_>,
    foreign_budget: &mut Budget<'_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let left = original.source(original_budget).map_err(source_error)?;
    let right = foreign.source(foreign_budget).map_err(source_error)?;
    assert!(!std::ptr::eq(left, right));
    assert_eq!(left.source_semantic(original_budget).map_err(source_error)?.semantic_sha256(),
        right.source_semantic(foreign_budget).map_err(source_error)?.semantic_sha256());
    let mut active = None;
    for instance in 0..left.instance_count(0, original_budget).map_err(source_error)? {
        if left.instance_active(0, instance, original_budget).map_err(source_error)? {
            active = Some(instance);
            break;
        }
    }
    let mut key = SourceInvocationV18::new(original, 0, active.expect("no original invocation"), original_budget)?;
    key.check(original, 0, original_budget)?;
    // Deliberately align the ledger metadata so that this hostile control
    // reaches the independent exact-source-relation check, not just custody.
    key.slot = foreign_budget as *const Budget<'_> as usize;
    key.ledger = foreign_budget.work_ledger_identity_v1();
    key.floor = foreign_budget.storage();
    let result = key.check(foreign, 0, foreign_budget);
    assert!(matches!(&result, Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
        CanonicalAssertionErrorV1::SourceOwned(ProductionSourceOwnedViewErrorV18::Binding(
            "invocation projection substituted its source relation or root"))))));
    result
}

pub(super) enum SourceProjectionScopeV18<'a, 'g> {
    SelectedBody(SemanticKernelBodySelectionV1),
    Invocation(&'a SourceInvocationV18<'a, 'g>),
}

// The containing source consumer reserves this header and the pending-call
// vector before entering the shared masked-table scope. Its postflight therefore
// observes their full floor before any scratch refund, including raw unwind.
#[allow(clippy::too_many_arguments)]
pub(super) fn with_source_projection_facts_v18<'g, 'i, 'work, T>(
    relation: &ProductionSourceCorrespondenceV18<'g>,
    sparse: &CanonicalKirSparseV18<'i, 'g>,
    effects: &CanonicalKirCallEffectsV18<'i, 'g>,
    cleanup: &fe2o3_pliron::CanonicalAnalysisCleanupV1<'_>,
    root: usize,
    selection: SemanticKernelBodySelectionV1,
    pending: &mut [Option<SourceCallObligationV18>],
    budget: &mut Budget<'work>,
    consume: impl FnOnce(
        &mut SourceAssertionFactsV18<'_, 'i, 'g, '_, 'work>,
    ) -> Result<T, ProductionRankedProjectionErrorV1>,
) -> Result<T, ProductionRankedProjectionErrorV1> {
    with_source_projection_scope_v18(relation, sparse, effects, cleanup, root,
        SourceProjectionScopeV18::SelectedBody(selection), pending, budget, consume)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn with_source_projection_scope_v18<'g, 'i, 'work, T>(
    relation: &ProductionSourceCorrespondenceV18<'g>,
    sparse: &CanonicalKirSparseV18<'i, 'g>,
    effects: &CanonicalKirCallEffectsV18<'i, 'g>,
    cleanup: &fe2o3_pliron::CanonicalAnalysisCleanupV1<'_>,
    root: usize,
    scope: SourceProjectionScopeV18<'_, 'g>,
    pending: &mut [Option<SourceCallObligationV18>],
    budget: &mut Budget<'work>,
    consume: impl FnOnce(&mut SourceAssertionFactsV18<'_, 'i, 'g, '_, 'work>)
        -> Result<T, ProductionRankedProjectionErrorV1>,
) -> Result<T, ProductionRankedProjectionErrorV1> {
    relation.check_query_v18(budget).map_err(source_error)?;
    let source = relation.source(budget).map_err(source_error)?;
    let original = source.source_semantic(budget).map_err(source_error)?;
    let (instance, original_root, semantic_function) = match scope {
        SourceProjectionScopeV18::SelectedBody(selection) => {
    let instance = relation
        .unique_source_instance(root, selection.body(), budget)
        .map_err(source_error)?
        .ok_or_else(|| binding("selected ranked body is absent from its original root"))?;
    if source.root(root, budget).map_err(source_error)?.0 != selection.root() {
        return Err(binding("ranked selection substituted the original root"));
    }
            (instance, selection.root(), selection.body())
        }
        SourceProjectionScopeV18::Invocation(invocation) => {
            invocation.check(relation, root, budget)?;
            if !invocation.active {
                return Err(binding("invocation projection requires an active original instance"));
            }
            (invocation.instance, invocation.original_root, invocation.function)
        }
    };
    budget
        .charge_work(1)
        .map_err(|error| retained_resource(relation, error))?;
    if original
        .functions()
        .get(semantic_function.index() as usize)
        .is_none_or(|function| function.blocks().len() != pending.len())
    {
        return Err(binding(
            "source call slots differ from the original selected block roster",
        ));
    }
    let mut consumer_failed = false;
    let result = canonical_assertion_facts_v1::with_source_masked_table_v18(
        original,
        semantic_function,
        cleanup,
        budget,
        |masked, budget| {
            let mut facts = SourceAssertionFactsV18 {
                relation,
                sparse,
                effects,
                root,
                instance,
                original_root,
                semantic_function,
                pending,
                slot: budget as *const Budget<'_> as usize,
                ledger: budget.work_ledger_identity_v1(),
                floor: budget.storage(),
                budget,
                masked,
                cleanup,
            };
            facts.check()?;
            // An already-selected projection failure is not replaced by the
            // later custody postflight. The shared scope preserves raw panic.
            let result = consume(&mut facts);
            consumer_failed = result.is_err();
            let postflight = facts.observe_custody().and_then(|()| {
                if result.is_ok() {
                    relation.check_query_v18(facts.budget).map_err(source_error)
                } else {
                    Ok(())
                }
            });
            match result {
                Err(error) => Err(error),
                Ok(value) => match postflight {
                    Ok(()) => Ok(value),
                    Err(error) => {
                        drop(value);
                        Err(error)
                    }
                },
            }
        },
    );
    result.map_err(|error| {
        if consumer_failed {
            error
        } else {
            retain_resource_error(relation, error)
        }
    })
}

// Rejoin every original reachable direct call, including absent pending rows.
// This checks the projection's obligation census, not call values or Storage
// safety. The closed final consumer must discharge every retained obligation.
pub(super) fn check_source_pending_calls_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    selection: SemanticKernelBodySelectionV1,
    summaries: &DefinedCallableEmptyEffectSummariesV1,
    effects: &CanonicalKirCallEffectsV18<'_, '_>,
    pending: &[Option<SourceCallObligationV18>],
    budget: &mut Budget<'_>,
) -> Result<usize, ProductionRankedProjectionErrorV1> {
    check_source_pending_calls_endpoint_v18(
        relation,
        root,
        selection,
        summaries,
        effects,
        pending,
        budget,
        SourceCallCensusEndpointV18::Original,
    )
}

enum SourceCallCensusEndpointV18<'a, 'i, 'g> {
    Original,
    Optimized {
        relation: &'a fe2o3_lower_mir_kernel::ProductionOptimizedSourceCorrespondenceV18<'g>,
        effects: &'a CanonicalKirCallEffectsV18<'i, 'g>,
    },
}

#[allow(clippy::too_many_arguments)]
pub(super) fn check_optimized_source_pending_calls_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &fe2o3_lower_mir_kernel::ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    selection: SemanticKernelBodySelectionV1,
    summaries: &DefinedCallableEmptyEffectSummariesV1,
    input_effects: &CanonicalKirCallEffectsV18<'_, '_>,
    output_effects: &CanonicalKirCallEffectsV18<'_, '_>,
    pending: &[Option<SourceCallObligationV18>],
    budget: &mut Budget<'_>,
) -> Result<usize, ProductionRankedProjectionErrorV1> {
    relation.check_query_v18(budget).map_err(source_error)?;
    let headers = (|| {
        source_ranked_consumer_resources_v18::add(
            std::mem::size_of::<SourceCallCensusEndpointV18<'_, '_, '_>>(),
            source_ranked_consumer_resources_v18::add(
                std::mem::size_of::<Result<usize, ProductionRankedProjectionErrorV1>>(),
                source_ranked_consumer_resources_v18::add(
                    std::mem::size_of::<
                        fe2o3_lower_mir_kernel::ProductionOptimizedSourceBlockControlV18<'_, '_>,
                    >(),
                    std::mem::size_of::<
                        Result<
                            fe2o3_lower_mir_kernel::ProductionOptimizedSourceBlockControlV18<
                                '_,
                                '_,
                            >,
                            ProductionSourceOwnedViewErrorV18,
                        >,
                    >(),
                )?,
            )?,
        )
    })()
    .map_err(|error| retain_resource_error(relation, error))?;
    budget
        .reserve_storage(headers)
        .map_err(|error| retained_resource(relation, error))?;
    budget
        .charge_work(5)
        .map_err(|error| retained_resource(relation, error))?;
    let source = relation.source(budget).map_err(source_error)?;
    let input = relation.inventory(budget).map_err(source_error)?;
    if !std::ptr::eq(
        source,
        optimized.original_source(budget).map_err(source_error)?,
    ) || !std::ptr::eq(
        input,
        optimized.input_inventory(budget).map_err(source_error)?,
    ) || !input_effects.belongs_to(input)
        || !output_effects.belongs_to(optimized.output_inventory(budget).map_err(source_error)?)
    {
        return Err(binding(
            "optimized pending-call census changed exact source or analysis endpoints",
        ));
    }
    let count = check_source_pending_calls_endpoint_v18(
        relation,
        root,
        selection,
        summaries,
        input_effects,
        pending,
        budget,
        SourceCallCensusEndpointV18::Optimized {
            relation: optimized,
            effects: output_effects,
        },
    )?;
    // These stack-only headers are no longer live. Other retained caller
    // rows and reports stay charged until the enclosing source scope drops them.
    budget
        .release_storage(headers)
        .map_err(|error| retained_resource(relation, error))?;
    Ok(count)
}

#[allow(clippy::too_many_arguments)]
fn check_source_pending_calls_endpoint_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    selection: SemanticKernelBodySelectionV1,
    summaries: &DefinedCallableEmptyEffectSummariesV1,
    effects: &CanonicalKirCallEffectsV18<'_, '_>,
    pending: &[Option<SourceCallObligationV18>],
    budget: &mut Budget<'_>,
    endpoint: SourceCallCensusEndpointV18<'_, '_, '_>,
) -> Result<usize, ProductionRankedProjectionErrorV1> {
    let source = relation.source(budget).map_err(source_error)?;
    let semantic = source.source_semantic(budget).map_err(source_error)?;
    let actual_selection = semantic
        .select_kernel_body_for_root_v1(selection.root())
        .ok_or_else(|| binding("source call census has no original selected body"))?;
    let function = semantic
        .functions()
        .get(selection.body().index() as usize)
        .ok_or_else(|| binding("source call census selected function is absent"))?;
    budget
        .charge_work(4)
        .map_err(|error| retained_resource(relation, error))?;
    if actual_selection.root() != selection.root()
        || actual_selection.body() != selection.body()
        || source.root(root, budget).map_err(source_error)?.0 != selection.root()
        || pending.len() != function.blocks().len()
        || !std::ptr::eq(
            relation.inventory(budget).map_err(source_error)?,
            effects.inventory(),
        )
    {
        return Err(binding(
            "source call census changed original root, body or inventory",
        ));
    }
    let caller = relation
        .unique_source_instance(root, selection.body(), budget)
        .map_err(source_error)?
        .ok_or_else(|| binding("source call census selected body is not an original instance"))?;
    let mut count = 0usize;
    for (index, block) in function.blocks().iter().enumerate() {
        budget
            .charge_work(3)
            .map_err(|error| retained_resource(relation, error))?;
        let block_id = SemanticBlockIdV1::from_index(
            u32::try_from(index).map_err(|_| retained_resource(relation, Resource::Arithmetic))?,
        );
        let reachable = match &endpoint {
            SourceCallCensusEndpointV18::Original => relation
                .source_block_entry(root, caller, block_id, budget)
                .map_err(source_error)?
                .is_some(),
            SourceCallCensusEndpointV18::Optimized { relation, .. } => {
                use fe2o3_lower_mir_kernel::ProductionOptimizedSourceSiteControlV18 as D;
                let control = relation
                    .source_block_control(root, caller, block_id, budget)
                    .map_err(source_error)?;
                match control.site(None, budget).map_err(source_error)? {
                    D::Retained => true,
                    D::RemovedUnreachable | D::OriginalUnmaterialized => false,
                    D::Mixed => {
                        return Err(binding(
                            "pending call has partially surviving source control",
                        ));
                    }
                }
            }
        };
        let actual = pending[index];
        let mut expected = None;
        if reachable {
            let callee = match block.terminator().kind() {
                SemanticTerminatorKindV1::Call(call) => Some((call.callee(), false)),
                SemanticTerminatorKindV1::TailCall(call) => Some((call.callee(), true)),
                _ => None,
            };
            if let Some((callee, tail)) = callee {
                if let Some(SemanticCallableDeclV1::Defined { function: callee }) =
                    semantic.callables().get(callee.index() as usize)
                {
                    let child = relation
                        .defined_call_instance(root, caller, block_id, budget)
                        .map_err(source_error)?;
                    if source.instance(root, child, budget).map_err(source_error)?
                        != (*callee, Some((caller, block_id)))
                    {
                        return Err(binding(
                            "source call census changed its exact original child",
                        ));
                    }
                    let (decision, output_decision) = match &endpoint {
                        SourceCallCensusEndpointV18::Original => {
                            let input = relation
                                .instance_effect_decision(root, child, effects, budget)
                                .map_err(source_error)?;
                            (input, input)
                        }
                        SourceCallCensusEndpointV18::Optimized {
                            relation,
                            effects: output,
                        } => {
                            let decisions = relation
                                .instance_effects(root, child, effects, output, budget)
                                .map_err(source_error)?;
                            (decisions.input, decisions.output)
                        }
                    };
                    if decision == CanonicalKirCallEffectDecisionV1::Incomplete
                        || output_decision == CanonicalKirCallEffectDecisionV1::Incomplete
                    {
                        return Err(binding(
                            "source call census cannot defer incomplete physical effects",
                        ));
                    }
                    if !summaries.is_exact_empty(*callee) {
                        if tail {
                            return Err(binding(
                                "pending source tail-call effects have no checked projection disposition",
                            ));
                        }
                        expected = Some(SourceCallObligationV18 {
                            root,
                            caller_instance: caller,
                            caller_function: selection.body(),
                            source_block: block_id,
                            callee_instance: child,
                            callee_function: *callee,
                        });
                    } else if decision != CanonicalKirCallEffectDecisionV1::CompleteEmpty
                        || output_decision != CanonicalKirCallEffectDecisionV1::CompleteEmpty
                    {
                        return Err(binding(
                            "source empty callable summary omitted a physical effect",
                        ));
                    }
                }
            }
        }
        if actual != expected {
            return Err(binding(
                "source call obligation census omitted, duplicated or substituted a call",
            ));
        }
        if actual.is_some() {
            count = count
                .checked_add(1)
                .ok_or_else(|| retained_resource(relation, Resource::Arithmetic))?;
        }
    }
    Ok(count)
}

impl SourceAssertionFactsV18<'_, '_, '_, '_, '_> {
    pub(super) fn observe_custody(&self) -> Result<(), ProductionRankedProjectionErrorV1> {
        if self.cleanup.refund_denied()
            || self.slot != self.budget as *const Budget<'_> as usize
            || self.ledger != self.budget.work_ledger_identity_v1()
            || self.budget.storage() < self.floor
        {
            self.cleanup.deny_refund();
            return Err(source_error(Resource::Accounting.into()));
        }
        Ok(())
    }

    fn check(&mut self) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.observe_custody()
            .map_err(|error| retain_resource_error(self.relation, error))?;
        self.relation
            .check_query_v18(self.budget)
            .map_err(source_error)?;
        let inventory = self.relation.inventory(self.budget).map_err(source_error)?;
        self.budget
            .charge_work(3)
            .map_err(|error| retained_resource(self.relation, error))?;
        if !std::ptr::eq(inventory, self.sparse.inventory())
            || !std::ptr::eq(inventory, self.effects.inventory())
        {
            return Err(binding(
                "source projection facts belong to another actual inventory",
            ));
        }
        let source = self.relation.source(self.budget).map_err(source_error)?;
        if source.root(self.root, self.budget).map_err(source_error)?.0 != self.original_root
            || source
                .instance(self.root, self.instance, self.budget)
                .map_err(source_error)?
                .0
                != self.semantic_function
        {
            return Err(binding(
                "source projection facts lost their original root/instance",
            ));
        }
        Ok(())
    }

    fn exact_child(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
    ) -> Result<(usize, SemanticFunctionIdV1), ProductionRankedProjectionErrorV1> {
        self.check()?;
        let source = self.relation.source(self.budget).map_err(source_error)?;
        let semantic = source.source_semantic(self.budget).map_err(source_error)?;
        let actual = semantic
            .functions()
            .get(self.semantic_function.index() as usize)
            .and_then(|function| function.blocks().get(block.index() as usize))
            .map(|block| block.terminator().kind())
            .ok_or_else(|| binding("source call block is outside its original function"))?;
        let SemanticTerminatorKindV1::Call(actual) = actual else {
            return Err(binding(
                "source call projection is not its original direct call",
            ));
        };
        if !std::ptr::eq(actual, call) {
            return Err(binding(
                "source call projection substituted an original call",
            ));
        }
        let child = self
            .relation
            .defined_call_instance(self.root, self.instance, block, self.budget)
            .map_err(source_error)?;
        let (function, incoming) = source
            .instance(self.root, child, self.budget)
            .map_err(source_error)?;
        if incoming != Some((self.instance, block)) {
            return Err(binding(
                "source call projection changed its exact incoming instance",
            ));
        }
        match self
            .relation
            .instance_effect_decision(self.root, child, self.effects, self.budget)
            .map_err(source_error)?
        {
            CanonicalKirCallEffectDecisionV1::Incomplete => {
                return Err(binding(
                    "incomplete source-instance effects cannot become pending local storage",
                ));
            }
            CanonicalKirCallEffectDecisionV1::CompleteEmpty
            | CanonicalKirCallEffectDecisionV1::CompleteNonempty => {}
        }
        Ok((child, function))
    }
}

impl ProjectedAssertionFactsV1 for SourceAssertionFactsV18<'_, '_, '_, '_, '_> {
    fn masked_assertion_source_proved_v1(
        &mut self,
        function: &SemanticFunctionDeclV1,
        block: usize,
        expected: bool,
        successor: SemanticBlockIdV1,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        self.check()?;
        let source = self.relation.source(self.budget).map_err(source_error)?;
        let semantic = source.source_semantic(self.budget).map_err(source_error)?;
        self.masked
            .proves(
                semantic,
                self.semantic_function,
                function,
                block,
                expected,
                successor,
                self.budget,
            )
            .map_err(|error| retain_resource_error(self.relation, error))
    }

    fn call_projection_disposition_v18<'call>(
        &mut self,
        block: usize,
        call: &'call SemanticDirectCallV1,
        _source: SemanticSourceProvenanceV1,
    ) -> Result<CallProjectionDispositionV18<'call>, ProductionRankedProjectionErrorV1> {
        let block = SemanticBlockIdV1::from_index(
            u32::try_from(block)
                .map_err(|_| retained_resource(self.relation, Resource::Arithmetic))?,
        );
        let (callee_instance, callee_function) = self.exact_child(block, call)?;
        Ok(CallProjectionDispositionV18::PendingStorage(
            PendingSourceCallV18 {
                scope: std::ptr::from_ref(self) as usize,
                root: self.root,
                caller_instance: self.instance,
                caller_function: self.semantic_function,
                source_block: block,
                callee_instance,
                callee_function,
                call,
            },
        ))
    }

    fn accept_pending_source_call_v18(
        &mut self,
        pending: PendingSourceCallV18<'_>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.check()?;
        if pending.scope != std::ptr::from_ref(self) as usize
            || pending.root != self.root
            || pending.caller_instance != self.instance
            || pending.caller_function != self.semantic_function
        {
            return Err(binding(
                "pending source call belongs to another live facts scope",
            ));
        }
        let (child, function) = self.exact_child(pending.source_block, pending.call)?;
        if child != pending.callee_instance || function != pending.callee_function {
            return Err(binding(
                "pending source call changed its original child instance",
            ));
        }
        self.budget
            .charge_work(2)
            .map_err(|error| retained_resource(self.relation, error))?;
        let row = SourceCallObligationV18 {
            root: self.root,
            caller_instance: self.instance,
            caller_function: self.semantic_function,
            source_block: pending.source_block,
            callee_instance: child,
            callee_function: function,
        };
        let slot = self
            .pending
            .get_mut(row.source_block.index() as usize)
            .ok_or_else(|| binding("pending source call is outside the original block roster"))?;
        if slot.is_some() {
            return Err(binding(
                "pending source call duplicated an original occurrence",
            ));
        }
        *slot = Some(row);
        Ok(())
    }

    fn charge_private_array_work(
        &mut self,
        amount: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.check()?;
        self.budget
            .charge_work(amount)
            .map_err(|error| retained_resource(self.relation, error))
    }

    fn helper_value_ledger_v1(
        &self,
    ) -> Result<
        (
            usize,
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        ),
        ProductionRankedProjectionErrorV1,
    > {
        self.observe_custody()
            .map_err(|error| retain_resource_error(self.relation, error))?;
        self.relation
            .check_query_v18(self.budget)
            .map_err(source_error)?;
        Ok((
            self.budget as *const Budget<'_> as usize,
            self.budget.work_ledger_identity_v1(),
        ))
    }

    fn scalar_private_storage_v1(&self) -> Result<usize, ProductionRankedProjectionErrorV1> {
        self.observe_custody()
            .map_err(|error| retain_resource_error(self.relation, error))?;
        self.relation
            .check_query_v18(self.budget)
            .map_err(source_error)?;
        Ok(self.budget.storage())
    }

    fn reserve_scalar_private_storage_v1(
        &mut self,
        amount: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.check()?;
        self.budget
            .reserve_storage(amount)
            .map_err(|error| retained_resource(self.relation, error))
    }

    fn release_scalar_private_storage_v1(
        &mut self,
        amount: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.check()?;
        if self
            .budget
            .storage()
            .checked_sub(amount)
            .is_none_or(|remaining| remaining < self.floor)
        {
            self.cleanup.deny_refund();
            return Err(retained_resource(self.relation, Resource::Accounting));
        }
        self.budget
            .release_storage(amount)
            .map_err(|error| retained_resource(self.relation, error))
    }

    fn private_array_initializer_count(
        &mut self,
        block: usize,
        statement: usize,
    ) -> Result<Option<u64>, ProductionRankedProjectionErrorV1> {
        self.check()?;
        let block = u32::try_from(block)
            .map_err(|_| retained_resource(self.relation, Resource::Arithmetic))?;
        let statement = u32::try_from(statement)
            .map_err(|_| retained_resource(self.relation, Resource::Arithmetic))?;
        self.relation
            .private_array_initializer_count(
                self.root,
                self.instance,
                SemanticBlockIdV1::from_index(block),
                statement,
                self.budget,
            )
            .map_err(source_error)
    }

    fn slice_access(
        &mut self,
        site: ProjectedSemanticAccessSiteV1,
        ordinal: u32,
        assertion: u32,
    ) -> Result<slice_projection_v1::ProjectedSliceInputV1, ProductionRankedProjectionErrorV1> {
        self.check()?;
        let block = u32::try_from(site.block)
            .map_err(|_| retained_resource(self.relation, Resource::Arithmetic))?;
        let statement = site
            .statement
            .map(u32::try_from)
            .transpose()
            .map_err(|_| retained_resource(self.relation, Resource::Arithmetic))?;
        let site = ProductionSliceAccessSiteV1::new(
            self.original_root,
            self.semantic_function,
            SemanticBlockIdV1::from_index(block),
            statement,
            ordinal,
            SemanticBlockIdV1::from_index(assertion),
        );
        self.relation
            .with_checked_slice_access_v18(
                self.root,
                self.instance,
                site,
                self.budget,
                checked_slice_projection_v18,
            )
            .map_err(source_error)
    }

    fn is_materialized_block(
        &mut self,
        block: usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        self.check()?;
        let block = u32::try_from(block)
            .map_err(|_| retained_resource(self.relation, Resource::Arithmetic))?;
        self.relation
            .source_block_entry(
                self.root,
                self.instance,
                SemanticBlockIdV1::from_index(block),
                self.budget,
            )
            .map(|row| row.is_some())
            .map_err(source_error)
    }

    fn condition(
        &mut self,
        block: usize,
        expected: bool,
        semantic_success: SemanticBlockIdV1,
    ) -> Result<ProjectedAssertionConditionV1, ProductionRankedProjectionErrorV1> {
        self.check()?;
        self.budget
            .charge_work(4)
            .map_err(|error| retained_resource(self.relation, error))?;
        let block = u32::try_from(block)
            .map_err(|_| retained_resource(self.relation, Resource::Arithmetic))?;
        let binding = self
            .relation
            .assertion(
                self.root,
                self.instance,
                SemanticBlockIdV1::from_index(block),
                self.budget,
            )
            .map_err(source_error)?;
        condition_from_checked_binding_v18(
            binding,
            expected,
            semantic_success,
            self.sparse,
            self.budget,
        )
        .map_err(|error| retain_resource_error(self.relation, error))
    }
}
