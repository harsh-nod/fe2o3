use super::*;

const ELEMENT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const POINTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const RESULT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);

fn sequence_layout(length: Option<u64>) -> SemanticTypeLayoutV1 {
    SemanticTypeLayoutV1::with_exact_rustc_layout(
        length.unwrap_or(0) * 4,
        4,
        SemanticFieldsShapeV1::array(4, length.unwrap_or(0)),
        SemanticRustcVariantsV1::Single { index: 0 },
        SemanticBackendReprV1::memory(length.is_some()),
        None,
        false,
        None,
        4,
        0,
        SemanticTypeLayoutDetailsV1::None,
    )
    .unwrap()
}

fn slice_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(2),
        layout_identity(2),
        sequence_layout(None),
        SemanticTypeShapeV1::Slice { element: ELEMENT },
    )
}

fn carrier(
    tag: u8,
    pointee: SemanticTypeIdV1,
    pointee_size: u64,
    pointee_alignment: u64,
    metadata: SemanticPointerMetadataV1,
    mutable: bool,
) -> SemanticTypeDeclV1 {
    let data = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    let (size, repr, second) = match metadata {
        SemanticPointerMetadataV1::None => (8, SemanticBackendReprV1::scalar(data), None),
        SemanticPointerMetadataV1::SliceLength => (
            16,
            SemanticBackendReprV1::scalar_pair(
                data,
                SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 64, 8),
                    SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                ),
            ),
            None,
        ),
        SemanticPointerMetadataV1::VTable => (
            16,
            SemanticBackendReprV1::scalar_pair(
                data,
                SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                ),
            ),
            Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
        ),
    };
    SemanticTypeDeclV1::new(
        type_identity(tag),
        layout_identity(tag),
        SemanticTypeLayoutV1::new_with_backend_repr(Some(size), 8, repr, false).unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                SemanticPointerKindV1::Reference,
                if mutable {
                    SemanticMutabilityV1::Mutable
                } else {
                    SemanticMutabilityV1::Immutable
                },
                0,
                64,
                metadata,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    if mutable {
                        SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                    } else {
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                    },
                    if metadata == SemanticPointerMetadataV1::None {
                        pointee_size
                    } else {
                        0
                    },
                    pointee_alignment,
                )
                .unwrap(),
            ),
            second,
        ),
    )
}

fn base_types(metadata: SemanticPointerMetadataV1, mutable: bool) -> Vec<SemanticTypeDeclV1> {
    vec![
        u32_type(1),
        slice_type(),
        carrier(3, SLICE, 0, 4, metadata, mutable),
        u64_type(4),
    ]
}

fn projection(kind: SemanticProjectionKindV1, ty: SemanticTypeIdV1) -> SemanticProjectionV1 {
    SemanticProjectionV1::new(kind, ty).unwrap()
}

fn length_request(
    types: Vec<SemanticTypeDeclV1>,
    holder: SemanticTypeIdV1,
    result: SemanticTypeIdV1,
    projections: Vec<SemanticProjectionV1>,
    place_type: SemanticTypeIdV1,
) -> InertSemanticMirRequestV1 {
    let output_extension = if matches!(
        types[result.index() as usize].shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
    ) {
        SemanticAbiExtensionV1::ZeroExtend
    } else {
        SemanticAbiExtensionV1::None
    };
    let function_abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(1)),
        layout_identity(1),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![],
        SemanticAbiValueV1::new(
            result,
            SemanticAbiPassModeV1::Direct(noundef_attributes(output_extension)),
        ),
    )
    .unwrap();
    request_with_statement(
        types,
        function_abi,
        vec![
            local(1, result, SemanticLocalRoleV1::Return),
            local(2, holder, SemanticLocalRoleV1::Temporary),
        ],
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], result).unwrap(),
            SemanticRvalueV1::new(
                result,
                SemanticRvalueKindV1::Length(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), projections, place_type)
                        .unwrap(),
                ),
            ),
        )),
    )
}

fn length_error() -> SemanticMirErrorV1 {
    SemanticMirErrorV1::InvalidTypeOperation {
        operation: SemanticTypeOperationV1::Length,
        location: location(),
    }
}

fn location() -> SemanticMirLocationV1 {
    SemanticMirLocationV1::Statement {
        function: SemanticFunctionIdV1::from_index(0),
        block: SemanticBlockIdV1::from_index(0),
        statement: 0,
    }
}

fn admit(
    request: InertSemanticMirRequestV1,
) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
    request.admit_current_production(SemanticMirLimitsV1::default())
}

fn roundtrip(request: InertSemanticMirRequestV1) {
    let owner = admit(request).unwrap();
    let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        owner.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.canonical_encoding(), owner.canonical_encoding());
    assert_eq!(decoded.types(), owner.types());
    assert_eq!(decoded.functions(), owner.functions());
    let SemanticStatementKindV1::Assign(assignment) =
        decoded.functions()[0].blocks()[0].statements()[0].kind()
    else {
        panic!("original length assignment missing");
    };
    assert!(matches!(
        assignment.value().kind(),
        SemanticRvalueKindV1::Length(_)
    ));
}

