//! Component fixtures; injected extent records are not source authentication.
use super::*;
const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);

#[path = "optional_precision_tests.rs"]
mod optional_precision;

#[path = "optional_read_tests.rs"]
mod optional_read;

fn checked_fixture(operation: SemanticBinaryOpV1) -> Fixture {
    let mut f = Fixture::new(operation, 1024, false);
    f.types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([88; 32]),
        SemanticLayoutIdentityV1::from_sha256([89; 32]),
        SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![WORD, BOOL]).unwrap()),
    ));
    f.locals.extend([PAIR, WORD]);
    let pair = SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
        SemanticCheckedBinaryOpV1::Multiply,
        constant(WORD, 64, 8),
        constant(WORD, 16, 8),
    ));
    f.replace_block(
        2,
        vec![
            assign(4, WORD, SemanticRvalueKindV1::Use(copy(3, WORD))),
            assign(6, PAIR, pair),
        ],
        SemanticTerminatorKindV1::Assert {
            condition: field(6, 1, BOOL),
            expected: false,
            message: SemanticAssertMessageV1::Overflow {
                operation: SemanticBinaryOpV1::Multiply,
                left: constant(WORD, 64, 8),
                right: constant(WORD, 16, 8),
            },
            target: edge(SemanticEdgeRoleV1::AssertSuccess, 5),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
    );
    f.blocks.push(block(
        5,
        vec![assign(
            5,
            BOOL,
            SemanticRvalueKindV1::Binary {
                operation,
                left: copy(4, WORD),
                right: field(6, 0, WORD),
            },
        )],
        switch(BOOL, 3, 4),
    ));
    f
}

fn field(local: u32, index: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty).unwrap()],
            ty,
        )
        .unwrap(),
    )
}

fn project(
    f: &Fixture,
    operation: SemanticBinaryOpV1,
    mutation: u8,
    spent: usize,
) -> Result<
    (
        Vec<Option<ProjectedDeterministicSwitchV1>>,
        Vec<ProductionRankedOperationV1>,
    ),
    ProductionRankedProjectionErrorV1,
