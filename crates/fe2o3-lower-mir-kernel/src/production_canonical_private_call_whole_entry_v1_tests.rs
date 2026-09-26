use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiArgumentV1, SemanticAbiExtensionV1, SemanticAbiRegularAttributesV1,
    SemanticAbiValueAttributesV1, SemanticSourceArgumentOwnershipV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Fixture {
    Empty,
    Private,
    SharedTyped,
}

#[test]
fn whole_entry_public_prepare_pays_complete_source_before_first_history_work() {
    check_prepare_prefix(false);
}

#[test]
fn whole_entry_public_prepare_import_and_capture_are_paid_before_execution() {
    check_prepare_prefix(true);
}

fn check_prepare_prefix(imported: bool) {
    use super::super::canonical_assertion_v1::private_call_whole_entry_oracle_v1_tests::{
        whole_prepare_import_prefix_v1, whole_prepare_source_prefix_v1,
    };
    type Oracle = fn(
        &ProductionPreRankedKirOwnerV1,
        usize,
        usize,
        usize,
    ) -> (usize, (usize, usize, usize, Option<usize>, Option<usize>));
    let oracle: Oracle = if imported {
        whole_prepare_import_prefix_v1
    } else {
        whole_prepare_source_prefix_v1
    };
    for fixture in [Fixture::Empty, Fixture::Private] {
        // Each run consumes a fresh real source. The expectation reads only its
        // immutable structural inputs, never a successful receipt or observation.
        for work_short in [false, true] {
            for storage_short in [false, true] {
                let source = fixture.source();
                let input_floor = source.unit_local_source_storage_floor_v1().unwrap();
                let floor = 43 + input_floor;
                let (prefix, boundary) = oracle(&source, floor, usize::MAX, usize::MAX);
                let work_limit = prefix - usize::from(work_short);
                let storage_limit = boundary.2 - usize::from(storage_short);
                let (_, expected) = oracle(&source, floor, work_limit, storage_limit);
                assert!(expected.3.is_some() || expected.4.is_some());
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(floor).unwrap();
                let error =
                    ProductionCanonicalScalarFixedPointOwnerV1::try_prepare_with_private_calls_v1(
                        source,
                        &mut budget,
                    )
                    .unwrap_err();
                if !work_short && !storage_short {
                    use fe2o3_kernel_opt::CheckedScalarFixedPointErrorV1 as Fixed;
                    use fe2o3_pliron::{
                        KirNeutralOptimizationErrorV1 as Observed,
                        PlironOptimizationErrorV12 as Execution,
                    };
                    assert!(
                        matches!(
                            (&error, imported),
                            (
                                ProductionCanonicalScalarSourceErrorV1::FixedPoint(
                                    Fixed::Resource(ArgumentResourceV1::Work(_))
                                ),
                                false
                            ) | (
                                ProductionCanonicalScalarSourceErrorV1::FixedPoint(
                                    Fixed::IntegerObservation(Observed::Execution(
                                        Execution::Resources(ArgumentResourceV1::Work(_))
                                    ))
                                ),
                                true
                            )
                        ),
                        "fixture={fixture:?}, error={error:?}"
                    );
                    assert_eq!(expected.0, prefix);
                    assert_eq!(expected.3, Some(boundary.0));
                }
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        budget.peak_storage(),
                        budget.failed_storage()
                    ),
                    (expected.0, floor, expected.2, expected.4),
                    "fixture={fixture:?}, error={error:?}"
                );
                // Consumed source and private candidate are gone on refusal;
                // only now does the caller retire the prepaid input boundary.
                budget.release_storage(input_floor).unwrap();
                assert_eq!(budget.storage(), 43);
                drop(budget);
                assert_eq!(work.failed_work(), expected.3);
            }
        }
    }
}

