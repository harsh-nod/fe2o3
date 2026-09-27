use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirBlockRefV1, CanonicalKirDefinitionRefV1, CanonicalKirFunctionRefV1,
    CanonicalKirKernelRefV1, CanonicalKirOperationRefV1,
};
use fe2o3_kernel_ir::{
    Axis, CanonicalKirDefinitionCoordinateV1, FunctionRole, IndexKind, IntrinsicKind,
    IntrinsicOperation, ValueDef,
};

fn headers() -> R<usize> {
    use std::mem::size_of;
    fn h<T>() -> usize {
        size_of::<T>() + size_of::<Option<T>>() + 2 * size_of::<R<T>>()
    }
    [
        2 * h::<&CanonicalKirInventoryV1<'_>>(),
        2 * h::<&CanonicalKirOperationRefV1<'_>>(),
        h::<&CanonicalKirFunctionRefV1<'_>>(),
        h::<&CanonicalKirBlockRefV1<'_>>(),
        h::<&CanonicalKirDefinitionRefV1<'_>>(),
        h::<&CanonicalKirKernelRefV1<'_>>(),
        2 * h::<&IntrinsicOperation>(),
        h::<&ValueDef>(),
        h::<&[ValueDef]>(),
        h::<&[CanonicalKirOperationRefV1<'_>]>(),
        h::<&[CanonicalKirFunctionRefV1<'_>]>(),
        h::<&[CanonicalKirBlockRefV1<'_>]>(),
        h::<&[CanonicalKirDefinitionRefV1<'_>]>(),
        h::<&std::ops::Range<usize>>(),
        h::<&OperationKind>(),
        h::<&Type>(),
        h::<Type>(),
        h::<IntrinsicKind>(),
        h::<fe2o3_kernel_ir::ValueId>(),
        h::<fe2o3_kernel_ir::CompilerOrderingEffectSummaryV12>(),
        h::<&[CanonicalKirKernelRefV1<'_>]>(),
        h::<std::slice::Iter<'_, CanonicalKirKernelRefV1<'_>>>(),
        h::<bool>(),
        h::<usize>(),
        std::mem::size_of::<CanonicalKirDefinitionCoordinateV1>(),
        2 * std::mem::size_of::<CanonicalKirOperationCoordinateV1>(),
        2 * std::mem::size_of::<&mut AssertOriginBudgetV1<'_>>(),
        8 * std::mem::size_of::<usize>(),
        2 * std::mem::size_of::<R<()>>(),
        2 * std::mem::size_of::<Result<(), AssertOriginResourceV1>>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or_else(arithmetic)
    })
}

// This classifies total scalar syntax only. Source replay, exact target-bound
// coordinates, transition payload equality and final memory proofs stay at the
// consuming admission boundary. No range/uniformity fact leaves this query.
pub(super) fn native(
    inventory: &CanonicalKirInventoryV1<'_>,
    ordinal: usize,
    row: &CanonicalKirOperationRefV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<bool> {
    // The constant-time borrowed identity/kind preflight precedes all debits,
    // like the enclosing exact-owner guards. Successful selected queries pay
    // its representations as part of headers(); it grants no retained fact.
    if !inventory
        .operations()
        .get(ordinal)
        .is_some_and(|actual| std::ptr::eq(actual, row))
    {
        return Err(refused("launch scalar", "same borrowed operation site"));
    }
    let OperationKind::Intrinsic(intrinsic) = &row.operation.kind else {
        return Ok(false);
    };
    if !matches!(
        intrinsic.kind,
        IntrinsicKind::InvocationIndex {
            kind: IndexKind::Local | IndexKind::Workgroup,
            axis: Axis::X,
        }
    ) {
        return Ok(false);
    }
    let bytes = headers()?;
    budget.reserve_storage(bytes).map_err(E::Resource)?;
    let result = check(inventory, ordinal, row, intrinsic, budget);
    budget.release_storage(bytes).map_err(E::Resource)?;
    result
}

fn check(
    inventory: &CanonicalKirInventoryV1<'_>,
    ordinal: usize,
    row: &CanonicalKirOperationRefV1<'_>,
    intrinsic: &IntrinsicOperation,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<bool> {
    // One fixed schema/coordinate join; kernel comparisons are paid separately.
    charge(budget, 64)?;
    let [result] = row.operation.results.as_slice() else {
        return Ok(false);
    };
    if intrinsic.result_type != Type::INDEX
        || result.ty != Type::INDEX
        || row.results.len() != 1
        || !row.operands.is_empty()
        || !row.effects.is_empty()
        || !row.compiler_ordering().is_empty()
    {
        return Ok(false);
    }
    let function = inventory
        .functions()
        .get(row.coordinate.block.function.0 as usize)
        .ok_or_else(|| refused("launch scalar", "exact function coordinate"))?;
    if function.coordinate != row.coordinate.block.function
        || function.function.role != FunctionRole::KernelEntry
        || !function.operations.contains(&ordinal)
    {
        return Ok(false);
    }
    let block_index = function
        .blocks
        .start
        .checked_add(row.coordinate.block.block as usize)
        .ok_or_else(arithmetic)?;
    let block = inventory
        .blocks()
        .get(block_index)
        .ok_or_else(|| refused("launch scalar", "exact block coordinate"))?;
    if !function.blocks.contains(&block_index)
        || block.coordinate != row.coordinate.block
        || block
            .operations
            .start
            .checked_add(row.coordinate.operation as usize)
            != Some(ordinal)
        || !block.operations.contains(&ordinal)
    {
        return Ok(false);
    }
    let definition = inventory
        .definitions()
        .get(row.results.start)
        .ok_or_else(|| refused("launch scalar", "exact result definition"))?;
    if definition.coordinate
        != (CanonicalKirDefinitionCoordinateV1::Result {
            operation: row.coordinate,
            result: 0,
        })
        || definition.value != Some(result.id)
        || *definition.ty != Type::INDEX
    {
        return Ok(false);
    }
    let mut bindings = 0usize;
    for kernel in inventory.kernels() {
        charge(budget, 2)?;
        if kernel.entry == function.coordinate {
            bindings = bindings.checked_add(1).ok_or_else(arithmetic)?;
        }
    }
    Ok(bindings == 1)
}

#[cfg(test)]
#[path = "production_checked_output_invocation_indices_v1_tests.rs"]
mod tests;
