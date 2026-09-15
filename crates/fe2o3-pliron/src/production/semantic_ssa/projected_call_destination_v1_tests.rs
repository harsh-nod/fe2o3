use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticMutabilityV1, SemanticPointerKindV1, SemanticPointerMetadataV1, SemanticPointerTypeV1,
};

mod owner_replay_tests {
    include!("projected_call_owner_replay_v1_tests.rs");
}

fn call_index(base: u32, index: u32) -> SemanticPlaceV1 {
    let scalar = SemanticTypeIdV1::from_index(1);
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(base),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(index)),
                scalar,
            )
            .unwrap(),
        ],
        scalar,
    )
    .unwrap()
}

fn call_array_types() -> Vec<SemanticTypeDeclV1> {
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
    types
}

fn returning_call(
    destination: SemanticPlaceV1,
    args: Vec<SemanticOperandV1>,
    target: u32,
) -> SemanticTerminatorKindV1 {
    test_call(
        0,
        args,
        Some(SemanticCallDestinationV1::new(
            destination,
            test_edge(SemanticEdgeRoleV1::CallReturn, target),
        )),
    )
}

fn call_events(call: &SemanticTerminatorKindV1) -> (Vec<SsaEventV1>, Trace) {
    let mut events = Vec::new();
    let mut trace = Trace::default();
    emit_terminator_events_v1(
        call,
        None,
        Site::Terminator { block: 0 },
        &mut events,
        &mut trace,
    )
    .unwrap();
    (events, trace)
}

#[test]
fn call_address_indices_precede_argument_move_without_reading_array_contents() {
    let call = returning_call(
        call_index(1, 2),
        vec![SemanticOperandV1::Move(test_scalar_place(2))],
        1,
    );
    let (events, trace) = call_events(&call);
    assert_eq!(events, [used(2), used(2), killed(2)]);
    let site = Site::Terminator { block: 0 };
    assert_eq!(
        trace.events,
        [
            row(
                site,
                Operand::CallDestinationAddress,
                EventRole::ProjectionIndexUse(0),
                0,
                used(2)
            ),
            row(
                site,
                Operand::CallArgument(0),
                EventRole::BaseUse,
                1,
                used(2)
            ),
            row(
                site,
                Operand::CallArgument(0),
                EventRole::MoveKill,
                2,
                killed(2)
            ),
        ]
    );
    assert_eq!(
        trace
            .visits
            .iter()
            .filter(|(visit, _)| *visit == Visit::Projection)
            .count(),
        2
    );
    assert_eq!(
        trace
            .visits
            .iter()
            .filter(|(visit, _)| *visit == Visit::Place)
            .count(),
        2
    );
    assert!(!events.iter().any(|event| event.variable() == variable(1)));
}

#[test]
fn pointer_and_repeated_index_address_uses_keep_original_projection_ordinals() {
    let scalar = SemanticTypeIdV1::from_index(1);
    let destination = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, scalar).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), scalar).unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(3)),
                scalar,
            )
            .unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(3)),
                scalar,
            )
            .unwrap(),
        ],
        scalar,
    )
    .unwrap();
    let call = returning_call(
        destination,
        vec![
            SemanticOperandV1::Move(test_scalar_place(1)),
            SemanticOperandV1::Move(test_scalar_place(3)),
        ],
        1,
    );
    let (events, trace) = call_events(&call);
    assert_eq!(
        events,
        [
            used(1),
            used(3),
            used(3),
            used(1),
            killed(1),
            used(3),
            killed(3)
        ]
    );
    let site = Site::Terminator { block: 0 };
    assert_eq!(
        trace.events[..3],
        [
            row(
                site,
                Operand::CallDestinationAddress,
                EventRole::BaseUse,
                0,
                used(1)
            ),
            row(
                site,
                Operand::CallDestinationAddress,
                EventRole::ProjectionIndexUse(2),
                1,
                used(3)
            ),
            row(
                site,
                Operand::CallDestinationAddress,
                EventRole::ProjectionIndexUse(3),
                2,
                used(3)
            ),
        ]
    );
    assert_eq!(
        trace
            .visits
            .iter()
            .filter(|(visit, _)| *visit == Visit::Projection)
            .count(),
        8
    );
    assert_eq!(
        trace
            .visits
            .iter()
            .filter(|(visit, _)| *visit == Visit::Place)
            .count(),
        3
    );
}