#[test]
fn whole_entry_original_source_complete_structural_view_exact_and_paired_denials() {
    use super::super::canonical_assertion_v1::private_call_whole_entry_oracle_v1_tests::whole_original_source_component_v1;
    // This is the complete public original-source entry, but not yet the
    // complete private/call factory or its fresh final-F consumer.
    for fixture in [Fixture::Empty, Fixture::Private] {
        let source = fixture.source();
        let floor = 43 + source.unit_local_source_storage_floor_v1().unwrap();
        let full = whole_original_source_component_v1(&source, floor, usize::MAX, usize::MAX);
        for work_short in [false, true] {
            for storage_short in [false, true] {
                let work_limit = full.0 - usize::from(work_short);
                let storage_limit = full.2 - usize::from(storage_short);
                let expected =
                    whole_original_source_component_v1(&source, floor, work_limit, storage_limit);
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(floor).unwrap();
                let called = std::cell::Cell::new(false);
                let result = source.with_checked_canonical_ranked_source_v1(&mut budget, |_, _| {
                    called.set(true);
                    Ok(())
                });
                let denied = expected.3.is_some() || expected.4.is_some();
                assert_eq!(result.is_err(), denied, "fixture={fixture:?}");
                assert_eq!(called.get(), !denied);
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        budget.peak_storage(),
                        budget.failed_storage()
                    ),
                    (expected.0, expected.1, expected.2, expected.4)
                );
                assert_eq!(budget.storage(), floor);
                drop(budget);
                assert_eq!(work.failed_work(), expected.3);
            }
        }
    }
}

#[test]
fn whole_entry_original_source_callback_error_and_panic_preserve_complete_prefix() {
    use super::super::canonical_assertion_v1::private_call_whole_entry_oracle_v1_tests::whole_original_source_component_v1;
    struct Payload(std::sync::Arc<std::sync::atomic::AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    for fixture in [Fixture::Empty, Fixture::Private] {
        let source = fixture.source();
        let floor = 43 + source.unit_local_source_storage_floor_v1().unwrap();
        let expected = whole_original_source_component_v1(&source, floor, usize::MAX, usize::MAX);
        for panics in [false, true] {
            let drops = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let mut work = Work::new(expected.0);
            let mut budget = Budget::new(&mut work, expected.2);
            budget.reserve_storage(floor).unwrap();
            let called = std::cell::Cell::new(false);
            let result = source.with_checked_canonical_ranked_source_v1(
                &mut budget,
                |_, _| -> CrResultV1<()> {
                    called.set(true);
                    if panics {
                        std::panic::panic_any(Payload(drops.clone()));
                    }
                    Err(ArgumentResourceV1::Arithmetic.into())
                },
            );
            assert!(called.get());
            assert!(matches!(
                (panics, result),
                (true, Err(ProductionCanonicalRankedSourceErrorV1::Panicked))
                    | (
                        false,
                        Err(ProductionCanonicalRankedSourceErrorV1::Resource(
                            ArgumentResourceV1::Arithmetic
                        ))
                    )
            ));
            assert_eq!(
                drops.load(std::sync::atomic::Ordering::SeqCst),
                usize::from(panics)
            );
            assert_eq!(
                (
                    budget.work(),
                    budget.storage(),
                    budget.peak_storage(),
                    budget.failed_storage()
                ),
                (expected.0, floor, expected.2, None)
            );
            drop(budget);
            assert_eq!(work.failed_work(), None);
        }
    }
}

#[test]
fn whole_entry_source_callable_rows_keep_nonempty_root_classification_and_exact_prefixes() {
    use super::super::canonical_assertion_v1::private_call_whole_entry_oracle_v1_tests::{
        execute_root_callable_component_v1, whole_root_callable_component_v1,
    };
    use fe2o3_mir_model::SemanticCallableDecisionV1 as Decision;
    for fixture in [Fixture::Empty, Fixture::Private] {
        let source = fixture.source();
        let floor = 43 + source.unit_local_source_storage_floor_v1().unwrap();
        let full = whole_root_callable_component_v1(&source, floor, usize::MAX, usize::MAX);
        for work_short in [false, true] {
            for storage_short in [false, true] {
                let limits = (
                    full.0 - usize::from(work_short),
                    full.2 - usize::from(storage_short),
                );
                let expected = whole_root_callable_component_v1(&source, floor, limits.0, limits.1);
                let mut work = Work::new(limits.0);
                let mut budget = Budget::new(&mut work, limits.1);
                budget.reserve_storage(floor).unwrap();
                let finished = std::cell::Cell::new(false);
                let result =
                    execute_root_callable_component_v1(&source, &mut budget, |kind, decision| {
                        assert_eq!(kind, ProductionCanonicalAssertionCallKindV1::Root);
                        assert_eq!(
                            decision,
                            match fixture {
                                Fixture::Empty => Decision::ExactEmptyDeterministicScalar,
                                Fixture::Private => Decision::Rejected,
                                Fixture::SharedTyped => unreachable!(),
                            }
                        );
                        finished.set(true);
                    });
                let refused = match &result {
                    Ok(inner) => inner.is_err(),
                    Err(_) => true,
                };
                let denied = expected.3.is_some() || expected.4.is_some();
                assert_eq!(refused, denied, "fixture={fixture:?}, limits={limits:?}, expected={expected:?}, result={result:?}, work={}, storage={}, peak={}", budget.work(), budget.storage(), budget.peak_storage());
                assert_eq!(finished.get(), !denied);
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        budget.peak_storage(),
                        budget.failed_storage()
                    ),
                    (expected.0, floor, expected.2, expected.4)
                );
                drop(budget);
                assert_eq!(work.failed_work(), expected.3);
            }
        }
    }
}

