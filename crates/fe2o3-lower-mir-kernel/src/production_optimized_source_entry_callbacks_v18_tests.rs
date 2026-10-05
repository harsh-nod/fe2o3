fn owned_optimizer_disposal_header_oracle_v26<T, E>() -> usize {
    source_owned_finish_header_oracle_v26::<
        (
            fe2o3_pliron::CheckedNeutralKernelIrOwnerV18,
            T,
            fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
        ),
        SourceConsumerErrorV18<ProductionSourceOptimizationErrorV18<E>>,
    >()
}

fn owned_optimizer_header_oracle_v1744<T, E, F>(_: &F) -> usize {
    use std::mem::{align_of, size_of};
    use std::panic::AssertUnwindSafe;
    type Output<T> = (
        fe2o3_pliron::CheckedNeutralKernelIrOwnerV18,
        T,
        fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
    );
    type Entry<'a, F> = (
        fe2o3_pliron::KirNeutralOptimizationOutputV18<'a>,
        usize,
        usize,
        F,
    );
    type Capture<'a, 'w, F> = (
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a mut ArgumentBudgetV1<'w>,
        &'a std::cell::Cell<usize>,
        usize,
        F,
    );
    type EntryResult<'a, E, F> =
        Result<Entry<'a, F>, SourceConsumerErrorV18<ProductionSourceOptimizationErrorV18<E>>>;
    type Adoption<'a, 'w, F> = (
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a mut ArgumentBudgetV1<'w>,
        fe2o3_pliron::KirNeutralOptimizationOutputV18<'a>,
        F,
    );
    type Adopted<T, E> = Result<Output<T>, fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1<E>>;
    type Settled<T, E> =
        Result<Output<T>, SourceConsumerErrorV18<ProductionSourceOptimizationErrorV18<E>>>;
    type Payload = Box<dyn std::any::Any + Send>;
    let cleanup = size_of::<[Option<Payload>; 2]>()
        + size_of::<AssertUnwindSafe<[Option<Payload>; 2]>>()
        + 2 * size_of::<Payload>()
        + size_of::<AssertUnwindSafe<Payload>>()
        + 2 * size_of::<Result<(), Payload>>()
        + size_of::<std::ops::Range<usize>>()
        + size_of::<usize>()
        + size_of::<bool>();
    2 * size_of::<Capture<'_, '_, F>>()
        + 2 * align_of::<Capture<'_, '_, F>>()
        + size_of::<AssertUnwindSafe<Capture<'_, '_, F>>>()
        + size_of::<Entry<'_, F>>()
        + 2 * size_of::<EntryResult<'_, E, F>>()
        + size_of::<std::thread::Result<EntryResult<'_, E, F>>>()
        + size_of::<AssertUnwindSafe<Entry<'_, F>>>()
        + size_of::<std::thread::Result<()>>()
        + size_of::<std::cell::Cell<usize>>()
        + 3 * size_of::<usize>()
        + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + size_of::<SourceOwnedResultV18<()>>()
        + 4 * size_of::<Adoption<'_, '_, F>>()
        + 4 * align_of::<Adoption<'_, '_, F>>()
        + size_of::<AssertUnwindSafe<Adoption<'_, '_, F>>>()
        + size_of::<Adopted<T, E>>()
        + size_of::<Settled<T, E>>()
        + size_of::<std::thread::Result<Settled<T, E>>>()
        + size_of::<Result<Output<T>, ProductionSourceOptimizationErrorV18<E>>>()
        + size_of::<AssertUnwindSafe<Result<Output<T>, ProductionSourceOptimizationErrorV18<E>>>>()
        + size_of::<ProductionSourceOptimizationErrorV18<E>>()
        + cleanup
        + owned_optimizer_disposal_header_oracle_v26::<T, E>()
}

#[test]
fn owned_optimizer_generic_result_frames_are_paid_independently_of_capture() {
    #[repr(align(256))]
    struct LargeResult([u8; 16384]);
    let consume = || ();
    assert_eq!(std::mem::size_of_val(&consume), 0);
    let expected = owned_optimizer_header_oracle_v1744::<LargeResult, LargeResult, _>(&consume);
    assert_eq!(
        optimized_source_consumer_resources_v18::optimizer_entry_headers::<
            LargeResult,
            LargeResult,
            _,
        >(&consume)
        .unwrap()
            + owned_optimizer_disposal_header_oracle_v26::<LargeResult, LargeResult>(),
        expected,
    );
    let small = owned_optimizer_header_oracle_v1744::<(), (), _>(&consume);
    assert!(expected > small + 4 * size_of::<LargeResult>());
    let correspondence =
        optimized_correspondence_header_oracle_v18::<LargeResult, LargeResult, _>(&consume);
    assert_eq!(
        optimized_source_v18::owned_headers_for_test_v1744::<LargeResult, LargeResult, _>(&consume)
            .unwrap(),
        correspondence
    );
    assert!(
        correspondence
            > optimized_correspondence_header_oracle_v18::<(), (), _>(&consume)
                + 2 * size_of::<LargeResult>()
    );
    let retained =
        owned_retained_optimizer_header_oracle_v1744::<LargeResult, LargeResult, _>(&consume);
    assert_eq!(
        optimized_source_consumer_resources_v18::retained_entry_headers::<
            LargeResult,
            LargeResult,
            _,
        >(&consume)
        .unwrap(),
        retained
    );
    assert!(
        retained
            > owned_retained_optimizer_header_oracle_v1744::<(), (), _>(&consume)
                + 4 * size_of::<LargeResult>()
    );
    let value = LargeResult([0; 16384]);
    assert_eq!(value.0.len(), 16384);
}

