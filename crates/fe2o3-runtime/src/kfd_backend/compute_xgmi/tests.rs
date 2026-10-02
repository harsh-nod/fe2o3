use super::super::kfd_backend_sdma_seam::{
    ScriptedBufferKindV1, ScriptedFailureModeV1, ScriptedRecycleOutcomeV1, ScriptedSdmaStepV1,
};
use super::*;
use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};

mod deferred_directed;
mod directed;
mod directed_consumer;
mod late_directed;
mod lifecycle;
mod packetized;
#[path = "../peer_readback/tests.rs"]
mod peer_readback;
#[path = "../progress_quantum_tests.rs"]
mod progress_quantum;
mod queued_consumer;
mod segments;
mod sharded;
mod subranges;

const BYTES: usize = 64;
const STAGES: [Stage; 6] = [
    Stage::Create,
    Stage::Copy,
    Stage::Poll,
    Stage::Finish,
    Stage::Retire,
    Stage::Restore,
];

struct Fixture {
    // A failing assertion must not destroy intentionally retained native custody.
    backend: ManuallyDrop<KfdMultiDeviceRuntimeBackendV1>,
    stream: u64,
    source: BackendMemoryRegionV1,
    destination: BackendMemoryRegionV1,
}

impl Fixture {
    fn new(failure: Option<Stage>, unwind: bool) -> Self {
        Self::configured(failure, unwind, 0, 2)
    }

    fn configured(
        failure: Option<Stage>,
        unwind: bool,
        pending_samples: usize,
        child_count: usize,
    ) -> Self {
        Self::with_destination_steps(failure, unwind, pending_samples, child_count, Vec::new())
    }

    fn with_destination_steps(
        failure: Option<Stage>,
        unwind: bool,
        pending_samples: usize,
        child_count: usize,
        destination_steps: Vec<ScriptedSdmaStepV1>,
    ) -> Self {
        Self::with_bytes(
            failure,
            unwind,
            pending_samples,
            child_count,
            BYTES,
            destination_steps,
        )
    }

    fn with_bytes(
        failure: Option<Stage>,
        unwind: bool,
        pending_samples: usize,
        child_count: usize,
        byte_len: usize,
        destination_steps: Vec<ScriptedSdmaStepV1>,
    ) -> Self {
        let mut child_steps: Vec<_> = (0..child_count).map(|_| Vec::new()).collect();
        child_steps[1] = destination_steps;
        Self::with_child_steps(failure, unwind, pending_samples, byte_len, child_steps)
    }

    fn with_child_steps(
        failure: Option<Stage>,
        unwind: bool,
        pending_samples: usize,
        byte_len: usize,
        child_steps: Vec<Vec<ScriptedSdmaStepV1>>,
    ) -> Self {
        let sizes = vec![byte_len; child_steps.len()];
        Self::with_child_sizes(failure, unwind, pending_samples, &sizes, child_steps)
    }

