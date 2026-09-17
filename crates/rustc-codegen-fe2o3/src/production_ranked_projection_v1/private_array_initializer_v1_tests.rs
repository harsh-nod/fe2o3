fn private_initializer_fixture_v1(values: [u32; 8], repetitions: usize) -> SemanticFunctionDeclV1 {
    let old = literal_assertion(true, true, false);
    let mut statements = old.blocks()[0].statements().to_vec();
    let SemanticStatementKindV1::Assign(write) = statements[1].kind() else {
        panic!("indexed write");
    };
    let local = write.destination().local();
    for _ in 0..repetitions {
        statements.insert(
            1,
            typed_assignment(
                local.index(),
                A_ARRAY,
                SemanticRvalueKindV1::aggregate(
                    SemanticAggregateKindV1::Array,
                    values
                        .into_iter()
                        .map(|value| typed_constant(A_U32, value.into(), 4))
                        .collect(),
                )
                .unwrap(),
            ),
        );
    }
    private_write_statements_v1(&old, statements)
}

fn assert_private_initializer_rows_v1(
    materialized: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    root: &ProductionRankedRootProgramV1,
    statement: u32,
) {
    let rows: Vec<_> = root
        .access_sources
        .iter()
        .filter(|row| row.semantic_block() == 0 && row.semantic_statement() == Some(statement))
        .collect();
    assert_eq!(rows.len(), 8);
    for (component, row) in rows.into_iter().enumerate() {
        assert_eq!(row.semantic_access_ordinal() as usize, component);
        assert!(matches!(
            root.lowering.kernel().blocks()[row.ranked_block() as usize].operations()
                [row.ranked_operation() as usize],
            ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Write,
                ..
            }
        ));
    }
    let floor = materialized.executable_storage().retained_storage()
        + materialized.assert_origin_storage().payload_storage();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, floor);
    budget.reserve_storage(floor).unwrap();
    assert_eq!(
        materialized.materialized_private_array_initializer_count(
            ROOT,
            ROOT,
            fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
                block: fe2o3_mir_model::SsaBlockIdV1::new(0),
                statement,
            },
            &mut budget
        ),
        Ok(Some(8))
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn private_array_literal_initialization_attaches_through_both_existing_routes() {
    // Normalized repeated Aggregate, not a new rustc Repeat collector claim.
    for values in [[0; 8], [0, 1, 2, 3, 7, 31, 255, u32::MAX]] {
        for legacy in [false, true] {
            let owner = attach_private_write_fixture_v1(
                private_initializer_fixture_v1(values, 2),
                legacy,
                |materialized, root| {
                    assert_eq!(root.access_sources.len(), 17);
                    for statement in [1, 2] {
                        assert_private_initializer_rows_v1(materialized, root, statement);
                    }
                    let row = root.access_sources.last().unwrap();
                    assert_eq!(
                        (row.semantic_statement(), row.semantic_access_ordinal()),
                        (Some(3), 0)
                    );
                },
            )
            .unwrap();
            assert!(!owner.grants_artifact_or_launch_authority());
        }
    }
}

#[test]
fn private_array_nonliteral_initializer_keeps_explicit_value_relation_refusal() {
    let old = private_initializer_fixture_v1([0; 8], 1);
    let mut statements = old.blocks()[0].statements().to_vec();
    let SemanticStatementKindV1::Assign(index) = statements[0].kind() else {
        panic!("index definition");
    };
    let operand = SemanticOperandV1::Copy(index.destination().clone());
    let SemanticStatementKindV1::Assign(initializer) = statements[1].kind() else {
        panic!("initializer");
    };
    statements[1] = typed_assignment(
        initializer.destination().local().index(),
        A_ARRAY,
        SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::Array, vec![operand; 8]).unwrap(),
    );
    let materialized = assertion_materialized(private_write_statements_v1(&old, statements));
    let result = assertion_project(materialized);
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
            canonical_assertion_facts_v1::CanonicalAssertionErrorV1::PrivateArray(
                fe2o3_lower_mir_kernel::SemanticKirPrivateArrayQueryErrorV1::Incomplete(
                    "private initializer value requires a separate SSA operand relation"
                )
            )
        ))
    ));
}

#[test]
fn private_array_initialized_assign_and_store_accept_constant_copy_and_move() {
    for legacy in [false, true] {
        for store in [false, true] {
            for operand_kind in 0..3 {
                let old = private_initializer_fixture_v1([11; 8], 1);
                let mut statements = old.blocks()[0].statements().to_vec();
                let SemanticStatementKindV1::Assign(index) = statements[0].kind() else {
                    panic!("index");
                };
                // The value may be moved; the destination index must remain live.
                let value_local = 2;
                assert_eq!(old.locals()[value_local as usize].ty(), A_U32);
                assert_eq!(
                    old.locals()[value_local as usize].role(),
                    SemanticLocalRoleV1::Temporary
                );
                assert_ne!(index.destination().local().index(), value_local);
                let operand = match operand_kind {
                    0 => typed_constant(A_U32, 99, 4),
                    1 => SemanticOperandV1::Copy(typed_place(value_local, A_U32)),
                    2 => SemanticOperandV1::Move(typed_place(value_local, A_U32)),
                    _ => unreachable!(),
                };
                let SemanticStatementKindV1::Assign(write) = statements[2].kind() else {
                    panic!("indexed write");
                };
                let destination = write.destination().clone();
                statements[2] = statement(if store {
                    SemanticStatementKindV1::Store(
                        fe2o3_mir_model::semantic_mir_v1::SemanticMemoryStoreV1::new(
                            destination,
                            operand,
                            SemanticVolatilityV1::NonVolatile,
                            None,
                        ),
                    )
                } else {
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        destination,
                        SemanticRvalueV1::new(A_U32, SemanticRvalueKindV1::Use(operand)),
                    ))
                });
                statements.insert(
                    2,
                    typed_assignment(
                        value_local,
                        A_U32,
                        SemanticRvalueKindV1::Use(typed_constant(A_U32, 99, 4)),
                    ),
                );
                let _owner = attach_private_write_fixture_v1(
                    private_write_statements_v1(&old, statements),
                    legacy,
                    |materialized, root| {
                        assert_eq!(root.access_sources.len(), 9);
                        assert_private_initializer_rows_v1(materialized, root, 1);
                        let row = root.access_sources.last().unwrap();
                        assert_eq!(
                            (row.semantic_statement(), row.semantic_access_ordinal()),
                            (Some(3), 0)
                        );
                    },
                )
                .unwrap();
            }
        }
    }
}

