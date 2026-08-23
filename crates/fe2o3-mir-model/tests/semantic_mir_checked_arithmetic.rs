use fe2o3_mir_model::semantic_mir_v1::*;

const MAGIC: &[u8] = b"fe2o3.inert-semantic-mir";
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const U32_BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const I32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const F32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const BOOL_U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const U32_U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const U32_BOOL_BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
const VALID_U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);
const VALID_U32_BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);

fn identity(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn scalar_layout(
    size_bytes: u64,
    alignment_bytes: u64,
    primitive: SemanticBackendPrimitiveV1,
    valid_range: SemanticScalarValidityRangeV1,
) -> SemanticTypeLayoutV1 {
    SemanticTypeLayoutV1::new_with_backend_repr(
        Some(size_bytes),
        alignment_bytes,
        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(primitive, valid_range)),
        false,
    )
    .unwrap()
}

fn scalar_type(
    tag: u8,
    layout: SemanticTypeLayoutV1,
    shape: SemanticScalarTypeV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity(tag)),
        SemanticLayoutIdentityV1::from_sha256(identity(tag)),
        layout,
        SemanticTypeShapeV1::Scalar(shape),
    )
}

fn tuple_type(
    tag: u8,
    fields: Vec<SemanticTypeIdV1>,
    field_offsets: Vec<u64>,
    padding: Vec<SemanticPaddingV1>,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity(tag)),
        SemanticLayoutIdentityV1::from_sha256(identity(tag)),
        SemanticTypeLayoutV1::aggregate(
            Some(8),
            4,
            SemanticAggregateLayoutV1::new(field_offsets, padding).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(fields).unwrap()),
    )
}

fn types() -> Vec<SemanticTypeDeclV1> {
    vec![
        scalar_type(
            1,
            scalar_layout(
                4,
                4,
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarValidityRangeV1::new(0, u128::from(u32::MAX)),
            ),
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
        ),
        scalar_type(
            2,
            scalar_layout(
                1,
                1,
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, 1),
            ),
            SemanticScalarTypeV1::Bool,
        ),
        tuple_type(
            3,
            vec![U32, BOOL],
            vec![0, 4],
            vec![SemanticPaddingV1::new(5, 3).unwrap()],
        ),
        scalar_type(
            4,
            scalar_layout(
                4,
                4,
                SemanticBackendPrimitiveV1::integer(true, 32, 4),
                SemanticScalarValidityRangeV1::new(0, u128::from(u32::MAX)),
            ),
            SemanticScalarTypeV1::Integer {
                signed: true,
                bits: 32,
            },
        ),
        scalar_type(
            5,
            scalar_layout(
                4,
                4,
                SemanticBackendPrimitiveV1::float(32, 4),
                SemanticScalarValidityRangeV1::new(0, u128::from(u32::MAX)),
            ),
            SemanticScalarTypeV1::Float { bits: 32 },
        ),
        tuple_type(
            6,
            vec![BOOL, U32],
            vec![4, 0],
            vec![SemanticPaddingV1::new(5, 3).unwrap()],
        ),
        tuple_type(7, vec![U32, U32], vec![0, 4], vec![]),
        tuple_type(
            8,
            vec![U32, BOOL, BOOL],
            vec![0, 4, 5],
            vec![SemanticPaddingV1::new(6, 2).unwrap()],
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(identity(9)),
            SemanticLayoutIdentityV1::from_sha256(identity(9)),
            scalar_layout(
                4,
                4,
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarValidityRangeV1::new(0, u128::from(u32::MAX)),
            ),
            SemanticTypeShapeV1::ValidityScalar(
                SemanticValidityScalarTypeV1::new(
                    SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 32,
                    },
                    vec![SemanticScalarValidityRangeV1::new(0, u128::from(u32::MAX))],
                )
                .unwrap(),
            ),
        ),
        tuple_type(
            10,
            vec![VALID_U32, BOOL],
            vec![0, 4],
            vec![SemanticPaddingV1::new(5, 3).unwrap()],
        ),
    ]
}

fn direct_value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}

fn operand(ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    let size = if ty == BOOL { 1 } else { 4 };
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, size).unwrap()),
    ))
}

