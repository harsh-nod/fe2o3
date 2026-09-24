fn private_copy_read_fixture_v1(read_index: u64, reads: usize) -> SemanticFunctionDeclV1 {
    let old = private_initializer_fixture_v1([11; 8], 1);
    let mut statements = old.blocks()[0].statements().to_vec();
    let SemanticStatementKindV1::Assign(write) = statements[2].kind() else {
        panic!("indexed write");
    };
    let place = SemanticPlaceV1::new(
        write.destination().local(),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::ConstantIndex {
                    offset: read_index,
                    minimum_length: 8,
                    from_end: false,
                },
                A_U32,
            )
            .unwrap(),
        ],
        A_U32,
    )
    .unwrap();
    assert_eq!(old.locals()[2].ty(), A_U32);
    for _ in 0..reads {
        statements.push(typed_assignment(
            2,
            A_U32,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place.clone())),
        ));
    }
    private_write_statements_v1(&old, statements)
}

fn private_copy_source_v1(statement: usize) -> ProjectedAccessSourceV1 {
    ProjectedAccessSourceV1 {
        block: 0,
        operation: 0,
        memory_space: MemorySpaceAttr::Private,
        access: AccessKindAttr::Read,
        source: SemanticSourceProvenanceV1::unavailable(),
        output_extent: None,
        semantic_site: Some(ProjectedSemanticAccessSiteV1 {
            block: 0,
            statement: Some(statement),
        }),
    }
}

#[test]
fn private_array_copy_read_attaches_and_resets_each_statement_ordinal() {
    for legacy in [false, true] {
        let owner = attach_private_write_fixture_v1(
            private_copy_read_fixture_v1(0, 2),
            legacy,
            |materialized, root| {
                assert_eq!(root.access_sources.len(), 11);
                assert_private_initializer_rows_v1(materialized, root, 1);
                for (statement, row) in (2..=4).zip(&root.access_sources[8..]) {
                    assert_eq!(
                        (
                            row.semantic_block(),
                            row.semantic_statement(),
                            row.semantic_access_ordinal()
                        ),
                        (0, Some(statement), 0)
                    );
                    let expected = if statement == 2 {
                        AccessKindAttr::Write
                    } else {
                        AccessKindAttr::Read
                    };
                    assert!(
                        matches!(root.lowering.kernel().blocks()[row.ranked_block() as usize]
                        .operations()[row.ranked_operation() as usize],
                        ProductionRankedOperationV1::Access { kind, .. } if kind == expected)
                    );
                }
            },
        )
        .unwrap();
        assert!(!owner.grants_artifact_or_launch_authority());
    }
}

#[test]
fn private_array_read_requires_independent_initialization() {
    use fe2o3_lower_mir_kernel::{
        ProductionMirPlironTranslationErrorV1, ProductionSemanticKirErrorV1,
    };
    for binary in [false, true] {
        for legacy in [false, true] {
            let old = if binary {
                private_binary_read_fixture_v1(true, BinaryReadOperandV1::Constant)
            } else {
                private_copy_read_fixture_v1(1, 1)
            };
            let mut entry = old.blocks()[0].statements().to_vec();
            let read = entry.pop().unwrap();
            let blocks = vec![
                SemanticBasicBlockV1::new(
                    old.blocks()[0].identity(),
                    old.blocks()[0].source(),
                    entry,
                    old.blocks()[0].terminator().clone(),
                )
                .unwrap(),
                SemanticBasicBlockV1::new(
                    old.blocks()[1].identity(),
                    old.blocks()[1].source(),
                    vec![read],
                    old.blocks()[1].terminator().clone(),
                )
                .unwrap(),
            ];
            let function = SemanticFunctionDeclV1::new(
                old.identity(),
                old.role(),
                old.item_definition_identity(),
                old.monomorphization_identity(),
                old.generic_type_arguments_identity(),
                old.const_generic_arguments_identity(),
                old.source(),
                old.abi().clone(),
                old.locals().to_vec(),
                old.entry(),
                blocks,
            )
            .unwrap()
            .with_kernel_entry(old.kernel_entry().unwrap().clone());
            let result = attach_private_write_fixture_v1(function, legacy, |materialized, root| {
                assert_eq!(root.access_sources.len(), if binary { 9 } else { 10 });
                with_canonical_assertions_v1(materialized, |session| {
                    assert_eq!(
                        session
                            .for_source(ROOT, ROOT)
                            .private_array_access_index_v1(
                                ProjectedSemanticAccessSiteV1 {
                                    block: 1,
                                    statement: Some(0)
                                },
                                fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::RvalueOperand(
                                    u32::from(binary)
                                ),
                            )?,
                        Some(u64::from(!binary))
                    );
                    Ok(())
                })
                .unwrap();
            });
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
                    ProductionMirPlironTranslationErrorV1::AllocationOriginMismatch { .. }
                ))
            ));
        }
    }
}

