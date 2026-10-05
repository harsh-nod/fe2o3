use super::*;

#[test]
fn explicit_store_reinitializes_the_exact_removed_local_or_field() {
    for whole in [false, true] {
        let destination = if whole {
            test_scalar_place(2)
        } else {
            test_place(1, Some(0))
        };
        let function = straight(vec![
            test_assign(2, SemanticOperandV1::Copy(test_place(1, Some(1)))),
            remove(destination.clone()),
            test_store(
                destination.clone(),
                SemanticOperandV1::Copy(test_place(1, Some(1))),
            ),
            test_assign(3, SemanticOperandV1::Copy(destination)),
        ]);
        let plan = plan_test_function(&function, &test_types(false)).unwrap();
        assert!(
            !plan
                .plan()
                .promoted_variables()
                .contains(&SsaVariableIdV1::new(if whole { 2 } else { 1 }))
        );
        assert_eq!(plan.partial_move_certificate().projected_moves(), 0);
        assert_eq!(plan.partial_move_certificate().state_entries(), 1);
    }
}

#[test]
fn explicit_store_checks_its_value_before_repairing_the_destination() {
    for moving in [false, true] {
        let destination = test_place(1, Some(0));
        let value = if moving {
            SemanticOperandV1::Move(destination.clone())
        } else {
            SemanticOperandV1::Copy(destination.clone())
        };
        let function = straight(vec![
            remove(destination.clone()),
            test_store(destination, value),
        ]);
        assert!(matches!(
            plan_test_function(&function, &test_types(false)),
            Err(ProductionSemanticSsaErrorV1::PartialMove {
                block: 0,
                statement: Some(1),
                local: 1,
                violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                ..
            })
        ));
    }
}

#[test]
fn explicit_store_repair_keeps_other_removed_siblings_unavailable() {
    for replacement in [0, 1] {
        let function = straight(vec![
            test_assign(2, SemanticOperandV1::Copy(test_place(1, Some(1)))),
            remove(test_place(1, Some(0))),
            test_store(
                test_place(1, Some(replacement)),
                SemanticOperandV1::Copy(test_scalar_place(2)),
            ),
            test_assign(3, SemanticOperandV1::Copy(test_place(1, Some(0)))),
        ]);
        let result = plan_test_function(&function, &test_types(false));
        if replacement == 0 {
            assert!(result.is_ok(), "{result:?}");
        } else {
            assert!(matches!(
                result,
                Err(ProductionSemanticSsaErrorV1::PartialMove {
                    block: 0,
                    statement: Some(3),
                    local: 1,
                    violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                    ..
                })
            ));
        }
    }
}

#[test]
fn explicit_store_move_reads_then_invalidates_then_initializes_only_the_destination() {
    for read_source in [false, true] {
        let function = straight(vec![
            remove(test_place(1, Some(1))),
            test_store(
                test_place(1, Some(1)),
                SemanticOperandV1::Move(test_place(1, Some(0))),
            ),
            test_assign(
                2,
                SemanticOperandV1::Copy(test_place(1, Some(if read_source { 0 } else { 1 }))),
            ),
        ]);
        let result = plan_test_function(&function, &test_types(false));
        if read_source {
            assert!(matches!(
                result,
                Err(ProductionSemanticSsaErrorV1::PartialMove {
                    block: 0,
                    statement: Some(2),
                    local: 1,
                    violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                    ..
                })
            ));
        } else {
            assert!(result.is_ok(), "{result:?}");
        }
    }
}

