use super::super::super::tests::host_visible_three_binding_launch_v1;
use super::*;

fn install_scaled(backend: &mut KfdRuntimeBackendV1, account: ResourceCreditAccountV1) {
    let state =
        RuntimeDispatchStateV1::try_new(RuntimeDispatchCapacityV1::qualification_1024(account))
            .unwrap();
    backend.dispatch_capacity = state.capacity;
    backend.compute_pipeline = state.primary;
    backend.auxiliary_compute_lanes = state.auxiliary;
}

#[test]
fn scaled_publication_exhaustion_precedes_recycled_detach_and_resident_consumption() {
    for record_exhaustion in [false, true] {
        let (mut backend, launch) = host_visible_three_binding_launch_v1();
        let account = account(
            if record_exhaustion {
                u64::MAX
            } else {
                2 * pipeline_bytes()
            },
            if record_exhaustion { 2 } else { 3 },
        );
        install_scaled(&mut backend, account.clone());
        let prepared = backend
            .prepare_launch(launch.borrowed(), false, false)
            .unwrap();
        assert!(
            !prepared.writebacks.is_empty(),
            "must reach production publication"
        );
        let PreparedLaunchStorageV1::Materialized(data) = &prepared.storage else {
            panic!("ordinary recipe");
        };
        let descriptors = resident_descriptors_v1(data).unwrap();
        let mut old_shape = prepared.dispatch_shape_sha256;
        old_shape[0] ^= 1;
        backend.recycled_dispatch = Some(RecycledDispatchV1 {
            kernel: launch.kernel,
            dispatch_shape_sha256: old_shape,
            descriptors: descriptors.clone(),
        });
        // Metadata-only fixture: no native DATA lease is fabricated.
        backend.resident_data = Some(ResidentDataRosterV1 {
            descriptors: descriptors.clone(),
            data: Vec::new(),
        });
        let storage = backend.resident_data.as_ref().unwrap().descriptors.as_ptr();
        let usage = account.usage();
        let snapshot = launch
            .bindings
            .iter()
            .map(|binding| {
                backend.allocations[&binding.region.allocation]
                    .bytes
                    .clone()
            })
            .collect::<Vec<_>>();
        let result = backend.publish(123, 1, None, Arc::new(launch.clone()), prepared);
        assert!(
            matches!(result, Err(RuntimeBackendFailureV1::Rejected(ref error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
                && error.detail.contains("epoch storage preflight"))
        );
        assert!(!backend.terminal && backend.active.is_none() && backend.queue.is_none());
        assert!(backend.admitted_device.is_none() && backend.terminal_memory.is_none());
        assert_eq!(
            backend
                .recycled_dispatch
                .as_ref()
                .unwrap()
                .dispatch_shape_sha256,
            old_shape
        );
        assert_eq!(
            backend.recycled_dispatch.as_ref().unwrap().descriptors,
            descriptors
        );
        assert_eq!(
            backend.resident_data.as_ref().unwrap().descriptors.as_ptr(),
            storage
        );
        assert_eq!(account.usage(), usage);
        for (binding, bytes) in launch.bindings.iter().zip(snapshot) {
            assert_eq!(backend.allocations[&binding.region.allocation].bytes, bytes);
        }
        backend.resident_data = None;
        backend.recycled_dispatch = None;
        drop(backend);
        assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
    }
}

#[test]
fn scaled_attached_reuse_skips_reservation_but_shape_change_requires_credit() {
    let (mut backend, launch) = host_visible_three_binding_launch_v1();
    let account = account(2 * pipeline_bytes(), 2);
    install_scaled(&mut backend, account.clone());
    let prepared = backend
        .prepare_launch(launch.borrowed(), false, false)
        .unwrap();
    let PreparedLaunchStorageV1::Materialized(data) = &prepared.storage else {
        panic!("ordinary recipe");
    };
    let descriptors = resident_descriptors_v1(data).unwrap();
    let recycled = RecycledDispatchV1 {
        kernel: launch.kernel,
        dispatch_shape_sha256: prepared.dispatch_shape_sha256,
        descriptors: descriptors.clone(),
    };
    let usage = account.usage();
    let reuse = recycled_dispatch_reuse_is_admitted_v1(
        &recycled,
        prepared.dispatch_shape_sha256,
        &descriptors,
        data,
    );
    assert!(reuse);
    assert!(
        backend
            .preallocate_native_binding_v1(reuse)
            .unwrap()
            .is_none()
    );
    let mut other_shape = prepared.dispatch_shape_sha256;
    other_shape[0] ^= 1;
    let reuse = recycled_dispatch_reuse_is_admitted_v1(&recycled, other_shape, &descriptors, data);
    assert!(!reuse);
    rejected(
        backend.preallocate_native_binding_v1(reuse),
        KfdRuntimeBackendErrorKindV1::Capacity,
    );
    assert_eq!(account.usage(), usage);
    assert!(!backend.terminal);
}

#[test]
fn scaled_accepted_launch_preflight_failure_settles_failed_and_refunds_logical_retains() {
    let (mut backend, launch) = host_visible_three_binding_launch_v1();
    let account = account(2 * pipeline_bytes() + 3 * custody_bytes(), 5);
    install_scaled(&mut backend, account.clone());
    let before = account.usage();
    // CPU-only admission switch; exhausted credits must precede any KFD access.
    backend.native_available = true;
    let id = backend.submit_v1(launch.borrowed()).unwrap();
    assert_eq!(
        backend.poll_v1(id).unwrap(),
        BackendPollV1::Failed { code: -1 }
    );
    assert_eq!(
        backend.submissions[&id].status,
        BackendPollV1::Failed { code: -1 }
    );
    assert!(backend.pending_compute.is_empty());
    assert!(backend.pending_compute_streams.is_empty());
    assert!(backend.allocation_custody.is_empty());
    assert!(backend.compute_module_retain_counts.is_empty());
    assert!(backend.compute_dependency_retain_counts.is_empty());
    assert!(backend.stream_compute_lanes.is_empty());
    assert_eq!(backend.compute_completion_reservations, 0);
    assert_eq!(account.usage(), before);
    assert!(backend.active.is_none() && !backend.terminal && backend.queue.is_none());
    backend.release_submission_v1(id).unwrap();
    assert!(backend.submissions.is_empty());
    assert!(backend.stream_submission_tails.is_empty());
    backend.native_available = false;
    drop(backend);
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
}

#[test]
fn scaled_preallocation_routing_precedes_destructive_publication_steps() {
    let source = include_str!("../../compute_dispatch.rs");
    let publication = source
        .split_once("pub(super) fn publish(")
        .unwrap()
        .1
        .split_once("pub(super) fn observe_materialized_dispatch_published_v1(")
        .unwrap()
        .0;
    let reserve = publication
        .find("self.preallocate_native_binding_v1(reuse_attached)?")
        .unwrap();
    for action in [
        "self.detach_recycled_dispatch()?",
        "self.admitted_device.take()",
        "self.resident_data.take()",
    ] {
        assert!(reserve < publication.find(action).unwrap());
    }
    for method in [
        "create_compute_aql_queue_with_preallocated_fixed_dispatch_v1",
        "bind_initial_fixed_dispatch_with_preallocation_v1",
        "create_auxiliary_compute_lane_with_preallocated_fixed_dispatch_v1",
        "bind_fixed_dispatch_with_preallocation_v1",
    ] {
        assert!(publication.contains(method));
    }
}
