use super::*;

type Early = RuntimeAsyncEventOperationV1<RuntimeDirectedScalarPeerCopyV1, MockError>;

fn enqueue(f: &Fixture, source: usize, destination: usize, deps: Vec<RuntimeEventIdV1>) -> Early {
    f.h.handle
        .directed_peer_copy_with_event(
            f.streams[destination % 2],
            f.region(source, false),
            f.region(destination, true),
            deps,
        )
        .unwrap()
}

fn admit(
    f: &mut Fixture,
    operation: Early,
) -> (
    RuntimeEventIdV1,
    RuntimeAsyncTrackedOperationV1<RuntimeDirectedScalarPeerCopyV1, MockError>,
    Box<dyn operation::EngineOperationV1<MockBackend>>,
) {
    let Early {
        mut event,
        mut operation,
    } = operation;
    let mut driver = f.h.pop();
    assert_eq!(driver.stream(), None);
    let calls = f.h.state.lock().unwrap().event_record_calls;
    assert!(!driver.advance(&mut f.h.context));
    assert!(pending(&mut event));
    assert!(pending(&mut operation));
    assert_eq!(f.h.state.lock().unwrap().event_record_calls, calls);
    let backend_calls = f.h.state.lock().unwrap().directed_calls.clone();
    assert!(!driver.advance(&mut f.h.context));
    assert!(pending(&mut operation));
    let event = join_command(event).unwrap().unwrap();
    assert_eq!(
        f.h.context.query_event(event).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert_eq!(f.h.state.lock().unwrap().event_record_calls, calls + 1);
    assert_eq!(f.h.state.lock().unwrap().directed_calls, backend_calls);
    (event, operation, driver)
}

#[test]
fn early_event_async_diamond_admits_every_consumer_before_producer_completion() {
    for journal in [false, true] {
        let mut f = Fixture::new(journal);
        let root = enqueue(&f, 0, 1, vec![]);
        let (root_event, root, root_driver) = admit(&mut f, root);
        let left = enqueue(&f, 1, 2, vec![root_event]);
        let (left_event, left, left_driver) = admit(&mut f, left);
        let right = enqueue(&f, 1, 4, vec![root_event]);
        let (right_event, right, right_driver) = admit(&mut f, right);
        f.h.context.release_event(root_event).unwrap();
        let join = enqueue(&f, 2, 3, vec![left_event, right_event]);
        let (join_event, join, join_driver) = admit(&mut f, join);
        for event in [left_event, right_event, join_event] {
            f.h.context.release_event(event).unwrap();
        }
        {
            let state = f.h.state.lock().unwrap();
            assert_eq!(state.directed_calls.len(), 4);
            assert!(
                state
                    .directed_calls
                    .iter()
                    .all(|(kind, _)| *kind == "submit")
            );
            assert!(
                state
                    .statuses
                    .values()
                    .all(|s| *s == BackendPollV1::Pending)
            );
            assert!(state.flush_calls.is_empty());
        }
        let mut registry = operation::OperationRegistryV1::new(4, true);
        for driver in [join_driver, right_driver, left_driver, root_driver] {
            registry.insert(driver);
        }
        f.complete();
        for _ in 0..8 {
            f.tick(&mut registry);
        }
        assert_eq!(registry.len(), 0);
        for operation in [root, left, right, join] {
            assert_eq!(
                join_command(operation).unwrap().observation.unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
        }
        assert_eq!(f.h.used(), 0);
        assert_eq!(f.h.handle.observer.reply_budget.used(), 0);
        assert!(f.h.state.lock().unwrap().flush_calls.is_empty());
        assert!(f.h.context.cleanup().is_complete());
    }
}

#[test]
fn early_event_enqueue_does_not_pin_dependency_until_consumer_admission() {
    let mut f = Fixture::new(true);
    let root = enqueue(&f, 0, 1, vec![]);
    let (event, root, root_driver) = admit(&mut f, root);
    let consumer = enqueue(&f, 1, 2, vec![event]);
    f.h.context.release_event(event).unwrap();
    let mut driver = f.h.pop();
    assert!(driver.advance(&mut f.h.context));
    assert!(matches!(
        join_command(consumer.event).unwrap(),
        Err(RuntimeAsyncOperationEventErrorV1::SubmissionUnavailable)
    ));
    assert!(matches!(
        join_command(consumer.operation).unwrap().observation,
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownEvent
        ))
    ));
    assert_eq!(f.h.state.lock().unwrap().directed_calls.len(), 1);
    drop((root, root_driver));
    f.complete();
    assert!(f.h.context.cleanup().is_complete());
}

