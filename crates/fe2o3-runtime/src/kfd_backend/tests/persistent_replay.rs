use super::*;

#[test]
fn scripted_persistent_compute_custody_repeats_without_materialization() {
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
    let expected = vec![0xa5_u8; byte_len];
    let expected_sha256: [u8; 32] = Sha256::digest(&expected).into();
    backend.write_allocation_v1(host, 0, &expected).unwrap();
    let (source, destination) =
        scripted_copy_regions_v1(host, device, u64::try_from(byte_len).unwrap());
    let copy = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);

    let ready = backend.take_h2d_ready_for_compute_v1(device, 101).unwrap();
    let physical = ready.owner.normalize();
    backend
        .restore_persistent_compute_completion_v1(
            device,
            101,
            physical,
            Gfx942PersistentComputeEffectV1::Read,
        )
        .unwrap();
    let record = &backend.allocations[&device];
    assert_eq!(record.content_sha256, Some(expected_sha256));
    assert!(!record.sdma_shadow_dirty);
    assert!(record.native_dirty.is_empty());

    let physical = match core::mem::replace(
        &mut backend.allocations.get_mut(&device).unwrap().sdma_storage,
        KfdRuntimeSdmaStorageV1::Synthetic,
    ) {
        KfdRuntimeSdmaStorageV1::Device(physical) => *physical,
        _ => panic!("read completion must restore the persistent device"),
    };
    let DirectionalSdmaDeviceOwnerV1::Scripted(physical) = physical else {
        panic!("scripted cycle must retain scripted storage")
    };
    backend.allocations.get_mut(&device).unwrap().sdma_storage =
        KfdRuntimeSdmaStorageV1::H2dReady(Box::new(PersistentComputeReadyStorageV1 {
            owner: PersistentComputeReadyOwnerV1::Scripted {
                device: physical,
                authenticated_sha256: expected_sha256,
            },
            promotion: None,
        }));
    let ready = backend.take_h2d_ready_for_compute_v1(device, 102).unwrap();
    let physical = ready.owner.normalize();
    backend
        .restore_persistent_compute_completion_v1(
            device,
            102,
            physical,
            Gfx942PersistentComputeEffectV1::ReadWrite,
        )
        .unwrap();
    let record = &backend.allocations[&device];
    let KfdRuntimeSdmaStorageV1::Device(physical) = &record.sdma_storage else {
        panic!("write completion must restore the persistent device")
    };
    assert_eq!(physical.scripted_bytes().unwrap(), expected);
    assert_eq!(record.content_sha256, None);
    assert!(record.last_full_host_write.is_none());
    assert!(record.sdma_shadow_dirty);
    assert!(record.native_dirty.is_empty());
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
        2
    );

    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(copy));
}

