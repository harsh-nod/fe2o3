use super::*;

#[test]
fn scripted_sdma_exact_linear_limit_uses_one_owned_single_transition() {
    let copy_bytes = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1;
    let byte_len = usize::try_from(copy_bytes).unwrap();
    let mut steps = vec![
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            copy_bytes,
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
    ];
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(byte_len, steps);
    let (source, destination) = scripted_copy_regions_v1(host, device, u64::from(copy_bytes));
    let submission = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(
        backend.poll_v1(submission).unwrap(),
        BackendPollV1::Succeeded
    );
    assert!(backend.active_sdma.is_empty());
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
}

#[test]
fn scripted_sdma_multi_packet_window_completes_as_one_owned_transition() {
    let first = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1;
    let byte_len = usize::try_from(first).unwrap() + 1;
    let mut steps = vec![
        scripted_submit_window_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            [
                DirectionalSdmaCopyRequestV1 {
                    host_offset: 0,
                    device_offset: 0,
                    copy_bytes: first,
                },
                DirectionalSdmaCopyRequestV1 {
                    host_offset: u64::from(first),
                    device_offset: u64::from(first),
                    copy_bytes: 1,
                },
            ],
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Pending),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
    ];
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(byte_len, steps);
    let (source, destination) = scripted_copy_regions_v1(host, device, byte_len as u64);
    let submission = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
    assert_eq!(
        backend.poll_v1(submission).unwrap(),
        BackendPollV1::Succeeded
    );
    assert!(!backend.quiescent_sdma_submissions.contains(&submission));
    assert!(backend.active_sdma.is_empty());
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
}

#[test]
fn scripted_sdma_dependency_pending_is_observed_without_publication() {
    let mut steps = vec![
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            8,
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
    ];
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
    let dependency = backend.next_id().unwrap();
    let event = backend.next_id().unwrap();
    backend.submissions.insert(
        dependency,
        SubmissionRecordV1 {
            origin: SubmissionOriginV1::Ordinary,
            stream,
            status: BackendPollV1::Pending,
            dependency_depth: 1,
            profile_dispatch_published: false,
        },
    );
    backend.events.insert(
        event,
        EventRecordV1 {
            submission: dependency,
        },
    );
    backend.event_submission_retain_counts.insert(dependency, 1);
    let (source, destination) = scripted_copy_regions_v1(host, device, 8);
    let submission = backend
        .copy_async_v1(stream, source, destination, &[event])
        .unwrap();
    assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
    assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 6);
    backend.submissions.get_mut(&dependency).unwrap().status = BackendPollV1::Succeeded;
    backend.flush_stream_v1(stream).unwrap();
    assert_eq!(
        backend.poll_v1(submission).unwrap(),
        BackendPollV1::Succeeded
    );
    backend.release_event_v1(event).unwrap();
    backend.release_submission_v1(dependency).unwrap();
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
}

#[test]
fn scripted_unpublished_sdma_dependency_rejection_is_terminal_and_reindexed() {
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, []);
    let dependency = backend.next_id().unwrap();
    backend.submissions.insert(
        dependency,
        SubmissionRecordV1 {
            origin: SubmissionOriginV1::Ordinary,
            stream,
            status: BackendPollV1::Pending,
            dependency_depth: 1,
            profile_dispatch_published: false,
        },
    );
    let event = backend.record_event_v1(stream, dependency).unwrap();
    let (source, destination) = scripted_copy_regions_v1(host, device, 8);
    let submission = backend
        .copy_async_v1(stream, source, destination, &[event])
        .unwrap();
    assert!(matches!(
        backend.active_sdma[&submission].phase,
        ActiveSdmaPhaseV1::Ready
    ));
    backend.submissions.remove(&dependency).unwrap();

    assert!(matches!(
        backend.poll_v1(submission),
        Err(RuntimeBackendFailureV1::Terminal(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
    ));
    assert!(backend.terminal);
    assert!(matches!(
        backend.active_sdma[&submission].phase,
        ActiveSdmaPhaseV1::Ready
    ));
    assert!(backend.terminal_sdma_custody.is_none());
    assert!(backend.published_sdma_submissions.is_empty());
    assert!(backend.published_sdma_index_is_consistent_v1());
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 2);
    assert_eq!(driver.unexpected_drops(), 0);
    disarm_scripted_drop_after_inspection_v1(&mut backend);
}

