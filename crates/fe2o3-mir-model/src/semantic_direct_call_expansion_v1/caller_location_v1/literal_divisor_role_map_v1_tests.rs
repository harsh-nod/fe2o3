use super::*;

fn permuted(
    function: &SemanticFunctionDeclV1,
    block_map: &[usize],
    local_map: &[usize],
) -> SemanticFunctionDeclV1 {
    assert_eq!(block_map.len(), function.blocks().len());
    assert_eq!(local_map.len(), function.locals().len());
    let check = |map: &[usize]| {
        let mut sorted = map.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..map.len()).collect::<Vec<_>>());
    };
    check(block_map);
    check(local_map);
    let map_place = |p: &SemanticPlaceV1| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local_map[p.local().index() as usize] as u32),
            p.projections().to_vec(),
            p.ty(),
        )
        .unwrap()
    };
    let operand = |value: &SemanticOperandV1| match value {
        SemanticOperandV1::Copy(p) => SemanticOperandV1::Copy(map_place(p)),
        SemanticOperandV1::Move(p) => SemanticOperandV1::Move(map_place(p)),
        SemanticOperandV1::Constant(_) => value.clone(),
    };
    let map_edge = |e: SemanticControlFlowEdgeV1| {
        SemanticControlFlowEdgeV1::new(
            e.role(),
            SemanticBlockIdV1::from_index(block_map[e.target().index() as usize] as u32),
        )
    };
    let mut blocks = vec![None; block_map.len()];
    for (old_index, original) in function.blocks().iter().enumerate() {
        let statements = original
            .statements()
            .iter()
            .map(|s| {
                let kind = match s.kind() {
                    SemanticStatementKindV1::Assign(a) => {
                        let kind = match a.value().kind() {
                            SemanticRvalueKindV1::Use(value) => {
                                SemanticRvalueKindV1::Use(operand(value))
                            }
                            SemanticRvalueKindV1::Binary {
                                operation,
                                left,
                                right,
                            } => SemanticRvalueKindV1::Binary {
                                operation: *operation,
                                left: operand(left),
                                right: operand(right),
                            },
                            SemanticRvalueKindV1::CheckedBinary(c) => {
                                SemanticRvalueKindV1::CheckedBinary(
                                    SemanticCheckedBinaryRvalueV1::new(
                                        c.operation(),
                                        operand(c.left()),
                                        operand(c.right()),
                                    ),
                                )
                            }
                            _ => panic!("test permutation requires one exact fixture expression"),
                        };
                        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                            map_place(a.destination()),
                            SemanticRvalueV1::new(a.value().result_type(), kind),
                        ))
                    }
                    SemanticStatementKindV1::Nop => SemanticStatementKindV1::Nop,
                    _ => panic!("test permutation requires one exact fixture statement"),
                };
                SemanticStatementV1::new(s.source(), kind)
            })
            .collect();
        let terminator = match original.terminator().kind() {
            SemanticTerminatorKindV1::Assert {
                condition,
                expected,
                message,
                target,
                unwind,
            } => {
                let message = match message {
                    SemanticAssertMessageV1::DivisionByZero(value) => {
                        SemanticAssertMessageV1::DivisionByZero(operand(value))
                    }
                    SemanticAssertMessageV1::RemainderByZero(value) => {
                        SemanticAssertMessageV1::RemainderByZero(operand(value))
                    }
                    SemanticAssertMessageV1::Overflow {
                        operation,
                        left,
                        right,
                    } => SemanticAssertMessageV1::Overflow {
                        operation: *operation,
                        left: operand(left),
                        right: operand(right),
                    },
                    _ => panic!("test permutation requires an original assertion kind"),
                };
                SemanticTerminatorKindV1::Assert {
                    condition: operand(condition),
                    expected: *expected,
                    message,
                    target: map_edge(*target),
                    unwind: match unwind {
                        SemanticUnwindActionV1::Cleanup(e) => {
                            SemanticUnwindActionV1::Cleanup(map_edge(*e))
                        }
                        other => *other,
                    },
                }
            }
            SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } => SemanticTerminatorKindV1::SwitchInt {
                discriminant: operand(discriminant),
                targets: SemanticSwitchTargetsV1::new(
                    targets
                        .values()
                        .iter()
                        .map(|t| SemanticSwitchTargetV1::new(t.value(), map_edge(t.edge())))
                        .collect(),
                    map_edge(targets.otherwise()),
                )
                .unwrap(),
            },
            SemanticTerminatorKindV1::Goto(e) => SemanticTerminatorKindV1::Goto(map_edge(*e)),
            SemanticTerminatorKindV1::Return => SemanticTerminatorKindV1::Return,
            SemanticTerminatorKindV1::Call(call) => SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    call.callee(),
                    call.arguments().iter().map(operand).collect(),
                    call.destination().map(|d| {
                        SemanticCallDestinationV1::new(map_place(d.place()), map_edge(d.edge()))
                    }),
                    call.unwind(),
                )
                .unwrap(),
            ),
            _ => panic!("test permutation requires a closed fixture terminator"),
        };
        blocks[block_map[old_index]] = Some(
            SemanticBasicBlockV1::new(
                function.blocks()[block_map[old_index]].identity(),
                original.source(),
                statements,
                SemanticTerminatorV1::new(original.terminator().source(), terminator),
            )
            .unwrap(),
        );
    }
    let mut locals = vec![None; local_map.len()];
    for (old, declaration) in function.locals().iter().enumerate() {
        // Each fixture is a distinct identity-sorted admitted source, not an
        // in-place reordering of an already authenticated document.
        locals[local_map[old]] = Some(SemanticLocalDeclV1::new(
            function.locals()[local_map[old]].identity(),
            declaration.ty(),
            declaration.role(),
            declaration.source(),
        ));
    }
    SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        function.abi().clone(),
        locals.into_iter().map(Option::unwrap).collect(),
        SemanticBlockIdV1::from_index(block_map[function.entry().index() as usize] as u32),
        blocks.into_iter().map(Option::unwrap).collect(),
    )
    .unwrap()
}

