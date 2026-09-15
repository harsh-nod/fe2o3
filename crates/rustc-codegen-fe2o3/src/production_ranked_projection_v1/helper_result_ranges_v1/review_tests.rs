use super::super::{
    Engine, SCRATCH_CELLS, STORAGE_LIMIT, Scalar, State, Test, Value, WorkingCells, arithmetic,
    compare, refine, state_nodes,
};
use super::*;

fn scalar(lo: u128, hi: u128, tag: usize) -> Value {
    Value::Scalar(
        Scalar {
            lo,
            hi,
            stamp: Some((100, tag)),
        },
        None,
    )
}

fn facts<'a>(
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    cells: &mut WorkingCells,
    work: &mut usize,
) -> HelperResultRangesV1<'a> {
    HelperResultRangesV1 {
        types,
        function,
        operands: super::super::retained_facts_v1::FactBuilderV1::new(cells, work).unwrap(),
        retained: None,
    }
}

#[test]
fn helper_working_scope_actual_body_matches_unscoped_facts_and_rolls_back_errors() {
    let types = types();
    let function = fixture(Mutation::None);
    let mut direct_cells = WorkingCells::new(super::super::MAX_CELLS);
    let mut direct_work = 0;
    let direct = HelperResultRangesV1::analyze_in_cells(
        &types,
        &function,
        &mut direct_work,
        &mut direct_cells,
    )
    .unwrap();
    let mut cells = WorkingCells::new(super::super::MAX_CELLS);
    cells.reserve(9).unwrap();
    let mut required_work = 0;
    let facts = HelperResultRangesV1::analyze_with_cells(
        &types,
        &function,
        &mut required_work,
        &mut cells,
    )
    .unwrap();
    let scope_cells =
        std::mem::size_of::<super::super::working_scope_v1::WorkingScopeV1<'_>>()
            .div_ceil(std::mem::size_of::<usize>());
    assert_eq!(required_work, direct_work + scope_cells);
    assert_eq!(cells.peak, direct_cells.peak + 9 + scope_cells);
    assert!(!facts.operands.is_empty());
    assert_eq!(facts.operands, direct.operands);
    let retained = facts.operands.retained_cells().unwrap();
    assert_eq!(cells.live, 9 + retained);
    assert_eq!(
        facts.at(&types, &function, use_operand(&function), 8, 1),
        direct.at(&types, &function, use_operand(&function), 8, 1)
    );
    drop(facts);
    cells.release(retained);
    let mut exhausted = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - required_work + 1;
    assert!(matches!(
        HelperResultRangesV1::analyze_with_cells(&types, &function, &mut exhausted, &mut cells),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis exceeds its work limit"
        ))
    ));
    assert_eq!(cells.live, 9);
    assert!(cells.peak <= cells.limit);
    assert!(exhausted > MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
    // Capacity was released, but the same exhausted work counter still fails.
    assert!(matches!(
        HelperResultRangesV1::analyze_with_cells(&types, &function, &mut exhausted, &mut cells),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis exceeds its work limit"
        ))
    ));
    assert_eq!(cells.live, 9);
}

#[test]
fn helper_working_scope_actual_body_enforces_inherited_peak_and_one_short() {
    let types = types();
    let function = fixture(Mutation::None);
    let mut measured = WorkingCells::new(super::super::MAX_CELLS);
    measured.reserve(9).unwrap();
    let expected =
        HelperResultRangesV1::analyze_with_cells(&types, &function, &mut 0, &mut measured)
            .unwrap();
    let peak = measured.peak;
    let mut exact = WorkingCells::new(peak);
    exact.reserve(9).unwrap();
    let facts = HelperResultRangesV1::analyze_with_cells(&types, &function, &mut 0, &mut exact)
        .unwrap();
    assert_eq!(facts.operands, expected.operands);
    assert_eq!(exact.live, 9 + facts.operands.retained_cells().unwrap());
    assert_eq!(exact.peak, peak);

    let mut short = WorkingCells::new(peak - 1);
    short.reserve(9).unwrap();
    let mut work = 0;
    assert!(matches!(
        HelperResultRangesV1::analyze_with_cells(&types, &function, &mut work, &mut short),
        Err(ProductionRankedProjectionErrorV1::Unsupported(STORAGE_LIMIT))
    ));
    assert_eq!(short.live, 9);
    assert!(short.peak <= short.limit);
    assert!(work > 0);
}

