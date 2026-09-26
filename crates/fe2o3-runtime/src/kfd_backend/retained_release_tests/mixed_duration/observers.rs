//! Native observer lifetime and command admission, not native-slot capacity.

use super::*;
use crate::{
    RuntimeAllocationIdV1, RuntimeAsyncEngineCallErrorV1, RuntimeAsyncEngineConfigV1,
    RuntimeAsyncLaunchRequestV1, RuntimeAsyncOperationControlV1, RuntimeAsyncOperationPhaseV1,
    RuntimeAsyncOwnedDispositionV1, RuntimeAsyncOwnedEngineV1, RuntimeAsyncProgressConfigV1,
    RuntimeAsyncProgressHandleV1, RuntimeAsyncTimeoutResultV1, RuntimeCompletionStatusV1,
    RuntimeStreamIdV1, RuntimeStreamObservationV1, RuntimeSubmissionV1, TypedRuntimeKernelV1,
};
use std::sync::mpsc::{SyncSender, sync_channel};

fn pending_receipt(backend: &mut KfdRuntimeBackendV1) -> Option<NativePendingReceipt> {
    let before = long_receipt(backend)?;
    let status = backend.poll_compute_submission_v1(before.lane, before.id);
    let after = long_receipt(backend);
    if matches!(status, Ok(BackendPollV1::Pending)) && after.as_ref() == Some(&before) {
        Some(before)
    } else {
        None
    }
}

struct Prepared {
    variant: Variant,
    stream: RuntimeStreamIdV1,
    allocation: RuntimeAllocationIdV1,
    kernel: Arc<TypedRuntimeKernelV1<Arguments>>,
}

impl Prepared {
    fn request(&self) -> RuntimeAsyncLaunchRequestV1<Arguments> {
        RuntimeAsyncLaunchRequestV1::new(
            self.stream,
            Arc::clone(&self.kernel),
            &Arguments::new(self.allocation),
            GEOMETRY,
            Vec::new(),
        )
        .unwrap()
    }
}

struct Fixture {
    engine: RuntimeAsyncOwnedEngineV1<KfdRuntimeBackendV1>,
    handle: RuntimeAsyncProgressHandleV1<KfdRuntimeBackendV1>,
    prepared: Vec<Prepared>,
}

