use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

#[path = "tests/frame_entry.rs"]
mod frame_entry;

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

const REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
const CAPTURE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);

fn projection(
    local: u32,
    kind: SemanticProjectionKindV1,
    ty: SemanticTypeIdV1,
) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![SemanticProjectionV1::new(kind, ty).unwrap()],
            ty,
        )
        .unwrap(),
    )
}
fn checked(
    local: u32,
    operation: SemanticCheckedBinaryOpV1,
    left: SemanticOperandV1,
    right: SemanticOperandV1,
) -> SemanticStatementV1 {
    assign(
        local,
        PAIR,
        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
            operation, left, right,
        )),
    )
}
fn checked_field(local: u32, index: u32) -> SemanticOperandV1 {
    let SemanticOperandV1::Copy(place) = projection(
        local,
        SemanticProjectionKindV1::Field(index),
        if index == 0 { WORD } else { BOOL },
    ) else {
        unreachable!()
    };
    SemanticOperandV1::Move(place)
}
fn overflow(
    local: u32,
    operation: SemanticBinaryOpV1,
    left: SemanticOperandV1,
    right: SemanticOperandV1,
    target: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Assert {
        condition: checked_field(local, 1),
        expected: false,
        message: SemanticAssertMessageV1::Overflow {
            operation,
            left,
            right,
        },
        target: edge(SemanticEdgeRoleV1::AssertSuccess, target),
        unwind: SemanticUnwindActionV1::Unreachable,
    }
}