fn owned_retained_optimizer_header_oracle_v1744<T, E, F>(_: &F) -> usize {
    use std::mem::{align_of, size_of};
    use std::panic::AssertUnwindSafe;
    type Capture<'a, 'w, F> = (
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a mut ArgumentBudgetV1<'w>,
        &'a std::cell::Cell<usize>,
        &'a std::cell::Cell<usize>,
        F,
    );
    type Output<T> = (
        fe2o3_pliron::CheckedNeutralKernelIrOwnerV18,
        T,
        fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
    );
    type Result<T, E> = std::result::Result<Output<T>, ProductionSourceOptimizationErrorV18<E>>;
    type Settled<T, E> = std::result::Result<
        Output<T>,
        SourceConsumerErrorV18<ProductionSourceOptimizationErrorV18<E>>,
    >;
    type Payload = Box<dyn std::any::Any + Send>;
    let cleanup = size_of::<[Option<Payload>; 2]>()
        + size_of::<AssertUnwindSafe<[Option<Payload>; 2]>>()
        + 2 * size_of::<Payload>()
        + size_of::<AssertUnwindSafe<Payload>>()
        + 2 * size_of::<std::result::Result<(), Payload>>()
        + size_of::<std::ops::Range<usize>>()
        + size_of::<usize>()
        + size_of::<bool>();
    2 * size_of::<Capture<'_, '_, F>>()
        + 2 * align_of::<Capture<'_, '_, F>>()
        + size_of::<AssertUnwindSafe<Capture<'_, '_, F>>>()
        + 2 * size_of::<std::cell::Cell<usize>>()
        + 2 * size_of::<usize>()
        + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + size_of::<SourceOwnedResultV18<()>>()
        + size_of::<Output<T>>()
        + size_of::<Result<T, E>>()
        + size_of::<std::thread::Result<Result<T, E>>>()
        + size_of::<AssertUnwindSafe<Output<T>>>()
        + size_of::<ProductionSourceOptimizationErrorV18<E>>()
        + size_of::<Settled<T, E>>()
        + size_of::<std::thread::Result<Settled<T, E>>>()
        + size_of::<AssertUnwindSafe<Settled<T, E>>>()
        + cleanup
}

