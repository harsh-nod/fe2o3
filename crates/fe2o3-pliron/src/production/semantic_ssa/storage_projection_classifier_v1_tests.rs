use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateLayoutV1, SemanticAggregateTypeV1, SemanticAssignmentV1, SemanticConstantV1,
    SemanticConstantValueV1, SemanticMemoryLoadV1, SemanticMutabilityV1, SemanticPointerKindV1,
    SemanticPointerMetadataV1, SemanticPointerTypeV1, SemanticScalarValueV1,
};
use fe2o3_mir_model::{SsaResolvedEventV1, SsaValueV1};

fn indirect_paths() -> Vec<Vec<SemanticProjectionKindV1>> {
    use SemanticProjectionKindV1::{Dereference as D, Field as F};
    vec![
        vec![D],
        vec![F(0), D],
        vec![F(0), F(0), D],
        vec![D, F(0)],
        vec![F(0), D, F(0)],
        vec![D, F(0), D],
        vec![F(0), D, D],
    ]
}

// Isolated adapter fixtures use a real typed projection chain, not a type checker
// bypass in the production owner. Actual source-owner emission is tested in C1.
fn typed_path(
    kinds: &[SemanticProjectionKindV1],
) -> (Vec<SemanticTypeDeclV1>, SemanticTypeIdV1, SemanticPlaceV1) {
    let mut types = test_types(false);
    let mut child = SemanticTypeIdV1::from_index(1);
    let mut projections = Vec::new();
    for (index, kind) in kinds.iter().rev().enumerate() {
        projections.push(SemanticProjectionV1::new(*kind, child).unwrap());
        let declaration = &types[child.index() as usize];
        let (layout, shape) = match kind {
            SemanticProjectionKindV1::Field(0) => (
                SemanticTypeLayoutV1::aggregate(
                    declaration.layout().size_bytes(),
                    declaration.layout().alignment_bytes(),
                    SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![child]).unwrap()),
            ),
            SemanticProjectionKindV1::Dereference => (
                SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        child,
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Mutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            ),
            _ => unreachable!("fixture supports field and pointer chains"),
        };
        child = SemanticTypeIdV1::from_index(types.len() as u32);
        let tag = u8::try_from(index).unwrap().checked_add(170).unwrap();
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(test_bytes(tag)),
            SemanticLayoutIdentityV1::from_sha256(test_bytes(tag)),
            layout,
            shape,
        ));
    }
    projections.reverse();
    let place = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        projections,
        SemanticTypeIdV1::from_index(1),
    )
    .unwrap();
    (types, child, place)
}

fn source_function(
    root: SemanticTypeIdV1,
    root_role: SemanticLocalRoleV1,
    statements: Vec<SemanticStatementV1>,
) -> SemanticFunctionDeclV1 {
    retype_source(
        test_function(vec![test_block(
            110,
            statements,
            SemanticTerminatorKindV1::Return,
        )]),
        root,
        root_role,
    )
}

fn retype_source(
    source: SemanticFunctionDeclV1,
    root: SemanticTypeIdV1,
    root_role: SemanticLocalRoleV1,
) -> SemanticFunctionDeclV1 {
    let mut locals = source.locals().to_vec();
    locals[1] = test_local(61, root.index(), root_role);
    locals[3] = test_local(63, root.index(), SemanticLocalRoleV1::Temporary);
    SemanticFunctionDeclV1::new(
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
    .unwrap()
}

fn rvalue(kind: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            test_scalar_place(2),
            SemanticRvalueV1::new(SemanticTypeIdV1::from_index(1), kind),
        )),
    )
}

fn load(place: SemanticPlaceV1) -> SemanticStatementV1 {
    rvalue(SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
        place,
        SemanticVolatilityV1::NonVolatile,
        None,
    )))
}

fn constant() -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        SemanticTypeIdV1::from_index(1),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(7, 4).unwrap()),
    ))
}

fn input(function: &SemanticFunctionDeclV1) -> (SsaConstructionInputV1, usize) {
    let (input, implicit, work) =
        semantic_function_ssa_input_v1(function, None, &[], &BTreeSet::new());
    assert!(implicit.is_empty());
    (input, work)
}