#[test]
fn explicit_store_reinitialization_must_hold_on_every_predecessor() {
    for both in [false, true] {
        for reverse in [false, true] {
            let restore = || {
                test_store(
                    test_place(1, Some(0)),
                    SemanticOperandV1::Copy(test_scalar_place(2)),
                )
            };
            let function = test_function(vec![
                test_block(
                    190,
                    vec![
                        test_assign(2, SemanticOperandV1::Copy(test_place(1, Some(1)))),
                        remove(test_place(1, Some(0))),
                    ],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: SemanticOperandV1::Copy(test_scalar_place(2)),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                test_edge(
                                    SemanticEdgeRoleV1::SwitchValue,
                                    if reverse { 2 } else { 1 },
                                ),
                            )],
                            test_edge(
                                SemanticEdgeRoleV1::SwitchOtherwise,
                                if reverse { 1 } else { 2 },
                            ),
                        )
                        .unwrap(),
                    },
                ),
                test_block(
                    191,
                    vec![restore()],
                    SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 3)),
                ),
                test_block(
                    192,
                    if both { vec![restore()] } else { vec![] },
                    SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 3)),
                ),
                test_block(
                    193,
                    vec![test_assign(
                        3,
                        SemanticOperandV1::Copy(test_place(1, Some(0))),
                    )],
                    SemanticTerminatorKindV1::Return,
                ),
            ]);
            let result = plan_test_function(&function, &test_types(false));
            if both {
                assert!(result.is_ok(), "{result:?}");
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionSemanticSsaErrorV1::PartialMove {
                        block: 3,
                        statement: Some(0),
                        local: 1,
                        violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                        ..
                    })
                ));
            }
        }
    }
}

#[test]
fn explicit_store_repair_has_independent_exact_and_one_short_work() {
    let function = straight(vec![
        test_assign(2, SemanticOperandV1::Copy(test_place(1, Some(1)))),
        remove(test_place(1, Some(0))),
        test_store(
            test_place(1, Some(0)),
            SemanticOperandV1::Copy(test_scalar_place(2)),
        ),
        test_assign(3, SemanticOperandV1::Copy(test_place(1, Some(0)))),
    ]);
    let types = test_types(false);
    let baseline = plan_test_function(&function, &types).unwrap();
    // One block visit, one parent-prefix probe, and one descendant removal.
    assert_eq!(baseline.partial_move_certificate().work_units(), 3);
    let work = baseline.resources().work_units() + baseline.auxiliary_resources.work_units + 3;
    for short in [false, true] {
        let limits = SsaPlannerLimitsV1::default();
        let limits = SsaPlannerLimitsV1::try_new(
            limits.max_variables(),
            limits.max_blocks(),
            limits.max_edges(),
            limits.max_events(),
            limits.max_edge_definitions(),
            limits.max_output_items(),
            limits.max_storage_words(),
            work - usize::from(short),
        )
        .unwrap();
        let result = plan_semantic_function_ssa_with_module_v1(
            SemanticFunctionIdV1::from_index(0),
            &function,
            &types,
            &[],
            ProductionSemanticSsaLimitsV1::new(limits),
        );
        if short {
            assert!(
                matches!(result, Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
                resource: SsaPlannerResourceV1::WorkUnits, required, limit, ..
            }) if required == work && limit + 1 == required)
            );
        } else {
            assert_eq!(result.unwrap(), baseline);
        }
    }
}

fn remove(place: SemanticPlaceV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Deinitialize(place),
    )
}

fn straight(statements: Vec<SemanticStatementV1>) -> SemanticFunctionDeclV1 {
    test_function(vec![test_block(
        190,
        statements,
        SemanticTerminatorKindV1::Return,
    )])
}

fn sibling_function() -> SemanticFunctionDeclV1 {
    straight(vec![
        remove(test_place(1, Some(0))),
        test_assign(2, SemanticOperandV1::Copy(test_place(1, Some(1)))),
    ])
}

#[test]
fn deinitialize_without_move_operands_gets_a_real_field_certificate_and_stays_promoted() {
    let function = sibling_function();
    let plan = plan_test_function(&function, &test_types(false)).unwrap();
    assert!(
        plan.plan()
            .promoted_variables()
            .contains(&SsaVariableIdV1::new(1))
    );
    assert_eq!(plan.partial_move_certificate().projected_moves(), 0);
    // One block visit plus the two prefix probes and descendant probe for
    // the surviving sibling. One removed field is retained, not a root kill.
    assert_eq!(plan.partial_move_certificate().work_units(), 4);
    assert_eq!(plan.partial_move_certificate().state_entries(), 1);
}

