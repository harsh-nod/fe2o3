use super::*;

#[test]
fn disjoint_sdma_publishes_and_retains_timeout_custody_during_persistent_compute() {
    assert_disjoint_sdma_timeout_custody_during_persistent_compute_v1(
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
    );
}

#[test]
fn disjoint_d2h_sdma_retires_without_releasing_persistent_compute() {
    assert_disjoint_sdma_timeout_custody_during_persistent_compute_v1(
        Gfx942PersistentSdmaDirectionV1::DeviceToHost,
    );
}

#[test]
fn persistent_compute_flush_publishes_without_polling_disjoint_sdma() {
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
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Pending),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
    ];
    steps.extend(scripted_release_steps_v1());
    steps.extend(scripted_release_steps_v1());
    let (mut backend, compute_stream, compute_host, compute_device) =
        scripted_direct_backend_v1(byte_len, steps);
    let copy_stream = backend.create_stream_v1(7).unwrap();
    let (copy_host, copy_device) = add_scripted_direct_pair_v1(&mut backend, byte_len);
    let module = backend
        .load_module_v1(7, &synthetic_cov6::module())
        .unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();

    backend
        .write_allocation_v1(compute_host, 0, &vec![0x73; byte_len])
        .unwrap();
    let (source, destination) = scripted_copy_regions_v1(
        compute_host,
        compute_device,
        u64::try_from(byte_len).unwrap(),
    );
    let initial_copy = backend
        .copy_async_v1(compute_stream, source, destination, &[])
        .unwrap();
    assert_eq!(
        backend.poll_v1(initial_copy).unwrap(),
        BackendPollV1::Succeeded
    );
    backend
        .write_allocation_v1(copy_host, 0, &vec![0x84; byte_len])
        .unwrap();
    let (source, destination) =
        scripted_copy_regions_v1(copy_host, copy_device, u64::try_from(byte_len).unwrap());
    let published_copy = backend
        .copy_async_v1(copy_stream, source, destination, &[])
        .unwrap();
    assert!(matches!(
        backend.active_sdma[&published_copy].phase,
        ActiveSdmaPhaseV1::DirectionalPublished(_)
    ));
    assert_eq!(backend.published_sdma_submissions, [published_copy]);
    assert!(backend.published_sdma_index_is_consistent_v1());

    let compute = submit_scripted_read_v1(
        &mut backend,
        compute_stream,
        kernel,
        compute_device,
        u64::try_from(byte_len).unwrap(),
        &[],
    );
    assert_eq!(backend.poll_v1(compute).unwrap(), BackendPollV1::Pending);
    assert_eq!(backend.published_sdma_submissions, [published_copy]);
    assert!(backend.published_sdma_index_is_consistent_v1());
    assert!(backend.pending_compute.contains_key(&compute));
    assert!(backend.active.is_none());
    assert!(matches!(
        backend.allocations[&compute_device].sdma_storage,
        KfdRuntimeSdmaStorageV1::H2dReady(_)
    ));
    assert_eq!(backend.poll_v1(compute).unwrap(), BackendPollV1::Pending);
    assert_eq!(backend.published_sdma_submissions, [published_copy]);
    assert!(backend.published_sdma_index_is_consistent_v1());
    assert!(backend.pending_compute.contains_key(&compute));
    assert!(backend.active.is_none());

    let mut pending = backend.pending_compute.remove(&compute).unwrap();
    assert_eq!(backend.persistent_compute_sdma_blocker_v1(&pending), None);
    let retained = std::mem::replace(&mut pending.retained_allocations, Box::new([]));
    assert_eq!(
        backend.persistent_compute_sdma_blocker_v1(&pending),
        Some(published_copy)
    );
    pending.retained_allocations = retained;
    for allocation in [compute_device, copy_host, copy_device] {
        let owner = backend.allocation_custody[&allocation].owners[0];
        backend
            .allocation_custody
            .get_mut(&allocation)
            .unwrap()
            .owners[0]
            .submission += 1;
        assert_eq!(
            backend.persistent_compute_sdma_blocker_v1(&pending),
            Some(published_copy)
        );
        backend
            .allocation_custody
            .get_mut(&allocation)
            .unwrap()
            .owners[0] = owner;
    }
    for allocation in [copy_host, copy_device] {
        backend
            .allocations
            .get_mut(&allocation)
            .unwrap()
            .sdma_storage =
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(published_copy + 1));
        assert_eq!(
            backend.persistent_compute_sdma_blocker_v1(&pending),
            Some(published_copy)
        );
        backend
            .allocations
            .get_mut(&allocation)
            .unwrap()
            .sdma_storage =
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(published_copy));
    }
    backend.allocations.get_mut(&copy_host).unwrap().kind = RuntimeMemoryKindV1::DeviceLocal;
    assert_eq!(
        backend.persistent_compute_sdma_blocker_v1(&pending),
        Some(published_copy)
    );
    backend.allocations.get_mut(&copy_host).unwrap().kind = RuntimeMemoryKindV1::HostVisible;
    assert_eq!(backend.persistent_compute_sdma_blocker_v1(&pending), None);
    backend.pending_compute.insert(compute, pending);

    backend.flush_stream_v1(compute_stream).unwrap();
    assert_eq!(
        backend.active.as_ref().map(|active| active.id),
        Some(compute)
    );
    assert_eq!(
        backend.poll_v1(published_copy).unwrap(),
        BackendPollV1::Pending
    );
    assert!(backend.persistent_compute_is_active_v1());
    assert_eq!(backend.published_sdma_submissions, [published_copy]);
    assert_eq!(
        backend.poll_v1(published_copy).unwrap(),
        BackendPollV1::Succeeded
    );
    assert!(backend.persistent_compute_is_active_v1());
    assert!(backend.published_sdma_submissions.is_empty());
    assert!(backend.published_sdma_index_is_consistent_v1());
    assert_eq!(backend.poll_v1(compute).unwrap(), BackendPollV1::Succeeded);
    for submission in [initial_copy, published_copy, compute] {
        backend.release_submission_v1(submission).unwrap();
    }
    release_scripted_direct_pair_v1(&mut backend, compute_host, compute_device);
    release_scripted_direct_pair_v1(&mut backend, copy_host, copy_device);
    backend.unload_module_v1(module).unwrap();
    backend.destroy_stream_v1(compute_stream).unwrap();
    backend.destroy_stream_v1(copy_stream).unwrap();
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 0);
    assert_eq!(driver.unexpected_drops(), 0);
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn disjoint_three_binding_compute_sdma_publish_in_both_orders() {
    for (direction, copy_first) in [
        (Gfx942PersistentSdmaDirectionV1::HostToDevice, false),
        (Gfx942PersistentSdmaDirectionV1::HostToDevice, true),
        (Gfx942PersistentSdmaDirectionV1::DeviceToHost, false),
        (Gfx942PersistentSdmaDirectionV1::DeviceToHost, true),
    ] {
        let byte_len = 64_usize;
        let mut steps = vec![
            ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len,
            },
            scripted_submit_step_v1(
                direction,
                0,
                0,
                byte_len as u32,
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Pending),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
            ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
        ];
        for _ in 0..3 {
            steps.extend([
                ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ]);
        }
        steps.extend(scripted_release_steps_v1());
        let (mut backend, compute_stream, allocations) =
            scripted_three_binding_backend_with_steps_v1(byte_len, steps);
        let copy_stream = backend.create_stream_v1(7).unwrap();
        let (host, device) = add_scripted_direct_pair_v1(&mut backend, byte_len);
        let module = backend
            .load_module_v1(7, &synthetic_cov6::three_binding_module())
            .unwrap();
        let kernel = backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        backend
            .write_allocation_v1(host, 0, &vec![0x64; byte_len])
            .unwrap();
        let (mut source, mut destination) = scripted_copy_regions_v1(host, device, byte_len as u64);
        if direction == Gfx942PersistentSdmaDirectionV1::DeviceToHost {
            source.allocation = device;
            destination.allocation = host;
        }
        let early_copy = copy_first.then(|| {
            backend
                .copy_async_v1(copy_stream, source, destination, &[])
                .unwrap()
        });
        let compute = submit_scripted_three_binding_v1(
            &mut backend,
            compute_stream,
            kernel,
            allocations,
            byte_len as u64,
        );
        backend.flush_stream_v1(compute_stream).unwrap();
        let copy = early_copy.unwrap_or_else(|| {
            backend
                .copy_async_v1(copy_stream, source, destination, &[])
                .unwrap()
        });
        assert!(backend.persistent_compute_is_active_v1());
        assert!(matches!(
            backend.active.as_ref().unwrap().execution,
            Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistent { .. })
        ));
        assert_eq!(backend.published_sdma_submissions, [copy]);
        assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Pending);
        assert!(backend.persistent_compute_is_active_v1());
        if copy_first {
            assert_eq!(backend.poll_v1(compute).unwrap(), BackendPollV1::Succeeded);
        }
        assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
        assert!(backend.allocations[&destination.allocation].sdma_shadow_dirty);
        assert!(!backend.allocations[&source.allocation].sdma_shadow_dirty);
        assert!(matches!(
            backend.allocations[&host].sdma_storage,
            KfdRuntimeSdmaStorageV1::Host(_)
        ));
        assert!(matches!(
            backend.allocations[&device].sdma_storage,
            KfdRuntimeSdmaStorageV1::Device(_)
        ));
        if !copy_first {
            assert!(backend.persistent_compute_is_active_v1());
            for allocation in allocations {
                assert!(backend.allocation_custody.contains_key(&allocation));
            }
            assert_eq!(backend.poll_v1(compute).unwrap(), BackendPollV1::Succeeded);
        }
        for submission in [copy, compute] {
            backend.release_submission_v1(submission).unwrap();
        }
        assert!(backend.allocation_custody.is_empty());
        backend.unload_module_v1(module).unwrap();
        for allocation in allocations {
            backend
                .allocations
                .get_mut(&allocation)
                .unwrap()
                .sdma_backed = false;
            backend.release_allocation_v1(allocation).unwrap();
        }
        release_scripted_direct_pair_v1(&mut backend, host, device);
        backend.destroy_stream_v1(compute_stream).unwrap();
        backend.destroy_stream_v1(copy_stream).unwrap();
        let driver = backend.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted());
        assert_eq!(driver.live_owner_count(), 0);
        assert_eq!(driver.unexpected_drops(), 0);
        backend.shutdown_native_v1().unwrap();
    }
}

