//! Late compute admission over actual scripted peer custody, not GPU arithmetic.

use super::*;
use crate::{
    BackendCancellationV1, BackendDirectedPeerRouteV1, BackendDirectedScalarPeerCopyV1,
    BackendLaunchProducerV1, BackendProducerAwareLaunchV1, RuntimeDirectedScalarPeerCopyBackendV1,
    RuntimeLaunchGeometryV1, RuntimeProducerAwareLaunchBackendV1,
};

struct Setup {
    peer: u64,
    event: u64,
    stream: u64,
    kernel: u64,
    gate: u64,
    gate_event: u64,
    gate_route: RoutedHandleV1,
}

fn directed(
    f: &mut Fixture,
    stream: u64,
    source: BackendMemoryRegionV1,
    destination: BackendMemoryRegionV1,
) -> u64 {
    let source_child = f.backend.allocations[&source.allocation].child;
    let destination_child = f.backend.allocations[&destination.allocation].child;
    f.backend
        .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
            route: BackendDirectedPeerRouteV1 {
                stream,
                source_device: 7 + source_child as u64,
                destination_device: 7 + destination_child as u64,
                source,
                destination,
            },
            dependencies: &[],
        })
        .unwrap()
}

fn setup(f: &mut Fixture) -> Setup {
    let stream = f.backend.create_stream_v1(8).unwrap();
    let module = f
        .backend
        .load_module_v1(8, &crate::synthetic_cov6::module())
        .unwrap();
    let kernel = f
        .backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();
    // An unpublished test record holds the child dependency pending; no test
    // fabricates kernel execution or a numerical compute result.
    let (gate, gate_event, gate_route) = f.producer_on(1, BackendPollV1::Pending);
    let (peer_stream, source, destination) = (f.stream, f.source, f.destination);
    let peer = directed(f, peer_stream, source, destination);
    let event = f.backend.record_event_v1(f.stream, peer).unwrap();
    Setup {
        peer,
        event,
        stream,
        kernel,
        gate,
        gate_event,
        gate_route,
    }
}

fn submit(
    f: &mut Fixture,
    s: &Setup,
    region: BackendMemoryRegionV1,
    dependencies: &[BackendLaunchProducerV1],
) -> Result<u64, Failure> {
    let mut kernarg = [0; 16];
    kernarg[8..].copy_from_slice(&(BYTES as u64 / 4).to_le_bytes());
    f.backend
        .submit_producer_aware_launch_v1(BackendProducerAwareLaunchV1 {
            stream: s.stream,
            kernel: s.kernel,
            explicit_kernarg: &kernarg,
            bindings: &[BackendBindingV1 {
                region,
                kernarg_byte_offset: 0,
            }],
            dependencies,
            geometry: RuntimeLaunchGeometryV1 {
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_shared_bytes: 0,
            },
        })
}

fn consumer(f: &mut Fixture, s: &Setup) -> u64 {
    let region = BackendMemoryRegionV1 {
        access: RuntimeAccessV1::Read,
        ..f.destination
    };
    submit(
        f,
        s,
        region,
        &[
            BackendLaunchProducerV1 {
                event: s.event,
                producer_submission: s.peer,
            },
            BackendLaunchProducerV1 {
                event: s.gate_event,
                producer_submission: s.gate,
            },
        ],
    )
    .unwrap()
}