#[test]
fn storage_loads_through_direct_aggregate_and_nested_pointers_keep_root_ssa() {
    for path in indirect_paths() {
        let (types, root, place) = typed_path(&path);
        let function = source_function(root, SemanticLocalRoleV1::Argument(0), vec![load(place)]);
        let (input, work) = input(&function);
        assert!(input.promotable()[1], "{path:?}");
        assert_eq!(work, path.len());
        assert_eq!(
            input.blocks()[0].events(),
            [
                SsaEventV1::Use(SsaVariableIdV1::new(1)),
                SsaEventV1::Define(SsaVariableIdV1::new(2)),
            ]
        );
        let plan = plan_test_function(&function, &types).unwrap();
        let entry = plan
            .plan
            .entry_definitions()
            .iter()
            .find(|entry| entry.variable() == SsaVariableIdV1::new(1))
            .unwrap();
        assert_eq!(
            plan.plan.resolved_event(SsaBlockIdV1::new(0), 0),
            Some(&SsaResolvedEventV1::Use {
                variable: SsaVariableIdV1::new(1),
                value: entry.value(),
            })
        );
        assert!(
            plan.plan
                .promoted_variables()
                .contains(&SsaVariableIdV1::new(1))
        );
    }
}

#[test]
fn storage_writes_crossing_any_dereference_do_not_address_the_holder() {
    for path in indirect_paths() {
        let (types, root, place) = typed_path(&path);
        for statement in [
            test_store(place.clone(), constant()),
            test_assign_to(place, constant()),
        ] {
            let function = source_function(root, SemanticLocalRoleV1::Argument(0), vec![statement]);
            let (input, work) = input(&function);
            assert!(input.promotable()[1], "{path:?}");
            assert_eq!(work, path.len());
            let plan = plan_test_function(&function, &types).unwrap();
            assert!(
                matches!(plan.plan.resolved_event(SsaBlockIdV1::new(0), 0), Some(
                SsaResolvedEventV1::Use { variable, .. }
            ) if *variable == SsaVariableIdV1::new(1))
            );
        }
    }
}

#[test]
fn storage_borrow_and_address_of_observe_the_pointee_allocation() {
    for path in indirect_paths() {
        let (_, root, place) = typed_path(&path);
        for kind in [
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place.clone(),
            },
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: place.clone(),
            },
        ] {
            let function =
                source_function(root, SemanticLocalRoleV1::Argument(0), vec![rvalue(kind)]);
            let (input, work) = input(&function);
            assert!(input.promotable()[1], "{path:?}");
            assert_eq!(work, path.len());
        }
    }
}

#[test]
fn storage_direct_and_field_only_observations_retain_root_memory() {
    use SemanticProjectionKindV1::Field;
    for path in [vec![], vec![Field(0)], vec![Field(0), Field(0)]] {
        let (_, root, place) = typed_path(&path);
        for statement in [
            load(place.clone()),
            test_store(place.clone(), constant()),
            rvalue(SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place.clone(),
            }),
            rvalue(SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: place.clone(),
            }),
        ] {
            let function = source_function(root, SemanticLocalRoleV1::Argument(0), vec![statement]);
            let (input, work) = input(&function);
            assert!(!input.promotable()[1], "{path:?}");
            assert_eq!(work, path.len());
        }
        if !path.is_empty() {
            let function = source_function(
                root,
                SemanticLocalRoleV1::Argument(0),
                vec![test_assign_to(place, constant())],
            );
            assert!(!input(&function).0.promotable()[1]);
        }
    }
}

#[test]
fn storage_genuine_holder_address_observation_is_sticky_in_either_order() {
    use SemanticProjectionKindV1::{Dereference, Field};
    let (_, root, indirect) = typed_path(&[Field(0), Dereference]);
    for prefix in [0, 1] {
        let addressed = if prefix == 0 {
            test_typed_place(1, root.index())
        } else {
            SemanticPlaceV1::new(
                indirect.local(),
                indirect.projections()[..1].to_vec(),
                indirect.projections()[0].result_type(),
            )
            .unwrap()
        };
        for before in [false, true] {
            let address = rvalue(SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: addressed.clone(),
            });
            let read = load(indirect.clone());
            let statements = if before {
                vec![address, read]
            } else {
                vec![read, address]
            };
            let function = source_function(root, SemanticLocalRoleV1::Argument(0), statements);
            let (input, work) = input(&function);
            assert!(!input.promotable()[1]);
            assert_eq!(work, 2 + prefix);
        }
    }
}

