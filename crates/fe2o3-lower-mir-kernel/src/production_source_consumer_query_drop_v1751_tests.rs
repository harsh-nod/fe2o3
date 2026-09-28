struct SourceQueryDropV1751<'a> {
    drops: &'a std::cell::Cell<usize>,
    deny: Option<&'a ScopedSourceCleanupV29>,
    payload: Option<Box<u64>>,
}

impl Drop for SourceQueryDropV1751<'_> {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
        if let Some(cleanup) = self.deny {
            cleanup.deny_refund();
        }
        if let Some(payload) = self.payload.take() {
            std::panic::resume_unwind(payload);
        }
    }
}

fn pending_query_drop_fixture_v1751(
    budget: &mut ArgumentBudgetV1<'_>,
) -> (ProductionPendingScopedSourceOwnerV29, usize) {
    with_pending_api_input(
        ModuleFixture::Ordinary,
        true,
        budget,
        |owner, launch, input, capture, budget| {
            (
                ProductionPendingScopedSourceOwnerV29::try_materialize_with_budget(
                    owner,
                    launch,
                    input,
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )
                .unwrap(),
                capture,
            )
        },
    )
}

fn leave_first_source_query_one_short_v1751(
    budget: &mut ArgumentBudgetV1<'_>,
    entrance_work: usize,
) {
    assert!(entrance_work > 1);
    let prefix = entrance_work - 1;
    budget
        .charge_work(MODULE_LIMIT - budget.work() - prefix)
        .unwrap();
}

#[test]
fn pending_first_view_query_drop_panic_retires_exact_inner_and_outer_headers() {
    for raw_drop in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (pending, capture) = pending_query_drop_fixture_v1751(&mut budget);
        let floor = budget.storage();
        let probe_called = std::cell::Cell::new(false);
        let before = budget.work();
        pending
            .with_checked_source_v18(&mut budget, |_, _| {
                probe_called.set(true);
                Ok(())
            })
            .unwrap();
        assert!(probe_called.get());
        let entrance_work = budget.work() - before;
        assert_eq!(budget.storage(), floor);
        let drops = std::cell::Cell::new(0);
        let invoked = std::cell::Cell::new(0);
        budget.reserve_storage(size_of::<u64>()).unwrap();
        let owned = SourceQueryDropV1751 {
            drops: &drops,
            deny: None,
            payload: raw_drop.then(|| Box::new(0x1751_0001_u64)),
        };
        let called = &invoked;
        leave_first_source_query_one_short_v1751(&mut budget, entrance_work);
        let incoming = budget.storage();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pending.with_checked_source_v18(&mut budget, move |_, _| {
                std::hint::black_box(&owned);
                called.set(called.get() + 1);
                Ok(())
            })
        }));
        assert_eq!((invoked.get(), drops.get()), (0, 1));
        assert_eq!(
            budget.work(),
            MODULE_LIMIT,
            "replay exhausted exactly the paid prefix"
        );
        if raw_drop {
            assert_eq!(*caught.unwrap_err().downcast::<u64>().unwrap(), 0x1751_0001);
        } else {
            assert!(
                matches!(caught.unwrap(), Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error)))
                if error.actual() == MODULE_LIMIT + 1 && error.limit() == MODULE_LIMIT)
            );
        }
        assert_eq!(
            budget.storage(),
            incoming,
            "no inner paid view header may survive the raw Drop path"
        );
        budget.release_storage(size_of::<u64>()).unwrap();
        let retained = pending.adopted_storage();
        drop(pending);
        budget.release_storage(retained + capture).unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
        assert_eq!(work.failed_work(), Some(MODULE_LIMIT + 1));
    }
}