#[test]
fn whole_entry_zero_assertions_still_pay_actual_sparse_component_and_transfer() {
    use super::super::canonical_assertion_v1::private_call_whole_entry_oracle_v1_tests::{
        whole_inventory_component_v1, whole_sparse_root_component_v1,
    };
    use fe2o3_kernel_analysis::{
        CanonicalKirInventoryV1, CanonicalKirSparseLimitsV1, CanonicalKirSparseV1,
    };
    for fixture in [Fixture::Empty, Fixture::Private] {
        let source = fixture.source();
        let floor = 43 + source.unit_local_source_storage_floor_v1().unwrap();
        let (iw, ir, _) = whole_inventory_component_v1(source.executable().module());
        let (full, retained) = whole_sparse_root_component_v1(
            source.executable().module(),
            floor + ir,
            usize::MAX,
            usize::MAX,
        );
        assert_eq!(
            full.0,
            match fixture {
                Fixture::Empty => 19,
                Fixture::Private => 92,
                Fixture::SharedTyped => unreachable!(),
            }
        );
        for work_short in [false, true] {
            for storage_short in [false, true] {
                let limits = (
                    iw + full.0 - usize::from(work_short),
                    full.2 - usize::from(storage_short),
                );
                let (expected, _) = whole_sparse_root_component_v1(
                    source.executable().module(),
                    floor + ir,
                    limits.0 - iw,
                    limits.1,
                );
                let mut work = Work::new(limits.0);
                let mut budget = Budget::new(&mut work, limits.1);
                budget.reserve_storage(floor).unwrap();
                let (inventory, receipt) =
                    CanonicalKirInventoryV1::derive(source.executable(), &mut budget).unwrap();
                assert_eq!(receipt.retained_storage(), ir);
                budget.reserve_storage(ir).unwrap();
                let result = CanonicalKirSparseV1::derive(
                    &inventory,
                    CanonicalKirSparseLimitsV1::default(),
                    &mut budget,
                );
                assert_eq!(
                    result.is_err(),
                    expected.3.is_some() || expected.4.is_some()
                );
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        budget.peak_storage(),
                        budget.failed_storage()
                    ),
                    (iw + expected.0, floor + ir, expected.2, expected.4)
                );
                if let Ok((report, receipt)) = result {
                    assert_eq!(receipt.retained_storage(), retained);
                    budget.reserve_storage(retained).unwrap();
                    assert!(report.belongs_to(&inventory));
                    assert_eq!(report.block_executable(0), Some(true));
                    drop(report);
                    budget.release_storage(retained).unwrap();
                }
                drop(inventory);
                budget.release_storage(ir).unwrap();
                assert_eq!(budget.storage(), floor);
                drop(budget);
                assert_eq!(work.failed_work(), expected.3.map(|x| iw + x));
            }
        }
    }
}

