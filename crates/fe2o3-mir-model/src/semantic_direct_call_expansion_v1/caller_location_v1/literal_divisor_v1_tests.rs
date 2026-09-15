use super::*;

const WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);

fn scalar(bits: u128, width: u16) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        WORD,
        SemanticConstantValueV1::Scalar(
            SemanticScalarValueV1::new(bits, (width / 8) as u8).unwrap(),
        ),
    ))
}
fn copy(index: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(index, ty))
}
fn moved(index: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Move(place(index, ty))
}
fn field(index: u32) -> SemanticOperandV1 {
    let ty = if index == 0 { WORD } else { BOOL };
    SemanticOperandV1::Move(
        SemanticPlaceV1::new(
            local(8),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty).unwrap()],
            ty,
        )
        .unwrap(),
    )
}
fn assignment(
    destination: u32,
    ty: SemanticTypeIdV1,
    kind: SemanticRvalueKindV1,
) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        provenance(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(destination, ty),
            SemanticRvalueV1::new(ty, kind),
        )),
    )
}
fn binary(
    destination: u32,
    ty: SemanticTypeIdV1,
    operation: SemanticBinaryOpV1,
    left: SemanticOperandV1,
    right: SemanticOperandV1,
) -> SemanticStatementV1 {
    assignment(
        destination,
        ty,
        SemanticRvalueKindV1::Binary {
            operation,
            left,
            right,
        },
    )
}
fn assertion(
    condition: SemanticOperandV1,
    message: SemanticAssertMessageV1,
    target: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Assert {
        condition,
        expected: false,
        message,
        target: edge(SemanticEdgeRoleV1::AssertSuccess, target),
        unwind: SemanticUnwindActionV1::Unreachable,
    }
}
fn make_function(
    index: u32,
    abi: SemanticFunctionAbiV1,
    local_types: Vec<(SemanticTypeIdV1, SemanticLocalRoleV1)>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let tag = index + 1;
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(identity(tag)),
        if index == 0 {
            SemanticFunctionRoleV1::KernelRoot
        } else {
            SemanticFunctionRoleV1::InternalHelper
        },
        SemanticItemDefinitionIdentityV1::from_sha256(identity(tag)),
        SemanticMonomorphizationIdentityV1::from_sha256(identity(tag)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(identity(tag)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(identity(tag)),
        provenance(),
        abi,
        local_types
            .into_iter()
            .enumerate()
            .map(|(index, (ty, role))| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256(identity(index as u32 + 1)),
                    ty,
                    role,
                    provenance(),
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}
fn replace_body(
    function: &SemanticFunctionDeclV1,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        function.abi().clone(),
        function.locals().to_vec(),
        function.entry(),
        blocks,
    )
    .unwrap()
}
fn word_abi(index: u32, root: bool) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(identity(index + 1)),
        SemanticLayoutIdentityV1::from_sha256(identity(index + 1)),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![direct(WORD), direct(WORD)],
        if root {
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore)
        } else {
            direct(WORD)
        },
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
    .unwrap()
}

