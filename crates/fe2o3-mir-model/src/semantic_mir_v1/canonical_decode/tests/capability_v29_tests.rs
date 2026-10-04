//! Inert codec/signature fixtures, not authenticated source or execution evidence.
use super::*;
use SemanticExecutionOperationV29 as Op;
use SemanticExecutionRoleV29 as Role;

mod abi_tests;
mod ownership_tests;

const SCALAR: SemanticTypeIdV1 = SemanticTypeIdV1(0);
const MARKER: SemanticTypeIdV1 = SemanticTypeIdV1(1);
const EPOCH: SemanticTypeIdV1 = SemanticTypeIdV1(2);
const INDEX: SemanticTypeIdV1 = SemanticTypeIdV1(3);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1(4);
const CONTEXT: SemanticTypeIdV1 = SemanticTypeIdV1(5);
const WORKGROUP: SemanticTypeIdV1 = SemanticTypeIdV1(6);
const VALUES: SemanticTypeIdV1 = SemanticTypeIdV1(7);
const MASK: SemanticTypeIdV1 = SemanticTypeIdV1(8);
const TILE: SemanticTypeIdV1 = SemanticTypeIdV1(9);
const FRAGMENT: SemanticTypeIdV1 = SemanticTypeIdV1(10);
const PARTS: SemanticTypeIdV1 = SemanticTypeIdV1(11);
const CONTEXT_REF: SemanticTypeIdV1 = SemanticTypeIdV1(12);
const WORKGROUP_REF: SemanticTypeIdV1 = SemanticTypeIdV1(13);
const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1(14);
const SLICE_REF: SemanticTypeIdV1 = SemanticTypeIdV1(15);

fn declaration(
    index: usize,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1(identity(20 + index as u8)),
        SemanticLayoutIdentityV1(identity(20 + index as u8)),
        layout,
        shape,
    )
}

fn aggregate(
    types: &[SemanticTypeDeclV1],
    fields: &[SemanticTypeIdV1],
    tuple: bool,
) -> SemanticTypeDeclV1 {
    let alignment = fields
        .iter()
        .map(|id| types[id.0 as usize].layout.alignment_bytes)
        .max()
        .unwrap_or(1);
    let mut size = 0u64;
    let mut offsets = Vec::new();
    let mut padding = Vec::new();
    for id in fields {
        let layout = &types[id.0 as usize].layout;
        let offset = size.next_multiple_of(layout.alignment_bytes);
        if offset != size {
            padding.push(SemanticPaddingV1::new(size, offset - size).unwrap());
        }
        offsets.push(offset);
        size = offset + layout.size_bytes.unwrap();
    }
    let aligned = size.next_multiple_of(alignment);
    if aligned != size {
        padding.push(SemanticPaddingV1::new(size, aligned - size).unwrap());
    }
    let fields = SemanticAggregateTypeV1::new(fields.to_vec()).unwrap();
    declaration(
        types.len(),
        SemanticTypeLayoutV1::aggregate(
            Some(aligned),
            alignment,
            SemanticAggregateLayoutV1::new(offsets, padding).unwrap(),
        )
        .unwrap(),
        if tuple {
            SemanticTypeShapeV1::Tuple(fields)
        } else {
            SemanticTypeShapeV1::Aggregate(fields)
        },
    )
}

fn integer(bits: u16, maximum: u128) -> SemanticBackendScalarV1 {
    SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, bits, u64::from(bits / 8)),
        SemanticScalarValidityRangeV1::new(0, maximum),
    )
}