fn first_assertion(source: &AdmittedInertSemanticMirV1) -> SemanticBlockIdV1 {
    let index = source.functions()[1]
        .blocks()
        .iter()
        .position(|b| {
            matches!(
                b.terminator().kind(),
                SemanticTerminatorKindV1::Assert { .. }
            )
        })
        .unwrap();
    SemanticBlockIdV1::from_index(index as u32)
}

fn audit(
    source: &AdmittedInertSemanticMirV1,
    caller_block: usize,
    budget: &mut Budget,
) -> Result<()> {
    let SemanticTerminatorKindV1::Call(call) = source.functions()[0].blocks()[caller_block]
        .terminator()
        .kind()
    else {
        panic!("actual retained caller");
    };
    caller_location_v1::require_unobserved(
        source,
        function_id(1),
        &source.functions()[1],
        call,
        (
            function_id(0),
            SemanticBlockIdV1::from_index(caller_block as u32),
        ),
        budget,
    )
}

fn decoded(source: &AdmittedInertSemanticMirV1) -> AdmittedInertSemanticMirV1 {
    AdmittedInertSemanticMirV1::decode_current_production_canonical(
        source.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap()
}

fn exact_positive(source: &AdmittedInertSemanticMirV1, caller_order: &[usize]) {
    let bytes = source.canonical_encoding().to_vec();
    let copy = decoded(source);
    for document in [source, &copy] {
        let mut budget = Budget::new(SemanticCallExpansionLimitsV1::default()).unwrap();
        for &block in caller_order {
            audit(document, block, &mut budget).unwrap();
        }
        let expansion = expand(document).unwrap();
        expansion.verify_replay(document).unwrap();
        let root = expansion.root(function_id(0)).unwrap();
        assert_eq!(root.instances().len(), caller_order.len() + 1);
        let mut assertion_count = vec![0; root.instances().len()];
        for (block, origin) in root.body().blocks().iter().zip(root.block_origins()) {
            if !matches!(
                block.terminator().kind(),
                SemanticTerminatorKindV1::Assert { .. }
            ) {
                continue;
            }
            let instance = &root.instances()[origin.instance().index() as usize];
            assert_eq!(instance.function(), function_id(1));
            assertion_count[origin.instance().index() as usize] += 1;
            let original = &document.functions()[1].blocks()[origin.block().index() as usize];
            let expected = remap::terminator(
                original.terminator().kind(),
                instance,
                origin.block(),
                &mut budget,
            )
            .unwrap();
            assert_eq!(block.terminator().kind(), &expected);
            assert_eq!(block.terminator().source(), original.terminator().source());
            assert_eq!(
                origin.terminator(),
                SemanticExpandedTerminatorOriginV1::Source
            );
        }
        assert_eq!(assertion_count[0], 0);
        assert!(assertion_count[1..].iter().all(|&count| count == 3));
        assert_eq!(document.canonical_encoding(), bytes);
    }
}

const BLOCKS: [usize; 7] = [5, 4, 3, 2, 1, 0, 6];
const LOCALS: [usize; 9] = [4, 8, 3, 7, 1, 6, 0, 2, 5];

#[test]
fn caller_location_role_map_all_rotations_preserve_source_decoded_and_expansion() {
    for block_rotation in 0..7 {
        for local_rotation in 0..9 {
            let mut fixture = Fixture::new(64, false, &[16, 2]);
            let blocks = (0..7).map(|i| (i + block_rotation) % 7).collect::<Vec<_>>();
            let locals = (0..9).map(|i| (i + local_rotation) % 9).collect::<Vec<_>>();
            fixture.functions[1] = permuted(&fixture.functions[1], &blocks, &locals);
            exact_positive(&fixture.admit(), &[0, 1]);
        }
    }
}

#[test]
fn caller_location_role_map_independent_caller_and_helper_ids_and_unsigned_widths() {
    for bits in [8, 16, 32, 64, 128] {
        let mut fixture = Fixture::new(bits, false, &[16, 2]);
        fixture.functions[1] = permuted(&fixture.functions[1], &BLOCKS, &LOCALS);
        fixture.functions[0] = permuted(&fixture.functions[0], &[2, 0, 1], &[5, 3, 4, 1, 0, 2]);
        exact_positive(&fixture.admit(), &[2, 0]);
    }
}

#[test]
fn caller_location_role_map_different_second_call_cannot_reuse_safe_certificate() {
    for second in [0, 1, 2] {
        let mut fixture = Fixture::new(64, false, &[16, second]);
        if second == 2 {
            fixture.edit_block(0, 1, |_, terminator| {
                let SemanticTerminatorKindV1::Call(call) = terminator else {
                    panic!()
                };
                let mut args = call.arguments().to_vec();
                args[1] = copy(2, WORD);
                *terminator = change_call(call, args);
            });
        }
        fixture.functions[1] = permuted(&fixture.functions[1], &BLOCKS, &LOCALS);
        fixture.functions[0] = permuted(&fixture.functions[0], &[2, 0, 1], &[5, 3, 4, 1, 0, 2]);
        let source = fixture.admit();
        let copy = decoded(&source);
        for document in [&source, &copy] {
            let mut budget = Budget::new(SemanticCallExpansionLimitsV1::default()).unwrap();
            audit(document, 2, &mut budget).expect("the actual earlier literal call is certified");
            let expected = first_assertion(document);
            assert!(matches!(audit(document, 0, &mut budget),
                Err(SemanticCallExpansionErrorV1::Unsupported { function, block: Some(block),
                    reason: "caller-location frame contains an observing assertion" })
                    if function == function_id(1) && block == expected));
            assert!(matches!(expand(document),
                Err(SemanticCallExpansionErrorV1::Unsupported { function, block: Some(block),
                    reason: "caller-location frame contains an observing assertion" })
                    if function == function_id(1) && block == expected));
        }
    }
}

#[test]
fn caller_location_role_map_bijection_edges_definitions_and_flag_association_reject() {
    for mutation in 0..9 {
        let mut fixture = Fixture::new(64, false, &[16]);
        match mutation {
            0 => fixture.edit_block(1, 0, |_, term| {
                let SemanticTerminatorKindV1::Assert { target, .. } = term else {
                    panic!()
                };
                *target = edge(SemanticEdgeRoleV1::AssertSuccess, 0);
            }),
            1 => fixture.edit_block(1, 4, |_, term| {
                *term = SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1));
            }),
            2 => fixture.edit_block(1, 5, |_, term| {
                *term = SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 4));
            }),
            3 => fixture.edit_block(1, 2, |s, _| {
                s[0] = binary(
                    3,
                    WORD,
                    SemanticBinaryOpV1::Remainder,
                    copy(1, WORD),
                    copy(2, WORD),
                );
            }),
            4 => fixture.edit_block(1, 1, |s, _| {
                s[1] = binary(
                    4,
                    BOOL,
                    SemanticBinaryOpV1::Equal,
                    copy(2, WORD),
                    scalar(0, 64),
                );
            }),
            5 => fixture.edit_block(1, 3, |_, term| {
                let SemanticTerminatorKindV1::Assert { condition, .. } = term else {
                    panic!()
                };
                *condition = moved(4, BOOL);
            }),
            6 => fixture.edit_block(1, 3, |_, term| {
                let SemanticTerminatorKindV1::Assert { expected, .. } = term else {
                    panic!()
                };
                *expected = true;
            }),
            7 => fixture.edit_block(1, 2, |s, _| {
                s[1] = binary(
                    7,
                    BOOL,
                    SemanticBinaryOpV1::Equal,
                    copy(5, WORD),
                    scalar(0, 64),
                );
            }),
            8 => fixture.edit_block(1, 3, |s, _| {
                let SemanticStatementKindV1::Assign(a) = s[0].kind() else {
                    panic!()
                };
                s[0] = SemanticStatementV1::new(
                    provenance(),
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        SemanticPlaceV1::new(
                            local(8),
                            vec![
                                SemanticProjectionV1::new(SemanticProjectionKindV1::Subtype, PAIR)
                                    .unwrap(),
                            ],
                            PAIR,
                        )
                        .unwrap(),
                        a.value().clone(),
                    )),
                );
            }),
            _ => unreachable!(),
        }
        fixture.functions[1] = permuted(&fixture.functions[1], &BLOCKS, &LOCALS);
        let source = fixture.admit();
        let copy = decoded(&source);
        for document in [&source, &copy] {
            let expected = first_assertion(document);
            assert!(
                matches!(expand(document),
                Err(SemanticCallExpansionErrorV1::Unsupported { function, block: Some(block),
                    reason: "caller-location frame contains an observing assertion" })
                    if function == function_id(1) && block == expected),
                "mutation {mutation}"
            );
        }
    }
}

#[test]
fn caller_location_role_map_shared_budget_boundary_remains_exact() {
    let mut fixture = Fixture::new(64, false, &[16, 2]);
    fixture.functions[1] = permuted(&fixture.functions[1], &BLOCKS, &LOCALS);
    let source = fixture.admit();
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
}
