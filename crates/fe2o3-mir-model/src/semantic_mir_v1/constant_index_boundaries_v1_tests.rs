use super::*;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const BYTE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const SEQUENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const POINTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);

fn projection(
    offset: u64,
    minimum_length: u64,
    from_end: bool,
) -> Result<SemanticProjectionV1, SemanticMirErrorV1> {
    SemanticProjectionV1::new(
        SemanticProjectionKindV1::ConstantIndex {
            offset,
            minimum_length,
            from_end,
        },
        BYTE,
    )
}

#[test]
fn constant_index_constructor_checks_direction_specific_endpoints() {
    for minimum in [0, 1, 2, 17, u64::MAX] {
        for offset in [
            0,
            1,
            minimum.saturating_sub(1),
            minimum,
            minimum.saturating_add(1),
            u64::MAX,
        ] {
            assert_eq!(
                projection(offset, minimum, false).is_ok(),
                offset < minimum,
                "forward {offset}/{minimum}"
            );
            assert_eq!(
                projection(offset, minimum, true).is_ok(),
                offset > 0 && offset <= minimum,
                "reverse {offset}/{minimum}"
            );
        }
    }
}

#[test]
fn reverse_one_is_the_last_element_even_for_minimum_one() {
    let actual = projection(1, 1, true).unwrap();
    assert_eq!(
        actual.kind(),
        SemanticProjectionKindV1::ConstantIndex {
            offset: 1,
            minimum_length: 1,
            from_end: true
        }
    );
    assert_eq!(
        projection(0, 1, true),
        Err(SemanticMirErrorV1::InvalidProjectionShape)
    );
    assert_eq!(
        projection(1, 1, false),
        Err(SemanticMirErrorV1::InvalidProjectionShape)
    );
}

fn declaration(
    tag: u8,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        layout,
        shape,
    )
}

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}

