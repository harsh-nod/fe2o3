use super::*;
use crate::RuntimeGfx942GeneratedReservationErrorV1 as ReserveError;

struct ReservablePayload {
    _local: LocalPayload,
    calls: Arc<AtomicUsize>,
    outcome: u8,
}

pub(super) fn roster() -> crate::generated_source::GeneratedHostRosterV1 {
    let hsaco = crate::synthetic_cov6::preparation_module();
    let mut explicit = vec![0; 16];
    explicit[8..].copy_from_slice(&4u64.to_le_bytes());
    let projection = crate::prepare_gfx942_runtime_dispatch_v1(
        &hsaco,
        "vecadd",
        crate::Gfx942RuntimeDispatchInputsV1::new(
            explicit,
            vec![
                crate::Gfx942RuntimeDispatchBufferV1::new(
                    vec![0; 16],
                    crate::Gfx942RuntimeBufferAccessV1::ReadWrite,
                )
                .unwrap(),
            ],
            vec![fe2o3_kfd::Gfx942KfdDispatchPointerFixupV1::new(0, 0, 0, 4)],
            fe2o3_aql::AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            0,
            1000,
        ),
    )
    .unwrap()
    .into_persistent_projection_v1(&hsaco)
    .unwrap();
    crate::generated_source::GeneratedHostRosterV1::from_projection(&projection).unwrap()
}

fn adapter<B: RuntimeBackendV1>(
    _context: &mut RuntimeContextV1<B>,
    payload: &mut ReservablePayload,
) -> Result<crate::generated_source::GeneratedHostRosterV1, ReserveError> {
    payload.calls.fetch_add(1, Ordering::SeqCst);
    match payload.outcome {
        1 => Err(ReserveError::InvalidRoster),
        2 => panic!("reservation adapter panic"),
        _ => Ok(roster()),
    }
}

#[test]
fn reservation_queue_cutoff_disconnect_and_reentrancy_recover_ticket() {
    for mode in 0..4 {
        let mut h = Harness::new(1, 2, true);
        let calls = Arc::new(AtomicUsize::new(0));
        let ticket = park(
            &mut h,
            calls.clone(),
            Arc::new(AtomicUsize::new(0)),
            0,
            false,
        );
        let key = ticket.key.clone();
        let expected = match mode {
            0 => {
                for _ in 0..4 {
                    h.handle
                        .observer
                        .sender
                        .try_send(RuntimeAsyncEngineCommandV1::Stop)
                        .unwrap_or_else(|_| panic!("queue capacity"));
                }
                RuntimeAsyncEngineCallErrorV1::CommandQueueFull
            }
            1 => {
                h.handle.observer.admission.close();
                RuntimeAsyncEngineCallErrorV1::EngineStopped
            }
            2 => {
                drop(h.receiver);
                RuntimeAsyncEngineCallErrorV1::EngineStopped
            }
            _ => {
                h.handle
                    .observer
                    .worker_thread
                    .set(thread::current().id())
                    .unwrap();
                RuntimeAsyncEngineCallErrorV1::ReentrantCall
            }
        };
        let failure = h
            .handle
            .try_reserve_prepared_v1(ticket)
            .err()
            .expect("admission rejection");
        assert!(matches!(failure.error, ReserveError::Engine(error) if error == expected));
        assert!(Arc::ptr_eq(&key, &failure.ticket.key));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
    }
}

