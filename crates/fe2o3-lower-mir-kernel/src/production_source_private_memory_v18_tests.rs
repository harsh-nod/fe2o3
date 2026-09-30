include!("production_source_safe_private_memory_v18_tests.rs");
#[path = "production_source_typed_endpoints_v36_tests.rs"]
mod typed_endpoint_tests;

#[test]
fn private_memory_source_error_preserves_typed_cause_and_existing_fixed_layout() {
    use fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1 as Physical;
    use std::mem::{align_of, size_of};
    #[allow(dead_code)]
    enum PriorError {
        Source(ProductionPendingScopedSourceErrorV29),
        Resource(ArgumentResourceV1),
        Binding(&'static str),
        Analysis(fe2o3_pliron::CanonicalAnalysisScopeErrorV1),
    }
    #[allow(dead_code)]
    enum PriorFailure {
        Resource(ArgumentResourceV1),
        Binding(&'static str),
        Analysis(fe2o3_pliron::CanonicalAnalysisScopeErrorV1),
    }
    assert_eq!(
        (
            size_of::<ProductionSourceOwnedViewErrorV18>(),
            align_of::<ProductionSourceOwnedViewErrorV18>()
        ),
        (size_of::<PriorError>(), align_of::<PriorError>())
    );
    assert_eq!(
        (
            size_of::<SourceOwnedResultV18<()>>(),
            align_of::<SourceOwnedResultV18<()>>()
        ),
        (
            size_of::<Result<(), PriorError>>(),
            align_of::<Result<(), PriorError>>()
        )
    );
    assert_eq!(
        (
            size_of::<SourceOwnedQueryFailureV18>(),
            align_of::<SourceOwnedQueryFailureV18>()
        ),
        (size_of::<PriorFailure>(), align_of::<PriorFailure>())
    );
    assert_eq!(
        (
            size_of::<Option<SourceOwnedQueryFailureV18>>(),
            align_of::<Option<SourceOwnedQueryFailureV18>>()
        ),
        (
            size_of::<Option<PriorFailure>>(),
            align_of::<Option<PriorFailure>>()
        )
    );
    for cause in [
        Physical::Unsupported {
            phase: "private",
            detail: "bounded nonzero allocation extent",
        },
        Physical::Panicked,
    ] {
        let error = ProductionSourceOwnedViewErrorV18::from(cause);
        assert!(
            matches!(&error, ProductionSourceOwnedViewErrorV18::PrivateMemory(actual) if *actual == cause)
        );
        assert_eq!(
            std::error::Error::source(&error)
                .unwrap()
                .downcast_ref::<Physical>(),
            Some(&cause)
        );
        assert_eq!(error.to_string(), cause.to_string());
        assert!(
            matches!(SourceOwnedQueryFailureV18::private_memory(cause).error(),
            ProductionSourceOwnedViewErrorV18::PrivateMemory(actual) if actual == cause)
        );
    }
    for cause in [
        Physical::Resource(ArgumentResourceV1::Accounting),
        Physical::Inventory(
            fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(
                ArgumentResourceV1::Accounting,
            ),
        ),
    ] {
        assert!(matches!(
            ProductionSourceOwnedViewErrorV18::from(cause),
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
        ));
        assert!(matches!(
            SourceOwnedQueryFailureV18::private_memory(cause).error(),
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
        ));
    }
    let inconsistent = fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::InconsistentOwner;
    assert!(matches!(
        SourceOwnedQueryFailureV18::private_memory(Physical::Inventory(inconsistent)).error(),
        ProductionSourceOwnedViewErrorV18::Analysis(fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Inventory(actual)) if actual == inconsistent
    ));
}

#[test]
fn private_memory_source_actual_physical_refusal_keeps_phase_detail_and_first_error() {
    use fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1 as Physical;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared =
        private_memory_prepared_v18(typed_root_entry_rhs_owner_v18, &mut budget).unwrap();
    let mut entered = false;
    let result = with_production_optimizer_result_v18(
        prepared,
        &mut budget,
        |original, optimized, budget| {
            let floor = budget.storage();
            scoped_raw_admission_v29::with_source_private_physical_v18(
                original,
                optimized,
                fe2o3_kernel_analysis::CanonicalKirPrivateMemoryLimitsV1 { max_cells: 64 },
                budget,
                |_, _| {
                    entered = true;
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                },
            )?;
            assert!(entered);
            assert_eq!(budget.storage(), floor);
            let refused = scoped_raw_admission_v29::with_source_private_physical_v18(
                original,
                optimized,
                fe2o3_kernel_analysis::CanonicalKirPrivateMemoryLimitsV1 { max_cells: 0 },
                budget,
                |_, _| -> SourceOwnedResultV18<()> { panic!("zero-cell physical proof published") },
            )
            .unwrap_err();
            let expected = Physical::Unsupported {
                phase: "private",
                detail: "bounded nonzero allocation extent",
            };
            assert!(
                matches!(&refused, ProductionSourceOwnedViewErrorV18::PrivateMemory(actual) if *actual == expected)
            );
            assert_eq!(
                std::error::Error::source(&refused)
                    .unwrap()
                    .downcast_ref::<Physical>(),
                Some(&expected)
            );
            assert_eq!(budget.storage(), floor);
            let stopped = (budget.work(), budget.storage());
            assert!(matches!(original.source.root_count(budget),
            Err(ProductionSourceOwnedViewErrorV18::PrivateMemory(actual)) if actual == expected));
            assert!(
                matches!(original.retain_query::<()>(Err(ProductionSourceOwnedViewErrorV18::Binding("later error"))),
            Err(ProductionSourceOwnedViewErrorV18::PrivateMemory(actual)) if actual == expected)
            );
            assert_eq!((budget.work(), budget.storage()), stopped);
            Ok(())
        },
    );
    assert!(matches!(
        result,
        Err(ProductionSourceOptimizationErrorV18::Source(
            ProductionSourceOwnedViewErrorV18::PrivateMemory(Physical::Unsupported {
                phase: "private",
                detail: "bounded nonzero allocation extent"
            })
        ))
    ));
    assert!(entered);
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn private_memory_source_typed_error_replay_preserves_prior_resource_custody() {
    use fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1 as Physical;
    for resource_first in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(scalar_payload_owner_v18, &mut budget);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            assert!(source.root_count(budget)? > 0);
            if resource_first {
                assert!(matches!(
                    source.retain_query::<()>(Err(ArgumentResourceV1::Arithmetic.into())),
                    Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Arithmetic
                    ))
                ));
            }
            let stopped = (budget.work(), budget.storage());
            let first = source.retain_query::<()>(Err(Physical::Panicked.into()));
            let replay = source.root_count(budget).map(|_| ());
            for error in [first.unwrap_err(), replay.unwrap_err()] {
                if resource_first {
                    assert!(matches!(
                        error,
                        ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Arithmetic)
                    ));
                } else {
                    assert!(matches!(
                        error,
                        ProductionSourceOwnedViewErrorV18::PrivateMemory(Physical::Panicked)
                    ));
                }
            }
            assert_eq!((budget.work(), budget.storage()), stopped);
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        });
        if resource_first {
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Arithmetic
                ))
            ));
        } else {
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::PrivateMemory(
                    Physical::Panicked
                ))
            ));
        }
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

