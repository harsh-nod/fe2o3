use super::*;

fn request(
    f: &Fixture,
    tracked: bool,
    events: Vec<RuntimeEventIdV1>,
) -> Result<CopyFuture, RuntimeAsyncEngineCallErrorV1> {
    if tracked {
        f.h.handle
            .directed_peer_copy_tracked(f.streams[0], f.region(1, false), f.region(2, true), events)
            .map(|operation| operation.future)
    } else {
        f.h.handle
            .directed_peer_copy(f.streams[0], f.region(1, false), f.region(2, true), events)
    }
}

#[test]
fn directed_async_capacity_channel_and_reentrant_refunds_cover_both_methods() {
    for tracked in [false, true] {
        for boundary in 0..5 {
            let mut f = Fixture::new(true);
            let (_, event, _) = f.producer(0, 1, &[]);
            let mut held_reply = None;
            let expected = match boundary {
                0 => {
                    f.h.handle.observer.snapshot_budget = snapshot::SnapshotBudgetV1::new(1);
                    RuntimeAsyncEngineCallErrorV1::SnapshotCapacity
                }
                1 => {
                    f.h.handle.observer.reply_budget = reply_budget::ReplyBudgetV1::new(1);
                    held_reply = Some(f.h.handle.observer.reply_budget.reserve().unwrap());
                    RuntimeAsyncEngineCallErrorV1::ReplyCapacity
                }
                2 => {
                    for _ in 0..8 {
                        f.h.handle
                            .observer
                            .sender
                            .try_send(RuntimeAsyncEngineCommandV1::Stop)
                            .unwrap_or_else(|_| panic!("empty channel"));
                    }
                    RuntimeAsyncEngineCallErrorV1::CommandQueueFull
                }
                3 => {
                    let (sender, receiver) = sync_channel(1);
                    f.h.handle.observer.sender = sender;
                    drop(receiver);
                    RuntimeAsyncEngineCallErrorV1::EngineStopped
                }
                4 => {
                    f.h.handle
                        .observer
                        .worker_thread
                        .set(thread::current().id())
                        .unwrap();
                    f.h.handle.observer.local_active = Some(Arc::new(AtomicBool::new(true)));
                    RuntimeAsyncEngineCallErrorV1::ReentrantCall
                }
                _ => unreachable!(),
            };
            assert_eq!(request(&f, tracked, vec![event]).err(), Some(expected));
            assert_eq!(f.h.used(), 0);
            assert_eq!(
                f.h.handle.observer().reply_cells_in_use(),
                usize::from(held_reply.is_some())
            );
            drop(held_reply);
            assert_eq!(f.h.handle.observer().reply_cells_in_use(), 0);
            assert_eq!(f.h.state.lock().unwrap().directed_calls.len(), 1);
            if boundary != 2 {
                assert!(f.h.receiver.try_recv().is_err());
            }
            f.complete();
            assert!(f.h.context.cleanup().is_complete());
        }
    }
}

