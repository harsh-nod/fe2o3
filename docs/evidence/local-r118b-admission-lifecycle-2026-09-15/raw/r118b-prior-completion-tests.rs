use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
struct WakeObservation {
    owner: usize,
    thread: ThreadId,
    payload_drops: usize,
}

struct CompletionWake {
    owner: usize,
    drops: Arc<AtomicUsize>,
    observations: Arc<Mutex<Vec<WakeObservation>>>,
    panics: bool,
}

impl std::task::Wake for CompletionWake {
    fn wake(self: Arc<Self>) {
        self.observations.lock().unwrap().push(WakeObservation {
            owner: self.owner,
            thread: thread::current().id(),
            payload_drops: self.drops.load(Ordering::SeqCst),
        });
        if self.panics {
            panic!("scripted reserved-completion wake panic");
        }
    }
}

fn poll_completion(
    ticket: &mut RuntimeAsyncReservedTicketV1,
    waker: &Waker,
) -> Poll<Result<(), RuntimeAsyncEngineCallErrorV1>> {
    Pin::new(ticket.completion_for_test_v1()).poll(&mut Context::from_waker(waker))
}

fn assert_stopped(ticket: &mut RuntimeAsyncReservedTicketV1) {
    assert_eq!(
        poll_completion(ticket, Waker::noop()),
        Poll::Ready(Err(RuntimeAsyncEngineCallErrorV1::EngineStopped))
    );
}

#[test]
fn reserved_completion_isolates_discard_order_and_abandoned_observers() {
    for order in [[0, 1], [1, 0]] {
        for abandoned in [false, true] {
            let mut h = Harness::new(2, 4, true);
            let drops: [Arc<AtomicUsize>; 2] =
                std::array::from_fn(|_| Arc::new(AtomicUsize::new(0)));
            let mut tickets = drops
                .each_ref()
                .map(|drops| Some(reservation_tests::reserved(&mut h, drops.clone())));
            let keys = tickets
                .each_ref()
                .map(|ticket| ticket.as_ref().unwrap().key.clone());
            let observations = Arc::new(Mutex::new(Vec::new()));
            let wakers: [Waker; 2] = std::array::from_fn(|owner| {
                Waker::from(Arc::new(CompletionWake {
                    owner,
                    drops: drops[owner].clone(),
                    observations: observations.clone(),
                    panics: false,
                }))
            });
            for (ticket, waker) in tickets.iter_mut().zip(&wakers) {
                assert!(poll_completion(ticket.as_mut().unwrap(), waker).is_pending());
            }
            assert_eq!(h.handle.observer.reply_cells_in_use(), 2);
            let [first, second] = order;
            if abandoned {
                drop(tickets[first].take());
                assert_eq!(h.handle.observer.reply_cells_in_use(), 2);
            }
            assert!(h.registry.discard_reserved(&keys[first]));
            assert_eq!(h.registry.len(), 1);
            assert_eq!(drops[first].load(Ordering::SeqCst), 1);
            assert_eq!(drops[second].load(Ordering::SeqCst), 0);
            let mut expected_wakes = if abandoned {
                Vec::new()
            } else {
                vec![WakeObservation {
                    owner: first,
                    thread: thread::current().id(),
                    payload_drops: 0,
                }]
            };
            assert_eq!(*observations.lock().unwrap(), expected_wakes);
            assert_eq!(
                h.handle.observer.reply_cells_in_use(),
                if abandoned { 1 } else { 2 }
            );
            if let Some(ticket) = tickets[first].as_mut() {
                assert_stopped(ticket);
            }
            drop(tickets[first].take());
            assert_eq!(h.handle.observer.reply_cells_in_use(), 1);
            assert!(!h.registry.discard_reserved(&keys[first]));
            for _ in 0..4 {
                h.advance();
            }
            assert!(
                poll_completion(tickets[second].as_mut().unwrap(), &wakers[second]).is_pending()
            );
            assert_eq!(drops[second].load(Ordering::SeqCst), 0);
            assert_eq!(*observations.lock().unwrap(), expected_wakes);
            assert!(h.registry.discard_reserved(&keys[second]));
            expected_wakes.push(WakeObservation {
                owner: second,
                thread: thread::current().id(),
                payload_drops: 0,
            });
            assert_eq!(*observations.lock().unwrap(), expected_wakes);
            assert_stopped(tickets[second].as_mut().unwrap());
            assert_eq!(h.handle.observer.reply_cells_in_use(), 1);
            drop(tickets[second].take());
            assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
            assert_eq!(drops.each_ref().map(|v| v.load(Ordering::SeqCst)), [1, 1]);
            assert_eq!((h.registry.len(), h.registry.active_len()), (0, 0));
            assert!(!h.context.is_terminal());
            let state = h.state.lock().unwrap();
            assert!(state.issues.is_empty() && state.flush_calls.is_empty());
        }
    }
}

