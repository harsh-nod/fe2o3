use super::*;

fn shared_borrow_v41(reference: u32) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(
            fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1::new(
                test_scalar_place(reference),
                SemanticRvalueV1::new(
                    SemanticTypeIdV1::from_index(1),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: test_typed_place(1, 0),
                    },
                ),
            ),
        ),
    )
}

fn shared_copy_chain_v41(count: u32) -> SemanticFunctionDeclV1 {
    let mut statements = vec![shared_borrow_v41(2)];
    for index in 0..count {
        statements.push(test_assign(
            3 + index,
            SemanticOperandV1::Copy(test_scalar_place(2 + index)),
        ));
    }
    test_function_with_reference_locals(
        vec![test_block(
            230,
            statements,
            occurrence_call(2 + count, None),
        )],
        (count as usize + 3).max(4),
    )
}

#[test]
fn shared_reference_copy_chains_keep_only_the_original_borrow_occurrence() {
    for count in [1, 8, 64, 128] {
        let function = shared_copy_chain_v41(count);
        let callables = [test_intrinsic_callable(function.abi().clone())];
        let sites = transparent_borrow_sites_v1(&function, &callables);
        assert_eq!(
            sites
                .iter()
                .map(|site| site.test_coordinates())
                .collect::<Vec<_>>(),
            [(0, 0)]
        );
        assert!(source_is_promotable(&function, &callables));
        plan_semantic_function_ssa_with_callables_v1(
            SemanticFunctionIdV1::from_index(0),
            &function,
            &callables,
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
    }
}

#[test]
fn shared_reference_original_and_every_copy_may_each_have_a_closed_consumer() {
    for copies in [1, 8, 32] {
        let mut statements = vec![shared_borrow_v41(2)];
        for index in 0..copies {
            statements.push(test_assign(
                3 + index,
                SemanticOperandV1::Copy(test_scalar_place(2)),
            ));
        }
        let mut blocks = vec![test_block(100, statements, occurrence_call(2, Some(1)))];
        for index in 0..copies {
            blocks.push(test_block(
                101 + index as u8,
                vec![],
                occurrence_call(3 + index, (index + 1 < copies).then_some(index + 2)),
            ));
        }
        let function = test_function_with_reference_locals(blocks, copies as usize + 3);
        let callables = [test_intrinsic_callable(function.abi().clone())];
        assert_eq!(transparent_borrow_sites_v1(&function, &callables).len(), 1);
        assert!(source_is_promotable(&function, &callables));
        plan_semantic_function_ssa_with_callables_v1(
            SemanticFunctionIdV1::from_index(0),
            &function,
            &callables,
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
    }
}

#[test]
fn one_escaped_shared_copy_poisons_the_entire_borrow_even_with_a_closed_sibling() {
    let function = test_function_with_reference_locals(
        vec![
            test_block(
                230,
                vec![
                    shared_borrow_v41(2),
                    test_assign(3, SemanticOperandV1::Copy(test_scalar_place(2))),
                ],
                occurrence_call(2, Some(1)),
            ),
            test_block(
                231,
                vec![test_assign(
                    4,
                    SemanticOperandV1::Move(test_scalar_place(3)),
                )],
                SemanticTerminatorKindV1::Return,
            ),
        ],
        5,
    );
    let callables = [test_intrinsic_callable(function.abi().clone())];
    assert!(transparent_borrow_sites_v1(&function, &callables).is_empty());
    assert!(!source_is_promotable(&function, &callables));
}

#[test]
fn copied_reference_types_and_mutability_cannot_manufacture_a_shared_origin() {
    for (borrow, operand) in [
        (
            test_borrow(2, 1),
            SemanticOperandV1::Copy(test_scalar_place(2)),
        ),
        (
            shared_borrow_v41(2),
            SemanticOperandV1::Copy(test_typed_place(2, 2)),
        ),
        (
            shared_borrow_v41(2),
            SemanticOperandV1::Copy(test_dereference_place(2, 0)),
        ),
    ] {
        let function = test_function(vec![test_block(
            230,
            vec![borrow, test_assign(3, operand)],
            occurrence_call(3, None),
        )]);
        let callables = [test_intrinsic_callable(function.abi().clone())];
        assert!(transparent_borrow_sites_v1(&function, &callables).is_empty());
        assert!(!source_is_promotable(&function, &callables));
    }
}

#[test]
fn shared_copy_before_its_unique_borrow_definition_is_not_authenticated() {
    let function = test_function(vec![test_block(
        230,
        vec![
            test_assign(3, SemanticOperandV1::Copy(test_scalar_place(2))),
            shared_borrow_v41(2),
        ],
        occurrence_call(3, None),
    )]);
    let callables = [test_intrinsic_callable(function.abi().clone())];
    assert!(transparent_borrow_sites_v1(&function, &callables).is_empty());
}

#[test]
fn repeated_shared_borrow_and_copy_temporaries_bind_each_local_occurrence() {
    let function = test_function(vec![
        test_block(
            230,
            vec![
                shared_borrow_v41(2),
                test_assign(3, SemanticOperandV1::Copy(test_scalar_place(2))),
            ],
            occurrence_call(3, Some(1)),
        ),
        test_block(
            231,
            vec![
                shared_borrow_v41(2),
                test_assign(3, SemanticOperandV1::Copy(test_scalar_place(2))),
            ],
            occurrence_call(3, None),
        ),
    ]);
    let callables = [test_intrinsic_callable(function.abi().clone())];
    let sites = transparent_borrow_sites_v1(&function, &callables);
    assert_eq!(
        sites
            .iter()
            .map(|site| site.test_coordinates())
            .collect::<Vec<_>>(),
        [(0, 0), (1, 0)]
    );
    assert!(source_is_promotable(&function, &callables));
}

#[test]
fn copied_alias_redefinition_and_ungrounded_cycles_do_not_reuse_a_receipt() {
    for statements in [
        vec![
            shared_borrow_v41(2),
            test_assign(3, SemanticOperandV1::Copy(test_scalar_place(2))),
            test_assign(3, SemanticOperandV1::Copy(test_scalar_place(0))),
        ],
        vec![
            shared_borrow_v41(2),
            test_assign(3, SemanticOperandV1::Copy(test_scalar_place(4))),
            test_assign(4, SemanticOperandV1::Copy(test_scalar_place(3))),
        ],
    ] {
        let function = test_function_with_reference_locals(
            vec![test_block(230, statements, occurrence_call(3, None))],
            5,
        );
        let callables = [test_intrinsic_callable(function.abi().clone())];
        assert!(transparent_borrow_sites_v1(&function, &callables).is_empty());
    }
}

#[test]
fn shared_copy_graph_pricing_has_an_independent_indexed_linear_envelope() {
    let mut previous = None;
    for count in [8, 16, 32, 64, 128] {
        let function = shared_copy_chain_v41(count);
        let resources = adapter::borrow_graph_resources_v41(&function, 0).unwrap();
        let count = count as usize;
        let height = (usize::BITS - (2 * count + 4).leading_zeros()) as usize + 1;
        assert_eq!(resources.storage_words, 64 + 64 * (count + 1));
        assert_eq!(
            resources.work_units,
            (2 * count + 3) * 32 * height + 32 * (count + 1)
        );
        if let Some((work, storage)) = previous {
            assert!(resources.work_units < 3 * work);
            assert!(resources.storage_words < 2 * storage);
        }
        previous = Some((resources.work_units, resources.storage_words));
    }
}

#[test]
fn shared_copy_graph_is_included_in_exact_and_one_short_function_resources() {
    let function = shared_copy_chain_v41(8);
    let callables = [test_intrinsic_callable(function.abi().clone())];
    let id = SemanticFunctionIdV1::from_index(0);
    let base = plan_semantic_function_ssa_with_callables_v1(
        id,
        &function,
        &callables,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let graph = adapter::borrow_graph_resources_v41(&function, 0).unwrap();
    assert!(base.auxiliary_resources.work_units >= graph.work_units);
    assert!(base.auxiliary_resources.storage_words >= graph.storage_words);
    let work = base.resources().work_units()
        + base.auxiliary_resources.work_units
        + base.partial_moves.work_units();
    let storage = base.resources().storage_words()
        + base.auxiliary_resources.storage_words
        + base.partial_moves.state_entries();
    let defaults = SsaPlannerLimitsV1::default();
    let limits = |storage, work| {
        ProductionSemanticSsaLimitsV1::new(
            SsaPlannerLimitsV1::try_new(
                defaults.max_variables(),
                defaults.max_blocks(),
                defaults.max_edges(),
                defaults.max_events(),
                defaults.max_edge_definitions(),
                defaults.max_output_items(),
                storage,
                work,
            )
            .unwrap(),
        )
    };
    let exact = plan_semantic_function_ssa_with_callables_v1(
        id,
        &function,
        &callables,
        limits(storage, work),
    )
    .unwrap();
    assert_eq!(exact, base);
    for (s, w, expected, limit) in [
        (
            storage - 1,
            work,
            SsaPlannerResourceV1::StorageWords,
            storage - 1,
        ),
        (storage, work - 1, SsaPlannerResourceV1::WorkUnits, work - 1),
    ] {
        assert!(
            matches!(plan_semantic_function_ssa_with_callables_v1(id, &function, &callables, limits(s, w)),
            Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit { resource, required, limit: actual_limit, .. })
            if resource == expected && actual_limit == limit && required == limit + 1)
        );
    }
}

fn occurrence_call(reference: u32, target: Option<u32>) -> SemanticTerminatorKindV1 {
    test_call(
        0,
        vec![SemanticOperandV1::Copy(test_scalar_place(reference))],
        target.map(|target| {
            SemanticCallDestinationV1::new(
                test_scalar_place(0),
                test_edge(SemanticEdgeRoleV1::CallReturn, target),
            )
        }),
    )
}

fn two_occurrences(first: Vec<SemanticStatementV1>) -> SemanticFunctionDeclV1 {
    test_function(vec![
        test_block(210, first, occurrence_call(2, Some(1))),
        test_block(211, vec![test_borrow(2, 1)], occurrence_call(2, None)),
    ])
}

#[test]
fn repeated_borrow_temporary_has_distinct_intrinsic_consumers() {
    let function = two_occurrences(vec![test_borrow(2, 1)]);
    let callables = [test_intrinsic_callable(function.abi().clone())];
    assert_eq!(transparent_borrow_sites_v1(&function, &callables).len(), 2);
    assert!(source_is_promotable(&function, &callables));
    plan_semantic_function_ssa_with_callables_v1(
        SemanticFunctionIdV1::from_index(0),
        &function,
        &callables,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
}

#[test]
fn repeated_pipeline_borrows_allow_a_grounded_source_loop() {
    let function = test_function(vec![
        test_block(212, vec![test_borrow(2, 1)], occurrence_call(2, Some(1))),
        test_block(213, vec![test_borrow(2, 1)], occurrence_call(2, Some(0))),
    ]);
    let callables = [test_operation_callable(
        function.abi().clone(),
        SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineEvent {
            pipeline: SemanticTypeIdV1::from_index(0),
            event: fe2o3_mir_model::semantic_mir_v1::SemanticWorkgroupPipelineEventV1::Stage,
        },
        120,
    )];
    assert_eq!(transparent_borrow_sites_v1(&function, &callables).len(), 2);
    assert!(source_is_promotable(&function, &callables));
    plan_semantic_function_ssa_with_callables_v1(
        SemanticFunctionIdV1::from_index(0),
        &function,
        &callables,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
}

#[test]
fn repeated_reference_reborrow_chains_retain_their_own_parent_occurrences() {
    let function = test_function_with_reference_locals(
        vec![
            test_block(
                214,
                vec![test_borrow(2, 1), test_reborrow(3, 2)],
                occurrence_call(3, Some(1)),
            ),
            test_block(
                215,
                vec![test_borrow(2, 1), test_reborrow(4, 2)],
                occurrence_call(4, None),
            ),
        ],
        5,
    );
    let callables = [test_intrinsic_callable(function.abi().clone())];
    let sites = transparent_borrow_sites_v1(&function, &callables);
    let (input, _, _) = semantic_function_ssa_input_v1(&function, None, &callables, &sites);
    assert_eq!(sites.len(), 4);
    assert!(input.promotable().iter().all(|promotable| *promotable));
}

#[test]
fn repeated_parent_and_child_reference_temporaries_are_occurrence_sensitive() {
    let function = test_function(vec![
        test_block(
            216,
            vec![test_borrow(2, 1), test_reborrow(3, 2)],
            occurrence_call(3, Some(1)),
        ),
        test_block(
            217,
            vec![test_borrow(2, 1), test_reborrow(3, 2)],
            occurrence_call(3, None),
        ),
    ]);
    let callables = [test_intrinsic_callable(function.abi().clone())];
    assert_eq!(transparent_borrow_sites_v1(&function, &callables).len(), 4);
    assert!(source_is_promotable(&function, &callables));
}

#[test]
fn a_unique_reference_can_still_reach_one_intrinsic_across_blocks() {
    let function = test_function(vec![
        test_block(
            218,
            vec![test_borrow(2, 1)],
            SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        test_block(219, vec![], occurrence_call(2, None)),
    ]);
    let callables = [test_intrinsic_callable(function.abi().clone())];
    assert_eq!(transparent_borrow_sites_v1(&function, &callables).len(), 1);
    assert!(source_is_promotable(&function, &callables));
}

#[test]
fn an_unknown_incoming_reference_invalidates_all_repeated_occurrences() {
    let function = test_function(vec![
        test_block(220, vec![test_borrow(2, 1)], occurrence_call(2, Some(1))),
        test_block(221, vec![test_reborrow(3, 2)], occurrence_call(3, Some(2))),
        test_block(222, vec![test_borrow(2, 1)], occurrence_call(2, None)),
    ]);
    let callables = [test_intrinsic_callable(function.abi().clone())];
    assert!(transparent_borrow_sites_v1(&function, &callables).is_empty());
    assert!(!source_is_promotable(&function, &callables));
}

#[test]
fn a_use_before_the_local_borrow_definition_is_not_grounded_by_that_definition() {
    let function = two_occurrences(vec![
        test_assign(3, SemanticOperandV1::Copy(test_scalar_place(2))),
        test_borrow(2, 1),
    ]);
    let callables = [test_intrinsic_callable(function.abi().clone())];
    assert!(transparent_borrow_sites_v1(&function, &callables).is_empty());
    assert!(!source_is_promotable(&function, &callables));
}

#[test]
fn ordinary_dereference_copy_and_move_escapes_are_not_transparent() {
    for (operand, accepted) in [
        (SemanticOperandV1::Copy(test_scalar_place(2)), 1),
        (SemanticOperandV1::Move(test_scalar_place(2)), 0),
        (SemanticOperandV1::Copy(test_dereference_place(2, 0)), 1),
    ] {
        let function = two_occurrences(vec![test_borrow(2, 1), test_assign(3, operand)]);
        let callables = [test_intrinsic_callable(function.abi().clone())];
        assert!(!source_is_promotable(&function, &callables));
        // A moved reference is no longer available at the call and therefore
        // also triggers the conservative unknown-use rejection for this local.
        assert_eq!(
            transparent_borrow_sites_v1(&function, &callables).len(),
            accepted
        );
    }
}

#[test]
fn overwriting_a_borrow_does_not_give_the_old_occurrence_a_consumer() {
    let function = test_function(vec![test_block(
        223,
        vec![test_borrow(2, 1), test_borrow(2, 1)],
        occurrence_call(2, None),
    )]);
    let callables = [test_intrinsic_callable(function.abi().clone())];
    assert_eq!(transparent_borrow_sites_v1(&function, &callables).len(), 1);
    assert!(!source_is_promotable(&function, &callables));
}

#[test]
fn an_ordinary_redefinition_or_storage_kill_cannot_reuse_a_borrow_occurrence() {
    for (replacement, independent_fresh_borrow) in [
        (
            test_assign(2, SemanticOperandV1::Copy(test_scalar_place(3))),
            true,
        ),
        (
            test_assign(2, SemanticOperandV1::Move(test_scalar_place(3))),
            false,
        ),
        (test_storage_dead(2), false),
        (
            SemanticStatementV1::new(
                fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(2)),
            ),
            false,
        ),
    ] {
        let function = two_occurrences(vec![test_borrow(2, 1), replacement]);
        let callables = [test_intrinsic_callable(function.abi().clone())];
        let sites = transparent_borrow_sites_v1(&function, &callables);
        let expected = if independent_fresh_borrow {
            // The Copy has a distinct, ungrounded definition. It cannot reuse
            // block 0's borrow, but need not poison block 1's fresh definition.
            [SemanticTransparentBorrowSiteV1 {
                block: 1,
                statement: 0,
            }]
            .into_iter()
            .collect()
        } else {
            BTreeSet::new()
        };
        assert_eq!(sites, expected);
        assert!(!sites.contains(&SemanticTransparentBorrowSiteV1 {
            block: 0,
            statement: 0,
        }));
        assert!(!source_is_promotable(&function, &callables));
    }
}

#[test]
fn repeated_borrow_occurrences_still_reject_forked_consumers() {
    let function = test_function_with_reference_locals(
        vec![
            test_block(
                224,
                vec![test_borrow(2, 1), test_reborrow(3, 2), test_reborrow(4, 2)],
                occurrence_call(3, Some(1)),
            ),
            test_block(225, vec![test_borrow(2, 1)], occurrence_call(2, None)),
        ],
        5,
    );
    let callables = [test_intrinsic_callable(function.abi().clone())];
    assert_eq!(transparent_borrow_sites_v1(&function, &callables).len(), 1);
    assert!(!source_is_promotable(&function, &callables));
}

#[test]
fn repeated_borrow_consumers_need_the_exact_authenticated_intrinsic_type() {
    let function = two_occurrences(vec![test_borrow(2, 1)]);
    let wrong_type = [test_operation_callable(
        function.abi().clone(),
        SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineEvent {
            pipeline: SemanticTypeIdV1::from_index(1),
            event: fe2o3_mir_model::semantic_mir_v1::SemanticWorkgroupPipelineEventV1::Stage,
        },
        126,
    )];
    let ordinary = [SemanticCallableDeclV1::defined(
        SemanticFunctionIdV1::from_index(0),
    )];
    for callables in [&wrong_type, &ordinary] {
        assert!(transparent_borrow_sites_v1(&function, callables).is_empty());
        assert!(!source_is_promotable(&function, callables));
    }
}

#[test]
fn storage_live_requires_a_fresh_repeated_borrow_definition_before_use() {
    for reset_before_borrow in [true, false] {
        let reset = SemanticStatementV1::new(
            fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(2)),
        );
        let borrow = test_borrow(2, 1);
        let statements = if reset_before_borrow {
            vec![reset, borrow]
        } else {
            vec![borrow, reset]
        };
        let function = two_occurrences(statements);
        let callables = [test_intrinsic_callable(function.abi().clone())];
        assert_eq!(
            transparent_borrow_sites_v1(&function, &callables).len(),
            if reset_before_borrow { 2 } else { 0 }
        );
        assert_eq!(
            source_is_promotable(&function, &callables),
            reset_before_borrow
        );
    }
}

fn repeated_return_reference_function(ignore_return: bool) -> SemanticFunctionDeclV1 {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAbiArgumentV1, SemanticAbiExtensionV1, SemanticAbiRegularAttributesV1,
        SemanticAbiValueAttributesV1,
    };

    let consume_child = |reference| {
        test_call(
            0,
            vec![SemanticOperandV1::Copy(test_scalar_place(reference))],
            Some(SemanticCallDestinationV1::new(
                test_scalar_place(4),
                test_edge(SemanticEdgeRoleV1::CallReturn, 3),
            )),
        )
    };
    let base = test_function_with_reference_locals(
        vec![
            test_block(
                226,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(test_scalar_place(5)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            1,
                            test_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        )],
                        test_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            test_block(
                227,
                vec![test_borrow(0, 1), test_reborrow(2, 0)],
                consume_child(2),
            ),
            test_block(
                228,
                vec![test_borrow(0, 1), test_reborrow(3, 0)],
                consume_child(3),
            ),
            test_block(229, vec![], SemanticTerminatorKindV1::Return),
        ],
        6,
    );
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::from_rustc_bits(0).unwrap(),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let return_mode = if ignore_return {
        SemanticAbiPassModeV1::Ignore
    } else {
        SemanticAbiPassModeV1::Direct(attributes)
    };
    let abi = SemanticFunctionAbiV1::from_rustc(
        base.abi().identity(),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(51)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        2,
        vec![
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                SemanticTypeIdV1::from_index(0),
                SemanticAbiPassModeV1::Direct(attributes),
            )),
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                SemanticTypeIdV1::from_index(1),
                SemanticAbiPassModeV1::Direct(attributes),
            )),
        ],
        SemanticAbiValueV1::new(SemanticTypeIdV1::from_index(1), return_mode),
    )
    .unwrap();
    let mut locals = base.locals().to_vec();
    locals[5] = test_local(77, 1, SemanticLocalRoleV1::Argument(1));
    SemanticFunctionDeclV1::new(
        base.identity(),
        SemanticFunctionRoleV1::InternalHelper,
        base.item_definition_identity(),
        base.monomorphization_identity(),
        base.generic_type_arguments_identity(),
        base.const_generic_arguments_identity(),
        base.source(),
        abi,
        locals,
        base.entry(),
        base.blocks().to_vec(),
    )
    .unwrap()
}

