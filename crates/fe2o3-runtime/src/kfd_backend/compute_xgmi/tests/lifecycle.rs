//! Public Context and owned-engine composition with scripted native-route owners.

use super::*;
use crate::kfd_backend::kfd_backend_sdma_seam::ScriptedExecutionOutcomeV1;
use crate::{
    RuntimeAllocationIdV1, RuntimeArgumentsV1, RuntimeAsyncCurrentThreadOwnedEngineV1,
    RuntimeAsyncDrainOutcomeV1, RuntimeAsyncDriveErrorV1, RuntimeAsyncEngineCallErrorV1,
    RuntimeAsyncEngineConfigV1, RuntimeAsyncOwnedDispositionV1, RuntimeAsyncOwnedShutdownV1,
    RuntimeAsyncProgressConfigV1, RuntimeAsyncProgressHandleV1, RuntimeBindingV1,
    RuntimeCancellationV1, RuntimeCompletionFailureV1, RuntimeCompletionStatusV1, RuntimeContextV1,
    RuntimeErrorV1, RuntimeMemoryRegionV1, RuntimePeerCopyV1, RuntimePollV1, RuntimeStreamIdV1,
    RuntimeSubmissionV1, RuntimeValidationErrorV1,
};
use std::sync::{Arc, Mutex};

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
type Copy = RuntimeSubmissionV1<RuntimePeerCopyV1>;
type Engine = RuntimeAsyncCurrentThreadOwnedEngineV1<KfdMultiDeviceRuntimeBackendV1>;
type Handle = RuntimeAsyncProgressHandleV1<KfdMultiDeviceRuntimeBackendV1>;
type Callbacks = Arc<Mutex<Vec<(usize, RuntimeCompletionStatusV1)>>>;

struct ContextFixture {
    context: ManuallyDrop<Context>,
    sources: Vec<RuntimeAllocationIdV1>,
    destinations: Vec<RuntimeAllocationIdV1>,
    streams: Vec<RuntimeStreamIdV1>,
}

fn region(allocation: RuntimeAllocationIdV1, access: RuntimeAccessV1) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: BYTES as u64,
    }
}

fn release_steps() -> [ScriptedSdmaStepV1; 8] {
    [
        ScriptedSdmaStepV1::Allocate {
            kind: ScriptedBufferKindV1::Host,
            byte_len: BYTES,
        },
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len: BYTES,
        },
        ScriptedSdmaStepV1::Submit {
            direction: Gfx942PersistentSdmaDirectionV1::HostToDevice,
            host_offset: 0,
            device_offset: 0,
            copy_bytes: BYTES as u32,
            outcome: ScriptedFailureModeV1::Success,
        },
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]
}

impl ContextFixture {
    fn new(children: usize, pending_samples: usize) -> Self {
        let backends = (0..children)
            .map(|index| {
                let mut child = KfdRuntimeBackendV1::mock();
                child.description.backend_device = 7 + index as u64;
                child
            })
            .collect();
        let mut context = RuntimeContextV1::open(
            KfdMultiDeviceRuntimeBackendV1::from_backends(backends).unwrap(),
        )
        .unwrap();
        let mut sources = Vec::new();
        let mut destinations = Vec::new();
        let mut streams = Vec::new();
        for index in 0..children {
            let device = context.devices()[index].id();
            streams.push(context.create_stream(device).unwrap());
            let source = context
                .allocate(device, RuntimeMemoryKindV1::DeviceLocal, BYTES as u64, 8)
                .unwrap();
            context
                .write_allocation(source, 0, &[0x31 + index as u8; BYTES])
                .unwrap();
            sources.push(source);
            destinations.push(
                context
                    .allocate(device, RuntimeMemoryKindV1::DeviceLocal, BYTES as u64, 8)
                    .unwrap(),
            );
        }
        let backend = context.backend_mut_for_test_v1();
        for child in &mut backend.children {
            let steps = (0..child.allocations.len()).flat_map(|_| release_steps());
            let driver = ScriptedSdmaDriverV1::new(steps);
            for record in child.allocations.values_mut() {
                let mut owner = driver.test_device_owner(BYTES);
                owner
                    .scripted_bytes_mut()
                    .unwrap()
                    .copy_from_slice(&record.bytes);
                record.sdma_storage = KfdRuntimeSdmaStorageV1::Device(Box::new(owner));
                record.sdma_backed = true;
                record.sdma_initialized = true;
                record.sdma_shadow_dirty = true;
            }
            child.native_available = true;
            child.sdma_enabled = true;
            child.peer_visible_device_allocations = true;
            child.scripted_sdma = Some(driver);
        }
        for source in (0..children).step_by(2) {
            for pair in [(source, source + 1), (source + 1, source)] {
                backend.compute_xgmi_routes.insert(
                    pair,
                    Route::Scripted {
                        failure: None,
                        unwind: false,
                        pending_samples,
                    },
                );
            }
        }
        Self {
            context: ManuallyDrop::new(context),
            sources,
            destinations,
            streams,
        }
    }