#[test]
fn fixed_projected_and_unprojected_calls_do_not_invent_content_uses() {
    for destination in [
        test_scalar_place(2),
        test_place(1, Some(0)),
        test_constant_index_place(1, 0, 8),
    ] {
        let (events, trace) = call_events(&returning_call(
            destination,
            vec![SemanticOperandV1::Constant(constant())],
            1,
        ));
        assert!(events.is_empty());
        assert_eq!(trace.constants[0].next_event, 0);
    }
    let (events, trace) = call_events(&test_call(
        0,
        vec![SemanticOperandV1::Move(test_scalar_place(2))],
        None,
    ));
    assert_eq!(events, [used(2), killed(2)]);
    assert!(
        trace
            .events
            .iter()
            .all(|event| event.operand == Operand::CallArgument(0))
    );
}

fn predecessor_call_source() -> SemanticFunctionDeclV1 {
    test_function(vec![
        test_block(
            180,
            vec![test_assign(2, SemanticOperandV1::Constant(constant()))],
            SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        test_block(181, vec![], returning_call(call_index(1, 2), vec![], 2)),
        test_block(182, vec![], SemanticTerminatorKindV1::Return),
    ])
}

#[test]
fn destination_only_index_resolves_the_actual_predecessor_definition() {
    let function = predecessor_call_source();
    let planned = plan_test_function(&function, &call_array_types()).unwrap();
    let plan = planned.plan();
    assert_eq!(plan.resources().input_events(), 2);
    assert_eq!(
        plan.resolved_events(SsaBlockIdV1::new(0)),
        Some(
            [(
                0,
                SsaResolvedEventV1::Define {
                    variable: variable(2),
                    value: SsaValueV1::Definition(SsaDefinitionIdV1::new(0))
                },
            )]
            .as_slice()
        )
    );
    assert_eq!(
        plan.resolved_events(SsaBlockIdV1::new(1)),
        Some(
            [(
                0,
                SsaResolvedEventV1::Use {
                    variable: variable(2),
                    value: SsaValueV1::Definition(SsaDefinitionIdV1::new(0))
                },
            )]
            .as_slice()
        )
    );
    assert_eq!(
        plan.live_in(SsaBlockIdV1::new(1)),
        Some([variable(2)].as_slice())
    );
    assert!(
        plan.edge_definitions(SsaEdgeIdV1::new(SsaBlockIdV1::new(1), 0))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn destination_only_index_produces_a_real_two_predecessor_phi() {
    let function = test_function(vec![
        test_block(
            183,
            vec![],
            SemanticTerminatorKindV1::FalseEdge {
                real_target: test_edge(SemanticEdgeRoleV1::FalseEdgeReal, 1),
                imaginary_target: test_edge(SemanticEdgeRoleV1::FalseEdgeImaginary, 2),
            },
        ),
        test_block(
            184,
            vec![test_assign(2, SemanticOperandV1::Constant(constant()))],
            SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
        test_block(
            185,
            vec![test_assign(2, SemanticOperandV1::Constant(constant()))],
            SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
        test_block(186, vec![], returning_call(call_index(1, 2), vec![], 4)),
        test_block(187, vec![], SemanticTerminatorKindV1::Return),
    ]);
    let planned = plan_test_function(&function, &call_array_types()).unwrap();
    let plan = planned.plan();
    assert_eq!(
        plan.merge_variables(SsaBlockIdV1::new(3)),
        Some([variable(2)].as_slice())
    );
    assert_eq!(
        plan.resolved_events(SsaBlockIdV1::new(3)),
        Some(
            [(
                0,
                SsaResolvedEventV1::Use {
                    variable: variable(2),
                    value: SsaValueV1::BlockArgument {
                        block: SsaBlockIdV1::new(3),
                        variable: variable(2),
                    }
                },
            )]
            .as_slice()
        )
    );
    for (block, definition) in [(1, 0), (2, 1)] {
        assert_eq!(
            plan.edge_arguments(SsaEdgeIdV1::new(SsaBlockIdV1::new(block), 0)),
            Some(
                [SsaArgumentV1::new(
                    variable(2),
                    SsaValueV1::Definition(SsaDefinitionIdV1::new(definition))
                ),]
                .as_slice()
            )
        );
    }
}

#[test]
fn undefined_and_killed_address_inputs_are_rejected_at_the_new_use() {
    for (statements, destination, expected_local, expected_event) in [
        (vec![], call_index(1, 2), 2, 0),
        (
            vec![
                test_assign(2, SemanticOperandV1::Constant(constant())),
                test_storage_dead(2),
            ],
            call_index(1, 2),
            2,
            2,
        ),
        (vec![], test_dereference_place(2, 1), 2, 0),
        (
            vec![test_storage_dead(1)],
            test_dereference_place(1, 1),
            1,
            1,
        ),
    ] {
        let function = test_function(vec![
            test_block(188, statements, returning_call(destination, vec![], 1)),
            test_block(189, vec![], SemanticTerminatorKindV1::Return),
        ]);
        assert!(matches!(plan_test_function(&function, &call_array_types()),
            Err(ProductionSemanticSsaErrorV1::Planner { error: SsaPlannerErrorV1::UndefinedAtUse { block, event, variable }, .. })
            if block == SsaBlockIdV1::new(0) && event == expected_event && variable.get() == expected_local
        ));
    }
}

fn move_and_repair_array(repair: bool) -> Vec<SemanticStatementV1> {
    let mut statements = vec![test_assign(
        2,
        SemanticOperandV1::Move(test_constant_index_place(1, 0, 8)),
    )];
    if repair {
        statements.push(test_assign_to(
            test_constant_index_place(1, 0, 8),
            SemanticOperandV1::Copy(test_scalar_place(2)),
        ));
    }
    statements
}

#[test]
fn a_snapshotted_index_stays_killed_at_the_post_call_read() {
    let function = test_function(vec![
        test_block(
            209,
            move_and_repair_array(true),
            returning_call(
                call_index(1, 2),
                vec![SemanticOperandV1::Move(test_scalar_place(2))],
                1,
            ),
        ),
        test_block(
            210,
            vec![test_assign(
                3,
                SemanticOperandV1::Copy(test_scalar_place(2)),
            )],
            SemanticTerminatorKindV1::Return,
        ),
    ]);
    // The pre-call address Use is valid, but the argument Kill still reaches
    // the single-predecessor continuation. No call edge defines this index.
    assert!(matches!(
        plan_test_function(&function, &call_array_types()),
        Err(ProductionSemanticSsaErrorV1::Planner {
            error: SsaPlannerErrorV1::UndefinedAtUse {
                block,
                event: 0,
                variable: local,
            },
            ..
        }) if block == SsaBlockIdV1::new(1) && local == variable(2)
    ));
}

#[test]
fn partial_move_call_snapshots_index_before_argument_move_and_does_not_resurrect_it() {
    let function = test_function(vec![
        test_block(
            190,
            move_and_repair_array(true),
            returning_call(
                call_index(1, 2),
                vec![SemanticOperandV1::Move(test_scalar_place(2))],
                1,
            ),
        ),
        test_block(
            191,
            vec![test_assign(
                3,
                SemanticOperandV1::Copy(test_constant_index_place(1, 0, 8)),
            )],
            SemanticTerminatorKindV1::Return,
        ),
    ]);
    let planned = plan_test_function(&function, &call_array_types()).unwrap();
    assert_eq!(planned.partial_move_certificate().projected_moves(), 1);
    let events = planned
        .plan()
        .resolved_events(SsaBlockIdV1::new(0))
        .unwrap();
    assert!(
        matches!(events.last(), Some((_, SsaResolvedEventV1::Kill { variable: local, previous: Some(_) })) if *local == variable(2))
    );
    assert!(
        planned
            .plan()
            .edge_definitions(SsaEdgeIdV1::new(SsaBlockIdV1::new(0), 0))
            .unwrap()
            .is_empty()
    );
    assert!(
        !planned
            .plan()
            .live_in(SsaBlockIdV1::new(1))
            .unwrap()
            .contains(&variable(2))
    );
}

#[test]
fn dynamic_call_and_assignment_writes_cannot_repair_a_partial_move() {
    let called = test_function(vec![
        test_block(
            192,
            move_and_repair_array(false),
            returning_call(call_index(1, 2), vec![], 1),
        ),
        test_block(193, vec![], SemanticTerminatorKindV1::Return),
    ]);
    assert!(matches!(
        plan_test_function(&called, &call_array_types()),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            block: 0,
            statement: None,
            local: 1,
            violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            ..
        })
    ));
    let mut statements = move_and_repair_array(false);
    statements.push(test_assign_to(
        call_index(1, 2),
        SemanticOperandV1::Copy(test_scalar_place(2)),
    ));
    let assigned = test_function(vec![test_block(
        194,
        statements,
        SemanticTerminatorKindV1::Return,
    )]);
    assert!(matches!(
        plan_test_function(&assigned, &call_array_types()),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            block: 0,
            statement: Some(1),
            local: 1,
            violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            ..
        })
    ));
    let mut statements = move_and_repair_array(true);
    statements.push(test_assign_to(
        call_index(1, 2),
        SemanticOperandV1::Move(test_scalar_place(2)),
    ));
    let assigned = test_function(vec![test_block(
        195,
        statements,
        SemanticTerminatorKindV1::Return,
    )]);
    assert!(
        matches!(plan_test_function(&assigned, &call_array_types()), Err(ProductionSemanticSsaErrorV1::Planner {
        error: SsaPlannerErrorV1::UndefinedAtUse { variable: local, .. }, ..
    }) if local == variable(2))
    );
}

#[test]
fn fixed_call_write_repairs_only_the_successful_edge_even_when_targets_match() {
    for same_target in [false, true] {
        let unwind_target = if same_target { 1 } else { 2 };
        let mut blocks = vec![
            test_block(
                196,
                move_and_repair_array(false),
                test_call_with_unwind(
                    SemanticCallDestinationV1::new(
                        test_constant_index_place(1, 0, 8),
                        test_edge(SemanticEdgeRoleV1::CallReturn, 1),
                    ),
                    test_edge(SemanticEdgeRoleV1::CallUnwind, unwind_target),
                ),
            ),
            test_block(
                197,
                vec![test_assign(
                    3,
                    SemanticOperandV1::Copy(test_constant_index_place(1, 0, 8)),
                )],
                SemanticTerminatorKindV1::Return,
            ),
        ];
        if !same_target {
            blocks.push(test_block(
                198,
                vec![test_assign(
                    3,
                    SemanticOperandV1::Copy(test_constant_index_place(1, 0, 8)),
                )],
                SemanticTerminatorKindV1::Return,
            ));
        }
        let function = test_function(blocks);
        let failed_block = if same_target { 1 } else { 2 };
        assert!(
            matches!(plan_test_function(&function, &call_array_types()), Err(ProductionSemanticSsaErrorV1::PartialMove {
            block, statement: Some(0), local: 1, violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed, ..
        }) if block == failed_block)
        );
    }
    let function = test_function(vec![
        test_block(
            199,
            move_and_repair_array(false),
            returning_call(test_constant_index_place(1, 0, 8), vec![], 1),
        ),
        test_block(
            200,
            vec![test_assign(
                3,
                SemanticOperandV1::Copy(test_constant_index_place(1, 0, 8)),
            )],
            SemanticTerminatorKindV1::Return,
        ),
    ]);
    assert_eq!(
        plan_test_function(&function, &call_array_types())
            .unwrap()
            .partial_move_certificate()
            .projected_moves(),
        1
    );
}

fn pointer_prefix_types() -> Vec<SemanticTypeDeclV1> {
    let mut types = test_types(false);
    types[0] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(40)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(41)),
        SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
        SemanticTypeShapeV1::Aggregate(
            fe2o3_mir_model::semantic_mir_v1::SemanticAggregateTypeV1::new(vec![
                SemanticTypeIdV1::from_index(2),
                SemanticTypeIdV1::from_index(1),
            ])
            .unwrap(),
        ),
    );
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(201)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(202)),
        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                SemanticTypeIdV1::from_index(1),
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    types
}

