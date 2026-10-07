use super::*;

#[test]
fn launch_snapshot_copies_only_the_alignment_preserving_bound_window() {
    let bytes = (0_u8..64).collect::<Vec<_>>();
    let mut allocations = AllocationTableV1::default();
    allocations.insert(
        9,
        AllocationRecordV1 {
            device: 7,
            kind: RuntimeMemoryKindV1::HostVisible,
            alignment: 8,
            bytes: bytes.into(),
            content_sha256: None,
            last_full_host_write: None,
            native_dirty: Vec::new(),
            sdma_storage: KfdRuntimeSdmaStorageV1::Synthetic,
            sdma_backed: false,
            sdma_initialized: false,
            sdma_shadow_dirty: false,
            persistent_storage_restore: None,
            #[cfg(test)]
            scripted_three_binding_replay: false,
        },
    );
    let bindings = [
        BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: 9,
                access: RuntimeAccessV1::Read,
                byte_offset: 19,
                byte_len: 4,
            },
            kernarg_byte_offset: 0,
        },
        BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: 9,
                access: RuntimeAccessV1::Write,
                byte_offset: 40,
                byte_len: 4,
            },
            kernarg_byte_offset: 8,
        },
    ];

    let staged = snapshot_bound_data_v1(&allocations, &bindings, 7).unwrap();
    assert_eq!(staged.data.len(), 1);
    assert_eq!(staged.data[0].allocation_offset, 16);
    assert_eq!(staged.data[0].content_sha256, None);
    assert_eq!(staged.data[0].bytes(), &allocations[&9].bytes[16..44]);
    assert_eq!(
        staged.placements[&9],
        StagedPlacementV1 {
            data_index: 0,
            allocation_offset: 16,
        }
    );
    assert!(staged.data[0].bytes().len() < allocations[&9].bytes.len());
}

