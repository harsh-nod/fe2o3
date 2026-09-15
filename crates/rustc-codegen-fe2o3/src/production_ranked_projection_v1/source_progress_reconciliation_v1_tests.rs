include!("source_progress_reconciliation_v1_tests/legacy.rs");

fn reconciliation_update_pair_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    loop_blocks: &[usize],
) {
    let definitions = local_definition_counts(function);
    let mut current = SemanticAssertProofsV1::new(types, function).unwrap();
    let mut previous = SemanticAssertProofsV1::new(types, function).unwrap();
    let graph = projected_loop_cfg_graph_v1(function).unwrap();
    let topology = ProjectedNaturalLoopTopologyV1 {
        preheader: 0,
        preheader_control: ProjectedInductionPreheaderControlV1::Direct,
        latch: 4,
        loop_blocks: loop_blocks.to_vec(),
    };
    let assignments = previous.assignments.clone();
    let mut current_alias = 0;
    let mut previous_alias = 0;
    let actual = source_induction_update_v1(
        &mut current,
        4,
        loop_blocks,
        SemanticLocalIdV1::from_index(1),
        0,
        &definitions,
        &mut current_alias,
    );
    let expected = legacy_source_induction_update_v1(
        function,
        &graph,
        &mut previous,
        &topology,
        SemanticLocalIdV1::from_index(1),
        0,
        &definitions,
        &assignments,
        &mut previous_alias,
    );
    match (actual, expected) {
        (Ok(actual), Ok(expected)) => {
            assert_eq!(actual, expected);
            if let (Some((_, actual)), Some((_, expected))) = (actual, expected) {
                assert!(std::ptr::eq(actual, expected));
            }
        }
        (Err(actual), Err(expected)) => assert_eq!(format!("{actual:?}"), format!("{expected:?}")),
        (actual, expected) => {
            panic!("update differential: actual={actual:?}, expected={expected:?}")
        }
    }
    assert_eq!(current.work, previous.work);
    assert_eq!(current_alias, previous_alias);
}

fn reconciliation_checked_query_v1<'a>(
    proof: &mut SemanticAssertProofsV1<'a>,
    definitions: &[u8],
    alias_work: &mut usize,
) -> Result<
    Option<(ProjectedSourceInductionUpdateV1, &'a SemanticOperandV1)>,
    ProductionRankedProjectionErrorV1,
> {
    source_induction_update_v1(
        proof,
        4,
        &[1, 2, 3, 4],
        SemanticLocalIdV1::from_index(1),
        0,
        definitions,
        alias_work,
    )
}

#[test]
fn reconciliation_borrowed_update_matches_frozen_source_semantics() {
    let types = assertion_proof_types();
    for latch in [
        WidenedLatchKind::Ordinary,
        WidenedLatchKind::Unchecked,
        WidenedLatchKind::Checked,
        WidenedLatchKind::CheckedWrongMessage,
        WidenedLatchKind::CheckedExpectedOverflow,
        WidenedLatchKind::CheckedReachableUnwind,
    ] {
        for step in [1, 16] {
            let function = widened_u64_induction_function_with_latch(step, latch);
            let original = function.clone();
            reconciliation_update_pair_v1(&types, &function, &[1, 2, 3, 4]);
            reconciliation_update_pair_v1(&types, &function, &[1, 2, 4]);
            assert_eq!(function, original);
        }
    }
}

#[test]
fn reconciliation_borrowed_update_rejects_unreachable_extra_predecessor() {
    let types = assertion_proof_types();
    let source = widened_u64_induction_function_with_latch(16, WidenedLatchKind::Checked);
    let mut blocks = source.blocks().to_vec();
    blocks.push(block(
        216,
        vec![],
        SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4)),
    ));
    let function = projection_function_with_locals(blocks, source.locals().to_vec());
    let original = function.clone();
    let definitions = local_definition_counts(&function);
    let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
    assert!(!proof.graph.is_entry_reachable(6));
    assert_eq!(proof.graph.predecessors(4), Some([3, 6].as_slice()));
    assert_incomplete(
        reconciliation_checked_query_v1(&mut proof, &definitions, &mut 0),
        "a checked induction latch does not have one exact producer predecessor",
    );
    reconciliation_update_pair_v1(&types, &function, &[1, 2, 3, 4]);
    assert_eq!(function, original);
}

