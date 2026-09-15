fn frame_test_state_v1(local: usize) -> AssertionStrictUpperBoundStateV1 {
    AssertionStrictUpperBoundStateV1 {
        local,
        use_block: local + 1,
        next_switch_block: local + 2,
        range: Some(UnsignedRangeProofV1 {
            minimum: 3,
            maximum: 17,
        }),
        proven_upper_bound: Some(11),
    }
}

#[test]
fn assertion_frame_compact_layout_does_not_inline_computed_continuations() {
    assert!(
        std::mem::size_of::<AssertionRangeFrameV1<'_>>()
            < std::mem::size_of::<AssertionRangeContinuationV1>()
    );
    let operand = typed_constant(U64_TYPE, 19, 8);
    let SemanticOperandV1::Constant(source) = &operand else {
        unreachable!()
    };
    let AssertionRangeOperandTaskV1::Constant(retained) =
        SemanticAssertProofsV1::assertion_range_operand_task_v1(&operand)
    else {
        panic!("constant source was not retained")
    };
    assert!(std::ptr::eq(source, retained));
}

#[test]
fn assertion_frame_continuations_preserve_nested_roles_values_and_sites() {
    let mut pending = Vec::new();
    let mut work = 0;
    for item in [
        AssertionRangeContinuationV1::StrictUpperBound(frame_test_state_v1(7)),
        AssertionRangeContinuationV1::RelationalRange(UnsignedRangeProofV1::exact(29)),
        AssertionRangeContinuationV1::StrictUpperBound(frame_test_state_v1(13)),
    ] {
        let capacity = pending.capacity();
        let length = pending.len();
        let before = work;
        let words = std::mem::size_of::<AssertionRangeContinuationV1>()
            .div_ceil(std::mem::size_of::<usize>());
        let growth = if length == capacity {
            capacity + capacity.max(1) + length
        } else {
            0
        };
        push_assertion_range_continuation_v1(&mut work, &mut pending, item).unwrap();
        assert_eq!(work - before, words * (1 + growth));
    }
    let inner = pop_assertion_strict_upper_bound_v1(&mut pending).unwrap();
    assert_eq!(
        (inner.local, inner.use_block, inner.next_switch_block),
        (13, 14, 15)
    );
    assert_eq!(
        inner.range,
        Some(UnsignedRangeProofV1 {
            minimum: 3,
            maximum: 17
        })
    );
    assert_eq!(inner.proven_upper_bound, Some(11));
    assert_eq!(
        pop_assertion_relational_range_v1(&mut pending).unwrap(),
        UnsignedRangeProofV1::exact(29)
    );
    let outer = pop_assertion_strict_upper_bound_v1(&mut pending).unwrap();
    assert_eq!(
        (outer.local, outer.use_block, outer.next_switch_block),
        (7, 8, 9)
    );
    assert!(pending.is_empty());
}

#[test]
fn assertion_frame_missing_or_changed_continuation_role_is_rejected() {
    for mut pending in [
        Vec::new(),
        vec![AssertionRangeContinuationV1::RelationalRange(
            UnsignedRangeProofV1::exact(7),
        )],
    ] {
        assert!(matches!(
            pop_assertion_strict_upper_bound_v1(&mut pending),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "assertion range evaluator strict continuation is inconsistent"
            ))
        ));
    }
    for mut pending in [
        Vec::new(),
        vec![AssertionRangeContinuationV1::StrictUpperBound(
            frame_test_state_v1(7),
        )],
    ] {
        assert!(matches!(
            pop_assertion_relational_range_v1(&mut pending),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "assertion range evaluator relational continuation is inconsistent"
            ))
        ));
    }
}

