use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateTypeV1, SemanticMutabilityV1, SemanticPointerKindV1,
    SemanticPointerMetadataV1, SemanticPointerTypeV1,
};

const WRAPPER: u32 = 0;
const WORD: u32 = 1;
const POINTER: u32 = 2;
const INNER: u32 = 3;

fn types(nested: bool, union: bool) -> Vec<SemanticTypeDeclV1> {
    let mut types = test_types(false);
    let fields = SemanticAggregateTypeV1::new(vec![
        SemanticTypeIdV1::from_index(if nested { INNER } else { POINTER }),
        SemanticTypeIdV1::from_index(WORD),
    ])
    .unwrap();
    types[0] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(40)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(41)),
        SemanticTypeLayoutV1::new(Some(if nested { 24 } else { 16 }), 8).unwrap(),
        if union {
            SemanticTypeShapeV1::Union(fields)
        } else {
            SemanticTypeShapeV1::Aggregate(fields)
        },
    );
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(201)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(202)),
        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                SemanticTypeIdV1::from_index(WORD),
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(203)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(204)),
        SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![
                SemanticTypeIdV1::from_index(POINTER),
                SemanticTypeIdV1::from_index(WORD),
            ])
            .unwrap(),
        ),
    ));
    types
}

fn pointer(nested: bool) -> SemanticPlaceV1 {
    let mut projections = Vec::new();
    if nested {
        projections.push(
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Field(0),
                SemanticTypeIdV1::from_index(INNER),
            )
            .unwrap(),
        );
    }
    projections.push(
        SemanticProjectionV1::new(
            SemanticProjectionKindV1::Field(0),
            SemanticTypeIdV1::from_index(POINTER),
        )
        .unwrap(),
    );
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        projections,
        SemanticTypeIdV1::from_index(POINTER),
    )
    .unwrap()
}

fn indirect(nested: bool) -> SemanticPlaceV1 {
    let mut projections = pointer(nested).projections().to_vec();
    projections.push(
        SemanticProjectionV1::new(
            SemanticProjectionKindV1::Dereference,
            SemanticTypeIdV1::from_index(WORD),
        )
        .unwrap(),
    );
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        projections,
        SemanticTypeIdV1::from_index(WORD),
    )
    .unwrap()
}

fn whole(local: u32, ty: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        Vec::new(),
        SemanticTypeIdV1::from_index(ty),
    )
    .unwrap()
}

fn fixture(blocks: Vec<SemanticBasicBlockV1>) -> SemanticFunctionDeclV1 {
    let old = test_function(blocks);
    SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        old.source(),
        old.abi().clone(),
        vec![
            test_local(60, WORD, SemanticLocalRoleV1::Return),
            test_local(61, WRAPPER, SemanticLocalRoleV1::Argument(0)),
            test_local(62, WORD, SemanticLocalRoleV1::Temporary),
            test_local(63, WORD, SemanticLocalRoleV1::Temporary),
            test_local(64, POINTER, SemanticLocalRoleV1::Temporary),
            test_local(65, WRAPPER, SemanticLocalRoleV1::Temporary),
        ],
        old.entry(),
        old.blocks().to_vec(),
    )
    .unwrap()
}

fn straight(statements: Vec<SemanticStatementV1>) -> SemanticFunctionDeclV1 {
    fixture(vec![test_block(
        210,
        statements,
        SemanticTerminatorKindV1::Return,
    )])
}

fn write(nested: bool, explicit: bool) -> SemanticStatementV1 {
    let value = SemanticOperandV1::Copy(whole(2, WORD));
    if explicit {
        test_store(indirect(nested), value)
    } else {
        test_assign_to(indirect(nested), value)
    }
}

#[test]
fn projected_pointee_moves_preserve_the_local_pointer_and_actual_assignment_order() {
    for nested in [false, true] {
        for explicit in [false, true] {
            let function = straight(vec![
                test_assign(2, SemanticOperandV1::Move(indirect(nested))),
                write(nested, explicit),
                test_assign(3, SemanticOperandV1::Move(indirect(nested))),
                test_assign_to(whole(4, POINTER), SemanticOperandV1::Copy(pointer(nested))),
            ]);
            let plan = plan_test_function(&function, &types(nested, false)).unwrap();
            assert_eq!(plan.partial_move_certificate().projected_moves(), 2);
            assert_eq!(plan.partial_move_certificate().state_entries(), 0);
            assert!(plan.partial_move_certificate().work_units() > 0);
        }
    }
}