#[test]
fn reconciliation_borrowed_update_keeps_exact_owner_allocations_and_result_borrow() {
    let types = assertion_proof_types();
    let function = widened_u64_induction_function_with_latch(16, WidenedLatchKind::Checked);
    let other = function.clone();
    let definitions = local_definition_counts(&function);
    let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
    let assignments = (proof.assignments.as_ptr(), proof.assignments.capacity());
    let escaped = (
        proof.address_escaped.as_ptr(),
        proof.address_escaped.capacity(),
    );
    let predecessor = proof.graph.predecessors(4).unwrap().as_ptr();
    let workspace = proof.graph.workspace_identity();
    let (kind, result) = reconciliation_checked_query_v1(&mut proof, &definitions, &mut 0)
        .unwrap()
        .unwrap();
    assert!(matches!(
        kind,
        ProjectedSourceInductionUpdateV1::Checked {
            producer_block: 3,
            producer_statement: 1,
            ..
        }
    ));
    let SemanticStatementKindV1::Assign(assignment) = function.blocks()[3].statements()[1].kind()
    else {
        unreachable!()
    };
    let SemanticRvalueKindV1::CheckedBinary(checked) = assignment.value().kind() else {
        unreachable!()
    };
    assert!(std::ptr::eq(result, checked.right()));
    assert_eq!(
        (proof.assignments.as_ptr(), proof.assignments.capacity()),
        assignments
    );
    assert_eq!(
        (
            proof.address_escaped.as_ptr(),
            proof.address_escaped.capacity()
        ),
        escaped
    );
    assert_eq!(proof.graph.predecessors(4).unwrap().as_ptr(), predecessor);
    assert_eq!(proof.graph.workspace_identity(), workspace);
    assert!(std::ptr::eq(proof.graph.source(), &function));
    let mut other_proof = SemanticAssertProofsV1::new(&types, &other).unwrap();
    let (_, other_result) = reconciliation_checked_query_v1(&mut other_proof, &definitions, &mut 0)
        .unwrap()
        .unwrap();
    assert_eq!(result, other_result);
    assert!(!std::ptr::eq(result, other_result));
    assert!(std::ptr::eq(other_proof.graph.source(), &other));
}

#[test]
fn reconciliation_borrowed_update_keeps_work_and_alias_ceiling_exact() {
    let types = assertion_proof_types();
    let function = widened_u64_induction_function_with_latch(16, WidenedLatchKind::Checked);
    let definitions = local_definition_counts(&function);
    let mut measured = SemanticAssertProofsV1::new(&types, &function).unwrap();
    let constructor = measured.work;
    let mut alias = 0;
    reconciliation_checked_query_v1(&mut measured, &definitions, &mut alias)
        .unwrap()
        .unwrap();
    let query = measured.work - constructor;
    assert!(query > 0 && alias > 0);
    for remaining in [query, query - 1] {
        let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
        let workspace = proof.graph.workspace_identity();
        proof
            .charge(MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - proof.work - remaining)
            .unwrap();
        let result = reconciliation_checked_query_v1(&mut proof, &definitions, &mut 0);
        if remaining == query {
            result.unwrap().unwrap();
            assert_eq!(proof.work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
        } else {
            assert_loop_unsupported(
                result,
                "uniform induction CFG analysis exceeds its work limit",
            );
            assert!(proof.work > MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
        }
        assert_eq!(proof.graph.workspace_identity(), workspace);
    }
    for remaining in [alias, alias - 1] {
        let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
        let mut used = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 - remaining;
        let result = reconciliation_checked_query_v1(&mut proof, &definitions, &mut used);
        if remaining == alias {
            result.unwrap().unwrap();
            assert_eq!(used, MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1);
        } else {
            assert_loop_unsupported(
                result,
                "uniform induction alias analysis exceeds its work limit",
            );
            assert!(used > MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1);
        }
    }
}

fn reconciliation_empty_v1(
    function: &SemanticFunctionDeclV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let types = projection_types();
    let locals = function.locals().len();
    reconcile_source_progress_and_emit_unsigned_casts_v1(
        &types,
        function,
        &vec![None; locals],
        &vec![None; locals],
        &local_definition_counts(function),
        &vec![None; locals],
        &mut [],
        &mut Vec::new(),
        &mut 0,
    )
}

#[test]
fn reconciliation_borrowed_graph_keeps_raw_node_capacity_gate_even_without_loops() {
    let exact = csr_source(MAX_RANKED_BOUNDS_BLOCKS, |_| {
        SemanticTerminatorKindV1::Return
    });
    reconciliation_empty_v1(&exact).unwrap();
    let oversized = csr_source(MAX_RANKED_BOUNDS_BLOCKS + 1, |_| {
        SemanticTerminatorKindV1::Return
    });
    assert!(matches!(
        reconciliation_empty_v1(&oversized),
        Err(ProductionRankedProjectionErrorV1::SourceCfgLimit(_))
    ));
}

#[test]
fn reconciliation_borrowed_graph_keeps_complete_normal_edge_capacity_gate() {
    let targets = (0..64).collect::<Vec<_>>();
    for extra in [false, true] {
        let function = csr_source(64, |block| {
            if block < 32 {
                csr_switch(&targets, 0)
            } else if extra && block == 32 {
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 0))
            } else {
                SemanticTerminatorKindV1::Return
            }
        });
        assert_eq!(MAX_RANKED_BOUNDS_EDGES, 32 * targets.len());
        if extra {
            assert_loop_unsupported(
                reconciliation_empty_v1(&function),
                "semantic CFG exceeds the ranked edge limit before loop analysis",
            );
        } else {
            reconciliation_empty_v1(&function).unwrap();
        }
    }
}