#[test]
fn shared_and_mutable_slice_length_roundtrip_as_original_length() {
    for mutable in [false, true] {
        roundtrip(length_request(
            base_types(SemanticPointerMetadataV1::SliceLength, mutable),
            POINTER,
            RESULT,
            vec![projection(SemanticProjectionKindV1::Dereference, SLICE)],
            SLICE,
        ));
    }
}

#[test]
fn slice_length_requires_unsigned_64_result() {
    let signed64 = SemanticTypeDeclV1::new(
        type_identity(4),
        layout_identity(4),
        scalar_layout(
            8,
            8,
            SemanticBackendPrimitiveV1::integer(true, 64, 8),
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        ),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: true,
            bits: 64,
        }),
    );
    for result in [u32_type(4), i32_type(4), signed64, bool_type(4)] {
        let mut types = base_types(SemanticPointerMetadataV1::SliceLength, false);
        types[3] = result;
        assert_eq!(
            admit(length_request(
                types,
                POINTER,
                RESULT,
                vec![projection(SemanticProjectionKindV1::Dereference, SLICE)],
                SLICE
            ))
            .unwrap_err(),
            length_error()
        );
    }
}

#[test]
fn slice_length_rejects_missing_or_vtable_metadata_at_operation_admission() {
    for metadata in [
        SemanticPointerMetadataV1::None,
        SemanticPointerMetadataV1::VTable,
    ] {
        assert_eq!(
            admit(length_request(
                base_types(metadata, false),
                POINTER,
                RESULT,
                vec![projection(SemanticProjectionKindV1::Dereference, SLICE)],
                SLICE
            ))
            .unwrap_err(),
            length_error()
        );
    }
}

#[test]
fn final_dereference_replaces_outer_metadata() {
    for outer in [
        SemanticPointerMetadataV1::None,
        SemanticPointerMetadataV1::SliceLength,
        SemanticPointerMetadataV1::VTable,
    ] {
        for inner in [
            SemanticPointerMetadataV1::None,
            SemanticPointerMetadataV1::SliceLength,
            SemanticPointerMetadataV1::VTable,
        ] {
            let mut types = base_types(inner, false);
            types.push(carrier(
                5,
                POINTER,
                if inner == SemanticPointerMetadataV1::None {
                    8
                } else {
                    16
                },
                8,
                outer,
                false,
            ));
            let request = length_request(
                types,
                SemanticTypeIdV1::from_index(4),
                RESULT,
                vec![
                    projection(SemanticProjectionKindV1::Dereference, POINTER),
                    projection(SemanticProjectionKindV1::Dereference, SLICE),
                ],
                SLICE,
            );
            if inner == SemanticPointerMetadataV1::SliceLength {
                roundtrip(request);
            } else {
                assert_eq!(
                    admit(request).unwrap_err(),
                    length_error(),
                    "outer {outer:?}, inner {inner:?}"
                );
            }
        }
    }
}

#[test]
fn metadata_follows_unsized_fields_and_same_type_casts_not_holder_fields() {
    let mut types = base_types(SemanticPointerMetadataV1::SliceLength, false);
    types.push(SemanticTypeDeclV1::new(
        type_identity(5),
        layout_identity(5),
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![POINTER]).unwrap()),
    ));
    roundtrip(length_request(
        types,
        SemanticTypeIdV1::from_index(4),
        RESULT,
        vec![
            projection(SemanticProjectionKindV1::Field(0), POINTER),
            projection(SemanticProjectionKindV1::Dereference, SLICE),
            projection(SemanticProjectionKindV1::OpaqueCast, SLICE),
            projection(SemanticProjectionKindV1::Subtype, SLICE),
        ],
        SLICE,
    ));

    for metadata in [
        SemanticPointerMetadataV1::SliceLength,
        SemanticPointerMetadataV1::None,
        SemanticPointerMetadataV1::VTable,
    ] {
        let mut types = base_types(metadata, false);
        let tail = SemanticTypeIdV1::from_index(4);
        types[2] = carrier(3, tail, 0, 4, metadata, false);
        types.push(SemanticTypeDeclV1::new(
            type_identity(5),
            layout_identity(5),
            SemanticTypeLayoutV1::aggregate(
                None,
                4,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![SLICE]).unwrap()),
        ));
        let request = length_request(
            types,
            POINTER,
            RESULT,
            vec![
                projection(SemanticProjectionKindV1::Dereference, tail),
                projection(SemanticProjectionKindV1::Field(0), SLICE),
            ],
            SLICE,
        );
        if metadata == SemanticPointerMetadataV1::SliceLength {
            roundtrip(request);
        } else {
            assert_eq!(admit(request).unwrap_err(), length_error());
        }
    }
}