#[derive(Clone)]
struct Fixture {
    types: Vec<SemanticTypeDeclV1>,
    functions: Vec<SemanticFunctionDeclV1>,
}
impl Fixture {
    fn new(bits: u16, frozen: bool, divisors: &[u128]) -> Self {
        let bytes = u64::from(bits / 8);
        let alignment = bytes.min(8);
        let maximum = if bits == 128 {
            u128::MAX
        } else {
            (1u128 << bits) - 1
        };
        let word = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, bits, alignment),
            SemanticScalarValidityRangeV1::new(0, maximum),
        );
        let boolean = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 8, 1),
            SemanticScalarValidityRangeV1::new(0, 1),
        );
        let mut types = location_types(frozen);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(identity(7)),
            SemanticLayoutIdentityV1::from_sha256(identity(7)),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(bytes),
                alignment,
                SemanticBackendReprV1::scalar(word),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits,
            }),
        ));
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(identity(8)),
            SemanticLayoutIdentityV1::from_sha256(identity(8)),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(bytes + alignment),
                alignment,
                SemanticBackendReprV1::scalar_pair(word, boolean),
                false,
                SemanticAggregateLayoutV1::new(vec![0, bytes], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![WORD, BOOL]).unwrap()),
        ));
        let mut root_blocks = Vec::new();
        for (index, divisor) in divisors.iter().enumerate() {
            root_blocks.push(block(
                index as u32,
                vec![],
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new(
                        function_id(1),
                        vec![
                            if index + 1 == divisors.len() {
                                moved(1, WORD)
                            } else {
                                copy(1, WORD)
                            },
                            scalar(*divisor, bits),
                        ],
                        Some(SemanticCallDestinationV1::new(
                            place(3, WORD),
                            edge(SemanticEdgeRoleV1::CallReturn, index as u32 + 1),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ));
        }
        root_blocks.push(block(
            divisors.len() as u32,
            vec![],
            SemanticTerminatorKindV1::Return,
        ));
        let root = make_function(
            0,
            word_abi(0, true),
            vec![
                (UNIT, SemanticLocalRoleV1::Return),
                (WORD, SemanticLocalRoleV1::Argument(0)),
                (WORD, SemanticLocalRoleV1::Argument(1)),
                (WORD, SemanticLocalRoleV1::Temporary),
                (PTR, SemanticLocalRoleV1::Temporary),
                (U32, SemanticLocalRoleV1::Temporary),
            ],
            root_blocks,
        );
        let helper_blocks = vec![
            block(
                0,
                vec![binary(
                    4,
                    BOOL,
                    SemanticBinaryOpV1::Equal,
                    copy(2, WORD),
                    scalar(0, bits),
                )],
                assertion(
                    moved(4, BOOL),
                    SemanticAssertMessageV1::DivisionByZero(copy(1, WORD)),
                    1,
                ),
            ),
            block(
                1,
                vec![
                    binary(
                        3,
                        WORD,
                        SemanticBinaryOpV1::Divide,
                        copy(1, WORD),
                        copy(2, WORD),
                    ),
                    binary(
                        6,
                        BOOL,
                        SemanticBinaryOpV1::Equal,
                        copy(2, WORD),
                        scalar(0, bits),
                    ),
                ],
                assertion(
                    moved(6, BOOL),
                    SemanticAssertMessageV1::RemainderByZero(copy(1, WORD)),
                    2,
                ),
            ),
            block(
                2,
                vec![
                    binary(
                        5,
                        WORD,
                        SemanticBinaryOpV1::Remainder,
                        copy(1, WORD),
                        copy(2, WORD),
                    ),
                    binary(
                        7,
                        BOOL,
                        SemanticBinaryOpV1::GreaterThan,
                        copy(5, WORD),
                        scalar(0, bits),
                    ),
                ],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: moved(7, BOOL),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 5),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
                    )
                    .unwrap(),
                },
            ),
            block(
                3,
                vec![assignment(
                    8,
                    PAIR,
                    SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                        SemanticCheckedBinaryOpV1::Add,
                        copy(3, WORD),
                        scalar(1, bits),
                    )),
                )],
                assertion(
                    field(1),
                    SemanticAssertMessageV1::Overflow {
                        operation: SemanticBinaryOpV1::Add,
                        left: copy(3, WORD),
                        right: scalar(1, bits),
                    },
                    4,
                ),
            ),
            block(
                4,
                vec![assignment(0, WORD, SemanticRvalueKindV1::Use(field(0)))],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 6)),
            ),
            block(
                5,
                vec![assignment(
                    0,
                    WORD,
                    SemanticRvalueKindV1::Use(copy(3, WORD)),
                )],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 6)),
            ),
            block(6, vec![], SemanticTerminatorKindV1::Return),
        ];
        let helper = make_function(
            1,
            word_abi(1, false),
            [WORD, WORD, WORD, WORD, BOOL, WORD, BOOL, BOOL, PAIR]
                .into_iter()
                .enumerate()
                .map(|(i, ty)| {
                    (
                        ty,
                        match i {
                            0 => SemanticLocalRoleV1::Return,
                            1 | 2 => SemanticLocalRoleV1::Argument((i - 1) as u32),
                            _ => SemanticLocalRoleV1::Temporary,
                        },
                    )
                })
                .collect(),
            helper_blocks,
        );
        Self {
            types,
            functions: vec![root, with_hidden(&helper, frozen)],
        }
    }
    fn request(&self) -> InertSemanticMirRequestV1 {
        InertSemanticMirRequestV1::new_with_callables(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(identity(
                250,
            ))),
            self.types.clone(),
            vec![],
            vec![],
            vec![],
            self.functions.clone(),
            (0..self.functions.len())
                .map(|i| SemanticCallableDeclV1::defined(function_id(i as u32)))
                .collect(),
            vec![function_id(0)],
        )
        .unwrap()
    }
    fn admit(&self) -> AdmittedInertSemanticMirV1 {
        self.request()
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap()
    }
    fn edit_block(
        &mut self,
        function: usize,
        index: usize,
        edit: impl FnOnce(&mut Vec<SemanticStatementV1>, &mut SemanticTerminatorKindV1),
    ) {
        let old = &self.functions[function];
        let mut blocks = old.blocks().to_vec();
        let mut statements = blocks[index].statements().to_vec();
        let mut terminator = blocks[index].terminator().kind().clone();
        edit(&mut statements, &mut terminator);
        blocks[index] = block(index as u32, statements, terminator);
        self.functions[function] = replace_body(old, blocks);
    }
    fn rejected(&self) {
        let source = self.admit();
        assert!(
            matches!(expand(&source), Err(SemanticCallExpansionErrorV1::Unsupported { function, block: Some(block), reason: "caller-location frame contains an observing assertion" })
            if function == function_id(1) && block.index() == 0)
        );
    }
}