#[test]
fn whole_entry_original_metadata_components_keep_backing_through_later_checks() {
    use super::super::canonical_assertion_v1::private_call_whole_entry_oracle_v1_tests::{
        whole_inventory_component_v1, whole_root_metadata_component_v1,
    };
    use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
    use std::mem::size_of;

    // Component qualification through the real call-index wrapper. This does
    // not claim the outer canonical metadata or private/call entry is covered.
    const SIBLING: usize = 43;
    for fixture in [Fixture::Empty, Fixture::Private] {
        let source = fixture.source();
        let floor = SIBLING + source.unit_local_source_storage_floor_v1().unwrap();
        let (inventory_work, inventory_storage, _) =
            whole_inventory_component_v1(source.executable().module());
        let call_floor = floor + inventory_storage;
        let full = whole_root_metadata_component_v1(&source, call_floor, usize::MAX, usize::MAX);
        for work_short in [false, true] {
            for storage_short in [false, true] {
                let work_limit = inventory_work + full.0 - usize::from(work_short);
                let storage_limit = full.2 - usize::from(storage_short);
                let expected = whole_root_metadata_component_v1(
                    &source,
                    call_floor,
                    work_limit - inventory_work,
                    storage_limit,
                );
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(floor).unwrap();
                let (inventory, receipt) =
                    CanonicalKirInventoryV1::derive(source.executable(), &mut budget).unwrap();
                budget.reserve_storage(inventory_storage).unwrap();
                assert_eq!(receipt.retained_storage(), inventory_storage);
                let finished = std::cell::Cell::new(false);
                let result = source.with_checked_canonical_calls_v1(
                    &inventory,
                    &mut budget,
                    |calls, budget| {
                        let paid = budget.storage();
                        let result = (|| -> CrResultV1<()> {
                            budget.reserve_storage(
                                size_of::<CrSourceRowsV1<'_>>()
                                    + size_of::<CrArgumentRowsV1<'_>>()
                                    + size_of::<CrContractsV1<'_>>()
                                    - size_of::<
                                        fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
                                    >(),
                            )?;
                            let rows = cr_build_source_rows_v1(&source, &inventory, calls, budget)?;
                            let arguments = cr_build_arguments_v1(&source, calls, budget)?;
                            let contracts =
                                cr_build_contracts_v1(&source, &inventory, &rows, budget)?;
                            cr_check_source_rows_v1(&source, &inventory, calls, &rows, budget)?;
                            cr_check_arguments_v1(&source, calls, &arguments, budget)?;
                            cr_check_contracts_v1(&source, &inventory, &rows, &contracts, budget)?;
                            finished.set(true);
                            Ok(())
                        })();
                        // Real owning components have left their scope before refund.
                        budget.release_storage(budget.storage() - paid)?;
                        Ok(result)
                    },
                );
                let refused = match result {
                    Ok(inner) => inner.is_err(),
                    Err(_) => true,
                };
                let denied = expected.3.is_some() || expected.4.is_some();
                assert_eq!(refused, denied, "fixture={fixture:?}");
                assert_eq!(finished.get(), !denied);
                assert_eq!(budget.work(), inventory_work + expected.0);
                assert_eq!(budget.storage(), expected.1);
                assert_eq!(budget.peak_storage(), expected.2);
                assert_eq!(budget.failed_storage(), expected.4);
                assert_eq!(budget.storage(), call_floor);
                drop(inventory);
                budget.release_storage(inventory_storage).unwrap();
                assert_eq!(budget.storage(), floor);
                drop(budget);
                assert_eq!(work.failed_work(), expected.3.map(|n| inventory_work + n));
            }
        }
    }
}

#[test]
fn whole_entry_actual_inventory_has_independent_retained_and_terminal_work_prefixes() {
    use super::super::canonical_assertion_v1::private_call_whole_entry_oracle_v1_tests::whole_inventory_component_v1;
    use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
    const SIBLING: usize = 43;
    for fixture in [Fixture::Empty, Fixture::Private, Fixture::SharedTyped] {
        let source = fixture.source();
        let floor = SIBLING + source.unit_local_source_storage_floor_v1().unwrap();
        let (expected_work, retained, terminal) =
            whole_inventory_component_v1(source.executable().module());
        for short in [false, true] {
            let mut work = Work::new(expected_work - usize::from(short));
            let mut budget = Budget::new(&mut work, floor + retained);
            budget.reserve_storage(floor).unwrap();
            let result = CanonicalKirInventoryV1::derive(source.executable(), &mut budget);
            assert_eq!(result.is_err(), short, "fixture={fixture:?}");
            assert_eq!(budget.work(), expected_work - usize::from(short) * terminal);
            assert_eq!(
                (budget.storage(), budget.peak_storage()),
                (floor, floor + retained)
            );
            assert_eq!(budget.failed_storage(), None);
            if let Ok((inventory, receipt)) = result {
                budget.reserve_storage(retained).unwrap();
                assert_eq!(receipt.retained_storage(), retained);
                assert!(inventory.belongs_to(source.executable()));
                drop(inventory);
                budget.release_storage(retained).unwrap();
            }
            drop(budget);
            assert_eq!(work.failed_work(), short.then_some(expected_work));
        }
    }
}

