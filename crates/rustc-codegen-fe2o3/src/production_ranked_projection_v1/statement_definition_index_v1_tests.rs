fn definition_index_reference_before_v1(
    function: &SemanticFunctionDeclV1,
    local: usize,
    block: usize,
    end: usize,
) -> Option<usize> {
    let body = function.blocks().get(block)?;
    if end > body.statements().len() {
        return None;
    }
    (0..end).rev().find(|&statement| {
        let mut found = false;
        visit_statement_definition_places(body.statements()[statement].kind(), &mut |place| {
            found |= local_definition_index(place) == Some(local);
        });
        found
    })
}

fn definition_index_result_v1(
    result: Result<Option<ScalarAssignmentSiteV1>, ProductionRankedProjectionErrorV1>,
) -> Result<Option<(usize, usize)>, String> {
    result
        .map(|site| site.map(|site| (site.block, site.statement)))
        .map_err(|error| format!("{error:?}"))
}

fn definition_index_check_queries_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
) {
    let mut indexed = SemanticAssertProofsV1::new(types, function).unwrap();
    indexed.statement_definitions = Some(StatementDefinitionIndexV1::new(function).unwrap());
    let mut scanned = SemanticAssertProofsV1::new(types, function).unwrap();
    for block in 0..=function.blocks().len() {
        let length = function
            .blocks()
            .get(block)
            .map_or(1, |block| block.statements().len());
        for statement in 0..=length + 1 {
            for local in (0..=function.locals().len()).chain([usize::MAX]) {
                for initial in [
                    0,
                    MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - 1,
                    MAX_PROJECTED_LOOP_GRAPH_WORK_V1,
                ] {
                    indexed.work = initial;
                    scanned.work = initial;
                    let site = ScalarAssignmentSiteV1 { block, statement };
                    assert_eq!(
                        definition_index_result_v1(
                            indexed.exact_reaching_assignment_v1(local, site)
                        ),
                        definition_index_result_v1(
                            scanned.exact_reaching_assignment_v1(local, site)
                        ),
                        "local={local} block={block} statement={statement} initial={initial}"
                    );
                    assert_eq!(indexed.work, scanned.work);
                }
            }
        }
    }
}

#[test]
fn statement_definition_index_matches_reverse_scan_for_every_definition_kind() {
    let scalar = typed_place(1, U64_TYPE);
    let dereference = dereferenced_place();
    let projected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), U64_TYPE).unwrap()],
        U64_TYPE,
    )
    .unwrap();
    let statements = vec![
        typed_assignment(
            1,
            U64_TYPE,
            SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, 7, 8)),
        ),
        statement(SemanticStatementKindV1::Nop),
        statement(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(1),
        )),
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(1),
        )),
        statement(SemanticStatementKindV1::Assume(typed_constant(
            BOOL_TYPE, 1, 1,
        ))),
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            projected.clone(),
            SemanticRvalueV1::new(
                U64_TYPE,
                SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, 9, 8)),
            ),
        ))),
        hostile_scalar_definition(HostileScalarDefinitionV1::Store),
        hostile_scalar_definition(HostileScalarDefinitionV1::AtomicRmw),
        hostile_scalar_definition(HostileScalarDefinitionV1::AtomicCompareExchange),
        statement(SemanticStatementKindV1::SetDiscriminant {
            place: projected,
            variant_index: 0,
        }),
        statement(SemanticStatementKindV1::Deinitialize(scalar.clone())),
        statement(SemanticStatementKindV1::AtomicRmw(
            SemanticAtomicRmwV1::new(
                scalar.clone(),
                scalar,
                typed_constant(U64_TYPE, 1, 8),
                SemanticAtomicRmwOpV1::Exchange,
                atomic_access(),
            ),
        )),
        statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            dereference,
            typed_constant(U64_TYPE, 0, 8),
            SemanticVolatilityV1::NonVolatile,
            None,
        ))),
    ];
    // Component fixtures exercise the exact existing visitor grammar, not new
    // semantic validity/admission rules for every synthetic statement combination.
    let function = projection_function_with_locals(
        vec![block(241, statements, SemanticTerminatorKindV1::Return)],
        (0..6)
            .map(|id| {
                if id == 0 {
                    local(210, SCALAR_TYPE, SemanticLocalRoleV1::Return)
                } else {
                    local(210 + id, U64_TYPE, SemanticLocalRoleV1::Temporary)
                }
            })
            .collect(),
    );
    let index = StatementDefinitionIndexV1::new(&function).unwrap();
    for local in [0, 1, 2, 3, 5, 6, usize::MAX] {
        for end in 0..=function.blocks()[0].statements().len() {
            assert_eq!(
                index.before(local, 0, end),
                definition_index_reference_before_v1(&function, local, 0, end)
            );
        }
    }
    // Both atomic definition visits stay represented; inventory counts are not deduplicated.
    assert_eq!(
        index.rows.iter().filter(|row| **row == (1, 0, 11)).count(),
        2
    );
    definition_index_check_queries_v1(&assertion_proof_types(), &function);
}