#[test]
fn reserved_stop_notifies_latest_wakers_before_disposal_and_contains_wake_panic() {
    for panics in [false, true] {
        let mut h = Harness::new(2, 4, true);
        let drops: [Arc<AtomicUsize>; 2] = std::array::from_fn(|_| Arc::new(AtomicUsize::new(0)));
        let mut tickets = drops
            .each_ref()
            .map(|drops| reservation_tests::reserved(&mut h, drops.clone()));
        let old = Arc::new(Mutex::new(Vec::new()));
        let latest = Arc::new(Mutex::new(Vec::new()));
        for (owner, ticket) in tickets.iter_mut().enumerate() {
            let old_waker = Waker::from(Arc::new(CompletionWake {
                owner,
                drops: drops[owner].clone(),
                observations: old.clone(),
                panics: false,
            }));
            let latest_waker = Waker::from(Arc::new(CompletionWake {
                owner,
                drops: drops[owner].clone(),
                observations: latest.clone(),
                panics: panics && owner == 0,
            }));
            for waker in [&old_waker, &latest_waker, &latest_waker] {
                assert!(poll_completion(ticket, waker).is_pending());
            }
        }
        h.handle
            .observer
            .sender
            .try_send(RuntimeAsyncEngineCommandV1::Stop)
            .unwrap_or_else(|_| panic!("Stop queue capacity"));
        let Harness {
            mut context,
            state,
            handle,
            receiver,
            mut registry,
            config,
        } = h;
        run_engine_context_v1(
            &mut context,
            &mut registry,
            receiver,
            config,
            None,
            handle.observer.admission.clone(),
        );
        assert!(!context.is_terminal());
        assert_eq!(registry.len(), 2);
        assert_eq!(drops.each_ref().map(|v| v.load(Ordering::SeqCst)), [0, 0]);
        for _ in 0..4 {
            assert!(!registry.stop_observations());
            operation::advance_operations_v1(
                &mut context,
                &mut registry,
                4,
                4,
                flush_stream_v1::<MockBackend>,
            );
        }
        assert!(!context.is_terminal());
        assert!(old.lock().unwrap().is_empty());
        let expected: Vec<_> = (0..2)
            .map(|owner| WakeObservation {
                owner,
                thread: thread::current().id(),
                payload_drops: 0,
            })
            .collect();
        assert_eq!(*latest.lock().unwrap(), expected);
        for ticket in &mut tickets {
            assert_stopped(ticket);
        }
        assert_eq!(handle.observer.reply_cells_in_use(), 2);
        assert_eq!(registry.len(), 2);
        assert_eq!(drops.each_ref().map(|v| v.load(Ordering::SeqCst)), [0, 0]);
        registry.dispose_quiescent();
        assert_eq!(drops.each_ref().map(|v| v.load(Ordering::SeqCst)), [1, 1]);
        assert_eq!(*latest.lock().unwrap(), expected);
        assert_eq!(handle.observer.reply_cells_in_use(), 2);
        drop(tickets);
        assert_eq!(handle.observer.reply_cells_in_use(), 0);
        let state = state.lock().unwrap();
        assert!(state.issues.is_empty() && state.flush_calls.is_empty());
    }
}