#[test]
fn directed_async_registry_capacity_refunds_second_snapshot_without_submission() {
    let mut f = Fixture::new(true);
    let (_, event, _) = f.producer(0, 1, &[]);
    let (first, _) = f.enqueue(1, 2, vec![event], false);
    let (second, _) = f.enqueue(1, 4, vec![event], true);
    let mut registry = operation::OperationRegistryV1::new(1, false);
    let mut scheduler =
        scheduler::SchedulerV1::new(f.h.handle.observer.admission.clone(), true, false);
    let config = RuntimeAsyncEngineConfigV1::new(8, 1, 8, 1, Duration::from_millis(1)).unwrap();
    let mode = RuntimeAsyncProgressModeV1 {
        config: RuntimeAsyncProgressConfigV1::new(8, 8).unwrap(),
        flush_stream: |context, stream| context.flush_stream(stream),
    };
    scheduler.tick(
        &mut f.h.context,
        &mut registry,
        &f.h.receiver,
        config,
        Some(&mode),
        Duration::ZERO,
    );
    assert_eq!(registry.len(), 1);
    assert!(matches!(
        join_command(second),
        Err(RuntimeAsyncEngineCallErrorV1::OperationCapacity)
    ));
    assert_eq!(f.h.used(), 0);
    assert_eq!(f.h.state.lock().unwrap().directed_calls.len(), 2);
    f.complete();
    f.tick(&mut registry);
    f.tick(&mut registry);
    assert_eq!(
        join_command(first).unwrap().observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    scheduler.finish(&mut f.h.context, &mut registry);
    assert!(f.h.context.cleanup().is_complete());
}

#[test]
fn directed_async_submit_errors_refund_snapshot_and_keep_uncertain_custody() {
    for tracked in [false, true] {
        for failure in 0..4 {
            let mut f = Fixture::new(true);
            let (_, event, _) = f.producer(0, 1, &[]);
            let (future, control) = f.enqueue(1, 2, vec![event], tracked);
            let mut registry = operation::OperationRegistryV1::new(4, true);
            registry.insert(f.h.pop());
            let error = MockError("submit diagnostic");
            if failure == 3 {
                f.h.state.lock().unwrap().panic_on_submit = true;
            } else {
                f.h.state
                    .lock()
                    .unwrap()
                    .submit_failures
                    .push_back(match failure {
                        0 => RuntimeBackendFailureV1::Rejected(error),
                        1 => RuntimeBackendFailureV1::Quiescent(error),
                        _ => RuntimeBackendFailureV1::Terminal(error),
                    });
            }
            f.tick(&mut registry);
            assert_eq!(f.h.used(), 0);
            assert_eq!(f.h.state.lock().unwrap().directed_calls.len(), 1);
            assert_eq!(f.h.state.lock().unwrap().release_calls, 0);
            assert_eq!(f.h.context.is_terminal(), failure >= 2);
            assert_eq!(registry.len(), usize::from(failure >= 2));
            if failure == 3 {
                assert!(matches!(
                    join_command(future),
                    Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
                ));
                if let Some(control) = control {
                    assert_eq!(
                        control.phase(),
                        RuntimeAsyncOperationPhaseV1::StoppedAfterSubmission
                    );
                }
            } else {
                let result = join_command(future).unwrap();
                assert!(result.submission.is_none());
                assert!(matches!(
                    (failure, result.observation),
                    (
                        0,
                        Err(RuntimeErrorV1::BackendRejected(MockError(
                            "submit diagnostic"
                        )))
                    ) | (
                        1,
                        Err(RuntimeErrorV1::BackendQuiescent(MockError(
                            "submit diagnostic"
                        )))
                    ) | (
                        2,
                        Err(RuntimeErrorV1::BackendTerminal(MockError(
                            "submit diagnostic"
                        )))
                    )
                ));
                if let Some(control) = control {
                    assert_eq!(
                        control.phase(),
                        RuntimeAsyncOperationPhaseV1::ObservationFinished
                    );
                }
            }
            assert_eq!(f.h.handle.observer().reply_cells_in_use(), 0);
            assert_eq!(
                f.h.context.version_journal_read_records_v1(),
                Some(if failure >= 2 { 2 } else { 1 })
            );
            if failure < 2 {
                f.complete();
                assert!(f.h.context.cleanup().is_complete());
            }
        }
    }
}

#[test]
fn directed_async_alias_and_stale_event_reject_on_owner_before_backend_entry() {
    for alias in [false, true] {
        let mut f = Fixture::new(true);
        let (producer, event, _) = f.producer(0, 1, &[]);
        let events = if alias {
            vec![event, f.h.context.record_event(&producer).unwrap()]
        } else {
            vec![event]
        };
        let (future, _) = f.enqueue(1, 2, events, true);
        if !alias {
            f.h.context.release_event(event).unwrap();
        }
        assert!(f.h.pop().advance(&mut f.h.context));
        let result = join_command(future).unwrap();
        assert!(result.submission.is_none());
        assert!(matches!(
            (alias, result.observation),
            (
                true,
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::DuplicateDependency
                ))
            ) | (
                false,
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::UnknownEvent
                ))
            )
        ));
        assert_eq!(f.h.used(), 0);
        assert_eq!(f.h.state.lock().unwrap().directed_calls.len(), 1);
        assert!(!f.h.context.is_terminal());
        f.complete();
        assert!(f.h.context.cleanup().is_complete());
    }
}

