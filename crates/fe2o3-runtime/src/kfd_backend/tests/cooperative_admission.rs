use super::*;

#[test]
fn multi_device_router_host_stages_peer_copy_and_preserves_event_custody() {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    right.description.name = "mock gfx942 right".to_owned();
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    let descriptions = backend.enumerate_devices_v1().unwrap();
    assert_eq!(descriptions.len(), 2);
    assert!(
        descriptions
            .iter()
            .all(|device| { device.capabilities.multi_device && device.capabilities.peer_copy })
    );

    let left_stream = backend.create_stream_v1(7).unwrap();
    let right_stream = backend.create_stream_v1(8).unwrap();
    let source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 32, 8)
        .unwrap();
    let destination = backend
        .allocate_v1(8, RuntimeMemoryKindV1::HostVisible, 32, 8)
        .unwrap();
    let expected = (1_u8..=32).collect::<Vec<_>>();
    backend.write_allocation_v1(source, 0, &expected).unwrap();
    let destination_route = backend.allocations[&destination];
    let submission = backend
        .peer_copy_v1(
            right_stream,
            BackendMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 32,
            },
            BackendMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: 32,
            },
            &[],
        )
        .unwrap();
    assert!(
        backend.children[destination_route.child].allocations[&destination_route.local]
            .bytes
            .iter()
            .all(|byte| *byte == 0)
    );
    assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
    let event = backend.record_event_v1(right_stream, submission).unwrap();
    let left_child = backend.child_for_device(7).unwrap();
    assert!(matches!(
        backend.dependency_for_child(event, left_child),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::WrongDevice
    ));
    assert!(matches!(
        backend.release_submission_v1(submission),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
    ));
    assert!(matches!(
        backend.read_allocation_v1(destination, 0, &mut [0_u8; 1]),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
    ));
    assert_eq!(
        backend.wait_v1(submission, Instant::now()).unwrap(),
        BackendPollV1::Pending
    );
    assert!(
        backend.children[destination_route.child].allocations[&destination_route.local]
            .bytes
            .iter()
            .all(|byte| *byte == 0)
    );
    backend.flush_stream_v1(right_stream).unwrap();
    assert_eq!(
        backend
            .wait_v1(submission, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    let mut observed = [0_u8; 32];
    backend
        .read_allocation_v1(destination, 0, &mut observed)
        .unwrap();
    assert_eq!(observed.as_slice(), expected);
    backend.release_event_v1(event).unwrap();
    backend.release_submission_v1(submission).unwrap();
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(left_stream).unwrap();
    backend.destroy_stream_v1(right_stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn multi_device_router_cooperatively_copies_on_one_device() {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    let stream = backend.create_stream_v1(7).unwrap();
    let source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 16, 8)
        .unwrap();
    let destination = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 16, 8)
        .unwrap();
    backend
        .write_allocation_v1(source, 4, &[9, 8, 7, 6])
        .unwrap();
    let submission = backend
        .copy_async_v1(
            stream,
            BackendMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 4,
                byte_len: 4,
            },
            BackendMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 8,
                byte_len: 4,
            },
            &[],
        )
        .unwrap();
    assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
    assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
    backend.flush_stream_v1(stream).unwrap();
    assert_eq!(
        backend.poll_v1(submission).unwrap(),
        BackendPollV1::Succeeded
    );
    let mut observed = [0_u8; 4];
    backend
        .read_allocation_v1(destination, 8, &mut observed)
        .unwrap();
    assert_eq!(observed, [9, 8, 7, 6]);
    backend.release_submission_v1(submission).unwrap();
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn cooperative_copy_dependency_translation_is_observational() {
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
    let event = backend.record_event_v1(stream, submission).unwrap();
    let child = backend.child_for_device(7).unwrap();

    assert!(matches!(
        backend.dependency_for_child(event, child),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
    ));
    assert!(matches!(
        &backend.submissions[&submission],
        RoutedSubmissionV1::CooperativeCopy(copy)
            if copy.phase == CooperativeCopyPhaseV1::Dependencies
                && copy.dependency_cursor == 0
                && copy.byte_cursor == 0
    ));
    let destination_route = backend.allocations[&destination];
    assert!(
        backend.children[destination_route.child].allocations[&destination_route.local]
            .bytes
            .iter()
            .all(|byte| *byte == 0)
    );

    backend.release_event_v1(event).unwrap();
    backend.flush_stream_v1(stream).unwrap();
    assert_eq!(
        backend
            .wait_v1(submission, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    backend.release_submission_v1(submission).unwrap();
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn cooperative_copy_rejects_native_allocation_custody_before_mutation() {
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
    let source_route = backend.allocations[&source];
    let mut active_allocations = HashSet::new();
    active_allocations.insert(source_route.local);
    backend.children[source_route.child].active = Some(ActiveSubmissionV1 {
        source_event: Default::default(),
        id: 99,
        stream: 1,
        ordered_predecessor: None,
        deferred_ordered_predecessor_retain: false,
        kernel: 1,
        dependency_depth: 1,
        allocations: active_allocations,
        writebacks: Vec::new(),
        resident_descriptors: Vec::new(),
        ordinary_recipe: None,
        dispatch_shape_sha256: [0; 32],
        published_at: Instant::now(),
        performance: KfdRuntimeLaunchPerformanceV1::default(),
        execution: None,
    });
    let child = &mut backend.children[source_route.child];
    let reserved = child
        .reserve_allocation_custody_v1(&[source_route.local])
        .unwrap();
    child.retain_allocation_custody_v1(
        &[source_route.local],
        RuntimeAllocationCustodyOwnerV1 {
            submission: 99,
            stream: 1,
            kind: RuntimeAllocationCustodyKindV1::Compute,
        },
        reserved,
    );
    let submissions_before = backend.submissions.len();
    let next_handle_before = backend.next_handle;
    let region = |allocation, access| BackendMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: 4,
    };

    assert!(matches!(
        backend.copy_async_v1(
            stream,
            region(source, RuntimeAccessV1::Read),
            region(destination, RuntimeAccessV1::Write),
            &[],
        ),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
    ));
    assert_eq!(backend.submissions.len(), submissions_before);
    assert_eq!(backend.next_handle, next_handle_before);

    backend.children[source_route.child].active = None;
    backend.children[source_route.child].release_allocation_custody_v1(source_route.local, 99);
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn cooperative_copy_backend_enforces_dependency_capacity() {
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
    let excessive = vec![0_u64; MAX_RUNTIME_DEPENDENCIES_V1 + 1];
    let region = |allocation, access| BackendMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: 4,
    };

    assert!(matches!(
        backend.copy_async_v1(
            stream,
            region(source, RuntimeAccessV1::Read),
            region(destination, RuntimeAccessV1::Write),
            &excessive,
        ),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
    ));
    assert!(backend.submissions.is_empty());

    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn cooperative_copy_rejects_both_out_of_bounds_ranges_before_publication() {
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
    backend.write_allocation_v1(source, 0, &[7; 8]).unwrap();
    let region = |allocation, byte_offset, access| BackendMemoryRegionV1 {
        allocation,
        access,
        byte_offset,
        byte_len: 8,
    };

    for (source_offset, destination_offset) in [(1, 0), (0, 1)] {
        assert!(matches!(
            backend.copy_async_v1(
                stream,
                region(source, source_offset, RuntimeAccessV1::Read),
                region(destination, destination_offset, RuntimeAccessV1::Write),
                &[],
            ),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
        ));
        assert!(backend.submissions.is_empty());
        let destination_route = backend.allocations[&destination];
        assert!(
            backend.children[destination_route.child].allocations[&destination_route.local]
                .bytes
                .iter()
                .all(|byte| *byte == 0)
        );
    }

    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn synthetic_kfd_async_copy_is_explicitly_unsupported() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let region = BackendMemoryRegionV1 {
        allocation: 1,
        access: RuntimeAccessV1::ReadWrite,
        byte_offset: 0,
        byte_len: 8,
    };
    assert!(matches!(
        backend.copy_async_v1(1, region, region, &[]),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported
    ));
}
