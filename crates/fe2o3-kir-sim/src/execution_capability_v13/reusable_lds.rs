use super::*;

#[cfg(test)]
mod tests;

pub(super) fn project(
    results: Vec<ValueDef>,
    contract: &ExecutionCapabilityOpV1,
    conversion: fe2o3_kernel_ir::ReusableLdsConversionV1,
    types: &BTreeMap<ValueId, Type>,
    aliases: &mut Vec<(ValueId, ValueId)>,
    promoted: &mut BTreeMap<ValueId, Type>,
) -> Result<Vec<Operation>, ExecutionCapabilityProjectionErrorV13> {
    let error = || {
        ExecutionCapabilityProjectionErrorV13::Invalid(
            "reusable LDS lost its exact allocated handle or epoch",
        )
    };
    let [input] = contract.operands.as_slice() else {
        return Err(error());
    };
    let [result] = results.as_slice() else {
        return Err(error());
    };
    let Some(Type::ExecutionCapability(input_type)) = types.get(input) else {
        return Err(error());
    };
    let expected = conversion.output_type(input_type).ok_or_else(error)?;
    if result.ty != Type::ExecutionCapability(expected)
        || input_type.provenance != contract.provenance
        || input_type.workgroup_brand != contract.workgroup_brand
        || input_type.epoch != contract.epoch_before
        || contract.epoch_after.is_some()
        || aliases
            .iter()
            .any(|(from, to)| *from == result.id || *to == *input)
    {
        return Err(error());
    }
    // The canonical verifier requires a direct, singly consumed allocation.
    // Do not resolve a guessed alias or replace an unused logical result.
    let Some(pointer @ Type::Pointer(p)) = promoted.get(input) else {
        return Err(error());
    };
    if p.address_space != AddressSpace::Workgroup || p.access != AccessMode::ReadWrite {
        return Err(error());
    }
    let pointer = pointer.clone();
    aliases
        .try_reserve(1)
        .map_err(|_| ExecutionCapabilityProjectionErrorV13::AllocationFailure)?;
    // The input remains the physical allocation definition, so retain its
    // physical type. That type cache is not a live ownership token.
    promoted.insert(result.id, pointer);
    aliases.push((result.id, *input));
    Ok(Vec::new())
}
