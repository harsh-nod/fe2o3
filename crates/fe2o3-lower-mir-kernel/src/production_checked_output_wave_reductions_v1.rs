use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirBlockRefV1, CanonicalKirDefinitionRefV1, CanonicalKirFunctionRefV1,
    CanonicalKirKernelRefV1, CanonicalKirOperationRefV1, CanonicalKirUseRefV1,
};
use fe2o3_kernel_ir::{
    CanonicalKirDefinitionCoordinateV1, CanonicalKirUseCoordinateV1, FunctionRole,
    SynchronizationScope, ValueDef, ValueId, WaveF32ReductionKindV1, WaveOperation,
    WaveOperationKind, WaveWidth,
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
        3 * h::<&CanonicalKirDefinitionRefV1<'_>>(),
        2 * h::<&CanonicalKirUseRefV1>(),
        h::<&CanonicalKirKernelRefV1<'_>>(),
        2 * h::<&WaveOperation>(),
        h::<&ValueDef>(),
        h::<&[ValueDef]>(),
        h::<&[CanonicalKirOperationRefV1<'_>]>(),
        h::<&[CanonicalKirFunctionRefV1<'_>]>(),
        h::<&[CanonicalKirBlockRefV1<'_>]>(),
        h::<&[CanonicalKirDefinitionRefV1<'_>]>(),
        h::<&[CanonicalKirUseRefV1]>(),
        h::<&[CanonicalKirKernelRefV1<'_>]>(),
        h::<std::slice::Iter<'_, CanonicalKirKernelRefV1<'_>>>(),
        2 * h::<&std::ops::Range<usize>>(),
        h::<&OperationKind>(),
        3 * h::<&Type>(),
        h::<Type>(),
        h::<ScalarType>(),
        h::<WaveOperationKind>(),
        h::<WaveF32ReductionKindV1>(),
        h::<WaveWidth>(),
        h::<fe2o3_kernel_ir::Convergence>(),
        h::<SynchronizationScope>(),
        2 * h::<ValueId>(),
        h::<(ValueId, u32, Option<ValueId>)>(),
        h::<u32>(),
        h::<bool>(),
        h::<usize>(),
        h::<fe2o3_kernel_ir::CompilerOrderingEffectSummaryV12>(),
        h::<CanonicalKirDefinitionCoordinateV1>(),
        h::<CanonicalKirUseCoordinateV1>(),
        2 * h::<CanonicalKirOperationCoordinateV1>(),
        2 * h::<&mut AssertOriginBudgetV1<'_>>(),
        9 * size_of::<usize>(),
        2 * size_of::<R<()>>(),
        2 * size_of::<Result<(), AssertOriginResourceV1>>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or_else(arithmetic)
    })
}

// This is an exact target-neutral opcode census, not a convergence, numerical
// rewrite, or launch proof. Source/N/B/O transport and the native target's
// full-wave/control and Broadcast lane-bound preflights remain mandatory.
// Wave remains ordered/non-pure; this query grants no range fact.
pub(super) fn native(
    inventory: &CanonicalKirInventoryV1<'_>,
    ordinal: usize,
    row: &CanonicalKirOperationRefV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<bool> {
    if !inventory
        .operations()
        .get(ordinal)
        .is_some_and(|actual| std::ptr::eq(actual, row))
    {
        return Err(refused("wave reduction", "same borrowed operation site"));
    }
    let OperationKind::Wave(wave) = &row.operation.kind else {
        return Ok(false);
    };
    if !matches!(
        wave.kind,
        WaveOperationKind::ReduceF32 { .. } | WaveOperationKind::BroadcastF32 { .. }
    ) {
        return Ok(false);
    }
    let bytes = headers()?;
    budget.reserve_storage(bytes).map_err(E::Resource)?;
    let result = check(inventory, ordinal, row, wave, budget);
    budget.release_storage(bytes).map_err(E::Resource)?;
    result
}

fn check(
    inventory: &CanonicalKirInventoryV1<'_>,
    ordinal: usize,
    row: &CanonicalKirOperationRefV1<'_>,
    wave: &WaveOperation,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<bool> {
    charge(budget, 96)?;
    let (value, tile_width, source_lane) = match wave.kind {
        WaveOperationKind::ReduceF32 {
            value,
            tile_width,
            kind: WaveF32ReductionKindV1::Sum | WaveF32ReductionKindV1::Maximum,
        } => (value, tile_width, None),
        WaveOperationKind::BroadcastF32 {
            value,
            source_lane,
            tile_width,
        } => (value, tile_width, Some(source_lane)),
        _ => return Ok(false),
    };
    let [result] = row.operation.results.as_slice() else {
        return Ok(false);
    };
    if wave.width != WaveWidth::Wave64
        || wave.active_lanes != 64
        || wave.convergence.scope() != SynchronizationScope::Subgroup
        || tile_width == 0
        || !tile_width.is_power_of_two()
        || tile_width > 64
        || result.ty != Type::F32
        || row.results.len() != 1
        || row.operands.len() != if source_lane.is_some() { 2 } else { 1 }
        || !row.effects.is_empty()
        || !row.compiler_ordering().is_empty()
    {
        return Ok(false);
    }
    let function = inventory
        .functions()
        .get(row.coordinate.block.function.0 as usize)
        .ok_or_else(|| refused("wave reduction", "exact function coordinate"))?;
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
        .ok_or_else(|| refused("wave reduction", "exact block coordinate"))?;
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
        .ok_or_else(|| refused("wave reduction", "exact result definition"))?;
    if definition.coordinate
        != (CanonicalKirDefinitionCoordinateV1::Result {
            operation: row.coordinate,
            result: 0,
        })
        || definition.value != Some(result.id)
        || *definition.ty != Type::F32
    {
        return Ok(false);
    }
    let operand = inventory
        .uses()
        .get(row.operands.start)
        .ok_or_else(|| refused("wave reduction", "exact operand occurrence"))?;
    let input = inventory
        .definitions()
        .get(operand.definition)
        .ok_or_else(|| refused("wave reduction", "exact input definition"))?;
    if operand.coordinate
        != (CanonicalKirUseCoordinateV1::OperationOperand {
            operation: row.coordinate,
            operand: 0,
        })
        || operand.value != value
        || input.value != Some(value)
        || *input.ty != Type::F32
        || !function.definitions.contains(&operand.definition)
    {
        return Ok(false);
    }
    if let Some(source_lane) = source_lane {
        charge(budget, 32)?;
        let operand_index = row.operands.start.checked_add(1).ok_or_else(arithmetic)?;
        let lane_operand = inventory
            .uses()
            .get(operand_index)
            .ok_or_else(|| refused("wave reduction", "exact broadcast lane occurrence"))?;
        let lane_input = inventory
            .definitions()
            .get(lane_operand.definition)
            .ok_or_else(|| refused("wave reduction", "exact broadcast lane definition"))?;
        if lane_operand.coordinate
            != (CanonicalKirUseCoordinateV1::OperationOperand {
                operation: row.coordinate,
                operand: 1,
            })
            || lane_operand.value != source_lane
            || lane_input.value != Some(source_lane)
            || *lane_input.ty != Type::Scalar(ScalarType::U32)
            || !function.definitions.contains(&lane_operand.definition)
        {
            return Ok(false);
        }
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
#[path = "production_checked_output_wave_reductions_v1_tests.rs"]
mod tests;
