use fe2o3_kernel_ir::{
    CanonicalFormalLaunchInputV19 as Launch, ControlFlowLimits, ExplicitLaunchExtent,
    FormalIndexWidth,
};

#[test]
fn paired_formal_report_error_chain_preserves_each_exact_typed_cause() {
    use fe2o3_kernel_ir::CanonicalFormalReportErrorV19 as Formal;
    use std::error::Error as _;
    type Reports = ProductionOptimizedSourceReportsErrorV19<ProductionSourceOwnedViewErrorV18>;
    let source = Reports::Source(ProductionSourceOwnedViewErrorV18::Binding("source custody"));
    let formal = Reports::Formal {
        error: Formal::ConsumerRejected,
        source_refusal: ProductionSourceOwnedViewErrorV18::Binding("retained source refusal"),
    };
    let consumer = Reports::Consumer(ProductionSourceOwnedViewErrorV18::Binding("exact consumer"));
    for (error, message) in [(&source, "source custody"), (&consumer, "exact consumer")] {
        assert!(matches!(
            error.source().unwrap().downcast_ref::<ProductionSourceOwnedViewErrorV18>(),
            Some(ProductionSourceOwnedViewErrorV18::Binding(actual)) if *actual == message
        ));
    }
    assert_eq!(
        formal.source().unwrap().downcast_ref::<Formal>(),
        Some(&Formal::ConsumerRejected)
    );
    assert!(
        formal
            .source()
            .unwrap()
            .downcast_ref::<ProductionSourceOwnedViewErrorV18>()
            .is_none()
    );
    let Reports::Formal { source_refusal, .. } = &formal else {
        unreachable!()
    };
    assert!(matches!(
        source_refusal,
        ProductionSourceOwnedViewErrorV18::Binding("retained source refusal")
    ));
    let nested = ProductionOptimizedSourceReportsErrorV19::Consumer(consumer);
    assert!(matches!(
        nested
            .source()
            .unwrap()
            .source()
            .unwrap()
            .downcast_ref::<ProductionSourceOwnedViewErrorV18>(),
        Some(ProductionSourceOwnedViewErrorV18::Binding("exact consumer"))
    ));
}

fn launch_v19() -> Launch {
    Launch::PhysicalEnvelope(ExplicitLaunchExtent::Exact {
        rank: 1,
        extents: [8, 1, 1],
    })
}

#[test]
fn paired_formal_reports_consume_live_original_and_real_integer_output_without_admission() {
    fn assert_root(view: &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'_, '_>, reports: usize) {
        let has_memory = view
            .original_function()
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::Load { .. } | OperationKind::Store { .. }
                )
            });
        if reports == 0 {
            assert!(has_memory);
        }
        assert_eq!(
            view.original_function().id,
            view.original_owner().module().kernels[reports].entry
        );
    }

    for factory in [
        integer_add_source_v18 as fn() -> _,
        integer_multiply_source_v18,
        integer_and_source_v18,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(factory, &mut budget);
        let mut completed = false;
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let floor = budget.storage();
            let mut reports = 0;
            let (output, (), receipt) = source
                .with_checked_integer_optimization_v18(budget, |original, optimized, budget| {
                    let input = original.source.canonical(budget)?;
                    let output = optimized.output_inventory(budget)?.owner();
                    let roots = original.source.root_count(budget)?;
                    let launches = vec![launch_v19(); roots];
                    let retained = budget.storage();
                    original
                        .with_optimized_formal_reports_v19(
                            optimized,
                            &launches,
                            FormalIndexWidth::Bits64,
                            ControlFlowLimits::DEFAULT,
                            budget,
                            |before, after, _| {
                                assert!(std::ptr::eq(before.original_owner(), input));
                                assert!(std::ptr::eq(after.original_owner(), output));
                                assert!(!std::ptr::eq(
                                    before.original_owner(),
                                    after.original_owner()
                                ));
                                assert_eq!(before.root_index(), reports);
                                assert_eq!(after.root_index(), reports);
                                assert_eq!(before.launch_input(), launch_v19());
                                assert_eq!(after.launch_input(), launch_v19());
                                assert_eq!(
                                    before.analysis().obligations().kernel(),
                                    after.analysis().obligations().kernel()
                                );
                                assert_eq!(
                                    before.analysis().obligations().entry(),
                                    after.analysis().obligations().entry()
                                );
                                // Only the primary source root invokes the memory-bearing
                                // helper. The companion scalar root must still be visited.
                                assert_root(before, reports);
                                assert_root(after, reports);
                                reports += 1;
                                Ok::<(), ProductionSourceOwnedViewErrorV18>(())
                            },
                        )
                        .unwrap();
                    assert_eq!(budget.storage(), retained);
                    assert_eq!(reports, roots);
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                })
                .unwrap();
            assert!(!output.grants_authority());
            assert!(!receipt.grants_authority());
            drop(output);
            assert_eq!(budget.storage(), floor);
            completed = true;
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        });
        assert!(completed);
        result.unwrap();
    }
}