#[test]
fn an_implicit_return_use_invalidates_both_branch_local_reborrow_chains() {
    for ignore_return in [true, false] {
        let function = repeated_return_reference_function(ignore_return);
        let callables = [test_intrinsic_callable(function.abi().clone())];
        // Both return-local definitions otherwise have one exact child consumer.
        // Only the non-Ignore ABI adds the implicit Use in the shared Return block.
        assert_eq!(
            transparent_borrow_sites_v1(&function, &callables).len(),
            if ignore_return { 4 } else { 0 }
        );
        assert_eq!(source_is_promotable(&function, &callables), ignore_return);
    }
}

#[test]
fn a_unique_return_role_reference_must_not_escape_its_intrinsic_reborrow_chain() {
    for ignore_return in [true, false] {
        let base = repeated_return_reference_function(ignore_return);
        let function = SemanticFunctionDeclV1::new(
            base.identity(),
            base.role(),
            base.item_definition_identity(),
            base.monomorphization_identity(),
            base.generic_type_arguments_identity(),
            base.const_generic_arguments_identity(),
            base.source(),
            base.abi().clone(),
            base.locals().to_vec(),
            base.entry(),
            vec![
                test_block(
                    230,
                    vec![test_borrow(0, 1), test_reborrow(2, 0)],
                    test_call(
                        0,
                        vec![SemanticOperandV1::Copy(test_scalar_place(2))],
                        Some(SemanticCallDestinationV1::new(
                            test_scalar_place(4),
                            test_edge(SemanticEdgeRoleV1::CallReturn, 1),
                        )),
                    ),
                ),
                test_block(231, vec![], SemanticTerminatorKindV1::Return),
            ],
        )
        .unwrap();
        let callables = [test_intrinsic_callable(function.abi().clone())];
        // Neither parent nor child is redefined. The sole extra escape is the
        // implicit return-local use, not an explicit Copy, Move or call operand.
        assert_eq!(
            transparent_borrow_sites_v1(&function, &callables).len(),
            if ignore_return { 2 } else { 0 }
        );
        assert_eq!(source_is_promotable(&function, &callables), ignore_return);
    }
}
