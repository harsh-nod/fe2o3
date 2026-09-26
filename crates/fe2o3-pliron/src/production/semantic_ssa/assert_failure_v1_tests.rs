use super::*;
use fe2o3_mir_model::{SsaPlannerErrorV1, plan_ssa_v1};

fn assertion(message: SemanticAssertMessageV1, cleanup: bool) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Assert {
        condition: SemanticOperandV1::Move(test_scalar_place(2)),
        expected: true,
        message,
        target: test_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
        unwind: if cleanup {
            SemanticUnwindActionV1::Cleanup(test_edge(SemanticEdgeRoleV1::AssertUnwind, 1))
        } else {
            SemanticUnwindActionV1::Unreachable
        },
    }
}

#[test]
fn diagnostic_boundary_follows_condition_use_and_kill() {
    let message = SemanticAssertMessageV1::BoundsCheck {
        length: SemanticOperandV1::Move(test_scalar_place(3)),
        index: SemanticOperandV1::Copy(test_scalar_place(2)),
    };
    let mut events = vec![];
    let mut trace = Trace::default();
    emit_terminator_events_v1(
        &assertion(message, false),
        None,
        Site::Terminator { block: 0 },
        &mut events,
        &mut trace,
    )
    .unwrap();
    assert_eq!(
        trace.failure_boundaries,
        [(Site::Terminator { block: 0 }, 2)]
    );
    assert_eq!(events, [used(2), killed(2), used(3), killed(3), used(2)]);
}

#[test]
fn empty_and_constant_messages_have_explicit_boundaries() {
    for message in [
        SemanticAssertMessageV1::NullPointerDereference,
        SemanticAssertMessageV1::BoundsCheck {
            length: SemanticOperandV1::Constant(constant()),
            index: SemanticOperandV1::Constant(constant()),
        },
    ] {
        let mut events = vec![];
        let mut trace = Trace::default();
        emit_terminator_events_v1(
            &assertion(message, false),
            None,
            Site::Terminator { block: 0 },
            &mut events,
            &mut trace,
        )
        .unwrap();
        assert_eq!(
            trace.failure_boundaries,
            [(Site::Terminator { block: 0 }, 2)]
        );
        assert_eq!(events, [used(2), killed(2)]);
    }
}

#[test]
fn cleanup_has_no_nonreturning_failure_admission() {
    let mut events = vec![];
    let mut trace = Trace::default();
    emit_terminator_events_v1(
        &assertion(
            SemanticAssertMessageV1::DivisionByZero(SemanticOperandV1::Move(test_scalar_place(3))),
            true,
        ),
        None,
        Site::Terminator { block: 0 },
        &mut events,
        &mut trace,
    )
    .unwrap();
    assert!(trace.failure_boundaries.is_empty());
    assert_eq!(events, [used(2), killed(2), used(3), killed(3)]);
}

fn prepared(hostile: bool) -> SsaConstructionInputV1 {
    let function = test_function(vec![
        test_block(
            90,
            vec![
                test_assign(2, SemanticOperandV1::Constant(constant())),
                test_assign(3, SemanticOperandV1::Constant(constant())),
            ],
            SemanticTerminatorKindV1::Assert {
                condition: SemanticOperandV1::Constant(constant()),
                expected: true,
                message: SemanticAssertMessageV1::BoundsCheck {
                    length: SemanticOperandV1::Move(test_scalar_place(3)),
                    index: SemanticOperandV1::Copy(test_scalar_place(if hostile { 3 } else { 2 })),
                },
                target: test_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        test_block(
            91,
            vec![test_assign(
                2,
                SemanticOperandV1::Copy(test_scalar_place(3)),
            )],
            SemanticTerminatorKindV1::Return,
        ),
    ]);
    let mut trace = Trace::default();
    let (input, _, _) = semantic_function_ssa_input_with_observer_v1(
        &function,
        None,
        &[],
        &BTreeSet::new(),
        &mut trace,
    )
    .unwrap();
    assert_eq!(input.blocks()[0].terminal_failure_start(), Some(2));
    assert_eq!(
        trace.failure_boundaries,
        [(Site::Terminator { block: 0 }, 2)]
    );
    input
}

#[test]
fn prepared_adapter_retains_ordered_failure_events_but_preserves_success_use() {
    let input = prepared(false);
    assert_eq!(
        input.blocks()[0].events(),
        [defined(2), defined(3), used(3), killed(3), used(2)]
    );
    let plan = plan_ssa_v1(&input).unwrap();
    assert_eq!(
        plan.resolved_event(SsaBlockIdV1::new(0), 2),
        plan.resolved_event(SsaBlockIdV1::new(1), 0)
    );
}

#[test]
fn prepared_adapter_rejects_diagnostic_move_then_use_at_original_event() {
    assert!(matches!(plan_ssa_v1(&prepared(true)),
        Err(SsaPlannerErrorV1::UndefinedAtUse { block, event: 4, variable })
            if block == SsaBlockIdV1::new(0) && variable == SsaVariableIdV1::new(3)));
}

#[test]
fn input_boundary_header_has_an_independent_exact_storage_cut() {
    let function = test_function(vec![test_block(
        97,
        vec![],
        SemanticTerminatorKindV1::Return,
    )]);
    let input = SsaConstructionInputV1::new(
        SsaBlockIdV1::new(0),
        4,
        vec![true; 4],
        vec![SsaVariableIdV1::new(1)],
        vec![SsaBlockInputV1::new(vec![], vec![])],
    );
    let resources = semantic_ssa_auxiliary_resources_v1(&function, &input).unwrap();
    // Four local envelopes (8 words each), one existing block envelope (12),
    // plus one owned and four temporary boundary slots; no statements/events/edges.
    let required = 4 * 8
        + 12
        + (5 * std::mem::size_of::<Option<usize>>()).div_ceil(std::mem::size_of::<usize>());
    assert_eq!(resources.storage_words, required);
    assert_eq!(resources.work_units, 4 * 8 + 12 + 8);
    for limit in [required, required - 1] {
        let limits = SsaPlannerLimitsV1::try_new(4, 1, 0, 0, 0, 0, limit, 52).unwrap();
        let result = enforce_function_resource_limit_v1(
            SemanticFunctionIdV1::from_index(0),
            resources,
            ProductionSemanticSsaLimitsV1::new(limits),
        );
        if limit == required {
            result.unwrap();
        } else {
            assert!(matches!(result,
            Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
                resource: SsaPlannerResourceV1::StorageWords, required: actual, limit: actual_limit, ..
            }) if actual == required && actual_limit == required - 1));
        }
    }
}

