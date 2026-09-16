fn project_address_formation(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    block_index: usize,
    place: &SemanticPlaceV1,
    local_contracts: &ProjectionLocalContractsV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let local_index = place.local().index() as usize;
    let local = function.locals().get(local_index).ok_or(
        ProductionRankedProjectionErrorV1::Unsupported(
            "an address formation with an out-of-range local",
        ),
    )?;
    if local_contracts
        .atomic_allocations
        .permits_address(block_index, place)?
    {
        return Ok(());
    }
    if local_contracts
        .atomic_allocations
        .contains_local(place.local())?
        && zero_atomic_field_v1(types, function, place)
    {
        return Ok(());
    }
    if place.projections().is_empty() {
        // Taking the address of a MIR local creates a private address and does
        // not observe the value stored in that local.
        return Ok(());
    }

    let mut current = local.ty();
    let mut crossed_dereference = false;
    for (projection_index, projection) in place.projections().iter().enumerate() {
        match projection.kind() {
            SemanticProjectionKindV1::Dereference if projection_index == 0 => {
                let ty = types.get(current.index() as usize).ok_or(
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "an address formation with an out-of-range type",
                    ),
                )?;
                let SemanticTypeShapeV1::Pointer(pointer) = ty.shape() else {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "an address formation whose dereferenced type is not a pointer",
                    ));
                };
                memory_space(pointer.address_space())?;
                crossed_dereference = true;
            }
            SemanticProjectionKindV1::Field(_)
            | SemanticProjectionKindV1::Downcast(_)
            | SemanticProjectionKindV1::OpaqueCast
            | SemanticProjectionKindV1::Subtype => {}
            SemanticProjectionKindV1::Dereference
            | SemanticProjectionKindV1::Index(_)
            | SemanticProjectionKindV1::ConstantIndex { .. }
            | SemanticProjectionKindV1::Subslice { .. } => {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "indexed address formation before exact bounds-only projection",
                ));
            }
        }
        current = projection.result_type();
    }

    if crossed_dereference {
        debug_assert_eq!(reborrowed_allocation_local_v1(place), Some(place.local()));
        let has_allocation_contract = local_contracts
            .allocations
            .get(local_index)
            .copied()
            .flatten()
            .is_some();
        let has_private_provenance = matches!(
            local_contracts
                .allocation_provenance
                .get(local_index)
                .copied()
                .flatten(),
            Some(LocalAllocationProvenanceV1::Private(_))
        );
        if !has_allocation_contract && !has_private_provenance {
            return Err(
                ProductionRankedProjectionErrorV1::MissingAllocationProvenance {
                    local: place.local().index(),
                    projections: place.projections().len(),
                    ty: local.ty().index(),
                },
            );
        }
    }
    Ok(())
}