struct Fixture {
    types: Vec<SemanticTypeDeclV1>,
    locals: Vec<SemanticTypeIdV1>,
    blocks: Vec<SemanticBasicBlockV1>,
}
impl Fixture {
    fn new(mask: u128) -> Self {
        let mut types = types(64);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([8; 32]),
            SemanticLayoutIdentityV1::from_sha256([8; 32]),
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    WORD,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ));
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([9; 32]),
            SemanticLayoutIdentityV1::from_sha256([9; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(16),
                8,
                SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![REF, WORD]).unwrap()),
        ));
        let c = |n| scalar(WORD, n, 8);
        Self {
            types,
            locals: vec![
                WORD, WORD, WORD, WORD, WORD, REF, CAPTURE, CAPTURE, CAPTURE, REF, WORD, WORD,
                WORD, WORD, BOOL, WORD, PAIR, WORD, PAIR, WORD, PAIR, WORD, PAIR, WORD, PTR, REF,
            ],
            blocks: vec![
                block(
                    0,
                    vec![
                        assign(
                            2,
                            WORD,
                            SemanticRvalueKindV1::Unary {
                                operation: SemanticUnaryOpV1::Not,
                                operand: c(63),
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
                        assign(
                            4,
                            WORD,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::BitAnd,
                                left: copy(1, WORD),
                                right: c(mask),
                            },
                        ),
                        assign(
                            5,
                            REF,
                            SemanticRvalueKindV1::Borrow {
                                kind: SemanticBorrowKindV1::Shared,
                                place: place(4, WORD),
                            },
                        ),
                        assign(
                            6,
                            CAPTURE,
                            SemanticRvalueKindV1::Aggregate(
                                SemanticAggregateRvalueV1::new(
                                    SemanticAggregateKindV1::Aggregate,
                                    vec![moved(5, REF), c(0)],
                                )
                                .unwrap(),
                            ),
                        ),
                    ],
                    goto(1),
                ),
                block(
                    1,
                    vec![assign(
                        7,
                        CAPTURE,
                        SemanticRvalueKindV1::Use(moved(6, CAPTURE)),
                    )],
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable(
                            SemanticCallableIdV1::from_index(0),
                            vec![],
                            Some(SemanticCallDestinationV1::new(
                                place(23, WORD),
                                edge(SemanticEdgeRoleV1::CallReturn, 2),
                            )),
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    ),
                ),
                block(
                    2,
                    vec![
                        assign(8, CAPTURE, SemanticRvalueKindV1::Use(moved(7, CAPTURE))),
                        assign(
                            9,
                            REF,
                            SemanticRvalueKindV1::Use(projection(
                                8,
                                SemanticProjectionKindV1::Field(0),
                                REF,
                            )),
                        ),
                        assign(
                            10,
                            WORD,
                            SemanticRvalueKindV1::Use(projection(
                                9,
                                SemanticProjectionKindV1::Dereference,
                                WORD,
                            )),
                        ),
                        assign(11, WORD, SemanticRvalueKindV1::Use(moved(10, WORD))),
                        assign(12, WORD, SemanticRvalueKindV1::Use(c(0))),
                    ],
                    goto(3),
                ),
                block(
                    3,
                    vec![
                        assign(15, WORD, SemanticRvalueKindV1::Use(copy(12, WORD))),
                        assign(
                            14,
                            BOOL,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::LessThan,
                                left: moved(15, WORD),
                                right: c(4),
                            },
                        ),
                    ],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: moved(14, BOOL),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                edge(SemanticEdgeRoleV1::SwitchValue, 9),
                            )],
                            edge(SemanticEdgeRoleV1::SwitchOtherwise, 4),
                        )
                        .unwrap(),
                    },
                ),
                block(
                    4,
                    vec![checked(
                        16,
                        SemanticCheckedBinaryOpV1::Add,
                        copy(3, WORD),
                        copy(11, WORD),
                    )],
                    overflow(
                        16,
                        SemanticBinaryOpV1::Add,
                        copy(3, WORD),
                        copy(11, WORD),
                        5,
                    ),
                ),
                block(
                    5,
                    vec![
                        assign(17, WORD, SemanticRvalueKindV1::Use(checked_field(16, 0))),
                        checked(
                            18,
                            SemanticCheckedBinaryOpV1::Multiply,
                            copy(12, WORD),
                            c(16),
                        ),
                    ],
                    overflow(18, SemanticBinaryOpV1::Multiply, copy(12, WORD), c(16), 6),
                ),
                block(
                    6,
                    vec![
                        assign(19, WORD, SemanticRvalueKindV1::Use(checked_field(18, 0))),
                        checked(
                            20,
                            SemanticCheckedBinaryOpV1::Add,
                            copy(17, WORD),
                            copy(19, WORD),
                        ),
                    ],
                    overflow(
                        20,
                        SemanticBinaryOpV1::Add,
                        copy(17, WORD),
                        copy(19, WORD),
                        7,
                    ),
                ),
                block(
                    7,
                    vec![
                        assign(21, WORD, SemanticRvalueKindV1::Use(checked_field(20, 0))),
                        checked(22, SemanticCheckedBinaryOpV1::Add, copy(12, WORD), c(1)),
                    ],
                    overflow(22, SemanticBinaryOpV1::Add, copy(12, WORD), c(1), 8),
                ),
                block(
                    8,
                    vec![assign(
                        12,
                        WORD,
                        SemanticRvalueKindV1::Use(checked_field(22, 0)),
                    )],
                    goto(3),
                ),
                block(
                    9,
                    vec![assign(0, WORD, SemanticRvalueKindV1::Use(c(0)))],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        }
    }
    fn edit(
        &mut self,
        index: usize,
        edit: impl FnOnce(&mut Vec<SemanticStatementV1>, &mut SemanticTerminatorKindV1),
    ) {
        let mut statements = self.blocks[index].statements().to_vec();
        let mut terminal = self.blocks[index].terminator().kind().clone();
        edit(&mut statements, &mut terminal);
        self.blocks[index] = block(index, statements, terminal);
    }
    fn function(&self) -> SemanticFunctionDeclV1 {
        let direct = |ty| {
            SemanticAbiValueV1::new(
                ty,
                SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
            )
        };
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([40; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([41; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([42; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([43; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([44; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            SemanticFunctionAbiV1::new(
                SemanticAbiIdentityV1::from_sha256([45; 32]),
                SemanticLayoutIdentityV1::from_sha256([46; 32]),
                SemanticCanonAbiV1::Rust,
                false,
                false,
                vec![direct(WORD)],
                direct(WORD),
            )
            .unwrap(),
            self.locals
                .iter()
                .enumerate()
                .map(|(i, &ty)| {
                    SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256([i as u8 + 50; 32]),
                        ty,
                        match i {
                            0 => SemanticLocalRoleV1::Return,
                            1 => SemanticLocalRoleV1::Argument(0),
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
    fn first_rejects(&self) {
        assert!(!SemanticAssertProofsV1::analyze(&self.types, &self.function()).unwrap()[4]);
    }
}

#[test]
fn shared_capture_range_real_striped_consumer_preserves_all_checked_operations() {
    let fixture = Fixture::new(15);
    let function = fixture.function();
    let retained = function.clone();
    let mut proof = SemanticAssertProofsV1::new(&fixture.types, &function).unwrap();
    assert_eq!(
        proof
            .shared_capture_range_at_operand_v1(
                &copy(11, WORD),
                ScalarAssignmentSiteV1 {
                    block: 4,
                    statement: 0
                }
            )
            .unwrap(),
        Some(UnsignedRangeProofV1 {
            minimum: 0,
            maximum: 15
        })
    );
    let results = SemanticAssertProofsV1::analyze(&fixture.types, &function).unwrap();
    for bb in [4, 5, 6, 7] {
        assert!(results[bb], "original checked operation bb{bb}");
    }
    assert!(proof.helper_result_ranges.is_none());
    assert_eq!(function, retained);
    assert!(matches!(
        function.blocks()[1].terminator().kind(),
        SemanticTerminatorKindV1::Call(_)
    ));
}
#[test]
fn shared_capture_range_offset_63_fits_64_fails_without_borrowing_its_own_assertion() {
    for mask in [15, 63] {
        let fixture = Fixture::new(mask);
        let result = SemanticAssertProofsV1::analyze(&fixture.types, &fixture.function()).unwrap();
        assert!(result[4]);
        assert_eq!(result[6], mask == 15);
    }
    Fixture::new(64).first_rejects();
}
#[test]
fn shared_capture_range_reassignments_moves_lifetimes_and_aliases_reject() {
    for mutation in 0..8 {
        let mut fixture = Fixture::new(15);
        match mutation {
            0 => fixture.edit(0, |s, _| {
                s.insert(
                    4,
                    assign(
                        4,
                        WORD,
                        SemanticRvalueKindV1::Use(scalar(WORD, u64::MAX.into(), 8)),
                    ),
                )
            }),
            1 => fixture.edit(1, |s, _| {
                s.push(assign(
                    7,
                    CAPTURE,
                    SemanticRvalueKindV1::Use(copy(6, CAPTURE)),
                ))
            }),
            2 => fixture.edit(0, |s, _| {
                s.insert(
                    3,
                    assign(23, WORD, SemanticRvalueKindV1::Use(moved(4, WORD))),
                )
            }),
            3 => fixture.edit(0, |s, _| {
                s.insert(
                    3,
                    SemanticStatementV1::new(
                        SemanticSourceProvenanceV1::unavailable(),
                        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(4)),
                    ),
                )
            }),
            4 => fixture.edit(1, |s, _| {
                s.push(SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(7)),
                ))
            }),
            5 => fixture.edit(0, |s, _| {
                s.insert(4, assign(25, REF, SemanticRvalueKindV1::Use(copy(5, REF))))
            }),
            6 => fixture.edit(0, |s, _| {
                s.insert(
                    3,
                    assign(
                        25,
                        REF,
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place: place(4, WORD),
                        },
                    ),
                )
            }),
            7 => fixture.edit(0, |s, _| {
                s.insert(
                    3,
                    assign(
                        24,
                        PTR,
                        SemanticRvalueKindV1::AddressOf {
                            mutability: SemanticMutabilityV1::Mutable,
                            place: place(4, WORD),
                        },
                    ),
                )
            }),
            _ => unreachable!(),
        }
        fixture.first_rejects();
    }
}
#[test]
fn shared_capture_range_type_kind_field_and_site_mismatches_reject() {
    for mutation in 0..5 {
        let mut fixture = Fixture::new(15);
        match mutation {
            0 => {
                let old = &fixture.types[7];
                fixture.types[7] = SemanticTypeDeclV1::new(
                    old.identity(),
                    old.layout_identity(),
                    old.layout().clone(),
                    SemanticTypeShapeV1::Pointer(
                        SemanticPointerTypeV1::new(
                            WORD,
                            SemanticMutabilityV1::Immutable,
                            0,
                            64,
                            SemanticPointerMetadataV1::None,
                        )
                        .unwrap(),
                    ),
                );
            }
            1 => fixture.edit(0, |s, _| {
                s[3] = assign(
                    5,
                    REF,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Fake,
                        place: place(4, WORD),
                    },
                )
            }),
            2 => fixture.edit(2, |s, _| {
                s[1] = assign(
                    9,
                    REF,
                    SemanticRvalueKindV1::Use(projection(
                        8,
                        SemanticProjectionKindV1::Field(1),
                        REF,
                    )),
                )
            }),
            3 => fixture.edit(2, |s, _| {
                s[2] = assign(
                    10,
                    WORD,
                    SemanticRvalueKindV1::Use(projection(
                        9,
                        SemanticProjectionKindV1::Dereference,
                        OTHER_WORD,
                    )),
                )
            }),
            4 => {
                let old = &fixture.types[0];
                fixture.types[0] = SemanticTypeDeclV1::new(
                    old.identity(),
                    old.layout_identity(),
                    old.layout().clone(),
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed: true,
                        bits: 64,
                    }),
                );
            }
            _ => unreachable!(),
        }
        fixture.first_rejects();
    }
    let fixture = Fixture::new(15);
    let function = fixture.function();
    let mut proof = SemanticAssertProofsV1::new(&fixture.types, &function).unwrap();
    let SemanticOperandV1::Copy(read) = projection(9, SemanticProjectionKindV1::Dereference, WORD)
    else {
        unreachable!()
    };
    for site in [
        ScalarAssignmentSiteV1 {
            block: 2,
            statement: 1,
        },
        ScalarAssignmentSiteV1 {
            block: 4,
            statement: 0,
        },
    ] {
        assert!(
            proof
                .shared_scalar_read_source_v1(&read, site)
                .unwrap()
                .is_none()
        );
    }
}
#[test]
fn shared_capture_range_unknown_calls_and_unwind_cannot_take_the_capture() {
    for mutation in 0..3 {
        let mut fixture = Fixture::new(15);
        fixture.edit(1, |_, terminal| {
            let SemanticTerminatorKindV1::Call(call) = terminal else {
                unreachable!()
            };
            *terminal = SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    call.callee(),
                    if mutation == 0 {
                        vec![copy(7, CAPTURE)]
                    } else {
                        vec![]
                    },
                    call.destination().cloned(),
                    match mutation {
                        0 => SemanticUnwindActionV1::Unreachable,
                        1 => SemanticUnwindActionV1::Continue,
                        _ => {
                            SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::CallUnwind, 2))
                        }
                    },
                )
                .unwrap(),
            );
        });
        fixture.first_rejects();
    }
}
#[test]
fn shared_capture_range_joins_bypasses_and_backedges_reject() {
    for mutation in 0..3 {
        let mut fixture = Fixture::new(15);
        fixture.blocks.push(block(10, vec![], goto(2)));
        if mutation < 2 {
            fixture.edit(1, |_, t| {
                *t = SemanticTerminatorKindV1::SwitchInt {
                    discriminant: scalar(BOOL, 1, 1),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 2),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 10),
                    )
                    .unwrap(),
                }
            });
            if mutation == 1 {
                fixture.edit(10, |s, _| {
                    s.push(assign(
                        4,
                        WORD,
                        SemanticRvalueKindV1::Use(scalar(WORD, u64::MAX.into(), 8)),
                    ))
                });
            }
        } else {
            fixture.edit(8, |_, t| *t = goto(2));
        }
        fixture.first_rejects();
    }
}
#[test]
fn shared_capture_range_shared_work_exhaustion_remains_fatal() {
    let fixture = Fixture::new(15);
    let function = fixture.function();
    let mut proof = SemanticAssertProofsV1::new(&fixture.types, &function).unwrap();
    proof.work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
    assert!(matches!(
        proof.range_at_operand(&copy(11, WORD), 4, 0),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis exceeds its work limit"
        ))
    ));
}

