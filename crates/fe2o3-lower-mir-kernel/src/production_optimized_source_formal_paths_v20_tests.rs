use fe2o3_kernel_analysis::{
    FormalPaidPathDecisionV20 as Decision, PresburgerQueryErrorV2 as Query,
    PresburgerQueryLimitsV2, PresburgerQueryResourceV2,
};
use fe2o3_kernel_ir::{
    CanonicalFormalLaunchInputV19 as Launch, ControlFlowLimits, ExplicitLaunchExtent,
    FormalIndexWidth,
};
use std::cell::Cell;

fn launches(
    original: &ProductionSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Vec<Launch> {
    vec![
        Launch::PhysicalEnvelope(ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [8, 1, 1]
        });
        original.source.root_count(budget).unwrap()
    ]
}

#[test]
fn paired_paths_visit_every_real_original_and_optimized_root_in_order() {
    for (factory, changed) in [
        (integer_add_source_v18 as fn() -> _, true),
        (integer_non_neutral_source_v18, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(factory, &mut budget);
        let completed = Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let floor = budget.storage();
            let (output, (), receipt) = source
                .with_checked_integer_optimization_v18(budget, |original, optimized, budget| {
                    let input = original.source.canonical(budget)?;
                    let output = optimized.output_inventory(budget)?.owner();
                    let launches = launches(original, budget);
                    let retained = budget.storage();
                    let mut visits = 0;
                    original
                        .with_optimized_formal_paths_v20(
                            optimized,
                            &launches,
                            FormalIndexWidth::Bits64,
                            ControlFlowLimits::DEFAULT,
                            budget,
                            |side, before, after, path, _| {
                                assert!(std::ptr::eq(before.original_owner(), input));
                                assert!(std::ptr::eq(after.original_owner(), output));
                                assert!(!std::ptr::eq(input, output));
                                assert_eq!(before.root_index(), visits / 2);
                                assert_eq!(after.root_index(), visits / 2);
                                assert_eq!(path.root_index(), visits / 2);
                                assert_eq!(
                                    side,
                                    if visits % 2 == 0 {
                                        ProductionFormalPathSideV20::Original
                                    } else {
                                        ProductionFormalPathSideV20::Optimized
                                    }
                                );
                                let report = if side == ProductionFormalPathSideV20::Original {
                                    before.analysis()
                                } else {
                                    after.analysis()
                                };
                                let owner = if side == ProductionFormalPathSideV20::Original {
                                    input
                                } else {
                                    output
                                };
                                assert!(std::ptr::eq(path.original_owner(), owner));
                                assert!(std::ptr::eq(path.analysis(), report));
                                assert_eq!(path.launch_input(), launches[visits / 2]);
                                assert_eq!(path.index_width(), FormalIndexWidth::Bits64);
                                let conflicts = report.obligations().inter_invocation_conflicts();
                                assert_eq!(path.observations().len(), conflicts.len());
                                for (ordinal, row) in path.observations().iter().enumerate() {
                                    assert_eq!(row.ordinal(), ordinal);
                                    assert_eq!(row.requirement(), conflicts[ordinal]);
                                }
                                assert_eq!(
                                    path.analysis().incomplete_reasons(),
                                    report.incomplete_reasons()
                                );
                                visits += 1;
                                Ok::<(), ProductionSourceOwnedViewErrorV18>(())
                            },
                        )
                        .unwrap();
                    assert_eq!(visits, 2 * launches.len());
                    assert_eq!(budget.storage(), retained);
                    completed.set(true);
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                })
                .unwrap();
            assert_eq!(output.report().passes()[0].changed(), changed);
            assert!(!output.grants_authority());
            assert!(!receipt.grants_authority());
            drop(output);
            assert_eq!(budget.storage(), floor);
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        });
        assert!(completed.get());
        result.unwrap();
    }
}