#[test]
fn whole_entry_root_call_index_has_real_return_anchor_and_paired_exact_cuts() {
    use super::super::canonical_assertion_v1::private_call_whole_entry_oracle_v1_tests::{
        whole_inventory_component_v1, whole_root_calls_component_v1,
    };
    use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
    const SIBLING: usize = 43;
    for fixture in [Fixture::Empty, Fixture::Private] {
        let source = fixture.source();
        let floor = SIBLING + source.unit_local_source_storage_floor_v1().unwrap();
        let (inventory_work, inventory_storage, _) =
            whole_inventory_component_v1(source.executable().module());
        let call_floor = floor + inventory_storage;
        let full = whole_root_calls_component_v1(&source, call_floor, usize::MAX, usize::MAX);
        let expected_work = inventory_work + full.0;
        assert_eq!(
            full.0,
            match fixture {
                Fixture::Empty => 440,
                Fixture::Private => 754,
                Fixture::SharedTyped => unreachable!(),
            } + 2 * source.executable().module().functions[0].id.as_str().len()
        );
        for work_short in [false, true] {
            for storage_short in [false, true] {
                let work_limit = expected_work - usize::from(work_short);
                let storage_limit = full.2 - usize::from(storage_short);
                let expected = whole_root_calls_component_v1(
                    &source,
                    call_floor,
                    work_limit - inventory_work,
                    storage_limit,
                );
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(floor).unwrap();
                let (inventory, receipt) =
                    CanonicalKirInventoryV1::derive(source.executable(), &mut budget).unwrap();
                budget.reserve_storage(inventory_storage).unwrap();
                assert_eq!(receipt.retained_storage(), inventory_storage);
                assert_eq!(budget.work(), inventory_work);
                let calls = std::cell::Cell::new(0);
                let result =
                    source.with_checked_canonical_calls_v1(&inventory, &mut budget, |view, _| {
                        calls.set(calls.get() + 1);
                        assert_eq!(view.function_count(), 1);
                        assert_eq!(view.call_count(), 0);
                        assert!(view.belongs_to(&inventory));
                        Ok(())
                    });
                let denied = expected.3.is_some() || expected.4.is_some();
                assert_eq!(result.is_err(), denied, "fixture={fixture:?}");
                assert_eq!(calls.get(), usize::from(!denied));
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        budget.peak_storage(),
                        budget.failed_storage()
                    ),
                    (
                        inventory_work + expected.0,
                        expected.1,
                        expected.2,
                        expected.4
                    )
                );
                drop(inventory);
                budget.release_storage(inventory_storage).unwrap();
                assert_eq!(budget.storage(), floor);
                drop(budget);
                assert_eq!(work.failed_work(), expected.3.map(|n| inventory_work + n));
                if storage_short {
                    assert_eq!(
                        expected.3, None,
                        "the earlier storage cut masks final work denial"
                    );
                }
            }
        }
    }
}

#[test]
fn whole_entry_borrowed_verifier_component_has_independent_exact_and_terminal_work_cut() {
    use super::super::canonical_assertion_v1::private_call_whole_entry_oracle_v1_tests::whole_borrowed_verifier_component_v1;
    use fe2o3_kernel_ir::verify_module_ref_with_budget_v1;
    const FLOOR: usize = 43;
    for fixture in [Fixture::Empty, Fixture::Private, Fixture::SharedTyped] {
        let source = fixture.source();
        let floor = FLOOR + source.unit_local_source_storage_floor_v1().unwrap();
        let module = source.executable().module();
        let (expected_work, expected_peak, terminal_debit) =
            whole_borrowed_verifier_component_v1(module);
        match fixture {
            Fixture::Empty => {
                assert_eq!(
                    expected_work,
                    178 + 6 * module.functions[0].id.as_str().len()
                );
                assert_eq!(expected_peak, 21);
            }
            Fixture::Private => {
                assert_eq!(
                    expected_work,
                    353 + 6 * module.functions[0].id.as_str().len()
                );
                assert_eq!(expected_peak, 63);
            }
            Fixture::SharedTyped => {}
        }
        for short in [false, true] {
            let mut work = Work::new(expected_work - usize::from(short));
            {
                let mut budget = Budget::new(&mut work, floor + expected_peak);
                budget.reserve_storage(floor).unwrap();
                let result = verify_module_ref_with_budget_v1(module, None, &mut budget);
                assert_eq!(result.is_err(), short, "fixture={fixture:?}");
                assert_eq!(
                    budget.work(),
                    expected_work - usize::from(short) * terminal_debit
                );
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.peak_storage(), floor + expected_peak);
                assert_eq!(budget.failed_storage(), None);
            }
            assert_eq!(work.failed_work(), short.then_some(expected_work));
        }
    }
}