#[test]
fn caller_location_literal_divisor_exact_consumer_preserves_assertions_and_replay() {
    for frozen in [false, true] {
        let source = Fixture::new(64, frozen, &[16, 2, u64::MAX.into()]).admit();
        let bytes = source.canonical_encoding().to_vec();
        let expansion = expand(&source).unwrap();
        expansion.verify_replay(&source).unwrap();
        let root = expansion.root(function_id(0)).unwrap();
        assert_eq!(root.instances().len(), 4);
        assert_eq!(
            root.body()
                .blocks()
                .iter()
                .filter(|b| matches!(
                    b.terminator().kind(),
                    SemanticTerminatorKindV1::Assert { .. }
                ))
                .count(),
            9
        );
        assert_eq!(source.canonical_encoding(), bytes);
        for (expanded, origin) in root.body().blocks().iter().zip(root.block_origins()) {
            if matches!(
                expanded.terminator().kind(),
                SemanticTerminatorKindV1::Assert { .. }
            ) {
                let instance = &root.instances()[origin.instance().index() as usize];
                let original = &source.functions()[instance.function().index() as usize].blocks()
                    [origin.block().index() as usize];
                let expected = remap::terminator(
                    original.terminator().kind(),
                    instance,
                    origin.block(),
                    &mut Budget::new(SemanticCallExpansionLimitsV1::default()).unwrap(),
                )
                .unwrap();
                assert_eq!(expanded.terminator().kind(), &expected);
                assert_eq!(
                    expanded.terminator().source(),
                    original.terminator().source()
                );
                assert_eq!(
                    origin.terminator(),
                    SemanticExpandedTerminatorOriginV1::Source
                );
            }
        }
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            &bytes,
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(expand(&decoded).unwrap(), expansion);
        expansion.verify_replay(&decoded).unwrap();
    }
}

#[test]
fn caller_location_literal_divisor_no_success_cache_for_same_callee() {
    for divisors in [&[16, 0][..], &[0, 16][..], &[16, 1][..]] {
        Fixture::new(64, false, divisors).rejected();
    }
}

#[test]
fn caller_location_literal_divisor_unsigned_widths_and_boundaries() {
    for bits in [8, 16, 32, 64, 128] {
        let maximum = if bits == 128 {
            u128::MAX
        } else {
            (1u128 << bits) - 1
        };
        for divisor in [2, 16, maximum] {
            let source = Fixture::new(bits, false, &[divisor]).admit();
            expand(&source).unwrap().verify_replay(&source).unwrap();
        }
        for divisor in [0, 1] {
            Fixture::new(bits, false, &[divisor]).rejected();
        }
    }
}

fn change_call(
    call: &SemanticDirectCallV1,
    arguments: Vec<SemanticOperandV1>,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            call.callee(),
            arguments,
            call.destination().cloned(),
            call.unwind(),
        )
        .unwrap(),
    )
}