#[test]
fn helper_retained_production_query_matches_builder_and_rejects_foreign_views() {
    let types = types();
    let function = fixture(Mutation::None);
    let operand = use_operand(&function);
    let mut cells = WorkingCells::new(super::super::MAX_CELLS);
    let builder =
        HelperResultRangesV1::analyze_with_cells(&types, &function, &mut 0, &mut cells).unwrap();
    let expected = builder.at(&types, &function, operand, 8, 1).unwrap();
    let retained = HelperResultRangesV1::analyze(&types, &function, &mut 0).unwrap();
    assert!(retained.operands.is_empty());
    assert_eq!(retained.operands.capacity(), 0);
    assert!(retained.retained.is_some());
    assert_eq!(
        retained
            .at_with_work(&types, &function, operand, 8, 1, &mut 0)
            .unwrap(),
        Some(expected)
    );
    let other_types = types.clone();
    let other_function = function.clone();
    let other_operand = operand.clone();
    for (types, body, operand, block, statement) in [
        (other_types.as_slice(), &function, operand, 8, 1),
        (types.as_slice(), &other_function, operand, 8, 1),
        (types.as_slice(), &function, &other_operand, 8, 1),
        (types.as_slice(), &function, operand, 7, 1),
        (types.as_slice(), &function, operand, 8, 0),
    ] {
        assert_eq!(
            retained
                .at_with_work(types, body, operand, block, statement, &mut 0)
                .unwrap(),
            None
        );
    }
    assert!(matches!(
        builder.at_with_work(&types, &function, operand, 8, 1, &mut 0),
        Err(ProductionRankedProjectionErrorV1::Incomplete(
            "helper range facts are not retained"
        ))
    ));
    let mut exhausted = MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
    assert!(
        retained
            .at_with_work(&types, &function, operand, 8, 1, &mut exhausted)
            .is_err()
    );
}

#[test]
fn helper_result_ranges_moves_are_sequential_and_self_moves_preserve_values() {
    let types = types();
    let function = fixture(Mutation::None);
    let mut work = 0;
    let mut cells = WorkingCells::new(SCRATCH_CELLS + 100);
    let mut state = State::from([(5, scalar(3, 3, 5))]);
    cells.reserve(SCRATCH_CELLS + state_nodes(&state)).unwrap();
    let mut engine = Engine {
        types: &types,
        function: &function,
        escaped: vec![false; function.locals().len()],
        work: &mut work,
        cells: &mut cells,
    };
    let mut facts = facts(&types, &function, engine.cells, engine.work);
    engine
        .statement(
            &mut state,
            assign(5, U64, SemanticRvalueKindV1::Use(moved(5, U64))).kind(),
            (0, 0),
            &mut facts,
        )
        .unwrap();
    assert_eq!(state[&5].scalar().unwrap().hi, 3);
    let twice = assign(
        7,
        PAIR,
        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
            SemanticCheckedBinaryOpV1::Multiply,
            moved(5, U64),
            moved(5, U64),
        )),
    );
    engine
        .statement(&mut state, twice.kind(), (0, 1), &mut facts)
        .unwrap();
    assert!(!state.contains_key(&5));
    assert!(
        !state.contains_key(&7),
        "second operand cannot read the first consumed value"
    );
    assert_eq!(
        engine.cells.live,
        SCRATCH_CELLS + state_nodes(&state) + facts.operands.retained_cells().unwrap()
    );
}

