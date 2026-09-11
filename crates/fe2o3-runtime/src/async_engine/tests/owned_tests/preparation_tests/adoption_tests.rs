use super::*;
use crate::{RuntimeAccessV1, RuntimeMemoryRegionV1};
use crate::{RuntimeGfx942GeneratedReservationErrorV1, RuntimeValidationErrorV1};
use generated_operation::adoption::{ActivationErrorV1, AdoptionHooksV1};

struct Payload {
    _local: LocalPayload,
    source: Vec<u8>,
    readback: Vec<u8>,
    pointers: (usize, usize),
    state: Arc<Mutex<MockState>>,
    mode: u8,
}

impl Drop for Payload {
    fn drop(&mut self) {
        self.state
            .lock()
            .unwrap()
            .adoption_order
            .push("payload_drop");
    }
}

trait RetireBackend: RuntimeBackendV1<Error = MockError> {
    fn retire_adoption(&mut self) -> Result<(), RuntimeErrorV1<MockError>>;
}

fn retire(state: &Mutex<MockState>) -> Result<(), RuntimeErrorV1<MockError>> {
    let mode = {
        let mut state = state.lock().unwrap();
        state.adoption_retire_calls += 1;
        state.adoption_order.push("retire");
        state.adoption_retire_mode
    };
    match mode {
        1 => Err(RuntimeValidationErrorV1::Unsupported.into()),
        2 => panic!("retirement panic after simulated disposal"),
        _ => Ok(()),
    }
}

impl RetireBackend for MockBackend {
    fn retire_adoption(&mut self) -> Result<(), RuntimeErrorV1<MockError>> {
        retire(&self.state)
    }
}
impl RetireBackend for ThreadBoundBackend {
    fn retire_adoption(&mut self) -> Result<(), RuntimeErrorV1<MockError>> {
        self.record("adoption_retire");
        retire(&self.inner.state)
    }
}

fn reserve<B: RuntimeBackendV1>(
    _: &mut RuntimeContextV1<B>,
    payload: &mut Payload,
) -> Result<crate::generated_source::GeneratedHostRosterV1, RuntimeGfx942GeneratedReservationErrorV1>
{
    payload.readback = vec![0; payload.source.len()];
    payload.pointers = (
        payload.source.as_ptr() as usize,
        payload.readback.as_ptr() as usize,
    );
    Ok(reservation_tests::roster())
}

fn hooks<B: RetireBackend>() -> AdoptionHooksV1<B, Payload> {
    AdoptionHooksV1 {
        preflight: |context, payload, _, _| {
            payload
                .state
                .lock()
                .unwrap()
                .adoption_order
                .push("preflight");
            match payload.mode {
                1 => Err(RuntimeValidationErrorV1::Unsupported.into()),
                2 => panic!("adoption preflight panic"),
                5 => {
                    context.quarantine_after_async_command_panic_v1();
                    Ok(())
                }
                _ => Ok(()),
            }
        },
        adopt: |context, payload, roster, hold| {
            payload.state.lock().unwrap().adoption_order.push("adopt");
            assert_eq!(roster.count, 1);
            assert_eq!(
                payload.pointers,
                (
                    payload.source.as_ptr() as usize,
                    payload.readback.as_ptr() as usize
                )
            );
            assert_eq!(payload.source, vec![7; 16]);
            assert!(matches!(
                context.destroy_stream(hold.stream()),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::ContextReserved
                ))
            ));
            assert!(matches!(
                context.reserve_graph_v1(1),
                Err(RuntimeValidationErrorV1::ContextReserved)
            ));
            assert!(!context.cleanup().is_complete());
            match payload.mode {
                3 => Err(RuntimeValidationErrorV1::Unsupported.into()),
                4 => panic!("adoption panic after simulated acquisition"),
                _ => Ok(()),
            }
        },
        // Also valid for the empty prefix when Stop preceded first advancement.
        retire: |context, _hold| context.backend_mut_for_test_v1().retire_adoption(),
    }
}