#[test]
fn private_array_binary_read_keeps_projected_destination_write_refused() {
    use fe2o3_lower_mir_kernel::{
        ProductionMirPlironTranslationErrorV1, ProductionSemanticKirErrorV1,
    };
    for legacy in [false, true] {
        let old = private_initializer_fixture_v1([0; 8], 1);
        let mut statements = old.blocks()[0].statements().to_vec();
        let SemanticStatementKindV1::Assign(write) = statements[2].kind() else {
            panic!("write");
        };
        let destination = write.destination().clone();
        statements[2] = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            destination.clone(),
            SemanticRvalueV1::new(
                A_U32,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Add,
                    left: SemanticOperandV1::Copy(destination),
                    right: typed_constant(A_U32, 1, 4),
                },
            ),
        )));
        let result = attach_private_write_fixture_v1(
            private_write_statements_v1(&old, statements),
            legacy,
            |materialized, root| {
                assert_eq!(root.access_sources.len(), 9);
                assert_private_initializer_rows_v1(materialized, root, 1);
                with_canonical_assertions_v1(materialized, |session| {
                    assert_eq!(
                        session
                            .for_source(ROOT, ROOT)
                            .private_array_access_index_v1(
                                private_copy_source_v1(2).semantic_site.unwrap(),
                                fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::RvalueOperand(0),
                            )?,
                        Some(0)
                    );
                    Ok(())
                })
                .unwrap();
            },
        );
        assert!(matches!(
            result,
            Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
                ProductionMirPlironTranslationErrorV1::MissingRankedEffect {
                    semantic_block: 0,
                    semantic_statement: Some(2),
                    semantic_access_ordinal: 1,
                }
            ))
        ));
    }
}