> {
    let original = f.function();
    let equality = matches!(
        operation,
        SemanticBinaryOpV1::Equal | SemanticBinaryOpV1::NotEqual
    );
    let mut locals = original.locals().to_vec();
    if equality {
        locals[7] = SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([99; 32]),
            WORD,
            SemanticLocalRoleV1::Argument(0),
            SemanticSourceProvenanceV1::unavailable(),
        );
    }
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([91; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([92; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([93; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([94; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        abi(if equality { vec![WORD] } else { vec![] }, UNIT),
        locals,
        SemanticBlockIdV1::from_index(0),
        f.blocks.clone(),
    )
    .unwrap();
    let foreign = function.clone();
    let inventory = assertion_definition_inventory(&function)?;
    // No exclusive-store pass ran: source comparisons must own initialization.
    let mut proofs = None;
    let mut indices = vec![None; f.locals.len()];
    for slot in &mut indices[1..=3] {
        *slot = Some(ProjectedDisjointIndexV1 {
            value: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
            mapping: SemanticDisjointIndexSpaceV1::Index1d,
            precondition: None,
            availability: None,
        });
    }
    let constants = vec![None; f.locals.len()];
    let mut arguments = vec![None; f.locals.len()];
    let mut extent_arguments = vec![None; f.locals.len()];
    let mut allocations = vec![None; f.locals.len()];
    let allocation = AllocationContractV1 {
        allocation_origin: 1,
        noalias_class: 4,
        writable: true,
        singleton_object: false,
    };
    allocations[7] = Some(allocation);
    let mut uses = ProjectedGlobalSemanticUsesV1::default();
    if mutation != 1 {
        let source = if mutation == 2 { &foreign } else { &function };
        let SemanticStatementKindV1::Assign(a) = source.blocks()[5].statements()[0].kind() else {
            unreachable!()
        };
        uses.source_unsigned_comparisons.insert(
            (5, 0),
            source_unsigned_switch_v1::SourceComparisonV1 {
                assignment: a,
                left: if equality {
                    Some(if mutation == 3 {
                        AllocationContractV1 {
                            allocation_origin: 2,
                            ..allocation
                        }
                    } else {
                        allocation
                    })
                } else {
                    None
                },
                right: None,
            },
        );
    }
    let mut next_argument = 0;
    let mut operations = vec![ProductionRankedOperationV1::InvocationIndex {
        result: ProductionRankedValueIdV1::new(0),
        dimension: 0,
        launch_extent: 1024,
    }];
    let mut next_value = 1;
    uses.ensure_source_comparison_proofs_v1(&f.types, &function, Some(1024), &mut proofs)?;
    let Some(proof) = proofs.as_mut() else {
        assert!(uses.source_unsigned_comparisons.is_empty());
        return Ok((Vec::new(), operations));
    };
    proof.charge(spent.saturating_sub(proof.work))?;
    let output = TotalUnsignedIndexProjectorV1::new(
        &f.types,
        &function,
        &constants,
        &inventory.counts,
        &inventory.address_escaped,
        &inventory.assignments,
        proof,
        &mut arguments,
        &mut next_argument,
        &mut operations,
        &mut next_value,
    )?
    .with_invocation_roots(&f.callables, &indices, 1024)?
    .optional_source_unsigned_switches_v1(&uses, &allocations, &mut extent_arguments)?;
    Ok((output, operations))
}

fn recorded_comparison(function: &SemanticFunctionDeclV1) -> ProjectedGlobalSemanticUsesV1 {
    let SemanticStatementKindV1::Assign(assignment) = function.blocks()[5].statements()[0].kind()
    else {
        unreachable!()
    };
    let mut uses = ProjectedGlobalSemanticUsesV1::default();
    uses.source_unsigned_comparisons.insert(
        (5, 0),
        source_unsigned_switch_v1::SourceComparisonV1 {
            assignment,
            left: None,
            right: None,
        },
    );
    uses
}

#[test]
fn source_comparison_owner_is_lazy_without_replay_or_launch() {
    let f = checked_fixture(SemanticBinaryOpV1::GreaterOrEqual);
    let function = f.function();
    let mut proofs = None;
    ProjectedGlobalSemanticUsesV1::default()
        .ensure_source_comparison_proofs_v1(&f.types, &function, Some(1024), &mut proofs)
        .unwrap();
    assert!(proofs.is_none());
    recorded_comparison(&function)
        .ensure_source_comparison_proofs_v1(&f.types, &function, None, &mut proofs)
        .unwrap();
    assert!(proofs.is_none());
}

#[test]
fn source_comparison_owner_initializes_without_exclusive_store() {
    let f = checked_fixture(SemanticBinaryOpV1::GreaterOrEqual);
    let function = f.function();
    let mut proofs = None;
    recorded_comparison(&function)
        .ensure_source_comparison_proofs_v1(&f.types, &function, Some(1024), &mut proofs)
        .unwrap();
    let proof = proofs
        .as_ref()
        .expect("source comparison initialized its owner");
    assert!(std::ptr::eq(proof.function, &function));
    assert!(std::ptr::eq(proof.types, f.types.as_slice()));
    assert!(
        proof.work > 0,
        "original bounded graph construction is charged"
    );
    assert!(
        project(&f, SemanticBinaryOpV1::GreaterOrEqual, 0, 0)
            .unwrap()
            .0[5]
            .is_some()
    );
}

#[test]
fn source_comparison_owner_preserves_existing_work_storage_and_dominance() {
    let f = checked_fixture(SemanticBinaryOpV1::GreaterOrEqual);
    let function = f.function();
    let mut proof = SemanticAssertProofsV1::new(&f.types, &function).unwrap();
    assert!(
        proof
            .assignment_dominates_use(
                ScalarAssignmentSiteV1 {
                    block: 2,
                    statement: 1
                },
                5,
                0,
            )
            .unwrap()
    );
    proof.charge(17).unwrap();
    let spent = proof.work;
    let definitions = proof.definition_counts.as_ptr();
    let dominance = proof.dominance.clone();
    assert!(!dominance.is_empty());
    let mut proofs = Some(proof);
    for _ in 0..2 {
        recorded_comparison(&function)
            .ensure_source_comparison_proofs_v1(&f.types, &function, Some(1024), &mut proofs)
            .unwrap();
        let proof = proofs.as_ref().unwrap();
        assert_eq!(proof.work, spent);
        assert_eq!(proof.definition_counts.as_ptr(), definitions);
        assert_eq!(proof.dominance, dominance);
    }
}

#[test]
fn source_comparison_owner_does_not_reset_an_exhausted_request() {
    let f = checked_fixture(SemanticBinaryOpV1::GreaterOrEqual);
    let function = f.function();
    let mut proof = SemanticAssertProofsV1::new(&f.types, &function).unwrap();
    proof
        .charge(MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - proof.work)
        .unwrap();
    let mut proofs = Some(proof);
    recorded_comparison(&function)
        .ensure_source_comparison_proofs_v1(&f.types, &function, Some(1024), &mut proofs)
        .unwrap();
    let proof = proofs.as_mut().unwrap();
    assert_eq!(proof.work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
    assert!(matches!(
        proof.charge(1),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis exceeds its work limit"
        ))
    ));
}

#[test]
fn source_comparison_owner_rejects_equal_but_foreign_function_or_types() {
    let f = checked_fixture(SemanticBinaryOpV1::GreaterOrEqual);
    let function = f.function();
    let foreign_function = function.clone();
    let foreign_types = f.types.clone();
    let uses = recorded_comparison(&function);
    let mut proofs = Some(SemanticAssertProofsV1::new(&f.types, &function).unwrap());
    let spent = proofs.as_ref().unwrap().work;
    for (types, function) in [
        (f.types.as_slice(), &foreign_function),
        (foreign_types.as_slice(), &function),
    ] {
        assert!(matches!(
            uses.ensure_source_comparison_proofs_v1(types, function, Some(1024), &mut proofs),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "source comparison proof owner differs from its exact execution function"
            ))
        ));
        assert_eq!(proofs.as_ref().unwrap().work, spent);
    }
}

#[test]
fn source_comparison_checked_product_retains_lt_and_original_ge_false_edge() {
    for operation in [
        SemanticBinaryOpV1::LessThan,
        SemanticBinaryOpV1::GreaterOrEqual,
    ] {
        let f = checked_fixture(operation);
        let (switches, operations) = project(&f, operation, 0, 0).unwrap();
        let projected = switches[5].as_ref().expect("exact total checked product");
        assert_eq!(
            projected.normalized_comparison,
            Some(SemanticBinaryOpV1::LessThan)
        );
        let [(_, rhs, yes)] = projected.targets.as_slice() else {
            panic!()
        };
        assert_eq!(
            *yes,
            if operation == SemanticBinaryOpV1::LessThan {
                4
            } else {
                3
            }
        );
        for i in [0, 1023, 1024, u32::MAX as u64] {
            assert_eq!(
                evaluate(&operations, projected.discriminant, i) < evaluate(&operations, *rhs, i),
                i < 1024
            );
        }
    }
}

#[test]
fn source_comparison_extent_eq_ne_requires_exact_replay_assignment_and_root() {
    for op in [SemanticBinaryOpV1::Equal, SemanticBinaryOpV1::NotEqual] {
        let mut f = checked_fixture(op);
        f.replace_block(
            5,
            vec![assign(
                5,
                BOOL,
                SemanticRvalueKindV1::Binary {
                    operation: op,
                    left: copy(7, WORD),
                    right: field(6, 0, WORD),
                },
            )],
            switch(BOOL, 3, 4),
        );
        let (switches, operations) = project(&f, op, 0, 0).unwrap();
        let output = switches[5].as_ref().unwrap();
        assert_eq!(
            output.normalized_comparison,
            Some(SemanticBinaryOpV1::Equal)
        );
        assert_eq!(output.discriminant, ProductionRankedValueV1::Argument(0));
        assert_eq!(evaluate(&operations, output.targets[0].1, 0), 1024);
        assert_eq!(
            output.targets[0].2,
            if op == SemanticBinaryOpV1::Equal {
                4
            } else {
                3
            }
        );
        assert!(!output.lane_uniform);
        for mutation in 1..=3 {
            assert!(
                project(&f, op, mutation, 0)
                    .unwrap()
                    .0
                    .get(5)
                    .is_none_or(Option::is_none),
                "mutation {mutation}"
            );
        }
    }
}

#[test]
fn source_comparison_rejects_type_projection_and_stale_result_mutations() {
    for mutation in 0..6 {
        let mut f = checked_fixture(SemanticBinaryOpV1::GreaterOrEqual);
        match mutation {
            0 => f.locals[4] = SIGNED,
            1 => f.replace_block(
                5,
                vec![assign(
                    5,
                    BOOL,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::GreaterOrEqual,
                        left: copy(4, WORD),
                        right: field(6, 1, WORD),
                    },
                )],
                switch(BOOL, 3, 4),
            ),
            2 => f.replace_block(
                5,
                vec![assign(
                    5,
                    BOOL,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::GreaterOrEqual,
                        left: copy(4, WORD),
                        right: field(6, 0, NARROW),
                    },
                )],
                switch(BOOL, 3, 4),
            ),
            3 => {
                let mut a = f.blocks[5].statements().to_vec();
                a.push(assign(
                    5,
                    BOOL,
                    SemanticRvalueKindV1::Use(constant(BOOL, 0, 1)),
                ));
                f.replace_block(5, a, switch(BOOL, 3, 4));
            }
            4 => f.replace_block(
                2,
                vec![],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 5)),
            ),
            5 => {
                let mut a = f.blocks[2].statements().to_vec();
                a.push(SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Deinitialize(place(6, PAIR)),
                ));
                let t = f.blocks[2].terminator().kind().clone();
                f.replace_block(2, a, t);
            }
            _ => unreachable!(),
        }
        assert!(
            project(&f, SemanticBinaryOpV1::GreaterOrEqual, 0, 0)
                .unwrap()
                .0[5]
                .is_none(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn source_comparison_charges_existing_shared_budget() {
    let f = checked_fixture(SemanticBinaryOpV1::GreaterOrEqual);
    assert!(matches!(
        project(
            &f,
            SemanticBinaryOpV1::GreaterOrEqual,
            0,
            MAX_PROJECTED_LOOP_GRAPH_WORK_V1
        ),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis exceeds its work limit"
        ))
    ));
}