#[test]
fn disjoint_sdma_dependency_and_invalid_coexistence_rosters_never_publish() {
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
            byte_len as u32,
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len,
        },
    ];
    steps.extend(scripted_release_steps_v1());
    steps.extend(scripted_release_steps_v1());
    let (mut backend, compute_stream, compute_host, compute_device) =
        scripted_direct_backend_v1(byte_len, steps);
    let copy_stream = backend.create_stream_v1(7).unwrap();
    let (copy_host, copy_device) = add_scripted_direct_pair_v1(&mut backend, byte_len);
    let module = backend
        .load_module_v1(7, &synthetic_cov6::module())
        .unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();
    backend
        .write_allocation_v1(compute_host, 0, &vec![0x71; byte_len])
        .unwrap();
    let (source, destination) =
        scripted_copy_regions_v1(compute_host, compute_device, byte_len as u64);
    let initial_copy = backend
        .copy_async_v1(compute_stream, source, destination, &[])
        .unwrap();
    assert_eq!(
        backend.poll_v1(initial_copy).unwrap(),
        BackendPollV1::Succeeded
    );
    backend
        .write_allocation_v1(copy_host, 0, &vec![0x82; byte_len])
        .unwrap();
    let compute = submit_scripted_read_v1(
        &mut backend,
        compute_stream,
        kernel,
        compute_device,
        byte_len as u64,
        &[],
    );
    backend.flush_stream_v1(compute_stream).unwrap();
    let event = backend.record_event_v1(compute_stream, compute).unwrap();
    let (source, destination) = scripted_copy_regions_v1(copy_host, copy_device, byte_len as u64);
    let copy = backend
        .copy_async_v1(copy_stream, source, destination, &[event])
        .unwrap();
    assert!(matches!(
        backend.active_sdma[&copy].phase,
        ActiveSdmaPhaseV1::Ready
    ));
    assert!(backend.published_sdma_submissions.is_empty());
    let mut ready = backend.active_sdma.remove(&copy).unwrap();
    assert!(backend.sdma_can_coexist_with_persistent_compute_v1(&ready));
    for allocation in [compute_device, copy_host, copy_device] {
        let owner = backend.allocation_custody[&allocation].owners[0];
        backend
            .allocation_custody
            .get_mut(&allocation)
            .unwrap()
            .owners[0]
            .stream += 1;
        assert!(!backend.sdma_can_coexist_with_persistent_compute_v1(&ready));
        backend
            .allocation_custody
            .get_mut(&allocation)
            .unwrap()
            .owners[0] = owner;
    }
    let roster = std::mem::take(&mut backend.active.as_mut().unwrap().allocations);
    assert!(!backend.sdma_can_coexist_with_persistent_compute_v1(&ready));
    backend.active.as_mut().unwrap().allocations = roster;
    backend
        .active
        .as_mut()
        .unwrap()
        .allocations
        .insert(copy_device);
    assert!(!backend.sdma_can_coexist_with_persistent_compute_v1(&ready));
    backend
        .active
        .as_mut()
        .unwrap()
        .allocations
        .remove(&copy_device);
    backend
        .allocations
        .get_mut(&compute_device)
        .unwrap()
        .sdma_storage = KfdRuntimeSdmaStorageV1::ComputeInFlight(compute + 1);
    assert!(!backend.sdma_can_coexist_with_persistent_compute_v1(&ready));
    backend
        .allocations
        .get_mut(&compute_device)
        .unwrap()
        .sdma_storage = KfdRuntimeSdmaStorageV1::ComputeInFlight(compute);
    ready.destination = compute_device;
    assert!(!backend.sdma_can_coexist_with_persistent_compute_v1(&ready));
    ready.destination = copy_device;
    backend.allocations.get_mut(&copy_host).unwrap().kind = RuntimeMemoryKindV1::DeviceLocal;
    assert!(!backend.sdma_can_coexist_with_persistent_compute_v1(&ready));
    backend.allocations.get_mut(&copy_host).unwrap().kind = RuntimeMemoryKindV1::HostVisible;
    backend.auxiliary_compute_lanes[0].active = Some(pipelined_active_for_test_v1(9000));
    assert!(!backend.sdma_can_coexist_with_persistent_compute_v1(&ready));
    backend.auxiliary_compute_lanes[0].active = None;
    assert!(backend.sdma_can_coexist_with_persistent_compute_v1(&ready));
    backend.active_sdma.insert(copy, ready);
    assert_eq!(
        backend.cancel_v1(copy).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert_eq!(backend.poll_v1(compute).unwrap(), BackendPollV1::Succeeded);
    backend.release_event_v1(event).unwrap();
    for submission in [initial_copy, copy, compute] {
        backend.release_submission_v1(submission).unwrap();
    }
    assert!(backend.allocation_custody.is_empty());
    release_scripted_direct_pair_v1(&mut backend, compute_host, compute_device);
    release_scripted_direct_pair_v1(&mut backend, copy_host, copy_device);
    backend.unload_module_v1(module).unwrap();
    backend.destroy_stream_v1(compute_stream).unwrap();
    backend.destroy_stream_v1(copy_stream).unwrap();
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 0);
    assert_eq!(driver.unexpected_drops(), 0);
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn disjoint_compute_sdma_failures_preserve_the_other_native_owner() {
    #[derive(Clone, Copy)]
    enum Failure {
        UnpublishedCopy,
        CopyObservation,
        ComputeObservation,
        CopyRestoration,
    }
    for failure in [
        Failure::UnpublishedCopy,
        Failure::CopyObservation,
        Failure::ComputeObservation,
        Failure::CopyRestoration,
    ] {
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
                byte_len as u32,
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
            ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len,
            },
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                byte_len as u32,
                if matches!(failure, Failure::UnpublishedCopy) {
                    ScriptedFailureModeV1::Retryable
                } else {
                    ScriptedFailureModeV1::Success
                },
            ),
        ];
        match failure {
            Failure::UnpublishedCopy => {
                steps.push(scripted_submit_step_v1(
                    Gfx942PersistentSdmaDirectionV1::HostToDevice,
                    0,
                    0,
                    byte_len as u32,
                    ScriptedFailureModeV1::Success,
                ));
                steps.push(ScriptedSdmaStepV1::Poll(
                    ScriptedExecutionOutcomeV1::Completed {
                        direction: None,
                        copy_bytes: None,
                    },
                ));
                steps.extend(scripted_release_steps_v1());
                steps.extend(scripted_release_steps_v1());
            }
            Failure::CopyObservation => {
                steps.push(ScriptedSdmaStepV1::Poll(
                    ScriptedExecutionOutcomeV1::Retryable,
                ));
            }
            Failure::ComputeObservation => {}
            Failure::CopyRestoration => {
                steps.push(ScriptedSdmaStepV1::Poll(
                    ScriptedExecutionOutcomeV1::Completed {
                        direction: None,
                        copy_bytes: None,
                    },
                ));
            }
        }
        let (mut backend, compute_stream, compute_host, compute_device) =
            scripted_direct_backend_v1(byte_len, steps);
        let copy_stream = backend.create_stream_v1(7).unwrap();
        let (copy_host, copy_device) = add_scripted_direct_pair_v1(&mut backend, byte_len);
        let module = backend
            .load_module_v1(7, &synthetic_cov6::module())
            .unwrap();
        let kernel = backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        backend
            .write_allocation_v1(compute_host, 0, &vec![0x47; byte_len])
            .unwrap();
        let (source, destination) =
            scripted_copy_regions_v1(compute_host, compute_device, byte_len as u64);
        let initial_copy = backend
            .copy_async_v1(compute_stream, source, destination, &[])
            .unwrap();
        assert_eq!(
            backend.poll_v1(initial_copy).unwrap(),
            BackendPollV1::Succeeded
        );
        backend
            .write_allocation_v1(copy_host, 0, &vec![0x58; byte_len])
            .unwrap();
        let compute = submit_scripted_read_v1(
            &mut backend,
            compute_stream,
            kernel,
            compute_device,
            byte_len as u64,
            &[],
        );
        backend.flush_stream_v1(compute_stream).unwrap();
        let (source, destination) =
            scripted_copy_regions_v1(copy_host, copy_device, byte_len as u64);
        let copy = backend
            .copy_async_v1(copy_stream, source, destination, &[])
            .unwrap();
        assert!(backend.persistent_compute_is_active_v1());
        if matches!(failure, Failure::UnpublishedCopy) {
            assert!(matches!(
                backend.poll_v1(copy).unwrap(),
                BackendPollV1::Failed { .. }
            ));
            assert!(backend.published_sdma_submissions.is_empty());
            assert!(!backend.allocation_custody.contains_key(&copy_host));
            assert!(!backend.allocation_custody.contains_key(&copy_device));
            assert!(backend.allocation_custody.contains_key(&compute_device));
            backend.release_submission_v1(copy).unwrap();
            let retried = backend
                .copy_async_v1(copy_stream, source, destination, &[])
                .unwrap();
            assert_ne!(retried, copy);
            assert_eq!(backend.published_sdma_submissions, [retried]);
            backend.flush_stream_v1(copy_stream).unwrap();
            assert_eq!(backend.poll_v1(retried).unwrap(), BackendPollV1::Succeeded);
            assert!(backend.persistent_compute_is_active_v1());
            assert_eq!(backend.poll_v1(compute).unwrap(), BackendPollV1::Succeeded);
            for submission in [initial_copy, retried, compute] {
                backend.release_submission_v1(submission).unwrap();
            }
            assert!(backend.allocation_custody.is_empty());
            release_scripted_direct_pair_v1(&mut backend, compute_host, compute_device);
            release_scripted_direct_pair_v1(&mut backend, copy_host, copy_device);
            backend.unload_module_v1(module).unwrap();
            backend.destroy_stream_v1(compute_stream).unwrap();
            backend.destroy_stream_v1(copy_stream).unwrap();
            let driver = backend.scripted_sdma.as_ref().unwrap();
            assert!(driver.is_exhausted());
            assert_eq!(driver.live_owner_count(), 0);
            assert_eq!(driver.unexpected_drops(), 0);
            backend.shutdown_native_v1().unwrap();
            continue;
        }
        let owners = [compute_device, copy_host, copy_device]
            .map(|allocation| backend.allocation_custody[&allocation].owners.clone());
        let observed = if matches!(failure, Failure::ComputeObservation) {
            backend.scripted_persistent_transition_failure =
                Some(ScriptedPersistentTransitionFailureV1::Poll);
            compute
        } else {
            if matches!(failure, Failure::CopyRestoration) {
                backend
                    .allocations
                    .get_mut(&copy_host)
                    .unwrap()
                    .sdma_storage =
                    KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(copy + 1));
            }
            copy
        };
        assert!(matches!(
            backend.poll_v1(observed),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(backend.terminal);
        for (allocation, expected) in [compute_device, copy_host, copy_device]
            .into_iter()
            .zip(owners)
        {
            assert_eq!(backend.allocation_custody[&allocation].owners, expected);
        }
        if matches!(failure, Failure::ComputeObservation) {
            assert_eq!(backend.published_sdma_submissions, [copy]);
        } else {
            assert!(backend.persistent_compute_is_active_v1());
            assert!(backend.terminal_sdma_custody.is_some());
        }
        for submission in [compute, copy] {
            assert!(matches!(
                backend.release_submission_v1(submission),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
        }
        let driver = backend.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted());
        assert_eq!(driver.live_owner_count(), 4);
        assert_eq!(driver.unexpected_drops(), 0);
        disarm_scripted_drop_after_inspection_v1(&mut backend);
    }
}