fn request(array: bool, projection: SemanticProjectionV1) -> InertSemanticMirRequestV1 {
    let target_id = SemanticLayoutIdentityV1::from_sha256([250; 32]);
    let source = SemanticSourceProvenanceV1::unavailable();
    let unit_layout = SemanticTypeLayoutV1::with_exact_rustc_layout(
        0,
        1,
        SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
        SemanticRustcVariantsV1::Single { index: 0 },
        SemanticBackendReprV1::memory(true),
        None,
        false,
        None,
        1,
        0,
        SemanticTypeLayoutDetailsV1::None,
    )
    .unwrap();
    let byte_layout = SemanticTypeLayoutV1::new_with_backend_repr(
        Some(1),
        1,
        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 8, 1),
            SemanticScalarValidityRangeV1::new(0, 255),
        )),
        false,
    )
    .unwrap();
    let sequence_layout = SemanticTypeLayoutV1::with_exact_rustc_layout(
        if array { 2 } else { 0 },
        1,
        SemanticFieldsShapeV1::array(1, if array { 2 } else { 0 }),
        SemanticRustcVariantsV1::Single { index: 0 },
        SemanticBackendReprV1::memory(array),
        None,
        false,
        None,
        1,
        0,
        SemanticTypeLayoutDetailsV1::None,
    )
    .unwrap();
    let pointer_scalar = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)),
    );
    let length_scalar = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)),
    );
    let pointer_layout = SemanticTypeLayoutV1::new_with_backend_repr(
        Some(if array { 8 } else { 16 }),
        8,
        if array {
            SemanticBackendReprV1::scalar(pointer_scalar)
        } else {
            SemanticBackendReprV1::scalar_pair(pointer_scalar, length_scalar)
        },
        false,
    )
    .unwrap();
    let types = vec![
        declaration(1, unit_layout, SemanticTypeShapeV1::Unit),
        declaration(
            2,
            byte_layout,
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 8,
            }),
        ),
        declaration(
            3,
            sequence_layout,
            if array {
                SemanticTypeShapeV1::Array {
                    element: BYTE,
                    length: 2,
                }
            } else {
                SemanticTypeShapeV1::Slice { element: BYTE }
            },
        ),
        declaration(
            4,
            pointer_layout,
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new(
                    SEQUENCE,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    if array {
                        SemanticPointerMetadataV1::None
                    } else {
                        SemanticPointerMetadataV1::SliceLength
                    },
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
                None,
            ),
        ),
    ];
    let argument_attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([10; 32]),
        target_id,
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            POINTER,
            if array {
                SemanticAbiPassModeV1::Direct(argument_attributes)
            } else {
                SemanticAbiPassModeV1::Pair {
                    first: argument_attributes,
                    second: argument_attributes,
                }
            },
        ))],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap();
    let locals = vec![
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([20; 32]),
            UNIT,
            SemanticLocalRoleV1::Return,
            source,
        ),
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([21; 32]),
            POINTER,
            SemanticLocalRoleV1::Argument(0),
            source,
        ),
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([22; 32]),
            BYTE,
            SemanticLocalRoleV1::Temporary,
            source,
        ),
    ];
    let projected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SEQUENCE).unwrap(),
            projection,
        ],
        projection.result_type(),
    )
    .unwrap();
    let block = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([30; 32]),
        source,
        vec![
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    place(2, BYTE),
                    SemanticRvalueV1::new(
                        BYTE,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected)),
                    ),
                )),
            ),
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    place(0, UNIT),
                    SemanticRvalueV1::new(
                        UNIT,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                            SemanticConstantV1::new(UNIT, SemanticConstantValueV1::ZeroSized),
                        )),
                    ),
                )),
            ),
        ],
        SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([40; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([41; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([42; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([43; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([44; 32]),
        source,
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap();
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(target_id),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticCallableDeclV1::defined(
            SemanticFunctionIdV1::from_index(0),
        )],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

#[test]
fn forward_and_reverse_slice_projections_are_admitted_and_round_trip() {
    for (offset, minimum, from_end) in [
        (0, 1, false),
        (1, 1, true),
        (2, 2, true),
        (u64::MAX, u64::MAX, true),
    ] {
        let expected = projection(offset, minimum, from_end).unwrap();
        let admitted = request(false, expected)
            .admit_minimal_compatible(SemanticMirLimitsV1::default())
            .unwrap();
        let decoded = AdmittedInertSemanticMirV1::decode_minimal_compatible_canonical(
            admitted.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(admitted.canonical_encoding(), decoded.canonical_encoding());
        assert_eq!(admitted.semantic_sha256(), decoded.semantic_sha256());
        let SemanticStatementKindV1::Assign(assignment) =
            decoded.functions()[0].blocks()[0].statements()[0].kind()
        else {
            panic!("source assignment changed")
        };
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) = assignment.value().kind()
        else {
            panic!("source slice operand changed")
        };
        assert_eq!(place.projections().last(), Some(&expected));
    }
}

#[test]
fn array_fixture_uses_forward_index_and_retains_static_minimum_bound() {
    request(true, projection(1, 2, false).unwrap())
        .admit_minimal_compatible(SemanticMirLimitsV1::default())
        .unwrap();
    assert!(matches!(
        request(true, projection(1, 3, false).unwrap())
            .admit_minimal_compatible(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeOperation {
            operation: SemanticTypeOperationV1::Projection,
            ..
        })
    ));
}

#[test]
fn slice_projection_keeps_exact_element_and_base_type_checks() {
    let wrong_element = SemanticProjectionV1::new(
        SemanticProjectionKindV1::ConstantIndex {
            offset: 1,
            minimum_length: 1,
            from_end: true,
        },
        UNIT,
    )
    .unwrap();
    assert!(
        request(false, wrong_element)
            .admit_minimal_compatible(SemanticMirLimitsV1::default())
            .is_err()
    );
    let mut wrong_base = request(false, projection(1, 1, true).unwrap());
    let mut functions = wrong_base.functions.to_vec();
    let mut blocks = functions[0].blocks.to_vec();
    let mut statements = blocks[0].statements.to_vec();
    let wrong_place = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(2),
        vec![projection(1, 1, true).unwrap()],
        BYTE,
    )
    .unwrap();
    statements[0] = SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(2, BYTE),
            SemanticRvalueV1::new(
                BYTE,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(wrong_place)),
            ),
        )),
    );
    blocks[0].statements = statements.into_boxed_slice();
    functions[0].blocks = blocks.into_boxed_slice();
    wrong_base.functions = functions.into_boxed_slice();
    assert!(matches!(
        wrong_base.admit_minimal_compatible(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeOperation {
            operation: SemanticTypeOperationV1::Projection,
            ..
        })
    ));
}

#[test]
fn exact_slice_and_array_fixtures_admit_and_round_trip_under_current_production() {
    for (array, offset, minimum, from_end) in [
        (false, 0, 1, false),
        (false, 1, 1, true),
        (false, 2, 2, true),
        (false, u64::MAX, u64::MAX, true),
        (true, 1, 2, false),
    ] {
        let expected = projection(offset, minimum, from_end).unwrap();
        let admitted = request(array, expected)
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap();
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            admitted.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(admitted.wire_version(), decoded.wire_version());
        assert_eq!(admitted.canonical_encoding(), decoded.canonical_encoding());
        assert_eq!(admitted.semantic_sha256(), decoded.semantic_sha256());
        let SemanticStatementKindV1::Assign(assignment) =
            decoded.functions()[0].blocks()[0].statements()[0].kind()
        else {
            panic!("current-profile source assignment changed")
        };
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) = assignment.value().kind()
        else {
            panic!("current-profile source operand changed")
        };
        assert_eq!(place.projections().last(), Some(&expected));
    }
}

#[test]
fn current_production_retains_array_bounds_and_slice_element_rejection() {
    assert!(matches!(
        request(true, projection(1, 3, false).unwrap())
            .admit_current_production(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeOperation {
            operation: SemanticTypeOperationV1::Projection,
            ..
        })
    ));
    let wrong = SemanticProjectionV1::new(
        SemanticProjectionKindV1::ConstantIndex {
            offset: 1,
            minimum_length: 1,
            from_end: true,
        },
        UNIT,
    )
    .unwrap();
    assert!(
        request(false, wrong)
            .admit_current_production(SemanticMirLimitsV1::default())
            .is_err()
    );
}
