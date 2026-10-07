//! Typed consumer admission over scripted native peer custody, not GPU arithmetic.

use super::*;
use crate::kfd_backend::materialized_completion::ScriptedCompletionStepV1;
use crate::{
    BackendCancellationV1, BackendDirectedPeerRouteV1, BackendDirectedScalarPeerCopyV1,
    BackendLaunchProducerV1, BackendProducerAwareLaunchV1, RuntimeDirectedScalarPeerCopyBackendV1,
    RuntimeLaunchGeometryV1, RuntimeProducerAwareLaunchBackendV1,
};

fn fixture(failure: Option<Stage>, unwind: bool, completes: bool) -> Fixture {
    Fixture::with_destination_steps(
        failure,
        unwind,
        3,
        2,
        if completes {
            vec![ScriptedSdmaStepV1::PromoteInitializedStorage(
                ScriptedFailureModeV1::Success,
            )]
        } else {
            Vec::new()
        },
    )
}

fn fixture_with_unrelated_host(child: usize) -> (Fixture, u64) {
    let mut steps = vec![
        Vec::new(),
        vec![ScriptedSdmaStepV1::PromoteInitializedStorage(
            ScriptedFailureModeV1::Success,
        )],
    ];
    // The host is released explicitly after the consumer, before the parent
    // fixture's existing device demote/recycle sequence.
    steps[child].push(ScriptedSdmaStepV1::Recycle(
        ScriptedRecycleOutcomeV1::Success,
    ));
    let mut f = Fixture::with_child_steps(None, false, 3, BYTES, steps);
    let allocation = f.unrelated(child);
    let route = f.backend.allocations[&allocation];
    let child = &mut f.backend.children[child];
    let mut owner = child.scripted_sdma.as_ref().unwrap().test_host_owner(BYTES);
    let record = child.allocations.get_mut(&route.local).unwrap();
    owner
        .scripted_bytes_mut()
        .unwrap()
        .copy_from_slice(&record.bytes);
    record.sdma_storage = KfdRuntimeSdmaStorageV1::Host(owner);
    record.sdma_backed = true;
    record.sdma_initialized = true;
    record.sdma_shadow_dirty = false;
    (f, allocation)
}

fn admit_materialized_completion(f: &mut Fixture, submission: u64) {
    let route = native_route(f, submission);
    let child = &mut f.backend.children[route.child];
    assert!(child.scripted_materialized_completion.is_none());
    child.scripted_materialized_completion = Some(std::collections::VecDeque::from([
        (route.local, ScriptedCompletionStepV1::PollReady),
        (route.local, ScriptedCompletionStepV1::RecycleReady),
    ]));
}

fn release_unrelated_host(f: &mut Fixture, allocation: u64) {
    let child = f.backend.allocations[&allocation].child;
    f.backend.release_allocation_v1(allocation).unwrap();
    let driver = f.backend.children[child].scripted_sdma.as_ref().unwrap();
    assert_eq!(driver.remaining_steps(), 2);
    assert_eq!(driver.live_owner_count(), 1);
    assert_eq!(driver.unexpected_drops(), 0);
}

fn kernel(f: &mut Fixture, child: usize) -> u64 {
    let device = f.backend.children[child].description.backend_device;
    let module = f
        .backend
        .load_module_v1(device, &crate::synthetic_cov6::module())
        .unwrap();
    f.backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap()
}

fn launch(
    f: &mut Fixture,
    stream: u64,
    kernel: u64,
    allocation: u64,
    dependencies: &[BackendLaunchProducerV1],
) -> u64 {
    let mut kernarg = [0; 16];
    kernarg[8..].copy_from_slice(&(BYTES as u64 / 4).to_le_bytes());
    f.backend
        .submit_producer_aware_launch_v1(BackendProducerAwareLaunchV1 {
            stream,
            kernel,
            explicit_kernarg: &kernarg,
            bindings: &[BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: BYTES as u64,
                },
                kernarg_byte_offset: 0,
            }],
            dependencies,
            geometry: RuntimeLaunchGeometryV1 {
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_shared_bytes: 0,
            },
        })
        .unwrap()
}

fn producer(f: &mut Fixture) -> u64 {
    let id = f
        .backend
        .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
            route: BackendDirectedPeerRouteV1 {
                stream: f.stream,
                source_device: 7,
                destination_device: 8,
                source: f.source,
                destination: f.destination,
            },
            dependencies: &[],
        })
        .unwrap();
    assert!(f.copy(id).directed.is_some());
    assert!(f.copy(id).compute_xgmi.is_some());
    assert!(f.copy(id).staging.is_empty());
    assert_eq!(f.copy(id).scratch_byte_len, 0);
    id
}

