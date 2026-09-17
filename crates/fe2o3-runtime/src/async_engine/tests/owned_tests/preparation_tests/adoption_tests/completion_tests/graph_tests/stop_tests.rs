use super::*;
use generated_operation::adoption::IssueHooksV1;

type OwnerPause = (SyncSender<()>, Receiver<()>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StopPhase {
    Adopting,
    Adopted,
    Issued,
    PhysicallySettled,
    Settled,
    Decoding,
    Decoded,
}

fn pause(pause: Option<OwnerPause>) {
    if let Some((entered, release)) = pause {
        entered.send(()).unwrap();
        release
            .recv_timeout(Duration::from_secs(5))
            .expect("Stop test did not release the owner checkpoint");
    }
}

fn checkpoint() -> (OwnerPause, Receiver<()>, SyncSender<()>) {
    let (entered, observed) = sync_channel(1);
    let (release, resumed) = sync_channel(1);
    ((entered, resumed), observed, release)
}

fn entered(observed: &Receiver<()>) {
    observed
        .recv_timeout(Duration::from_secs(5))
        .expect("owner did not reach the Stop checkpoint");
}

fn queue_stop(handle: &RuntimeAsyncProgressHandleV1<ThreadBoundBackend>) {
    handle.observer.admission.close();
    handle
        .observer
        .sender
        .try_send(RuntimeAsyncEngineCommandV1::Stop)
        .unwrap_or_else(|_| panic!("bounded Stop queue"));
}

struct PausedPayload {
    inner: Payload,
    phase: StopPhase,
    pause: Option<OwnerPause>,
}

impl PausedPayload {
    fn checkpoint(&mut self, phase: StopPhase) {
        if self.phase == phase {
            pause(self.pause.take());
        }
    }
}

fn prepare_paused(
    handle: &RuntimeAsyncProgressHandleV1<ThreadBoundBackend>,
    state: Arc<Mutex<MockState>>,
    drops: Arc<AtomicUsize>,
    domain: Arc<()>,
    mode: u8,
    phase: StopPhase,
    pause: Option<OwnerPause>,
) -> RuntimeAsyncReservedTicketV1 {
    let hooks = AdoptionHooksV1 {
        preflight: |context, payload: &PausedPayload, roster, stream, access| {
            (completion_hooks().preflight)(context, &payload.inner, roster, stream, access)
        },
        ready: |context: &RuntimeContextV1<ThreadBoundBackend>, hold| {
            let ready = (completion_hooks().ready)(context, hold)?;
            let checkpoint = context
                .backend()
                .trace
                .lock()
                .unwrap()
                .adoption_ready_pause
                .take();
            self::pause(checkpoint);
            Ok(ready)
        },
        adopt: |context, payload: &mut PausedPayload, roster, hold| {
            (completion_hooks().adopt)(context, &mut payload.inner, roster, hold)?;
            payload.checkpoint(StopPhase::Adopted);
            Ok(())
        },
        retire: |context, hold| (completion_hooks().retire)(context, hold),
        issue: Some(IssueHooksV1 {
            progress: |context, payload: &mut PausedPayload, roster, hold| {
                let complete = (completion_hooks().issue.unwrap().progress)(
                    context,
                    &mut payload.inner,
                    roster,
                    hold,
                )?;
                payload.checkpoint(if complete {
                    StopPhase::PhysicallySettled
                } else {
                    StopPhase::Issued
                });
                Ok(complete)
            },
            retire_stopped: |context, hold| {
                (completion_hooks().issue.unwrap().retire_stopped)(context, hold)
            },
            completion: Some(CompletionHooksV1 {
                domain: |payload: &PausedPayload| {
                    (completion_hooks::<ThreadBoundBackend>()
                        .issue
                        .unwrap()
                        .completion
                        .unwrap()
                        .domain)(&payload.inner)
                },
                settle: |context, payload: &mut PausedPayload, roster, hold| {
                    (completion_hooks().issue.unwrap().completion.unwrap().settle)(
                        context,
                        &mut payload.inner,
                        roster,
                        hold,
                    )?;
                    payload.checkpoint(StopPhase::Settled);
                    Ok(())
                },
                decode: |mut payload: PausedPayload| {
                    payload.checkpoint(StopPhase::Decoding);
                    let result = (completion_hooks::<ThreadBoundBackend>()
                        .issue
                        .unwrap()
                        .completion
                        .unwrap()
                        .decode)(payload.inner);
                    if payload.phase == StopPhase::Decoded {
                        self::pause(payload.pause.take());
                    }
                    result
                },
            }),
        }),
    };
    let prepared = join(
        handle
            .enqueue_preparation_with_adoption_v1(
                Box::new(move |_| {
                    Ok::<_, ()>(PausedPayload {
                        inner: Payload {
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
                            stream: None,
                            issue_advances: 0,
                            result_domain: domain,
                        },
                        phase,
                        pause,
                    })
                }),
                Some(|context, payload| reserve(context, &mut payload.inner)),
                Some(hooks),
            )
            .unwrap(),
    )
    .unwrap()
    .unwrap();
    join(handle.try_reserve_prepared_v1(prepared).unwrap())
        .unwrap()
        .unwrap()
}

fn prepare_plain(
    handle: &RuntimeAsyncProgressHandleV1<ThreadBoundBackend>,
    state: Arc<Mutex<MockState>>,
    drops: Arc<AtomicUsize>,
) -> RuntimeAsyncReservedTicketV1 {
    let prepared = join(preparation_with_hooks(
        handle,
        state,
        drops,
        0,
        Some(completion_hooks()),
    ))
    .unwrap()
    .unwrap();
    join(handle.try_reserve_prepared_v1(prepared).unwrap())
        .unwrap()
        .unwrap()
}

fn owner_config() -> RuntimeAsyncEngineConfigV1 {
    RuntimeAsyncEngineConfigV1::new(8, 8, 8, 1, Duration::from_millis(1)).unwrap()
}

fn assert_cancelled_suffix(report: &RuntimeGeneratedGraphReportV1<MockError>) {
    assert_eq!(report.graph.completion.entries().len(), 3);
    assert!(
        matches!(report.graph.completion.entries()[1].state(), CompletionNodeStateV1::Cancelled { origin, .. } if origin == id(2))
    );
    assert!(
        matches!(report.graph.completion.entries()[2].state(), CompletionNodeStateV1::DependencyCancelled { origin, .. } if origin == id(2))
    );
}

fn assert_original_result(
    report: &RuntimeGeneratedGraphReportV1<MockError>,
    domain: &Arc<()>,
    mode: u8,
) {
    assert_cancelled_suffix(report);
    assert_eq!(report.completions.len(), usize::from(mode == 0));
    assert_eq!(report.errors.len(), usize::from(mode != 0));
    if mode != 0 {
        assert!(
            matches!(report.graph.completion.entries()[0].state(), CompletionNodeStateV1::Failed { origin, .. } if origin == id(1))
        );
    }
    match mode {
        0 => {
            assert_eq!(report.completions[0].0, id(1));
            assert!(report.completions[0].1.matches_owner(domain));
            assert!(!report.completions[0].1.matches_owner(&Arc::new(())));
            assert_eq!(
                report.graph.completion.entries()[0].state(),
                CompletionNodeStateV1::Succeeded
            );
        }
        23 => assert!(
            matches!(report.errors.as_slice(), [(node, RuntimeGeneratedGraphNodeErrorV1::Readback(crate::RuntimeGfx942ReadbackErrorV1::InvalidStorage))] if *node == id(1))
        ),
        24 => assert!(
            matches!(report.errors.as_slice(), [(node, RuntimeGeneratedGraphNodeErrorV1::Engine(RuntimeAsyncEngineCallErrorV1::CommandPanicked))] if *node == id(1))
        ),
        _ => unreachable!(),
    }
}

fn stopped_at(phase: StopPhase, mode: u8, drop_observer: bool, explicitly_ready: bool) {
    let state = Arc::new(Mutex::new(MockState::default()));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start_with_config(state.clone(), trace.clone(), owner_config());
    let owner = *handle.observer.worker_thread.get().unwrap();
    let stream = handle
        .observer
        .try_with_context(|context| context.create_stream(context.devices()[0].id()).unwrap())
        .unwrap();
    let (checkpoint, observed, release) = checkpoint();
    let first_drops = Arc::new(AtomicUsize::new(0));
    let suffix_drops = Arc::new(AtomicUsize::new(0));
    let unrelated_drops = Arc::new(AtomicUsize::new(0));
    let domain = Arc::new(());
    let checkpoint = if phase == StopPhase::Adopting {
        state.lock().unwrap().adoption_ready_mode = 1;
        trace.lock().unwrap().adoption_ready_pause = Some(checkpoint);
        None
    } else {
        Some(checkpoint)
    };
    let first = prepare_paused(
        &handle,
        state.clone(),
        first_drops.clone(),
        domain.clone(),
        mode,
        phase,
        checkpoint,
    );
    let mut tickets = vec![first];
    for _ in 0..2 {
        tickets.push(prepare_plain(&handle, state.clone(), suffix_drops.clone()));
    }
    let unrelated = prepare_plain(&handle, state.clone(), unrelated_drops.clone());
    let request = handle
        .observer
        .try_with_context(move |context| request(context, stream, tickets))
        .unwrap();
    let mut future = handle.try_submit_generated_graph_v1(request).unwrap();
    entered(&observed);
    // No graph observation can run while the original callback is gated.
    assert!(
        Pin::new(&mut future)
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    let future = if drop_observer {
        drop(future);
        None
    } else {
        Some(future)
    };
    if explicitly_ready {
        // The queued context command runs after reply publication but before
        // the next graph-progress phase. Stop is queued while that command waits.
        let (second, observed, resumed) = self::checkpoint();
        drop(
            handle
                .observer
                .enqueue_with_context(move |_| pause(Some(second)))
                .unwrap(),
        );
        release.send(()).unwrap();
        entered(&observed);
        assert_eq!(first_drops.load(Ordering::SeqCst), 1);
        assert_eq!(suffix_drops.load(Ordering::SeqCst), 0);
        assert_eq!(unrelated_drops.load(Ordering::SeqCst), 0);
        queue_stop(&handle);
        resumed.send(()).unwrap();
    } else {
        queue_stop(&handle);
        release.send(()).unwrap();
    }
    let shutdown = engine.shutdown().unwrap();
    let settled = matches!(
        phase,
        StopPhase::Settled | StopPhase::Decoding | StopPhase::Decoded
    );
    assert!(!shutdown.worker_panicked);
    assert!(shutdown.native_failure.is_none());
    assert_eq!(
        shutdown.disposition,
        if settled {
            RuntimeAsyncOwnedDispositionV1::Released
        } else {
            RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
        },
        "{phase:?}, mode {mode}"
    );
    assert_eq!(shutdown.cleanup.unwrap().is_complete(), settled);
    assert_eq!(first_drops.load(Ordering::SeqCst), usize::from(settled));
    assert_eq!(suffix_drops.load(Ordering::SeqCst), 2);
    assert_eq!(unrelated_drops.load(Ordering::SeqCst), usize::from(settled));
    if let Some(future) = future {
        if settled {
            assert_original_result(&ready(future).unwrap().unwrap(), &domain, mode);
        } else {
            assert!(matches!(
                ready(future),
                Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
            ));
        }
    }
    drop(unrelated);
    assert_eq!(handle.observer.reply_cells_in_use(), 0);
    assert!(!handle.observer.graph_slot.load(Ordering::Acquire));
    assert!(matches!(
        handle.observer.enqueue_with_context(|_| ()),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    let state = state.lock().unwrap();
    let mut expected = vec!["preflight"; 4]; // Three admission checks, then exact activation.
    if phase != StopPhase::Adopting {
        expected.push("adopt");
    }
    if !matches!(phase, StopPhase::Adopting | StopPhase::Adopted) {
        expected.push("issue");
    }
    if !matches!(
        phase,
        StopPhase::Adopting | StopPhase::Adopted | StopPhase::Issued
    ) {
        expected.push("poll");
    }
    if settled {
        expected.extend([
            "copy",
            "postcheck",
            "native_retire",
            "retire",
            "records_retire",
            "hold_release",
            "hold_released",
            "decode",
        ]);
        if mode == 0 {
            expected.push("gate_commit");
        }
        expected.push("payload_drop");
    }
    expected.extend(["payload_drop"; 2]);
    if settled {
        expected.push("payload_drop");
    }
    assert_eq!(state.adoption_order, expected, "{phase:?}, mode {mode}");
    assert!(
        state
            .adoption_payload_drops
            .iter()
            .all(|(_, thread)| *thread == owner)
    );
    drop(state);
    let trace = trace.lock().unwrap();
    assert!(trace.calls.iter().all(|(_, thread)| *thread == owner));
    assert_eq!(
        trace
            .calls
            .iter()
            .filter(|(call, _)| *call == "finalize")
            .count(),
        usize::from(settled)
    );
    assert_eq!(
        trace
            .calls
            .iter()
            .filter(|(call, _)| *call == "drop")
            .count(),
        usize::from(settled)
    );
    assert!(
        !trace
            .calls
            .iter()
            .any(|(call, _)| matches!(*call, "poll_v1" | "release_submission_v1" | "flush"))
    );
}

#[test]
fn generated_graph_owned_stop_preserves_each_unsettled_phase() {
    for phase in [
        StopPhase::Adopting,
        StopPhase::Adopted,
        StopPhase::Issued,
        StopPhase::PhysicallySettled,
    ] {
        for drop_observer in [false, true] {
            stopped_at(phase, 0, drop_observer, false);
        }
    }
}

#[test]
fn generated_graph_owned_stop_after_settlement_keeps_original_decoder_outcome() {
    for phase in [StopPhase::Settled, StopPhase::Decoding, StopPhase::Decoded] {
        for mode in [0, 23, 24] {
            if phase == StopPhase::Decoded && mode == 24 {
                continue;
            }
            for drop_observer in [false, true] {
                stopped_at(phase, mode, drop_observer, false);
            }
        }
    }
}

#[test]
fn generated_graph_owned_stop_collects_reply_published_before_stop() {
    for mode in [0, 23, 24] {
        for drop_observer in [false, true] {
            stopped_at(StopPhase::Decoding, mode, drop_observer, true);
        }
    }
}

#[test]
fn generated_graph_owned_stop_before_admission_or_before_activation() {
    for before_admission in [false, true] {
        for drop_observer in [false, true] {
            let state = Arc::new(Mutex::new(MockState::default()));
            let trace = Arc::new(Mutex::new(OwnerTrace::default()));
            let (engine, handle) = start_with_config(state.clone(), trace, owner_config());
            let stream = handle
                .observer
                .try_with_context(|context| {
                    context.create_stream(context.devices()[0].id()).unwrap()
                })
                .unwrap();
            let drops = Arc::new(AtomicUsize::new(0));
            let tickets = (0..3)
                .map(|_| prepare_plain(&handle, state.clone(), drops.clone()))
                .collect();
            let request = handle
                .observer
                .try_with_context(move |context| request(context, stream, tickets))
                .unwrap();
            let (checkpoint, observed, release) = checkpoint();
            drop(
                handle
                    .observer
                    .enqueue_with_context(move |_| pause(Some(checkpoint)))
                    .unwrap(),
            );
            entered(&observed);
            if before_admission {
                // A graph accepted concurrently after the ordered Stop command
                // stays queued and must return all original tickets on Drop.
                handle
                    .observer
                    .sender
                    .try_send(RuntimeAsyncEngineCommandV1::Stop)
                    .unwrap_or_else(|_| panic!("Stop queue"));
            }
            let future = handle.try_submit_generated_graph_v1(request).unwrap();
            let future = if drop_observer {
                drop(future);
                None
            } else {
                Some(future)
            };
            queue_stop(&handle);
            release.send(()).unwrap();
            assert_eq!(
                engine.shutdown().unwrap().disposition,
                RuntimeAsyncOwnedDispositionV1::Released
            );
            assert_eq!(drops.load(Ordering::SeqCst), 3);
            if let Some(future) = future {
                let result = ready(future).unwrap();
                if before_admission {
                    let Err(RuntimeGeneratedGraphFailureV1::Rejected(mut failure)) = result else {
                        panic!("queued rejection");
                    };
                    for n in 1..=3 {
                        assert!(failure.request.take_reserved(id(n)).is_some());
                    }
                } else {
                    let report = result.unwrap();
                    assert!(report.completions.is_empty());
                    assert!(report.errors.is_empty());
                    assert!(
                        matches!(report.graph.completion.entries()[0].state(), CompletionNodeStateV1::Cancelled { origin, .. } if origin == id(1))
                    );
                    assert!(report.graph.completion.entries()[1..].iter().all(|entry| matches!(entry.state(), CompletionNodeStateV1::DependencyCancelled { origin, .. } if origin == id(1))));
                }
            }
            assert_eq!(handle.observer.reply_cells_in_use(), 0);
            assert!(!handle.observer.graph_slot.load(Ordering::Acquire));
            let order = state.lock().unwrap().adoption_order.clone();
            let mut expected = if before_admission {
                Vec::new()
            } else {
                vec!["preflight"; 3]
            };
            expected.extend(["payload_drop"; 3]);
            assert_eq!(order, expected);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Companion {
    Ready,
    PendingGenerated,
    PendingOrdinary,
}

fn independent_stop(companion: Companion, mode: u8, drop_observer: bool) {
    let state = Arc::new(Mutex::new(MockState::default()));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let config = RuntimeAsyncEngineConfigV1::new(8, 8, 8, 4, Duration::from_millis(1)).unwrap();
    let (engine, handle) = start_with_config(state.clone(), trace.clone(), config);
    let drops = [Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0))];
    let domains = [Arc::new(()), Arc::new(())];
    let first = (companion != Companion::PendingOrdinary).then(|| {
        prepare_paused(
            &handle,
            state.clone(),
            drops[0].clone(),
            domains[0].clone(),
            if companion == Companion::PendingGenerated {
                12
            } else {
                0
            },
            StopPhase::Decoding,
            None,
        )
    });
    let (checkpoint, observed, release) = checkpoint();
    let second = prepare_paused(
        &handle,
        state.clone(),
        drops[1].clone(),
        domains[1].clone(),
        mode,
        StopPhase::Decoding,
        Some(checkpoint),
    );
    let request = handle
        .observer
        .try_with_context(move |context| {
            let device = context.devices()[0].id();
            let streams = [
                context.create_stream(device).unwrap(),
                context.create_stream(device).unwrap(),
            ];
            let identities =
                streams.map(|stream| context.completion_stream_identity_v1(stream).unwrap());
            let graph = CompletionGraphV1::new(
                identities[0].context(),
                identities.to_vec(),
                vec![
                    CompletionNodeV1::future(
                        id(1),
                        FutureIdentityV1::new(identities[0], [1; 32]),
                        None,
                    ),
                    CompletionNodeV1::future(
                        id(2),
                        FutureIdentityV1::new(identities[1], [2; 32]),
                        None,
                    ),
                ],
            )
            .unwrap();
            let mut request =
                RuntimeGraphRequestV1::new(graph, identities.into_iter().zip(streams).collect())
                    .unwrap();
            if first.is_none() {
                let module = context.load_module(device, &[1]).unwrap();
                let kernel = Arc::new(
                    context
                        .resolve_kernel::<EmptyArgs>(module, "pending")
                        .unwrap(),
                );
                request
                    .bind_launch(id(1), kernel, &EmptyArgs, geometry())
                    .unwrap();
            }
            let mut request = RuntimeGeneratedGraphRequestV1::new(request);
            if let Some(first) = first {
                request.bind_reserved(id(1), first).unwrap();
            }
            request.bind_reserved(id(2), second).unwrap();
            request
        })
        .unwrap();
    let future = handle.try_submit_generated_graph_v1(request).unwrap();
    entered(&observed);
    assert_eq!(
        drops[0].load(Ordering::SeqCst),
        usize::from(companion == Companion::Ready)
    );
    assert_eq!(drops[1].load(Ordering::SeqCst), 0);
    let calls_before = trace.lock().unwrap().calls.len();
    let order_before = state.lock().unwrap().adoption_order.clone();
    let future = if drop_observer {
        drop(future);
        None
    } else {
        Some(future)
    };
    queue_stop(&handle);
    release.send(()).unwrap();
    let shutdown = engine.shutdown().unwrap();
    assert_eq!(
        shutdown.disposition,
        if companion == Companion::Ready {
            RuntimeAsyncOwnedDispositionV1::Released
        } else {
            RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
        }
    );
    assert_eq!(drops[1].load(Ordering::SeqCst), 1);
    assert_eq!(
        drops[0].load(Ordering::SeqCst),
        usize::from(companion == Companion::Ready)
    );
    if let Some(future) = future {
        if companion == Companion::Ready {
            let report = ready(future).unwrap().unwrap();
            assert_eq!(report.completions.len(), if mode == 0 { 2 } else { 1 });
            assert_eq!(report.errors.len(), usize::from(mode != 0));
            for (node, receipt) in &report.completions {
                let index = if *node == id(1) {
                    0
                } else {
                    assert_eq!(*node, id(2));
                    1
                };
                assert!(receipt.matches_owner(&domains[index]));
                assert!(!receipt.matches_owner(&domains[1 - index]));
            }
            assert_eq!(
                report.graph.completion.entries()[0].state(),
                CompletionNodeStateV1::Succeeded
            );
            match mode {
                0 => assert_eq!(
                    report.graph.completion.entries()[1].state(),
                    CompletionNodeStateV1::Succeeded
                ),
                23 => assert!(
                    matches!(report.errors.as_slice(), [(node, RuntimeGeneratedGraphNodeErrorV1::Readback(crate::RuntimeGfx942ReadbackErrorV1::InvalidStorage))] if *node == id(2))
                ),
                24 => assert!(
                    matches!(report.errors.as_slice(), [(node, RuntimeGeneratedGraphNodeErrorV1::Engine(RuntimeAsyncEngineCallErrorV1::CommandPanicked))] if *node == id(2))
                ),
                _ => unreachable!(),
            }
        } else {
            assert!(matches!(
                ready(future),
                Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
            ));
        }
    }
    assert_eq!(handle.observer.reply_cells_in_use(), 0);
    assert!(!handle.observer.graph_slot.load(Ordering::Acquire));
    let mut expected = order_before;
    expected.push("decode");
    if mode == 0 {
        expected.push("gate_commit");
    }
    expected.push("payload_drop");
    assert_eq!(state.lock().unwrap().adoption_order, expected);
    let calls_after = trace.lock().unwrap().calls[calls_before..].to_vec();
    if companion == Companion::Ready {
        assert_eq!(
            calls_after
                .iter()
                .map(|(call, _)| *call)
                .collect::<Vec<_>>(),
            ["destroy_stream_v1", "destroy_stream_v1", "finalize", "drop"]
        );
    } else {
        assert!(
            calls_after.is_empty(),
            "Stop entered backend: {calls_after:?}"
        );
    }
}

#[test]
fn generated_graph_owned_stop_collects_all_independent_ready_results() {
    for mode in [0, 23, 24] {
        for drop_observer in [false, true] {
            independent_stop(Companion::Ready, mode, drop_observer);
        }
    }
}

#[test]
fn generated_graph_owned_stop_handles_pending_generated_and_ready_completion() {
    for mode in [0, 23, 24] {
        for drop_observer in [false, true] {
            independent_stop(Companion::PendingGenerated, mode, drop_observer);
        }
    }
}

#[test]
fn generated_graph_owned_stop_never_polls_or_retires_pending_ordinary_work() {
    for mode in [0, 23, 24] {
        for drop_observer in [false, true] {
            independent_stop(Companion::PendingOrdinary, mode, drop_observer);
        }
    }
}