#[test]
fn paired_paths_preserve_every_unknown_launch_and_width_reason() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let prepared = scalar_payload_prepared_from_v18(integer_add_source_v18, &mut budget);
    let completed = Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let (output, (), _) = source.with_checked_integer_optimization_v18(budget, |original, optimized, budget| {
            let launches = vec![Launch::Exact(ExplicitLaunchExtent::Unknown); original.source.root_count(budget)?];
            let mut visits = 0;
            original.with_optimized_formal_paths_v20(optimized, &launches, FormalIndexWidth::Unknown, ControlFlowLimits::DEFAULT, budget, |side, before, after, path, _| {
                let report = if side == ProductionFormalPathSideV20::Original { before.analysis() } else { after.analysis() };
                assert!(std::ptr::eq(path.analysis(), report));
                assert!(report.incomplete_reasons().contains(&fe2o3_kernel_ir::FormalMemoryIncompleteReason::UnsupportedIndexWidth { width: FormalIndexWidth::Unknown }));
                assert!(path.observations().iter().all(|row| row.decision() == Decision::NotProved));
                visits += 1;
                Ok::<(), ProductionSourceOwnedViewErrorV18>(())
            }).unwrap();
            assert_eq!(visits, launches.len() * 2);
            completed.set(true);
            Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
        }).unwrap();
        drop(output);
        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    });
    assert!(completed.get());
    result.unwrap();
}

#[test]
fn paired_paths_keep_exact_consumer_rejection_without_poisoning_source_custody() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let prepared = scalar_payload_prepared_from_v18(integer_add_source_v18, &mut budget);
    let completed = Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let (output, (), _) = source
            .with_checked_integer_optimization_v18(budget, |original, optimized, budget| {
                let launches = launches(original, budget);
                let floor = budget.storage();
                let mut visits = 0;
                let result = original.with_optimized_formal_paths_v20(
                    optimized,
                    &launches,
                    FormalIndexWidth::Bits64,
                    ControlFlowLimits::DEFAULT,
                    budget,
                    |side, _, _, _, _| {
                        assert_eq!(side, ProductionFormalPathSideV20::Original);
                        visits += 1;
                        Err::<(), _>(37_u32)
                    },
                );
                assert!(matches!(
                    result,
                    Err(ProductionOptimizedSourcePathsErrorV20::Consumer(37))
                ));
                assert_eq!(visits, 1);
                assert_eq!(budget.storage(), floor);
                original.query(budget)?;
                completed.set(true);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
            })
            .unwrap();
        drop(output);
        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    });
    assert!(completed.get());
    result.unwrap();
}

#[test]
fn paired_paths_require_complete_roster_and_preserve_cumulative_solver_entry_limit() {
    for missing_roster in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let prepared = scalar_payload_prepared_from_v18(integer_add_source_v18, &mut budget);
        let completed = Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let result = source.with_checked_integer_optimization_v18(
                budget,
                |original, optimized, budget| {
                    let launches = if missing_roster {
                        vec![]
                    } else {
                        launches(original, budget)
                    };
                    let result = original.with_optimized_formal_paths_limits_v20(
                        optimized,
                        &launches,
                        FormalIndexWidth::Bits64,
                        ControlFlowLimits::DEFAULT,
                        PresburgerQueryLimitsV2 {
                            work: if missing_roster { 1_000_000 } else { 3 },
                            ..Default::default()
                        },
                        budget,
                        |_, _, _, _, _| -> Result<(), u32> {
                            panic!("entry refusal cannot consume a path");
                        },
                    );
                    if missing_roster {
                        assert!(matches!(
                            result,
                            Err(ProductionOptimizedSourcePathsErrorV20::Source(
                                ProductionSourceOwnedViewErrorV18::Binding(
                                    "paired formal report complete launch roster"
                                )
                            ))
                        ));
                    } else {
                        assert!(matches!(
                            result,
                            Err(ProductionOptimizedSourcePathsErrorV20::Query {
                                error: Query::Limit {
                                    resource: PresburgerQueryResourceV2::Work,
                                    actual: 4,
                                    limit: 3
                                },
                                ..
                            })
                        ));
                    }
                    assert!(original.source.guard.first.get().is_some());
                    completed.set(true);
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                },
            );
            assert!(result.is_err());
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        });
        assert!(completed.get());
        assert!(result.is_err());
    }
}

#[repr(align(64))]
struct DropCapture<'a>(&'a Cell<usize>, [u8; 128]);
impl DropCapture<'_> {
    fn touch(&self) -> u8 {
        self.1[0]
    }
}
impl Drop for DropCapture<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
        panic!("paired path owned capture");
    }
}