#[test]
fn valid_cov6_module_reaches_cached_launch_and_native_acquisition_boundary() {
    let image = synthetic_cov6::module();
    let mut backend = KfdRuntimeBackendV1::mock_with_semantic_authority_v1();
    let stream = backend.create_stream_v1(7).unwrap();
    let module = backend.load_module_v1(7, &image).unwrap();
    assert_eq!(backend.modules[&module].validated.validation_passes(), 1);
    let kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();
    assert_eq!(
        backend.kernels[&kernel].validated.semantic_binding_passes(),
        1
    );
    let allocation = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 64, 8)
        .unwrap();
    let initial = (0_u8..64).collect::<Vec<_>>();
    backend
        .write_allocation_v1(allocation, 0, &initial)
        .unwrap();

    let mut explicit_kernarg = [0_u8; 16];
    explicit_kernarg[8..].copy_from_slice(&13_u64.to_le_bytes());
    let bindings = [BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation,
            access: RuntimeAccessV1::Read,
            byte_offset: 11,
            byte_len: 13,
        },
        kernarg_byte_offset: 0,
    }];
    let geometry = crate::RuntimeLaunchGeometryV1 {
        grid: [64, 1, 1],
        workgroup: [64, 1, 1],
        dynamic_shared_bytes: 0,
    };
    let prepared = backend
        .prepare_launch(
            BackendLaunchV1 {
                stream,
                kernel,
                explicit_kernarg: &explicit_kernarg,
                bindings: &bindings,
                dependencies: &[],
                geometry,
                semantic_launch: KfdRuntimeSemanticLaunchV1::Atomic(atomic_contract_v1()),
            },
            false,
            false,
        )
        .unwrap();
    let PreparedLaunchStorageV1::Materialized(data) = &prepared.storage else {
        panic!("ordinary test launch must retain materialized storage")
    };
    assert_eq!(data.len(), 1);
    assert_eq!(data[0].allocation_offset, 8);
    assert_eq!(data[0].bytes(), &initial[8..24]);
    let reconciled =
        build_program_v1(&prepared.program, prepared.signature, &prepared.abi_rows).unwrap();
    assert!(reconciled.dispatch_abi_identity().is_some());
    drop(reconciled);
    drop(prepared);

    assert!(matches!(
        backend.submit_v1(BackendLaunchV1 {
            stream,
            kernel,
            explicit_kernarg: &explicit_kernarg,
            bindings: &bindings,
            dependencies: &[],
            geometry,
            semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
        }),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported
                && error.detail() == "the admitted KFD queue lifecycle has already retired"
    ));

    backend.release_allocation_v1(allocation).unwrap();
    backend.unload_module_v1(module).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn launch_authority_panic_fails_before_publication_and_releases_custody() {
    let image = synthetic_cov6::module();
    let mut backend = KfdRuntimeBackendV1::mock_with_panicking_authority_v1();
    let stream = backend.create_stream_v1(7).unwrap();
    let module = backend.load_module_v1(7, &image).unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();
    let allocation = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 64, 8)
        .unwrap();
    {
        let record = backend.allocations.get_mut(&allocation).unwrap();
        record.sdma_backed = true;
        record.sdma_initialized = true;
    }
    backend.native_available = true;
    let mut explicit_kernarg = [0_u8; 16];
    explicit_kernarg[8..].copy_from_slice(&13_u64.to_le_bytes());
    let bindings = [BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation,
            access: RuntimeAccessV1::Read,
            byte_offset: 11,
            byte_len: 13,
        },
        kernarg_byte_offset: 0,
    }];
    let submission = backend
        .submit_v1(BackendLaunchV1 {
            stream,
            kernel,
            explicit_kernarg: &explicit_kernarg,
            bindings: &bindings,
            dependencies: &[],
            geometry: crate::RuntimeLaunchGeometryV1 {
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_shared_bytes: 0,
            },
            semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
        })
        .unwrap();

    assert_eq!(
        backend.poll_v1(submission).unwrap(),
        BackendPollV1::Failed { code: -1 }
    );
    assert!(backend.pending_compute.is_empty());
    assert!(backend.pending_compute_streams.is_empty());
    assert!(backend.stream_compute_lanes.is_empty());
    assert!(backend.allocation_custody.is_empty());
    assert!(backend.compute_module_retain_counts.is_empty());
    assert!(backend.compute_dependency_retain_counts.is_empty());
    assert_eq!(backend.compute_completion_reservations, 0);
    assert!(backend.active.is_none());
    assert!(
        backend
            .auxiliary_compute_lanes
            .iter()
            .all(|lane| lane.active.is_none() && lane.owner_stream.is_none())
    );

    backend.release_submission_v1(submission).unwrap();
    backend.release_allocation_v1(allocation).unwrap();
    backend.unload_module_v1(module).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.native_available = false;
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn direct_kfd_spi_enforces_kernarg_and_binding_bounds_before_custody() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    backend.native_available = true;
    let geometry = crate::RuntimeLaunchGeometryV1 {
        grid: [1, 1, 1],
        workgroup: [1, 1, 1],
        dynamic_shared_bytes: 0,
    };
    let oversized_kernarg = vec![0_u8; MAX_RUNTIME_EXPLICIT_KERNARG_BYTES_V1 + 1];
    assert!(matches!(
        backend.submit_v1(BackendLaunchV1 {
            stream,
            kernel: 99,
            explicit_kernarg: &oversized_kernarg,
            bindings: &[],
            dependencies: &[],
            geometry,
            semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
        }),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
    ));
    let binding = BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation: 99,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 1,
        },
        kernarg_byte_offset: 0,
    };
    let oversized_bindings = vec![binding; fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1 + 1];
    assert!(matches!(
        backend.submit_v1(BackendLaunchV1 {
            stream,
            kernel: 99,
            explicit_kernarg: &[],
            bindings: &oversized_bindings,
            dependencies: &[],
            geometry,
            semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
        }),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
    ));
    assert!(backend.pending_compute.is_empty());
    assert!(backend.allocation_custody.is_empty());
    backend.native_available = false;
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn logical_streams_and_events_enforce_submission_ownership() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let left = backend.create_stream_v1(7).unwrap();
    let right = backend.create_stream_v1(7).unwrap();
    backend.submissions.insert(
        99,
        SubmissionRecordV1 {
            stream: left,
            status: BackendPollV1::Succeeded,
            dependency_depth: 1,
            profile_dispatch_published: false,
        },
    );
    let event = backend.record_event_v1(left, 99).unwrap();
    assert_eq!(
        backend.collect_compute_dependencies_v1(&[event]).unwrap(),
        vec![99].into_boxed_slice()
    );
    assert!(matches!(
        backend.record_event_v1(right, 99),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    backend.release_event_v1(event).unwrap();
    backend.release_submission_v1(99).unwrap();
    assert!(matches!(
        backend.release_submission_v1(99),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
}

#[test]
fn logical_stream_destroy_and_recreate_preserves_backend_lifecycle() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    let replacement = backend.create_stream_v1(7).unwrap();
    backend.destroy_stream_v1(replacement).unwrap();
    backend.shutdown_native_v1().unwrap();
    assert!(matches!(
        backend.create_stream_v1(7),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported
    ));
}

#[test]
fn terminal_state_stays_terminal_across_the_spi() {
    let mut backend = KfdRuntimeBackendV1::mock();
    backend.terminal = true;
    assert!(matches!(
        backend.enumerate_devices_v1(),
        Err(RuntimeBackendFailureV1::Terminal(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
    ));
    // Production drop aborts to enact the terminal process-teardown
    // contract. This synthetic backend owns no native resource.
    std::mem::forget(backend);
}

#[test]
fn live_event_retains_completed_submission_state() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    backend.submissions.insert(
        42,
        SubmissionRecordV1 {
            stream,
            status: BackendPollV1::Succeeded,
            dependency_depth: 1,
            profile_dispatch_published: false,
        },
    );
    let event = backend.record_event_v1(stream, 42).unwrap();
    assert!(matches!(
        backend.release_submission_v1(42),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
    ));
    backend.release_event_v1(event).unwrap();
    backend.release_submission_v1(42).unwrap();
}

#[test]
fn deadline_wait_returns_pending_without_a_poll_budget_loop() {
    let start = Instant::now();
    let deadline = start + Duration::from_millis(2);
    let mut polls = 0_u32;
    let status = wait_with_deadline_v1(deadline, || {
        polls += 1;
        Ok::<_, ()>(BackendPollV1::Pending)
    })
    .unwrap();
    assert_eq!(status, BackendPollV1::Pending);
    assert!(Instant::now() >= deadline);
    assert!(polls < 10_000);
}

#[test]
fn deadline_wait_stops_on_success() {
    let mut polls = 0;
    let status = wait_with_deadline_v1(Instant::now() + Duration::from_secs(1), || {
        polls += 1;
        Ok::<_, ()>(if polls == 3 {
            BackendPollV1::Succeeded
        } else {
            BackendPollV1::Pending
        })
    })
    .unwrap();
    assert_eq!(status, BackendPollV1::Succeeded);
    assert_eq!(polls, 3);
}
