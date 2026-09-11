use super::*;
use std::sync::atomic::AtomicUsize;

mod reservation_tests;

fn ready<F: Future + Unpin>(mut future: F) -> F::Output {
    match Pin::new(&mut future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("expected finite preparation result"),
    }
}

fn join<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Poll::Ready(value) = future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            return value;
        }
        assert!(Instant::now() < deadline, "preparation stalled");
        thread::yield_now();
    }
}

struct LocalPayload {
    local: Rc<Cell<usize>>,
    drops: Arc<AtomicUsize>,
    owner: ThreadId,
    panic_on_drop: bool,
}

impl Drop for LocalPayload {
    fn drop(&mut self) {
        assert_eq!(thread::current().id(), self.owner);
        self.local.set(self.local.get() + 1);
        self.drops.fetch_add(1, Ordering::SeqCst);
        assert!(!self.panic_on_drop, "prepared payload disposal panic");
    }
}

fn prepare<B: RuntimeBackendV1 + 'static>(
    handle: &RuntimeAsyncProgressHandleV1<B>,
    calls: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
    panic_on_drop: bool,
) -> Result<RuntimeAsyncPreparationV1<()>, RuntimeAsyncEngineCallErrorV1> {
    handle.enqueue_preparation_v1(Box::new(move |_| {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(LocalPayload {
            local: Rc::new(Cell::new(0)),
            drops,
            owner: thread::current().id(),
            panic_on_drop,
        })
    }))
}

struct Harness {
    context: RuntimeContextV1<MockBackend>,
    state: Arc<Mutex<MockState>>,
    handle: RuntimeAsyncProgressHandleV1<MockBackend>,
    receiver: Receiver<RuntimeAsyncEngineCommandV1<MockBackend>>,
    registry: operation::OperationRegistryV1<MockBackend>,
    config: RuntimeAsyncEngineConfigV1,
}

impl Harness {
    fn new(capacity: usize, replies: usize, owned: bool) -> Self {
        let state = Arc::new(Mutex::new(MockState::default()));
        let context = RuntimeContextV1::open(MockBackend {
            state: state.clone(),
        })
        .unwrap();
        let config = RuntimeAsyncEngineConfigV1::new(4, capacity, 4, 4, Duration::from_millis(1))
            .unwrap()
            .with_reply_capacity(replies)
            .unwrap();
        let (sender, receiver) = sync_channel(4);
        let handle = RuntimeAsyncProgressHandleV1 {
            observer: RuntimeAsyncEngineHandleV1 {
                context_generation: context.capture_context_generation_v1(),
                capture_budget: None,
                reply_budget: reply_budget::ReplyBudgetV1::new(replies),
                admission: drain::AdmissionV1::new(),
                sender,
                worker_thread: Arc::new(OnceLock::new()),
                quarantine_command_panics: true,
                graph_slot: Arc::new(AtomicBool::new(false)),
                snapshot_budget: snapshot::SnapshotBudgetV1::new(8),
            },
        };
        Self {
            context,
            state,
            handle,
            receiver,
            registry: operation::OperationRegistryV1::new(capacity, owned),
            config,
        }
    }

    fn command(&mut self) -> bool {
        let command = self.receiver.try_recv().unwrap();
        handle_command_v1(
            &mut self.context,
            &mut BTreeMap::new(),
            &mut self.registry,
            &mut None,
            None,
            command,
            self.config,
            Some(RuntimeAsyncProgressConfigV1::default()),
        )
    }

    fn advance(&mut self) {
        operation::advance_operations_v1(
            &mut self.context,
            &mut self.registry,
            4,
            4,
            flush_stream_v1::<MockBackend>,
        );
    }

    fn park(&mut self, drops: Arc<AtomicUsize>) -> RuntimeAsyncPreparedTicketV1 {
        let calls = Arc::new(AtomicUsize::new(0));
        let future = prepare(&self.handle, calls.clone(), drops, false).unwrap();
        let control = future.control();
        self.command();
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        self.advance();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            control.phase(),
            RuntimeAsyncOperationPhaseV1::ObservationFinished
        );
        ready(future).unwrap().unwrap()
    }
}