#[test]
fn caller_location_literal_divisor_unknown_alias_and_swapped_arguments_reject() {
    for mutation in 0..3 {
        let mut fixture = Fixture::new(64, false, &[16]);
        fixture.edit_block(0, 0, |statements, terminator| {
            let SemanticTerminatorKindV1::Call(call) = terminator else {
                panic!()
            };
            let mut arguments = call.arguments().to_vec();
            match mutation {
                0 => arguments[1] = copy(2, WORD),
                1 => {
                    statements.push(assignment(
                        3,
                        WORD,
                        SemanticRvalueKindV1::Use(scalar(16, 64)),
                    ));
                    arguments[1] = moved(3, WORD);
                }
                2 => arguments.swap(0, 1),
                _ => unreachable!(),
            }
            *terminator = change_call(call, arguments);
        });
        fixture.rejected();
    }
}

#[path = "literal_divisor_role_map_v1_tests.rs"]
mod literal_divisor_role_map_v1_tests;

#[test]
fn caller_location_literal_divisor_body_definitions_operations_and_moves_reject() {
    for mutation in 0..12 {
        let mut fixture = Fixture::new(64, false, &[16]);
        let (bb, statement) = match mutation {
            0..=3 => (1, 0),
            4..=6 => (2, 0),
            7..=9 => (3, 0),
            10 => (0, 0),
            _ => (4, 0),
        };
        fixture.edit_block(1, bb, |statements, _| {
            statements[statement] = match mutation {
                0 => binary(
                    3,
                    WORD,
                    SemanticBinaryOpV1::Divide,
                    copy(2, WORD),
                    copy(1, WORD),
                ),
                1 => binary(
                    3,
                    WORD,
                    SemanticBinaryOpV1::Divide,
                    copy(1, WORD),
                    scalar(0, 64),
                ),
                2 => binary(
                    3,
                    WORD,
                    SemanticBinaryOpV1::Remainder,
                    copy(1, WORD),
                    copy(2, WORD),
                ),
                3 => binary(
                    3,
                    WORD,
                    SemanticBinaryOpV1::Divide,
                    moved(1, WORD),
                    copy(2, WORD),
                ),
                4 => binary(
                    5,
                    WORD,
                    SemanticBinaryOpV1::Remainder,
                    copy(3, WORD),
                    copy(2, WORD),
                ),
                5 => binary(
                    5,
                    WORD,
                    SemanticBinaryOpV1::Remainder,
                    copy(1, WORD),
                    copy(1, WORD),
                ),
                6 => binary(
                    5,
                    WORD,
                    SemanticBinaryOpV1::Divide,
                    copy(1, WORD),
                    copy(2, WORD),
                ),
                7 => assignment(
                    8,
                    PAIR,
                    SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                        SemanticCheckedBinaryOpV1::Add,
                        copy(1, WORD),
                        scalar(1, 64),
                    )),
                ),
                8 => assignment(
                    8,
                    PAIR,
                    SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                        SemanticCheckedBinaryOpV1::Add,
                        copy(3, WORD),
                        scalar(16, 64),
                    )),
                ),
                9 => assignment(
                    8,
                    PAIR,
                    SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                        SemanticCheckedBinaryOpV1::Subtract,
                        copy(3, WORD),
                        scalar(1, 64),
                    )),
                ),
                10 => binary(
                    4,
                    BOOL,
                    SemanticBinaryOpV1::Equal,
                    copy(1, WORD),
                    scalar(0, 64),
                ),
                11 => assignment(
                    0,
                    WORD,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(match field(0) {
                        SemanticOperandV1::Move(p) => p,
                        _ => unreachable!(),
                    })),
                ),
                _ => unreachable!(),
            };
        });
        fixture.rejected();
    }
}

