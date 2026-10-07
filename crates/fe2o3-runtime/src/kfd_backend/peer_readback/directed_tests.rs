//! Directed-native readback composition over actual router and scripted owners.
//! These CPU ownership and byte transitions are not native hardware evidence.

use super::*;
use crate::{
    BackendCancellationV1, BackendDirectedPeerRouteV1, BackendDirectedScalarPeerCopyV1,
    RuntimeDirectedScalarPeerCopyBackendV1, RuntimeDirectedScalarPeerCopyV1,
};

type Directed = RuntimeSubmissionV1<RuntimeDirectedScalarPeerCopyV1>;

fn directed(f: &mut ReadbackFixture) -> (Directed, u64, RuntimeEventIdV1, u64) {
    let peer = f
        .context
        .directed_peer_copy_v1(
            f.peer_stream,
            region(f.source, RuntimeAccessV1::Read),
            region(f.destination, RuntimeAccessV1::Write),
            &[],
        )
        .unwrap();
    let id = f.context.backend_submission_for_test_v1(&peer).unwrap();
    let event = f.context.record_event(&peer).unwrap();
    let backend_event = *f.context.backend().events.keys().max().unwrap();
    assert!(f.copy(id).directed.is_some());
    assert!(f.copy(id).compute_xgmi.is_some());
    assert!(f.copy(id).staging.is_empty());
    (peer, id, event, backend_event)
}

fn owners(f: &ReadbackFixture, peer: u64) -> [Option<u64>; 2] {
    [f.copy(peer).source, f.copy(peer).destination].map(|route| {
        let KfdRuntimeSdmaStorageV1::Device(owner) =
            &f.context.backend().children[route.child].allocations[&route.local].sdma_storage
        else {
            panic!("exact peer owner must be locally restored")
        };
        owner.scripted_owner_id()
    })
}

fn backend_regions(f: &ReadbackFixture, peer: u64) -> [BackendMemoryRegionV1; 2] {
    let backend = f.context.backend();
    let host = *backend
        .allocations
        .iter()
        .find(|(_, route)| {
            backend.children[route.child].allocations[&route.local].kind
                == RuntimeMemoryKindV1::HostVisible
        })
        .unwrap()
        .0;
    [
        BackendMemoryRegionV1 {
            access: RuntimeAccessV1::Read,
            ..f.copy(peer).destination_region
        },
        BackendMemoryRegionV1 {
            allocation: host,
            access: RuntimeAccessV1::Write,
            byte_offset: 0,
            byte_len: BYTES as u64,
        },
    ]
}

#[test]
fn directed_native_readback_admits_before_and_after_publication_without_child_custody() {
    for journal in [false, true] {
        for published in [false, true] {
            let mut f = ReadbackFixture::new(true, journal, None, false);
            let (mut peer, peer_id, event, _) = directed(&mut f);
            let original = owners(&f, peer_id);
            if published {
                f.context.flush_stream(f.peer_stream).unwrap();
                assert_eq!(
                    f.copy(peer_id).compute_xgmi.as_ref().unwrap().phase,
                    Phase::Published
                );
            }
            let before = f.snapshot();
            let (mut readback, readback_id) = f.readback(event);
            assert_eq!(f.snapshot(), before);
            assert_eq!(f.copy(readback_id).dependencies, [peer_id]);
            assert!(f.copy(readback_id).directed.is_none());
            assert!(f.copy(readback_id).sdma_leaf.is_none());
            assert!(
                f.context
                    .backend()
                    .children
                    .iter()
                    .all(|child| child.active_sdma.is_empty())
            );
            f.context.release_event(event).unwrap();
            assert!(
                f.context
                    .backend_mut_for_test_v1()
                    .release_submission_v1(peer_id)
                    .is_err()
            );
            assert!(f.context.release_allocation(f.destination).is_err());
            assert!(f.context.release_allocation(f.host).is_err());
            for _ in 0..3 {
                assert_eq!(
                    f.context.poll(&mut readback).unwrap(),
                    RuntimePollV1::Pending
                );
                assert_eq!(
                    f.context.wait(&mut readback, Duration::ZERO).unwrap(),
                    RuntimePollV1::Pending
                );
                assert!(f.context.drain(&mut readback, Instant::now()).is_err());
                assert_eq!(f.snapshot(), before);
            }
            let deadline = Instant::now() + Duration::from_secs(1);
            for _ in 0..32 {
                if f.context.drain(&mut readback, deadline).unwrap() == RuntimePollV1::Succeeded {
                    break;
                }
            }
            assert_eq!(
                f.context.poll(&mut readback).unwrap(),
                RuntimePollV1::Succeeded
            );
            assert_eq!(f.context.poll(&mut peer).unwrap(), RuntimePollV1::Succeeded);
            assert_eq!(owners(&f, peer_id), original);
            assert!(
                f.context
                    .backend()
                    .compute_xgmi_children
                    .iter()
                    .all(Option::is_none)
            );
            assert_eq!(
                f.copy(peer_id).compute_xgmi.as_ref().unwrap().trace.last(),
                Some(&Stage::Restore)
            );
            assert_eq!(f.context.backend().completed_compute_xgmi_copies_v1(), 0);
            let mut bytes = [0; BYTES];
            f.context.read_allocation(f.host, 0, &mut bytes).unwrap();
            assert_eq!(bytes, [0x53; BYTES]);
            f.context.release_submission(readback).unwrap();
            f.context.release_submission(peer).unwrap();
            f.clean();
        }
    }
}