#[test]
fn shared_capture_range_existing_total_proof_stays_lazy_at_exact_work_boundary() {
    let mut fixture = Fixture::new(15);
    fixture.edit(4, |s, t| {
        s[0] = checked(
            16,
            SemanticCheckedBinaryOpV1::Add,
            copy(11, WORD),
            scalar(WORD, 0, 8),
        );
        *t = overflow(
            16,
            SemanticBinaryOpV1::Add,
            copy(11, WORD),
            scalar(WORD, 0, 8),
            5,
        );
    });
    for bb in 10..74 {
        fixture.blocks.push(block(
            bb,
            vec![
                SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Nop
                );
                128
            ],
            SemanticTerminatorKindV1::Return,
        ));
    }
    let function = fixture.function();
    let SemanticTerminatorKindV1::Assert {
        condition,
        expected,
        message,
        ..
    } = function.blocks()[4].terminator().kind()
    else {
        unreachable!()
    };
    let mut baseline = SemanticAssertProofsV1::new(&fixture.types, &function).unwrap();
    let construction_work = baseline.work;
    assert!(
        baseline
            .proves_checked_overflow_assert_v1(condition, *expected, message, 4)
            .unwrap()
    );
    assert!(baseline.shared_capture_seen);
    assert!(!baseline.shared_capture_precision);
    assert!(baseline.helper_result_ranges.is_none());
    let query_work = baseline.work - construction_work;
    assert!(query_work > 0);
    let mut exact = SemanticAssertProofsV1::new(&fixture.types, &function).unwrap();
    exact
        .charge(MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - exact.work - query_work)
        .unwrap();
    assert!(
        exact
            .proves_checked_overflow_assert_v1(condition, *expected, message, 4)
            .unwrap()
    );
    assert_eq!(exact.work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
    assert!(exact.helper_result_ranges.is_none());
    let mut short = SemanticAssertProofsV1::new(&fixture.types, &function).unwrap();
    short
        .charge(MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - short.work - query_work + 1)
        .unwrap();
    assert!(matches!(
        short.proves_checked_overflow_assert_v1(condition, *expected, message, 4),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis exceeds its work limit"
        ))
    ));
    assert!(short.helper_result_ranges.is_none());
}