#[test]
fn caller_location_literal_divisor_overwrites_storage_and_extra_effects_reject() {
    for mutation in 0..6 {
        let mut fixture = Fixture::new(64, false, &[16]);
        fixture.edit_block(1, 1, |statements, _| {
            statements.push(match mutation {
                0 => assignment(2, WORD, SemanticRvalueKindV1::Use(scalar(0, 64))),
                1 => assignment(3, WORD, SemanticRvalueKindV1::Use(copy(1, WORD))),
                2 => assignment(4, BOOL, SemanticRvalueKindV1::Use(copy(6, BOOL))),
                3 => SemanticStatementV1::new(
                    provenance(),
                    SemanticStatementKindV1::StorageDead(local(2)),
                ),
                4 => SemanticStatementV1::new(
                    provenance(),
                    SemanticStatementKindV1::StorageLive(local(3)),
                ),
                5 => SemanticStatementV1::new(provenance(), SemanticStatementKindV1::Nop),
                _ => unreachable!(),
            });
        });
        fixture.rejected();
    }
}

#[test]
fn caller_location_literal_divisor_assert_conditions_messages_and_edges_reject() {
    for bb in [0, 1, 3] {
        for mutation in 0..5 {
            let mut fixture = Fixture::new(64, false, &[16]);
            fixture.edit_block(1, bb, |_, term| {
                let SemanticTerminatorKindV1::Assert {
                    condition,
                    expected,
                    message,
                    target,
                    ..
                } = term
                else {
                    panic!()
                };
                match mutation {
                    0 => *expected = true,
                    1 => *condition = copy(7, BOOL),
                    2 => *message = SemanticAssertMessageV1::DivisionByZero(copy(2, WORD)),
                    3 => *target = edge(SemanticEdgeRoleV1::AssertSuccess, 6),
                    4 => {
                        *message = SemanticAssertMessageV1::Overflow {
                            operation: SemanticBinaryOpV1::Multiply,
                            left: copy(3, WORD),
                            right: scalar(1, 64),
                        }
                    }
                    _ => unreachable!(),
                }
            });
            fixture.rejected();
        }
    }
}

#[test]
fn caller_location_literal_divisor_bypass_join_backedge_and_unreachable_observer_reject() {
    for mutation in 0..5 {
        let mut fixture = Fixture::new(64, false, &[16]);
        match mutation {
            0 => fixture.edit_block(1, 0, |_, term| {
                *term = SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 2))
            }),
            1 => fixture.edit_block(1, 2, |_, term| {
                *term = SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3))
            }),
            2 => fixture.edit_block(1, 4, |_, term| {
                *term = SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1))
            }),
            3 => fixture.edit_block(1, 2, |_, term| {
                *term = SemanticTerminatorKindV1::SwitchInt {
                    discriminant: moved(7, BOOL),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 3),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 5),
                    )
                    .unwrap(),
                }
            }),
            4 => {
                let old = &fixture.functions[1];
                let mut blocks = old.blocks().to_vec();
                blocks.push(block(
                    7,
                    vec![],
                    assertion(
                        copy(4, BOOL),
                        SemanticAssertMessageV1::DivisionByZero(copy(1, WORD)),
                        6,
                    ),
                ));
                fixture.functions[1] = replace_body(old, blocks);
            }
            _ => unreachable!(),
        }
        let source = fixture.admit();
        let expected_bb = if mutation == 0 { 1 } else { 0 };
        assert!(
            matches!(expand(&source), Err(SemanticCallExpansionErrorV1::Unsupported { function, block: Some(block), reason: "caller-location frame contains an observing assertion" }) if function == function_id(1) && block.index() == expected_bb)
        );
    }
}

#[test]
fn caller_location_literal_divisor_source_owner_call_and_site_are_exact() {
    let source = Fixture::new(64, false, &[16, 16]).admit();
    let other = Fixture::new(64, false, &[16, 16]).admit();
    let function = &source.functions()[1];
    let SemanticTerminatorKindV1::Call(call) =
        source.functions()[0].blocks()[0].terminator().kind()
    else {
        panic!()
    };
    let cloned_call = call.clone();
    let cloned_function = function.clone();
    for mutation in 0..5 {
        let mut budget = Budget::new(SemanticCallExpansionLimitsV1::default()).unwrap();
        let result = caller_location_v1::require_unobserved(
            if mutation == 0 { &other } else { &source },
            function_id(1),
            if mutation == 1 {
                &cloned_function
            } else {
                function
            },
            if mutation == 2 { &cloned_call } else { call },
            (
                function_id(if mutation == 3 { 1 } else { 0 }),
                SemanticBlockIdV1::from_index(if mutation == 4 { 1 } else { 0 }),
            ),
            &mut budget,
        );
        assert!(matches!(
            result,
            Err(SemanticCallExpansionErrorV1::Unsupported {
                reason: "caller-location frame contains an observing assertion",
                ..
            })
        ));
    }
    let expansion = expand(&source).unwrap();
    assert_eq!(
        expansion.verify_replay(&Fixture::new(64, false, &[2, 16]).admit()),
        Err(SemanticCallExpansionErrorV1::SourceMismatch)
    );
}

