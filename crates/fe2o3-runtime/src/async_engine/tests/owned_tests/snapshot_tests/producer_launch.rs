use super::*;
use crate::{
    BackendProducerAwareLaunchV1, RuntimeAccessV1, RuntimeAsyncEventOperationV1,
    RuntimeAsyncOperationEventErrorV1, RuntimeAsyncOperationEventFutureV1,
    RuntimeAsyncOperationFutureV1, RuntimeProducerAwareLaunchBackendV1,
};

impl RuntimeProducerAwareLaunchBackendV1 for MockBackend {
    fn submit_producer_aware_launch_v1(
        &mut self,
        request: BackendProducerAwareLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        let mut dependencies = Vec::new();
        {
            let state = self.state.lock().unwrap();
            for (index, dependency) in request.dependencies.iter().enumerate() {
                assert_eq!(
                    state.event_sources.get(&dependency.event),
                    Some(&dependency.producer_submission)
                );
                assert!(
                    request.dependencies[..index].iter().all(|prior| {
                        prior.producer_submission != dependency.producer_submission
                    })
                );
                dependencies.push(dependency.event);
            }
        }
        // Tests supply terminal device facts explicitly; this checks the exact
        // frozen launch transport, not native publication or device execution.
        self.submit_v1(BackendLaunchV1 {
            stream: request.stream,
            kernel: request.kernel,
            explicit_kernarg: request.explicit_kernarg,
            bindings: request.bindings,
            dependencies: &dependencies,
            geometry: request.geometry,
            semantic_launch: crate::BackendSemanticLaunchV1::Ordinary,
        })
    }
}

impl RuntimeProducerAwareLaunchBackendV1 for ThreadBoundBackend {
    fn submit_producer_aware_launch_v1(
        &mut self,
        request: BackendProducerAwareLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.record("submit_producer_aware_launch_v1");
        self.inner.submit_producer_aware_launch_v1(request)
    }
}