#[test]
fn statement_definition_index_preserves_cross_block_dominance_call_and_loop_fallback() {
    let types = assertion_proof_types();
    let locals = vec![
        local(210, SCALAR_TYPE, SemanticLocalRoleV1::Return),
        local(211, U64_TYPE, SemanticLocalRoleV1::Temporary),
        local(212, BOOL_TYPE, SemanticLocalRoleV1::Argument(0)),
    ];
    for call_destination in [false, true] {
        let last = if call_destination {
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(0),
                    vec![],
                    Some(SemanticCallDestinationV1::new(
                        typed_place(1, U64_TYPE),
                        cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            )
        } else {
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1))
        };
        let function = projection_function_with_locals(
            vec![
                block(
                    220,
                    vec![typed_assignment(
                        1,
                        U64_TYPE,
                        SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, 7, 8)),
                    )],
                    last,
                ),
                block(
                    221,
                    vec![statement(SemanticStatementKindV1::Nop)],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: typed_operand(2, BOOL_TYPE),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                            )],
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                        )
                        .unwrap(),
                    },
                ),
                block(
                    222,
                    vec![statement(SemanticStatementKindV1::Nop)],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
            locals.clone(),
        );
        definition_index_check_queries_v1(&types, &function);
    }
    let diamond = projection_function_with_locals(
        vec![
            block(
                220,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: typed_operand(2, BOOL_TYPE),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        )],
                        cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(
                221,
                vec![typed_assignment(
                    1,
                    U64_TYPE,
                    SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, 7, 8)),
                )],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            block(
                222,
                vec![],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            block(
                223,
                vec![statement(SemanticStatementKindV1::Nop)],
                SemanticTerminatorKindV1::Return,
            ),
        ],
        locals,
    );
    definition_index_check_queries_v1(&types, &diamond);
}

#[test]
fn statement_definition_index_preserves_escaped_and_unknown_local_early_returns() {
    let types = aggregate_types_v2();
    let function = aggregate_function_v2(vec![
        aggregate_assignment_v2(1, ARRAY_TYPE, aggregate_array_v2([1, 2, 3, 4])),
        aggregate_assignment_v2(
            3,
            POINTER_TYPE,
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], ARRAY_TYPE)
                    .unwrap(),
            },
        ),
    ]);
    definition_index_check_queries_v1(&types, &function);
}

#[test]
fn statement_definition_index_virtual_debit_matches_unit_loop_at_every_boundary() {
    for initial in [
        0,
        1,
        MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - 2,
        MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - 1,
        MAX_PROJECTED_LOOP_GRAPH_WORK_V1,
        MAX_PROJECTED_LOOP_GRAPH_WORK_V1 + 1,
        usize::MAX - 1,
        usize::MAX,
    ] {
        for visits in 0..8 {
            let mut indexed = initial;
            let mut scanned = initial;
            let fast = charge_statement_scan_equivalent_v1(&mut indexed, visits);
            let slow = (|| {
                for _ in 0..visits {
                    project_loop_graph_charge_v1(&mut scanned, 1)?;
                }
                Ok::<(), ProductionRankedProjectionErrorV1>(())
            })();
            assert_eq!(format!("{fast:?}"), format!("{slow:?}"));
            assert_eq!(indexed, scanned);
        }
    }
    let mut work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - 1;
    assert!(charge_statement_scan_equivalent_v1(&mut work, usize::MAX).is_err());
    assert_eq!(work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1 + 1);
}

#[test]
fn statement_definition_index_is_sparse_deterministic_and_function_local() {
    let empty = aggregate_function_v2(vec![statement(SemanticStatementKindV1::Nop)]);
    let empty_index = StatementDefinitionIndexV1::new(&empty).unwrap();
    assert!(empty_index.rows.is_empty());
    assert_eq!(empty_index.rows.capacity(), 0);
    let first = aggregate_function_v2(vec![
        aggregate_assignment_v2(2, SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(7))),
        statement(SemanticStatementKindV1::Nop),
    ]);
    let second = aggregate_function_v2(vec![
        statement(SemanticStatementKindV1::Nop),
        aggregate_assignment_v2(2, SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(11))),
    ]);
    let first_index = StatementDefinitionIndexV1::new(&first).unwrap();
    let second_index = StatementDefinitionIndexV1::new(&second).unwrap();
    assert_eq!(first_index.before(2, 0, 2), Some(0));
    assert_eq!(second_index.before(2, 0, 2), Some(1));
    assert_eq!(
        first_index.rows,
        StatementDefinitionIndexV1::new(&first).unwrap().rows
    );
    assert_eq!(first_index.before(2, usize::MAX, usize::MAX), None);
    assert_eq!(first_index.before(usize::MAX, 0, usize::MAX), None);
}

