use super::*;

#[test]
fn persistent_compute_poll_and_wait_share_one_completion_handler_without_poll_waiting() {
    let compute_dispatch = include_str!("../compute_dispatch/polling.rs");
    let completion = include_str!("../persistent_completion.rs");
    let three_completion = include_str!("../three_binding_completion.rs");
    let poll = compute_dispatch
        .split("fn poll_compute_lane_v1")
        .nth(1)
        .unwrap()
        .split("fn wait_published_persistent_compute_lane_v1")
        .next()
        .unwrap();
    let wait = compute_dispatch
        .split("fn wait_published_persistent_compute_lane_v1")
        .nth(1)
        .unwrap()
        .split("fn poll_retained_pending_dependency_v1")
        .next()
        .unwrap();
    assert_eq!(
        completion
            .matches(".poll_and_recycle_directional_persistent_fixed_dispatch_v1(dispatch)")
            .count(),
        1
    );
    assert_eq!(
        completion
            .matches(".wait_and_recycle_directional_persistent_fixed_dispatch_until_v1(")
            .count(),
        1
    );
    assert_eq!(
        three_completion
            .matches(
                ".wait_and_recycle_three_binding_directional_persistent_fixed_dispatch_until_v1("
            )
            .count(),
        1
    );
    assert_eq!(
        poll.matches("advance_scalar_completion_v1(None)").count(),
        1
    );
    assert_eq!(
        wait.matches("advance_scalar_completion_v1(Some(deadline))")
            .count(),
        1
    );
    assert_eq!(
        wait.matches("advance_three_completion_v1(Some(deadline))")
            .count(),
        1
    );
    assert_eq!(poll.matches("advance_three_completion_v1(None)").count(), 1);
    assert_eq!(
        three_completion
            .matches(".poll_and_recycle_three_binding_directional_persistent_fixed_dispatch_v1(")
            .count(),
        1
    );
    assert!(
        !poll.contains(
            "wait_and_recycle_three_binding_directional_persistent_fixed_dispatch_until_v1"
        )
    );
    assert!(!poll.contains("wait_and_recycle_directional_persistent_fixed_dispatch_until_v1"));

    let runtime_wait = include_str!("../../kfd_backend.rs")
        .split("fn wait_v1(")
        .nth(1)
        .unwrap()
        .split("fn release_submission_v1")
        .next()
        .unwrap();
    assert_eq!(
        runtime_wait
            .matches("published_persistent_compute_lane_v1(submission)")
            .count(),
        1
    );
    assert_eq!(
        runtime_wait
            .matches("wait_published_persistent_compute_lane_v1(lane, deadline)")
            .count(),
        1
    );
}

#[test]
fn scripted_persistent_poll_retry_is_terminal_with_exact_custody() {
    assert_scripted_persistent_transition_retry_is_terminal_v1(
        ScriptedPersistentTransitionFailureV1::Poll,
        false,
    );
}

#[test]
fn scripted_persistent_recycle_retry_is_terminal_with_exact_custody() {
    assert_scripted_persistent_transition_retry_is_terminal_v1(
        ScriptedPersistentTransitionFailureV1::Recycle,
        false,
    );
}

#[test]
fn scripted_persistent_detach_retry_is_terminal_with_exact_custody() {
    assert_scripted_persistent_transition_retry_is_terminal_v1(
        ScriptedPersistentTransitionFailureV1::Detach,
        false,
    );
}

#[test]
fn scripted_persistent_wait_preserves_all_terminal_failure_stage_custody() {
    for stage in [
        ScriptedPersistentTransitionFailureV1::Poll,
        ScriptedPersistentTransitionFailureV1::Recycle,
        ScriptedPersistentTransitionFailureV1::Detach,
    ] {
        assert_scripted_persistent_transition_retry_is_terminal_v1(stage, true);
    }
}