    fn with_child_sizes(
        failure: Option<Stage>,
        unwind: bool,
        pending_samples: usize,
        sizes: &[usize],
        mut child_steps: Vec<Vec<ScriptedSdmaStepV1>>,
    ) -> Self {
        let child_count = child_steps.len();
        assert_eq!(sizes.len(), child_count);
        let children = (0..child_count)
            .map(|index| {
                let mut child = KfdRuntimeBackendV1::mock();
                child.description.backend_device = 7 + index as u64;
                child
            })
            .collect();
        let mut backend =
            ManuallyDrop::new(KfdMultiDeviceRuntimeBackendV1::from_backends(children).unwrap());
        let stream = backend.create_stream_v1(8).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, sizes[0] as u64, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(8, RuntimeMemoryKindV1::DeviceLocal, sizes[1] as u64, 8)
            .unwrap();
        for (index, size) in sizes.iter().enumerate().skip(2) {
            backend
                .allocate_v1(
                    7 + index as u64,
                    RuntimeMemoryKindV1::DeviceLocal,
                    *size as u64,
                    8,
                )
                .unwrap();
        }
        for allocation in backend.allocations.keys().copied().collect::<Vec<_>>() {
            let route = backend.allocations[&allocation];
            let fill = if route.child.is_multiple_of(2) {
                0x53
            } else {
                0x17
            };
            let mut steps = core::mem::take(&mut child_steps[route.child]);
            steps.extend([
                ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ]);
            let driver = ScriptedSdmaDriverV1::new(steps);
            let mut owner = driver.test_device_owner(sizes[route.child]);
            owner.scripted_bytes_mut().unwrap().fill(fill);
            let child = &mut backend.children[route.child];
            let record = child.allocations.get_mut(&route.local).unwrap();
            record.sdma_storage = KfdRuntimeSdmaStorageV1::Device(Box::new(owner));
            record.sdma_backed = true;
            record.sdma_initialized = true;
            record.sdma_shadow_dirty = true;
            record.content_sha256 = Some([fill; 32]);
            child.native_available = true;
            child.sdma_enabled = true;
            child.peer_visible_device_allocations = true;
            child.scripted_sdma = Some(driver);
        }
        for source in (0..child_count).step_by(2) {
            backend.compute_xgmi_routes.insert(
                (source, source + 1),
                Route::Scripted {
                    failure,
                    unwind,
                    pending_samples,
                },
            );
        }
        Self {
            backend,
            stream,
            source: BackendMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: sizes[0] as u64,
            },
            destination: BackendMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: sizes[1] as u64,
            },
        }
    }

    fn submit(&mut self, dependencies: &[u64]) -> u64 {
        self.backend
            .peer_copy_v1(self.stream, self.source, self.destination, dependencies)
            .unwrap()
    }

    fn copy(&self, submission: u64) -> &CooperativeCopySubmissionV1 {
        let RoutedSubmissionV1::CooperativeCopy(copy) = &self.backend.submissions[&submission]
        else {
            panic!("cooperative owner expected")
        };
        copy
    }

    fn root(&self, submission: u64) -> &Root {
        self.copy(submission).compute_xgmi.as_deref().unwrap()
    }

    fn record(&self, source: bool) -> &AllocationRecordV1 {
        let allocation = if source {
            self.source.allocation
        } else {
            self.destination.allocation
        };
        let route = self.backend.allocations[&allocation];
        &self.backend.children[route.child].allocations[&route.local]
    }

    fn record_mut(&mut self, source: bool) -> &mut AllocationRecordV1 {
        let allocation = if source {
            self.source.allocation
        } else {
            self.destination.allocation
        };
        let route = self.backend.allocations[&allocation];
        self.backend.children[route.child]
            .allocations
            .get_mut(&route.local)
            .unwrap()
    }

    fn owner(&self, source: bool) -> &DirectionalSdmaDeviceOwnerV1 {
        let KfdRuntimeSdmaStorageV1::Device(owner) = &self.record(source).sdma_storage else {
            panic!("restored device owner expected")
        };
        owner
    }

    fn unrelated(&mut self, child: usize) -> u64 {
        let device = self.backend.children[child].description.backend_device;
        self.backend.children[child].native_available = false;
        self.backend.children[child].sdma_enabled = false;
        let allocation = self
            .backend
            .allocate_v1(device, RuntimeMemoryKindV1::HostVisible, BYTES as u64, 8)
            .unwrap();
        self.backend.children[child].native_available = true;
        self.backend.children[child].sdma_enabled = true;
        allocation
    }

    fn producer(&mut self, status: BackendPollV1) -> (u64, u64, RoutedHandleV1) {
        self.producer_on(0, status)
    }

    fn producer_on(&mut self, child: usize, status: BackendPollV1) -> (u64, u64, RoutedHandleV1) {
        let device = self.backend.children[child].description.backend_device;
        let stream = self.backend.create_stream_v1(device).unwrap();
        let stream_route = self.backend.streams[&stream];
        let local = self.backend.children[child].next_id().unwrap();
        self.backend.children[child].submissions.insert(
            local,
            SubmissionRecordV1 {
                stream: stream_route.local,
                status,
                dependency_depth: 1,
                profile_dispatch_published: false,
            },
        );
        self.backend
            .reserve_native_stream_submission_v1(stream)
            .unwrap();
        let producer = self.backend.next_id().unwrap();
        let route = RoutedHandleV1 { child, local };
        self.backend
            .submissions
            .insert(producer, RoutedSubmissionV1::Native { route, stream });
        self.backend.retain_native_stream_submission_v1(stream);
        let event = self.backend.record_event_v1(stream, producer).unwrap();
        (producer, event, route)
    }

    fn clean(mut self) {
        for event in self.backend.events.keys().copied().collect::<Vec<_>>() {
            self.backend.release_event_v1(event).unwrap();
        }
        let mut submissions = self.backend.submissions.keys().copied().collect::<Vec<_>>();
        submissions.sort_unstable_by(|left, right| right.cmp(left));
        for submission in submissions {
            self.backend.release_submission_v1(submission).unwrap();
        }
        for allocation in self.backend.allocations.keys().copied().collect::<Vec<_>>() {
            let route = self.backend.allocations[&allocation];
            self.backend.children[route.child]
                .allocations
                .get_mut(&route.local)
                .unwrap()
                .sdma_backed = false;
            self.backend.release_allocation_v1(allocation).unwrap();
        }
        for module in self.backend.modules.keys().copied().collect::<Vec<_>>() {
            self.backend.unload_module_v1(module).unwrap();
        }
        for stream in self.backend.streams.keys().copied().collect::<Vec<_>>() {
            self.backend.destroy_stream_v1(stream).unwrap();
        }
        self.backend.assert_cooperative_indexes_consistent();
        assert_eq!(self.backend.cooperative_staging_bytes, 0);
        for child in &self.backend.children {
            let driver = child.scripted_sdma.as_ref().unwrap();
            assert_eq!(driver.remaining_steps(), 0);
            assert_eq!(driver.live_owner_count(), 0);
            assert_eq!(driver.unexpected_drops(), 0);
        }
        self.backend.shutdown_native_v1().unwrap();
        drop(ManuallyDrop::into_inner(self.backend));
    }
}

