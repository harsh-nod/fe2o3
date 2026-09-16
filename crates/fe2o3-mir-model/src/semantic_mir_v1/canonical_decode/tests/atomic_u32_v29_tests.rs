use super::*;

fn atomic_request() -> InertSemanticMirRequestV1 {
    let mut request = minimal_request();
    let mut types = request.types.to_vec();
    for index in 1..=3 {
        let layout = SemanticTypeLayoutV1::aggregate(
            Some(4),
            4,
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
        )
        .unwrap();
        let kind = if index == 3 {
            SemanticRustTypeKindV1::CoreAtomicU32
        } else {
            SemanticRustTypeKindV1::Ordinary
        };
        types.push(
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1(identity(20 + index as u8)),
                SemanticLayoutIdentityV1(identity(30 + index as u8)),
                layout,
                SemanticTypeShapeV1::Aggregate(
                    SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(index - 1)])
                        .unwrap(),
                ),
            )
            .with_rust_type_kind(kind),
        );
    }
    request.types = types.into_boxed_slice();
    let mut locals = request.functions[0].locals.to_vec();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1(identity(40)),
        SemanticTypeIdV1::from_index(3),
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    request.functions[0].locals = locals.into_boxed_slice();
    request
}

#[test]
fn atomic_u32_v29_round_trip_preserves_nominal_and_storage_identity() {
    let request = atomic_request();
    assert_eq!(
        minimum_wire_version(&request),
        SemanticMirWireVersionV1::V29
    );
    let admitted = request
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    let bytes = admitted.canonical_encoding();
    let decoded = AdmittedInertSemanticMirV1::decode_exact_v29_canonical(
        bytes,
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.canonical_encoding(), bytes);
    assert!(
        AdmittedInertSemanticMirV1::decode_current_production_canonical(
            bytes,
            SemanticMirLimitsV1::default(),
        )
        .is_ok()
    );
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v28_canonical(
            bytes,
            SemanticMirLimitsV1::default(),
        )
        .is_err()
    );
    assert!(
        atomic_request()
            .admit_exact_v28(SemanticMirLimitsV1::default())
            .is_err()
    );
}

#[test]
fn atomic_u32_v29_does_not_reencode_legacy_types_or_infer_nominality() {
    let mut ordinary = atomic_request();
    ordinary.types[3].rust_type_kind = SemanticRustTypeKindV1::Ordinary;
    assert_eq!(
        minimum_wire_version(&ordinary),
        SemanticMirWireVersionV1::V2
    );
    let legacy = ordinary
        .clone()
        .admit_exact_v28(SemanticMirLimitsV1::default())
        .unwrap();
    let future = ordinary
        .admit_exact_v29(SemanticMirLimitsV1::default())
        .unwrap();
    // Only the envelope version changes when the new marker is absent.
    let left = legacy.canonical_encoding();
    let right = future.canonical_encoding();
    let differences = left.iter().zip(right).filter(|(a, b)| a != b).count();
    assert_eq!(left.len(), right.len());
    assert_eq!(differences, 1);
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v28_canonical(
            left,
            SemanticMirLimitsV1::default(),
        )
        .is_ok()
    );
}

#[test]
fn atomic_u32_v29_rejects_invalid_marker_storage_and_legacy_tag_laundering() {
    for mutation in 0..5 {
        let mut request = atomic_request();
        match mutation {
            0 => {
                request.types[3].shape =
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 32,
                    })
            }
            1 => request.types[3].layout.alignment_bytes = 8,
            2 => request.types[3].layout.rustc_size_bytes = 8,
            3 => request.types[2].rust_type_kind = SemanticRustTypeKindV1::CoreAtomicU32,
            4 => {
                request.types[3].shape = SemanticTypeShapeV1::Aggregate(
                    SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(0)]).unwrap(),
                )
            }
            _ => unreachable!(),
        }
        assert!(
            request
                .admit_exact_v29(SemanticMirLimitsV1::default())
                .is_err(),
            "mutation {mutation}"
        );
    }
    let request = atomic_request();
    let ty = &request.types[3];
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    encode_type(&mut writer, ty).unwrap();
    let bytes = writer.finish();
    let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
    decoder.wire_version = SemanticMirWireVersionV1::V28;
    assert!(decoder.ty().is_err());
}