fn consumer(f: &mut Fixture, producer: u64, stream: u64, kernel: u64) -> u64 {
    let event = f.backend.record_event_v1(f.stream, producer).unwrap();
    let allocation = f.destination.allocation;
    let id = launch(
        f,
        stream,
        kernel,
        allocation,
        &[BackendLaunchProducerV1 {
            event,
            producer_submission: producer,
        }],
    );
    f.backend.release_event_v1(event).unwrap();
    assert!(f.backend.peer_launch_retains.retains(producer));
    assert!(f.backend.release_submission_v1(producer).is_err());
    let route = native_route(f, id);
    let pending = &f.backend.children[route.child].pending_compute[&route.local];
    assert!(pending.peer_gate.unwrap().owns(id, route.local));
    assert_eq!(pending.dependency_depth, 2);
    assert!(!f.backend.children[route.child].any_compute_active_v1());
    id
}

fn native_route(f: &Fixture, id: u64) -> RoutedHandleV1 {
    let RoutedSubmissionV1::Native { route, .. } = f.backend.submissions[&id] else {
        panic!("directed provenance must use the existing child peer-gated admission")
    };
    route
}

fn assert_observers(f: &mut Fixture, peer: u64, consumer: u64) {
    let before = f.root(peer).trace.clone();
    let steps = f
        .backend
        .children
        .iter()
        .map(|child| child.scripted_sdma.as_ref().unwrap().remaining_steps())
        .collect::<Vec<_>>();
    for _ in 0..3 {
        assert_eq!(f.backend.poll_v1(consumer).unwrap(), BackendPollV1::Pending);
        assert_eq!(
            f.backend.wait_v1(consumer, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(
            f.backend.drain_v1(consumer, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
    }
    assert_eq!(f.root(peer).trace, before);
    assert_eq!(
        f.backend
            .children
            .iter()
            .map(|child| child.scripted_sdma.as_ref().unwrap().remaining_steps())
            .collect::<Vec<_>>(),
        steps
    );
    let route = native_route(f, consumer);
    assert!(
        f.backend.children[route.child]
            .pending_compute
            .contains_key(&route.local)
    );
    assert!(!f.backend.children[route.child].any_compute_active_v1());
}

fn publish(f: &mut Fixture, peer: u64) {
    for _ in 0..8 {
        assert_eq!(
            f.backend.progress_retained_directed_peer_v1(peer).unwrap(),
            BackendPollV1::Pending
        );
        if !f.root(peer).is_quiescent() {
            assert_eq!(f.backend.compute_xgmi_children, [Some(peer), Some(peer)]);
            return;
        }
    }
    panic!("native peer did not acquire its exact endpoint pair")
}

fn release_scripted_materialized_metadata(f: &mut Fixture, child: usize) {
    let child = &mut f.backend.children[child];
    assert!(child.queue.is_none() && child.admitted_device.is_none());
    assert!(!child.any_compute_active_v1());
    assert!(child.pending_compute.is_empty());
    assert!(
        child
            .scripted_materialized_completion
            .as_ref()
            .unwrap()
            .is_empty()
    );
    assert_eq!(child.native_dirty_extents, 0);
    assert!(
        child
            .allocations
            .ordinary_iter()
            .all(|(_, record)| record.native_dirty.is_empty())
    );
    assert!(child.recycled_dispatch.is_some());
    // The scripted materialized completion has no native DATA cache. Discard
    // only its fixture metadata, retaining the real completion and FIFO record.
    child.recycled_dispatch = None;
}

fn finish(f: &mut Fixture, peer: u64, consumer: u64, owners: [Option<u64>; 2]) {
    assert_eq!(
        f.backend
            .drain_v1(consumer, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(f.backend.poll_v1(peer).unwrap(), BackendPollV1::Succeeded);
    assert!(f.backend.peer_launch_retains.is_empty());
    assert_eq!(f.backend.compute_xgmi_children, [None, None]);
    assert!(f.root(peer).is_quiescent());
    assert!(f.root(peer).trace.contains(&Stage::Restore));
    let destination = f.backend.allocations[&f.destination.allocation];
    assert!(
        f.backend.children[destination.child]
            .exact_submission_quiescent_v1(native_route(f, consumer).local)
    );
    // Restore the settled persistent-storage wrapper before inspecting the
    // original scripted device owner; this does not execute a copy or kernel.
    f.backend.children[destination.child]
        .normalize_h2d_ready_v1(destination.local)
        .unwrap();
    for (index, source) in [true, false].into_iter().enumerate() {
        assert_eq!(f.owner(source).scripted_owner_id(), owners[index]);
        assert_eq!(f.owner(source).scripted_bytes().unwrap(), &[0x53; BYTES]);
    }
    assert_eq!(f.backend.cooperative_staging_bytes, 0);
    assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
    let route = native_route(f, consumer);
    assert!(
        !f.backend.children[route.child]
            .pending_compute
            .contains_key(&route.local)
    );
    assert!(
        f.backend.children[route.child]
            .active_compute_lane_v1(route.local)
            .is_none()
    );
    assert_eq!(
        f.backend.children[route.child]
            .last_launch_performance_v1()
            .unwrap()
            .user_data_materializations(),
        0
    );
}

#[test]
fn directed_native_consumer_drain_preserves_exact_peer_permits_and_observers() {
    let mut f = fixture(None, false, true);
    let owners = [
        f.owner(true).scripted_owner_id(),
        f.owner(false).scripted_owner_id(),
    ];
    let stream = f.backend.create_stream_v1(8).unwrap();
    let kernel = kernel(&mut f, 1);
    let peer = producer(&mut f);
    let consumer = consumer(&mut f, peer, stream, kernel);
    let destination = f.backend.allocations[&f.destination.allocation];
    let origin = f
        .backend
        .peer_copy_origin_for_leg_v1(peer, PeerCopyLegV1::Write)
        .unwrap();
    assert!(origin.is_some());
    let child = &f.backend.children[destination.child];
    assert!(child.allocation_is_active(destination.local));
    for purpose in [PeerAccessPurposeV1::Copy, PeerAccessPurposeV1::Reconcile] {
        assert!(!child.peer_access_has_conflict_v1(destination.local, origin, purpose));
        assert!(child.peer_access_has_conflict_v1(destination.local, None, purpose));
    }
    assert_observers(&mut f, peer, consumer);
    assert!(f.root(peer).trace.is_empty());
    // Only the consumer drain drives the peer through publication and restoration.
    finish(&mut f, peer, consumer, owners);
    f.clean();
}

#[test]
fn directed_native_consumer_drain_skips_completed_prefix_while_peer_owns_children() {
    let (mut f, unrelated) = fixture_with_unrelated_host(1);
    let owners = [
        f.owner(true).scripted_owner_id(),
        f.owner(false).scripted_owner_id(),
    ];
    let stream = f.backend.create_stream_v1(8).unwrap();
    let kernel = kernel(&mut f, 1);
    let predecessor = launch(&mut f, stream, kernel, unrelated, &[]);
    admit_materialized_completion(&mut f, predecessor);
    assert_eq!(
        f.backend
            .drain_v1(predecessor, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    let prior_route = native_route(&f, predecessor);
    release_scripted_materialized_metadata(&mut f, 1);
    let peer = producer(&mut f);
    let consumer = consumer(&mut f, peer, stream, kernel);
    let route = native_route(&f, consumer);
    assert_eq!(
        f.backend.children[1].pending_compute[&route.local].ordered_predecessor,
        Some(prior_route.local)
    );
    assert_eq!(
        f.backend.children[1]
            .native_dependency_prefix_v1(core::iter::once(route.local))
            .unwrap()
            .into_iter()
            .map(|(id, _)| id)
            .collect::<Vec<_>>(),
        [prior_route.local, route.local]
    );
    publish(&mut f, peer);
    assert_observers(&mut f, peer, consumer);
    // The completed predecessor is visited before the target's retained gate.
    // Occupancy must skip child I/O without ending that metadata walk.
    finish(&mut f, peer, consumer, owners);
    assert_eq!(
        f.backend.poll_v1(predecessor).unwrap(),
        BackendPollV1::Succeeded
    );
    release_unrelated_host(&mut f, unrelated);
    f.clean();
}

#[test]
fn directed_native_consumer_keeps_unrelated_active_compute_exclusion() {
    let (mut f, unrelated) = fixture_with_unrelated_host(0);
    let owners = [
        f.owner(true).scripted_owner_id(),
        f.owner(false).scripted_owner_id(),
    ];
    let busy_stream = f.backend.create_stream_v1(7).unwrap();
    let busy_kernel = kernel(&mut f, 0);
    let busy = launch(&mut f, busy_stream, busy_kernel, unrelated, &[]);
    admit_materialized_completion(&mut f, busy);
    f.backend.flush_stream_v1(busy_stream).unwrap();
    assert!(f.backend.children[0].any_compute_active_v1());
    let stream = f.backend.create_stream_v1(8).unwrap();
    let kernel = kernel(&mut f, 1);
    let peer = producer(&mut f);
    let consumer = consumer(&mut f, peer, stream, kernel);
    let route = native_route(&f, consumer);
    for _ in 0..4 {
        f.backend
            .service_native_peer_prefix_v1(route, true)
            .unwrap();
        assert!(f.root(peer).trace.is_empty());
        assert_eq!(f.backend.compute_xgmi_children, [None, None]);
        assert!(f.backend.children[0].any_compute_active_v1());
    }
    assert_eq!(
        f.backend
            .drain_v1(busy, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    release_scripted_materialized_metadata(&mut f, 0);
    finish(&mut f, peer, consumer, owners);
    release_unrelated_host(&mut f, unrelated);
    f.clean();
}

#[test]
fn directed_native_cancelled_peer_fails_consumer_without_compute_publication() {
    let mut f = fixture(None, false, false);
    let stream = f.backend.create_stream_v1(8).unwrap();
    let kernel = kernel(&mut f, 1);
    let peer = producer(&mut f);
    let consumer = consumer(&mut f, peer, stream, kernel);
    let route = native_route(&f, consumer);
    assert_eq!(
        f.backend.cancel_v1(peer).unwrap(),
        BackendCancellationV1::Cancelled
    );
    assert!(matches!(
        f.backend
            .drain_v1(consumer, Instant::now() + Duration::from_secs(1)),
        Ok(BackendPollV1::Failed { .. }) | Err(RuntimeBackendFailureV1::Quiescent(_))
    ));
    assert!(matches!(
        f.backend.poll_v1(consumer).unwrap(),
        BackendPollV1::Failed { .. }
    ));
    assert!(f.root(peer).trace.is_empty());
    assert!(!f.backend.children[1].any_compute_active_v1());
    assert!(
        !f.backend.children[1]
            .pending_compute
            .contains_key(&route.local)
    );
    assert!(
        f.backend.children[1]
            .active_compute_lane_v1(route.local)
            .is_none()
    );
    assert!(!f.backend.children[1].submissions[&route.local].profile_dispatch_published);
    assert!(f.backend.peer_launch_retains.is_empty());
    assert_eq!(f.owner(false).scripted_bytes().unwrap(), &[0x17; BYTES]);
    f.clean();
}

#[test]
fn directed_native_consumer_cancellation_waits_for_published_peer_restoration() {
    let mut f = fixture(None, false, false);
    let stream = f.backend.create_stream_v1(8).unwrap();
    let kernel = kernel(&mut f, 1);
    let peer = producer(&mut f);
    let consumer = consumer(&mut f, peer, stream, kernel);
    publish(&mut f, peer);
    let before = f.root(peer).trace.clone();
    assert!(matches!(f.backend.cancel_v1(consumer),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy));
    assert_eq!(f.root(peer).trace, before);
    assert_eq!(f.backend.compute_xgmi_children, [Some(peer), Some(peer)]);
    assert!(f.backend.peer_launch_retains.retains(peer));
    assert_eq!(
        f.backend
            .drain_v1(peer, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(
        f.backend.cancel_v1(consumer).unwrap(),
        BackendCancellationV1::Cancelled
    );
    assert!(f.backend.peer_launch_retains.is_empty());
    assert!(!f.backend.children[1].any_compute_active_v1());
    assert_eq!(f.owner(false).scripted_bytes().unwrap(), &[0x53; BYTES]);
    f.clean();
}

#[test]
fn directed_native_peer_fault_retains_consumer_without_compute_publication() {
    for unwind in [false, true] {
        let mut f = fixture(Some(Stage::Poll), unwind, false);
        let stream = f.backend.create_stream_v1(8).unwrap();
        let kernel = kernel(&mut f, 1);
        let peer = producer(&mut f);
        let consumer = consumer(&mut f, peer, stream, kernel);
        let route = native_route(&f, consumer);
        let result = catch_unwind(AssertUnwindSafe(|| {
            f.backend
                .drain_v1(consumer, Instant::now() + Duration::from_secs(1))
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
        assert!(f.backend.children.iter().all(|child| child.terminal));
        assert_eq!(f.backend.compute_xgmi_children, [Some(peer), Some(peer)]);
        assert!(!f.root(peer).is_quiescent());
        assert!(!f.root(peer).trace.contains(&Stage::Restore));
        assert!(f.backend.peer_launch_retains.retains(peer));
        assert!(
            f.backend.children[1]
                .pending_compute
                .contains_key(&route.local)
        );
        assert!(!f.backend.children[1].submissions.contains_key(&route.local));
        assert!(!f.backend.children[1].any_compute_active_v1());
        assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
        // Ambiguous native owners and their logical consumer are intentionally retained.
    }
}