#[test]
fn private_array_initializer_missing_row_and_wrong_index_reach_final_relation() {
    use fe2o3_lower_mir_kernel::{
        ProductionMirPlironTranslationErrorV1, ProductionSemanticKirErrorV1,
    };
    for legacy in [false, true] {
        for missing in [false, true] {
            let result = attach_private_write_fixture_v1(
                private_initializer_fixture_v1([11; 8], 1),
                legacy,
                |materialized, root| {
                    assert_private_initializer_rows_v1(materialized, root, 1);
                    if missing {
                        // Keep a dense prefix so receipt custody reaches the final relation.
                        let removed: Vec<_> = root.access_sources.drain(3..8).collect();
                        assert_eq!(removed.len(), 5);
                        for (offset, row) in removed.into_iter().enumerate() {
                            assert_eq!(row.semantic_statement(), Some(1));
                            assert_eq!(row.semantic_access_ordinal() as usize, offset + 3);
                        }
                        return;
                    }
                    let kernel = root.lowering.kernel();
                    let mut changed = 0;
                    let blocks = kernel
                        .blocks()
                        .iter()
                        .map(|block| {
                            let mut operations = block.operations().to_vec();
                            for operation in &mut operations {
                                if let ProductionRankedOperationV1::IndexConstant { value, .. } =
                                    operation
                                    && *value == 3
                                {
                                    *value = 2;
                                    changed += 1;
                                }
                            }
                            ProductionRankedBlockV1::with_index_arguments(
                                block.index_argument_count(),
                                operations,
                                block.terminator().clone(),
                            )
                        })
                        .collect();
                    assert_eq!(changed, 1);
                    let changed = ProductionRankedKernelV1::new(
                        kernel.function_name(),
                        kernel.argument_count(),
                        blocks,
                    )
                    .unwrap();
                    root.ranked_ir =
                        format_ranked_cfg(changed.function_name(), changed.blocks()).unwrap();
                    root.lowering = fe2o3_pliron::compile_ranked_kernel_for_lowering_v1(
                        ProductionConstructionV1::ranked_kernel(ROOT_NAME_V1, changed).unwrap(),
                        ProductionSessionLimitsV1::default(),
                    )
                    .unwrap();
                    assert!(root.lowering.all_mandatory_reports_are_clean());
                },
            );
            if missing {
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
                        ProductionMirPlironTranslationErrorV1::MissingRankedEffect {
                            semantic_block: 0,
                            semantic_statement: Some(1),
                            semantic_access_ordinal: 3,
                        }
                    ))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
                        ProductionMirPlironTranslationErrorV1::AllocationOriginMismatch { .. }
                    ))
                ));
            }
        }
    }
}

#[test]
fn private_array_initializer_probe_prepays_eight_before_source_or_query() {
    let function = private_initializer_fixture_v1([11; 8], 1);
    let types = assertion_types();
    for (statement_index, limit) in [(0, 8), (0, 7), (1, 7)] {
        let mut operations = Vec::new();
        let mut sources = Vec::new();
        let mut next = 0;
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 23);
        budget.reserve_storage(23).unwrap();
        let mut facts = PrivateSourceMeterV1 {
            budget: &mut budget,
            calls: 0,
        };
        let mut views = ProjectedViewsV1::new(function.locals().len(), Some(&mut facts));
        let result = project_private_array_initializer_v1(
            &types,
            &function,
            &function.blocks()[0].statements()[statement_index],
            0,
            statement_index,
            &mut views,
            &mut operations,
            &mut sources,
            &mut next,
        );
        assert!((0..function.locals().len()).all(|local| views.get_mut(local).unwrap().is_none()));
        drop(views);
        assert_eq!(facts.calls, 1);
        if limit == 8 {
            assert!(!result.unwrap());
            assert_eq!(facts.budget.work(), 8);
        } else {
            assert!(
                matches!(result, Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Work(error))))
                if error.actual() == 8 && error.limit() == 7)
            );
            assert_eq!(facts.budget.work(), 0);
        }
        assert!(operations.is_empty() && sources.is_empty());
        assert_eq!(
            (next, facts.budget.storage(), facts.budget.peak_storage()),
            (0, 23, 23)
        );
    }
}

#[test]
fn private_initializer_then_canonical_slice_reads_keeps_exact_site_census() {
    for legacy in [false, true] {
        for mode in [
            PrivateSliceCompositionV1::InitializerRead,
            PrivateSliceCompositionV1::InitializerTwoReads,
        ] {
            let _owner = private_slice_composition_attach_v1(mode, legacy)
                .expect("literal initializer and later checked canonical reads must attach");
        }
    }
}
