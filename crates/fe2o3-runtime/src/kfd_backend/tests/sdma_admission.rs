use super::*;

#[test]
fn capability_inventory_is_fail_closed() {
    let capabilities = kfd_capabilities_v1();
    assert!(capabilities.typed_async_launch);
    assert!(capabilities.streams);
    assert!(capabilities.events);
    assert!(capabilities.device_memory);
    assert!(capabilities.host_visible_memory);
    assert!(!capabilities.peer_copy);
    assert!(!capabilities.multi_device);
    assert!(!capabilities.atomics);
    assert!(!capabilities.collectives);
}

#[test]
fn worker_v3_generated_only_profile_denies_generic_compute_before_encoding() {
    let mut backend = KfdRuntimeBackendV1::mock_worker_v3_generated_only_v1();
    assert!(!backend.description.capabilities.typed_async_launch);
    assert!(!backend.description.capabilities.atomics);
    assert!(!backend.description.capabilities.collectives);
    backend.native_available = true;
    let execution = backend.execution_capabilities_v1(backend.description.backend_device);
    assert!(!execution.concurrent_compute);
    assert!(!execution.compute_copy_overlap);
    assert!(!execution.atomics);
    assert!(!execution.collectives);
    assert!(execution.native_async_copy);
    assert!(execution.memory_pool);
    backend.native_available = false;

    let mut context = crate::RuntimeContextV1::open(backend).unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let module = context
        .load_module(device, &synthetic_cov6::three_binding_module())
        .unwrap();
    let kernel = context
        .resolve_kernel::<GeneratedOnlyRejectedArgumentsV1>(module, "vecadd")
        .unwrap();
    let arguments = GeneratedOnlyRejectedArgumentsV1;
    let geometry = crate::RuntimeLaunchGeometryV1 {
        grid: [64, 1, 1],
        workgroup: [64, 1, 1],
        dynamic_shared_bytes: 0,
    };

    assert!(matches!(
        context.launch(stream, &kernel, &arguments, geometry, &[]),
        Err(crate::RuntimeErrorV1::Validation(
            crate::RuntimeValidationErrorV1::Unsupported
        ))
    ));
    assert!(matches!(
        context.launch_atomic(
            stream,
            &kernel,
            &arguments,
            RuntimeAtomicLaunchContractV1 {
                operation: RuntimeAtomicOperationV1::Add,
                scope: RuntimeMemoryScopeV1::Workgroup,
                order: RuntimeMemoryOrderV1::Relaxed,
                failure_order: None,
                weak: false,
                geometry,
            },
            &[],
        ),
        Err(crate::RuntimeErrorV1::Validation(
            crate::RuntimeValidationErrorV1::Unsupported
        ))
    ));
    assert!(matches!(
        context.launch_collective(
            stream,
            &kernel,
            &arguments,
            RuntimeCollectiveLaunchContractV1 {
                operation: crate::RuntimeCollectiveOperationV1::ReduceSum,
                scope: RuntimeMemoryScopeV1::Workgroup,
                order: RuntimeMemoryOrderV1::AcquireRelease,
                participants: 64,
                geometry,
            },
            &[],
        ),
        Err(crate::RuntimeErrorV1::Validation(
            crate::RuntimeValidationErrorV1::Unsupported
        ))
    ));

    context.unload_module(module).unwrap();
    context.destroy_stream(stream).unwrap();
    let _backend = context.shutdown().unwrap();
}