pub(super) fn request(lanes: u16, elements: u16) -> InertSemanticMirRequestV1 {
    let mut request = minimal_request();
    let mut types = request.types.into_vec();
    types.push(aggregate(&types, &[], false));
    types.push(aggregate(&types, &[MARKER; 3], false));
    for (size, scalar, backend) in [
        (
            8,
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            },
            integer(64, u64::MAX.into()),
        ),
        (1, SemanticScalarTypeV1::Bool, integer(8, 1)),
    ] {
        types.push(declaration(
            types.len(),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(size),
                size,
                SemanticBackendReprV1::scalar(backend),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(scalar),
        ));
    }
    for (fields, role) in [
        (vec![MARKER; 5], Role::KernelContext),
        (vec![INDEX, INDEX, EPOCH, MARKER], Role::Workgroup),
    ] {
        let mut ty = aggregate(&types, &fields, false);
        ty.rust_type_kind = SemanticRustTypeKindV1::Execution(role);
        types.push(ty);
    }
    for element in [SCALAR, BOOL] {
        let element_layout = &types[element.0 as usize].layout;
        let stride = element_layout.size_bytes.unwrap();
        let mut layout = SemanticTypeLayoutV1::new(
            Some(stride * u64::from(elements)),
            element_layout.alignment_bytes,
        )
        .unwrap();
        layout.fields = SemanticFieldsShapeV1::Array {
            stride_bytes: stride,
            count: elements.into(),
        };
        types.push(declaration(
            types.len(),
            layout,
            SemanticTypeShapeV1::Array {
                element,
                length: elements.into(),
            },
        ));
    }
    for role in [
        Role::MaskedTileU32 { lanes, elements },
        Role::LaneFragmentU32 { lanes, elements },
    ] {
        let mut ty = aggregate(&types, &[VALUES, MASK, MARKER, MARKER], false);
        ty.rust_type_kind = SemanticRustTypeKindV1::Execution(role);
        types.push(ty);
    }
    types.push(aggregate(&types, &[VALUES, MASK], true));
    for (pointee, mutability) in [
        (CONTEXT, SemanticMutabilityV1::Mutable),
        (WORKGROUP, SemanticMutabilityV1::Immutable),
    ] {
        types.push(reference(&types, pointee, mutability, false));
    }
    let mut slice = SemanticTypeLayoutV1::new(None, 4).unwrap();
    slice.fields = SemanticFieldsShapeV1::Array {
        stride_bytes: 4,
        count: 0,
    };
    types.push(declaration(
        types.len(),
        slice,
        SemanticTypeShapeV1::Slice { element: SCALAR },
    ));
    types.push(reference(
        &types,
        SLICE,
        SemanticMutabilityV1::Immutable,
        true,
    ));
    request.types = types.into_boxed_slice();
    let mut locals = request.functions[0].locals.to_vec();
    locals.extend(
        (1..request.types.len())
            .filter(|index| *index != SLICE.0 as usize)
            .map(|index| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1(identity(80 + index as u8)),
                    SemanticTypeIdV1(index as u32),
                    SemanticLocalRoleV1::Temporary,
                    SemanticSourceProvenanceV1::unavailable(),
                )
            }),
    );
    request.functions[0].locals = locals.into_boxed_slice();
    request
}

fn reference(
    types: &[SemanticTypeDeclV1],
    pointee: SemanticTypeIdV1,
    mutability: SemanticMutabilityV1,
    slice: bool,
) -> SemanticTypeDeclV1 {
    let pointee_layout = &types[pointee.0 as usize].layout;
    let pointer = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    let backend = if slice {
        SemanticBackendReprV1::scalar_pair(pointer, integer(64, u64::MAX.into()))
    } else {
        SemanticBackendReprV1::scalar(pointer)
    };
    declaration(
        types.len(),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(if slice { 16 } else { 8 }),
            8,
            backend,
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                SemanticPointerKindV1::Reference,
                mutability,
                0,
                64,
                if slice {
                    SemanticPointerMetadataV1::SliceLength
                } else {
                    SemanticPointerMetadataV1::None
                },
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    if mutability == SemanticMutabilityV1::Mutable {
                        SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                    } else {
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                    },
                    pointee_layout.size_bytes.unwrap_or(0),
                    pointee_layout.alignment_bytes,
                )
                .unwrap(),
            ),
            None,
        ),
    )
}

fn operations() -> [Op; 5] {
    [
        Op::ContextIssue { context: CONTEXT },
        Op::WorkgroupDerive {
            context: CONTEXT,
            workgroup: WORKGROUP,
        },
        Op::MaskedTileLoadU32 {
            workgroup: WORKGROUP,
            tile: TILE,
        },
        Op::MaskedTileIntoFragmentU32 {
            tile: TILE,
            fragment: FRAGMENT,
        },
        Op::LaneFragmentIntoPartsU32 {
            fragment: FRAGMENT,
            parts: PARTS,
        },
    ]
}