#[test]
fn helper_result_ranges_partial_move_and_overwrite_kill_only_the_selected_value() {
    let types = types();
    let function = fixture(Mutation::None);
    let mut work = 0;
    let mut cells = WorkingCells::new(SCRATCH_CELLS + 100);
    let mut state = State::from([(7, Value::Fields(vec![scalar(5, 5, 5), Value::exact(0)]))]);
    cells.reserve(SCRATCH_CELLS + state_nodes(&state)).unwrap();
    let mut engine = Engine {
        types: &types,
        function: &function,
        escaped: vec![false; function.locals().len()],
        work: &mut work,
        cells: &mut cells,
    };
    let mut facts = facts(&types, &function, engine.cells, engine.work);
    engine
        .statement(
            &mut state,
            assign(3, BOOL, SemanticRvalueKindV1::Use(field(7, 1, BOOL))).kind(),
            (0, 0),
            &mut facts,
        )
        .unwrap();
    assert_eq!(
        engine
            .operand(&state, &field(7, 0, U64))
            .scalar()
            .unwrap()
            .hi,
        5
    );
    assert_eq!(engine.operand(&state, &field(7, 1, BOOL)), Value::Unknown);
    engine
        .statement(
            &mut state,
            statement(SemanticStatementKindV1::Deinitialize(place(7, PAIR))).kind(),
            (0, 1),
            &mut facts,
        )
        .unwrap();
    assert!(!state.contains_key(&7));
    assert_eq!(state[&3].scalar().unwrap().hi, 0);
    assert_eq!(
        engine.cells.live,
        SCRATCH_CELLS + state_nodes(&state) + facts.operands.retained_cells().unwrap()
    );
}

#[test]
fn helper_result_ranges_unchecked_and_assertions_cannot_supply_their_own_proof() {
    let types = types();
    let function = fixture(Mutation::None);
    let mut work = 0;
    let mut cells = WorkingCells::new(SCRATCH_CELLS + 100);
    let mut state = State::from([
        (5, scalar(u64::MAX.into(), u64::MAX.into(), 5)),
        (6, scalar(1, 1, 6)),
    ]);
    cells.reserve(SCRATCH_CELLS + state_nodes(&state)).unwrap();
    let mut engine = Engine {
        types: &types,
        function: &function,
        escaped: vec![false; function.locals().len()],
        work: &mut work,
        cells: &mut cells,
    };
    let mut facts = facts(&types, &function, engine.cells, engine.work);
    let unchecked = assign(
        5,
        U64,
        SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
            SemanticUncheckedBinaryOpV1::Add,
            copy(5, U64),
            copy(6, U64),
        )),
    );
    let original = unchecked.clone();
    engine
        .statement(&mut state, unchecked.kind(), (0, 0), &mut facts)
        .unwrap();
    assert_eq!(unchecked, original);
    assert_eq!(
        (
            state[&5].scalar().unwrap().lo,
            state[&5].scalar().unwrap().hi
        ),
        (0, u64::MAX.into())
    );
    let condition = constant(BOOL, 0);
    let assertion = SemanticTerminatorKindV1::Assert {
        condition: condition.clone(),
        expected: true,
        message: SemanticAssertMessageV1::Overflow {
            operation: SemanticBinaryOpV1::Add,
            left: copy(5, U64),
            right: copy(6, U64),
        },
        target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
        unwind: SemanticUnwindActionV1::Unreachable,
    };
    let snapshot = state.clone();
    let mut outputs = engine
        .terminator(Some(state), &assertion, (0, 1), &mut facts)
        .unwrap();
    let mut state = outputs.pop().unwrap().unwrap();
    assert_eq!(
        state, snapshot,
        "even a literal failing assertion grants no successor assumptions"
    );
    engine
        .statement(
            &mut state,
            assign(
                7,
                PAIR,
                SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                    SemanticCheckedBinaryOpV1::Add,
                    copy(5, U64),
                    copy(6, U64),
                )),
            )
            .kind(),
            (1, 0),
            &mut facts,
        )
        .unwrap();
    assert!(!state.contains_key(&7));
    assert!(
        facts.at(&types, &function, &condition, 0, 1).is_none(),
        "Boolean facts are never exposed to the assert consumer"
    );
}

#[test]
fn helper_result_ranges_storage_reservations_include_queued_and_outgoing_states() {
    let types = types();
    let function = fixture(Mutation::None);
    let mut work = 0;
    let state = State::from([(5, scalar(1, 4, 5))]);
    let size = state_nodes(&state);
    let mut cells = WorkingCells::new(SCRATCH_CELLS + size * 2);
    // One queued state and the active state both remain physically live.
    cells.reserve(SCRATCH_CELLS + size * 2).unwrap();
    let mut engine = Engine {
        types: &types,
        function: &function,
        escaped: vec![false; function.locals().len()],
        work: &mut work,
        cells: &mut cells,
    };
    assert!(matches!(
        engine.copies(state, 2),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            STORAGE_LIMIT
        ))
    ));
    assert!(engine.cells.peak <= engine.cells.limit);
}