#[test]
fn shared_capture_range_link_and_segment_limits_are_inclusive() {
    for extra in [11, 12] {
        let mut fixture = Fixture::new(15);
        for _ in 0..extra {
            fixture.locals.push(REF);
        }
        fixture.edit(2, |s, _| {
            let mut from = 9;
            for index in 0..extra {
                let to = 26 + index;
                s.insert(
                    2 + index as usize,
                    assign(to, REF, SemanticRvalueKindV1::Use(moved(from, REF))),
                );
                from = to;
            }
            s[2 + extra as usize] = assign(
                10,
                WORD,
                SemanticRvalueKindV1::Use(projection(
                    from,
                    SemanticProjectionKindV1::Dereference,
                    WORD,
                )),
            );
        });
        assert_eq!(
            SemanticAssertProofsV1::analyze(&fixture.types, &fixture.function()).unwrap()[4],
            extra == 11
        );
    }
    for extra in [13, 14] {
        let mut fixture = Fixture::new(15);
        fixture.edit(1, |_, term| {
            let SemanticTerminatorKindV1::Call(call) = term else {
                unreachable!()
            };
            *term = SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    call.callee(),
                    vec![],
                    Some(SemanticCallDestinationV1::new(
                        place(23, WORD),
                        edge(SemanticEdgeRoleV1::CallReturn, 10),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            );
        });
        for offset in 0..extra {
            let bb = 10 + offset;
            fixture.blocks.push(block(
                bb,
                vec![],
                goto(if offset + 1 == extra {
                    2
                } else {
                    (bb + 1) as u32
                }),
            ));
        }
        assert_eq!(
            SemanticAssertProofsV1::analyze(&fixture.types, &fixture.function()).unwrap()[4],
            extra == 13
        );
    }
}

#[test]
fn shared_capture_range_duplicate_field_alias_and_indirect_write_reject() {
    let mut duplicate = Fixture::new(15);
    let old = &duplicate.types[8];
    duplicate.types[8] = SemanticTypeDeclV1::new(
        old.identity(),
        old.layout_identity(),
        old.layout().clone(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![REF, REF]).unwrap()),
    );
    duplicate.edit(0, |s, _| {
        s[4] = assign(
            6,
            CAPTURE,
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Aggregate,
                    vec![copy(5, REF), copy(5, REF)],
                )
                .unwrap(),
            ),
        )
    });
    duplicate.first_rejects();
    let mut indirect = Fixture::new(15);
    indirect.edit(1, |s, _| {
        let SemanticOperandV1::Copy(destination) =
            projection(24, SemanticProjectionKindV1::Dereference, WORD)
        else {
            unreachable!()
        };
        s.push(SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                destination,
                SemanticRvalueV1::new(WORD, SemanticRvalueKindV1::Use(scalar(WORD, 0, 8))),
            )),
        ));
    });
    indirect.first_rejects();
    let mut moved_read = Fixture::new(15);
    moved_read.edit(2, |s, _| {
        let SemanticOperandV1::Copy(p) = projection(9, SemanticProjectionKindV1::Dereference, WORD)
        else {
            unreachable!()
        };
        s[2] = assign(
            10,
            WORD,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(p)),
        );
    });
    moved_read.first_rejects();
}