fn projected_assertion(index: SemanticOperandV1) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Assert {
        condition: SemanticOperandV1::Constant(constant()),
        expected: true,
        message: SemanticAssertMessageV1::BoundsCheck {
            length: SemanticOperandV1::Move(test_place(1, Some(0))),
            index,
        },
        target: test_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
        unwind: SemanticUnwindActionV1::Unreachable,
    }
}

#[test]
fn failure_projected_move_preserves_disjoint_diagnostic_and_success_fields() {
    let function = test_function(vec![
        test_block(
            98,
            vec![],
            projected_assertion(SemanticOperandV1::Copy(test_place(1, Some(1)))),
        ),
        test_block(
            99,
            vec![test_assign(
                2,
                SemanticOperandV1::Copy(test_place(1, Some(0))),
            )],
            SemanticTerminatorKindV1::Return,
        ),
    ]);
    let plan = plan_test_function(&function, &test_types(false)).unwrap();
    assert_eq!(plan.partial_move_certificate().projected_moves(), 1);
}

#[test]
fn failure_projected_move_rejects_overlapping_diagnostic_read() {
    let function = test_function(vec![
        test_block(
            98,
            vec![],
            projected_assertion(SemanticOperandV1::Copy(test_place(1, Some(0)))),
        ),
        test_block(99, vec![], SemanticTerminatorKindV1::Return),
    ]);
    assert!(matches!(
        plan_test_function(&function, &test_types(false)),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            local: 1,
            block: 0,
            statement: None,
            violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            ..
        })
    ));
}

#[test]
fn failure_move_rejects_later_dynamic_projection_index_use() {
    let mut types = test_types(false);
    types[0] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(40)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(41)),
        SemanticTypeLayoutV1::new(Some(32), 4).unwrap(),
        SemanticTypeShapeV1::Array {
            element: SemanticTypeIdV1::from_index(1),
            length: 8,
        },
    );
    let scalar = SemanticTypeIdV1::from_index(1);
    let indexed = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                scalar,
            )
            .unwrap(),
        ],
        scalar,
    )
    .unwrap();
    let function = test_function(vec![
        test_block(
            100,
            vec![test_assign(2, SemanticOperandV1::Constant(constant()))],
            SemanticTerminatorKindV1::Assert {
                condition: SemanticOperandV1::Constant(constant()),
                expected: true,
                message: SemanticAssertMessageV1::BoundsCheck {
                    length: SemanticOperandV1::Move(test_scalar_place(2)),
                    index: SemanticOperandV1::Copy(indexed),
                },
                target: test_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        test_block(101, vec![], SemanticTerminatorKindV1::Return),
    ]);
    let result = plan_test_function(&function, &types);
    assert!(
        matches!(result,
        Err(ProductionSemanticSsaErrorV1::Planner {
            error: SsaPlannerErrorV1::UndefinedAtUse { block, variable, .. }, ..
        }) if block == SsaBlockIdV1::new(0) && variable == SsaVariableIdV1::new(2)),
        "unexpected moved diagnostic index result: {result:?}"
    );
}