#[test]
fn whole_entry_inverse_component_predicts_wire_ownership_and_terminal_hash_cut() {
    use super::super::canonical_assertion_v1::private_call_whole_entry_oracle_v1_tests::whole_inverse_component_v1;
    use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12;
    const SIBLING: usize = 43;
    for fixture in [Fixture::Empty, Fixture::Private, Fixture::SharedTyped] {
        let source = fixture.source();
        let floor = SIBLING + source.unit_local_source_storage_floor_v1().unwrap();
        let module = source.executable().module();
        let (expected_work, expected_retained, expected_peak, hash_debit, bytes) =
            whole_inverse_component_v1(module);
        assert_eq!(
            bytes,
            source.executable().canonical().canonical_bytes().len()
        );
        for short in [false, true] {
            let mut work = Work::new(expected_work - usize::from(short));
            {
                let mut budget = Budget::new(&mut work, floor + expected_peak);
                budget.reserve_storage(floor).unwrap();
                let result = VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(module, &mut budget);
                assert_eq!(result.is_err(), short, "fixture={fixture:?}");
                assert_eq!(
                    budget.work(),
                    expected_work - usize::from(short) * hash_debit
                );
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.peak_storage(), floor + expected_peak);
                assert_eq!(budget.failed_storage(), None);
                if let Ok((owner, receipt)) = result {
                    budget.reserve_storage(expected_retained).unwrap();
                    assert_eq!(receipt.retained_storage(), expected_retained);
                    assert_eq!(owner.module(), module);
                    assert_eq!(
                        owner.canonical().canonical_bytes(),
                        source.executable().canonical().canonical_bytes()
                    );
                    drop(owner);
                    budget.release_storage(expected_retained).unwrap();
                }
                assert_eq!(budget.storage(), floor);
            }
            assert_eq!(work.failed_work(), short.then_some(expected_work));
        }
    }
}

impl Fixture {
    fn source(self) -> ProductionPreRankedKirOwnerV1 {
        match self {
            Self::Empty => cr_noop(&["empty"]),
            Self::Private => cpc_memory_source(false, true, false, 11),
            Self::SharedTyped => shared_typed_source(),
        }
    }
}

#[test]
fn whole_entry_shared_typed_source_summaries_keep_exact_and_paired_prefixes() {
    use super::super::canonical_assertion_v1::private_call_whole_entry_oracle_v1_tests::{
        execute_typed_source_component_v1, whole_typed_source_component_v1,
    };
    let source = shared_typed_source();
    let source_floor = source.unit_local_source_storage_floor_v1().unwrap();
    let floor = 43 + source_floor;
    for component in [0, 1, 2, 3, 4, 5, 6, 7] {
        let full =
            whole_typed_source_component_v1(&source, component, floor, usize::MAX, usize::MAX);
        assert_eq!((full.1, full.3, full.4), (floor, None, None));
        for (w, s) in [
            (full.0, full.2),
            (full.0 - 1, full.2),
            (full.0, full.2 - 1),
            (full.0 - 1, full.2 - 1),
        ] {
            let expected = whole_typed_source_component_v1(&source, component, floor, w, s);
            let mut work = Work::new(11 + w);
            work.charge_work(11).unwrap();
            {
                let mut budget = Budget::new(&mut work, s);
                budget.reserve_storage(floor).unwrap();
                let result = execute_typed_source_component_v1(&source, component, &mut budget);
                assert_eq!(result.is_ok(), expected.3.is_none() && expected.4.is_none(), "component={component}, limits=({w}, {s}), expected={expected:?}, result={result:?}, work={}, storage={}, peak={}", budget.work(), budget.storage(), budget.peak_storage());
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        budget.peak_storage(),
                        budget.failed_storage()
                    ),
                    (11 + expected.0, expected.1, expected.2, expected.4)
                );
                budget.release_storage(43).unwrap();
                assert_eq!(budget.storage(), source_floor);
            }
            assert_eq!(work.failed_work(), expected.3.map(|amount| 11 + amount));
        }
    }
}