#[test]
fn scripted_initial_compute_progress_rejection_preserves_accepted_custody() {
    let steps = [
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            8,
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
    ];
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
    let owner_ids = |backend: &KfdRuntimeBackendV1| {
        let KfdRuntimeSdmaStorageV1::Host(SdmaBufferOwnerV1::Scripted(host_owner)) =
            &backend.allocations[&host].sdma_storage
        else {
            panic!("missing scripted host owner");
        };
        let KfdRuntimeSdmaStorageV1::Device(device_owner) =
            &backend.allocations[&device].sdma_storage
        else {
            panic!("missing scripted device owner");
        };
        (
            host_owner.observation().0,
            device_owner.scripted_owner_id().unwrap(),
        )
    };
    let original_owners = owner_ids(&backend);
    let (source, destination) = scripted_copy_regions_v1(host, device, 8);
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
    // Inject a broken predecessor index, retaining its real ordered stream
    // tail. No test authority or scripted token is native GPU authority.
    backend.submissions.remove(&copy).unwrap();
    assert_eq!(backend.stream_submission_tails[&stream], copy);
    assert!(!backend.allocations[&host].sdma_shadow_dirty);
    assert!(backend.allocations[&host].native_dirty.is_empty());
    assert_eq!(backend.native_dirty_extents, 0);
    let accepted_id = backend.next_handle;
    let mut kernarg = [0; 16];
    kernarg[8..].copy_from_slice(&13_u64.to_le_bytes());
    let result = backend.submit_v1(BackendLaunchV1 {
        stream,
        kernel,
        explicit_kernarg: &kernarg,
        bindings: &[BackendBindingV1 {
            region: source,
            kernarg_byte_offset: 0,
        }],
        dependencies: &[],
        geometry: crate::RuntimeLaunchGeometryV1 {
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
            dynamic_shared_bytes: 0,
        },
        semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
    });
    let Err(RuntimeBackendFailureV1::Terminal(error)) = result else {
        panic!("accepted custody reported definite rejection");
    };
    assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Terminal);
    assert_eq!(error.detail(), "unknown KFD submission");
    assert!(backend.terminal);
    let pending = &backend.pending_compute[&accepted_id];
    assert_eq!(pending.id, accepted_id);
    assert_eq!(pending.module, module);
    assert_eq!(pending.ordered_predecessor, Some(copy));
    assert_eq!(&*pending.retained_allocations, &[host]);
    assert_eq!(pending.launch.bindings[0].region.allocation, host);
    assert_eq!(
        backend.pending_compute_streams[&stream].front(),
        Some(&accepted_id)
    );
    assert_eq!(backend.stream_submission_tails[&stream], accepted_id);
    assert_eq!(backend.compute_module_retain_counts[&module], 1);
    assert_eq!(backend.compute_dependency_retain_counts[&copy], 1);
    assert_eq!(
        backend.allocation_custody[&host]
            .owners
            .front()
            .unwrap()
            .submission,
        accepted_id
    );
    assert_eq!(backend.compute_completion_reservations, 1);
    assert!(backend.active.is_none());
    assert_eq!(owner_ids(&backend), original_owners);
    let next = backend.next_handle;
    assert!(matches!(
        backend.poll_v1(accepted_id),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert_eq!(backend.next_handle, next);
    assert!(backend.pending_compute.contains_key(&accepted_id));
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 2);
    assert_eq!(driver.unexpected_drops(), 0);
    disarm_scripted_drop_after_inspection_v1(&mut backend);
}