#[test]
fn queued_producer_chain_runs_and_releases_on_the_non_send_owner_thread() {
    for orphan in [false, true] {
        let state = Arc::new(Mutex::new(MockState::default()));
        let trace = Arc::new(Mutex::new(OwnerTrace::default()));
        let (engine, handle) = start_with_context_profile(
            state.clone(),
            trace.clone(),
            RuntimeAsyncEngineConfigV1::default(),
            true,
        );
        let (streams, kernel, x, owner) = join_command(
            handle
                .observer()
                .enqueue_with_context(|context| {
                    let device = context.devices()[0].id();
                    let streams =
                        core::array::from_fn::<_, 3, _>(|_| context.create_stream(device).unwrap());
                    let module = context.load_module(device, &[1]).unwrap();
                    let kernel = Arc::new(context.resolve_kernel::<Args>(module, "args").unwrap());
                    let x = context
                        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
                        .unwrap();
                    (streams, kernel, x, thread::current().id())
                })
                .unwrap(),
        )
        .unwrap();
        assert_ne!(owner, thread::current().id());
        let mut events = Vec::new();
        let mut captures = Vec::new();
        let mut consumer = None;
        for (stage, stream) in streams.into_iter().enumerate() {
            let args = arguments(vec![binding(
                x,
                if stage == 2 {
                    RuntimeAccessV1::Read
                } else {
                    RuntimeAccessV1::Write
                },
                0,
            )]);
            let request = RuntimeAsyncLaunchRequestV1::new(
                stream,
                kernel.clone(),
                &args,
                geometry(),
                events.last().copied().into_iter().collect(),
            )
            .unwrap();
            let queued = handle.enqueue_producer_launch_with_event(request).unwrap();
            events.push(join_command(queued.event).unwrap().unwrap());
            captures.push(args);
            if stage == 2 && !orphan {
                consumer = Some(queued.operation.future);
            }
        }
        let [ae, be, ce]: [_; 3] = events.try_into().unwrap();
        let counts = join_command(
            handle
                .observer()
                .enqueue_with_context(move |context| {
                    context.release_event(ae).unwrap();
                    context.release_event(be).unwrap();
                    (
                        context.version_journal_read_records_v1(),
                        context.version_journal_writer_records_v1(),
                        context.query_event(ce).unwrap(),
                    )
                })
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            counts,
            (Some(1), Some(2), RuntimeCompletionStatusV1::Pending)
        );
        {
            let mut state = state.lock().unwrap();
            assert_eq!(state.issues.len(), 3);
            let ids: Vec<_> = state.issues.iter().map(|issue| issue.1).collect();
            for index in 1..3 {
                let dependencies: Vec<_> = state.submission_dependencies[&ids[index]]
                    .iter()
                    .map(|event| state.event_sources[event])
                    .collect();
                assert_eq!(dependencies, vec![ids[index - 1]]);
            }
            state
                .statuses
                .values_mut()
                .for_each(|status| *status = BackendPollV1::Succeeded);
        }
        if let Some(future) = consumer {
            assert_eq!(
                join_command(future).unwrap().observation.unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
        }
        let mut complete = false;
        for _ in 0..16 {
            let status = join_command(
                handle
                    .observer()
                    .enqueue_with_context(move |context| context.poll_event(ce).unwrap())
                    .unwrap(),
            )
            .unwrap();
            if status == RuntimeCompletionStatusV1::Succeeded {
                complete = true;
                break;
            }
        }
        assert!(complete);
        let counts = join_command(
            handle
                .observer()
                .enqueue_with_context(move |context| {
                    context.release_event(ce).unwrap();
                    (
                        context.version_journal_read_records_v1(),
                        context.version_journal_writer_records_v1(),
                    )
                })
                .unwrap(),
        )
        .unwrap();
        assert_eq!(counts, (Some(0), Some(0)));
        for args in captures {
            assert_eq!(args.encodes.load(Ordering::SeqCst), 1);
            assert_eq!(args.bindings.load(Ordering::SeqCst), 1);
        }
        assert_eq!(handle.observer().snapshot_bytes_in_use(), 0);
        // Orphan drivers must retire while the owner is live, not just at Stop.
        for _ in 0..16 {
            if handle.observer().reply_cells_in_use() == 0 {
                break;
            }
            join_command(handle.observer().enqueue_with_context(|_| ()).unwrap()).unwrap();
        }
        assert_eq!(handle.observer().reply_cells_in_use(), 0);
        let report = engine.shutdown().unwrap();
        assert_eq!(report.disposition, RuntimeAsyncOwnedDispositionV1::Released);
        assert!(report.cleanup.unwrap().is_complete());
        assert!(!report.worker_panicked);
        assert!(report.native_failure.is_none());
        let trace = trace.lock().unwrap();
        assert!(trace.calls.iter().all(|(_, id)| *id == owner));
        assert_eq!(
            trace
                .calls
                .iter()
                .filter(|(call, _)| *call == "submit_producer_aware_launch_v1")
                .count(),
            3
        );
        assert_eq!(trace.calls[trace.calls.len() - 2].0, "finalize");
        assert_eq!(trace.calls.last().unwrap().0, "drop");
    }
}

#[derive(Clone, Copy)]
enum Mode {
    Ordinary,
    Tracked,
    Event,
}

const MODES: [Mode; 3] = [Mode::Ordinary, Mode::Tracked, Mode::Event];

struct Queued {
    future: RuntimeAsyncOperationFutureV1<Args, MockError>,
    event: Option<RuntimeAsyncOperationEventFutureV1<MockError>>,
    control: Option<RuntimeAsyncOperationControlV1>,
}

fn enqueue(
    h: &Harness,
    request: RuntimeAsyncLaunchRequestV1<Args>,
    mode: Mode,
) -> Result<Queued, RuntimeAsyncEngineCallErrorV1> {
    Ok(match mode {
        Mode::Ordinary => Queued {
            future: h.handle.enqueue_producer_launch(request)?,
            event: None,
            control: None,
        },
        Mode::Tracked => {
            let operation = h.handle.enqueue_producer_launch_tracked(request)?;
            Queued {
                control: Some(operation.control()),
                future: operation.future,
                event: None,
            }
        }
        Mode::Event => {
            let RuntimeAsyncEventOperationV1 { event, operation } =
                h.handle.enqueue_producer_launch_with_event(request)?;
            Queued {
                control: Some(operation.control()),
                future: operation.future,
                event: Some(event),
            }
        }
    })
}

fn arguments(effects: Vec<RuntimeBindingV1>) -> Args {
    Args {
        encodes: Arc::new(AtomicUsize::new(0)),
        bindings: Arc::new(AtomicUsize::new(0)),
        length: (effects.len() * 8).max(8),
        effects,
    }
}

fn allocation(h: &mut Harness) -> crate::RuntimeAllocationIdV1 {
    h.context
        .allocate(
            h.context.devices()[0].id(),
            RuntimeMemoryKindV1::DeviceLocal,
            8,
            8,
        )
        .unwrap()
}

fn binding(
    allocation: crate::RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
    index: u32,
) -> RuntimeBindingV1 {
    RuntimeBindingV1 {
        region: crate::RuntimeMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 8,
        },
        kernarg_byte_offset: index * 8,
    }
}