fn request(
    checked: Option<SemanticCheckedBinaryOpV1>,
    left: SemanticTypeIdV1,
    right: SemanticTypeIdV1,
    result: SemanticTypeIdV1,
) -> InertSemanticMirRequestV1 {
    request_with_operands(
        checked.map(|operation| (operation, operand(left), operand(right))),
        result,
    )
}

fn request_with_operands(
    checked: Option<(
        SemanticCheckedBinaryOpV1,
        SemanticOperandV1,
        SemanticOperandV1,
    )>,
    result: SemanticTypeIdV1,
) -> InertSemanticMirRequestV1 {
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(identity(20)),
        SemanticLayoutIdentityV1::from_sha256(identity(21)),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![],
        direct_value(U32),
    )
    .unwrap();
    let mut locals = vec![SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(identity(30)),
        U32,
        SemanticLocalRoleV1::Return,
        SemanticSourceProvenanceV1::unavailable(),
    )];
    for index in 0..10_u32 {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(identity(31 + index as u8)),
            SemanticTypeIdV1::from_index(index),
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        ));
    }
    let statement = checked.map_or(SemanticStatementKindV1::Nop, |(operation, left, right)| {
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(result.index() + 1),
                vec![],
                result,
            )
            .unwrap(),
            SemanticRvalueV1::new(
                result,
                SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                    operation, left, right,
                )),
            ),
        ))
    });
    let block = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(identity(50)),
        SemanticSourceProvenanceV1::unavailable(),
        vec![SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            statement,
        )],
        SemanticTerminatorV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticTerminatorKindV1::Return,
        ),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(identity(51)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(identity(52)),
        SemanticMonomorphizationIdentityV1::from_sha256(identity(53)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(identity(54)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(identity(55)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap();
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(identity(60))),
        types(),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

fn full_valid_range(bits: u16) -> SemanticScalarValidityRangeV1 {
    let maximum = if bits == 128 {
        u128::MAX
    } else {
        (1_u128 << bits) - 1
    };
    SemanticScalarValidityRangeV1::new(0, maximum)
}

fn integer_request(
    signed: bool,
    bits: u16,
    size_bytes: u8,
    identity_tag: u8,
) -> InertSemanticMirRequestV1 {
    let integer = SemanticTypeIdV1::from_index(0);
    let boolean = SemanticTypeIdV1::from_index(1);
    let checked_result = SemanticTypeIdV1::from_index(2);
    let unit = SemanticTypeIdV1::from_index(3);
    let size = u64::from(size_bytes);
    let alignment = size.min(8);
    let tuple_size = (size + 1 + alignment - 1) & !(alignment - 1);
    let padding = (tuple_size > size + 1)
        .then(|| SemanticPaddingV1::new(size + 1, tuple_size - size - 1).unwrap())
        .into_iter()
        .collect();
    let types = vec![
        scalar_type(
            identity_tag,
            scalar_layout(
                size,
                alignment,
                SemanticBackendPrimitiveV1::integer(signed, bits, alignment),
                full_valid_range(bits),
            ),
            SemanticScalarTypeV1::Integer { signed, bits },
        ),
        scalar_type(
            identity_tag.wrapping_add(1),
            scalar_layout(
                1,
                1,
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, 1),
            ),
            SemanticScalarTypeV1::Bool,
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(identity(identity_tag.wrapping_add(2))),
            SemanticLayoutIdentityV1::from_sha256(identity(identity_tag.wrapping_add(2))),
            SemanticTypeLayoutV1::aggregate(
                Some(tuple_size),
                alignment,
                SemanticAggregateLayoutV1::new(vec![0, size], padding).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(
                SemanticAggregateTypeV1::new(vec![integer, boolean]).unwrap(),
            ),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(identity(identity_tag.wrapping_add(14))),
            SemanticLayoutIdentityV1::from_sha256(identity(identity_tag.wrapping_add(14))),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
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
            .unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
    ];
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(identity(identity_tag.wrapping_add(3))),
        SemanticLayoutIdentityV1::from_sha256(identity(identity_tag.wrapping_add(4))),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let locals = vec![
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(identity(identity_tag.wrapping_add(5))),
            unit,
            SemanticLocalRoleV1::Return,
            SemanticSourceProvenanceV1::unavailable(),
        ),
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(identity(identity_tag.wrapping_add(6))),
            checked_result,
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        ),
    ];
    let constant = || {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            integer,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, size_bytes).unwrap()),
        ))
    };
    let assignment = SemanticAssignmentV1::new(
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], checked_result).unwrap(),
        SemanticRvalueV1::new(
            checked_result,
            SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                SemanticCheckedBinaryOpV1::Add,
                constant(),
                constant(),
            )),
        ),
    );
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(identity(identity_tag.wrapping_add(7))),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(identity(identity_tag.wrapping_add(8))),
        SemanticMonomorphizationIdentityV1::from_sha256(identity(identity_tag.wrapping_add(9))),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(identity(
            identity_tag.wrapping_add(10),
        )),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(identity(
            identity_tag.wrapping_add(11),
        )),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256(identity(identity_tag.wrapping_add(12))),
                SemanticSourceProvenanceV1::unavailable(),
                vec![SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Assign(assignment),
                )],
                SemanticTerminatorV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticTerminatorKindV1::Return,
                ),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(identity(
            identity_tag.wrapping_add(13),
        ))),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

