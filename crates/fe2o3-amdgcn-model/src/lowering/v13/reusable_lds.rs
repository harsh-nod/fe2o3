use super::*;

#[cfg(test)]
mod tests;

pub(super) fn lower(
    module: &Module,
    operation: &Operation,
    contract: &ExecutionCapabilityOpV1,
    conversion: fe2o3_kernel_ir::ReusableLdsConversionV1,
    original_types: &BTreeMap<ValueId, Type>,
    lowered_types: &mut BTreeMap<ValueId, Type>,
    aliases: &mut BTreeMap<ValueId, ExecutionAliasV1>,
) -> Result<(), LoweringErrors> {
    let error = || {
        incomplete(
            module,
            "reusable LDS requires the exact previously allocated physical handle",
        )
    };
    let [input] = contract.operands.as_slice() else {
        return Err(error());
    };
    let [result] = operation.results.as_slice() else {
        return Err(error());
    };
    let Some(Type::ExecutionCapability(input_type)) = original_types.get(input) else {
        return Err(error());
    };
    let expected = conversion.output_type(input_type).ok_or_else(error)?;
    let Some(ExecutionAliasV1::Physical(physical)) = aliases.get(input).copied() else {
        return Err(error());
    };
    if result.ty != Type::ExecutionCapability(expected)
        || input_type.provenance != contract.provenance
        || input_type.epoch != contract.epoch_before
        || input_type.workgroup_brand != contract.workgroup_brand
        || contract.epoch_after.is_some()
        || physical.extent != Some(PhysicalExtentV1::Static(conversion.elements))
        || aliases.contains_key(&result.id)
    {
        return Err(error());
    }
    let Some(pointer @ Type::Pointer(p)) = lowered_types.get(&physical.value) else {
        return Err(error());
    };
    if p.address_space != fe2o3_kernel_ir::AddressSpace::Workgroup
        || p.access != AccessMode::ReadWrite
    {
        return Err(error());
    }
    let pointer = pointer.clone();
    // Transfer the existing physical view, including its exact extent. Never
    // create storage or emit a write/barrier for this ownership-only operation.
    aliases.remove(input);
    aliases.insert(result.id, ExecutionAliasV1::Physical(physical));
    lowered_types.insert(result.id, pointer);
    Ok(())
}