#[test]
fn statement_definition_index_retains_actual_gpu_values_errors_and_resolver_cleanup() {
    let types = aggregate_types_v2();
    for value in [7, 19] {
        let function = aggregate_function_v2(vec![
            aggregate_assignment_v2(1, ARRAY_TYPE, aggregate_array_v2([value, 2, 3, 4])),
            aggregate_assignment_v2(
                5,
                ARRAY_TYPE,
                SemanticRvalueKindV1::Use(aggregate_copy_v2(1, ARRAY_TYPE, vec![])),
            ),
            aggregate_assignment_v2(1, ARRAY_TYPE, aggregate_array_v2([90, 91, 92, 93])),
            aggregate_output_v2(aggregate_copy_v2(
                5,
                SCALAR_TYPE,
                vec![aggregate_index_v2(0)],
            )),
        ]);
        for initial in [0, MAX_PROJECTED_LOOP_GRAPH_WORK_V1] {
            let mut indexed = GpuSemanticExpressionResolverV2::new(&types, &function).unwrap();
            let mut scanned = GpuSemanticExpressionResolverV2::new(&types, &function).unwrap();
            assert!(indexed.definitions.statement_definitions.is_some());
            scanned.definitions.statement_definitions = None;
            indexed.definitions.work = initial;
            scanned.definitions.work = initial;
            let site = ScalarAssignmentSiteV1 {
                block: 0,
                statement: 3,
            };
            let actual =
                indexed.resolve_store_v2(function.blocks()[0].statements()[3].kind(), site);
            let reference =
                scanned.resolve_store_v2(function.blocks()[0].statements()[3].kind(), site);
            assert_eq!(actual, reference);
            if initial == 0 {
                assert_eq!(
                    actual,
                    Ok(aggregate_expected_v2(u64::try_from(value).unwrap()))
                );
            }
            assert_eq!(indexed.definitions.work, scanned.definitions.work);
            assert!(indexed.use_site.is_none() && scanned.use_site.is_none());
            assert!(indexed.visiting.is_empty() && scanned.visiting.is_empty());
        }
    }
}

#[test]
fn statement_definition_index_retains_future_and_self_use_refusals_without_cycle_cache() {
    let types = aggregate_types_v2();
    for self_use in [false, true] {
        let destination = if self_use { 1 } else { 5 };
        let mut statements = vec![aggregate_assignment_v2(
            destination,
            ARRAY_TYPE,
            SemanticRvalueKindV1::Use(aggregate_copy_v2(1, ARRAY_TYPE, vec![])),
        )];
        if !self_use {
            statements.push(aggregate_assignment_v2(
                1,
                ARRAY_TYPE,
                aggregate_array_v2([7, 11, 13, 17]),
            ));
        }
        statements.push(aggregate_output_v2(aggregate_copy_v2(
            destination,
            SCALAR_TYPE,
            vec![aggregate_index_v2(0)],
        )));
        let function = aggregate_function_v2(statements);
        let site = ScalarAssignmentSiteV1 {
            block: 0,
            statement: function.blocks()[0].statements().len() - 1,
        };
        let mut indexed = GpuSemanticExpressionResolverV2::new(&types, &function).unwrap();
        let mut scanned = GpuSemanticExpressionResolverV2::new(&types, &function).unwrap();
        scanned.definitions.statement_definitions = None;
        let actual = indexed.resolve_store_v2(
            function.blocks()[0].statements()[site.statement].kind(),
            site,
        );
        let reference = scanned.resolve_store_v2(
            function.blocks()[0].statements()[site.statement].kind(),
            site,
        );
        assert_eq!(actual, reference);
        assert_eq!(
            actual.unwrap_err(),
            "GPU semantic local has no exact reaching assignment"
        );
        assert_eq!(indexed.definitions.work, scanned.definitions.work);
        assert!(indexed.use_site.is_none() && indexed.visiting.is_empty());
        assert!(scanned.use_site.is_none() && scanned.visiting.is_empty());
    }
}

#[test]
fn statement_definition_index_large_prefix_changes_search_not_logical_work() {
    const STATEMENTS: usize = 20_000;
    let mut statements = vec![aggregate_assignment_v2(
        2,
        SCALAR_TYPE,
        SemanticRvalueKindV1::Use(constant(7)),
    )];
    statements.resize_with(STATEMENTS, || statement(SemanticStatementKindV1::Nop));
    let function = aggregate_function_v2(statements);
    let index = StatementDefinitionIndexV1::new(&function).unwrap();
    assert_eq!(index.rows.len(), 1);
    // Two linear construction walks plus Q binary predecessor lookups replace
    // Q*STATEMENTS statement visits. This is a count argument, not a timing claim.
    for _ in 0..STATEMENTS {
        assert_eq!(index.before(2, 0, STATEMENTS), Some(0));
    }
    let types = aggregate_types_v2();
    let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
    proof.statement_definitions = Some(index);
    let result = proof
        .exact_reaching_assignment_v1(
            2,
            ScalarAssignmentSiteV1 {
                block: 0,
                statement: STATEMENTS,
            },
        )
        .unwrap();
    assert_eq!(
        result.map(|site| (site.block, site.statement)),
        Some((0, 0))
    );
    assert_eq!(proof.work, STATEMENTS);
}
