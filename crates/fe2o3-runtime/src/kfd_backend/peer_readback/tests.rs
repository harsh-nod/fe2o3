use super::*;
use crate::kfd_backend::kfd_backend_sdma_seam::ScriptedExecutionOutcomeV1;
use crate::{
    RuntimeAllocationIdV1, RuntimeCancellationV1, RuntimeContextV1, RuntimeCopyV1,
    RuntimeEventIdV1, RuntimeMemoryRegionV1, RuntimePeerCopyV1, RuntimePollV1, RuntimeStreamIdV1,
    RuntimeSubmissionV1,
};

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
type Peer = RuntimeSubmissionV1<RuntimePeerCopyV1>;
type Readback = RuntimeSubmissionV1<RuntimeCopyV1>;

fn region(allocation: RuntimeAllocationIdV1, access: RuntimeAccessV1) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: BYTES as u64,
    }
}

fn allocation_steps() -> [ScriptedSdmaStepV1; 2] {
    [
        ScriptedSdmaStepV1::Allocate {
            kind: ScriptedBufferKindV1::Host,
            byte_len: BYTES,
        },
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len: BYTES,
        },
    ]
}

fn release_device_steps() -> Vec<ScriptedSdmaStepV1> {
    let mut steps: Vec<_> = allocation_steps().into_iter().collect();
    steps.extend([
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
    ]);
    steps
}

fn readback_steps() -> Vec<ScriptedSdmaStepV1> {
    let mut steps: Vec<_> = allocation_steps().into_iter().collect();
    steps.extend([
        ScriptedSdmaStepV1::Submit {
            direction: Gfx942PersistentSdmaDirectionV1::DeviceToHost,
            host_offset: 0,
            device_offset: 0,
            copy_bytes: BYTES as u32,
            outcome: ScriptedFailureModeV1::Success,
        },
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Pending),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Read {
            offset: 0,
            byte_len: BYTES as u64,
        },
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]);
    steps.extend(allocation_steps());
    steps.extend([
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len: BYTES,
        },
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ScriptedSdmaStepV1::Read {
            offset: 0,
            byte_len: BYTES as u64,
        },
    ]);
    steps
}

struct ReadbackFixture {
    context: ManuallyDrop<Context>,
    source: RuntimeAllocationIdV1,
    destination: RuntimeAllocationIdV1,
    host: RuntimeAllocationIdV1,
    peer_stream: RuntimeStreamIdV1,
    readback_stream: RuntimeStreamIdV1,
    backend_readback_stream: u64,
}

