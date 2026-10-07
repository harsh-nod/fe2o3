//! Public queued admission; scripted copy completion never represents GPU compute.

use super::*;
use crate::kfd_backend::kfd_backend_sdma_seam::ScriptedExecutionOutcomeV1;
use crate::{
    RuntimeAllocationIdV1, RuntimeArgumentsV1, RuntimeBindingV1, RuntimeCancellationV1,
    RuntimeContextV1, RuntimeErrorV1, RuntimeEventIdV1, RuntimeLaunchGeometryV1,
    RuntimeMemoryRegionV1, RuntimeModuleIdV1, RuntimePeerCopyV1, RuntimePollV1, RuntimeStreamIdV1,
    RuntimeSubmissionV1, RuntimeValidationErrorV1, TypedRuntimeKernelV1,
};

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
type Copy = RuntimeSubmissionV1<RuntimePeerCopyV1>;
type Consumer = RuntimeSubmissionV1<Arguments>;

#[derive(Debug)]
struct Arguments(Vec<RuntimeMemoryRegionV1>);

impl RuntimeArgumentsV1 for Arguments {
    const SIGNATURE_V1: [u8; 32] = [7; 32];

    fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
        vec![0; 16]
    }

    fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
        self.0
            .iter()
            .enumerate()
            .map(|(index, region)| RuntimeBindingV1 {
                region: *region,
                kernarg_byte_offset: index as u32 * 8,
            })
            .collect()
    }
}

fn region(allocation: RuntimeAllocationIdV1, access: RuntimeAccessV1) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: BYTES as u64,
    }
}

