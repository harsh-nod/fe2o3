#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrivateNativeCaseV18 {
    Complete,
    OldLifecycle,
    MissingRoot,
    DuplicateRoot,
    ForeignRoot,
    MissingOperation,
    SelectedRootError,
    RootPanic,
    RootFloorError,
    RootFloorPanic,
    ForeignLedger,
    SelectedNativeError,
    NativePanic,
    NativeGrowth,
    NativeFloorError,
    NativeWork,
    NativeStorage,
}

struct PrivateNativeRunV18 {
    outer: ProductionOptimizerTestResultV18,
    observed: Option<Result<(), NativeLifecycleErrorV18>>,
    roots: usize,
    entered: bool,
    selected_root: bool,
    checked_refusal: bool,
    work: usize,
    peak: usize,
    storage: usize,
}

fn private_native_view_error_v18(
    error: fe2o3_kernel_analysis::CanonicalRankedViewErrorV1,
) -> ProductionSourceOwnedViewErrorV18 {
    match error {
        fe2o3_kernel_analysis::CanonicalRankedViewErrorV1::Resource(error) => error.into(),
        other => panic!("the unchanged output must build its structural view: {other:?}"),
    }
}

// This helper is fallible even before the native callback. Its actual owned
// metadata/candidate and borrowed queries all die inside a unit scratch scope;
// the sole observed error copy has its envelope paid before source admission.
fn private_native_ranked_fixture_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    observed: &mut Option<Result<(), NativeLifecycleErrorV18>>,
    run: impl FnOnce(
        &mut fe2o3_kernel_analysis::CheckedCanonicalRankedViewV18<'_, '_, '_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), NativeLifecycleErrorV18>,
) -> SourceOwnedResultV18<()> {
    use fe2o3_kernel_analysis::{
        CanonicalRankedMetadataV18, CanonicalRankedViewErrorV1,
        build_canonical_ranked_candidate_v18, with_checked_canonical_ranked_view_v18,
    };
    source_scalar_normalization_scratch_v18(original.source.cleanup, budget, 0, |budget| {
        let output = optimized.output_inventory(budget)?;
        let metadata = CanonicalRankedMetadataV18::new(output.owner(), &[]);
        let metadata_storage = metadata
            .storage_extent(budget)
            .map_err(private_native_view_error_v18)?;
        budget.reserve_storage(metadata_storage)?;
        let (candidate, receipt) = build_canonical_ranked_candidate_v18(output, &metadata, budget)
            .map_err(private_native_view_error_v18)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let result = with_checked_canonical_ranked_view_v18(
            output,
            &metadata,
            &candidate,
            budget,
            |checked, budget| Ok::<_, CanonicalRankedViewErrorV1>(run(checked, budget)),
        )
        .map_err(private_native_view_error_v18)?;
        *observed = Some(result);
        drop(candidate);
        drop(metadata);
        Ok(())
    })
}

fn private_native_complete_request_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    request: &ProductionSourcePrivateMemoryRootRequestV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    case: PrivateNativeCaseV18,
) -> SourceOwnedResultV18<()> {
    let root = request.root(budget)?;
    original.with_optimized_source_scalar_leaves_v18(optimized, root, budget, |leaves, budget| {
        leaves.with_checked_entry_writes_v18(
            budget,
            |entry, budget| check_fixture_entry_rhs_v18(leaves, entry, budget),
            |entries, budget| {
                request.check_entry_writes(entries, budget)?;
                match case {
                    PrivateNativeCaseV18::DuplicateRoot => {
                        request.check_entry_writes(entries, budget)
                    }
                    PrivateNativeCaseV18::ForeignRoot => {
                        request.test_foreign_root_v18(entries, budget)
                    }
                    PrivateNativeCaseV18::MissingOperation => {
                        request.test_remove_completed_operation_v18(budget)
                    }
                    _ => Ok(()),
                }
            },
        )
    })
}