#[test]
fn shared_capture_range_guard_and_original_assertions_are_still_required() {
    for mutation in 0..4 {
        let mut fixture = Fixture::new(if mutation == 0 { 16 } else { 15 });
        match mutation {
            0 => {}
            1 => fixture.edit(3, |s, _| {
                s[1] = assign(
                    14,
                    BOOL,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: moved(15, WORD),
                        right: scalar(WORD, 5, 8),
                    },
                )
            }),
            2 => fixture.edit(3, |_, t| *t = goto(4)),
            3 => fixture.edit(6, |_, t| {
                let SemanticTerminatorKindV1::Assert { expected, .. } = t else {
                    unreachable!()
                };
                *expected = true;
            }),
            _ => unreachable!(),
        }
        let result = SemanticAssertProofsV1::analyze(&fixture.types, &fixture.function()).unwrap();
        assert!(result[4]);
        assert!(!result[6]);
    }
}

fn source_kill_fixture(predecessor: bool) -> Fixture {
    let mut fixture = Fixture::new(15);
    if predecessor {
        let statements = fixture.blocks[0].statements().to_vec();
        fixture.blocks[0] = block(0, statements[..3].to_vec(), goto(10));
        fixture
            .blocks
            .push(block(10, statements[3..].to_vec(), goto(1)));
    }
    check_source_kill_origin(&fixture, false);
    fixture
}