#[test]
fn direct_kfd_compute_sdma_overlap_is_allocation_scoped() {
    let mut custody = HashMap::new();
    for allocation in 1_000..2_000 {
        custody.insert(
            allocation,
            RuntimeAllocationCustodyV1 {
                owners: VecDeque::from([RuntimeAllocationCustodyOwnerV1 {
                    submission: allocation,
                    stream: allocation,
                    kind: RuntimeAllocationCustodyKindV1::Sdma,
                }]),
                sole_stream: Some(allocation),
                owner_counts: [0, 1],
                metadata_credits: None,
            },
        );
    }
    custody.insert(
        21,
        RuntimeAllocationCustodyV1 {
            owners: VecDeque::from([RuntimeAllocationCustodyOwnerV1 {
                submission: 50,
                stream: 5,
                kind: RuntimeAllocationCustodyKindV1::Sdma,
            }]),
            sole_stream: Some(5),
            owner_counts: [0, 1],
            metadata_credits: None,
        },
    );
    let disjoint = [BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation: 10,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 8,
        },
        kernarg_byte_offset: 0,
    }];
    let overlapping = [BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation: 21,
            ..disjoint[0].region
        },
        kernarg_byte_offset: 0,
    }];
    let mut publication_lookups = 0;
    assert_eq!(
        indexed_published_sdma_conflict_v1(&disjoint, &custody, 60, 6, |_| {
            publication_lookups += 1;
            true
        },),
        None
    );
    assert_eq!(publication_lookups, 0);
    assert_eq!(
        indexed_published_sdma_conflict_v1(&overlapping, &custody, 60, 6, |submission| {
            publication_lookups += 1;
            submission == 50
        }),
        Some(50)
    );
    assert_eq!(publication_lookups, 1);
}