fn private_native_run_v18(
    factory: fn() -> ProductionSemanticSsaOwnerV1,
    case: PrivateNativeCaseV18,
    work_limit: usize,
    storage_limit: usize,
) -> PrivateNativeRunV18 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    let headers = native_lifecycle_test_envelopes_v18()
        + size_of::<Option<Result<(), NativeLifecycleErrorV18>>>()
        + size_of::<Result<Result<(), ProductionSourceOwnedViewErrorV18>, NativeLifecycleErrorV18>>(
        );
    budget.reserve_storage(MODULE_FLOOR + headers).unwrap();
    let mut observed = None;
    let mut roots = 0;
    let mut entered = false;
    let mut selected_root = false;
    let mut checked_refusal = false;
    let outer = (|| {
        let prepared = private_memory_prepared_v18(factory, &mut budget)?;
        with_production_optimizer_result_v18(
            prepared,
            &mut budget,
            |original, optimized, budget| {
                private_native_ranked_fixture_v18(
                    original,
                    optimized,
                    budget,
                    &mut observed,
                    |checked, budget| {
                        if case == PrivateNativeCaseV18::OldLifecycle {
                            return optimized.with_lifecycle_native_policies_v18(
                                checked,
                                ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
                                budget,
                                |_, _| {
                                    panic!(
                                        "lifecycle-only API completed an actual Memory obligation"
                                    )
                                },
                            );
                        }
                        let count = optimized.output_inventory(budget)?.definitions().len();
                        let result = optimized.with_private_memory_native_policies_v18(
                        checked, ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
                        fe2o3_kernel_analysis::CanonicalKirPrivateMemoryLimitsV1 { max_cells: count }, budget,
                        |request, budget| {
                            let root = request.root(budget)?;
                            assert_eq!(root, roots, "constructor owns the exact ordered root roster");
                            roots += 1;
                            match case {
                                PrivateNativeCaseV18::MissingRoot => return Ok(()),
                                PrivateNativeCaseV18::SelectedRootError => return Err(
                                    ProductionSourceOwnedViewErrorV18::Binding("selected private root callback")),
                                PrivateNativeCaseV18::RootPanic => panic!("selected private root panic"),
                                PrivateNativeCaseV18::RootFloorError | PrivateNativeCaseV18::RootFloorPanic => {
                                    budget.release_storage(1).unwrap();
                                    if case == PrivateNativeCaseV18::RootFloorPanic {
                                        panic!("private root higher-floor panic");
                                    }
                                    return Err(ProductionSourceOwnedViewErrorV18::Binding(
                                        "selected private root callback"));
                                }
                                PrivateNativeCaseV18::ForeignLedger => {
                                    let mut other_work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                                    let mut other = ArgumentBudgetV1::new(&mut other_work, storage_limit);
                                    assert!(matches!(request.root(&mut other), Err(
                                        ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting))));
                                    assert_eq!((other.work(), other.storage()), (0, 0));
                                    let before = (budget.work(), budget.storage());
                                    assert!(matches!(request.root(budget), Err(
                                        ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting))));
                                    assert_eq!((budget.work(), budget.storage()), before);
                                    checked_refusal = true;
                                    return Ok(());
                                }
                                _ => (),
                            }
                            private_native_complete_request_v18(original, optimized, request, budget, case)
                        },
                        |native, budget| {
                            entered = true;
                            native.check_source_subject_v18(original, optimized, budget)?;
                            assert!(!native.ranked_verification_is_complete());
                            assert!(!native.grants_artifact_or_launch_authority());
                            let mut reports = 0;
                            for function in 0..native.function_count(budget)? {
                                if let Some(report) = native.report(function, budget)? {
                                    assert!(report.is_clean());
                                    assert_eq!(report.pass_order().len(), 9);
                                    assert!(native.history(function, budget)?.is_some());
                                    reports += 1;
                                }
                            }
                            assert!(reports > 0);
                            assert!(native.diagnostic(budget)?.last_invocation().is_some());
                            match case {
                                PrivateNativeCaseV18::SelectedNativeError => return Err(
                                    NativeLifecycleErrorV18::Source(ProductionSourceOwnedViewErrorV18::Binding(
                                        "selected private native callback"))),
                                PrivateNativeCaseV18::NativePanic => panic!("selected private native panic"),
                                PrivateNativeCaseV18::NativeGrowth => budget.reserve_storage(1).unwrap(),
                                PrivateNativeCaseV18::NativeFloorError => {
                                    native.test_undercut_completion_floor_v18(budget)?;
                                    return Err(NativeLifecycleErrorV18::Source(
                                        ProductionSourceOwnedViewErrorV18::Binding("selected private native callback")));
                                }
                                PrivateNativeCaseV18::NativeWork => {
                                    budget.charge_work(work_limit - budget.work()).unwrap();
                                    let error = native.function_count(budget).unwrap_err();
                                    let stopped = (budget.work(), budget.storage());
                                    assert!(native.function_count(budget).is_err());
                                    assert_eq!((budget.work(), budget.storage()), stopped);
                                    return Err(error);
                                }
                                PrivateNativeCaseV18::NativeStorage => {
                                    let error = budget.reserve_storage(storage_limit - budget.storage() + 1).unwrap_err();
                                    assert!(matches!(error, ArgumentResourceV1::Storage(error)
                                        if error.limit() == storage_limit && error.actual() == storage_limit + 1));
                                    return Err(NativeLifecycleErrorV18::Source(
                                        original.retain_query_resource_error_v18(error)));
                                }
                                _ => (),
                            }
                            Ok(())
                        });
                        match result {
                            Ok(Err(error)) => {
                                assert_eq!(case, PrivateNativeCaseV18::SelectedRootError);
                                assert!(matches!(
                                    error,
                                    ProductionSourceOwnedViewErrorV18::Binding(
                                        "selected private root callback"
                                    )
                                ));
                                selected_root = true;
                                Err(NativeLifecycleErrorV18::Source(error))
                            }
                            Ok(Ok(())) => Ok(()),
                            Err(error) => Err(error),
                        }
                    },
                )
            },
        )
    })();
    // These are the test runner's independent output envelopes, not source
    // proof backing. A denied source refund must remain visible below.
    budget.release_storage(headers).unwrap();
    PrivateNativeRunV18 {
        outer,
        observed,
        roots,
        entered,
        selected_root,
        checked_refusal,
        work: budget.work(),
        peak: budget.peak_storage(),
        storage: budget.storage(),
    }
}

