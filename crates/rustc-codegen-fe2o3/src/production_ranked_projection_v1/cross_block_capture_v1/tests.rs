use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

const WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const PTR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const NARROW: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const SIGNED: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);

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

fn field(local: u32, index: u32) -> SemanticOperandV1 {
    let ty = if index == 0 { WORD } else { BOOL };
    SemanticOperandV1::Move(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty).unwrap()],
            ty,
        )
        .unwrap(),
    )
}

fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
}

fn assign(local: u32, ty: SemanticTypeIdV1, kind: SemanticRvalueKindV1) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(local, ty),
        SemanticRvalueV1::new(ty, kind),
    )))
}

fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn goto(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target))
}

fn switch(discriminant: u32, zero: u32, nonzero: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: moved(discriminant, BOOL),
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

fn checked(
    local: u32,
    op: SemanticBinaryOpV1,
    left: SemanticOperandV1,
    right: SemanticOperandV1,
) -> SemanticStatementV1 {
    let operation = match op {
        SemanticBinaryOpV1::Add => SemanticCheckedBinaryOpV1::Add,
        SemanticBinaryOpV1::Multiply => SemanticCheckedBinaryOpV1::Multiply,
        _ => unreachable!(),
    };
    assign(
        local,
        PAIR,
        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
            operation, left, right,
        )),
    )
}