impl ReadbackFixture {
    fn new(readback: bool, journal: bool, failure: Option<Stage>, unwind: bool) -> Self {
        let children = (0..2)
            .map(|index| {
                let mut child = KfdRuntimeBackendV1::mock();
                child.description.backend_device = 7 + index;
                child
            })
            .collect();
        let backend = KfdMultiDeviceRuntimeBackendV1::from_backends(children).unwrap();
        let mut context = ManuallyDrop::new(if journal {
            Context::open_with_version_journal_members_v1(backend, 16, 16, 16).unwrap()
        } else {
            Context::open(backend).unwrap()
        });
        let devices = [context.devices()[0].id(), context.devices()[1].id()];
        let source = context
            .allocate(
                devices[0],
                RuntimeMemoryKindV1::DeviceLocal,
                BYTES as u64,
                8,
            )
            .unwrap();
        let destination = context
            .allocate(
                devices[1],
                RuntimeMemoryKindV1::DeviceLocal,
                BYTES as u64,
                8,
            )
            .unwrap();
        let host = context
            .allocate(
                devices[1],
                RuntimeMemoryKindV1::HostVisible,
                BYTES as u64,
                8,
            )
            .unwrap();
        for (allocation, byte) in [(source, 0x53), (destination, 0x17), (host, 0x29)] {
            context
                .write_allocation(allocation, 0, &[byte; BYTES])
                .unwrap();
        }
        let peer_stream = context.create_stream(devices[1]).unwrap();
        let readback_stream = context.create_stream(devices[1]).unwrap();
        let backend = context.backend_mut_for_test_v1();
        let backend_readback_stream = *backend.streams.keys().max().unwrap();
        for (index, child) in backend.children.iter_mut().enumerate() {
            let mut steps = if index == 1 && readback {
                readback_steps()
            } else {
                Vec::new()
            };
            steps.extend(release_device_steps());
            if index == 1 {
                steps.push(ScriptedSdmaStepV1::Recycle(
                    ScriptedRecycleOutcomeV1::Success,
                ));
            }
            let driver = ScriptedSdmaDriverV1::new(steps);
            for record in child.allocations.values_mut() {
                record.sdma_storage = if record.kind == RuntimeMemoryKindV1::DeviceLocal {
                    let mut owner = driver.test_device_owner(BYTES);
                    owner
                        .scripted_bytes_mut()
                        .unwrap()
                        .copy_from_slice(&record.bytes);
                    KfdRuntimeSdmaStorageV1::Device(Box::new(owner))
                } else {
                    let mut owner = driver.test_host_owner(BYTES);
                    owner
                        .scripted_bytes_mut()
                        .unwrap()
                        .copy_from_slice(&record.bytes);
                    KfdRuntimeSdmaStorageV1::Host(owner)
                };
                record.sdma_backed = true;
                record.sdma_initialized = true;
                record.sdma_shadow_dirty = record.kind == RuntimeMemoryKindV1::DeviceLocal;
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
                pending_samples: 3,
            },
        );
        Self {
            context,
            source,
            destination,
            host,
            peer_stream,
            readback_stream,
            backend_readback_stream,
        }
    }