#[test]
fn scripted_pending_compute_dependency_rejection_is_terminal_and_reindexed() {
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
    backend
        .write_allocation_v1(host, 0, &vec![0xd8; byte_len])
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
    let dependency = backend.next_id().unwrap();
    backend.submissions.insert(
        dependency,
        SubmissionRecordV1 {
            origin: SubmissionOriginV1::Ordinary,
            stream,
            status: BackendPollV1::Pending,
            dependency_depth: 1,
            profile_dispatch_published: false,
        },
    );
    let event = backend.record_event_v1(stream, dependency).unwrap();
    let compute = submit_scripted_read_v1(
        &mut backend,
        stream,
        kernel,
        device,
        u64::try_from(byte_len).unwrap(),
        &[event],
    );
    assert!(backend.pending_compute.contains_key(&compute));
    backend.submissions.remove(&dependency).unwrap();

    assert!(matches!(
        backend.poll_v1(compute),
        Err(RuntimeBackendFailureV1::Terminal(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
    ));
    assert!(backend.terminal);
    assert!(backend.pending_compute.contains_key(&compute));
    assert!(backend.active.is_none());
    assert!(matches!(
        backend.allocations[&device].sdma_storage,
        KfdRuntimeSdmaStorageV1::H2dReady(_)
    ));
    assert!(backend.terminal_sdma_custody.is_none());
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 2);
    assert_eq!(driver.unexpected_drops(), 0);
    let _ = (copy, module);
    disarm_scripted_drop_after_inspection_v1(&mut backend);
}

#[test]
fn scripted_sdma_metadata_and_retirement_failures_retain_terminal_custody() {
    for steps in [
        vec![
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                8,
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: Some(7),
            }),
        ],
        vec![
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                8,
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
            ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::ProcessTeardown),
        ],
        vec![
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                8,
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                direction: Some(Gfx942PersistentSdmaDirectionV1::DeviceToHost),
                copy_bytes: None,
            }),
        ],
    ] {
        let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
        let (source, destination) = scripted_copy_regions_v1(host, device, 8);
        let submission = backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();
        assert!(matches!(
            backend.poll_v1(submission),
            Err(RuntimeBackendFailureV1::Terminal(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
        ));
        assert!(backend.terminal);
        assert!(backend.terminal_sdma_custody.is_some());
        let driver = backend.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted());
        assert_eq!(driver.live_owner_count(), 2);
        assert_eq!(driver.unexpected_drops(), 0);
        disarm_scripted_drop_after_inspection_v1(&mut backend);
    }
}

#[test]
fn scripted_sdma_window_reordered_completion_is_terminal_with_exact_custody() {
    let first = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1;
    let byte_len = usize::try_from(first).unwrap() + 1;
    let requests = [
        DirectionalSdmaCopyRequestV1 {
            host_offset: 0,
            device_offset: 0,
            copy_bytes: first,
        },
        DirectionalSdmaCopyRequestV1 {
            host_offset: u64::from(first),
            device_offset: u64::from(first),
            copy_bytes: 1,
        },
    ];
    let steps = [
        scripted_submit_window_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            requests,
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::CompletedWindow {
            direction: None,
            copy_bytes: None,
            requests: Some(vec![requests[1], requests[0]]),
        }),
    ];
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(byte_len, steps);
    let (source, destination) = scripted_copy_regions_v1(host, device, byte_len as u64);
    let submission = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert!(matches!(
        backend.poll_v1(submission),
        Err(RuntimeBackendFailureV1::Terminal(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
    ));
    assert!(backend.terminal_sdma_custody.is_some());
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 2);
    assert_eq!(driver.unexpected_drops(), 0);
    disarm_scripted_drop_after_inspection_v1(&mut backend);
}

#[test]
fn scripted_sdma_window_aggregate_offset_substitution_is_terminal() {
    for reported in [
        DirectionalSdmaCopyRequestV1 {
            host_offset: 1,
            device_offset: 0,
            copy_bytes: 8,
        },
        DirectionalSdmaCopyRequestV1 {
            host_offset: 0,
            device_offset: 1,
            copy_bytes: 8,
        },
    ] {
        let steps = [
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                8,
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::CompletedWindow {
                direction: None,
                copy_bytes: None,
                requests: Some(vec![reported]),
            }),
        ];
        let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
        let (source, destination) = scripted_copy_regions_v1(host, device, 8);
        let submission = backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();
        assert!(matches!(
            backend.poll_v1(submission),
            Err(RuntimeBackendFailureV1::Terminal(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
        ));
        assert!(backend.terminal_sdma_custody.is_some());
        let driver = backend.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted());
        assert_eq!(driver.live_owner_count(), 2);
        assert_eq!(driver.unexpected_drops(), 0);
        disarm_scripted_drop_after_inspection_v1(&mut backend);
    }
}