fn overflow(
    local: u32,
    operation: SemanticBinaryOpV1,
    left: SemanticOperandV1,
    right: SemanticOperandV1,
    target: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Assert {
        condition: field(local, 1),
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

fn block(
    tag: usize,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag as u8 + 30; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}

fn types() -> Vec<SemanticTypeDeclV1> {
    let scalar_type = |tag, bytes, scalar| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            SemanticTypeLayoutV1::new(Some(bytes), bytes).unwrap(),
            SemanticTypeShapeV1::Scalar(scalar),
        )
    };
    vec![
        scalar_type(
            1,
            8,
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            },
        ),
        scalar_type(2, 1, SemanticScalarTypeV1::Bool),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([3; 32]),
            SemanticLayoutIdentityV1::from_sha256([3; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(16),
                8,
                SemanticAggregateLayoutV1::new(
                    vec![0, 8],
                    vec![SemanticPaddingV1::new(9, 7).unwrap()],
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
        scalar_type(
            5,
            4,
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
        ),
        scalar_type(
            6,
            8,
            SemanticScalarTypeV1::Integer {
                signed: true,
                bits: 64,
            },
        ),
    ]
}

struct Fixture {
    blocks: Vec<SemanticBasicBlockV1>,
    local_types: Vec<SemanticTypeIdV1>,
    comparison_statement: usize,
}

impl Fixture {
    fn split_product_guard() -> Self {
        Self {
            blocks: vec![
                block(
                    0,
                    vec![assign(
                        1,
                        WORD,
                        SemanticRvalueKindV1::Use(scalar(WORD, 0, 8)),
                    )],
                    goto(1),
                ),
                block(
                    1,
                    vec![
                        assign(2, WORD, SemanticRvalueKindV1::Use(copy(1, WORD))),
                        checked(
                            3,
                            SemanticBinaryOpV1::Multiply,
                            scalar(WORD, 16, 8),
                            scalar(WORD, 2, 8),
                        ),
                    ],
                    overflow(
                        3,
                        SemanticBinaryOpV1::Multiply,
                        scalar(WORD, 16, 8),
                        scalar(WORD, 2, 8),
                        2,
                    ),
                ),
                block(
                    2,
                    vec![
                        assign(4, WORD, SemanticRvalueKindV1::Use(field(3, 0))),
                        assign(
                            5,
                            BOOL,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::LessThan,
                                left: moved(2, WORD),
                                right: moved(4, WORD),
                            },
                        ),
                    ],
                    switch(5, 6, 3),
                ),
                block(3, vec![], goto(4)),
                block(
                    4,
                    vec![checked(
                        6,
                        SemanticBinaryOpV1::Add,
                        copy(1, WORD),
                        scalar(WORD, 1, 8),
                    )],
                    overflow(
                        6,
                        SemanticBinaryOpV1::Add,
                        copy(1, WORD),
                        scalar(WORD, 1, 8),
                        5,
                    ),
                ),
                block(
                    5,
                    vec![assign(1, WORD, SemanticRvalueKindV1::Use(field(6, 0)))],
                    goto(1),
                ),
                block(
                    6,
                    vec![assign(0, WORD, SemanticRvalueKindV1::Use(copy(1, WORD)))],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
            local_types: vec![WORD, WORD, WORD, PAIR, WORD, BOOL, PAIR, WORD, PTR, BOOL],
            comparison_statement: 1,
        }
    }

    fn site(&self) -> ScalarAssignmentSiteV1 {
        ScalarAssignmentSiteV1 {
            block: 2,
            statement: self.comparison_statement,
        }
    }

    fn statements(&mut self, index: usize, statements: Vec<SemanticStatementV1>) {
        self.blocks[index] = block(
            index,
            statements,
            self.blocks[index].terminator().kind().clone(),
        );
    }

    fn terminator(&mut self, index: usize, terminator: SemanticTerminatorKindV1) {
        self.blocks[index] = block(index, self.blocks[index].statements().to_vec(), terminator);
    }

    fn insert_before_comparison(&mut self, statement: SemanticStatementV1) {
        let mut statements = self.blocks[2].statements().to_vec();
        statements.insert(self.comparison_statement, statement);
        self.comparison_statement += 1;
        self.statements(2, statements);
    }

    fn function(&self) -> SemanticFunctionDeclV1 {
        let locals = self
            .local_types
            .iter()
            .enumerate()
            .map(|(i, &ty)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([i as u8 + 60; 32]),
                    ty,
                    if i == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    SemanticSourceProvenanceV1::unavailable(),
                )
            })
            .collect();
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([10; 32]),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256([11; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([12; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([13; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([14; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            SemanticFunctionAbiV1::from_rustc(
                SemanticAbiIdentityV1::from_sha256([15; 32]),
                SemanticLayoutIdentityV1::from_sha256([15; 32]),
                SemanticCanonAbiV1::GpuKernel,
                SemanticExternAbiV1::GpuKernel,
                false,
                false,
                0,
                vec![],
                SemanticAbiValueV1::new(WORD, SemanticAbiPassModeV1::Ignore),
            )
            .unwrap(),
            locals,
            SemanticBlockIdV1::from_index(0),
            self.blocks.clone(),
        )
        .unwrap()
    }

    fn accepted(&self) -> bool {
        let types = types();
        let function = self.function();
        SemanticAssertProofsV1::new(&types, &function)
            .unwrap()
            .exact_cross_block_guard_capture_v1(2, 1, self.site())
            .unwrap()
    }
}

#[test]
fn split_guard_capture_preserves_exact_copy_through_checked_product() {
    let fixture = Fixture::split_product_guard();
    let types = types();
    let function = fixture.function();
    let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
    assert!(
        !proof
            .local_is_value_preserving_alias_of(2, 1, 2, 1)
            .unwrap()
    );
    assert!(
        proof
            .exact_cross_block_guard_capture_v1(2, 1, fixture.site())
            .unwrap()
    );
    assert!(proof.work > 0);
    assert_eq!(
        function,
        fixture.function(),
        "proof must not rewrite original MIR"
    );
}

#[test]
fn split_guard_capture_rejects_writes_moves_lifetime_and_alias_uncertainty() {
    let mut mutations = vec![
        assign(1, WORD, SemanticRvalueKindV1::Use(scalar(WORD, 99, 8))),
        assign(2, WORD, SemanticRvalueKindV1::Use(scalar(WORD, 99, 8))),
        assign(7, WORD, SemanticRvalueKindV1::Use(moved(1, WORD))),
        assign(7, WORD, SemanticRvalueKindV1::Use(moved(2, WORD))),
        assign(
            8,
            PTR,
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: place(1, WORD),
            },
        ),
        assign(
            8,
            PTR,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(1, WORD),
            },
        ),
        statement(SemanticStatementKindV1::Deinitialize(place(1, WORD))),
        statement(SemanticStatementKindV1::Assume(copy(9, BOOL))),
    ];
    for local in [1, 2] {
        mutations.push(statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(local),
        )));
        mutations.push(statement(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(local),
        )));
    }
    for (index, mutation) in mutations.into_iter().enumerate() {
        let mut fixture = Fixture::split_product_guard();
        fixture.insert_before_comparison(mutation);
        assert!(!fixture.accepted(), "mutation {index}");
    }
}

#[test]
fn split_guard_capture_rejects_nonexact_operand_types_and_capture_operations() {
    for mutation in 0..7 {
        let mut fixture = Fixture::split_product_guard();
        let mut capture = fixture.blocks[1].statements().to_vec();
        let mut comparison = fixture.blocks[2].statements().to_vec();
        match mutation {
            0 => capture[0] = assign(2, WORD, SemanticRvalueKindV1::Use(moved(1, WORD))),
            1 => {
                fixture.local_types[2] = NARROW;
                capture[0] = assign(
                    2,
                    NARROW,
                    SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::Integer,
                        operand: copy(1, WORD),
                    },
                );
            }
            2 => fixture.local_types[1] = SIGNED,
            3 => {
                comparison[1] = assign(
                    5,
                    BOOL,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: moved(4, WORD),
                        right: moved(2, WORD),
                    },
                )
            }
            4 => {
                comparison[1] = assign(
                    5,
                    BOOL,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: moved(2, WORD),
                        right: moved(1, WORD),
                    },
                )
            }
            5 => {
                comparison[1] = assign(
                    5,
                    BOOL,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: moved(2, WORD),
                        right: scalar(NARROW, 32, 4),
                    },
                )
            }
            6 => {
                comparison[1] = assign(
                    5,
                    BOOL,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Equal,
                        left: moved(2, WORD),
                        right: moved(4, WORD),
                    },
                )
            }
            _ => unreachable!(),
        }
        fixture.statements(1, capture);
        fixture.statements(2, comparison);
        assert!(!fixture.accepted(), "mutation {mutation}");
    }
}