fn insert_source_kill(fixture: &mut Fixture, kind: SemanticStatementKindV1) {
    fixture.edit(0, |statements, _| {
        statements.insert(
            3,
            SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind),
        );
    });
}

fn check_source_kill_origin(fixture: &Fixture, killed: bool) {
    let function = fixture.function();
    let retained = function.clone();
    let mut proof = SemanticAssertProofsV1::new(&fixture.types, &function).unwrap();
    assert_eq!(proof.definition_counts[4], if killed { 2 } else { 1 });
    assert_eq!(proof.assignments[4].is_none(), killed);
    let SemanticOperandV1::Copy(read) = projection(9, SemanticProjectionKindV1::Dereference, WORD)
    else {
        unreachable!()
    };
    let origin = proof
        .shared_scalar_read_source_v1(
            &read,
            ScalarAssignmentSiteV1 {
                block: 2,
                statement: 2,
            },
        )
        .unwrap();
    if killed {
        assert!(
            origin.is_none(),
            "the origin query must reject before SSA validation"
        );
    } else {
        let (operand, site) = origin.expect("the unmutated capture must retain its origin");
        assert_eq!(operand, copy(4, WORD));
        assert_eq!(
            (site.block, site.statement),
            if fixture.blocks.len() == 11 {
                (10, 0)
            } else {
                (0, 3)
            }
        );
    }
    let proofs = SemanticAssertProofsV1::analyze(&fixture.types, &function).unwrap();
    assert_eq!(proofs[4], !killed, "the retained first checked addition");
    assert_eq!(function, retained);
}

