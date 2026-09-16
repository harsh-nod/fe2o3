// These structural records are inert. Genuine provider identity, consuming
// origin, and whole-kernel absence of writes/escapes are compiler obligations.
fn is_read_only_allocation_intrinsic_v30(operation: SemanticCompilerIntrinsicOperationV1) -> bool {
    matches!(
        operation,
        SemanticCompilerIntrinsicOperationV1::DisjointSliceIntoReadOnly { .. }
            | SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLen { .. }
            | SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLoadOr { .. }
    )
}

fn read_only_allocation_fields_v30(
    request: &InertSemanticMirRequestV1,
    view: SemanticTypeIdV1,
) -> Option<(SemanticTypeIdV1, SemanticTypeIdV1)> {
    let declaration = request.types.get(view.0 as usize)?;
    let SemanticTypeShapeV1::Aggregate(fields) = &declaration.shape else {
        return None;
    };
    let [pointer_id, length] = fields.fields.as_ref() else {
        return None;
    };
    let pointer_decl = request.types.get(pointer_id.0 as usize)?;
    let SemanticTypeShapeV1::Pointer(pointer) = &pointer_decl.shape else {
        return None;
    };
    if pointer.kind != SemanticPointerKindV1::Raw
        || pointer.mutability != SemanticMutabilityV1::Immutable
        || pointer.address_space != 0
        || pointer.pointer_width_bits != 64
        || pointer.metadata != SemanticPointerMetadataV1::None
        || !is_unsigned_integer_with_bits(request, *length, 64)
        || !matches!(
            scalar_type(request, pointer.pointee),
            Some(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 16
            }) | Some(SemanticScalarTypeV1::Float { bits: 32 })
        )
    {
        return None;
    }
    let SemanticTypeLayoutDetailsV1::Aggregate(layout) = &declaration.layout.details else {
        return None;
    };
    let SemanticBackendReprV1::Scalar(first) = pointer_decl.layout.backend_repr else {
        return None;
    };
    let SemanticBackendReprV1::Scalar(second) =
        request.types.get(length.0 as usize)?.layout.backend_repr
    else {
        return None;
    };
    (declaration.layout.size_bytes == Some(16)
        && declaration.layout.alignment_bytes == 8
        && layout.field_offsets.as_ref() == [0, 8]
        && declaration.layout.backend_repr == SemanticBackendReprV1::ScalarPair { first, second })
    .then_some((pointer.pointee, *length))
}

fn read_only_allocation_signature_matches_v30(
    request: &InertSemanticMirRequestV1,
    operation: SemanticCompilerIntrinsicOperationV1,
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
) -> bool {
    match operation {
        SemanticCompilerIntrinsicOperationV1::DisjointSliceIntoReadOnly {
            slice,
            view,
            element,
        } => {
            inputs == [slice]
                && output == view
                && read_only_allocation_fields_v30(request, view).is_some_and(|(actual, length)| {
                    actual == element
                        && exclusive_disjoint_slice_type_matches(request, slice, element, length)
                })
        }
        SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLen { view } => {
            inputs.len() == 1
                && shared_reference_to(request, inputs[0], view)
                && read_only_allocation_fields_v30(request, view)
                    .is_some_and(|(_, length)| output == length)
        }
        SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLoadOr { view, element } => {
            inputs.len() == 3
                && shared_reference_to(request, inputs[0], view)
                && inputs[2] == element
                && output == element
                && read_only_allocation_fields_v30(request, view)
                    .is_some_and(|(actual, length)| actual == element && inputs[1] == length)
        }
        _ => false,
    }
}

fn encode_read_only_allocation_intrinsic_v30(
    writer: &mut CanonicalWriterV1,
    operation: SemanticCompilerIntrinsicOperationV1,
    wire_version: SemanticMirWireVersionV1,
) -> Result<(), SemanticMirErrorV1> {
    if wire_version < SemanticMirWireVersionV1::V30 {
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: wire_version,
            required: SemanticMirWireVersionV1::V30,
        });
    }
    match operation {
        SemanticCompilerIntrinsicOperationV1::DisjointSliceIntoReadOnly {
            slice,
            view,
            element,
        } => {
            writer.u8(69)?;
            writer.u32(slice.0)?;
            writer.u32(view.0)?;
            writer.u32(element.0)
        }
        SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLen { view } => {
            writer.u8(70)?;
            writer.u32(view.0)
        }
        SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLoadOr { view, element } => {
            writer.u8(71)?;
            writer.u32(view.0)?;
            writer.u32(element.0)
        }
        _ => unreachable!("only consuming read-only allocation terminals"),
    }
}
