//! Shared actual root entry prefix. DATA only: no source or ready constructor.
//! The paid caller must supply the original-ledger OUTER assembly and the actual
//! retained launch/reference inputs; this helper itself authenticates no owner.
use super::bf16_nominal_preparation_resources_v1::{PreparationResourcesV1, resource};
use super::*;
use crate::reference_effect_v1::{
    AuthenticatedReferenceEffectBindingV1, AuthenticatedReferenceEffectBindingsV1,
    ReferenceOutputCoordinateV1,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
type R<T> = std::result::Result<T, ProductionRankedProjectionErrorV1>;

pub(super) struct RootEntryPrefixV1 {
    pub(super) entry_operations: Vec<ProductionRankedOperationV1>,
    pub(super) next_value: u32,
    pub(super) reserved_reference_values: Option<Vec<ProductionRankedValueIdV1>>,
    // Paid-only source-rank scratch stays in the physical outer assembly.
    rank_scratch: Vec<usize>,
}
impl RootEntryPrefixV1 {
    pub(super) const fn empty() -> Self {
        Self {
            entry_operations: Vec::new(),
            next_value: 0,
            reserved_reference_values: None,
            rank_scratch: Vec::new(),
        }
    }
}

/// Preserve original vec![ExecutionLayout], original output-rank/count helper
/// calls (including their double traversal/allocation), and Vec::with_capacity.
pub(super) fn prepare_root_entry_prefix_legacy_v1(
    source_root: ProductionSourceLaunchRootV1,
    reference_bindings: &AuthenticatedReferenceEffectBindingsV1,
) -> R<RootEntryPrefixV1> {
    let mut entry_operations = vec![ranked_execution_layout_v1(source_root.layout())];
    let mut next_value = 0_u32;
    let reserved_reference_values = if reference_bindings.as_slice().is_empty() {
        None
    } else {
        let output_ranks =
            crate::production_reference_effect_join_v2::reserved_reference_output_ranks_v2(
                reference_bindings,
            )?;
        let count = crate::production_reference_effect_join_v2::reserved_reference_value_count_v2(
            reference_bindings,
        )?;
        let mut values = Vec::with_capacity(count);
        emit_reference_prefix_v1(
            output_ranks,
            &mut values,
            &mut entry_operations,
            &mut next_value,
            &mut PreparationResourcesV1::unmetered(),
        )?;
        debug_assert_eq!(values.len(), count);
        Some(values)
    };
    Ok(RootEntryPrefixV1 {
        entry_operations,
        next_value,
        reserved_reference_values,
        rank_scratch: Vec::new(),
    })
}

fn push_prefix_operation_v1(
    operations: &mut Vec<ProductionRankedOperationV1>,
    operation: ProductionRankedOperationV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> R<()> {
    if resources.is_metered() {
        resources.push(operations, operation)
    } else {
        operations.push(operation);
        Ok(())
    }
}
fn push_prefix_value_v1(
    values: &mut Vec<ProductionRankedValueIdV1>,
    value: ProductionRankedValueIdV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> R<()> {
    if resources.is_metered() {
        resources.push(values, value)
    } else {
        values.push(value);
        Ok(())
    }
}

// Both adapters execute the original source-rank / 3 constants / axis order.
fn emit_reference_prefix_v1(
    output_ranks: impl IntoIterator<Item = usize>,
    values: &mut Vec<ProductionRankedValueIdV1>,
    entry_operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> R<()> {
    for rank in output_ranks {
        resources.work(16)?;
        for _ in 0..3 {
            resources.work(32)?;
            let result = next_value_id(next_value)?;
            push_prefix_value_v1(values, result, resources)?;
            push_prefix_operation_v1(
                entry_operations,
                ProductionRankedOperationV1::SemanticConstant { result, value: 0 },
                resources,
            )?;
        }
        for axis in 0..rank {
            resources.work(32)?;
            let symbol = u32::try_from(axis).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "reference-effect logical point rank does not fit the semantic symbol domain",
                )
            })?;
            let result = next_value_id(next_value)?;
            push_prefix_value_v1(values, result, resources)?;
            push_prefix_operation_v1(
                entry_operations,
                ProductionRankedOperationV1::SemanticSymbol { result, symbol },
                resources,
            )?;
        }
    }
    Ok(())
}

/// Component helper called ONLY while a joined factory lends the actual retained
/// input borrows. No caller count, zeroed allocator or prebuilt operation vector.
pub(super) fn prepare_root_entry_prefix_paid_v1(
    source_root: ProductionSourceLaunchRootV1,
    root_reference_bindings: &[AuthenticatedReferenceEffectBindingV1],
    pending: &mut RootEntryPrefixV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> R<()> {
    if !resources.is_metered() || resources.has_denial() {
        return Err(resource(Resource::Accounting));
    }
    resources.work(64)?;
    if !pending.entry_operations.is_empty()
        || pending.next_value != 0
        || pending.reserved_reference_values.is_some()
        || !pending.rank_scratch.is_empty()
    {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "actual root entry prefix cannot be replaced or retried",
        ));
    }
    // ExecutionLayout precedes reference validation, exactly as in ordinary code.
    push_prefix_operation_v1(
        &mut pending.entry_operations,
        ranked_execution_layout_v1(source_root.layout()),
        resources,
    )?;
    prepare_reference_prefix_paid_v1(root_reference_bindings, pending, resources)
}

// Shared paid reference stage: tests may call this with an inert preexisting
// operation prefix, but cannot construct the pipeline lexical input loan.
fn prepare_reference_prefix_paid_v1(
    root_reference_bindings: &[AuthenticatedReferenceEffectBindingV1],
    pending: &mut RootEntryPrefixV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> R<()> {
    if !resources.is_metered() || resources.has_denial() {
        return Err(resource(Resource::Accounting));
    }
    if root_reference_bindings.is_empty() {
        return Ok(());
    }
    resources.work(64)?;
    let writes =
        crate::production_reference_effect_join_v2::checked_reference_prefix_output_writes_v2(
            root_reference_bindings,
        )?;
    // Same validation order as original output_ranks: every coordinate is checked
    // before any SSA ID is allocated. Ranks are retained OUTER before allocation.
    for write in writes {
        resources.work(32)?;
        let rank = match &write.coordinate {
            ReferenceOutputCoordinateV1::LogicalPoint(axes) => axes.len(),
            _ => {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "reference-effect projection requires independently indexed logical point outputs",
                ));
            }
        };
        resources.push(&mut pending.rank_scratch, rank)?;
    }
    let mut count = 0usize;
    for rank in &pending.rank_scratch {
        resources.work(16)?;
        count = count
            .checked_add(3)
            .and_then(|n| n.checked_add(*rank))
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "reference-effect scalar reservation count overflowed",
            ))?;
    }
    pending.reserved_reference_values = Some(Vec::new());
    let values = pending
        .reserved_reference_values
        .as_mut()
        .expect("outer reference values installed");
    resources.reserve(values, count)?;
    emit_reference_prefix_v1(
        pending.rank_scratch.iter().copied(),
        values,
        &mut pending.entry_operations,
        &mut pending.next_value,
        resources,
    )?;
    if resources.has_denial() {
        return Err(resource(Resource::Accounting));
    }
    Ok(())
}

#[cfg(test)]
#[path = "root_entry_prefix_preparation_v1_tests.rs"]
mod tests;