fn cumulative_checked_request(
    function_count: u32,
    statements_per_function: u32,
) -> InertSemanticMirRequestV1 {
    let mut functions = Vec::new();
    let mut roots = Vec::new();
    for function_index in 0..function_count {
        let tag = u8::try_from(100 + function_index * 50).unwrap();
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256(identity(tag)),
            SemanticLayoutIdentityV1::from_sha256(identity(tag + 1)),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![],
            direct_value(U32),
        )
        .unwrap();
        let mut locals = vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(identity(tag + 2)),
                U32,
                SemanticLocalRoleV1::Return,
                SemanticSourceProvenanceV1::unavailable(),
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(identity(tag + 3)),
                U32_BOOL,
                SemanticLocalRoleV1::Temporary,
                SemanticSourceProvenanceV1::unavailable(),
            ),
        ];
        for index in 0..10_u32 {
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(identity(
                    tag + 20 + u8::try_from(index).unwrap(),
                )),
                SemanticTypeIdV1::from_index(index),
                SemanticLocalRoleV1::Temporary,
                SemanticSourceProvenanceV1::unavailable(),
            ));
        }
        let statements = (0..statements_per_function)
            .map(|_| {
                SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], U32_BOOL)
                            .unwrap(),
                        SemanticRvalueV1::new(
                            U32_BOOL,
                            SemanticRvalueKindV1::CheckedBinary(
                                SemanticCheckedBinaryRvalueV1::new(
                                    SemanticCheckedBinaryOpV1::Multiply,
                                    operand(U32),
                                    operand(U32),
                                ),
                            ),
                        ),
                    )),
                )
            })
            .collect();
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(identity(tag + 4)),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256(identity(tag + 5)),
            SemanticMonomorphizationIdentityV1::from_sha256(identity(tag + 6)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(identity(tag + 7)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(identity(tag + 8)),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            vec![
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256(identity(tag + 9)),
                    SemanticSourceProvenanceV1::unavailable(),
                    statements,
                    SemanticTerminatorV1::new(
                        SemanticSourceProvenanceV1::unavailable(),
                        SemanticTerminatorKindV1::Return,
                    ),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        roots.push(SemanticFunctionIdV1::from_index(function_index));
        functions.push(function);
    }
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(identity(200))),
        types(),
        vec![],
        vec![],
        vec![],
        functions,
        roots,
    )
    .unwrap()
}

fn admitted(operation: SemanticCheckedBinaryOpV1) -> AdmittedInertSemanticMirV1 {
    request(Some(operation), U32, U32, U32_BOOL)
        .admit(SemanticMirLimitsV1::default())
        .unwrap()
}

fn wire_version(bytes: &[u8]) -> u16 {
    u16::from_le_bytes(bytes[MAGIC.len()..MAGIC.len() + 2].try_into().unwrap())
}

