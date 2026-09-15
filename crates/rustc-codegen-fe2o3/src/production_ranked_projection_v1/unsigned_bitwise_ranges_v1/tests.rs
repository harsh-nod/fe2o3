use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

const WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const PTR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const NARROW: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const SIGNED: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const OTHER_WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn copy(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, ty))
}
fn moved(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Move(place(local, ty))
}
fn scalar(ty: SemanticTypeIdV1, bits: u128, bytes: u8) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, bytes).unwrap()),
    ))
}
fn assign(local: u32, ty: SemanticTypeIdV1, kind: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, ty),
            SemanticRvalueV1::new(ty, kind),
        )),
    )
}
fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}
fn goto(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target))
}
fn switch(zero: u32, nonzero: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: copy(8, BOOL),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                edge(SemanticEdgeRoleV1::SwitchValue, zero),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, nonzero),
        )
        .unwrap(),
    }
}
fn field(index: u32) -> SemanticOperandV1 {
    let ty = if index == 0 { WORD } else { BOOL };
    SemanticOperandV1::Move(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(5),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty).unwrap()],
            ty,
        )
        .unwrap(),
    )
}
fn block(
    index: usize,
    statements: Vec<SemanticStatementV1>,
    terminal: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([index as u8 + 40; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminal),
    )
    .unwrap()
}
fn types(bits: u16) -> Vec<SemanticTypeDeclV1> {
    let bytes = u64::from(bits / 8);
    let integer = |tag, signed, bits: u16| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            SemanticTypeLayoutV1::new(Some(u64::from(bits / 8)), u64::from(bits / 8)).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }),
        )
    };
    vec![
        integer(1, false, bits),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([2; 32]),
            SemanticLayoutIdentityV1::from_sha256([2; 32]),
            SemanticTypeLayoutV1::new(Some(1), 1).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([3; 32]),
            SemanticLayoutIdentityV1::from_sha256([3; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(2 * bytes),
                bytes,
                SemanticAggregateLayoutV1::new(
                    vec![0, bytes],
                    if bytes == 1 {
                        vec![]
                    } else {
                        vec![SemanticPaddingV1::new(bytes + 1, bytes - 1).unwrap()]
                    },
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![WORD, BOOL]).unwrap()),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([4; 32]),
            SemanticLayoutIdentityV1::from_sha256([4; 32]),
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new(
                    WORD,
                    SemanticMutabilityV1::Mutable,
                    5,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ),
        integer(5, false, 32),
        integer(6, true, bits),
        integer(7, false, bits),
    ]
}

