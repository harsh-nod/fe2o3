impl ProductionOptimizedSourceCorrespondenceV18<'_> {
    pub(in super::super::super) fn original_for_test_v30(
        &self,
    ) -> &ProductionSourceCorrespondenceV18<'_> {
        self.original
    }
}

thread_local! {
    static SELECTED_FINAL_CONSTRUCTION_ACCOUNTING_V30: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SELECTED_FINAL_ATTEMPT_STORAGE_V30: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn construction_projection_for_test_v30<'a>(
    mut projection: index::ProjectionIndexV30<'a>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<index::ProjectionIndexV30<'a>> {
    if SELECTED_FINAL_CONSTRUCTION_ACCOUNTING_V30.with(|fault| fault.replace(false)) {
        assert!(!projection.accesses.is_empty());
        budget.charge_work(projection.accesses.len())?;
        // Corrupt only the private derived index, after genuine owner/F replay.
        // The unmodified constructor must detect its missing dense access row.
        projection.accesses.clear();
    }
    Ok(projection)
}

fn observe_attempt_result_for_test_v30(budget: &ArgumentBudgetV1<'_>) {
    SELECTED_FINAL_ATTEMPT_STORAGE_V30.with(|storage| storage.set(budget.storage()));
}

pub(in super::super::super) fn raw_result_custody_v30(
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    case: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    assert!(case < 4);
    let initial = budget.storage();
    let callbacks = std::cell::Cell::new(0);
    let callback_floor = std::cell::Cell::new(0);
    let attempts = std::cell::Cell::new(0);
    SELECTED_FINAL_ATTEMPT_STORAGE_V30.with(|storage| storage.set(0));
    SELECTED_FINAL_CONSTRUCTION_ACCOUNTING_V30.with(|fault| fault.set(case == 2));
    let result = fe2o3_kernel_ir::with_canonical_selected_slice_domains_v30(
        optimized.checked.output().owner(),
        &[fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
            rank: 3,
            extents: [64, 1, 1],
        }],
        fe2o3_kernel_ir::FormalIndexWidth::Bits64,
        Default::default(),
        budget,
        |domains, budget| {
            let floor = budget.storage();
            let result =
                optimized.with_selected_final_sources_v30(domains, 0, budget, |_, budget| {
                    callbacks.set(callbacks.get() + 1);
                    callback_floor.set(budget.storage());
                    if case == 0 {
                        Err::<(), _>(ProductionSourceOwnedViewErrorV18::Binding(
                            "selected test callback binding",
                        ))
                    } else {
                        if case == 3 {
                            assert!(matches!(
                                optimized
                                    .original
                                    .source
                                    .missing::<()>("selected test first binding"),
                                Err(ProductionSourceOwnedViewErrorV18::Binding(
                                    "selected test first binding"
                                ))
                            ));
                        }
                        Err(ArgumentResourceV1::Accounting.into())
                    }
                });
            attempts.set(attempts.get() + 1);
            let at_return = SELECTED_FINAL_ATTEMPT_STORAGE_V30.with(std::cell::Cell::get);
            assert!(at_return > floor);
            assert_eq!(callbacks.get(), usize::from(case != 2));
            if case == 0 {
                assert!(matches!(
                    result,
                    Err(ProductionSourceOwnedViewErrorV18::Binding(
                        "selected test callback binding"
                    ))
                ));
                assert_eq!(budget.storage(), floor);
                assert!(!optimized.original.source.cleanup.is_denied());
                domains.owner(budget)?;
            } else {
                if case == 3 {
                    assert!(
                        matches!(
                            result,
                            Err(ProductionSourceOwnedViewErrorV18::Binding(
                                "selected test first binding"
                            ))
                        ),
                        "{result:?}"
                    );
                } else {
                    assert!(
                        matches!(
                            result,
                            Err(ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Accounting
                            ))
                        ),
                        "{result:?}"
                    );
                }
                assert_eq!(
                    budget.storage(),
                    at_return,
                    "no credit refunded by source attempt"
                );
                if case != 2 {
                    assert_eq!(at_return, callback_floor.get());
                }
                assert!(optimized.original.source.cleanup.is_denied());
                let before = budget.work();
                assert!(matches!(
                    domains.owner(budget),
                    Err(
                        fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1::Resource(
                            fe2o3_kernel_ir::FormalGuardedMemoryResourceErrorV1::Accounting
                        )
                    )
                ));
                let retry: SourceOwnedResultV18<()> =
                    optimized.with_selected_final_sources_v30(domains, 0, budget, |_, _| {
                        panic!("accounting-poisoned source callback repeated")
                    });
                if case == 3 {
                    assert!(matches!(
                        retry,
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "selected test first binding"
                        ))
                    ));
                } else {
                    assert!(matches!(
                        retry,
                        Err(ProductionSourceOwnedViewErrorV18::Resource(
                            ArgumentResourceV1::Accounting
                        ))
                    ));
                }
                assert_eq!((budget.work(), budget.storage()), (before, at_return));
            }
            Ok(result)
        },
    );
    assert_eq!(attempts.get(), 1);
    assert!(!SELECTED_FINAL_CONSTRUCTION_ACCOUNTING_V30.with(std::cell::Cell::get));
    if case == 0 {
        assert_eq!(budget.storage(), initial);
    } else {
        assert_eq!(
            budget.storage(),
            SELECTED_FINAL_ATTEMPT_STORAGE_V30.with(std::cell::Cell::get),
            "no credit refunded by actual-domain scope"
        );
    }
    match result {
        Err(error) => Err(optimized_source_observed_formal_error_v18(
            optimized.original,
            &error,
        )),
        Ok(Some(result)) => result,
        Ok(None) => panic!("raw callback baseline was not selected"),
    }
}