#[test]
fn directed_async_stop_before_and_after_retained_success_is_observation_only() {
    for advances in 0..3 {
        let mut f = Fixture::new(true);
        let (_, event, _) = f.producer(0, 1, &[]);
        let (future, control) = f.enqueue(1, 2, vec![event], true);
        let mut driver = f.h.pop();
        for _ in 0..advances {
            assert!(!driver.advance(&mut f.h.context));
            f.complete();
        }
        driver.reject(RuntimeAsyncEngineCallErrorV1::EngineStopped);
        drop(driver);
        assert!(matches!(
            join_command(future),
            Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
        ));
        assert_eq!(
            control.unwrap().phase(),
            if advances == 0 {
                RuntimeAsyncOperationPhaseV1::StoppedBeforeSubmission
            } else {
                RuntimeAsyncOperationPhaseV1::StoppedAfterSubmission
            }
        );
        assert_eq!(f.h.used(), 0);
        assert_eq!(f.h.state.lock().unwrap().release_calls, 0);
        assert_eq!(
            f.h.context.version_journal_read_records_v1(),
            Some(if advances == 0 { 1 } else { 2 })
        );
        f.complete();
        assert!(f.h.context.cleanup().is_complete());
    }
}

#[test]
fn directed_async_timeout_recovers_same_operation_after_pending_submission() {
    let mut f = Fixture::new(true);
    let (_, event, _) = f.producer(0, 1, &[]);
    let operation =
        f.h.handle
            .directed_peer_copy_tracked(
                f.streams[0],
                f.region(1, false),
                f.region(2, true),
                vec![event],
            )
            .unwrap();
    let control = operation.control();
    let mut driver = f.h.pop();
    assert!(!driver.advance(&mut f.h.context));
    let timed = futures_executor::block_on(operation.observe_with_timeout(std::future::ready(())));
    let RuntimeAsyncTimeoutResultV1::TimedOut { operation } = timed else {
        panic!("pending operation must time out")
    };
    assert!(control.same_operation(&operation.control()));
    assert_eq!(control.phase(), RuntimeAsyncOperationPhaseV1::Observing);
    f.complete();
    assert!(!driver.advance(&mut f.h.context));
    drop(operation);
    assert!(driver.advance(&mut f.h.context));
    assert_eq!(
        control.phase(),
        RuntimeAsyncOperationPhaseV1::ObservationFinished
    );
    assert_eq!(f.h.state.lock().unwrap().directed_calls.len(), 4);
    assert_eq!(f.h.state.lock().unwrap().release_calls, 0);
    assert!(f.h.context.cleanup().is_complete());
}

#[test]
fn directed_async_retained_success_cannot_promote_failed_or_discarded_producer() {
    for outcome in 0..3 {
        let mut f = Fixture::new(true);
        let (_, event, producer) = f.producer(0, 1, &[]);
        let (future, _) = f.enqueue(1, 2, vec![event], true);
        let mut registry = operation::OperationRegistryV1::new(4, true);
        registry.insert(f.h.pop());
        f.tick(&mut registry);
        f.complete();
        f.tick(&mut registry);
        {
            let mut state = f.h.state.lock().unwrap();
            if outcome == 2 {
                state
                    .poll_failures
                    .push_back(RuntimeBackendFailureV1::Quiescent(MockError(
                        "producer discarded",
                    )));
            } else {
                state.statuses.insert(
                    producer,
                    if outcome == 0 {
                        BackendPollV1::Pending
                    } else {
                        BackendPollV1::Failed { code: 9 }
                    },
                );
            }
        }
        f.tick(&mut registry);
        let result = join_command(future).unwrap();
        assert_eq!(f.h.context.is_terminal(), outcome != 2);
        assert_eq!(registry.len(), usize::from(outcome != 2));
        if outcome == 2 {
            assert!(matches!(
                result.observation,
                Err(RuntimeErrorV1::BackendQuiescent(MockError(
                    "producer discarded"
                )))
            ));
            assert_eq!(
                f.h.context
                    .query_submission(&result.submission.unwrap())
                    .unwrap(),
                RuntimeCompletionStatusV1::QuiescentWithoutResult
            );
            assert_eq!(f.h.context.version_journal_read_records_v1(), Some(0));
            assert!(f.h.context.cleanup().is_complete());
        } else {
            assert!(matches!(
                result.observation,
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::InvalidBackendDescription
                ))
            ));
            assert!(result.submission.is_some());
            assert_eq!(f.h.state.lock().unwrap().release_calls, 0);
            assert!(!registry.stop_observations());
        }
        assert_eq!(f.h.state.lock().unwrap().directed_calls.len(), 4);
    }
}