fn private_memory_prepared_v18(
    factory: fn() -> ProductionSemanticSsaOwnerV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<ProductionPreparedSourceV18> {
    let projection = factory();
    let owner = factory();
    let (_, launch) = with_module_fixture_view(&owner, ModuleFixture::Ordinary, budget, |_, _| ())?;
    with_module_fixture_view(
        &projection,
        ModuleFixture::Ordinary,
        budget,
        |source, budget| {
            ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                owner,
                launch,
                source.input,
                ProductionSemanticKirLimitsV1::default(),
                budget,
            )
        },
    )?
    .0
}

fn private_memory_run_v18(
    factory: fn() -> ProductionSemanticSsaOwnerV1,
    work_limit: usize,
    storage_limit: usize,
    mut consume: impl for<'scope, 'work> FnMut(
        &scoped_raw_admission_v29::CheckedSourcePrivateMemoryV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> (
    ProductionOptimizerTestResultV18,
    usize,
    usize,
    usize,
    [usize; 3],
    bool,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut counts = [0; 3];
    let result = (|| {
        let prepared = private_memory_prepared_v18(factory, &mut budget)?;
        with_production_optimizer_result_v18(
            prepared,
            &mut budget,
            |original, optimized, budget| {
                let floor = budget.storage();
                scoped_raw_admission_v29::with_source_private_physical_v18(
                    original,
                    optimized,
                    fe2o3_kernel_analysis::CanonicalKirPrivateMemoryLimitsV1 { max_cells: 64 },
                    budget,
                    |physical, budget| {
                        original.with_optimized_analysis_v18(optimized, budget, |analysis, budget| {
                        analysis.with_memory_versions(budget, |input_memory, output_memory, budget| {
                            for root in 0..original.source.root_count(budget)? {
                                let function = optimized_source_root_function_v18(original, optimized, root, budget)?.function;
                                // This only supplies the existing collision namespace. It is
                                // not a ranked memory recipe or final effect certificate.
                                let recipe = effect_order_recipe_v18(function, 0, false);
                                original.with_optimized_scalar_leaves_v18(optimized, root, &recipe, budget, |leaves, budget| {
                                    leaves.with_checked_entry_writes_v18(budget,
                                        |request, budget| check_fixture_entry_rhs_v18(leaves, request, budget),
                                        |entries, budget| {
                                            scoped_raw_admission_v29::with_checked_optimized_source_memory_v18(
                                                original, optimized, root, input_memory, output_memory, budget,
                                                |currentness, budget| {
                                                    let floor = budget.storage();
                                                    physical.with_root_memory_v18(root, currentness, entries, budget,
                                                        |memory, budget| {
                                                            let observed = memory.test_counts_v18(budget)?;
                                                            for (total, added) in counts.iter_mut().zip(observed) {
                                                                *total += added;
                                                            }
                                                            consume(memory, budget)
                                                        })?;
                                                    assert_eq!(budget.storage(), floor);
                                                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                                                })
                                        })
                                })?;
                            }
                            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                        })
                    })
                    },
                )?;
                assert_eq!(budget.storage(), floor);
                Ok(())
            },
        )
    })();
    let completed = result.is_ok();
    (
        result,
        budget.work(),
        budget.peak_storage(),
        budget.storage(),
        counts,
        completed,
    )
}

#[test]
fn private_memory_source_composition_checks_root_and_repeated_helper_entry_reads() {
    for (factory, minimum) in [
        (
            typed_root_entry_rhs_owner_v18 as fn() -> ProductionSemanticSsaOwnerV1,
            1,
        ),
        (typed_entry_rhs_owner_v18, 2),
    ] {
        let (result, _, _, floor, counts, completed) = private_memory_run_v18(
            factory,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |_, _| Ok(()),
        );
        result.unwrap();
        assert!(completed);
        assert_eq!(floor, MODULE_FLOOR);
        assert!(
            counts[0] >= minimum && counts[1] >= minimum && counts[2] >= minimum,
            "{counts:?}"
        );
        assert_eq!(
            counts[0], counts[1],
            "each authentic allocation has its entry initialization"
        );
    }
}

#[test]
fn private_memory_source_composition_has_exact_and_one_short_complete_transactions() {
    let factory = typed_entry_rhs_owner_v18;
    let (result, work, storage, floor, counts, completed) = private_memory_run_v18(
        factory,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |_, _| Ok(()),
    );
    result.unwrap();
    assert!(completed && counts.into_iter().all(|count| count >= 2));
    assert_eq!(floor, MODULE_FLOOR);
    let (exact, used, peak, floor, exact_counts, completed) =
        private_memory_run_v18(factory, work, storage, |_, _| Ok(()));
    exact.unwrap();
    assert!(completed);
    assert_eq!(
        (used, peak, floor, exact_counts),
        (work, storage, MODULE_FLOOR, counts)
    );
    for (work_limit, storage_limit, expected_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let (result, _, _, floor, _, completed) =
            private_memory_run_v18(factory, work_limit, storage_limit, |_, _| Ok(()));
        assert!(!completed);
        assert_eq!(floor, MODULE_FLOOR);
        let error = match result {
            Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Resource(error),
            ))
            | Err(ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                    ProductionSourceOwnedViewErrorV18::Resource(error),
                ),
            ))
            | Err(ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Resource(error),
            )) => error,
            Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Source(
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
                    ),
                ),
            )) => error,
            other => panic!("exact composition resource refusal required: {other:?}"),
        };
        match (expected_work, error) {
            (true, ArgumentResourceV1::Work(error)) => {
                assert_eq!(error.limit(), work_limit);
                assert!(error.actual() > error.limit());
            }
            (false, ArgumentResourceV1::Storage(error)) => {
                assert_eq!(error.limit(), storage_limit);
                assert!(error.actual() > error.limit());
            }
            _ => panic!("wrong resource boundary: {error:?}"),
        }
    }
}

