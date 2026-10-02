use super::super::kfd_backend_sdma_seam::{
    ScriptedFailureModeV1, ScriptedRecycleOutcomeV1, ScriptedSdmaStepV1,
};
use super::*;
use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};

const BYTES: usize = 64;
const STAGES: [Stage; 4] = [Stage::Create, Stage::Copy, Stage::Retire, Stage::Restore];

struct Fixture {
    // A failing assertion must not destroy intentionally retained native custody.
    backend: ManuallyDrop<KfdMultiDeviceRuntimeBackendV1>,
    stream: u64,
    source: BackendMemoryRegionV1,
    destination: BackendMemoryRegionV1,
}

impl Fixture {
    fn new(failure: Option<Stage>, unwind: bool) -> Self {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let stream = backend.create_stream_v1(8).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, BYTES as u64, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(8, RuntimeMemoryKindV1::DeviceLocal, BYTES as u64, 8)
            .unwrap();
        for (allocation, fill) in [(source, 0x53), (destination, 0x17)] {
            let driver = ScriptedSdmaDriverV1::new([
                ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ]);
            let mut owner = driver.test_device_owner(BYTES);
            owner.scripted_bytes_mut().unwrap().fill(fill);
            let route = backend.allocations[&allocation];
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
        backend
            .compute_xgmi_routes
            .insert((0, 1), Route::Scripted { failure, unwind });
        Self {
            backend: ManuallyDrop::new(backend),
            stream,
            source: BackendMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: BYTES as u64,
            },
            destination: BackendMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: BYTES as u64,
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

    fn producer(&mut self, status: BackendPollV1) -> (u64, u64, RoutedHandleV1) {
        let stream = self.backend.create_stream_v1(7).unwrap();
        let stream_route = self.backend.streams[&stream];
        let local = self.backend.children[0].next_id().unwrap();
        self.backend.children[0].submissions.insert(
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
        let route = RoutedHandleV1 { child: 0, local };
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
fn native_route_selection_keeps_unqualified_and_partial_copies_staged() {
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
        assert!(f.copy(copy).compute_xgmi.is_none());
        assert_eq!(f.copy(copy).staging.len() as u64, f.source.byte_len);
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
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            f.backend.progress_cooperative_copy(copy)
        }));
        match fault {
            None => {
                assert_eq!(outcome.unwrap().unwrap(), BackendPollV1::Succeeded);
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
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                f.backend.progress_cooperative_copy(copy)
            }));
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
                    KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous)
                ));
            }
            assert_eq!(f.copy(copy).phase, CooperativeCopyPhaseV1::Read);
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