#[test]
fn queued_producer_async_chain_keeps_exact_frozen_dependencies_after_observer_loss() {
    for mode in MODES {
        for drop_consumer in [false, true] {
            let mut h = Harness::with_journal(4096, 1, false, true);
            let x = allocation(&mut h);
            let device = h.context.devices()[0].id();
            let mut events = Vec::new();
            let mut parents = Vec::new();
            let mut captures = Vec::new();
            for _ in 0..2 {
                let stream = h.context.create_stream(device).unwrap();
                let args = arguments(vec![binding(x, RuntimeAccessV1::Write, 0)]);
                let request = RuntimeAsyncLaunchRequestV1::new(
                    stream,
                    h.kernel.clone(),
                    &args,
                    geometry(),
                    events.last().copied().into_iter().collect(),
                )
                .unwrap();
                let bytes = request.snapshot_bytes();
                let mut queued = enqueue(&h, request, Mode::Event).unwrap();
                assert_eq!(h.used(), bytes);
                let mut driver = h.pop();
                assert!(!driver.advance(&mut h.context));
                assert_eq!(h.used(), 0);
                assert!(!driver.advance(&mut h.context));
                events.push(join_command(queued.event.take().unwrap()).unwrap().unwrap());
                captures.push(args);
                parents.push(driver);
                drop(queued);
            }
            let args = arguments(vec![binding(x, RuntimeAccessV1::Read, 0)]);
            let request = RuntimeAsyncLaunchRequestV1::new(
                h.stream,
                h.kernel.clone(),
                &args,
                geometry(),
                vec![events[1]],
            )
            .unwrap();
            let bytes = request.snapshot_bytes();
            let mut queued = enqueue(&h, request, mode).unwrap();
            assert_eq!(h.used(), bytes);
            let mut driver = h.pop();
            assert!(!driver.advance(&mut h.context));
            assert_eq!(h.used(), 0);
            captures.push(args);
            assert_eq!(h.context.version_journal_read_records_v1(), Some(1));
            assert_eq!(h.context.version_journal_writer_records_v1(), Some(2));
            {
                let state = h.state.lock().unwrap();
                assert_eq!(state.issues.len(), 3);
                let ids: Vec<_> = state.issues.iter().map(|issue| issue.1).collect();
                assert!(state.submission_dependencies[&ids[0]].is_empty());
                for index in 1..3 {
                    let deps: Vec<_> = state.submission_dependencies[&ids[index]]
                        .iter()
                        .map(|event| state.event_sources[event])
                        .collect();
                    assert_eq!(deps, vec![ids[index - 1]]);
                }
                for (index, issue) in state.issues.iter().enumerate() {
                    assert_eq!(issue.2, vec![0; 8]);
                    assert_eq!(issue.3.len(), 1);
                    assert_eq!(
                        issue.3[0].region.allocation,
                        state.issues[0].3[0].region.allocation
                    );
                    assert_eq!(
                        issue.3[0].region.access,
                        if index == 2 {
                            RuntimeAccessV1::Read
                        } else {
                            RuntimeAccessV1::Write
                        }
                    );
                }
            }
            for event in events {
                h.context.release_event(event).unwrap();
            }
            if let Some(event) = queued.event.take() {
                assert!(!driver.advance(&mut h.context));
                h.context
                    .release_event(join_command(event).unwrap().unwrap())
                    .unwrap();
            }
            drop(queued.control.take());
            let future = (!drop_consumer).then_some(queued.future);
            assert!(!driver.advance(&mut h.context));
            assert_eq!(h.context.version_journal_read_records_v1(), Some(1));
            // Supply mutually consistent device facts before producer-first logical
            // reconciliation. A completed child with a still-Pending parent is invalid.
            h.state
                .lock()
                .unwrap()
                .statuses
                .values_mut()
                .for_each(|status| *status = BackendPollV1::Succeeded);
            assert!(!driver.advance(&mut h.context));
            assert_eq!(h.context.version_journal_read_records_v1(), Some(1));
            let mut complete = false;
            for _ in 0..16 {
                if driver.advance(&mut h.context) {
                    complete = true;
                    break;
                }
            }
            assert!(complete);
            if let Some(future) = future {
                assert_eq!(
                    join_command(future).unwrap().observation.unwrap(),
                    RuntimeCompletionStatusV1::Succeeded
                );
            }
            for parent in &mut parents {
                assert!(parent.advance(&mut h.context));
            }
            for args in captures {
                assert_eq!(args.encodes.load(Ordering::SeqCst), 1);
                assert_eq!(args.bindings.load(Ordering::SeqCst), 1);
            }
            assert_eq!(h.context.version_journal_read_records_v1(), Some(0));
            assert_eq!(h.context.version_journal_writer_records_v1(), Some(0));
            drop(driver);
            drop(parents);
            assert_eq!(h.handle.observer().reply_cells_in_use(), 0);
            assert!(h.context.cleanup().is_complete());
        }
    }
}