#[test]
fn private_memory_source_join_refuses_copied_slot_root_pointer_and_missing_allocation() {
    for fault in 0..4 {
        let mut reached = false;
        let (result, _, _, floor, counts, completed) = private_memory_run_v18(
            typed_entry_rhs_owner_v18,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |memory, budget| {
                if memory.test_counts_v18(budget)?[0] == 0 {
                    return Ok(());
                }
                reached = true;
                let error = memory
                    .test_replaced_allocation_v18(fault, budget)
                    .unwrap_err();
                assert!(
                    matches!(error, ProductionSourceOwnedViewErrorV18::Binding(_)),
                    "{error:?}"
                );
                let stopped = (budget.work(), budget.storage());
                assert!(matches!(
                    memory.test_check_v18(budget),
                    Err(ProductionSourceOwnedViewErrorV18::Binding(_))
                ));
                assert_eq!((budget.work(), budget.storage()), stopped);
                Err(error)
            },
        );
        assert!(reached && !completed && counts.into_iter().all(|count| count >= 2));
        assert_eq!(floor, MODULE_FLOOR);
        assert!(matches!(
            result,
            Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Binding(_)
            )) | Err(ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                    ProductionSourceOwnedViewErrorV18::Binding(_)
                )
            ))
        ));
    }
}