#[test]
fn removed_field_and_whole_value_reads_refuse_without_any_move_operand() {
    for read in [test_place(1, Some(0)), test_place(1, None)] {
        let function = straight(vec![
            remove(test_place(1, Some(0))),
            test_assign(2, SemanticOperandV1::Copy(read)),
        ]);
        assert!(matches!(
            plan_test_function(&function, &test_types(false)),
            Err(ProductionSemanticSsaErrorV1::PartialMove {
                block: 0,
                statement: Some(1),
                local: 1,
                violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                ..
            })
        ));
    }
    let function = straight(vec![
        remove(test_place(1, None)),
        test_assign(2, SemanticOperandV1::Copy(test_place(1, Some(1)))),
    ]);
    assert!(matches!(
        plan_test_function(&function, &test_types(false)),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            statement: Some(1),
            violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            ..
        })
    ));
}

#[test]
fn only_exact_reinitialization_repairs_a_removed_field() {
    for replacement in [0, 1] {
        let function = straight(vec![
            test_assign(2, SemanticOperandV1::Copy(test_place(1, Some(1)))),
            remove(test_place(1, Some(0))),
            test_assign_to(
                test_place(1, Some(replacement)),
                SemanticOperandV1::Copy(test_scalar_place(2)),
            ),
            test_assign(3, SemanticOperandV1::Copy(test_place(1, None))),
        ]);
        let result = plan_test_function(&function, &test_types(false));
        if replacement == 0 {
            assert_eq!(
                result.unwrap().partial_move_certificate().projected_moves(),
                0
            );
        } else {
            assert!(matches!(
                result,
                Err(ProductionSemanticSsaErrorV1::PartialMove {
                    statement: Some(3),
                    violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                    ..
                })
            ));
        }
    }
}

#[test]
fn repeated_removal_keeps_the_existing_readability_precondition() {
    for place in [test_place(1, Some(0)), test_place(1, None)] {
        let function = straight(vec![remove(place.clone()), remove(place)]);
        assert!(matches!(
            plan_test_function(&function, &test_types(false)),
            Err(ProductionSemanticSsaErrorV1::PartialMove {
                block: 0,
                statement: Some(1),
                local: 1,
                violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                ..
            })
        ));
    }
}

#[test]
fn deinitialize_joins_keep_siblings_and_refuse_a_maybe_removed_field_in_both_orders() {
    for reverse in [false, true] {
        for both in [false, true] {
            for read_removed in [false, true] {
                let function = test_function(vec![
                    test_block(
                        190,
                        vec![],
                        SemanticTerminatorKindV1::SwitchInt {
                            discriminant: SemanticOperandV1::Copy(test_place(1, Some(1))),
                            targets: SemanticSwitchTargetsV1::new(
                                vec![SemanticSwitchTargetV1::new(
                                    0,
                                    test_edge(
                                        SemanticEdgeRoleV1::SwitchValue,
                                        if reverse { 2 } else { 1 },
                                    ),
                                )],
                                test_edge(
                                    SemanticEdgeRoleV1::SwitchOtherwise,
                                    if reverse { 1 } else { 2 },
                                ),
                            )
                            .unwrap(),
                        },
                    ),
                    test_block(
                        191,
                        vec![remove(test_place(1, Some(0)))],
                        SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 3)),
                    ),
                    test_block(
                        192,
                        if both {
                            vec![remove(test_place(1, Some(0)))]
                        } else {
                            vec![]
                        },
                        SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 3)),
                    ),
                    test_block(
                        193,
                        vec![test_assign(
                            2,
                            SemanticOperandV1::Copy(test_place(
                                1,
                                Some(if read_removed { 0 } else { 1 }),
                            )),
                        )],
                        SemanticTerminatorKindV1::Return,
                    ),
                ]);
                let result = plan_test_function(&function, &test_types(false));
                if read_removed {
                    assert!(matches!(
                        result,
                        Err(ProductionSemanticSsaErrorV1::PartialMove {
                            block: 3,
                            statement: Some(0),
                            violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                            ..
                        })
                    ));
                } else {
                    let plan = result.unwrap();
                    assert!(
                        plan.plan()
                            .promoted_variables()
                            .contains(&SsaVariableIdV1::new(1))
                    );
                    assert!(plan.retained_cross_edge_variables().is_empty());
                }
            }
        }
    }
}