#[test]
fn split_guard_capture_rejects_signed_type_even_with_matching_operand_ids() {
    let fixture = Fixture::split_product_guard();
    let function = fixture.function();
    let mut types = types();
    types[0] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([1; 32]),
        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: true,
            bits: 64,
        }),
    );
    assert!(
        !SemanticAssertProofsV1::new(&types, &function)
            .unwrap()
            .exact_cross_block_guard_capture_v1(2, 1, fixture.site())
            .unwrap()
    );
}

#[test]
fn split_guard_capture_rejects_unknown_calls_writes_and_unwind() {
    let mut fixture = Fixture::split_product_guard();
    fixture.terminator(
        1,
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    place(7, WORD),
                    edge(SemanticEdgeRoleV1::CallReturn, 2),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    );
    assert!(!fixture.accepted());

    let mut fixture = Fixture::split_product_guard();
    fixture.insert_before_comparison(statement(SemanticStatementKindV1::Store(
        SemanticMemoryStoreV1::new(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(8),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, WORD).unwrap(),
                ],
                WORD,
            )
            .unwrap(),
            scalar(WORD, 99, 8),
            SemanticVolatilityV1::NonVolatile,
            None,
        ),
    )));
    assert!(!fixture.accepted());

    for unwind in [
        SemanticUnwindActionV1::Continue,
        SemanticUnwindActionV1::Terminate,
    ] {
        let mut fixture = Fixture::split_product_guard();
        let mut terminal = fixture.blocks[1].terminator().kind().clone();
        if let SemanticTerminatorKindV1::Assert { unwind: slot, .. } = &mut terminal {
            *slot = unwind;
        } else {
            unreachable!();
        }
        fixture.terminator(1, terminal);
        assert!(!fixture.accepted());
    }
}