#[test]
fn directed_readback_cancellation_is_independent_and_failed_parent_never_issues_dma() {
    for journal in [false, true] {
        for cancel_parent in [false, true] {
            let mut f = ReadbackFixture::new(false, journal, None, false);
            let (mut peer, peer_id, event, _) = directed(&mut f);
            if !cancel_parent {
                f.context.flush_stream(f.peer_stream).unwrap();
            }
            let (mut readback, id) = f.readback(event);
            f.context.release_event(event).unwrap();
            let before = f.snapshot();
            if cancel_parent {
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
                assert!(
                    f.copy(peer_id)
                        .compute_xgmi
                        .as_ref()
                        .unwrap()
                        .trace
                        .is_empty()
                );
            } else {
                assert_eq!(
                    f.context.cancel(&mut readback).unwrap(),
                    RuntimeCancellationV1::Cancelled
                );
                assert_eq!(f.snapshot(), before);
                assert_eq!(
                    f.context.backend().compute_xgmi_children,
                    [Some(peer_id), Some(peer_id)]
                );
                assert_eq!(
                    f.context
                        .drain(&mut peer, Instant::now() + Duration::from_secs(1))
                        .unwrap(),
                    RuntimePollV1::Succeeded
                );
            }
            assert_eq!(f.snapshot().0, before.0);
            assert_eq!(f.snapshot().1, 0);
            f.context.release_submission(readback).unwrap();
            f.context.release_submission(peer).unwrap();
            f.clean();
        }
    }
}

#[test]
fn directed_readback_authenticates_exact_event_access_and_contained_ranges() {
    let mut f = ReadbackFixture::new(false, false, None, false);
    let (mut peer, peer_id, event, backend_event) = directed(&mut f);
    let [source, destination] = backend_regions(&f, peer_id);
    for (source, destination, dependencies) in [
        (source, destination, vec![]),
        (source, destination, vec![u64::MAX]),
        (source, destination, vec![backend_event, backend_event]),
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
            source,
            BackendMemoryRegionV1 {
                byte_offset: u64::MAX,
                ..destination
            },
            vec![backend_event],
        ),
    ] {
        let before = f.snapshot();
        let backend = f.context.backend_mut_for_test_v1();
        let size = backend.submissions.len();
        let staging = backend.cooperative_staging_bytes;
        assert!(
            backend
                .copy_async_v1(
                    f.backend_readback_stream,
                    source,
                    destination,
                    &dependencies
                )
                .is_err()
        );
        assert_eq!(backend.submissions.len(), size);
        assert_eq!(backend.cooperative_staging_bytes, staging);
        assert!(!backend.terminal);
        assert_eq!(f.snapshot(), before);
    }
    let before = f.snapshot();
    let id = f
        .context
        .backend_mut_for_test_v1()
        .copy_async_v1(
            f.backend_readback_stream,
            BackendMemoryRegionV1 {
                byte_offset: 7,
                byte_len: 17,
                ..source
            },
            BackendMemoryRegionV1 {
                byte_offset: 3,
                byte_len: 17,
                ..destination
            },
            &[backend_event],
        )
        .unwrap();
    assert_eq!(f.snapshot(), before);
    assert_eq!(f.copy(id).dependencies, [peer_id]);
    assert_eq!(f.copy(id).staging.len(), 17);
    assert_eq!(
        f.context.backend_mut_for_test_v1().cancel_v1(id).unwrap(),
        BackendCancellationV1::Cancelled
    );
    f.context
        .backend_mut_for_test_v1()
        .release_submission_v1(id)
        .unwrap();
    f.context.release_event(event).unwrap();
    assert_eq!(
        f.context.cancel(&mut peer).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    f.context.release_submission(peer).unwrap();
    f.clean();
}

