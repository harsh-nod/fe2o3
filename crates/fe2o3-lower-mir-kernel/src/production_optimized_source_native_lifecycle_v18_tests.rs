type NativeLifecycleErrorV18 = ProductionSourceNativeLifecycleErrorV18;

fn native_lifecycle_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    let base = module_fixture_owner(ModuleFixture::Mixed);
    let semantic = base.source_semantic();
    let mut functions = semantic.functions().to_vec();
    // This positive has genuine original lifecycle calls but no independent
    // assertion role. The unresolved-role controls use their original corpus.
    for (index, tag) in [(0, 60), (4, 150)] {
        let prior = &functions[index];
        functions[index] = function(
            tag,
            prior.role(),
            prior.abi().clone(),
            prior.locals().to_vec(),
            vec![block(tag + 4, vec![], SemanticTerminatorKindV1::Return)],
        )
        .with_kernel_entry(prior.kernel_entry().unwrap().clone());
    }
    // The two removed assertions were the only users of the fixture's
    // appended Boolean type. Keep the admitted type closure exact.
    let mut types = semantic.types().to_vec();
    let assertion_type = types.pop().unwrap();
    assert!(matches!(
        assertion_type.shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
    ));
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
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

fn native_lifecycle_prepared_v18(budget: &mut ArgumentBudgetV1<'_>) -> ProductionPreparedSourceV18 {
    let projection = native_lifecycle_owner_v18();
    let owner = native_lifecycle_owner_v18();
    let (_, launch) =
        with_module_fixture_view(&owner, ModuleFixture::Mixed, budget, |_, _| ()).unwrap();
    with_module_fixture_view(
        &projection,
        ModuleFixture::Mixed,
        budget,
        |source, budget| {
            ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                owner,
                launch,
                source.input,
                ProductionSemanticKirLimitsV1::default(),
                budget,
            )
            .unwrap()
        },
    )
    .unwrap()
    .0
}

