#[test]
fn lossless_csr_consumer_resume_state_has_only_scalar_fields() {
    let state = AssertionStrictUpperBoundStateV1 {
        local: 2,
        use_block: 4,
        next_switch_block: 1,
        range: Some(UnsignedRangeProofV1 {
            minimum: 0,
            maximum: 15,
        }),
        proven_upper_bound: Some(7),
    };
    let mut frames = Vec::new();
    let mut continuations = Vec::new();
    let mut work = 0;
    push_assertion_range_continuation_v1(
        &mut work,
        &mut continuations,
        AssertionRangeContinuationV1::StrictUpperBound(state),
    )
    .unwrap();
    push_assertion_range_frame_v1(
        &mut work,
        &mut frames,
        AssertionRangeFrameV1::ApplyStrictUpperBoundCandidate {
            switch_block: 1,
            true_target: 2,
        },
    )
    .unwrap();
    let Some(AssertionRangeFrameV1::ApplyStrictUpperBoundCandidate {
        switch_block,
        true_target,
    }) = frames.pop()
    else {
        panic!("the exact scalar continuation was not retained");
    };
    let state = pop_assertion_strict_upper_bound_v1(&mut continuations).unwrap();
    assert_eq!(
        (state.local, state.use_block, state.next_switch_block),
        (2, 4, 1)
    );
    assert_eq!((switch_block, true_target), (1, 2));
    assert_eq!(state.proven_upper_bound, Some(7));
    assert_eq!(
        work,
        2 * (std::mem::size_of::<AssertionRangeFrameV1<'_>>()
            + std::mem::size_of::<AssertionRangeContinuationV1>())
            .div_ceil(std::mem::size_of::<usize>())
    );
}

#[test]
fn lossless_csr_consumer_stack_growth_charges_allocation_and_relocation() {
    let mut stack = Vec::<usize>::new();
    let mut work = 0;
    for value in 0..17 {
        let capacity = stack.capacity();
        let length = stack.len();
        let before = work;
        let growth = if capacity == length {
            capacity + capacity.max(1) + length
        } else {
            0
        };
        reserve_assertion_stack_slot_v1(&mut work, &mut stack, "test reservation").unwrap();
        assert_eq!(work - before, 1 + growth);
        if growth != 0 {
            assert!(stack.capacity() >= capacity + capacity.max(1));
        } else {
            assert_eq!(stack.capacity(), capacity);
        }
        stack.push(value);
    }
    assert_eq!(stack, (0..17).collect::<Vec<_>>());
    let mut zero_sized = Vec::<()>::new();
    let before = work;
    reserve_assertion_stack_slot_v1(&mut work, &mut zero_sized, "test reservation").unwrap();
    assert_eq!(work, before + 1);
}

#[test]
fn lossless_csr_consumer_stack_growth_fails_before_relocating_live_values() {
    let mut stack = Vec::<usize>::with_capacity(2);
    stack.resize(stack.capacity(), 7);
    let capacity = stack.capacity();
    let pointer = stack.as_ptr();
    let mut work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - 1;
    assert!(reserve_assertion_stack_slot_v1(&mut work, &mut stack, "test reservation").is_err());
    assert_eq!(stack.capacity(), capacity);
    assert_eq!(stack.as_ptr(), pointer);
    assert!(stack.iter().all(|&value| value == 7));
}

#[test]
fn lossless_csr_consumer_frame_and_value_reservations_fail_before_allocation() {
    let mut work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
    let mut frames = Vec::new();
    assert!(
        push_assertion_range_frame_v1(
            &mut work,
            &mut frames,
            AssertionRangeFrameV1::FinishProjectedPlace { local: 2 }
        )
        .is_err()
    );
    assert_eq!(frames.capacity(), 0);
    assert!(frames.is_empty());
    let mut work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
    let mut values = Vec::new();
    assert!(push_assertion_range_value_v1(&mut work, &mut values, None).is_err());
    assert_eq!(values.capacity(), 0);
    assert!(values.is_empty());
}

