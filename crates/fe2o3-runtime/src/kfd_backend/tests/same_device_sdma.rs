use super::*;

#[test]
fn scripted_same_device_sdma_remains_on_window_route_and_dirties_only_destination() {
    let request = SameDeviceSdmaCopyRequestV1 {
        source_offset: 0,
        destination_offset: 0,
        copy_bytes: 8,
    };
    let mut steps = vec![
        scripted_same_device_submit_window_step_v1([request], ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::PollSameDevice(ScriptedSameDeviceExecutionOutcomeV1::Pending),
        ScriptedSdmaStepV1::PollSameDevice(ScriptedSameDeviceExecutionOutcomeV1::Completed {
            copy_bytes: None,
            requests: None,
            swap_allocations: false,
        }),
        ScriptedSdmaStepV1::RetireSameDevice(ScriptedFailureModeV1::Success),
    ];
    steps.extend(scripted_same_device_release_steps_v1());
    let (mut backend, stream, source, destination) = scripted_same_device_backend_v1(8, steps);
    let expected = [1, 3, 5, 7, 9, 11, 13, 15];
    match &mut backend.allocations.get_mut(&source).unwrap().sdma_storage {
        KfdRuntimeSdmaStorageV1::Device(owner) => owner
            .scripted_bytes_mut()
            .unwrap()
            .copy_from_slice(&expected),
        _ => unreachable!("scripted source starts with device custody"),
    }
    let (source_region, destination_region) =
        scripted_same_device_copy_regions_v1(source, destination, 8);
    let submission = backend
        .copy_async_v1(stream, source_region, destination_region, &[])
        .unwrap();
    assert_eq!(backend.published_sdma_submissions, [submission]);
    assert!(backend.published_sdma_index_is_consistent_v1());
    let event = backend.record_event_v1(stream, submission).unwrap();

    assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
    assert_eq!(backend.published_sdma_submissions, [submission]);
    assert!(backend.published_sdma_index_is_consistent_v1());
    assert_eq!(
        backend.poll_v1(submission).unwrap(),
        BackendPollV1::Succeeded
    );
    assert!(backend.published_sdma_submissions.is_empty());
    assert!(backend.published_sdma_index_is_consistent_v1());
    assert!(matches!(
        backend.release_submission_v1(submission),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
    ));
    backend.release_event_v1(event).unwrap();

    let source_record = &backend.allocations[&source];
    let destination_record = &backend.allocations[&destination];
    let source_bytes = match &source_record.sdma_storage {
        KfdRuntimeSdmaStorageV1::Device(owner) => owner.scripted_bytes().unwrap(),
        _ => unreachable!("completed source restores device custody"),
    };
    let destination_bytes = match &destination_record.sdma_storage {
        KfdRuntimeSdmaStorageV1::Device(owner) => owner.scripted_bytes().unwrap(),
        _ => unreachable!("completed destination restores device custody"),
    };
    assert_eq!(source_bytes, expected);
    assert_eq!(destination_bytes, expected);
    assert!(!source_record.sdma_shadow_dirty);
    assert!(destination_record.sdma_shadow_dirty);
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
        2
    );
    clean_scripted_same_device_backend_v1(
        &mut backend,
        stream,
        source,
        destination,
        Some(submission),
    );
}

#[test]
fn scripted_same_device_poll_retry_is_terminal_with_exact_custody() {
    let request = SameDeviceSdmaCopyRequestV1 {
        source_offset: 0,
        destination_offset: 0,
        copy_bytes: 8,
    };
    let steps = [
        scripted_same_device_submit_window_step_v1([request], ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::PollSameDevice(ScriptedSameDeviceExecutionOutcomeV1::Retryable),
    ];
    let (mut backend, stream, source, destination) = scripted_same_device_backend_v1(8, steps);
    let (source_region, destination_region) =
        scripted_same_device_copy_regions_v1(source, destination, 8);
    let submission = backend
        .copy_async_v1(stream, source_region, destination_region, &[])
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
        Some(KfdRuntimeTerminalSdmaCustodyV1::SameDevicePending(
            SameDeviceSdmaSubmissionOwnerV1::Scripted(_)
        ))
    ));
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
    let _ = stream;
    disarm_scripted_drop_after_inspection_v1(&mut backend);
}