#[test]
fn storage_call_destination_and_drop_use_the_same_allocation_boundary() {
    for path in indirect_paths() {
        let (_, root, place) = typed_path(&path);
        let call = test_call(
            0,
            vec![],
            Some(SemanticCallDestinationV1::new(
                place.clone(),
                test_edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
        );
        let drop = SemanticTerminatorKindV1::Drop {
            place,
            drop_glue: SemanticFunctionIdV1::from_index(0),
            target: test_edge(SemanticEdgeRoleV1::DropReturn, 1),
            unwind: SemanticUnwindActionV1::Unreachable,
        };
        for terminator in [call, drop] {
            let function = retype_source(
                test_function(vec![
                    test_block(110, vec![], terminator),
                    test_block(111, vec![], SemanticTerminatorKindV1::Return),
                ]),
                root,
                SemanticLocalRoleV1::Argument(0),
            );
            let (input, work) = input(&function);
            assert!(input.promotable()[1], "{path:?}");
            assert_eq!(work, path.len());
            let plan = plan_semantic_function_ssa_v1(
                SemanticFunctionIdV1::from_index(0),
                &function,
                ProductionSemanticSsaLimitsV1::default(),
            )
            .unwrap();
            assert!(
                matches!(plan.plan.resolved_event(SsaBlockIdV1::new(0), 0), Some(
                SsaResolvedEventV1::Use { variable, .. }
            ) if *variable == SsaVariableIdV1::new(1))
            );
        }
    }
}

#[test]
fn storage_pointer_classification_does_not_initialize_a_missing_holder() {
    for path in indirect_paths() {
        let (_, root, place) = typed_path(&path);
        let function = source_function(root, SemanticLocalRoleV1::Temporary, vec![load(place)]);
        assert!(matches!(plan_semantic_function_ssa_v1(
            SemanticFunctionIdV1::from_index(0), &function, ProductionSemanticSsaLimitsV1::default(),
        ), Err(ProductionSemanticSsaErrorV1::Planner {
            error: SsaPlannerErrorV1::UndefinedAtUse { variable, .. }, ..
        }) if variable == SsaVariableIdV1::new(1)));
    }
}

#[test]
fn storage_pointer_classification_does_not_resurrect_a_moved_holder() {
    for path in indirect_paths() {
        let (_, root, place) = typed_path(&path);
        let function = source_function(
            root,
            SemanticLocalRoleV1::Argument(0),
            vec![
                test_assign_to(
                    test_typed_place(3, root.index()),
                    SemanticOperandV1::Move(test_typed_place(1, root.index())),
                ),
                load(place),
            ],
        );
        assert!(matches!(plan_semantic_function_ssa_v1(
            SemanticFunctionIdV1::from_index(0), &function, ProductionSemanticSsaLimitsV1::default(),
        ), Err(ProductionSemanticSsaErrorV1::Planner {
            error: SsaPlannerErrorV1::UndefinedAtUse { variable, .. }, ..
        }) if variable == SsaVariableIdV1::new(1)));
    }
}

fn work_limit(work: usize) -> ProductionSemanticSsaLimitsV1 {
    let defaults = SsaPlannerLimitsV1::default();
    ProductionSemanticSsaLimitsV1::new(
        SsaPlannerLimitsV1::try_new(
            defaults.max_variables(),
            defaults.max_blocks(),
            defaults.max_edges(),
            defaults.max_events(),
            defaults.max_edge_definitions(),
            defaults.max_output_items(),
            defaults.max_storage_words(),
            work,
        )
        .unwrap(),
    )
}

#[test]
fn storage_scan_work_has_independent_exact_and_one_short_planner_boundaries() {
    use SemanticProjectionKindV1::{Dereference, Field};
    for depth in [1, 2, 17, 33] {
        for pointer_first in [false, true] {
            let mut path = vec![Field(0); depth];
            path[if pointer_first { 0 } else { depth - 1 }] = Dereference;
            let (_, root, place) = typed_path(&path);
            let reference = source_function(
                root,
                SemanticLocalRoleV1::Argument(0),
                vec![test_assign(2, SemanticOperandV1::Copy(place.clone()))],
            );
            let target = source_function(root, SemanticLocalRoleV1::Argument(0), vec![load(place)]);
            let (reference_input, reference_work) = input(&reference);
            let (target_input, target_work) = input(&target);
            assert_eq!(reference_work, 0);
            assert_eq!(target_work, depth);
            assert_eq!(target_input, reference_input);

            // Four locals, one block, one statement, two events, no edges or
            // projected moves: (4*8 + 12 + 8 + 2*4) + 2 + 8 = 70.
            let auxiliary = 70 + depth;
            let reference_plan = plan_semantic_function_ssa_v1(
                SemanticFunctionIdV1::from_index(0),
                &reference,
                ProductionSemanticSsaLimitsV1::default(),
            )
            .unwrap();
            assert_eq!(reference_plan.auxiliary_resources.work_units, 70);
            assert_eq!(reference_plan.partial_moves.work_units(), 0);
            let expected = reference_plan.resources().work_units() + auxiliary;
            let exact = plan_semantic_function_ssa_v1(
                SemanticFunctionIdV1::from_index(0),
                &target,
                work_limit(expected),
            )
            .unwrap();
            assert_eq!(exact.auxiliary_resources.work_units, auxiliary);
            assert_eq!(exact.resources(), reference_plan.resources());
            assert!(matches!(plan_semantic_function_ssa_v1(
                SemanticFunctionIdV1::from_index(0), &target, work_limit(expected - 1),
            ), Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
                resource: SsaPlannerResourceV1::WorkUnits, required, limit, ..
            }) if required == expected && limit == expected - 1));
            assert!(matches!(plan_semantic_function_ssa_v1(
                SemanticFunctionIdV1::from_index(0), &target, work_limit(auxiliary - 1),
            ), Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
                resource: SsaPlannerResourceV1::WorkUnits, required, limit, ..
            }) if required == auxiliary && limit == auxiliary - 1));
        }
    }
}