fn checked_kind(model: &AdmittedInertSemanticMirV1) -> &SemanticRvalueKindV1 {
    let SemanticStatementKindV1::Assign(assignment) =
        model.functions()[0].blocks()[0].statements()[0].kind()
    else {
        panic!("expected checked assignment")
    };
    assignment.value().kind()
}

#[test]
fn checked_add_sub_mul_round_trip_with_exact_value_overflow_contract() {
    let mut canonical_encodings = Vec::new();
    for operation in [
        SemanticCheckedBinaryOpV1::Add,
        SemanticCheckedBinaryOpV1::Subtract,
        SemanticCheckedBinaryOpV1::Multiply,
    ] {
        let original = admitted(operation);
        assert_eq!(
            wire_version(original.canonical_encoding()),
            INERT_SEMANTIC_MIR_VERSION_V3
        );
        let decoded = AdmittedInertSemanticMirV1::decode_canonical(
            original.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.canonical_encoding(), original.canonical_encoding());
        assert_eq!(decoded.semantic_sha256(), original.semantic_sha256());

        let SemanticRvalueKindV1::CheckedBinary(checked) = checked_kind(&decoded) else {
            panic!("checked arithmetic was erased during decoding")
        };
        assert_eq!(checked.operation(), operation);
        assert_eq!(checked.left().ty(), U32);
        assert_eq!(checked.right().ty(), U32);
        let mut visited = Vec::new();
        checked_kind(&decoded)
            .try_visit_operands::<()>(|operand| {
                visited.push(operand.ty());
                Ok(())
            })
            .unwrap();
        assert_eq!(visited, [U32, U32]);
        let SemanticTypeShapeV1::Tuple(result) = decoded.types()[U32_BOOL.index() as usize].shape()
        else {
            panic!("checked result is not a tuple")
        };
        assert_eq!(result.fields(), [U32, BOOL]);
        canonical_encodings.push(original.canonical_encoding().to_vec());
    }
    assert!(
        canonical_encodings
            .windows(2)
            .all(|pair| pair[0] != pair[1])
    );
}

#[test]
fn checked_copy_and_move_projections_round_trip_and_are_counted_exactly() {
    let projected_place = |field| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(U32_U32.index() + 1),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), U32).unwrap()],
            U32,
        )
        .unwrap()
    };
    let make_request = || {
        request_with_operands(
            Some((
                SemanticCheckedBinaryOpV1::Add,
                SemanticOperandV1::Copy(projected_place(0)),
                SemanticOperandV1::Move(projected_place(1)),
            )),
            U32_BOOL,
        )
    };
    let limits = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::Operands, 2)
        .unwrap()
        .with_limit(SemanticMirResourceV1::Projections, 2)
        .unwrap();
    let admitted = make_request().admit(limits).unwrap();
    let decoded =
        AdmittedInertSemanticMirV1::decode_canonical(admitted.canonical_encoding(), limits)
            .unwrap();
    let SemanticRvalueKindV1::CheckedBinary(checked) = checked_kind(&decoded) else {
        panic!("checked projection operands were erased")
    };
    let SemanticOperandV1::Copy(left) = checked.left() else {
        panic!("left Copy operand changed kind")
    };
    let SemanticOperandV1::Move(right) = checked.right() else {
        panic!("right Move operand changed kind")
    };
    assert_eq!(
        left.projections()[0].kind(),
        SemanticProjectionKindV1::Field(0)
    );
    assert_eq!(
        right.projections()[0].kind(),
        SemanticProjectionKindV1::Field(1)
    );

    let one_short = limits
        .with_limit(SemanticMirResourceV1::Projections, 1)
        .unwrap();
    assert_eq!(
        make_request().admit(one_short).unwrap_err(),
        SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::Projections,
            actual: 2,
            max: 1,
        }
    );
    assert_eq!(
        AdmittedInertSemanticMirV1::decode_canonical(admitted.canonical_encoding(), one_short,)
            .unwrap_err(),
        SemanticMirDecodeErrorV1::Validation(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::Projections,
            actual: 2,
            max: 1,
        })
    );
}

