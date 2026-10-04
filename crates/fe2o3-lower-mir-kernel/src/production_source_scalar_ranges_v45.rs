// Original scalar windows are independently rederived during immutable replay.
// They describe initialized payload bytes, not pointer or active-tag authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceScalarByteRangeV45 {
    start: u64,
    end: u64,
}

type SourceScalarRangeFrameV45<'a> = (
    &'a SemanticFunctionDeclV1,
    &'a [SemanticTypeDeclV1],
    &'a SemanticPlaceV1,
    &'a SemanticTypeDeclV1,
    &'a [SemanticTypeIdV1],
    &'a [u64],
    std::slice::Iter<'a, SemanticProjectionV1>,
    &'a SemanticProjectionV1,
    SemanticTypeIdV1,
    SemanticTypeIdV1,
    SourceScalarByteRangeV45,
    u64,
    u64,
    u64,
);

fn source_scalar_range_v45(
    original: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    place: &SemanticPlaceV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(SemanticTypeIdV1, SourceScalarByteRangeV45), ProductionSemanticKirErrorV1> {
    let mut output = None;
    with_canonical_call_scratch_v1(budget, |budget| {
        budget.reserve_storage(source_reference_emission_headers_v29::<
            SourceScalarRangeFrameV45<'_>,
        >()?)?;
        budget.reserve_storage(source_reference_emission_headers_v29::<(
            SemanticTypeIdV1,
            SourceScalarByteRangeV45,
        )>()?)?;
        budget.charge_work(6)?;
        let root = original
            .locals()
            .get(place.local().index() as usize)
            .ok_or_else(scoped_memory_error_v29)?
            .ty();
        let mut ty = root;
        let mut start = 0u64;
        for projection in place.projections() {
            budget.charge_work(9)?;
            let declaration = types
                .get(ty.index() as usize)
                .ok_or_else(scoped_memory_error_v29)?;
            require_ordinary_execution_representation_v29(declaration)?;
            let (child, displacement) = match (declaration.shape(), projection.kind()) {
                (
                    SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields),
                    SemanticProjectionKindV1::Field(field),
                ) => {
                    let offsets = declaration
                        .layout()
                        .fields()
                        .source_order_offsets_bytes()
                        .ok_or_else(scoped_memory_error_v29)?;
                    if offsets.len() != fields.fields().len() {
                        return Err(scoped_memory_error_v29());
                    }
                    (
                        *fields
                            .fields()
                            .get(field as usize)
                            .ok_or_else(scoped_memory_error_v29)?,
                        *offsets
                            .get(field as usize)
                            .ok_or_else(scoped_memory_error_v29)?,
                    )
                }
                (
                    SemanticTypeShapeV1::Array { element, length },
                    SemanticProjectionKindV1::ConstantIndex {
                        offset,
                        minimum_length,
                        from_end,
                    },
                ) => {
                    let SemanticFieldsShapeV1::Array {
                        stride_bytes,
                        count,
                    } = declaration.layout().fields()
                    else {
                        return Err(scoped_memory_error_v29());
                    };
                    if count != length {
                        return Err(scoped_memory_error_v29());
                    }
                    let index = source_static_constant_index_v29(
                        *length,
                        offset,
                        minimum_length,
                        from_end,
                    )?;
                    (
                        *element,
                        stride_bytes
                            .checked_mul(index)
                            .ok_or(ArgumentResourceV1::Arithmetic)?,
                    )
                }
                _ => {
                    return Err(source_reference_error_v29(
                        "source scalar history requires an exact static nonpointer path",
                    ));
                }
            };
            let child_bytes = types
                .get(child.index() as usize)
                .and_then(|child| child.layout().size_bytes())
                .ok_or_else(scoped_memory_error_v29)?;
            if child != projection.result_type()
                || displacement
                    .checked_add(child_bytes)
                    .ok_or(ArgumentResourceV1::Arithmetic)?
                    > declaration
                        .layout()
                        .size_bytes()
                        .ok_or_else(scoped_memory_error_v29)?
            {
                return Err(scoped_memory_error_v29());
            }
            start = start
                .checked_add(displacement)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            ty = child;
        }
        budget.charge_work(6)?;
        let leaf = types
            .get(ty.index() as usize)
            .ok_or_else(scoped_memory_error_v29)?;
        require_ordinary_execution_representation_v29(leaf)?;
        if ty != place.ty()
            || !matches!(
                leaf.shape(),
                SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
            )
        {
            return Err(source_reference_error_v29(
                "source scalar history leaf is not an original scalar",
            ));
        }
        let bytes = leaf
            .layout()
            .size_bytes()
            .ok_or_else(scoped_memory_error_v29)?;
        let end = start
            .checked_add(bytes)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if bytes == 0
            || end
                > types[root.index() as usize]
                    .layout()
                    .size_bytes()
                    .ok_or_else(scoped_memory_error_v29)?
        {
            return Err(scoped_memory_error_v29());
        }
        output = Some((root, SourceScalarByteRangeV45 { start, end }));
        Ok(())
    })?;
    output.ok_or(ArgumentResourceV1::Accounting.into())
}
