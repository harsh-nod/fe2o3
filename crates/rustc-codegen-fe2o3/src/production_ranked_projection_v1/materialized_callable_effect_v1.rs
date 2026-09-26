//! Join source helper associations to independently classified executable effects.

use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirCallEffectDecisionV1 as Decision, CanonicalKirCallEffectsV1,
    CanonicalKirInventoryV1,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;

// The real V18 continuation owns this result and its entire conservative
// reservation. The source and canonical summaries are distinct analyses;
// neither an empty effect list nor this result certifies scalar determinism.
pub(super) struct SourceCallableSummariesV18 {
    pub(super) summaries: DefinedCallableEmptyEffectSummariesV1,
    pub(super) retained_storage: usize,
}

pub(super) fn derive_source_callable_effect_summaries_v18(
    relation: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
    effects: &fe2o3_kernel_analysis::CanonicalKirCallEffectsV18<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<SourceCallableSummariesV18, ProductionRankedProjectionErrorV1> {
    use canonical_source_facts_v18::source_error;
    use source_ranked_consumer_resources_v18 as resource;
    let floor = budget.storage();
    let view = relation.source(budget).map_err(source_error)?;
    let semantic = view.source_semantic(budget).map_err(source_error)?;
    budget.charge_work(1).map_err(resource::resource)?;
    if !std::ptr::eq(relation.inventory(budget).map_err(source_error)?, effects.inventory()) {
        return Err(source_error(fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Binding(
            "source callable summaries changed actual inventory",
        )));
    }
    budget.reserve_storage(std::mem::size_of::<SourceCallableSummariesV18>())
        .map_err(resource::resource)?;
    let source = neutral_assertion::SemanticDefinedCallableSummariesV1::new_metered(
        semantic.types(), semantic.functions(), semantic.callables(),
        assertion_compatibility_limits_v1(), &mut resource::SourceAssertionMeterV18(budget),
    ).map_err(assertion_projection_error_v1)?;
    let mut decisions = resource::rows(semantic.functions().len(), budget)?;
    for index in 0..semantic.functions().len() {
        budget.charge_work(1).map_err(resource::resource)?;
        let function = SemanticFunctionIdV1::from_index(u32::try_from(index)
            .map_err(|_| resource::resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic))?);
        resource::push(&mut decisions, match source.decision(function) {
            Some(neutral_assertion::SemanticCallableDecisionV1::ExactEmptyDeterministicScalar) =>
                DefinedCallableEmptyEffectDecisionV1::ExactEmptyDeterministicScalar,
            Some(neutral_assertion::SemanticCallableDecisionV1::ExactEmptyOnly) =>
                DefinedCallableEmptyEffectDecisionV1::ExactEmptyOnly,
            Some(neutral_assertion::SemanticCallableDecisionV1::Rejected) | None =>
                DefinedCallableEmptyEffectDecisionV1::Rejected,
        })?;
    }
    for root in 0..view.root_count(budget).map_err(source_error)? {
        for instance in 0..view.instance_count(root, budget).map_err(source_error)? {
            if !view.instance_active(root, instance, budget).map_err(source_error)? { continue; }
            let (function, incoming) = view.instance(root, instance, budget).map_err(source_error)?;
            if incoming.is_none() { continue; }
            let decision = relation.instance_effect_decision(root, instance, effects, budget)
                .map_err(source_error)?;
            budget.charge_work(1).map_err(resource::resource)?;
            let summary = decisions.get_mut(function.index() as usize)
                .ok_or_else(|| source_error(fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Binding(
                    "source callable instance is outside the original roster",
                )))?;
            match decision {
                Decision::CompleteEmpty => join_raw_empty_summary_v1(summary)?,
                Decision::CompleteNonempty => *summary = DefinedCallableEmptyEffectDecisionV1::LocalMemoryRequiresCall,
                Decision::Incomplete => {
                    return Err(source_error(fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Binding(
                        "incomplete original helper effects cannot acquire ranked-source correspondence",
                    )));
                }
            }
        }
    }
    // Pay for a possible Vec-to-Box compaction while both backings coexist.
    budget.reserve_storage(resource::product(decisions.len(), std::mem::size_of::<DefinedCallableEmptyEffectDecisionV1>())?)
        .map_err(resource::resource)?;
    let summaries = DefinedCallableEmptyEffectSummariesV1 { decisions: decisions.into_boxed_slice() };
    drop(source);
    let retained_storage = budget.storage().checked_sub(floor)
        .ok_or_else(|| resource::resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting))?;
    Ok(SourceCallableSummariesV18 { summaries, retained_storage })
}