#[test]
fn checked_v3_encoding_has_a_frozen_length_and_digest() {
    let admitted = admitted(SemanticCheckedBinaryOpV1::Add);
    assert_eq!(wire_version(admitted.canonical_encoding()), 3);
    assert_eq!(admitted.canonical_encoding().len(), 2_900);
    assert_eq!(
        admitted.semantic_sha256().as_bytes(),
        &[
            41, 137, 142, 198, 191, 143, 224, 156, 18, 204, 235, 84, 68, 67, 114, 119, 83, 245,
            185, 10, 14, 248, 189, 164, 156, 129, 24, 242, 141, 103, 180, 154,
        ],
        "update only when the V3 checked-arithmetic grammar intentionally changes"
    );
}

#[test]
fn legacy_models_remain_v2_and_v3_without_v3_content_is_noncanonical() {
    let legacy = request(None, U32, U32, U32_BOOL)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    assert_eq!(
        wire_version(legacy.canonical_encoding()),
        INERT_SEMANTIC_MIR_VERSION_V2
    );
    let decoded = AdmittedInertSemanticMirV1::decode_canonical(
        legacy.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.canonical_encoding(), legacy.canonical_encoding());

    let mut nonminimal = legacy.canonical_encoding().to_vec();
    nonminimal[MAGIC.len()..MAGIC.len() + 2]
        .copy_from_slice(&INERT_SEMANTIC_MIR_VERSION_V3.to_le_bytes());
    assert_eq!(
        AdmittedInertSemanticMirV1::decode_canonical(&nonminimal, SemanticMirLimitsV1::default())
            .unwrap_err(),
        SemanticMirDecodeErrorV1::NonCanonical
    );
}

#[test]
fn checked_rvalue_is_not_decodable_under_v2() {
    let original = admitted(SemanticCheckedBinaryOpV1::Add);
    let mut downgraded = original.canonical_encoding().to_vec();
    downgraded[MAGIC.len()..MAGIC.len() + 2]
        .copy_from_slice(&INERT_SEMANTIC_MIR_VERSION_V2.to_le_bytes());
    assert!(matches!(
        AdmittedInertSemanticMirV1::decode_canonical(&downgraded, SemanticMirLimitsV1::default()),
        Err(SemanticMirDecodeErrorV1::InvalidTag {
            context: "rvalue",
            value: 10,
            ..
        })
    ));
}

#[test]
fn invalid_checked_opcode_and_trailing_bytes_fail_closed() {
    let add = admitted(SemanticCheckedBinaryOpV1::Add);
    let subtract = admitted(SemanticCheckedBinaryOpV1::Subtract);
    let differing: Vec<_> = add
        .canonical_encoding()
        .iter()
        .zip(subtract.canonical_encoding())
        .enumerate()
        .filter_map(|(index, (left, right))| (left != right).then_some(index))
        .collect();
    assert_eq!(differing.len(), 1, "only the checked opcode may differ");
    let mut invalid = add.canonical_encoding().to_vec();
    invalid[differing[0]] = 3;
    assert!(matches!(
        AdmittedInertSemanticMirV1::decode_canonical(&invalid, SemanticMirLimitsV1::default()),
        Err(SemanticMirDecodeErrorV1::InvalidTag {
            context: "checked binary operation",
            value: 3,
            ..
        })
    ));

    let mut trailing = add.canonical_encoding().to_vec();
    trailing.push(0);
    assert!(matches!(
        AdmittedInertSemanticMirV1::decode_canonical(&trailing, SemanticMirLimitsV1::default()),
        Err(SemanticMirDecodeErrorV1::TrailingBytes { trailing: 1, .. })
    ));
}

#[test]
fn every_v3_truncation_and_single_bit_mutation_is_panic_total() {
    let original = admitted(SemanticCheckedBinaryOpV1::Multiply);
    let encoded = original.canonical_encoding();
    for end in 0..encoded.len() {
        assert!(
            AdmittedInertSemanticMirV1::decode_canonical(
                &encoded[..end],
                SemanticMirLimitsV1::default()
            )
            .is_err(),
            "accepted truncation at byte {end}"
        );
    }
    for byte in 0..encoded.len() {
        for bit in 0..8 {
            let mut mutated = encoded.to_vec();
            mutated[byte] ^= 1 << bit;
            let result = std::panic::catch_unwind(|| {
                AdmittedInertSemanticMirV1::decode_canonical(
                    &mutated,
                    SemanticMirLimitsV1::default(),
                )
            });
            let decoded = result
                .unwrap_or_else(|_| panic!("decoder panicked for mutated byte {byte}, bit {bit}"));
            if let Ok(decoded) = decoded {
                assert_eq!(decoded.canonical_encoding(), mutated);
            }
        }
    }
}