#[test]
fn queued_producer_async_preissue_cancel_never_acquires_or_refunds_parent_custody() {
    for mode in [Mode::Tracked, Mode::Event] {
        let mut h = Harness::with_journal(4096, 1, false, true);
        let x = allocation(&mut h);
        let args = arguments(vec![binding(x, RuntimeAccessV1::Write, 0)]);
        let a = h
            .context
            .launch_producer_aware_v1(h.stream, &h.kernel, &args, geometry(), &[])
            .unwrap();
        let ae = h.context.record_event(&a).unwrap();
        let b_args = arguments(vec![binding(x, RuntimeAccessV1::Write, 0)]);
        let b = h
            .context
            .launch_producer_aware_v1(h.stream, &h.kernel, &b_args, geometry(), &[ae])
            .unwrap();
        let be = h.context.record_event(&b).unwrap();
        let input = arguments(vec![binding(x, RuntimeAccessV1::Read, 0)]);
        let request = RuntimeAsyncLaunchRequestV1::new(
            h.stream,
            h.kernel.clone(),
            &input,
            geometry(),
            vec![be],
        )
        .unwrap();
        let bytes = request.snapshot_bytes();
        let queued = enqueue(&h, request, mode).unwrap();
        assert_eq!(
            queued.control.as_ref().unwrap().cancel_before_submission(),
            crate::RuntimeAsyncCancelResultV1::CancelledBeforeSubmission
        );
        drop(queued);
        assert_eq!(h.used(), bytes);
        assert!(h.pop().advance(&mut h.context));
        assert_eq!(h.used(), 0);
        assert_eq!(h.state.lock().unwrap().issues.len(), 2);
        assert_eq!(h.context.version_journal_read_records_v1(), Some(0));
        assert_eq!(h.context.version_journal_writer_records_v1(), Some(2));
        assert_eq!(
            h.context.query_submission(&a).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
        assert_eq!(
            h.context.query_submission(&b).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
        assert_eq!(input.encodes.load(Ordering::SeqCst), 1);
        assert_eq!(input.bindings.load(Ordering::SeqCst), 1);
        h.state
            .lock()
            .unwrap()
            .statuses
            .values_mut()
            .for_each(|status| *status = BackendPollV1::Succeeded);
        assert!(h.context.cleanup().is_complete());
        assert_eq!(h.handle.observer().reply_cells_in_use(), 0);
    }
}

#[test]
fn producer_launch_snapshots_freeze_once_and_keep_original_driver_lifecycle() {
    for mode in MODES {
        let mut h = Harness::with_journal(8, 1, false, true);
        let queued = enqueue(&h, h.request(), mode).unwrap();
        assert_eq!(h.used(), 8);
        assert_eq!(h.args.encodes.load(Ordering::SeqCst), 1);
        assert_eq!(h.args.bindings.load(Ordering::SeqCst), 1);
        assert!(h.state.lock().unwrap().issues.is_empty());
        let mut operation = h.pop();
        assert!(!operation.advance(&mut h.context));
        assert_eq!(h.used(), 0);
        assert_eq!(h.args.encodes.load(Ordering::SeqCst), 1);
        {
            let state = h.state.lock().unwrap();
            assert_eq!(state.issues.len(), 1);
            assert_eq!(state.issues[0].2, vec![0; 8]);
            assert_eq!(state.poll_calls, 0);
            assert_eq!(state.event_record_calls, 0);
        }
        if let Some(event) = queued.event {
            assert!(!operation.advance(&mut h.context));
            assert_eq!(h.state.lock().unwrap().poll_calls, 0);
            let event = join_command(event).unwrap().unwrap();
            assert_eq!(
                h.context.query_event(event).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
            h.context.release_event(event).unwrap();
        }
        h.state
            .lock()
            .unwrap()
            .statuses
            .values_mut()
            .for_each(|status| *status = BackendPollV1::Succeeded);
        assert!(operation.advance(&mut h.context));
        assert_eq!(
            join_command(queued.future).unwrap().observation.unwrap(),
            RuntimeCompletionStatusV1::Succeeded
        );
        assert_eq!(h.state.lock().unwrap().release_calls, 0);
        assert!(h.context.cleanup().is_complete());
    }
}

#[test]
fn producer_launch_snapshot_credit_survives_observer_loss_and_local_cancellation() {
    for mode in MODES {
        let mut h = Harness::with_journal(8, 1, false, true);
        let queued = enqueue(&h, h.request(), mode).unwrap();
        if let Some(control) = &queued.control {
            control.cancel_before_submission();
        }
        drop(queued);
        assert_eq!(h.used(), 8);
        assert!(matches!(
            enqueue(&h, h.request(), mode),
            Err(RuntimeAsyncEngineCallErrorV1::SnapshotCapacity)
        ));
        let mut operation = h.pop();
        if matches!(mode, Mode::Ordinary) {
            assert!(!operation.advance(&mut h.context));
            assert_eq!(h.state.lock().unwrap().issues.len(), 1);
            h.state
                .lock()
                .unwrap()
                .statuses
                .values_mut()
                .for_each(|status| *status = BackendPollV1::Succeeded);
            assert!(operation.advance(&mut h.context));
        } else {
            assert!(operation.advance(&mut h.context));
            assert!(h.state.lock().unwrap().issues.is_empty());
        }
        assert_eq!(h.used(), 0);
        assert!(h.context.cleanup().is_complete());
    }
}

#[test]
fn producer_launch_queue_and_reply_rejections_refund_snapshot_credit() {
    for mode in MODES {
        let h = Harness::with_journal(8, 1, false, true);
        h.handle
            .observer
            .sender
            .try_send(RuntimeAsyncEngineCommandV1::Stop)
            .unwrap_or_else(|_| panic!("empty channel"));
        assert!(matches!(
            enqueue(&h, h.request(), mode),
            Err(RuntimeAsyncEngineCallErrorV1::CommandQueueFull)
        ));
        assert_eq!(h.used(), 0);
        drop(h.receiver.try_recv().unwrap());
        let queued = enqueue(&h, h.request(), mode).unwrap();
        drop(h.pop_factory());
        assert_eq!(h.used(), 0);
        assert!(matches!(
            join_command(queued.future),
            Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
        ));
        if let Some(event) = queued.event {
            assert!(matches!(
                join_command(event),
                Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
            ));
        }
    }
    let mut h = Harness::with_journal(8, 1, false, true);
    h.handle.observer.reply_budget = reply_budget::ReplyBudgetV1::new(1);
    assert!(matches!(
        h.handle.enqueue_producer_launch_with_event(h.request()),
        Err(RuntimeAsyncEngineCallErrorV1::ReplyCapacity)
    ));
    assert_eq!(h.used(), 0);
    assert!(h.receiver.try_recv().is_err());
}

#[test]
fn producer_launch_owner_revalidates_journal_and_stream_before_native_entry() {
    for mode in MODES {
        for journal in [false, true] {
            let mut h = Harness::with_journal(8, 1, false, journal);
            let queued = enqueue(&h, h.request(), mode).unwrap();
            if journal {
                h.context.destroy_stream(h.stream).unwrap();
            }
            assert!(h.pop().advance(&mut h.context));
            assert_eq!(h.used(), 0);
            let expected = if journal {
                RuntimeValidationErrorV1::UnknownStream
            } else {
                RuntimeValidationErrorV1::Unsupported
            };
            assert!(
                matches!(join_command(queued.future).unwrap().observation, Err(RuntimeErrorV1::Validation(error)) if error == expected)
            );
            if let Some(event) = queued.event {
                assert!(matches!(
                    join_command(event).unwrap(),
                    Err(RuntimeAsyncOperationEventErrorV1::SubmissionUnavailable)
                ));
            }
            assert!(h.state.lock().unwrap().issues.is_empty());
            assert!(h.context.cleanup().is_complete());
        }
    }
}

#[test]
fn producer_launch_event_failure_keeps_the_single_accepted_operation() {
    let mut h = Harness::with_journal(8, 1, false, true);
    h.state
        .lock()
        .unwrap()
        .event_record_failures
        .push_back(RuntimeBackendFailureV1::Rejected(MockError("record busy")));
    let queued = h
        .handle
        .enqueue_producer_launch_with_event(h.request())
        .unwrap();
    let mut operation = h.pop();
    assert!(!operation.advance(&mut h.context));
    assert!(!operation.advance(&mut h.context));
    assert!(matches!(
        join_command(queued.event).unwrap(),
        Err(RuntimeAsyncOperationEventErrorV1::RecordingFailed(
            RuntimeErrorV1::BackendRejected(MockError("record busy"))
        ))
    ));
    assert_eq!(h.used(), 0);
    assert_eq!(h.state.lock().unwrap().poll_calls, 0);
    assert_eq!(h.state.lock().unwrap().issues.len(), 1);
    h.state
        .lock()
        .unwrap()
        .statuses
        .values_mut()
        .for_each(|status| *status = BackendPollV1::Succeeded);
    assert!(operation.advance(&mut h.context));
    assert_eq!(
        join_command(queued.operation.future)
            .unwrap()
            .observation
            .unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert!(h.context.cleanup().is_complete());
}

#[test]
fn producer_launch_async_read_only_join_retains_two_pending_inputs_and_one_stable_input() {
    for mode in MODES {
        let mut h = Harness::with_journal(4096, 1, false, true);
        let first = allocation(&mut h);
        let second = allocation(&mut h);
        let stable = allocation(&mut h);
        let device = h.context.devices()[0].id();
        let mut parents = Vec::new();
        let mut events = Vec::new();
        for allocation in [first, second] {
            let stream = h.context.create_stream(device).unwrap();
            let args = arguments(vec![binding(allocation, RuntimeAccessV1::Write, 0)]);
            let producer = h
                .context
                .launch_producer_aware_v1(stream, &h.kernel, &args, geometry(), &[])
                .unwrap();
            events.push(h.context.record_event(&producer).unwrap());
            parents.push(producer);
        }
        events.reverse();
        let args = arguments(vec![
            binding(first, RuntimeAccessV1::Read, 0),
            binding(stable, RuntimeAccessV1::Read, 1),
            binding(second, RuntimeAccessV1::Read, 2),
        ]);
        let request = RuntimeAsyncLaunchRequestV1::new(
            h.stream,
            h.kernel.clone(),
            &args,
            geometry(),
            events.clone(),
        )
        .unwrap();
        let expected_bytes = request.snapshot_bytes();
        let queued = enqueue(&h, request, mode).unwrap();
        assert_eq!(h.used(), expected_bytes);
        let mut operation = h.pop();
        assert!(!operation.advance(&mut h.context));
        assert_eq!(h.used(), 0);
        assert_eq!(args.encodes.load(Ordering::SeqCst), 1);
        assert_eq!(args.bindings.load(Ordering::SeqCst), 1);
        assert_eq!(h.context.version_journal_read_records_v1(), Some(3));
        assert_eq!(h.context.version_journal_writer_records_v1(), Some(2));
        for source in [first, second, stable] {
            assert!(matches!(
                h.context.write_allocation(source, 0, &[1]),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::ContextReserved
                ))
            ));
        }
        for event in events {
            h.context.release_event(event).unwrap();
        }
        if let Some(event) = queued.event {
            assert!(!operation.advance(&mut h.context));
            h.context
                .release_event(join_command(event).unwrap().unwrap())
                .unwrap();
        }
        {
            let mut state = h.state.lock().unwrap();
            assert_eq!(state.issues.len(), 3);
            let consumer = state.issues[2].1;
            let second_parent = state.issues[1].1;
            let first_parent = state.issues[0].1;
            let supplied: Vec<_> = state.submission_dependencies[&consumer]
                .iter()
                .map(|event| state.event_sources[event])
                .collect();
            assert_eq!(supplied, vec![second_parent, first_parent]);
            state
                .statuses
                .values_mut()
                .for_each(|status| *status = BackendPollV1::Succeeded);
        }
        let mut completed = false;
        for _ in 0..16 {
            if operation.advance(&mut h.context) {
                completed = true;
                break;
            }
        }
        assert!(completed, "bounded consumer-first reconciliation");
        assert_eq!(
            join_command(queued.future).unwrap().observation.unwrap(),
            RuntimeCompletionStatusV1::Succeeded
        );
        for parent in &parents {
            assert_eq!(
                h.context.query_submission(parent).unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
        }
        assert_eq!(h.context.version_journal_read_records_v1(), Some(0));
        assert_eq!(h.context.version_journal_writer_records_v1(), Some(0));
        assert!(h.context.cleanup().is_complete());
    }
}