#[test]
fn assertion_frame_continuation_work_fails_before_allocation_or_relocation() {
    let mut pending = Vec::new();
    let mut work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
    assert!(
        push_assertion_range_continuation_v1(
            &mut work,
            &mut pending,
            AssertionRangeContinuationV1::StrictUpperBound(frame_test_state_v1(3))
        )
        .is_err()
    );
    assert_eq!(pending.capacity(), 0);
    assert!(pending.is_empty());
    let mut work = 0;
    push_assertion_range_continuation_v1(
        &mut work,
        &mut pending,
        AssertionRangeContinuationV1::StrictUpperBound(frame_test_state_v1(3)),
    )
    .unwrap();
    while pending.len() < pending.capacity() {
        push_assertion_range_continuation_v1(
            &mut work,
            &mut pending,
            AssertionRangeContinuationV1::StrictUpperBound(frame_test_state_v1(3)),
        )
        .unwrap();
    }
    let pointer = pending.as_ptr();
    let capacity = pending.capacity();
    let words =
        std::mem::size_of::<AssertionRangeContinuationV1>().div_ceil(std::mem::size_of::<usize>());
    let mut work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - words;
    assert!(
        push_assertion_range_continuation_v1(
            &mut work,
            &mut pending,
            AssertionRangeContinuationV1::StrictUpperBound(frame_test_state_v1(11))
        )
        .is_err()
    );
    assert_eq!(pending.as_ptr(), pointer);
    assert_eq!(pending.capacity(), capacity);
    assert_eq!(pending.len(), capacity);
    assert!(work > MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
    assert_eq!(
        pop_assertion_strict_upper_bound_v1(&mut pending)
            .unwrap()
            .local,
        3
    );
}

#[test]
fn assertion_frame_whole_consumer_has_exact_shared_work_boundary() {
    let (mut statements, mut locals) = linear_scalar_alias_chain_v1(128, U64_TYPE);
    statements.insert(
        0,
        typed_assignment(
            1,
            U64_TYPE,
            SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, 7, 8)),
        ),
    );
    locals[1] = local(1, U64_TYPE, SemanticLocalRoleV1::Temporary);
    let function = projection_function_with_locals(
        vec![block(211, statements, SemanticTerminatorKindV1::Return)],
        locals,
    );
    let types = assertion_proof_types();
    let operand = typed_operand(129, U64_TYPE);
    let use_statement = function.blocks()[0].statements().len();
    let mut measured = SemanticAssertProofsV1::new(&types, &function).unwrap();
    let constructor = measured.work;
    assert_eq!(
        measured
            .range_at_operand(&operand, 0, use_statement)
            .unwrap(),
        Some(UnsignedRangeProofV1::exact(7))
    );
    let delta = measured.work - constructor;
    let mut exact = SemanticAssertProofsV1::new(&types, &function).unwrap();
    exact
        .charge(MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - exact.work - delta)
        .unwrap();
    assert_eq!(
        exact.range_at_operand(&operand, 0, use_statement).unwrap(),
        Some(UnsignedRangeProofV1::exact(7))
    );
    assert_eq!(exact.work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
    let mut short = SemanticAssertProofsV1::new(&types, &function).unwrap();
    short
        .charge(MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - short.work - delta + 1)
        .unwrap();
    assert!(matches!(
        short.range_at_operand(&operand, 0, use_statement),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis exceeds its work limit"
        ))
    ));
    assert!(short.work > MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
}

#[derive(Clone, Copy, Debug)]
enum FrameResultGuardV1 {
    Exact,
    Missing,
    WrongMessage,
    WrongPolarity,
    Bypass,
}