    fn submit(&mut self, source: usize, destination: usize) -> (Copy, u64) {
        let copy = self
            .context
            .peer_copy(
                self.streams[destination],
                region(self.sources[source], RuntimeAccessV1::Read),
                region(self.destinations[destination], RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        let id = self.context.backend_submission_for_test_v1(&copy).unwrap();
        assert!(self.copy(id).compute_xgmi.is_some());
        assert!(self.copy(id).staging.is_empty());
        (copy, id)
    }

    fn copy(&self, id: u64) -> &CooperativeCopySubmissionV1 {
        let RoutedSubmissionV1::CooperativeCopy(copy) = &self.context.backend().submissions[&id]
        else {
            panic!("native-route cooperative submission expected")
        };
        copy
    }

    fn root(&self, id: u64) -> &Root {
        self.copy(id).compute_xgmi.as_deref().unwrap()
    }

    fn owner(&self, route: RoutedHandleV1) -> &DirectionalSdmaDeviceOwnerV1 {
        let KfdRuntimeSdmaStorageV1::Device(owner) =
            &self.context.backend().children[route.child].allocations[&route.local].sdma_storage
        else {
            panic!("restored device owner expected")
        };
        owner
    }

    fn assert_published(&self, id: u64) {
        let copy = self.copy(id);
        let root = self.root(id);
        assert_eq!(root.phase, Phase::Published);
        assert!(!root.is_quiescent());
        assert!(root.scripted_owners.iter().all(Option::is_some));
        assert!(root.shells.iter().all(Option::is_some));
        for route in [copy.source, copy.destination] {
            let child = &self.context.backend().children[route.child];
            assert!(matches!(
                child.allocations[&route.local].sdma_storage,
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::ComputeXgmi(owner))
                    if owner == id
            ));
            assert_eq!(
                self.context.backend().compute_xgmi_children[route.child],
                Some(id)
            );
        }
    }

    fn on_completion(&mut self, copy: &Copy, tag: usize, callbacks: &Callbacks) {
        let callbacks = Arc::clone(callbacks);
        self.context
            .on_completion(copy, move |status| {
                callbacks.lock().unwrap().push((tag, status))
            })
            .unwrap();
    }

    fn into_engine(self) -> (Engine, Handle) {
        let context = ManuallyDrop::into_inner(self.context);
        RuntimeAsyncCurrentThreadOwnedEngineV1::new_with_progress(
            || Ok::<_, ()>(context),
            RuntimeAsyncEngineConfigV1::new(8, 8, 1, 1, Duration::from_micros(1)).unwrap(),
            RuntimeAsyncProgressConfigV1::new(8, 1).unwrap(),
        )
        .unwrap()
    }

    fn clean(mut self) {
        let cleanup = self.context.cleanup();
        assert!(cleanup.is_complete(), "{cleanup:?}");
        assert!(cleanup.failures().is_empty());
        let backend = self.context.backend_mut_for_test_v1();
        for child in &backend.children {
            let driver = child.scripted_sdma.as_ref().unwrap();
            assert!(driver.is_exhausted());
            assert_eq!(driver.live_owner_count(), 0);
            assert_eq!(driver.unexpected_drops(), 0);
        }
        backend.shutdown_native_v1().unwrap();
        drop(ManuallyDrop::into_inner(self.context));
    }
}

fn released(report: RuntimeAsyncOwnedShutdownV1<KfdRuntimeBackendErrorV1>) {
    assert_eq!(report.disposition, RuntimeAsyncOwnedDispositionV1::Released);
    assert!(!report.worker_panicked);
    assert!(report.native_failure.is_none());
    let cleanup = report.cleanup.unwrap();
    assert!(cleanup.is_complete(), "{cleanup:?}");
    assert!(cleanup.failures().is_empty());
}

fn retained(
    report: RuntimeAsyncOwnedShutdownV1<KfdRuntimeBackendErrorV1>,
    terminal: bool,
    submissions: usize,
    allocations: usize,
) {
    assert_eq!(
        report.disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
    assert!(!report.worker_panicked);
    assert!(report.native_failure.is_none());
    let cleanup = report.cleanup.unwrap();
    assert!(!cleanup.is_complete());
    assert_eq!(cleanup.is_terminal(), terminal);
    assert_eq!(cleanup.retained().submissions, submissions);
    assert_eq!(cleanup.retained().allocations, allocations);
    assert!(cleanup.retained().streams > 0);
}

#[test]
fn public_context_drain_retains_native_custody_and_requires_explicit_event_release() {
    let mut f = ContextFixture::new(2, 3);
    let (mut copy, id) = f.submit(0, 1);
    let routes = [f.copy(id).source, f.copy(id).destination];
    let identities = routes.map(|route| f.owner(route).scripted_owner_id());
    let callbacks = Callbacks::default();
    f.on_completion(&copy, 0, &callbacks);
    let event = f.context.record_event(&copy).unwrap();
    assert!(matches!(
        f.context.drain(&mut copy, Instant::now()),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::InvalidDeadline
        ))
    ));
    assert!(f.root(id).trace.is_empty());
    assert!(
        f.context
            .backend()
            .compute_xgmi_children
            .iter()
            .all(Option::is_none)
    );
    f.context.flush_stream(f.streams[1]).unwrap();
    f.assert_published(id);
    let trace = f.root(id).trace.clone();
    assert!(matches!(
        f.context.drain(&mut copy, Instant::now()),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::InvalidDeadline
        ))
    ));
    assert_eq!(f.root(id).trace, trace);
    assert_eq!(
        f.context.cancel(&mut copy).unwrap(),
        RuntimeCancellationV1::TooLate
    );
    let failure = f.context.release_submission(copy).unwrap_err();
    assert!(matches!(
        failure.error(),
        RuntimeErrorV1::Validation(RuntimeValidationErrorV1::SubmissionPending)
    ));
    let (mut copy, _) = failure.into_parts();
    assert!(callbacks.lock().unwrap().is_empty());
    assert_eq!(
        f.context
            .drain(&mut copy, Instant::now() + Duration::from_secs(2))
            .unwrap(),
        RuntimePollV1::Succeeded
    );
    assert!(f.root(id).is_quiescent());
    assert_eq!(
        routes.map(|route| f.owner(route).scripted_owner_id()),
        identities
    );
    assert_eq!(f.owner(routes[1]).scripted_bytes().unwrap(), &[0x31; BYTES]);
    assert!(
        f.context
            .backend()
            .compute_xgmi_children
            .iter()
            .all(Option::is_none)
    );
    assert_eq!(
        f.context.query_submission(&copy).unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    let failure = f.context.release_submission(copy).unwrap_err();
    assert!(matches!(
        failure.error(),
        RuntimeErrorV1::Validation(RuntimeValidationErrorV1::SubmissionRetainedByEvent)
    ));
    let (copy, _) = failure.into_parts();
    f.context.release_event(event).unwrap();
    f.context.release_submission(copy).unwrap();
    assert!(!f.context.backend().submissions.contains_key(&id));
    assert_eq!(
        *callbacks.lock().unwrap(),
        [(0, RuntimeCompletionStatusV1::Succeeded)]
    );
    f.clean();
}

