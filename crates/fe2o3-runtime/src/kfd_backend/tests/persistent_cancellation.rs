use super::*;

#[test]
fn scripted_persistent_prepared_cancel_restores_ready_custody_without_publication() {
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
    ];
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) =
        scripted_direct_backend_configured_v1(byte_len, steps, |backend| {
            backend
                .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([0xd2; 32], 128).unwrap())
                .unwrap();
        });
    let expected = vec![0xd2; byte_len];
    backend.write_allocation_v1(host, 0, &expected).unwrap();
    let (source, destination) =
        scripted_copy_regions_v1(host, device, u64::try_from(byte_len).unwrap());
    let copy = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
    let ready = backend.allocations[&device]
        .sdma_storage
        .persistent_compute_ready_facts_v1()
        .unwrap();
    let promotion = backend.last_ready_promotion_performance_v1().unwrap();
    let expected_sha256: [u8; 32] = Sha256::digest(&expected).into();
    assert_eq!(ready.authenticated_sha256, expected_sha256);

    let module = backend
        .load_module_v1(7, &synthetic_cov6::module())
        .unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();
    backend.scripted_persistent_publication_retries = 1;
    let compute = submit_scripted_read_v1(
        &mut backend,
        stream,
        kernel,
        device,
        u64::try_from(byte_len).unwrap(),
        &[],
    );
    let event = backend.record_event_v1(stream, compute).unwrap();
    backend.flush_stream_v1(stream).unwrap();
    let active = backend.active.as_ref().unwrap();
    assert_eq!(active.id, compute);
    assert_eq!(active.ordered_predecessor, Some(copy));
    assert!(matches!(
        active.execution,
        Some(ActiveComputeExecutionV1::ScriptedPersistentPrepared { .. })
    ));
    assert_eq!(active.performance.ready_promotion(), Some(promotion));
    assert_eq!(backend.compute_completion_reservations, 1);
    assert_eq!(backend.stream_compute_lanes.get(&stream), Some(&0));
    assert_eq!(backend.last_launch_performance_v1(), None);

    assert_eq!(
        backend.cancel_v1(compute).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert!(backend.active.is_none());
    assert_eq!(backend.compute_completion_reservations, 0);
    assert!(backend.allocation_custody.is_empty());
    assert!(backend.compute_module_retain_counts.is_empty());
    assert!(backend.compute_dependency_retain_counts.is_empty());
    assert!(!backend.stream_compute_lanes.contains_key(&stream));
    assert!(!backend.stream_submission_tails.contains_key(&stream));
    assert_eq!(
        backend.allocations[&device]
            .sdma_storage
            .persistent_compute_ready_facts_v1(),
        Some(ready)
    );
    assert_eq!(
        backend.allocations[&device]
            .sdma_storage
            .ready_promotion_performance_v1(),
        Some(promotion)
    );
    assert_eq!(
        backend.allocations[&device].content_sha256,
        Some(expected_sha256)
    );
    assert_eq!(backend.allocations[&device].bytes.as_ref(), expected);
    assert!(backend.allocations[&device].native_dirty.is_empty());
    assert!(!backend.allocations[&device].sdma_shadow_dirty);
    assert_eq!(backend.events[&event].submission, compute);
    assert_eq!(
        backend.poll_v1(compute).unwrap(),
        BackendPollV1::Failed { code: -2 }
    );

    backend.release_event_v1(event).unwrap();
    for submission in [copy, compute] {
        backend.release_submission_v1(submission).unwrap();
    }
    backend.unload_module_v1(module).unwrap();
    release_scripted_direct_pair_v1(&mut backend, host, device);
    backend.destroy_stream_v1(stream).unwrap();
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 0);
    assert_eq!(driver.unexpected_drops(), 0);
    backend.shutdown_native_v1().unwrap();
    let capture = backend.finish_profiler_v1().unwrap();
    capture.validate().unwrap();
    assert!(capture.coverage.complete_runtime_operation_history);
    assert_eq!(capture.coverage.dropped_events, 0);
    assert_eq!(
        capture
            .events
            .iter()
            .filter(|event| matches!(
                event.event,
                KfdRuntimeProfileEventKindV1::DispatchPublished { .. }
            ))
            .count(),
        0
    );
    assert_eq!(
        capture
            .events
            .iter()
            .filter(|event| matches!(
                event.event,
                KfdRuntimeProfileEventKindV1::DispatchCompleted { .. }
            ))
            .count(),
        0
    );
}