#[test]
fn scripted_same_device_sdma_uses_authoritative_device_content_without_shadow_download() {
    let request = SameDeviceSdmaCopyRequestV1 {
        source_offset: 0,
        destination_offset: 0,
        copy_bytes: 8,
    };
    let mut steps = vec![
        scripted_same_device_submit_window_step_v1([request], ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::PollSameDevice(ScriptedSameDeviceExecutionOutcomeV1::Completed {
            copy_bytes: None,
            requests: None,
            swap_allocations: false,
        }),
        ScriptedSdmaStepV1::RetireSameDevice(ScriptedFailureModeV1::Success),
    ];
    steps.extend(scripted_same_device_release_steps_v1());
    let (mut backend, stream, source, destination) = scripted_same_device_backend_v1(8, steps);
    let expected = [2, 4, 6, 8, 10, 12, 14, 16];
    match &mut backend.allocations.get_mut(&source).unwrap().sdma_storage {
        KfdRuntimeSdmaStorageV1::Device(owner) => owner
            .scripted_bytes_mut()
            .unwrap()
            .copy_from_slice(&expected),
        _ => unreachable!("scripted source starts with device custody"),
    }
    backend
        .allocations
        .get_mut(&source)
        .unwrap()
        .sdma_shadow_dirty = true;
    backend
        .allocations
        .get_mut(&destination)
        .unwrap()
        .sdma_shadow_dirty = true;
    let (source_region, destination_region) =
        scripted_same_device_copy_regions_v1(source, destination, 8);

    let submission = backend
        .copy_async_v1(stream, source_region, destination_region, &[])
        .unwrap();
    assert!(matches!(
        &backend.active_sdma[&submission].phase,
        ActiveSdmaPhaseV1::SameDevicePublished(_)
    ));
    assert_eq!(
        backend.poll_v1(submission).unwrap(),
        BackendPollV1::Succeeded
    );
    let destination_bytes = match &backend.allocations[&destination].sdma_storage {
        KfdRuntimeSdmaStorageV1::Device(owner) => owner.scripted_bytes().unwrap(),
        _ => unreachable!("completed destination restores device custody"),
    };
    assert_eq!(destination_bytes, expected);
    assert!(backend.allocations[&source].sdma_shadow_dirty);
    assert!(backend.allocations[&destination].sdma_shadow_dirty);
    clean_scripted_same_device_backend_v1(
        &mut backend,
        stream,
        source,
        destination,
        Some(submission),
    );
}

#[test]
fn scripted_same_device_sdma_initial_retry_restores_both_allocations() {
    let mut steps = vec![scripted_same_device_submit_window_step_v1(
        [SameDeviceSdmaCopyRequestV1 {
            source_offset: 0,
            destination_offset: 0,
            copy_bytes: 8,
        }],
        ScriptedFailureModeV1::Retryable,
    )];
    steps.extend(scripted_same_device_release_steps_v1());
    let (mut backend, stream, source, destination) = scripted_same_device_backend_v1(8, steps);
    let (source_region, destination_region) =
        scripted_same_device_copy_regions_v1(source, destination, 8);
    let submission = backend
        .copy_async_v1(stream, source_region, destination_region, &[])
        .unwrap();

    assert_eq!(
        backend.poll_v1(submission).unwrap(),
        BackendPollV1::Failed {
            code: COOPERATIVE_COPY_FAILURE_CODE_V1
        }
    );
    for allocation in [source, destination] {
        assert!(matches!(
            backend.allocations[&allocation].sdma_storage,
            KfdRuntimeSdmaStorageV1::Device(_)
        ));
        assert!(!backend.allocations[&allocation].sdma_shadow_dirty);
    }
    clean_scripted_same_device_backend_v1(
        &mut backend,
        stream,
        source,
        destination,
        Some(submission),
    );
}

#[test]
fn scripted_same_device_sdma_clean_retry_after_progress_is_quiescent() {
    let mut steps = vec![scripted_same_device_submit_window_step_v1(
        [SameDeviceSdmaCopyRequestV1 {
            source_offset: 1,
            destination_offset: 1,
            copy_bytes: 7,
        }],
        ScriptedFailureModeV1::Retryable,
    )];
    steps.extend(scripted_same_device_release_steps_v1());
    let (mut backend, stream, source, destination) = scripted_same_device_backend_v1(8, steps);
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
    let (source_region, destination_region) =
        scripted_same_device_copy_regions_v1(source, destination, 8);
    let submission = backend
        .copy_async_v1(stream, source_region, destination_region, &[event])
        .unwrap();
    backend
        .active_sdma
        .get_mut(&submission)
        .unwrap()
        .completed_bytes = 1;
    backend.submissions.get_mut(&dependency).unwrap().status = BackendPollV1::Succeeded;

    assert!(matches!(
        backend.flush_stream_v1(stream),
        Err(RuntimeBackendFailureV1::Quiescent(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Native
    ));
    assert!(backend.quiescent_sdma_submissions.contains(&submission));
    assert!(backend.active_sdma.is_empty());
    for allocation in [source, destination] {
        assert!(matches!(
            backend.allocations[&allocation].sdma_storage,
            KfdRuntimeSdmaStorageV1::Device(_)
        ));
    }
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
fn scripted_same_device_sdma_prepublication_cancel_preserves_pair() {
    let steps = scripted_same_device_release_steps_v1();
    let (mut backend, stream, source, destination) = scripted_same_device_backend_v1(8, steps);
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
    let (source_region, destination_region) =
        scripted_same_device_copy_regions_v1(source, destination, 8);
    let submission = backend
        .copy_async_v1(stream, source_region, destination_region, &[event])
        .unwrap();

    assert_eq!(
        backend.cancel_v1(submission).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    for allocation in [source, destination] {
        assert!(matches!(
            backend.allocations[&allocation].sdma_storage,
            KfdRuntimeSdmaStorageV1::Device(_)
        ));
        assert!(!backend.allocations[&allocation].sdma_shadow_dirty);
    }
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
fn scripted_same_device_sdma_identity_changes_are_terminal_and_absorb_pair() {
    let canonical = SameDeviceSdmaCopyRequestV1 {
        source_offset: 0,
        destination_offset: 0,
        copy_bytes: 8,
    };
    let cases = [
        ScriptedSameDeviceExecutionOutcomeV1::Completed {
            copy_bytes: Some(7),
            requests: None,
            swap_allocations: false,
        },
        ScriptedSameDeviceExecutionOutcomeV1::Completed {
            copy_bytes: None,
            requests: Some(vec![SameDeviceSdmaCopyRequestV1 {
                destination_offset: 1,
                ..canonical
            }]),
            swap_allocations: false,
        },
        ScriptedSameDeviceExecutionOutcomeV1::Completed {
            copy_bytes: None,
            requests: None,
            swap_allocations: true,
        },
    ];
    let cases = cases
        .into_iter()
        .chain([ScriptedSameDeviceExecutionOutcomeV1::ProcessTeardown]);
    for outcome in cases {
        let steps = [
            scripted_same_device_submit_window_step_v1([canonical], ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::PollSameDevice(outcome),
        ];
        let (mut backend, stream, source, destination) = scripted_same_device_backend_v1(8, steps);
        let (source_region, destination_region) =
            scripted_same_device_copy_regions_v1(source, destination, 8);
        let submission = backend
            .copy_async_v1(stream, source_region, destination_region, &[])
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
fn scripted_sdma_clean_retry_after_prior_window_progress_is_quiescent() {
    let mut steps = vec![scripted_submit_step_v1(
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        1,
        1,
        7,
        ScriptedFailureModeV1::Retryable,
    )];
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
    backend
        .active_sdma
        .get_mut(&submission)
        .unwrap()
        .completed_bytes = 1;
    backend.submissions.get_mut(&dependency).unwrap().status = BackendPollV1::Succeeded;
    assert!(matches!(
        backend.flush_stream_v1(stream),
        Err(RuntimeBackendFailureV1::Quiescent(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Native
    ));
    assert!(backend.quiescent_sdma_submissions.contains(&submission));
    assert!(backend.active_sdma.is_empty());
    backend.release_event_v1(event).unwrap();
    backend.release_submission_v1(dependency).unwrap();
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
}

#[test]
fn scripted_sdma_poll_pending_and_completion_preserve_facade_owner() {
    let mut steps = vec![
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            8,
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
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
    let (source, destination) = scripted_copy_regions_v1(host, device, 8);
    let submission = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
    assert_eq!(
        backend.poll_v1(submission).unwrap(),
        BackendPollV1::Succeeded
    );
    assert!(backend.active_sdma.is_empty());
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
}