#[test]
fn caller_location_literal_divisor_shared_work_exact_boundary() {
    let source = Fixture::new(64, false, &[16, 16]).admit();
    let work = expand(&source).unwrap().work_units();
    let limits = SemanticCallExpansionLimitsV1 {
        work,
        ..SemanticCallExpansionLimitsV1::default()
    };
    SemanticCallExpansionV1::try_new(&source, limits)
        .unwrap()
        .verify_replay(&source)
        .unwrap();
    assert!(matches!(
        SemanticCallExpansionV1::try_new(
            &source,
            SemanticCallExpansionLimitsV1 {
                work: work - 1,
                ..limits
            }
        ),
        Err(SemanticCallExpansionErrorV1::Limit(
            SemanticCallExpansionResourceV1::Work
        ))
    ));
    let mut budget = Budget::new(limits).unwrap();
    budget.work(work - 1).unwrap();
    let SemanticTerminatorKindV1::Call(call) =
        source.functions()[0].blocks()[0].terminator().kind()
    else {
        panic!()
    };
    assert!(matches!(
        caller_location_v1::require_unobserved(
            &source,
            function_id(1),
            &source.functions()[1],
            call,
            (function_id(0), SemanticBlockIdV1::from_index(0)),
            &mut budget
        ),
        Err(SemanticCallExpansionErrorV1::Limit(
            SemanticCallExpansionResourceV1::Work
        ))
    ));
    assert_eq!(
        budget.used[SemanticCallExpansionResourceV1::Work as usize],
        work - 1
    );
}

#[test]
fn caller_location_literal_divisor_signed_and_unspecified_ownership_reject() {
    let mut signed = Fixture::new(64, false, &[16]);
    let word = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(true, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    let boolean = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 8, 1),
        SemanticScalarValidityRangeV1::new(0, 1),
    );
    signed.types[6] = SemanticTypeDeclV1::new(
        signed.types[6].identity(),
        signed.types[6].layout_identity(),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(word),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: true,
            bits: 64,
        }),
    );
    signed.types[7] = SemanticTypeDeclV1::new(
        signed.types[7].identity(),
        signed.types[7].layout_identity(),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::scalar_pair(word, boolean),
            false,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        signed.types[7].shape().clone(),
    );
    signed.rejected();

    let mut fixture = Fixture::new(64, false, &[16]);
    let old = &fixture.functions[1];
    fixture.functions[1] = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        old.source(),
        old.abi()
            .clone()
            .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::Unspecified; 2])
            .unwrap(),
        old.locals().to_vec(),
        old.entry(),
        old.blocks().to_vec(),
    )
    .unwrap();
    fixture.rejected();
}

#[test]
fn caller_location_literal_divisor_malformed_types_constants_and_arity_fail_admission() {
    for mutation in 0..5 {
        let mut fixture = Fixture::new(64, false, &[16]);
        fixture.edit_block(0, 0, |_, terminator| {
            let SemanticTerminatorKindV1::Call(call) = terminator else {
                panic!()
            };
            let mut args = call.arguments().to_vec();
            match mutation {
                0 => {
                    args[1] = SemanticOperandV1::Constant(SemanticConstantV1::new(
                        WORD,
                        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(16, 4).unwrap()),
                    ))
                }
                1 => {
                    args[1] = SemanticOperandV1::Constant(SemanticConstantV1::new(
                        BOOL,
                        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 1).unwrap()),
                    ))
                }
                2 => args[1] = copy(5, U32),
                3 => {
                    args.pop();
                }
                4 => {
                    args[1] = SemanticOperandV1::Constant(SemanticConstantV1::new(
                        WORD,
                        SemanticConstantValueV1::ZeroSized,
                    ))
                }
                _ => unreachable!(),
            }
            *terminator = change_call(call, args);
        });
        let result = fixture
            .request()
            .admit_current_production(SemanticMirLimitsV1::default());
        match mutation {
            0 | 4 => assert!(
                matches!(result, Err(SemanticMirErrorV1::InvalidTypeOperation { operation: SemanticTypeOperationV1::Constant, location: SemanticMirLocationV1::Terminator { function, block } }) if function == function_id(0) && block.index() == 0),
                "mutation {mutation}: {result:?}"
            ),
            1 | 2 => assert!(
                matches!(
                    result,
                    Err(SemanticMirErrorV1::TypeMismatch { expected: WORD, .. })
                ),
                "mutation {mutation}: {result:?}"
            ),
            3 => assert!(
                matches!(result, Err(SemanticMirErrorV1::InvalidCallShape { .. })),
                "mutation {mutation}: {result:?}"
            ),
            _ => unreachable!(),
        }
    }
}

