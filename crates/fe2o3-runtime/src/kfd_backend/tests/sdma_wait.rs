use super::*;

#[cfg(feature = "hardware-qualification")]
#[test]
fn drain_observer_tracks_real_async_copy_ownership_without_recounting_pending_polls() {
    let mut steps = vec![
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            8,
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Pending),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Pending),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
    ];
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
    backend.launch_gate = KfdRuntimeLaunchGateV1::CopyOnlyQualification;
    backend.drain_capture_publications =
        Some(qualification_drain_capture::PublicationHistoryV1::new());
    let (source, destination) = scripted_copy_regions_v1(host, device, 8);
    let submission = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert!(!backend.submissions.contains_key(&submission));
    let published = backend.drain_capture_runtime_roster_v1().unwrap();
    assert_eq!(published.publication_ids(), &[submission]);
    assert_eq!(published.copies().len(), 1);
    assert_eq!(
        published.copies()[0].phase,
        KfdDrainCaptureCopyPhaseV1::DirectionalPublished
    );
    // Scripted custody may test runtime shape, but never grants native evidence.
    assert_eq!(
        backend.diagnose_drain_capture_custody_v1(),
        Err(KfdDrainCaptureObservationFailureV1::NativeQueueUnavailable)
    );
    for _ in 0..2 {
        assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
        assert!(!backend.submissions.contains_key(&submission));
        assert_eq!(
            backend.drain_capture_runtime_roster_v1().unwrap(),
            published
        );
    }
    assert_eq!(
        backend.poll_v1(submission).unwrap(),
        BackendPollV1::Succeeded
    );
    let completed = backend.drain_capture_runtime_roster_v1().unwrap();
    assert!(completed.copies().is_empty());
    assert_eq!(completed.publication_ids(), &[submission]);
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
}