fn owned_correspondence_fixture_v1744(
    consume: impl for<'a, 'i, 'input, 'output, 'rows, 'work> FnOnce(
        &ProductionSourceCorrespondenceV18<'a>,
        &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV18<'i, 'input, 'output, 'rows>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> (SourceOwnedResultV18<()>, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(folding_source_owner_v18, &mut budget);
    let entered = std::cell::Cell::new(false);
    let result =
        with_actual_optimized_transition_v18(prepared, &mut budget, |source, checked, budget| {
            source.with_ranked_correspondence_v18(checked.input(), budget, |original, budget| {
                entered.set(true);
                consume(original, checked, budget)
            })
        });
    assert!(
        entered.get(),
        "genuine checked transition must reach the fixture"
    );
    (result, budget.storage())
}

fn owned_optimizer_fixture_v1744(
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionSourceOwnedViewV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> (SourceOwnedResultV18<()>, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(folding_source_owner_v18, &mut budget);
    let result = prepared.with_source_consumer_v18(&mut budget, consume);
    (result, budget.storage())
}

#[test]
fn owned_correspondence_large_header_short_and_control_refusal_keep_the_first_error() {
    for (short, deny) in [(true, false), (false, false), (true, true), (false, true)] {
        let drops = std::cell::Cell::new(0);
        let invoked = std::cell::Cell::new(0);
        let refusal = std::cell::Cell::new(None);
        let (result, retained) = owned_correspondence_fixture_v1744(|original, checked, budget| {
            let owned = OptimizedAnalysisOwnedCaptureV18 {
                bytes: [0x27; 2048],
                drops: &drops,
                deny: deny.then_some(original.source.cleanup),
            };
            let called = &invoked;
            let consume = move |_: &ProductionOptimizedSourceCorrespondenceV18<'_>,
                                _: &mut ArgumentBudgetV1<'_>| {
                std::hint::black_box(&owned);
                called.set(called.get() + 1);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            };
            assert_eq!(std::mem::align_of_val(&consume), 256);
            let header = optimized_correspondence_header_oracle_v18::<
                (),
                ProductionSourceOwnedViewErrorV18,
                _,
            >(&consume);
            let filler = MODULE_LIMIT - budget.storage() - header + usize::from(short);
            budget.reserve_storage(filler)?;
            let floor = budget.storage();
            let work = budget.work();
            let Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(
                error,
            ))) = original.with_optimized_correspondence_v18(checked, budget, consume)
            else {
                panic!("header or first control allocation must refuse");
            };
            let next = if short {
                1
            } else {
                size_of::<fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV18<'_, '_, '_>>()
            };
            assert_eq!(
                (error.actual(), error.limit()),
                (MODULE_LIMIT + next, MODULE_LIMIT)
            );
            assert_eq!(
                budget.work() - work,
                (if short { 2 } else { 3 }) + 32 + 2 + 1 + 4
            );
            assert_eq!(
                budget.storage(),
                floor + if deny && !short { header } else { 0 }
            );
            assert_eq!(
                budget.peak_storage(),
                if short { floor } else { MODULE_LIMIT }
            );
            assert_eq!((invoked.get(), drops.get()), (0, 1));
            assert_eq!(original.source.cleanup.is_denied(), deny);
            refusal.set(Some(error));
            budget.release_storage(filler)?;
            Ok(())
        });
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error)))
            if Some(error) == refusal.get())
        );
        if deny {
            assert!(retained > MODULE_FLOOR);
        } else {
            assert_eq!(retained, MODULE_FLOOR);
        }
    }
}

#[test]
fn owned_correspondence_success_keeps_only_independent_callback_backing() {
    let drops = std::cell::Cell::new(0);
    let invoked = std::cell::Cell::new(0);
    let (result, retained) = owned_correspondence_fixture_v1744(|original, checked, budget| {
        let owned = OptimizedAnalysisOwnedCaptureV18 {
            bytes: [0x27; 2048],
            drops: &drops,
            deny: None,
        };
        let called = &invoked;
        let expected = std::cell::Cell::new(0);
        let paid = &expected;
        let floor = budget.storage();
        let consume = move |_: &ProductionOptimizedSourceCorrespondenceV18<'_>,
                            budget: &mut ArgumentBudgetV1<'_>| {
            std::hint::black_box(&owned);
            called.set(called.get() + 1);
            assert_eq!(budget.storage(), floor + paid.get());
            budget.reserve_storage(37)?;
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        };
        expected.set(optimized_source_retained_oracle_v18(
            original, checked, &consume,
        ));
        original.with_optimized_correspondence_v18(checked, budget, consume)?;
        assert_eq!((invoked.get(), drops.get()), (1, 1));
        assert_eq!(
            budget.storage(),
            floor + 37,
            "no helper credit or callback storage is refunded twice"
        );
        budget.release_storage(37)?;
        Ok(())
    });
    result.unwrap();
    assert_eq!(retained, MODULE_FLOOR);
}

