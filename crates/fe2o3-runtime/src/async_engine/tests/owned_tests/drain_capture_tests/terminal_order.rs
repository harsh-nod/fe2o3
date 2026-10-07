use super::*;

#[test]
fn drn3b_active_graph_releases_exact_reservation_before_capture_outcomes() {
    for outcome in [
        PrefixCaptureOutcome::Success,
        PrefixCaptureOutcome::Rejected,
        PrefixCaptureOutcome::Terminal,
    ] {
        let state = Arc::new(Mutex::new(MockState::default()));
        let trace = Arc::new(Mutex::new(outcome.trace()));
        let (engine, handle) = start_with_config(state.clone(), trace.clone(), capture_config(16));
        let mut prefix = CapturePrefix::new(&handle, &state);
        let (request, context_id, graph_id) = prefix.graph_request(&handle);
        let mut graph = Box::pin(handle.submit_graph(request).unwrap());
        let deadline = Instant::now() + Duration::from_secs(5);
        while state.lock().unwrap().issues.is_empty() {
            assert!(Instant::now() < deadline, "graph prefix was not issued");
            thread::yield_now();
        }
        let release = paused_owner(&handle);
        assert_eq!(state.lock().unwrap().issues.len(), 1);
        assert!(handle.observer.graph_slot.load(Ordering::Acquire));
        let graph_wakes = arm_capture_future(graph.as_mut());
        let mut capture = Box::pin(
            handle
                .begin_drain_with_capture(
                    128,
                    prefix.source.take().unwrap(),
                    vec![0xcc; 16].into_boxed_slice(),
                )
                .unwrap(),
        );
        let capture_wakes = arm_capture_future(capture.as_mut());
        assert_eq!(handle.observer().reply_cells_in_use(), 3);
        assert_eq!(handle.observer().drain_capture_bytes_in_use(), 16);
        assert_eq!(trace.lock().unwrap().capture_calls, 0);
        assert!(matches!(
            handle.observer().enqueue_with_context(|_| ()),
            Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
        ));
        state.lock().unwrap().complete_on_flush = true;
        release.wait();
        let captured = outcome.check(await_capture_future(capture.as_mut(), &capture_wakes), 0);
        let graph_report = await_capture_future(graph.as_mut(), &graph_wakes)
            .unwrap()
            .unwrap();
        assert_eq!(graph_report.execution.context(), context_id);
        assert_eq!(graph_report.execution.graph_identity(), graph_id);
        assert_ne!(graph_report.execution.generation(), 0);
        assert_eq!(
            graph_report.observations,
            vec![
                (capture_node(1), RuntimeCompletionStatusV1::Succeeded),
                (capture_node(4), RuntimeCompletionStatusV1::Succeeded),
            ]
        );
        assert!(graph_report.errors.is_empty());
        assert_eq!(graph_report.rejected_observations, 0);
        assert_eq!(graph_report.rejected_releases, 0);
        assert!(!handle.observer.graph_slot.load(Ordering::Acquire));
        assert_capture_shutdown(engine, outcome, 0);
        assert_eq!(graph_wakes.0.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(capture_wakes.0.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(handle.observer().reply_cells_in_use(), 2);
        assert_eq!(handle.observer().snapshot_bytes_in_use(), 0);
        drop((capture, graph));
        assert_eq!(handle.observer().reply_cells_in_use(), 0);
        assert_eq!(
            handle.observer().drain_capture_bytes_in_use(),
            if captured.is_some() { 16 } else { 0 }
        );
        drop(captured);
        assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
        let state = state.lock().unwrap();
        assert_eq!(state.issues.len(), 2);
        assert_eq!(state.issues[0].0, prefix.native_streams[0]);
        assert_eq!(state.issues[1].0, prefix.native_streams[1]);
        assert_ne!(state.issues[0].1, state.issues[1].1);
        assert!(state.statuses.is_empty());
        assert_eq!(state.release_calls, 2);
        let trace = trace.lock().unwrap();
        assert_eq!(trace.capture_calls, 1);
        assert_eq!(
            trace.capture_requests,
            vec![(1, prefix.native_source, 8, 16)]
        );
        assert_eq!(trace.capture_submission_rosters, vec![vec![]]);
        assert_eq!(
            trace.released_submissions,
            vec![state.issues[0].1, state.issues[1].1]
        );
        assert_eq!(
            trace
                .calls
                .iter()
                .filter(|(call, _)| *call == "submit_v1")
                .count(),
            2
        );
        let captured_at = trace
            .calls
            .iter()
            .position(|(call, _)| *call == "capture")
            .unwrap();
        let retired_at = trace
            .calls
            .iter()
            .rposition(|(call, _)| *call == "release_submission_v1")
            .unwrap();
        assert!(retired_at < captured_at);
        if matches!(outcome, PrefixCaptureOutcome::Terminal) {
            assert!(
                !trace.calls.iter().any(|(call, _)| matches!(
                    *call,
                    "release_allocation_v1" | "finalize" | "drop"
                ))
            );
        } else {
            let freed_at = trace
                .calls
                .iter()
                .position(|(call, _)| *call == "release_allocation_v1")
                .unwrap();
            assert!(captured_at < freed_at);
        }
    }
}

#[test]
fn drn3b_observation_rejection_retains_active_prefix_and_suppresses_capture() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let config = RuntimeAsyncEngineConfigV1::new(8, 8, 8, 1, Duration::from_millis(1))
        .unwrap()
        .with_drain_capture_byte_capacity(16)
        .unwrap();
    let (engine, handle) = start_with_config(state.clone(), trace.clone(), config);
    let mut prefix = CapturePrefix::new(&handle, &state);
    let (_, event) = prefix.raw_predecessor(&handle);
    let mut waiter = Box::pin(handle.observer().event_future(event).unwrap());
    let waiter_wakes = arm_capture_future(waiter.as_mut());
    let release = paused_owner(&handle);
    let initial_polls = state.lock().unwrap().poll_calls;
    state.lock().unwrap().poll_failures.extend(
        (0..64).map(|_| RuntimeBackendFailureV1::Rejected(MockError("capture-prefix-observation"))),
    );
    let request = prefix.request(1, vec![event]);
    let bytes = request.snapshot_bytes();
    let mut operation = Box::pin(handle.enqueue_launch_tracked(request).unwrap());
    let control = operation.control();
    let operation_wakes = arm_capture_future(operation.as_mut());
    let mut capture = Box::pin(
        handle
            .begin_drain_with_capture(
                8,
                prefix.source.take().unwrap(),
                vec![0xcc; 16].into_boxed_slice(),
            )
            .unwrap(),
    );
    let capture_wakes = arm_capture_future(capture.as_mut());
    assert_eq!(handle.observer().snapshot_bytes_in_use(), bytes);
    assert_eq!(handle.observer().reply_cells_in_use(), 3);
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 16);
    release.wait();
    let report = await_capture_future(capture.as_mut(), &capture_wakes).unwrap();
    assert_eq!(
        report.drain.outcome,
        RuntimeAsyncDrainOutcomeV1::BudgetExhausted
    );
    assert_eq!(report.drain.ticks, 8);
    assert_eq!(report.drain.retained_submissions.total_submissions, 2);
    assert_eq!(report.drain.retained_submissions.pending, 2);
    assert_eq!(report.drain.operations_remaining, 1);
    assert!(report.drain.queued_commands_exhausted);
    assert!(!report.drain.graph_active);
    assert!(matches!(
        report.capture,
        Err(CaptureError::CaptureIncomplete)
    ));
    assert!(matches!(
        await_capture_future(waiter.as_mut(), &waiter_wakes),
        Err(RuntimeAsyncEventErrorV1::Runtime(
            RuntimeErrorV1::BackendRejected(MockError("capture-prefix-observation"))
        ))
    ));
    assert_eq!(waiter.event(), event);
    assert!(matches!(
        await_capture_future(operation.as_mut(), &operation_wakes),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    assert!(control.same_operation(&operation.control()));
    assert_eq!(control.phase(), Phase::StoppedAfterSubmission);
    assert_eq!(
        control.cancel_before_submission(),
        Cancel::NotCancellable(Phase::StoppedAfterSubmission)
    );
    assert_capture_shutdown(engine, PrefixCaptureOutcome::Terminal, 2);
    assert_eq!(handle.observer().snapshot_bytes_in_use(), 0);
    assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
    assert_eq!(handle.observer().reply_cells_in_use(), 2);
    for wakes in [&capture_wakes, &operation_wakes, &waiter_wakes] {
        assert_eq!(wakes.0.load(AtomicOrdering::SeqCst), 1);
    }
    drop((capture, operation, waiter));
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
    let state = state.lock().unwrap();
    assert_eq!(state.issues.len(), 2);
    assert_eq!(state.issues[0].0, prefix.native_streams[0]);
    assert_eq!(state.issues[1].0, prefix.native_streams[1]);
    assert_ne!(state.issues[0].1, state.issues[1].1);
    assert_eq!(state.statuses.len(), 2);
    assert!(
        state
            .statuses
            .values()
            .all(|status| *status == BackendPollV1::Pending)
    );
    assert_eq!(state.release_calls, 0);
    let rejected_polls = state.poll_calls - initial_polls;
    assert!(rejected_polls >= 4);
    assert_eq!(state.poll_failures.len(), 64 - rejected_polls);
    assert!(!state.poll_failures.is_empty());
    let trace = trace.lock().unwrap();
    assert_eq!(trace.capture_calls, 0);
    assert!(trace.capture_requests.is_empty());
    assert!(trace.capture_submission_rosters.is_empty());
    assert!(trace.released_submissions.is_empty());
    assert_eq!(
        trace
            .calls
            .iter()
            .filter(|(call, _)| *call == "submit_v1")
            .count(),
        2
    );
    for issue in &state.issues {
        assert!(
            trace
                .polled_submissions
                .iter()
                .filter(|&&id| id == issue.1)
                .count()
                >= 2
        );
    }
    assert!(
        trace
            .polled_submissions
            .iter()
            .all(|id| state.statuses.contains_key(id))
    );
    assert!(
        !trace
            .calls
            .iter()
            .any(|(call, _)| matches!(*call, "release_allocation_v1" | "finalize" | "drop"))
    );
}

#[test]
fn drn3b_stop_before_pickup_or_during_capture_obeys_owner_order() {
    for during_capture in [false, true] {
        let state = Arc::new(Mutex::new(MockState::default()));
        let trace = Arc::new(Mutex::new(OwnerTrace::default()));
        let (entered_sender, entered_receiver) = sync_channel(1);
        let (release_sender, release_receiver) = sync_channel(1);
        if during_capture {
            trace.lock().unwrap().capture_pause = Some((entered_sender, release_receiver));
        }
        let (engine, handle) = start_with_config(state.clone(), trace.clone(), capture_config(16));
        let mut prefix = CapturePrefix::new(&handle, &state);
        let (predecessor, event) = prefix.raw_predecessor(&handle);
        let mut waiter = Box::pin(handle.observer().event_future(event).unwrap());
        let waiter_wakes = arm_capture_future(waiter.as_mut());
        let release_owner = paused_owner(&handle);
        let request = prefix.request(1, vec![event]);
        let bytes = request.snapshot_bytes();
        let mut operation = Box::pin(handle.enqueue_launch_tracked(request).unwrap());
        let control = operation.control();
        let operation_wakes = arm_capture_future(operation.as_mut());
        let mut capture = Box::pin(
            handle
                .begin_drain_with_capture(
                    128,
                    prefix.source.take().unwrap(),
                    vec![0xcc; 16].into_boxed_slice(),
                )
                .unwrap(),
        );
        let capture_wakes = arm_capture_future(capture.as_mut());
        assert_eq!(handle.observer().snapshot_bytes_in_use(), bytes);
        assert_eq!(handle.observer().reply_cells_in_use(), 3);
        assert_eq!(handle.observer().drain_capture_bytes_in_use(), 16);
        if during_capture {
            state.lock().unwrap().complete_on_flush = true;
            release_owner.wait();
            entered_receiver
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
            assert_eq!(control.phase(), Phase::ObservationFinished);
            assert_eq!(handle.observer().snapshot_bytes_in_use(), 0);
            assert_eq!(state.lock().unwrap().issues.len(), 2);
            assert_eq!(state.lock().unwrap().release_calls, 0);
            let trace = trace.lock().unwrap();
            assert_eq!(trace.capture_calls, 1);
            assert_eq!(trace.capture_submission_rosters[0].len(), 2);
            assert!(
                trace.capture_submission_rosters[0]
                    .iter()
                    .all(|(_, status)| *status == BackendPollV1::Succeeded)
            );
            assert!(
                !trace
                    .calls
                    .iter()
                    .any(|(call, _)| *call == "release_allocation_v1")
            );
        }
        handle
            .observer
            .sender
            .send(RuntimeAsyncEngineCommandV1::Stop)
            .unwrap();
        let waker = Waker::from(capture_wakes.clone());
        assert!(
            capture
                .as_mut()
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
        assert_eq!(capture_wakes.0.load(AtomicOrdering::SeqCst), 0);
        assert_eq!(handle.observer().drain_capture_bytes_in_use(), 16);
        if during_capture {
            // Stop is queued behind the synchronous callback, not a copy interruption.
            release_sender.send(()).unwrap();
        } else {
            release_owner.wait();
        }
        let result = await_capture_future(capture.as_mut(), &capture_wakes);
        let captured = if during_capture {
            PrefixCaptureOutcome::Success.check(result, 2)
        } else {
            assert!(matches!(
                result,
                Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
            ));
            None
        };
        let operation_result = await_capture_future(operation.as_mut(), &operation_wakes);
        let waiter_result = await_capture_future(waiter.as_mut(), &waiter_wakes);
        assert!(control.same_operation(&operation.control()));
        assert_eq!(waiter.event(), event);
        if during_capture {
            let result = operation_result.unwrap();
            let submission = result.submission.unwrap();
            assert_ne!(submission.id(), predecessor);
            assert_eq!(submission.stream(), prefix.streams[1]);
            assert_eq!(
                result.observation.unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
            assert_eq!(waiter_result.unwrap(), RuntimeCompletionStatusV1::Succeeded);
            assert_eq!(control.phase(), Phase::ObservationFinished);
            assert_capture_shutdown(engine, PrefixCaptureOutcome::Success, 2);
        } else {
            assert!(matches!(
                operation_result,
                Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
            ));
            assert!(matches!(
                waiter_result,
                Err(RuntimeAsyncEventErrorV1::EngineStopped)
            ));
            assert_eq!(control.phase(), Phase::StoppedBeforeSubmission);
            assert_capture_shutdown(engine, PrefixCaptureOutcome::Terminal, 1);
        }
        assert_eq!(handle.observer().snapshot_bytes_in_use(), 0);
        assert_eq!(handle.observer().reply_cells_in_use(), 2);
        for wakes in [&capture_wakes, &operation_wakes, &waiter_wakes] {
            assert_eq!(wakes.0.load(AtomicOrdering::SeqCst), 1);
        }
        drop((capture, operation, waiter));
        assert_eq!(handle.observer().reply_cells_in_use(), 0);
        assert_eq!(
            handle.observer().drain_capture_bytes_in_use(),
            if during_capture { 16 } else { 0 }
        );
        if let Some(captured) = captured {
            assert_eq!(captured.as_bytes(), &[0x5a; 16]);
            drop(captured);
        }
        assert_eq!(handle.observer().drain_capture_bytes_in_use(), 0);
        let state = state.lock().unwrap();
        let trace = trace.lock().unwrap();
        assert_eq!(trace.capture_calls, usize::from(during_capture));
        assert_eq!(state.issues.len(), if during_capture { 2 } else { 1 });
        assert_eq!(
            trace
                .calls
                .iter()
                .filter(|(call, _)| *call == "submit_v1")
                .count(),
            state.issues.len()
        );
        if during_capture {
            let mut released = trace.released_submissions.clone();
            released.sort_unstable();
            let mut expected: Vec<_> = state.issues.iter().map(|issue| issue.1).collect();
            expected.sort_unstable();
            assert_eq!(released, expected);
            assert_eq!(
                trace.capture_requests,
                vec![(1, prefix.native_source, 8, 16)]
            );
            assert!(state.statuses.is_empty());
        } else {
            assert!(trace.released_submissions.is_empty());
            assert!(trace.capture_requests.is_empty());
            assert_eq!(state.statuses[&state.issues[0].1], BackendPollV1::Pending);
            assert!(
                !trace.calls.iter().any(|(call, _)| matches!(
                    *call,
                    "release_allocation_v1" | "finalize" | "drop"
                ))
            );
        }
    }
}