#[test]
fn helper_result_ranges_storage_reservations_precede_active_value_growth() {
    let types = types();
    let function = fixture(Mutation::None);
    let mut work = 0;
    let mut state = State::from([(5, scalar(1, 4, 5)), (6, scalar(1, 1, 6))]);
    let mut cells = WorkingCells::new(super::super::MAX_CELLS);
    cells.reserve(SCRATCH_CELLS + state_nodes(&state)).unwrap();
    let mut engine = Engine {
        types: &types,
        function: &function,
        escaped: vec![false; function.locals().len()],
        work: &mut work,
        cells: &mut cells,
    };
    let mut facts = facts(&types, &function, engine.cells, engine.work);
    let checked = assign(
        7,
        PAIR,
        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
            SemanticCheckedBinaryOpV1::Multiply,
            copy(5, U64),
            copy(6, U64),
        )),
    );
    let SemanticStatementKindV1::Assign(assignment) = checked.kind() else {
        unreachable!()
    };
    let mut inputs = Vec::new();
    assignment
        .value()
        .kind()
        .try_visit_operands(|operand| {
            inputs.push(engine.record(&state, operand, (0, 0), &mut facts)?);
            Ok::<(), ProductionRankedProjectionErrorV1>(())
        })
        .unwrap();
    let result = engine.rvalue(&state, assignment.value(), (0, 0), &inputs);
    assert!(matches!(result, Value::Fields(_)));
    // Fact capacities are already owned. Exhaust storage at the actual value
    // insertion, not at an earlier builder growth introduced by this change.
    let previous_peak = engine.cells.peak;
    engine.cells.limit = engine.cells.live;
    assert!(matches!(
        engine.replace_local(&mut state, 7, result),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            STORAGE_LIMIT
        ))
    ));
    assert!(!state.contains_key(&7));
    assert_eq!(engine.cells.live, engine.cells.limit);
    assert_eq!(engine.cells.peak, previous_peak);
}

#[test]
fn helper_result_ranges_join_peak_and_tight_storage_failure_are_exact() {
    let types = types();
    let function = fixture(Mutation::None);
    let mut cells = WorkingCells::new(super::super::MAX_CELLS);
    let proof =
        HelperResultRangesV1::analyze_with_cells(&types, &function, &mut 0, &mut cells).unwrap();
    assert!(
        proof
            .at(&types, &function, use_operand(&function), 8, 1)
            .is_some()
    );
    assert_eq!(
        cells.live,
        proof.operands.retained_cells().unwrap(),
        "only the returned inventory remains allocated"
    );
    let mut exact = WorkingCells::new(cells.peak);
    HelperResultRangesV1::analyze_with_cells(&types, &function, &mut 0, &mut exact).unwrap();
    let mut short = WorkingCells::new(cells.peak - 1);
    assert!(matches!(
        HelperResultRangesV1::analyze_with_cells(&types, &function, &mut 0, &mut short),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            STORAGE_LIMIT
        ))
    ));
    assert!(short.peak <= short.limit);
}

#[test]
fn helper_result_ranges_constants_require_declared_integer_and_exact_byte_width() {
    let types = types();
    let function = fixture(Mutation::None);
    let mut work = 0;
    let mut cells = WorkingCells::new(100);
    let engine = Engine {
        types: &types,
        function: &function,
        escaped: vec![false; function.locals().len()],
        work: &mut work,
        cells: &mut cells,
    };
    for (ty, bytes, bits) in [
        (U32, 8, 4),
        (BOOL, 4, 0),
        (BOOL, 1, 2),
        (PAIR, 8, 0),
        (I64, 8, 4),
    ] {
        let operand = SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, bytes).unwrap()),
        ));
        assert_eq!(engine.operand(&State::new(), &operand), Value::Unknown);
    }
}