#[test]
fn split_guard_capture_rejects_source_escape_before_capture_and_mutation_after_it() {
    let mut fixture = Fixture::split_product_guard();
    let mut statements = fixture.blocks[0].statements().to_vec();
    statements.push(assign(
        8,
        PTR,
        SemanticRvalueKindV1::AddressOf {
            mutability: SemanticMutabilityV1::Mutable,
            place: place(1, WORD),
        },
    ));
    fixture.statements(0, statements);
    assert!(!fixture.accepted());

    let mut fixture = Fixture::split_product_guard();
    let mut statements = fixture.blocks[1].statements().to_vec();
    statements.push(assign(
        1,
        WORD,
        SemanticRvalueKindV1::Use(scalar(WORD, 99, 8)),
    ));
    fixture.statements(1, statements);
    assert!(!fixture.accepted());
}

#[test]
fn split_guard_capture_rejects_join_and_reentry_that_skip_recapture() {
    for mutation in 0..4 {
        let mut fixture = Fixture::split_product_guard();
        match mutation {
            0 => fixture.terminator(0, switch(9, 1, 2)),
            1 => fixture.terminator(5, goto(2)),
            2 => {
                fixture.terminator(1, switch(9, 2, 7));
                fixture.blocks.push(block(7, vec![], goto(2)));
            }
            3 => {
                fixture.terminator(1, switch(9, 2, 7));
                fixture.blocks.push(block(
                    7,
                    vec![assign(
                        1,
                        WORD,
                        SemanticRvalueKindV1::Use(scalar(WORD, 99, 8)),
                    )],
                    goto(2),
                ));
            }
            _ => unreachable!(),
        }
        assert!(!fixture.accepted(), "mutation {mutation}");
    }
}

#[test]
fn split_guard_capture_rejects_omitted_cleanup_and_duplicate_source_entries() {
    let mut fixture = Fixture::split_product_guard();
    let mut terminal = fixture.blocks[4].terminator().kind().clone();
    if let SemanticTerminatorKindV1::Assert { unwind, .. } = &mut terminal {
        *unwind = SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::AssertUnwind, 2));
    } else {
        unreachable!();
    }
    fixture.terminator(4, terminal);
    // Normal predecessor inventory is unchanged; only complete source edges reveal this entry.
    assert_eq!(
        projected_loop_cfg_graph_v1(&fixture.function())
            .unwrap()
            .predecessors[2],
        vec![1]
    );
    assert!(!fixture.accepted());

    let mut fixture = Fixture::split_product_guard();
    fixture.terminator(1, switch(9, 2, 2));
    assert_eq!(
        projected_loop_cfg_graph_v1(&fixture.function())
            .unwrap()
            .predecessors[2],
        vec![1]
    );
    assert!(!fixture.accepted());
}

#[test]
fn split_guard_capture_obeys_segment_and_shared_work_limits() {
    for additional in [
        MAX_CAPTURE_SEGMENT_BLOCKS - 2,
        MAX_CAPTURE_SEGMENT_BLOCKS - 1,
    ] {
        let mut fixture = Fixture::split_product_guard();
        fixture.terminator(
            1,
            overflow(
                3,
                SemanticBinaryOpV1::Multiply,
                scalar(WORD, 16, 8),
                scalar(WORD, 2, 8),
                7,
            ),
        );
        for index in 0..additional {
            fixture.blocks.push(block(
                7 + index,
                vec![],
                goto(if index + 1 == additional {
                    2
                } else {
                    8 + index as u32
                }),
            ));
        }
        assert_eq!(
            fixture.accepted(),
            additional + 2 == MAX_CAPTURE_SEGMENT_BLOCKS
        );
    }
    let fixture = Fixture::split_product_guard();
    let types = types();
    let function = fixture.function();
    let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
    proof.work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
    assert!(matches!(
        proof.exact_cross_block_guard_capture_v1(2, 1, fixture.site()),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis exceeds its work limit"
        ))
    ));
    assert_eq!(proof.work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1 + 1);
}

#[path = "tests/consumer_tests.rs"]
mod consumer_tests;