#[test]
fn checked_arithmetic_rejects_every_result_contract_and_operand_type_mutation() {
    for (left, right, result) in [
        (U32, U32, U32),
        (U32, U32, BOOL_U32),
        (U32, U32, U32_U32),
        (U32, U32, U32_BOOL_BOOL),
        (U32, I32, U32_BOOL),
        (F32, F32, U32_BOOL),
        (BOOL, BOOL, U32_BOOL),
    ] {
        assert!(matches!(
            request(Some(SemanticCheckedBinaryOpV1::Add), left, right, result,)
                .admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidTypeOperation {
                operation: SemanticTypeOperationV1::CheckedBinary,
                ..
            })
        ));
    }
}

#[test]
fn checked_arithmetic_rejects_restricted_validity_integer_shapes() {
    for (left, right, result) in [
        (VALID_U32, VALID_U32, VALID_U32_BOOL),
        (VALID_U32, VALID_U32, U32_BOOL),
        (U32, U32, VALID_U32_BOOL),
    ] {
        assert!(matches!(
            request(
                Some(SemanticCheckedBinaryOpV1::Multiply),
                left,
                right,
                result,
            )
            .admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidTypeOperation {
                operation: SemanticTypeOperationV1::CheckedBinary,
                ..
            })
        ));
    }
}

#[test]
fn checked_arithmetic_accepts_signed_narrow_wide_and_pointer_sized_integers() {
    for (name, signed, bits, size, tag) in [
        ("u8", false, 8, 1, 11),
        ("i8", true, 8, 1, 31),
        ("u16", false, 16, 2, 51),
        ("i16", true, 16, 2, 71),
        ("u32", false, 32, 4, 91),
        ("i32", true, 32, 4, 111),
        ("u64", false, 64, 8, 131),
        ("i64", true, 64, 8, 151),
        ("u128", false, 128, 16, 171),
        ("i128", true, 128, 16, 191),
        ("usize-gfx942", false, 64, 8, 211),
        ("isize-gfx942", true, 64, 8, 231),
    ] {
        let admitted = integer_request(signed, bits, size, tag)
            .admit(SemanticMirLimitsV1::default())
            .unwrap_or_else(|error| panic!("{name} checked arithmetic was rejected: {error}"));
        let decoded = AdmittedInertSemanticMirV1::decode_canonical(
            admitted.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap_or_else(|error| panic!("{name} checked arithmetic did not decode: {error}"));
        assert_eq!(decoded.semantic_sha256(), admitted.semantic_sha256());
    }
}

#[test]
fn checked_limits_have_exact_operand_and_canonical_byte_boundaries() {
    let original = admitted(SemanticCheckedBinaryOpV1::Subtract);
    let encoded = original.canonical_encoding();
    let encoded_len = u64::try_from(encoded.len()).unwrap();
    let exact_bytes = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::CanonicalBytes, encoded_len)
        .unwrap();
    let exact_admission = request(
        Some(SemanticCheckedBinaryOpV1::Subtract),
        U32,
        U32,
        U32_BOOL,
    )
    .admit(exact_bytes)
    .unwrap();
    assert_eq!(exact_admission.canonical_encoding(), encoded);
    assert!(AdmittedInertSemanticMirV1::decode_canonical(encoded, exact_bytes).is_ok());
    let short_bytes = exact_bytes
        .with_limit(SemanticMirResourceV1::CanonicalBytes, encoded_len - 1)
        .unwrap();
    assert_eq!(
        AdmittedInertSemanticMirV1::decode_canonical(encoded, short_bytes).unwrap_err(),
        SemanticMirDecodeErrorV1::InputLimitExceeded {
            actual: encoded_len,
            max: encoded_len - 1,
        }
    );
    assert_eq!(
        request(
            Some(SemanticCheckedBinaryOpV1::Subtract),
            U32,
            U32,
            U32_BOOL,
        )
        .admit(short_bytes)
        .unwrap_err(),
        SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            actual: encoded_len,
            max: encoded_len - 1,
        }
    );

    let exact_operands = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::Operands, 2)
        .unwrap();
    assert!(
        request(
            Some(SemanticCheckedBinaryOpV1::Subtract),
            U32,
            U32,
            U32_BOOL,
        )
        .admit(exact_operands)
        .is_ok()
    );
    assert!(AdmittedInertSemanticMirV1::decode_canonical(encoded, exact_operands).is_ok());
}