#[test]
fn paired_paths_entry_denials_drain_whole_owned_capture_after_retaining_first_error() {
    for work_denial in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let prepared = scalar_payload_prepared_from_v18(integer_add_source_v18, &mut budget);
        let completed = Cell::new(false);
        let dropped = Cell::new(0);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let result = source.with_checked_integer_optimization_v18(budget, |original, optimized, budget| {
                let launches = launches(original, budget);
                let floor = budget.storage();
                let padding = if work_denial { 0 } else { MODULE_LIMIT - floor };
                budget.reserve_storage(padding).unwrap();
                let first = work_denial.then(|| budget.charge_work(OPTIMIZED_SOURCE_WORK_LIMIT_V18).unwrap_err());
                let capture = DropCapture(&dropped, [7; 128]);
                let result = original.with_optimized_formal_paths_v20(optimized, &launches, FormalIndexWidth::Bits64, ControlFlowLimits::DEFAULT, budget, move |_, _, _, _, _| -> Result<(), u32> {
                    assert_eq!(capture.touch(), 7);
                    panic!("entry-denied callback cannot run");
                });
                let Err(ProductionOptimizedSourcePathsErrorV20::Source(ProductionSourceOwnedViewErrorV18::Resource(error))) = result else { panic!("exact source entry resource error required"); };
                if let Some(first) = first { assert_eq!(error, first); }
                else { assert!(matches!(error, ArgumentResourceV1::Storage(error) if error.limit() == MODULE_LIMIT && error.actual() > MODULE_LIMIT)); }
                assert_eq!(dropped.get(), 1);
                assert_eq!(budget.storage(), floor + padding);
                budget.release_storage(padding).unwrap();
                assert_eq!(budget.storage(), floor);
                assert!(matches!(original.source.guard.first.get().unwrap().error(), ProductionSourceOwnedViewErrorV18::Resource(first) if first == error));
                completed.set(true);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
            });
            assert!(result.is_err());
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        });
        assert!(completed.get());
        assert_eq!(dropped.get(), 1);
        assert!(result.is_err());
    }
}

#[test]
fn paired_path_typed_errors_keep_the_exact_source_chain() {
    use std::error::Error as _;
    type Failure = ProductionOptimizedSourcePathsErrorV20<ProductionSourceOwnedViewErrorV18>;
    let query = Query::Limit {
        resource: PresburgerQueryResourceV2::Queries,
        actual: 2,
        limit: 1,
    };
    let failure = Failure::Query {
        error: query.clone(),
        source_refusal: ProductionSourceOwnedViewErrorV18::Binding("retained"),
    };
    assert_eq!(
        failure.source().unwrap().downcast_ref::<Query>(),
        Some(&query)
    );
    let path = fe2o3_kernel_analysis::FormalPaidPathErrorV20::Query(query.clone());
    let failure = Failure::Path {
        side: ProductionFormalPathSideV20::Optimized,
        error: path,
        source_refusal: ProductionSourceOwnedViewErrorV18::Binding("retained"),
    };
    assert_eq!(
        failure
            .source()
            .unwrap()
            .source()
            .unwrap()
            .downcast_ref::<Query>(),
        Some(&query)
    );
}

#[test]
fn paired_paths_do_not_refund_observed_undercut_or_replace_prior_work_denial() {
    for prior_work in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let prepared = scalar_payload_prepared_from_v18(integer_add_source_v18, &mut budget);
        let entered = Cell::new(false);
        let settled = Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let result = source.with_checked_integer_optimization_v18(
                budget,
                |original, optimized, budget| {
                    let launches = launches(original, budget);
                    let first = Cell::new(None);
                    let result = original.with_optimized_formal_paths_v20(
                        optimized,
                        &launches,
                        FormalIndexWidth::Bits64,
                        ControlFlowLimits::DEFAULT,
                        budget,
                        |side, _, _, _, budget| {
                            assert_eq!(side, ProductionFormalPathSideV20::Original);
                            if prior_work {
                                first.set(Some(
                                    budget
                                        .charge_work(OPTIMIZED_SOURCE_WORK_LIMIT_V18)
                                        .unwrap_err(),
                                ));
                            }
                            let current = budget.storage();
                            assert!(current > 0);
                            budget.release_storage(current).unwrap();
                            assert_eq!(budget.storage(), 0);
                            entered.set(true);
                            Ok::<(), ProductionSourceOwnedViewErrorV18>(())
                        },
                    );
                    assert!(result.is_err());
                    assert!(original.source.cleanup.is_denied());
                    assert_eq!(budget.storage(), 0);
                    let retained = original.source.guard.first.get().unwrap().error();
                    if let Some(first) = first.get() {
                        assert!(matches!(retained, ProductionSourceOwnedViewErrorV18::Resource(actual) if actual == first));
                    } else {
                        assert!(matches!(retained, ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)));
                    }
                    settled.set(true);
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                },
            );
            assert!(result.is_err());
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        });
        assert!(entered.get());
        assert!(settled.get());
        assert!(result.is_err());
    }
}