#[test]
fn native_route_observers_do_not_progress_and_success_restores_exact_owners() {
    let mut f = Fixture::new(None, false);
    let identities = [
        f.owner(true).scripted_owner_id(),
        f.owner(false).scripted_owner_id(),
    ];
    let source_digest = f.record(true).content_sha256;
    let copy = f.submit(&[]);
    assert_eq!(f.backend.cooperative_staging_bytes, 0);
    assert!(f.copy(copy).staging.is_empty());
    assert_eq!(f.copy(copy).scratch_byte_len, 0);
    assert!(matches!(
        f.backend.release_submission_v1(copy),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert!(matches!(
        f.backend.release_allocation_v1(f.destination.allocation),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert!(matches!(
        f.backend.shutdown_native_v1(),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    for _ in 0..3 {
        assert_eq!(f.backend.poll_v1(copy).unwrap(), BackendPollV1::Pending);
        assert_eq!(
            f.backend.wait_v1(copy, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert!(f.root(copy).trace.is_empty());
        assert_eq!(f.owner(false).scripted_bytes().unwrap(), &[0x17; BYTES]);
    }
    f.backend.flush_stream_v1(f.stream).unwrap();
    assert_eq!(f.backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
    assert_eq!(f.root(copy).trace, STAGES);
    assert!(f.root(copy).is_quiescent());
    assert_eq!(
        [
            f.owner(true).scripted_owner_id(),
            f.owner(false).scripted_owner_id()
        ],
        identities
    );
    assert_eq!(f.owner(true).scripted_bytes().unwrap(), &[0x53; BYTES]);
    assert_eq!(f.owner(false).scripted_bytes().unwrap(), &[0x53; BYTES]);
    assert_eq!(f.record(true).content_sha256, source_digest);
    assert!(f.record(false).content_sha256.is_none());
    assert!(f.record(false).last_full_host_write.is_none());
    assert!(f.record(false).sdma_initialized && f.record(false).sdma_shadow_dirty);
    assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
    f.backend.assert_cooperative_indexes_consistent();
    f.clean();
}

#[test]
fn native_route_selection_admits_checked_ranges_but_keeps_unqualified_copies_staged() {
    for variant in 0..9 {
        let mut f = Fixture::new(None, false);
        match variant {
            0 => {
                f.backend.compute_xgmi_routes.clear();
            }
            1 => {
                f.backend.children[0].peer_visible_device_allocations = false;
            }
            2 => {
                f.source.byte_offset = 1;
                f.source.byte_len -= 1;
                f.destination.byte_len -= 1;
            }
            3 => {
                f.source.byte_len -= 1;
                f.destination.byte_len -= 1;
            }
            4 => {
                f.record_mut(false).kind = RuntimeMemoryKindV1::HostVisible;
            }
            5 => {
                f.record_mut(true).sdma_initialized = false;
            }
            6 => {
                f.record_mut(false).sdma_initialized = false;
            }
            7 | 8 => {
                let allocation = if variant == 7 {
                    f.source.allocation
                } else {
                    f.destination.allocation
                };
                let route = f.backend.allocations[&allocation];
                let child = &mut f.backend.children[route.child];
                let slot = &mut child
                    .allocations
                    .get_mut(&route.local)
                    .unwrap()
                    .sdma_storage;
                let KfdRuntimeSdmaStorageV1::Device(owner) = core::mem::replace(
                    slot,
                    KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous),
                ) else {
                    panic!("scripted device owner expected")
                };
                let buffer = child.demote_sdma_device_v1(route.local, owner).unwrap();
                let record = child.allocations.get_mut(&route.local).unwrap();
                record.sdma_storage = KfdRuntimeSdmaStorageV1::DemotedDevice(buffer);
                assert!(record.sdma_backed && record.sdma_initialized);
            }
            _ => unreachable!(),
        }
        let copy = f.submit(&[]);
        let native = matches!(variant, 2 | 3);
        assert_eq!(f.copy(copy).compute_xgmi.is_some(), native);
        assert_eq!(
            f.copy(copy).staging.len() as u64,
            if native { 0 } else { f.source.byte_len }
        );
        assert_eq!(
            f.backend.cancel_v1(copy).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        f.record_mut(false).kind = RuntimeMemoryKindV1::DeviceLocal;
        f.clean();
    }
}

#[test]
fn native_route_waits_for_dependency_but_not_retained_completed_handles() {
    let mut f = Fixture::new(None, false);
    let (producer, event, route) = f.producer(BackendPollV1::Pending);
    let copy = f.submit(&[event]);
    assert_eq!(
        f.backend.progress_cooperative_copy(copy).unwrap(),
        BackendPollV1::Pending
    );
    assert!(f.root(copy).trace.is_empty());
    assert_eq!(f.owner(false).scripted_bytes().unwrap(), &[0x17; BYTES]);
    f.backend.children[route.child]
        .submissions
        .get_mut(&route.local)
        .unwrap()
        .status = BackendPollV1::Succeeded;
    f.backend.flush_stream_v1(f.stream).unwrap();
    assert_eq!(f.backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
    assert!(f.backend.submissions.contains_key(&producer));
    assert!(f.backend.events.contains_key(&event));
    assert!(f.backend.cooperative_dependency_retain_counts.is_empty());
    f.clean();
}

#[test]
fn native_route_dependency_failure_and_cancellation_have_no_native_effects() {
    for cancel in [false, true] {
        let mut f = Fixture::new(None, false);
        let (_, event, _) = f.producer(BackendPollV1::Failed { code: 9 });
        let copy = f.submit(&[event]);
        if cancel {
            assert_eq!(
                f.backend.cancel_v1(copy).unwrap(),
                crate::BackendCancellationV1::Cancelled
            );
        } else {
            assert!(matches!(
                f.backend.progress_cooperative_copy(copy).unwrap(),
                BackendPollV1::Failed { .. }
            ));
        }
        assert!(f.root(copy).trace.is_empty());
        assert!(f.root(copy).is_quiescent());
        assert_eq!(f.owner(false).scripted_bytes().unwrap(), &[0x17; BYTES]);
        assert_eq!(f.backend.cooperative_staging_bytes, 0);
        f.backend.assert_cooperative_indexes_consistent();
        f.clean();
    }
}

#[test]
fn native_route_pending_producer_is_rejected_for_queued_consumer() {
    let mut f = Fixture::new(None, false);
    let copy = f.submit(&[]);
    let event = f.backend.record_event_v1(f.stream, copy).unwrap();
    assert!(matches!(
        f.backend.exact_launch_dependency_for_child(
            BackendLaunchProducerV1 {
                event,
                producer_submission: copy,
            },
            1
        ),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert!(
        f.backend
            .children
            .iter()
            .all(|child| child.pending_compute.is_empty() && child.allocation_custody.is_empty())
    );
    assert!(f.root(copy).trace.is_empty());
    f.backend.flush_stream_v1(f.stream).unwrap();
    assert_eq!(f.backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
    assert!(
        f.backend
            .exact_launch_dependency_for_child(
                BackendLaunchProducerV1 {
                    event,
                    producer_submission: copy,
                },
                1
            )
            .unwrap()
            .is_none()
    );
    f.clean();
}

#[test]
fn native_route_rechecks_both_storage_slots_before_extracting_either() {
    for invalid_source in [false, true] {
        let mut f = Fixture::new(None, false);
        let identities = [
            f.owner(true).scripted_owner_id(),
            f.owner(false).scripted_owner_id(),
        ];
        let copy = f.submit(&[]);
        f.record_mut(invalid_source).sdma_initialized = false;
        assert_eq!(
            f.backend.progress_cooperative_copy(copy).unwrap(),
            BackendPollV1::Pending
        );
        assert!(matches!(
            f.backend.progress_cooperative_copy(copy).unwrap(),
            BackendPollV1::Failed { .. }
        ));
        assert!(f.root(copy).trace.is_empty());
        assert!(f.root(copy).is_quiescent());
        assert_eq!(
            [
                f.owner(true).scripted_owner_id(),
                f.owner(false).scripted_owner_id()
            ],
            identities
        );
        f.clean();
    }
}

#[test]
fn native_route_active_allocation_is_pending_without_extraction() {
    let mut f = Fixture::new(None, false);
    let copy = f.submit(&[]);
    let route = f.backend.allocations[&f.source.allocation];
    f.backend.children[route.child].allocation_custody.insert(
        route.local,
        RuntimeAllocationCustodyV1 {
            owners: VecDeque::from([RuntimeAllocationCustodyOwnerV1 {
                submission: 999,
                stream: 999,
                kind: RuntimeAllocationCustodyKindV1::Compute,
            }]),
            sole_stream: Some(999),
            owner_counts: [1, 0],
            metadata_credits: None,
        },
    );
    for _ in 0..3 {
        assert_eq!(
            f.backend.progress_cooperative_copy(copy).unwrap(),
            BackendPollV1::Pending
        );
        assert!(f.root(copy).trace.is_empty());
        assert!(f.root(copy).is_quiescent());
    }
    f.backend.children[route.child]
        .allocation_custody
        .remove(&route.local);
    f.backend.flush_stream_v1(f.stream).unwrap();
    assert_eq!(f.backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
    f.clean();
}

#[test]
fn native_route_wrong_second_storage_slot_does_not_extract_first_owner() {
    let mut f = Fixture::new(None, false);
    let source_identity = f.owner(true).scripted_owner_id();
    let copy = f.submit(&[]);
    let saved = core::mem::replace(
        &mut f.record_mut(false).sdma_storage,
        KfdRuntimeSdmaStorageV1::Synthetic,
    );
    assert_eq!(
        f.backend.progress_cooperative_copy(copy).unwrap(),
        BackendPollV1::Pending
    );
    assert!(matches!(
        f.backend.progress_cooperative_copy(copy).unwrap(),
        BackendPollV1::Failed { .. }
    ));
    assert_eq!(f.owner(true).scripted_owner_id(), source_identity);
    assert!(f.root(copy).trace.is_empty());
    assert!(f.root(copy).is_quiescent());
    f.record_mut(false).sdma_storage = saved;
    f.clean();
}

#[test]
fn native_route_normalizes_ready_storage_without_replacing_owner_identity() {
    let mut f = Fixture::new(None, false);
    let identities = [
        f.owner(true).scripted_owner_id(),
        f.owner(false).scripted_owner_id(),
    ];
    for source in [false, true] {
        let record = f.record_mut(source);
        let KfdRuntimeSdmaStorageV1::Device(owner) =
            core::mem::replace(&mut record.sdma_storage, KfdRuntimeSdmaStorageV1::Synthetic)
        else {
            panic!("device owner expected")
        };
        let DirectionalSdmaDeviceOwnerV1::Scripted(device) = *owner else {
            panic!("scripted owner expected")
        };
        record.sdma_storage =
            KfdRuntimeSdmaStorageV1::H2dReady(Box::new(PersistentComputeReadyStorageV1 {
                owner: PersistentComputeReadyOwnerV1::Scripted {
                    device,
                    authenticated_sha256: [3; 32],
                },
                promotion: None,
            }));
    }
    let copy = f.submit(&[]);
    f.backend.flush_stream_v1(f.stream).unwrap();
    assert_eq!(f.backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
    assert_eq!(
        [
            f.owner(true).scripted_owner_id(),
            f.owner(false).scripted_owner_id()
        ],
        identities
    );
    f.clean();
}

#[test]
fn native_route_releases_unrelated_retained_control_before_native_effects() {
    for fault in [
        None,
        Some(ScriptedThreeCompletionFaultV1::ControlFailure),
        Some(ScriptedThreeCompletionFaultV1::ControlUnwind),
    ] {
        let mut f = Fixture::new(None, false);
        f.backend.children[0].native_available = false;
        f.backend.children[0].sdma_enabled = false;
        let unrelated = f
            .backend
            .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
            .unwrap();
        let unrelated = f.backend.allocations[&unrelated].local;
        f.backend.children[0].native_available = true;
        f.backend.children[0].sdma_enabled = true;
        f.backend.children[0].retained_persistent_dispatch = Some(RetainedPersistentDispatchV1 {
            allocation: unrelated,
            dispatch_shape_sha256: [7; 32],
        });
        f.backend.children[0].scripted_three_completion_fault = fault;
        let identities = [
            f.owner(true).scripted_owner_id(),
            f.owner(false).scripted_owner_id(),
        ];
        let copy = f.submit(&[]);
        assert_eq!(
            f.backend.progress_cooperative_copy(copy).unwrap(),
            BackendPollV1::Pending
        );
        let outcome = catch_unwind(AssertUnwindSafe(|| f.backend.flush_stream_v1(f.stream)));
        match fault {
            None => {
                outcome.unwrap().unwrap();
                assert_eq!(f.backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
                assert!(f.backend.children[0].retained_persistent_dispatch.is_none());
                f.clean();
            }
            Some(fault) => {
                if fault == ScriptedThreeCompletionFaultV1::ControlUnwind {
                    assert!(outcome.is_err());
                } else {
                    assert!(matches!(
                        outcome.unwrap(),
                        Err(RuntimeBackendFailureV1::Terminal(_))
                    ));
                }
                assert!(f.backend.terminal);
                assert!(f.backend.children.iter().all(|child| child.terminal));
                assert!(f.root(copy).trace.is_empty());
                assert!(f.root(copy).is_quiescent());
                assert_eq!(
                    [
                        f.owner(true).scripted_owner_id(),
                        f.owner(false).scripted_owner_id()
                    ],
                    identities
                );
            }
        }
    }
}

#[test]
fn native_route_rejects_new_endpoint_dirty_state_before_cache_release() {
    let mut f = Fixture::new(None, false);
    let copy = f.submit(&[]);
    f.record_mut(true).native_dirty.push(NativeDirtyExtentV1 {
        compute_lane: 0,
        data_index: 0,
        allocation_offset: 0,
        data_offset: 0,
        byte_len: BYTES as u64,
    });
    assert_eq!(
        f.backend.progress_cooperative_copy(copy).unwrap(),
        BackendPollV1::Pending
    );
    assert!(matches!(
        f.backend.progress_cooperative_copy(copy).unwrap(),
        BackendPollV1::Failed { .. }
    ));
    assert!(f.root(copy).trace.is_empty());
    assert_eq!(f.record(true).native_dirty.len(), 1);
    assert_eq!(f.owner(false).scripted_bytes().unwrap(), &[0x17; BYTES]);
    f.record_mut(true).native_dirty.clear();
    f.clean();
}

#[test]
fn native_route_every_failure_and_unwind_retains_both_owners_and_poisons_both_children() {
    for (index, stage) in STAGES.into_iter().enumerate() {
        for unwind in [false, true] {
            let mut f = Fixture::new(Some(stage), unwind);
            let identities = [
                f.owner(true).scripted_owner_id(),
                f.owner(false).scripted_owner_id(),
            ];
            let copy = f.submit(&[]);
            assert_eq!(
                f.backend.progress_cooperative_copy(copy).unwrap(),
                BackendPollV1::Pending
            );
            let outcome = catch_unwind(AssertUnwindSafe(|| f.backend.flush_stream_v1(f.stream)));
            if unwind {
                assert!(outcome.is_err());
            } else {
                assert!(matches!(
                    outcome.unwrap(),
                    Err(RuntimeBackendFailureV1::Terminal(_))
                ));
            }
            assert!(f.backend.terminal);
            assert!(f.backend.children.iter().all(|child| child.terminal));
            let root = f.root(copy);
            assert_eq!(root.trace, STAGES[..=index]);
            assert!(!root.is_quiescent());
            assert_eq!(
                root.scripted_owners
                    .each_ref()
                    .map(|owner| owner.as_ref().unwrap().scripted_owner_id()),
                identities
            );
            assert!(root.shells.iter().all(Option::is_some));
            for source in [false, true] {
                assert!(matches!(
                    f.record(source).sdma_storage,
                    KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::ComputeXgmi(owner)) if owner == copy
                ));
            }
            assert_eq!(f.copy(copy).phase, CooperativeCopyPhaseV1::Read);
            assert_eq!(f.backend.compute_xgmi_children, [Some(copy), Some(copy)]);
            assert!(matches!(
                f.backend.poll_v1(copy),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
            assert!(matches!(
                f.backend.shutdown_native_v1(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
            // Deliberately retain terminal custody, matching the runtime contract.
        }
    }
}

fn busy<T: fmt::Debug>(result: Result<T, Failure>) {
    assert!(
        matches!(result,
            Err(RuntimeBackendFailureV1::Rejected(ref error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ),
        "expected settled Busy, got {result:?}"
    );
}

#[test]
fn native_route_pending_samples_keep_exact_custody_and_observers_do_not_sample() {
    let mut f = Fixture::configured(None, false, 3, 2);
    let identities = [
        f.owner(true).scripted_owner_id(),
        f.owner(false).scripted_owner_id(),
    ];
    let copy = f.submit(&[]);
    assert_eq!(
        f.backend.drain_v1(copy, Instant::now()).unwrap(),
        BackendPollV1::Pending
    );
    assert_eq!(f.copy(copy).phase, CooperativeCopyPhaseV1::Dependencies);
    assert!(f.root(copy).trace.is_empty());
    assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
    f.backend.flush_stream_v1(f.stream).unwrap();
    assert_eq!(
        f.root(copy).trace,
        [Stage::Create, Stage::Copy, Stage::Poll]
    );
    assert_eq!(f.backend.compute_xgmi_children, [Some(copy), Some(copy)]);
    assert_eq!(f.root(copy).phase, Phase::Published);
    let generation = f.backend.cooperative_progress_generation;
    for _ in 0..3 {
        assert_eq!(f.backend.poll_v1(copy).unwrap(), BackendPollV1::Pending);
        assert_eq!(
            f.backend.wait_v1(copy, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(
            f.backend.drain_v1(copy, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(f.root(copy).trace.len(), 3);
        assert_eq!(f.backend.cooperative_progress_generation, generation);
    }
    assert_eq!(
        f.backend.cancel_v1(copy).unwrap(),
        crate::BackendCancellationV1::TooLate
    );
    busy(f.backend.release_submission_v1(copy));
    busy(f.backend.release_allocation_v1(f.source.allocation));
    busy(f.backend.release_allocation_v1(f.destination.allocation));
    busy(f.backend.shutdown_native_v1());
    f.backend.flush_stream_v1(f.stream).unwrap();
    assert_eq!(f.root(copy).trace.len(), 4);
    assert_eq!(f.backend.cooperative_progress_generation, generation);
    assert_eq!(
        f.root(copy)
            .scripted_owners
            .each_ref()
            .map(|owner| owner.as_ref().unwrap().scripted_owner_id()),
        identities
    );
    f.backend.assert_cooperative_indexes_consistent();
    assert_eq!(
        f.backend
            .drain_v1(copy, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(f.backend.compute_xgmi_children, [None, None]);
    assert_eq!(
        f.root(copy)
            .trace
            .iter()
            .filter(|stage| **stage == Stage::Poll)
            .count(),
        4
    );
    assert_eq!(
        [
            f.owner(true).scripted_owner_id(),
            f.owner(false).scripted_owner_id()
        ],
        identities
    );
    assert_eq!(f.owner(false).scripted_bytes().unwrap(), &[0x53; BYTES]);
    f.clean();
}

#[test]
fn native_route_occupied_children_reject_mutation_but_allow_bookkeeping() {
    let mut f = Fixture::configured(None, false, 2, 4);
    let unrelated = [f.unrelated(0), f.unrelated(1), f.unrelated(2)];
    let (completed, event, _) = f.producer(BackendPollV1::Succeeded);
    let copy = f.submit(&[]);
    f.backend.flush_stream_v1(f.stream).unwrap();
    let trace = f.root(copy).trace.clone();
    assert_eq!(
        f.backend.poll_v1(completed).unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(
        f.backend.wait_v1(completed, Instant::now()).unwrap(),
        BackendPollV1::Succeeded
    );
    f.backend.release_event_v1(event).unwrap();
    f.backend.release_submission_v1(completed).unwrap();
    for (child, allocation) in unrelated[..2].iter().copied().enumerate() {
        let device = 7 + child as u64;
        let stream = f.backend.create_stream_v1(device).unwrap();
        let module = f
            .backend
            .load_module_v1(device, &crate::synthetic_cov6::module())
            .unwrap();
        let kernel = f
            .backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        busy(
            f.backend
                .allocate_v1(device, RuntimeMemoryKindV1::HostVisible, 8, 8),
        );
        busy(f.backend.release_allocation_v1(allocation));
        busy(f.backend.write_allocation_v1(allocation, 0, &[0]));
        busy(f.backend.read_allocation_v1(allocation, 0, &mut [0]));
        busy(f.backend.unload_module_v1(module));
        let geometry = crate::RuntimeLaunchGeometryV1 {
            grid: [1; 3],
            workgroup: [1; 3],
            dynamic_shared_bytes: 0,
        };
        busy(f.backend.submit_v1(BackendLaunchV1 {
            stream,
            kernel,
            explicit_kernarg: &[],
            bindings: &[],
            dependencies: &[],
            geometry,
            semantic_launch: BackendSemanticLaunchV1::Ordinary,
        }));
        busy(
            f.backend
                .submit_producer_aware_launch_v1(BackendProducerAwareLaunchV1 {
                    stream,
                    kernel,
                    explicit_kernarg: &[],
                    bindings: &[],
                    dependencies: &[],
                    geometry,
                }),
        );
        let region = BackendMemoryRegionV1 {
            allocation,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 8,
        };
        busy(f.backend.copy_async_v1(
            stream,
            region,
            BackendMemoryRegionV1 {
                access: RuntimeAccessV1::Write,
                ..region
            },
            &[],
        ));
        f.backend.flush_stream_v1(stream).unwrap();
        f.backend.destroy_stream_v1(stream).unwrap();
    }
    f.backend
        .write_allocation_v1(unrelated[2], 0, &[0x45])
        .unwrap();
    let mut observed = [0];
    f.backend
        .read_allocation_v1(unrelated[2], 0, &mut observed)
        .unwrap();
    assert_eq!(observed, [0x45]);
    assert_eq!(f.root(copy).trace, trace);
    assert_eq!(
        f.backend.compute_xgmi_children,
        [Some(copy), Some(copy), None, None]
    );
    assert_eq!(
        f.backend
            .drain_v1(copy, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    for allocation in unrelated[..2].iter().copied() {
        f.backend
            .write_allocation_v1(allocation, 0, &[0x31])
            .unwrap();
    }
    f.clean();
}

#[test]
fn native_route_occupied_dependency_observation_defers_without_failing_copy() {
    // The native write leg allocates and retires scratch even for a HostVisible endpoint.
    let mut f = Fixture::with_destination_steps(
        None,
        false,
        2,
        2,
        vec![
            ScriptedSdmaStepV1::Allocate {
                kind: ScriptedBufferKindV1::Host,
                byte_len: 8,
            },
            ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len: 8,
            },
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ],
    );
    let source = f.unrelated(0);
    let destination = f.unrelated(1);
    f.backend
        .write_allocation_v1(source, 0, &[0x6d; 8])
        .unwrap();
    let other_stream = f.backend.create_stream_v1(8).unwrap();
    let (producer, event, producer_route) = f.producer(BackendPollV1::Pending);
    let copy = f.submit(&[]);
    let dependent = f
        .backend
        .peer_copy_v1(
            other_stream,
            BackendMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 8,
            },
            BackendMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: 8,
            },
            &[event],
        )
        .unwrap();
    f.backend.flush_stream_v1(f.stream).unwrap();
    f.backend.flush_stream_v1(other_stream).unwrap();
    assert_eq!(
        f.backend.poll_v1(dependent).unwrap(),
        BackendPollV1::Pending
    );
    assert_eq!(f.copy(dependent).dependency_cursor, 0);
    f.backend.children[producer_route.child]
        .submissions
        .get_mut(&producer_route.local)
        .unwrap()
        .status = BackendPollV1::Succeeded;
    f.backend.flush_stream_v1(other_stream).unwrap();
    assert_eq!(f.copy(dependent).phase, CooperativeCopyPhaseV1::Read);
    assert_eq!(
        f.backend.poll_v1(dependent).unwrap(),
        BackendPollV1::Pending
    );
    assert_eq!(
        f.backend.poll_v1(producer).unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(f.root(copy).trace.len(), 3);
    f.backend.assert_cooperative_indexes_consistent();
    assert_eq!(
        f.backend
            .drain_v1(copy, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    f.backend.flush_stream_v1(other_stream).unwrap();
    assert_eq!(
        f.backend.poll_v1(dependent).unwrap(),
        BackendPollV1::Succeeded
    );
    let mut copied = [0; 8];
    f.backend
        .read_allocation_v1(destination, 0, &mut copied)
        .unwrap();
    assert_eq!(copied, [0x6d; 8]);
    assert_eq!(
        f.backend.children[1]
            .scripted_sdma
            .as_ref()
            .unwrap()
            .remaining_steps(),
        2
    );
    f.clean();
}

#[test]
fn native_route_disjoint_pairs_progress_independently_through_flush_and_drain() {
    let mut f = Fixture::configured(None, false, 2, 4);
    let allocation = |child| {
        f.backend
            .allocations
            .iter()
            .find_map(|(id, route)| (route.child == child).then_some(*id))
            .unwrap()
    };
    let source = allocation(2);
    let destination = allocation(3);
    let other_stream = f.backend.create_stream_v1(10).unwrap();
    let first = f.submit(&[]);
    let second = f
        .backend
        .peer_copy_v1(
            other_stream,
            BackendMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: BYTES as u64,
            },
            BackendMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: BYTES as u64,
            },
            &[],
        )
        .unwrap();
    f.backend.flush_stream_v1(f.stream).unwrap();
    f.backend.flush_stream_v1(other_stream).unwrap();
    assert_eq!(
        f.backend.compute_xgmi_children,
        [Some(first), Some(first), Some(second), Some(second)]
    );
    let first_trace = f.root(first).trace.clone();
    assert_eq!(
        f.backend
            .drain_v1(second, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(
        f.backend.compute_xgmi_children,
        [Some(first), Some(first), None, None]
    );
    assert_eq!(f.root(first).trace, first_trace);
    assert_eq!(f.backend.poll_v1(first).unwrap(), BackendPollV1::Pending);
    f.backend.assert_cooperative_indexes_consistent();
    assert_eq!(
        f.backend
            .drain_v1(first, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
    f.clean();
}

#[test]
fn native_route_retained_slot_or_reservation_mismatch_poison_both_without_retirement() {
    for reservation in [false, true] {
        let mut f = Fixture::configured(None, false, 1, 2);
        let copy = f.submit(&[]);
        f.backend.flush_stream_v1(f.stream).unwrap();
        if reservation {
            f.backend.compute_xgmi_children[1] = Some(copy + 1);
        } else {
            f.record_mut(false).sdma_storage =
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::ComputeXgmi(copy + 1));
        }
        assert!(matches!(
            f.backend.progress_cooperative_copy(copy),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(f.backend.terminal && f.backend.children.iter().all(|child| child.terminal));
        assert_eq!(
            f.root(copy).trace,
            [Stage::Create, Stage::Copy, Stage::Poll]
        );
        assert!(f.root(copy).scripted_owners.iter().all(Option::is_some));
        assert!(f.root(copy).shells.iter().all(Option::is_some));
    }
}