#[test]
fn storage_scan_work_counts_each_site_even_after_a_root_is_memory() {
    use SemanticProjectionKindV1::{Dereference, Field};
    let (_, root, place) = typed_path(&[Field(0), Field(0), Dereference]);
    let field = SemanticPlaceV1::new(
        place.local(),
        place.projections()[..2].to_vec(),
        place.projections()[1].result_type(),
    )
    .unwrap();
    let function = source_function(
        root,
        SemanticLocalRoleV1::Argument(0),
        vec![
            rvalue(SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: field,
            }),
            load(place.clone()),
            load(place.clone()),
            test_store(place, constant()),
        ],
    );
    let (input, work) = input(&function);
    assert!(!input.promotable()[1]);
    assert_eq!(work, 2 + 3 + 3 + 3);
}

#[test]
fn static_field_assignment_defines_the_root_and_later_uses_the_new_version() {
    for depth in [1, 2, 8] {
        let (types, root, place) = typed_path(&vec![SemanticProjectionKindV1::Field(0); depth]);
        let function = source_function(
            root,
            SemanticLocalRoleV1::Argument(0),
            vec![
                test_assign_to(place.clone(), constant()),
                rvalue(SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place))),
            ],
        );
        let (input, _, work) =
            semantic_function_ssa_input_v1(&function, Some(&types), &[], &BTreeSet::new());
        assert!(input.promotable()[1]);
        assert_eq!(work, 3 + 4 * depth);
        assert_eq!(
            input.blocks()[0].events(),
            [
                SsaEventV1::Use(SsaVariableIdV1::new(1)),
                SsaEventV1::Define(SsaVariableIdV1::new(1)),
                SsaEventV1::Use(SsaVariableIdV1::new(1)),
                SsaEventV1::Define(SsaVariableIdV1::new(2)),
            ]
        );
        let plan = plan_test_function(&function, &types).unwrap();
        let Some(SsaResolvedEventV1::Define { variable, value }) =
            plan.plan.resolved_event(SsaBlockIdV1::new(0), 1)
        else {
            panic!("missing field-update root definition")
        };
        assert_eq!(*variable, SsaVariableIdV1::new(1));
        assert_eq!(
            plan.plan.resolved_event(SsaBlockIdV1::new(0), 2),
            Some(&SsaResolvedEventV1::Use {
                variable: *variable,
                value: *value
            },)
        );
        let Some(SsaResolvedEventV1::Use { value: old, .. }) =
            plan.plan.resolved_event(SsaBlockIdV1::new(0), 0)
        else {
            panic!("missing old holder use")
        };
        assert_ne!(old, value);
    }
}

