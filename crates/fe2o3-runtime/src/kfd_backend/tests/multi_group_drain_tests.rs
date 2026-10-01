//! Real router/scheduler custody with CPU receipts, not device execution.

use super::super::ordinary_queue_io::CpuOrdinaryQueueV1;
use super::*;
use crate::{
    RuntimeAllocationIdV1, RuntimeArgumentsV1, RuntimeAsyncCurrentThreadOwnedEngineV1,
    RuntimeAsyncDrainOutcomeV1, RuntimeAsyncDriveErrorV1, RuntimeAsyncEngineConfigV1,
    RuntimeAsyncOwnedDispositionV1, RuntimeAsyncProgressConfigV1, RuntimeBindingV1,
    RuntimeContextV1, RuntimeMemoryRegionV1, RuntimePollV1, RuntimeStreamIdV1,
};
use fe2o3_kfd::CpuFixedDispatchFixtureV1;
use std::sync::{Arc, Mutex};

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
const BYTES: u64 = 4096;

fn context() -> Context {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    RuntimeContextV1::open(
        KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap(),
    )
    .unwrap()
}

fn region(allocation: RuntimeAllocationIdV1, access: RuntimeAccessV1) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: BYTES,
    }
}

fn allocations(context: &mut Context) -> [RuntimeAllocationIdV1; 2] {
    std::array::from_fn(|index| {
        context
            .allocate(
                context.devices()[index].id(),
                RuntimeMemoryKindV1::HostVisible,
                BYTES,
                8,
            )
            .unwrap()
    })
}

fn streams(context: &mut Context) -> [RuntimeStreamIdV1; 2] {
    std::array::from_fn(|index| {
        context
            .create_stream(context.devices()[index].id())
            .unwrap()
    })
}

fn released(report: crate::RuntimeAsyncOwnedShutdownV1<KfdRuntimeBackendErrorV1>) {
    assert_eq!(report.disposition, RuntimeAsyncOwnedDispositionV1::Released);
    assert!(!report.worker_panicked);
    assert!(report.native_failure.is_none());
    let cleanup = report.cleanup.unwrap();
    assert!(cleanup.is_complete());
    assert!(cleanup.failures().is_empty());
}