#[test]
fn capability_roles_round_trip_and_refuse_production_or_old_versions() {
    let limits = SemanticMirLimitsV1::default();
    for (lanes, elements) in [(1, 1), (2, 2), (256, 125)] {
        let request = request(lanes, elements);
        let admitted = request.clone().admit_exact_v29(limits).unwrap();
        let decoded = AdmittedInertSemanticMirV1::decode_exact_v29_canonical(
            admitted.canonical_encoding(),
            limits,
        )
        .unwrap();
        assert_eq!(decoded.types(), admitted.types());
        assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
        assert!(matches!(
            request.clone().admit_current_production(limits),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                required: SemanticMirWireVersionV1::V29,
                ..
            })
        ));
        assert!(request.admit_exact_v28(limits).is_err());
        assert!(matches!(
            AdmittedInertSemanticMirV1::decode_current_production_canonical(
                admitted.canonical_encoding(),
                limits
            ),
            Err(SemanticMirDecodeErrorV1::UnsupportedProductionWireVersion(
                SemanticMirWireVersionV1::V29
            ))
        ));
        let mut changed = admitted.canonical_encoding().to_vec();
        changed[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&28u16.to_le_bytes());
        assert!(AdmittedInertSemanticMirV1::decode_exact_v28_canonical(&changed, limits).is_err());
    }
}

#[test]
fn capability_geometry_and_carrier_mutations_reject() {
    let limits = SemanticMirLimitsV1::default();
    for id in [EPOCH, VALUES, MASK] {
        let mut changed = request(2, 2);
        changed.types[id.0 as usize].layout.uninhabited = true;
        assert_eq!(
            changed.admit_exact_v29(limits).unwrap_err(),
            SemanticMirErrorV1::InvalidTypeLayout
        );
    }
    for (lanes, elements) in [(0, 2), (257, 2), (2, 0), (2, 126)] {
        let mut changed = request(2, 2);
        changed.types[TILE.0 as usize].rust_type_kind =
            SemanticRustTypeKindV1::Execution(Role::MaskedTileU32 { lanes, elements });
        assert_eq!(
            changed.admit_exact_v29(limits).unwrap_err(),
            SemanticMirErrorV1::InvalidTypeLayout
        );
    }
    for id in [CONTEXT, WORKGROUP, TILE, FRAGMENT] {
        let mut changed = request(2, 2);
        let SemanticTypeShapeV1::Aggregate(fields) = &mut changed.types[id.0 as usize].shape else {
            unreachable!()
        };
        fields.fields[0] = BOOL;
        assert!(changed.admit_exact_v29(limits).is_err(), "{id:?}");
    }
}

#[test]
fn capability_opcodes_are_exact_and_historical_holes_never_panic() {
    let limits = SemanticMirLimitsV1::default();
    for (operation, tag) in operations().into_iter().zip([81, 82, 84, 85, 86]) {
        let mut writer =
            CanonicalWriterV1::new(limits.limit(SemanticMirResourceV1::CanonicalBytes));
        let operation = SemanticCompilerIntrinsicOperationV1::Execution(operation);
        encode_compiler_intrinsic_operation(&mut writer, operation, SemanticMirWireVersionV1::V29)
            .unwrap();
        let bytes = writer.finish();
        assert_eq!(bytes[0], tag);
        let mut decoder = CanonicalDecoderV1::new(&bytes, limits);
        decoder.wire_version = SemanticMirWireVersionV1::V29;
        assert_eq!(decoder.compiler_intrinsic().unwrap(), operation);
        decoder.finish().unwrap();
        let mut old = CanonicalWriterV1::new(limits.limit(SemanticMirResourceV1::CanonicalBytes));
        assert!(
            encode_compiler_intrinsic_operation(&mut old, operation, SemanticMirWireVersionV1::V28)
                .is_err()
        );
    }
    for tag in (69..=80).chain([83, 87, 255]) {
        let bytes = [tag];
        let mut decoder = CanonicalDecoderV1::new(&bytes, limits);
        decoder.wire_version = SemanticMirWireVersionV1::V29;
        assert!(
            matches!(decoder.compiler_intrinsic(), Err(SemanticMirDecodeErrorV1::InvalidTag { value, .. }) if value == tag)
        );
    }
}