#[test]
fn static_field_self_move_evaluates_source_before_holder_and_definition() {
    let (types, root, place) = typed_path(&[SemanticProjectionKindV1::Field(0)]);
    let function = source_function(
        root,
        SemanticLocalRoleV1::Argument(0),
        vec![test_assign_to(
            place.clone(),
            SemanticOperandV1::Move(place),
        )],
    );
    let (input, _, _) =
        semantic_function_ssa_input_v1(&function, Some(&types), &[], &BTreeSet::new());
    assert_eq!(
        input.blocks()[0].events(),
        [
            SsaEventV1::Use(SsaVariableIdV1::new(1)),
            SsaEventV1::Use(SsaVariableIdV1::new(1)),
            SsaEventV1::Define(SsaVariableIdV1::new(1)),
        ]
    );
    let plan = plan_test_function(&function, &types).unwrap();
    assert_eq!(
        plan.plan.resolved_event(SsaBlockIdV1::new(0), 0),
        plan.plan.resolved_event(SsaBlockIdV1::new(0), 1)
    );
}

#[test]
fn static_field_update_preserves_a_disjoint_removed_leaf() {
    let types = test_types(false);
    let statements = vec![
        rvalue(SemanticRvalueKindV1::Use(SemanticOperandV1::Move(
            test_place(1, Some(1)),
        ))),
        test_assign_to(test_place(1, Some(0)), constant()),
        rvalue(SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
            test_place(1, Some(0)),
        ))),
    ];
    let function = source_function(
        SemanticTypeIdV1::from_index(0),
        SemanticLocalRoleV1::Argument(0),
        statements.clone(),
    );
    let plan = plan_test_function(&function, &types).unwrap();
    assert!(
        plan.plan
            .promoted_variables()
            .contains(&SsaVariableIdV1::new(1))
    );
    let mut hostile = statements;
    hostile.push(rvalue(SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
        test_place(1, Some(1)),
    ))));
    let hostile = source_function(
        SemanticTypeIdV1::from_index(0),
        SemanticLocalRoleV1::Argument(0),
        hostile,
    );
    assert!(matches!(
        plan_test_function(&hostile, &types),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            local: 1,
            statement: Some(3),
            ..
        })
    ));
}

#[test]
fn static_field_updates_do_not_promote_address_observed_or_union_storage() {
    for union in [false, true] {
        let types = test_types(union);
        let mut statements = vec![test_assign_to(test_place(1, Some(0)), constant())];
        if !union {
            statements.push(rvalue(SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: test_place(1, Some(0)),
            }));
        }
        let function = source_function(
            SemanticTypeIdV1::from_index(0),
            SemanticLocalRoleV1::Argument(0),
            statements,
        );
        let (input, _, _) =
            semantic_function_ssa_input_v1(&function, Some(&types), &[], &BTreeSet::new());
        assert!(!input.promotable()[1]);
    }
}

#[test]
fn static_field_updated_holders_receive_join_and_loop_block_arguments() {
    let types = test_types(false);
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let goto = |target| SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target));
    let switch = |left, right| SemanticTerminatorKindV1::SwitchInt {
        discriminant: constant(),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                edge(SemanticEdgeRoleV1::SwitchValue, left),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, right),
        )
        .unwrap(),
    };
    for looping in [false, true] {
        let update = test_assign_to(test_place(1, Some(0)), constant());
        let read = rvalue(SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
            test_place(1, Some(0)),
        )));
        let blocks = if looping {
            vec![
                test_block(110, vec![], goto(1)),
                test_block(111, vec![update], switch(1, 2)),
                test_block(112, vec![read], SemanticTerminatorKindV1::Return),
            ]
        } else {
            vec![
                test_block(110, vec![], switch(1, 2)),
                test_block(111, vec![update.clone()], goto(3)),
                test_block(112, vec![update], goto(3)),
                test_block(113, vec![read], SemanticTerminatorKindV1::Return),
            ]
        };
        let function = retype_source(
            test_function(blocks),
            SemanticTypeIdV1::from_index(0),
            SemanticLocalRoleV1::Argument(0),
        );
        let plan = plan_test_function(&function, &types).unwrap();
        let joined = SsaBlockIdV1::new(if looping { 1 } else { 3 });
        assert!(
            plan.plan
                .transport_variables(joined)
                .unwrap()
                .contains(&SsaVariableIdV1::new(1))
        );
        assert!(
            matches!(plan.plan.resolved_event(joined, 0), Some(SsaResolvedEventV1::Use {
            variable, value: SsaValueV1::BlockArgument { block, variable: argument },
        }) if *block == joined && *variable == SsaVariableIdV1::new(1) && variable == argument)
        );
    }
}

