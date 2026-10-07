//! CPU lifecycle stress, not native admission or hardware concurrency evidence.
use super::*;

const COUNT: usize = 4096;

fn ticks(index: usize, round: usize) -> usize {
    // Distinct short and long cohorts force out-of-order decoders and periods
    // without transitions, so the actual caller-provided notification is used.
    if (index + round).is_multiple_of(2) {
        64 + (COUNT - 1 - index) % 8
    } else {
        (COUNT - 1 - index) % 8
    }
}

fn stress(tokio: bool) {
    let mut context = context();
    let device = context.devices()[0].id();
    let streams: Vec<_> = (0..COUNT)
        .map(|_| context.create_stream(device).unwrap())
        .collect();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let order = RefCell::new(Vec::with_capacity(COUNT));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let pulses = (!tokio).then(Pulse::new);
    for round in 0..2 {
        order.borrow_mut().clear();
        let inputs: Vec<_> = (0..COUNT)
            .map(|index| {
                context.bound_preparation_for_test_v1(Borrowed {
                    ticks: Cell::new(ticks(index, round)),
                    decoded: &decoded,
                    dropped: &dropped,
                    domain: Arc::new(()),
                    completion_order: Some((&order, index)),
                })
            })
            .collect();
        let (slots, copies) = allocate_rosters(COUNT).unwrap();
        let mut scope = RuntimeGfx942GeneratedScopeV1 {
            epoch: context.scope_epoch.begin().unwrap(),
            context: &mut context,
            slots,
            copies,
            graph: None,
            capacity: COUNT,
            deadline: Instant::now() + Duration::from_secs(120),
            identity: Rc::new(()),
            invariant: PhantomData,
            hooks: hooks(),
        };
        let capacities = (scope.slots.capacity(), scope.copies.capacity());
        let arrays = (scope.slots.as_ptr(), scope.copies.as_ptr());
        assert_eq!(
            scope.roster_capacity_bytes_v1().unwrap(),
            capacities.0 * size_of::<Slot<Borrowed<'_>>>()
                + capacities.1 * size_of::<crate::context::generated_scope::copies::CopySlot>()
        );
        assert!(
            scope.roster_capacity_bytes_v1().unwrap() <= MAX_RUNTIME_GFX942_SCOPED_ROSTER_BYTES_V1
        );
        let tickets: Vec<_> = inputs
            .into_iter()
            .zip(&streams)
            .map(|(prepared, &stream)| scope.admit(prepared, stream).unwrap())
            .collect();
        assert_eq!(scope.pending_v1(), COUNT);
        assert_eq!(
            scope
                .context
                .streams
                .values()
                .filter(|record| record.unpublished.is_some())
                .count(),
            COUNT
        );
        assert_eq!(
            (decoded.get(), dropped.get()),
            (round * COUNT, round * COUNT)
        );
        assert!(matches!(
            scope.check_submission(),
            Err(RuntimeGfx942ScopeErrorV1::Capacity)
        ));
        let mut observers: Vec<_> = tickets
            .iter()
            .map(|ticket| scope.completion_future_v1(ticket).unwrap())
            .collect();
        for observer in &mut observers {
            assert!(poll(observer, Waker::noop()).is_pending());
        }
        assert_eq!(
            scope.pending_v1(),
            COUNT,
            "polling observers may not progress owners"
        );
        let waits = Cell::new(0);
        let (driver, results) = if tokio {
            runtime.block_on(async {
                futures_util::future::join(
                    scope.drive_with_wake_v1(|deadline| {
                        waits.set(waits.get() + 1);
                        tokio::time::sleep_until(
                            std::cmp::min(deadline, Instant::now() + Duration::from_millis(1))
                                .into(),
                        )
                    }),
                    futures_util::future::join_all(observers),
                )
                .await
            })
        } else {
            futures_executor::LocalPool::new().run_until(futures_util::future::join(
                scope.drive_with_wake_v1(|_| {
                    waits.set(waits.get() + 1);
                    pulses.as_ref().unwrap().next()
                }),
                futures_util::future::join_all(observers),
            ))
        };
        driver.unwrap();
        assert_eq!(results.len(), COUNT);
        assert!(results.into_iter().all(|result| result.is_ok()));
        assert!(
            waits.get() > 0,
            "mixed durations must use actual executor notifications"
        );
        assert_eq!(scope.pending_v1(), 0);
        assert!(!scope.context.has_unpublished_holds_v1());
        assert_eq!(
            (decoded.get(), dropped.get()),
            ((round + 1) * COUNT, (round + 1) * COUNT)
        );
        let mut expected: Vec<_> = (0..COUNT).collect();
        expected.sort_by_key(|&index| (ticks(index, round), index));
        assert_ne!(expected, (0..COUNT).collect::<Vec<_>>());
        assert_eq!(*order.borrow(), expected, "actual original decoder order");
        assert_eq!(
            arrays,
            (scope.slots.as_ptr(), scope.copies.as_ptr()),
            "owner arrays reallocated"
        );
        assert_eq!(
            capacities,
            (scope.slots.capacity(), scope.copies.capacity())
        );
        for ticket in &tickets {
            assert!(matches!(scope.completion_v1(ticket).unwrap(), Some(Ok(()))));
        }
        assert!(
            matches!(
                scope.check_submission(),
                Err(RuntimeGfx942ScopeErrorV1::Capacity)
            ),
            "settlement must not silently reuse ticket slots"
        );
        drop((tickets, scope));
        assert!(!context.has_unpublished_holds_v1());
        assert!(context.submissions.is_empty());
    }
    for stream in streams {
        context.destroy_stream(stream).unwrap();
    }
    assert!(context.cleanup().is_complete());
}

#[test]
fn scoped_4096_mixed_duration_futures_settle_out_of_order_on_tokio_and_reuse_context() {
    stress(true);
}

#[test]
fn scoped_4096_mixed_duration_futures_settle_out_of_order_on_local_pool_and_reuse_context() {
    stress(false);
}

#[test]
fn scoped_roster_count_and_exact_backing_bytes_are_checked_before_allocation() {
    for capacity in [0, 4097, usize::MAX] {
        assert!(matches!(
            allocate_rosters::<Borrowed<'_>>(capacity),
            Err(RuntimeGfx942ScopeErrorV1::Capacity)
        ));
    }
    // A large inline carrier is refused even at a valid submission count. No
    // such value or allocation is constructed by this size-only test.
    assert!(matches!(
        allocate_rosters::<[u8; 16384]>(4096),
        Err(RuntimeGfx942ScopeErrorV1::Capacity)
    ));
    assert!(matches!(
        roster_bytes::<Borrowed<'_>>(usize::MAX, 1),
        Err(RuntimeGfx942ScopeErrorV1::Capacity)
    ));
    assert!(matches!(
        roster_bytes::<Borrowed<'_>>(1, usize::MAX),
        Err(RuntimeGfx942ScopeErrorV1::Capacity)
    ));
    let exact = MAX_RUNTIME_GFX942_SCOPED_ROSTER_BYTES_V1 / size_of::<Slot<Borrowed<'_>>>();
    assert_eq!(
        roster_bytes::<Borrowed<'_>>(exact, 0).unwrap(),
        exact * size_of::<Slot<Borrowed<'_>>>()
    );
    assert!(matches!(
        roster_bytes::<Borrowed<'_>>(exact + 1, 0),
        Err(RuntimeGfx942ScopeErrorV1::Capacity)
    ));
}