fn with_native_ranked_test_v18(
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    run: impl FnOnce(
        &mut fe2o3_kernel_analysis::CheckedCanonicalRankedViewV18<'_, '_, '_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), NativeLifecycleErrorV18>,
) -> Result<(), NativeLifecycleErrorV18> {
    use fe2o3_kernel_analysis::{
        CanonicalRankedMetadataV18, CanonicalRankedViewErrorV1,
        build_canonical_ranked_candidate_v18, with_checked_canonical_ranked_view_v18,
    };
    let output = optimized.output_inventory(budget)?;
    let metadata = CanonicalRankedMetadataV18::new(output.owner(), &[]);
    let metadata_storage = metadata.storage_extent(budget).unwrap();
    budget.reserve_storage(metadata_storage).unwrap();
    let (candidate, receipt) =
        build_canonical_ranked_candidate_v18(output, &metadata, budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let result = with_checked_canonical_ranked_view_v18(
        output,
        &metadata,
        &candidate,
        budget,
        |checked, budget| Ok::<_, CanonicalRankedViewErrorV1>(run(checked, budget)),
    )
    .unwrap();
    drop(candidate);
    drop(metadata);
    // The test runner owns only these two concrete constructor objects. Tested
    // callbacks below cannot export backing or release either object's credit.
    budget
        .release_storage(metadata_storage + receipt.retained_storage())
        .unwrap();
    result
}

fn native_lifecycle_test_envelopes_v18() -> usize {
    size_of::<NativeLifecycleErrorV18>()
        + size_of::<Result<(), NativeLifecycleErrorV18>>()
        + size_of::<
            Result<
                Result<(), NativeLifecycleErrorV18>,
                fe2o3_kernel_analysis::CanonicalRankedViewErrorV1,
            >,
        >()
        + 2 * size_of::<std::cell::Cell<Option<ProductionSourceNativeLifecycleDiagnosticV18>>>()
        + size_of::<Result<ProductionSourceNativeLifecycleDiagnosticV18, NativeLifecycleErrorV18>>()
}

#[test]
fn original_lifecycle_native_consumer_uses_actual_nine_stage_reports() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let headers = native_lifecycle_test_envelopes_v18();
    budget.reserve_storage(MODULE_FLOOR + headers).unwrap();
    let prepared = native_lifecycle_prepared_v18(&mut budget);
    let entered = std::cell::Cell::new(false);
    with_actual_optimized_source_v18(prepared, &mut budget, |optimized, budget| {
        with_native_ranked_test_v18(optimized, budget, |checked, budget| {
            optimized.with_lifecycle_native_policies_v18(
                checked,
                ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
                budget,
                |policies, budget| {
                    assert!(policies.lifecycle_source_roles_are_complete());
                    assert!(!policies.ranked_verification_is_complete());
                    assert!(!policies.grants_artifact_or_launch_authority());
                    let mut definitions = 0;
                    for function in 0..policies.function_count(budget)? {
                        if let Some(report) = policies.report(function, budget)? {
                            assert!(report.is_clean());
                            assert_eq!(report.pass_order().len(), 9);
                            assert!(policies.history(function, budget)?.is_some());
                            definitions += 1;
                        }
                    }
                    assert!(definitions > 0);
                    assert!(policies.observation(budget)?.work_upper_bound() > 0);
                    entered.set(true);
                    Ok(())
                },
            )
        })
        .unwrap();
        Ok(())
    })
    .unwrap();
    assert!(entered.get());
    budget.release_storage(headers).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn original_lifecycle_native_census_rejects_same_candidate_missing_duplicate_foreign_and_kind_changes()
 {
    for fault in 0..6 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let headers = native_lifecycle_test_envelopes_v18();
        budget.reserve_storage(MODULE_FLOOR + headers).unwrap();
        let prepared = native_lifecycle_prepared_v18(&mut budget);
        let entered = std::cell::Cell::new(false);
        let verified = std::cell::Cell::new(false);
        let result =
            with_actual_optimized_source_v18(prepared, &mut budget, |optimized, budget| {
                let result = with_native_ranked_test_v18(optimized, budget, |checked, budget| {
                    optimized.test_native_lifecycle_join_v18(
                        checked,
                        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
                        budget,
                        fault,
                        &verified,
                    )
                });
                entered.set(true);
                match result {
                    Err(NativeLifecycleErrorV18::Source(
                        error @ ProductionSourceOwnedViewErrorV18::Binding(_),
                    )) => Err(error),
                    other => panic!("native census fault={fault}: {other:?}"),
                }
            });
        assert!(entered.get());
        assert!(
            verified.get(),
            "source first refusal cannot mask a failed hostile census oracle"
        );
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(_))
        ));
        budget.release_storage(headers).unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn original_lifecycle_native_scope_keeps_selected_error_panic_and_exact_floor_contract() {
    for mode in 0..4 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let headers = native_lifecycle_test_envelopes_v18();
        budget.reserve_storage(MODULE_FLOOR + headers).unwrap();
        let prepared = native_lifecycle_prepared_v18(&mut budget);
        let entered = std::cell::Cell::new(false);
        let disposition_verified = std::cell::Cell::new(false);
        let result = with_actual_optimized_source_v18(
            prepared,
            &mut budget,
            |optimized, budget| {
                let result = with_native_ranked_test_v18(optimized, budget, |checked, budget| {
                    optimized.with_lifecycle_native_policies_v18(checked,
                    ProductionSemanticKirLimitsV1::default().storage_layout_limits(), budget,
                    |policies, budget| {
                        assert!(policies.function_count(budget)? > 0);
                        entered.set(true);
                        if mode == 3 {
                            assert!(matches!(policies.report(usize::MAX, budget),
                                Err(NativeLifecycleErrorV18::Pending(fe2o3_pliron::CanonicalRankedPolicyFailureV1::InvalidQuery { function })) if function == usize::MAX));
                            panic!("later panic after the first native query refusal");
                        }
                        if mode == 1 { panic!("same-candidate native consumer panic"); }
                        if mode == 2 { budget.release_storage(1).unwrap(); }
                        Err(NativeLifecycleErrorV18::Source(ProductionSourceOwnedViewErrorV18::Binding(
                            "selected native consumer error")))
                    })
                });
                match (mode, result) {
                    (
                        0,
                        Err(NativeLifecycleErrorV18::SourceAfterNative {
                            source:
                                ProductionSourceOwnedViewErrorV18::Binding(
                                    "selected native consumer error",
                                ),
                            diagnostic,
                        }),
                    ) => {
                        assert!(diagnostic.last_invocation().is_some());
                        Ok(())
                    }
                    (1, Err(NativeLifecycleErrorV18::Native(error))) => {
                        assert!(matches!(
                            error.failure(),
                            fe2o3_pliron::CanonicalRankedPolicyFailureV1::Panicked
                        ));
                        assert!(error.last_invocation().is_some());
                        assert!(error.observation().work_upper_bound() > 0);
                        Ok(())
                    }
                    (3, Err(NativeLifecycleErrorV18::Native(error))) => {
                        assert!(
                            matches!(error.failure(), fe2o3_pliron::CanonicalRankedPolicyFailureV1::InvalidQuery { function } if *function == usize::MAX)
                        );
                        assert!(error.last_invocation().is_some());
                        Ok(())
                    }
                    (
                        2,
                        Err(NativeLifecycleErrorV18::SourceAfterNative {
                            source:
                                error @ ProductionSourceOwnedViewErrorV18::Resource(
                                    ArgumentResourceV1::Accounting,
                                ),
                            diagnostic,
                        }),
                    ) => {
                        assert!(diagnostic.last_invocation().is_some());
                        disposition_verified.set(true);
                        Err(error)
                    }
                    (_, other) => panic!("native disposition {mode}: {other:?}"),
                }
            },
        );
        assert!(entered.get());
        if mode == 2 {
            assert!(disposition_verified.get());
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
        } else {
            result.unwrap();
        }
        budget.release_storage(headers).unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn original_lifecycle_native_resource_refusal_preserves_actual_history_through_source_cleanup() {
    for storage_short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let headers = native_lifecycle_test_envelopes_v18();
        budget.reserve_storage(MODULE_FLOOR + headers).unwrap();
        let prepared = native_lifecycle_prepared_v18(&mut budget);
        let diagnostic = std::cell::Cell::new(None);
        let entered = std::cell::Cell::new(false);
        let completed = std::cell::Cell::new(false);
        let result = with_actual_optimized_source_v18(
            prepared,
            &mut budget,
            |optimized, budget| {
                let original = optimized.original_source(budget)?;
                let result = with_native_ranked_test_v18(optimized, budget, |checked, budget| {
                    let layouts = ProductionSemanticKirLimitsV1::default().storage_layout_limits();
                    // The same checked output and source recipe first complete the
                    // genuine native run, then receive the hostile resource cut.
                    optimized.with_lifecycle_native_policies_v18(
                        checked,
                        layouts,
                        budget,
                        |policies, budget| {
                            assert!(policies.diagnostic(budget)?.last_invocation().is_some());
                            Ok(())
                        },
                    )?;
                    optimized.with_lifecycle_native_policies_v18(checked, layouts, budget, |policies, budget| {
                    let before = policies.diagnostic(budget)?;
                    assert!(before.last_invocation().is_some());
                    entered.set(true);
                    if storage_short {
                        let refusal = budget.reserve_storage(MODULE_LIMIT - budget.storage() + 1).unwrap_err();
                        assert!(matches!(refusal, ArgumentResourceV1::Storage(error) if error.actual() == MODULE_LIMIT + 1));
                        Err(NativeLifecycleErrorV18::Source(original.retain_query_resource_error_v18(refusal)))
                    } else {
                        budget.charge_work(OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work()).unwrap();
                        policies.function_count(budget).map(|_| ())
                    }
                })
                });
                match result {
                    Err(NativeLifecycleErrorV18::SourceAfterNative {
                        source,
                        diagnostic: observed,
                    }) => {
                        assert!(observed.last_invocation().is_some());
                        assert!(observed.observation().work_upper_bound() > 0);
                        diagnostic.set(Some(observed));
                        match source {
                            ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Storage(_),
                            ) if storage_short => (),
                            ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Work(_),
                            ) if !storage_short => (),
                            other => panic!("wrong native resource primary: {other:?}"),
                        }
                        completed.set(true);
                        Err(source)
                    }
                    other => panic!("native resource history was erased: {other:?}"),
                }
            },
        );
        assert!(entered.get());
        assert!(
            completed.get(),
            "source first refusal cannot hide failed diagnostic assertions"
        );
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Resource(_))
        ));
        assert!(diagnostic.get().unwrap().last_invocation().is_some());
        budget.release_storage(headers).unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn original_lifecycle_native_header_exact_and_one_short_are_independent_boundaries() {
    for short in [false, true] {
        run_native_lifecycle_header_boundary_v18(short, false);
    }
}

