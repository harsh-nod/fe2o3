use super::*;

#[test]
fn direct_kfd_cancels_only_an_unpublished_dependency_waiter() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    let source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let destination = backend
        .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
        .unwrap();
    backend.submissions.insert(
        40,
        SubmissionRecordV1 {
            origin: SubmissionOriginV1::Ordinary,
            stream,
            status: BackendPollV1::Pending,
            dependency_depth: 1,
            profile_dispatch_published: false,
        },
    );
    backend.sdma_dependency_retain_counts.insert(40, 1);
    backend.active_sdma.insert(
        41,
        ActiveSdmaCopyV1 {
            id: 41,
            stream,
            prior_stream_submission: Some(40),
            source,
            destination,
            source_offset: 0,
            destination_offset: 0,
            byte_len: 8,
            completed_bytes: 0,
            window_bytes: 0,
            window_requests: None,
            dependencies: vec![40],
            dependency_cursor: 0,
            dependency_depth: 1,
            peer_access: None,
            phase: ActiveSdmaPhaseV1::Ready,
        },
    );
    index_sdma_custody_for_test_v1(&mut backend, 41);

    assert_eq!(
        backend.cancel_v1(41).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert!(!backend.active_sdma.contains_key(&41));
    assert!(backend.sdma_dependency_retain_counts.is_empty());
    assert_eq!(
        backend.submissions[&41].status,
        BackendPollV1::Failed { code: -2 }
    );
    backend.release_submission_v1(40).unwrap();
    backend.release_submission_v1(41).unwrap();
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_execution_capabilities_claim_native_queue_concurrency() {
    let mut backend = KfdRuntimeBackendV1::mock();
    assert_eq!(
        backend
            .sdma_memory_pool_observation_v1()
            .unwrap_err()
            .kind(),
        KfdRuntimeBackendErrorKindV1::Unsupported
    );
    assert_eq!(
        backend.execution_capabilities_v1(7),
        RuntimeExecutionCapabilitiesV1::default()
    );
    backend.native_available = true;
    let capabilities = backend.execution_capabilities_v1(7);
    assert!(capabilities.native_async_copy);
    assert!(capabilities.memory_pool);
    assert!(capabilities.cancellation);
    assert!(!capabilities.native_peer_copy);
    assert!(capabilities.concurrent_compute);
    assert!(capabilities.compute_copy_overlap);
    backend.native_available = false;

    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut multi = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    multi.children[0].native_available = true;
    let capabilities = multi.execution_capabilities_v1(7);
    assert!(capabilities.native_async_copy);
    assert!(capabilities.concurrent_compute);
    assert!(capabilities.compute_copy_overlap);
    assert!(capabilities.cancellation);
    multi.children[0].native_available = false;
    multi.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_logical_streams_lease_two_native_lanes_deterministically() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let first = backend.create_stream_v1(7).unwrap();
    let second = backend.create_stream_v1(7).unwrap();
    let third = backend.create_stream_v1(7).unwrap();
    assert!(backend.stream_compute_lanes.is_empty());

    let first_lane = backend.free_compute_lane_v1().unwrap();
    assert_eq!(first_lane, 0);
    backend.lease_compute_lane_v1(third, first_lane);
    let second_lane = backend.free_compute_lane_v1().unwrap();
    assert_eq!(second_lane, 1);
    backend.lease_compute_lane_v1(first, second_lane);
    assert_eq!(backend.free_compute_lane_v1(), None);
    assert_eq!(backend.auxiliary_compute_lanes[0].owner_stream, Some(first));

    backend.release_compute_lane_lease_v1(third, 0);
    assert_eq!(backend.free_compute_lane_v1(), Some(0));
    backend.release_compute_lane_lease_v1(first, 1);
    assert!(backend.stream_compute_lanes.is_empty());
    assert_eq!(backend.auxiliary_compute_lanes[0].owner_stream, None);
    backend.destroy_stream_v1(first).unwrap();
    backend.destroy_stream_v1(second).unwrap();
    backend.destroy_stream_v1(third).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_active_sdma_stream_index_retains_and_releases_fifo() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    for submission in [40, 41, 42] {
        let queue = backend.reserve_active_sdma_stream_v1(stream).unwrap();
        backend.retain_active_sdma_stream_v1(stream, submission, queue);
    }
    assert_eq!(
        backend.active_sdma_streams[&stream],
        VecDeque::from([40, 41, 42])
    );

    backend.release_active_sdma_stream_v1(stream, 41);
    assert_eq!(
        backend.active_sdma_streams[&stream],
        VecDeque::from([40, 42])
    );
    backend.release_active_sdma_stream_v1(stream, 40);
    assert_eq!(backend.active_sdma_streams[&stream], VecDeque::from([42]));
    backend.release_active_sdma_stream_v1(stream, 42);
    assert!(backend.active_sdma_streams.is_empty());

    let queue = backend.reserve_active_sdma_stream_v1(stream).unwrap();
    backend.retain_active_sdma_stream_v1(stream, 43, queue);
    assert_eq!(backend.active_sdma_streams[&stream], VecDeque::from([43]));
    backend.release_active_sdma_stream_v1(stream, 43);
    assert!(backend.active_sdma_streams.is_empty());
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_allocation_custody_is_bounded_and_fifo_indexed() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let allocation = 77;
    for submission in 1..=MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1 as u64 {
        let new_entries = backend
            .reserve_allocation_custody_v1(&[allocation])
            .unwrap();
        backend.retain_allocation_custody_v1(
            &[allocation],
            RuntimeAllocationCustodyOwnerV1 {
                submission,
                stream: 9,
                kind: RuntimeAllocationCustodyKindV1::Compute,
            },
            new_entries,
        );
    }
    let custody = &backend.allocation_custody[&allocation];
    assert_eq!(
        custody.owners.len(),
        MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1
    );
    assert_eq!(custody.sole_stream, Some(9));
    assert_eq!(
        custody.owner_counts,
        [MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1, 0]
    );
    assert!(matches!(
        backend.reserve_allocation_custody_v1(&[allocation]),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
    ));
    backend.release_allocation_custody_v1(allocation, 1);
    backend
        .release_allocation_custody_v1(allocation, MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1 as u64);
    for submission in 2..MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1 as u64 {
        backend.release_allocation_custody_v1(allocation, submission);
    }
    assert!(!backend.allocation_is_active(allocation));
    assert!(backend.allocation_custody.is_empty());
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_submission_capacity_counts_compute_sdma_and_completed() {
    let mut backend = KfdRuntimeBackendV1::mock();
    backend.submissions.insert(
        1,
        SubmissionRecordV1 {
            origin: SubmissionOriginV1::Ordinary,
            stream: 1,
            status: BackendPollV1::Succeeded,
            dependency_depth: 1,
            profile_dispatch_published: false,
        },
    );
    backend.compute_completion_reservations = MAX_RUNTIME_SUBMISSIONS_V1 - 2;
    backend.sdma_completion_reservations = 1;
    assert!(matches!(
        backend.require_submission_capacity_v1(),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
    ));
    backend.submissions.clear();
    backend.compute_completion_reservations = 0;
    backend.sdma_completion_reservations = 0;
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_cancelled_tail_restores_earlier_stream_head() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    backend.compute_completion_reservations = 2;
    backend
        .pending_compute_streams
        .insert(stream, VecDeque::from([40, 41]));
    backend
        .pending_compute
        .insert(40, pending_compute_for_test_v1(40, stream, 100, vec![]));
    let mut second = pending_compute_for_test_v1(41, stream, 101, vec![]);
    second.ordered_predecessor = Some(40);
    backend.pending_compute.insert(41, second);
    index_pending_compute_custody_for_test_v1(&mut backend, 40);
    index_pending_compute_custody_for_test_v1(&mut backend, 41);
    backend.compute_dependency_retain_counts.insert(40, 1);
    backend.stream_submission_tails.insert(stream, 41);

    assert_eq!(
        backend.cancel_v1(41).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert_eq!(backend.stream_submission_tails.get(&stream), Some(&40));
    assert_eq!(
        backend.pending_compute_streams[&stream],
        VecDeque::from([40])
    );
    assert_eq!(backend.compute_completion_reservations, 1);
    assert!(!backend.compute_dependency_retain_counts.contains_key(&40));

    assert_eq!(
        backend.cancel_v1(40).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert!(!backend.stream_submission_tails.contains_key(&stream));
    assert!(!backend.pending_compute_streams.contains_key(&stream));
    assert_eq!(backend.compute_completion_reservations, 0);
    assert!(backend.allocation_custody.is_empty());
    assert!(backend.compute_module_retain_counts.is_empty());
    backend.release_submission_v1(40).unwrap();
    backend.release_submission_v1(41).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    release_pending_compute_test_resources_v1(&mut backend);
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_interior_compute_cancellation_preserves_ordering_chain() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    backend.compute_completion_reservations = 3;
    backend
        .pending_compute_streams
        .insert(stream, VecDeque::from([40, 41, 42]));
    backend
        .pending_compute
        .insert(40, pending_compute_for_test_v1(40, stream, 100, vec![]));
    let mut second = pending_compute_for_test_v1(41, stream, 101, vec![]);
    second.ordered_predecessor = Some(40);
    backend.pending_compute.insert(41, second);
    let mut third = pending_compute_for_test_v1(42, stream, 102, vec![]);
    third.ordered_predecessor = Some(41);
    backend.pending_compute.insert(42, third);
    for submission in [40, 41, 42] {
        index_pending_compute_custody_for_test_v1(&mut backend, submission);
    }
    backend.compute_dependency_retain_counts.insert(40, 1);
    backend.compute_dependency_retain_counts.insert(41, 1);
    backend.stream_submission_tails.insert(stream, 42);

    assert_eq!(
        backend.cancel_v1(41).unwrap(),
        crate::BackendCancellationV1::TooLate
    );
    assert_eq!(
        backend.pending_compute_streams[&stream],
        VecDeque::from([40, 41, 42])
    );
    assert_eq!(backend.compute_completion_reservations, 3);
    assert!(!backend.submissions.contains_key(&41));
    assert_eq!(backend.stream_submission_tails.get(&stream), Some(&42));

    for submission in [42, 41, 40] {
        assert_eq!(
            backend.cancel_v1(submission).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        backend.release_submission_v1(submission).unwrap();
    }
    backend.destroy_stream_v1(stream).unwrap();
    release_pending_compute_test_resources_v1(&mut backend);
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_blocked_target_progress_roster_includes_both_lanes() {
    fn active(id: u64, stream: u64) -> ActiveSubmissionV1 {
        ActiveSubmissionV1 {
            source_event: Default::default(),
            id,
            stream,
            ordered_predecessor: None,
            deferred_ordered_predecessor_retain: false,
            kernel: 9,
            dependency_depth: 1,
            allocations: HashSet::new(),
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
    backend.active = Some(active(10, 1));
    backend.auxiliary_compute_lanes[0].active = Some(active(11, 2));
    assert_eq!(backend.active_compute_progress_roster_v1(), [true, true]);
    backend.active = None;
    backend.auxiliary_compute_lanes[0].active = None;
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_poll_never_prepares_dependency_ready_queued_compute() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    backend.compute_completion_reservations = 1;
    backend
        .pending_compute_streams
        .insert(stream, VecDeque::from([40]));
    backend
        .pending_compute
        .insert(40, pending_compute_for_test_v1(40, stream, 100, vec![]));
    index_pending_compute_custody_for_test_v1(&mut backend, 40);
    backend.stream_submission_tails.insert(stream, 40);

    assert_eq!(backend.poll_v1(40).unwrap(), BackendPollV1::Pending);
    assert!(backend.pending_compute.contains_key(&40));
    assert!(backend.stream_compute_lanes.is_empty());
    assert_eq!(backend.compute_completion_reservations, 1);

    assert_eq!(
        backend.cancel_v1(40).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    backend.release_submission_v1(40).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    release_pending_compute_test_resources_v1(&mut backend);
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_wait_observes_but_does_not_prepare_queued_predecessors() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let producer_stream = backend.create_stream_v1(7).unwrap();
    let consumer_stream = backend.create_stream_v1(7).unwrap();
    backend.compute_completion_reservations = 2;
    backend
        .pending_compute_streams
        .insert(producer_stream, VecDeque::from([40]));
    backend
        .pending_compute_streams
        .insert(consumer_stream, VecDeque::from([41]));
    backend.pending_compute.insert(
        40,
        pending_compute_for_test_v1(40, producer_stream, 100, vec![]),
    );
    let mut consumer = pending_compute_for_test_v1(41, consumer_stream, 101, vec![40]);
    consumer.ordered_predecessor = None;
    backend.pending_compute.insert(41, consumer);
    index_pending_compute_custody_for_test_v1(&mut backend, 40);
    index_pending_compute_custody_for_test_v1(&mut backend, 41);
    backend.compute_dependency_retain_counts.insert(40, 1);
    backend.stream_submission_tails.insert(producer_stream, 40);
    backend.stream_submission_tails.insert(consumer_stream, 41);

    assert_eq!(
        backend
            .wait_v1(41, Instant::now() + Duration::from_millis(1))
            .unwrap(),
        BackendPollV1::Pending
    );
    assert!(backend.pending_compute.contains_key(&40));
    assert!(backend.pending_compute.contains_key(&41));
    assert!(backend.stream_compute_lanes.is_empty());

    backend.cancel_v1(41).unwrap();
    backend.cancel_v1(40).unwrap();
    backend.release_submission_v1(40).unwrap();
    backend.release_submission_v1(41).unwrap();
    backend.destroy_stream_v1(producer_stream).unwrap();
    backend.destroy_stream_v1(consumer_stream).unwrap();
    release_pending_compute_test_resources_v1(&mut backend);
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_wait_does_not_enter_fixed_wait_for_native_dirty_publication() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    let allocation = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    backend
        .allocations
        .get_mut(&allocation)
        .unwrap()
        .native_dirty
        .push(NativeDirtyExtentV1 {
            compute_lane: 0,
            data_index: 0,
            allocation_offset: 0,
            data_offset: 0,
            byte_len: 8,
        });
    backend.native_dirty_extents = 1;
    backend.compute_completion_reservations = 1;
    backend
        .pending_compute_streams
        .insert(stream, VecDeque::from([40]));
    backend.pending_compute.insert(
        40,
        pending_compute_for_test_v1(40, stream, allocation, vec![]),
    );
    index_pending_compute_custody_for_test_v1(&mut backend, 40);
    backend.stream_submission_tails.insert(stream, 40);

    assert_eq!(
        backend
            .wait_v1(40, Instant::now() + Duration::from_millis(10))
            .unwrap(),
        BackendPollV1::Pending
    );
    assert!(backend.pending_compute.contains_key(&40));
    assert!(backend.stream_compute_lanes.is_empty());

    backend.cancel_v1(40).unwrap();
    backend.release_submission_v1(40).unwrap();
    backend.native_dirty_extents = 0;
    backend
        .allocations
        .get_mut(&allocation)
        .unwrap()
        .native_dirty
        .clear();
    backend.release_allocation_v1(allocation).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    release_pending_compute_test_resources_v1(&mut backend);
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_invalid_host_ranges_do_not_reconcile_dirty_authority() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let allocation = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    backend
        .allocations
        .get_mut(&allocation)
        .unwrap()
        .native_dirty
        .push(NativeDirtyExtentV1 {
            compute_lane: 0,
            data_index: 0,
            allocation_offset: 0,
            data_offset: 0,
            byte_len: 8,
        });
    backend.native_dirty_extents = 1;
    let dirty_before = backend.allocations[&allocation].native_dirty.clone();

    assert!(matches!(
        backend.write_allocation_v1(allocation, 8, &[1]),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
    ));
    let mut destination = [0_u8; 1];
    assert!(matches!(
        backend.read_allocation_v1(allocation, 8, &mut destination),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
    ));
    assert_eq!(backend.allocations[&allocation].native_dirty, dirty_before);
    assert_eq!(backend.native_dirty_extents, 1);

    backend.native_dirty_extents = 0;
    backend
        .allocations
        .get_mut(&allocation)
        .unwrap()
        .native_dirty
        .clear();
    backend.release_allocation_v1(allocation).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_alternating_compute_copy_dependency_chain_is_bounded() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let mut deepest = pending_compute_for_test_v1(40, 1, 100, vec![]);
    deepest.dependency_depth = MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1 - 1;
    backend.pending_compute.insert(40, deepest);
    assert_eq!(
        backend.next_dependency_depth_v1(None, &[40]),
        Ok(MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1)
    );
    backend.active_sdma.insert(
        41,
        ActiveSdmaCopyV1 {
            id: 41,
            stream: 1,
            prior_stream_submission: Some(40),
            source: 100,
            destination: 101,
            source_offset: 0,
            destination_offset: 0,
            byte_len: 8,
            completed_bytes: 0,
            window_bytes: 0,
            window_requests: None,
            dependencies: vec![40],
            dependency_cursor: 0,
            dependency_depth: MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1,
            peer_access: None,
            phase: ActiveSdmaPhaseV1::Ready,
        },
    );
    assert_eq!(
        backend.next_dependency_depth_v1(None, &[41]),
        Err(DirectSdmaDependencyDepthErrorV1::LimitExceeded)
    );
    backend.pending_compute.clear();
    backend.active_sdma.clear();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_flush_rejects_unknown_stream_without_mutation() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let next_handle = backend.next_handle;
    assert!(matches!(
        backend.flush_stream_v1(99),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::UnknownHandle
    ));
    assert_eq!(backend.next_handle, next_handle);
    assert!(backend.pending_compute.is_empty());
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_flush_covers_dependency_ready_unpublished_sdma() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    let source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let destination = backend
        .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
        .unwrap();
    backend.active_sdma.insert(
        40,
        ActiveSdmaCopyV1 {
            id: 40,
            stream,
            prior_stream_submission: None,
            source,
            destination,
            source_offset: 0,
            destination_offset: 0,
            byte_len: 8,
            completed_bytes: 0,
            window_bytes: 0,
            window_requests: None,
            dependencies: Vec::new(),
            dependency_cursor: 0,
            dependency_depth: 1,
            peer_access: None,
            phase: ActiveSdmaPhaseV1::Ready,
        },
    );
    index_sdma_custody_for_test_v1(&mut backend, 40);
    backend.stream_submission_tails.insert(stream, 40);

    assert!(matches!(
        backend.flush_stream_v1(stream),
        Err(RuntimeBackendFailureV1::Quiescent(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Native
    ));
    assert!(!backend.active_sdma.contains_key(&40));
    assert_eq!(
        backend.submissions[&40].status,
        BackendPollV1::Failed {
            code: COOPERATIVE_COPY_FAILURE_CODE_V1
        }
    );

    backend.release_submission_v1(40).unwrap();
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_flush_reports_quiescent_compute_prepublication_failure() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    backend.compute_completion_reservations = 1;
    backend
        .pending_compute_streams
        .insert(stream, VecDeque::from([40]));
    backend
        .pending_compute
        .insert(40, pending_compute_for_test_v1(40, stream, 100, vec![]));
    index_pending_compute_custody_for_test_v1(&mut backend, 40);
    backend.stream_submission_tails.insert(stream, 40);

    assert!(matches!(
        backend.flush_stream_v1(stream),
        Err(RuntimeBackendFailureV1::Quiescent(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Native
    ));
    assert!(!backend.pending_compute.contains_key(&40));
    assert_eq!(backend.compute_completion_reservations, 0);
    assert_eq!(
        backend.submissions[&40].status,
        BackendPollV1::Failed { code: -1 }
    );

    backend.release_submission_v1(40).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    release_pending_compute_test_resources_v1(&mut backend);
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_flush_does_not_treat_later_same_stream_owner_as_conflict() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    let allocation = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    backend.compute_completion_reservations = 2;
    backend
        .pending_compute_streams
        .insert(stream, VecDeque::from([40, 41]));
    backend.pending_compute.insert(
        40,
        pending_compute_for_test_v1(40, stream, allocation, vec![]),
    );
    let mut second = pending_compute_for_test_v1(41, stream, allocation, vec![40]);
    second.ordered_predecessor = Some(40);
    backend.pending_compute.insert(41, second);
    index_pending_compute_custody_for_test_v1(&mut backend, 40);
    index_pending_compute_custody_for_test_v1(&mut backend, 41);
    backend.compute_dependency_retain_counts.insert(40, 1);
    backend.stream_submission_tails.insert(stream, 41);

    assert!(matches!(
        backend.flush_stream_v1(stream),
        Err(RuntimeBackendFailureV1::Quiescent(_))
    ));
    assert!(!backend.pending_compute.contains_key(&40));
    assert!(backend.pending_compute.contains_key(&41));

    backend.cancel_v1(41).unwrap();
    backend.release_submission_v1(40).unwrap();
    backend.release_submission_v1(41).unwrap();
    backend.release_allocation_v1(allocation).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    release_pending_compute_test_resources_v1(&mut backend);
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_flush_rejects_published_conflict_before_observation() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    let allocation = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    backend.active = Some(ActiveSubmissionV1 {
        source_event: Default::default(),
        id: 50,
        stream: 2,
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
    });
    backend.compute_completion_reservations = 1;
    backend
        .pending_compute_streams
        .insert(stream, VecDeque::from([40]));
    backend.pending_compute.insert(
        40,
        pending_compute_for_test_v1(40, stream, allocation, vec![]),
    );
    index_pending_compute_custody_for_test_v1(&mut backend, 40);
    backend.stream_submission_tails.insert(stream, 40);

    let custody_before = backend.allocation_custody[&allocation].owners.len();
    assert!(matches!(
        backend.flush_stream_v1(stream),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
    ));
    assert_eq!(backend.active.as_ref().map(|active| active.id), Some(50));
    assert!(backend.pending_compute.contains_key(&40));
    assert_eq!(
        backend.allocation_custody[&allocation].owners.len(),
        custody_before
    );
    assert_eq!(backend.compute_completion_reservations, 1);

    backend.active = None;
    backend.cancel_v1(40).unwrap();
    backend.release_submission_v1(40).unwrap();
    backend.release_allocation_v1(allocation).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    release_pending_compute_test_resources_v1(&mut backend);
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_poll_never_publishes_dependency_ready_sdma() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    let source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let destination = backend
        .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
        .unwrap();
    backend.active_sdma.insert(
        40,
        ActiveSdmaCopyV1 {
            id: 40,
            stream,
            prior_stream_submission: None,
            source,
            destination,
            source_offset: 0,
            destination_offset: 0,
            byte_len: 8,
            completed_bytes: 0,
            window_bytes: 0,
            window_requests: None,
            dependencies: Vec::new(),
            dependency_cursor: 0,
            dependency_depth: 1,
            peer_access: None,
            phase: ActiveSdmaPhaseV1::Ready,
        },
    );
    index_sdma_custody_for_test_v1(&mut backend, 40);
    backend.stream_submission_tails.insert(stream, 40);

    assert_eq!(backend.poll_v1(40).unwrap(), BackendPollV1::Pending);
    assert!(backend.active_sdma.contains_key(&40));
    assert!(backend.submissions.is_empty());

    backend.cancel_v1(40).unwrap();
    backend.release_submission_v1(40).unwrap();
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_partial_sdma_continuation_is_observed_and_not_cancellable() {
    sdma_cancellation_custody_tests::inspect_partial_cancellation(0);
}
