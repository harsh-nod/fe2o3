// These records are inert signatures. Source custody, exact effects, and the
// current-writer publication theorem must be proved by downstream owners.
fn is_static_publication_intrinsic_v31(operation: SemanticCompilerIntrinsicOperationV1) -> bool {
    matches!(
        operation,
        SemanticCompilerIntrinsicOperationV1::StaticPublication128PublishF32 { .. }
            | SemanticCompilerIntrinsicOperationV1::StaticPublication128TryReadF32 { .. }
    )
}

fn static_publication_result_fields_v31(
    request: &InertSemanticMirRequestV1,
    result: SemanticTypeIdV1,
) -> Option<(SemanticTypeIdV1, SemanticTypeIdV1)> {
    let declaration = request.types.get(result.0 as usize)?;
    let SemanticTypeShapeV1::Aggregate(fields) = &declaration.shape else {
        return None;
    };
    let [status, value] = fields.fields.as_ref() else {
        return None;
    };
    if !is_unsigned_integer_with_bits(request, *status, 32)
        || scalar_type(request, *value) != Some(SemanticScalarTypeV1::Float { bits: 32 })
    {
        return None;
    }
    let SemanticTypeLayoutDetailsV1::Aggregate(layout) = &declaration.layout.details else {
        return None;
    };
    let SemanticBackendReprV1::Scalar(first) =
        request.types.get(status.0 as usize)?.layout.backend_repr
    else {
        return None;
    };
    let SemanticBackendReprV1::Scalar(second) =
        request.types.get(value.0 as usize)?.layout.backend_repr
    else {
        return None;
    };
    (declaration.layout.size_bytes == Some(8)
        && declaration.layout.alignment_bytes == 4
        && layout.field_offsets.as_ref() == [0, 4]
        && declaration.layout.backend_repr == SemanticBackendReprV1::ScalarPair { first, second })
    .then_some((*status, *value))
}

fn static_publication_flags_type_v31(
    request: &InertSemanticMirRequestV1,
    flags: SemanticTypeIdV1,
) -> bool {
    let Some(SemanticTypeShapeV1::Pointer(pointer)) =
        request.types.get(flags.0 as usize).map(|ty| &ty.shape)
    else {
        return false;
    };
    let Some(SemanticTypeShapeV1::Slice { element }) = request
        .types
        .get(pointer.pointee.0 as usize)
        .map(|ty| &ty.shape)
    else {
        return false;
    };
    // Request validation independently checks every layer of the pinned V29
    // atomic layout before validating this terminal signature.
    shared_slice_reference_with_element(request, flags, *element)
        && request
            .types
            .get(element.0 as usize)
            .is_some_and(|ty| ty.rust_type_kind == SemanticRustTypeKindV1::CoreAtomicU32)
}

fn static_publication_signature_matches_v31(
    request: &InertSemanticMirRequestV1,
    operation: SemanticCompilerIntrinsicOperationV1,
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
) -> bool {
    let (payload, flags, result, publish) = match operation {
        SemanticCompilerIntrinsicOperationV1::StaticPublication128PublishF32 {
            payload,
            flags,
            result,
        } => (payload, flags, result, true),
        SemanticCompilerIntrinsicOperationV1::StaticPublication128TryReadF32 {
            payload,
            flags,
            result,
        } => (payload, flags, result, false),
        _ => return false,
    };
    let Some((_, value)) = static_publication_result_fields_v31(request, result) else {
        return false;
    };
    inputs.len() == (if publish { 4 } else { 3 })
        && inputs[0] == payload
        && inputs[1] == flags
        && output == result
        && is_unsigned_integer_with_bits(request, inputs[2], 64)
        && (!publish || inputs[3] == value)
        && exclusive_disjoint_slice_type_matches(request, payload, value, inputs[2])
        && static_publication_flags_type_v31(request, flags)
}

fn encode_static_publication_intrinsic_v31(
    writer: &mut CanonicalWriterV1,
    operation: SemanticCompilerIntrinsicOperationV1,
    wire_version: SemanticMirWireVersionV1,
) -> Result<(), SemanticMirErrorV1> {
    if wire_version < SemanticMirWireVersionV1::V31 {
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: wire_version,
            required: SemanticMirWireVersionV1::V31,
        });
    }
    let (tag, payload, flags, result) = match operation {
        SemanticCompilerIntrinsicOperationV1::StaticPublication128PublishF32 {
            payload,
            flags,
            result,
        } => (72, payload, flags, result),
        SemanticCompilerIntrinsicOperationV1::StaticPublication128TryReadF32 {
            payload,
            flags,
            result,
        } => (73, payload, flags, result),
        _ => unreachable!("only static publication terminals"),
    };
    writer.u8(tag)?;
    writer.u32(payload.0)?;
    writer.u32(flags.0)?;
    writer.u32(result.0)
}