#[test]
fn owned_optimizer_header_exact_and_short_refusals_are_caught_and_latched() {
    for (short, deny) in [(true, false), (false, false), (true, true), (false, true)] {
        let drops = std::cell::Cell::new(0);
        let invoked = std::cell::Cell::new(0);
        let refusal = std::cell::Cell::new(None);
        let (result, retained) = owned_optimizer_fixture_v1744(|source, budget| {
            let owned = OptimizedAnalysisOwnedCaptureV18 {
                bytes: [0x27; 2048],
                drops: &drops,
                deny: deny.then_some(source.cleanup),
            };
            let called = &invoked;
            let consume = move |_: &ProductionSourceCorrespondenceV18<'_>,
                                _: &ProductionOptimizedSourceCorrespondenceV18<'_>,
                                _: &mut ArgumentBudgetV1<'_>| {
                std::hint::black_box(&owned);
                called.set(called.get() + 1);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
            };
            let header =
                owned_optimizer_header_oracle_v1744::<(), ProductionSourceOwnedViewErrorV18, _>(
                    &consume,
                );
            assert_eq!(
                optimized_source_consumer_resources_v18::optimizer_entry_headers::<
                    (),
                    ProductionSourceOwnedViewErrorV18,
                    _,
                >(&consume)?
                    + owned_optimizer_disposal_header_oracle_v26::<
                        (),
                        ProductionSourceOwnedViewErrorV18,
                    >(),
                header
            );
            let filler = MODULE_LIMIT - budget.storage() - header + usize::from(short);
            budget.reserve_storage(filler)?;
            let floor = budget.storage();
            let actual = source.with_checked_optimization_v18(budget, consume);
            let error = if short {
                let Err(ProductionSourceOptimizationErrorV18::Source(
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error)),
                )) = actual
                else {
                    panic!("owned optimizer header must refuse before observation");
                };
                assert_eq!(
                    (error.actual(), error.limit()),
                    (MODULE_LIMIT + 1, MODULE_LIMIT)
                );
                error
            } else {
                let Err(ProductionSourceOptimizationErrorV18::Observation(error)) = actual else {
                    panic!("exact header precedes observation storage refusal");
                };
                let SourceOwnedQueryFailureV18::Resource(ArgumentResourceV1::Storage(storage)) =
                    observed_optimizer_refusal_v18(&error)
                else {
                    panic!("observation must retain its original storage refusal");
                };
                assert!(storage.actual() > MODULE_LIMIT);
                storage
            };
            assert_eq!(budget.failed_storage(), Some(error.actual()));
            assert_eq!((invoked.get(), drops.get()), (0, 1));
            assert_eq!(
                budget.storage(),
                floor + if deny && !short { header } else { 0 }
            );
            assert_eq!(
                budget.peak_storage(),
                if short { floor } else { MODULE_LIMIT }
            );
            refusal.set(Some(error));
            budget.release_storage(filler)?;
            Ok(())
        });
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error)))
            if Some(error) == refusal.get())
        );
        if deny {
            assert!(retained > MODULE_FLOOR);
        } else {
            assert_eq!(retained, MODULE_FLOOR);
        }
    }
}

#[test]
fn owned_optimizer_success_preserves_exact_adoption_transfer_floor() {
    let drops = std::cell::Cell::new(0);
    let invoked = std::cell::Cell::new(0);
    let (result, retained) = owned_optimizer_fixture_v1744(|source, budget| {
        let owned = OptimizedAnalysisOwnedCaptureV18 {
            bytes: [0x27; 2048],
            drops: &drops,
            deny: None,
        };
        let called = &invoked;
        let floor = budget.storage();
        let (owner, (), receipt) = source
            .with_checked_optimization_v18(budget, move |original, optimized, budget| {
                std::hint::black_box(&owned);
                called.set(called.get() + 1);
                assert!(std::ptr::eq(
                    original.inventory,
                    optimized.input_inventory(budget)?
                ));
                Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
            })
            .map_err(|error| match error {
                ProductionSourceOptimizationErrorV18::Source(error) => error,
                other => panic!("genuine optimized handoff: {other:?}"),
            })?;
        assert_eq!((invoked.get(), drops.get()), (1, 1));
        assert_eq!(
            budget.storage(),
            floor,
            "observation credit and entry header retire exactly once"
        );
        let transferred = owner.storage().retained_storage() + receipt.retained_storage();
        assert!(transferred > 0);
        budget.reserve_storage(transferred)?;
        drop(owner);
        budget.release_storage(transferred)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
    result.unwrap();
    assert_eq!(retained, MODULE_FLOOR);
}

#[test]
fn owned_retained_optimizer_header_exact_and_short_refusals_keep_custody() {
    for (short, deny) in [(true, false), (false, false), (true, true), (false, true)] {
        let drops = std::cell::Cell::new(0);
        let invoked = std::cell::Cell::new(0);
        let refusal = std::cell::Cell::new(None);
        let (result, retained) = owned_optimizer_fixture_v1744(|source, budget| {
            let owned = OptimizedAnalysisOwnedCaptureV18 {
                bytes: [0x27; 2048],
                drops: &drops,
                deny: deny.then_some(source.cleanup),
            };
            let called = &invoked;
            let consume = move |_: &ProductionSourceCorrespondenceV18<'_>,
                                _: &ProductionOptimizedSourceCorrespondenceV18<'_>,
                                _: &mut ArgumentBudgetV1<'_>| {
                std::hint::black_box(&owned);
                called.set(called.get() + 1);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
            };
            let header = owned_retained_optimizer_header_oracle_v1744::<
                (),
                ProductionSourceOwnedViewErrorV18,
                _,
            >(&consume);
            let inner =
                owned_optimizer_header_oracle_v1744::<(), ProductionSourceOwnedViewErrorV18, _>(
                    &consume,
                );
            let filler = MODULE_LIMIT - budget.storage() - header + usize::from(short);
            budget.reserve_storage(filler)?;
            let floor = budget.storage();
            let work = budget.work();
            let Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error)),
            )) = source.with_retained_checked_optimization_v18(budget, consume)
            else {
                panic!("outer or checked-entry header must refuse before observation");
            };
            assert_eq!(
                (error.actual(), error.limit()),
                (MODULE_LIMIT + if short { 1 } else { inner }, MODULE_LIMIT)
            );
            assert_eq!(budget.failed_storage(), Some(error.actual()));
            assert_eq!(
                budget.work() - work,
                if short {
                    1
                } else {
                    3 + SOURCE_REFERENCE_PAYLOAD_ATTEMPTS_V29 + 32 + 2 + 1 + 4
                }
            );
            assert_eq!(
                budget.storage(),
                floor + if deny && !short { header } else { 0 }
            );
            assert_eq!(
                budget.peak_storage(),
                if short { floor } else { MODULE_LIMIT }
            );
            assert_eq!((invoked.get(), drops.get()), (0, 1));
            assert_eq!(source.cleanup.is_denied(), deny);
            refusal.set(Some(error));
            budget.release_storage(filler)?;
            Ok(())
        });
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error)))
            if Some(error) == refusal.get())
        );
        if deny {
            assert!(retained > MODULE_FLOOR);
        } else {
            assert_eq!(retained, MODULE_FLOOR);
        }
    }
}