#[test]
fn reconciliation_borrowed_full_producer_matches_frozen_replay_and_mutations() {
    let types = assertion_proof_types();
    for latch in [
        WidenedLatchKind::Ordinary,
        WidenedLatchKind::Unchecked,
        WidenedLatchKind::Checked,
    ] {
        let function = widened_u64_induction_function_with_latch(16, latch);
        let original = function.clone();
        let constants = constant_locals(&function).unwrap();
        let origins = local_stable_argument_origins(&types, &function).unwrap();
        let definitions = local_definition_counts(&function);
        let mut arguments = vec![None; function.locals().len()];
        let mut next_argument = 1;
        let mut operations = Vec::new();
        let mut next_value = 0;
        let inductions = project_uniform_inductions_v1(
            &[],
            &types,
            &function,
            &constants,
            &origins,
            &definitions,
            &mut arguments,
            &mut next_argument,
            &mut operations,
            &mut next_value,
        )
        .unwrap();
        assert_eq!(inductions.len(), 1);
        for mutation in 0..4 {
            let mut actual = inductions.clone();
            match mutation {
                1 => actual[0].source_progress.step_value += 1,
                2 => actual[0].source_progress.ranked_bound = ProductionRankedValueV1::Argument(99),
                3 => {
                    actual[0]
                        .bound_cast
                        .as_mut()
                        .expect("finite u32 bound")
                        .bit_width = 64
                }
                _ => {}
            }
            let mut expected = actual.clone();
            let roster = (
                actual[0].loop_blocks.as_ptr(),
                actual[0].loop_blocks.capacity(),
            );
            let mut actual_operations = operations.clone();
            let mut expected_operations = operations.clone();
            let mut actual_next = next_value;
            let mut expected_next = next_value;
            let result = reconcile_source_progress_and_emit_unsigned_casts_v1(
                &types,
                &function,
                &constants,
                &origins,
                &definitions,
                &arguments,
                &mut actual,
                &mut actual_operations,
                &mut actual_next,
            );
            let oracle = legacy_reconcile_source_progress_and_emit_unsigned_casts_v1(
                &types,
                &function,
                &constants,
                &origins,
                &definitions,
                &arguments,
                &mut expected,
                &mut expected_operations,
                &mut expected_next,
            );
            assert_eq!(format!("{result:?}"), format!("{oracle:?}"));
            if mutation == 0 {
                result.unwrap();
                assert!(actual_operations.iter().any(|operation| matches!(
                    operation,
                    ProductionRankedOperationV1::IndexUnsignedCast { bit_width: 32, .. }
                )));
            } else {
                let message = match mutation {
                    1 => {
                        "a compiler-derived source induction latch changed type, step, or overflow semantics"
                    }
                    2 => "a compiler-derived source induction comparison changed type or value",
                    3 => {
                        "a compiler-derived unsigned cast does not match its exact semantic range and ranked source"
                    }
                    _ => unreachable!(),
                };
                assert_incomplete(result, message);
            }
            assert_eq!(actual, expected);
            assert_eq!(actual_operations, expected_operations);
            assert_eq!(actual_next, expected_next);
            assert_eq!(
                (
                    actual[0].loop_blocks.as_ptr(),
                    actual[0].loop_blocks.capacity()
                ),
                roster
            );
            assert_eq!(function, original);
        }
    }
}