struct Fixture {
    types: Vec<SemanticTypeDeclV1>,
    locals: Vec<SemanticTypeIdV1>,
    blocks: Vec<SemanticBasicBlockV1>,
}
impl Fixture {
    fn new(bits: u16, offset: u128, alias: bool) -> Self {
        let bytes = (bits / 8) as u8;
        let input = copy(if alias { 4 } else { 3 }, WORD);
        Self {
            types: types(bits),
            locals: vec![WORD, WORD, WORD, WORD, WORD, PAIR, WORD, PTR, BOOL, WORD],
            blocks: vec![
                block(
                    0,
                    vec![
                        assign(
                            2,
                            WORD,
                            SemanticRvalueKindV1::Unary {
                                operation: SemanticUnaryOpV1::Not,
                                operand: scalar(WORD, 63, bytes),
                            },
                        ),
                        assign(
                            3,
                            WORD,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::BitAnd,
                                left: copy(1, WORD),
                                right: moved(2, WORD),
                            },
                        ),
                        assign(4, WORD, SemanticRvalueKindV1::Use(copy(3, WORD))),
                    ],
                    goto(1),
                ),
                block(
                    1,
                    vec![],
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable(
                            SemanticCallableIdV1::from_index(0),
                            vec![copy(1, WORD)],
                            Some(SemanticCallDestinationV1::new(
                                place(6, WORD),
                                edge(SemanticEdgeRoleV1::CallReturn, 2),
                            )),
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    ),
                ),
                block(
                    2,
                    vec![assign(
                        5,
                        PAIR,
                        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                            SemanticCheckedBinaryOpV1::Add,
                            input.clone(),
                            scalar(WORD, offset, bytes),
                        )),
                    )],
                    SemanticTerminatorKindV1::Assert {
                        condition: field(1),
                        expected: false,
                        message: SemanticAssertMessageV1::Overflow {
                            operation: SemanticBinaryOpV1::Add,
                            left: input,
                            right: scalar(WORD, offset, bytes),
                        },
                        target: edge(SemanticEdgeRoleV1::AssertSuccess, 3),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(
                    3,
                    vec![assign(0, WORD, SemanticRvalueKindV1::Use(field(0)))],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        }
    }
    fn set_statements(&mut self, index: usize, statements: Vec<SemanticStatementV1>) {
        self.blocks[index] = block(
            index,
            statements,
            self.blocks[index].terminator().kind().clone(),
        );
    }
    fn set_terminal(&mut self, index: usize, terminal: SemanticTerminatorKindV1) {
        self.blocks[index] = block(index, self.blocks[index].statements().to_vec(), terminal);
    }
    fn function(&self) -> SemanticFunctionDeclV1 {
        let direct = |ty| {
            SemanticAbiValueV1::new(
                ty,
                SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
            )
        };
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([20; 32]),
            SemanticLayoutIdentityV1::from_sha256([20; 32]),
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            2,
            vec![
                SemanticAbiArgumentV1::source(direct(WORD)),
                SemanticAbiArgumentV1::source(direct(BOOL)),
            ],
            direct(WORD),
        )
        .unwrap();
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([21; 32]),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256([22; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([23; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([24; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([25; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
            self.locals
                .iter()
                .enumerate()
                .map(|(i, &ty)| {
                    SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256([i as u8 + 60; 32]),
                        ty,
                        match i {
                            0 => SemanticLocalRoleV1::Return,
                            1 => SemanticLocalRoleV1::Argument(0),
                            8 => SemanticLocalRoleV1::Argument(1),
                            _ => SemanticLocalRoleV1::Temporary,
                        },
                        SemanticSourceProvenanceV1::unavailable(),
                    )
                })
                .collect(),
            SemanticBlockIdV1::from_index(0),
            self.blocks.clone(),
        )
        .unwrap()
    }
    fn proves(&self) -> bool {
        SemanticAssertProofsV1::analyze(&self.types, &self.function()).unwrap()[2]
    }
}

#[test]
fn unsigned_bitwise_consumer_offsets_1_through_63_fit_but_64_does_not() {
    for bits in [8, 16, 32, 64, 128] {
        for offset in 1..=63 {
            assert!(
                Fixture::new(bits, offset, false).proves(),
                "width={bits} offset={offset}"
            );
        }
        assert!(
            !Fixture::new(bits, 64, false).proves(),
            "width={bits} offset=64"
        );
    }
}

#[test]
fn unsigned_bitwise_consumer_retains_checked_body_calls_and_precise_range_without_dense_analysis() {
    let fixture = Fixture::new(64, 2, false);
    let function = fixture.function();
    let original = function.clone();
    let mut proof = SemanticAssertProofsV1::new(&fixture.types, &function).unwrap();
    assert_eq!(
        proof.range_at_operand(&copy(3, WORD), 2, 0).unwrap(),
        Some(UnsignedRangeProofV1 {
            minimum: 0,
            maximum: u128::from(u64::MAX - 63)
        })
    );
    let SemanticTerminatorKindV1::Assert {
        condition,
        expected,
        message,
        ..
    } = function.blocks()[2].terminator().kind()
    else {
        panic!("retained assert");
    };
    assert!(
        proof
            .proves_checked_overflow_assert_v1(condition, *expected, message, 2)
            .unwrap()
    );
    assert!(proof.helper_result_ranges.is_none());
    assert!(proof.work > 0 && proof.work < MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
    assert_eq!(function, original);
    assert!(matches!(
        function.blocks()[1].terminator().kind(),
        SemanticTerminatorKindV1::Call(_)
    ));
}

#[test]
fn unsigned_bitwise_consumer_preserves_copy_and_move_aliases() {
    for move_alias in [false, true] {
        let mut fixture = Fixture::new(64, 2, true);
        if move_alias {
            let mut statements = fixture.blocks[0].statements().to_vec();
            statements[2] = assign(4, WORD, SemanticRvalueKindV1::Use(moved(3, WORD)));
            fixture.set_statements(0, statements);
        }
        assert!(fixture.proves());
    }
}

#[test]
fn unsigned_bitwise_consumer_rejects_mutations_stale_aliases_and_escapes() {
    for mutation in 0..5 {
        let mut fixture = Fixture::new(64, 2, mutation == 2);
        let mut statements = fixture.blocks[1].statements().to_vec();
        match mutation {
            0 | 2 => statements.push(assign(
                3,
                WORD,
                SemanticRvalueKindV1::Use(scalar(WORD, u128::from(u64::MAX), 8)),
            )),
            1 => statements.push(assign(
                2,
                WORD,
                SemanticRvalueKindV1::Use(scalar(WORD, u128::from(u64::MAX), 8)),
            )),
            3 => statements.push(assign(
                7,
                PTR,
                SemanticRvalueKindV1::AddressOf {
                    mutability: SemanticMutabilityV1::Mutable,
                    place: place(3, WORD),
                },
            )),
            4 => {
                let mut definitions = fixture.blocks[0].statements().to_vec();
                definitions[1] = assign(
                    3,
                    WORD,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::BitOr,
                        left: copy(1, WORD),
                        right: moved(2, WORD),
                    },
                );
                fixture.set_statements(0, definitions);
            }
            _ => unreachable!(),
        }
        fixture.set_statements(1, statements);
        assert!(!fixture.proves(), "mutation {mutation}");
    }
}

#[test]
fn unsigned_bitwise_consumer_rejects_bypassed_definitions_joins_and_mutating_backedges() {
    for mutation in 0..3 {
        let mut fixture = Fixture::new(64, 2, false);
        let definitions = fixture.blocks[0].statements().to_vec();
        fixture.set_statements(0, vec![]);
        fixture.blocks.push(block(4, definitions, goto(1)));
        fixture.set_terminal(0, switch(4, 1));
        if mutation > 0 {
            fixture.blocks.push(block(
                5,
                vec![assign(
                    3,
                    WORD,
                    SemanticRvalueKindV1::Use(scalar(WORD, u128::from(u64::MAX), 8)),
                )],
                goto(1),
            ));
            fixture.set_terminal(0, if mutation == 1 { switch(4, 5) } else { goto(4) });
            if mutation == 2 {
                fixture.set_terminal(3, goto(5));
            }
        }
        assert!(!fixture.proves(), "mutation {mutation}");
    }
}

#[test]
fn unsigned_bitwise_consumer_keeps_assertion_operand_message_and_polarity_checks() {
    for mutation in 0..3 {
        let mut fixture = Fixture::new(64, 2, false);
        let mut terminal = fixture.blocks[2].terminator().kind().clone();
        let SemanticTerminatorKindV1::Assert {
            expected, message, ..
        } = &mut terminal
        else {
            unreachable!();
        };
        match mutation {
            0 => *expected = true,
            1 => {
                *message = SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::Subtract,
                    left: copy(3, WORD),
                    right: scalar(WORD, 2, 8),
                }
            }
            2 => {
                *message = SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::Add,
                    left: copy(1, WORD),
                    right: scalar(WORD, 2, 8),
                }
            }
            _ => unreachable!(),
        }
        fixture.set_terminal(2, terminal);
        assert!(!fixture.proves(), "mutation {mutation}");
    }
}

#[test]
fn unsigned_bitwise_rejects_malformed_nominal_signed_bool_and_constant_types() {
    let fixture = Fixture::new(64, 2, false);
    let function = fixture.function();
    let proof = SemanticAssertProofsV1::new(&fixture.types, &function).unwrap();
    for (result, operand) in [
        (SIGNED, scalar(SIGNED, 63, 8)),
        (BOOL, scalar(BOOL, 1, 1)),
        (WORD, scalar(NARROW, 63, 4)),
        (WORD, scalar(OTHER_WORD, 63, 8)),
        (WORD, scalar(WORD, 63, 4)),
        (WORD, copy(8, WORD)),
        (
            WORD,
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                WORD,
                SemanticConstantValueV1::Bytes(SemanticConstantBytesV1::new(vec![1; 8]).unwrap()),
            )),
        ),
        (
            WORD,
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                WORD,
                SemanticConstantValueV1::ZeroSized,
            )),
        ),
    ] {
        let not = SemanticRvalueV1::new(
            result,
            SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::Not,
                operand: operand.clone(),
            },
        );
        assert!(matches!(
            proof.assertion_range_expression_task_v1(&not),
            AssertionRangeExpressionTaskV1::Unsupported
        ));
        for (left, right) in [
            (copy(1, result), operand.clone()),
            (operand, copy(1, result)),
        ] {
            let and = SemanticRvalueV1::new(
                result,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::BitAnd,
                    left,
                    right,
                },
            );
            assert!(matches!(
                proof.assertion_range_expression_task_v1(&and),
                AssertionRangeExpressionTaskV1::Unsupported
            ));
        }
    }
    let mut wrong_layout = fixture.types.clone();
    wrong_layout[0] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([1; 32]),
        SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    );
    let proof = SemanticAssertProofsV1::new(&wrong_layout, &function).unwrap();
    assert!(
        proof
            .unsigned_bitwise_maximum_v1(&SemanticRvalueV1::new(
                WORD,
                SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::Not,
                    operand: scalar(WORD, 63, 8)
                }
            ))
            .is_none()
    );
}

