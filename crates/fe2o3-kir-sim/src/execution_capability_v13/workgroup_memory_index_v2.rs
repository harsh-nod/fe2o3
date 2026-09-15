use super::*;

#[cfg(test)]
mod tests;

pub(super) fn issue(
    mut results: Vec<ValueDef>,
    workgroup: Option<fe2o3_kernel_ir::WorkgroupSize>,
    promoted: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
) -> Result<Vec<Operation>, ExecutionCapabilityProjectionErrorV13> {
    let error = || {
        ExecutionCapabilityProjectionErrorV13::Incomplete(
            IncompleteExecutionCapabilityOperationV13::WorkgroupMemoryIndexV2,
        )
    };
    let witness = sole_logical_result_mut(&mut results).ok_or_else(error)?;
    let size = workgroup
        .filter(|size| {
            size.x != 0
                && size.y != 0
                && size.z != 0
                && u64::from(size.x)
                    .checked_mul(u64::from(size.y))
                    .and_then(|xy| xy.checked_mul(u64::from(size.z)))
                    .is_some()
        })
        .ok_or_else(error)?;
    let mut emitter = Emitter::new(next_value);
    let rank = emit_flat_local_rank(&mut emitter, size)?;
    let zero = index_constant(&mut emitter, 0)?;
    emitter.with_id(
        witness.id,
        Type::INDEX,
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: rank,
            rhs: zero,
        },
    )?;
    promoted.insert(witness.id, Type::INDEX);
    Ok(emitter.finish())
}

pub(super) fn into_disjoint(
    mut results: Vec<ValueDef>,
    contract: &ExecutionCapabilityOpV1,
    types: &BTreeMap<ValueId, Type>,
    aliases: &mut Vec<(ValueId, ValueId)>,
    promoted: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
) -> Result<Vec<Operation>, ExecutionCapabilityProjectionErrorV13> {
    let error = || {
        ExecutionCapabilityProjectionErrorV13::Incomplete(
            IncompleteExecutionCapabilityOperationV13::WorkgroupMemoryIndexIntoDisjoint,
        )
    };
    let witness = sole_logical_result_mut(&mut results).ok_or_else(error)?;
    let physical = physical_operands(contract, types, aliases, promoted)?;
    let [rank] = physical.as_slice() else {
        return Err(error());
    };
    let [input] = contract.operands.as_slice() else {
        return Err(error());
    };
    if promoted.get(input).or_else(|| promoted.get(rank)) != Some(&Type::INDEX) {
        return Err(error());
    }
    let mut emitter = Emitter::new(next_value);
    let zero = index_constant(&mut emitter, 0)?;
    emitter.with_id(
        witness.id,
        Type::INDEX,
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: *rank,
            rhs: zero,
        },
    )?;
    promoted.insert(witness.id, Type::INDEX);
    Ok(emitter.finish())
}

pub(super) fn prepare_parameters(
    body: &mut fe2o3_kernel_ir::FunctionBody,
    promoted: &mut BTreeMap<ValueId, Type>,
) {
    // The source module is verified before projection. Only this physical
    // index role changes representation; all incoming edge identities remain.
    for parameter in body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.parameters)
    {
        if matches!(&parameter.ty, Type::ExecutionCapability(capability)
            if capability.role == fe2o3_kernel_ir::ExecutionCapabilityRoleV1::WorkgroupMemoryIndex)
        {
            promoted.insert(parameter.id, Type::INDEX);
            parameter.ty = Type::INDEX;
        }
    }
}
