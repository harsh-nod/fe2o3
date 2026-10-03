//! Finite native DATA routing witness, with no generated dispatch publication.

use super::*;

fn install_multi(
    backend: &mut KfdMultiDeviceRuntimeBackendV1,
    uid: u64,
    stream: u64,
) -> GeneratedShellPlanV1 {
    let model = backend
        .with_retained_preparation_device_v1(uid, |device| device.model_admission())
        .unwrap();
    let (roster, program, buffers, packet) = native_fixture(false);
    let (binding, logical) =
        RuntimeContextV1::generated_native_test_ids_v1(uid, stream, model, buffers.len());
    let pending = backend
        .prepare_generated_shells_v1(binding, &roster, &logical)
        .unwrap();
    let global = *pending.plan();
    let bound = backend
        .bind_generated_shell_requests_v1(pending, core::array::from_fn(|_| None))
        .unwrap();
    // The exact admitted packet bypasses carrier registration only. Both route
    // installation layers are the same cores used by production commits.
    backend.commit_generated_route_v1(bound, |child, authenticated| {
        child.commit_generated_shell_control_v1(authenticated, &roster, |control| {
            assert!(control.is_none());
            *control = Some(packet);
            true
        });
    });
    backend
        .adopt_generated_data_v1(&global, &roster, program, &buffers)
        .unwrap();
    assert!(backend.validate_generated_shell_records_v1(&global));
    let child = backend.child_for_device(uid).unwrap();
    assert_eq!(backend.children[child].generated_shells.len(), 1);
    let record = backend.children[child]
        .generated_shells
        .values()
        .next()
        .unwrap();
    assert_eq!(record.native.as_ref().unwrap().phase, PhaseV1::Adopted);
    assert!(record.control.is_none());
    assert!(record.native.as_ref().unwrap().submission.is_none());
    global
}

fn assert_unpublished(backend: &KfdMultiDeviceRuntimeBackendV1) {
    assert!(backend.submissions.is_empty());
    assert!(backend.compute_xgmi_children.iter().all(Option::is_none));
    for child in &backend.children {
        assert!(child.submissions.is_empty());
        assert!(child.generated_submissions.is_empty());
        assert!(child.pending_compute.is_empty());
        assert!(!child.any_compute_active_v1());
        assert_eq!(child.compute_completion_reservations, 0);
        assert_eq!(child.sdma_completion_reservations, 0);
    }
}

fn retire_multi(backend: &mut KfdMultiDeviceRuntimeBackendV1, plan: &GeneratedShellPlanV1) {
    backend.retire_generated_data_v1(plan).unwrap();
    let child = backend
        .child_for_device(plan.binding.backend_device)
        .unwrap();
    let record = backend.children[child]
        .generated_shells
        .values()
        .next()
        .unwrap();
    assert_eq!(
        record.native.as_ref().unwrap().returned.completed,
        plan.count
    );
    assert!(backend.validate_generated_shell_disposal_v1(plan));
    backend.dispose_generated_shells_v1(plan);
    assert!(backend.children[child].generated_shells.is_empty());
    assert!(backend.children[child].allocations.is_empty());
    assert!(backend.children[child].stream_compute_lanes.is_empty());
}

#[test]
#[ignore = "requires an idle MI300X pair, FE2O3_TEST_NATIVE_ISOLATED=1 and FE2O3_TEST_NATIVE_UNIQUE_IDS"]
fn generated_native_multi_data_adopt_rebind_retire_without_publication() {
    assert_eq!(
        std::env::var("FE2O3_TEST_NATIVE_ISOLATED").as_deref(),
        Ok("1")
    );
    let raw = std::env::var("FE2O3_TEST_NATIVE_UNIQUE_IDS").expect("two explicit device UIDs");
    let uids: Vec<_> = raw
        .split(',')
        .map(|uid| u64::from_str_radix(uid.strip_prefix("0x").unwrap_or(uid), 16).unwrap())
        .collect();
    let [first, second] = <[u64; 2]>::try_from(uids).expect("exactly two devices");
    assert_ne!(first, second);
    let children = [first, second]
        .map(|uid| KfdRuntimeBackendV1::open_gfx942_vecadd_qualification_v1(uid).unwrap());
    // Failed assertions must never destroy retained native custody.
    let mut backend = std::mem::ManuallyDrop::new(
        KfdMultiDeviceRuntimeBackendV1::from_backends(Vec::from(children)).unwrap(),
    );
    let streams = [first, second].map(|uid| backend.create_stream_v1(uid).unwrap());
    let a = install_multi(&mut backend, first, streams[0]);
    let b = install_multi(&mut backend, second, streams[1]);
    assert_ne!(a.key, b.key);
    assert!(backend.allocations.is_empty());
    assert_eq!(backend.generated_allocations.len(), 6);
    let primary = backend.children[0].native_compute_lanes[0].unwrap();
    assert_unpublished(&backend);
    retire_multi(&mut backend, &a);
    assert!(backend.validate_generated_shell_records_v1(&b));
    let rebound = install_multi(&mut backend, first, streams[0]);
    assert_eq!(backend.children[0].native_compute_lanes[0], Some(primary));
    assert!(backend.validate_generated_shell_records_v1(&b));
    assert_unpublished(&backend);
    retire_multi(&mut backend, &b);
    retire_multi(&mut backend, &rebound);
    assert!(backend.generated_shells.is_empty() && backend.generated_allocations.is_empty());
    for stream in streams {
        backend.destroy_stream_v1(stream).unwrap();
    }
    backend.shutdown_native_v1().unwrap();
    assert_unpublished(&backend);
    assert!(backend.children.iter().all(|child| child.queue.is_none()));
    drop(std::mem::ManuallyDrop::into_inner(backend));
    eprintln!(
        "MULTI DATA PASS: two exact devices, independent retirement, primary rebound, 9 DATA releases, zero publication, shutdown complete"
    );
}