#[test]
fn owned_retained_optimizer_success_pays_both_transferred_owners_without_double_reserve() {
    let drops = std::cell::Cell::new(0);
    let invoked = std::cell::Cell::new(0);
    let (result, retained) = owned_optimizer_fixture_v1744(|source, budget| {
        budget.reserve_storage(29)?;
        let floor = budget.storage();
        let owned = OptimizedAnalysisOwnedCaptureV18 {
            bytes: [0x27; 2048],
            drops: &drops,
            deny: None,
        };
        let called = &invoked;
        let (owner, payload, receipt) = source
            .with_retained_checked_optimization_v18(budget, move |_, _, budget| {
                std::hint::black_box(&owned);
                called.set(called.get() + 1);
                let header = size_of::<Vec<u8>>();
                budget.reserve_storage(header + 33)?;
                let mut payload = Vec::<u8>::new();
                payload
                    .try_reserve_exact(33)
                    .map_err(|_| ArgumentResourceV1::Allocation)?;
                budget.reserve_storage(payload.capacity() - 33)?;
                budget.charge_work(33)?;
                payload.resize(33, 0x44);
                let bytes = header + payload.capacity();
                budget.release_storage(bytes)?;
                Ok::<_, ProductionSourceOwnedViewErrorV18>((payload, bytes))
            })
            .map_err(|error| match error {
                ProductionSourceOptimizationErrorV18::Source(error) => error,
                other => panic!("genuine retained transfer: {other:?}"),
            })?;
        assert_eq!((invoked.get(), drops.get()), (1, 1));
        assert_eq!(payload.len(), 33);
        assert!(payload.iter().all(|byte| *byte == 0x44));
        let transfer = owner.storage().retained_storage() + receipt.retained_storage();
        assert_eq!(budget.storage(), floor + transfer);
        assert!(receipt.retained_storage() >= size_of::<Vec<u8>>() + payload.capacity());
        drop((owner, payload, receipt));
        budget.release_storage(transfer)?;
        assert_eq!(budget.storage(), floor, "caller-owned backing remains paid");
        budget.release_storage(29)?;
        Ok(())
    });
    result.unwrap();
    assert_eq!(retained, MODULE_FLOOR);
}

fn rejected_owned_entry_v1744(
    entrant: u8,
    original: &ProductionSourceCorrespondenceV18<'_>,
    checked: &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV18<'_, '_, '_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
    owned: OptimizedAnalysisOwnedCaptureV18<'_>,
    called: &std::cell::Cell<usize>,
) -> SourceOwnedResultV18<()> {
    if entrant != 0 {
        let consume = move |_: &ProductionSourceCorrespondenceV18<'_>,
                            _: &ProductionOptimizedSourceCorrespondenceV18<'_>,
                            _: &mut ArgumentBudgetV1<'_>| {
            std::hint::black_box(&owned);
            called.set(called.get() + 1);
            Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
        };
        let actual = if entrant == 2 {
            original
                .source
                .with_retained_checked_optimization_v18(budget, consume)
        } else {
            original
                .source
                .with_checked_optimization_v18(budget, consume)
        };
        actual
            .map(|(owner, (), _)| drop(owner))
            .map_err(|error| match error {
                ProductionSourceOptimizationErrorV18::Source(error) => error,
                other => panic!("rejected entry reached observation/adoption: {other:?}"),
            })
    } else {
        original.with_optimized_correspondence_v18(checked, budget, move |_, _| {
            std::hint::black_box(&owned);
            called.set(called.get() + 1);
            Ok(())
        })
    }
}

