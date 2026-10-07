use super::*;
use generated_operation::adoption::CompletionHooksV1;
mod current_thread_tests;
mod graph_tests;

type Observation =
    Result<Result<(), crate::RuntimeGfx942ReadbackErrorV1>, RuntimeAsyncEngineCallErrorV1>;
type WakeRecord = (Observation, Vec<&'static str>, usize);

struct ResultWake {
    probe: Box<dyn Fn() -> Option<Observation> + Send + Sync>,
    state: Arc<Mutex<MockState>>,
    drops: Arc<AtomicUsize>,
    observations: Arc<Mutex<Vec<WakeRecord>>>,
    panic: bool,
}

impl std::task::Wake for ResultWake {
    fn wake(self: Arc<Self>) {
        self.observations.lock().unwrap().push((
            (self.probe)().expect("original reply installed before wake"),
            self.state.lock().unwrap().adoption_order.clone(),
            self.drops.load(Ordering::SeqCst),
        ));
        assert!(!self.panic, "completion observer wake panic");
    }
}

fn completion_hooks<B: RetireBackend>() -> AdoptionHooksV1<B, Payload> {
    let mut hooks = issue_tests::issue_hooks();
    hooks.issue.as_mut().unwrap().completion = Some(CompletionHooksV1 {
        domain: |payload| {
            Ok(RuntimeGeneratedResultDomainV1::from_owner(
                payload.result_domain.clone(),
            ))
        },
        settle: |context: &mut RuntimeContextV1<B>, payload: &mut Payload, _, hold| {
            assert_eq!(payload.stream, Some(hold.stream()));
            assert_eq!(
                payload.pointers,
                (
                    payload.source.as_ptr() as usize,
                    payload.readback.as_ptr() as usize
                )
            );
            for (index, step) in [
                "copy",
                "postcheck",
                "native_retire",
                "records_retire",
                "hold_release",
            ]
            .into_iter()
            .enumerate()
            {
                payload.state.lock().unwrap().adoption_order.push(step);
                if payload.mode == 30 + index as u8 {
                    return Err(RuntimeValidationErrorV1::Unsupported.into());
                }
                assert_ne!(
                    payload.mode,
                    40 + index as u8,
                    "completion settlement panic"
                );
                match index {
                    0 => payload.readback.copy_from_slice(&payload.source),
                    2 => context
                        .backend_mut_for_test_v1()
                        .retire_adoption(hold.stream())?,
                    4 => context.release_unpublished_hold_v1(hold)?,
                    _ => {}
                }
            }
            payload
                .state
                .lock()
                .unwrap()
                .adoption_order
                .push("hold_released");
            Ok(())
        },
        decode: |payload| {
            {
                let mut state = payload.state.lock().unwrap();
                assert_eq!(state.adoption_order.last(), Some(&"hold_released"));
                assert_eq!(payload.readback, payload.source);
                state.adoption_order.push("decode");
            }
            match payload.mode {
                23 => Err(crate::RuntimeGfx942ReadbackErrorV1::InvalidStorage),
                24 => panic!("completed host decoder panic"),
                _ => {
                    payload
                        .state
                        .lock()
                        .unwrap()
                        .adoption_order
                        .push("gate_commit");
                    Ok(())
                }
            }
        },
    });
    hooks
}

fn reserve_completion(
    h: &mut Harness,
    drops: Arc<AtomicUsize>,
    mode: u8,
) -> (RuntimeAsyncReservedTicketV1, RuntimeStreamIdV1) {
    reserve_completion_with_domain(h, drops, mode, Arc::new(()))
}

fn reserve_completion_with_domain(
    h: &mut Harness,
    drops: Arc<AtomicUsize>,
    mode: u8,
    domain: Arc<()>,
) -> (RuntimeAsyncReservedTicketV1, RuntimeStreamIdV1) {
    let stream = h
        .context
        .create_stream(h.context.devices()[0].id())
        .unwrap();
    let future = preparation_with_domain(
        &h.handle,
        h.state.clone(),
        drops,
        mode,
        Some(completion_hooks()),
        domain,
    );
    h.command();
    h.advance();
    let ticket = ready(future).unwrap().unwrap();
    let future = h.handle.try_reserve_prepared_v1(ticket).unwrap();
    h.command();
    (ready(future).unwrap().unwrap(), stream)
}

fn watch(
    h: &Harness,
    future: &mut RuntimeAsyncGeneratedCompletionV1,
    drops: Arc<AtomicUsize>,
    panic: bool,
) -> Arc<Mutex<Vec<WakeRecord>>> {
    let observations = Arc::new(Mutex::new(Vec::new()));
    let waker = Waker::from(Arc::new(ResultWake {
        probe: Box::new(future.result_probe_for_test_v1()),
        state: h.state.clone(),
        drops,
        observations: observations.clone(),
        panic,
    }));
    assert!(
        Pin::new(future)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    observations
}

fn activate_observer(
    h: &mut Harness,
    ticket: RuntimeAsyncReservedTicketV1,
    stream: RuntimeStreamIdV1,
) -> RuntimeAsyncGeneratedCompletionV1 {
    let activation = h.handle.try_activate_reserved_v1(ticket, stream).unwrap();
    assert!(!h.command());
    ready(activation).unwrap().unwrap()
}

#[test]
fn generated_completion_domain_failure_precedes_reservation_and_preserves_exact_owner() {
    use generated_operation::adoption::IssueHooksV1;

    type Trace = Arc<Mutex<Vec<(&'static str, usize, usize)>>>;
    struct DomainPayload {
        _local: LocalPayload,
        source: Vec<u8>,
        domain: Arc<()>,
        mode: Arc<AtomicUsize>,
        trace: Trace,
    }
    fn observe(payload: &DomainPayload, step: &'static str) {
        assert_eq!(payload.source, [7; 16]);
        payload.trace.lock().unwrap().push((
            step,
            payload as *const DomainPayload as usize,
            payload.source.as_ptr() as usize,
        ));
    }

    for panics in [false, true] {
        let mut h = Harness::new(1, 2, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let domain = Arc::new(());
        let mode = Arc::new(AtomicUsize::new(if panics { 2 } else { 1 }));
        let trace: Trace = Arc::new(Mutex::new(Vec::new()));
        let captured = (drops.clone(), domain.clone(), mode.clone(), trace.clone());
        let preparation = h
            .handle
            .enqueue_preparation_with_adoption_v1(
                Box::new(move |_| {
                    Ok::<_, ()>(DomainPayload {
                        _local: LocalPayload {
                            local: Rc::new(Cell::new(0)),
                            drops: captured.0,
                            owner: thread::current().id(),
                            panic_on_drop: false,
                        },
                        source: vec![7; 16],
                        domain: captured.1,
                        mode: captured.2,
                        trace: captured.3,
                    })
                }),
                Some(|_, payload| {
                    observe(payload, "reserve");
                    Ok(reservation_tests::roster())
                }),
                Some(AdoptionHooksV1 {
                    preflight: |_, _, _, _, _| panic!("no adoption preflight"),
                    ready: |_, _| panic!("no adoption readiness"),
                    adopt: |_, _, _, _| panic!("no adoption"),
                    retire: |_, _| panic!("no native retirement"),
                    issue: Some(IssueHooksV1 {
                        progress: |_, _, _, _| panic!("no issue"),
                        retire_stopped: |_, _| panic!("no issued retirement"),
                        completion: Some(CompletionHooksV1 {
                            settle: |_, _, _, _| panic!("no settlement"),
                            decode: |_| panic!("no decode"),
                            domain: |payload| {
                                observe(payload, "domain");
                                match payload.mode.load(Ordering::SeqCst) {
                                    1 => Err(crate::RuntimeGfx942ReadbackErrorV1::InvalidStorage),
                                    2 => panic!("domain lookup panic"),
                                    _ => Ok(RuntimeGeneratedResultDomainV1::from_owner(
                                        payload.domain.clone(),
                                    )),
                                }
                            },
                        }),
                    }),
                }),
            )
            .unwrap();
        assert!(!h.command());
        h.advance();
        let ticket = ready(preparation).unwrap().unwrap();
        let key = ticket.key.clone();
        let reservation = h.handle.try_reserve_prepared_v1(ticket).unwrap();
        assert_eq!(h.handle.observer.reply_cells_in_use(), 2);
        assert_eq!(h.command(), panics);
        let result = ready(reservation);
        assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
        assert_eq!((h.registry.len(), h.registry.active_len()), (1, 0));
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert_eq!(Arc::strong_count(&domain), 2);
        let first = trace.lock().unwrap().clone();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].0, "domain");
        mode.store(0, Ordering::SeqCst);
        if panics {
            assert!(matches!(
                result,
                Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
            ));
            assert!(h.context.is_terminal());
            for _ in 0..3 {
                h.advance();
                h.registry.retire_unpublished_v1(&mut h.context, 1);
                assert!(!h.registry.stop_observations());
            }
            assert_eq!(*trace.lock().unwrap(), first);
            assert_eq!(h.registry.len(), 1);
            assert_eq!(drops.load(Ordering::SeqCst), 0);
            core::mem::forget(h.registry);
        } else {
            let failure = result.unwrap().unwrap_err();
            assert!(matches!(
                failure.error,
                RuntimeGfx942GeneratedReservationErrorV1::Readback(
                    crate::RuntimeGfx942ReadbackErrorV1::InvalidStorage
                )
            ));
            assert!(Arc::ptr_eq(&key, &failure.ticket.key));
            assert!(!h.context.is_terminal());
            let retry = h.handle.try_reserve_prepared_v1(failure.ticket).unwrap();
            assert!(!h.command());
            let reserved = ready(retry).unwrap().unwrap();
            assert!(Arc::ptr_eq(&key, &reserved.key));
            assert_eq!(Arc::strong_count(&domain), 3);
            assert_eq!(
                *trace.lock().unwrap(),
                [first[0], first[0], ("reserve", first[0].1, first[0].2)]
            );
            assert_eq!(h.handle.observer.reply_cells_in_use(), 1);
            let discard = h.handle.try_discard_reserved_v1(reserved).unwrap();
            assert!(!h.command());
            assert_eq!(ready(discard), Ok(()));
            assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
            assert_eq!(drops.load(Ordering::SeqCst), 1);
            assert_eq!(Arc::strong_count(&domain), 1);
        }
    }
}

#[test]
fn generated_completion_settles_then_decodes_then_resolves_original_cell_once() {
    for panic_wake in [false, true] {
        let mut h = Harness::new(1, 2, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let (ticket, stream) = reserve_completion(&mut h, drops.clone(), 0);
        let mut completion = activate_observer(&mut h, ticket, stream);
        let observations = watch(&h, &mut completion, drops.clone(), panic_wake);
        for _ in 0..3 {
            h.advance();
            assert!(observations.lock().unwrap().is_empty());
            assert_eq!(drops.load(Ordering::SeqCst), 0);
            assert!(h.context.destroy_stream(stream).is_err());
        }
        h.advance();
        assert!(!h.context.is_terminal());
        assert_eq!(h.registry.active_len(), 0);
        assert_eq!(h.handle.observer.reply_cells_in_use(), 1);
        assert!(ready(completion).unwrap().is_ok());
        assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        h.context.destroy_stream(stream).unwrap();
        let observations = observations.lock().unwrap();
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].0, Ok(Ok(())));
        assert_eq!(observations[0].2, 1);
        assert_eq!(
            observations[0].1,
            [
                "preflight",
                "adopt",
                "issue",
                "poll",
                "copy",
                "postcheck",
                "native_retire",
                "retire",
                "records_retire",
                "hold_release",
                "hold_released",
                "decode",
                "gate_commit",
                "payload_drop"
            ]
        );
    }
}

#[test]
fn generated_completion_settlement_faults_keep_prepared_owner_and_forbid_decode_retry() {
    for mode in (30..35).chain(40..45) {
        let mut h = Harness::new(1, 2, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let (ticket, stream) = reserve_completion(&mut h, drops.clone(), mode);
        let mut completion = activate_observer(&mut h, ticket, stream);
        let observations = watch(&h, &mut completion, drops.clone(), false);
        for _ in 0..4 {
            h.advance();
        }
        assert!(h.context.is_terminal(), "mode {mode}");
        let order = h.state.lock().unwrap().adoption_order.clone();
        assert!(!order.contains(&"decode"));
        assert!(!order.contains(&"gate_commit"));
        for _ in 0..3 {
            h.advance();
            h.registry.retire_unpublished_v1(&mut h.context, 1);
        }
        assert_eq!(h.state.lock().unwrap().adoption_order, order);
        assert_eq!(h.registry.active_len(), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        if mode < 40 {
            assert!(observations.lock().unwrap().is_empty());
            assert!(!h.registry.stop_observations());
            let observations = observations.lock().unwrap();
            assert_eq!(observations.len(), 1);
            assert_eq!(
                observations[0].0,
                Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
            );
        } else {
            let observations = observations.lock().unwrap();
            assert_eq!(observations.len(), 1);
            assert_eq!(
                observations[0].0,
                Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
            );
        }
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert!(ready(completion).is_err());
        core::mem::forget(h.registry);
    }
}

#[test]
fn generated_completion_decoder_error_or_panic_is_conclusive_host_failure() {
    for mode in [23, 24] {
        let mut h = Harness::new(1, 2, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let (ticket, stream) = reserve_completion(&mut h, drops.clone(), mode);
        let mut completion = activate_observer(&mut h, ticket, stream);
        let observations = watch(&h, &mut completion, drops.clone(), false);
        for _ in 0..4 {
            h.advance();
        }
        assert!(!h.context.is_terminal());
        assert_eq!(h.registry.active_len(), 0);
        assert_eq!(h.handle.observer.reply_cells_in_use(), 1);
        let result = ready(completion);
        if mode == 23 {
            assert!(matches!(
                result,
                Ok(Err(crate::RuntimeGfx942ReadbackErrorV1::InvalidStorage))
            ));
        } else {
            assert!(matches!(
                result,
                Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
            ));
        }
        assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        h.context.destroy_stream(stream).unwrap();
        let observations = observations.lock().unwrap();
        assert_eq!(observations.len(), 1);
        assert!(!observations[0].1.contains(&"gate_commit"));
        assert_eq!(observations[0].2, 1);
        if mode == 23 {
            assert_eq!(
                observations[0].0,
                Ok(Err(crate::RuntimeGfx942ReadbackErrorV1::InvalidStorage))
            );
        } else {
            assert_eq!(
                observations[0].0,
                Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
            );
        }
    }
}

#[test]
fn generated_completion_stop_preserves_disposal_without_decoding() {
    let mut h = Harness::new(1, 2, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let (ticket, stream) = reserve_completion(&mut h, drops.clone(), 0);
    let mut completion = activate_observer(&mut h, ticket, stream);
    let observations = watch(&h, &mut completion, drops.clone(), false);
    for _ in 0..3 {
        h.advance();
    }
    assert!(!h.registry.stop_observations());
    h.advance();
    h.registry.retire_unpublished_v1(&mut h.context, 1);
    assert!(!h.context.is_terminal());
    assert_eq!(h.registry.active_len(), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    h.context.destroy_stream(stream).unwrap();
    let observations = observations.lock().unwrap();
    assert_eq!(observations.len(), 1);
    assert_eq!(
        observations[0].0,
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    );
    assert!(!h.state.lock().unwrap().adoption_order.contains(&"decode"));
    assert!(matches!(
        ready(completion),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
}

#[test]
fn generated_completion_does_not_require_observer_polling_and_drain_keeps_delivery() {
    let mut h = Harness::new(1, 2, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let (ticket, stream) = reserve_completion(&mut h, drops.clone(), 0);
    let completion = activate_observer(&mut h, ticket, stream);
    h.state.lock().unwrap().adoption_ready_mode = 1;
    for _ in 0..2 {
        h.advance();
        h.registry.retire_unpublished_v1(&mut h.context, 1);
        assert_eq!(h.registry.active_len(), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert_eq!(h.state.lock().unwrap().adoption_retire_calls, 0);
    }
    h.state.lock().unwrap().adoption_ready_mode = 0;
    for _ in 0..3 {
        h.registry.retire_unpublished_v1(&mut h.context, 1);
        assert_eq!(h.registry.active_len(), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        h.advance();
    }
    h.registry.retire_unpublished_v1(&mut h.context, 1);
    assert_eq!(h.registry.active_len(), 1);
    h.advance();
    assert!(!h.context.is_terminal());
    assert_eq!(h.registry.active_len(), 0);
    assert_eq!(h.handle.observer.reply_cells_in_use(), 1);
    assert!(ready(completion).unwrap().is_ok());
    assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(
        h.state
            .lock()
            .unwrap()
            .adoption_order
            .contains(&"gate_commit")
    );
    h.context.destroy_stream(stream).unwrap();
}

#[test]
fn generated_completion_observer_drop_preserves_execution_and_original_owner() {
    let mut h = Harness::new(1, 2, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let (ticket, stream) = reserve_completion(&mut h, drops.clone(), 0);
    let completion = activate_observer(&mut h, ticket, stream);
    drop(completion);
    assert_eq!(h.handle.observer.reply_cells_in_use(), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    for _ in 0..4 {
        h.advance();
    }
    assert!(!h.context.is_terminal());
    assert_eq!(h.registry.active_len(), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
    assert!(
        h.state
            .lock()
            .unwrap()
            .adoption_order
            .contains(&"gate_commit")
    );
    h.context.destroy_stream(stream).unwrap();
}

#[test]
fn generated_completion_activation_pressure_returns_exact_ticket_without_another_cell() {
    let mut h = Harness::new(1, 2, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let (ticket, stream) = reserve_completion(&mut h, drops.clone(), 0);
    let key = ticket.key.clone();
    let held = owned::Reply::<()>::budgeted_pair(&h.handle.observer.reply_budget).unwrap();
    let failure = h
        .handle
        .enqueue_reserved_activation_v1(ticket, stream, true)
        .err()
        .expect("reply capacity rejection");
    assert!(matches!(
        failure.error,
        ActivationErrorV1::Engine(RuntimeAsyncEngineCallErrorV1::ReplyCapacity)
    ));
    assert!(Arc::ptr_eq(&key, &failure.ticket.key));
    assert_eq!(h.handle.observer.reply_cells_in_use(), 2);
    assert!(h.state.lock().unwrap().adoption_order.is_empty());
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(held);
    let activation = h
        .handle
        .enqueue_reserved_activation_v1(failure.ticket, stream, true)
        .unwrap();
    assert_eq!(h.handle.observer.reply_cells_in_use(), 2);
    h.command();
    let completion = ready(activation).unwrap().unwrap();
    assert_eq!(h.handle.observer.reply_cells_in_use(), 1);
    for _ in 0..4 {
        h.advance();
    }
    assert!(ready(completion).unwrap().is_ok());
    assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
    h.context.destroy_stream(stream).unwrap();
}

#[test]
fn generated_completion_activation_preserves_preflight_precedence_and_custody() {
    for mode in 0..3 {
        let mut h = Harness::new(1, 2, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let (ticket, stream) = reserved(&mut h, drops.clone(), 0, true);
        let key = ticket.key.clone();
        let foreign = Harness::new(1, 2, true);
        if mode == 0 {
            h.handle
                .observer
                .worker_thread
                .set(thread::current().id())
                .unwrap();
        }
        let handle = if mode == 1 {
            &foreign.handle
        } else {
            &h.handle
        };
        let failure = handle
            .enqueue_reserved_activation_v1(ticket, stream, true)
            .err()
            .expect("preflight rejection");
        match mode {
            0 => assert!(matches!(
                failure.error,
                ActivationErrorV1::Engine(RuntimeAsyncEngineCallErrorV1::ReentrantCall)
            )),
            1 => assert!(matches!(
                failure.error,
                ActivationErrorV1::Engine(RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket)
            )),
            _ => assert!(matches!(
                failure.error,
                ActivationErrorV1::Context(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::Unsupported
                ))
            )),
        }
        assert!(Arc::ptr_eq(&key, &failure.ticket.key));
        assert!(h.state.lock().unwrap().adoption_order.is_empty());
        assert_eq!(h.handle.observer.reply_cells_in_use(), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert!(h.registry.discard_reserved(&key));
        drop(failure.ticket);
        assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
        h.context.destroy_stream(stream).unwrap();
    }
}