#[test]
fn partial_move_address_read_checks_the_pointer_prefix_not_the_final_pointee() {
    for moved_field in [0, 1] {
        let moved_type = if moved_field == 0 { 2 } else { 1 };
        let source_place = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Field(moved_field),
                    SemanticTypeIdV1::from_index(moved_type),
                )
                .unwrap(),
            ],
            SemanticTypeIdV1::from_index(moved_type),
        )
        .unwrap();
        let destination = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Field(0),
                    SemanticTypeIdV1::from_index(2),
                )
                .unwrap(),
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Dereference,
                    SemanticTypeIdV1::from_index(1),
                )
                .unwrap(),
            ],
            SemanticTypeIdV1::from_index(1),
        )
        .unwrap();
        let source = test_function(vec![
            test_block(
                203,
                vec![test_assign_to(
                    test_typed_place(2, moved_type),
                    SemanticOperandV1::Move(source_place),
                )],
                returning_call(destination, vec![], 1),
            ),
            test_block(204, vec![], SemanticTerminatorKindV1::Return),
        ]);
        let mut locals = source.locals().to_vec();
        locals[2] = test_local(62, moved_type, SemanticLocalRoleV1::Temporary);
        let function = SemanticFunctionDeclV1::new(
            source.identity(),
            source.role(),
            source.item_definition_identity(),
            source.monomorphization_identity(),
            source.generic_type_arguments_identity(),
            source.const_generic_arguments_identity(),
            source.source(),
            source.abi().clone(),
            locals,
            source.entry(),
            source.blocks().to_vec(),
        )
        .unwrap();
        let result = plan_test_function(&function, &pointer_prefix_types());
        if moved_field == 0 {
            assert!(matches!(
                result,
                Err(ProductionSemanticSsaErrorV1::PartialMove {
                    block: 0,
                    statement: None,
                    local: 1,
                    violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                    ..
                })
            ));
        } else {
            // The disjoint moved field does not block the address read, but
            // this nested destination remains unsupported by the write half.
            assert!(matches!(
                result,
                Err(ProductionSemanticSsaErrorV1::PartialMove {
                    block: 0,
                    statement: None,
                    local: 1,
                    violation: SemanticPartialMoveViolationV1::UnsupportedProjection,
                    ..
                })
            ));
        }
    }
}