impl Fixture {
    fn new(variants: Vec<Variant>, config: RuntimeAsyncEngineConfigV1) -> Self {
        let unique_id = native_device();
        let (engine, handle) = RuntimeAsyncOwnedEngineV1::spawn_with_progress(
            move || -> Result<_, Box<dyn std::error::Error + Send + Sync>> {
                let mut backend =
                    KfdRuntimeBackendV1::open_gfx942_mixed_duration_qualification_v1(unique_id)?;
                backend
                    .configure_host_visible_backing_budget_v1(
                        Gfx942HostVisibleBackingBudgetV1::new(64 * 1024 * 1024, 128).unwrap(),
                    )
                    .map_err(|error| format!("host backing: {error:?}"))?;
                backend
                    .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([0x79; 32], 128).unwrap())
                    .map_err(|error| format!("profiler: {error:?}"))?;
                Ok(RuntimeContextV1::open(backend)?)
            },
            config,
            RuntimeAsyncProgressConfigV1::default(),
        )
        .unwrap();
        let prepared = await_bounded(
            handle
                .observer()
                .enqueue_with_context(move |context| {
                    let device = context.devices()[0].id();
                    variants
                        .into_iter()
                        .map(|variant| {
                            let module = context.load_module(device, variant.hsaco()).unwrap();
                            let kernel = Arc::new(
                                context
                                    .resolve_kernel::<Arguments>(module, variant.kernel_name())
                                    .unwrap(),
                            );
                            let stream = context.create_stream(device).unwrap();
                            let allocation = context
                                .allocate(device, RuntimeMemoryKindV1::HostVisible, BYTES as u64, 4)
                                .unwrap();
                            context.write_allocation(allocation, 0, &initial()).unwrap();
                            Prepared {
                                variant,
                                stream,
                                allocation,
                                kernel,
                            }
                        })
                        .collect()
                })
                .unwrap(),
        )
        .unwrap();
        Self {
            engine,
            handle,
            prepared,
        }
    }

    fn pending_long(
        &self,
        control: &RuntimeAsyncOperationControlV1,
    ) -> Option<NativePendingReceipt> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let (before, status, after) = await_bounded(
                self.handle
                    .observer()
                    .enqueue_with_context(|context| {
                        let backend = context.backend_mut_for_test_v1();
                        let before = long_receipt(backend);
                        let status = before.as_ref().map(|receipt| {
                            backend.poll_compute_submission_v1(receipt.lane, receipt.id)
                        });
                        (before, status, long_receipt(backend))
                    })
                    .unwrap(),
            )
            .unwrap();
            if let Some(status) = status {
                return if matches!(status, Ok(BackendPollV1::Pending)) && before == after {
                    before
                } else {
                    None
                };
            }
            if control.phase() == RuntimeAsyncOperationPhaseV1::ObservationFinished
                || Instant::now() >= deadline
            {
                return None;
            }
        }
    }

    fn finish(self, returned: Vec<RuntimeSubmissionV1<Arguments>>) -> Vec<u64> {
        let Self {
            engine,
            handle,
            prepared,
        } = self;
        let (ids, output, observations, usage, profile_ids, profile) = await_bounded(
            handle
                .observer()
                .enqueue_with_context(move |context| {
                    let mut returned_ids = returned
                        .iter()
                        .map(|submission| {
                            context.backend_submission_for_test_v1(submission).unwrap()
                        })
                        .collect::<Vec<_>>();
                    let observations = prepared
                        .iter()
                        .map(|row| context.query_stream(row.stream).unwrap())
                        .collect::<Vec<_>>();
                    let mut ids = context
                        .backend()
                        .submissions
                        .iter()
                        .map(|(&id, record)| {
                            assert_eq!(record.status, BackendPollV1::Succeeded);
                            assert!(record.profile_dispatch_published);
                            id
                        })
                        .collect::<Vec<_>>();
                    ids.sort_unstable();
                    assert_eq!(ids.len(), prepared.len());
                    if !returned_ids.is_empty() {
                        returned_ids.sort_unstable();
                        assert_eq!(returned_ids, ids);
                    }
                    let profile_ids = ids
                        .iter()
                        .map(|id| {
                            context
                                .backend()
                                .profile_resource_v1(KfdProfileResourceKindV1::Dispatch, *id)
                                .unwrap()
                        })
                        .collect::<Vec<_>>();
                    let output = prepared
                        .iter()
                        .map(|row| {
                            let mut bytes = [0; BYTES];
                            context
                                .read_allocation(row.allocation, 0, &mut bytes)
                                .unwrap();
                            (row.variant, bytes)
                        })
                        .collect::<Vec<_>>();
                    drop(returned);
                    drop(prepared);
                    assert!(context.cleanup().is_complete());
                    let backend = context.backend_mut_for_test_v1();
                    let usage = observe_shutdown(backend);
                    assert_retired_shutdown_is_inert(backend);
                    let profile = backend.finish_profiler_v1().unwrap();
                    (ids, output, observations, usage, profile_ids, profile)
                })
                .unwrap(),
        )
        .unwrap();
        let shutdown = engine.shutdown().unwrap();
        assert_eq!(
            shutdown.disposition,
            RuntimeAsyncOwnedDispositionV1::Released
        );
        assert!(shutdown.cleanup.unwrap().is_complete());
        assert!(!shutdown.worker_panicked && shutdown.native_failure.is_none());
        assert_eq!(handle.observer().reply_cells_in_use(), 0);
        assert_eq!(handle.observer().snapshot_bytes_in_use(), 0);
        for observation in observations {
            assert_eq!(
                observation,
                RuntimeStreamObservationV1 {
                    total_submissions: 1,
                    succeeded: 1,
                    ..Default::default()
                }
            );
        }
        for (variant, bytes) in output {
            assert!(variant.validate_output(&bytes));
            let hex = bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            println!("mixed_observer variant={variant:?} observed_hex={hex}");
        }
        assert!(profile.coverage.complete_runtime_operation_history);
        assert_eq!(profile.coverage.dropped_events, 0);
        let published = profile
            .events
            .iter()
            .filter_map(|entry| match entry.event {
                KfdRuntimeProfileEventKindV1::DispatchPublished {
                    dispatch, queue, ..
                } => Some((entry.sequence, dispatch, queue)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let completed = profile
            .events
            .iter()
            .filter_map(|entry| match entry.event {
                KfdRuntimeProfileEventKindV1::DispatchCompleted { dispatch, .. } => {
                    Some((entry.sequence, dispatch))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let released = profile
            .events
            .iter()
            .filter_map(|entry| match entry.event {
                KfdRuntimeProfileEventKindV1::SubmissionReleased { dispatch } => {
                    Some((entry.sequence, dispatch))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(published.len(), ids.len());
        assert_eq!(completed.len(), ids.len());
        assert_eq!(released.len(), ids.len());
        for id in profile_ids {
            let pubs = published
                .iter()
                .filter(|(_, dispatch, _)| *dispatch == id)
                .collect::<Vec<_>>();
            let done = completed
                .iter()
                .filter(|(_, dispatch)| *dispatch == id)
                .collect::<Vec<_>>();
            let freed = released
                .iter()
                .filter(|(_, dispatch)| *dispatch == id)
                .collect::<Vec<_>>();
            assert_eq!((pubs.len(), done.len(), freed.len()), (1, 1, 1));
            assert!(pubs[0].0 < done[0].0 && done[0].0 < freed[0].0);
            let created = profile
                .events
                .iter()
                .filter_map(|entry| match entry.event {
                    KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue }
                        if queue == pubs[0].2 =>
                    {
                        Some(entry.sequence)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let destroyed = profile
                .events
                .iter()
                .filter_map(|entry| match entry.event {
                    KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue }
                        if queue == pubs[0].2 =>
                    {
                        Some(entry.sequence)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!((created.len(), destroyed.len()), (1, 1));
            assert!(created[0] < pubs[0].0 && freed[0].0 < destroyed[0]);
        }
        println!("mixed_observer ids={ids:?} shutdown={usage:?} physical_overlap=not_measured");
        println!(
            "mixed_observer_profile_json={}",
            serde_json::to_string(&profile).unwrap()
        );
        ids
    }
}

#[test]
#[ignore = "requires isolated MI300X and an external process-group deadline"]
fn native_owned_timeout_recovers_exact_published_operation_and_full_output() {
    let fixture = Fixture::new(vec![Variant::Long], RuntimeAsyncEngineConfigV1::default());
    let operation = fixture
        .handle
        .enqueue_launch_tracked(fixture.prepared[0].request())
        .unwrap();
    let control = operation.control();
    let prefix = fixture.pending_long(&control);
    let (timed, before, after) = await_bounded(
        fixture
            .handle
            .observer()
            .enqueue_with_context(move |context| {
                let backend = context.backend_mut_for_test_v1();
                let before = pending_receipt(backend);
                let mut timed = operation.observe_with_timeout(std::future::ready(()));
                let Poll::Ready(outcome) =
                    std::pin::Pin::new(&mut timed).poll(&mut Context::from_waker(Waker::noop()))
                else {
                    unreachable!("an immediately ready timer cannot return Pending");
                };
                (outcome, before, pending_receipt(backend))
            })
            .unwrap(),
    )
    .unwrap();
    let (timed_out, same_operation, result) = match timed {
        RuntimeAsyncTimeoutResultV1::TimedOut { operation } => {
            let same = control.same_operation(&operation.control());
            (true, same, await_bounded(operation))
        }
        RuntimeAsyncTimeoutResultV1::Completed(result) => (false, false, result),
    };
    let result = result.unwrap();
    assert_eq!(
        result.observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(result.rejected_observations, 0);
    let ids = fixture.finish(vec![result.submission.unwrap()]);
    assert!(
        timed_out && same_operation,
        "timing miss is not timeout qualification"
    );
    assert_eq!(
        before, prefix,
        "the timeout observes the original native occurrence"
    );
    assert_eq!(
        after, before,
        "native Pending custody survives the immediate timeout"
    );
    assert_eq!(
        ids,
        vec![prefix.expect("actual published Pending signal required").id]
    );
    assert_eq!(
        control.phase(),
        RuntimeAsyncOperationPhaseV1::ObservationFinished
    );
    println!("mixed_observer_case=timeout recovered_same_operation=true");
}

#[test]
#[ignore = "requires isolated MI300X and an external process-group deadline"]
fn native_owned_dropped_observer_keeps_autonomous_progress_and_full_output() {
    let fixture = Fixture::new(vec![Variant::Long], RuntimeAsyncEngineConfigV1::default());
    let operation = fixture
        .handle
        .enqueue_launch_tracked(fixture.prepared[0].request())
        .unwrap();
    let control = operation.control();
    let prefix = fixture.pending_long(&control);
    let (before, after) = await_bounded(
        fixture
            .handle
            .observer()
            .enqueue_with_context(move |context| {
                let backend = context.backend_mut_for_test_v1();
                let before = pending_receipt(backend);
                drop(operation);
                (before, pending_receipt(backend))
            })
            .unwrap(),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while control.phase() != RuntimeAsyncOperationPhaseV1::ObservationFinished
        && Instant::now() < deadline
    {
        thread::sleep(Duration::from_millis(1));
    }
    // No event observer, explicit flush, synchronization or replacement operation drives completion.
    assert_eq!(
        control.phase(),
        RuntimeAsyncOperationPhaseV1::ObservationFinished
    );
    let ids = fixture.finish(Vec::new());
    assert_eq!(
        before, prefix,
        "the dropped observer belongs to the original native occurrence"
    );
    assert_eq!(
        after, before,
        "the same native signal is still Pending after observer Drop"
    );
    assert_eq!(
        ids,
        vec![prefix.expect("actual published Pending signal required").id]
    );
    println!("mixed_observer_case=drop autonomous_completion=true");
}

struct ReleaseGate(SyncSender<()>);

impl Drop for ReleaseGate {
    fn drop(&mut self) {
        let _ = self.0.try_send(());
    }
}

#[test]
#[ignore = "requires isolated MI300X and an external process-group deadline"]
fn native_owned_command_backpressure_refunds_rejected_launch_and_recovers() {
    let config = RuntimeAsyncEngineConfigV1::new(1, 4, 1, 1, Duration::from_millis(1))
        .unwrap()
        .with_reply_capacity(8)
        .unwrap();
    let fixture = Fixture::new(vec![Variant::Short, Variant::Long], config);
    let (entered_tx, entered_rx) = sync_channel(1);
    let (release_tx, release_rx) = sync_channel(1);
    let release = ReleaseGate(release_tx);
    let gate = fixture
        .handle
        .observer()
        .enqueue_with_context(move |_| {
            entered_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(10))
        })
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let request = fixture.prepared[0].request();
    let charged = request.snapshot_bytes();
    let accepted = fixture.handle.enqueue_launch_tracked(request);
    let before = (
        fixture.handle.observer().snapshot_bytes_in_use(),
        fixture.handle.observer().reply_cells_in_use(),
    );
    let rejected = fixture
        .handle
        .enqueue_launch_tracked(fixture.prepared[1].request());
    let after = (
        fixture.handle.observer().snapshot_bytes_in_use(),
        fixture.handle.observer().reply_cells_in_use(),
    );
    drop(release);
    await_bounded(gate).unwrap().unwrap();
    assert!(matches!(
        rejected,
        Err(RuntimeAsyncEngineCallErrorV1::CommandQueueFull)
    ));
    assert!(charged > 0);
    assert_eq!(before, (charged, 2));
    assert_eq!(after, before);
    let short = await_bounded(accepted.unwrap()).unwrap();
    assert_eq!(
        short.observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    let stream = fixture.prepared[1].stream;
    let allocation = fixture.prepared[1].allocation;
    let (unissued, unchanged) = await_bounded(
        fixture
            .handle
            .observer()
            .enqueue_with_context(move |context| {
                let observation = context.query_stream(stream).unwrap();
                let mut bytes = [0; BYTES];
                context.read_allocation(allocation, 0, &mut bytes).unwrap();
                (observation, bytes)
            })
            .unwrap(),
    )
    .unwrap();
    let long = await_bounded(
        fixture
            .handle
            .enqueue_launch_tracked(fixture.prepared[1].request())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        long.observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(
        (short.rejected_observations, long.rejected_observations),
        (0, 0)
    );
    fixture.finish(vec![short.submission.unwrap(), long.submission.unwrap()]);
    assert_eq!(unissued, RuntimeStreamObservationV1::default());
    assert_eq!(unchanged, initial());
    println!(
        "mixed_observer_case=backpressure command_queue_full=1 rejected_launch_issued=false recovered=true native_slot_saturation=not_measured"
    );
}