#[test]
fn scripted_sdma_wait_completes_directional_scalar_and_window_owners() {
    for direction in [
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        Gfx942PersistentSdmaDirectionV1::DeviceToHost,
    ] {
        let mut steps = vec![
            scripted_submit_step_v1(direction, 0, 0, 8, ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
            ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
        ];
        steps.extend(scripted_release_steps_v1());
        let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
        let (source, destination) = match direction {
            Gfx942PersistentSdmaDirectionV1::HostToDevice => {
                scripted_copy_regions_v1(host, device, 8)
            }
            Gfx942PersistentSdmaDirectionV1::DeviceToHost => (
                BackendMemoryRegionV1 {
                    allocation: device,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 8,
                },
                BackendMemoryRegionV1 {
                    allocation: host,
                    access: RuntimeAccessV1::Write,
                    byte_offset: 0,
                    byte_len: 8,
                },
            ),
        };
        let submission = backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();

        assert_eq!(
            backend
                .wait_v1(submission, Instant::now() + Duration::from_secs(1))
                .unwrap(),
            BackendPollV1::Succeeded
        );
        assert!(backend.active_sdma.is_empty());
        assert!(backend.published_sdma_submissions.is_empty());
        assert!(backend.published_sdma_index_is_consistent_v1());
        assert!(matches!(
            backend.allocations[&host].sdma_storage,
            KfdRuntimeSdmaStorageV1::Host(_)
        ));
        assert!(matches!(
            backend.allocations[&device].sdma_storage,
            KfdRuntimeSdmaStorageV1::Device(_)
        ));
        assert!(backend.allocations[&destination.allocation].sdma_shadow_dirty);
        clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
    }

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
    let mut steps = vec![
        scripted_submit_window_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            requests,
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::CompletedWindow {
            direction: None,
            copy_bytes: None,
            requests: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
    ];
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(byte_len, steps);
    let (source, destination) = scripted_copy_regions_v1(host, device, byte_len as u64);
    let submission = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();

    assert_eq!(
        backend
            .wait_v1(submission, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    assert!(backend.active_sdma.is_empty());
    assert!(backend.published_sdma_submissions.is_empty());
    assert!(backend.published_sdma_index_is_consistent_v1());
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
}

#[test]
fn scripted_sdma_wait_completes_h2d_promotion_and_same_device_copy() {
    let byte_len = usize::try_from(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1).unwrap();
    let mut h2d_steps = vec![
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
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
    ];
    h2d_steps.extend(scripted_release_steps_v1());
    let (mut h2d, stream, host, device) = scripted_direct_backend_v1(byte_len, h2d_steps);
    let expected = vec![0x5a; byte_len];
    h2d.write_allocation_v1(host, 0, &expected).unwrap();
    let (source, destination) =
        scripted_copy_regions_v1(host, device, u64::try_from(byte_len).unwrap());
    let submission = h2d.copy_async_v1(stream, source, destination, &[]).unwrap();

    assert_eq!(
        h2d.wait_v1(submission, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    let ready = h2d.allocations[&device]
        .sdma_storage
        .persistent_compute_ready_facts_v1()
        .expect("wait completion must retain authenticated H2D-ready custody");
    assert_eq!(ready.logical_bytes, u64::try_from(byte_len).unwrap());
    let expected_sha256: [u8; 32] = Sha256::digest(&expected).into();
    assert_eq!(ready.authenticated_sha256, expected_sha256);
    assert!(h2d.last_ready_promotion_performance_v1().is_some());
    clean_scripted_direct_backend_v1(&mut h2d, stream, host, device, Some(submission));

    let request = SameDeviceSdmaCopyRequestV1 {
        source_offset: 0,
        destination_offset: 0,
        copy_bytes: 8,
    };
    let mut d2d_steps = vec![
        scripted_same_device_submit_window_step_v1([request], ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::WaitSameDevice(ScriptedSameDeviceExecutionOutcomeV1::Completed {
            copy_bytes: None,
            requests: None,
            swap_allocations: false,
        }),
        ScriptedSdmaStepV1::RetireSameDevice(ScriptedFailureModeV1::Success),
    ];
    d2d_steps.extend(scripted_same_device_release_steps_v1());
    let (mut d2d, stream, source, destination) = scripted_same_device_backend_v1(8, d2d_steps);
    let (source_region, destination_region) =
        scripted_same_device_copy_regions_v1(source, destination, 8);
    let submission = d2d
        .copy_async_v1(stream, source_region, destination_region, &[])
        .unwrap();

    assert_eq!(
        d2d.wait_v1(submission, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    assert!(d2d.allocations[&destination].sdma_shadow_dirty);
    assert!(d2d.active_sdma.is_empty());
    assert!(d2d.published_sdma_submissions.is_empty());
    assert!(d2d.published_sdma_index_is_consistent_v1());
    clean_scripted_same_device_backend_v1(&mut d2d, stream, source, destination, Some(submission));
}

#[test]
fn scripted_sdma_zero_deadline_wait_observes_once_and_restores_every_index() {
    let mut steps = vec![
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            8,
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Pending),
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
    ];
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
    let dependency = backend.next_id().unwrap();
    backend.submissions.insert(
        dependency,
        SubmissionRecordV1 {
            origin: SubmissionOriginV1::Ordinary,
            stream,
            status: BackendPollV1::Succeeded,
            dependency_depth: 1,
            profile_dispatch_published: false,
        },
    );
    let event = backend.record_event_v1(stream, dependency).unwrap();
    let (source, destination) = scripted_copy_regions_v1(host, device, 8);
    let submission = backend
        .copy_async_v1(stream, source, destination, &[event])
        .unwrap();
    let dependency_retains = backend.sdma_dependency_retain_counts.clone();
    let host_custody = backend.allocation_custody[&host].owners.clone();
    let device_custody = backend.allocation_custody[&device].owners.clone();
    let active_stream = backend.active_sdma_streams[&stream].clone();
    assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 6);

    assert_eq!(
        backend.wait_v1(submission, Instant::now()).unwrap(),
        BackendPollV1::Pending
    );
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
        5,
        "an expired deadline still performs one native observation"
    );
    let active = &backend.active_sdma[&submission];
    assert_eq!(active.completed_bytes, 0);
    assert_eq!(active.window_bytes, 8);
    assert!(active.window_requests.is_some());
    assert!(matches!(
        active.phase,
        ActiveSdmaPhaseV1::DirectionalPublished(_)
    ));
    assert_eq!(backend.published_sdma_submissions, [submission]);
    assert!(backend.published_sdma_index_is_consistent_v1());
    assert_eq!(backend.sdma_dependency_retain_counts, dependency_retains);
    assert_eq!(backend.allocation_custody[&host].owners, host_custody);
    assert_eq!(backend.allocation_custody[&device].owners, device_custody);
    assert_eq!(backend.active_sdma_streams[&stream], active_stream);
    for allocation in [host, device] {
        assert!(matches!(
            backend.allocations[&allocation].sdma_storage,
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(actual))
                if actual == submission
        ));
    }

    assert_eq!(
        backend
            .wait_v1(submission, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    assert!(backend.sdma_dependency_retain_counts.is_empty());
    assert!(!backend.allocation_custody.contains_key(&host));
    assert!(!backend.allocation_custody.contains_key(&device));
    assert!(!backend.active_sdma_streams.contains_key(&stream));
    assert!(backend.published_sdma_submissions.is_empty());
    assert!(backend.published_sdma_index_is_consistent_v1());
    backend.release_event_v1(event).unwrap();
    backend.release_submission_v1(dependency).unwrap();
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
}

#[test]
fn scripted_same_device_wait_timeout_restores_pair_index_and_retains() {
    let request = SameDeviceSdmaCopyRequestV1 {
        source_offset: 0,
        destination_offset: 0,
        copy_bytes: 8,
    };
    let mut steps = vec![
        scripted_same_device_submit_window_step_v1([request], ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::WaitSameDevice(ScriptedSameDeviceExecutionOutcomeV1::Pending),
        ScriptedSdmaStepV1::WaitSameDevice(ScriptedSameDeviceExecutionOutcomeV1::Completed {
            copy_bytes: None,
            requests: None,
            swap_allocations: false,
        }),
        ScriptedSdmaStepV1::RetireSameDevice(ScriptedFailureModeV1::Success),
    ];
    steps.extend(scripted_same_device_release_steps_v1());
    let (mut backend, stream, source, destination) = scripted_same_device_backend_v1(8, steps);
    let dependency = backend.next_id().unwrap();
    backend.submissions.insert(
        dependency,
        SubmissionRecordV1 {
            origin: SubmissionOriginV1::Ordinary,
            stream,
            status: BackendPollV1::Succeeded,
            dependency_depth: 1,
            profile_dispatch_published: false,
        },
    );
    let event = backend.record_event_v1(stream, dependency).unwrap();
    let (source_region, destination_region) =
        scripted_same_device_copy_regions_v1(source, destination, 8);
    let submission = backend
        .copy_async_v1(stream, source_region, destination_region, &[event])
        .unwrap();
    let dependency_retains = backend.sdma_dependency_retain_counts.clone();
    let source_custody = backend.allocation_custody[&source].owners.clone();
    let destination_custody = backend.allocation_custody[&destination].owners.clone();

    assert_eq!(
        backend.wait_v1(submission, Instant::now()).unwrap(),
        BackendPollV1::Pending
    );
    assert!(matches!(
        backend.active_sdma[&submission].phase,
        ActiveSdmaPhaseV1::SameDevicePublished(_)
    ));
    assert_eq!(backend.published_sdma_submissions, [submission]);
    assert!(backend.published_sdma_index_is_consistent_v1());
    assert_eq!(backend.sdma_dependency_retain_counts, dependency_retains);
    assert_eq!(backend.allocation_custody[&source].owners, source_custody);
    assert_eq!(
        backend.allocation_custody[&destination].owners,
        destination_custody
    );
    for allocation in [source, destination] {
        assert!(matches!(
            backend.allocations[&allocation].sdma_storage,
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(actual))
                if actual == submission
        ));
    }

    assert_eq!(
        backend
            .wait_v1(submission, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    assert!(backend.sdma_dependency_retain_counts.is_empty());
    assert!(!backend.allocation_custody.contains_key(&source));
    assert!(!backend.allocation_custody.contains_key(&destination));
    assert!(!backend.active_sdma_streams.contains_key(&stream));
    assert!(backend.published_sdma_submissions.is_empty());
    assert!(backend.published_sdma_index_is_consistent_v1());
    backend.release_event_v1(event).unwrap();
    backend.release_submission_v1(dependency).unwrap();
    clean_scripted_same_device_backend_v1(
        &mut backend,
        stream,
        source,
        destination,
        Some(submission),
    );
}

#[test]
fn scripted_same_device_wait_terminal_outcomes_absorb_exact_pair() {
    let request = SameDeviceSdmaCopyRequestV1 {
        source_offset: 0,
        destination_offset: 0,
        copy_bytes: 8,
    };
    let outcomes = [
        ScriptedSameDeviceExecutionOutcomeV1::Retryable,
        ScriptedSameDeviceExecutionOutcomeV1::ProcessTeardown,
        ScriptedSameDeviceExecutionOutcomeV1::Completed {
            copy_bytes: None,
            requests: None,
            swap_allocations: true,
        },
    ];
    for outcome in outcomes {
        let steps = [
            scripted_same_device_submit_window_step_v1([request], ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::WaitSameDevice(outcome),
        ];
        let (mut backend, stream, source, destination) = scripted_same_device_backend_v1(8, steps);
        let (source_region, destination_region) =
            scripted_same_device_copy_regions_v1(source, destination, 8);
        let submission = backend
            .copy_async_v1(stream, source_region, destination_region, &[])
            .unwrap();

        assert!(matches!(
            backend.wait_v1(submission, Instant::now()),
            Err(RuntimeBackendFailureV1::Terminal(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
        ));
        assert!(backend.terminal);
        assert!(matches!(
            backend.active_sdma[&submission].phase,
            ActiveSdmaPhaseV1::Quarantined
        ));
        assert!(backend.published_sdma_submissions.is_empty());
        assert!(backend.published_sdma_index_is_consistent_v1());
        assert!(backend.terminal_sdma_custody.is_some());
        for allocation in [source, destination] {
            assert!(matches!(
                backend.allocations[&allocation].sdma_storage,
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(actual))
                    if actual == submission
            ));
        }
        let driver = backend.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted());
        assert_eq!(driver.live_owner_count(), 2);
        assert_eq!(driver.unexpected_drops(), 0);
        disarm_scripted_drop_after_inspection_v1(&mut backend);
    }
}

#[test]
fn scripted_sdma_wait_non_timeout_retry_and_teardown_are_terminal_with_custody() {
    for outcome in [
        ScriptedExecutionOutcomeV1::Retryable,
        ScriptedExecutionOutcomeV1::ProcessTeardown,
    ] {
        let steps = [
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                8,
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Wait(outcome),
        ];
        let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
        let (source, destination) = scripted_copy_regions_v1(host, device, 8);
        let submission = backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();

        assert!(matches!(
            backend.wait_v1(submission, Instant::now()),
            Err(RuntimeBackendFailureV1::Terminal(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
        ));
        assert!(backend.terminal);
        assert!(matches!(
            backend.active_sdma[&submission].phase,
            ActiveSdmaPhaseV1::Quarantined
        ));
        assert!(backend.published_sdma_submissions.is_empty());
        assert!(backend.published_sdma_index_is_consistent_v1());
        assert!(backend.terminal_sdma_custody.is_some());
        for allocation in [host, device] {
            assert!(matches!(
                backend.allocations[&allocation].sdma_storage,
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(actual))
                    if actual == submission
            ));
        }
        let driver = backend.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted());
        assert_eq!(driver.live_owner_count(), 2);
        assert_eq!(driver.unexpected_drops(), 0);
        disarm_scripted_drop_after_inspection_v1(&mut backend);
    }
}

#[test]
fn scripted_sdma_wait_changed_completion_identity_is_terminal() {
    let steps = [
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            8,
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Completed {
            direction: Some(Gfx942PersistentSdmaDirectionV1::DeviceToHost),
            copy_bytes: None,
        }),
    ];
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
    let (source, destination) = scripted_copy_regions_v1(host, device, 8);
    let submission = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();

    assert!(matches!(
        backend.wait_v1(submission, Instant::now()),
        Err(RuntimeBackendFailureV1::Terminal(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
    ));
    assert!(matches!(
        backend.terminal_sdma_custody,
        Some(KfdRuntimeTerminalSdmaCustodyV1::Completed(
            DirectionalSdmaCompletedOwnerV1::Scripted(_)
        ))
    ));
    assert!(matches!(
        backend.active_sdma[&submission].phase,
        ActiveSdmaPhaseV1::Quarantined
    ));
    assert!(backend.published_sdma_submissions.is_empty());
    assert!(backend.published_sdma_index_is_consistent_v1());
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 2);
    assert_eq!(driver.unexpected_drops(), 0);
    disarm_scripted_drop_after_inspection_v1(&mut backend);
}

#[test]
fn scripted_sdma_wait_completion_leaves_continuation_for_explicit_flush() {
    let mut steps = vec![
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            8,
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
    ];
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
    let (source, destination) = scripted_copy_regions_v1(host, device, 8);
    let submission = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    backend.active_sdma.get_mut(&submission).unwrap().byte_len = 16;

    assert_eq!(
        backend
            .wait_v1(submission, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Pending
    );
    let active = &backend.active_sdma[&submission];
    assert_eq!(active.completed_bytes, 8);
    assert_eq!(active.byte_len, 16);
    assert_eq!(active.window_bytes, 0);
    assert!(active.window_requests.is_none());
    assert!(matches!(active.phase, ActiveSdmaPhaseV1::Ready));
    assert!(backend.published_sdma_submissions.is_empty());
    assert!(backend.published_sdma_index_is_consistent_v1());
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
        3,
        "wait completion must not publish the next copy window"
    );

    let active = backend.active_sdma.get_mut(&submission).unwrap();
    active.byte_len = 8;
    active.completed_bytes = 0;
    assert_eq!(
        backend.cancel_v1(submission).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
}

#[test]
fn scripted_sdma_poll_and_wait_keep_distinct_native_routes() {
    let mut steps = vec![
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            8,
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Pending),
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
    ];
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
    let (source, destination) = scripted_copy_regions_v1(host, device, 8);
    let submission = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();

    assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
    assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 5);
    assert_eq!(
        backend
            .wait_v1(submission, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 3);
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
}

#[test]
fn scripted_directional_poll_retry_is_terminal_with_exact_custody() {
    let steps = [
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            8,
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Retryable),
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
    assert!(backend.terminal);
    assert!(matches!(
        backend.active_sdma[&submission].phase,
        ActiveSdmaPhaseV1::Quarantined
    ));
    assert!(backend.published_sdma_submissions.is_empty());
    assert!(backend.published_sdma_index_is_consistent_v1());
    assert!(matches!(
        backend.terminal_sdma_custody,
        Some(KfdRuntimeTerminalSdmaCustodyV1::Pending(
            DirectionalSdmaSubmissionOwnerV1::Scripted(_)
        ))
    ));
    for allocation in [host, device] {
        assert!(matches!(
            backend.allocations[&allocation].sdma_storage,
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(actual))
                if actual == submission
        ));
    }
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 2);
    assert_eq!(driver.unexpected_drops(), 0);
    let _ = stream;
    disarm_scripted_drop_after_inspection_v1(&mut backend);
}
