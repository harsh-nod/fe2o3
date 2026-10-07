use super::*;

#[test]
fn cooperative_staging_budget_rejects_before_publication_and_releases_at_quiescence() {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    backend.cooperative_staging_limit_bytes = 8;
    let stream = backend.create_stream_v1(7).unwrap();
    let allocations = (0..6)
        .map(|_| {
            backend
                .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let region = |allocation, access, byte_len| BackendMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len,
    };
    let first = backend
        .copy_async_v1(
            stream,
            region(allocations[0], RuntimeAccessV1::Read, 4),
            region(allocations[1], RuntimeAccessV1::Write, 4),
            &[],
        )
        .unwrap();
    let second = backend
        .copy_async_v1(
            stream,
            region(allocations[2], RuntimeAccessV1::Read, 4),
            region(allocations[3], RuntimeAccessV1::Write, 4),
            &[],
        )
        .unwrap();
    assert_eq!(backend.cooperative_staging_bytes, 8);
    backend.assert_cooperative_indexes_consistent();

    let submissions_before = backend.submissions.len();
    let next_handle_before = backend.next_handle;
    let allocation_owners_before = backend.cooperative_allocation_owners.clone();
    let dependency_counts_before = backend.cooperative_dependency_retain_counts.clone();
    let stream_counts_before = backend.cooperative_stream_pending_counts.clone();
    let event_counts_before = backend.event_submission_retain_counts.clone();
    let events_before = backend.events.len();
    assert!(matches!(
        backend.copy_async_v1(
            stream,
            region(allocations[4], RuntimeAccessV1::Read, 1),
            region(allocations[5], RuntimeAccessV1::Write, 1),
            &[],
        ),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
    ));
    assert_eq!(backend.submissions.len(), submissions_before);
    assert_eq!(backend.next_handle, next_handle_before);
    assert_eq!(backend.cooperative_staging_bytes, 8);
    assert_eq!(
        backend.cooperative_allocation_owners,
        allocation_owners_before
    );
    assert_eq!(
        backend.cooperative_dependency_retain_counts,
        dependency_counts_before
    );
    assert_eq!(
        backend.cooperative_stream_pending_counts,
        stream_counts_before
    );
    assert_eq!(backend.event_submission_retain_counts, event_counts_before);
    assert_eq!(backend.events.len(), events_before);
    backend.assert_cooperative_indexes_consistent();

    while backend.progress_cooperative_copy(first).unwrap() == BackendPollV1::Pending {}
    assert_eq!(backend.cooperative_staging_bytes, 4);
    assert!(matches!(
        &backend.submissions[&first],
        RoutedSubmissionV1::CooperativeCopy(copy) if copy.staging.is_empty()
    ));
    backend.assert_cooperative_indexes_consistent();

    let third = backend
        .copy_async_v1(
            stream,
            region(allocations[4], RuntimeAccessV1::Read, 1),
            region(allocations[5], RuntimeAccessV1::Write, 1),
            &[],
        )
        .unwrap();
    assert_eq!(backend.cooperative_staging_bytes, 5);
    backend.flush_stream_v1(stream).unwrap();
    for submission in [second, third] {
        assert_eq!(
            backend.poll_v1(submission).unwrap(),
            BackendPollV1::Succeeded
        );
    }
    assert_eq!(backend.cooperative_staging_bytes, 0);
    backend.assert_cooperative_indexes_consistent();

    for submission in [first, second, third] {
        backend.release_submission_v1(submission).unwrap();
    }
    for allocation in allocations {
        backend.release_allocation_v1(allocation).unwrap();
    }
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn cooperative_copy_index_overflow_rejects_before_publication() {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    let stream = backend.create_stream_v1(7).unwrap();
    let allocations = (0..4)
        .map(|_| {
            backend
                .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let region = |allocation, access| BackendMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: 4,
    };
    let first = backend
        .copy_async_v1(
            stream,
            region(allocations[0], RuntimeAccessV1::Read),
            region(allocations[1], RuntimeAccessV1::Write),
            &[],
        )
        .unwrap();
    let submissions_before = backend.submissions.len();
    let next_handle_before = backend.next_handle;
    let owners_before = backend.cooperative_allocation_owners.clone();

    backend
        .cooperative_stream_pending_counts
        .insert(stream, usize::MAX);
    assert!(matches!(
        backend.copy_async_v1(
            stream,
            region(allocations[2], RuntimeAccessV1::Read),
            region(allocations[3], RuntimeAccessV1::Write),
            &[],
        ),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
    ));
    assert_eq!(backend.submissions.len(), submissions_before);
    assert_eq!(backend.next_handle, next_handle_before);
    assert_eq!(backend.cooperative_allocation_owners, owners_before);
    backend.cooperative_stream_pending_counts.insert(stream, 1);
    backend.assert_cooperative_indexes_consistent();

    backend
        .event_submission_retain_counts
        .insert(first, usize::MAX);
    assert!(matches!(
        backend.record_event_v1(stream, first),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
    ));
    assert!(backend.events.is_empty());
    assert_eq!(backend.next_handle, next_handle_before);
    backend.event_submission_retain_counts.remove(&first);
    backend.assert_cooperative_indexes_consistent();

    let event = backend.record_event_v1(stream, first).unwrap();
    let next_handle_before = backend.next_handle;
    backend
        .cooperative_dependency_retain_counts
        .insert(first, usize::MAX);
    assert!(matches!(
        backend.copy_async_v1(
            stream,
            region(allocations[2], RuntimeAccessV1::Read),
            region(allocations[3], RuntimeAccessV1::Write),
            &[event],
        ),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
    ));
    assert_eq!(backend.submissions.len(), submissions_before);
    assert_eq!(backend.next_handle, next_handle_before);
    assert_eq!(backend.cooperative_allocation_owners, owners_before);
    backend.cooperative_dependency_retain_counts.remove(&first);
    backend.assert_cooperative_indexes_consistent();

    backend.release_event_v1(event).unwrap();
    backend.flush_stream_v1(stream).unwrap();
    assert_eq!(
        backend
            .wait_v1(first, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    backend.assert_cooperative_indexes_consistent();
    backend.release_submission_v1(first).unwrap();
    for allocation in allocations {
        backend.release_allocation_v1(allocation).unwrap();
    }
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn cooperative_copy_dependency_depth_is_bounded_before_publication() {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    let stream = backend.create_stream_v1(7).unwrap();
    let mut allocations = Vec::new();
    let mut submissions = Vec::new();
    let mut dependency_event = None;
    let region = |allocation, access| BackendMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: 1,
    };

    for expected_depth in 1..=MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 1, 1)
            .unwrap();
        let destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 1, 1)
            .unwrap();
        let dependencies = dependency_event.as_slice();
        let submission = backend
            .copy_async_v1(
                stream,
                region(source, RuntimeAccessV1::Read),
                region(destination, RuntimeAccessV1::Write),
                dependencies,
            )
            .unwrap();
        assert!(matches!(
            &backend.submissions[&submission],
            RoutedSubmissionV1::CooperativeCopy(copy)
                if copy.dependency_depth == expected_depth
        ));
        if let Some(event) =
            dependency_event.replace(backend.record_event_v1(stream, submission).unwrap())
        {
            backend.release_event_v1(event).unwrap();
        }
        allocations.extend([source, destination]);
        submissions.push(submission);
    }
    backend.assert_cooperative_indexes_consistent();

    let rejected_source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 1, 1)
        .unwrap();
    let rejected_destination = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 1, 1)
        .unwrap();
    let submissions_before = backend.submissions.len();
    let next_handle_before = backend.next_handle;
    let allocation_owners_before = backend.cooperative_allocation_owners.clone();
    let dependency_counts_before = backend.cooperative_dependency_retain_counts.clone();
    let stream_counts_before = backend.cooperative_stream_pending_counts.clone();
    let event_counts_before = backend.event_submission_retain_counts.clone();
    assert!(matches!(
        backend.copy_async_v1(
            stream,
            region(rejected_source, RuntimeAccessV1::Read),
            region(rejected_destination, RuntimeAccessV1::Write),
            dependency_event.as_slice(),
        ),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
    ));
    assert_eq!(backend.submissions.len(), submissions_before);
    assert_eq!(backend.next_handle, next_handle_before);
    assert_eq!(
        backend.cooperative_allocation_owners,
        allocation_owners_before
    );
    assert_eq!(
        backend.cooperative_dependency_retain_counts,
        dependency_counts_before
    );
    assert_eq!(
        backend.cooperative_stream_pending_counts,
        stream_counts_before
    );
    assert_eq!(backend.event_submission_retain_counts, event_counts_before);
    backend.assert_cooperative_indexes_consistent();
    backend.release_allocation_v1(rejected_source).unwrap();
    backend.release_allocation_v1(rejected_destination).unwrap();

    let last = *submissions.last().unwrap();
    backend.flush_stream_v1(stream).unwrap();
    assert_eq!(
        backend
            .wait_v1(last, Instant::now() + Duration::from_secs(2))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    backend.release_event_v1(dependency_event.unwrap()).unwrap();
    backend.assert_cooperative_indexes_consistent();
    assert!(backend.cooperative_allocation_owners.is_empty());
    assert!(backend.cooperative_dependency_retain_counts.is_empty());
    assert!(backend.cooperative_stream_pending_counts.is_empty());
    assert!(backend.event_submission_retain_counts.is_empty());
    for submission in submissions {
        backend.release_submission_v1(submission).unwrap();
    }
    for allocation in allocations {
        backend.release_allocation_v1(allocation).unwrap();
    }
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn cooperative_copy_poll_is_observational_and_flush_drives_fifo_fan_in() {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    let stream = backend.create_stream_v1(7).unwrap();
    let allocations = (0..6)
        .map(|_| {
            backend
                .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
                .unwrap()
        })
        .collect::<Vec<_>>();
    backend
        .write_allocation_v1(allocations[0], 0, &[1, 2, 3, 4])
        .unwrap();
    backend
        .write_allocation_v1(allocations[2], 0, &[5, 6, 7, 8])
        .unwrap();
    backend
        .write_allocation_v1(allocations[4], 0, &[9, 10, 11, 12])
        .unwrap();
    let region = |allocation, access| BackendMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: 4,
    };
    let first = backend
        .copy_async_v1(
            stream,
            region(allocations[0], RuntimeAccessV1::Read),
            region(allocations[1], RuntimeAccessV1::Write),
            &[],
        )
        .unwrap();
    let second = backend
        .copy_async_v1(
            stream,
            region(allocations[2], RuntimeAccessV1::Read),
            region(allocations[3], RuntimeAccessV1::Write),
            &[],
        )
        .unwrap();
    for submission in [first, second] {
        assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
        assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
    }
    let first_event = backend.record_event_v1(stream, first).unwrap();
    let second_event = backend.record_event_v1(stream, second).unwrap();
    let dependent = backend
        .copy_async_v1(
            stream,
            region(allocations[4], RuntimeAccessV1::Read),
            region(allocations[5], RuntimeAccessV1::Write),
            &[first_event, second_event],
        )
        .unwrap();

    let generation = backend.cooperative_progress_generation;
    assert_eq!(backend.poll_v1(dependent).unwrap(), BackendPollV1::Pending);
    assert_eq!(backend.cooperative_progress_generation, generation);
    for submission in [first, second, dependent] {
        assert!(matches!(
            &backend.submissions[&submission],
            RoutedSubmissionV1::CooperativeCopy(copy)
                if copy.status() == BackendPollV1::Pending
        ));
    }
    let second_destination = backend.allocations[&allocations[3]];
    assert!(
        backend.children[second_destination.child].allocations[&second_destination.local]
            .bytes
            .iter()
            .all(|byte| *byte == 0)
    );

    assert_eq!(
        backend.wait_v1(dependent, Instant::now()).unwrap(),
        BackendPollV1::Pending
    );
    assert_eq!(backend.cooperative_progress_generation, generation);
    backend.flush_stream_v1(stream).unwrap();
    for submission in [first, second, dependent] {
        assert_eq!(
            backend.poll_v1(submission).unwrap(),
            BackendPollV1::Succeeded
        );
    }
    backend.release_event_v1(first_event).unwrap();
    backend.release_event_v1(second_event).unwrap();
    for submission in [first, second, dependent] {
        backend.release_submission_v1(submission).unwrap();
    }
    for allocation in allocations {
        backend.release_allocation_v1(allocation).unwrap();
    }
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn cooperative_copy_terminal_failure_latches_and_retains_custody() {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    let stream = backend.create_stream_v1(8).unwrap();
    let source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let destination = backend
        .allocate_v1(8, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let submission = backend
        .peer_copy_v1(
            stream,
            BackendMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 8,
            },
            BackendMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: 8,
            },
            &[],
        )
        .unwrap();
    assert_eq!(backend.cooperative_staging_bytes, 8);
    assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
    let source_child = backend.allocations[&source].child;
    backend.children[source_child].terminal = true;
    assert!(matches!(
        backend.flush_stream_v1(stream),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(backend.terminal);
    assert_eq!(backend.cooperative_staging_bytes, 8);
    assert!(backend.submissions.contains_key(&submission));
    assert!(matches!(
        backend.poll_v1(submission),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));

    // Private test-only repair prevents the mock child's fail-closed Drop
    // path from aborting the test process; production has no reset API.
    backend.children[source_child].terminal = false;
    backend.terminal = false;
    backend.finish_cooperative_copy(submission, CooperativeCopyPhaseV1::Failed);
    assert_eq!(backend.cooperative_staging_bytes, 0);
    backend.assert_cooperative_indexes_consistent();
    backend.release_submission_v1(submission).unwrap();
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn multi_device_router_latches_a_child_terminal_failure_globally() {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    backend.children[0].terminal = true;
    assert!(matches!(
        backend.enumerate_devices_v1(),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    backend.children[0].terminal = false;
    assert!(matches!(
        backend.create_stream_v1(8),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    backend.terminal = false;
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn multi_device_router_rejects_invalid_peer_access_before_copy() {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    let stream = backend.create_stream_v1(8).unwrap();
    let source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let destination = backend
        .allocate_v1(8, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let region = |allocation, access| BackendMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: 8,
    };
    assert!(matches!(
        backend.peer_copy_v1(
            stream,
            region(source, RuntimeAccessV1::Write),
            region(destination, RuntimeAccessV1::Read),
            &[],
        ),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
    ));
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn runtime_context_composes_multi_device_peer_copy_and_cleanup() {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    let mut context = crate::RuntimeContextV1::open(backend).unwrap();
    let source_device = context.devices()[0].id();
    let destination_device = context.devices()[1].id();
    let stream = context.create_stream(destination_device).unwrap();
    let source = context
        .allocate(source_device, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let destination = context
        .allocate(destination_device, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    context
        .write_allocation(source, 0, &[1, 2, 3, 4, 5, 6, 7, 8])
        .unwrap();
    let mut submission = context
        .peer_copy(
            stream,
            crate::RuntimeMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 8,
            },
            crate::RuntimeMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: 8,
            },
            &[],
        )
        .unwrap();
    context.flush_stream(stream).unwrap();
    assert_eq!(
        context
            .wait(&mut submission, Duration::from_secs(1))
            .unwrap(),
        crate::RuntimePollV1::Succeeded
    );
    let mut observed = [0_u8; 8];
    context
        .read_allocation(destination, 0, &mut observed)
        .unwrap();
    assert_eq!(observed, [1, 2, 3, 4, 5, 6, 7, 8]);
    context.release_submission(submission).unwrap();
    context.release_allocation(source).unwrap();
    context.release_allocation(destination).unwrap();
    context.destroy_stream(stream).unwrap();
    let mut backend = context.shutdown().unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn multi_device_router_rejects_peer_copy_on_the_source_stream() {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    let left_stream = backend.create_stream_v1(7).unwrap();
    let source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let destination = backend
        .allocate_v1(8, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let region = |allocation, access| BackendMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: 8,
    };
    assert!(matches!(
        backend.peer_copy_v1(
            left_stream,
            region(source, RuntimeAccessV1::Read),
            region(destination, RuntimeAccessV1::Write),
            &[],
        ),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
    ));
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(left_stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn multi_device_route_exhaustion_precedes_child_mutation() {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    backend.next_handle = u64::MAX;
    assert!(matches!(
        backend.create_stream_v1(7),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
    ));
    assert!(backend.streams.is_empty());
    assert!(backend.children[0].streams.is_empty());
}
