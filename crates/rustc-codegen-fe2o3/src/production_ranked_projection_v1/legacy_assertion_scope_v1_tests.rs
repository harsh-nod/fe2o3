mod legacy_scope_tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;

    fn attach_private_write_fixture_v1(
        function: SemanticFunctionDeclV1,
        legacy: bool,
        inspect: impl FnOnce(
            &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
            &mut ProductionRankedRootProgramV1,
        ),
    ) -> Result<
        fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
        fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1,
    > {
        use fe2o3_lower_mir_kernel::{
            ProductionRankedSemanticProjectionReceiptV1, ProductionSemanticKirLimitsV1,
            ProductionSemanticKirOwnerV1,
        };
        let program = assertion_project(assertion_materialized(function.clone())).unwrap();
        let ProductionRankedSemanticProgramV1 {
            materialized,
            roots,
        } = program;
        let mut root = roots.into_vec().into_iter().next().unwrap();
        assert!(root.lowering.all_mandatory_reports_are_clean());
        inspect(&materialized, &mut root);
        if legacy {
            let source = assertion_ssa_functions(assertion_types(), vec![function])
                .into_source_owner()
                .unwrap();
            assert_eq!(
                source.semantic().semantic_sha256(),
                materialized
                    .semantic_ssa()
                    .source_semantic()
                    .semantic_sha256()
            );
            let receipt = ProductionRankedSemanticProjectionReceiptV1::from_unvalidated_projection_candidate_with_generated_effects(
                source, root.lowering, root.ranked_ir, root.access_sources, root.executable_effect_sources,
            ).unwrap();
            ProductionSemanticKirOwnerV1::try_lower_after_ranked_checks(
                receipt,
                ProductionSemanticKirLimitsV1::default(),
                1,
            )
        } else {
            ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(
                materialized_ranked_fixture_receipt_v1(materialized, root),
            )
        }
    }

    fn assert_private_write_row_v1(
        materialized: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
        root: &ProductionRankedRootProgramV1,
        statement: u32,
    ) {
        assert_eq!(root.access_sources.len(), 1);
        let row = root.access_sources[0];
        assert_eq!(
            (
                row.semantic_block(),
                row.semantic_statement(),
                row.semantic_access_ordinal()
            ),
            (0, Some(statement), 0)
        );
        assert!(matches!(
            root.lowering.kernel().blocks()[row.ranked_block() as usize].operations()
                [row.ranked_operation() as usize],
            ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Write,
                ..
            }
        ));
        let floor = materialized.executable_storage().retained_storage()
            + materialized.assert_origin_storage().payload_storage();
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, floor);
        budget.reserve_storage(floor).unwrap();
        assert_eq!(
            materialized.materialized_private_array_constant_index(
                ROOT,
                ROOT,
                fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
                    block: fe2o3_mir_model::SsaBlockIdV1::new(0),
                    statement,
                },
                fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::Destination,
                &mut budget,
            ),
            Ok(Some(0))
        );
        assert_eq!(budget.storage(), floor);
    }

    fn private_write_statements_v1(
        old: &SemanticFunctionDeclV1,
        statements: Vec<SemanticStatementV1>,
    ) -> SemanticFunctionDeclV1 {
        let mut blocks = old.blocks().to_vec();
        let entry = &blocks[0];
        blocks[0] = SemanticBasicBlockV1::new(
            entry.identity(),
            entry.source(),
            statements,
            entry.terminator().clone(),
        )
        .unwrap();
        SemanticFunctionDeclV1::new(
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
        .with_kernel_entry(old.kernel_entry().unwrap().clone())
    }

    #[test]
    fn private_array_indexed_write_attaches_through_both_existing_routes() {
        for legacy in [false, true] {
            attach_private_write_fixture_v1(
                literal_assertion(true, true, false),
                legacy,
                |materialized, root| assert_private_write_row_v1(materialized, root, 1),
            )
            .unwrap();
        }
    }

    #[test]
    fn private_array_indexed_write_reaches_exact_allocation_relation() {
        use fe2o3_lower_mir_kernel::{
            ProductionMirPlironTranslationErrorV1, ProductionSemanticKirErrorV1,
        };
        for legacy in [false, true] {
            let result = attach_private_write_fixture_v1(
                literal_assertion(true, true, false),
                legacy,
                |materialized, root| {
                    assert_private_write_row_v1(materialized, root, 1);
                    let kernel = root.lowering.kernel();
                    let mut changed = 0;
                    let blocks = kernel
                        .blocks()
                        .iter()
                        .map(|block| {
                            let mut operations = block.operations().to_vec();
                            for operation in &mut operations {
                                if let ProductionRankedOperationV1::ViewInSpace {
                                    memory_space: MemorySpaceAttr::Private,
                                    allocation_origin,
                                    noalias_class,
                                    ..
                                } = operation
                                {
                                    *allocation_origin += 1;
                                    *noalias_class += 1;
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
            assert!(
                matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
                        ProductionMirPlironTranslationErrorV1::AllocationOriginMismatch { .. }
                    ))
                ),
                "private-array origin substitution must reach the unchanged exact relation"
            );
        }
    }

    #[test]
    fn private_array_whole_initialization_keeps_its_distinct_refusal() {
        use fe2o3_lower_mir_kernel::{
            ProductionMirPlironTranslationErrorV1, ProductionSemanticKirErrorV1,
        };
        for legacy in [false, true] {
            let old = literal_assertion(true, true, false);
            let mut statements = old.blocks()[0].statements().to_vec();
            let SemanticStatementKindV1::Assign(write) = statements[1].kind() else {
                panic!("indexed write");
            };
            let local = write.destination().local();
            statements.insert(
                1,
                typed_assignment(
                    local.index(),
                    A_ARRAY,
                    SemanticRvalueKindV1::aggregate(
                        SemanticAggregateKindV1::Array,
                        vec![typed_constant(A_U32, 0, 4); 8],
                    )
                    .unwrap(),
                ),
            );
            let function = private_write_statements_v1(&old, statements);
            assert!(matches!(function.blocks()[0].statements()[1].kind(),
                SemanticStatementKindV1::Assign(value) if value.destination().projections().is_empty()
                    && matches!(value.value().kind(), SemanticRvalueKindV1::Aggregate(_))));
            let result = attach_private_write_fixture_v1(function, legacy, |materialized, root| {
                assert_private_write_row_v1(materialized, root, 2)
            });
            assert!(
                matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
                        ProductionMirPlironTranslationErrorV1::MissingRankedEffect {
                            semantic_block: 0,
                            semantic_statement: Some(1),
                            semantic_access_ordinal: 0,
                        }
                    ))
                ),
                "whole-array initialization has no indexed-write recipe"
            );
        }
    }

    #[test]
    fn private_array_rows_do_not_capture_promoted_or_scalar_slot_writes() {
        for retained_scalar in [false, true] {
            for legacy in [false, true] {
                let old = literal_assertion(true, true, false);
                let mut statements = vec![typed_assignment(
                    1,
                    A_BOOL,
                    SemanticRvalueKindV1::Use(typed_constant(A_BOOL, 1, 1)),
                )];
                if retained_scalar {
                    let pointer = old
                        .locals()
                        .iter()
                        .position(|local| local.ty() == A_BOOL_PTR)
                        .unwrap();
                    statements.push(typed_assignment(
                        pointer as u32,
                        A_BOOL_PTR,
                        SemanticRvalueKindV1::AddressOf {
                            mutability: SemanticMutabilityV1::Mutable,
                            place: typed_place(1, A_BOOL),
                        },
                    ));
                }
                let prefix = statements.len() as u32;
                statements.extend_from_slice(old.blocks()[0].statements());
                let function = private_write_statements_v1(&old, statements);
                attach_private_write_fixture_v1(function, legacy, |materialized, root| {
                    assert_private_write_row_v1(materialized, root, prefix + 1);
                    let promoted = materialized
                        .semantic_ssa()
                        .plan_for_function(ROOT)
                        .unwrap()
                        .plan()
                        .promoted_variables()
                        .iter()
                        .any(|variable| variable.get() == 1);
                    assert_eq!(promoted, !retained_scalar);
                    let scalar_slots = materialized
                        .executable()
                        .module()
                        .functions
                        .iter()
                        .filter_map(|function| function.body.as_ref())
                        .flat_map(|body| &body.blocks)
                        .flat_map(|block| &block.operations)
                        .filter(|operation| {
                            matches!(
                                operation.kind,
                                fe2o3_kernel_ir::OperationKind::Alloca {
                                    count: None,
                                    address_space: fe2o3_kernel_ir::AddressSpace::Private,
                                    ..
                                }
                            )
                        })
                        .count();
                    assert_eq!(scalar_slots, usize::from(retained_scalar));
                })
                .unwrap();
            }
        }
    }

    struct PrivateSourceMeterV1<'a, 'w> {
        budget: &'a mut Budget<'w>,
        calls: usize,
    }

    #[test]
    fn private_array_mixed_read_write_never_advertises_a_write_ordinal() {
        use fe2o3_lower_mir_kernel::{
            ProductionMirPlironTranslationErrorV1, ProductionSemanticKirErrorV1,
        };
        for legacy in [false, true] {
            let old = literal_assertion(true, true, false);
            let mut statements = old.blocks()[0].statements().to_vec();
            let SemanticStatementKindV1::Assign(write) = statements[1].kind() else {
                panic!("indexed write");
            };
            let place = write.destination().clone();
            statements[1] = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place.clone(),
                SemanticRvalueV1::new(
                    A_U32,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place.clone())),
                ),
            )));
            statements.insert(
                1,
                typed_assignment(
                    place.local().index(),
                    A_ARRAY,
                    SemanticRvalueKindV1::aggregate(
                        SemanticAggregateKindV1::Array,
                        vec![typed_constant(A_U32, 0, 4); 8],
                    )
                    .unwrap(),
                ),
            );
            let function = private_write_statements_v1(&old, statements);
            let result = attach_private_write_fixture_v1(function, legacy, |materialized, root| {
                assert!(root.access_sources.is_empty());
                let floor = materialized.executable_storage().retained_storage()
                    + materialized.assert_origin_storage().payload_storage();
                let mut work = Work::new(1_000_000);
                let mut budget = Budget::new(&mut work, floor);
                budget.reserve_storage(floor).unwrap();
                for role in [
                    fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::RvalueOperand(0),
                    fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::Destination,
                ] {
                    assert_eq!(
                        materialized.materialized_private_array_constant_index(
                            ROOT,
                            ROOT,
                            fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
                                block: fe2o3_mir_model::SsaBlockIdV1::new(0),
                                statement: 2,
                            },
                            role,
                            &mut budget,
                        ),
                        Ok(Some(0))
                    );
                }
                assert_eq!(budget.storage(), floor);
                let memory = materialized
                    .executable()
                    .module()
                    .functions
                    .iter()
                    .filter_map(|function| function.body.as_ref())
                    .flat_map(|body| &body.blocks)
                    .flat_map(|block| &block.operations)
                    .filter_map(|operation| match operation.kind {
                        fe2o3_kernel_ir::OperationKind::Load { .. } => Some(AccessKindAttr::Read),
                        fe2o3_kernel_ir::OperationKind::Store { .. } => Some(AccessKindAttr::Write),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let mut expected = vec![AccessKindAttr::Write; 8];
                expected.extend([AccessKindAttr::Read, AccessKindAttr::Write]);
                assert_eq!(memory, expected);

                // With this freshly admitted mixed source, even a component
                // roster with an independently tagged global RHS read must
                // not advertise the private destination at ordinal zero.
                let source = materialized.semantic_ssa().source_semantic();
                let value = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0));
                let blocks = [ProductionRankedBlockV1::new(
                    vec![
                        ProductionRankedOperationV1::Access {
                            kind: AccessKindAttr::Write,
                            view: value,
                            indices: vec![value],
                        },
                        ProductionRankedOperationV1::Access {
                            kind: AccessKindAttr::Read,
                            view: value,
                            indices: vec![value],
                        },
                    ],
                    ProductionRankedTerminatorV1::Return,
                )];
                let sources = [MemorySpaceAttr::Private, MemorySpaceAttr::Global]
                    .into_iter()
                    .enumerate()
                    .map(|(operation, memory_space)| ProjectedAccessSourceV1 {
                        block: 0,
                        operation,
                        memory_space,
                        access: if operation == 0 {
                            AccessKindAttr::Write
                        } else {
                            AccessKindAttr::Read
                        },
                        source: SemanticSourceProvenanceV1::unavailable(),
                        semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                            block: 0,
                            statement: Some(2),
                        }),
                    })
                    .collect::<Vec<_>>();
                let mut facts = PrivateSourceMeterV1 {
                    budget: &mut budget,
                    calls: 0,
                };
                let rows = production_access_sources(
                    source.types(),
                    &source.functions()[0],
                    &blocks,
                    &sources,
                    &mut facts,
                )
                .unwrap();
                assert_eq!(facts.calls, 1);
                assert_eq!(rows.len(), 1);
                assert_eq!(
                    (
                        rows[0].ranked_operation(),
                        rows[0].semantic_access_ordinal()
                    ),
                    (1, 0)
                );
            });
            // The actual whole-array initialization still fails first. The
            // no-write-row assertion above independently tests the mixed RHS
            // exclusion; this is not a claim to reach a later ordinal error.
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
                    ProductionMirPlironTranslationErrorV1::MissingRankedEffect {
                        semantic_block: 0,
                        semantic_statement: Some(1),
                        semantic_access_ordinal: 0,
                    }
                ))
            ));
        }
    }

    impl ProjectedAssertionFactsV1 for PrivateSourceMeterV1<'_, '_> {
        fn charge_private_array_work(
            &mut self,
            amount: usize,
        ) -> Result<(), ProductionRankedProjectionErrorV1> {
            assert_eq!(amount, 40);
            self.calls += 1;
            self.budget
                .charge_work(amount)
                .map_err(ranked_projection_source_v1::resource)
        }

        fn is_materialized_block(
            &mut self,
            _: usize,
        ) -> Result<bool, ProductionRankedProjectionErrorV1> {
            panic!("source-row classification must not query a CFG");
        }

        fn condition(
            &mut self,
            _: usize,
            _: bool,
            _: SemanticBlockIdV1,
        ) -> Result<
            canonical_assertion_facts_v1::ProjectedAssertionConditionV1,
            ProductionRankedProjectionErrorV1,
        > {
            panic!("source-row classification must not query an assertion");
        }
    }

    #[test]
    fn private_array_source_row_component_prepays_exact_candidate_bound() {
        // Inert row/operation components with genuine source syntax; this is
        // not a checked graph or a full-engine budget threshold.
        let function = literal_assertion(true, true, false);
        let types = assertion_types();
        let value = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0));
        let blocks = vec![ProductionRankedBlockV1::new(
            vec![ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Write,
                view: value,
                indices: vec![value],
            }],
            ProductionRankedTerminatorV1::Return,
        )];
        for limit in [39, 40] {
            for statement in [1, usize::MAX] {
                let sources = [ProjectedAccessSourceV1 {
                    block: 0,
                    operation: 0,
                    access: AccessKindAttr::Write,
                    memory_space: MemorySpaceAttr::Private,
                    source: SemanticSourceProvenanceV1::unavailable(),
                    semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                        block: 0,
                        statement: Some(statement),
                    }),
                }];
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, 17);
                budget.reserve_storage(17).unwrap();
                let mut facts = PrivateSourceMeterV1 {
                    budget: &mut budget,
                    calls: 0,
                };
                let result =
                    production_access_sources(&types, &function, &blocks, &sources, &mut facts);
                assert_eq!(facts.calls, 1);
                if limit == 39 {
                    assert!(
                        matches!(result, Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                        CanonicalAssertionErrorV1::Resource(Resource::Work(error))
                    )) if error.actual() == 40 && error.limit() == 39)
                    );
                } else {
                    assert_eq!(result.unwrap().len(), usize::from(statement == 1));
                    assert_eq!(facts.budget.work(), 40);
                }
                assert_eq!(facts.budget.storage(), 17);
            }
        }
    }

    #[test]
    fn private_array_source_row_component_keeps_omissions_and_dense_ordinals() {
        let function = literal_assertion(true, true, false);
        let types = assertion_types();
        let value = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0));
        let blocks = vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Read,
                    view: value,
                    indices: vec![value],
                },
                ProductionRankedOperationV1::PredicatedAccess {
                    kind: AccessKindAttr::Write,
                    view: value,
                    index: value,
                    success: value,
                },
                ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Write,
                    view: value,
                    indices: vec![value],
                },
                ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Write,
                    view: value,
                    indices: vec![value],
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )];
        let sources = (0..4)
            .map(|operation| ProjectedAccessSourceV1 {
                block: 0,
                operation,
                access: if operation == 0 {
                    AccessKindAttr::Read
                } else {
                    AccessKindAttr::Write
                },
                memory_space: MemorySpaceAttr::Private,
                source: SemanticSourceProvenanceV1::unavailable(),
                semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                    block: 0,
                    statement: Some(1),
                }),
            })
            .collect::<Vec<_>>();
        let mut work = Work::new(80);
        let mut budget = Budget::new(&mut work, 0);
        let mut facts = PrivateSourceMeterV1 {
            budget: &mut budget,
            calls: 0,
        };
        let rows =
            production_access_sources(&types, &function, &blocks, &sources, &mut facts).unwrap();
        assert_eq!(facts.calls, 2);
        assert_eq!(facts.budget.work(), 80);
        assert_eq!(
            rows.iter()
                .map(|row| (row.ranked_operation(), row.semantic_access_ordinal()))
                .collect::<Vec<_>>(),
            vec![(2, 0), (3, 1)]
        );
        // Duplicate inert rows exercise the converter's existing ordinal rule,
        // not a claim that two such source effects pass translation validation.
    }

    #[test]
    fn same_legacy_owner_preserves_graph_and_canonical_buffers_through_projection_and_attachment() {
        let materialized = collective_assertion_materialized_v1();
        with_canonical_assertions_v1(&materialized, |session| {
            let mut facts = session.for_source(ROOT, ROOT);
            assert_eq!(
                facts.condition(3, true, SemanticBlockIdV1::from_index(4))?,
                ProjectedAssertionConditionV1::Bool(true),
            );
            Ok(())
        })
        .unwrap();
        let graph = materialized.executable();
        let identity = *graph.canonical().identity();
        let functions = graph.module().functions.as_ptr();
        let canonical_buffer = graph.canonical().canonical_bytes().as_ptr();
        let graph_storage = materialized.executable_storage();
        let origins_storage = materialized.assert_origin_storage();
        let program = project_and_verify_ranked_materialized_semantic_mir_v1(
            materialized,
            &[ranked_root_input_1d("neutral_generated_hostile", 247, 64)],
            &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
        )
        .unwrap();
        assert_eq!(program.root_count(), 1);
        assert!(program.all_kernel_checks_are_clean());
        assert!(!program.grants_artifact_or_launch_authority());
        assert_eq!(
            program
                .materialized
                .executable()
                .module()
                .functions
                .as_ptr(),
            functions
        );
        assert_eq!(
            program
                .materialized
                .executable()
                .canonical()
                .canonical_bytes()
                .as_ptr(),
            canonical_buffer
        );
        let ProductionRankedSemanticProgramV1 {
            materialized,
            roots,
        } = program;
        let root = roots.into_vec().into_iter().next().unwrap();
        assert!(root.lowering.all_mandatory_reports_are_clean());
        // This source/ranked receipt is the existing checked-attachment input,
        // not a fabricated functional-verification or publication receipt.
        let receipt = materialized_ranked_fixture_receipt_v1(materialized, root);
        let attached = fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(receipt).unwrap();
        let retained = attached.pre_ranked_executable().unwrap();
        assert_eq!(retained.canonical().identity(), &identity);
        assert_eq!(retained.module().functions.as_ptr(), functions);
        assert_eq!(
            retained.canonical().canonical_bytes().as_ptr(),
            canonical_buffer
        );
        assert!(std::ptr::eq(attached.module(), retained.module()));
        assert!(std::ptr::eq(
            attached.pre_ranked_assert_origins().unwrap().executable(),
            retained
        ));
        assert_eq!(
            attached.pre_ranked_executable_storage(),
            Some(graph_storage)
        );
        assert_eq!(
            attached.pre_ranked_assert_origin_storage(),
            Some(origins_storage)
        );
    }

    #[test]
    fn private_array_missing_source_row_remains_rejected_by_both_attachment_routes() {
        use fe2o3_lower_mir_kernel::{
            ProductionMirPlironTranslationErrorV1, ProductionRankedSemanticProjectionReceiptV1,
            ProductionSemanticKirErrorV1, ProductionSemanticKirLimitsV1,
            ProductionSemanticKirOwnerV1,
        };

        for legacy in [false, true] {
            let function = literal_assertion(true, true, false);
            let program = assertion_project(assertion_materialized(function.clone())).unwrap();
            let ProductionRankedSemanticProgramV1 {
                materialized,
                roots,
            } = program;
            let mut root = roots.into_vec().into_iter().next().unwrap();
            assert_private_write_row_v1(&materialized, &root, 1);
            // Preserve the original exact rejection as a missing-row mutation,
            // rather than treating all private stores as compiler-owned noise.
            root.access_sources.clear();
            let result = if legacy {
                let source = assertion_ssa_functions(assertion_types(), vec![function])
                    .into_source_owner()
                    .unwrap();
                assert_eq!(
                    source.semantic().semantic_sha256(),
                    materialized
                        .semantic_ssa()
                        .source_semantic()
                        .semantic_sha256(),
                );
                let receipt = ProductionRankedSemanticProjectionReceiptV1::from_unvalidated_projection_candidate_with_generated_effects(
                    source, root.lowering, root.ranked_ir, root.access_sources,
                    root.executable_effect_sources,
                )
                .unwrap();
                ProductionSemanticKirOwnerV1::try_lower_after_ranked_checks(
                    receipt,
                    ProductionSemanticKirLimitsV1::default(),
                    1,
                )
            } else {
                ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(
                    materialized_ranked_fixture_receipt_v1(materialized, root),
                )
            };
            assert!(
                matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
                        ProductionMirPlironTranslationErrorV1::MissingRankedEffect {
                            semantic_block: 0,
                            semantic_statement: Some(1),
                            semantic_access_ordinal: 0,
                        }
                    ))
                ),
                "private-array attachment unexpectedly changed (legacy={legacy})"
            );
        }
    }

    #[test]
    fn legacy_scope_requires_both_borrowed_payloads_before_any_callback_or_work() {
        let source = assertion_materialized(literal_assertion(true, true, false));
        let graph = source.executable_storage().retained_storage();
        let origins = source.assert_origin_storage().payload_storage();
        assert!(origins > 0);
        for floor in [0, graph, graph + origins - 1] {
            let mut work = Work::new(7);
            let mut budget = Budget::new(&mut work, floor);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(floor).unwrap();
            let result = with_canonical_assertions_budget_v1(
                &source,
                &mut budget,
                |_| -> Result<(), ProductionRankedProjectionErrorV1> {
                    panic!("an underreserved owner must not reach analysis or the callback")
                },
            );
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(Resource::Accounting)
                ))
            ));
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (7, floor, floor)
            );
        }
    }

    #[test]
    fn legacy_unwind_restores_the_floor_and_preserves_history_and_original_payload() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        struct BodyDrop(Arc<AtomicUsize>);
        impl Drop for BodyDrop {
            fn drop(&mut self) {
                assert_eq!(self.0.swap(1, Ordering::SeqCst), 0);
            }
        }
        struct Payload(Arc<AtomicUsize>);
        impl Drop for Payload {
            fn drop(&mut self) {
                assert_eq!(self.0.swap(2, Ordering::SeqCst), 1);
            }
        }
        let source = assertion_materialized(literal_assertion(true, true, false));
        let executable = source.executable() as *const _;
        let floor = source.executable_storage().retained_storage()
            + source.assert_origin_storage().payload_storage()
            + 31;
        let work_limit =
            usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap();
        let storage_limit = crate::production_canonical_phase_policy_v1::STORAGE_LIMIT;
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(floor).unwrap();
        // These deliberately oversized attempts seed history, not a measured
        // threshold for the later inventory/sparse scope.
        let failed_work = match budget.charge_work(work_limit) {
            Err(Resource::Work(limit)) => limit.actual(),
            other => panic!("expected oversized work failure, got {other:?}"),
        };
        assert!(matches!(
            budget.reserve_storage(storage_limit),
            Err(Resource::Storage(_))
        ));
        let failed_storage = floor + storage_limit;
        budget.reserve_storage(storage_limit - floor).unwrap();
        budget.release_storage(storage_limit - floor).unwrap();
        let order = Arc::new(AtomicUsize::new(0));
        let payload = Box::new(Payload(Arc::clone(&order)));
        let payload_address = (&*payload as *const Payload) as usize;
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<(), ProductionRankedProjectionErrorV1> =
                with_canonical_assertions_budget_v1(&source, &mut budget, |session| {
                    let _drop_before_resume = BodyDrop(Arc::clone(&order));
                    assert!(session.retained_floor_for_test_v1() > floor);
                    let mut facts = session.for_source(ROOT, ROOT);
                    assert_eq!(
                        facts.condition(0, true, SemanticBlockIdV1::from_index(1))?,
                        ProjectedAssertionConditionV1::Bool(true)
                    );
                    std::panic::panic_any(payload)
                });
        }))
        .expect_err("the original panic must propagate out of the private scope");
        let payload = *caught
            .downcast::<Box<Payload>>()
            .expect("original payload type");
        assert_eq!((&*payload as *const Payload) as usize, payload_address);
        assert_eq!(order.load(Ordering::SeqCst), 1);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), storage_limit);
        assert_eq!(budget.failed_storage(), Some(failed_storage));
        assert!(budget.work() > 7);
        assert_eq!(source.executable() as *const _, executable);
        source.semantic_ssa().verify_replay().unwrap();
        drop(payload);
        assert_eq!(order.load(Ordering::SeqCst), 2);
        assert_eq!(work.failed_work(), Some(failed_work));
    }
}