fn preparation<B: RetireBackend + 'static>(
    handle: &RuntimeAsyncProgressHandleV1<B>,
    state: Arc<Mutex<MockState>>,
    drops: Arc<AtomicUsize>,
    mode: u8,
    enabled: bool,
) -> RuntimeAsyncPreparationV1<()> {
    handle
        .enqueue_preparation_with_adoption_v1(
            Box::new(move |_| {
                Ok(Payload {
                    _local: LocalPayload {
                        local: Rc::new(Cell::new(0)),
                        drops,
                        owner: thread::current().id(),
                        panic_on_drop: false,
                    },
                    source: vec![7; 16],
                    readback: Vec::new(),
                    pointers: (0, 0),
                    state,
                    mode,
                })
            }),
            Some(reserve),
            enabled.then(hooks),
        )
        .unwrap()
}

fn reserved(
    h: &mut Harness,
    drops: Arc<AtomicUsize>,
    mode: u8,
    enabled: bool,
) -> (RuntimeAsyncReservedTicketV1, RuntimeStreamIdV1) {
    let stream = h
        .context
        .create_stream(h.context.devices()[0].id())
        .unwrap();
    let future = preparation(&h.handle, h.state.clone(), drops, mode, enabled);
    h.command();
    h.advance();
    let ticket = ready(future).unwrap().unwrap();
    let future = h.handle.try_reserve_prepared_v1(ticket).unwrap();
    h.command();
    (ready(future).unwrap().unwrap(), stream)
}

fn activate(h: &mut Harness, ticket: RuntimeAsyncReservedTicketV1, stream: RuntimeStreamIdV1) {
    let future = h.handle.try_activate_reserved_v1(ticket, stream).unwrap();
    assert!(!h.command());
    ready(future).unwrap().unwrap();
}

