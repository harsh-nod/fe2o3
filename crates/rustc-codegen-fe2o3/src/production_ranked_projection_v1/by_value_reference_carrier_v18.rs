// A scalar ABI component of a by-value aggregate is not an allocation contract
// for that aggregate. Keep nested safe references pending, without discarding
// or strengthening their original rustc provenance.
fn source_argument_allocation_contract_v18(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    ownership: SemanticSourceArgumentOwnershipV1,
    argument: &fe2o3_mir_model::semantic_mir_v1::SemanticAbiArgumentV1,
    pointee: fe2o3_mir_model::semantic_mir_v1::SemanticAbiPointeeInfoV1,
    contract: AllocationContractV1,
    allocation: &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18<'_>,
) -> Result<Option<AllocationContractV1>, ProductionRankedProjectionErrorV1> {
    allocation.header::<Option<AllocationContractV1>>()?;
    allocation
        .header::<Result<Option<AllocationContractV1>, ProductionRankedProjectionErrorV1>>()?;
    allocation.header::<Result<AllocationContractV1, ProductionRankedProjectionErrorV1>>()?;
    allocation.header::<Option<&SemanticTypeDeclV1>>()?;
    allocation.header::<Option<&SemanticTypeShapeV1>>()?;
    allocation.charge(6)?;
    if ownership == SemanticSourceArgumentOwnershipV1::ByValue
        && matches!(
            pointee.kind(),
            SemanticAbiPointeeKindV1::SharedReference { .. }
                | SemanticAbiPointeeKindV1::MutableReference { .. }
        )
        && matches!(
            types
                .get(ty.index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Tuple(_) | SemanticTypeShapeV1::Aggregate(_))
        )
    {
        check_by_value_reference_carrier_v18(types, ty, argument, pointee, allocation)?;
        return Ok(None);
    }
    Ok(Some(authenticated_source_allocation_contract_v1(
        ownership,
        pointee.kind(),
        contract,
    )?))
}

fn check_by_value_reference_carrier_v18(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    argument: &fe2o3_mir_model::semantic_mir_v1::SemanticAbiArgumentV1,
    pointee: fe2o3_mir_model::semantic_mir_v1::SemanticAbiPointeeInfoV1,
    allocation: &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18<'_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAbiArgumentRoleV1, SemanticAggregateTypeV1, SemanticBackendScalarV1,
        SemanticPointerTypeV1, SemanticRustcVariantsV1,
    };
    allocation.header::<Result<(), ProductionRankedProjectionErrorV1>>()?;
    allocation.header::<(
        Option<&SemanticTypeDeclV1>,
        &SemanticTypeDeclV1,
        &SemanticTypeDeclV1,
        Option<&SemanticTypeDeclV1>,
        Option<&[u64]>,
        &[u64],
        &SemanticBackendScalarV1,
        &SemanticAggregateTypeV1,
        &SemanticPointerTypeV1,
        Option<SemanticTypeIdV1>,
        SemanticTypeIdV1,
        Option<SemanticBackendScalarV1>,
        usize,
        usize,
        std::iter::Enumerate<std::slice::Iter<'_, SemanticTypeIdV1>>,
        Result<&SemanticTypeDeclV1, ProductionRankedProjectionErrorV1>,
        Result<&[u64], ProductionRankedProjectionErrorV1>,
        Result<SemanticTypeIdV1, ProductionRankedProjectionErrorV1>,
        Result<usize, ProductionRankedProjectionErrorV1>,
    )>()?;
    allocation.charge(8)?;
    let refusal = || {
        ProductionRankedProjectionErrorV1::Unsupported(
            "by-value safe-reference carrier has no exact scalar ABI leaf",
        )
    };
    if argument.role() != SemanticAbiArgumentRoleV1::Source
        || argument.value().source_ty() != ty
        || argument.value().adjusted().is_some()
        || argument.value().pointee_override().is_some()
        || !matches!(argument.mode(), SemanticAbiPassModeV1::Direct(_))
    {
        return Err(refusal());
    }
    let mut current = ty;
    let mut expected = None;
    let mut remaining = 256usize;
    loop {
        allocation.charge(12)?;
        remaining = remaining.checked_sub(1).ok_or_else(refusal)?;
        let row = types.get(current.index() as usize).ok_or_else(refusal)?;
        let SemanticBackendReprV1::Scalar(scalar) = row.layout().backend_repr() else {
            return Err(refusal());
        };
        if !matches!(
            scalar,
            SemanticBackendScalarV1::Initialized {
                primitive: SemanticBackendPrimitiveV1::Pointer {
                    address_space: 0,
                    size_bytes: 8,
                    alignment_bytes: 8
                },
                ..
            }
        ) || row.layout().size_bytes() != Some(8)
            || row.layout().alignment_bytes() != 8
            || row.layout().is_uninhabited()
            || !matches!(
                row.layout().variants(),
                SemanticRustcVariantsV1::Single { index: 0 }
            )
            || row.abi_properties().first_pointee() != Some(pointee)
            || row.abi_properties().second_pointee().is_some()
            || expected.is_some_and(|expected| expected != *scalar)
        {
            return Err(refusal());
        }
        expected = Some(*scalar);
        match row.shape() {
            SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                let offsets = row
                    .layout()
                    .fields()
                    .source_order_offsets_bytes()
                    .ok_or_else(refusal)?;
                if offsets.len() != fields.fields().len() {
                    return Err(refusal());
                }
                let mut next = None;
                for (index, &field) in fields.fields().iter().enumerate() {
                    allocation.charge(6)?;
                    remaining = remaining.checked_sub(1).ok_or_else(refusal)?;
                    let child = types.get(field.index() as usize).ok_or_else(refusal)?;
                    match child.layout().size_bytes() {
                        Some(0)
                            if offsets[index] <= 8
                                && child.layout().alignment_bytes() <= 8
                                && !child.layout().is_uninhabited()
                                && child.abi_properties().first_pointee().is_none()
                                && child.abi_properties().second_pointee().is_none() => {}
                        Some(8) if offsets[index] == 0 && next.is_none() => next = Some(field),
                        _ => return Err(refusal()),
                    }
                }
                current = next.ok_or_else(refusal)?;
            }
            SemanticTypeShapeV1::Pointer(pointer) => {
                allocation.charge(4)?;
                if current == ty
                    || pointer.kind() != SemanticPointerKindV1::Reference
                    || pointer.metadata() != SemanticPointerMetadataV1::None
                    || pointer.address_space() != 0
                    || pointer.pointer_width_bits() != 64
                    || !matches!(
                        (pointer.mutability(), pointee.kind()),
                        (
                            SemanticMutabilityV1::Immutable,
                            SemanticAbiPointeeKindV1::SharedReference { .. }
                        ) | (
                            SemanticMutabilityV1::Mutable,
                            SemanticAbiPointeeKindV1::MutableReference { .. }
                        )
                    )
                {
                    return Err(refusal());
                }
                return Ok(());
            }
            _ => return Err(refusal()),
        }
    }
}