#[test]
fn slice_length_keeps_exact_projected_type_validation() {
    assert_eq!(
        admit(length_request(
            base_types(SemanticPointerMetadataV1::SliceLength, false),
            POINTER,
            RESULT,
            vec![projection(SemanticProjectionKindV1::Dereference, ELEMENT)],
            ELEMENT
        ))
        .unwrap_err(),
        SemanticMirErrorV1::TypeMismatch {
            expected: SLICE,
            actual: ELEMENT,
            location: location()
        }
    );
    assert_eq!(
        admit(length_request(
            base_types(SemanticPointerMetadataV1::SliceLength, false),
            POINTER,
            RESULT,
            vec![
                projection(SemanticProjectionKindV1::Dereference, SLICE),
                projection(SemanticProjectionKindV1::OpaqueCast, ELEMENT)
            ],
            ELEMENT
        ))
        .unwrap_err(),
        SemanticMirErrorV1::InvalidTypeOperation {
            operation: SemanticTypeOperationV1::Projection,
            location: location()
        }
    );
}

#[test]
fn slice_length_does_not_accept_whole_pointer_or_metadata_free_slice_local() {
    assert_eq!(
        admit(length_request(
            base_types(SemanticPointerMetadataV1::SliceLength, false),
            POINTER,
            RESULT,
            vec![],
            POINTER
        ))
        .unwrap_err(),
        length_error()
    );
    assert_eq!(
        admit(length_request(
            vec![u32_type(1), slice_type(), u64_type(3)],
            SLICE,
            SemanticTypeIdV1::from_index(2),
            vec![],
            SLICE
        ))
        .unwrap_err(),
        length_error()
    );
}

#[test]
fn scalar_and_str_places_do_not_gain_slice_length() {
    let pointer = SemanticTypeIdV1::from_index(1);
    let result = SemanticTypeIdV1::from_index(2);
    let scalar = length_request(
        vec![
            u32_type(1),
            carrier(
                2,
                ELEMENT,
                4,
                4,
                SemanticPointerMetadataV1::SliceLength,
                false,
            ),
            u64_type(3),
        ],
        pointer,
        result,
        vec![projection(SemanticProjectionKindV1::Dereference, ELEMENT)],
        ELEMENT,
    );
    assert_eq!(admit(scalar).unwrap_err(), length_error());
    let str_type = SemanticTypeDeclV1::new(
        type_identity(1),
        layout_identity(1),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::array(1, 0),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(false),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Opaque,
    )
    .with_rust_type_kind(SemanticRustTypeKindV1::Str);
    assert_eq!(
        admit(length_request(
            vec![
                str_type,
                carrier(
                    2,
                    ELEMENT,
                    0,
                    1,
                    SemanticPointerMetadataV1::SliceLength,
                    false
                ),
                u64_type(3)
            ],
            pointer,
            result,
            vec![projection(SemanticProjectionKindV1::Dereference, ELEMENT)],
            ELEMENT
        ))
        .unwrap_err(),
        length_error()
    );
}

#[test]
fn array_length_preserves_unsigned_result_width_rule() {
    for output in [u32_type(3), u64_type(3)] {
        let array = SemanticTypeDeclV1::new(
            type_identity(2),
            layout_identity(2),
            sequence_layout(Some(4)),
            SemanticTypeShapeV1::Array {
                element: ELEMENT,
                length: 4,
            },
        );
        roundtrip(length_request(
            vec![u32_type(1), array, output],
            SLICE,
            SemanticTypeIdV1::from_index(2),
            vec![],
            SLICE,
        ));
    }
}

#[test]
fn slice_length_metadata_reuses_the_exact_projection_resource_boundary() {
    let request = || {
        length_request(
            base_types(SemanticPointerMetadataV1::SliceLength, false),
            POINTER,
            RESULT,
            vec![projection(SemanticProjectionKindV1::Dereference, SLICE)],
            SLICE,
        )
    };
    let exact = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::Projections, 1)
        .unwrap();
    let owner = request().admit_current_production(exact).unwrap();
    let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        owner.canonical_encoding(),
        exact,
    )
    .unwrap();
    assert_eq!(decoded.canonical_encoding(), owner.canonical_encoding());
    let short = exact
        .with_limit(SemanticMirResourceV1::Projections, 0)
        .unwrap();
    let expected = SemanticMirErrorV1::LimitExceeded {
        resource: SemanticMirResourceV1::Projections,
        actual: 1,
        max: 0,
    };
    assert_eq!(
        request().admit_current_production(short).unwrap_err(),
        expected
    );
    assert_eq!(
        AdmittedInertSemanticMirV1::decode_current_production_canonical(
            owner.canonical_encoding(),
            short,
        )
        .unwrap_err(),
        expected
    );
}