#[test]
fn reservation_terminal_context_returns_original_ticket_without_hook() {
    let mut h = Harness::new(1, 2, true);
    let calls = Arc::new(AtomicUsize::new(0));
    let ticket = park(
        &mut h,
        calls.clone(),
        Arc::new(AtomicUsize::new(0)),
        0,
        false,
    );
    let key = ticket.key.clone();
    let future = h.handle.try_reserve_prepared_v1(ticket).unwrap();
    h.context.quarantine_after_async_command_panic_v1();
    assert!(h.command());
    let failure = ready(future).unwrap().unwrap_err();
    assert!(matches!(
        failure.error,
        ReserveError::Engine(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    assert!(Arc::ptr_eq(&key, &failure.ticket.key));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn reserved_owned_engine_drain_and_shutdown_dispose_on_owner() {
    let drops = Arc::new(AtomicUsize::new(0));
    let captured = drops.clone();
    let state = Arc::new(Mutex::new(MockState::default()));
    let (engine, handle) = start(state.clone(), Arc::new(Mutex::new(OwnerTrace::default())));
    let preparation = handle
        .enqueue_preparation_with_reservation_v1(
            Box::new(move |_| {
                Ok::<_, ()>(ReservablePayload {
                    _local: LocalPayload {
                        local: Rc::new(Cell::new(0)),
                        drops: captured,
                        owner: thread::current().id(),
                        panic_on_drop: false,
                    },
                    calls: Arc::new(AtomicUsize::new(0)),
                    outcome: 0,
                })
            }),
            Some(adapter),
        )
        .unwrap();
    let ticket = join(preparation).unwrap().unwrap();
    let ticket = join(handle.try_reserve_prepared_v1(ticket).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    let report = join(handle.begin_drain(16).unwrap()).unwrap();
    assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    assert_eq!(report.operations_remaining, 0);
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(state.lock().unwrap().issues.is_empty());
    assert!(state.lock().unwrap().flush_calls.is_empty());
    drop(ticket);
    assert_eq!(handle.observer.reply_cells_in_use(), 0);
}

fn park(
    h: &mut Harness,
    calls: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
    outcome: u8,
    panic_on_drop: bool,
) -> RuntimeAsyncPreparedTicketV1 {
    let future = h
        .handle
        .enqueue_preparation_with_reservation_v1(
            Box::new(move |_| {
                Ok::<_, ()>(ReservablePayload {
                    _local: LocalPayload {
                        local: Rc::new(Cell::new(0)),
                        drops,
                        owner: thread::current().id(),
                        panic_on_drop,
                    },
                    calls,
                    outcome,
                })
            }),
            Some(adapter),
        )
        .unwrap();
    h.command();
    h.advance();
    ready(future).unwrap().unwrap()
}

fn reserved(h: &mut Harness, drops: Arc<AtomicUsize>) -> RuntimeAsyncReservedTicketV1 {
    let calls = Arc::new(AtomicUsize::new(0));
    let ticket = park(h, calls.clone(), drops, 0, false);
    let future = h.handle.try_reserve_prepared_v1(ticket).unwrap();
    h.command();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    ready(future).unwrap().unwrap()
}

#[test]
fn reservation_is_finite_parked_and_disposable_without_native_effects() {
    let mut h = Harness::new(1, 3, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let ticket = reserved(&mut h, drops.clone());
    assert_eq!((h.registry.len(), h.registry.active_len()), (1, 0));
    assert_eq!(h.handle.observer.reply_cells_in_use(), 1);
    for _ in 0..4 {
        h.advance();
    }
    assert!(h.state.lock().unwrap().flush_calls.is_empty());
    assert!(h.state.lock().unwrap().issues.is_empty());
    let discard = h.handle.try_discard_reserved_v1(ticket).unwrap();
    h.command();
    assert_eq!(ready(discard), Ok(()));
    assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
    assert_eq!(h.registry.len(), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn reservation_two_cell_exhaustion_returns_unchanged_ticket_before_hook() {
    for occupied in [false, true] {
        let mut h = Harness::new(1, 1, true);
        let calls = Arc::new(AtomicUsize::new(0));
        let ticket = park(
            &mut h,
            calls.clone(),
            Arc::new(AtomicUsize::new(0)),
            0,
            false,
        );
        let key = ticket.key.clone();
        let held = occupied
            .then(|| owned::Reply::<()>::budgeted_pair(&h.handle.observer.reply_budget).unwrap());
        let failure = h
            .handle
            .try_reserve_prepared_v1(ticket)
            .err()
            .expect("reply capacity");
        assert!(Arc::ptr_eq(&failure.ticket.key, &key));
        assert!(matches!(
            failure.error,
            ReserveError::Engine(RuntimeAsyncEngineCallErrorV1::ReplyCapacity)
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            h.handle.observer.reply_cells_in_use(),
            usize::from(occupied)
        );
        assert!(h.receiver.try_recv().is_err());
        drop(held);
    }
}

#[test]
fn reservation_queued_stop_returns_exact_ticket_without_calling_hook() {
    let mut h = Harness::new(1, 2, true);
    let calls = Arc::new(AtomicUsize::new(0));
    let ticket = park(
        &mut h,
        calls.clone(),
        Arc::new(AtomicUsize::new(0)),
        0,
        false,
    );
    let key = ticket.key.clone();
    let future = h.handle.try_reserve_prepared_v1(ticket).unwrap();
    drop(h.receiver.try_recv().unwrap());
    let failure = ready(future).unwrap().unwrap_err();
    assert!(Arc::ptr_eq(&failure.ticket.key, &key));
    assert!(matches!(
        failure.error,
        ReserveError::Engine(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
    let discard = h.handle.try_discard_prepared_v1(failure.ticket).unwrap();
    h.command();
    assert_eq!(ready(discard), Ok(()));
}

#[test]
fn reservation_generic_ticket_rejects_without_discarding_payload() {
    let mut h = Harness::new(1, 2, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let ticket = h.park(drops.clone());
    let key = ticket.key.clone();
    let future = h.handle.try_reserve_prepared_v1(ticket).unwrap();
    h.command();
    let failure = ready(future).unwrap().unwrap_err();
    assert!(Arc::ptr_eq(&failure.ticket.key, &key));
    assert!(matches!(
        failure.error,
        ReserveError::UnsupportedPreparation
    ));
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(h.registry.len(), 1);
}

#[test]
fn reservation_rejects_foreign_unknown_and_replayed_ticket_identity() {
    let mut h = Harness::new(1, 3, true);
    let other = Harness::new(1, 3, true);
    let ticket = park(
        &mut h,
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicUsize::new(0)),
        0,
        false,
    );
    let failure = other
        .handle
        .try_reserve_prepared_v1(ticket)
        .err()
        .expect("foreign context");
    assert!(matches!(
        failure.error,
        ReserveError::Engine(RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket)
    ));
    let replay = RuntimeAsyncPreparedTicketV1 {
        key: failure.ticket.key.clone(),
    };
    let unknown = RuntimeAsyncPreparedTicketV1 {
        key: Arc::new(generated_operation::PreparedKeyV1 {
            context_generation: failure.ticket.key.context_generation,
        }),
    };
    let future = h.handle.try_reserve_prepared_v1(unknown).unwrap();
    h.command();
    assert!(matches!(
        ready(future).unwrap().unwrap_err().error,
        ReserveError::Engine(RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket)
    ));
    let future = h.handle.try_reserve_prepared_v1(failure.ticket).unwrap();
    h.command();
    let reserved = ready(future).unwrap().unwrap();
    let discard = h.handle.try_discard_prepared_v1(replay).unwrap();
    h.command();
    assert_eq!(
        ready(discard),
        Err(RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket)
    );
    let replay = RuntimeAsyncPreparedTicketV1 {
        key: reserved.key.clone(),
    };
    let future = h.handle.try_reserve_prepared_v1(replay).unwrap();
    h.command();
    assert!(matches!(
        ready(future).unwrap().unwrap_err().error,
        ReserveError::Engine(RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket)
    ));
    assert_eq!(h.registry.len(), 1);
}

#[test]
fn reservation_ordinary_adapter_error_preserves_original_owner_and_ticket() {
    let mut h = Harness::new(1, 2, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(AtomicUsize::new(0));
    let ticket = park(&mut h, calls.clone(), drops.clone(), 1, false);
    let key = ticket.key.clone();
    let future = h.handle.try_reserve_prepared_v1(ticket).unwrap();
    h.command();
    let failure = ready(future).unwrap().unwrap_err();
    assert!(matches!(failure.error, ReserveError::InvalidRoster));
    assert!(Arc::ptr_eq(&key, &failure.ticket.key));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert!(!h.context.is_terminal());
    assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
}

#[test]
fn reservation_adapter_panic_terminalizes_context_without_returning_retry_ticket() {
    let mut h = Harness::new(1, 2, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let ticket = park(
        &mut h,
        Arc::new(AtomicUsize::new(0)),
        drops.clone(),
        2,
        false,
    );
    let future = h.handle.try_reserve_prepared_v1(ticket).unwrap();
    assert!(h.command());
    assert!(matches!(
        ready(future),
        Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
    ));
    assert!(h.context.is_terminal());
    assert_eq!(h.registry.len(), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
}

#[test]
fn reservation_observer_loss_keeps_completion_producer_and_payload_until_stop() {
    let mut h = Harness::new(1, 2, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let ticket = park(
        &mut h,
        Arc::new(AtomicUsize::new(0)),
        drops.clone(),
        0,
        false,
    );
    let future = h.handle.try_reserve_prepared_v1(ticket).unwrap();
    drop(future);
    h.command();
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(h.handle.observer.reply_cells_in_use(), 1);
    assert!(!h.registry.stop_observations());
    assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
    assert_eq!(h.registry.len(), 1);
    h.registry.dispose_quiescent();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn reservation_and_reserved_discard_notify_latest_waker() {
    let mut h = Harness::new(1, 3, true);
    let ticket = park(
        &mut h,
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicUsize::new(0)),
        0,
        false,
    );
    let mut future = h.handle.try_reserve_prepared_v1(ticket).unwrap();
    let prior = Arc::new(PreparationWakeCount(AtomicUsize::new(0)));
    let latest = Arc::new(PreparationWakeCount(AtomicUsize::new(0)));
    for waker in [Waker::from(prior.clone()), Waker::from(latest.clone())] {
        assert!(
            Pin::new(&mut future)
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
    }
    h.command();
    assert_eq!(prior.0.load(Ordering::SeqCst), 0);
    assert_eq!(latest.0.load(Ordering::SeqCst), 1);
    let mut discard = h
        .handle
        .try_discard_reserved_v1(ready(future).unwrap().unwrap())
        .unwrap();
    for waker in [Waker::from(prior.clone()), Waker::from(latest.clone())] {
        assert!(
            Pin::new(&mut discard)
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
    }
    h.command();
    assert_eq!(prior.0.load(Ordering::SeqCst), 0);
    assert_eq!(latest.0.load(Ordering::SeqCst), 2);
    assert_eq!(ready(discard), Ok(()));
}

#[test]
fn reserved_discard_admission_returns_foreign_and_capacity_limited_ticket() {
    let mut h = Harness::new(1, 2, true);
    let other = Harness::new(1, 2, true);
    let ticket = reserved(&mut h, Arc::new(AtomicUsize::new(0)));
    let failure = other
        .handle
        .try_discard_reserved_v1(ticket)
        .err()
        .expect("foreign context");
    assert_eq!(
        failure.error,
        RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket
    );
    let held = owned::Reply::<()>::budgeted_pair(&h.handle.observer.reply_budget).unwrap();
    let failure = h
        .handle
        .try_discard_reserved_v1(failure.ticket)
        .err()
        .expect("reply capacity");
    assert_eq!(failure.error, RuntimeAsyncEngineCallErrorV1::ReplyCapacity);
    drop(held);
    let discard = h.handle.try_discard_reserved_v1(failure.ticket).unwrap();
    h.command();
    assert_eq!(ready(discard), Ok(()));
}

#[test]
fn reserved_discard_panic_does_not_dispose_later_parked_owner() {
    let mut h = Harness::new(2, 3, true);
    let first = Arc::new(AtomicUsize::new(0));
    let later = Arc::new(AtomicUsize::new(0));
    let ticket = park(
        &mut h,
        Arc::new(AtomicUsize::new(0)),
        first.clone(),
        0,
        true,
    );
    let _later = h.park(later.clone());
    let future = h.handle.try_reserve_prepared_v1(ticket).unwrap();
    h.command();
    let discard = h
        .handle
        .try_discard_reserved_v1(ready(future).unwrap().unwrap())
        .unwrap();
    assert!(h.command());
    assert_eq!(
        ready(discard),
        Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
    );
    assert_eq!(first.load(Ordering::SeqCst), 1);
    assert_eq!(later.load(Ordering::SeqCst), 0);
    assert_eq!(h.registry.len(), 1);
}