#[test]
fn deinitialize_union_or_missing_context_cannot_create_disjoint_sibling_proofs() {
    let function = sibling_function();
    assert!(matches!(
        plan_test_function(&function, &test_types(true)),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            violation: SemanticPartialMoveViolationV1::UnionField,
            ..
        })
    ));
    assert!(matches!(
        plan_semantic_function_ssa_v1(
            SemanticFunctionIdV1::from_index(0),
            &function,
            ProductionSemanticSsaLimitsV1::default(),
        ),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            violation: SemanticPartialMoveViolationV1::MissingTypeContext,
            ..
        })
    ));
}

#[test]
fn deinitialize_does_not_remove_address_or_drop_observability() {
    let borrowed = straight(vec![remove(test_place(1, Some(0))), test_borrow(2, 1)]);
    assert!(!source_is_promotable(&borrowed, &[]));
    let dropped = test_function(vec![
        test_block(
            190,
            vec![remove(test_place(1, Some(0)))],
            SemanticTerminatorKindV1::Drop {
                place: test_place(1, Some(1)),
                drop_glue: SemanticFunctionIdV1::from_index(0),
                target: test_edge(SemanticEdgeRoleV1::DropReturn, 1),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        test_block(191, vec![], SemanticTerminatorKindV1::Return),
    ]);
    assert!(!source_is_promotable(&dropped, &[]));
    for place in [
        test_constant_index_place(1, 0, 2),
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                    SemanticTypeIdV1::from_index(1),
                )
                .unwrap(),
            ],
            SemanticTypeIdV1::from_index(1),
        )
        .unwrap(),
    ] {
        assert!(!source_is_promotable(&straight(vec![remove(place)]), &[]));
    }
}

#[test]
fn deinitialize_certificate_exact_and_one_short_resources_keep_the_first_denial() {
    let function = sibling_function();
    let types = test_types(false);
    let baseline = plan_test_function(&function, &types).unwrap();
    let work = baseline.resources().work_units() + baseline.auxiliary_resources.work_units + 4;
    let storage =
        baseline.resources().storage_words() + baseline.auxiliary_resources.storage_words + 1;
    for short in [
        None,
        Some(SsaPlannerResourceV1::WorkUnits),
        Some(SsaPlannerResourceV1::StorageWords),
    ] {
        let limits = SsaPlannerLimitsV1::default();
        let limits = SsaPlannerLimitsV1::try_new(
            limits.max_variables(),
            limits.max_blocks(),
            limits.max_edges(),
            limits.max_events(),
            limits.max_edge_definitions(),
            limits.max_output_items(),
            storage - usize::from(short == Some(SsaPlannerResourceV1::StorageWords)),
            work - usize::from(short == Some(SsaPlannerResourceV1::WorkUnits)),
        )
        .unwrap();
        let result = plan_semantic_function_ssa_with_module_v1(
            SemanticFunctionIdV1::from_index(0),
            &function,
            &types,
            &[],
            ProductionSemanticSsaLimitsV1::new(limits),
        );
        match short {
            None => assert_eq!(result.unwrap(), baseline),
            Some(expected) => assert!(matches!(result,
                Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit { resource, required, limit, .. })
                if resource == expected && required == limit + 1)),
        }
    }
}