#[test]
fn source_comparison_empty_replay_inventory_adds_no_precision_work() {
    let f = checked_fixture(SemanticBinaryOpV1::GreaterOrEqual);
    let (switches, _) = project(
        &f,
        SemanticBinaryOpV1::GreaterOrEqual,
        1,
        MAX_PROJECTED_LOOP_GRAPH_WORK_V1,
    )
    .unwrap();
    assert!(switches.is_empty());
}

#[test]
fn source_comparison_swapped_ordering_and_move_keep_exact_boolean_edges() {
    for operation in [
        SemanticBinaryOpV1::LessThan,
        SemanticBinaryOpV1::GreaterThan,
        SemanticBinaryOpV1::LessOrEqual,
        SemanticBinaryOpV1::GreaterOrEqual,
    ] {
        for swapped in [false, true] {
            let mut f = checked_fixture(operation);
            let mut left = SemanticOperandV1::Move(place(4, WORD));
            let SemanticOperandV1::Copy(right_place) = field(6, 0, WORD) else {
                unreachable!()
            };
            let mut right = SemanticOperandV1::Move(right_place);
            if swapped {
                std::mem::swap(&mut left, &mut right);
            }
            f.replace_block(
                5,
                vec![assign(
                    5,
                    BOOL,
                    SemanticRvalueKindV1::Binary {
                        operation,
                        left,
                        right,
                    },
                )],
                switch(BOOL, 3, 4),
            );
            let (switches, operations) = project(&f, operation, 0, 0).unwrap();
            let s = switches[5].as_ref().unwrap();
            for i in [0, 1023, 1024, 1025] {
                let (left, right) = if swapped { (1024, i) } else { (i, 1024) };
                let original = match operation {
                    SemanticBinaryOpV1::LessThan => left < right,
                    SemanticBinaryOpV1::GreaterThan => left > right,
                    SemanticBinaryOpV1::LessOrEqual => left <= right,
                    SemanticBinaryOpV1::GreaterOrEqual => left >= right,
                    _ => unreachable!(),
                };
                let normalized = evaluate(&operations, s.discriminant, i)
                    < evaluate(&operations, s.targets[0].1, i);
                let target = if normalized {
                    s.targets[0].2
                } else {
                    s.otherwise
                };
                assert_eq!(
                    target,
                    if original { 4 } else { 3 },
                    "{operation:?}, swapped {swapped}, point {i}"
                );
            }
        }
    }
}