#[test]
fn direct_kfd_sdma_dependency_depth_is_bounded_before_mutation() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    let source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 1, 1)
        .unwrap();
    let destination = backend
        .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 1, 1)
        .unwrap();
    for allocation in [source, destination] {
        let record = backend.allocations.get_mut(&allocation).unwrap();
        record.sdma_backed = true;
        record.sdma_initialized = true;
    }
    backend.native_available = true;
    backend.active_sdma.insert(
        100,
        ActiveSdmaCopyV1 {
            id: 100,
            stream,
            prior_stream_submission: None,
            source: 1_000,
            destination: 1_001,
            source_offset: 0,
            destination_offset: 0,
            byte_len: 1,
            completed_bytes: 0,
            window_bytes: 0,
            window_requests: None,
            dependencies: Vec::new(),
            dependency_cursor: 0,
            dependency_depth: MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1,
            peer_access: None,
            phase: ActiveSdmaPhaseV1::Ready,
        },
    );
    backend
        .events
        .insert(200, EventRecordV1 { submission: 100 });
    let next_handle_before = backend.next_handle;
    let active_before = backend.active_sdma.len();
    let region = |allocation, access| BackendMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: 1,
    };

    assert!(matches!(
        backend.copy_async_v1(
            stream,
            region(source, RuntimeAccessV1::Read),
            region(destination, RuntimeAccessV1::Write),
            &[200],
        ),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
    ));
    assert_eq!(backend.next_handle, next_handle_before);
    assert_eq!(backend.active_sdma.len(), active_before);
    assert!(backend.submissions.is_empty());
    assert!(backend.sdma_dependency_retain_counts.is_empty());

    backend.active_sdma.get_mut(&100).unwrap().dependency_depth =
        MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1 - 1;
    assert_eq!(
        backend.next_dependency_depth_v1(None, &[100]),
        Ok(MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1)
    );
    backend.active_sdma.get_mut(&100).unwrap().dependency_depth = usize::MAX;
    assert_eq!(
        backend.next_dependency_depth_v1(None, &[100]),
        Err(DirectSdmaDependencyDepthErrorV1::Overflow)
    );

    backend.events.remove(&200);
    backend.active_sdma.remove(&100);
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_sdma_capacity_rejection_precedes_native_reconciliation() {
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
    backend
        .allocations
        .get_mut(&source)
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
    backend.native_available = true;
    backend.next_handle = u64::MAX;
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
            &[],
        ),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
    ));
    assert!(!backend.terminal);
    assert_eq!(backend.allocations[&source].native_dirty.len(), 1);
    assert!(backend.active_sdma.is_empty());
    assert!(backend.sdma_dependency_retain_counts.is_empty());

    backend.native_available = false;
    backend.native_dirty_extents = 0;
    backend
        .allocations
        .get_mut(&source)
        .unwrap()
        .native_dirty
        .clear();
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_sdma_submit_defers_dirty_native_reconciliation() {
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
    backend
        .allocations
        .get_mut(&source)
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
    backend.native_available = true;
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
    assert!(!backend.terminal);
    assert_eq!(backend.allocations[&source].native_dirty.len(), 1);
    assert!(matches!(
        backend.active_sdma[&submission].phase,
        ActiveSdmaPhaseV1::Ready
    ));

    assert_eq!(
        backend.cancel_v1(submission).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    backend.release_submission_v1(submission).unwrap();
    backend.native_available = false;
    backend.native_dirty_extents = 0;
    backend
        .allocations
        .get_mut(&source)
        .unwrap()
        .native_dirty
        .clear();
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_rebind_requires_synchronizing_detach_for_disjoint_or_new_shape() {
    let prior = ResidentDataDescriptorV1 {
        allocation: 10,
        kind: RuntimeMemoryKindV1::HostVisible,
        alignment: 8,
        allocation_offset: 0,
        byte_len: 8,
        host_content_sha256: None,
        device_may_have_modified: true,
    };
    let recycled = RecycledDispatchV1 {
        kernel: 1,
        dispatch_shape_sha256: [7; 32],
        descriptors: vec![prior],
    };
    let data_for = |allocation, kind| DataSpecV1 {
        allocation,
        kind,
        alignment: 8,
        allocation_offset: 0,
        bytes: Arc::from([0_u8; 8]),
        byte_range: 0..8,
        content_sha256: None,
    };

    assert!(recycled_dispatch_reuse_is_admitted_v1(
        &recycled,
        [7; 32],
        &[prior],
        &[data_for(10, RuntimeMemoryKindV1::HostVisible)],
    ));
    let disjoint = ResidentDataDescriptorV1 {
        allocation: 20,
        ..prior
    };
    assert!(!recycled_dispatch_reuse_is_admitted_v1(
        &recycled,
        [7; 32],
        &[disjoint],
        &[data_for(20, RuntimeMemoryKindV1::HostVisible)],
    ));
    assert!(!recycled_dispatch_reuse_is_admitted_v1(
        &recycled,
        [8; 32],
        &[prior],
        &[data_for(10, RuntimeMemoryKindV1::HostVisible)],
    ));
    assert!(!recycled_dispatch_reuse_is_admitted_v1(
        &recycled,
        [7; 32],
        &[prior],
        &[data_for(10, RuntimeMemoryKindV1::DeviceLocal)],
    ));
}

#[test]
fn direct_kfd_sdma_direction_preflight_is_explicit() {
    assert_eq!(
        direct_sdma_direction_v1(
            RuntimeMemoryKindV1::HostVisible,
            RuntimeMemoryKindV1::DeviceLocal
        ),
        Some(Gfx942PersistentSdmaDirectionV1::HostToDevice)
    );
    assert_eq!(
        direct_sdma_direction_v1(
            RuntimeMemoryKindV1::DeviceLocal,
            RuntimeMemoryKindV1::HostVisible
        ),
        Some(Gfx942PersistentSdmaDirectionV1::DeviceToHost)
    );
    assert_eq!(
        direct_sdma_direction_v1(
            RuntimeMemoryKindV1::HostVisible,
            RuntimeMemoryKindV1::HostVisible
        ),
        None
    );
    assert_eq!(
        direct_sdma_direction_v1(
            RuntimeMemoryKindV1::DeviceLocal,
            RuntimeMemoryKindV1::DeviceLocal
        ),
        None
    );
}

#[test]
fn direct_kfd_sdma_window_plan_covers_256_mib_as_63_plus_2_packets() {
    let cap = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1);
    let mut active = ActiveSdmaCopyV1 {
        id: 1,
        stream: 2,
        prior_stream_submission: None,
        source: 3,
        destination: 4,
        source_offset: 11,
        destination_offset: 29,
        byte_len: KFD_RUNTIME_MAX_STAGED_ALLOCATION_BYTES_V1,
        completed_bytes: 0,
        window_bytes: 0,
        window_requests: None,
        dependencies: Vec::new(),
        dependency_cursor: 0,
        dependency_depth: 1,
        peer_access: None,
        phase: ActiveSdmaPhaseV1::Ready,
    };
    let mut window_packet_counts = Vec::new();
    let mut packet_count = 0_usize;
    let mut last_bytes = 0;
    while active.completed_bytes < active.byte_len {
        let window = direct_sdma_window_plan_v1(&active).unwrap();
        window_packet_counts.push(window.requests.packet_count());
        let same_device_requests = same_device_sdma_requests_v1(&window.requests).unwrap();
        assert_eq!(same_device_requests.len(), window.requests.packet_count());
        for (index, request) in window.requests.as_slice().iter().enumerate() {
            let packet_progress = u64::try_from(index).unwrap() * cap;
            assert_eq!(
                request.source_offset,
                11 + active.completed_bytes + packet_progress
            );
            assert_eq!(
                request.destination_offset,
                29 + active.completed_bytes + packet_progress
            );
            assert_eq!(
                same_device_requests[index],
                SameDeviceSdmaCopyRequestV1 {
                    source_offset: request.source_offset,
                    destination_offset: request.destination_offset,
                    copy_bytes: request.copy_bytes,
                }
            );
            packet_count += 1;
            last_bytes = request.copy_bytes;
        }
        active.completed_bytes += window.copy_bytes;
    }
    assert_eq!(window_packet_counts, [63, 2]);
    assert_eq!(packet_count, 65);
    assert_eq!(last_bytes, 2_048);
    assert_eq!(active.completed_bytes, 256 * 1024 * 1024);
    assert_eq!(cap, 0x003f_ffe0);
}

#[test]
fn direct_kfd_sdma_window_plan_honors_packet_boundaries() {
    let cap = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1);
    for (byte_len, expected_packets, expected_bytes) in [
        (1, 1, 1),
        (cap, 1, cap),
        (cap + 1, 2, cap + 1),
        (63 * cap, 63, 63 * cap),
        (63 * cap + 1, 63, 63 * cap),
    ] {
        let active = ActiveSdmaCopyV1 {
            id: 1,
            stream: 2,
            prior_stream_submission: None,
            source: 3,
            destination: 4,
            source_offset: 0,
            destination_offset: 0,
            byte_len,
            completed_bytes: 0,
            window_bytes: 0,
            window_requests: None,
            dependencies: Vec::new(),
            dependency_cursor: 0,
            dependency_depth: 1,
            peer_access: None,
            phase: ActiveSdmaPhaseV1::Ready,
        };
        let window = direct_sdma_window_plan_v1(&active).unwrap();
        assert_eq!(window.requests.packet_count(), expected_packets);
        assert_eq!(window.copy_bytes, expected_bytes);
        assert_eq!(
            matches!(&window.requests, DirectSdmaRequestPlanV1::Single(_)),
            byte_len <= cap
        );
        assert_eq!(
            matches!(&window.requests, DirectSdmaRequestPlanV1::Window(_)),
            byte_len > cap
        );
    }
}