fn private_native_full_v18(factory: fn() -> ProductionSemanticSsaOwnerV1) -> PrivateNativeRunV18 {
    private_native_run_v18(
        factory,
        PrivateNativeCaseV18::Complete,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
    )
}

fn assert_private_native_positive_v18(run: &PrivateNativeRunV18) {
    assert!(run.outer.is_ok(), "outer: {:?}", run.outer);
    assert!(
        matches!(run.observed, Some(Ok(()))),
        "native: {:?}",
        run.observed
    );
    assert!(run.entered && run.roots > 0);
    assert_eq!(run.storage, MODULE_FLOOR);
}

#[test]
fn private_native_complete_source_root_and_repeated_helper_reach_real_nine_stages() {
    for factory in [
        typed_root_entry_rhs_owner_v18 as fn() -> _,
        typed_entry_rhs_owner_v18,
    ] {
        assert_private_native_positive_v18(&private_native_full_v18(factory));
    }
}

#[test]
fn private_native_safe_readonly_aliases_reach_real_nine_stages() {
    for factory in [
        safe_private_root_owner_v18 as fn() -> _,
        safe_private_helper_owner_v18,
        safe_private_nested_owner_v18,
    ] {
        assert_private_native_positive_v18(&private_native_full_v18(factory));
    }
}