#[test]
fn scripted_initial_persistent_cancel_allows_materialized_retry() {
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
    ];
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(byte_len, steps);
    backend
        .write_allocation_v1(host, 0, &vec![0x91; byte_len])
        .unwrap();
    let (source, destination) =
        scripted_copy_regions_v1(host, device, u64::try_from(byte_len).unwrap());
    let copy = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
    let module = backend
        .load_module_v1(7, &synthetic_cov6::module())
        .unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();

    backend.scripted_persistent_publication_retries = 1;
    let cancelled = submit_scripted_read_v1(
        &mut backend,
        stream,
        kernel,
        device,
        u64::try_from(byte_len).unwrap(),
        &[],
    );
    backend.flush_stream_v1(stream).unwrap();
    assert!(matches!(
        backend
            .active
            .as_ref()
            .and_then(|active| active.execution.as_ref()),
        Some(ActiveComputeExecutionV1::ScriptedPersistentPrepared { .. })
    ));
    assert_eq!(
        backend.cancel_v1(cancelled).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert!(backend.retained_persistent_dispatch.is_none());

    let mut explicit_kernarg = [0_u8; 16];
    explicit_kernarg[8..].copy_from_slice(&13_u64.to_le_bytes());
    let retry = backend
        .submit_v1(BackendLaunchV1 {
            stream,
            kernel,
            explicit_kernarg: &explicit_kernarg,
            bindings: &[BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation: device,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: u64::try_from(byte_len / 2).unwrap(),
                },
                kernarg_byte_offset: 0,
            }],
            dependencies: &[],
            geometry: crate::RuntimeLaunchGeometryV1 {
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_shared_bytes: 0,
            },
            semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
        })
        .unwrap();
    backend.flush_stream_v1(stream).unwrap();
    let mut retry_status = BackendPollV1::Pending;
    for _ in 0..4 {
        retry_status = backend.poll_v1(retry).unwrap();
        if retry_status != BackendPollV1::Pending {
            break;
        }
    }
    assert_eq!(retry_status, BackendPollV1::Succeeded);
    let performance = backend.last_launch_performance_v1().unwrap();
    assert_eq!(
        performance.data_path(),
        KfdRuntimeLaunchDataPathV1::Materialized
    );
    assert!(!performance.persistent_control_reused());

    for submission in [copy, cancelled, retry] {
        backend.release_submission_v1(submission).unwrap();
    }
    backend.unload_module_v1(module).unwrap();
    release_scripted_direct_pair_v1(&mut backend, host, device);
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn scripted_persistent_prepared_cancel_does_not_restore_a_released_stream_tail() {
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
    ];
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(byte_len, steps);
    backend
        .write_allocation_v1(host, 0, &vec![0xe3; byte_len])
        .unwrap();
    let (source, destination) =
        scripted_copy_regions_v1(host, device, u64::try_from(byte_len).unwrap());
    let copy = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
    let module = backend
        .load_module_v1(7, &synthetic_cov6::module())
        .unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();

    backend.scripted_persistent_publication_retries = 1;
    let cancelled = submit_scripted_read_v1(
        &mut backend,
        stream,
        kernel,
        device,
        u64::try_from(byte_len).unwrap(),
        &[],
    );
    backend.flush_stream_v1(stream).unwrap();
    assert!(matches!(
        backend
            .active
            .as_ref()
            .and_then(|active| active.execution.as_ref()),
        Some(ActiveComputeExecutionV1::ScriptedPersistentPrepared { .. })
    ));
    assert!(!backend.compute_dependency_retain_counts.contains_key(&copy));
    backend.release_submission_v1(copy).unwrap();
    assert_eq!(
        backend.stream_submission_tails.get(&stream),
        Some(&cancelled)
    );
    assert_eq!(
        backend.cancel_v1(cancelled).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert!(!backend.stream_submission_tails.contains_key(&stream));

    let replacement = submit_scripted_read_v1(
        &mut backend,
        stream,
        kernel,
        device,
        u64::try_from(byte_len).unwrap(),
        &[],
    );
    assert_eq!(
        backend.poll_v1(replacement).unwrap(),
        BackendPollV1::Succeeded
    );
    for submission in [cancelled, replacement] {
        backend.release_submission_v1(submission).unwrap();
    }
    backend.unload_module_v1(module).unwrap();
    release_scripted_direct_pair_v1(&mut backend, host, device);
    backend.destroy_stream_v1(stream).unwrap();
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 0);
    assert_eq!(driver.unexpected_drops(), 0);
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn profiled_pending_compute_cancel_has_no_dispatch_lifecycle() {
    let byte_len = HOST_VISIBLE_MEMORY_PAGE_BYTES_V1;
    let mut backend = KfdRuntimeBackendV1::mock();
    backend
        .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([0xf4; 32], 64).unwrap())
        .unwrap();
    let stream = backend.create_stream_v1(7).unwrap();
    let allocation = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, byte_len, 8)
        .unwrap();
    let module = backend
        .load_module_v1(7, &synthetic_cov6::module())
        .unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();
    let predecessor = backend.next_id().unwrap();
    let mut pending = pending_compute_for_test_v1(predecessor, stream, allocation, vec![]);
    pending.module = module;
    let mut recipe = pending.launch.unaccounted_copy_for_test();
    recipe.kernel = kernel;
    pending.launch = Arc::new(RetainedComputeLaunchV1::unaccounted_for_test(recipe));
    backend.pending_compute.insert(predecessor, pending);
    backend
        .pending_compute_streams
        .insert(stream, VecDeque::from([predecessor]));
    backend.compute_completion_reservations = 1;
    index_pending_compute_custody_for_test_v1(&mut backend, predecessor);
    backend.stream_submission_tails.insert(stream, predecessor);
    backend.native_available = true;
    let record = backend.allocations.get_mut(&allocation).unwrap();
    record.sdma_backed = true;
    record.sdma_initialized = true;

    let cancelled =
        submit_scripted_read_v1(&mut backend, stream, kernel, allocation, byte_len, &[]);
    assert!(backend.pending_compute.contains_key(&cancelled));
    assert_eq!(
        backend.cancel_v1(cancelled).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert_eq!(
        backend.poll_v1(cancelled).unwrap(),
        BackendPollV1::Failed { code: -2 }
    );
    assert_eq!(
        backend.stream_submission_tails.get(&stream),
        Some(&predecessor)
    );

    assert_eq!(
        backend.cancel_v1(predecessor).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    for submission in [cancelled, predecessor] {
        backend.release_submission_v1(submission).unwrap();
    }
    backend.unload_module_v1(module).unwrap();
    backend
        .allocations
        .get_mut(&allocation)
        .unwrap()
        .sdma_backed = false;
    backend.release_allocation_v1(allocation).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
    let capture = backend.finish_profiler_v1().unwrap();
    capture.validate().unwrap();
    assert!(!capture.events.iter().any(|event| matches!(
        event.event,
        KfdRuntimeProfileEventKindV1::DispatchPublished { .. }
            | KfdRuntimeProfileEventKindV1::DispatchCompleted { .. }
            | KfdRuntimeProfileEventKindV1::SubmissionReleased { .. }
    )));
}

#[test]
fn scripted_persistent_wait_zero_deadline_restores_exact_lane_and_retains_for_retry() {
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
    ];
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(byte_len, steps);
    backend
        .write_allocation_v1(host, 0, &vec![0xc8; byte_len])
        .unwrap();
    let (source, destination) =
        scripted_copy_regions_v1(host, device, u64::try_from(byte_len).unwrap());
    let copy = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
    let dependency_event = backend.record_event_v1(stream, copy).unwrap();
    let module = backend
        .load_module_v1(7, &synthetic_cov6::module())
        .unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();

    backend.scripted_persistent_wait_pending_observations = 1;
    let compute = submit_scripted_read_v1(
        &mut backend,
        stream,
        kernel,
        device,
        u64::try_from(byte_len).unwrap(),
        &[dependency_event],
    );
    backend.flush_stream_v1(stream).unwrap();
    let lane = backend.active_compute_lane_v1(compute).unwrap();
    let active = backend.active_compute_submission_v1(compute).unwrap();
    let active_identity = (
        active.id,
        active.stream,
        active.ordered_predecessor,
        active.kernel,
        active.dependency_depth,
        active.allocations.clone(),
        active.dispatch_shape_sha256,
    );
    assert!(matches!(
        active.execution,
        Some(ActiveComputeExecutionV1::ScriptedPersistent { .. })
    ));
    let lane_index = backend.stream_compute_lanes.clone();
    let module_retains = backend.compute_module_retain_counts.clone();
    let dependency_retains = backend.compute_dependency_retain_counts.clone();
    let event_retains = backend.event_submission_retain_counts.clone();
    let completion_reservations = backend.compute_completion_reservations;
    let stream_tails = backend.stream_submission_tails.clone();
    let allocation_custody = backend.allocation_custody[&device].owners.clone();
    let allocation_sole_stream = backend.allocation_custody[&device].sole_stream;
    let allocation_owner_counts = backend.allocation_custody[&device].owner_counts;
    assert!(matches!(
        backend.allocations[&device].sdma_storage,
        KfdRuntimeSdmaStorageV1::ComputeInFlight(actual) if actual == compute
    ));

    assert_eq!(
        backend.wait_v1(compute, Instant::now()).unwrap(),
        BackendPollV1::Pending
    );
    assert_eq!(backend.scripted_persistent_wait_observations, 1);
    assert_eq!(backend.scripted_persistent_wait_pending_observations, 0);
    assert_eq!(backend.active_compute_lane_v1(compute), Some(lane));
    let active = backend.active_compute_submission_v1(compute).unwrap();
    assert_eq!(
        (
            active.id,
            active.stream,
            active.ordered_predecessor,
            active.kernel,
            active.dependency_depth,
            active.allocations.clone(),
            active.dispatch_shape_sha256,
        ),
        active_identity
    );
    assert!(matches!(
        active.execution,
        Some(ActiveComputeExecutionV1::ScriptedPersistent { .. })
    ));
    assert_eq!(backend.stream_compute_lanes, lane_index);
    assert_eq!(backend.compute_module_retain_counts, module_retains);
    assert_eq!(backend.compute_dependency_retain_counts, dependency_retains);
    assert_eq!(backend.event_submission_retain_counts, event_retains);
    assert_eq!(backend.stream_submission_tails, stream_tails);
    assert_eq!(
        backend.compute_completion_reservations,
        completion_reservations
    );
    assert_eq!(
        backend.allocation_custody[&device].owners,
        allocation_custody
    );
    assert_eq!(
        backend.allocation_custody[&device].sole_stream,
        allocation_sole_stream
    );
    assert_eq!(
        backend.allocation_custody[&device].owner_counts,
        allocation_owner_counts
    );
    assert!(matches!(
        backend.allocations[&device].sdma_storage,
        KfdRuntimeSdmaStorageV1::ComputeInFlight(actual) if actual == compute
    ));
    assert!(!backend.submissions.contains_key(&compute));

    assert_eq!(
        backend
            .wait_v1(compute, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(backend.scripted_persistent_wait_observations, 2);

    backend.release_event_v1(dependency_event).unwrap();
    for submission in [copy, compute] {
        backend.release_submission_v1(submission).unwrap();
    }
    backend.unload_module_v1(module).unwrap();
    release_scripted_direct_pair_v1(&mut backend, host, device);
    backend.destroy_stream_v1(stream).unwrap();
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 0);
    assert_eq!(driver.unexpected_drops(), 0);
    backend.shutdown_native_v1().unwrap();
}
