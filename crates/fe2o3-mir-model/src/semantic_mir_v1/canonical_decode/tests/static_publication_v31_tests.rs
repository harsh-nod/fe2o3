use super::*;

fn publication_type(
    types: &mut Vec<SemanticTypeDeclV1>,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeIdV1 {
    let index = u32::try_from(types.len()).unwrap();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1(identity(20 + index as u8)),
        SemanticLayoutIdentityV1(identity(60 + index as u8)),
        layout,
        shape,
    ));
    SemanticTypeIdV1::from_index(index)
}

fn publication_request() -> (InertSemanticMirRequestV1, [SemanticTypeIdV1; 5]) {
    let mut request = minimal_request();
    let mut types = request.types.to_vec();
    let scalar = |primitive, max| {
        SemanticBackendScalarV1::initialized(primitive, SemanticScalarValidityRangeV1::new(0, max))
    };
    let float = scalar(
        SemanticBackendPrimitiveV1::float(32, 4),
        u128::from(u32::MAX),
    );
    let index = scalar(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        u128::from(u64::MAX),
    );
    let pointer = scalar(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        u128::from(u64::MAX),
    );
    let value = publication_type(
        &mut types,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::Scalar(float),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
    );
    let cell = publication_type(
        &mut types,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::Scalar(index),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    );
    let payload_pointer = publication_type(
        &mut types,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::Scalar(pointer),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new(
                value,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    );
    let marker = publication_type(
        &mut types,
        SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
        SemanticTypeShapeV1::Unit,
    );
    let payload = publication_type(
        &mut types,
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::ScalarPair {
                first: pointer,
                second: index,
            },
            false,
            SemanticAggregateLayoutV1::new(vec![0, 8, 16], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![payload_pointer, cell, marker]).unwrap(),
        ),
    );
    let mut atomic = SemanticTypeIdV1::from_index(0);
    for _ in 0..3 {
        atomic = publication_type(
            &mut types,
            SemanticTypeLayoutV1::aggregate(
                Some(4),
                4,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![atomic]).unwrap()),
        );
    }
    types[atomic.0 as usize].rust_type_kind = SemanticRustTypeKindV1::CoreAtomicU32;
    let slice = publication_type(
        &mut types,
        SemanticTypeLayoutV1::new(None, 4).unwrap(),
        SemanticTypeShapeV1::Slice { element: atomic },
    );
    let reference_pointer = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u128::from(u64::MAX)),
    );
    let flags = publication_type(
        &mut types,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::ScalarPair {
                first: reference_pointer,
                second: index,
            },
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                slice,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::SliceLength,
            )
            .unwrap(),
        ),
    );
    let SemanticBackendReprV1::Scalar(status) = types[0].layout.backend_repr else {
        panic!("u32 fixture");
    };
    let result = publication_type(
        &mut types,
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(8),
            4,
            SemanticBackendReprV1::ScalarPair {
                first: status,
                second: float,
            },
            false,
            SemanticAggregateLayoutV1::new(vec![0, 4], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(0), value]).unwrap(),
        ),
    );
    request.types = types.into_boxed_slice();
    (request, [payload, flags, result, cell, value])
}

#[test]
fn publication_v31_signatures_are_exact_and_do_not_infer_atomic_nominality() {
    let (request, [payload, flags, result, cell, value]) = publication_request();
    for (operation, inputs) in [
        (
            SemanticCompilerIntrinsicOperationV1::StaticPublication128PublishF32 {
                payload,
                flags,
                result,
            },
            vec![payload, flags, cell, value],
        ),
        (
            SemanticCompilerIntrinsicOperationV1::StaticPublication128TryReadF32 {
                payload,
                flags,
                result,
            },
            vec![payload, flags, cell],
        ),
    ] {
        assert!(static_publication_signature_matches_v31(
            &request, operation, &inputs, result
        ));
        for bad_inputs in [
            vec![],
            vec![payload, flags],
            vec![flags, payload, cell],
            vec![payload, flags, value],
        ] {
            assert!(!static_publication_signature_matches_v31(
                &request,
                operation,
                &bad_inputs,
                result
            ));
        }
        assert!(!static_publication_signature_matches_v31(
            &request, operation, &inputs, value
        ));
        for mutation in 0..10 {
            let mut bad = request.clone();
            match mutation {
                0 => bad.types[result.0 as usize].layout.alignment_bytes = 8,
                1 => bad.types[result.0 as usize].layout.size_bytes = Some(16),
                2 => {
                    if let SemanticTypeLayoutDetailsV1::Aggregate(layout) =
                        &mut bad.types[result.0 as usize].layout.details
                    {
                        layout.field_offsets[1] = 0;
                    }
                }
                3 => {
                    if let SemanticTypeShapeV1::Aggregate(fields) =
                        &mut bad.types[result.0 as usize].shape
                    {
                        fields.fields.swap(0, 1);
                    }
                }
                4 => bad.types[8].rust_type_kind = SemanticRustTypeKindV1::Ordinary,
                5 => {
                    if let SemanticTypeShapeV1::Pointer(pointer) =
                        &mut bad.types[flags.0 as usize].shape
                    {
                        pointer.mutability = SemanticMutabilityV1::Mutable;
                    }
                }
                6 => {
                    if let SemanticTypeShapeV1::Pointer(pointer) =
                        &mut bad.types[flags.0 as usize].shape
                    {
                        pointer.kind = SemanticPointerKindV1::Raw;
                    }
                }
                7 => {
                    if let SemanticTypeShapeV1::Pointer(pointer) =
                        &mut bad.types[flags.0 as usize].shape
                    {
                        pointer.address_space = 3;
                    }
                }
                8 => {
                    if let SemanticTypeShapeV1::Pointer(pointer) = &mut bad.types[3].shape {
                        pointer.mutability = SemanticMutabilityV1::Immutable;
                    }
                }
                9 => {
                    if let SemanticTypeShapeV1::Pointer(pointer) =
                        &mut bad.types[flags.0 as usize].shape
                    {
                        pointer.metadata = SemanticPointerMetadataV1::None;
                    }
                }
                _ => unreachable!(),
            }
            assert!(
                !static_publication_signature_matches_v31(&bad, operation, &inputs, result),
                "mutation {mutation}"
            );
        }
    }
}