#[test]
fn scripted_persistent_compute_completion_mismatch_quarantines_device() {
    let byte_len = usize::try_from(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1).unwrap();
    let steps = [
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len,
        },
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            u32::try_from(byte_len).unwrap(),
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
    ];
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(byte_len, steps);
    let expected = vec![0x3c_u8; byte_len];
    backend.write_allocation_v1(host, 0, &expected).unwrap();
    let (source, destination) =
        scripted_copy_regions_v1(host, device, u64::try_from(byte_len).unwrap());
    let copy = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
    let ready = backend.take_h2d_ready_for_compute_v1(device, 201).unwrap();
    let failure = backend
        .restore_persistent_compute_completion_v1(
            device,
            202,
            ready.owner.normalize(),
            Gfx942PersistentComputeEffectV1::Read,
        )
        .unwrap_err();
    assert!(matches!(failure, RuntimeBackendFailureV1::Terminal(_)));
    assert!(backend.terminal);
    assert!(backend.terminal_sdma_custody.is_some());
    assert!(matches!(
        backend.allocations[&device].sdma_storage,
        KfdRuntimeSdmaStorageV1::ComputeInFlight(201)
    ));
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
        2
    );
    let _ = (stream, host, copy);
    disarm_scripted_drop_after_inspection_v1(&mut backend);
}

