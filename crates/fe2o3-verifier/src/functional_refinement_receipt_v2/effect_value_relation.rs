//! Shared formula relation, not a wire mode, source declaration or proof.

use super::*;
use fe2o3_pliron::ProductionEffectRefinementContractV2;

type Pair = (ProductionRankedValueV1, ProductionRankedValueV1);

#[derive(Clone, Copy)]
pub(super) enum EffectValueRelation {
    Exact,
    NumericalRequest { block: usize, operation: usize },
}

/// Existing codecs select Exact. NumericalRequest is an unresolved intent:
/// even coherent same-store inputs cannot produce equality pairs or a proof.
pub(super) fn pairs(
    kernel: &ProductionRankedKernelV1,
    effect: Option<&ProductionEffectRefinementContractV2>,
    subjects: FunctionalRefinementSubjectsV2,
    relation: EffectValueRelation,
) -> Result<Vec<Pair>, FunctionalRefinementVerusExecutionErrorV2> {
    match relation {
        EffectValueRelation::Exact => {
            let contract = effect.ok_or_else(invalid_ranked_recipe)?;
            // Preserve the old pair order and allocation behavior exactly.
            let mut pairs = contract
                .gpu_coordinates()
                .iter()
                .copied()
                .zip(contract.reference_coordinates().iter().copied())
                .collect::<Vec<_>>();
            pairs.extend([
                (contract.gpu_domain(), contract.reference_domain()),
                (
                    contract.gpu_precondition(),
                    contract.reference_precondition(),
                ),
                (contract.gpu_value(), contract.reference_value()),
            ]);
            Ok(pairs)
        }
        EffectValueRelation::NumericalRequest { block, operation } => {
            let Some(ProductionRankedOperationV1::RequestNumericalRefinement {
                contract: numerical,
                subjects: requested_subjects,
            }) = kernel
                .blocks()
                .get(block)
                .and_then(|b| b.operations().get(operation))
            else {
                return Err(invalid_ranked_recipe());
            };
            if subjects != *requested_subjects {
                return Err(invalid_ranked_recipe());
            }
            if let Some(effect) = effect {
                if effect.gpu_value() != numerical.actual()
                    || effect.reference_value() != numerical.reference()
                    || effect.gpu_domain() != numerical.domain()
                    || effect.reference_domain() != numerical.domain()
                    || effect.gpu_precondition() != numerical.precondition()
                    || effect.reference_precondition() != numerical.precondition()
                {
                    return Err(invalid_ranked_recipe());
                }
                let site = effect.gpu_write_site();
                let Some(ProductionRankedOperationV1::ValueAccess {
                    kind: dialect_kernel::AccessKindAttr::Write,
                    view,
                    indices,
                    value,
                }) = kernel
                    .blocks()
                    .get(site.block() as usize)
                    .and_then(|b| b.operations().get(site.operation() as usize))
                else {
                    return Err(invalid_ranked_recipe());
                };
                if *view != effect.view()
                    || indices != effect.indices()
                    || *value != numerical.actual()
                {
                    return Err(invalid_ranked_recipe());
                }
            }
            // No scan, tree clone, pair allocation, normalization or renderer
            // precedes this rejection. Site lookups and bounded-rank comparisons
            // are inert checks, not authenticated output coverage or codec joins.
            Err(claim_specific_numerical_proof_required())
        }
    }
}

/// Called by the existing aggregate replay AFTER its receipt/obligation check.
/// Keeping the relation gate before build prevents equality-as-error fallback.
pub(super) fn replay_program(
    kernel: &ProductionRankedKernelV1,
    effect: &ProductionEffectRefinementContractV2,
    subjects: FunctionalRefinementSubjectsV2,
    relation: EffectValueRelation,
) -> Result<(SemanticFormulaProgramV2, Vec<Pair>), FunctionalRefinementVerusExecutionErrorV2> {
    let pairs = pairs(kernel, Some(effect), subjects, relation)?;
    let program = SemanticFormulaProgramV2::build(kernel, &pairs)?;
    Ok((program, pairs))
}

#[cfg(test)]
#[path = "effect_value_relation/tests.rs"]
mod tests;