#[test]
fn lossless_csr_consumer_binary_frame_borrows_exact_operand_payloads() {
    let function = csr_source(1, |_| SemanticTerminatorKindV1::Return);
    let types = assertion_proof_types();
    let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
    let value = SemanticRvalueV1::new(
        SCALAR_TYPE,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::Add,
            left: constant(3),
            right: constant(4),
        },
    );
    let SemanticRvalueKindV1::Binary { left, right, .. } = value.kind() else {
        unreachable!()
    };
    let task = proof.assertion_range_expression_task_v1(&value);
    let mut frames = Vec::new();
    proof
        .schedule_assertion_range_expression_v1(
            &mut frames,
            &mut Vec::new(),
            task,
            ScalarAssignmentSiteV1 {
                block: 0,
                statement: 0,
            },
        )
        .unwrap();
    let AssertionRangeFrameV1::FinishBinary {
        left_source,
        right_source,
        ..
    } = &frames[0]
    else {
        panic!("the binary continuation was not retained");
    };
    assert!(std::ptr::eq(*left_source, left));
    assert!(std::ptr::eq(*right_source, right));
}

#[test]
fn lossless_csr_consumer_site_prefix_does_not_become_whole_block_freshness() {
    let function = projection_function(vec![
        block(
            231,
            vec![],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        block(
            232,
            vec![
                statement(SemanticStatementKindV1::Nop),
                typed_assignment(2, SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(7))),
            ],
            SemanticTerminatorKindV1::Return,
        ),
    ]);
    let mut work = 0;
    let mut graph = LosslessCsrV1::build(
        &function,
        CsrWorkV1::new(&mut work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
    )
    .unwrap();
    let allocation = graph.workspace_identity();
    graph
        .query(
            CsrWorkV1::new(&mut work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
            |mut query| {
                assert!(query.stable_edge_to_site(
                    2,
                    1,
                    ScalarAssignmentSiteV1 {
                        block: 1,
                        statement: 1
                    }
                )?);
                assert!(!query.stable_edge_to_site(
                    2,
                    1,
                    ScalarAssignmentSiteV1 {
                        block: 1,
                        statement: 2
                    }
                )?);
                assert!(!query.stable_edge_to_block(2, 1, 1)?);
                assert!(query.stable_capture_successors_to_site(
                    2,
                    0,
                    ScalarAssignmentSiteV1 {
                        block: 1,
                        statement: 1
                    }
                )?);
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(graph.workspace_identity(), allocation);
}

#[test]
fn lossless_csr_consumer_each_dynamic_use_requires_the_same_success_edge() {
    for bypass in [false, true] {
        let function = csr_source(4, |block| match block {
            0 => csr_switch(&[3], 1),
            1 => SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
            2 => SemanticTerminatorKindV1::Goto(cfg_edge(
                SemanticEdgeRoleV1::Goto,
                if bypass { 1 } else { 0 },
            )),
            _ => SemanticTerminatorKindV1::Return,
        });
        let mut work = 0;
        let mut graph = LosslessCsrV1::build(
            &function,
            CsrWorkV1::new(&mut work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
        )
        .unwrap();
        graph
            .query(
                CsrWorkV1::new(&mut work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
                |mut query| {
                    assert!(query.edge_set_dominates(&HashSet::from([(0, 1)]), 1)?);
                    assert_eq!(query.guard_authenticates_each_use((0, 1), 1)?, !bypass);
                    assert!(!query.guard_authenticates_each_use((0, 3), 1)?);
                    Ok(())
                },
            )
            .unwrap();
    }
}

#[test]
fn lossless_csr_consumer_raw_cap_retains_the_source_diagnostic() {
    let function = csr_source(MAX_RANKED_BOUNDS_BLOCKS + 1, |_| {
        SemanticTerminatorKindV1::Return
    });
    let types = assertion_proof_types();
    assert!(matches!(
        SemanticAssertProofsV1::new(&types, &function),
        Err(ProductionRankedProjectionErrorV1::SourceCfgLimit(_))
    ));
}

#[test]
fn lossless_csr_consumer_keeps_graph_header_and_construction_debits() {
    let function = csr_source(3, |block| {
        if block < 2 {
            SemanticTerminatorKindV1::Goto(cfg_edge(
                SemanticEdgeRoleV1::Goto,
                u32::try_from(block + 1).unwrap(),
            ))
        } else {
            SemanticTerminatorKindV1::Return
        }
    });
    let types = assertion_proof_types();
    let mut expected =
        std::mem::size_of::<LosslessCsrV1<'_>>().div_ceil(std::mem::size_of::<usize>());
    LosslessCsrV1::build(
        &function,
        CsrWorkV1::new(&mut expected, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
    )
    .unwrap();
    let proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
    assert!(std::ptr::eq(proof.graph.source(), &function));
    assert_eq!(proof.work, expected);
}