#[test]
fn prepared_first_view_query_drop_panic_retires_pending_owner_and_inner_headers() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let probe = prepared_source_fixture(ModuleFixture::Ordinary, true, &mut budget);
    let probe_called = std::cell::Cell::new(false);
    let before = budget.work();
    probe
        .with_checked_source_v18(&mut budget, |_, _| {
            probe_called.set(true);
            Ok(())
        })
        .unwrap();
    assert!(probe_called.get());
    let entrance_work = budget.work() - before;
    assert_eq!(budget.storage(), MODULE_FLOOR);
    let prepared = prepared_source_fixture(ModuleFixture::Ordinary, true, &mut budget);
    let adopted = prepared.adopted_storage();
    let drops = std::cell::Cell::new(0);
    let invoked = std::cell::Cell::new(0);
    budget.reserve_storage(size_of::<u64>()).unwrap();
    let owned = SourceQueryDropV1751 {
        drops: &drops,
        deny: None,
        payload: Some(Box::new(0x1751_0002_u64)),
    };
    let called = &invoked;
    leave_first_source_query_one_short_v1751(&mut budget, entrance_work);
    let incoming = budget.storage();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        prepared.with_checked_source_v18(&mut budget, move |_, _| {
            std::hint::black_box(&owned);
            called.set(called.get() + 1);
            Ok(())
        })
    }));
    assert_eq!(*caught.unwrap_err().downcast::<u64>().unwrap(), 0x1751_0002);
    assert_eq!((invoked.get(), drops.get()), (0, 1));
    assert_eq!(budget.work(), MODULE_LIMIT);
    assert_eq!(
        budget.storage(),
        incoming - adopted,
        "only caller-owned floor and panic backing survive"
    );
    budget.release_storage(size_of::<u64>()).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
    assert_eq!(work.failed_work(), Some(MODULE_LIMIT + 1));
}

#[test]
fn pending_first_view_query_drop_observes_linked_denial_before_any_header_refund() {
    for deny in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (pending, capture) = pending_query_drop_fixture_v1751(&mut budget);
        budget.reserve_storage(size_of::<u64>()).unwrap();
        let incoming = budget.storage();
        let drops = std::cell::Cell::new(0);
        let invoked = std::cell::Cell::new(0);
        let result = with_scoped_source_cleanup_v29(
            &mut budget,
            incoming,
            |cleanup, budget| -> SourceOwnedResultV18<()> {
                let before = budget.work();
                pending
                    .with_source_consumer_with_cleanup_v18(cleanup, budget, |_, _| {
                        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                    })
                    .map_err(|error| error.0)?;
                let entrance_work = budget.work() - before;
                let owned = SourceQueryDropV1751 {
                    drops: &drops,
                    deny: deny.then_some(cleanup),
                    payload: Some(Box::new(0x1751_0003_u64)),
                };
                let called = &invoked;
                leave_first_source_query_one_short_v1751(budget, entrance_work);
                let floor = budget.storage();
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    pending
                        .with_source_consumer_with_cleanup_v18(cleanup, budget, move |_, _| {
                            std::hint::black_box(&owned);
                            called.set(called.get() + 1);
                            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                        })
                        .map_err(|error| error.0)
                }));
                assert_eq!(*caught.unwrap_err().downcast::<u64>().unwrap(), 0x1751_0003);
                assert_eq!((invoked.get(), drops.get()), (0, 1));
                assert_eq!(budget.work(), MODULE_LIMIT);
                let headers = size_of::<SourceOwnedQueryGuardV18>()
                    + size_of::<ProductionSourceOwnedViewV18<'_>>()
                    + size_of::<std::thread::Result<Result<(), ProductionSourceOwnedViewErrorV18>>>(
                    );
                assert_eq!(budget.storage(), floor + if deny { headers } else { 0 });
                assert_eq!(cleanup.is_denied(), deny);
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
        budget.release_storage(size_of::<u64>()).unwrap();
        let retained = pending.adopted_storage();
        drop(pending);
        if !deny {
            budget.release_storage(retained + capture).unwrap();
            assert_eq!(budget.storage(), MODULE_FLOOR);
        }
        assert_eq!(work.failed_work(), Some(MODULE_LIMIT + 1));
    }
}