#[test]
fn private_array_read_missing_row_and_wrong_ranked_index_fail_final_relation() {
    use fe2o3_lower_mir_kernel::{
        ProductionMirPlironTranslationErrorV1, ProductionSemanticKirErrorV1,
    };
    for binary in [false, true] {
        for legacy in [false, true] {
            for missing in [false, true] {
                let result = attach_private_write_fixture_v1(
                    if binary {
                        private_binary_read_fixture_v1(true, BinaryReadOperandV1::Move)
                    } else {
                        private_copy_read_fixture_v1(0, 1)
                    },
                    legacy,
                    |_, root| {
                        let row = *root.access_sources.last().unwrap();
                        assert_eq!(row.semantic_statement(), Some(3));
                        if missing {
                            root.access_sources.pop();
                            return;
                        }
                        let kernel = root.lowering.kernel();
                        let ProductionRankedOperationV1::Access {
                            indices,
                            kind: AccessKindAttr::Read,
                            ..
                        } = &kernel.blocks()[row.ranked_block() as usize].operations()
                            [row.ranked_operation() as usize]
                        else {
                            panic!("read");
                        };
                        let [ProductionRankedValueV1::Local(index)] = indices.as_slice() else {
                            panic!("index");
                        };
                        let mut changed = 0;
                        let blocks = kernel
                            .blocks()
                            .iter()
                            .map(|block| {
                                let mut operations = block.operations().to_vec();
                                for operation in &mut operations {
                                    if let ProductionRankedOperationV1::IndexConstant {
                                        result,
                                        value,
                                    } = operation
                                        && result == index
                                    {
                                        assert_eq!(*value, 0);
                                        *value = 1;
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
                                semantic_statement: Some(3),
                                semantic_access_ordinal: 0,
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
}

#[test]
fn private_array_read_checks_caller_budget_and_rejects_duplicate_occurrences() {
    for binary in [false, true] {
        let function = if binary {
            private_binary_read_fixture_v1(true, BinaryReadOperandV1::Move)
        } else {
            private_copy_read_fixture_v1(0, 1)
        };
        let overhead = if binary { 100 } else { 52 };
        let program = assertion_project(assertion_materialized(function)).unwrap();
        let root = &program.roots[0];
        let row = root.access_sources.last().unwrap();
        let mut source = private_copy_source_v1(3);
        source.block = row.ranked_block() as usize;
        source.operation = row.ranked_operation() as usize;
        let semantic = program.materialized.semantic_ssa().source_semantic();
        with_canonical_assertions_v1(&program.materialized, |session| {
            let floor = program.materialized.retained_analysis_storage_v1();
            let direct = |limit| {
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, floor);
                budget.reserve_storage(floor).unwrap();
                let result = program
                    .materialized
                    .materialized_private_array_constant_index(
                        ROOT,
                        ROOT,
                        fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
                            block: fe2o3_mir_model::SsaBlockIdV1::new(0),
                            statement: 3,
                        },
                        fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::RvalueOperand(u32::from(
                            binary,
                        )),
                        &mut budget,
                    );
                (result, budget.work())
            };
            let (baseline, query_work) = direct(1_000_000);
            assert_eq!(baseline, Ok(Some(0)));
            // The genuine lowerer query is the independent adapter/caller oracle.
            // This measures row conversion, not the whole source factory.
            for query_limit in [query_work, query_work - 1] {
                let (expected, accepted) = direct(query_limit);
                let mut work = Work::new(query_limit + overhead);
                let mut budget = Budget::new(&mut work, floor);
                budget.reserve_storage(floor).unwrap();
                let result = production_access_sources(
                    semantic.types(),
                    &semantic.functions()[0],
                    root.lowering.kernel().blocks(),
                    &[source],
                    &mut session.for_source_with_query_budget_v1(&mut budget, ROOT, ROOT),
                );
                match (expected, result) {
                (Ok(Some(0)), Ok(rows)) => assert_eq!(rows, vec![*row]),
                (
                    Err(fe2o3_lower_mir_kernel::SemanticKirPrivateArrayQueryErrorV1::Resource(
                        Resource::Work(expected),
                    )),
                    Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                        CanonicalAssertionErrorV1::PrivateArray(
                            fe2o3_lower_mir_kernel::SemanticKirPrivateArrayQueryErrorV1::Resource(
                                Resource::Work(actual),
                            ),
                        ),
                    )),
                ) => {
                    assert_eq!(actual.actual(), expected.actual() + overhead);
                    assert_eq!(actual.limit(), expected.limit() + overhead);
                }
                other => panic!("caller changed lowerer query: {other:?}"),
            }
                assert_eq!(budget.work(), accepted + overhead);
                assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
            }
            let result = production_access_sources(
                semantic.types(),
                &semantic.functions()[0],
                root.lowering.kernel().blocks(),
                &[source, source],
                &mut session.for_source(ROOT, ROOT),
            );
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "private array read has duplicate source correspondence"
                ))
            ));
            Ok(())
        })
        .unwrap();
    }
}

include!("private_array_binary_read_source_v1_tests.rs");

#[test]
fn private_array_copy_read_prepays_candidate_and_requires_real_facts() {
    let function = private_copy_read_fixture_v1(0, 1);
    let types = assertion_types();
    for (statement, limit) in [(1, 47), (3, 47), (1, 48), (3, 48)] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 23);
        budget.reserve_storage(23).unwrap();
        let mut facts = PrivateSourceMeterV1 {
            budget: &mut budget,
            calls: 0,
        };
        let result = private_array_read_source_v1::retained_read(
            &types,
            &function,
            &private_copy_source_v1(statement),
            &mut facts,
        );
        if limit == 47 {
            assert!(
                matches!(result, Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                CanonicalAssertionErrorV1::Resource(Resource::Work(error))))
                if error.actual() == 48 && error.limit() == 47)
            );
            assert_eq!(facts.budget.work(), 0);
        } else if statement == 1 {
            assert!(!result.unwrap());
            assert_eq!(facts.budget.work(), 48);
        } else {
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "private array read requires live canonical correspondence"
                ))
            ));
            assert_eq!(facts.budget.work(), 48);
        }
        assert_eq!(facts.calls, 1);
        assert_eq!(
            (facts.budget.storage(), facts.budget.peak_storage()),
            (23, 23)
        );
    }
}

