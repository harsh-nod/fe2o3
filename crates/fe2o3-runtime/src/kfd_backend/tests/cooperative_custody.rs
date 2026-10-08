use super::*;

#[test]
fn multi_device_poll_and_expired_wait_leave_cooperative_copy_for_explicit_flush() {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    let stream = backend.create_stream_v1(7).unwrap();
    let source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let destination = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    backend
        .write_allocation_v1(source, 0, &[1, 2, 3, 4])
        .unwrap();
    let region = |allocation, access| BackendMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: 4,
    };
    let submission = backend
        .copy_async_v1(
            stream,
            region(source, RuntimeAccessV1::Read),
            region(destination, RuntimeAccessV1::Write),
            &[],
        )
        .unwrap();
    let destination_route = backend.allocations[&destination];
    let generation = backend.cooperative_progress_generation;
    assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
    assert_eq!(
        backend.wait_v1(submission, Instant::now()).unwrap(),
        BackendPollV1::Pending
    );
    assert_eq!(backend.cooperative_progress_generation, generation);
    assert!(
        backend.children[destination_route.child].allocations[&destination_route.local]
            .bytes
            .iter()
            .all(|byte| *byte == 0)
    );

    backend.flush_stream_v1(stream).unwrap();
    assert_eq!(
        backend.poll_v1(submission).unwrap(),
        BackendPollV1::Succeeded
    );
    backend.release_submission_v1(submission).unwrap();
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_stream_tail_does_not_consume_explicit_dependency_capacity() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    let mut events = Vec::new();
    for index in 0..MAX_RUNTIME_DEPENDENCIES_V1 {
        let submission = 1_000 + index as u64;
        let event = 2_000 + index as u64;
        backend.submissions.insert(
            submission,
            SubmissionRecordV1 {
                origin: SubmissionOriginV1::Ordinary,
                stream,
                status: BackendPollV1::Succeeded,
                dependency_depth: 1,
                profile_dispatch_published: false,
            },
        );
        backend.events.insert(event, EventRecordV1 { submission });
        events.push(event);
    }
    backend.stream_submission_tails.insert(stream, 9_999);
    backend.submissions.insert(
        9_999,
        SubmissionRecordV1 {
            origin: SubmissionOriginV1::Ordinary,
            stream,
            status: BackendPollV1::Succeeded,
            dependency_depth: 1,
            profile_dispatch_published: false,
        },
    );

    let explicit = backend.collect_compute_dependencies_v1(&events).unwrap();
    assert_eq!(explicit.len(), MAX_RUNTIME_DEPENDENCIES_V1);
    assert!(!explicit.contains(&9_999));

    backend.events.clear();
    backend.submissions.clear();
    backend.stream_submission_tails.clear();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_copy_stream_tail_cannot_exceed_dependency_bound() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    let source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let destination = backend
        .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
        .unwrap();
    for allocation in [source, destination] {
        let record = backend.allocations.get_mut(&allocation).unwrap();
        record.sdma_backed = true;
        record.sdma_initialized = true;
    }
    let mut events = Vec::new();
    for index in 0..MAX_RUNTIME_DEPENDENCIES_V1 {
        let submission = 1_000 + index as u64;
        let event = 2_000 + index as u64;
        backend.submissions.insert(
            submission,
            SubmissionRecordV1 {
                origin: SubmissionOriginV1::Ordinary,
                stream,
                status: BackendPollV1::Succeeded,
                dependency_depth: 1,
                profile_dispatch_published: false,
            },
        );
        backend.events.insert(event, EventRecordV1 { submission });
        events.push(event);
    }
    backend.stream_submission_tails.insert(stream, 9_999);
    backend.submissions.insert(
        9_999,
        SubmissionRecordV1 {
            origin: SubmissionOriginV1::Ordinary,
            stream,
            status: BackendPollV1::Succeeded,
            dependency_depth: 1,
            profile_dispatch_published: false,
        },
    );
    backend.native_available = true;
    let region = |allocation, access| BackendMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: 8,
    };

    assert!(matches!(
        backend.copy_async_v1(
            stream,
            region(source, RuntimeAccessV1::Read),
            region(destination, RuntimeAccessV1::Write),
            &events,
        ),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
    ));
    assert!(backend.active_sdma.is_empty());

    backend.native_available = false;
    for allocation in [source, destination] {
        backend
            .allocations
            .get_mut(&allocation)
            .unwrap()
            .sdma_backed = false;
    }
    backend.events.clear();
    backend.submissions.clear();
    backend.stream_submission_tails.clear();
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_copy_cannot_pass_unpublished_cross_stream_compute() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let compute_stream = backend.create_stream_v1(7).unwrap();
    let copy_stream = backend.create_stream_v1(7).unwrap();
    let source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let destination = backend
        .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
        .unwrap();
    for allocation in [source, destination] {
        let record = backend.allocations.get_mut(&allocation).unwrap();
        record.sdma_backed = true;
        record.sdma_initialized = true;
    }
    backend.compute_completion_reservations = 1;
    backend
        .pending_compute_streams
        .insert(compute_stream, VecDeque::from([40]));
    backend.pending_compute.insert(
        40,
        pending_compute_for_test_v1(40, compute_stream, source, vec![]),
    );
    index_pending_compute_custody_for_test_v1(&mut backend, 40);
    backend.stream_submission_tails.insert(compute_stream, 40);
    backend.native_available = true;
    let region = |allocation, access| BackendMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: 8,
    };

    assert!(matches!(
        backend.copy_async_v1(
            copy_stream,
            region(source, RuntimeAccessV1::Read),
            region(destination, RuntimeAccessV1::Write),
            &[],
        ),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
    ));
    assert!(backend.active_sdma.is_empty());

    backend.native_available = false;
    backend.cancel_v1(40).unwrap();
    backend.release_submission_v1(40).unwrap();
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(compute_stream).unwrap();
    backend.destroy_stream_v1(copy_stream).unwrap();
    release_pending_compute_test_resources_v1(&mut backend);
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_active_compute_custody_is_exact_per_lane() {
    fn active(id: u64, stream: u64, allocation: u64) -> ActiveSubmissionV1 {
        ActiveSubmissionV1 {
            source_event: Default::default(),
            id,
            stream,
            ordered_predecessor: None,
            deferred_ordered_predecessor_retain: false,
            kernel: 9,
            dependency_depth: 1,
            allocations: HashSet::from([allocation]),
            writebacks: Vec::new(),
            resident_descriptors: Vec::new(),
            ordinary_recipe: None,
            dispatch_shape_sha256: [0; 32],
            published_at: Instant::now(),
            performance: KfdRuntimeLaunchPerformanceV1::default(),
            execution: None,
        }
    }

    let mut backend = KfdRuntimeBackendV1::mock();
    backend.active = Some(active(11, 1, 101));
    backend.auxiliary_compute_lanes[0].active = Some(active(12, 2, 202));
    for (submission, stream, allocation) in [(11, 1, 101), (12, 2, 202)] {
        let new_entries = backend
            .reserve_allocation_custody_v1(&[allocation])
            .unwrap();
        backend.retain_allocation_custody_v1(
            &[allocation],
            RuntimeAllocationCustodyOwnerV1 {
                submission,
                stream,
                kind: RuntimeAllocationCustodyKindV1::Compute,
            },
            new_entries,
        );
    }
    assert_eq!(backend.active_compute_lane_v1(11), Some(0));
    assert_eq!(backend.active_compute_lane_v1(12), Some(1));
    assert!(backend.allocation_is_active(101));
    assert!(backend.allocation_is_active(202));
    assert!(!backend.allocation_is_active(303));
    let disjoint = [BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation: 303,
            access: RuntimeAccessV1::ReadWrite,
            byte_offset: 0,
            byte_len: 8,
        },
        kernarg_byte_offset: 0,
    }];
    let conflicting = [BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation: 202,
            access: RuntimeAccessV1::ReadWrite,
            byte_offset: 0,
            byte_len: 8,
        },
        kernarg_byte_offset: 0,
    }];
    let active = backend.active.iter().chain(
        backend
            .auxiliary_compute_lanes
            .iter()
            .filter_map(|lane| lane.active.as_ref()),
    );
    assert!(!launch_overlaps_active_compute_v1(&disjoint, active));
    let active = backend.active.iter().chain(
        backend
            .auxiliary_compute_lanes
            .iter()
            .filter_map(|lane| lane.active.as_ref()),
    );
    assert!(launch_overlaps_active_compute_v1(&conflicting, active));

    backend.with_compute_lane_state_v1(1, |selected| {
        assert_eq!(selected.active.as_ref().map(|active| active.id), Some(12));
    });
    let failed: Result<(), &'static str> = backend.with_compute_lane_state_v1(1, |selected| {
        assert_eq!(selected.active.as_ref().map(|active| active.id), Some(12));
        Err("injected lane-local rejection")
    });
    assert_eq!(failed, Err("injected lane-local rejection"));
    assert_eq!(backend.active.as_ref().map(|active| active.id), Some(11));
    assert_eq!(
        backend.auxiliary_compute_lanes[0]
            .active
            .as_ref()
            .map(|active| active.id),
        Some(12)
    );
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        backend.with_compute_lane_state_v1(1, |_| panic!("injected lane-local panic"));
    }));
    assert!(panicked.is_err());
    assert_eq!(backend.active.as_ref().map(|active| active.id), Some(11));
    assert_eq!(
        backend.auxiliary_compute_lanes[0]
            .active
            .as_ref()
            .map(|active| active.id),
        Some(12)
    );
    backend.active = None;
    backend.auxiliary_compute_lanes[0].active = None;
    backend.release_allocation_custody_v1(101, 11);
    backend.release_allocation_custody_v1(202, 12);
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn cooperative_copy_dependency_retains_prior_submission_until_completion() {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    let stream = backend.create_stream_v1(7).unwrap();
    let first_source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let shared = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let final_destination = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    backend
        .write_allocation_v1(first_source, 0, &[1, 3, 3, 7])
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
            region(first_source, RuntimeAccessV1::Read),
            region(shared, RuntimeAccessV1::Write),
            &[],
        )
        .unwrap();
    let event = backend.record_event_v1(stream, first).unwrap();
    let second = backend
        .copy_async_v1(
            stream,
            region(shared, RuntimeAccessV1::Read),
            region(final_destination, RuntimeAccessV1::Write),
            &[event],
        )
        .unwrap();
    assert_eq!(backend.poll_v1(second).unwrap(), BackendPollV1::Pending);
    backend.release_event_v1(event).unwrap();
    assert!(matches!(
        backend.release_submission_v1(first),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
    ));
    backend.flush_stream_v1(stream).unwrap();
    assert_eq!(
        backend
            .wait_v1(second, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(backend.poll_v1(first).unwrap(), BackendPollV1::Succeeded);
    backend.release_submission_v1(first).unwrap();
    backend.release_submission_v1(second).unwrap();
    let mut observed = [0_u8; 4];
    backend
        .read_allocation_v1(final_destination, 0, &mut observed)
        .unwrap();
    assert_eq!(observed, [1, 3, 3, 7]);
    for allocation in [first_source, shared, final_destination] {
        backend.release_allocation_v1(allocation).unwrap();
    }
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn cooperative_same_stream_overlap_uses_transitive_fifo_tail() {
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
    backend
        .write_allocation_v1(allocations[0], 0, &[4, 3, 2, 1])
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
            region(allocations[1], RuntimeAccessV1::Read),
            region(allocations[2], RuntimeAccessV1::Write),
            &[],
        )
        .unwrap();
    let third = backend
        .copy_async_v1(
            stream,
            region(allocations[2], RuntimeAccessV1::Read),
            region(allocations[3], RuntimeAccessV1::Write),
            &[],
        )
        .unwrap();
    assert!(matches!(
        &backend.submissions[&second],
        RoutedSubmissionV1::CooperativeCopy(copy) if copy.dependencies == [first]
    ));
    assert!(matches!(
        &backend.submissions[&third],
        RoutedSubmissionV1::CooperativeCopy(copy) if copy.dependencies == [second]
    ));
    assert_eq!(backend.poll_v1(third).unwrap(), BackendPollV1::Pending);
    backend.flush_stream_v1(stream).unwrap();
    for submission in [first, second, third] {
        assert_eq!(
            backend.poll_v1(submission).unwrap(),
            BackendPollV1::Succeeded
        );
        backend.release_submission_v1(submission).unwrap();
    }
    let mut observed = [0_u8; 4];
    backend
        .read_allocation_v1(allocations[3], 0, &mut observed)
        .unwrap();
    assert_eq!(observed, [4, 3, 2, 1]);
    for allocation in allocations {
        backend.release_allocation_v1(allocation).unwrap();
    }
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn native_copy_rejects_parent_cooperative_allocation_custody_before_child_call() {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    let cooperative_stream = backend.create_stream_v1(7).unwrap();
    let native_stream = backend.create_stream_v1(7).unwrap();
    let source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let destination = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let cooperative = backend
        .copy_async_v1(
            cooperative_stream,
            BackendMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 4,
            },
            BackendMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: 4,
            },
            &[],
        )
        .unwrap();
    let child = backend.allocations[&source].child;
    let child_next_handle = backend.children[child].next_handle;
    backend.children[child].native_available = true;
    assert!(matches!(
        backend.copy_async_v1(
            native_stream,
            BackendMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 4,
            },
            BackendMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: 4,
            },
            &[],
        ),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
    ));
    assert_eq!(backend.children[child].next_handle, child_next_handle);
    backend.children[child].native_available = false;
    backend.flush_stream_v1(cooperative_stream).unwrap();
    backend.release_submission_v1(cooperative).unwrap();
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(cooperative_stream).unwrap();
    backend.destroy_stream_v1(native_stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn cooperative_copy_indexes_track_fan_out_and_quiescence_exactly() {
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
    backend.assert_cooperative_indexes_consistent();
    assert_eq!(backend.cooperative_stream_pending_counts[&stream], 1);

    let first_event = backend.record_event_v1(stream, first).unwrap();
    let second_event = backend.record_event_v1(stream, first).unwrap();
    backend.assert_cooperative_indexes_consistent();
    assert_eq!(backend.event_submission_retain_counts[&first], 2);

    let second = backend
        .copy_async_v1(
            stream,
            region(allocations[2], RuntimeAccessV1::Read),
            region(allocations[3], RuntimeAccessV1::Write),
            &[first_event],
        )
        .unwrap();
    let third = backend
        .copy_async_v1(
            stream,
            region(allocations[4], RuntimeAccessV1::Read),
            region(allocations[5], RuntimeAccessV1::Write),
            &[second_event],
        )
        .unwrap();
    backend.assert_cooperative_indexes_consistent();
    assert_eq!(backend.cooperative_dependency_retain_counts[&first], 2);
    assert_eq!(backend.cooperative_stream_pending_counts[&stream], 3);

    backend.release_event_v1(first_event).unwrap();
    backend.assert_cooperative_indexes_consistent();
    assert_eq!(backend.event_submission_retain_counts[&first], 1);
    backend.release_event_v1(second_event).unwrap();
    backend.assert_cooperative_indexes_consistent();
    assert!(!backend.event_submission_retain_counts.contains_key(&first));
    assert!(matches!(
        backend.release_submission_v1(first),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
    ));

    while backend.progress_cooperative_copy(second).unwrap() == BackendPollV1::Pending {}
    backend.assert_cooperative_indexes_consistent();
    assert_eq!(backend.cooperative_dependency_retain_counts[&first], 1);
    assert_eq!(backend.cooperative_stream_pending_counts[&stream], 1);
    assert!(matches!(
        backend.release_submission_v1(first),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
    ));

    while backend.progress_cooperative_copy(third).unwrap() == BackendPollV1::Pending {}
    backend.assert_cooperative_indexes_consistent();
    assert!(backend.cooperative_allocation_owners.is_empty());
    assert!(backend.cooperative_dependency_retain_counts.is_empty());
    assert!(backend.cooperative_stream_pending_counts.is_empty());
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
fn multi_device_cancellation_before_destination_write_releases_exact_custody() {
    fn requires_worker_v4_backend<B>()
    where
        B: RuntimeBackendV1
            + RuntimeAsyncCopyBackendV1
            + RuntimeFlushBackendV1
            + RuntimeCancellationBackendV1,
    {
    }
    requires_worker_v4_backend::<KfdMultiDeviceRuntimeBackendV1>();

    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    assert!(backend.execution_capabilities_v1(7).cancellation);
    let stream = backend.create_stream_v1(7).unwrap();
    let source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let destination = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let region = |allocation, access| BackendMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: 8,
    };
    let submission = backend
        .copy_async_v1(
            stream,
            region(source, RuntimeAccessV1::Read),
            region(destination, RuntimeAccessV1::Write),
            &[],
        )
        .unwrap();
    let progress_before = backend.cooperative_progress_generation;
    assert_eq!(
        backend.drain_v1(submission, Instant::now()).unwrap(),
        BackendPollV1::Pending
    );
    assert_eq!(backend.cooperative_progress_generation, progress_before);

    assert_eq!(
        backend.cancel_v1(submission).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert_eq!(
        backend.poll_v1(submission).unwrap(),
        BackendPollV1::Failed { code: -2 }
    );
    assert_eq!(backend.cooperative_staging_bytes, 0);
    assert!(!backend.cooperative_stream_tails.contains_key(&stream));
    backend.assert_cooperative_indexes_consistent();

    let replacement = backend
        .copy_async_v1(
            stream,
            region(source, RuntimeAccessV1::Read),
            region(destination, RuntimeAccessV1::Write),
            &[],
        )
        .unwrap();
    assert_eq!(
        backend.cancel_v1(replacement).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    backend.assert_cooperative_indexes_consistent();
    backend.release_submission_v1(submission).unwrap();
    backend.release_submission_v1(replacement).unwrap();
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn multi_device_cancellation_is_too_late_after_first_destination_write() {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    let stream = backend.create_stream_v1(8).unwrap();
    let byte_len = u64::try_from(COOPERATIVE_COPY_CHUNK_BYTES_V1 + 1).unwrap();
    let source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, byte_len, 8)
        .unwrap();
    let destination = backend
        .allocate_v1(8, RuntimeMemoryKindV1::HostVisible, byte_len, 8)
        .unwrap();
    let submission = backend
        .peer_copy_v1(
            stream,
            BackendMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len,
            },
            BackendMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len,
            },
            &[],
        )
        .unwrap();

    for _ in 0..8 {
        let first_write_completed = matches!(
            &backend.submissions[&submission],
            RoutedSubmissionV1::CooperativeCopy(copy)
                if copy.phase == CooperativeCopyPhaseV1::Write && copy.byte_cursor != 0
        );
        if first_write_completed {
            break;
        }
        assert_eq!(
            backend.progress_cooperative_copy(submission).unwrap(),
            BackendPollV1::Pending
        );
    }
    assert!(matches!(
        &backend.submissions[&submission],
        RoutedSubmissionV1::CooperativeCopy(copy)
            if copy.phase == CooperativeCopyPhaseV1::Write
                && copy.byte_cursor == COOPERATIVE_COPY_CHUNK_BYTES_V1
    ));
    assert_eq!(
        backend.cancel_v1(submission).unwrap(),
        crate::BackendCancellationV1::TooLate
    );
    assert_eq!(backend.cooperative_staging_bytes, byte_len);
    backend.assert_cooperative_indexes_consistent();

    assert_eq!(
        backend
            .drain_v1(submission, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    backend.assert_cooperative_indexes_consistent();
    backend.release_submission_v1(submission).unwrap();
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}
