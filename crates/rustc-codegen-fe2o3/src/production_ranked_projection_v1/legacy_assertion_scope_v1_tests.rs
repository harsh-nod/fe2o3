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
            let _owner = attach_private_write_fixture_v1(
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
                let _owner = attach_private_write_fixture_v1(function, legacy, |materialized, root| {
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

    fn ordered_effect_source_v1(
        store: bool,
        moved: bool,
        alias: bool,
        binary: bool,
    ) -> (
        Vec<SemanticTypeDeclV1>,
        SemanticFunctionDeclV1,
        Vec<SemanticCallableDeclV1>,
    ) {
        let mut types = assertion_types();
        types[A_BOOL_PTR.index() as usize] = neutral_pointer_type_v1(
            235,
            A_U32,
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Mutable,
            0,
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
                None,
            ),
        );
        let witness = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(237)),
            SemanticLayoutIdentityV1::from_sha256(bytes(237)),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(8),
                8,
                *types[A_U64.index() as usize].layout().backend_repr(),
                false,
                SemanticAggregateLayoutV1::new(vec![0, 0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![A_U64, A_UNIT]).unwrap(),
            ),
        ));
        let dereference = |local| {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(local),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, A_U32)
                        .unwrap(),
                ],
                A_U32,
            )
            .unwrap()
        };
        let operand = |local| {
            if moved {
                SemanticOperandV1::Move(dereference(local))
            } else {
                SemanticOperandV1::Copy(dereference(local))
            }
        };
        let mut statements = Vec::new();
        if alias {
            statements.push(typed_assignment(
                3,
                A_BOOL_PTR,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], A_BOOL_PTR)
                        .unwrap(),
                )),
            ));
        }
        let destination = dereference(if alias { 3 } else { 2 });
        statements.push(if store {
            assert!(!binary);
            statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                destination,
                operand(1),
                SemanticVolatilityV1::NonVolatile,
                None,
            )))
        } else {
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                destination,
                SemanticRvalueV1::new(
                    A_U32,
                    if binary {
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::BitXor,
                            left: operand(1),
                            right: operand(2),
                        }
                    } else {
                        SemanticRvalueKindV1::Use(operand(1))
                    },
                ),
            )))
        });
        let original = assertion_root_with_access(
            vec![
                (A_UNIT, SemanticLocalRoleV1::Return),
                (A_BOOL_PTR, SemanticLocalRoleV1::Argument(0)),
                (A_BOOL_PTR, SemanticLocalRoleV1::Argument(1)),
                (A_BOOL_PTR, SemanticLocalRoleV1::Temporary),
                (witness, SemanticLocalRoleV1::Temporary),
            ],
            vec![A_BOOL_PTR, A_BOOL_PTR],
            vec![
                block(
                    220,
                    vec![],
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable(
                            SemanticCallableIdV1::from_index(1),
                            vec![],
                            Some(SemanticCallDestinationV1::new(
                                SemanticPlaceV1::new(
                                    SemanticLocalIdV1::from_index(4),
                                    vec![],
                                    witness,
                                )
                                .unwrap(),
                                cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
                            )),
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    ),
                ),
                block(221, statements, SemanticTerminatorKindV1::Return),
            ],
            false,
        );
        let dimensions = SemanticWorkgroupDimensionsV1::new([1, 1, 1]).unwrap();
        let function = SemanticFunctionDeclV1::new(
            original.identity(),
            original.role(),
            original.item_definition_identity(),
            original.monomorphization_identity(),
            original.generic_type_arguments_identity(),
            original.const_generic_arguments_identity(),
            original.source(),
            original
                .abi()
                .clone()
                .with_source_argument_ownership(vec![
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner;
                2
            ])
                .unwrap(),
            original.locals().to_vec(),
            original.entry(),
            original.blocks().to_vec(),
        )
        .unwrap()
        .with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(A_NAME.as_bytes().to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256(bytes(247)),
            SemanticKernelSourceContractV1::new(
                Some(
                    SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                        .unwrap(),
                ),
                None,
                None,
            )
            .unwrap(),
        ));
        let intrinsic_abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(bytes(248)),
            SemanticLayoutIdentityV1::from_sha256(bytes(250)),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            0,
            vec![],
            neutral_plain_direct_abi_value_v1(witness),
        )
        .unwrap();
        let callables = vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::CompilerIntrinsic {
                binding: SemanticNonBodyCallableBindingV1::new(
                    SemanticFunctionIdentityV1::from_sha256(bytes(248)),
                    SemanticItemDefinitionIdentityV1::from_sha256(bytes(248)),
                    SemanticMonomorphizationIdentityV1::from_sha256(bytes(248)),
                    SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(248)),
                    SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(248)),
                    SemanticSourceProvenanceV1::unavailable(),
                    intrinsic_abi,
                ),
                operation: SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                    index_witness: witness,
                    raw_index: A_U64,
                },
                operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(bytes(248)),
            },
        ];
        (types, function, callables)
    }

    fn ordered_effect_ssa_v1(
        types: Vec<SemanticTypeDeclV1>,
        function: SemanticFunctionDeclV1,
        callables: Vec<SemanticCallableDeclV1>,
    ) -> ProductionSemanticSsaOwnerV1 {
        let admitted = InertSemanticMirRequestV1::new_with_callables(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
            types,
            vec![],
            vec![],
            vec![],
            vec![function],
            callables,
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        let semantic = ProductionSemanticMirOwnerV1::try_new(
            admitted,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        ProductionSemanticSsaOwnerV1::try_new(
            semantic,
            fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap()
    }

    fn ordered_effect_attach_v1(
        store: bool,
        moved: bool,
        alias: bool,
        binary: bool,
        legacy: bool,
        reverse_rows: bool,
    ) -> Result<
        fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
        fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1,
    > {
        use fe2o3_lower_mir_kernel::{
            ProductionRankedSemanticProjectionReceiptV1, ProductionSemanticKirLimitsV1,
            ProductionSemanticKirOwnerV1,
        };
        let (types, function, callables) = ordered_effect_source_v1(store, moved, alias, binary);
        let inputs = [ranked_root_input_1d(A_NAME, 247, 1)];
        let ssa = ordered_effect_ssa_v1(types.clone(), function.clone(), callables.clone());
        let materialized = materialize_ranked_fixture_v1(ssa, &inputs).unwrap();
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
        let mut expected = vec![AccessKindAttr::Read; if binary { 2 } else { 1 }];
        expected.push(AccessKindAttr::Write);
        assert_eq!(memory, expected, "actual executable source effect order");
        let program = project_and_verify_ranked_materialized_semantic_mir_v1(
            materialized,
            &inputs,
            &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
        )
        .unwrap();
        let ProductionRankedSemanticProgramV1 {
            materialized,
            roots,
        } = program;
        let mut root = roots.into_vec().into_iter().next().unwrap();
        assert!(root.lowering.all_mandatory_reports_are_clean());
        assert_eq!(root.access_sources.len(), expected.len());
        let mut views = Vec::new();
        for (ordinal, (row, expected_kind)) in root.access_sources.iter().zip(&expected).enumerate()
        {
            assert_eq!(
                (
                    row.semantic_block(),
                    row.semantic_statement(),
                    row.semantic_access_ordinal()
                ),
                (1, Some(u32::from(alias)), ordinal as u32)
            );
            let ProductionRankedOperationV1::Access { kind, view, .. } =
                &root.lowering.kernel().blocks()[row.ranked_block() as usize].operations()
                    [row.ranked_operation() as usize]
            else {
                panic!("ordinary source effect row");
            };
            assert_eq!(kind, expected_kind);
            views.push(*view);
        }
        if alias {
            let origins = views
                .iter()
                .map(|view| {
                    root.lowering
                        .kernel()
                        .blocks()
                        .iter()
                        .flat_map(|block| block.operations())
                        .find_map(|operation| match operation {
                            ProductionRankedOperationV1::ViewInSpace {
                                result,
                                allocation_origin,
                                ..
                            } if *view == ProductionRankedValueV1::Local(*result) => {
                                Some(*allocation_origin)
                            }
                            _ => None,
                        })
                        .unwrap()
                })
                .collect::<Vec<_>>();
            assert_eq!(origins[0], *origins.last().unwrap(), "actual alias origin");
        }
        if reverse_rows {
            assert_eq!(root.access_sources.len(), 2);
            let old = root.access_sources.clone();
            for (ordinal, row) in root.access_sources.iter_mut().enumerate() {
                let wrong = old[1 - ordinal];
                *row = fe2o3_lower_mir_kernel::ProductionRankedAccessSourceV1::new(
                    row.semantic_block(),
                    row.semantic_statement(),
                    row.semantic_access_ordinal(),
                    wrong.ranked_block(),
                    wrong.ranked_operation(),
                );
            }
        }
        if legacy {
            let source = ordered_effect_ssa_v1(types, function, callables)
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

    #[test]
    fn source_effect_order_read_before_write_attaches_for_assign_store_copy_move_and_aliases() {
        for legacy in [false, true] {
            for store in [false, true] {
                for moved in [false, true] {
                    for alias in [false, true] {
                        let _owner =
                            ordered_effect_attach_v1(store, moved, alias, false, legacy, false)
                                .unwrap();
                    }
                }
            }
        }
    }

    #[test]
    fn source_effect_order_binary_rhs_keeps_both_reads_before_destination() {
        for legacy in [false, true] {
            let _owner =
                ordered_effect_attach_v1(false, false, false, true, legacy, false).unwrap();
        }
    }

    #[test]
    fn source_effect_order_final_correspondence_rejects_write_first_rows() {
        use fe2o3_lower_mir_kernel::{
            ProductionMirPlironTranslationErrorV1, ProductionSemanticKirErrorV1,
        };
        for legacy in [false, true] {
            assert!(matches!(
                ordered_effect_attach_v1(false, false, true, false, legacy, true),
                Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
                    ProductionMirPlironTranslationErrorV1::AccessKindMismatch { .. }
                ))
            ));
        }
    }

    #[test]
    fn source_effect_order_private_uninitialized_rhs_precedes_and_prevents_assignment() {
        use fe2o3_lower_mir_kernel::{
            ProductionPreRankedKirErrorV1, ProductionPreRankedKirOwnerV1,
            ProductionSemanticKirErrorV1, ProductionSemanticKirLimitsV1,
        };
        for store in [false, true] {
            let old = literal_assertion(true, true, false);
            let mut statements = old.blocks()[0].statements().to_vec();
            let SemanticStatementKindV1::Assign(write) = statements[1].kind() else {
                panic!("existing indexed array destination");
            };
            let destination = write.destination().clone();
            let value = SemanticOperandV1::Copy(destination.clone());
            statements[1] = statement(if store {
                SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                    destination,
                    value,
                    SemanticVolatilityV1::NonVolatile,
                    None,
                ))
            } else {
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    destination,
                    SemanticRvalueV1::new(A_U32, SemanticRvalueKindV1::Use(value)),
                ))
            });
            let function = private_write_statements_v1(&old, statements);
            let ssa = assertion_ssa_functions(assertion_types(), vec![function]);
            // Observe the general projector on the actual admitted source,
            // independently of the materializer's earlier initialization gate.
            let semantic = ssa.source_semantic();
            let function = &semantic.functions()[0];
            let constants = constant_locals(function).unwrap();
            let effects = derive_defined_callable_empty_effect_summaries_v1(
                semantic.types(),
                semantic.functions(),
                semantic.callables(),
            )
            .unwrap();
            let mut entry_operations = Vec::new();
            let mut next_value = 0;
            let mut text = String::new();
            let intrinsic = project_intrinsic_contracts(
                semantic.callables(),
                &effects,
                semantic.types(),
                function,
                Some(64),
                &constants,
                &mut entry_operations,
                &mut next_value,
                &mut text,
            )
            .unwrap();
            let first_value = next_value;
            let mut operations = Vec::new();
            let mut sources = Vec::new();
            let mut guarded = Vec::new();
            let mut views = ProjectedViewsV1::new(function.locals().len(), None);
            project_statement_accesses(
                semantic.types(),
                function,
                0,
                &[],
                &function.blocks()[0].statements()[1],
                &constants,
                &intrinsic.local_contracts,
                &intrinsic.guarded_accesses,
                &mut guarded,
                &mut views,
                &mut operations,
                &mut sources,
                &mut next_value,
                &mut text,
            )
            .unwrap();
            assert!(guarded.is_empty());
            assert_eq!(
                access_kinds(&operations),
                vec![AccessKindAttr::Read, AccessKindAttr::Write]
            );
            assert_eq!(
                sources
                    .iter()
                    .map(|source| source.access)
                    .collect::<Vec<_>>(),
                vec![AccessKindAttr::Read, AccessKindAttr::Write]
            );
            assert!(
                sources
                    .iter()
                    .all(|source| source.memory_space == MemorySpaceAttr::Private)
            );
            assert_eq!(
                (operations.len(), sources.len(), next_value - first_value),
                (5, 2, 3)
            );
            let inputs = [ranked_root_input_1d(A_NAME, 247, 64)];
            let launch = source_launch_roster_for_ranked_inputs_v1(&ssa, &inputs).unwrap();
            let mut work = Work::new(1_000_000);
            let mut budget = Budget::new(&mut work, 1_000_000);
            assert!(matches!(
                ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
                    ssa,
                    launch,
                    ProductionSemanticKirLimitsV1::default(),
                    &mut budget,
                ),
                Err(ProductionPreRankedKirErrorV1::Lowering(
                    ProductionSemanticKirErrorV1::Unsupported {
                        function: 0,
                        block: Some(0),
                        statement: Some(1),
                        detail: "retained array read requires whole-array initialization",
                    }
                ))
            ));
        }
    }

    // Genuine source/canonical-owner composition, not the cursor's RecordingFacts model.
    const COMPOSITION_SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
    const COMPOSITION_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);
    const COMPOSITION_WRAPPER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);

    #[derive(Clone, Copy, Debug)]
    enum PrivateSliceCompositionV1 {
        Read,
        TwoReads,
        ProjectedPrivateWrite,
    }

    fn private_slice_composition_types_v1() -> Vec<SemanticTypeDeclV1> {
        use fe2o3_mir_model::semantic_mir_v1::*;
        let mut types = assertion_types();
        assert_eq!(types.len(), COMPOSITION_SLICE.index() as usize);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([237; 32]),
            SemanticLayoutIdentityV1::from_sha256([237; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                4,
                SemanticFieldsShapeV1::array(4, 0),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(false),
                None,
                false,
                None,
                4,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Slice { element: A_U32 },
        ));
        let pair = SemanticBackendReprV1::scalar_pair(
            SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            ),
            SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            ),
        );
        types.push(
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([238; 32]),
                SemanticLayoutIdentityV1::from_sha256([238; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(Some(16), 8, pair, false).unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        COMPOSITION_SLICE,
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Immutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::SliceLength,
                    )
                    .unwrap(),
                ),
            )
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                    Some(
                        SemanticAbiPointeeInfoV1::new(
                            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                            0,
                            4,
                        )
                        .unwrap(),
                    ),
                    None,
                ),
            ),
        );
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([239; 32]),
            SemanticLayoutIdentityV1::from_sha256([239; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(16),
                8,
                pair,
                false,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(
                SemanticAggregateTypeV1::new(vec![COMPOSITION_REF]).unwrap(),
            ),
        ));
        types
    }

    fn private_slice_composition_source_v1(
        mode: PrivateSliceCompositionV1,
    ) -> SemanticFunctionDeclV1 {
        use fe2o3_mir_model::semantic_mir_v1::*;
        let field = || {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(2),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), COMPOSITION_REF)
                        .unwrap(),
                ],
                COMPOSITION_REF,
            )
            .unwrap()
        };
        let slice_read = || {
            SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(2),
                    vec![
                        SemanticProjectionV1::new(
                            SemanticProjectionKindV1::Field(0),
                            COMPOSITION_REF,
                        )
                        .unwrap(),
                        SemanticProjectionV1::new(
                            SemanticProjectionKindV1::Dereference,
                            COMPOSITION_SLICE,
                        )
                        .unwrap(),
                        SemanticProjectionV1::new(
                            SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(4)),
                            A_U32,
                        )
                        .unwrap(),
                    ],
                    A_U32,
                )
                .unwrap(),
            )
        };
        let private_place = || {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(7),
                vec![
                    SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(4)),
                        A_U32,
                    )
                    .unwrap(),
                ],
                A_U32,
            )
            .unwrap()
        };
        let later = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            if matches!(mode, PrivateSliceCompositionV1::ProjectedPrivateWrite) {
                private_place()
            } else {
                typed_place(6, A_U32)
            },
            SemanticRvalueV1::new(
                A_U32,
                if matches!(mode, PrivateSliceCompositionV1::TwoReads) {
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::BitXor,
                        left: slice_read(),
                        right: slice_read(),
                    }
                } else {
                    SemanticRvalueKindV1::Use(slice_read())
                },
            ),
        )));
        let old = assertion_root_with_access(
            vec![
                (A_UNIT, SemanticLocalRoleV1::Return),
                (COMPOSITION_REF, SemanticLocalRoleV1::Argument(0)),
                (COMPOSITION_WRAPPER, SemanticLocalRoleV1::Temporary),
                (A_U64, SemanticLocalRoleV1::Temporary),
                (A_U64, SemanticLocalRoleV1::Temporary),
                (A_BOOL, SemanticLocalRoleV1::Temporary),
                (A_U32, SemanticLocalRoleV1::Temporary),
                (A_ARRAY, SemanticLocalRoleV1::Temporary),
            ],
            vec![],
            vec![
                block(
                    210,
                    vec![
                        typed_assignment(
                            2,
                            COMPOSITION_WRAPPER,
                            SemanticRvalueKindV1::aggregate(
                                SemanticAggregateKindV1::Tuple,
                                vec![SemanticOperandV1::Copy(typed_place(1, COMPOSITION_REF))],
                            )
                            .unwrap(),
                        ),
                        typed_assignment(
                            3,
                            A_U64,
                            SemanticRvalueKindV1::Unary {
                                operation: SemanticUnaryOpV1::PointerMetadata,
                                operand: SemanticOperandV1::Copy(field()),
                            },
                        ),
                        typed_assignment(
                            4,
                            A_U64,
                            SemanticRvalueKindV1::Use(typed_constant(A_U64, 0, 8)),
                        ),
                        typed_assignment(
                            5,
                            A_BOOL,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::LessThan,
                                left: SemanticOperandV1::Copy(typed_place(4, A_U64)),
                                right: SemanticOperandV1::Copy(typed_place(3, A_U64)),
                            },
                        ),
                    ],
                    SemanticTerminatorKindV1::Assert {
                        condition: SemanticOperandV1::Copy(typed_place(5, A_BOOL)),
                        expected: true,
                        message: SemanticAssertMessageV1::BoundsCheck {
                            length: SemanticOperandV1::Copy(typed_place(3, A_U64)),
                            index: SemanticOperandV1::Copy(typed_place(4, A_U64)),
                        },
                        target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(
                    211,
                    vec![
                        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                            private_place(),
                            SemanticRvalueV1::new(
                                A_U32,
                                SemanticRvalueKindV1::Use(typed_constant(A_U32, 17, 4)),
                            ),
                        ))),
                        later,
                    ],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
            false,
        );
        let first = SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(
                true,
                Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                true,
                true,
                false,
                true,
            ),
            SemanticAbiExtensionV1::None,
            0,
            Some(4),
        )
        .unwrap();
        let second = SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
            SemanticAbiExtensionV1::None,
            0,
            None,
        )
        .unwrap();
        let abi = SemanticFunctionAbiV1::from_rustc(
            old.abi().identity(),
            old.abi().layout_identity(),
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                COMPOSITION_REF,
                SemanticAbiPassModeV1::Pair { first, second },
            ))],
            SemanticAbiValueV1::new(A_UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow])
        .unwrap();
        SemanticFunctionDeclV1::new(
            old.identity(),
            old.role(),
            old.item_definition_identity(),
            old.monomorphization_identity(),
            old.generic_type_arguments_identity(),
            old.const_generic_arguments_identity(),
            old.source(),
            abi,
            old.locals().to_vec(),
            old.entry(),
            old.blocks().to_vec(),
        )
        .unwrap()
        .with_kernel_entry(old.kernel_entry().unwrap().clone())
    }

    fn private_slice_composition_attach_v1(
        mode: PrivateSliceCompositionV1,
        legacy: bool,
    ) -> Result<
        fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
        fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1,
    > {
        use fe2o3_lower_mir_kernel::{
            ProductionRankedSemanticProjectionReceiptV1, ProductionSemanticKirLimitsV1,
            ProductionSemanticKirOwnerV1,
        };
        let types = private_slice_composition_types_v1();
        let function = private_slice_composition_source_v1(mode);
        let ssa = assertion_ssa_functions(types.clone(), vec![function.clone()]);
        let materialized =
            materialize_ranked_fixture_v1(ssa, &[ranked_root_input_1d(A_NAME, 247, 64)]);
        let reads = if matches!(mode, PrivateSliceCompositionV1::TwoReads) {
            2
        } else {
            1
        };
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
        let mut expected_memory = vec![AccessKindAttr::Write];
        expected_memory.extend((0..reads).map(|_| AccessKindAttr::Read));
        if matches!(mode, PrivateSliceCompositionV1::ProjectedPrivateWrite) {
            expected_memory.push(AccessKindAttr::Write);
        }
        assert_eq!(
            memory, expected_memory,
            "actual N must evaluate the canonical RHS before a retained private destination"
        );
        // These are real owner/inventory/budget queries, not fabricated slice facts.
        with_canonical_assertions_v1(&materialized, |session| {
            let mut facts = session.for_source(ROOT, ROOT);
            for ordinal in 0..reads {
                let input = facts.slice_access(
                    ProjectedSemanticAccessSiteV1 {
                        block: 1,
                        statement: Some(1),
                    },
                    ordinal,
                    0,
                )?;
                assert_eq!(input.source_argument, 0);
                assert_eq!(input.element_width, 4);
            }
            assert!(
                facts
                    .slice_access(
                        ProjectedSemanticAccessSiteV1 {
                            block: 1,
                            statement: Some(1),
                        },
                        reads,
                        0
                    )
                    .is_err(),
                "the next occurrence is not another canonical read"
            );
            Ok(())
        })
        .unwrap();
        let floor = materialized.executable_storage().retained_storage()
            + materialized.assert_origin_storage().payload_storage();
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, floor);
        budget.reserve_storage(floor).unwrap();
        for statement in 0..=u32::from(matches!(
            mode,
            PrivateSliceCompositionV1::ProjectedPrivateWrite
        )) {
            assert_eq!(
                materialized.materialized_private_array_constant_index(
                    ROOT,
                    ROOT,
                    fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
                        block: fe2o3_mir_model::SsaBlockIdV1::new(1),
                        statement,
                    },
                    fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::Destination,
                    &mut budget,
                ),
                Ok(Some(0))
            );
        }
        assert_eq!(budget.storage(), floor);
        drop(budget);
        let program = assertion_project(materialized).unwrap();
        assert!(program.all_kernel_checks_are_clean());
        let ProductionRankedSemanticProgramV1 {
            materialized,
            roots,
        } = program;
        let root = roots.into_vec().into_iter().next().unwrap();
        assert!(root.lowering.all_mandatory_reports_are_clean());
        let rows = root
            .access_sources
            .iter()
            .map(|row| {
                let kind = match root.lowering.kernel().blocks()[row.ranked_block() as usize]
                    .operations()[row.ranked_operation() as usize]
                {
                    ProductionRankedOperationV1::Access { kind, .. } => kind,
                    _ => panic!("composition row must retain an ordinary memory effect"),
                };
                (
                    row.semantic_block(),
                    row.semantic_statement(),
                    row.semantic_access_ordinal(),
                    kind,
                )
            })
            .collect::<Vec<_>>();
        let mut expected = vec![(1, Some(0), 0, AccessKindAttr::Write)];
        expected.extend((0..reads).map(|ordinal| (1, Some(1), ordinal, AccessKindAttr::Read)));
        assert_eq!(
            rows, expected,
            "private write does not shift a later site's canonical read cursor"
        );
        if legacy {
            let source = assertion_ssa_functions(types, vec![function])
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

    #[test]
    fn private_write_then_canonical_slice_read_resets_the_real_site_cursor() {
        for legacy in [false, true] {
            let _owner =
                private_slice_composition_attach_v1(PrivateSliceCompositionV1::Read, legacy)
                    .expect("private write followed by checked canonical read must attach");
        }
    }

    #[test]
    fn private_write_then_two_canonical_reads_retains_same_statement_ordinals() {
        for legacy in [false, true] {
            let _owner =
                private_slice_composition_attach_v1(PrivateSliceCompositionV1::TwoReads, legacy)
                    .expect("both checked RHS reads must precede the promoted scalar destination");
        }
    }

    #[test]
    fn canonical_read_into_private_destination_keeps_the_projected_rhs_refusal() {
        use fe2o3_lower_mir_kernel::{
            ProductionMirPlironTranslationErrorV1, ProductionSemanticKirErrorV1,
        };
        for legacy in [false, true] {
            let result = private_slice_composition_attach_v1(
                PrivateSliceCompositionV1::ProjectedPrivateWrite,
                legacy,
            );
            assert!(
                matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
                        ProductionMirPlironTranslationErrorV1::MissingRankedEffect {
                            semantic_block: 1,
                            semantic_statement: Some(1),
                            semantic_access_ordinal: 1,
                        }
                    ))
                ),
                "unsupported projected RHS must fail at the later private destination, after its canonical read"
            );
        }
    }
}