#[test]
fn direct_kfd_directional_plan_preserves_the_exact_single_packet_boundary() {
    let cap = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1);
    for direction in [
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        Gfx942PersistentSdmaDirectionV1::DeviceToHost,
    ] {
        for (byte_len, expect_single) in [(cap, true), (cap + 1, false)] {
            let active = ActiveSdmaCopyV1 {
                id: 1,
                stream: 2,
                prior_stream_submission: None,
                source: 3,
                destination: 4,
                source_offset: 11,
                destination_offset: 29,
                byte_len,
                completed_bytes: 0,
                window_bytes: 0,
                window_requests: None,
                dependencies: Vec::new(),
                dependency_cursor: 0,
                dependency_depth: 1,
                peer_access: None,
                phase: ActiveSdmaPhaseV1::Ready,
            };
            let window = direct_sdma_window_plan_v1(&active).unwrap();
            let directional = directional_sdma_requests_v1(&window.requests, direction).unwrap();
            assert_eq!(
                matches!(&directional, DirectionalSdmaRequestPlanV1::Single(_)),
                expect_single
            );
            assert_eq!(
                matches!(&directional, DirectionalSdmaRequestPlanV1::Window(_)),
                !expect_single
            );

            let same_device = same_device_sdma_requests_v1(&window.requests).unwrap();
            assert_eq!(same_device.len(), if expect_single { 1 } else { 2 });
        }
    }
}

