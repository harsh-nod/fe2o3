const SOURCE_OWNED_CALLBACK_WORK_LIMIT_V18: usize = 500_000_000;

#[derive(Debug)]
enum OwnedCallbackErrorV18 {
    Source(ProductionSourceOwnedViewErrorV18),
    Payload(Vec<u64>),
}

impl From<ProductionSourceOwnedViewErrorV18> for OwnedCallbackErrorV18 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}

fn owned_callback_payload_v18(budget: &mut ArgumentBudgetV1<'_>) -> Vec<u64> {
    budget
        .reserve_storage(64 * std::mem::size_of::<u64>())
        .unwrap();
    let payload = vec![0x271_u64; 64];
    assert_eq!(payload.capacity(), 64);
    payload
}

#[test]
fn source_callback_success_error_and_raw_panic_keep_only_owned_backing() {
    for mode in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SOURCE_OWNED_CALLBACK_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(scalar_payload_owner_v18, &mut budget);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            prepared.with_source_consumer_v18(&mut budget, |source, budget| {
                source.root_count(budget)?;
                let payload = owned_callback_payload_v18(budget);
                match mode {
                    0 => Ok(payload),
                    1 => Err(OwnedCallbackErrorV18::Payload(payload)),
                    _ => {
                        budget
                            .reserve_storage(std::mem::size_of::<Vec<u64>>())
                            .unwrap();
                        std::panic::resume_unwind(Box::new(payload))
                    }
                }
            })
        }));
        let extra = if mode == 2 {
            std::mem::size_of::<Vec<u64>>()
        } else {
            0
        };
        let backing = 64 * std::mem::size_of::<u64>() + extra;
        assert_eq!(budget.storage(), MODULE_FLOOR + backing);
        match (mode, caught) {
            (0, Ok(Ok(payload))) | (1, Ok(Err(OwnedCallbackErrorV18::Payload(payload)))) => {
                assert_eq!(payload, vec![0x271; 64]);
                drop(payload);
            }
            (2, Err(payload)) => {
                let payload = payload.downcast::<Vec<u64>>().unwrap();
                assert_eq!(*payload, vec![0x271; 64]);
                drop(payload);
            }
            other => panic!("callback disposition changed: {other:?}"),
        }
        budget.release_storage(backing).unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
        assert_eq!(
            (budget.failed_work(), budget.failed_storage()),
            (None, None)
        );
    }
}

#[derive(Debug)]
struct HostileOwnedPayloadV26<'a> {
    dropped: &'a std::cell::Cell<bool>,
    backing: Vec<u64>,
}
impl Drop for HostileOwnedPayloadV26<'_> {
    fn drop(&mut self) {
        assert_eq!(self.backing.len(), 64);
        self.dropped.set(true);
        panic!("discarded source callback payload");
    }
}

#[derive(Debug)]
enum HostileOwnedErrorV26<'a> {
    Source(ProductionSourceOwnedViewErrorV18),
    Payload(HostileOwnedPayloadV26<'a>),
}
impl From<ProductionSourceOwnedViewErrorV18> for HostileOwnedErrorV26<'_> {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}

#[test]
fn source_callback_ignored_first_error_protects_returned_value_and_error_destructors() {
    for returned_error in [false, true] {
        let dropped = std::cell::Cell::new(false);
        let reached = std::cell::Cell::new(false);
        let last_work = std::cell::Cell::new(0);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SOURCE_OWNED_CALLBACK_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(scalar_payload_owner_v18, &mut budget);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            prepared.with_source_consumer_v18(&mut budget, |source, budget| {
                let payload = HostileOwnedPayloadV26 {
                    dropped: &dropped,
                    backing: owned_callback_payload_v18(budget),
                };
                let _ = source.missing::<()>("first callback query refusal");
                reached.set(true);
                last_work.set(budget.work());
                if returned_error {
                    Err(HostileOwnedErrorV26::Payload(payload))
                } else {
                    Ok(payload)
                }
            })
        }));
        assert!(reached.get());
        assert!(dropped.get());
        assert!(matches!(
            caught,
            Ok(Err(HostileOwnedErrorV26::Source(
                ProductionSourceOwnedViewErrorV18::Binding("first callback query refusal")
            )))
        ));
        assert_eq!(
            budget.work(),
            last_work.get(),
            "settlement must use prepaid work"
        );
        // The generic boundary cannot refund consumer-owned heap credit even
        // after rejecting and safely destroying the value which held it.
        assert_eq!(budget.storage(), MODULE_FLOOR + 64 * size_of::<u64>());
        budget.release_storage(64 * size_of::<u64>()).unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[derive(Debug)]