#[test]
fn directed_deferred_services_independent_native_blocker_without_success_dependency() {
    for handed_off in [false, true] {
        let mut steps: Vec<_> = (0..4).map(|_| Vec::new()).collect();
        steps[1] = vec![
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ];
        let mut f = Fixture::with_child_steps(None, false, 2, BYTES, steps);
        f.backend.children[1].native_available = false;
        f.backend.children[1].sdma_enabled = false;
        let extra = f
            .backend
            .allocate_v1(8, RuntimeMemoryKindV1::DeviceLocal, BYTES as u64, 8)
            .unwrap();
        f.backend.children[1].native_available = true;
        f.backend.children[1].sdma_enabled = true;
        let extra_route = f.backend.allocations[&extra];
        let mut owner = f.backend.children[1]
            .scripted_sdma
            .as_ref()
            .unwrap()
            .test_device_owner(BYTES);
        owner.scripted_bytes_mut().unwrap().fill(0x71);
        let record = f.backend.children[1]
            .allocations
            .get_mut(&extra_route.local)
            .unwrap();
        record.sdma_storage = KfdRuntimeSdmaStorageV1::Device(Box::new(owner));
        record.sdma_backed = true;
        record.sdma_initialized = true;
        record.sdma_shadow_dirty = true;
        record.content_sha256 = Some([0x71; 32]);
        f.backend.compute_xgmi_routes.insert(
            (2, 1),
            Route::Scripted {
                failure: None,
                unwind: false,
                pending_samples: 2,
            },
        );
        let source_allocation = *f
            .backend
            .allocations
            .iter()
            .find(|(_, route)| route.child == 2)
            .unwrap()
            .0;
        let source = BackendMemoryRegionV1 {
            allocation: source_allocation,
            ..f.source
        };
        let destination = BackendMemoryRegionV1 {
            allocation: extra,
            ..f.destination
        };
        let stream = f.backend.create_stream_v1(8).unwrap();
        let blocker = directed(&mut f, stream, source, destination);
        let s = setup(&mut f);
        publish(&mut f, s.peer);
        let id = consumer(&mut f, &s);
        finish_peer(&mut f, s.peer);
        if handed_off {
            assert_eq!(
                f.backend.progress_deferred_compute_v1(id).unwrap(),
                BackendPollV1::Pending
            );
            assert!(f.backend.deferred_compute_v1(id).unwrap().route.is_some());
        }
        publish(&mut f, blocker);
        assert!(!f.backend.peer_launch_retains.retains(blocker));
        for _ in 0..64 {
            let before = f.backend.deferred_compute_v1(id).unwrap().route;
            let occupied = f.backend.compute_xgmi_children[1].is_some();
            assert_eq!(
                f.backend.progress_deferred_compute_v1(id).unwrap(),
                BackendPollV1::Pending
            );
            if occupied {
                assert_eq!(f.backend.deferred_compute_v1(id).unwrap().route, before);
            }
            assert!(!f.backend.children[1].any_compute_active_v1());
            if f.copy(blocker).status() == BackendPollV1::Succeeded {
                break;
            }
        }
        assert_eq!(f.copy(blocker).status(), BackendPollV1::Succeeded);
        assert!(f.root(blocker).trace.contains(&Stage::Restore));
        assert_eq!(f.backend.compute_xgmi_children, [None; 4]);
        assert!(!f.backend.peer_launch_retains.retains(blocker));
        assert!(f.backend.peer_launch_retains.retains(s.peer));
        let KfdRuntimeSdmaStorageV1::Device(owner) =
            &f.backend.children[1].allocations[&extra_route.local].sdma_storage
        else {
            panic!("independent owner not restored")
        };
        assert_eq!(owner.scripted_bytes().unwrap(), &[0x53; BYTES]);
        assert_eq!(
            f.backend.progress_deferred_compute_v1(id).unwrap(),
            BackendPollV1::Pending
        );
        assert!(f.backend.deferred_compute_v1(id).unwrap().route.is_some());
        f.backend.assert_cooperative_indexes_consistent();
        finish(f, s, Some(id));
    }
}

fn publish(f: &mut Fixture, peer: u64) {
    for _ in 0..8 {
        assert_eq!(
            f.backend.progress_retained_directed_peer_v1(peer).unwrap(),
            BackendPollV1::Pending
        );
        if !f.root(peer).is_quiescent() {
            return;
        }
    }
    panic!("native peer did not retain its original endpoint owners");
}

fn finish_peer(f: &mut Fixture, peer: u64) {
    for _ in 0..64 {
        if f.backend.progress_retained_directed_peer_v1(peer).unwrap() == BackendPollV1::Succeeded {
            return;
        }
    }
    panic!("scripted peer did not finish");
}

fn finish(mut f: Fixture, s: Setup, consumer: Option<u64>) {
    finish_peer(&mut f, s.peer);
    if let Some(consumer) = consumer {
        assert_eq!(
            f.backend.cancel_v1(consumer).unwrap(),
            BackendCancellationV1::Cancelled
        );
    }
    let gate = f.backend.children[s.gate_route.child]
        .submissions
        .get_mut(&s.gate_route.local)
        .unwrap();
    assert!(!gate.profile_dispatch_published);
    gate.status = BackendPollV1::Failed { code: -2 };
    assert!(
        f.backend
            .children
            .iter()
            .all(|child| !child.any_compute_active_v1())
    );
    f.clean();
}