#[test]
fn source_comparison_checked_custody_rejects_bypass_overflow_unwind_and_wrong_flag() {
    for mutation in 0..5 {
        let mut f = checked_fixture(SemanticBinaryOpV1::GreaterOrEqual);
        let mut statements = f.blocks[2].statements().to_vec();
        let mut terminal = f.blocks[2].terminator().kind().clone();
        match mutation {
            0 => terminal = SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 5)),
            1 => {
                let SemanticTerminatorKindV1::Assert { expected, .. } = &mut terminal else {
                    unreachable!()
                };
                *expected = true;
            }
            2 => {
                let SemanticTerminatorKindV1::Assert { unwind, .. } = &mut terminal else {
                    unreachable!()
                };
                *unwind = SemanticUnwindActionV1::Continue;
            }
            3 => {
                statements[1] = assign(
                    6,
                    PAIR,
                    SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                        SemanticCheckedBinaryOpV1::Multiply,
                        constant(WORD, u64::MAX, 8),
                        constant(WORD, 2, 8),
                    )),
                );
                let SemanticTerminatorKindV1::Assert { message, .. } = &mut terminal else {
                    unreachable!()
                };
                *message = SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::Multiply,
                    left: constant(WORD, u64::MAX, 8),
                    right: constant(WORD, 2, 8),
                };
            }
            4 => {
                let SemanticTerminatorKindV1::Assert { message, .. } = &mut terminal else {
                    unreachable!()
                };
                *message = SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::Add,
                    left: constant(WORD, 64, 8),
                    right: constant(WORD, 16, 8),
                };
            }
            _ => unreachable!(),
        }
        f.replace_block(2, statements, terminal);
        assert!(
            project(&f, SemanticBinaryOpV1::GreaterOrEqual, 0, 0)
                .unwrap()
                .0[5]
                .is_none(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn source_comparison_records_are_function_and_switch_use_scoped() {
    let mut f = checked_fixture(SemanticBinaryOpV1::GreaterOrEqual);
    // Same Boolean has an earlier use bypassing both definitions and the
    // retained overflow success edge. The later use remains exact.
    let statements = f.blocks[0].statements().to_vec();
    f.replace_block(0, statements, switch(BOOL, 1, 1));
    let (switches, _) = project(&f, SemanticBinaryOpV1::GreaterOrEqual, 0, 0).unwrap();
    assert!(switches[0].is_none());
    // Removing the invocation producer also conservatively removes late proof.
    assert!(switches[5].is_none());
    let f = checked_fixture(SemanticBinaryOpV1::GreaterOrEqual);
    assert!(
        project(&f, SemanticBinaryOpV1::GreaterOrEqual, 2, 0)
            .unwrap()
            .0[5]
            .is_none()
    );
}

#[test]
fn source_comparison_join_and_backedge_do_not_restore_overwritten_operands() {
    for mutation in 0..3 {
        let mut f = checked_fixture(SemanticBinaryOpV1::GreaterOrEqual);
        let mut terminal = f.blocks[2].terminator().kind().clone();
        let SemanticTerminatorKindV1::Assert { target, .. } = &mut terminal else {
            unreachable!()
        };
        *target = edge(SemanticEdgeRoleV1::AssertSuccess, 6);
        f.replace_block(2, f.blocks[2].statements().to_vec(), terminal);
        f.locals[7] = BOOL;
        f.blocks.push(block(
            6,
            vec![assign(
                7,
                BOOL,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Equal,
                    left: copy(3, WORD),
                    right: constant(WORD, 0, 8),
                },
            )],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: copy(7, BOOL),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, 7),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 8),
                )
                .unwrap(),
            },
        ));
        f.blocks.push(block(
            7,
            if mutation == 1 {
                vec![assign(
                    4,
                    WORD,
                    SemanticRvalueKindV1::Use(constant(WORD, 1024, 8)),
                )]
            } else {
                vec![]
            },
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 5)),
        ));
        f.blocks.push(block(
            8,
            vec![],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 5)),
        ));
        if mutation == 2 {
            f.replace_block(
                4,
                vec![assign(
                    4,
                    WORD,
                    SemanticRvalueKindV1::Use(constant(WORD, 1024, 8)),
                )],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 6)),
            );
        }
        assert_eq!(
            project(&f, SemanticBinaryOpV1::GreaterOrEqual, 0, 0)
                .unwrap()
                .0[5]
                .is_some(),
            mutation == 0,
            "mutation {mutation}"
        );
    }
}

#[test]
fn source_comparison_exact_overflow_success_must_dominate_every_use() {
    let mut f = checked_fixture(SemanticBinaryOpV1::GreaterOrEqual);
    f.locals[7] = BOOL;
    f.replace_block(
        1,
        f.blocks[1].statements().to_vec(),
        call(1, vec![copy(2, REFERENCE)], 3, WORD, 6),
    );
    f.blocks.push(block(
        6,
        vec![assign(
            7,
            BOOL,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Equal,
                left: copy(3, WORD),
                right: constant(WORD, 0, 8),
            },
        )],
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: copy(7, BOOL),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    0,
                    edge(SemanticEdgeRoleV1::SwitchValue, 2),
                )],
                edge(SemanticEdgeRoleV1::SwitchOtherwise, 5),
            )
            .unwrap(),
        },
    ));
    assert!(
        project(&f, SemanticBinaryOpV1::GreaterOrEqual, 0, 0)
            .unwrap()
            .0[5]
            .is_none()
    );
}