fn geometry() -> RuntimeLaunchGeometryV1 {
    RuntimeLaunchGeometryV1 {
        grid: [64, 1, 1],
        workgroup: [64, 1, 1],
        dynamic_shared_bytes: 0,
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

struct Fixture {
    context: ManuallyDrop<Context>,
    source: RuntimeAllocationIdV1,
    destination: RuntimeAllocationIdV1,
    gate_allocation: RuntimeAllocationIdV1,
    copy_stream: RuntimeStreamIdV1,
    consumer_stream: RuntimeStreamIdV1,
    module: RuntimeModuleIdV1,
    kernel: TypedRuntimeKernelV1<Arguments>,
    gate: Option<Consumer>,
    gate_route: RoutedHandleV1,
    events: Vec<RuntimeEventIdV1>,
}

impl Fixture {
    fn new(failure: Option<Stage>, unwind: bool, pending_samples: usize) -> Self {
        let children = (0..2)
            .map(|index| {
                let mut child = KfdRuntimeBackendV1::mock();
                child.description.backend_device = 7 + index;
                child
            })
            .collect();
        let mut context = Context::open_with_version_journal_members_v1(
            KfdMultiDeviceRuntimeBackendV1::from_backends(children).unwrap(),
            16,
            16,
            16,
        )
        .unwrap();
        let source_device = context.devices()[0].id();
        let destination_device = context.devices()[1].id();
        let source = context
            .allocate(
                source_device,
                RuntimeMemoryKindV1::DeviceLocal,
                BYTES as u64,
                8,
            )
            .unwrap();
        let destination = context
            .allocate(
                destination_device,
                RuntimeMemoryKindV1::DeviceLocal,
                BYTES as u64,
                8,
            )
            .unwrap();
        let gate_allocation = context
            .allocate(
                destination_device,
                RuntimeMemoryKindV1::DeviceLocal,
                BYTES as u64,
                8,
            )
            .unwrap();
        for (allocation, byte) in [(source, 0x53), (destination, 0x17), (gate_allocation, 0x29)] {
            context
                .write_allocation(allocation, 0, &[byte; BYTES])
                .unwrap();
        }
        let copy_stream = context.create_stream(destination_device).unwrap();
        let consumer_stream = context.create_stream(destination_device).unwrap();
        let gate_stream = context.create_stream(destination_device).unwrap();
        let module = context
            .load_module(destination_device, &crate::synthetic_cov6::module())
            .unwrap();
        let kernel = context
            .resolve_kernel::<Arguments>(module, "vecadd")
            .unwrap();
        let gate_module = context
            .load_module(destination_device, &crate::synthetic_cov6::module())
            .unwrap();
        let gate_kernel = context
            .resolve_kernel::<Arguments>(gate_module, "vecadd")
            .unwrap();
        let backend = context.backend_mut_for_test_v1();
        for child in &mut backend.children {
            let driver = ScriptedSdmaDriverV1::new(
                (0..child.allocations.len()).flat_map(|_| release_steps()),
            );
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
        backend.compute_xgmi_routes.insert(
            (0, 1),
            Route::Scripted {
                failure,
                unwind,
                pending_samples,
            },
        );
        // This genuine native submission remains unflushed on an unrelated stream.
        let gate = context
            .launch_producer_aware_v1(
                gate_stream,
                &gate_kernel,
                &Arguments(vec![region(gate_allocation, RuntimeAccessV1::Read)]),
                geometry(),
                &[],
            )
            .unwrap();
        let gate_id = context.backend_submission_for_test_v1(&gate).unwrap();
        let RoutedSubmissionV1::Native {
            route: gate_route, ..
        } = context.backend().submissions[&gate_id]
        else {
            panic!("ordinary native gate submission expected")
        };
        let gate_event = context.record_event(&gate).unwrap();
        Self {
            context: ManuallyDrop::new(context),
            source,
            destination,
            gate_allocation,
            copy_stream,
            consumer_stream,
            module,
            kernel,
            gate: Some(gate),
            gate_route,
            events: vec![gate_event],
        }
    }

    fn submit_copy(&mut self) -> (Copy, u64) {
        self.submit_copy_range(0, 0, BYTES as u64)
    }

    fn submit_copy_range(
        &mut self,
        source_offset: u64,
        destination_offset: u64,
        bytes: u64,
    ) -> (Copy, u64) {
        let mut source = region(self.source, RuntimeAccessV1::Read);
        source.byte_offset = source_offset;
        source.byte_len = bytes;
        let mut destination = region(self.destination, RuntimeAccessV1::Write);
        destination.byte_offset = destination_offset;
        destination.byte_len = bytes;
        let copy = self
            .context
            .peer_copy(self.copy_stream, source, destination, &[])
            .unwrap();
        let id = self.context.backend_submission_for_test_v1(&copy).unwrap();
        self.events.push(self.context.record_event(&copy).unwrap());
        assert!(self.copy(id).compute_xgmi.is_some());
        assert!(self.copy(id).staging.is_empty());
        (copy, id)
    }

    fn launch(
        &mut self,
        arguments: Arguments,
        events: &[RuntimeEventIdV1],
    ) -> Result<Consumer, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.context.launch_producer_aware_v1(
            self.consumer_stream,
            &self.kernel,
            &arguments,
            geometry(),
            events,
        )
    }

    fn consumer(&mut self) -> (Consumer, u64) {
        let events = self.events.clone();
        let consumer = self
            .launch(
                Arguments(vec![region(self.destination, RuntimeAccessV1::Read)]),
                &events,
            )
            .unwrap();
        let id = self
            .context
            .backend_submission_for_test_v1(&consumer)
            .unwrap();
        (consumer, id)
    }

    fn copy(&self, id: u64) -> &CooperativeCopySubmissionV1 {
        let RoutedSubmissionV1::CooperativeCopy(copy) = &self.context.backend().submissions[&id]
        else {
            panic!("native-route cooperative copy expected")
        };
        copy
    }

    fn root(&self, id: u64) -> &Root {
        self.copy(id).compute_xgmi.as_deref().unwrap()
    }

    fn assert_waiting(&self, consumer: u64, copy: u64, published: bool) {
        let backend = self.context.backend();
        let RoutedSubmissionV1::DeferredCompute(deferred) = &backend.submissions[&consumer] else {
            panic!("router-owned deferred compute expected")
        };
        assert!(deferred.route.is_none());
        assert_eq!(deferred.status, BackendPollV1::Pending);
        assert!(backend.children[0].pending_compute.is_empty());
        assert_eq!(backend.children[1].pending_compute.len(), 1);
        assert!(
            backend.children[1]
                .pending_compute
                .contains_key(&self.gate_route.local)
        );
        let copy_id = copy;
        let copy = self.copy(copy_id);
        for (index, endpoint) in [copy.source, copy.destination].into_iter().enumerate() {
            assert!(
                !backend.children[endpoint.child]
                    .allocation_custody
                    .contains_key(&endpoint.local)
            );
            assert_eq!(
                backend.compute_xgmi_children[endpoint.child].is_some(),
                published
            );
            let record = &backend.children[endpoint.child].allocations[&endpoint.local];
            let owner = if published {
                assert!(matches!(record.sdma_storage,
                    KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::ComputeXgmi(owner))
                        if owner == copy_id));
                self.root(copy_id).scripted_owners[index].as_ref().unwrap()
            } else {
                let KfdRuntimeSdmaStorageV1::Device(owner) = &record.sdma_storage else {
                    panic!("unpublished copy leaves the exact device mapping in its slot")
                };
                owner
            };
            assert_eq!(
                owner.scripted_bytes().unwrap(),
                &[if index == 0 { 0x53 } else { 0x17 }; BYTES]
            );
        }
        assert!(
            backend
                .children
                .iter()
                .all(|child| !child.any_compute_active_v1())
        );
    }

    fn restored(&self, id: u64) -> [Option<u64>; 2] {
        let copy = self.copy(id);
        let mut identities = [None, None];
        for (index, endpoint) in [copy.source, copy.destination].into_iter().enumerate() {
            let record =
                &self.context.backend().children[endpoint.child].allocations[&endpoint.local];
            let KfdRuntimeSdmaStorageV1::Device(owner) = &record.sdma_storage else {
                panic!("restored exact device owner expected")
            };
            let mut expected = [if index == 0 { 0x53 } else { 0x17 }; BYTES];
            if index == 1 && copy.status() == BackendPollV1::Succeeded {
                let start = copy.destination_region.byte_offset as usize;
                expected[start..start + copy.destination_region.byte_len as usize].fill(0x53);
            }
            assert_eq!(owner.scripted_bytes().unwrap(), expected);
            identities[index] = owner.scripted_owner_id();
        }
        identities
    }

    fn release_events(&mut self) {
        for event in core::mem::take(&mut self.events) {
            self.context.release_event(event).unwrap();
        }
    }

    fn clean(mut self, mut copy: Copy, mut consumer: Consumer) {
        self.context.cancel(&mut consumer).unwrap();
        if self.context.poll(&mut copy).unwrap() == RuntimePollV1::Pending {
            self.context
                .drain(&mut copy, Instant::now() + Duration::from_secs(2))
                .unwrap();
        }
        let mut gate = self.gate.take().unwrap();
        assert_eq!(
            self.context.cancel(&mut gate).unwrap(),
            RuntimeCancellationV1::Cancelled
        );
        self.release_events();
        self.context.release_submission(consumer).unwrap();
        self.context.release_submission(gate).unwrap();
        self.context.release_submission(copy).unwrap();
        let report = self.context.cleanup();
        assert!(report.is_complete(), "{report:?}");
        assert!(report.failures().is_empty());
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

#[test]
fn queued_native_consumer_admits_before_and_after_publication_without_endpoint_custody() {
    for published in [false, true] {
        let mut f = Fixture::new(None, false, 2);
        let (copy, copy_id) = f.submit_copy();
        let owners = f.restored(copy_id);
        if published {
            f.context.flush_stream(f.copy_stream).unwrap();
            assert_eq!(f.root(copy_id).phase, Phase::Published);
        }
        let trace = f.root(copy_id).trace.clone();
        let (mut consumer, id) = f.consumer();
        f.assert_waiting(id, copy_id, published);
        if published {
            assert_eq!(
                f.root(copy_id)
                    .scripted_owners
                    .each_ref()
                    .map(|owner| owner.as_ref().and_then(|owner| owner.scripted_owner_id())),
                owners,
            );
        }
        assert_eq!(f.root(copy_id).trace, trace);
        for _ in 0..2 {
            assert_eq!(
                f.context.poll(&mut consumer).unwrap(),
                RuntimePollV1::Pending
            );
            assert_eq!(
                f.context.wait(&mut consumer, Duration::ZERO).unwrap(),
                RuntimePollV1::Pending
            );
            f.assert_waiting(id, copy_id, published);
            assert_eq!(f.root(copy_id).trace, trace);
        }
        assert!(matches!(
            f.context.drain(&mut consumer, Instant::now()),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::InvalidDeadline
            ))
        ));
        assert_eq!(f.root(copy_id).trace, trace);
        assert_eq!(
            f.context
                .drain(&mut consumer, Instant::now() + Duration::from_millis(20))
                .unwrap(),
            RuntimePollV1::Pending,
        );
        f.context.flush_stream(f.consumer_stream).unwrap();
        for _ in 0..8 {
            f.context.flush_stream(f.consumer_stream).unwrap();
            if f.copy(copy_id).status() == BackendPollV1::Succeeded {
                break;
            }
        }
        assert_eq!(f.copy(copy_id).status(), BackendPollV1::Succeeded);
        assert_eq!(
            f.root(copy_id).trace,
            [
                Stage::Create,
                Stage::Copy,
                Stage::Poll,
                Stage::Poll,
                Stage::Poll,
                Stage::Finish,
                Stage::Retire,
                Stage::Restore
            ]
        );
        assert_eq!(f.restored(copy_id), owners);
        assert!(
            f.context
                .backend()
                .compute_xgmi_children
                .iter()
                .all(Option::is_none)
        );
        assert_eq!(f.context.backend().completed_compute_xgmi_copies, 0);
        let RoutedSubmissionV1::DeferredCompute(deferred) = &f.context.backend().submissions[&id]
        else {
            panic!("deferred routing identity retained through handoff")
        };
        let route = deferred
            .route
            .expect("native consumer admitted after copy restoration");
        assert_eq!(route.child, 1);
        assert!(
            f.context.backend().children[1]
                .pending_compute
                .contains_key(&route.local)
        );
        assert_eq!(
            f.context.poll(&mut consumer).unwrap(),
            RuntimePollV1::Pending
        );
        assert!(
            f.context
                .backend()
                .children
                .iter()
                .all(|child| !child.any_compute_active_v1())
        );
        f.clean(copy, consumer);
    }
}

#[test]
fn queued_native_consumer_retains_resources_after_public_events_are_released() {
    let mut f = Fixture::new(None, false, 2);
    let (copy, copy_id) = f.submit_copy();
    let (consumer, id) = f.consumer();
    f.release_events();
    f.assert_waiting(id, copy_id, false);
    assert!(f.context.release_allocation(f.destination).is_err());
    assert!(f.context.release_allocation(f.source).is_err());
    assert!(f.context.release_allocation(f.gate_allocation).is_err());
    assert!(f.context.unload_module(f.module).is_err());
    assert!(f.context.destroy_stream(f.consumer_stream).is_err());
    assert!(f.context.destroy_stream(f.copy_stream).is_err());
    assert!(matches!(
        f.context
            .backend_mut_for_test_v1()
            .release_submission_v1(copy_id),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    f.assert_waiting(id, copy_id, false);
    f.clean(copy, consumer);
}

#[test]
fn deferred_compute_event_rejects_exact_downstream_and_retains_independently() {
    let mut f = Fixture::new(None, false, 2);
    let device = f.context.devices()[1].id();
    let downstream_stream = f.context.create_stream(device).unwrap();
    let (copy, copy_id) = f.submit_copy();
    let (mut consumer, id) = f.consumer();
    let first_event = f.context.record_event(&consumer).unwrap();
    let second_event = f.context.record_event(&consumer).unwrap();
    let gate_id = f
        .context
        .backend_submission_for_test_v1(f.gate.as_ref().unwrap())
        .unwrap();
    let submissions = f.context.backend().submissions.len();
    let event_retains = f.context.backend().event_submission_retain_counts.clone();
    assert_eq!(event_retains[&id], 2);
    assert!(matches!(
        f.context.launch_producer_aware_v1(
            downstream_stream,
            &f.kernel,
            &Arguments(vec![region(f.gate_allocation, RuntimeAccessV1::Read)]),
            geometry(),
            &[first_event],
        ),
        Err(RuntimeErrorV1::BackendRejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported
    ));
    assert_eq!(f.context.backend().submissions.len(), submissions);
    assert_eq!(
        f.context.backend().event_submission_retain_counts,
        event_retains
    );
    assert!(!f.context.is_terminal());
    f.assert_waiting(id, copy_id, false);
    assert!(f.root(copy_id).trace.is_empty());

    f.context.release_event(first_event).unwrap();
    f.release_events();
    let backend = f.context.backend();
    assert_eq!(backend.event_submission_retain_counts[&id], 1);
    for producer in [copy_id, gate_id] {
        assert!(
            !backend
                .event_submission_retain_counts
                .contains_key(&producer)
        );
        assert!(backend.peer_launch_retains.retains(producer));
    }
    assert_eq!(
        f.context.cancel(&mut consumer).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    let backend = f.context.backend();
    assert_eq!(backend.event_submission_retain_counts[&id], 1);
    assert!(!backend.peer_launch_retains.retains(copy_id));
    assert!(!backend.peer_launch_retains.retains(gate_id));
    assert!(matches!(
        f.context.backend_mut_for_test_v1().release_submission_v1(id),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
    ));
    assert_eq!(f.copy(copy_id).status(), BackendPollV1::Pending);
    assert!(f.root(copy_id).trace.is_empty());
    f.context.release_event(second_event).unwrap();
    assert!(
        !f.context
            .backend()
            .event_submission_retain_counts
            .contains_key(&id)
    );
    f.context.destroy_stream(downstream_stream).unwrap();
    f.clean(copy, consumer);
}

#[test]
fn queued_native_consumer_cancellation_does_not_cancel_its_published_copy() {
    for published in [false, true] {
        let mut f = Fixture::new(None, false, 2);
        let (copy, copy_id) = f.submit_copy();
        let (mut consumer, id) = f.consumer();
        if published {
            f.context.flush_stream(f.copy_stream).unwrap();
        }
        let trace = f.root(copy_id).trace.clone();
        assert_eq!(
            f.context.cancel(&mut consumer).unwrap(),
            RuntimeCancellationV1::Cancelled
        );
        assert_eq!(f.root(copy_id).trace, trace);
        assert_eq!(f.copy(copy_id).status(), BackendPollV1::Pending);
        let RoutedSubmissionV1::DeferredCompute(deferred) = &f.context.backend().submissions[&id]
        else {
            panic!("cancelled router identity remains releasable")
        };
        assert!(deferred.route.is_none());
        assert!(matches!(deferred.status, BackendPollV1::Failed { .. }));
        f.clean(copy, consumer);
    }
}

#[test]
fn queued_native_consumer_failed_copy_never_acquires_child_compute_custody() {
    let mut f = Fixture::new(None, false, 2);
    let (mut copy, copy_id) = f.submit_copy();
    let (mut consumer, id) = f.consumer();
    assert_eq!(
        f.context.cancel(&mut copy).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert!(matches!(
        f.context
            .drain(&mut consumer, Instant::now() + Duration::from_secs(2))
            .unwrap(),
        RuntimePollV1::Failed { .. }
    ));
    assert!(f.root(copy_id).trace.is_empty());
    let RoutedSubmissionV1::DeferredCompute(deferred) = &f.context.backend().submissions[&id]
    else {
        panic!("failed router identity remains releasable")
    };
    assert!(deferred.route.is_none());
    assert_eq!(f.context.backend().children[1].pending_compute.len(), 1);
    assert!(!f.context.is_terminal());
    f.clean(copy, consumer);
}

#[test]
fn queued_native_consumer_native_uncertainty_never_hands_off_or_drops_copy_owners() {
    for stage in STAGES {
        for unwind in [false, true] {
            let mut f = Fixture::new(Some(stage), unwind, 0);
            let (_copy, copy_id) = f.submit_copy();
            let owners = f.restored(copy_id);
            let (mut consumer, id) = f.consumer();
            let result = catch_unwind(AssertUnwindSafe(|| {
                f.context
                    .drain(&mut consumer, Instant::now() + Duration::from_secs(2))
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert!(f.context.is_terminal());
            let RoutedSubmissionV1::DeferredCompute(deferred) =
                &f.context.backend().submissions[&id]
            else {
                panic!("uncertain deferred consumer remains rooted")
            };
            assert!(deferred.route.is_none());
            let root = f.root(copy_id);
            assert_eq!(
                root.scripted_owners
                    .each_ref()
                    .map(|owner| owner.as_ref().and_then(|owner| owner.scripted_owner_id())),
                owners
            );
            assert!(root.shells.iter().all(Option::is_some));
            assert_eq!(f.context.backend().children[1].pending_compute.len(), 1);
            for child in &f.context.backend().children {
                assert_eq!(child.scripted_sdma.as_ref().unwrap().unexpected_drops(), 0);
            }
            // Terminal ownership deliberately remains retained until process teardown.
        }
    }
}

#[test]
fn queued_native_consumer_rejects_missing_duplicate_wrong_device_and_unsupported_bindings() {
    for variant in 0..6 {
        let mut f = Fixture::new(None, false, 1);
        let (copy, copy_id) = f.submit_copy();
        let mut events = f.events.clone();
        let mut bindings = vec![region(f.destination, RuntimeAccessV1::Read)];
        match variant {
            0 => {
                events.pop();
            }
            1 => {
                events.push(events[1]);
            }
            2 => {
                bindings[0].allocation = f.source;
            }
            3 => {
                bindings[0].byte_offset = 1;
            }
            4 => {
                bindings[0].access = RuntimeAccessV1::Write;
            }
            5 => {
                bindings.push(region(f.destination, RuntimeAccessV1::Write));
            }
            _ => unreachable!(),
        }
        let before = f.context.backend().submissions.len();
        let expected = match variant {
            1 => RuntimeValidationErrorV1::DuplicateDependency,
            2 => RuntimeValidationErrorV1::WrongDevice,
            3 => RuntimeValidationErrorV1::InvalidRange,
            _ => RuntimeValidationErrorV1::ContextReserved,
        };
        assert!(matches!(f.launch(Arguments(bindings), &events),
            Err(RuntimeErrorV1::Validation(error)) if error == expected));
        assert_eq!(f.context.backend().submissions.len(), before);
        assert!(f.root(copy_id).trace.is_empty());
        assert!(!f.context.is_terminal());
        let (consumer, id) = f.consumer();
        f.assert_waiting(id, copy_id, false);
        f.clean(copy, consumer);
    }
}

fn drive_restored_handoff(f: &mut Fixture, copy_id: u64, id: u64) -> RoutedHandleV1 {
    for _ in 0..8 {
        let RoutedSubmissionV1::DeferredCompute(deferred) = &f.context.backend().submissions[&id]
        else {
            panic!("deferred routing identity remains retained")
        };
        if let Some(route) = deferred.route {
            assert_eq!(f.copy(copy_id).status(), BackendPollV1::Succeeded);
            assert_eq!(f.root(copy_id).phase, Phase::Retired);
            assert!(
                f.context
                    .backend()
                    .compute_xgmi_children
                    .iter()
                    .all(Option::is_none)
            );
            assert!(
                f.context
                    .backend()
                    .children
                    .iter()
                    .all(|child| !child.any_compute_active_v1())
            );
            return route;
        }
        if f.copy(copy_id).status() == BackendPollV1::Pending {
            for endpoint in [f.copy(copy_id).source, f.copy(copy_id).destination] {
                assert!(
                    !f.context.backend().children[endpoint.child]
                        .allocation_custody
                        .contains_key(&endpoint.local)
                );
            }
            assert_eq!(f.context.backend().children[1].pending_compute.len(), 1);
        }
        f.context.flush_stream(f.consumer_stream).unwrap();
    }
    panic!("bounded native-copy progress must restore and hand off the consumer")
}

#[test]
fn queued_native_consumer_preserves_contained_read_ranges_and_read_aliases() {
    for aliases in [false, true] {
        let mut f = Fixture::new(None, false, 2);
        let (copy, copy_id) = f.submit_copy();
        let owners = f.restored(copy_id);
        let mut first = region(f.destination, RuntimeAccessV1::Read);
        first.byte_offset = 8;
        first.byte_len = 16;
        let mut bindings = vec![first];
        let mut events = f.events.clone();
        if aliases {
            let mut second = first;
            second.byte_offset = 32;
            second.byte_len = 8;
            bindings.push(second);
            let alias = f.context.record_event(&copy).unwrap();
            events[1] = alias;
            f.events.push(alias);
        }
        let mut consumer = f.launch(Arguments(bindings.clone()), &events).unwrap();
        let id = f.context.backend_submission_for_test_v1(&consumer).unwrap();
        f.assert_waiting(id, copy_id, false);
        f.release_events();
        assert_eq!(
            f.context.poll(&mut consumer).unwrap(),
            RuntimePollV1::Pending
        );
        assert!(f.root(copy_id).trace.is_empty());
        let route = drive_restored_handoff(&mut f, copy_id, id);
        assert_eq!(f.restored(copy_id), owners);
        let pending = &f.context.backend().children[route.child].pending_compute[&route.local];
        let actual = &pending.launch.bindings;
        assert_eq!(actual.len(), bindings.len());
        for (index, (actual, expected)) in actual.iter().zip(&bindings).enumerate() {
            assert_eq!(actual.region.allocation, f.copy(copy_id).destination.local);
            assert_eq!(actual.region.access, RuntimeAccessV1::Read);
            assert_eq!(actual.region.byte_offset, expected.byte_offset);
            assert_eq!(actual.region.byte_len, expected.byte_len);
            assert_eq!(actual.kernarg_byte_offset, index as u32 * 8);
        }
        assert_eq!(
            pending.retained_allocations.as_ref(),
            &[f.copy(copy_id).destination.local]
        );
        assert_eq!(
            f.context.wait(&mut consumer, Duration::ZERO).unwrap(),
            RuntimePollV1::Pending
        );
        f.clean(copy, consumer);
    }
}

#[test]
fn native_subrange_deferred_consumer_accepts_only_contained_reads_and_restores_whole_owners() {
    for published in [false, true] {
        let mut f = Fixture::new(None, false, 2);
        let (copy, copy_id) = f.submit_copy_range(5, 17, 31);
        let owners = f.restored(copy_id);
        if published {
            f.context.flush_stream(f.copy_stream).unwrap();
            assert_eq!(f.root(copy_id).phase, Phase::Published);
        }
        let mut read = region(f.destination, RuntimeAccessV1::Read);
        read.byte_offset = 16;
        read.byte_len = 17;
        let events = f.events.clone();
        let before = f.context.backend().submissions.len();
        let trace = f.root(copy_id).trace.clone();
        assert!(f.launch(Arguments(vec![read]), &events).is_err());
        assert_eq!(f.context.backend().submissions.len(), before);
        assert_eq!(f.root(copy_id).trace, trace);
        read.byte_offset = 22;
        let mut consumer = f.launch(Arguments(vec![read]), &events).unwrap();
        let id = f.context.backend_submission_for_test_v1(&consumer).unwrap();
        f.assert_waiting(id, copy_id, published);
        f.release_events();
        assert_eq!(
            f.context.poll(&mut consumer).unwrap(),
            RuntimePollV1::Pending
        );
        assert_eq!(f.root(copy_id).trace, trace);
        let route = drive_restored_handoff(&mut f, copy_id, id);
        assert_eq!(f.restored(copy_id), owners);
        let pending = &f.context.backend().children[route.child].pending_compute[&route.local];
        assert_eq!(pending.launch.bindings[0].region.byte_offset, 22);
        assert_eq!(pending.launch.bindings[0].region.byte_len, 17);
        assert_eq!(
            pending.retained_allocations.as_ref(),
            &[f.copy(copy_id).destination.local]
        );
        // The real unflushed gate prevents native compute publication in this CPU fixture.
        assert_eq!(
            f.context.wait(&mut consumer, Duration::ZERO).unwrap(),
            RuntimePollV1::Pending
        );
        f.clean(copy, consumer);
    }
}

#[test]
fn queued_native_consumer_control_only_dependency_still_waits_for_exact_copy_restoration() {
    let mut f = Fixture::new(None, false, 2);
    let (copy, copy_id) = f.submit_copy();
    let owners = f.restored(copy_id);
    let events = f.events.clone();
    let mut consumer = f
        .launch(
            Arguments(vec![region(f.gate_allocation, RuntimeAccessV1::Read)]),
            &events,
        )
        .unwrap();
    let id = f.context.backend_submission_for_test_v1(&consumer).unwrap();
    f.assert_waiting(id, copy_id, false);
    f.release_events();
    assert_eq!(
        f.context.poll(&mut consumer).unwrap(),
        RuntimePollV1::Pending
    );
    assert!(f.root(copy_id).trace.is_empty());
    let route = drive_restored_handoff(&mut f, copy_id, id);
    assert_eq!(f.restored(copy_id), owners);
    let child = &f.context.backend().children[route.child];
    let pending = &child.pending_compute[&route.local];
    let gate = &child.pending_compute[&f.gate_route.local];
    assert_eq!(pending.retained_allocations, gate.retained_allocations);
    assert_eq!(pending.retained_allocations.len(), 1);
    assert!(
        !pending
            .retained_allocations
            .contains(&f.copy(copy_id).destination.local)
    );
    assert!(
        f.context.backend().children[0]
            .allocation_custody
            .is_empty()
    );
    assert_eq!(
        f.context.wait(&mut consumer, Duration::ZERO).unwrap(),
        RuntimePollV1::Pending
    );
    f.clean(copy, consumer);
}