#[test]
fn directed_deferred_admission_preserves_native_prepublication_route_and_late_custody() {
    for published in [false, true] {
        let mut f = Fixture::configured(None, false, 2, 2);
        let owners = [
            f.owner(true).scripted_owner_id(),
            f.owner(false).scripted_owner_id(),
        ];
        let s = setup(&mut f);
        if published {
            publish(&mut f, s.peer);
        }
        let before = f.root(s.peer).trace.clone();
        let id = consumer(&mut f, &s);
        assert_eq!(f.root(s.peer).trace, before);
        if published {
            assert!(
                matches!(&f.backend.submissions[&id], RoutedSubmissionV1::DeferredCompute(root) if root.route.is_none())
            );
            assert!(f.backend.children[1].pending_compute.is_empty());
        } else {
            let RoutedSubmissionV1::Native { route, .. } = f.backend.submissions[&id] else {
                panic!("unpublished directed peers retain the original native permit path")
            };
            assert!(
                f.backend.children[1].pending_compute[&route.local]
                    .peer_gate
                    .is_some()
            );
        }
        f.backend.release_event_v1(s.event).unwrap();
        f.backend.release_event_v1(s.gate_event).unwrap();
        assert!(f.backend.peer_launch_retains.retains(s.peer));
        assert!(f.backend.release_submission_v1(s.peer).is_err());
        assert!(f.backend.destroy_stream_v1(s.stream).is_err());
        for _ in 0..3 {
            assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
            assert_eq!(
                f.backend.wait_v1(id, Instant::now()).unwrap(),
                BackendPollV1::Pending
            );
            assert_eq!(
                f.backend.drain_v1(id, Instant::now()).unwrap(),
                BackendPollV1::Pending
            );
        }
        assert_eq!(f.root(s.peer).trace, before);
        if published {
            for _ in 0..64 {
                assert_eq!(
                    f.backend.progress_deferred_compute_v1(id).unwrap(),
                    BackendPollV1::Pending
                );
                if f.backend.deferred_compute_v1(id).unwrap().route.is_some() {
                    break;
                }
            }
            let route = f
                .backend
                .deferred_compute_v1(id)
                .unwrap()
                .route
                .expect("restored child handoff");
            assert_eq!(
                f.backend.children[route.child].pending_compute[&route.local].dependency_depth,
                2
            );
            assert_eq!(f.copy(s.peer).status(), BackendPollV1::Succeeded);
            assert!(f.root(s.peer).trace.contains(&Stage::Restore));
            assert_eq!(f.backend.compute_xgmi_children, [None, None]);
            assert_eq!(
                [
                    f.owner(true).scripted_owner_id(),
                    f.owner(false).scripted_owner_id()
                ],
                owners
            );
            assert_eq!(f.owner(false).scripted_bytes().unwrap(), &[0x53; BYTES]);
            assert!(
                f.backend
                    .children
                    .iter()
                    .all(|child| !child.any_compute_active_v1())
            );
        }
        f.backend.assert_cooperative_indexes_consistent();
        finish(f, s, Some(id));
    }
}