fn event_limits(events: usize) -> ProductionSemanticSsaLimitsV1 {
    let defaults = SsaPlannerLimitsV1::default();
    ProductionSemanticSsaLimitsV1::new(
        SsaPlannerLimitsV1::try_new(
            defaults.max_variables(),
            defaults.max_blocks(),
            defaults.max_edges(),
            events,
            defaults.max_edge_definitions(),
            defaults.max_output_items(),
            defaults.max_storage_words(),
            defaults.max_work_units(),
        )
        .unwrap(),
    )
}

#[test]
fn source_adapter_event_limits_count_the_new_address_use_independently() {
    let function = predecessor_call_source();
    let types = call_array_types();
    // Exactly one index Define in block0 and one address Use in block1.
    let admitted = plan_semantic_function_ssa_with_module_v1(
        SemanticFunctionIdV1::from_index(0),
        &function,
        &types,
        &[],
        event_limits(2),
    )
    .unwrap();
    assert_eq!(admitted.resources().input_events(), 2);
    assert!(matches!(
        plan_semantic_function_ssa_with_module_v1(
            SemanticFunctionIdV1::from_index(0),
            &function,
            &types,
            &[],
            event_limits(1),
        ),
        Err(ProductionSemanticSsaErrorV1::Planner {
            error: SsaPlannerErrorV1::ResourceLimitExceeded {
                resource: SsaPlannerResourceV1::Events,
                required: 2,
                limit: 1,
            },
            ..
        })
    ));
}