pub(super) fn derive_materialized_callable_effect_summaries_v1(
    source: &RankedProjectionSourceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<DefinedCallableEmptyEffectSummariesV1, ProductionRankedProjectionErrorV1> {
    source.require_floor(budget)?;
    budget
        .charge_work(1)
        .map_err(ranked_projection_source_v1::resource)?;
    if !inventory.belongs_to(source.executable()) {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "materialized helper effects belong to another executable owner",
        ));
    }
    let effect_error = |error| {
        ProductionRankedProjectionErrorV1::CanonicalAssertions(
            CanonicalAssertionErrorV1::CallEffects(error),
        )
    };
    let (effects, storage) =
        CanonicalKirCallEffectsV1::derive(inventory, budget).map_err(effect_error)?;
    budget
        .reserve_storage(storage.retained_storage())
        .map_err(ranked_projection_source_v1::resource)?;
    let result = (|| {
        let semantic = source.semantic_ssa().source_semantic();
        // The neutral source closure cannot grant a materialized call-local memory effect.
        let mut summaries = derive_defined_callable_empty_effect_summaries_v1(
            semantic.types(),
            semantic.functions(),
            semantic.callables(),
        )?;
        source
            .owner()
            .with_checked_canonical_calls_v1(inventory, budget, |calls, budget| {
                Ok(join_empty_helpers_v1(
                    source,
                    inventory,
                    calls,
                    &effects,
                    &mut summaries,
                    budget,
                ))
            })
            .map_err(ProductionRankedProjectionErrorV1::StructuralValidation)??;
        Ok(summaries)
    })();
    drop(effects);
    budget
        .release_storage(storage.retained_storage())
        .map_err(ranked_projection_source_v1::resource)?;
    result
}

fn join_empty_helpers_v1(
    source: &RankedProjectionSourceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    calls: &fe2o3_lower_mir_kernel::ProductionCanonicalCallsV1<'_>,
    effects: &CanonicalKirCallEffectsV1<'_, '_>,
    summaries: &mut DefinedCallableEmptyEffectSummariesV1,
    budget: &mut Budget<'_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    budget
        .charge_work(calls.function_count())
        .map_err(ranked_projection_source_v1::resource)?;
    for function in calls.functions() {
        let helper = function.source();
        if helper.role() != fe2o3_lower_mir_kernel::SemanticKirFunctionRoleV1::InternalHelper {
            continue;
        }
        if effects
            .decision(function.canonical().coordinate, budget)
            .map_err(|error| {
                ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::CallEffects(error),
                )
            })?
            != Decision::CompleteEmpty
        {
            source
                .owner()
                .with_checked_unit_local_source_v1(inventory, budget, |view, budget| {
                    let association = view.association(
                        helper.correspondence_owner(),
                        helper.semantic_function(),
                        budget,
                    )?;
                    budget.charge_work(3)?;
                    let Some(association) = association else {
                        return Ok(Err(ProductionRankedProjectionErrorV1::Unsupported(
                            "materialized helper effects are not independently complete and empty",
                        )));
                    };
                    if association.physical_function() != function.canonical().coordinate.0 as usize
                    {
                        return Ok(Err(ProductionRankedProjectionErrorV1::Unsupported(
                            "materialized local helper relation names another physical function",
                        )));
                    }
                    Ok(Ok(()))
                })
                .map_err(ProductionRankedProjectionErrorV1::StructuralValidation)??;
            let decision = summaries
                .decisions
                .get_mut(helper.semantic_function().index() as usize)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "materialized helper effect fact is outside its source owner",
                ))?;
            // Local storage is real. Only the separate exact call query can
            // discharge its ranked bounds effect at a caller site.
            *decision = DefinedCallableEmptyEffectDecisionV1::LocalMemoryRequiresCall;
            continue;
        }
        let decision = summaries
            .decisions
            .get_mut(helper.semantic_function().index() as usize)
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "materialized helper effect fact is outside its source owner",
            ))?;
        join_raw_empty_summary_v1(decision)?;
    }
    Ok(())
}

pub(super) fn join_raw_empty_summary_v1(
    decision: &mut DefinedCallableEmptyEffectDecisionV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    match decision {
        // Empty effects establish neither scalar values nor determinism.
        DefinedCallableEmptyEffectDecisionV1::Rejected => {
            *decision = DefinedCallableEmptyEffectDecisionV1::ExactEmptyOnly;
        }
        DefinedCallableEmptyEffectDecisionV1::ExactEmptyOnly
        | DefinedCallableEmptyEffectDecisionV1::ExactEmptyDeterministicScalar
        | DefinedCallableEmptyEffectDecisionV1::LocalMemoryRequiresCall => {}
        #[cfg(test)]
        DefinedCallableEmptyEffectDecisionV1::Unknown => {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "materialized helper effect fact has an unfinished source summary",
            ));
        }
    }
    Ok(())
}