#[test]
fn unsigned_bitwise_interval_transfers_are_bounded_and_sound() {
    for value in 0..=u8::MAX {
        assert_eq!(
            unsigned_not_range_v1(UnsignedRangeProofV1::exact(u128::from(value)), 255),
            Some(UnsignedRangeProofV1::exact(u128::from(!value)))
        );
        for mask in 0..=u8::MAX {
            let range = unsigned_and_range_v1(
                UnsignedRangeProofV1 {
                    minimum: 0,
                    maximum: u128::from(value),
                },
                UnsignedRangeProofV1::exact(u128::from(mask)),
                255,
            )
            .unwrap();
            assert!(
                u128::from(value & mask) >= range.minimum
                    && u128::from(value & mask) <= range.maximum
            );
        }
    }
    for input in [
        UnsignedRangeProofV1 {
            minimum: 2,
            maximum: 1,
        },
        UnsignedRangeProofV1 {
            minimum: 0,
            maximum: 256,
        },
    ] {
        assert!(unsigned_not_range_v1(input, 255).is_none());
        assert!(unsigned_and_range_v1(input, UnsignedRangeProofV1::exact(63), 255).is_none());
        assert!(unsigned_and_range_v1(UnsignedRangeProofV1::exact(63), input, 255).is_none());
    }
    assert_eq!(
        unsigned_not_range_v1(
            UnsignedRangeProofV1 {
                minimum: 3,
                maximum: 7
            },
            255
        ),
        Some(UnsignedRangeProofV1 {
            minimum: 248,
            maximum: 252
        })
    );
}

#[test]
fn unsigned_bitwise_consumer_zero_and_all_ones_masks_and_shared_work_exhaustion() {
    for (not_input, expected) in [(u128::from(u64::MAX), true), (0, false)] {
        let mut fixture = Fixture::new(64, 2, false);
        let mut statements = fixture.blocks[0].statements().to_vec();
        statements[0] = assign(
            2,
            WORD,
            SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::Not,
                operand: scalar(WORD, not_input, 8),
            },
        );
        fixture.set_statements(0, statements);
        assert_eq!(fixture.proves(), expected);
    }
    let fixture = Fixture::new(64, 2, false);
    let function = fixture.function();
    let mut proof = SemanticAssertProofsV1::new(&fixture.types, &function).unwrap();
    proof.work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - 1;
    assert!(matches!(
        proof.range_at_operand(&copy(3, WORD), 2, 0),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis exceeds its work limit"
        ))
    ));
    assert!(proof.work > MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
}