#[test]
fn old_omitted_address_plan_fails_replay_against_the_actual_adapter_input() {
    let function = predecessor_call_source();
    let (input, _, _) =
        semantic_function_ssa_input_v1(&function, Some(&call_array_types()), &[], &BTreeSet::new());
    assert_eq!(input.blocks()[1].events(), [used(2)]);
    // Negative-only reproduction of the old omitted-use input, never admitted
    // as source evidence or installed into a production graph owner.
    let mut old_blocks = input.blocks().to_vec();
    old_blocks[1] = SsaBlockInputV1::new(vec![], old_blocks[1].edges().to_vec());
    let old_input = SsaConstructionInputV1::new(
        input.entry(),
        input.variable_count(),
        input.promotable().to_vec(),
        input.entry_definitions().to_vec(),
        old_blocks,
    );
    let old_plan = plan_ssa_with_limits_v1(&old_input, SsaPlannerLimitsV1::default()).unwrap();
    let actual = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
    assert_ne!(old_plan.identity(), actual.identity());
    assert!(matches!(
        old_plan.verify_replay(&input, SsaPlannerLimitsV1::default()),
        Err(SsaPlannerErrorV1::ReplayMismatch { .. })
    ));
    actual
        .verify_replay(&input, SsaPlannerLimitsV1::default())
        .unwrap();
}