#[test]
fn owned_optimized_entrants_reject_foreign_ledger_and_wrong_slot_without_charging() {
    for entrant in 0..3 {
        for wrong_slot in [false, true] {
            let drops = std::cell::Cell::new(0);
            let invoked = std::cell::Cell::new(0);
            let (result, retained) =
                owned_correspondence_fixture_v1744(|original, checked, budget| {
                    let owned = OptimizedAnalysisOwnedCaptureV18 {
                        bytes: [0x27; 2048],
                        drops: &drops,
                        deny: None,
                    };
                    let before = (budget.work(), budget.storage());
                    let meter = Box::leak(Box::new(CanonicalKernelIrWorkBudgetV1::new(
                        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                    )));
                    let mut foreign = ArgumentBudgetV1::new(meter, MODULE_LIMIT);
                    foreign.reserve_storage(before.1)?;
                    if wrong_slot {
                        std::mem::swap(budget, &mut foreign);
                    }
                    let other_before = (foreign.work(), foreign.storage());
                    let actual = rejected_owned_entry_v1744(
                        entrant,
                        original,
                        checked,
                        &mut foreign,
                        owned,
                        &invoked,
                    );
                    assert!(matches!(
                        actual,
                        Err(ProductionSourceOwnedViewErrorV18::Resource(
                            ArgumentResourceV1::Accounting
                        ))
                    ));
                    assert_eq!((foreign.work(), foreign.storage()), other_before);
                    if wrong_slot {
                        std::mem::swap(budget, &mut foreign);
                    }
                    assert_eq!((budget.work(), budget.storage()), before);
                    assert_eq!((invoked.get(), drops.get()), (0, 1));
                    assert!(original.source.cleanup.is_denied());
                    Ok(())
                });
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert!(retained > MODULE_FLOOR);
        }
    }
}

#[test]
fn owned_optimized_entrants_keep_early_typed_refusal_despite_drop_denial() {
    for entrant in 0..3 {
        let drops = std::cell::Cell::new(0);
        let invoked = std::cell::Cell::new(0);
        let (result, retained) = owned_correspondence_fixture_v1744(|original, checked, budget| {
            let _ = original.retain_query::<()>(Err(ProductionSourceOwnedViewErrorV18::Binding(
                "owned entry first refusal",
            )));
            let owned = OptimizedAnalysisOwnedCaptureV18 {
                bytes: [0x27; 2048],
                drops: &drops,
                deny: Some(original.source.cleanup),
            };
            let before = (budget.work(), budget.storage());
            let actual =
                rejected_owned_entry_v1744(entrant, original, checked, budget, owned, &invoked);
            assert!(matches!(
                actual,
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "owned entry first refusal"
                ))
            ));
            assert_eq!((budget.work(), budget.storage()), before);
            assert_eq!((invoked.get(), drops.get()), (0, 1));
            assert!(original.source.cleanup.is_denied());
            Ok(())
        });
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "owned entry first refusal"
            ))
        ));
        assert!(retained > MODULE_FLOOR);
    }
}

#[test]
fn owned_optimized_entrants_keep_single_early_drop_panic_raw() {
    struct PanicCapture<'a>(OptimizedAnalysisOwnedCaptureV18<'a>);
    impl Drop for PanicCapture<'_> {
        fn drop(&mut self) {
            std::panic::resume_unwind(Box::new(0x1744_u64));
        }
    }
    for entrant in 0..3 {
        let drops = std::cell::Cell::new(0);
        let invoked = std::cell::Cell::new(0);
        let (result, retained) = owned_correspondence_fixture_v1744(|original, checked, budget| {
            let _ = original.retain_query::<()>(Err(ProductionSourceOwnedViewErrorV18::Binding(
                "before entry Drop panic",
            )));
            let capture = PanicCapture(OptimizedAnalysisOwnedCaptureV18 {
                bytes: [0x27; 2048],
                drops: &drops,
                deny: Some(original.source.cleanup),
            });
            budget.reserve_storage(size_of::<u64>())?;
            let before = (budget.work(), budget.storage());
            let called = &invoked;
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if entrant != 0 {
                    let consume =
                        move |_: &ProductionSourceCorrespondenceV18<'_>,
                              _: &ProductionOptimizedSourceCorrespondenceV18<'_>,
                              _: &mut ArgumentBudgetV1<'_>| {
                            std::hint::black_box(&capture);
                            called.set(called.get() + 1);
                            Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                        };
                    let _ = if entrant == 2 {
                        original
                            .source
                            .with_retained_checked_optimization_v18(budget, consume)
                    } else {
                        original
                            .source
                            .with_checked_optimization_v18(budget, consume)
                    };
                } else {
                    let _ =
                        original.with_optimized_correspondence_v18(checked, budget, move |_, _| {
                            std::hint::black_box(&capture);
                            called.set(called.get() + 1);
                            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                        });
                }
            }));
            assert_eq!(*caught.unwrap_err().downcast::<u64>().unwrap(), 0x1744);
            assert_eq!((budget.work(), budget.storage()), before);
            assert_eq!((invoked.get(), drops.get()), (0, 1));
            assert!(original.source.cleanup.is_denied());
            budget.release_storage(size_of::<u64>())?;
            Ok(())
        });
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "before entry Drop panic"
            ))
        ));
        assert!(retained > MODULE_FLOOR);
    }
}