#[test]
fn checked_operand_limits_accumulate_across_statements_and_functions() {
    let exact = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::Operands, 8)
        .unwrap();
    let admitted = cumulative_checked_request(2, 2).admit(exact).unwrap();
    assert!(
        AdmittedInertSemanticMirV1::decode_canonical(admitted.canonical_encoding(), exact).is_ok()
    );

    let one_short = exact
        .with_limit(SemanticMirResourceV1::Operands, 7)
        .unwrap();
    assert_eq!(
        cumulative_checked_request(2, 2)
            .admit(one_short)
            .unwrap_err(),
        SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::Operands,
            actual: 8,
            max: 7,
        }
    );
    assert_eq!(
        AdmittedInertSemanticMirV1::decode_canonical(admitted.canonical_encoding(), one_short,)
            .unwrap_err(),
        SemanticMirDecodeErrorV1::Validation(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::Operands,
            actual: 8,
            max: 7,
        })
    );
}

#[test]
fn checked_validation_work_has_an_exact_admission_boundary() {
    const EXACT_CHECKED_VALIDATION_WORK: u64 = 159;
    let exact = SemanticMirLimitsV1::default()
        .with_limit(
            SemanticMirResourceV1::ValidationWork,
            EXACT_CHECKED_VALIDATION_WORK,
        )
        .unwrap();
    let admitted = request(Some(SemanticCheckedBinaryOpV1::Add), U32, U32, U32_BOOL)
        .admit(exact)
        .unwrap();
    assert!(
        AdmittedInertSemanticMirV1::decode_canonical(admitted.canonical_encoding(), exact,).is_ok()
    );
    let one_short = exact
        .with_limit(
            SemanticMirResourceV1::ValidationWork,
            EXACT_CHECKED_VALIDATION_WORK - 1,
        )
        .unwrap();
    assert_eq!(
        request(Some(SemanticCheckedBinaryOpV1::Add), U32, U32, U32_BOOL,)
            .admit(one_short)
            .unwrap_err(),
        SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            actual: EXACT_CHECKED_VALIDATION_WORK,
            max: EXACT_CHECKED_VALIDATION_WORK - 1,
        }
    );
    assert_eq!(
        AdmittedInertSemanticMirV1::decode_canonical(admitted.canonical_encoding(), one_short,)
            .unwrap_err(),
        SemanticMirDecodeErrorV1::Validation(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            actual: EXACT_CHECKED_VALIDATION_WORK,
            max: EXACT_CHECKED_VALIDATION_WORK - 1,
        })
    );
}

#[test]
fn checked_operands_are_bounded_during_admission_and_decode() {
    let limits = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::Operands, 1)
        .unwrap();
    assert_eq!(
        request(
            Some(SemanticCheckedBinaryOpV1::Subtract),
            U32,
            U32,
            U32_BOOL,
        )
        .admit(limits)
        .unwrap_err(),
        SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::Operands,
            actual: 2,
            max: 1,
        }
    );

    let encoded = admitted(SemanticCheckedBinaryOpV1::Subtract)
        .canonical_encoding()
        .to_vec();
    assert_eq!(
        AdmittedInertSemanticMirV1::decode_canonical(&encoded, limits).unwrap_err(),
        SemanticMirDecodeErrorV1::Validation(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::Operands,
            actual: 2,
            max: 1,
        })
    );
}