#[test]
fn opposite_device_copies_drain_fairly_and_expired_deadline_keeps_custody() {
    let mut context = context();
    let streams = streams(&mut context);
    let sources = allocations(&mut context);
    let destinations = allocations(&mut context);
    let completed = Arc::new(Mutex::new(Vec::new()));
    for (index, &source) in sources.iter().enumerate() {
        context
            .write_allocation(source, 0, &vec![17 + index as u8; BYTES as usize])
            .unwrap();
        let target = 1 - index;
        let copy = context
            .peer_copy(
                streams[target],
                region(source, RuntimeAccessV1::Read),
                region(destinations[target], RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        let completed = Arc::clone(&completed);
        context
            .on_completion(&copy, move |status| {
                completed.lock().unwrap().push((target, status))
            })
            .unwrap();
    }
    assert_eq!(context.backend().cooperative_stream_pending_counts.len(), 2);
    assert_eq!(context.backend().cooperative_stream_tails.len(), 2);
    let config = RuntimeAsyncEngineConfigV1::new(8, 8, 1, 1, Duration::from_micros(1)).unwrap();
    let progress = RuntimeAsyncProgressConfigV1::new(8, 1).unwrap();
    let (mut engine, handle) = RuntimeAsyncCurrentThreadOwnedEngineV1::new_with_progress(
        || Ok::<_, ()>(context),
        config,
        progress,
    )
    .unwrap();
    let mut draining = Box::pin(handle.begin_drain(64).unwrap());
    assert_eq!(
        engine.drive_until_ready(draining.as_mut(), Instant::now()),
        Err(RuntimeAsyncDriveErrorV1::DeadlineExceeded)
    );
    assert!(completed.lock().unwrap().is_empty());
    let report = engine
        .drive_until_ready(draining.as_mut(), Instant::now() + Duration::from_secs(2))
        .unwrap()
        .unwrap();
    assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    assert_eq!(report.retained_submissions.total_submissions, 2);
    assert_eq!(report.retained_submissions.succeeded, 2);
    assert_eq!(report.retained_submissions.pending, 0);
    assert_eq!(report.retained_submissions.failed, 0);
    assert!(report.queued_commands_exhausted);
    let mut callbacks = completed.lock().unwrap().clone();
    callbacks.sort_by_key(|(child, _)| *child);
    assert_eq!(
        callbacks,
        vec![
            (0, crate::RuntimeCompletionStatusV1::Succeeded),
            (1, crate::RuntimeCompletionStatusV1::Succeeded),
        ]
    );
    // Drain retained both results. Only explicit shutdown can release them.
    released(engine.shutdown());
    assert_eq!(completed.lock().unwrap().len(), 2);
}

#[test]
fn failed_copy_is_quiescent_but_not_a_successful_group_result() {
    let mut context = context();
    let streams = streams(&mut context);
    let sources = allocations(&mut context);
    let destinations = allocations(&mut context);
    for index in 0..2 {
        let mut copy = context
            .peer_copy(
                streams[1 - index],
                region(sources[index], RuntimeAccessV1::Read),
                region(destinations[1 - index], RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        if index == 0 {
            assert_eq!(
                context.cancel(&mut copy).unwrap(),
                crate::RuntimeCancellationV1::Cancelled
            );
        }
    }
    let (mut engine, handle) = RuntimeAsyncCurrentThreadOwnedEngineV1::new_with_progress(
        || Ok::<_, ()>(context),
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap();
    let mut draining = Box::pin(handle.begin_drain(64).unwrap());
    let report = engine
        .drive_until_ready(draining.as_mut(), Instant::now() + Duration::from_secs(2))
        .unwrap()
        .unwrap();
    assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    assert_eq!(report.retained_submissions.total_submissions, 2);
    assert_eq!(report.retained_submissions.succeeded, 1);
    assert_eq!(report.retained_submissions.failed, 1);
    assert_eq!(report.retained_submissions.pending, 0);
    assert!(report.retained_submissions.first_failure.is_some());
    released(engine.shutdown());
}

struct ReadArguments(RuntimeAllocationIdV1);

impl RuntimeArgumentsV1 for ReadArguments {
    const SIGNATURE_V1: [u8; 32] = [7; 32];

    fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
        let mut bytes = vec![0; 16];
        bytes[8..].copy_from_slice(&13_u64.to_le_bytes());
        bytes
    }

    fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
        vec![RuntimeBindingV1 {
            region: region(self.0, RuntimeAccessV1::Read),
            kernarg_byte_offset: 0,
        }]
    }
}

fn install_cpu_receipts(context: &mut Context) {
    for child in &mut context.backend_mut_for_test_v1().children {
        // Only the first allocation is a synthetic CPU dispatch binding.
        // This provider supplies compute receipts, not native SDMA backing.
        let binding = *child.allocations.keys().min().unwrap();
        let record = child.allocations.get_mut(&binding).unwrap();
        record.sdma_backed = true;
        record.sdma_initialized = true;
        let fixture = CpuFixedDispatchFixtureV1::new().unwrap();
        child.native_compute_lanes[0] = Some(fixture.primary_lane());
        child.native_compute_lanes[1] = Some(fixture.auxiliary_lane());
        child.cpu_queue = Some(Box::new(CpuOrdinaryQueueV1 {
            fixture,
            next_outer_fault: None,
            lane_control: Default::default(),
            before_outer_fault: None,
        }));
        child.native_available = true;
        child.require_cpu_provider_v1().unwrap();
    }
}

fn complete_cpu_receipts(context: &mut Context) {
    for child in &mut context.backend_mut_for_test_v1().children {
        let active = child
            .active
            .as_ref()
            .expect("both child queues were published");
        let Some(ActiveComputeExecutionV1::Materialized(
            MaterializedCompletionReceiptV1::Published(batch),
        )) = active.execution.as_ref()
        else {
            panic!("expected actual opaque CPU publication receipt");
        };
        let handle = child.native_compute_lanes[0].unwrap();
        child
            .cpu_queue
            .as_mut()
            .unwrap()
            .fixture
            .with_lane(handle, |lane| lane.complete_signal(batch).unwrap())
            .unwrap();
    }
}

#[test]
fn typed_producers_on_both_children_publish_before_observation_and_group_cleanup() {
    let mut context = context();
    let streams = streams(&mut context);
    let source = allocations(&mut context);
    let destination = allocations(&mut context);
    let copy_stream = context.create_stream(context.devices()[1].id()).unwrap();
    let kernels: Vec<_> = (0..2)
        .map(|index| {
            let module = context
                .load_module(context.devices()[index].id(), &synthetic_cov6::module())
                .unwrap();
            context
                .resolve_kernel::<ReadArguments>(module, "vecadd")
                .unwrap()
        })
        .collect();
    install_cpu_receipts(&mut context);
    let mut producers: Vec<_> = (0..2)
        .map(|index| {
            let submission = context
                .launch(
                    streams[index],
                    &kernels[index],
                    &ReadArguments(source[index]),
                    crate::RuntimeLaunchGeometryV1 {
                        grid: [64, 1, 1],
                        workgroup: [64, 1, 1],
                        dynamic_shared_bytes: 0,
                    },
                    &[],
                )
                .unwrap();
            context.flush_stream(streams[index]).unwrap();
            submission
        })
        .collect();
    // Both lower queues are published before either completion is signalled or observed.
    assert!(
        context
            .backend()
            .children
            .iter()
            .all(|child| child.active.is_some())
    );
    assert!(
        context
            .peer_copy(
                copy_stream,
                region(source[0], RuntimeAccessV1::Read),
                region(destination[1], RuntimeAccessV1::Write),
                &[],
            )
            .is_err()
    );
    complete_cpu_receipts(&mut context);
    for producer in &mut producers {
        assert_eq!(context.poll(producer).unwrap(), RuntimePollV1::Succeeded);
    }
    let (mut engine, handle) = RuntimeAsyncCurrentThreadOwnedEngineV1::new_with_progress(
        || Ok::<_, ()>(context),
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap();
    let mut draining = Box::pin(handle.begin_drain(64).unwrap());
    let report = engine
        .drive_until_ready(draining.as_mut(), Instant::now() + Duration::from_secs(2))
        .unwrap()
        .unwrap();
    assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    assert_eq!(report.retained_submissions.total_submissions, 2);
    assert_eq!(report.retained_submissions.succeeded, 2);
    released(engine.shutdown());
}

fn retained_copy() -> (KfdMultiDeviceRuntimeBackendV1, u64, u64, [u64; 2]) {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    let stream = backend.create_stream_v1(8).unwrap();
    let allocations = [7, 8].map(|device| {
        backend
            .allocate_v1(device, RuntimeMemoryKindV1::HostVisible, BYTES, 8)
            .unwrap()
    });
    let region = |index, access| BackendMemoryRegionV1 {
        allocation: allocations[index],
        access,
        byte_offset: 0,
        byte_len: BYTES,
    };
    let copy = backend
        .peer_copy_v1(
            stream,
            region(0, RuntimeAccessV1::Read),
            region(1, RuntimeAccessV1::Write),
            &[],
        )
        .unwrap();
    (backend, stream, copy, allocations)
}

#[test]
fn pending_tail_stream_destruction_is_no_effect_and_completed_tail_keeps_event() {
    let (mut backend, stream, copy, allocations) = retained_copy();
    let route = backend.streams[&stream];
    assert!(
        matches!(backend.destroy_stream_v1(stream), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
    );
    assert_eq!(backend.streams[&stream], route);
    assert_eq!(backend.cooperative_stream_tails[&stream], copy);
    assert!(backend.submissions.contains_key(&copy));
    backend.flush_stream_v1(stream).unwrap();
    let event = backend.record_event_v1(stream, copy).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    assert!(!backend.streams.contains_key(&stream));
    assert!(!backend.cooperative_stream_tails.contains_key(&stream));
    assert!(backend.submissions.contains_key(&copy));
    assert!(backend.events.contains_key(&event));
    backend.release_event_v1(event).unwrap();
    backend.release_submission_v1(copy).unwrap();
    for allocation in allocations {
        backend.release_allocation_v1(allocation).unwrap();
    }
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn failed_quiescent_tail_destruction_does_not_release_its_result() {
    let (mut backend, stream, copy, allocations) = retained_copy();
    // Inject a settled copy failure through the actual custody-release transition.
    assert_eq!(
        backend.finish_cooperative_copy(copy, CooperativeCopyPhaseV1::Failed),
        BackendPollV1::Failed {
            code: COOPERATIVE_COPY_FAILURE_CODE_V1
        }
    );
    let event = backend.record_event_v1(stream, copy).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    assert_eq!(
        backend.poll_v1(copy).unwrap(),
        BackendPollV1::Failed {
            code: COOPERATIVE_COPY_FAILURE_CODE_V1
        }
    );
    assert!(backend.events.contains_key(&event));
    backend.release_event_v1(event).unwrap();
    backend.release_submission_v1(copy).unwrap();
    for allocation in allocations {
        backend.release_allocation_v1(allocation).unwrap();
    }
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn stop_retains_pending_peer_custody_instead_of_implicitly_draining() {
    let mut context = context();
    let streams = streams(&mut context);
    let allocations = allocations(&mut context);
    let _copy = context
        .peer_copy(
            streams[1],
            region(allocations[0], RuntimeAccessV1::Read),
            region(allocations[1], RuntimeAccessV1::Write),
            &[],
        )
        .unwrap();
    let (engine, _handle) = RuntimeAsyncCurrentThreadOwnedEngineV1::new_with_progress(
        || Ok::<_, ()>(context),
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .unwrap();
    let shutdown = engine.shutdown();
    assert_eq!(
        shutdown.disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
    assert!(!shutdown.worker_panicked);
    assert!(!shutdown.cleanup.unwrap().is_complete());
}

#[test]
fn contradictory_tail_latches_terminal_without_destroying_child_stream() {
    for corruption in 0..3 {
        let (mut backend, stream, copy, _) = retained_copy();
        backend.flush_stream_v1(stream).unwrap();
        let route = backend.streams[&stream];
        match corruption {
            0 => {
                backend.cooperative_stream_tails.insert(stream, u64::MAX);
            }
            1 => {
                let RoutedSubmissionV1::CooperativeCopy(record) =
                    backend.submissions.get_mut(&copy).unwrap()
                else {
                    unreachable!()
                };
                record.stream = u64::MAX;
            }
            _ => {
                let RoutedSubmissionV1::CooperativeCopy(record) =
                    backend.submissions.get_mut(&copy).unwrap()
                else {
                    unreachable!()
                };
                record.staging.push(1);
            }
        }
        assert!(matches!(
            backend.destroy_stream_v1(stream),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(backend.terminal);
        assert_eq!(backend.streams[&stream], route);
        assert!(
            backend.children[route.child]
                .streams
                .contains_key(&route.local)
        );
        assert!(backend.submissions.contains_key(&copy));
        // Corrupted ownership is deliberately retained; never fabricate repair/cleanup.
        std::mem::forget(backend);
    }
}
