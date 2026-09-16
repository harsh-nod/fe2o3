//! Join source helper associations to independently classified executable effects.

use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirCallEffectDecisionV1 as Decision, CanonicalKirCallEffectsV1,
    CanonicalKirInventoryV1,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;

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
        let mut summaries = derive_defined_callable_empty_effect_summaries_v1(
            semantic.types(),
            semantic.functions(),
            semantic.callables(),
        )?;
        let helpers = source.empty_effect_helpers();
        budget
            .charge_work(helpers.scanned_function_count())
            .map_err(ranked_projection_source_v1::resource)?;
        for helper in helpers.iter() {
            let physical = inventory
                .function_for_name(helper.kernel_ir_function().as_str(), budget)
                .map_err(|error| {
                    ProductionRankedProjectionErrorV1::CanonicalAssertions(
                        CanonicalAssertionErrorV1::Inventory(error),
                    )
                })?
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "materialized helper is absent from its executable owner",
                ))?;
            if effects
                .decision(physical.coordinate, budget)
                .map_err(effect_error)?
                != Decision::CompleteEmpty
            {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "materialized helper effects are not independently complete and empty",
                ));
            }
            let decision = summaries
                .decisions
                .get_mut(helper.semantic_function().index() as usize)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "materialized helper effect fact is outside its source owner",
                ))?;
            match decision {
                // Empty effects establish neither scalar values nor determinism.
                DefinedCallableEmptyEffectDecisionV1::Rejected => {
                    *decision = DefinedCallableEmptyEffectDecisionV1::ExactEmptyOnly
                }
                DefinedCallableEmptyEffectDecisionV1::ExactEmptyOnly
                | DefinedCallableEmptyEffectDecisionV1::ExactEmptyDeterministicScalar => {}
                DefinedCallableEmptyEffectDecisionV1::Unknown => {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "materialized helper effect fact has an unfinished source summary",
                    ));
                }
            }
        }
        Ok(summaries)
    })();
    drop(effects);
    budget
        .release_storage(storage.retained_storage())
        .map_err(ranked_projection_source_v1::resource)?;
    result
}
