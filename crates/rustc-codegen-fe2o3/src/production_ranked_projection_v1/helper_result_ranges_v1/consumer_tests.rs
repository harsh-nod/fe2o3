use super::{MAX_PROJECTED_LOOP_GRAPH_WORK_V1, SemanticAssertProofsV1};
use fe2o3_mir_model::semantic_mir_v1::*;

fn fixture() -> (Vec<SemanticTypeDeclV1>, SemanticFunctionDeclV1) {
    let u32_ty = SemanticTypeIdV1::from_index(0);
    let u64_ty = SemanticTypeIdV1::from_index(1);
    let bool_ty = SemanticTypeIdV1::from_index(2);
    let pair_ty = SemanticTypeIdV1::from_index(3);
    let shapes = [
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![u64_ty, bool_ty]).unwrap()),
    ];
    let types = shapes
        .into_iter()
        .enumerate()
        .map(|(i, shape)| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([i as u8 + 1; 32]),
                SemanticLayoutIdentityV1::from_sha256([i as u8 + 1; 32]),
                SemanticTypeLayoutV1::new(Some([4, 8, 1, 16][i]), [4, 8, 1, 8][i]).unwrap(),
                shape,
            )
        })
        .collect();
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let copy = |local, ty| SemanticOperandV1::Copy(place(local, ty));
    let scale = SemanticOperandV1::Constant(SemanticConstantV1::new(
        u64_ty,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(256, 8).unwrap()),
    ));
    let source = SemanticSourceProvenanceV1::unavailable();
    let assign = |local, ty, kind| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(local, ty),
                SemanticRvalueV1::new(ty, kind),
            )),
        )
    };
    let mut blocks = vec![
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([1; 32]),
            source,
            vec![
                assign(
                    2,
                    u64_ty,
                    SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::Integer,
                        operand: copy(1, u32_ty),
                    },
                ),
                assign(
                    3,
                    pair_ty,
                    SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                        SemanticCheckedBinaryOpV1::Multiply,
                        copy(2, u64_ty),
                        scale.clone(),
                    )),
                ),
            ],
            SemanticTerminatorV1::new(
                source,
                SemanticTerminatorKindV1::Assert {
                    condition: SemanticOperandV1::Copy(
                        SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(3),
                            vec![
                                SemanticProjectionV1::new(
                                    SemanticProjectionKindV1::Field(1),
                                    bool_ty,
                                )
                                .unwrap(),
                            ],
                            bool_ty,
                        )
                        .unwrap(),
                    ),
                    expected: false,
                    message: SemanticAssertMessageV1::Overflow {
                        operation: SemanticBinaryOpV1::Multiply,
                        left: copy(2, u64_ty),
                        right: scale,
                    },
                    target: SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::AssertSuccess,
                        SemanticBlockIdV1::from_index(1),
                    ),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ),
        )
        .unwrap(),
    ];
    // Reachable, large and irrelevant to the already total u32-as-u64 product.
    for i in 1..=128 {
        blocks.push(
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([i as u8 + 1; 32]),
                source,
                vec![SemanticStatementV1::new(source, SemanticStatementKindV1::Nop); 512],
                SemanticTerminatorV1::new(
                    source,
                    if i == 128 {
                        SemanticTerminatorKindV1::Return
                    } else {
                        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::Goto,
                            SemanticBlockIdV1::from_index(i + 1),
                        ))
                    },
                ),
            )
            .unwrap(),
        );
    }
    let locals = [u64_ty, u32_ty, u64_ty, pair_ty]
        .into_iter()
        .enumerate()
        .map(|(i, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([i as u8 + 20; 32]),
                ty,
                if i == 0 {
                    SemanticLocalRoleV1::Return
                } else if i == 1 {
                    SemanticLocalRoleV1::Argument(0)
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                source,
            )
        })
        .collect();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([11; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([12; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([13; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([14; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([15; 32]),
        source,
        SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256([16; 32]),
            SemanticLayoutIdentityV1::from_sha256([16; 32]),
            SemanticCanonAbiV1::GpuKernel,
            false,
            false,
            vec![SemanticAbiValueV1::new(
                u32_ty,
                SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
            )],
            SemanticAbiValueV1::new(u64_ty, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap(),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    (types, function)
}

#[test]
fn helper_result_ranges_existing_sufficient_proof_ignores_large_unrelated_cfg() {
    let (types, function) = fixture();
    let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
    proof.work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - 32_768;
    let SemanticTerminatorKindV1::Assert {
        condition,
        expected,
        message,
        ..
    } = function.blocks()[0].terminator().kind()
    else {
        panic!()
    };
    assert!(
        proof
            .proves_checked_overflow_assert_v1(condition, *expected, message, 0)
            .unwrap()
    );
    assert!(
        proof.helper_result_ranges.is_none(),
        "sufficient existing proof must not start optional precision"
    );
    assert!(proof.work < MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
}

#[test]
fn helper_result_ranges_ordinary_nonexact_range_queries_do_not_start_analysis() {
    let (types, function) = fixture();
    let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
    proof.work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - 32_768;
    let SemanticStatementKindV1::Assign(assignment) = function.blocks()[0].statements()[0].kind()
    else {
        panic!()
    };
    let SemanticRvalueKindV1::Cast { operand, .. } = assignment.value().kind() else {
        panic!()
    };
    let range = proof.range_at_operand(operand, 0, 0).unwrap().unwrap();
    assert_eq!((range.minimum, range.maximum), (0, u32::MAX.into()));
    assert!(proof.helper_result_ranges.is_none());
}
