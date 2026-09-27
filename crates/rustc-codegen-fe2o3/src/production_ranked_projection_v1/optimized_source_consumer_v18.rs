use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_lower_mir_kernel::{
    ProductionOptimizedSourceScalarLeavesV18, ProductionOptimizedSourceScalarStoreDispositionV18,
    ProductionCheckedSourceEntryWritesV18,
};

/// Counts explicit source dispositions, not proved memory obligations.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct OptimizedScalarStoreCensusV18 {
    pub(super) retained: usize,
    pub(super) unreachable: usize,
}

/// Resolves original helper arguments using the existing source resolver, then
/// lends only completed original/output typed-entry RHS correspondence rows.
pub(super) fn with_checked_source_entry_writes_v18<'work, T>(
    semantic: &AdmittedInertSemanticMirV1,
    leaves: &ProductionOptimizedSourceScalarLeavesV18<'_>,
    budget: &mut Budget<'work>,
    consume: impl for<'scope> FnOnce(&ProductionCheckedSourceEntryWritesV18<'scope>,
        &mut Budget<'work>) -> Result<T, ProductionRankedProjectionErrorV1>,
) -> Result<T, ProductionRankedProjectionErrorV1> {
    let original = leaves.original_leaves(budget)?;
    leaves.with_checked_entry_writes_v18(budget, |request, budget| {
        source_ranked_consumer_v18::check_source_entry_write_v18(
            semantic, original, request, budget)
    }, consume)
}

pub(super) fn check_optimized_source_scalar_stores_v18(
    semantic: &AdmittedInertSemanticMirV1,
    leaves: &ProductionOptimizedSourceScalarLeavesV18<'_>,
    budget: &mut Budget<'_>,
) -> Result<OptimizedScalarStoreCensusV18, ProductionRankedProjectionErrorV1> {
    let original = leaves
        .original_leaves(budget)
        .map_err(canonical_source_facts_v18::source_error)?;
    let mut census = OptimizedScalarStoreCensusV18::default();
    let visited = leaves.visit_store_inputs(budget, |disposition, budget| {
        budget
            .charge_work(1)
            .map_err(source_ranked_consumer_resources_v18::resource)?;
        let count = match disposition {
            ProductionOptimizedSourceScalarStoreDispositionV18::Retained(request) => {
                source_ranked_consumer_v18::check_optimized_source_scalar_store_v18(
                    semantic, original, request, budget,
                )?;
                &mut census.retained
            }
            ProductionOptimizedSourceScalarStoreDispositionV18::RemovedUnreachable { .. } => {
                &mut census.unreachable
            }
        };
        *count = count.checked_add(1).ok_or_else(|| {
            source_ranked_consumer_resources_v18::resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
            )
        })?;
        Ok::<_, ProductionRankedProjectionErrorV1>(())
    })?;
    let expected = census
        .retained
        .checked_add(census.unreachable)
        .ok_or_else(|| {
            source_ranked_consumer_resources_v18::resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
            )
        })?;
    if visited != expected {
        return Err(canonical_source_facts_v18::source_error(
            fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Binding(
                "optimized scalar Store disposition census",
            ),
        ));
    }
    Ok(census)
}