#[test]
fn reserved_discard_wakes_before_payload_drop_even_when_wake_panics() {
    for panics in [false, true] {
        let mut h = Harness::new(1, 3, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let mut ticket = reservation_tests::reserved(&mut h, drops.clone());
        let observations = Arc::new(Mutex::new(Vec::new()));
        let waker = Waker::from(Arc::new(CompletionWake {
            owner: 0,
            drops: drops.clone(),
            observations: observations.clone(),
            panics,
        }));
        assert!(poll_completion(&mut ticket, &waker).is_pending());
        assert!(h.registry.discard_reserved(&ticket.key));
        assert_eq!(
            *observations.lock().unwrap(),
            [WakeObservation {
                owner: 0,
                thread: thread::current().id(),
                payload_drops: 0,
            }]
        );
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(!h.registry.discard_reserved(&ticket.key));
        assert!(!h.context.is_terminal());
        assert_stopped(&mut ticket);
        assert_eq!(h.handle.observer.reply_cells_in_use(), 1);
        drop(ticket);
        assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
    }
}

#[test]
fn unpublished_retirement_preserves_exact_failed_owner_and_untouched_neighbor() {
    for mode in [0, 1, 2] {
        let mut h = Harness::new(3, 8, true);
        let drops: [Arc<AtomicUsize>; 3] = std::array::from_fn(|_| Arc::new(AtomicUsize::new(0)));
        let reserved = drops
            .each_ref()
            .map(|drops| adoption_tests::reserved(&mut h, drops.clone(), 0, true));
        let streams = reserved.each_ref().map(|(_, stream)| *stream);
        let wakes = Arc::new(Mutex::new(Vec::new()));
        h.state
            .lock()
            .unwrap()
            .adoption_retire_modes
            .insert(streams[1], mode);
        for (owner, (mut ticket, stream)) in reserved.into_iter().enumerate() {
            let waker = Waker::from(Arc::new(CompletionWake {
                owner,
                drops: drops[owner].clone(),
                observations: wakes.clone(),
                panics: false,
            }));
            assert!(poll_completion(&mut ticket, &waker).is_pending());
            adoption_tests::activate(&mut h, ticket, stream);
        }
        h.advance();
        assert!(!h.context.is_terminal());
        assert_eq!(
            h.state.lock().unwrap().adoption_completed,
            streams.map(|stream| (stream, thread::current().id()))
        );
        let held = streams.map(|stream| h.context.unpublished_identity_for_test_v1(stream));
        let identities: HashSet<_> = held.iter().map(|value| value.unwrap().unwrap()).collect();
        assert_eq!(identities.len(), 3);
        h.registry.retire_unpublished_v1(&mut h.context, 3);
        let success = mode == 0;
        assert_eq!(h.context.is_terminal(), !success);
        let expected_drops = if success { [1, 1, 1] } else { [1, 0, 0] };
        let expected_holds = if success {
            [Some(None); 3]
        } else {
            [Some(None), held[1], held[2]]
        };
        let attempt_count = if success { 3 } else { 2 };
        let attempts: Vec<_> = streams[..attempt_count]
            .iter()
            .map(|&stream| (stream, thread::current().id()))
            .collect();
        let disposals: Vec<_> = streams[..if success { 3 } else { 1 }]
            .iter()
            .map(|&stream| (Some(stream), thread::current().id()))
            .collect();
        for repetition in 0..5 {
            if repetition != 0 {
                h.registry.retire_unpublished_v1(&mut h.context, 3);
                h.advance();
                assert!(!h.registry.stop_observations());
            }
            assert_eq!(
                drops.each_ref().map(|v| v.load(Ordering::SeqCst)),
                expected_drops
            );
            assert_eq!(
                streams.map(|stream| h.context.unpublished_identity_for_test_v1(stream)),
                expected_holds
            );
            assert_eq!(h.registry.len(), if success { 0 } else { 2 });
            {
                let state = h.state.lock().unwrap();
                assert_eq!(state.adoption_retire_attempts, attempts);
                assert_eq!(state.adoption_payload_drops, disposals);
                assert!(state.issues.is_empty() && state.flush_calls.is_empty());
            }
        }
        assert_eq!(
            h.handle.observer.reply_cells_in_use(),
            if success { 0 } else { 2 }
        );
        let expected_wakes: Vec<_> = (0..3)
            .map(|owner| WakeObservation {
                owner,
                thread: thread::current().id(),
                payload_drops: 0,
            })
            .collect();
        assert_eq!(*wakes.lock().unwrap(), expected_wakes);
    }
}

#[test]
fn owned_shutdown_retires_in_order_and_retains_failed_and_unvisited_owners() {
    for mode in [0, 1, 2] {
        let state = Arc::new(Mutex::new(MockState::default()));
        let trace = Arc::new(Mutex::new(OwnerTrace::default()));
        let config = RuntimeAsyncEngineConfigV1::new(8, 3, 3, 3, Duration::from_millis(1))
            .unwrap()
            .with_reply_capacity(8)
            .unwrap();
        let (engine, handle) = start_with_config(state.clone(), trace.clone(), config);
        let streams: [RuntimeStreamIdV1; 3] = handle
            .observer
            .try_with_context(|context| {
                std::array::from_fn(|_| context.create_stream(context.devices()[0].id()).unwrap())
            })
            .unwrap();
        state
            .lock()
            .unwrap()
            .adoption_retire_modes
            .insert(streams[1], mode);
        let drops: [Arc<AtomicUsize>; 3] = std::array::from_fn(|_| Arc::new(AtomicUsize::new(0)));
        let tickets = drops.each_ref().map(|drops| {
            let prepared = join(adoption_tests::preparation(
                &handle,
                state.clone(),
                drops.clone(),
                0,
                true,
            ))
            .unwrap()
            .unwrap();
            join(handle.try_reserve_prepared_v1(prepared).unwrap())
                .unwrap()
                .unwrap()
        });
        for (ticket, stream) in tickets.into_iter().zip(streams) {
            join(handle.try_activate_reserved_v1(ticket, stream).unwrap())
                .unwrap()
                .unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while state.lock().unwrap().adoption_completed.len() != 3 {
            assert!(
                Instant::now() < deadline,
                "three owners did not finish adoption"
            );
            thread::yield_now();
        }
        let owner = trace
            .lock()
            .unwrap()
            .calls
            .iter()
            .find(|(call, _)| *call == "construct")
            .unwrap()
            .1;
        assert_ne!(owner, thread::current().id());
        assert_eq!(
            state.lock().unwrap().adoption_completed,
            streams.map(|stream| (stream, owner))
        );
        assert_eq!(
            drops.each_ref().map(|v| v.load(Ordering::SeqCst)),
            [0, 0, 0]
        );
        let report = engine.shutdown().unwrap();
        let success = mode == 0;
        assert!(!report.worker_panicked);
        assert_eq!(
            report.disposition,
            if success {
                RuntimeAsyncOwnedDispositionV1::Released
            } else {
                RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
            }
        );
        assert_eq!(
            drops.each_ref().map(|v| v.load(Ordering::SeqCst)),
            if success { [1, 1, 1] } else { [1, 0, 0] }
        );
        assert_eq!(
            handle.observer.reply_cells_in_use(),
            if success { 0 } else { 2 }
        );
        let state = state.lock().unwrap();
        let attempts: Vec<_> = streams[..if success { 3 } else { 2 }]
            .iter()
            .map(|&stream| (stream, owner))
            .collect();
        let disposals: Vec<_> = streams[..if success { 3 } else { 1 }]
            .iter()
            .map(|&stream| (Some(stream), owner))
            .collect();
        assert_eq!(state.adoption_retire_attempts, attempts);
        assert_eq!(state.adoption_payload_drops, disposals);
        assert!(state.issues.is_empty() && state.flush_calls.is_empty());
    }
}
