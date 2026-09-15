use super::*;

#[cfg(test)]
mod tests;

pub(super) fn is_index(ty: &Type) -> bool {
    matches!(ty, Type::ExecutionCapability(capability)
        if capability.role == ExecutionCapabilityRoleV1::WorkgroupMemoryIndex)
}

pub(super) fn prepare_parameters(
    body: &mut fe2o3_kernel_ir::FunctionBody,
    types: &mut BTreeMap<ValueId, Type>,
    aliases: &mut BTreeMap<ValueId, ExecutionAliasV1>,
) {
    for parameter in body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.parameters)
    {
        if is_index(&parameter.ty) {
            types.insert(parameter.id, Type::INDEX);
            aliases.insert(
                parameter.id,
                ExecutionAliasV1::Physical(PhysicalValueV1::scalar(parameter.id)),
            );
            parameter.ty = Type::INDEX;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    module: &Module,
    function: &Function,
    operation: &Operation,
    contract: &ExecutionCapabilityOpV1,
    original_types: &BTreeMap<ValueId, Type>,
    types: &mut BTreeMap<ValueId, Type>,
    aliases: &mut BTreeMap<ValueId, ExecutionAliasV1>,
    next_value: &mut u32,
    output: &mut Vec<Operation>,
) -> Result<(), LoweringErrors> {
    let result = sole_capability_result(module, operation)?;
    let rank = match &contract.operation {
        ExecutionCapabilityOperationV1::WorkgroupMemoryIndexV2 { .. } => {
            let size = static_workgroup_size(module, &function.id)?;
            if size.x == 0
                || size.y == 0
                || size.z == 0
                || u64::from(size.x)
                    .checked_mul(u64::from(size.y))
                    .and_then(|xy| xy.checked_mul(u64::from(size.z)))
                    .is_none()
            {
                return Err(incomplete(
                    module,
                    "scoped index geometry exceeds its rank range",
                ));
            }
            emit_flat_local_index(module, size, types, next_value, output)?
        }
        ExecutionCapabilityOperationV1::WorkgroupMemoryIndexIntoDisjoint { .. } => {
            let physical = physical_operands(contract, original_types, types, aliases);
            let [(rank, Type::Scalar(ScalarType::Index))] = physical.as_slice() else {
                return Err(incomplete(
                    module,
                    "scoped index conversion lost its one physical index",
                ));
            };
            *rank
        }
        _ => return Err(incomplete(module, "not a scoped workgroup index operation")),
    };
    // A physical copy retains the logical result's SSA name, including phi uses.
    let zero = emit_constant_index(module, 0, types, next_value, output)?;
    emit_binary(
        module,
        BinaryOp::Add,
        rank,
        zero,
        Type::INDEX,
        Some(result.id),
        types,
        next_value,
        output,
    )?;
    aliases.insert(
        result.id,
        ExecutionAliasV1::Physical(PhysicalValueV1::scalar(result.id)),
    );
    Ok(())
}