#[test]
fn static_field_partial_construction_is_retained_before_the_real_ssa_planner() {
    for depth in [1, 2, 8] {
        let (types, root, place) = typed_path(&vec![SemanticProjectionKindV1::Field(0); depth]);
        for argument in [false, true] {
            let function = source_function(
                root,
                if argument {
                    SemanticLocalRoleV1::Argument(0)
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                vec![
                    test_assign_to(place.clone(), constant()),
                    rvalue(SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                        place.clone(),
                    ))),
                ],
            );
            let plan = plan_test_function(&function, &types).unwrap();
            assert_eq!(
                plan.plan
                    .promoted_variables()
                    .contains(&SsaVariableIdV1::new(1)),
                argument
            );
            assert_eq!(
                plan.plan.resolved_event(SsaBlockIdV1::new(0), 0).is_some(),
                argument
            );
            assert_eq!(
                plan.plan.resolved_event(SsaBlockIdV1::new(0), 1).is_some(),
                argument
            );
            assert_eq!(
                plan.plan.resolved_event(SsaBlockIdV1::new(0), 2).is_some(),
                argument
            );
            assert!(matches!(plan.plan.resolved_event(SsaBlockIdV1::new(0), 3),
                Some(SsaResolvedEventV1::Define { variable, .. }) if *variable == SsaVariableIdV1::new(2)));
        }
    }
}

#[test]
fn static_field_holder_reset_is_retained_but_whole_redefinition_restores_ssa() {
    let types = test_types(false);
    for restore in [false, true] {
        let mut statements = vec![SemanticStatementV1::new(
            fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(1)),
        )];
        if restore {
            // Save the entry holder in a distinct scalar-SSA aggregate local
            // before resetting its original storage generation.
            statements.insert(
                0,
                test_assign_to(
                    test_place(3, None),
                    SemanticOperandV1::Copy(test_place(1, None)),
                ),
            );
            statements.push(test_assign_to(
                test_place(1, None),
                SemanticOperandV1::Copy(test_place(3, None)),
            ));
        }
        statements.push(test_assign_to(test_place(1, Some(0)), constant()));
        let function = source_function(
            SemanticTypeIdV1::from_index(0),
            SemanticLocalRoleV1::Argument(0),
            statements,
        );
        let plan = plan_test_function(&function, &types).unwrap();
        assert_eq!(
            plan.plan
                .promoted_variables()
                .contains(&SsaVariableIdV1::new(1)),
            restore
        );
    }
}

#[test]
fn static_field_availability_demotes_only_the_incomplete_candidate_at_a_real_join() {
    let types = test_types(false);
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let goto = |target| SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target));
    for missing in [false, true] {
        let initialize = || {
            test_assign_to(
                test_place(3, None),
                SemanticOperandV1::Copy(test_place(1, None)),
            )
        };
        let function = retype_source(
            test_function(vec![
                test_block(
                    110,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: constant(),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                edge(SemanticEdgeRoleV1::SwitchValue, 1),
                            )],
                            edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                        )
                        .unwrap(),
                    },
                ),
                test_block(111, vec![initialize()], goto(3)),
                test_block(
                    112,
                    if missing { vec![] } else { vec![initialize()] },
                    goto(3),
                ),
                test_block(
                    113,
                    vec![
                        test_assign_to(test_place(1, Some(0)), constant()),
                        test_assign_to(test_place(3, Some(0)), constant()),
                    ],
                    SemanticTerminatorKindV1::Return,
                ),
            ]),
            SemanticTypeIdV1::from_index(0),
            SemanticLocalRoleV1::Argument(0),
        );
        let plan = plan_test_function(&function, &types).unwrap();
        assert!(
            plan.plan
                .promoted_variables()
                .contains(&SsaVariableIdV1::new(1))
        );
        assert_eq!(
            plan.plan
                .promoted_variables()
                .contains(&SsaVariableIdV1::new(3)),
            !missing
        );
    }
}