#[test]
fn persistent_waiter_observes_busy_primary_then_flush_publishes() {
    let byte_len = usize::try_from(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1).unwrap();
    let mut steps = Vec::new();
    for _ in 0..2 {
        steps.extend([
            ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len,
            },
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                u32::try_from(byte_len).unwrap(),
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
        ]);
    }
    steps.extend(scripted_release_steps_v1());
    steps.extend(scripted_release_steps_v1());
    let (mut backend, first_stream, first_host, first_device) =
        scripted_direct_backend_v1(byte_len, steps);
    let second_stream = backend.create_stream_v1(7).unwrap();
    let (second_host, second_device) = add_scripted_direct_pair_v1(&mut backend, byte_len);
    let module = backend
        .load_module_v1(7, &synthetic_cov6::module())
        .unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();
    let mut copies = Vec::new();
    for (stream, host, device, fill) in [
        (first_stream, first_host, first_device, 0x31_u8),
        (second_stream, second_host, second_device, 0x42_u8),
    ] {
        backend
            .write_allocation_v1(host, 0, &vec![fill; byte_len])
            .unwrap();
        let (source, destination) =
            scripted_copy_regions_v1(host, device, u64::try_from(byte_len).unwrap());
        let copy = backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();
        assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
        copies.push(copy);
    }

    let first_compute = submit_scripted_read_v1(
        &mut backend,
        first_stream,
        kernel,
        first_device,
        u64::try_from(byte_len).unwrap(),
        &[],
    );
    backend.flush_stream_v1(first_stream).unwrap();
    assert_eq!(
        backend.active.as_ref().map(|active| active.id),
        Some(first_compute)
    );
    assert_eq!(backend.free_compute_lane_v1(), Some(1));
    let second_compute = submit_scripted_read_v1(
        &mut backend,
        second_stream,
        kernel,
        second_device,
        u64::try_from(byte_len).unwrap(),
        &[],
    );
    assert!(backend.pending_compute.contains_key(&second_compute));
    assert!(matches!(
        backend.allocations[&second_device].sdma_storage,
        KfdRuntimeSdmaStorageV1::H2dReady(_)
    ));

    assert_eq!(
        backend.poll_v1(second_compute).unwrap(),
        BackendPollV1::Pending
    );
    assert_eq!(
        backend.submissions[&first_compute].status,
        BackendPollV1::Succeeded
    );
    assert!(backend.pending_compute.contains_key(&second_compute));
    assert!(backend.active.is_none());
    assert!(matches!(
        backend.allocations[&second_device].sdma_storage,
        KfdRuntimeSdmaStorageV1::H2dReady(_)
    ));

    backend.flush_stream_v1(second_stream).unwrap();
    assert_eq!(
        backend.active.as_ref().map(|active| active.id),
        Some(second_compute)
    );
    assert!(!backend.pending_compute.contains_key(&second_compute));
    assert!(matches!(
        backend.allocations[&second_device].sdma_storage,
        KfdRuntimeSdmaStorageV1::ComputeInFlight(actual) if actual == second_compute
    ));
    assert_eq!(
        backend.poll_v1(second_compute).unwrap(),
        BackendPollV1::Succeeded
    );

    for submission in copies.into_iter().chain([first_compute, second_compute]) {
        backend.release_submission_v1(submission).unwrap();
    }
    release_scripted_direct_pair_v1(&mut backend, first_host, first_device);
    release_scripted_direct_pair_v1(&mut backend, second_host, second_device);
    backend.unload_module_v1(module).unwrap();
    backend.destroy_stream_v1(first_stream).unwrap();
    backend.destroy_stream_v1(second_stream).unwrap();
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 0);
    assert_eq!(driver.unexpected_drops(), 0);
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn nonpersistent_compute_waits_for_active_persistent_compute() {
    let byte_len = usize::try_from(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1).unwrap();
    let mut steps = vec![
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len,
        },
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            u32::try_from(byte_len).unwrap(),
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len,
        },
    ];
    steps.extend(scripted_release_steps_v1());
    steps.extend(scripted_release_steps_v1());
    let (mut backend, persistent_stream, persistent_host, persistent_device) =
        scripted_direct_backend_v1(byte_len, steps);
    let ordinary_stream = backend.create_stream_v1(7).unwrap();
    let (ordinary_host, ordinary_device) = add_scripted_direct_pair_v1(&mut backend, byte_len);
    let module = backend
        .load_module_v1(7, &synthetic_cov6::module())
        .unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();

    backend
        .write_allocation_v1(persistent_host, 0, &vec![0x35; byte_len])
        .unwrap();
    let (source, destination) = scripted_copy_regions_v1(
        persistent_host,
        persistent_device,
        u64::try_from(byte_len).unwrap(),
    );
    let copy = backend
        .copy_async_v1(persistent_stream, source, destination, &[])
        .unwrap();
    assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
    let dependency = backend.record_event_v1(persistent_stream, copy).unwrap();
    backend
        .write_allocation_v1(ordinary_host, 0, &vec![0x46; byte_len])
        .unwrap();

    let persistent = submit_scripted_read_v1(
        &mut backend,
        persistent_stream,
        kernel,
        persistent_device,
        u64::try_from(byte_len).unwrap(),
        &[],
    );
    backend.flush_stream_v1(persistent_stream).unwrap();
    assert!(backend.persistent_compute_is_active_v1());
    assert_eq!(backend.free_compute_lane_v1(), Some(1));

    let leases = backend.stream_compute_lanes.clone();
    for _ in 0..3 {
        assert!(!backend.generated_lane_ready_v1().unwrap());
        assert_eq!(backend.active.as_ref().unwrap().id, persistent);
        assert_eq!(backend.stream_compute_lanes, leases);
        assert!(!backend.terminal);
    }

    let ordinary = submit_scripted_read_v1(
        &mut backend,
        ordinary_stream,
        kernel,
        ordinary_host,
        u64::try_from(byte_len).unwrap(),
        &[dependency],
    );
    assert!(backend.pending_compute.contains_key(&ordinary));
    assert!(backend.auxiliary_compute_lanes[0].active.is_none());
    assert!(backend.persistent_compute_is_active_v1());

    assert_eq!(backend.poll_v1(ordinary).unwrap(), BackendPollV1::Pending);
    assert_eq!(
        backend.submissions[&persistent].status,
        BackendPollV1::Succeeded
    );
    assert!(backend.pending_compute.contains_key(&ordinary));
    assert!(backend.active.is_none());
    assert!(backend.auxiliary_compute_lanes[0].active.is_none());
    assert!(backend.generated_lane_ready_v1().unwrap());

    backend.flush_stream_v1(ordinary_stream).unwrap();
    assert_eq!(
        backend.active.as_ref().map(|active| active.id),
        Some(ordinary)
    );
    assert!(!backend.pending_compute.contains_key(&ordinary));
    assert_eq!(backend.poll_v1(ordinary).unwrap(), BackendPollV1::Succeeded);

    backend.release_event_v1(dependency).unwrap();
    for submission in [copy, persistent, ordinary] {
        backend.release_submission_v1(submission).unwrap();
    }
    release_scripted_direct_pair_v1(&mut backend, persistent_host, persistent_device);
    release_scripted_direct_pair_v1(&mut backend, ordinary_host, ordinary_device);
    backend.unload_module_v1(module).unwrap();
    backend.destroy_stream_v1(persistent_stream).unwrap();
    backend.destroy_stream_v1(ordinary_stream).unwrap();
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 0);
    assert_eq!(driver.unexpected_drops(), 0);
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn persistent_compute_waits_for_published_auxiliary_compute() {
    let byte_len = usize::try_from(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1).unwrap();
    let mut steps = vec![
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len,
        },
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            u32::try_from(byte_len).unwrap(),
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len,
        },
    ];
    steps.extend(scripted_release_steps_v1());
    steps.extend(scripted_release_steps_v1());
    let (mut backend, persistent_stream, persistent_host, persistent_device) =
        scripted_direct_backend_v1(byte_len, steps);
    let ordinary_stream = backend.create_stream_v1(7).unwrap();
    let lane_zero_reservation_stream = backend.create_stream_v1(7).unwrap();
    let (ordinary_host, ordinary_device) = add_scripted_direct_pair_v1(&mut backend, byte_len);
    let module = backend
        .load_module_v1(7, &synthetic_cov6::module())
        .unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();

    backend
        .write_allocation_v1(persistent_host, 0, &vec![0x57; byte_len])
        .unwrap();
    let (source, destination) = scripted_copy_regions_v1(
        persistent_host,
        persistent_device,
        u64::try_from(byte_len).unwrap(),
    );
    let copy = backend
        .copy_async_v1(persistent_stream, source, destination, &[])
        .unwrap();
    assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
    backend
        .write_allocation_v1(ordinary_host, 0, &vec![0x68; byte_len])
        .unwrap();

    backend.lease_compute_lane_v1(lane_zero_reservation_stream, 0);
    let ordinary = submit_scripted_read_v1(
        &mut backend,
        ordinary_stream,
        kernel,
        ordinary_host,
        u64::try_from(byte_len).unwrap(),
        &[],
    );
    assert_eq!(
        backend.auxiliary_compute_lanes[0]
            .active
            .as_ref()
            .map(|active| active.id),
        Some(ordinary)
    );
    assert!(backend.active.is_none());
    backend.release_compute_lane_lease_v1(lane_zero_reservation_stream, 0);
    assert_eq!(backend.free_compute_lane_v1(), Some(0));

    let persistent = submit_scripted_read_v1(
        &mut backend,
        persistent_stream,
        kernel,
        persistent_device,
        u64::try_from(byte_len).unwrap(),
        &[],
    );
    assert!(backend.pending_compute.contains_key(&persistent));
    assert!(backend.active.is_none());
    assert!(matches!(
        backend.allocations[&persistent_device].sdma_storage,
        KfdRuntimeSdmaStorageV1::H2dReady(_)
    ));

    assert_eq!(backend.poll_v1(persistent).unwrap(), BackendPollV1::Pending);
    assert_eq!(
        backend.submissions[&ordinary].status,
        BackendPollV1::Succeeded
    );
    assert!(backend.pending_compute.contains_key(&persistent));
    assert!(backend.active.is_none());
    assert!(backend.auxiliary_compute_lanes[0].active.is_none());
    assert!(matches!(
        backend.allocations[&persistent_device].sdma_storage,
        KfdRuntimeSdmaStorageV1::H2dReady(_)
    ));

    backend.flush_stream_v1(persistent_stream).unwrap();
    assert_eq!(
        backend.active.as_ref().map(|active| active.id),
        Some(persistent)
    );
    assert_eq!(
        backend.poll_v1(persistent).unwrap(),
        BackendPollV1::Succeeded
    );

    for submission in [copy, ordinary, persistent] {
        backend.release_submission_v1(submission).unwrap();
    }
    release_scripted_direct_pair_v1(&mut backend, persistent_host, persistent_device);
    release_scripted_direct_pair_v1(&mut backend, ordinary_host, ordinary_device);
    backend.unload_module_v1(module).unwrap();
    backend.destroy_stream_v1(persistent_stream).unwrap();
    backend.destroy_stream_v1(ordinary_stream).unwrap();
    backend
        .destroy_stream_v1(lane_zero_reservation_stream)
        .unwrap();
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 0);
    assert_eq!(driver.unexpected_drops(), 0);
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn persistent_compute_binds_after_auxiliary_cache_release_and_module_unload() {
    let byte_len = usize::try_from(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1).unwrap();
    let mut steps = vec![
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len,
        },
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            u32::try_from(byte_len).unwrap(),
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len,
        },
    ];
    steps.extend(scripted_release_steps_v1());
    steps.extend(scripted_release_steps_v1());
    let (mut backend, persistent_stream, persistent_host, persistent_device) =
        scripted_direct_backend_v1(byte_len, steps);
    let ordinary_stream = backend.create_stream_v1(7).unwrap();
    let lane_zero_reservation_stream = backend.create_stream_v1(7).unwrap();
    let (ordinary_host, ordinary_device) = add_scripted_direct_pair_v1(&mut backend, byte_len);
    let first_module = backend
        .load_module_v1(7, &synthetic_cov6::module())
        .unwrap();
    let first_kernel = backend
        .resolve_kernel_v1(first_module, "vecadd", [7; 32])
        .unwrap();

    backend
        .write_allocation_v1(persistent_host, 0, &vec![0x79; byte_len])
        .unwrap();
    let (source, destination) = scripted_copy_regions_v1(
        persistent_host,
        persistent_device,
        u64::try_from(byte_len).unwrap(),
    );
    let copy = backend
        .copy_async_v1(persistent_stream, source, destination, &[])
        .unwrap();
    assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
    backend
        .write_allocation_v1(ordinary_host, 0, &vec![0x8a; byte_len])
        .unwrap();

    backend.lease_compute_lane_v1(lane_zero_reservation_stream, 0);
    let ordinary = submit_scripted_read_v1(
        &mut backend,
        ordinary_stream,
        first_kernel,
        ordinary_host,
        u64::try_from(byte_len).unwrap(),
        &[],
    );
    backend.release_compute_lane_lease_v1(lane_zero_reservation_stream, 0);
    assert_eq!(
        backend.auxiliary_compute_lanes[0]
            .active
            .as_ref()
            .map(|active| active.id),
        Some(ordinary)
    );
    assert_eq!(backend.poll_v1(ordinary).unwrap(), BackendPollV1::Succeeded);
    assert!(backend.auxiliary_compute_lanes[0].active.is_none());
    assert!(
        backend.auxiliary_compute_lanes[0]
            .recycled_dispatch
            .is_none()
    );
    assert!(backend.auxiliary_compute_lanes[0].resident_data.is_none());
    backend.release_compute_lane_cache_v1(1).unwrap();
    assert_eq!(backend.free_compute_lane_v1(), Some(0));
    backend.release_submission_v1(ordinary).unwrap();
    backend.unload_module_v1(first_module).unwrap();

    let second_module = backend
        .load_module_v1(7, &synthetic_cov6::module())
        .unwrap();
    let second_kernel = backend
        .resolve_kernel_v1(second_module, "vecadd", [7; 32])
        .unwrap();
    let persistent = submit_scripted_read_v1(
        &mut backend,
        persistent_stream,
        second_kernel,
        persistent_device,
        u64::try_from(byte_len).unwrap(),
        &[],
    );
    backend.flush_stream_v1(persistent_stream).unwrap();
    assert_eq!(
        backend.active.as_ref().map(|active| active.id),
        Some(persistent)
    );
    assert!(matches!(
        backend.allocations[&persistent_device].sdma_storage,
        KfdRuntimeSdmaStorageV1::ComputeInFlight(actual) if actual == persistent
    ));
    assert_eq!(
        backend.poll_v1(persistent).unwrap(),
        BackendPollV1::Succeeded
    );
    let performance = backend.last_launch_performance_v1().unwrap();
    assert_eq!(
        performance.data_path(),
        KfdRuntimeLaunchDataPathV1::PersistentDeviceReused
    );
    assert_eq!(performance.user_data_materializations(), 0);

    for submission in [copy, persistent] {
        backend.release_submission_v1(submission).unwrap();
    }
    release_scripted_direct_pair_v1(&mut backend, persistent_host, persistent_device);
    release_scripted_direct_pair_v1(&mut backend, ordinary_host, ordinary_device);
    backend.unload_module_v1(second_module).unwrap();
    backend.destroy_stream_v1(persistent_stream).unwrap();
    backend.destroy_stream_v1(ordinary_stream).unwrap();
    backend
        .destroy_stream_v1(lane_zero_reservation_stream)
        .unwrap();
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 0);
    assert_eq!(driver.unexpected_drops(), 0);
    backend.shutdown_native_v1().unwrap();
}