#[test]
fn early_event_cancellation_and_stop_resolve_both_receipts_without_recording() {
    for materialized in [false, true] {
        for cancel in [false, true] {
            let mut f = Fixture::new(true);
            let early = enqueue(&f, 0, 1, vec![]);
            let control = early.operation.control();
            if cancel {
                assert_eq!(
                    control.cancel_before_submission(),
                    RuntimeAsyncCancelResultV1::CancelledBeforeSubmission
                );
            }
            if materialized {
                let mut driver = f.h.pop();
                if cancel {
                    assert!(driver.advance(&mut f.h.context));
                } else {
                    driver.reject(RuntimeAsyncEngineCallErrorV1::EngineStopped);
                }
            } else {
                drop(f.h.pop_factory());
            }
            let expected = if cancel {
                RuntimeAsyncEngineCallErrorV1::CancelledBeforeSubmission
            } else {
                RuntimeAsyncEngineCallErrorV1::EngineStopped
            };
            assert!(matches!(join_command(early.event), Err(error) if error == expected));
            assert!(matches!(join_command(early.operation), Err(error) if error == expected));
            assert_eq!(f.h.handle.observer.reply_budget.used(), 0);
            assert_eq!(f.h.state.lock().unwrap().event_record_calls, 0);
            assert!(f.h.state.lock().unwrap().directed_calls.is_empty());
        }
    }
}