#[test]
fn scripted_sdma_poll_teardown_and_sync_timeout_fail_closed_with_custody() {
    let poll_steps = [
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            8,
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::ProcessTeardown),
    ];
    let (mut poll_backend, stream, host, device) = scripted_direct_backend_v1(8, poll_steps);
    let (source, destination) = scripted_copy_regions_v1(host, device, 8);
    let submission = poll_backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert!(matches!(
        poll_backend.poll_v1(submission),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert_eq!(
        poll_backend
            .scripted_sdma
            .as_ref()
            .unwrap()
            .live_owner_count(),
        2
    );
    disarm_scripted_drop_after_inspection_v1(&mut poll_backend);

    let sync_steps = [
        ScriptedSdmaStepV1::Allocate {
            kind: ScriptedBufferKindV1::Host,
            byte_len: 8,
        },
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len: 8,
        },
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            8,
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Pending),
    ];
    let (mut sync_backend, _, _, sync_device) = scripted_direct_backend_v1(8, sync_steps);
    assert!(matches!(
        sync_backend.write_allocation_v1(sync_device, 0, &[9; 8]),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(sync_backend.terminal_sdma_custody.is_some());
    assert_eq!(
        sync_backend
            .scripted_sdma
            .as_ref()
            .unwrap()
            .live_owner_count(),
        3
    );
    disarm_scripted_drop_after_inspection_v1(&mut sync_backend);
}

#[test]
fn scripted_sdma_hidden_zero_failure_cleans_unreachable_allocation() {
    let steps = [
        ScriptedSdmaStepV1::Allocate {
            kind: ScriptedBufferKindV1::Device,
            byte_len: 8,
        },
        ScriptedSdmaStepV1::Promote(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Allocate {
            kind: ScriptedBufferKindV1::Host,
            byte_len: 8,
        },
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len: 8,
        },
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            8,
            ScriptedFailureModeV1::Retryable,
        ),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ];
    let mut backend = KfdRuntimeBackendV1::mock();
    backend.native_available = true;
    backend.scripted_sdma = Some(ScriptedSdmaDriverV1::new(steps));
    assert!(matches!(
        backend.allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8),
        Err(RuntimeBackendFailureV1::Quiescent(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Native
    ));
    assert!(backend.allocations.is_empty());
    assert_eq!(backend.staged_context_bytes, 0);
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 0);
    assert_eq!(driver.unexpected_drops(), 0);
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn scripted_sdma_chunk_n_upload_and_zero_failures_mark_device_shadow_dirty() {
    let first = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1;
    let byte_len = usize::try_from(first).unwrap() + 1;
    for operation in 0..3 {
        let zero = operation == 0;
        let mut steps = scripted_sync_copy_steps_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            first,
            ScriptedFailureModeV1::Success,
        );
        steps.extend(scripted_sync_copy_steps_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            u64::from(first),
            1,
            ScriptedFailureModeV1::Retryable,
        ));
        steps.extend(scripted_release_steps_v1());
        let (mut backend, stream, host, device) = scripted_direct_backend_v1(byte_len, steps);
        let device_owner = match &mut backend.allocations.get_mut(&device).unwrap().sdma_storage {
            KfdRuntimeSdmaStorageV1::Device(device) => device,
            _ => unreachable!("scripted device allocation remains directional"),
        };
        device_owner
            .scripted_bytes_mut()
            .unwrap()
            .fill(if zero { 0xa5 } else { 0 });
        let result = if zero {
            backend.zero_sdma_range_v1(device, byte_len as u64)
        } else if operation == 2 {
            backend.write_allocation_v1(device, 0, &vec![0x5a; byte_len])
        } else {
            backend.upload_sdma_range_v1(device, 0, &vec![0x5a; byte_len])
        };
        assert!(matches!(
            result,
            Err(RuntimeBackendFailureV1::Quiescent(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Native
        ));
        let record = &backend.allocations[&device];
        assert!(record.sdma_shadow_dirty);
        assert!(record.content_sha256.is_none());
        assert!(record.last_full_host_write.is_none());
        let device_bytes = match &record.sdma_storage {
            KfdRuntimeSdmaStorageV1::Device(device) => device.scripted_bytes().unwrap(),
            _ => unreachable!("recovered chunk failure restores device custody"),
        };
        let first = usize::try_from(first).unwrap();
        assert!(
            device_bytes[..first]
                .iter()
                .all(|byte| *byte == if zero { 0 } else { 0x5a })
        );
        assert_eq!(device_bytes[first], if zero { 0xa5 } else { 0 });
        clean_scripted_direct_backend_v1(&mut backend, stream, host, device, None);
    }
}

#[test]
fn scripted_sdma_chunk_n_download_and_shadow_failure_are_quiescent() {
    let first = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1;
    let byte_len = usize::try_from(first).unwrap() + 1;
    for reconcile_shadow in [false, true] {
        let mut steps = scripted_sync_copy_steps_v1(
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
            0,
            first,
            ScriptedFailureModeV1::Success,
        );
        steps.extend(scripted_sync_copy_steps_v1(
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
            u64::from(first),
            1,
            ScriptedFailureModeV1::Retryable,
        ));
        steps.extend(scripted_release_steps_v1());
        let (mut backend, stream, host, device) = scripted_direct_backend_v1(byte_len, steps);
        let result = if reconcile_shadow {
            let record = backend.allocations.get_mut(&device).unwrap();
            record.sdma_shadow_dirty = true;
            Arc::make_mut(&mut record.bytes).fill(0xff);
            let result = backend.synchronize_sdma_shadow_v1(device);
            let bytes = &backend.allocations[&device].bytes;
            let first = usize::try_from(first).unwrap();
            assert!(bytes[..first].iter().all(|byte| *byte == 0));
            assert_eq!(bytes[first], 0xff);
            result
        } else {
            let mut destination = vec![0xff; byte_len];
            let result = backend
                .download_sdma_range_v1(device, 0, &mut destination)
                .map(|_| ());
            let first = usize::try_from(first).unwrap();
            assert!(destination[..first].iter().all(|byte| *byte == 0));
            assert_eq!(destination[first], 0xff);
            result
        };
        assert!(matches!(
            result,
            Err(RuntimeBackendFailureV1::Quiescent(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Native
        ));
        if reconcile_shadow {
            assert!(backend.allocations[&device].sdma_shadow_dirty);
        }
        clean_scripted_direct_backend_v1(&mut backend, stream, host, device, None);
    }
}

#[test]
fn scripted_sdma_device_release_executes_scrub_before_demotion_and_recycle() {
    let mut steps = scripted_sync_copy_steps_v1(
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        0,
        8,
        ScriptedFailureModeV1::Success,
    );
    steps.extend([
        ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]);
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
    backend.release_allocation_v1(device).unwrap();
    backend.release_allocation_v1(host).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 0);
    assert_eq!(driver.unexpected_drops(), 0);
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn scripted_sdma_demotion_and_submit_teardown_retain_exact_runtime_custody() {
    let (mut demotion, _, _, device) = scripted_direct_backend_v1(
        8,
        [ScriptedSdmaStepV1::Demote(
            ScriptedFailureModeV1::ProcessTeardown,
        )],
    );
    demotion.allocations.get_mut(&device).unwrap().sdma_backed = false;
    assert!(matches!(
        demotion.release_allocation_v1(device),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(demotion.terminal_sdma_custody.is_some());
    assert_eq!(
        demotion.scripted_sdma.as_ref().unwrap().live_owner_count(),
        2
    );
    disarm_scripted_drop_after_inspection_v1(&mut demotion);

    let steps = [scripted_submit_step_v1(
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        0,
        0,
        8,
        ScriptedFailureModeV1::ProcessTeardown,
    )];
    let (mut submit, stream, host, device) = scripted_direct_backend_v1(8, steps);
    let (source, destination) = scripted_copy_regions_v1(host, device, 8);
    assert!(matches!(
        submit.copy_async_v1(stream, source, destination, &[]),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(submit.terminal_sdma_custody.is_some());
    assert_eq!(submit.scripted_sdma.as_ref().unwrap().live_owner_count(), 2);
    disarm_scripted_drop_after_inspection_v1(&mut submit);
}

#[test]
fn scripted_sdma_host_read_and_ambiguous_recycle_follow_runtime_policy() {
    let mut read_steps = vec![ScriptedSdmaStepV1::Read {
        offset: 0,
        byte_len: 8,
    }];
    read_steps.extend(scripted_release_steps_v1());
    let (mut read_backend, stream, host, device) = scripted_direct_backend_v1(8, read_steps);
    let mut bytes = [0xff; 8];
    read_backend
        .read_allocation_v1(host, 0, &mut bytes)
        .unwrap();
    assert_eq!(bytes, [0; 8]);
    clean_scripted_direct_backend_v1(&mut read_backend, stream, host, device, None);

    let steps = [ScriptedSdmaStepV1::Recycle(
        ScriptedRecycleOutcomeV1::Ambiguous,
    )];
    let (mut ambiguous, _, host, _) = scripted_direct_backend_v1(8, steps);
    assert!(matches!(
        ambiguous.release_allocation_v1(host),
        Err(RuntimeBackendFailureV1::Terminal(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
    ));
    assert!(ambiguous.terminal);
    assert_eq!(
        ambiguous.scripted_sdma.as_ref().unwrap().live_owner_count(),
        2
    );
    assert!(
        ambiguous
            .scripted_sdma
            .as_ref()
            .unwrap()
            .recycle_custody()
            .is_some()
    );
    disarm_scripted_drop_after_inspection_v1(&mut ambiguous);
}