#[test]
fn native_group_drain_resumes_original_deadline_future_for_opposite_routes() {
    let mut f = ContextFixture::new(2, 4);
    let callbacks = Callbacks::default();
    let (first, first_id) = f.submit(0, 1);
    let (second, second_id) = f.submit(1, 0);
    f.on_completion(&first, 0, &callbacks);
    f.on_completion(&second, 1, &callbacks);
    f.context.flush_stream(f.streams[1]).unwrap();
    f.assert_published(first_id);
    f.context.flush_stream(f.streams[0]).unwrap();
    assert!(f.root(second_id).trace.is_empty());
    f.assert_published(first_id);
    let (mut engine, handle) = f.into_engine();
    let mut draining = Box::pin(handle.begin_drain(64).unwrap());
    engine.tick().unwrap();
    assert_eq!(
        engine.drive_until_ready(draining.as_mut(), Instant::now()),
        Err(RuntimeAsyncDriveErrorV1::DeadlineExceeded)
    );
    assert!(callbacks.lock().unwrap().is_empty());
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
    assert_eq!(
        *callbacks.lock().unwrap(),
        [
            (0, RuntimeCompletionStatusV1::Succeeded),
            (1, RuntimeCompletionStatusV1::Succeeded)
        ]
    );
    // A quiescent drain observes retained results; shutdown separately retires them.
    released(engine.shutdown());
    assert_eq!(callbacks.lock().unwrap().len(), 2);
}