#[test]
fn preparation_parks_once_without_flush_and_discard_reclaims_capacity_after_drop() {
    let mut h = Harness::new(2, 2, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let ticket = h.park(drops.clone());
    assert_eq!((h.registry.len(), h.registry.active_len()), (1, 0));
    assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
    for _ in 0..8 {
        h.advance();
    }
    assert!(h.state.lock().unwrap().flush_calls.is_empty());
    assert!(h.state.lock().unwrap().issues.is_empty());
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    let future = h.handle.try_discard_prepared_v1(ticket).unwrap();
    h.command();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(h.registry.len(), 0);
    assert_eq!(ready(future), Ok(()));
}

struct PreparationWakeCount(AtomicUsize);

impl std::task::Wake for PreparationWakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn preparation_and_discard_notify_only_the_latest_registered_waker() {
    let mut h = Harness::new(1, 1, true);
    let calls = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let mut future = prepare(&h.handle, calls.clone(), drops.clone(), false).unwrap();
    let prior = Arc::new(PreparationWakeCount(AtomicUsize::new(0)));
    let latest = Arc::new(PreparationWakeCount(AtomicUsize::new(0)));
    let prior_waker = Waker::from(prior.clone());
    let latest_waker = Waker::from(latest.clone());
    for waker in [&prior_waker, &latest_waker] {
        assert!(
            Pin::new(&mut future)
                .poll(&mut Context::from_waker(waker))
                .is_pending()
        );
    }
    h.command();
    h.advance();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(prior.0.load(Ordering::SeqCst), 0);
    assert_eq!(latest.0.load(Ordering::SeqCst), 1);
    let ticket = ready(future).unwrap().unwrap();
    assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
    let mut discard = h.handle.try_discard_prepared_v1(ticket).unwrap();
    for waker in [&prior_waker, &latest_waker] {
        assert!(
            Pin::new(&mut discard)
                .poll(&mut Context::from_waker(waker))
                .is_pending()
        );
    }
    h.command();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(ready(discard), Ok(()));
    for _ in 0..4 {
        h.advance();
    }
    assert_eq!(prior.0.load(Ordering::SeqCst), 0);
    assert_eq!(latest.0.load(Ordering::SeqCst), 2);
    assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
    assert_eq!(h.registry.len(), 0);
}

struct PanickingPreparationCompletion {
    inner: Box<dyn operation::EngineOperationV1<MockBackend>>,
    complete_first: bool,
}

impl operation::EngineOperationV1<MockBackend> for PanickingPreparationCompletion {
    fn advance(&mut self, context: &mut RuntimeContextV1<MockBackend>) -> bool {
        self.inner.advance(context)
    }

    fn stream(&self) -> Option<RuntimeStreamIdV1> {
        self.inner.stream()
    }

    fn prepared_key(&self) -> Option<&Arc<generated_operation::PreparedKeyV1>> {
        self.inner.prepared_key()
    }

    fn complete_preparation(&mut self) {
        if self.complete_first {
            self.inner.complete_preparation();
        }
        panic!("parked completion adapter panic");
    }

    fn reject(&mut self, error: RuntimeAsyncEngineCallErrorV1) {
        self.inner.reject(error);
    }
}

#[test]
fn preparation_completion_panic_preserves_parked_custody_before_and_after_reply() {
    for complete_first in [false, true] {
        let mut h = Harness::new(1, 1, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let calls = Arc::new(AtomicUsize::new(0));
        let future = prepare(&h.handle, calls.clone(), drops.clone(), false).unwrap();
        let RuntimeAsyncEngineCommandV1::Operation(mut factory) = h.receiver.try_recv().unwrap()
        else {
            panic!("expected preparation factory");
        };
        // Wrap the real driver only at its post-park completion callback.
        h.registry.insert(Box::new(PanickingPreparationCompletion {
            inner: factory.materialize(),
            complete_first,
        }));
        drop(factory);
        h.advance();
        assert!(h.context.is_terminal());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!((h.registry.len(), h.registry.active_len()), (1, 0));
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        if complete_first {
            let ticket = ready(future).unwrap().unwrap();
            let discard = h.handle.try_discard_prepared_v1(ticket).unwrap();
            assert!(h.command());
            assert_eq!(
                ready(discard),
                Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
            );
        } else {
            assert!(matches!(
                ready(future),
                Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
            ));
        }
        assert!(!h.registry.stop_observations());
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
        assert!(h.state.lock().unwrap().flush_calls.is_empty());
        std::mem::forget(h.registry);
    }
}

#[test]
fn preparation_parked_and_active_entries_share_one_capacity() {
    let mut h = Harness::new(2, 4, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let ticket = h.park(drops.clone());
    let calls = Arc::new(AtomicUsize::new(0));
    let active = prepare(&h.handle, calls.clone(), drops.clone(), false).unwrap();
    h.command();
    assert_eq!((h.registry.len(), h.registry.active_len()), (2, 1));
    let rejected = prepare(&h.handle, calls.clone(), drops.clone(), false).unwrap();
    h.command();
    assert!(matches!(
        ready(rejected),
        Err(RuntimeAsyncEngineCallErrorV1::OperationCapacity)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    h.advance();
    let second = ready(active).unwrap().unwrap();
    assert_eq!((h.registry.len(), h.registry.active_len()), (2, 0));
    for ticket in [ticket, second] {
        let future = h.handle.try_discard_prepared_v1(ticket).unwrap();
        h.command();
        assert_eq!(ready(future), Ok(()));
    }
    assert_eq!(drops.load(Ordering::SeqCst), 2);
}

#[test]
fn preparation_cancellation_precedes_callback_and_does_not_stop_engine() {
    let mut h = Harness::new(1, 2, true);
    let calls = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let future = prepare(&h.handle, calls.clone(), drops.clone(), false).unwrap();
    let control = future.control();
    assert_eq!(
        control.cancel_before_submission(),
        RuntimeAsyncCancelResultV1::CancelledBeforeSubmission
    );
    h.command();
    h.advance();
    assert!(matches!(
        ready(future),
        Err(RuntimeAsyncEngineCallErrorV1::CancelledBeforeSubmission)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(!h.context.is_terminal());
    let ticket = h.park(drops);
    drop(ticket);
    assert_eq!(h.registry.len(), 1);
}

#[test]
fn preparation_owner_requirement_and_capacity_reject_before_callback() {
    for owned in [false, true] {
        let mut h = Harness::new(1, 2, owned);
        let calls = Arc::new(AtomicUsize::new(0));
        let drops = Arc::new(AtomicUsize::new(0));
        if owned {
            drop(h.park(drops.clone()));
        }
        let future = prepare(&h.handle, calls.clone(), drops, false).unwrap();
        h.command();
        let expected = if owned {
            RuntimeAsyncEngineCallErrorV1::OperationCapacity
        } else {
            RuntimeAsyncEngineCallErrorV1::EngineStopped
        };
        assert!(matches!(ready(future), Err(error) if error == expected));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn preparation_observer_and_ticket_drop_do_not_dispose_parked_owner() {
    for drop_early in [false, true] {
        let mut h = Harness::new(1, 2, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let future = prepare(
            &h.handle,
            Arc::new(AtomicUsize::new(0)),
            drops.clone(),
            false,
        )
        .unwrap();
        h.command();
        if drop_early {
            drop(future);
            h.advance();
        } else {
            h.advance();
            drop(ready(future).unwrap().unwrap());
        }
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
        assert_eq!((h.registry.len(), h.registry.active_len()), (1, 0));
        h.registry.dispose_quiescent();
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn preparation_discard_exact_identity_rejects_foreign_and_replayed_tickets() {
    let mut h = Harness::new(2, 4, true);
    let mut foreign = Harness::new(1, 2, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let ticket = h.park(drops.clone());
    let replay = RuntimeAsyncPreparedTicketV1 {
        key: Arc::clone(&ticket.key),
    };
    let rejected = foreign
        .handle
        .try_discard_prepared_v1(ticket)
        .err()
        .unwrap();
    assert_eq!(
        rejected.error,
        RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket
    );
    let foreign_ticket = foreign.park(Arc::new(AtomicUsize::new(0)));
    let future = h.handle.try_discard_prepared_v1(rejected.ticket).unwrap();
    h.command();
    assert_eq!(ready(future), Ok(()));
    let next = h.park(drops.clone());
    let invalid = h.handle.try_discard_prepared_v1(replay).unwrap();
    h.command();
    assert_eq!(
        ready(invalid),
        Err(RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket)
    );
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(h.registry.len(), 1);
    drop((next, foreign_ticket));
}

#[test]
fn preparation_discard_budget_and_queue_rejection_return_same_ticket() {
    let mut h = Harness::new(1, 1, true);
    let ticket = h.park(Arc::new(AtomicUsize::new(0)));
    let key = Arc::clone(&ticket.key);
    let (held, held_future) =
        owned::Reply::<()>::budgeted_pair(&h.handle.observer.reply_budget).unwrap();
    let rejected = h.handle.try_discard_prepared_v1(ticket).err().unwrap();
    assert_eq!(rejected.error, RuntimeAsyncEngineCallErrorV1::ReplyCapacity);
    assert!(Arc::ptr_eq(&key, &rejected.ticket.key));
    drop((held, held_future));
    for _ in 0..4 {
        h.handle
            .observer
            .sender
            .try_send(RuntimeAsyncEngineCommandV1::Stop)
            .unwrap_or_else(|_| panic!("channel capacity"));
    }
    let rejected = h
        .handle
        .try_discard_prepared_v1(rejected.ticket)
        .err()
        .unwrap();
    assert_eq!(
        rejected.error,
        RuntimeAsyncEngineCallErrorV1::CommandQueueFull
    );
    assert!(Arc::ptr_eq(&key, &rejected.ticket.key));
    for _ in 0..4 {
        drop(h.receiver.try_recv().unwrap());
    }
    let future = h.handle.try_discard_prepared_v1(rejected.ticket).unwrap();
    h.command();
    assert_eq!(ready(future), Ok(()));
}

#[test]
fn preparation_error_and_callback_panic_never_create_a_ready_ticket() {
    for panics in [false, true] {
        let mut h = Harness::new(1, 2, true);
        let future = h
            .handle
            .enqueue_preparation_v1::<Rc<()>, _>(Box::new(move |_| {
                assert!(!panics, "preparation callback panic");
                Err("rejected preparation")
            }))
            .unwrap();
        h.command();
        h.advance();
        let result = ready(future);
        if panics {
            assert!(matches!(
                result,
                Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
            ));
            assert!(h.context.is_terminal());
            assert_eq!(h.registry.len(), 1);
        } else {
            assert!(matches!(result, Ok(Err("rejected preparation"))));
            assert_eq!(h.registry.len(), 0);
        }
        assert!(h.state.lock().unwrap().issues.is_empty());
        assert!(h.state.lock().unwrap().flush_calls.is_empty());
    }
}

#[test]
fn preparation_parked_only_owned_drain_completes_and_disposes_on_owner() {
    let drops = Arc::new(AtomicUsize::new(0));
    let state = Arc::new(Mutex::new(MockState::default()));
    let (engine, handle) = start(state.clone(), Arc::new(Mutex::new(OwnerTrace::default())));
    let ticket =
        join(prepare(&handle, Arc::new(AtomicUsize::new(0)), drops.clone(), false).unwrap())
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
}

#[test]
fn preparation_discard_and_shutdown_drop_panics_retain_later_owners() {
    for explicit in [false, true] {
        let first = Arc::new(AtomicUsize::new(0));
        let later = Arc::new(AtomicUsize::new(0));
        let (engine, handle) = start(
            Arc::new(Mutex::new(MockState::default())),
            Arc::new(Mutex::new(OwnerTrace::default())),
        );
        let ticket =
            join(prepare(&handle, Arc::new(AtomicUsize::new(0)), first.clone(), true).unwrap())
                .unwrap()
                .unwrap();
        let retained =
            join(prepare(&handle, Arc::new(AtomicUsize::new(0)), later.clone(), false).unwrap())
                .unwrap()
                .unwrap();
        if explicit {
            assert_eq!(
                join_command(handle.try_discard_prepared_v1(ticket).unwrap()),
                Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
            );
        } else {
            drop(ticket);
        }
        let report = engine.shutdown().unwrap();
        assert_eq!(
            report.disposition,
            RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
        );
        assert_eq!(first.load(Ordering::SeqCst), 1);
        assert_eq!(later.load(Ordering::SeqCst), 0);
        drop(retained);
    }
}

#[test]
fn preparation_reply_and_command_exhaustion_never_enter_callback() {
    let h = Harness::new(1, 1, true);
    let calls = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let (reply, future) =
        owned::Reply::<()>::budgeted_pair(&h.handle.observer.reply_budget).unwrap();
    assert!(matches!(
        prepare(&h.handle, calls.clone(), drops.clone(), false),
        Err(RuntimeAsyncEngineCallErrorV1::ReplyCapacity)
    ));
    drop((reply, future));
    for _ in 0..4 {
        h.handle
            .observer
            .sender
            .try_send(RuntimeAsyncEngineCommandV1::Stop)
            .unwrap_or_else(|_| panic!("channel capacity"));
    }
    assert!(matches!(
        prepare(&h.handle, calls.clone(), drops, false),
        Err(RuntimeAsyncEngineCallErrorV1::CommandQueueFull)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
}

#[test]
fn preparation_discard_cutoff_and_queued_stop_have_distinct_ticket_outcomes() {
    let mut h = Harness::new(1, 2, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let ticket = h.park(drops.clone());
    let key = Arc::clone(&ticket.key);
    let future = h.handle.try_discard_prepared_v1(ticket).unwrap();
    h.handle.observer.admission.close();
    // This models an accepted discard left behind an owner Stop: command
    // disposal resolves the reply but cannot access the parked payload.
    drop(h.receiver.try_recv().unwrap());
    assert_eq!(
        ready(future),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    );
    assert_eq!((h.registry.len(), drops.load(Ordering::SeqCst)), (1, 0));
    let ticket = RuntimeAsyncPreparedTicketV1 {
        key: Arc::clone(&key),
    };
    let rejected = h.handle.try_discard_prepared_v1(ticket).err().unwrap();
    assert_eq!(rejected.error, RuntimeAsyncEngineCallErrorV1::EngineStopped);
    assert!(Arc::ptr_eq(&rejected.ticket.key, &key));
    assert_eq!(h.registry.len(), 1);
}

#[test]
fn preparation_park_and_discard_do_not_remove_other_active_streams() {
    struct Pending(RuntimeStreamIdV1, Arc<AtomicUsize>);
    impl operation::EngineOperationV1<MockBackend> for Pending {
        fn advance(&mut self, _: &mut RuntimeContextV1<MockBackend>) -> bool {
            self.1.fetch_add(1, Ordering::SeqCst);
            false
        }
        fn stream(&self) -> Option<RuntimeStreamIdV1> {
            Some(self.0)
        }
        fn reject(&mut self, _: RuntimeAsyncEngineCallErrorV1) {}
    }
    let mut h = Harness::new(2, 2, true);
    let stream = h
        .context
        .create_stream(h.context.devices()[0].id())
        .unwrap();
    let advances = Arc::new(AtomicUsize::new(0));
    h.registry
        .insert(Box::new(Pending(stream, advances.clone())));
    let ticket = h.park(Arc::new(AtomicUsize::new(0)));
    for _ in 0..3 {
        h.advance();
    }
    assert_eq!((h.registry.len(), h.registry.active_len()), (2, 1));
    assert_eq!(advances.load(Ordering::SeqCst), 4);
    assert_eq!(h.state.lock().unwrap().flush_calls.len(), 4);
    let future = h.handle.try_discard_prepared_v1(ticket).unwrap();
    h.command();
    assert_eq!(ready(future), Ok(()));
    h.advance();
    assert_eq!(h.state.lock().unwrap().flush_calls.len(), 5);
    assert_eq!((h.registry.len(), h.registry.active_len()), (1, 1));
}

#[test]
fn preparation_parked_owner_does_not_block_graph_admission_guard() {
    struct Probe(Arc<AtomicBool>);
    impl graph::EngineGraphV1<MockBackend> for Probe {
        fn admit(&mut self, _: &mut RuntimeContextV1<MockBackend>) -> bool {
            self.0.store(true, Ordering::SeqCst);
            false
        }
        fn advance(&mut self, _: &mut RuntimeContextV1<MockBackend>, _: usize, _: usize) -> bool {
            panic!("probe never installs")
        }
        fn reject(&mut self, _: RuntimeGraphErrorV1<MockError>) {
            panic!("inert park must not block graph admission")
        }
        fn stop(&mut self, _: &mut RuntimeContextV1<MockBackend>) {
            panic!("probe never installs")
        }
    }
    let mut h = Harness::new(1, 2, true);
    let ticket = h.park(Arc::new(AtomicUsize::new(0)));
    let admitted = Arc::new(AtomicBool::new(false));
    h.handle
        .observer
        .sender
        .try_send(RuntimeAsyncEngineCommandV1::Graph(Box::new(Probe(
            admitted.clone(),
        ))))
        .unwrap_or_else(|_| panic!("channel capacity"));
    assert!(!h.command());
    assert!(admitted.load(Ordering::SeqCst));
    assert_eq!((h.registry.len(), h.registry.active_len()), (1, 0));
    drop(ticket);
}

#[test]
fn preparation_parked_custody_survives_all_owned_shutdown_failures() {
    for mode in 0..4 {
        let trace = Arc::new(Mutex::new(OwnerTrace {
            cleanup_fails: mode == 0,
            cleanup_panics: mode == 1,
            finalizer_fails: mode == 2,
            finalizer_panics: mode == 3,
            ..OwnerTrace::default()
        }));
        let drops = Arc::new(AtomicUsize::new(0));
        let (engine, handle) = start(Arc::new(Mutex::new(MockState::default())), trace);
        handle
            .observer()
            .try_with_context(|context| context.create_stream(context.devices()[0].id()).unwrap())
            .unwrap();
        let ticket =
            join(prepare(&handle, Arc::new(AtomicUsize::new(0)), drops.clone(), false).unwrap())
                .unwrap()
                .unwrap();
        let report = engine.shutdown().unwrap();
        assert_eq!(
            report.disposition,
            RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
        );
        assert_eq!(report.worker_panicked, mode == 1 || mode == 3);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert_eq!(handle.observer.reply_cells_in_use(), 0);
        drop(ticket);
    }
}

#[test]
fn preparation_public_gfx942_bridge_rejects_synthetic_owner_without_callback() {
    use crate::KfdRuntimeBackendV1;
    fn assert_send<T: Send>() {}
    assert_send::<RuntimeAsyncPreparedTicketV1>();
    let (engine, handle) = RuntimeAsyncOwnedEngineV1::spawn_with_progress(
        || RuntimeContextV1::open(KfdRuntimeBackendV1::mock()),
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap();
    let device = handle
        .observer()
        .try_with_context(|context| context.devices()[0].id())
        .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let result = join(
        handle
            .try_prepare_gfx942_v1(device, move |_| {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok::<_, ()>(Rc::new(()))
            })
            .unwrap(),
    );
    assert!(matches!(
        result,
        Ok(Err(crate::RuntimeGfx942PreparationErrorV1::Context(_)))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
}