#[test]
fn caller_location_literal_divisor_extra_call_drop_and_unwind_audits_remain_closed() {
    for mutation in 0..4 {
        let mut fixture = Fixture::new(64, false, &[16]);
        fixture.edit_block(1, 6, |_, terminator| {
            *terminator = match mutation {
                0 => SemanticTerminatorKindV1::Abort,
                1 => SemanticTerminatorKindV1::UnwindResume,
                2 => SemanticTerminatorKindV1::Drop {
                    place: place(3, WORD),
                    drop_glue: function_id(2),
                    target: edge(SemanticEdgeRoleV1::DropReturn, 5),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
                3 => SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new(
                        function_id(1),
                        vec![copy(1, WORD), scalar(16, 64)],
                        Some(SemanticCallDestinationV1::new(
                            place(3, WORD),
                            edge(SemanticEdgeRoleV1::CallReturn, 5),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
                _ => unreachable!(),
            }
        });
        if mutation == 2 {
            let pointer = SemanticTypeIdV1::from_index(8);
            let old = &fixture.types[PTR.index() as usize];
            fixture.types.push(
                SemanticTypeDeclV1::new(
                    SemanticTypeIdentityV1::from_sha256(identity(9)),
                    SemanticLayoutIdentityV1::from_sha256(identity(9)),
                    old.layout().clone(),
                    SemanticTypeShapeV1::Pointer(
                        SemanticPointerTypeV1::new(
                            WORD,
                            SemanticMutabilityV1::Mutable,
                            0,
                            64,
                            SemanticPointerMetadataV1::None,
                        )
                        .unwrap(),
                    ),
                )
                .with_rustc_abi_properties(old.abi_properties()),
            );
            let argument = SemanticAbiValueV1::new(
                pointer,
                SemanticAbiPassModeV1::Direct(
                    SemanticAbiValueAttributesV1::new(
                        SemanticAbiRegularAttributesV1::new(false, None, true, false, false, true),
                        SemanticAbiExtensionV1::None,
                        0,
                        Some(8),
                    )
                    .unwrap(),
                ),
            )
            .with_pointee_override(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::MutableReference { unpin: false },
                    0,
                    8,
                )
                .unwrap(),
            );
            let abi = SemanticFunctionAbiV1::new(
                SemanticAbiIdentityV1::from_sha256(identity(3)),
                SemanticLayoutIdentityV1::from_sha256(identity(3)),
                SemanticCanonAbiV1::Rust,
                false,
                false,
                vec![argument],
                SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
            )
            .unwrap();
            let helper = make_function(
                2,
                abi,
                vec![
                    (UNIT, SemanticLocalRoleV1::Return),
                    (pointer, SemanticLocalRoleV1::Argument(0)),
                ],
                vec![block(0, vec![], SemanticTerminatorKindV1::Return)],
            );
            fixture.functions.push(
                SemanticFunctionDeclV1::new(
                    helper.identity(),
                    SemanticFunctionRoleV1::DropGlue(WORD),
                    helper.item_definition_identity(),
                    helper.monomorphization_identity(),
                    helper.generic_type_arguments_identity(),
                    helper.const_generic_arguments_identity(),
                    helper.source(),
                    helper.abi().clone(),
                    helper.locals().to_vec(),
                    helper.entry(),
                    helper.blocks().to_vec(),
                )
                .unwrap(),
            );
        }
        fixture.rejected();
    }
}