fn shared_typed_source() -> ProductionPreRankedKirOwnerV1 {
    let seed = cpc_shared_source();
    let semantic = seed.semantic_ssa().source_semantic();
    let unit = SemanticTypeIdV1::from_index(0);
    let word = SemanticTypeIdV1::from_index(1);
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let value = |ty| {
        SemanticAbiValueV1::new(
            ty,
            if ty == unit {
                SemanticAbiPassModeV1::Ignore
            } else {
                SemanticAbiPassModeV1::Direct(attributes)
            },
        )
    };
    let mut functions = Vec::new();
    for (ordinal, function) in semantic.functions().iter().enumerate() {
        let helper = function.role() == SemanticFunctionRoleV1::InternalHelper;
        assert_eq!(helper, ordinal == 1);
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([90 + ordinal as u8; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            if helper {
                SemanticCanonAbiV1::Rust
            } else {
                SemanticCanonAbiV1::GpuKernel
            },
            if helper {
                SemanticExternAbiV1::Rust
            } else {
                SemanticExternAbiV1::GpuKernel
            },
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(value(word))],
            value(if helper { word } else { unit }),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
        .unwrap();
        let decl = |tag, ty, role| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([tag; 32]),
                ty,
                role,
                function.source(),
            )
        };
        let mut locals = vec![
            decl(
                100,
                if helper { word } else { unit },
                SemanticLocalRoleV1::Return,
            ),
            decl(101, word, SemanticLocalRoleV1::Argument(0)),
        ];
        let blocks = if helper {
            vec![body(
                110,
                vec![assign(0, word, SemanticRvalueKindV1::Use(local(1, word)))],
                SemanticTerminatorKindV1::Return,
            )]
        } else {
            locals.push(decl(102, word, SemanticLocalRoleV1::Temporary));
            let call = SemanticDirectCallV1::new(
                SemanticFunctionIdV1::from_index(1),
                vec![local(1, word)],
                Some(SemanticCallDestinationV1::new(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], word).unwrap(),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(1),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap();
            vec![
                body(110, vec![], SemanticTerminatorKindV1::Call(call)),
                body(111, vec![], SemanticTerminatorKindV1::Return),
            ]
        };
        let new = SemanticFunctionDeclV1::new(
            function.identity(),
            function.role(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            function.source(),
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap();
        functions.push(match function.kernel_entry() {
            Some(entry) => new.with_kernel_entry(entry.clone()),
            None => new,
        });
    }
    rebuild(&seed, semantic.types().to_vec(), functions)
}

// All expectations are fixed before prepare, including the independent native
// unit predictions. No returned receipt or observation feeds an expectation.
#[test]
fn whole_entry_fixture_family_uses_real_scalar_private_and_nonzero_typed_calls() {
    use super::super::canonical_assertion_v1::private_call_whole_entry_oracle_v1_tests::{
        assert_whole_native_units_v1, whole_dynamic_native_work_v1, whole_native_units_v1,
        whole_root_history_retained_component_v1, whole_shared_history_retained_component_v1,
    };
    for fixture in [Fixture::Empty, Fixture::Private, Fixture::SharedTyped] {
        let source = fixture.source();
        let native_units = whole_native_units_v1(source.executable().module());
        let dynamic = (!matches!(fixture, Fixture::SharedTyped))
            .then(|| whole_dynamic_native_work_v1(source.executable().module()));
        let retained = (!matches!(fixture, Fixture::SharedTyped))
            .then(|| whole_root_history_retained_component_v1(source.executable().module()));
        let shared = matches!(fixture, Fixture::SharedTyped)
            .then(|| whole_shared_history_retained_component_v1(source.executable().module()));
        let original_floor = source.unit_local_source_storage_floor_v1().unwrap();
        let source_bytes = source.executable().canonical().canonical_bytes().to_vec();
        let expected = match fixture {
            Fixture::Empty => ([0, 0, 0, 0, 0], 1, 1),
            Fixture::Private => ([1, 0, 1, 1, 0], 1, 1),
            Fixture::SharedTyped => ([0, 0, 0, 0, 2], 2, 3),
        };
        assert_eq!(cpc_counts(source.executable()), expected.0);
        assert_eq!(source.executable().module().kernels.len(), expected.1);
        assert_eq!(source.executable().module().functions.len(), expected.2);
        if matches!(fixture, Fixture::SharedTyped) {
            let module = source.executable().module();
            for function in &module.functions {
                assert_eq!(
                    function.signature.parameters,
                    [fe2o3_kernel_ir::Type::Scalar(
                        fe2o3_kernel_ir::ScalarType::U64
                    )]
                );
                if function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper {
                    assert_eq!(
                        function.signature.results,
                        [fe2o3_kernel_ir::Type::Scalar(
                            fe2o3_kernel_ir::ScalarType::U64
                        )]
                    );
                }
            }
            for operation in module
                .functions
                .iter()
                .filter_map(|f| f.body.as_ref())
                .flat_map(|b| &b.blocks)
                .flat_map(|b| &b.operations)
            {
                if let OperationKind::Call { arguments, .. } = &operation.kind {
                    assert_eq!(arguments.len(), 1);
                    assert_eq!(operation.results.len(), 1);
                    assert_eq!(
                        operation.results[0].ty,
                        fe2o3_kernel_ir::Type::Scalar(fe2o3_kernel_ir::ScalarType::U64)
                    );
                }
            }
        }
        let owner = cpc_prepare(source);
        if !matches!(fixture, Fixture::SharedTyped) {
            assert_eq!(owner.history().rounds().len(), 1);
            assert_eq!(owner.output().canonical().canonical_bytes(), source_bytes);
            // Diagnostic bytes only, never decoded into execution authority.
            // Both sealed schemas put the dynamic debit after header8,
            // two identity40 records, and five preceding u64 profile fields.
            let round = &owner.history().rounds()[0];
            let [integer, scalar, history, additional] = retained.unwrap();
            assert_eq!(round.integer().storage().retained_storage(), integer);
            assert_eq!(round.scalar().storage().retained_storage(), scalar);
            assert_eq!(owner.history().retained_storage(), history);
            assert_eq!(owner.additional_storage().retained_storage(), additional);
            assert_eq!(
                owner.retained_storage_floor_v1(),
                original_floor + additional
            );
            let field = |wire: &[u8]| {
                usize::try_from(u64::from_le_bytes(wire[128..136].try_into().unwrap())).unwrap()
            };
            assert_eq!(
                (
                    field(round.integer().execution().canonical_bytes()),
                    field(round.scalar().execution().canonical_bytes()),
                ),
                dynamic.unwrap()
            );
        } else {
            let (final_module, [i0, s0, i1, s1, history, additional]) = shared.unwrap();
            assert_eq!(owner.history().rounds().len(), 2);
            assert_eq!(owner.output().module(), &final_module);
            let [first, terminal] = owner.history().rounds() else {
                unreachable!()
            };
            assert_eq!(
                first.integer().owner().module(),
                owner.original_source().executable().module()
            );
            assert_eq!(first.scalar().owner().module(), &final_module);
            assert_eq!(terminal.integer().owner().module(), &final_module);
            assert_eq!(first.integer().storage().retained_storage(), i0);
            assert_eq!(first.scalar().storage().retained_storage(), s0);
            assert_eq!(terminal.integer().storage().retained_storage(), i1);
            assert_eq!(terminal.scalar().storage().retained_storage(), s1);
            assert_eq!(owner.history().retained_storage(), history);
            assert_eq!(owner.additional_storage().retained_storage(), additional);
            assert_eq!(
                owner.retained_storage_floor_v1(),
                original_floor + additional
            );
        }
        cpc_run(&owner, |view, budget| {
            cpc_reports(&owner, view, budget)?;
            assert_whole_native_units_v1(&native_units, view.policies(budget)?, budget)?;
            assert_eq!(view.assertion_count(budget)?, 0);
            assert_eq!(view.policies(budget)?.pair_count(budget)?, 0);
            assert_eq!(
                view.memory_census(budget)?,
                if matches!(fixture, Fixture::Private) {
                    [1, 2]
                } else {
                    [0, 0]
                }
            );
            assert_eq!(view.calls(budget)?.len(), expected.0[4]);
            Ok(())
        })
        .unwrap();
    }
}

include!("production_canonical_private_call_whole_lifecycle_v1_tests.rs");