#[test]
fn owned_correspondence_typed_and_raw_callback_failures_preserve_selection() {
    for raw in [false, true] {
        for deny in [false, true] {
            let drops = std::cell::Cell::new(0);
            let invoked = std::cell::Cell::new(0);
            let (result, retained) =
                owned_correspondence_fixture_v1744(|original, checked, budget| {
                    let owned = OptimizedAnalysisOwnedCaptureV18 {
                        bytes: [0x27; 2048],
                        drops: &drops,
                        deny: deny.then_some(original.source.cleanup),
                    };
                    let called = &invoked;
                    budget.reserve_storage(size_of::<u64>())?;
                    let floor = budget.storage();
                    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        original.with_optimized_correspondence_v18(checked, budget, move |_, _| {
                            std::hint::black_box(&owned);
                            called.set(called.get() + 1);
                            if raw {
                                std::panic::resume_unwind(Box::new(0x1744_0008_u64));
                            }
                            Err::<(), _>(ProductionSourceOwnedViewErrorV18::Binding(
                                "selected owned correspondence callback",
                            ))
                        })
                    }));
                    if raw {
                        assert_eq!(*caught.unwrap_err().downcast::<u64>().unwrap(), 0x1744_0008);
                    } else {
                        assert!(matches!(
                            caught,
                            Ok(Err(ProductionSourceOwnedViewErrorV18::Binding(
                                "selected owned correspondence callback"
                            )))
                        ));
                    }
                    assert_eq!((invoked.get(), drops.get()), (1, 1));
                    if deny {
                        assert!(budget.storage() > floor);
                    } else {
                        assert_eq!(budget.storage(), floor);
                    }
                    budget.release_storage(size_of::<u64>())?;
                    if raw {
                        Ok(())
                    } else {
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "selected owned correspondence callback",
                        ))
                    }
                });
            if !raw {
                assert!(matches!(
                    result,
                    Err(ProductionSourceOwnedViewErrorV18::Binding(
                        "selected owned correspondence callback"
                    ))
                ));
            } else if deny {
                assert!(matches!(
                    result,
                    Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Accounting
                    ))
                ));
            } else {
                result.unwrap();
            }
            if deny {
                assert!(retained > MODULE_FLOOR);
            } else {
                assert_eq!(retained, MODULE_FLOOR);
            }
        }
    }
}

#[test]
fn owned_correspondence_restored_callback_floor_keeps_denial_sticky() {
    let drops = std::cell::Cell::new(0);
    let invoked = std::cell::Cell::new(0);
    let (result, retained) = owned_correspondence_fixture_v1744(|original, checked, budget| {
        let owned = OptimizedAnalysisOwnedCaptureV18 {
            bytes: [0x27; 2048],
            drops: &drops,
            deny: None,
        };
        let called = &invoked;
        let live = std::cell::Cell::new(0);
        let last = &live;
        let actual =
            original.with_optimized_correspondence_v18(checked, budget, move |view, budget| {
                std::hint::black_box(&owned);
                called.set(called.get() + 1);
                budget.release_storage(1)?;
                let work = budget.work();
                assert!(matches!(
                    view.input_inventory(budget),
                    Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Accounting
                    ))
                ));
                assert_eq!(budget.work(), work);
                budget.reserve_storage(1)?;
                last.set(budget.storage());
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            });
        assert!(matches!(
            actual,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Accounting
            ))
        ));
        assert_eq!(budget.storage(), live.get());
        assert_eq!((invoked.get(), drops.get()), (1, 1));
        assert!(original.source.cleanup.is_denied());
        Ok(())
    });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
    assert!(retained > MODULE_FLOOR);
}