#[test]
fn scripted_persistent_compute_replays_same_launch_without_new_h2d() {
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
        .write_allocation_v1(host, 0, &vec![0x6a; byte_len])
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

    let first = submit_scripted_read_v1(
        &mut backend,
        stream,
        kernel,
        device,
        u64::try_from(byte_len).unwrap(),
        &[],
    );
    backend.flush_stream_v1(stream).unwrap();
    let mut first_status = BackendPollV1::Pending;
    for _ in 0..4 {
        first_status = backend.poll_v1(first).unwrap();
        if first_status != BackendPollV1::Pending {
            break;
        }
    }
    assert_eq!(first_status, BackendPollV1::Succeeded);
    assert!(backend.retained_persistent_dispatch.is_some());
    assert!(matches!(
        backend.allocations[&device].sdma_storage,
        KfdRuntimeSdmaStorageV1::Device(_)
    ));

    let second = submit_scripted_read_v1(
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
        Some(ActiveComputeExecutionV1::ScriptedPersistent { .. })
    ));
    let mut second_status = BackendPollV1::Pending;
    for _ in 0..4 {
        second_status = backend.poll_v1(second).unwrap();
        if second_status != BackendPollV1::Pending {
            break;
        }
    }
    assert_eq!(second_status, BackendPollV1::Succeeded);
    let performance = backend.last_launch_performance_v1().unwrap();
    assert_eq!(
        performance.data_path(),
        KfdRuntimeLaunchDataPathV1::PersistentDeviceReused
    );
    assert_eq!(performance.user_data_materializations(), 0);
    assert!(performance.persistent_control_reused());

    for submission in [copy, first, second] {
        backend.release_submission_v1(submission).unwrap();
    }
    assert!(backend.retained_persistent_dispatch.is_some());
    backend.unload_module_v1(module).unwrap();
    assert!(backend.retained_persistent_dispatch.is_none());
    release_scripted_direct_pair_v1(&mut backend, host, device);
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn scripted_cancelled_exact_replay_clears_retained_control_before_teardown() {
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
        .write_allocation_v1(host, 0, &vec![0x71; byte_len])
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

    let first = submit_scripted_read_v1(
        &mut backend,
        stream,
        kernel,
        device,
        u64::try_from(byte_len).unwrap(),
        &[],
    );
    backend.flush_stream_v1(stream).unwrap();
    let mut first_status = BackendPollV1::Pending;
    for _ in 0..4 {
        first_status = backend.poll_v1(first).unwrap();
        if first_status != BackendPollV1::Pending {
            break;
        }
    }
    assert_eq!(first_status, BackendPollV1::Succeeded);
    assert!(backend.retained_persistent_dispatch.is_some());

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
    let active = backend.active.as_ref().unwrap();
    assert!(matches!(
        active.execution,
        Some(ActiveComputeExecutionV1::ScriptedPersistentPrepared { .. })
    ));
    assert!(active.performance.persistent_control_reused());
    assert_eq!(
        backend.cancel_v1(cancelled).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert!(backend.retained_persistent_dispatch.is_none());
    assert!(matches!(
        backend.allocations[&device].sdma_storage,
        KfdRuntimeSdmaStorageV1::Device(_)
    ));
    assert_eq!(
        backend.poll_v1(cancelled).unwrap(),
        BackendPollV1::Failed { code: -2 }
    );

    for submission in [copy, first, cancelled] {
        backend.release_submission_v1(submission).unwrap();
    }
    backend.unload_module_v1(module).unwrap();
    release_scripted_direct_pair_v1(&mut backend, host, device);
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn scripted_changed_identity_without_fresh_h2d_releases_control_and_materializes() {
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
        .write_allocation_v1(host, 0, &vec![0x7b; byte_len])
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
    let first_kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();
    let changed_kernel_and_role = backend
        .resolve_kernel_v1(module, "vecadd", [8; 32])
        .unwrap();
    let first = submit_scripted_read_v1(
        &mut backend,
        stream,
        first_kernel,
        device,
        u64::try_from(byte_len).unwrap(),
        &[],
    );
    backend.flush_stream_v1(stream).unwrap();
    let mut first_status = BackendPollV1::Pending;
    for _ in 0..4 {
        first_status = backend.poll_v1(first).unwrap();
        if first_status != BackendPollV1::Pending {
            break;
        }
    }
    assert_eq!(first_status, BackendPollV1::Succeeded);
    assert!(backend.retained_persistent_dispatch.is_some());

    let explicit_kernarg = [0_u8; 16];
    let fallback = backend
        .submit_v1(BackendLaunchV1 {
            stream,
            kernel: changed_kernel_and_role,
            explicit_kernarg: &explicit_kernarg,
            bindings: &[BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation: device,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: u64::try_from(byte_len).unwrap(),
                },
                kernarg_byte_offset: 0,
            }],
            dependencies: &[],
            geometry: crate::RuntimeLaunchGeometryV1 {
                grid: [32, 1, 1],
                workgroup: [32, 1, 1],
                dynamic_shared_bytes: 0,
            },
            semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
        })
        .unwrap();
    backend.flush_stream_v1(stream).unwrap();
    assert!(matches!(
        backend
            .active
            .as_ref()
            .and_then(|active| active.execution.as_ref()),
        Some(ActiveComputeExecutionV1::ScriptedMaterialized)
    ));
    let mut fallback_status = BackendPollV1::Pending;
    for _ in 0..4 {
        fallback_status = backend.poll_v1(fallback).unwrap();
        if fallback_status != BackendPollV1::Pending {
            break;
        }
    }
    assert!(backend.retained_persistent_dispatch.is_none());
    assert_eq!(fallback_status, BackendPollV1::Succeeded);
    let performance = backend.last_launch_performance_v1().unwrap();
    assert_eq!(
        performance.data_path(),
        KfdRuntimeLaunchDataPathV1::Materialized
    );
    assert_eq!(performance.user_data_materializations(), 1);
    assert!(!performance.persistent_control_reused());

    for submission in [copy, first, fallback] {
        backend.release_submission_v1(submission).unwrap();
    }
    backend.unload_module_v1(module).unwrap();
    release_scripted_direct_pair_v1(&mut backend, host, device);
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn scripted_authenticated_h2d_replaces_mismatched_persistent_control() {
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
        .write_allocation_v1(host, 0, &vec![0x5d; byte_len])
        .unwrap();
    let (source, destination) =
        scripted_copy_regions_v1(host, device, u64::try_from(byte_len).unwrap());
    let first_copy = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(
        backend.poll_v1(first_copy).unwrap(),
        BackendPollV1::Succeeded
    );
    let module = backend
        .load_module_v1(7, &synthetic_cov6::module())
        .unwrap();
    let first_kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();
    let changed_kernel_and_role = backend
        .resolve_kernel_v1(module, "vecadd", [8; 32])
        .unwrap();
    let first = submit_scripted_read_v1(
        &mut backend,
        stream,
        first_kernel,
        device,
        u64::try_from(byte_len).unwrap(),
        &[],
    );
    backend.flush_stream_v1(stream).unwrap();
    assert_eq!(backend.poll_v1(first).unwrap(), BackendPollV1::Succeeded);
    let first_retained = backend.retained_persistent_dispatch.unwrap();

    let second_copy = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(
        backend.poll_v1(second_copy).unwrap(),
        BackendPollV1::Succeeded
    );
    let explicit_kernarg = [0_u8; 16];
    let changed = backend
        .submit_v1(BackendLaunchV1 {
            stream,
            kernel: changed_kernel_and_role,
            explicit_kernarg: &explicit_kernarg,
            bindings: &[BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation: device,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: u64::try_from(byte_len).unwrap(),
                },
                kernarg_byte_offset: 0,
            }],
            dependencies: &[],
            geometry: crate::RuntimeLaunchGeometryV1 {
                grid: [32, 1, 1],
                workgroup: [32, 1, 1],
                dynamic_shared_bytes: 0,
            },
            semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
        })
        .unwrap();
    backend.flush_stream_v1(stream).unwrap();
    assert!(matches!(
        backend
            .active
            .as_ref()
            .and_then(|active| active.execution.as_ref()),
        Some(ActiveComputeExecutionV1::ScriptedPersistent { .. })
    ));
    assert_eq!(backend.poll_v1(changed).unwrap(), BackendPollV1::Succeeded);
    let replaced = backend.retained_persistent_dispatch.unwrap();
    assert_eq!(replaced.allocation, device);
    assert_ne!(
        replaced.dispatch_shape_sha256,
        first_retained.dispatch_shape_sha256
    );
    let performance = backend.last_launch_performance_v1().unwrap();
    assert_eq!(
        performance.data_path(),
        KfdRuntimeLaunchDataPathV1::PersistentDeviceReused
    );
    assert_eq!(performance.user_data_materializations(), 0);
    assert!(!performance.persistent_control_reused());

    for submission in [first_copy, first, second_copy, changed] {
        backend.release_submission_v1(submission).unwrap();
    }
    backend.unload_module_v1(module).unwrap();
    release_scripted_direct_pair_v1(&mut backend, host, device);
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn scripted_persistent_publication_retry_stays_accepted_and_never_materializes() {
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
        .write_allocation_v1(host, 0, &vec![0xb6; byte_len])
        .unwrap();
    let (source, destination) =
        scripted_copy_regions_v1(host, device, u64::try_from(byte_len).unwrap());
    let copy = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
    let promotion = backend.last_ready_promotion_performance_v1().unwrap();
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
    backend.flush_stream_v1(stream).unwrap();
    assert!(matches!(
        backend
            .active
            .as_ref()
            .and_then(|active| active.execution.as_ref()),
        Some(ActiveComputeExecutionV1::ScriptedPersistentPrepared { .. })
    ));
    assert!(matches!(
        backend.allocations[&device].sdma_storage,
        KfdRuntimeSdmaStorageV1::ComputeInFlight(actual) if actual == compute
    ));
    assert_eq!(backend.scripted_persistent_publication_retries, 0);

    assert_eq!(backend.poll_v1(compute).unwrap(), BackendPollV1::Pending);
    assert!(matches!(
        backend
            .active
            .as_ref()
            .and_then(|active| active.execution.as_ref()),
        Some(ActiveComputeExecutionV1::ScriptedPersistent { .. })
    ));
    assert_eq!(backend.poll_v1(compute).unwrap(), BackendPollV1::Succeeded);
    let performance = backend.last_launch_performance_v1().unwrap();
    assert_eq!(
        performance.data_path(),
        KfdRuntimeLaunchDataPathV1::PersistentDeviceReused
    );
    assert_eq!(performance.user_data_materializations(), 0);
    assert_eq!(performance.ready_promotion(), Some(promotion));

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