#[test]
fn private_memory_source_query_rejects_foreign_ledger_before_debit_and_retains_first_failure() {
    let mut reached = false;
    let (result, _, _, retained, _, completed) = private_memory_run_v18(
        typed_root_entry_rhs_owner_v18,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |memory, budget| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
            let mut foreign = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            assert!(matches!(
                memory.test_check_v18(&mut foreign),
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert_eq!((foreign.work(), foreign.storage()), (0, 0));
            let stopped = (budget.work(), budget.storage());
            assert!(matches!(
                memory.test_check_v18(budget),
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert_eq!((budget.work(), budget.storage()), stopped);
            reached = true;
            Ok(())
        },
    );
    assert!(reached && !completed);
    assert!(retained > MODULE_FLOOR);
    assert!(matches!(
        result,
        Err(ProductionSourceOptimizationErrorV18::Source(
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
        )) | Err(ProductionSourceOptimizationErrorV18::Adoption(
            fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
            )
        ))
    ));
}

fn private_memory_nonentry_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    let owner = typed_root_entry_rhs_owner_v18();
    let source = owner.source_semantic();
    let mut functions = source.functions().to_vec();
    let root = &functions[0];
    assert_eq!(
        root.blocks().len(),
        1,
        "the non-entry mutation preserves the complete original CFG"
    );
    let mut statements = root.blocks()[0].statements().to_vec();
    statements.push(assign(
        place(1, U32),
        SemanticRvalueKindV1::Use(literal(23)),
    ));
    let blocks = vec![
        SemanticBasicBlockV1::new(
            root.blocks()[0].identity(),
            root.blocks()[0].source(),
            statements,
            root.blocks()[0].terminator().clone(),
        )
        .unwrap(),
    ];
    functions[0] = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        root.source(),
        root.abi().clone(),
        root.locals().to_vec(),
        root.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        source.callables().to_vec(),
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn private_memory_source_composition_never_omits_a_nonentry_typed_write() {
    let positive = private_memory_run_v18(
        typed_root_entry_rhs_owner_v18,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |_, _| Ok(()),
    );
    positive.0.unwrap();
    assert!(positive.5 && positive.4.into_iter().all(|count| count >= 1));
    let (result, _, _, floor, counts, completed) = private_memory_run_v18(
        private_memory_nonentry_owner_v18,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |_, _| panic!("non-entry write was silently omitted"),
    );
    assert!(!completed);
    assert_eq!(counts, [0; 3]);
    assert_eq!(floor, MODULE_FLOOR);
    assert!(
        matches!(
            result,
            Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Binding(
                    "source private non-entry write remains unresolved"
                )
            )) | Err(ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "source private non-entry write remains unresolved"
                    )
                )
            ))
        ),
        "{result:?}"
    );
}