#[test]
fn directed_readback_corrupt_provenance_fails_before_readback_admission() {
    for published in [false, true] {
        let mut f = ReadbackFixture::new(false, false, None, false);
        let (_, peer_id, _, event) = directed(&mut f);
        if published {
            f.context.flush_stream(f.peer_stream).unwrap();
        }
        let [source, destination] = backend_regions(&f, peer_id);
        let before = f.snapshot();
        let backend = f.context.backend_mut_for_test_v1();
        let size = backend.submissions.len();
        let RoutedSubmissionV1::CooperativeCopy(copy) =
            backend.submissions.get_mut(&peer_id).unwrap()
        else {
            unreachable!()
        };
        copy.byte_cursor = 1;
        assert!(matches!(
            backend.copy_async_v1(f.backend_readback_stream, source, destination, &[event]),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(backend.terminal);
        assert_eq!(backend.submissions.len(), size);
        assert_eq!(f.snapshot(), before);
        assert_eq!(f.context.backend().cooperative_staging_bytes, 0);
        // Intentionally corrupted/ambiguous roots remain under ManuallyDrop.
    }
}

#[test]
fn directed_readback_native_failure_and_unwind_retain_both_roots_without_child_dma() {
    for unwind in [false, true] {
        let mut f = ReadbackFixture::new(false, true, Some(Stage::Poll), unwind);
        let (_, peer_id, event, _) = directed(&mut f);
        let (mut readback, id) = f.readback(event);
        f.context.release_event(event).unwrap();
        let steps = f.snapshot().0;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            f.context
                .drain(&mut readback, Instant::now() + Duration::from_secs(1))
        }));
        if unwind {
            assert!(outcome.is_err());
        } else {
            assert!(outcome.unwrap().is_err());
        }
        assert!(f.context.backend().terminal);
        assert!(
            f.context
                .backend()
                .children
                .iter()
                .all(|child| child.terminal)
        );
        assert!(f.copy(id).sdma_leaf.is_none());
        assert!(
            !f.copy(peer_id)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .is_quiescent()
        );
        assert_eq!(
            f.context.backend().compute_xgmi_children,
            [Some(peer_id), Some(peer_id)]
        );
        assert_eq!(
            f.context.backend().cooperative_dependency_retain_counts[&peer_id],
            1
        );
        assert_eq!(f.snapshot().0, steps);
        assert_eq!(f.snapshot().1, 0);
        // No cleanup or quiescence is claimed for the ambiguous prefix.
    }
}