#[test]
fn early_event_stop_after_submission_retains_custody_without_event() {
    let mut f = Fixture::new(true);
    let early = enqueue(&f, 0, 1, vec![]);
    let mut registry = operation::OperationRegistryV1::new(1, true);
    registry.insert(f.h.pop());
    f.tick(&mut registry);
    assert!(matches!(
        early.operation.control().cancel_before_submission(),
        RuntimeAsyncCancelResultV1::NotCancellable(RuntimeAsyncOperationPhaseV1::Observing)
    ));
    assert!(!registry.stop_observations());
    assert!(matches!(
        join_command(early.event),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    assert!(matches!(
        join_command(early.operation),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    assert_eq!(registry.len(), 1);
    assert_eq!(f.h.state.lock().unwrap().event_record_calls, 0);
    assert_eq!(f.h.state.lock().unwrap().release_calls, 0);
    assert_eq!(f.h.context.async_drain_counts_v1().pending, 1);
    f.complete();
    assert!(f.h.context.cleanup().is_complete());
}

#[test]
fn early_event_submit_failure_preserves_original_error_and_never_records() {
    for terminal in [false, true] {
        let mut f = Fixture::new(true);
        f.h.state
            .lock()
            .unwrap()
            .submit_failures
            .push_back(if terminal {
                RuntimeBackendFailureV1::Terminal(MockError("exact submit error"))
            } else {
                RuntimeBackendFailureV1::Rejected(MockError("exact submit error"))
            });
        let early = enqueue(&f, 0, 1, vec![]);
        let mut driver = f.h.pop();
        assert!(driver.advance(&mut f.h.context));
        assert!(matches!(
            join_command(early.event).unwrap(),
            Err(RuntimeAsyncOperationEventErrorV1::SubmissionUnavailable)
        ));
        let result = join_command(early.operation).unwrap();
        assert!(result.submission.is_none());
        assert!(
            matches!(result.observation, Err(RuntimeErrorV1::BackendRejected(MockError("exact submit error"))) if !terminal)
                || matches!(result.observation, Err(RuntimeErrorV1::BackendTerminal(MockError("exact submit error"))) if terminal)
        );
        assert_eq!(f.h.context.is_terminal(), terminal);
        assert_eq!(f.h.state.lock().unwrap().event_record_calls, 0);
    }
}

#[test]
fn early_event_record_failure_does_not_resubmit_or_launder_completion() {
    for mode in 0..3 {
        let mut f = Fixture::new(true);
        let early = enqueue(&f, 0, 1, vec![]);
        let mut registry = operation::OperationRegistryV1::new(1, true);
        registry.insert(f.h.pop());
        f.tick(&mut registry);
        f.h.state
            .lock()
            .unwrap()
            .event_record_failures
            .push_back(match mode {
                0 => RuntimeBackendFailureV1::Rejected(MockError("exact event error")),
                1 => RuntimeBackendFailureV1::Quiescent(MockError("exact event error")),
                _ => RuntimeBackendFailureV1::Terminal(MockError("exact event error")),
            });
        f.tick(&mut registry);
        let error = join_command(early.event).unwrap().unwrap_err();
        assert!(if mode == 0 {
            matches!(
                error,
                RuntimeAsyncOperationEventErrorV1::RecordingFailed(
                    RuntimeErrorV1::BackendRejected(MockError("exact event error"))
                )
            )
        } else if mode == 1 {
            matches!(
                error,
                RuntimeAsyncOperationEventErrorV1::RecordingFailed(
                    RuntimeErrorV1::BackendQuiescent(MockError("exact event error"))
                )
            )
        } else {
            matches!(
                error,
                RuntimeAsyncOperationEventErrorV1::RecordingFailed(
                    RuntimeErrorV1::BackendTerminal(MockError("exact event error"))
                )
            )
        });
        assert_eq!(f.h.state.lock().unwrap().directed_calls.len(), 1);
        assert_eq!(f.h.state.lock().unwrap().event_record_calls, 1);
        assert_eq!(f.h.state.lock().unwrap().release_calls, 0);
        if mode == 2 {
            assert!(f.h.context.is_terminal());
            assert!(!registry.stop_observations());
            assert!(matches!(
                join_command(early.operation),
                Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
            ));
            assert_eq!(registry.len(), 1);
        } else {
            assert!(!f.h.context.is_terminal());
            f.complete();
            f.tick(&mut registry);
            assert_eq!(registry.len(), 0);
            assert_eq!(
                join_command(early.operation).unwrap().observation.unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
            assert_eq!(f.h.state.lock().unwrap().event_record_calls, 1);
            assert!(f.h.context.cleanup().is_complete());
        }
    }
}

#[test]
fn early_event_record_panic_preserves_driver_and_submitted_custody() {
    let mut f = Fixture::new(true);
    let early = enqueue(&f, 0, 1, vec![]);
    let mut registry = operation::OperationRegistryV1::new(1, true);
    registry.insert(f.h.pop());
    f.tick(&mut registry);
    f.h.state.lock().unwrap().panic_on_event_record = true;
    f.tick(&mut registry);
    assert!(f.h.context.is_terminal());
    assert_eq!(registry.len(), 1);
    assert!(matches!(
        join_command(early.event),
        Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
    ));
    assert!(matches!(
        join_command(early.operation),
        Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
    ));
    assert_eq!(f.h.state.lock().unwrap().release_calls, 0);
}

#[test]
fn early_event_invalid_backend_event_seals_context_without_success_receipt() {
    let mut f = Fixture::new(true);
    let early = enqueue(&f, 0, 1, vec![]);
    let mut registry = operation::OperationRegistryV1::new(1, true);
    registry.insert(f.h.pop());
    f.tick(&mut registry);
    f.h.state.lock().unwrap().event_record_override = Some(0);
    f.tick(&mut registry);
    assert!(matches!(
        join_command(early.event).unwrap(),
        Err(RuntimeAsyncOperationEventErrorV1::RecordingFailed(
            RuntimeErrorV1::BackendProtocol(_)
        ))
    ));
    assert!(f.h.context.is_terminal());
    assert_eq!(registry.len(), 1);
    assert!(!registry.stop_observations());
    assert!(matches!(
        join_command(early.operation),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    assert_eq!(f.h.state.lock().unwrap().release_calls, 0);
    assert_eq!(f.h.state.lock().unwrap().event_release_calls, 0);
}

#[test]
fn early_event_stop_after_receipt_does_not_revoke_recorded_event() {
    let mut f = Fixture::new(true);
    let early = enqueue(&f, 0, 1, vec![]);
    let (event, completion, mut driver) = admit(&mut f, early);
    assert!(matches!(
        completion.control().cancel_before_submission(),
        RuntimeAsyncCancelResultV1::NotCancellable(RuntimeAsyncOperationPhaseV1::Observing)
    ));
    driver.reject(RuntimeAsyncEngineCallErrorV1::EngineStopped);
    assert!(matches!(
        join_command(completion),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    assert_eq!(
        f.h.context.query_event(event).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert_eq!(f.h.state.lock().unwrap().event_record_calls, 1);
    assert_eq!(f.h.state.lock().unwrap().event_release_calls, 0);
    f.complete();
    assert!(f.h.context.cleanup().is_complete());
    assert_eq!(f.h.state.lock().unwrap().event_release_calls, 1);
}

#[test]
fn early_event_operation_capacity_rejects_both_observers_before_submission() {
    let mut f = Fixture::new(true);
    let first = enqueue(&f, 0, 1, vec![]);
    let second = enqueue(&f, 2, 3, vec![]);
    let mut registry = operation::OperationRegistryV1::new(1, true);
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
        join_command(second.event),
        Err(RuntimeAsyncEngineCallErrorV1::OperationCapacity)
    ));
    assert!(matches!(
        join_command(second.operation),
        Err(RuntimeAsyncEngineCallErrorV1::OperationCapacity)
    ));
    assert_eq!(f.h.state.lock().unwrap().directed_calls.len(), 1);
    f.tick(&mut registry);
    let event = join_command(first.event).unwrap().unwrap();
    f.complete();
    f.tick(&mut registry);
    assert_eq!(
        join_command(first.operation).unwrap().observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    f.h.context.release_event(event).unwrap();
    assert_eq!(f.h.handle.observer.reply_budget.used(), 0);
    scheduler.finish(&mut f.h.context, &mut registry);
    assert!(f.h.context.cleanup().is_complete());
}

#[test]
fn early_event_drop_observers_does_not_cancel_event_or_operation() {
    let mut f = Fixture::new(true);
    let early = enqueue(&f, 0, 1, vec![]);
    drop(early);
    assert_eq!(f.h.handle.observer.reply_budget.used(), 2);
    let mut registry = operation::OperationRegistryV1::new(1, true);
    registry.insert(f.h.pop());
    f.tick(&mut registry);
    f.tick(&mut registry);
    assert_eq!(f.h.handle.observer.reply_budget.used(), 1);
    assert_eq!(f.h.state.lock().unwrap().event_record_calls, 1);
    assert_eq!(f.h.state.lock().unwrap().event_release_calls, 0);
    f.complete();
    f.tick(&mut registry);
    assert_eq!(registry.len(), 0);
    assert_eq!(f.h.handle.observer.reply_budget.used(), 0);
    assert_eq!(f.h.state.lock().unwrap().event_release_calls, 0);
    assert_eq!(f.h.state.lock().unwrap().release_calls, 0);
    assert!(f.h.context.cleanup().is_complete());
    assert_eq!(f.h.state.lock().unwrap().event_release_calls, 1);
}

#[test]
fn early_event_reply_and_command_capacity_roll_back_both_cells_and_snapshot() {
    for capacity in [0, 1, 2] {
        let mut f = Fixture::new(true);
        f.h.handle.observer.reply_budget = reply_budget::ReplyBudgetV1::new(capacity);
        let (_, dependency, _) = f.producer(0, 1, &[]);
        let result = f.h.handle.directed_peer_copy_with_event(
            f.streams[0],
            f.region(1, false),
            f.region(2, true),
            vec![dependency],
        );
        if capacity < 2 {
            assert!(matches!(
                result,
                Err(RuntimeAsyncEngineCallErrorV1::ReplyCapacity)
            ));
            assert!(f.h.receiver.try_recv().is_err());
        } else {
            let early = result.unwrap();
            assert_eq!(f.h.handle.observer.reply_budget.used(), 2);
            drop(early);
            drop(f.h.pop_factory());
        }
        assert_eq!(f.h.handle.observer.reply_budget.used(), 0);
        assert_eq!(f.h.used(), 0);
    }
    let mut h = Harness::new(4096, 0);
    assert!(matches!(
        h.handle.enqueue_launch_with_event(h.request()),
        Err(RuntimeAsyncEngineCallErrorV1::CommandQueueFull)
    ));
    assert_eq!(h.used(), 0);
    assert_eq!(h.handle.observer.reply_budget.used(), 0);
    assert!(h.context.cleanup().is_complete());
}

#[test]
fn early_event_frozen_typed_launch_and_same_device_copy_use_shared_lifecycle() {
    let mut h = Harness::new(4096, 4);
    let request = h.request();
    let early = h.handle.enqueue_launch_with_event(request).unwrap();
    let mut driver = h.pop();
    assert_eq!(driver.stream(), Some(h.stream));
    assert!(!driver.advance(&mut h.context));
    assert!(!driver.advance(&mut h.context));
    let event = join_command(early.event).unwrap().unwrap();
    assert_eq!(h.args.encodes.load(Ordering::SeqCst), 1);
    assert_eq!(h.args.bindings.load(Ordering::SeqCst), 1);
    assert_eq!(h.used(), 0);
    let device = h.context.devices()[0].id();
    let allocation = h
        .context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 32, 8)
        .unwrap();
    let source = RuntimeMemoryRegionV1 {
        allocation,
        byte_offset: 0,
        byte_len: 8,
        access: RuntimeAccessV1::Read,
    };
    let destination = RuntimeMemoryRegionV1 {
        allocation: h
            .context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 32, 8)
            .unwrap(),
        byte_offset: 16,
        access: RuntimeAccessV1::Write,
        ..source
    };
    let copy = h
        .handle
        .copy_async_with_event(h.stream, source, destination, vec![event])
        .unwrap();
    let mut copy_driver = h.pop();
    assert!(!copy_driver.advance(&mut h.context));
    assert!(!copy_driver.advance(&mut h.context));
    let copy_event = join_command(copy.event).unwrap().unwrap();
    h.context.release_event(event).unwrap();
    h.context.release_event(copy_event).unwrap();
    for status in h.state.lock().unwrap().statuses.values_mut() {
        *status = BackendPollV1::Succeeded;
    }
    assert!(driver.advance(&mut h.context));
    assert!(copy_driver.advance(&mut h.context));
    assert_eq!(
        join_command(early.operation).unwrap().observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(
        join_command(copy.operation).unwrap().observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert!(h.context.cleanup().is_complete());
}

#[test]
fn early_event_unpolled_receipt_survives_stop_and_disconnected_enqueue_refunds() {
    let mut f = Fixture::new(true);
    let early = enqueue(&f, 0, 1, vec![]);
    let mut driver = f.h.pop();
    assert!(!driver.advance(&mut f.h.context));
    assert!(!driver.advance(&mut f.h.context));
    driver.reject(RuntimeAsyncEngineCallErrorV1::EngineStopped);
    let event = join_command(early.event).unwrap().unwrap();
    assert!(matches!(
        join_command(early.operation),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    assert_eq!(
        f.h.context.query_event(event).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert_eq!(f.h.state.lock().unwrap().event_release_calls, 0);
    f.complete();
    assert!(f.h.context.cleanup().is_complete());
    let h = Harness::new(4096, 4);
    let request = h.request();
    drop(h.receiver);
    assert!(matches!(
        h.handle.enqueue_launch_with_event(request),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    assert_eq!(h.handle.observer().snapshot_bytes_in_use(), 0);
    assert_eq!(h.handle.observer().reply_cells_in_use(), 0);
    assert!(h.state.lock().unwrap().issues.is_empty());
}