pub(in super::super::super) fn assert_join_v30(
    view: &CheckedSelectedFinalSourcesV30<'_>,
    expected: usize,
    mixed: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    assert_eq!(view.original_access_count(budget)?, expected);
    assert_eq!(view.source.len(), expected);
    assert!(!view.grants_artifact_or_launch_authority());
    assert!(
        !view
            .domains
            .source_and_runtime_requirements_are_discharged()
    );
    let mut issued = 0;
    let mut descriptors = 0;
    let mut count = 0;
    for access in &view.rows.accesses {
        let original = &view.source[access.original];
        match access.disposition {
            ProductionOptimizedSourceOperationV18::Retained { output, .. } => {
                let actual = formal(
                    view.optimized.original,
                    view.domains.choices_at(output, budget),
                )?
                .unwrap();
                let choices = &view.rows.choices[access.choices.range()?];
                assert!(!choices.is_empty());
                let mut covered = vec![false; actual.len()];
                for join in choices {
                    covered[join.actual] = true;
                    let leaf = &original.leaves[join.original_leaf];
                    let guard = &original.guards[join.original_guard];
                    assert_eq!(guard.leaf, join.original_leaf);
                    match leaf.origin {
                        PendingSourceSelectedLeafOriginV30::Issued(_) => issued += 1,
                        PendingSourceSelectedLeafOriginV30::Descriptor(_) => descriptors += 1,
                    }
                    let obligations = &view.rows.obligations[join.obligations.range()?];
                    assert!(!obligations.is_empty());
                    for &ordinal in obligations {
                        assert_eq!(original.obligations[ordinal].leaf, join.original_leaf);
                    }
                    count += 1;
                }
                assert!(covered.into_iter().all(|row| row));
                assert!(!view.rows.edges[access.edges.range()?].is_empty());
            }
            _ => {
                assert_eq!(access.choices.count, 0);
                assert_eq!(access.edges.count, 0);
            }
        }
    }
    assert_eq!(count, view.retained_choice_count(budget)?);
    assert!(issued > 0);
    if mixed {
        assert!(descriptors > 0);
    } else {
        assert_eq!(descriptors, 0);
    }
    Ok(())
}

pub(in super::super::super) fn hostile_index_v30(
    view: &CheckedSelectedFinalSourcesV30<'_>,
    fault: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let floor = budget.storage();
    budget.reserve_storage(index::ProjectionIndexV30::headers()? + build::headers()?)?;
    let mut index = index::ProjectionIndexV30::build(
        view.optimized,
        view.root,
        view.transport,
        view.source,
        budget,
    )?;
    index.test_corrupt_v30(fault, budget)?;
    let error = match build::build(
        view.optimized,
        view.domains,
        view.function,
        &index,
        view.source,
        budget,
    ) {
        Ok(_) => panic!("hostile selected source index accepted {fault}"),
        Err(error) => error,
    };
    let expected = match fault {
        0 | 3 => "selected final conditional choice lost its exact source leaf or guard",
        1 => "selected final actual edge has no exact source occurrence",
        2 => "selected final actual edge changed its source endpoints",
        _ => unreachable!(),
    };
    assert!(
        matches!(error, ProductionSourceOwnedViewErrorV18::Binding(detail) if detail == expected),
        "fault={fault}: {error:?}"
    );
    drop(index);
    budget.release_storage(budget.storage() - floor)?;
    // This is private synthetic-index rejection, not public scope recovery.
    assert_join_v30(view, 2, false, budget)
}

pub(in super::super::super) fn foreign_domain_owner_v30(
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let floor = budget.storage();
    let (foreign, receipt) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        optimized.checked.output().owner().module(), ProductionSemanticKirLimitsV1::default().storage_layout_limits, budget,
    ).unwrap();
    budget.reserve_storage(receipt.retained_storage())?;
    assert_eq!(
        foreign.canonical_bytes(),
        optimized.checked.output().owner().canonical_bytes()
    );
    assert!(!std::ptr::eq(&foreign, optimized.checked.output().owner()));
    let reached = std::cell::Cell::new(false);
    let result = fe2o3_kernel_ir::with_canonical_selected_slice_domains_v30(
        &foreign,
        &[fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
            rank: 3,
            extents: [64, 1, 1],
        }],
        fe2o3_kernel_ir::FormalIndexWidth::Bits64,
        Default::default(),
        budget,
        |domains, budget| {
            Ok(
                optimized.with_selected_final_sources_v30(domains, 0, budget, |_, _| {
                    reached.set(true);
                    Ok(())
                }),
            )
        },
    )
    .unwrap()
    .expect("the byte-identical foreign graph has genuine actual bounds");
    assert!(!reached.get());
    drop(foreign);
    budget.release_storage(receipt.retained_storage())?;
    assert_eq!(budget.storage(), floor);
    result
}