    fn peer(&mut self) -> (Peer, u64, RuntimeEventIdV1, u64) {
        let peer = self
            .context
            .peer_copy(
                self.peer_stream,
                region(self.source, RuntimeAccessV1::Read),
                region(self.destination, RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        let id = self.context.backend_submission_for_test_v1(&peer).unwrap();
        let event = self.context.record_event(&peer).unwrap();
        let backend_event = *self.context.backend().events.keys().max().unwrap();
        (peer, id, event, backend_event)
    }

    fn readback(&mut self, event: RuntimeEventIdV1) -> (Readback, u64) {
        let readback = self
            .context
            .copy_async(
                self.readback_stream,
                region(self.destination, RuntimeAccessV1::Read),
                region(self.host, RuntimeAccessV1::Write),
                &[event],
            )
            .unwrap();
        let id = self
            .context
            .backend_submission_for_test_v1(&readback)
            .unwrap();
        (readback, id)
    }

    fn copy(&self, id: u64) -> &CooperativeCopySubmissionV1 {
        let RoutedSubmissionV1::CooperativeCopy(copy) = &self.context.backend().submissions[&id]
        else {
            panic!("router copy expected");
        };
        copy
    }

    fn snapshot(&self) -> (usize, usize, Vec<Stage>) {
        let backend = self.context.backend();
        let steps = backend
            .children
            .iter()
            .map(|child| child.scripted_sdma.as_ref().unwrap().remaining_steps())
            .sum();
        let active = backend
            .children
            .iter()
            .map(|child| child.active_sdma.len())
            .sum();
        let trace = backend
            .submissions
            .values()
            .find_map(|submission| match submission {
                RoutedSubmissionV1::CooperativeCopy(copy) => {
                    copy.compute_xgmi.as_ref().map(|root| root.trace.clone())
                }
                _ => None,
            })
            .unwrap_or_default();
        (steps, active, trace)
    }

    fn clean(mut self) {
        for allocation in [self.source, self.destination, self.host] {
            self.context.release_allocation(allocation).unwrap();
        }
        self.context.destroy_stream(self.readback_stream).unwrap();
        self.context.destroy_stream(self.peer_stream).unwrap();
        let cleanup = self.context.cleanup();
        assert!(
            cleanup.is_complete() && cleanup.failures().is_empty(),
            "{cleanup:?}"
        );
        let backend = self.context.backend_mut_for_test_v1();
        backend.assert_cooperative_indexes_consistent();
        assert_eq!(backend.cooperative_staging_bytes, 0);
        for child in &backend.children {
            let driver = child.scripted_sdma.as_ref().unwrap();
            assert_eq!(driver.remaining_steps(), 0);
            assert_eq!(driver.live_owner_count(), 0);
            assert_eq!(driver.unexpected_drops(), 0);
        }
        backend.shutdown_native_v1().unwrap();
        drop(ManuallyDrop::into_inner(self.context));
    }
}

#[test]
fn pending_native_peer_readback_admits_before_and_after_publication_without_child_custody() {
    for journal in [false, true] {
        for published in [false, true] {
            let mut f = ReadbackFixture::new(true, journal, None, false);
            let (mut peer, peer_id, event, _) = f.peer();
            if published {
                f.context.flush_stream(f.peer_stream).unwrap();
                assert_eq!(
                    f.copy(peer_id).compute_xgmi.as_ref().unwrap().phase,
                    Phase::Published
                );
                for route in [f.copy(peer_id).source, f.copy(peer_id).destination] {
                    assert!(
                        matches!(f.context.backend().children[route.child].allocations[&route.local].sdma_storage,
                        KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::ComputeXgmi(id)) if id == peer_id)
                    );
                }
            }
            let before = f.snapshot();
            let (mut readback, readback_id) = f.readback(event);
            assert_eq!(f.snapshot(), before);
            assert_eq!(f.copy(readback_id).dependencies, [peer_id]);
            assert!(f.copy(readback_id).compute_xgmi.is_none());
            assert!(f.copy(readback_id).sdma_leaf.is_none());
            assert_eq!(f.copy(readback_id).staging.len(), BYTES);
            f.context.release_event(event).unwrap();
            assert!(f.context.release_allocation(f.destination).is_err());
            assert!(f.context.release_allocation(f.host).is_err());
            assert!(f.context.destroy_stream(f.readback_stream).is_err());
            assert!(
                f.context
                    .write_allocation(f.destination, 0, &[0; BYTES])
                    .is_err()
            );
            assert!(
                f.context
                    .backend_mut_for_test_v1()
                    .release_submission_v1(peer_id)
                    .is_err()
            );
            for _ in 0..3 {
                assert_eq!(
                    f.context.poll(&mut readback).unwrap(),
                    RuntimePollV1::Pending
                );
                assert_eq!(
                    f.context
                        .wait(&mut readback, Duration::from_millis(1))
                        .unwrap(),
                    RuntimePollV1::Pending
                );
                assert_eq!(f.snapshot(), before);
            }
            assert!(f.context.drain(&mut readback, Instant::now()).is_err());
            assert_eq!(f.snapshot(), before);
            for _ in 0..64 {
                f.context.flush_stream(f.readback_stream).unwrap();
                if f.copy(readback_id).sdma_leaf.is_some() {
                    assert_eq!(f.copy(peer_id).status(), BackendPollV1::Succeeded);
                    assert!(
                        f.copy(peer_id)
                            .compute_xgmi
                            .as_ref()
                            .unwrap()
                            .is_quiescent()
                    );
                    assert!(
                        f.context
                            .backend()
                            .compute_xgmi_children
                            .iter()
                            .all(Option::is_none)
                    );
                }
                if f.context.poll(&mut readback).unwrap() == RuntimePollV1::Succeeded {
                    break;
                }
            }
            assert_eq!(
                f.context.poll(&mut readback).unwrap(),
                RuntimePollV1::Succeeded
            );
            assert_eq!(f.context.poll(&mut peer).unwrap(), RuntimePollV1::Succeeded);
            assert_eq!(
                f.copy(peer_id).compute_xgmi.as_ref().unwrap().trace.last(),
                Some(&Stage::Restore)
            );
            assert_eq!(f.context.backend().completed_compute_xgmi_copies_v1(), 0);
            let mut output = [0; BYTES];
            f.context.read_allocation(f.host, 0, &mut output).unwrap();
            assert_eq!(output, [0x53; BYTES]);
            f.context.release_submission(readback).unwrap();
            f.context.release_submission(peer).unwrap();
            f.clean();
        }
    }
}

#[test]
fn cancelling_unstarted_readback_keeps_published_peer_and_its_original_owners() {
    for journal in [false, true] {
        let mut f = ReadbackFixture::new(false, journal, None, false);
        let (mut peer, peer_id, event, _) = f.peer();
        f.context.flush_stream(f.peer_stream).unwrap();
        let (mut readback, _) = f.readback(event);
        let before = f.snapshot();
        assert_eq!(
            f.context.cancel(&mut readback).unwrap(),
            RuntimeCancellationV1::Cancelled
        );
        assert_eq!(f.snapshot(), before);
        assert_eq!(
            f.copy(peer_id).compute_xgmi.as_ref().unwrap().phase,
            Phase::Published
        );
        f.context.release_submission(readback).unwrap();
        f.context.release_event(event).unwrap();
        for _ in 0..32 {
            f.context.flush_stream(f.peer_stream).unwrap();
            if f.context.poll(&mut peer).unwrap() == RuntimePollV1::Succeeded {
                break;
            }
        }
        assert_eq!(f.context.poll(&mut peer).unwrap(), RuntimePollV1::Succeeded);
        f.context.release_submission(peer).unwrap();
        f.clean();
    }
}

#[test]
fn cancelled_peer_fails_its_readback_without_issuing_a_child_leaf() {
    for journal in [false, true] {
        let mut f = ReadbackFixture::new(false, journal, None, false);
        let (mut peer, peer_id, event, _) = f.peer();
        let (mut readback, id) = f.readback(event);
        f.context.release_event(event).unwrap();
        let before = f.snapshot();
        assert_eq!(
            f.context.cancel(&mut peer).unwrap(),
            RuntimeCancellationV1::Cancelled
        );
        assert!(f.context.flush_stream(f.readback_stream).is_err());
        assert!(matches!(
            f.context.poll(&mut readback).unwrap(),
            RuntimePollV1::Failed { .. }
        ));
        assert!(f.copy(id).sdma_leaf.is_none());
        assert_eq!(f.snapshot().0, before.0);
        assert!(
            f.copy(peer_id)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .is_quiescent()
        );
        f.context.release_submission(readback).unwrap();
        f.context.release_submission(peer).unwrap();
        f.clean();
    }
}

#[test]
fn peer_readback_rejects_missing_duplicate_foreign_and_unsupported_profiles_before_effects() {
    let mut f = ReadbackFixture::new(false, false, None, false);
    let (mut peer, peer_id, event, backend_event) = f.peer();
    let source = BackendMemoryRegionV1 {
        access: RuntimeAccessV1::Read,
        ..f.copy(peer_id).destination_region
    };
    let host = *f
        .context
        .backend()
        .allocations
        .iter()
        .find(|(_, route)| {
            f.context.backend().children[route.child].allocations[&route.local].kind
                == RuntimeMemoryKindV1::HostVisible
        })
        .unwrap()
        .0;
    let destination = BackendMemoryRegionV1 {
        allocation: host,
        access: RuntimeAccessV1::Write,
        byte_offset: 0,
        byte_len: BYTES as u64,
    };
    for (read, write, dependencies) in [
        (source, destination, vec![]),
        (source, destination, vec![backend_event, backend_event]),
        (source, destination, vec![u64::MAX]),
        (
            source,
            destination,
            vec![backend_event; MAX_RUNTIME_DEPENDENCIES_V1 + 1],
        ),
        (
            BackendMemoryRegionV1 {
                access: RuntimeAccessV1::ReadWrite,
                ..source
            },
            destination,
            vec![backend_event],
        ),
        (
            source,
            BackendMemoryRegionV1 {
                access: RuntimeAccessV1::ReadWrite,
                ..destination
            },
            vec![backend_event],
        ),
        (
            BackendMemoryRegionV1 {
                byte_offset: 1,
                ..source
            },
            destination,
            vec![backend_event],
        ),
        (
            BackendMemoryRegionV1 {
                byte_offset: u64::MAX,
                ..source
            },
            destination,
            vec![backend_event],
        ),
        (
            BackendMemoryRegionV1 {
                byte_len: 0,
                ..source
            },
            BackendMemoryRegionV1 {
                byte_len: 0,
                ..destination
            },
            vec![backend_event],
        ),
        (source, source, vec![backend_event]),
    ] {
        let before = f.snapshot();
        let backend = f.context.backend_mut_for_test_v1();
        let submissions = backend.submissions.len();
        let staging = backend.cooperative_staging_bytes;
        assert!(
            backend
                .copy_async_v1(f.backend_readback_stream, read, write, &dependencies)
                .is_err()
        );
        assert_eq!(backend.submissions.len(), submissions);
        assert_eq!(backend.cooperative_staging_bytes, staging);
        backend.assert_cooperative_indexes_consistent();
        assert_eq!(f.snapshot(), before);
    }
    f.context.release_event(event).unwrap();
    assert!(
        f.context
            .backend_mut_for_test_v1()
            .copy_async_v1(
                f.backend_readback_stream,
                source,
                destination,
                &[backend_event]
            )
            .is_err()
    );
    assert_eq!(
        f.context.cancel(&mut peer).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    f.context.release_submission(peer).unwrap();
    f.clean();
}

#[test]
fn peer_readback_authenticates_contained_ranges_and_preserves_capacity_admission() {
    let mut f = ReadbackFixture::new(false, false, None, false);
    let (mut peer, peer_id, event, backend_event) = f.peer();
    let source = BackendMemoryRegionV1 {
        access: RuntimeAccessV1::Read,
        ..f.copy(peer_id).destination_region
    };
    let original_source = f.copy(peer_id).source_region.allocation;
    let backend = f.context.backend_mut_for_test_v1();
    let host = *backend
        .allocations
        .iter()
        .find(|(_, route)| {
            backend.children[route.child].allocations[&route.local].kind
                == RuntimeMemoryKindV1::HostVisible
        })
        .unwrap()
        .0;
    let destination = BackendMemoryRegionV1 {
        allocation: host,
        access: RuntimeAccessV1::Write,
        byte_offset: 0,
        byte_len: BYTES as u64,
    };
    let source_route = backend.allocations[&source.allocation];
    let destination_route = backend.allocations[&host];
    let stream = backend.streams[&f.backend_readback_stream];
    let eligible = |backend: &KfdMultiDeviceRuntimeBackendV1, source, destination| {
        backend.pending_native_peer_readback_v1(
            stream,
            source,
            source_route,
            destination,
            destination_route,
            &[backend_event],
        )
    };
    assert!(eligible(backend, source, destination));
    assert!(eligible(
        backend,
        BackendMemoryRegionV1 {
            byte_offset: 7,
            byte_len: 17,
            ..source
        },
        BackendMemoryRegionV1 {
            byte_offset: 3,
            byte_len: 17,
            ..destination
        }
    ));
    let original_event = backend.events[&backend_event];
    backend.events.insert(
        backend_event,
        RoutedEventV1::CooperativeCopy {
            submission: peer_id,
            child: 0,
        },
    );
    assert!(!eligible(backend, source, destination));
    backend.events.insert(backend_event, original_event);
    let original_route = backend.allocations[&original_source];
    backend.allocations.insert(
        original_source,
        RoutedHandleV1 {
            local: u64::MAX,
            ..original_route
        },
    );
    assert!(!eligible(backend, source, destination));
    backend.allocations.insert(original_source, original_route);
    if let RoutedSubmissionV1::CooperativeCopy(copy) =
        backend.submissions.get_mut(&peer_id).unwrap()
    {
        copy.destination_region.byte_len -= 1;
    }
    assert!(!eligible(backend, source, destination));
    if let RoutedSubmissionV1::CooperativeCopy(copy) =
        backend.submissions.get_mut(&peer_id).unwrap()
    {
        copy.destination_region.byte_len += 1;
    }
    for fault in 0..3 {
        let limit = backend.cooperative_staging_limit_bytes;
        match fault {
            0 => backend.cooperative_staging_limit_bytes = 2 * BYTES as u64 - 1,
            1 => {
                backend
                    .cooperative_dependency_retain_counts
                    .insert(peer_id, usize::MAX);
            }
            _ => {
                let RoutedSubmissionV1::CooperativeCopy(copy) =
                    backend.submissions.get_mut(&peer_id).unwrap()
                else {
                    unreachable!()
                };
                copy.dependency_depth = MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1;
            }
        }
        let before = (
            backend.submissions.len(),
            backend.next_handle,
            backend.cooperative_staging_bytes,
        );
        assert!(
            matches!(backend.copy_async_v1(f.backend_readback_stream, source, destination, &[backend_event]), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity)
        );
        assert_eq!(
            (
                backend.submissions.len(),
                backend.next_handle,
                backend.cooperative_staging_bytes
            ),
            before
        );
        backend.cooperative_staging_limit_bytes = limit;
        backend
            .cooperative_dependency_retain_counts
            .remove(&peer_id);
        let RoutedSubmissionV1::CooperativeCopy(copy) =
            backend.submissions.get_mut(&peer_id).unwrap()
        else {
            unreachable!()
        };
        copy.dependency_depth = 1;
        backend.assert_cooperative_indexes_consistent();
    }
    f.context.release_event(event).unwrap();
    assert_eq!(
        f.context.cancel(&mut peer).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    f.context.release_submission(peer).unwrap();
    f.clean();
}

#[test]
fn peer_readback_terminal_or_unwind_preserves_both_roots_and_never_enters_the_child() {
    for unwind in [false, true] {
        for stage in [
            Stage::Create,
            Stage::Copy,
            Stage::Poll,
            Stage::Finish,
            Stage::Retire,
            Stage::Restore,
        ] {
            let mut f = ReadbackFixture::new(false, false, Some(stage), unwind);
            let (_, peer_id, event, _) = f.peer();
            let (_, readback_id) = f.readback(event);
            f.context.release_event(event).unwrap();
            let steps = f.snapshot().0;
            let result = catch_unwind(AssertUnwindSafe(|| {
                for _ in 0..32 {
                    f.context.flush_stream(f.readback_stream)?;
                }
                Ok::<_, crate::RuntimeErrorV1<KfdRuntimeBackendErrorV1>>(())
            }));
            assert!(result.is_err() || result.unwrap().is_err());
            assert!(f.copy(readback_id).sdma_leaf.is_none());
            assert!(
                !f.copy(peer_id)
                    .compute_xgmi
                    .as_ref()
                    .unwrap()
                    .is_quiescent()
            );
            assert_eq!(f.snapshot().0, steps);
            assert_eq!(f.snapshot().1, 0);
            assert_eq!(
                f.context.backend().cooperative_dependency_retain_counts[&peer_id],
                1
            );
            assert!(f.context.release_allocation(f.destination).is_err());
            // ManuallyDrop deliberately preserves uncertain native roots for process exit.
        }
    }
}