#[test]
fn native_group_drain_distinguishes_disjoint_success_and_cancelled_result() {
    let mut f = ContextFixture::new(4, 2);
    let callbacks = Callbacks::default();
    let (copy, id) = f.submit(0, 1);
    let (mut cancelled, cancelled_id) = f.submit(2, 3);
    f.on_completion(&copy, 0, &callbacks);
    f.on_completion(&cancelled, 1, &callbacks);
    assert_eq!(
        f.context.cancel(&mut cancelled).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert!(f.root(cancelled_id).trace.is_empty());
    f.context.flush_stream(f.streams[1]).unwrap();
    f.assert_published(id);
    let (mut engine, handle) = f.into_engine();
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
    assert_eq!(
        *callbacks.lock().unwrap(),
        [
            (
                1,
                RuntimeCompletionStatusV1::Failed(RuntimeCompletionFailureV1::Cancelled)
            ),
            (0, RuntimeCompletionStatusV1::Succeeded),
        ]
    );
    released(engine.shutdown());
}

#[test]
fn native_group_drain_uncertainty_retains_ownership_after_disjoint_success() {
    let mut f = ContextFixture::new(4, 1);
    f.context
        .backend_mut_for_test_v1()
        .compute_xgmi_routes
        .insert(
            (2, 3),
            Route::Scripted {
                failure: Some(Stage::Retire),
                unwind: false,
                pending_samples: 1,
            },
        );
    let callbacks = Callbacks::default();
    let (good, good_id) = f.submit(0, 1);
    let (uncertain, uncertain_id) = f.submit(2, 3);
    f.on_completion(&good, 0, &callbacks);
    f.on_completion(&uncertain, 1, &callbacks);
    for stream in [f.streams[1], f.streams[3]] {
        f.context.flush_stream(stream).unwrap();
    }
    f.assert_published(good_id);
    f.assert_published(uncertain_id);
    let (mut engine, handle) = f.into_engine();
    let mut draining = Box::pin(handle.begin_drain(64).unwrap());
    assert_eq!(
        engine.drive_until_ready(draining.as_mut(), Instant::now() + Duration::from_secs(2)),
        Err(RuntimeAsyncDriveErrorV1::EngineStopped)
    );
    assert_eq!(
        *callbacks.lock().unwrap(),
        [(0, RuntimeCompletionStatusV1::Succeeded)]
    );
    // Terminal ambiguity is not a quiescent failed result or permission to clean up.
    retained(engine.shutdown(), true, 2, 8);
    assert_eq!(
        std::future::Future::poll(
            draining.as_mut(),
            &mut std::task::Context::from_waker(std::task::Waker::noop()),
        ),
        std::task::Poll::Ready(Err(RuntimeAsyncEngineCallErrorV1::EngineStopped))
    );
}

#[test]
fn native_group_stop_and_tick_exhaustion_retain_published_roots() {
    for mode in 0..3 {
        let mut f = ContextFixture::new(2, 8);
        let callbacks = Callbacks::default();
        let (copy, id) = f.submit(0, 1);
        f.on_completion(&copy, 0, &callbacks);
        f.context.flush_stream(f.streams[1]).unwrap();
        f.assert_published(id);
        let (mut engine, handle) = f.into_engine();
        let mut draining = (mode != 0).then(|| Box::pin(handle.begin_drain(1).unwrap()));
        if mode == 2 {
            let report = engine
                .drive_until_ready(
                    draining.as_mut().unwrap().as_mut(),
                    Instant::now() + Duration::from_secs(2),
                )
                .unwrap()
                .unwrap();
            assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::BudgetExhausted);
            assert_eq!(report.retained_submissions.total_submissions, 1);
            assert_eq!(report.retained_submissions.pending, 1);
            assert_eq!(report.retained_submissions.succeeded, 0);
            assert_eq!(report.retained_submissions.failed, 0);
            assert_eq!(report.ticks, 1);
        }
        retained(engine.shutdown(), mode != 0, 1, 4);
        if mode == 1 {
            assert_eq!(
                std::future::Future::poll(
                    draining.as_mut().unwrap().as_mut(),
                    &mut std::task::Context::from_waker(std::task::Waker::noop()),
                ),
                std::task::Poll::Ready(Err(RuntimeAsyncEngineCallErrorV1::EngineStopped))
            );
        }
        assert!(callbacks.lock().unwrap().is_empty());
    }
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

#[test]
fn public_context_pending_native_copy_does_not_authorize_compute_consumer() {
    let mut f = ContextFixture::new(2, 2);
    let device = f.context.devices()[1].id();
    let module = f
        .context
        .load_module(device, &crate::synthetic_cov6::module())
        .unwrap();
    let kernel = f
        .context
        .resolve_kernel::<ReadArguments>(module, "vecadd")
        .unwrap();
    let consumer_stream = f.context.create_stream(device).unwrap();
    let (mut copy, id) = f.submit(0, 1);
    let event = f.context.record_event(&copy).unwrap();
    f.context.flush_stream(f.streams[1]).unwrap();
    f.assert_published(id);
    let trace = f.root(id).trace.clone();
    let result = f.context.launch(
        consumer_stream,
        &kernel,
        &ReadArguments(f.destinations[1]),
        crate::RuntimeLaunchGeometryV1 {
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
            dynamic_shared_bytes: 0,
        },
        &[event],
    );
    assert!(matches!(
        result,
        Err(RuntimeErrorV1::BackendRejected(ref error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
    ));
    assert!(!f.context.is_terminal());
    assert_eq!(f.root(id).trace, trace);
    f.assert_published(id);
    assert_eq!(f.context.backend().submissions.len(), 1);
    assert!(
        f.context.backend().children.iter().all(|child| {
            child.pending_compute.is_empty() && child.allocation_custody.is_empty()
        })
    );
    // Pending ordinary native copy -> compute remains unsupported, not pipeline parity.
    assert_eq!(
        f.context
            .drain(&mut copy, Instant::now() + Duration::from_secs(2))
            .unwrap(),
        RuntimePollV1::Succeeded
    );
    f.context.release_event(event).unwrap();
    f.context.release_submission(copy).unwrap();
    f.clean();
}