#[test]
fn private_native_safe_aliases_cannot_replace_missing_source_coverage() {
    assert_private_native_positive_v18(&private_native_full_v18(safe_private_root_owner_v18));
    for case in [
        PrivateNativeCaseV18::MissingRoot,
        PrivateNativeCaseV18::MissingOperation,
        PrivateNativeCaseV18::OldLifecycle,
    ] {
        let run = private_native_run_v18(
            safe_private_root_owner_v18,
            case,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        assert!(!run.entered, "incomplete source entered native policies");
        match case {
            PrivateNativeCaseV18::MissingRoot | PrivateNativeCaseV18::MissingOperation => {
                let Some(Err(NativeLifecycleErrorV18::Source(
                    ProductionSourceOwnedViewErrorV18::Binding(detail),
                ))) = &run.observed
                else {
                    panic!("{case:?}: {:?}", run.observed);
                };
                assert_eq!(
                    *detail,
                    if matches!(case, PrivateNativeCaseV18::MissingRoot) {
                        "private native root request was not completed"
                    } else {
                        "private native global physical/source census incomplete"
                    }
                );
                assert!(run.outer.is_err() && run.roots > 0);
            }
            PrivateNativeCaseV18::OldLifecycle => {
                let Some(Err(NativeLifecycleErrorV18::Unresolved(obligation))) = &run.observed
                else {
                    panic!("old lifecycle Memory refusal: {:?}", run.observed);
                };
                assert_eq!(
                    obligation.requirement(),
                    fe2o3_pliron::CanonicalRankedSourceRequirementV18::Memory
                );
                assert!(run.outer.is_ok() && run.roots == 0);
            }
            _ => unreachable!(),
        }
        assert_eq!(run.storage, MODULE_FLOOR);
    }
}

#[test]
fn private_native_does_not_widen_the_old_lifecycle_completed_api() {
    assert_private_native_positive_v18(&private_native_full_v18(typed_root_entry_rhs_owner_v18));
    let run = private_native_run_v18(
        typed_root_entry_rhs_owner_v18,
        PrivateNativeCaseV18::OldLifecycle,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
    );
    let Some(Err(NativeLifecycleErrorV18::Unresolved(obligation))) = &run.observed else {
        panic!("old lifecycle Memory refusal: {:?}", run.observed);
    };
    assert_eq!(
        obligation.requirement(),
        fe2o3_pliron::CanonicalRankedSourceRequirementV18::Memory
    );
    assert!(run.outer.is_ok() && !run.entered && run.roots == 0);
    assert_eq!(run.storage, MODULE_FLOOR);
}

#[test]
fn private_native_keeps_complete_original_lifecycle_subroster_across_two_roots() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let headers = native_lifecycle_test_envelopes_v18()
        + size_of::<Option<Result<(), NativeLifecycleErrorV18>>>()
        + size_of::<Result<Result<(), ProductionSourceOwnedViewErrorV18>, NativeLifecycleErrorV18>>(
        );
    budget.reserve_storage(MODULE_FLOOR + headers).unwrap();
    let prepared = native_lifecycle_prepared_v18(&mut budget);
    let mut observed = None;
    let roots = std::cell::Cell::new(0);
    let semantic_roots = std::cell::Cell::new(0u32);
    let output_roots = std::cell::Cell::new(0usize);
    let mut entered = false;
    with_production_optimizer_result_v18(prepared, &mut budget, |original, optimized, budget| {
        // The historical test name refers to the two extra ordinary roots.
        // Mixed also retains its original lifecycle kernel root (semantic 1).
        let expected_roots = original.source.root_count(budget)?;
        assert_eq!(expected_roots, 3);
        let output = optimized.output_inventory(budget)?;
        let expected_functions = output.functions().len();
        assert_eq!(expected_functions, 3);
        assert!(
            output
                .functions()
                .iter()
                .all(|row| row.function.body.is_some())
        );
        private_native_ranked_fixture_v18(
            original,
            optimized,
            budget,
            &mut observed,
            |checked, budget| {
                optimized
                    .with_private_memory_native_policies_v18(
                        checked,
                        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
                        fe2o3_kernel_analysis::CanonicalKirPrivateMemoryLimitsV1 {
                            max_cells: optimized.output_inventory(budget)?.definitions().len(),
                        },
                        budget,
                        |request, budget| {
                            assert_eq!(request.root(budget)?, roots.get());
                            let (semantic, input_function) =
                                original.source.root(roots.get(), budget)?;
                            let mapped = optimized_source_root_function_v18(
                                original,
                                optimized,
                                roots.get(),
                                budget,
                            )?;
                            assert_eq!(
                                mapped.function.id,
                                original.inventory.functions()[input_function].function.id
                            );
                            let function = mapped.coordinate.0 as usize;
                            assert!(matches!(semantic.index(), 0 | 1 | 4));
                            assert!(function < expected_functions);
                            let semantic_bit = 1u32 << semantic.index();
                            let function_bit = 1usize << function;
                            assert_eq!(semantic_roots.get() & semantic_bit, 0);
                            assert_eq!(output_roots.get() & function_bit, 0);
                            semantic_roots.set(semantic_roots.get() | semantic_bit);
                            output_roots.set(output_roots.get() | function_bit);
                            roots.set(roots.get() + 1);
                            private_native_complete_request_v18(
                                original,
                                optimized,
                                request,
                                budget,
                                PrivateNativeCaseV18::Complete,
                            )
                        },
                        |native, budget| {
                            assert_eq!(roots.get(), expected_roots);
                            assert_eq!(semantic_roots.get(), (1 << 0) | (1 << 1) | (1 << 4));
                            assert_eq!(output_roots.get(), (1 << expected_functions) - 1);
                            assert_eq!(native.function_count(budget)?, expected_functions);
                            for function in 0..expected_functions {
                                let report = native
                                    .report(function, budget)?
                                    .expect("every authentic definition has a native report");
                                assert!(report.is_clean());
                                assert_eq!(report.pass_order().len(), 9);
                                let history = native
                                    .history(function, budget)?
                                    .expect("every authentic definition has invocation history");
                                assert_eq!(history.function(), function);
                            }
                            assert!(native.diagnostic(budget)?.last_invocation().is_some());
                            entered = true;
                            Ok(())
                        },
                    )?
                    .map_err(NativeLifecycleErrorV18::Source)
            },
        )
    })
    .unwrap();
    assert!(matches!(observed, Some(Ok(()))), "{observed:?}");
    assert!(entered && roots.get() == 3);
    budget.release_storage(headers).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn private_native_requires_each_authentic_root_once_and_every_physical_operation() {
    assert_private_native_positive_v18(&private_native_full_v18(typed_entry_rhs_owner_v18));
    for case in [
        PrivateNativeCaseV18::MissingRoot,
        PrivateNativeCaseV18::DuplicateRoot,
        PrivateNativeCaseV18::ForeignRoot,
        PrivateNativeCaseV18::MissingOperation,
    ] {
        let run = private_native_run_v18(
            typed_entry_rhs_owner_v18,
            case,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        assert!(run.roots > 0 && !run.entered, "{case:?}");
        let Some(Err(NativeLifecycleErrorV18::Source(ProductionSourceOwnedViewErrorV18::Binding(
            detail,
        )))) = &run.observed
        else {
            panic!("{case:?}: {:?}", run.observed);
        };
        match case {
            PrivateNativeCaseV18::MissingRoot => {
                assert_eq!(*detail, "private native root request was not completed")
            }
            PrivateNativeCaseV18::DuplicateRoot => {
                assert_eq!(*detail, "private native root completed twice")
            }
            PrivateNativeCaseV18::MissingOperation => assert_eq!(
                *detail,
                "private native global physical/source census incomplete"
            ),
            PrivateNativeCaseV18::ForeignRoot => (),
            _ => unreachable!(),
        }
        assert!(run.outer.is_err());
        assert_eq!(run.storage, MODULE_FLOOR);
    }
}

#[test]
fn private_native_root_selected_error_and_panic_precede_native_history() {
    assert_private_native_positive_v18(&private_native_full_v18(typed_root_entry_rhs_owner_v18));
    for case in [
        PrivateNativeCaseV18::SelectedRootError,
        PrivateNativeCaseV18::RootPanic,
    ] {
        let run = private_native_run_v18(
            typed_root_entry_rhs_owner_v18,
            case,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        assert_eq!(run.roots, 1);
        assert!(!run.entered);
        let error = run.observed.as_ref().unwrap().as_ref().unwrap_err();
        assert!(error.native_diagnostic().is_none());
        if case == PrivateNativeCaseV18::SelectedRootError {
            assert!(run.selected_root);
        } else {
            assert!(
                matches!(
                    error,
                    NativeLifecycleErrorV18::Pending(
                        fe2o3_pliron::CanonicalRankedPolicyFailureV1::Panicked
                    )
                ),
                "{error:?}"
            );
        }
        assert_eq!(run.storage, MODULE_FLOOR);
    }
}

#[test]
fn private_native_final_error_and_panic_keep_actual_native_history() {
    assert_private_native_positive_v18(&private_native_full_v18(typed_root_entry_rhs_owner_v18));
    for case in [
        PrivateNativeCaseV18::SelectedNativeError,
        PrivateNativeCaseV18::NativePanic,
    ] {
        let run = private_native_run_v18(
            typed_root_entry_rhs_owner_v18,
            case,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        assert!(run.entered);
        let error = run.observed.as_ref().unwrap().as_ref().unwrap_err();
        assert!(
            error
                .native_diagnostic()
                .unwrap()
                .last_invocation()
                .is_some()
        );
        match (case, error) {
            (
                PrivateNativeCaseV18::SelectedNativeError,
                NativeLifecycleErrorV18::SourceAfterNative {
                    source:
                        ProductionSourceOwnedViewErrorV18::Binding("selected private native callback"),
                    ..
                },
            ) => (),
            (PrivateNativeCaseV18::NativePanic, NativeLifecycleErrorV18::Native(error)) => {
                assert!(matches!(
                    error.failure(),
                    fe2o3_pliron::CanonicalRankedPolicyFailureV1::Panicked
                ))
            }
            _ => panic!("{case:?}: {error:?}"),
        }
        assert_eq!(run.storage, MODULE_FLOOR);
    }
}

#[test]
fn private_native_higher_floor_loss_cannot_refund_parent_credits() {
    let baseline = private_native_run_v18(
        typed_root_entry_rhs_owner_v18,
        PrivateNativeCaseV18::SelectedRootError,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
    );
    assert!(baseline.selected_root && baseline.storage == MODULE_FLOOR);
    for case in [
        PrivateNativeCaseV18::RootFloorError,
        PrivateNativeCaseV18::RootFloorPanic,
        PrivateNativeCaseV18::NativeFloorError,
    ] {
        let run = private_native_run_v18(
            typed_root_entry_rhs_owner_v18,
            case,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        assert!(run.outer.is_err(), "{case:?}: {:?}", run.observed);
        assert!(
            run.storage > baseline.storage,
            "{case:?}: denied credits were refunded"
        );
        if case == PrivateNativeCaseV18::NativeFloorError {
            assert!(run.entered);
            assert!(
                run.observed
                    .as_ref()
                    .unwrap()
                    .as_ref()
                    .unwrap_err()
                    .native_diagnostic()
                    .unwrap()
                    .last_invocation()
                    .is_some()
            );
        } else {
            assert!(!run.entered);
        }
    }
}

#[test]
fn private_native_consumer_growth_is_refused_by_the_existing_exact_floor_guard() {
    assert_private_native_positive_v18(&private_native_full_v18(typed_root_entry_rhs_owner_v18));
    let run = private_native_run_v18(
        typed_root_entry_rhs_owner_v18,
        PrivateNativeCaseV18::NativeGrowth,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
    );
    assert!(run.entered);
    let Some(Err(NativeLifecycleErrorV18::SourceAfterNative {
        source: ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting),
        diagnostic,
    })) = &run.observed
    else {
        panic!("native exact-floor refusal: {:?}", run.observed);
    };
    assert!(run.outer.is_err());
    assert!(diagnostic.last_invocation().is_some());
    assert_eq!(run.storage, MODULE_FLOOR);
}

#[test]
fn private_native_foreign_ledger_is_no_debit_sticky_and_never_native() {
    let run = private_native_run_v18(
        typed_root_entry_rhs_owner_v18,
        PrivateNativeCaseV18::ForeignLedger,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
    );
    assert!(run.checked_refusal && !run.entered && run.outer.is_err());
    assert!(matches!(
        run.observed,
        Some(Err(NativeLifecycleErrorV18::Source(
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
        )))
    ));
    assert!(run.storage > MODULE_FLOOR);
}

#[test]
fn private_native_post_invocation_resource_refusals_retain_first_error_and_history() {
    assert_private_native_positive_v18(&private_native_full_v18(typed_root_entry_rhs_owner_v18));
    for case in [
        PrivateNativeCaseV18::NativeWork,
        PrivateNativeCaseV18::NativeStorage,
    ] {
        let run = private_native_run_v18(
            typed_root_entry_rhs_owner_v18,
            case,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        assert!(run.entered && run.outer.is_err());
        let Some(Err(NativeLifecycleErrorV18::SourceAfterNative { source, diagnostic })) =
            &run.observed
        else {
            panic!("{case:?}: {:?}", run.observed);
        };
        assert!(diagnostic.last_invocation().is_some());
        match (case, source) {
            (
                PrivateNativeCaseV18::NativeWork,
                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error)),
            ) => assert_eq!(error.limit(), OPTIMIZED_SOURCE_WORK_LIMIT_V18),
            (
                PrivateNativeCaseV18::NativeStorage,
                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error)),
            ) => assert_eq!(error.limit(), MODULE_LIMIT),
            _ => panic!("wrong native resource: {source:?}"),
        }
        assert_eq!(run.storage, MODULE_FLOOR);
    }
}

#[test]
fn private_native_nonentry_writes_stay_unresolved_after_authentic_source_preparation() {
    assert_private_native_positive_v18(&private_native_full_v18(typed_root_entry_rhs_owner_v18));
    let run = private_native_full_v18(private_memory_nonentry_owner_v18);
    assert!(!run.entered && run.outer.is_err());
    assert!(
        matches!(
            run.observed,
            Some(Err(NativeLifecycleErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Binding(
                    "source private non-entry write remains unresolved"
                )
            )))
        ),
        "{:?}",
        run.observed
    );
    assert_eq!(run.storage, MODULE_FLOOR);
}