#[test]
fn directed_deferred_bad_event_alias_and_ranges_reject_before_effects() {
    for variant in 0..4 {
        let mut f = Fixture::new(None, false);
        let s = setup(&mut f);
        publish(&mut f, s.peer);
        let trace = f.root(s.peer).trace.clone();
        let next = f.backend.next_handle;
        let mut region = BackendMemoryRegionV1 {
            access: RuntimeAccessV1::Read,
            ..f.destination
        };
        let mut dependencies = [BackendLaunchProducerV1 {
            event: s.event,
            producer_submission: s.peer,
        }];
        match variant {
            0 => dependencies[0].event = s.gate_event,
            1 => region.access = RuntimeAccessV1::ReadWrite,
            2 => {
                region.byte_offset = BYTES as u64 - 1;
                region.byte_len = 2;
            }
            3 => region.allocation = f.source.allocation,
            _ => unreachable!(),
        }
        assert!(matches!(
            submit(&mut f, &s, region, &dependencies),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert_eq!(f.backend.next_handle, next);
        assert_eq!(f.root(s.peer).trace, trace);
        assert!(f.backend.children[1].pending_compute.is_empty());
        assert!(f.backend.deferred_compute_retains.is_empty());
        assert!(!f.backend.peer_launch_retains.retains(s.peer));
        assert!(!f.backend.terminal);
        finish(f, s, None);
    }
}

#[test]
fn directed_deferred_cancellation_does_not_cancel_published_parent() {
    let mut f = Fixture::configured(None, false, 3, 2);
    let s = setup(&mut f);
    publish(&mut f, s.peer);
    let id = consumer(&mut f, &s);
    let trace = f.root(s.peer).trace.clone();
    assert_eq!(
        f.backend.cancel_v1(id).unwrap(),
        BackendCancellationV1::Cancelled
    );
    assert_eq!(f.copy(s.peer).status(), BackendPollV1::Pending);
    assert_eq!(f.root(s.peer).trace, trace);
    assert_eq!(
        f.backend.compute_xgmi_children,
        [Some(s.peer), Some(s.peer)]
    );
    assert!(f.backend.deferred_compute_retains.is_empty());
    assert!(!f.backend.peer_launch_retains.retains(s.peer));
    assert!(f.backend.children[1].pending_compute.is_empty());
    finish(f, s, None);
}

#[test]
fn directed_deferred_changed_parent_metadata_fails_before_progress_or_cancellation() {
    for variant in 0..3 {
        let mut f = Fixture::new(None, false);
        let s = setup(&mut f);
        publish(&mut f, s.peer);
        let id = consumer(&mut f, &s);
        let trace = f.root(s.peer).trace.clone();
        let RoutedSubmissionV1::CooperativeCopy(copy) =
            f.backend.submissions.get_mut(&s.peer).unwrap()
        else {
            unreachable!()
        };
        match variant {
            0 => copy.directed = None,
            1 => copy.destination_region.byte_len -= 1,
            2 => copy.dependency_depth += 1,
            _ => unreachable!(),
        }
        let result = if variant == 2 {
            f.backend.cancel_v1(id).map(|_| BackendPollV1::Pending)
        } else {
            f.backend.progress_deferred_compute_v1(id)
        };
        assert!(matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))));
        assert!(f.backend.terminal);
        assert_eq!(f.root(s.peer).trace, trace);
        assert!(f.backend.peer_launch_retains.retains(s.peer));
        assert!(f.backend.deferred_compute_v1(id).unwrap().route.is_none());
    }
}

#[test]
fn directed_deferred_native_error_or_unwind_retains_unrestored_peer_and_consumer() {
    for unwind in [false, true] {
        let mut f = Fixture::configured(Some(Stage::Poll), unwind, 0, 2);
        let s = setup(&mut f);
        publish(&mut f, s.peer);
        let id = consumer(&mut f, &s);
        let result = catch_unwind(AssertUnwindSafe(|| {
            f.backend.progress_deferred_compute_v1(id)
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
        }
        assert!(f.backend.terminal);
        assert_eq!(
            f.backend.compute_xgmi_children,
            [Some(s.peer), Some(s.peer)]
        );
        assert!(f.backend.peer_launch_retains.retains(s.peer));
        assert!(f.backend.deferred_compute_v1(id).unwrap().route.is_none());
        assert!(f.backend.children[1].pending_compute.is_empty());
        assert!(!f.root(s.peer).trace.contains(&Stage::Restore));
    }
}

#[test]
fn directed_deferred_rejects_foreign_or_dangling_native_occupancy_before_progress() {
    for foreign in [false, true] {
        let mut f = Fixture::configured(None, false, 2, 4);
        let s = setup(&mut f);
        publish(&mut f, s.peer);
        let id = consumer(&mut f, &s);
        finish_peer(&mut f, s.peer);
        let owner = if foreign {
            let allocation = |child| {
                *f.backend
                    .allocations
                    .iter()
                    .find(|(_, route)| route.child == child)
                    .unwrap()
                    .0
            };
            let source = BackendMemoryRegionV1 {
                allocation: allocation(2),
                access: RuntimeAccessV1::Read,
                ..f.source
            };
            let destination = BackendMemoryRegionV1 {
                allocation: allocation(3),
                ..f.destination
            };
            let stream = f.backend.create_stream_v1(10).unwrap();
            let other = directed(&mut f, stream, source, destination);
            publish(&mut f, other);
            other
        } else {
            u64::MAX
        };
        let trace = foreign.then(|| f.root(owner).trace.clone());
        f.backend.compute_xgmi_children[1] = Some(owner);
        assert!(matches!(
            f.backend.progress_deferred_compute_v1(id),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(f.backend.terminal);
        assert!(f.backend.deferred_compute_v1(id).unwrap().route.is_none());
        assert!(f.backend.peer_launch_retains.retains(s.peer));
        if let Some(trace) = trace {
            assert_eq!(f.root(owner).trace, trace);
        }
    }
}
