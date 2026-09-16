use super::*;

fn read_only_request() -> (InertSemanticMirRequestV1, [SemanticTypeIdV1; 5]) {
    let mut request = minimal_request();
    let mut types = request.types.to_vec();
    let id = SemanticTypeIdV1::from_index;
    let scalar = |primitive, max| {
        SemanticBackendScalarV1::initialized(primitive, SemanticScalarValidityRangeV1::new(0, max))
    };
    let unsigned = scalar(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        u128::from(u64::MAX),
    );
    let pointer = scalar(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        u128::from(u64::MAX),
    );
    let mut push = |layout, shape| {
        let tag = 20 + u8::try_from(types.len()).unwrap();
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1(identity(tag)),
            SemanticLayoutIdentityV1(identity(tag + 40)),
            layout,
            shape,
        ));
    };
    push(
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(2),
            2,
            SemanticBackendReprV1::Scalar(scalar(
                SemanticBackendPrimitiveV1::integer(false, 16, 2),
                u128::from(u16::MAX),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 16,
        }),
    );
    push(
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::Scalar(unsigned),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    );
    for mutability in [
        SemanticMutabilityV1::Immutable,
        SemanticMutabilityV1::Mutable,
    ] {
        push(
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::Scalar(pointer),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new(
                    id(1),
                    mutability,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        );
    }
    push(
        SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
        SemanticTypeShapeV1::Unit,
    );
    for (fields, offsets) in [
        (vec![id(3), id(2)], vec![0, 8]),
        (vec![id(4), id(2), id(5)], vec![0, 8, 16]),
    ] {
        push(
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::ScalarPair {
                    first: pointer,
                    second: unsigned,
                },
                false,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
        );
    }
    push(
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::Scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u128::from(u64::MAX)),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                id(6),
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    );
    request.types = types.into_boxed_slice();
    (request, [id(7), id(6), id(1), id(2), id(8)])
}

#[test]
fn read_only_v30_signatures_keep_consuming_shape_and_exact_scalar_layout() {
    let (request, [slice, view, element, length, receiver]) = read_only_request();
    let cases = [
        (
            SemanticCompilerIntrinsicOperationV1::DisjointSliceIntoReadOnly {
                slice,
                view,
                element,
            },
            vec![slice],
            view,
        ),
        (
            SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLen { view },
            vec![receiver],
            length,
        ),
        (
            SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLoadOr { view, element },
            vec![receiver, length, element],
            element,
        ),
    ];
    for (operation, inputs, output) in cases {
        assert!(read_only_allocation_signature_matches_v30(
            &request, operation, &inputs, output
        ));
        assert!(!read_only_allocation_signature_matches_v30(
            &request,
            operation,
            &[],
            output
        ));
        assert!(!read_only_allocation_signature_matches_v30(
            &request,
            operation,
            &inputs,
            SemanticTypeIdV1::from_index(0)
        ));
        for mutation in 0..7 {
            let mut bad = request.clone();
            match mutation {
                0 => bad.types[view.0 as usize].layout.alignment_bytes = 16,
                1 => bad.types[view.0 as usize].layout.size_bytes = Some(24),
                2 => {
                    if let SemanticTypeLayoutDetailsV1::Aggregate(layout) =
                        &mut bad.types[view.0 as usize].layout.details
                    {
                        layout.field_offsets[1] = 0;
                    }
                }
                3 => {
                    if let SemanticTypeShapeV1::Pointer(pointer) = &mut bad.types[3].shape {
                        pointer.mutability = SemanticMutabilityV1::Mutable;
                    }
                }
                4 => {
                    if let SemanticTypeShapeV1::Pointer(pointer) = &mut bad.types[3].shape {
                        pointer.address_space = 3;
                    }
                }
                5 => {
                    if let SemanticTypeShapeV1::Pointer(pointer) = &mut bad.types[3].shape {
                        pointer.pointee = SemanticTypeIdV1::from_index(0);
                    }
                }
                6 => {
                    if let SemanticTypeShapeV1::Aggregate(fields) =
                        &mut bad.types[view.0 as usize].shape
                    {
                        fields.fields.swap(0, 1);
                    }
                }
                _ => unreachable!(),
            }
            assert!(
                !read_only_allocation_signature_matches_v30(&bad, operation, &inputs, output),
                "mutation {mutation}"
            );
        }
    }
}

#[test]
fn read_only_v30_tags_roundtrip_and_reject_legacy_or_truncated_records() {
    let (_, [slice, view, element, _, _]) = read_only_request();
    let operations = [
        SemanticCompilerIntrinsicOperationV1::DisjointSliceIntoReadOnly {
            slice,
            view,
            element,
        },
        SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLen { view },
        SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLoadOr { view, element },
    ];
    for (ordinal, operation) in operations.into_iter().enumerate() {
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        encode_compiler_intrinsic_operation(&mut writer, operation, SemanticMirWireVersionV1::V30)
            .unwrap();
        let bytes = writer.finish();
        assert_eq!(bytes[0], 69 + u8::try_from(ordinal).unwrap());
        let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
        decoder.wire_version = SemanticMirWireVersionV1::V30;
        assert_eq!(decoder.compiler_intrinsic().unwrap(), operation);
        for version in [
            SemanticMirWireVersionV1::V15,
            SemanticMirWireVersionV1::V28,
            SemanticMirWireVersionV1::V29,
        ] {
            let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
            assert!(encode_compiler_intrinsic_operation(&mut writer, operation, version).is_err());
            let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
            decoder.wire_version = version;
            assert!(decoder.compiler_intrinsic().is_err());
        }
        for end in 0..bytes.len() {
            let mut decoder =
                CanonicalDecoderV1::new(&bytes[..end], SemanticMirLimitsV1::default());
            decoder.wire_version = SemanticMirWireVersionV1::V30;
            assert!(decoder.compiler_intrinsic().is_err());
        }
        let request = version_selection_request([operation]);
        assert_eq!(
            minimum_wire_version(&request),
            SemanticMirWireVersionV1::V30
        );
    }
}

#[test]
fn read_only_v30_preserves_legacy_bytes_and_exact_version_boundaries() {
    let limits = SemanticMirLimitsV1::default();
    let legacy = minimal_request().admit_exact_v29(limits).unwrap();
    let current = minimal_request().admit_exact_v30(limits).unwrap();
    assert_eq!(
        legacy.canonical_encoding().len(),
        current.canonical_encoding().len()
    );
    assert_eq!(
        legacy
            .canonical_encoding()
            .iter()
            .zip(current.canonical_encoding())
            .filter(|(a, b)| a != b)
            .count(),
        1
    );
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v30_canonical(
            current.canonical_encoding(),
            limits
        )
        .is_ok()
    );
    assert!(
        AdmittedInertSemanticMirV1::decode_current_production_canonical(
            current.canonical_encoding(),
            limits
        )
        .is_ok()
    );
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v29_canonical(
            current.canonical_encoding(),
            limits
        )
        .is_err()
    );
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v30_canonical(legacy.canonical_encoding(), limits)
            .is_err()
    );
    let mut decoder = CanonicalDecoderV1::new(&[72], limits);
    decoder.wire_version = SemanticMirWireVersionV1::V30;
    assert!(decoder.compiler_intrinsic().is_err());
}