#[test]
fn publication_v31_tags_are_exact_closed_and_versioned() {
    let (_, [payload, flags, result, _, _]) = publication_request();
    for (tag, operation) in [
        (
            72,
            SemanticCompilerIntrinsicOperationV1::StaticPublication128PublishF32 {
                payload,
                flags,
                result,
            },
        ),
        (
            73,
            SemanticCompilerIntrinsicOperationV1::StaticPublication128TryReadF32 {
                payload,
                flags,
                result,
            },
        ),
    ] {
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        encode_compiler_intrinsic_operation(&mut writer, operation, SemanticMirWireVersionV1::V31)
            .unwrap();
        let bytes = writer.finish();
        assert_eq!(bytes[0], tag);
        assert_eq!(bytes.len(), 13);
        let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
        decoder.wire_version = SemanticMirWireVersionV1::V31;
        assert_eq!(decoder.compiler_intrinsic().unwrap(), operation);
        for version in [
            SemanticMirWireVersionV1::V15,
            SemanticMirWireVersionV1::V28,
            SemanticMirWireVersionV1::V29,
            SemanticMirWireVersionV1::V30,
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
            decoder.wire_version = SemanticMirWireVersionV1::V31;
            assert!(decoder.compiler_intrinsic().is_err());
        }
        assert_eq!(
            minimum_wire_version(&version_selection_request([operation])),
            SemanticMirWireVersionV1::V31
        );
    }
    let mut decoder = CanonicalDecoderV1::new(&[74], SemanticMirLimitsV1::default());
    decoder.wire_version = SemanticMirWireVersionV1::V31;
    assert!(decoder.compiler_intrinsic().is_err());
}

#[test]
fn publication_v31_preserves_v30_bytes_and_exact_envelopes() {
    let limits = SemanticMirLimitsV1::default();
    let old = minimal_request().admit_exact_v30(limits).unwrap();
    let new = minimal_request().admit_exact_v31(limits).unwrap();
    assert_eq!(
        old.canonical_encoding().len(),
        new.canonical_encoding().len()
    );
    assert_eq!(
        old.canonical_encoding()
            .iter()
            .zip(new.canonical_encoding())
            .filter(|(a, b)| a != b)
            .count(),
        1
    );
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v31_canonical(new.canonical_encoding(), limits)
            .is_ok()
    );
    assert!(
        AdmittedInertSemanticMirV1::decode_current_production_canonical(
            new.canonical_encoding(),
            limits
        )
        .is_ok()
    );
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v30_canonical(new.canonical_encoding(), limits)
            .is_err()
    );
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v31_canonical(old.canonical_encoding(), limits)
            .is_err()
    );
    assert_eq!(
        minimum_wire_version(&minimal_request()),
        SemanticMirWireVersionV1::V2
    );
    for operation in [
        SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLen {
            view: SemanticTypeIdV1::from_index(0),
        },
        SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLoadOr {
            view: SemanticTypeIdV1::from_index(0),
            element: SemanticTypeIdV1::from_index(1),
        },
    ] {
        let mut old = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        let mut new = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        encode_compiler_intrinsic_operation(&mut old, operation, SemanticMirWireVersionV1::V30)
            .unwrap();
        encode_compiler_intrinsic_operation(&mut new, operation, SemanticMirWireVersionV1::V31)
            .unwrap();
        assert_eq!(old.finish(), new.finish());
    }
}