#[test]
fn owned_correspondence_late_index_work_refusal_settles_paid_components_only() {
    for deny in [false, true] {
        let drops = std::cell::Cell::new(0);
        let invoked = std::cell::Cell::new(0);
        let refusal = std::cell::Cell::new(None);
        let (result, retained) = owned_correspondence_fixture_v1744(|original, checked, budget| {
            let before = budget.work();
            let (control, receipt) =
                fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV18::derive_v18(
                    checked, budget,
                )
                .map_err(|_| ProductionSourceOwnedViewErrorV18::Binding("control setup"))?;
            let control_work = budget.work() - before;
            let control_bytes = receipt.retained_storage();
            drop(control);
            let prefix = 2 + 32 + 2 + 1 + 4 + control_work + 3 + 1;
            budget.charge_work(OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work() - prefix)?;
            let owned = OptimizedAnalysisOwnedCaptureV18 {
                bytes: [0x27; 2048],
                drops: &drops,
                deny: deny.then_some(original.source.cleanup),
            };
            let called = &invoked;
            let consume = move |_: &ProductionOptimizedSourceCorrespondenceV18<'_>,
                                _: &mut ArgumentBudgetV1<'_>| {
                std::hint::black_box(&owned);
                called.set(called.get() + 1);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            };
            let headers = optimized_correspondence_header_oracle_v18::<
                (),
                ProductionSourceOwnedViewErrorV18,
                _,
            >(&consume);
            let vector = checked.input().operations().len()
                * size_of::<Option<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>>();
            let floor = budget.storage();
            let Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error))) =
                original.with_optimized_correspondence_v18(checked, budget, consume)
            else {
                panic!(
                    "second index-vector work debit must refuse after paid control and first vector"
                );
            };
            assert_eq!(
                (error.actual(), error.limit()),
                (
                    OPTIMIZED_SOURCE_WORK_LIMIT_V18 + 1,
                    OPTIMIZED_SOURCE_WORK_LIMIT_V18
                )
            );
            assert_eq!(budget.work(), OPTIMIZED_SOURCE_WORK_LIMIT_V18);
            assert_eq!(
                budget.failed_work(),
                Some(OPTIMIZED_SOURCE_WORK_LIMIT_V18 + 1)
            );
            assert_eq!(
                budget.storage(),
                floor
                    + if deny {
                        headers + control_bytes + vector
                    } else {
                        0
                    }
            );
            assert_eq!((invoked.get(), drops.get()), (0, 1));
            refusal.set(Some(error));
            Ok(())
        });
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error)))
            if Some(error) == refusal.get())
        );
        if deny {
            assert!(retained > MODULE_FLOOR);
        } else {
            assert_eq!(retained, MODULE_FLOOR);
        }
    }
}

#[test]
fn owned_optimizer_adoption_keeps_origin_error_and_existing_panic_classification() {
    for raw in [false, true] {
        for deny in [false, true] {
            let drops = std::cell::Cell::new(0);
            let invoked = std::cell::Cell::new(0);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let prepared = scalar_payload_prepared_from_v18(folding_source_owner_v18, &mut budget);
            let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
                budget
                    .reserve_storage(size_of::<u64>())
                    .map_err(ProductionSourceOwnedViewErrorV18::from)?;
                let payload = Box::new(0x1744_0011_u64);
                let owned = OptimizedAnalysisOwnedCaptureV18 {
                    bytes: [0x27; 2048],
                    drops: &drops,
                    deny: deny.then_some(source.cleanup),
                };
                let called = &invoked;
                let floor = budget.storage();
                let actual = source.with_checked_optimization_v18(budget, move |_, _, _| {
                    std::hint::black_box(&owned);
                    called.set(called.get() + 1);
                    if raw {
                        std::panic::resume_unwind(payload);
                    }
                    Err::<((), usize), _>(ProductionSourceOwnedViewErrorV18::Binding(
                        "selected owned optimizer origin",
                    ))
                });
                assert_eq!((invoked.get(), drops.get()), (1, 1));
                if deny {
                    assert!(budget.storage() > floor);
                } else {
                    assert_eq!(budget.storage(), floor);
                }
                if raw {
                    assert!(matches!(
                        actual,
                        Err(ProductionSourceOptimizationErrorV18::Adoption(
                            fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Panicked
                        ))
                    ));
                } else {
                    assert!(matches!(
                        actual,
                        Err(ProductionSourceOptimizationErrorV18::Adoption(
                            fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                                ProductionSourceOwnedViewErrorV18::Binding(
                                    "selected owned optimizer origin"
                                )
                            )
                        ))
                    ));
                }
                budget
                    .release_storage(size_of::<u64>())
                    .map_err(ProductionSourceOwnedViewErrorV18::from)?;
                actual.map(|(owner, (), _)| drop(owner))
            });
            if raw {
                assert!(matches!(
                    result,
                    Err(ProductionSourceOptimizationErrorV18::Source(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "actual source optimizer adoption rejected"
                        )
                    ))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionSourceOptimizationErrorV18::Adoption(
                        fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                            ProductionSourceOwnedViewErrorV18::Binding(
                                "selected owned optimizer origin"
                            )
                        )
                    ))
                ));
            }
            if deny {
                assert!(budget.storage() > MODULE_FLOOR);
            } else {
                assert_eq!(budget.storage(), MODULE_FLOOR);
            }
        }
    }
}
