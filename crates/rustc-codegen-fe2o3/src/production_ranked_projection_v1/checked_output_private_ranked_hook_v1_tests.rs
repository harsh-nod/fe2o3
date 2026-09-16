mod private_ranked_hook_tests {
    use super::*;
    use fe2o3_pliron::ProductionSemanticSsaOperandRoleV1 as Role;

    fn sparse_write() -> SemanticFunctionDeclV1 {
        assertion_root_with_access(
            vec![
                (A_UNIT, SemanticLocalRoleV1::Return),
                (A_ARRAY, SemanticLocalRoleV1::Temporary),
                (A_U32, SemanticLocalRoleV1::Temporary),
            ],
            vec![],
            vec![
                block(
                    201,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
                ),
                block(202, vec![], SemanticTerminatorKindV1::Return),
                block(
                    203,
                    assertion_ranked_write_statements(1, 2),
                    SemanticTerminatorKindV1::Return,
                ),
            ],
            false,
        )
    }

    fn source_row(
        function: &SemanticFunctionDeclV1,
        access: ProductionRankedAccessSourceV1,
    ) -> ProjectedAccessSourceV1 {
        assert_eq!(access.semantic_block(), 2);
        assert_eq!(access.semantic_statement(), Some(1));
        assert_eq!(access.semantic_access_ordinal(), 0);
        assert!(matches!(
            function.blocks()[2].statements()[1].kind(),
            SemanticStatementKindV1::Assign(_)
        ));
        ProjectedAccessSourceV1 {
            block: access.ranked_block() as usize,
            operation: access.ranked_operation() as usize,
            access: AccessKindAttr::Write,
            memory_space: MemorySpaceAttr::Private,
            source: function.blocks()[2].statements()[1].source(),
            semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                block: 2,
                statement: Some(1),
            }),
            // The actual source above is Assign(destination, constant); this
            // role is not inferred from semantic_access_ordinal == 0.
            private_array_role: Some(Role::Destination),
        }
    }

    fn operation_count(owner: &Owner) -> usize {
        owner
            .module()
            .functions
            .iter()
            .filter_map(|function| function.body.as_ref())
            .flat_map(|body| &body.blocks)
            .map(|block| block.operations.len())
            .sum()
    }

    #[test]
    fn real_sparse_write_uses_the_mandatory_hook_and_transformed_output_ordinal() {
        let function = sparse_write();
        let source = assertion_materialized(function.clone());
        assert_eq!(operation_count(source.executable()), 7);
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_actual(&source, profile, |bound, checked, budget| {
                assert_eq!(operation_count(bound), 7);
                assert_eq!(operation_count(checked.owner()), 6);
                project_output(&source, bound, checked, profile, budget, |roots, view, budget| {
                    let [root] = roots else {
                        panic!("one actual source array root");
                    };
                    assert!(root.all_kernel_checks_are_clean());
                    let [access] = root.access_sources.as_slice() else {
                        panic!("one retained private write must emit one source receipt");
                    };
                    let _original_role = source_row(&function, *access);
                    let placement = view
                        .private_array_write(
                            ROOT,
                            ROOT,
                            fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
                                block: fe2o3_mir_model::SsaBlockIdV1::new(2),
                                statement: 1,
                            },
                            Role::Destination,
                            budget,
                        )
                        .unwrap();
                    let fe2o3_lower_mir_kernel::ProductionSourceOutputPrivateArrayAccessV1::Retained {
                        index: 0,
                        coordinate,
                        executable: true,
                    } = placement else {
                        panic!("actual checked output write must be retained and executable");
                    };
                    assert_eq!(coordinate.operation.block.function.0, 0);
                    assert_eq!(coordinate.operation.block.block, 0);
                    assert_eq!(coordinate.operation.operation, 5);
                    assert_eq!(coordinate.effect, 0);
                    assert!(!view.grants_authority());
                    Ok(())
                })
                .unwrap();
            });
        }
        source.semantic_ssa().verify_replay().unwrap();
    }

    #[test]
    fn actual_private_hook_has_exact_and_one_under_construction_storage() {
        let function = sparse_write();
        let source = assertion_materialized(function.clone());
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for one_under in [false, true] {
                with_actual(&source, profile, |bound, checked, budget| {
                    project_output(
                        &source,
                        bound,
                        checked,
                        profile,
                        budget,
                        |roots, _, budget| {
                            let [root] = roots else { panic!("one actual array root") };
                            let [access] = root.access_sources.as_slice() else {
                                panic!("one actual private write")
                            };
                            let source_row = source_row(&function, *access);
                            let blocks = root.lowering.kernel().blocks();
                            let mut views = 0;
                            let mut constants = 0;
                            for operation in blocks.iter().flat_map(|block| block.operations()) {
                                match operation {
                                    ProductionRankedOperationV1::ViewInSpace { .. } => views += 1,
                                    ProductionRankedOperationV1::IndexConstant { .. } => constants += 1,
                                    _ => {}
                                }
                            }
                            assert_eq!((views, constants), (1, 1));
                            // This fixture has exactly two numeric definitions. Each is
                            // three u32 cells in each of two buffers, plus two vector
                            // headers and the fixed 256-cell radix histogram.
                            let scratch_bytes = 12 * std::mem::size_of::<u32>()
                                + 2 * std::mem::size_of::<Vec<[u32; 3]>>()
                                + 256 * std::mem::size_of::<usize>();
                            with_checked_output_assertions_budget_v1(
                                &source,
                                bound,
                                checked,
                                profile,
                                budget,
                                |session| {
                                    let (floor, ballast, entry_work) =
                                        session.with_output_occurrences_v1(|_, budget| {
                                            let floor = budget.storage();
                                            let available = scratch_bytes - usize::from(one_under);
                                            let ballast = budget.storage_limit() - floor - available;
                                            budget.reserve_storage(ballast).unwrap();
                                            Ok((floor, ballast, budget.work()))
                                        })?;
                                    let result = {
                                        let mut facts = session.for_source(ROOT, ROOT);
                                        facts.check_ranked_private_array_sources(
                                            blocks,
                                            std::slice::from_ref(&source_row),
                                        )
                                    };
                                    session.with_output_occurrences_v1(|_, budget| {
                                        assert_eq!(budget.storage(), floor + ballast);
                                        assert!(budget.work() > entry_work);
                                        if one_under {
                                            assert!(matches!(
                                                result,
                                                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                                                    CanonicalAssertionErrorV1::Resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Storage(error))
                                                )) if error.actual() == budget.storage_limit() + 1
                                                    && error.limit() == budget.storage_limit()
                                            ));
                                            assert_eq!(budget.failed_storage(), Some(budget.storage_limit() + 1));
                                        } else {
                                            result.unwrap();
                                            assert_eq!(budget.peak_storage(), budget.storage_limit());
                                            assert_eq!(budget.failed_storage(), None);
                                        }
                                        budget.release_storage(ballast).unwrap();
                                        assert_eq!(budget.storage(), floor);
                                        Ok(())
                                    })
                                },
                            )
                        },
                    )
                    .unwrap();
                });
            }
        }
    }

    #[derive(Clone, Copy)]
    enum Mutation {
        Index,
        Extent,
        Width,
        Origin,
        Noalias,
        Readonly,
        Access,
    }

    fn changed_blocks(
        blocks: &[ProductionRankedBlockV1],
        source: &ProjectedAccessSourceV1,
        mutation: Mutation,
    ) -> Vec<ProductionRankedBlockV1> {
        let ProductionRankedOperationV1::Access { view, indices, .. } =
            &blocks[source.block].operations()[source.operation]
        else {
            panic!("the actual projector emits ordinary Access");
        };
        let ProductionRankedValueV1::Local(view_id) = *view else {
            panic!("actual ranked view is an operation definition");
        };
        let [ProductionRankedValueV1::Local(index_id)] = indices.as_slice() else {
            panic!("actual ranked index is an operation definition");
        };
        let mut changed = 0;
        let result = blocks
            .iter()
            .enumerate()
            .map(|(block_index, block)| {
                let mut operations = block.operations().to_vec();
                for (operation_index, operation) in operations.iter_mut().enumerate() {
                    match operation {
                        ProductionRankedOperationV1::IndexConstant { result, value }
                            if result == index_id && matches!(mutation, Mutation::Index) =>
                        {
                            *value = 1;
                            changed += 1;
                        }
                        ProductionRankedOperationV1::ViewInSpace {
                            result,
                            shape,
                            element_width,
                            allocation_origin,
                            noalias_class,
                            writable,
                            ..
                        } if *result == view_id => {
                            match mutation {
                                Mutation::Extent => shape[0] += 1,
                                Mutation::Width => *element_width += 8,
                                Mutation::Origin => *allocation_origin += 1,
                                Mutation::Noalias => *noalias_class += 1,
                                Mutation::Readonly => *writable = false,
                                Mutation::Index | Mutation::Access => continue,
                            }
                            changed += 1;
                        }
                        ProductionRankedOperationV1::Access { kind, .. }
                            if block_index == source.block
                                && operation_index == source.operation
                                && matches!(mutation, Mutation::Access) =>
                        {
                            *kind = AccessKindAttr::Read;
                            changed += 1;
                        }
                        _ => {}
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
        result
    }

    #[test]
    fn source_receipt_hook_rejects_mutated_private_view_index_and_access() {
        let function = sparse_write();
        let source = assertion_materialized(function.clone());
        with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
            project_output(
                &source,
                bound,
                checked,
                Profile::Gfx942,
                budget,
                |roots, _, budget| {
                    let [root] = roots else {
                        panic!("one actual root")
                    };
                    let [access] = root.access_sources.as_slice() else {
                        panic!("one actual array write")
                    };
                    let source_row = source_row(&function, *access);
                    let blocks = root.lowering.kernel().blocks();
                    // These are deliberately inert candidate mutations. They
                    // must fail the real pre-receipt hook, not become a checked
                    // ranked owner or a substituted executable O.
                    with_checked_output_assertions_budget_v1(
                        &source,
                        bound,
                        checked,
                        Profile::Gfx942,
                        budget,
                        |session| {
                            let mut facts = session.for_source(ROOT, ROOT);
                            let positive = production_access_sources(
                                blocks,
                                std::slice::from_ref(&source_row),
                                &mut facts,
                            )?;
                            assert_eq!(positive.as_slice(), std::slice::from_ref(access));
                            for mutation in [
                                Mutation::Index,
                                Mutation::Extent,
                                Mutation::Width,
                                Mutation::Origin,
                                Mutation::Noalias,
                                Mutation::Readonly,
                                Mutation::Access,
                            ] {
                                let blocks = changed_blocks(blocks, &source_row, mutation);
                                let expected = match mutation {
                                    Mutation::Index => {
                                        "array allocation/index ranked index changed"
                                    }
                                    Mutation::Access => {
                                        "array allocation/index ranked access changed"
                                    }
                                    _ => "array allocation/index ranked view changed",
                                };
                                let error = production_access_sources(
                                    &blocks,
                                    std::slice::from_ref(&source_row),
                                    &mut facts,
                                )
                                .unwrap_err();
                                assert!(matches!(
                                    error,
                                    ProductionRankedProjectionErrorV1::CanonicalAssertions(
                                        CanonicalAssertionErrorV1::Output(
                                            fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1::Invalid(actual)
                                        )
                                    ) if actual == expected
                                ));
                            }
                            Ok(())
                        },
                    )
                },
            )
            .unwrap();
        });
    }

    #[test]
    fn actual_sparse_output_memory_is_covered_in_both_directions() {
        let source = assertion_materialized(sparse_write());
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_actual(&source, profile, |bound, checked, budget| {
                assert_eq!(operation_count(bound), 7);
                assert_eq!(operation_count(checked.owner()), 6);
                project_output(
                    &source,
                    bound,
                    checked,
                    profile,
                    budget,
                    |roots, view, budget| {
                        let [root] = roots else {
                            panic!("one actual sparse root")
                        };
                        let floor = budget.storage();
                        let before = budget.work();
                        let census = view
                            .check_ranked_output_effect_census(
                                root.semantic_root,
                                root.semantic_u32_induction.function(),
                                &root.lowering,
                                &root.access_sources,
                                &root.executable_effect_sources,
                                budget,
                            )
                            .map_err(|error| {
                                ProductionRankedProjectionErrorV1::CanonicalAssertions(
                                    CanonicalAssertionErrorV1::Output(error),
                                )
                            })?;
                        assert_eq!((census.accesses(), census.private_allocations()), (1, 1));
                        assert_eq!(budget.storage(), floor);
                        assert!(budget.work() > before);
                        assert!(std::ptr::eq(view.output(), checked.owner()));
                        Ok(())
                    },
                )
                .unwrap();
            });
        }
    }

    #[test]
    fn actual_output_census_rejects_missing_duplicate_and_wrong_source_receipts() {
        let source = assertion_materialized(sparse_write());
        with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
            project_output(
                &source,
                bound,
                checked,
                Profile::Gfx942,
                budget,
                |roots, view, budget| {
                    let [root] = roots else {
                        panic!("one actual sparse root")
                    };
                    let [receipt] = root.access_sources.as_slice() else {
                        panic!("one actual write")
                    };
                    let wrong_site = ProductionRankedAccessSourceV1::new(
                        0,
                        receipt.semantic_statement(),
                        receipt.semantic_access_ordinal(),
                        receipt.ranked_block(),
                        receipt.ranked_operation(),
                    );
                    let wrong_ranked = ProductionRankedAccessSourceV1::new(
                        receipt.semantic_block(),
                        receipt.semantic_statement(),
                        receipt.semantic_access_ordinal(),
                        receipt.ranked_block(),
                        u32::MAX,
                    );
                    let wrong_ordinal = ProductionRankedAccessSourceV1::new(
                        receipt.semantic_block(),
                        receipt.semantic_statement(),
                        1,
                        receipt.ranked_block(),
                        receipt.ranked_operation(),
                    );
                    for candidate in [
                        Vec::new(),
                        vec![*receipt, *receipt],
                        vec![wrong_site],
                        vec![wrong_ranked],
                        vec![wrong_ordinal],
                    ] {
                        let floor = budget.storage();
                        let before = budget.work();
                        assert!(
                            view.check_ranked_output_effect_census(
                                root.semantic_root,
                                root.semantic_u32_induction.function(),
                                &root.lowering,
                                &candidate,
                                &root.executable_effect_sources,
                                budget,
                            )
                            .is_err()
                        );
                        assert_eq!(budget.storage(), floor);
                        assert!(budget.work() > before);
                    }
                    Ok(())
                },
            )
            .unwrap();
        });
    }

    #[test]
    fn actual_output_census_has_exact_and_one_under_scratch_storage() {
        let source = assertion_materialized(sparse_write());
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_actual(&source, profile, |bound, checked, budget| {
                project_output(&source, bound, checked, profile, budget, |roots, view, budget| {
                    let [root] = roots else { panic!("one actual sparse root") };
                    assert_eq!(root.access_sources.len(), 1);
                    // One access row has five u32 coordinate cells, two byte
                    // enum tags and two alignment bytes. The allocation row
                    // is three u32 cells; each vector has one inline header.
                    let scratch_bytes = 36 + 2 * std::mem::size_of::<Vec<u32>>();
                    let floor = budget.storage();
                    for one_under in [true, false] {
                        let available = scratch_bytes - usize::from(one_under);
                        let ballast = budget.storage_limit() - floor - available;
                        budget.reserve_storage(ballast).unwrap();
                        let before = budget.work();
                        let result = view.check_ranked_output_effect_census(
                            root.semantic_root,
                            root.semantic_u32_induction.function(),
                            &root.lowering,
                            &root.access_sources,
                            &root.executable_effect_sources,
                            budget,
                        );
                        assert_eq!(budget.storage(), floor + ballast);
                        assert!(budget.work() > before);
                        if one_under {
                            assert!(matches!(result,
                                Err(fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1::Resource(
                                    fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Storage(error)
                                )) if error.actual() == budget.storage_limit() + 1 && error.limit() == budget.storage_limit()
                            ));
                        } else {
                            let census = result.unwrap();
                            assert_eq!((census.accesses(), census.private_allocations()), (1, 1));
                            assert_eq!(budget.peak_storage(), budget.storage_limit());
                        }
                        assert_eq!(budget.failed_storage(), Some(budget.storage_limit() + 1));
                        budget.release_storage(ballast).unwrap();
                        assert_eq!(budget.storage(), floor);
                    }
                    Ok(())
                }).unwrap();
            });
        }
    }

    #[test]
    fn actual_output_census_covers_dynamic_global_reads_and_writes() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for shape in [
                GlobalWriteExpressionShapeV1::Parameter,
                GlobalWriteExpressionShapeV1::ParameterArithmetic,
                GlobalWriteExpressionShapeV1::LoadArithmetic,
            ] {
                let source = genuine_dynamic_global_source_v1(shape, 1);
                with_actual_dynamic_global_root_v1(&source, profile, 1, |root, view, budget| {
                    let floor = budget.storage();
                    let census = view
                        .check_ranked_output_effect_census(
                            root.semantic_root,
                            root.semantic_u32_induction.function(),
                            &root.lowering,
                            &root.access_sources,
                            &root.executable_effect_sources,
                            budget,
                        )
                        .map_err(|error| {
                            ProductionRankedProjectionErrorV1::CanonicalAssertions(
                                CanonicalAssertionErrorV1::Output(error),
                            )
                        })?;
                    let accesses = if matches!(shape, GlobalWriteExpressionShapeV1::LoadArithmetic)
                    {
                        2
                    } else {
                        1
                    };
                    assert_eq!(
                        (census.accesses(), census.private_allocations()),
                        (accesses, 0)
                    );
                    assert_eq!(budget.storage(), floor);
                    Ok(())
                })
                .unwrap();
            }
        }
    }

    #[test]
    fn actual_output_census_has_source_derived_work_prefix_and_failure_bounds() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            let source =
                genuine_dynamic_global_source_v1(GlobalWriteExpressionShapeV1::Parameter, 1);
            with_actual_dynamic_global_root_v1(&source, profile, 1, |root, view, inherited| {
                let [receipt] = root.access_sources.as_slice() else { panic!("one actual Global store") };
                assert_eq!((receipt.semantic_block(), receipt.semantic_statement(), receipt.semantic_access_ordinal()), (1, Some(0), 0));
                let semantic = source.semantic_ssa().source_semantic();
                assert_eq!(semantic.functions().len(), 1);
                assert_eq!(semantic.functions()[0].blocks().len(), 3);
                assert_eq!(semantic.functions()[0].blocks()[0].statements().len(), 0);
                assert_eq!(semantic.functions()[0].blocks()[2].statements().len(), 4);
                assert_eq!(semantic.functions()[0].blocks()[1].statements().len(), 1);
                let ranked = root.lowering.kernel().blocks();
                let output_module = view.output().module();
                let output = output_module.functions[0].body.as_ref().unwrap();
                assert_eq!(output_module.functions.iter().filter(|function| function.body.is_some()).count(), 1);
                assert_eq!(output.blocks.len(), 3);
                assert_eq!(A_NAME.len(), 24);
                let original = source.executable().module().functions[0].body.as_ref().unwrap();
                assert_eq!(original.blocks.len(), 4);
                // No retained-local/enum storage prologue can add synthetic
                // spans; this source has only the single shared failure span.
                assert!(!original.blocks.iter().flat_map(|block| &block.operations).any(|operation| matches!(
                    operation.kind, fe2o3_kernel_ir::OperationKind::Alloca { .. }
                        | fe2o3_kernel_ir::OperationKind::Load { access: fe2o3_kernel_ir::MemoryAccess { address_space: fe2o3_kernel_ir::AddressSpace::Private, .. }, .. }
                        | fe2o3_kernel_ir::OperationKind::Store { access: fe2o3_kernel_ir::MemoryAccess { address_space: fe2o3_kernel_ir::AddressSpace::Private, .. }, .. }
                )));
                let last = output.blocks.last().unwrap();
                let trap = last.operations.last().unwrap();
                let fe2o3_kernel_ir::OperationKind::Call { callee, arguments } = &trap.kind else { panic!("last output row is the retained runtime failure") };
                assert!(arguments.is_empty() && trap.results.is_empty());
                assert!(matches!(last.terminator, Some(fe2o3_kernel_ir::Terminator::Unreachable)));
                assert_eq!(fe2o3_kernel_ir::AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments), Some(fe2o3_kernel_ir::AmdGpuDiagnosticOperation::Trap));
                assert_eq!(output.blocks.iter().flat_map(|block| &block.operations).filter(|operation| matches!(operation.kind, fe2o3_kernel_ir::OperationKind::Call { .. })).count(), 1);
                // Query component only, not source/optimizer/root admission.
                // Eight source spans: bb0 terminator, bb1 Store, bb1 terminator,
                // bb2 statements0..3, bb2 terminator. Midpoints4,2,1 find the Store.
                // Global lookup:4+1+3*(1+5)+2+3=28, twelve more than the old fixture.
                // Alias/name/reservation and initial refusal prefixes are unchanged.
                const HISTORY: usize = 7;
                const FIXED: usize = 5 + 3 + 5 + 25 + 7 + 2 + 2 + 35 + 6 + 1 + 10 + 2;
                assert_eq!(FIXED, 103);
                let ranked_operations = ranked.iter().map(|block| block.operations().len()).sum::<usize>();
                let output_operations = output.blocks.iter().map(|block| block.operations.len()).sum::<usize>();
                // This trap is the final row of the only function body. Finding
                // the last element by midpoint search takes floor(log2(n+1))
                // visits, each charged1+3; this formula does not run the query.
                let trap_lookup_visits = (output_operations + 1).ilog2() as usize;
                // Call branch5, row access1+3, graph6, one source span4+5+1,
                // roles2, payload8+4, byte equality1+callee.len():40+len.
                // The new ordinary-call dispatch has one non-defined Call:
                // entry/floor6 + one index search(1+3) + classification4.
                // It adds no allocation and leaves the exact trap rule intact.
                let trap_work = 14 + 40 + callee.as_str().len() + 4 * trap_lookup_visits;
                let query_work = FIXED + ranked.len() + 2 * ranked_operations
                    + output.blocks.len() + 2 * output_operations + trap_work;
                let floor = inherited.storage();
                let scratch = 24 + 2 * std::mem::size_of::<Vec<u32>>();
                for (repeats, one_under) in [(1, false), (1, true), (2, false), (2, true)] {
                    let total = HISTORY + repeats * query_work;
                    let mut work = Work::new(total - usize::from(one_under));
                    let mut budget = Budget::new(&mut work, floor + scratch);
                    budget.charge_work(HISTORY).unwrap();
                    budget.reserve_storage(floor).unwrap();
                    let mut result = Ok(());
                    for _ in 0..repeats {
                        result = view.check_ranked_output_effect_census(
                            root.semantic_root, root.semantic_u32_induction.function(),
                            &root.lowering, &root.access_sources,
                            &root.executable_effect_sources, &mut budget,
                        ).map(|census| assert_eq!((census.accesses(), census.private_allocations()), (1, 0)));
                        if result.is_err() { break; }
                    }
                    assert_eq!(result.is_err(), one_under);
                    assert_eq!(budget.work(), total - if one_under { 2 } else { 0 });
                    assert_eq!(budget.storage(), floor);
                    assert_eq!(budget.peak_storage(), floor + scratch);
                    assert_eq!(budget.failed_storage(), None);
                    if one_under {
                        assert!(matches!(result, Err(ProductionSourceOutputErrorV1::Resource(
                            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(error)
                        )) if error.actual() == total && error.limit() == total - 1));
                        // Retry the rejected whole charge; a smaller charge is
                        // allowed by the meter while preserving failure history.
                        assert!(budget.charge_work(2).is_err());
                        assert_eq!(budget.work(), total - 2);
                    }
                    assert_eq!(work.failed_work(), one_under.then_some(total));
                }
                // Scratch rejection happens at the independently derived45
                // prefix. A malformed ranked coordinate fails after49+5,
                // while both paths retain caller history and restore its floor.
                for under_storage in [false, true] {
                    let mut work = Work::new(HISTORY + query_work);
                    let mut budget = Budget::new(&mut work, floor + scratch - usize::from(under_storage));
                    budget.charge_work(HISTORY).unwrap();
                    budget.reserve_storage(floor).unwrap();
                    let malformed = ProductionRankedAccessSourceV1::new(1, Some(0), 0, receipt.ranked_block(), u32::MAX);
                    assert!(view.check_ranked_output_effect_census(
                        root.semantic_root, root.semantic_u32_induction.function(),
                        &root.lowering, std::slice::from_ref(&malformed),
                        &root.executable_effect_sources, &mut budget,
                    ).is_err());
                    assert_eq!(budget.work(), HISTORY + if under_storage { 45 } else { 54 });
                    assert_eq!(budget.storage(), floor);
                    assert_eq!(budget.peak_storage(), floor + if under_storage { 0 } else { scratch });
                    assert_eq!(budget.failed_storage(), under_storage.then_some(floor + scratch));
                    assert_eq!(work.failed_work(), None);
                }
                Ok(())
            }).unwrap();
        }
    }

    #[test]
    fn mandatory_output_census_preserves_live_owner_on_consumer_error_and_unwind() {
        let source = assertion_materialized(sparse_write());
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_actual(&source, profile, |bound, checked, budget| {
                let floor = budget.storage();
                for unwind in [false, true] {
                    let called = std::cell::Cell::new(false);
                    let before = budget.work();
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        project_output(
                            &source,
                            bound,
                            checked,
                            profile,
                            budget,
                            |roots, view, budget| {
                                called.set(true);
                                let [root] = roots else {
                                    panic!("one actual sparse root")
                                };
                                assert!(std::ptr::eq(view.output(), checked.owner()));
                                let query_floor = budget.storage();
                                let census = view
                                    .check_ranked_output_effect_census(
                                        root.semantic_root,
                                        root.semantic_u32_induction.function(),
                                        &root.lowering,
                                        &root.access_sources,
                                        &root.executable_effect_sources,
                                        budget,
                                    )
                                    .unwrap();
                                assert_eq!(
                                    (census.accesses(), census.private_allocations()),
                                    (1, 1)
                                );
                                assert_eq!(budget.storage(), query_floor);
                                if unwind {
                                    panic!("consumer after actual output census");
                                }
                                Err::<(), _>(ProductionRankedProjectionErrorV1::Incomplete(
                                    "consumer after actual output census",
                                ))
                            },
                        )
                    }));
                    assert!(called.get());
                    assert_eq!(result.is_err(), unwind);
                    if let Ok(result) = result {
                        assert!(result.is_err());
                    }
                    assert_eq!(budget.storage(), floor);
                    assert!(budget.work() > before);
                    assert_eq!(operation_count(checked.owner()), 6);
                }
            });
        }
    }
}