#[test]
fn private_memory_source_scope_preserves_selected_error_unwind_and_owned_callback_backing() {
    for mode in [0u8, 1, 4] {
        let mut visited = 0;
        let (result, _, _, floor, counts, completed) = private_memory_run_v18(
            typed_root_entry_rhs_owner_v18,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |memory, budget| {
                memory.test_nested_exit_v18(mode, budget)?;
                visited += 1;
                Ok(())
            },
        );
        result.unwrap();
        assert!(completed && visited > 0 && counts.into_iter().all(|count| count > 0));
        assert_eq!(floor, MODULE_FLOOR);
    }
}

#[test]
fn private_memory_source_scope_observes_higher_floor_loss_on_error_and_unwind() {
    for mode in [2u8, 3] {
        let mut visited = 0;
        let (result, _, _, retained, counts, completed) = private_memory_run_v18(
            typed_root_entry_rhs_owner_v18,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |memory, budget| {
                memory.test_nested_exit_v18(mode, budget)?;
                visited += 1;
                Ok(())
            },
        );
        assert!(!completed && visited > 0 && counts.into_iter().all(|count| count > 0));
        assert!(retained > MODULE_FLOOR);
        assert!(
            matches!(
                result,
                Err(ProductionSourceOptimizationErrorV18::Source(
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                )) | Err(ProductionSourceOptimizationErrorV18::Adoption(
                    fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                        ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                    )
                ))
            ),
            "{result:?}"
        );
    }
}

#[test]
fn private_memory_direct_object_invocation_rejoins_original_identity_and_slot() {
    for factory in [
        typed_root_entry_rhs_owner_v18 as fn() -> ProductionSemanticSsaOwnerV1,
        typed_entry_rhs_owner_v18,
    ] {
        let ((result, _, _, floor, counts, completed), checked) =
            scoped_raw_admission_v29::with_direct_object_activation_test_v29(|| {
                private_memory_run_v18(
                    factory,
                    OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                    MODULE_LIMIT,
                    |_, _| Ok(()),
                )
            });
        result.unwrap();
        assert!(completed && counts.into_iter().all(|n| n > 0));
        assert!(checked >= 6, "full repeated producer census: {checked}");
        assert_eq!(floor, MODULE_FLOOR);
    }
}

#[test]
fn private_memory_direct_object_invocation_exact_and_short_keep_complete_census() {
    let run = |work, storage| {
        scoped_raw_admission_v29::with_direct_object_activation_test_v29(|| {
            private_memory_run_v18(typed_root_entry_rhs_owner_v18, work, storage, |_, _| Ok(()))
        })
    };
    let ((positive, work, storage, floor, counts, completed), checked) =
        run(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    positive.unwrap();
    assert!(completed && checked >= 6 && counts.into_iter().all(|n| n > 0));
    assert_eq!(floor, MODULE_FLOOR);
    let ((exact, used, peak, floor, exact_counts, completed), exact_checked) = run(work, storage);
    exact.unwrap();
    assert!(completed);
    assert_eq!(
        (used, peak, floor, exact_counts, exact_checked),
        (work, storage, MODULE_FLOOR, counts, checked)
    );
    for (work_limit, storage_limit, expected_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let ((refused, _, _, floor, _, completed), _) = run(work_limit, storage_limit);
        assert!(!completed);
        assert!(refused.is_err());
        assert_eq!(floor, MODULE_FLOOR);
        let error = match refused {
            Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Resource(error),
            ))
            | Err(ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                    ProductionSourceOwnedViewErrorV18::Resource(error),
                ),
            ))
            | Err(ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Resource(error),
            )) => error,
            Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Source(
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
                    ),
                ),
            )) => error,
            other => panic!("exact composition resource refusal required: {other:?}"),
        };
        match (expected_work, error) {
            (true, ArgumentResourceV1::Work(error)) => {
                assert_eq!(error.limit(), work_limit);
                assert!(error.actual() > error.limit());
            }
            (false, ArgumentResourceV1::Storage(error)) => {
                assert_eq!(error.limit(), storage_limit);
                assert!(error.actual() > error.limit());
            }
            _ => panic!("wrong resource boundary: {error:?}"),
        }
    }
}