#[test]
fn direct_kfd_unsupported_copy_direction_is_mutation_free() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    let source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let destination = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    for allocation in [source, destination] {
        let record = backend.allocations.get_mut(&allocation).unwrap();
        record.sdma_backed = true;
        record.sdma_initialized = true;
    }
    backend.native_available = true;
    let next_handle = backend.next_handle;
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
            &[],
        ),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported
    ));
    assert_eq!(backend.next_handle, next_handle);
    assert!(backend.active_sdma.is_empty());
    assert!(backend.allocation_custody.is_empty());
    assert!(backend.sdma_dependency_retain_counts.is_empty());
    assert!(backend.stream_submission_tails.is_empty());
    backend.native_available = false;
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_quiescent_copy_marker_has_no_live_custody() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    let source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let destination = backend
        .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
        .unwrap();
    let dependency = 30;
    let submission = 40;
    backend.active_sdma.insert(
        submission,
        ActiveSdmaCopyV1 {
            id: submission,
            stream,
            prior_stream_submission: None,
            source,
            destination,
            source_offset: 0,
            destination_offset: 0,
            byte_len: 8,
            completed_bytes: 4,
            window_bytes: 0,
            window_requests: None,
            dependencies: vec![dependency],
            dependency_cursor: 1,
            dependency_depth: 1,
            peer_access: None,
            phase: ActiveSdmaPhaseV1::Ready,
        },
    );
    index_sdma_custody_for_test_v1(&mut backend, submission);
    backend.sdma_dependency_retain_counts.insert(dependency, 1);
    backend.stream_submission_tails.insert(stream, submission);
    backend.fail_quiescent_sdma_copy_v1(submission).unwrap();

    assert!(backend.quiescent_sdma_submissions.contains(&submission));
    assert!(!backend.active_sdma.contains_key(&submission));
    assert!(backend.active_sdma_streams.is_empty());
    assert!(backend.allocation_custody.is_empty());
    assert!(backend.sdma_dependency_retain_counts.is_empty());
    assert_eq!(backend.sdma_completion_reservations, 0);
    assert!(matches!(
        backend.poll_v1(submission),
        Err(RuntimeBackendFailureV1::Quiescent(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Native
    ));
    assert!(matches!(
        backend.wait_v1(submission, Instant::now() + Duration::from_secs(1)),
        Err(RuntimeBackendFailureV1::Quiescent(_))
    ));
    assert!(matches!(
        backend.drain_v1(submission, Instant::now() + Duration::from_secs(1)),
        Err(RuntimeBackendFailureV1::Quiescent(_))
    ));
    let event = backend.record_event_v1(stream, submission).unwrap();
    assert!(matches!(
        backend.release_submission_v1(submission),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
    ));
    let dependent = 41;
    backend.active_sdma.insert(
        dependent,
        ActiveSdmaCopyV1 {
            id: dependent,
            stream,
            prior_stream_submission: Some(submission),
            source,
            destination,
            source_offset: 0,
            destination_offset: 0,
            byte_len: 8,
            completed_bytes: 0,
            window_bytes: 0,
            window_requests: None,
            dependencies: vec![submission],
            dependency_cursor: 0,
            dependency_depth: 2,
            peer_access: None,
            phase: ActiveSdmaPhaseV1::Ready,
        },
    );
    index_sdma_custody_for_test_v1(&mut backend, dependent);
    backend.sdma_dependency_retain_counts.insert(submission, 1);
    backend.stream_submission_tails.insert(stream, dependent);
    assert!(matches!(
        backend.poll_v1(dependent),
        Err(RuntimeBackendFailureV1::Quiescent(_))
    ));
    assert!(backend.quiescent_sdma_submissions.contains(&dependent));
    assert!(!backend.active_sdma.contains_key(&dependent));
    assert!(backend.allocation_custody.is_empty());
    assert!(backend.sdma_dependency_retain_counts.is_empty());
    assert_eq!(backend.sdma_completion_reservations, 0);
    assert!(backend.quiescent_sdma_marker_capacity_is_reserved_v1());
    backend.release_event_v1(event).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    assert!(backend.quiescent_sdma_submissions.contains(&submission));
    assert!(backend.quiescent_sdma_submissions.contains(&dependent));
    assert!(matches!(
        backend.shutdown_native_v1(),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
    ));
    backend.release_submission_v1(dependent).unwrap();
    backend.release_submission_v1(submission).unwrap();
    assert!(backend.quiescent_sdma_submissions.is_empty());
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_zero_progress_failure_is_conclusive_without_marker() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    let source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let destination = backend
        .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
        .unwrap();
    let submission = 40;
    backend.active_sdma.insert(
        submission,
        ActiveSdmaCopyV1 {
            id: submission,
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
    index_sdma_custody_for_test_v1(&mut backend, submission);
    backend.stream_submission_tails.insert(stream, submission);
    assert_eq!(
        backend.fail_unpublished_sdma_copy_v1(submission).unwrap(),
        BackendPollV1::Failed {
            code: COOPERATIVE_COPY_FAILURE_CODE_V1
        }
    );
    assert!(!backend.quiescent_sdma_submissions.contains(&submission));
    assert_eq!(
        backend.poll_v1(submission).unwrap(),
        BackendPollV1::Failed {
            code: COOPERATIVE_COPY_FAILURE_CODE_V1
        }
    );
    assert!(!backend.active_sdma.contains_key(&submission));
    assert!(backend.active_sdma_streams.is_empty());
    assert!(backend.allocation_custody.is_empty());
    assert_eq!(backend.sdma_completion_reservations, 0);
    backend.release_submission_v1(submission).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.release_allocation_v1(source).unwrap();
    backend.release_allocation_v1(destination).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_quiescent_marker_capacity_covers_every_reserved_result() {
    let mut backend = KfdRuntimeBackendV1::mock();
    backend.quiescent_sdma_submissions.insert(10);
    backend.sdma_completion_reservations = 4;
    backend
        .quiescent_sdma_submissions
        .try_reserve(backend.sdma_completion_reservations)
        .unwrap();
    assert!(backend.quiescent_sdma_marker_capacity_is_reserved_v1());

    for submission in 11..15 {
        backend.sdma_completion_reservations -= 1;
        assert!(backend.quiescent_sdma_submissions.insert(submission));
        assert!(backend.quiescent_sdma_marker_capacity_is_reserved_v1());
    }
    assert_eq!(backend.quiescent_sdma_submissions.len(), 5);
    backend.quiescent_sdma_submissions.clear();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_native_copy_requires_initialization_and_scrub_retains_custody() {
    let mut allocation = AllocationRecordV1 {
        device: 7,
        kind: RuntimeMemoryKindV1::DeviceLocal,
        alignment: 8,
        bytes: Arc::from([0_u8; 16]),
        content_sha256: None,
        last_full_host_write: None,
        native_dirty: Vec::new(),
        sdma_storage: KfdRuntimeSdmaStorageV1::Synthetic,
        sdma_backed: true,
        sdma_initialized: false,
        sdma_shadow_dirty: false,
        persistent_storage_restore: None,
        #[cfg(test)]
        scripted_three_binding_replay: false,
    };
    let region = BackendMemoryRegionV1 {
        allocation: 1,
        access: RuntimeAccessV1::Read,
        byte_offset: 0,
        byte_len: 16,
    };
    assert!(!native_sdma_region_is_admitted_v1(
        Some(&allocation),
        7,
        region
    ));
    allocation.sdma_initialized = true;
    assert!(native_sdma_region_is_admitted_v1(
        Some(&allocation),
        7,
        region
    ));
    assert!(!native_sdma_region_is_admitted_v1(
        Some(&allocation),
        8,
        region
    ));
    assert!(!native_sdma_region_is_admitted_v1(
        Some(&allocation),
        7,
        BackendMemoryRegionV1 {
            byte_offset: 1,
            ..region
        }
    ));

    assert!(
        !allocation
            .sdma_storage
            .is_available_for_kind_v1(RuntimeMemoryKindV1::DeviceLocal)
    );
    allocation.sdma_storage =
        KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(17));
    assert!(
        !allocation
            .sdma_storage
            .is_available_for_kind_v1(RuntimeMemoryKindV1::DeviceLocal)
    );
}
