//! One physical/source lifetime reader for original and sealed transported sites.
use super::*;

pub(in crate::production_semantic_kir_v1) fn with_canonical_private_source_reader_v1<'w, T>(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    budget: &mut ArgumentBudgetV1<'w>,
    callback: impl FnOnce([usize; 2], &mut ArgumentBudgetV1<'w>) -> CrPolicyResultV1<T>,
) -> CrPolicyResultV1<T> {
    let proof = check(source.inventory, source.owner.limits.max_operations, budget)
        .map_err(ProductionCanonicalRankedPolicyErrorV1::PrivateSource)?;
    let result = with_sites(source, source.inventory, &proof, Some, budget, callback);
    drop(proof);
    result
}

impl canonical_assertion_v1::CpcSiteViewV1<'_, '_, '_> {
    pub(in crate::production_semantic_kir_v1) fn with_private_source_reader_v1<'w, T>(
        &self,
        proof: &PrivateMemory<'_, '_>,
        budget: &mut ArgumentBudgetV1<'w>,
        callback: impl FnOnce([usize; 2], &mut ArgumentBudgetV1<'w>) -> CrPolicyResultV1<T>,
    ) -> CrPolicyResultV1<T> {
        self.source().guard.query(budget)?;
        with_sites(
            self.source(),
            self.inventory(),
            proof,
            |ordinal| self.original_operation(ordinal),
            budget,
            callback,
        )
    }
}

// Raw lookup closures stay private. Only the original owner wrapper or the sealed
// history capability above can select source coordinates for this shared engine.
fn with_sites<'w, T>(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    proof: &PrivateMemory<'_, '_>,
    original_operation: impl Fn(usize) -> Option<usize>,
    budget: &mut ArgumentBudgetV1<'w>,
    callback: impl FnOnce([usize; 2], &mut ArgumentBudgetV1<'w>) -> CrPolicyResultV1<T>,
) -> CrPolicyResultV1<T> {
    budget.charge_work(1)?;
    if !proof.is_for(inventory) {
        return Err(cr_policy_unsupported_v1(
            ProductionCanonicalRankedSourceRequirementV1::GraphIdentity,
            0,
        ));
    }
    let mut sites = scratch::<Option<(SemanticFunctionIdV1, SemanticBlockIdV1, u32)>>(
        inventory.operations().len(),
        budget,
    )
    .map_err(ProductionCanonicalRankedPolicyErrorV1::PrivateSource)?;
    budget.charge_work(inventory.operations().len())?;
    sites.resize(inventory.operations().len(), None);
    let mut counts = [0usize; 2];
    for (index, row) in inventory.operations().iter().enumerate() {
        budget.charge_work(2)?;
        if matches!(row.operation.kind, OperationKind::Alloca { .. }) {
            counts[0] = argument_sum_v1(&[counts[0], 1])?;
        }
        if matches!(
            row.operation.kind,
            OperationKind::Load { .. } | OperationKind::Store { .. }
        ) {
            if !proof.operation(index) {
                return Err(ProductionCanonicalRankedPolicyErrorV1::PrivateSource(
                    refused("canonical private", "every physical access is checked"),
                ));
            }
            counts[1] = argument_sum_v1(&[counts[1], 1])?;
        }
    }
    for (selected, group) in source.calls.groups.iter().enumerate() {
        let selected_function = group.function.canonical.coordinate;
        for (index, operation) in inventory.operations().iter().enumerate() {
            budget.charge_work(2)?;
            let Some(original) = original_operation(index) else {
                if !matches!(operation.operation.kind, OperationKind::Constant(_)) {
                    return Err(cr_policy_unsupported_v1(
                        ProductionCanonicalRankedSourceRequirementV1::GraphIdentity,
                        index,
                    ));
                }
                sites[index] = None;
                continue;
            };
            let original_row = source.inventory.operations().get(original).ok_or_else(|| {
                cr_policy_unsupported_v1(
                    ProductionCanonicalRankedSourceRequirementV1::GraphIdentity,
                    index,
                )
            })?;
            let origins = &source.source.origins[source.source.operation_origins[original].clone()];
            budget.charge_work(origins.len())?;
            let origin = if original_row.coordinate.block.function == selected_function {
                origins
                    .iter()
                    .find(|origin| source.source.spans[origin.span].association == selected)
            } else {
                origins.first()
            }
            .ok_or_else(|| {
                cr_policy_unsupported_v1(
                    ProductionCanonicalRankedSourceRequirementV1::GraphIdentity,
                    index,
                )
            })?;
            sites[index] = match source.source.spans[origin.span].site {
                ProductionCanonicalRankedSourceSiteV1::Statement { span, .. } => Some((
                    span.semantic_function(),
                    span.semantic_block(),
                    span.statement_ordinal(),
                )),
                _ => None,
            };
        }
        source_lifetimes_from_sites(
            source.owner.semantic_ssa.source_semantic(),
            proof,
            &sites,
            budget,
        )
        .map_err(ProductionCanonicalRankedPolicyErrorV1::PrivateSource)?;
    }
    let result = callback(counts, budget);
    drop(sites);
    result
}