#[test]
fn directed_readback_tail_progress_retires_an_active_shared_source_sibling() {
    let mut destination_steps = readback_steps();
    destination_steps.push(ScriptedSdmaStepV1::Recycle(
        ScriptedRecycleOutcomeV1::Success,
    ));
    let mut f = Fixture::with_child_steps(
        None,
        false,
        3,
        BYTES,
        vec![Vec::new(), Vec::new(), destination_steps],
    );
    f.backend.compute_xgmi_routes.remove(&(2, 3));
    f.backend.compute_xgmi_routes.insert(
        (0, 2),
        Route::Scripted {
            failure: None,
            unwind: false,
            pending_samples: 3,
        },
    );
    let third = *f
        .backend
        .allocations
        .iter()
        .find(|(_, route)| route.child == 2)
        .unwrap()
        .0;
    let third_route = f.backend.allocations[&third];
    let KfdRuntimeSdmaStorageV1::Device(owner) = &mut f.backend.children[2]
        .allocations
        .get_mut(&third_route.local)
        .unwrap()
        .sdma_storage
    else {
        unreachable!()
    };
    owner.scripted_bytes_mut().unwrap().fill(0x29);
    let sibling_stream = f.backend.create_stream_v1(9).unwrap();
    let readback_stream = f.backend.create_stream_v1(9).unwrap();
    let host = f.unrelated(2);
    let host_route = f.backend.allocations[&host];
    let mut host_owner = f.backend.children[2]
        .scripted_sdma
        .as_ref()
        .unwrap()
        .test_host_owner(BYTES);
    host_owner.scripted_bytes_mut().unwrap().fill(0x29);
    let record = f.backend.children[2]
        .allocations
        .get_mut(&host_route.local)
        .unwrap();
    Arc::make_mut(&mut record.bytes).fill(0x29);
    record.sdma_storage = KfdRuntimeSdmaStorageV1::Host(host_owner);
    record.sdma_backed = true;
    record.sdma_initialized = true;
    let first_route = BackendDirectedPeerRouteV1 {
        stream: f.stream,
        source_device: 7,
        destination_device: 8,
        source: f.source,
        destination: f.destination,
    };
    let sibling_route = BackendDirectedPeerRouteV1 {
        stream: sibling_stream,
        destination_device: 9,
        destination: BackendMemoryRegionV1 {
            allocation: third,
            ..f.destination
        },
        ..first_route
    };
    let endpoints = [f.source.allocation, f.destination.allocation, third]
        .map(|allocation| f.backend.allocations[&allocation]);
    let owners = endpoints.map(|route| {
        let KfdRuntimeSdmaStorageV1::Device(owner) =
            &f.backend.children[route.child].allocations[&route.local].sdma_storage
        else {
            unreachable!()
        };
        owner.scripted_owner_id()
    });
    let first = f
        .backend
        .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
            route: first_route,
            dependencies: &[],
        })
        .unwrap();
    let sibling = f
        .backend
        .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
            route: sibling_route,
            dependencies: &[],
        })
        .unwrap();
    assert!(f.copy(first).compute_xgmi.is_some() && f.copy(sibling).compute_xgmi.is_some());
    assert!(f.copy(sibling).dependencies.is_empty());
    let event = f.backend.record_event_v1(sibling_stream, sibling).unwrap();
    for _ in 0..8 {
        f.backend.progress_retained_directed_peer_v1(first).unwrap();
        if !f.root(first).is_quiescent() {
            break;
        }
    }
    assert_eq!(
        f.backend.compute_xgmi_children,
        [Some(first), Some(first), None]
    );
    let readback = f
        .backend
        .copy_async_v1(
            readback_stream,
            BackendMemoryRegionV1 {
                access: RuntimeAccessV1::Read,
                ..sibling_route.destination
            },
            BackendMemoryRegionV1 {
                allocation: host,
                ..f.destination
            },
            &[event],
        )
        .unwrap();
    f.backend.release_event_v1(event).unwrap();
    assert_eq!(f.copy(readback).dependencies, [sibling]);
    assert!(
        f.backend
            .children
            .iter()
            .all(|child| child.active_sdma.is_empty())
    );
    assert_eq!(
        f.backend.drain_v1(readback, Instant::now()).unwrap(),
        BackendPollV1::Pending
    );
    assert_eq!(
        f.backend
            .drain_v1(readback, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(f.backend.poll_v1(first).unwrap(), BackendPollV1::Succeeded);
    assert_eq!(
        f.backend.poll_v1(sibling).unwrap(),
        BackendPollV1::Succeeded
    );
    assert!(
        f.copy(sibling)
            .directed
            .as_ref()
            .unwrap()
            .dependencies()
            .is_empty()
    );
    assert_eq!(f.backend.compute_xgmi_children, [None, None, None]);
    assert_eq!(f.backend.completed_compute_xgmi_copies_v1(), 0);
    for (route, expected) in endpoints.into_iter().zip(owners) {
        let KfdRuntimeSdmaStorageV1::Device(owner) =
            &f.backend.children[route.child].allocations[&route.local].sdma_storage
        else {
            panic!("native peer owner must be restored before readback success")
        };
        assert_eq!(owner.scripted_owner_id(), expected);
        assert_eq!(owner.scripted_bytes().unwrap(), &[0x53; BYTES]);
    }
    let mut bytes = [0; BYTES];
    f.backend.read_allocation_v1(host, 0, &mut bytes).unwrap();
    assert_eq!(bytes, [0x53; BYTES]);
    f.backend.release_allocation_v1(host).unwrap();
    f.clean();
}

#[test]
fn directed_readback_after_parent_success_drives_an_independent_native_sibling() {
    for terminal in [false, true] {
        let mut steps = if terminal {
            Vec::new()
        } else {
            readback_steps()
        };
        steps.extend([
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ]);
        let mut f =
            Fixture::with_child_steps(None, false, 3, BYTES, vec![Vec::new(), steps, Vec::new()]);
        f.backend.compute_xgmi_routes.remove(&(2, 3));
        f.backend.compute_xgmi_routes.insert(
            (2, 1),
            Route::Scripted {
                failure: terminal.then_some(Stage::Poll),
                unwind: false,
                pending_samples: 3,
            },
        );
        let third = *f
            .backend
            .allocations
            .iter()
            .find(|(_, route)| route.child == 2)
            .unwrap()
            .0;
        let third_route = f.backend.allocations[&third];
        let record = f.backend.children[2]
            .allocations
            .get_mut(&third_route.local)
            .unwrap();
        let KfdRuntimeSdmaStorageV1::Device(owner) = &mut record.sdma_storage else {
            unreachable!()
        };
        owner.scripted_bytes_mut().unwrap().fill(0x7A);
        record.content_sha256 = None;
        let host = f.unrelated(1);
        let host_route = f.backend.allocations[&host];
        f.backend.children[1].native_available = false;
        f.backend.children[1].sdma_enabled = false;
        let extra = f
            .backend
            .allocate_v1(8, RuntimeMemoryKindV1::DeviceLocal, BYTES as u64, 8)
            .unwrap();
        f.backend.children[1].native_available = true;
        f.backend.children[1].sdma_enabled = true;
        let extra_route = f.backend.allocations[&extra];
        let child = &mut f.backend.children[1];
        let driver = child.scripted_sdma.as_ref().unwrap();
        let mut host_owner = driver.test_host_owner(BYTES);
        host_owner.scripted_bytes_mut().unwrap().fill(0x29);
        let mut extra_owner = driver.test_device_owner(BYTES);
        extra_owner.scripted_bytes_mut().unwrap().fill(0x17);
        let record = child.allocations.get_mut(&host_route.local).unwrap();
        Arc::make_mut(&mut record.bytes).fill(0x29);
        record.sdma_storage = KfdRuntimeSdmaStorageV1::Host(host_owner);
        record.sdma_backed = true;
        record.sdma_initialized = true;
        let record = child.allocations.get_mut(&extra_route.local).unwrap();
        record.sdma_storage = KfdRuntimeSdmaStorageV1::Device(Box::new(extra_owner));
        record.sdma_backed = true;
        record.sdma_initialized = true;
        record.sdma_shadow_dirty = true;
        let readback_stream = f.backend.create_stream_v1(8).unwrap();
        let sibling_stream = f.backend.create_stream_v1(8).unwrap();
        let parent_route = BackendDirectedPeerRouteV1 {
            stream: f.stream,
            source_device: 7,
            destination_device: 8,
            source: f.source,
            destination: f.destination,
        };
        let parent = f
            .backend
            .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                route: parent_route,
                dependencies: &[],
            })
            .unwrap();
        let event = f.backend.record_event_v1(f.stream, parent).unwrap();
        let readback = f
            .backend
            .copy_async_v1(
                readback_stream,
                BackendMemoryRegionV1 {
                    access: RuntimeAccessV1::Read,
                    ..f.destination
                },
                BackendMemoryRegionV1 {
                    allocation: host,
                    ..f.destination
                },
                &[event],
            )
            .unwrap();
        f.backend.release_event_v1(event).unwrap();
        let sibling = f
            .backend
            .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                route: BackendDirectedPeerRouteV1 {
                    stream: sibling_stream,
                    source_device: 9,
                    destination_device: 8,
                    source: BackendMemoryRegionV1 {
                        allocation: third,
                        ..f.source
                    },
                    destination: BackendMemoryRegionV1 {
                        allocation: extra,
                        ..f.destination
                    },
                },
                dependencies: &[],
            })
            .unwrap();
        assert!(f.copy(sibling).compute_xgmi.is_some());
        let endpoints = [f.source.allocation, f.destination.allocation, third, extra]
            .map(|allocation| f.backend.allocations[&allocation]);
        let identities = endpoints.map(|route| {
            let KfdRuntimeSdmaStorageV1::Device(owner) =
                &f.backend.children[route.child].allocations[&route.local].sdma_storage
            else {
                unreachable!()
            };
            owner.scripted_owner_id()
        });
        assert_eq!(
            f.backend
                .drain_v1(parent, Instant::now() + Duration::from_secs(1))
                .unwrap(),
            BackendPollV1::Succeeded
        );
        let before = f.backend.children[1]
            .scripted_sdma
            .as_ref()
            .unwrap()
            .remaining_steps();
        for _ in 0..3 {
            if f.copy(readback).phase == CooperativeCopyPhaseV1::Read {
                break;
            }
            assert_eq!(
                f.backend
                    .progress_cooperative_copy_step_v1(readback)
                    .unwrap(),
                BackendPollV1::Pending
            );
        }
        assert_eq!(f.copy(readback).phase, CooperativeCopyPhaseV1::Read);
        assert!(f.copy(readback).sdma_leaf.is_none());
        for _ in 0..8 {
            assert_eq!(
                f.backend
                    .progress_retained_directed_peer_v1(sibling)
                    .unwrap(),
                BackendPollV1::Pending
            );
            if !f.root(sibling).is_quiescent() {
                break;
            }
        }
        assert_eq!(
            f.backend.compute_xgmi_children,
            [None, Some(sibling), Some(sibling)]
        );
        assert_eq!(f.copy(readback).dependencies, [parent]);
        assert!(!f.backend.submission_retained_as_dependency(sibling));
        let trace = f.root(sibling).trace.clone();
        assert_eq!(f.backend.poll_v1(readback).unwrap(), BackendPollV1::Pending);
        assert_eq!(
            f.backend.wait_v1(readback, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(
            f.backend.drain_v1(readback, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(f.root(sibling).trace, trace);
        assert_eq!(
            f.backend.children[1]
                .scripted_sdma
                .as_ref()
                .unwrap()
                .remaining_steps(),
            before
        );
        assert!(
            f.backend
                .children
                .iter()
                .all(|child| child.active_sdma.is_empty())
        );
        let result = f
            .backend
            .drain_v1(readback, Instant::now() + Duration::from_secs(1));
        if terminal {
            assert!(matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))));
            assert!(f.backend.terminal);
            assert!(!f.backend.children[0].terminal);
            assert!(f.backend.children[1].terminal && f.backend.children[2].terminal);
            assert_eq!(f.copy(parent).status(), BackendPollV1::Succeeded);
            assert_eq!(f.copy(readback).status(), BackendPollV1::Pending);
            assert!(f.copy(readback).sdma_leaf.is_none());
            assert_eq!(
                f.backend.compute_xgmi_children,
                [None, Some(sibling), Some(sibling)]
            );
            assert_eq!(
                f.backend.children[1]
                    .scripted_sdma
                    .as_ref()
                    .unwrap()
                    .remaining_steps(),
                before
            );
            assert!(f.backend.submission_retained_as_dependency(parent));
            // The unrelated terminal owner is not rewritten as a settled failure
            // of the readback, and no cleanup of either retained root is claimed.
            continue;
        }
        assert_eq!(result.unwrap(), BackendPollV1::Succeeded);
        assert_eq!(
            f.backend.poll_v1(sibling).unwrap(),
            BackendPollV1::Succeeded
        );
        assert_eq!(f.backend.compute_xgmi_children, [None, None, None]);
        for ((route, identity), value) in endpoints
            .into_iter()
            .zip(identities)
            .zip([0x53, 0x53, 0x7A, 0x7A])
        {
            let KfdRuntimeSdmaStorageV1::Device(owner) =
                &f.backend.children[route.child].allocations[&route.local].sdma_storage
            else {
                unreachable!()
            };
            assert_eq!(owner.scripted_owner_id(), identity);
            assert_eq!(owner.scripted_bytes().unwrap(), &[value; BYTES]);
        }
        let mut bytes = [0; BYTES];
        f.backend.read_allocation_v1(host, 0, &mut bytes).unwrap();
        assert_eq!(bytes, [0x53; BYTES]);
        assert_eq!(f.backend.completed_compute_xgmi_copies_v1(), 0);
        f.backend.release_allocation_v1(host).unwrap();
        f.clean();
    }
}