#[test]
fn paired_formal_reports_preserve_unknown_width_and_launch_reasons() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let prepared = scalar_payload_prepared_from_v18(integer_add_source_v18, &mut budget);
    let mut completed = false;
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let (output, (), _) = source.with_checked_integer_optimization_v18(budget, |original, optimized, budget| {
            let launches = vec![Launch::Exact(ExplicitLaunchExtent::Unknown); original.source.root_count(budget)?];
            original.with_optimized_formal_reports_v19(optimized, &launches,
                FormalIndexWidth::Unknown, ControlFlowLimits::DEFAULT, budget, |before, after, _| {
                    for report in [before.analysis(), after.analysis()] {
                        let fe2o3_kernel_ir::FormalMemoryObligationAnalysis::Incomplete { reasons, .. } = report else {
                            panic!("descriptive unknown inputs cannot become complete");
                        };
                        assert!(reasons.contains(&fe2o3_kernel_ir::FormalMemoryIncompleteReason::UnsupportedIndexWidth { width: FormalIndexWidth::Unknown }));
                        assert!(!reasons.is_empty());
                    }
                    Ok::<(), ProductionSourceOwnedViewErrorV18>(())
                }).unwrap();
            completed = true;
            Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
        }).unwrap();
        drop(output); Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    });
    assert!(completed);
    result.unwrap();
}

#[test]
fn paired_formal_reports_preserve_exact_consumer_failure_and_cleanup() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let prepared = scalar_payload_prepared_from_v18(integer_add_source_v18, &mut budget);
    let mut completed = false;
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let (output, (), _) = source
            .with_checked_integer_optimization_v18(budget, |original, optimized, budget| {
                let floor = budget.storage();
                let launches = vec![launch_v19(); original.source.root_count(budget)?];
                let result = original.with_optimized_formal_reports_v19(
                    optimized,
                    &launches,
                    FormalIndexWidth::Bits64,
                    ControlFlowLimits::DEFAULT,
                    budget,
                    |_, _, _| Err::<(), _>(37_u32),
                );
                assert!(matches!(
                    result,
                    Err(ProductionOptimizedSourceReportsErrorV19::Consumer(37))
                ));
                assert_eq!(budget.storage(), floor);
                completed = true;
                Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
            })
            .unwrap();
        drop(output);
        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    });
    assert!(completed);
    result.unwrap();
}

#[test]
fn paired_formal_reports_reject_incomplete_launch_roster_before_callback() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let prepared = scalar_payload_prepared_from_v18(integer_add_source_v18, &mut budget);
    let mut completed = false;
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let result =
            source.with_checked_integer_optimization_v18(budget, |original, optimized, budget| {
                let result = original.with_optimized_formal_reports_v19(
                    optimized,
                    &[],
                    FormalIndexWidth::Bits64,
                    ControlFlowLimits::DEFAULT,
                    budget,
                    |_, _, _| -> Result<(), u32> {
                        panic!("bad root callback");
                    },
                );
                assert!(matches!(
                    result,
                    Err(ProductionOptimizedSourceReportsErrorV19::Source(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "paired formal report complete launch roster"
                        )
                    ))
                ));
                completed = true;
                Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
            });
        assert!(result.is_err());
        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    });
    assert!(completed);
    assert!(result.is_err());
}

#[test]
fn paired_formal_reports_entry_refusal_retains_first_error_through_captured_drop_panic() {
    struct Capture<'a>(&'a std::cell::Cell<usize>);
    impl Drop for Capture<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            panic!("rejected paired-report capture");
        }
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let prepared = scalar_payload_prepared_from_v18(integer_add_source_v18, &mut budget);
    let dropped = std::cell::Cell::new(0);
    let mut completed = false;
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let result = source.with_checked_integer_optimization_v18(budget, |original, optimized, budget| {
            let launches = vec![launch_v19(); original.source.root_count(budget)?];
            let first = budget.charge_work(OPTIMIZED_SOURCE_WORK_LIMIT_V18).unwrap_err();
            let before = (budget.work(), budget.storage());
            let capture = Capture(&dropped);
            let result = original.with_optimized_formal_reports_v19(optimized, &launches, FormalIndexWidth::Bits64,
                ControlFlowLimits::DEFAULT, budget, move |_, _, _| -> Result<(), u32> {
                    let _ = &capture; panic!("denied consumer cannot run");
                });
            assert!(matches!(result, Err(ProductionOptimizedSourceReportsErrorV19::Source(ProductionSourceOwnedViewErrorV18::Resource(error))) if error == first));
            assert_eq!((budget.work(), budget.storage()), before);
            assert_eq!(dropped.get(), 1);
            assert!(matches!(original.source.guard.first.get().unwrap().error(), ProductionSourceOwnedViewErrorV18::Resource(error) if error == first));
            completed = true;
            Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
        });
        assert!(result.is_err());
        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    });
    assert!(completed);
    assert_eq!(dropped.get(), 1);
    assert!(result.is_err());
}