#[test]
fn capability_signatures_bind_roles_borrows_geometry_and_ordered_parts() {
    let request = request(2, 2);
    let signatures = [
        (vec![], CONTEXT),
        (vec![CONTEXT_REF], WORKGROUP),
        (vec![WORKGROUP_REF, SLICE_REF, INDEX], TILE),
        (vec![TILE], FRAGMENT),
        (vec![FRAGMENT], PARTS),
    ];
    for (operation, (inputs, output)) in operations().into_iter().zip(signatures) {
        assert!(
            operation.signature_matches(&request, &inputs, output),
            "{operation:?}"
        );
        assert!(!operation.signature_matches(&request, &inputs, SCALAR));
        let mut wrong = inputs.clone();
        wrong.push(INDEX);
        assert!(!operation.signature_matches(&request, &wrong, output));
    }
    let mut changed = request.clone();
    changed.types[FRAGMENT.0 as usize].rust_type_kind =
        SemanticRustTypeKindV1::Execution(Role::LaneFragmentU32 {
            lanes: 4,
            elements: 2,
        });
    assert!(!operations()[3].signature_matches(&changed, &[TILE], FRAGMENT));
    let mut changed = request;
    let SemanticTypeShapeV1::Tuple(fields) = &mut changed.types[PARTS.0 as usize].shape else {
        unreachable!()
    };
    fields.fields.swap(0, 1);
    assert!(!operations()[4].signature_matches(&changed, &[FRAGMENT], PARTS));
}

#[test]
fn capability_documents_enforce_truncation_and_exact_resource_boundaries() {
    let limits = SemanticMirLimitsV1::default();
    let request = request(2, 2);
    let admitted = request.clone().admit_exact_v29(limits).unwrap();
    let bytes = admitted.canonical_encoding();
    for length in 0..bytes.len() {
        assert!(
            AdmittedInertSemanticMirV1::decode_exact_v29_canonical(&bytes[..length], limits)
                .is_err()
        );
    }
    let exact = limits
        .with_limit(SemanticMirResourceV1::CanonicalBytes, bytes.len() as u64)
        .unwrap();
    assert!(request.clone().admit_exact_v29(exact).is_ok());
    let short = limits
        .with_limit(
            SemanticMirResourceV1::CanonicalBytes,
            bytes.len() as u64 - 1,
        )
        .unwrap();
    assert!(matches!(
        request.admit_exact_v29(short),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            ..
        })
    ));
}

#[test]
fn capability_type_tags_preserve_the_existing_str_encoding() {
    let limits = SemanticMirLimitsV1::default();
    let mut layout = SemanticTypeLayoutV1::new(None, 1).unwrap();
    layout.fields = SemanticFieldsShapeV1::Array {
        stride_bytes: 1,
        count: 0,
    };
    let ty = declaration(0, layout, SemanticTypeShapeV1::Opaque)
        .with_rust_type_kind(SemanticRustTypeKindV1::Str);
    let encode = |version| {
        let mut writer =
            CanonicalWriterV1::new(limits.limit(SemanticMirResourceV1::CanonicalBytes));
        encode_type(&mut writer, &ty, version).unwrap();
        writer.finish()
    };
    let bytes = encode(SemanticMirWireVersionV1::V29);
    assert_eq!(bytes, encode(SemanticMirWireVersionV1::V28));
    assert_eq!(bytes.last(), Some(&13));
    let mut decoder = CanonicalDecoderV1::new(&bytes, limits);
    decoder.wire_version = SemanticMirWireVersionV1::V29;
    assert_eq!(decoder.ty().unwrap(), ty);
    decoder.finish().unwrap();
}