#[test]
fn adoption_preserves_owner_capacity_reply_and_never_flushes_or_discards() {
    let mut h = Harness::new(1, 2, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let (ticket, stream) = reserved(&mut h, drops.clone(), 0, true);
    let key = ticket.key.clone();
    assert_eq!(h.handle.observer.reply_cells_in_use(), 1);
    activate(&mut h, ticket, stream);
    assert_eq!((h.registry.len(), h.registry.active_len()), (1, 1));
    assert_eq!(h.handle.observer.reply_cells_in_use(), 1);
    assert!(!h.registry.discard_reserved(&key));
    assert!(!h.registry.discard_prepared(&key));
    for _ in 0..4 {
        h.advance();
    }
    assert!(h.state.lock().unwrap().issues.is_empty());
    assert!(h.state.lock().unwrap().flush_calls.is_empty());
    assert_eq!(
        h.state.lock().unwrap().adoption_order,
        ["preflight", "adopt"]
    );
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    h.registry.retire_unpublished_v1(&mut h.context, 1);
    assert_eq!(h.registry.len(), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
    assert_eq!(
        h.state.lock().unwrap().adoption_order,
        ["preflight", "adopt", "retire", "payload_drop"]
    );
    h.context.destroy_stream(stream).unwrap();
}

#[test]
fn adoption_unsupported_and_preflight_rejection_recover_exact_ticket_without_hold() {
    for (enabled, mode) in [(false, 0), (true, 1)] {
        let mut h = Harness::new(1, 2, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let (ticket, stream) = reserved(&mut h, drops.clone(), mode, enabled);
        let key = ticket.key.clone();
        let future = h.handle.try_activate_reserved_v1(ticket, stream).unwrap();
        assert!(!h.command());
        let failure = ready(future).unwrap().unwrap_err();
        assert!(Arc::ptr_eq(&key, &failure.ticket.key));
        assert!(matches!(
            failure.error,
            ActivationErrorV1::Context(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::Unsupported
            ))
        ));
        assert_eq!((h.registry.len(), h.registry.active_len()), (1, 0));
        assert_eq!(h.handle.observer.reply_cells_in_use(), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        h.context.destroy_stream(stream).unwrap();
        assert!(h.registry.discard_reserved(&key));
    }
}

#[test]
fn adoption_panic_terminal_preflight_and_post_effect_errors_never_retry_or_drop() {
    for mode in [2, 3, 4, 5] {
        let mut h = Harness::new(1, 2, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let (ticket, stream) = reserved(&mut h, drops.clone(), mode, true);
        let key = ticket.key.clone();
        let future = h.handle.try_activate_reserved_v1(ticket, stream).unwrap();
        let stopped = h.command();
        if mode == 2 || mode == 5 {
            assert!(stopped);
            assert!(ready(future).is_err());
        } else {
            assert!(!stopped);
            ready(future).unwrap().unwrap();
            h.advance();
            assert!(!h.registry.discard_reserved(&key));
        }
        assert!(h.context.is_terminal());
        let before = h.state.lock().unwrap().adoption_order.clone();
        h.registry.retire_unpublished_v1(&mut h.context, 4);
        assert_eq!(h.state.lock().unwrap().adoption_order, before);
        assert_eq!(h.registry.len(), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert!(!h.context.cleanup().is_complete());
        // This is a fake backend. Assert production retention before disposing
        // the test-only fixture; owned-engine tests exercise real quarantine.
    }
}

#[test]
fn adoption_retirement_error_or_panic_preserves_owner_and_hold_without_retry() {
    for mode in [1, 2] {
        let mut h = Harness::new(1, 2, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let (ticket, stream) = reserved(&mut h, drops.clone(), 0, true);
        activate(&mut h, ticket, stream);
        h.advance();
        h.state.lock().unwrap().adoption_retire_mode = mode;
        h.registry.retire_unpublished_v1(&mut h.context, 1);
        assert!(h.context.is_terminal());
        h.registry.retire_unpublished_v1(&mut h.context, 1);
        assert_eq!(h.state.lock().unwrap().adoption_retire_calls, 1);
        assert_eq!(h.registry.len(), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert_eq!(h.context.cleanup().retained().streams, 1);
    }
}

#[test]
fn adoption_stop_before_first_advance_retires_empty_prefix_without_adopting() {
    let mut h = Harness::new(1, 2, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let (ticket, stream) = reserved(&mut h, drops.clone(), 0, true);
    activate(&mut h, ticket, stream);
    assert!(!h.registry.stop_observations());
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    h.registry.retire_unpublished_v1(&mut h.context, 1);
    assert_eq!(
        h.state.lock().unwrap().adoption_order,
        ["preflight", "retire", "payload_drop"]
    );
    h.context.destroy_stream(stream).unwrap();
}

#[test]
fn adoption_dropped_acknowledgement_does_not_release_active_custody() {
    let mut h = Harness::new(1, 2, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let (ticket, stream) = reserved(&mut h, drops.clone(), 0, true);
    drop(h.handle.try_activate_reserved_v1(ticket, stream).unwrap());
    h.command();
    h.advance();
    assert_eq!(h.handle.observer.reply_cells_in_use(), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    h.registry.retire_unpublished_v1(&mut h.context, 1);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn adoption_queued_stop_and_admission_failures_return_original_reserved_ticket() {
    for mode in 0..5 {
        let mut h = Harness::new(1, 2, true);
        let (ticket, stream) = reserved(&mut h, Arc::new(AtomicUsize::new(0)), 0, true);
        let key = ticket.key.clone();
        let failure = if mode == 0 {
            let future = h.handle.try_activate_reserved_v1(ticket, stream).unwrap();
            drop(h.receiver.try_recv().unwrap());
            ready(future).unwrap().unwrap_err()
        } else {
            match mode {
                1 => h.handle.observer.admission.close(),
                2 => {
                    h.handle
                        .observer
                        .worker_thread
                        .set(thread::current().id())
                        .unwrap();
                }
                3 => {
                    for _ in 0..4 {
                        h.handle
                            .observer
                            .sender
                            .try_send(RuntimeAsyncEngineCommandV1::Stop)
                            .unwrap_or_else(|_| panic!("queue"));
                    }
                }
                _ => {
                    drop(h.receiver);
                }
            }
            h.handle
                .try_activate_reserved_v1(ticket, stream)
                .err()
                .expect("activation admission rejected")
        };
        assert!(Arc::ptr_eq(&key, &failure.ticket.key));
        assert!(matches!(failure.error, ActivationErrorV1::Engine(_)));
        assert_eq!((h.registry.len(), h.registry.active_len()), (1, 0));
        assert!(h.state.lock().unwrap().adoption_order.is_empty());
    }
}

#[test]
fn adoption_owned_drain_and_shutdown_retire_before_carrier_disposal() {
    for drain in [false, true] {
        let state = Arc::new(Mutex::new(MockState::default()));
        let trace = Arc::new(Mutex::new(OwnerTrace::default()));
        let drops = Arc::new(AtomicUsize::new(0));
        let (engine, handle) = start(state.clone(), trace.clone());
        let stream = handle
            .observer
            .try_with_context(|c| c.create_stream(c.devices()[0].id()).unwrap())
            .unwrap();
        let ticket = join(preparation(&handle, state.clone(), drops.clone(), 0, true))
            .unwrap()
            .unwrap();
        let ticket = join(handle.try_reserve_prepared_v1(ticket).unwrap())
            .unwrap()
            .unwrap();
        join(handle.try_activate_reserved_v1(ticket, stream).unwrap())
            .unwrap()
            .unwrap();
        if drain {
            let report = join(handle.begin_drain(16).unwrap()).unwrap();
            assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
            assert_eq!(report.operations_remaining, 0);
        }
        assert_eq!(
            engine.shutdown().unwrap().disposition,
            RuntimeAsyncOwnedDispositionV1::Released
        );
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        let order = &state.lock().unwrap().adoption_order;
        assert!(
            order.iter().position(|v| *v == "retire").unwrap()
                < order.iter().position(|v| *v == "payload_drop").unwrap()
        );
        let trace = trace.lock().unwrap();
        assert!(
            trace
                .calls
                .iter()
                .position(|v| v.0 == "adoption_retire")
                .unwrap()
                < trace
                    .calls
                    .iter()
                    .position(|v| v.0 == "destroy_stream_v1")
                    .unwrap()
        );
    }
}

#[test]
fn adoption_owned_failed_retirement_quarantines_carrier_and_context() {
    for mode in [1, 2] {
        let state = Arc::new(Mutex::new(MockState {
            adoption_retire_mode: mode,
            ..MockState::default()
        }));
        let drops = Arc::new(AtomicUsize::new(0));
        let (engine, handle) = start(state.clone(), Arc::new(Mutex::new(OwnerTrace::default())));
        let stream = handle
            .observer
            .try_with_context(|c| c.create_stream(c.devices()[0].id()).unwrap())
            .unwrap();
        let ticket = join(preparation(&handle, state.clone(), drops.clone(), 0, true))
            .unwrap()
            .unwrap();
        let ticket = join(handle.try_reserve_prepared_v1(ticket).unwrap())
            .unwrap()
            .unwrap();
        join(handle.try_activate_reserved_v1(ticket, stream).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(
            engine.shutdown().unwrap().disposition,
            RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
        );
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert_eq!(state.lock().unwrap().adoption_retire_calls, 1);
    }
}

struct PendingCounter(Arc<AtomicUsize>);
impl operation::EngineOperationV1<MockBackend> for PendingCounter {
    fn advance(&mut self, _: &mut RuntimeContextV1<MockBackend>) -> bool {
        self.0.fetch_add(1, Ordering::SeqCst);
        false
    }
    fn stream(&self) -> Option<RuntimeStreamIdV1> {
        None
    }
    fn reject(&mut self, _: RuntimeAsyncEngineCallErrorV1) {}
}

#[test]
fn adoption_and_ordinary_progress_do_not_starve_with_unit_drain_budget() {
    for ordinary_first in [false, true] {
        let mut h = Harness::new(2, 2, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let polls = Arc::new(AtomicUsize::new(0));
        let (ticket, stream) = reserved(&mut h, drops.clone(), 0, true);
        if ordinary_first {
            h.registry.insert(Box::new(PendingCounter(polls.clone())));
        }
        activate(&mut h, ticket, stream);
        if !ordinary_first {
            h.registry.insert(Box::new(PendingCounter(polls.clone())));
        }
        for _ in 0..4 {
            operation::advance_operations_v1(
                &mut h.context,
                &mut h.registry,
                1,
                1,
                flush_stream_v1::<MockBackend>,
            );
            h.registry.retire_unpublished_v1(&mut h.context, 1);
        }
        assert!(polls.load(Ordering::SeqCst) > 0);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(h.state.lock().unwrap().adoption_retire_calls, 1);
        assert_eq!(h.registry.len(), 1);
        assert!(!h.context.is_terminal());
    }
}

#[test]
fn adoption_hold_rejects_work_on_only_its_stream_and_blocks_graph_and_cleanup() {
    let mut h = Harness::new(1, 2, true);
    let device = h.context.devices()[0].id();
    let stream = h.context.create_stream(device).unwrap();
    let other = h.context.create_stream(device).unwrap();
    let module = h.context.load_module(device, &[1]).unwrap();
    let kernel = h
        .context
        .resolve_kernel::<EmptyArgs>(module, "kernel")
        .unwrap();
    let first = h
        .context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 16, 4)
        .unwrap();
    let second = h
        .context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 16, 4)
        .unwrap();
    let source = RuntimeMemoryRegionV1 {
        allocation: first,
        access: RuntimeAccessV1::Read,
        byte_offset: 0,
        byte_len: 16,
    };
    let destination = RuntimeMemoryRegionV1 {
        allocation: second,
        access: RuntimeAccessV1::Write,
        byte_offset: 0,
        byte_len: 16,
    };
    let hold = h.context.hold_unpublished_stream_v1(stream).unwrap();
    for result in [
        h.context.destroy_stream(stream),
        h.context.flush_stream(stream),
        h.context
            .launch(
                stream,
                &kernel,
                &EmptyArgs,
                RuntimeLaunchGeometryV1 {
                    grid: [1, 1, 1],
                    workgroup: [1, 1, 1],
                    dynamic_shared_bytes: 0,
                },
                &[],
            )
            .map(drop),
        h.context
            .copy_async(stream, source, destination, &[])
            .map(drop),
        h.context
            .peer_copy(stream, source, destination, &[])
            .map(drop),
    ] {
        assert!(matches!(
            result,
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ContextReserved
            ))
        ));
    }
    assert!(matches!(
        h.context.reserve_graph_v1(1),
        Err(RuntimeValidationErrorV1::ContextReserved)
    ));
    assert!(!h.context.cleanup().is_complete());
    assert!(h.state.lock().unwrap().issues.is_empty());
    assert!(h.state.lock().unwrap().copy_issues.is_empty());
    assert!(h.state.lock().unwrap().flush_calls.is_empty());
    h.context.flush_stream(other).unwrap();
    h.context.destroy_stream(other).unwrap();
    h.context.release_unpublished_hold_v1(&hold).unwrap();
    let next = h.context.hold_unpublished_stream_v1(stream).unwrap();
    assert!(h.context.release_unpublished_hold_v1(&hold).is_err());
    assert!(h.context.destroy_stream(stream).is_err());
    h.context.release_unpublished_hold_v1(&next).unwrap();
    h.context.destroy_stream(stream).unwrap();
}

#[test]
fn adoption_foreign_ticket_unknown_stream_and_occupied_hold_preserve_parked_owner() {
    let mut h = Harness::new(1, 2, true);
    let (ticket, stream) = reserved(&mut h, Arc::new(AtomicUsize::new(0)), 0, true);
    let other = Harness::new(1, 2, true);
    let key = ticket.key.clone();
    let failure = other
        .handle
        .try_activate_reserved_v1(ticket, stream)
        .err()
        .unwrap();
    assert!(Arc::ptr_eq(&key, &failure.ticket.key));
    let hold = h.context.hold_unpublished_stream_v1(stream).unwrap();
    let future = h
        .handle
        .try_activate_reserved_v1(failure.ticket, stream)
        .unwrap();
    h.command();
    let failure = ready(future).unwrap().unwrap_err();
    assert!(matches!(
        failure.error,
        ActivationErrorV1::Context(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    assert!(Arc::ptr_eq(&key, &failure.ticket.key));
    h.context.release_unpublished_hold_v1(&hold).unwrap();
    h.context.destroy_stream(stream).unwrap();
    let future = h
        .handle
        .try_activate_reserved_v1(failure.ticket, stream)
        .unwrap();
    h.command();
    let failure = ready(future).unwrap().unwrap_err();
    assert!(matches!(
        failure.error,
        ActivationErrorV1::Context(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownStream
        ))
    ));
    assert!(Arc::ptr_eq(&key, &failure.ticket.key));
    assert_eq!((h.registry.len(), h.registry.active_len()), (1, 0));
    assert!(!h.state.lock().unwrap().adoption_order.contains(&"adopt"));
}

#[test]
fn adoption_replayed_key_cannot_consume_another_reserved_owner() {
    let mut h = Harness::new(2, 3, true);
    let (first, first_stream) = reserved(&mut h, Arc::new(AtomicUsize::new(0)), 0, true);
    let first_key = first.key.clone();
    activate(&mut h, first, first_stream);
    let (mut second, stream) = reserved(&mut h, Arc::new(AtomicUsize::new(0)), 0, true);
    let second_key = second.key.clone();
    second.key = first_key.clone(); // Private malformed-ticket fixture, not a public constructor.
    let future = h.handle.try_activate_reserved_v1(second, stream).unwrap();
    h.command();
    let mut failure = ready(future).unwrap().unwrap_err();
    assert!(matches!(
        failure.error,
        ActivationErrorV1::Engine(RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket)
    ));
    assert!(Arc::ptr_eq(&first_key, &failure.ticket.key));
    assert_eq!((h.registry.len(), h.registry.active_len()), (2, 1));
    failure.ticket.key = second_key;
    activate(&mut h, failure.ticket, stream);
    assert_eq!((h.registry.len(), h.registry.active_len()), (2, 2));
    h.registry.retire_unpublished_v1(&mut h.context, 2);
    assert_eq!(h.registry.len(), 0);
}

#[test]
fn adoption_graph_reservation_and_ack_capacity_leave_ticket_retryable() {
    for graph in [false, true] {
        let mut h = Harness::new(1, 2, true);
        let (ticket, stream) = reserved(&mut h, Arc::new(AtomicUsize::new(0)), 0, true);
        let key = ticket.key.clone();
        let failure = if graph {
            let token = h.context.reserve_graph_v1(1).unwrap();
            let future = h.handle.try_activate_reserved_v1(ticket, stream).unwrap();
            h.command();
            let failure = ready(future).unwrap().unwrap_err();
            assert!(matches!(
                failure.error,
                ActivationErrorV1::Context(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::ContextReserved
                ))
            ));
            h.context.close_graph_issue_v1(token).unwrap();
            h.context.release_graph_v1(token).unwrap();
            failure
        } else {
            let held = owned::Reply::<()>::budgeted_pair(&h.handle.observer.reply_budget).unwrap();
            let failure = h
                .handle
                .try_activate_reserved_v1(ticket, stream)
                .err()
                .unwrap();
            assert!(matches!(
                failure.error,
                ActivationErrorV1::Engine(RuntimeAsyncEngineCallErrorV1::ReplyCapacity)
            ));
            drop(held);
            failure
        };
        assert!(Arc::ptr_eq(&key, &failure.ticket.key));
        assert_eq!((h.registry.len(), h.registry.active_len()), (1, 0));
        activate(&mut h, failure.ticket, stream);
        h.registry.retire_unpublished_v1(&mut h.context, 1);
        assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
    }
}

#[test]
fn adoption_pending_submission_rejects_hold_until_exact_quiescence() {
    let mut h = Harness::new(1, 2, true);
    let (ticket, stream) = reserved(&mut h, Arc::new(AtomicUsize::new(0)), 0, true);
    let key = ticket.key.clone();
    let (event, native) = append_submission_on_stream(&mut h.context, &h.state, stream, 1, "old");
    let future = h.handle.try_activate_reserved_v1(ticket, stream).unwrap();
    h.command();
    let failure = ready(future).unwrap().unwrap_err();
    assert!(Arc::ptr_eq(&key, &failure.ticket.key));
    assert!(matches!(
        failure.error,
        ActivationErrorV1::Context(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    assert_eq!(h.registry.active_len(), 0);
    assert!(!h.state.lock().unwrap().adoption_order.contains(&"adopt"));
    h.context.flush_stream(stream).unwrap();
    h.state
        .lock()
        .unwrap()
        .statuses
        .insert(native, BackendPollV1::Succeeded);
    assert_eq!(
        h.context.poll_event(event).unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    activate(&mut h, failure.ticket, stream);
    h.advance();
    h.registry.retire_unpublished_v1(&mut h.context, 1);
    assert_eq!(h.registry.len(), 0);
}
