use super::*;

const DEPTH: usize = crate::MAX_RUNTIME_DEPENDENCIES_V1;

fn deep_fixture(journal: bool) -> Fixture {
    let mut h = Harness::with_journal_limits(4096, 8, true, journal, DEPTH + 4, DEPTH + 4);
    let streams = [
        h.stream,
        h.context
            .create_stream(h.context.devices()[1].id())
            .unwrap(),
    ];
    let allocations = (0..=DEPTH)
        .map(|index| {
            h.context
                .allocate(
                    h.context.devices()[index % 2].id(),
                    RuntimeMemoryKindV1::DeviceLocal,
                    16,
                    8,
                )
                .unwrap()
        })
        .collect();
    Fixture {
        h,
        streams,
        allocations,
    }
}

#[test]
fn directed_async_quiescent_producer_keeps_bounded_settlement_owned() {
    for journal in [false, true] {
        for observed_policy in [false, true] {
            for observer_end in 0..4 {
                let mut f = deep_fixture(journal);
                let callbacks = Arc::new(Mutex::new(Vec::new()));
                let mut producers = Vec::new();
                let mut backend_ids = Vec::new();
                let mut event = None;
                for index in 0..DEPTH - 1 {
                    let (submission, next_event, backend) =
                        f.producer(index, index + 1, event.as_slice());
                    if let Some(previous) = event.replace(next_event) {
                        f.h.context.release_event(previous).unwrap();
                    }
                    let output = callbacks.clone();
                    f.h.context
                        .on_completion(&submission, move |status| {
                            output.lock().unwrap().push((index, status));
                        })
                        .unwrap();
                    producers.push(submission);
                    backend_ids.push(backend);
                }
                let dependency = event.unwrap();
                let stream = f.streams[DEPTH % 2];
                let source = f.region(DEPTH - 1, false);
                let destination = f.region(DEPTH, true);
                // Exercise both ordinary driver policies with the same public
                // Context submission path; the directed branch uses its public API.
                let operation = if observed_policy {
                    f.h.handle
                        .enqueue_tracked_operation(
                            stream,
                            Box::new(move |context| {
                                context.directed_peer_copy_v1(
                                    stream,
                                    source,
                                    destination,
                                    &[dependency],
                                )
                            }),
                        )
                        .unwrap()
                } else {
                    f.h.handle
                        .directed_peer_copy_tracked(stream, source, destination, vec![dependency])
                        .unwrap()
                };
                let control = operation.control;
                let mut future = Some(operation.future);
                let mut registry = operation::OperationRegistryV1::new(4, false);
                registry.insert(f.h.pop());
                let tick = |f: &mut Fixture, registry: &mut operation::OperationRegistryV1<MockBackend>| {
                    operation::advance_operations_v1(
                        &mut f.h.context, registry, 1, 0, |context, stream| context.flush_stream(stream),
                    );
                };
                tick(&mut f, &mut registry);
                f.h.context.release_event(dependency).unwrap();
                backend_ids.push(f.last_id());
                f.complete();
                for _ in 0..DEPTH - 1 {
                    tick(&mut f, &mut registry);
                    assert_eq!(registry.len(), 1);
                    assert!(pending(future.as_mut().unwrap()));
                    assert!(callbacks.lock().unwrap().is_empty());
                }
                f.h.state.lock().unwrap().poll_failures.push_back(
                    RuntimeBackendFailureV1::Quiescent(MockError("deep producer discarded")),
                );
                tick(&mut f, &mut registry);
                assert!(!f.h.context.is_terminal());
                assert_eq!(
                    registry.len(),
                    1,
                    "requested root still needs local settlement"
                );
                assert_eq!(control.phase(), RuntimeAsyncOperationPhaseV1::Observing);
                assert!(pending(future.as_mut().unwrap()));
                let settled = callbacks.lock().unwrap().len();
                assert_eq!(settled, 131);
                for (index, submission) in producers.iter().enumerate() {
                    assert_eq!(
                        f.h.context.query_submission(submission).unwrap(),
                        if index < settled {
                            RuntimeCompletionStatusV1::QuiescentWithoutResult
                        } else {
                            RuntimeCompletionStatusV1::Pending
                        }
                    );
                }
                assert_eq!(
                    f.h.context.version_journal_read_records_v1(),
                    journal.then_some(DEPTH - settled)
                );
                let calls = f.h.state.lock().unwrap().directed_calls.clone();
                assert_eq!(
                    calls
                        .iter()
                        .filter(|(kind, _)| *kind != "submit")
                        .map(|(_, id)| *id)
                        .collect::<Vec<_>>(),
                    backend_ids.iter().rev().copied().collect::<Vec<_>>()
                );
                assert_eq!(f.h.handle.observer().reply_cells_in_use(), 1);
                if observer_end == 1 {
                    drop(future.take());
                    assert_eq!(f.h.handle.observer().reply_cells_in_use(), 1);
                }
                if observer_end == 3 {
                    f.h.context.quarantine_after_async_command_panic_v1();
                    tick(&mut f, &mut registry);
                    assert_eq!(registry.len(), 1);
                    assert!(pending(future.as_mut().unwrap()));
                    assert_eq!(callbacks.lock().unwrap().len(), settled);
                    assert_eq!(
                        f.h.context.version_journal_read_records_v1(),
                        journal.then_some(DEPTH - settled)
                    );
                }
                if observer_end >= 2 {
                    assert!(!registry.stop_observations());
                    assert!(matches!(
                        join_command(future.take().unwrap()),
                        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
                    ));
                    assert_eq!(f.h.handle.observer().reply_cells_in_use(), 0);
                    assert_eq!(
                        control.phase(),
                        RuntimeAsyncOperationPhaseV1::StoppedAfterSubmission
                    );
                    assert_eq!(registry.len(), 1);
                    assert_eq!(f.h.state.lock().unwrap().directed_calls, calls);
                    assert_eq!(f.h.state.lock().unwrap().release_calls, 0);
                    if observer_end == 3 {
                        assert!(!f.h.context.cleanup().is_complete());
                        continue;
                    }
                }
                tick(&mut f, &mut registry);
                assert_eq!(registry.len(), 0);
                assert_eq!(
                    control.phase(),
                    if observer_end == 2 {
                        RuntimeAsyncOperationPhaseV1::StoppedAfterSubmission
                    } else {
                        RuntimeAsyncOperationPhaseV1::ObservationFinished
                    }
                );
                assert!(!f.h.context.is_terminal());
                assert_eq!(f.h.state.lock().unwrap().directed_calls, calls);
                assert_eq!(
                    f.h.context.version_journal_read_records_v1(),
                    journal.then_some(0)
                );
                assert_eq!(
                    *callbacks.lock().unwrap(),
                    (0..DEPTH - 1)
                        .map(|index| (index, RuntimeCompletionStatusV1::QuiescentWithoutResult))
                        .collect::<Vec<_>>()
                );
                if let Some(future) = future {
                    let result = join_command(future).unwrap();
                    assert!(matches!(
                        result.observation,
                        Err(RuntimeErrorV1::BackendQuiescent(MockError(
                            "deep producer discarded"
                        )))
                    ));
                    assert_eq!(result.rejected_observations, 0);
                    let mut root = result.submission.unwrap();
                    assert_eq!(
                        f.h.context.query_submission(&root).unwrap(),
                        RuntimeCompletionStatusV1::QuiescentWithoutResult
                    );
                    assert_eq!(
                        f.h.context.poll(&mut root).unwrap(),
                        crate::RuntimePollV1::Failed { code: -3 }
                    );
                    assert_eq!(f.h.state.lock().unwrap().directed_calls, calls);
                }
                assert!(f.h.state.lock().unwrap().flush_calls.is_empty());
                assert_eq!(f.h.state.lock().unwrap().release_calls, 0);
                assert_eq!(f.h.handle.observer().reply_cells_in_use(), 0);
                assert!(f.h.context.cleanup().is_complete());
            }
        }
    }
}