#[test]
fn moving_the_pointer_field_or_holder_still_forbids_reads_and_indirect_writes() {
    for nested in [false, true] {
        for whole_holder in [false, true] {
            for access in 0..3 {
                let initial = test_assign(2, SemanticOperandV1::Move(indirect(nested)));
                let removed = if whole_holder {
                    test_assign_to(
                        whole(5, WRAPPER),
                        SemanticOperandV1::Move(whole(1, WRAPPER)),
                    )
                } else {
                    test_assign_to(whole(4, POINTER), SemanticOperandV1::Move(pointer(nested)))
                };
                let expected_event = 5 + u32::from(access != 0);
                let access = match access {
                    0 => test_assign(3, SemanticOperandV1::Move(indirect(nested))),
                    1 => write(nested, false),
                    _ => write(nested, true),
                };
                let result = plan_test_function(
                    &straight(vec![initial, removed, access]),
                    &types(nested, false),
                );
                if whole_holder {
                    // Whole-local moves already kill the SSA definition. The
                    // planner refuses its exact next address read first.
                    assert!(
                        matches!(result, Err(ProductionSemanticSsaErrorV1::Planner {
                        function, error: SsaPlannerErrorV1::UndefinedAtUse { block, event, variable },
                    }) if function.index() == 0 && block == SsaBlockIdV1::new(0)
                        && event == expected_event && variable == SsaVariableIdV1::new(1)),
                        "nested={nested}, whole_holder={whole_holder}: {result:?}"
                    );
                } else {
                    assert!(
                        matches!(
                            result,
                            Err(ProductionSemanticSsaErrorV1::PartialMove {
                                block: 0,
                                statement: Some(2),
                                local: 1,
                                violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                                ..
                            })
                        ),
                        "nested={nested}, whole_holder={whole_holder}: {result:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn restoring_the_exact_pointer_field_restores_indirect_access_without_referent_authority() {
    for nested in [false, true] {
        let function = straight(vec![
            test_assign_to(whole(4, POINTER), SemanticOperandV1::Move(pointer(nested))),
            test_assign_to(pointer(nested), SemanticOperandV1::Move(whole(4, POINTER))),
            test_assign(2, SemanticOperandV1::Move(indirect(nested))),
            write(nested, false),
        ]);
        assert!(plan_test_function(&function, &types(nested, false)).is_ok());
    }
}

#[test]
fn indirect_assignment_keeps_disjoint_local_siblings_moved() {
    for explicit in [false, true] {
        let sibling = test_place(1, Some(1));
        let prefix = vec![
            test_assign(2, SemanticOperandV1::Move(sibling.clone())),
            write(false, explicit),
        ];
        assert!(plan_test_function(&straight(prefix.clone()), &types(false, false)).is_ok());
        let mut statements = prefix;
        statements.push(test_assign(3, SemanticOperandV1::Copy(sibling)));
        assert!(matches!(
            plan_test_function(&straight(statements), &types(false, false)),
            Err(ProductionSemanticSsaErrorV1::PartialMove {
                statement: Some(2),
                local: 1,
                violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                ..
            })
        ));
    }
}

#[test]
fn an_indirect_write_does_not_repair_a_maybe_moved_pointer_at_a_join() {
    for reverse in [false, true] {
        let function = fixture(vec![
            test_block(
                210,
                vec![test_assign(2, SemanticOperandV1::Copy(indirect(false)))],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(whole(2, WORD)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            test_edge(SemanticEdgeRoleV1::SwitchValue, if reverse { 2 } else { 1 }),
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
                211,
                vec![test_assign_to(
                    whole(4, POINTER),
                    SemanticOperandV1::Move(pointer(false)),
                )],
                SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            test_block(
                212,
                vec![],
                SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            test_block(
                213,
                vec![write(false, false)],
                SemanticTerminatorKindV1::Return,
            ),
        ]);
        assert!(matches!(
            plan_test_function(&function, &types(false, false)),
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

#[test]
fn union_missing_context_and_dynamic_prefixes_remain_closed() {
    let function = straight(vec![test_assign(
        2,
        SemanticOperandV1::Move(indirect(false)),
    )]);
    assert!(matches!(
        plan_test_function(&function, &types(false, true)),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            violation: SemanticPartialMoveViolationV1::UnionField,
            ..
        })
    ));
    assert!(matches!(
        plan_semantic_function_ssa_v1(
            SemanticFunctionIdV1::from_index(0),
            &function,
            ProductionSemanticSsaLimitsV1::default()
        ),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            violation: SemanticPartialMoveViolationV1::MissingTypeContext,
            ..
        })
    ));
    let dynamic = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                SemanticTypeIdV1::from_index(POINTER),
            )
            .unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Dereference,
                SemanticTypeIdV1::from_index(WORD),
            )
            .unwrap(),
        ],
        SemanticTypeIdV1::from_index(WORD),
    )
    .unwrap();
    let fixed = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::ConstantIndex {
                    offset: 0,
                    minimum_length: 2,
                    from_end: false,
                },
                SemanticTypeIdV1::from_index(POINTER),
            )
            .unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Dereference,
                SemanticTypeIdV1::from_index(WORD),
            )
            .unwrap(),
        ],
        SemanticTypeIdV1::from_index(WORD),
    )
    .unwrap();
    let mut array_types = types(false, false);
    array_types[0] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(40)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(41)),
        SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
        SemanticTypeShapeV1::Array {
            element: SemanticTypeIdV1::from_index(POINTER),
            length: 2,
        },
    );
    let baseline = straight(vec![test_assign(2, SemanticOperandV1::Move(fixed.clone()))]);
    assert!(plan_test_function(&baseline, &array_types).is_ok());
    let function = straight(vec![
        test_assign(2, SemanticOperandV1::Move(fixed)),
        test_assign(3, SemanticOperandV1::Move(dynamic)),
    ]);
    assert!(matches!(
        plan_test_function(&function, &array_types),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            violation: SemanticPartialMoveViolationV1::UnsupportedProjection,
            ..
        })
    ));
}

#[test]
fn indirect_move_certificate_keeps_exact_and_one_short_work_limits() {
    let function = straight(vec![
        test_assign(2, SemanticOperandV1::Move(indirect(true))),
        write(true, false),
        test_assign(3, SemanticOperandV1::Move(indirect(true))),
    ]);
    let types = types(true, false);
    let baseline = plan_test_function(&function, &types).unwrap();
    // One block plus the three destination projections and one pointer-prefix
    // query. Adapter construction retains its separate prepaid budget.
    assert_eq!(baseline.partial_move_certificate().work_units(), 5);
    assert_eq!(baseline.partial_move_certificate().state_entries(), 0);
    let work = baseline.resources().work_units() + baseline.auxiliary_resources.work_units + 5;
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