pub(in super::super::super) fn poison_join_v30(
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    lost_credit: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let reached = std::cell::Cell::new(false);
    let result = fe2o3_kernel_ir::with_canonical_selected_slice_domains_v30(
        optimized.checked.output().owner(),
        &[fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
            rank: 3,
            extents: [64, 1, 1],
        }],
        fe2o3_kernel_ir::FormalIndexWidth::Bits64,
        Default::default(),
        budget,
        |domains, budget| {
            Ok(
                optimized.with_selected_final_sources_v30(domains, 0, budget, |view, budget| {
                    let floor = budget.storage();
                    let error = if lost_credit {
                        budget.release_storage(1)?;
                        let error = view.original_access_count(budget).unwrap_err();
                        budget.reserve_storage(1)?;
                        error
                    } else {
                        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                        let mut foreign = ArgumentBudgetV1::new(&mut work, usize::MAX);
                        foreign.reserve_storage(floor)?;
                        let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
                        let error = view.original_access_count(&mut foreign).unwrap_err();
                        assert_eq!(
                            (foreign.work(), foreign.storage(), foreign.peak_storage()),
                            before
                        );
                        error
                    };
                    assert!(
                        matches!(
                            error,
                            ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Accounting
                            )
                        ),
                        "{error:?}"
                    );
                    assert_eq!(budget.storage(), floor);
                    let work = budget.work();
                    assert!(matches!(
                        view.original_access_count(budget),
                        Err(ProductionSourceOwnedViewErrorV18::Resource(
                            ArgumentResourceV1::Accounting
                        ))
                    ));
                    assert_eq!(budget.work(), work);
                    reached.set(true);
                    Ok(())
                }),
            )
        },
    );
    assert!(reached.get());
    match result {
        Err(error) => Err(optimized_source_observed_formal_error_v18(
            optimized.original,
            &error,
        )),
        Ok(Some(result)) => result,
        Ok(None) => panic!("poison baseline was not selected"),
    }
}

#[test]
fn selected_final_named_scope_frames_have_independent_field_and_result_oracles() {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<SourceOwnedResultV18<T>>()
    }
    type Capture<'a> = (
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        &'a Domains<'a, 'a>,
        usize,
        [u64; 3],
    );
    type Execution<'a> = (Capture<'a>, &'a mut ArgumentBudgetV1<'a>);
    type Scope<'a> = (
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        &'a Domains<'a, 'a>,
        usize,
        FunctionCoordinate,
        &'a [PendingSourceSelectedAccessV30],
        &'a [SelectedTransportRowV30],
        &'a SelectedFinalRowsV30,
        DescriptorRoleScopeV18,
    );
    assert_eq!(
        size_of::<SelectedFinalCaptureV30<'_, [u64; 3]>>(),
        size_of::<Capture<'_>>()
    );
    assert_eq!(
        size_of::<SelectedFinalExecutionV30<'_, '_, '_, [u64; 3]>>(),
        size_of::<Execution<'_>>()
    );
    assert_eq!(
        size_of::<CheckedSelectedFinalSourcesV30<'_>>(),
        size_of::<Scope<'_>>()
    );
    let expected = index::ProjectionIndexV30::headers().unwrap()
        + build::headers().unwrap()
        + h::<Capture<'_>>()
        + h::<Execution<'_>>()
        + h::<Scope<'_>>()
        + h::<DescriptorRoleScopeV18>()
        + h::<&[PendingSourceSelectedAccessV30]>()
        + h::<&[SelectedTransportRowV30]>()
        + h::<&fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>>()
        + h::<&fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18>()
        + h::<
            Result<
                &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
                fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
            >,
        >()
        + h::<u64>()
        + h::<SourceOwnedResultV18<u64>>()
        + h::<Range<usize>>()
        + h::<&index::ProjectionIndexV30<'_>>()
        + h::<&SelectedFinalRowsV30>()
        + h::<&CheckedSelectedFinalSourcesV30<'_>>()
        + 4 * h::<usize>()
        + h::<()>();
    assert_eq!(headers::<u64, [u64; 3]>().unwrap(), expected);
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let limit = 17 + expected - usize::from(short);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        budget.reserve_storage(17).unwrap();
        let result = budget.reserve_storage(headers::<u64, [u64; 3]>().unwrap());
        if short {
            assert!(
                matches!(result, Err(ArgumentResourceV1::Storage(error)) if error.actual() == 17 + expected && error.limit() == limit)
            );
        } else {
            result.unwrap();
            budget.release_storage(expected).unwrap();
        }
        assert_eq!(budget.storage(), 17);
    }
}