#[test]
fn unprojected_call_keeps_its_legacy_input_and_plan_identity() {
    let function = test_function(vec![
        test_block(
            205,
            vec![],
            returning_call(
                test_scalar_place(2),
                vec![SemanticOperandV1::Copy(test_place(1, None))],
                1,
            ),
        ),
        test_block(206, vec![], SemanticTerminatorKindV1::Return),
    ]);
    let (input, _, _) = semantic_function_ssa_input_v1(&function, None, &[], &BTreeSet::new());
    let legacy = SsaConstructionInputV1::new(
        SsaBlockIdV1::new(0),
        4,
        vec![true; 4],
        vec![variable(1)],
        vec![
            SsaBlockInputV1::new(
                vec![used(1)],
                vec![SsaEdgeInputV1::new(
                    SsaEdgeRoleV1::new(semantic_edge_role_v1(SemanticEdgeRoleV1::CallReturn)),
                    SsaBlockIdV1::new(1),
                    vec![variable(2)],
                )],
            ),
            SsaBlockInputV1::new(vec![], vec![]),
        ],
    );
    assert_eq!(input, legacy);
    assert_eq!(
        plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default())
            .unwrap()
            .identity(),
        plan_ssa_with_limits_v1(&legacy, SsaPlannerLimitsV1::default())
            .unwrap()
            .identity(),
    );
}

#[test]
fn address_hooks_fail_before_classification_emission_and_argument_mutation() {
    let scalar = SemanticTypeIdV1::from_index(1);
    let destination = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, scalar).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), scalar).unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(3)),
                scalar,
            )
            .unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(3)),
                scalar,
            )
            .unwrap(),
        ],
        scalar,
    )
    .unwrap();
    let call = returning_call(
        destination,
        vec![SemanticOperandV1::Move(test_scalar_place(1))],
        1,
    );
    for (reject_hook, reject_event, expected_projections, retained_events) in [
        (Some(RejectedHook::Visit(Visit::Place)), None, 0, vec![]),
        (
            Some(RejectedHook::Visit(Visit::Projection)),
            None,
            1,
            vec![],
        ),
        (None, Some(0), 4, vec![]),
        (None, Some(1), 7, vec![used(1)]),
    ] {
        let mut events = Vec::new();
        let mut trace = Trace {
            reject_hook,
            reject_event,
            ..Trace::default()
        };
        let error = emit_terminator_events_v1(
            &call,
            None,
            Site::Terminator { block: 0 },
            &mut events,
            &mut trace,
        )
        .unwrap_err();
        assert_eq!(error, reject_event.unwrap_or(HOOK_DENIED));
        assert_eq!(events, retained_events);
        assert_eq!(
            trace
                .visits
                .iter()
                .filter(|(visit, _)| *visit == Visit::Projection)
                .count(),
            expected_projections
        );
        assert!(
            trace
                .events
                .iter()
                .all(|event| event.operand == Operand::CallDestinationAddress)
        );
        assert!(
            !trace
                .events
                .iter()
                .any(|event| matches!(event.event, SsaEventV1::Kill(_) | SsaEventV1::Define(_)))
        );
    }
}

#[test]
fn address_uses_are_before_both_same_target_edges_without_projected_definitions() {
    let return_edge = test_edge(SemanticEdgeRoleV1::CallReturn, 1);
    let unwind_edge = test_edge(SemanticEdgeRoleV1::CallUnwind, 1);
    let function = test_function(vec![
        test_block(
            207,
            vec![test_assign(2, SemanticOperandV1::Constant(constant()))],
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(0),
                    vec![SemanticOperandV1::Move(test_scalar_place(2))],
                    Some(SemanticCallDestinationV1::new(
                        call_index(1, 2),
                        return_edge,
                    )),
                    SemanticUnwindActionV1::Cleanup(unwind_edge),
                )
                .unwrap(),
            ),
        ),
        test_block(208, vec![], SemanticTerminatorKindV1::Return),
    ]);
    let mut trace = Trace::default();
    let (input, _, _) = semantic_function_ssa_input_with_observer_v1(
        &function,
        Some(&call_array_types()),
        &[],
        &BTreeSet::new(),
        &mut trace,
    )
    .unwrap();
    assert_eq!(
        input.blocks()[0].events(),
        [defined(2), used(2), used(2), killed(2)]
    );
    assert_eq!(trace.successors, [(0, 0, return_edge), (0, 1, unwind_edge)]);
    assert!(trace.edge_definitions.is_empty());
    assert!(
        input.blocks()[0]
            .edges()
            .iter()
            .all(|edge| edge.definitions().is_empty())
    );
    let plan = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
    for ordinal in [0, 1] {
        assert!(
            plan.edge_definitions(SsaEdgeIdV1::new(SsaBlockIdV1::new(0), ordinal))
                .unwrap()
                .is_empty()
        );
    }
}