#[test]
fn helper_result_ranges_sparse_joins_require_every_predecessor() {
    let types = types();
    for (other, expected) in [
        (Some(3), Some((3, 3))),
        (Some(7), Some((3, 7))),
        (None, None),
    ] {
        let template = fixture(Mutation::None);
        let function = SemanticFunctionDeclV1::new(
            template.identity(),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256([12; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([13; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([14; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([15; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            template.abi().clone(),
            template.locals().to_vec(),
            SemanticBlockIdV1::from_index(0),
            vec![
                block(0, vec![], switch(1, U32, 1, 2)),
                block(
                    1,
                    vec![assign(5, U64, SemanticRvalueKindV1::Use(constant(U64, 3)))],
                    goto(3),
                ),
                block(
                    2,
                    other
                        .map(|value| {
                            assign(5, U64, SemanticRvalueKindV1::Use(constant(U64, value)))
                        })
                        .into_iter()
                        .collect(),
                    goto(3),
                ),
                block(
                    3,
                    vec![assign(
                        12,
                        PAIR,
                        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                            SemanticCheckedBinaryOpV1::Multiply,
                            copy(5, U64),
                            constant(U64, 256),
                        )),
                    )],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        )
        .unwrap();
        let proof = HelperResultRangesV1::analyze(&types, &function, &mut 0).unwrap();
        let SemanticStatementKindV1::Assign(assignment) =
            function.blocks()[3].statements()[0].kind()
        else {
            panic!()
        };
        let SemanticRvalueKindV1::CheckedBinary(value) = assignment.value().kind() else {
            panic!()
        };
        assert_eq!(
            proof
                .at(&types, &function, value.left(), 3, 0)
                .map(|range| (range.minimum, range.maximum)),
            expected
        );
    }
}

#[test]
fn helper_result_ranges_interval_operations_cover_all_concrete_small_values() {
    use SemanticBinaryOpV1::*;
    for lo in 0..=5 {
        for hi in lo..=5 {
            for blo in 0..=5 {
                for bhi in blo..=5 {
                    let a = Scalar {
                        lo,
                        hi,
                        stamp: Some((0, 1)),
                    };
                    let b = Scalar {
                        lo: blo,
                        hi: bhi,
                        stamp: Some((0, 2)),
                    };
                    for op in [Add, Subtract, Multiply, Divide, Remainder] {
                        if let Some(range) = arithmetic(op, &a, &b, 15) {
                            for x in lo..=hi {
                                for y in blo..=bhi {
                                    let actual = match op {
                                        Add => x.checked_add(y),
                                        Subtract => x.checked_sub(y),
                                        Multiply => x.checked_mul(y),
                                        Divide => x.checked_div(y),
                                        Remainder => x.checked_rem(y),
                                        _ => unreachable!(),
                                    }
                                    .unwrap();
                                    assert!(
                                        range.lo <= actual && actual <= range.hi && actual <= 15
                                    );
                                }
                            }
                        }
                    }
                    for op in [
                        Equal,
                        NotEqual,
                        LessThan,
                        LessOrEqual,
                        GreaterThan,
                        GreaterOrEqual,
                    ] {
                        for truth in [false, true] {
                            let state = State::from([
                                (1, Value::Scalar(a.clone(), None)),
                                (2, Value::Scalar(b.clone(), None)),
                            ]);
                            let test = Value::Scalar(
                                Scalar {
                                    lo: 0,
                                    hi: 1,
                                    stamp: None,
                                },
                                Some(Test::Compare(op, a.clone(), b.clone())),
                            );
                            let refined = refine(state, &test, Some(u128::from(truth)), &[]);
                            for x in lo..=hi {
                                for y in blo..=bhi {
                                    let exact_a = Scalar {
                                        lo: x,
                                        hi: x,
                                        stamp: None,
                                    };
                                    let exact_b = Scalar {
                                        lo: y,
                                        hi: y,
                                        stamp: None,
                                    };
                                    if compare(op, &exact_a, &exact_b) == Some(truth) {
                                        let refined = refined
                                            .as_ref()
                                            .expect("feasible concrete edge remains");
                                        for (local, actual) in [(1, x), (2, y)] {
                                            let range = refined[&local].scalar().unwrap();
                                            assert!(range.lo <= actual && actual <= range.hi);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