#[test]
fn static_field_holder_analysis_is_in_the_production_function_work_envelope() {
    let (types, root, place) = typed_path(&[SemanticProjectionKindV1::Field(0)]);
    let function = source_function(
        root,
        SemanticLocalRoleV1::Temporary,
        vec![
            test_assign_to(place.clone(), constant()),
            rvalue(SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place))),
        ],
    );
    let plan = plan_test_function(&function, &types).unwrap();
    // Four locals, two statements, four events, one block, no entry/edge defs.
    // Original adapter: 76 items + 4 event work + 8 partial-state work + 7
    // projection checks. Holder: 28 marker + 3 census + 396 sparse work.
    let auxiliary = 95 + 28 + 3 + 396;
    assert_eq!(plan.auxiliary_resources.work_units, auxiliary);
    let total = auxiliary + plan.resources().work_units();
    let exact = plan_semantic_function_ssa_with_module_v1(
        SemanticFunctionIdV1::from_index(0),
        &function,
        &types,
        &[],
        work_limit(total),
    )
    .unwrap();
    assert_eq!(exact, plan);
    for required in [auxiliary, total] {
        assert!(matches!(plan_semantic_function_ssa_with_module_v1(
            SemanticFunctionIdV1::from_index(0), &function, &types, &[], work_limit(required - 1),
        ), Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits, required: actual, limit, ..
        }) if actual == required && limit == required - 1));
    }
}

#[test]
fn static_holder_classifier_keeps_enum_and_dynamic_array_updates_in_memory() {
    use fe2o3_mir_model::semantic_mir_v1::SemanticEnumVariantV1;
    for enumeration in [false, true] {
        let mut types = test_types(false);
        let scalar = SemanticTypeIdV1::from_index(1);
        let root = SemanticTypeIdV1::from_index(types.len() as u32);
        let shape = if enumeration {
            SemanticTypeShapeV1::Enum {
                discriminant: scalar,
                variants: vec![SemanticEnumVariantV1::new(
                    0,
                    SemanticAggregateTypeV1::new(vec![scalar]).unwrap(),
                )]
                .into_boxed_slice(),
            }
        } else {
            SemanticTypeShapeV1::Array {
                element: scalar,
                length: 4,
            }
        };
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(test_bytes(201)),
            SemanticLayoutIdentityV1::from_sha256(test_bytes(202)),
            SemanticTypeLayoutV1::new(Some(if enumeration { 8 } else { 16 }), 4).unwrap(),
            shape,
        ));
        let projections = if enumeration {
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(0), root).unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), scalar).unwrap(),
            ]
        } else {
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                    scalar,
                )
                .unwrap(),
            ]
        };
        let place =
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), projections, scalar).unwrap();
        let function = source_function(
            root,
            SemanticLocalRoleV1::Argument(0),
            vec![test_assign_to(place, constant())],
        );
        let (input, _, _) =
            semantic_function_ssa_input_v1(&function, Some(&types), &[], &BTreeSet::new());
        assert!(!input.promotable()[1], "enum={enumeration}");
        assert!(
            !input.blocks()[0]
                .events()
                .iter()
                .any(|event| *event == SsaEventV1::Define(SsaVariableIdV1::new(1)))
        );
    }
}

#[test]
fn static_holder_classifier_requires_exact_complete_projection_types() {
    let (types, root, valid) = typed_path(&[SemanticProjectionKindV1::Field(0)]);
    let valid_function = source_function(
        root,
        SemanticLocalRoleV1::Argument(0),
        vec![test_assign_to(valid, constant())],
    );
    let (checked, _, _) =
        semantic_function_ssa_input_v1(&valid_function, Some(&types), &[], &BTreeSet::new());
    assert!(checked.promotable()[1]);
    for mode in 0..3 {
        let declared = if mode == 0 {
            root
        } else {
            SemanticTypeIdV1::from_index(1)
        };
        let place = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Field(if mode == 1 { 99 } else { 0 }),
                    declared,
                )
                .unwrap(),
            ],
            declared,
        )
        .unwrap();
        let function = source_function(
            root,
            SemanticLocalRoleV1::Argument(0),
            vec![test_assign_to(place, constant())],
        );
        let (input, _, _) = semantic_function_ssa_input_v1(
            &function,
            if mode == 2 { None } else { Some(&types) },
            &[],
            &BTreeSet::new(),
        );
        assert!(!input.promotable()[1], "mode={mode}");
    }
}