fn projected_source_kill_place() -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(4),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Subtype, WORD).unwrap()],
        WORD,
    )
    .unwrap()
}

#[test]
fn shared_capture_range_source_deinitialize_same_block_rejects_before_origin() {
    let mut fixture = source_kill_fixture(false);
    insert_source_kill(
        &mut fixture,
        SemanticStatementKindV1::Deinitialize(place(4, WORD)),
    );
    check_source_kill_origin(&fixture, true);
}

#[test]
fn shared_capture_range_source_deinitialize_predecessor_rejects_before_origin() {
    let mut fixture = source_kill_fixture(true);
    insert_source_kill(
        &mut fixture,
        SemanticStatementKindV1::Deinitialize(place(4, WORD)),
    );
    check_source_kill_origin(&fixture, true);
}

#[test]
fn shared_capture_range_source_set_discriminant_rejects_before_origin() {
    // Deliberately malformed on a primitive: the origin query must not depend
    // on a subsequent structural validator rejecting this write.
    for predecessor in [false, true] {
        let mut fixture = source_kill_fixture(predecessor);
        insert_source_kill(
            &mut fixture,
            SemanticStatementKindV1::SetDiscriminant {
                place: place(4, WORD),
                variant_index: 0,
            },
        );
        check_source_kill_origin(&fixture, true);
    }
}

#[test]
fn shared_capture_range_source_direct_store_rejects_before_origin() {
    for predecessor in [false, true] {
        let mut fixture = source_kill_fixture(predecessor);
        insert_source_kill(
            &mut fixture,
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                place(4, WORD),
                scalar(WORD, u64::MAX.into(), 8),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        );
        check_source_kill_origin(&fixture, true);
    }
}

#[test]
fn shared_capture_range_source_projected_assignment_rejects_before_origin() {
    for predecessor in [false, true] {
        let mut fixture = source_kill_fixture(predecessor);
        insert_source_kill(
            &mut fixture,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                projected_source_kill_place(),
                SemanticRvalueV1::new(
                    WORD,
                    SemanticRvalueKindV1::Use(scalar(WORD, u64::MAX.into(), 8)),
                ),
            )),
        );
        check_source_kill_origin(&fixture, true);
    }
}

#[test]
fn shared_capture_range_source_projected_store_rejects_before_origin() {
    for predecessor in [false, true] {
        let mut fixture = source_kill_fixture(predecessor);
        insert_source_kill(
            &mut fixture,
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                projected_source_kill_place(),
                scalar(WORD, u64::MAX.into(), 8),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        );
        check_source_kill_origin(&fixture, true);
    }
}