struct PanickingSourceErrorConversionV26;
impl From<ProductionSourceOwnedViewErrorV18> for PanickingSourceErrorConversionV26 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        assert!(matches!(
            error,
            ProductionSourceOwnedViewErrorV18::Binding("conversion boundary refusal")
        ));
        panic!("selected source error conversion");
    }
}

#[test]
fn source_callback_error_conversion_panic_settles_before_resuming_unwind() {
    let reached = std::cell::Cell::new(false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SOURCE_OWNED_CALLBACK_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(scalar_payload_owner_v18, &mut budget);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        prepared.with_source_consumer_v18(&mut budget, |source, _| {
            let _ = source.missing::<()>("conversion boundary refusal");
            reached.set(true);
            Ok::<_, PanickingSourceErrorConversionV26>(())
        })
    }));
    assert!(reached.get());
    let payload = caught.expect_err("the generic conversion deliberately panics");
    assert_eq!(
        payload.downcast_ref::<&str>(),
        Some(&"selected source error conversion")
    );
    drop(payload);
    assert_eq!(budget.storage(), MODULE_FLOOR);
    assert_eq!(
        (budget.failed_work(), budget.failed_storage()),
        (None, None)
    );
}

#[test]
fn source_consumer_preparation_refusal_drops_uncalled_capture_before_attempt_refund() {
    for accepted_view_headers in [false, true] {
        for deny in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let (pending, capture) = pending_query_drop_fixture_v1751(&mut budget);
            let incoming = budget.storage();
            let result = with_scoped_source_cleanup_v29(
                &mut budget,
                incoming,
                |cleanup, budget| -> SourceOwnedResultV18<()> {
                    let invoked = std::cell::Cell::new(false);
                    let dropped = std::cell::Cell::new(false);
                    let payload = SourceFinishDropV26 {
                        cleanup,
                        dropped: &dropped,
                        deny,
                    };
                    let called = &invoked;
                    let consume =
                        move |_: &ProductionSourceOwnedViewV18<'_>,
                              _: &mut ArgumentBudgetV1<'_>| {
                            std::hint::black_box(&payload);
                            called.set(true);
                            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                        };
                    fn callback_headers<F>(_: &F) -> usize {
                        size_of::<Option<F>>() + std::mem::align_of::<Option<F>>()
                    }
                    let view_headers = callback_headers(&consume)
                        + source_owned_finish_header_oracle_v26::<
                            (),
                            ProductionSourceOwnedViewErrorV18,
                        >()
                        + size_of::<SourceOwnedQueryGuardV18>()
                        + size_of::<ProductionSourceOwnedViewV18<'_>>()
                        + size_of::<
                            std::thread::Result<Result<(), ProductionSourceOwnedViewErrorV18>>,
                        >();
                    let remaining = 32 + 2 + 1 + 4 - usize::from(!accepted_view_headers);
                    budget
                        .charge_work(MODULE_LIMIT - budget.work() - remaining)
                        .unwrap();
                    let floor = budget.storage();
                    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        pending.with_source_consumer_with_cleanup_v18(cleanup, budget, consume)
                    }));
                    assert!(dropped.get());
                    assert!(!invoked.get());
                    let Ok(Err(SourceConsumerErrorV18(
                        ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(
                            error,
                        )),
                    ))) = caught
                    else {
                        panic!("typed preparation refusal must survive uncalled callback Drop");
                    };
                    assert_eq!(error.limit(), MODULE_LIMIT);
                    assert_eq!(budget.failed_work(), Some(error.actual()));
                    assert_eq!(cleanup.is_denied(), deny);
                    if deny {
                        assert!(budget.storage() > floor);
                        if accepted_view_headers {
                            assert!(budget.storage() >= floor + view_headers);
                        }
                    } else {
                        assert_eq!(budget.storage(), floor);
                    }
                    Ok(())
                },
            );
            if deny {
                assert!(matches!(
                    result,
                    Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Accounting
                    ))
                ));
                assert!(budget.storage() > incoming);
            } else {
                result.unwrap();
                assert_eq!(budget.storage(), incoming);
            }
            let retained = pending.adopted_storage();
            drop(pending);
            if !deny {
                budget.release_storage(retained + capture).unwrap();
                assert_eq!(budget.storage(), MODULE_FLOOR);
            }
        }
    }
}