#[test]
fn paired_formal_reports_new_entry_header_denial_is_sticky_before_capture_destruction() {
    struct Capture<'a>(&'a std::cell::Cell<usize>);
    impl Drop for Capture<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            panic!("new header refusal capture");
        }
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let prepared = scalar_payload_prepared_from_v18(integer_add_source_v18, &mut budget);
    let dropped = std::cell::Cell::new(0);
    let mut completed = false;
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let result = source.with_checked_integer_optimization_v18(budget, |original, optimized, budget| {
            let launches = vec![launch_v19(); original.source.root_count(budget)?];
            let floor = budget.storage();
            let padding = budget.storage_limit() - floor;
            budget.reserve_storage(padding).unwrap();
            let capture = Capture(&dropped);
            let result = original.with_optimized_formal_reports_v19(optimized, &launches, FormalIndexWidth::Bits64,
                ControlFlowLimits::DEFAULT, budget, move |_, _, _| -> Result<(), u32> {
                    let _ = &capture; panic!("header-refused consumer cannot run");
                });
            let Err(ProductionOptimizedSourceReportsErrorV19::Source(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error)))) = result else {
                panic!("exact new header storage refusal required");
            };
            assert_eq!(Some(error.actual()), budget.failed_storage());
            assert_eq!(error.limit(), MODULE_LIMIT); assert!(error.actual() > error.limit());
            assert_eq!(budget.storage(), MODULE_LIMIT); assert_eq!(dropped.get(), 1);
            assert!(matches!(original.source.guard.first.get().unwrap().error(), ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(first)) if first == error));
            budget.release_storage(padding).unwrap();
            assert_eq!(budget.storage(), floor);
            completed = true;
            Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
        });
        assert!(result.is_err()); Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    });
    assert!(completed);
    assert_eq!(dropped.get(), 1);
    assert!(result.is_err());
}

#[test]
fn paired_formal_reports_callback_denial_survives_rejected_mutable_capture_panic() {
    struct Capture<'a>(&'a std::cell::Cell<usize>);
    impl Drop for Capture<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            panic!("post-report rejected capture");
        }
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let prepared = scalar_payload_prepared_from_v18(integer_add_source_v18, &mut budget);
    let dropped = std::cell::Cell::new(0);
    let first = std::cell::Cell::new(None);
    let called = std::cell::Cell::new(false);
    let mut completed = false;
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let result = source.with_checked_integer_optimization_v18(budget, |original, optimized, budget| {
            let launches = vec![launch_v19(); original.source.root_count(budget)?];
            let capture = Capture(&dropped);
            let first_ref = &first;
            let called_ref = &called;
            let result = original.with_optimized_formal_reports_v19(optimized, &launches, FormalIndexWidth::Bits64,
                ControlFlowLimits::DEFAULT, budget, move |_, _, budget| -> Result<(), u32> {
                    let _ = &capture;
                    let error = budget.charge_work(OPTIMIZED_SOURCE_WORK_LIMIT_V18).unwrap_err();
                    first_ref.set(Some(error)); called_ref.set(true); Ok(())
                });
            let first = first.get().expect("genuine report callback denied work");
            assert!(called.get()); assert_eq!(dropped.get(), 1);
            let Err(ProductionOptimizedSourceReportsErrorV19::Formal { error, source_refusal }) = result else {
                panic!("formal resource refusal must retain both diagnostics");
            };
            assert!(matches!(error, fe2o3_kernel_ir::CanonicalFormalReportErrorV19::Effects(fe2o3_kernel_ir::CanonicalEffectErrorV19::Resource(error)) if error == first));
            assert!(matches!(source_refusal, ProductionSourceOwnedViewErrorV18::Resource(error) if error == first));
            assert!(matches!(original.source.guard.first.get().unwrap().error(), ProductionSourceOwnedViewErrorV18::Resource(error) if error == first));
            completed = true;
            Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
        });
        assert!(result.is_err()); Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    });
    assert!(completed);
    assert!(called.get());
    assert_eq!(dropped.get(), 1);
    assert!(result.is_err());
}