fn frame_scaled_remainder_consumer_v1(
    hostility: ScaledRemainderHostilityV1,
    guard: FrameResultGuardV1,
) -> SemanticFunctionDeclV1 {
    let source = scaled_remainder_function(hostility);
    let mut blocks = source.blocks().to_vec();
    let mut result_guard = blocks[4].terminator().kind().clone();
    let SemanticTerminatorKindV1::Assert {
        target,
        expected,
        message,
        ..
    } = &mut result_guard
    else {
        unreachable!()
    };
    *target = cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 7);
    match guard {
        FrameResultGuardV1::WrongMessage => {
            let SemanticAssertMessageV1::Overflow { operation, .. } = message else {
                unreachable!()
            };
            *operation = SemanticBinaryOpV1::Add;
        }
        FrameResultGuardV1::WrongPolarity => *expected = true,
        FrameResultGuardV1::Exact | FrameResultGuardV1::Missing | FrameResultGuardV1::Bypass => {}
    }
    if matches!(guard, FrameResultGuardV1::Missing) {
        result_guard = SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 7));
    }
    blocks[4] = block(221, blocks[4].statements().to_vec(), result_guard);
    if matches!(guard, FrameResultGuardV1::Bypass) {
        blocks[3] = block(
            220,
            blocks[3].statements().to_vec(),
            zero_switch(14, BOOL_TYPE, 7, 4),
        );
    }
    // A separate success-only consumer avoids the fixture's rejecting-edge return join.
    blocks.push(block(
        224,
        vec![typed_assignment(
            0,
            U64_TYPE,
            SemanticRvalueKindV1::Use(checked_field_operand(15, 0, U64_TYPE)),
        )],
        SemanticTerminatorKindV1::Return,
    ));
    projection_function_with_locals(blocks, source.locals().to_vec())
}

fn frame_scaled_remainder_range_v1(
    proof: &mut SemanticAssertProofsV1<'_>,
) -> Option<UnsignedRangeProofV1> {
    let function = proof.function;
    let SemanticStatementKindV1::Assign(assignment) = function.blocks()[7].statements()[0].kind()
    else {
        unreachable!()
    };
    let SemanticRvalueKindV1::Use(operand) = assignment.value().kind() else {
        unreachable!()
    };
    proof.range_at_operand(operand, 7, 0).unwrap()
}

#[test]
fn assertion_frame_actual_scaled_result_preserves_mixed_relational_strict_nesting() {
    let types = assertion_proof_types();
    let function = frame_scaled_remainder_consumer_v1(
        ScaledRemainderHostilityV1::Exact,
        FrameResultGuardV1::Exact,
    );
    let original = function.clone();
    let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
    assert_eq!(
        frame_scaled_remainder_range_v1(&mut proof),
        Some(UnsignedRangeProofV1 {
            minimum: 0,
            maximum: 7
        })
    );
    assert_eq!(function, original);
    assert!(proof.work <= MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
}

#[test]
fn assertion_frame_actual_scaled_result_rejects_invalid_relational_ranges() {
    for hostility in [
        ScaledRemainderHostilityV1::OffsetNotBelowScale,
        ScaledRemainderHostilityV1::WrongNumerator,
        ScaledRemainderHostilityV1::WrongDivisorExtent,
        ScaledRemainderHostilityV1::WrongDivisorScale,
        ScaledRemainderHostilityV1::UnstableNumerator,
        ScaledRemainderHostilityV1::UnstableExtent,
        ScaledRemainderHostilityV1::UnstableScale,
        ScaledRemainderHostilityV1::MissingSumAssertion,
        ScaledRemainderHostilityV1::MissingHeadAssertion,
    ] {
        let types = assertion_proof_types();
        let function = frame_scaled_remainder_consumer_v1(hostility, FrameResultGuardV1::Exact);
        let original = function.clone();
        let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
        assert_eq!(
            frame_scaled_remainder_range_v1(&mut proof),
            None,
            "{hostility:?}"
        );
        assert_eq!(function, original);
    }
}

#[test]
fn assertion_frame_actual_scaled_result_rejects_missing_substituted_or_bypassed_guard() {
    for guard in [
        FrameResultGuardV1::Missing,
        FrameResultGuardV1::WrongMessage,
        FrameResultGuardV1::WrongPolarity,
        FrameResultGuardV1::Bypass,
    ] {
        let types = assertion_proof_types();
        let function = frame_scaled_remainder_consumer_v1(ScaledRemainderHostilityV1::Exact, guard);
        let original = function.clone();
        let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
        assert_eq!(
            frame_scaled_remainder_range_v1(&mut proof),
            None,
            "{guard:?}"
        );
        assert_eq!(function, original);
    }
}