#[test]
fn private_native_complete_transaction_has_exact_and_both_one_short_limits() {
    private_native_exact_limits_v18(typed_entry_rhs_owner_v18);
}

#[test]
fn private_native_safe_alias_transaction_has_exact_and_both_one_short_limits() {
    private_native_exact_limits_v18(safe_private_nested_owner_v18);
}

fn private_native_exact_limits_v18(factory: fn() -> ProductionSemanticSsaOwnerV1) {
    let ample = private_native_full_v18(factory);
    assert_private_native_positive_v18(&ample);
    let exact = private_native_run_v18(
        factory,
        PrivateNativeCaseV18::Complete,
        ample.work,
        ample.peak,
    );
    assert_private_native_positive_v18(&exact);
    assert_eq!(
        (exact.work, exact.peak, exact.roots),
        (ample.work, ample.peak, ample.roots)
    );
    for (work, storage, work_short) in [
        (ample.work - 1, ample.peak, true),
        (ample.work, ample.peak - 1, false),
    ] {
        let run = private_native_run_v18(factory, PrivateNativeCaseV18::Complete, work, storage);
        assert!(run.outer.is_err() || !matches!(run.observed, Some(Ok(()))));
        assert_eq!(run.storage, MODULE_FLOOR);
        let resource = match &run.observed {
            Some(Err(NativeLifecycleErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Resource(error),
            )))
            | Some(Err(NativeLifecycleErrorV18::SourceAfterNative {
                source: ProductionSourceOwnedViewErrorV18::Resource(error),
                ..
            })) => *error,
            _ => match &run.outer {
                Err(ProductionSourceOptimizationErrorV18::Source(
                    ProductionSourceOwnedViewErrorV18::Resource(error),
                ))
                | Err(ProductionSourceOptimizationErrorV18::Adoption(
                    fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Resource(error),
                ))
                | Err(ProductionSourceOptimizationErrorV18::Adoption(
                    fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                        ProductionSourceOwnedViewErrorV18::Resource(error),
                    ),
                )) => *error,
                Err(ProductionSourceOptimizationErrorV18::Source(
                    ProductionSourceOwnedViewErrorV18::Source(
                        ProductionPendingScopedSourceErrorV29::Source(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
                        ),
                    ),
                )) => *error,
                other => panic!(
                    "whole transaction resource boundary: {other:?}, {:?}",
                    run.observed
                ),
            },
        };
        match (work_short, resource) {
            (true, ArgumentResourceV1::Work(error)) => {
                assert_eq!(error.limit(), work);
                assert!(error.actual() > error.limit());
            }
            (false, ArgumentResourceV1::Storage(error)) => {
                assert_eq!(error.limit(), storage);
                assert!(error.actual() > error.limit());
            }
            _ => panic!("wrong exact resource boundary: {resource:?}"),
        }
    }
}