#[test]
fn original_lifecycle_native_attempt_header_refusal_stays_first_after_credit_is_restored() {
    run_native_lifecycle_header_boundary_v18(true, true);
}

fn run_native_lifecycle_header_boundary_v18(short: bool, attempt_only: bool) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let headers = native_lifecycle_test_envelopes_v18();
    budget.reserve_storage(MODULE_FLOOR + headers).unwrap();
    let prepared = native_lifecycle_prepared_v18(&mut budget);
    let verified = std::cell::Cell::new(false);
    let result = with_actual_optimized_source_v18(prepared, &mut budget, |optimized, budget| {
        let result = with_native_ranked_test_v18(optimized, budget, |checked, budget| {
            optimized.test_native_lifecycle_header_v18(
                checked,
                ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
                budget,
                MODULE_LIMIT,
                short,
                attempt_only,
                &verified,
            )
        });
        match result {
            Err(NativeLifecycleErrorV18::Source(
                error @ ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(_)),
            )) => Err(error),
            other => {
                panic!("native header cut lost its exact constructor refusal: {other:?}")
            }
        }
    });
    assert!(verified.get());
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Storage(_)
        ))
    ));
    budget.release_storage(headers).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn original_lifecycle_native_repeated_scope_has_constant_peak_and_exact_live_floor() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let headers = native_lifecycle_test_envelopes_v18();
    budget.reserve_storage(MODULE_FLOOR + headers).unwrap();
    let prepared = native_lifecycle_prepared_v18(&mut budget);
    with_actual_optimized_source_v18(prepared, &mut budget, |optimized, budget| {
        with_native_ranked_test_v18(optimized, budget, |checked, budget| {
            let floor = budget.storage();
            let mut peak = None;
            for _ in 0..3 {
                optimized.with_lifecycle_native_policies_v18(
                    checked,
                    ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
                    budget,
                    |policies, budget| {
                        assert!(policies.diagnostic(budget)?.last_invocation().is_some());
                        Ok(())
                    },
                )?;
                assert_eq!(budget.storage(), floor);
                if let Some(peak) = peak {
                    assert_eq!(budget.peak_storage(), peak);
                } else {
                    peak = Some(budget.peak_storage());
                }
            }
            Ok(())
        })
        .unwrap();
        Ok(())
    })
    .unwrap();
    budget.release_storage(headers).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}