#[test]
fn private_array_copy_read_adapter_preserves_work_prefix_and_owner_floor() {
    use fe2o3_lower_mir_kernel::SemanticKirPrivateArrayQueryErrorV1 as QueryError;
    let materialized = assertion_materialized(private_copy_read_fixture_v1(0, 1));
    let floor = materialized.retained_analysis_storage_v1();
    let site = private_copy_source_v1(3).semantic_site.unwrap();
    let role = fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::RvalueOperand(0);
    with_canonical_assertions_v1(&materialized, |session| {
        let direct = |limit| {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, floor);
            budget.reserve_storage(floor).unwrap();
            let result = materialized.materialized_private_array_constant_index(
                ROOT,
                ROOT,
                fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
                    block: fe2o3_mir_model::SsaBlockIdV1::new(0),
                    statement: 3,
                },
                role,
                &mut budget,
            );
            assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
            (result, budget.work())
        };
        let (baseline, query_work) = direct(1_000_000);
        assert_eq!(baseline, Ok(Some(0)));
        for query_limit in [query_work, query_work - 1] {
            let (expected, accepted) = direct(query_limit);
            let mut work = Work::new(query_limit + 4);
            let mut budget = Budget::new(&mut work, floor);
            budget.reserve_storage(floor).unwrap();
            let actual = session
                .for_source_with_query_budget_v1(&mut budget, ROOT, ROOT)
                .private_array_access_index_v1(site, role);
            match (expected, actual) {
                (Ok(expected), Ok(actual)) => assert_eq!(actual, expected),
                (
                    Err(QueryError::Resource(Resource::Work(expected))),
                    Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                        CanonicalAssertionErrorV1::PrivateArray(QueryError::Resource(
                            Resource::Work(actual),
                        )),
                    )),
                ) => {
                    assert_eq!(actual.actual(), expected.actual() + 4);
                    assert_eq!(actual.limit(), expected.limit() + 4);
                }
                other => panic!("adapter changed direct query result: {other:?}"),
            }
            assert_eq!(budget.work(), accepted + 4);
            assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
        }
        for (limit, prepaid, accepted, attempted) in [
            (3, floor, 0, 4),
            (4, floor, 4, 5),
            (7, floor, 5, 8),
            (8, floor - 1, 8, 0),
        ] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, prepaid);
            budget.reserve_storage(prepaid).unwrap();
            let result = session
                .for_source_with_query_budget_v1(&mut budget, ROOT, ROOT)
                .private_array_access_index_v1(site, role);
            match result {
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(Resource::Work(error)),
                ))
                | Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::PrivateArray(QueryError::Resource(Resource::Work(
                        error,
                    ))),
                )) => {
                    assert_ne!(attempted, 0);
                    assert_eq!((error.actual(), error.limit()), (attempted, limit));
                }
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::PrivateArray(QueryError::Resource(
                        Resource::Accounting,
                    )),
                )) => assert_eq!(attempted, 0),
                other => panic!("unexpected prefix result: {other:?}"),
            }
            assert_eq!(budget.work(), accepted);
            assert_eq!(
                (budget.storage(), budget.peak_storage()),
                (prepaid, prepaid)
            );
        }
        Ok(())
    })
    .unwrap();
}
