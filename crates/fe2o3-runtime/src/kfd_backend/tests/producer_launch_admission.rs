use super::*;

#[test]
fn producer_launch_active_three_binding_inputs_wait_without_materialization() {
    for cross_stream in [false, true] {
        let mut fixture = ScriptedActiveProducerFixtureV1::new(cross_stream);
        let consumer = fixture.submit().unwrap();
        assert_eq!(
            fixture.backend.active.as_ref().unwrap().id,
            fixture.producer
        );
        let pending = &fixture.backend.pending_compute[&consumer];
        assert_eq!(&*pending.explicit_success_dependencies, &[fixture.producer]);
        assert_eq!(pending.explicit_dependency_cursor, 0);
        fixture.backend.release_event_v1(fixture.event).unwrap();
        assert!(
            matches!(fixture.backend.release_submission_v1(fixture.producer),
                Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
        );
        assert_eq!(
            fixture.backend.poll_v1(consumer).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(
            fixture.backend.submissions[&fixture.producer].status,
            BackendPollV1::Succeeded
        );
        fixture
            .backend
            .flush_stream_v1(fixture.launch.stream)
            .unwrap();
        assert_eq!(
            fixture
                .backend
                .wait_v1(consumer, Instant::now() + Duration::from_secs(1))
                .unwrap(),
            BackendPollV1::Succeeded
        );
        let performance = fixture.backend.last_launch_performance_v1().unwrap();
        assert_eq!(
            performance.data_path(),
            KfdRuntimeLaunchDataPathV1::PersistentDeviceReused
        );
        assert_eq!(performance.user_data_materializations(), 0);
        fixture.finish(Some(consumer));
    }
}

#[test]
fn producer_launch_active_three_binding_cancellation_keeps_parent_owners() {
    let mut fixture = ScriptedActiveProducerFixtureV1::new(true);
    let consumer = fixture.submit().unwrap();
    fixture.backend.release_event_v1(fixture.event).unwrap();
    assert_eq!(
        fixture.backend.cancel_v1(consumer).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert_eq!(
        fixture.backend.active.as_ref().unwrap().id,
        fixture.producer
    );
    assert!(
        !fixture
            .backend
            .compute_dependency_retain_counts
            .contains_key(&fixture.producer)
    );
    assert!(
        !fixture
            .backend
            .allocation_custody
            .contains_key(&fixture.allocations[3])
    );
    for allocation in fixture.allocations.into_iter().take(3) {
        assert!(
            matches!(fixture.backend.allocations[&allocation].sdma_storage,
                KfdRuntimeSdmaStorageV1::ComputeInFlight(owner) if owner == fixture.producer)
        );
    }
    fixture.finish(Some(consumer));
}

#[test]
fn producer_launch_active_three_binding_rejects_missing_or_foreign_authority() {
    let mut fixture = ScriptedActiveProducerFixtureV1::new(true);
    let launch = &fixture.launch;
    for (dependencies, mode) in [
        (vec![], ComputeInputAdmissionV1::ExactProducers),
        (
            vec![fixture.producer + 100],
            ComputeInputAdmissionV1::ExactProducers,
        ),
        (vec![fixture.producer], ComputeInputAdmissionV1::Ready),
    ] {
        assert!(
            matches!(fixture.backend.validate_compute_launch_v1(&launch.borrowed(), &dependencies, mode),
                Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch)
        );
    }
    for input in [fixture.allocations[2], fixture.allocations[1]] {
        let custody = fixture.backend.allocation_custody.remove(&input).unwrap();
        assert!(
            fixture
                .backend
                .validate_compute_launch_v1(
                    &launch.borrowed(),
                    &[fixture.producer],
                    ComputeInputAdmissionV1::ExactProducers
                )
                .is_err()
        );
        fixture.backend.allocation_custody.insert(input, custody);
        fixture
            .backend
            .allocations
            .get_mut(&input)
            .unwrap()
            .sdma_initialized = false;
        assert!(
            fixture
                .backend
                .validate_compute_launch_v1(
                    &launch.borrowed(),
                    &[fixture.producer],
                    ComputeInputAdmissionV1::ExactProducers
                )
                .is_err()
        );
        fixture
            .backend
            .allocations
            .get_mut(&input)
            .unwrap()
            .sdma_initialized = true;
    }
    let execution = fixture.backend.active.as_mut().unwrap().execution.take();
    assert!(
        fixture
            .backend
            .validate_compute_launch_v1(
                &launch.borrowed(),
                &[fixture.producer],
                ComputeInputAdmissionV1::ExactProducers
            )
            .is_err()
    );
    fixture.backend.active.as_mut().unwrap().execution = execution;
    assert!(fixture.backend.pending_compute.is_empty());
    fixture.finish(None);
}

#[test]
fn producer_launch_active_three_binding_requires_restored_ready_backing() {
    let mut fixture = ScriptedActiveProducerFixtureV1::new(true);
    let consumer = fixture.submit().unwrap();
    assert_eq!(
        fixture.backend.poll_v1(fixture.producer).unwrap(),
        BackendPollV1::Succeeded
    );
    fixture
        .backend
        .allocations
        .get_mut(&fixture.allocations[2])
        .unwrap()
        .sdma_initialized = false;
    assert_eq!(
        fixture.backend.poll_v1(consumer).unwrap(),
        BackendPollV1::Pending
    );
    assert!(
        matches!(fixture.backend.flush_stream_v1(fixture.launch.stream),
            Err(RuntimeBackendFailureV1::Quiescent(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Native)
    );
    assert_eq!(
        fixture.backend.poll_v1(consumer).unwrap(),
        BackendPollV1::Failed { code: -1 }
    );
    assert!(fixture.backend.active.is_none());
    fixture
        .backend
        .allocations
        .get_mut(&fixture.allocations[2])
        .unwrap()
        .sdma_initialized = true;
    fixture.finish(Some(consumer));
}

#[test]
fn producer_launch_active_three_binding_public_rejections_preserve_custody() {
    let mut fixture = ScriptedActiveProducerFixtureV1::new(true);
    let next = fixture.backend.next_handle;
    let ordinary = BackendLaunchV1 {
        dependencies: &[fixture.event],
        ..fixture.launch.borrowed()
    };
    assert!(matches!(fixture.backend.submit_v1(ordinary),
            Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch));
    assert_eq!(fixture.backend.next_handle, next);
    for allocation in [fixture.allocations[2], fixture.allocations[1]] {
        fixture
            .backend
            .allocations
            .get_mut(&allocation)
            .unwrap()
            .sdma_storage = KfdRuntimeSdmaStorageV1::ComputeInFlight(fixture.producer + 100);
        assert!(matches!(fixture.submit(),
                Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch));
        fixture
            .backend
            .allocations
            .get_mut(&allocation)
            .unwrap()
            .sdma_storage = KfdRuntimeSdmaStorageV1::ComputeInFlight(fixture.producer);
        assert_eq!(fixture.backend.next_handle, next);
    }
    fixture.launch.bindings[2].region.allocation = fixture.allocations[2];
    assert!(matches!(fixture.submit(),
            Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch));
    fixture.launch.bindings[2].region.allocation = fixture.allocations[3];
    for binding in &mut fixture.launch.bindings {
        binding.region.byte_len -= 4;
    }
    assert!(matches!(fixture.submit(),
            Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch));
    for binding in &mut fixture.launch.bindings {
        binding.region.byte_len += 4;
    }
    assert_eq!(fixture.backend.next_handle, next);
    assert!(fixture.backend.pending_compute.is_empty());
    assert!(fixture.backend.compute_dependency_retain_counts.is_empty());
    assert_eq!(
        fixture.backend.compute_module_retain_counts[&fixture.module],
        1
    );
    assert_eq!(
        fixture.backend.active.as_ref().unwrap().id,
        fixture.producer
    );
    fixture.finish(None);
}

#[cfg(feature = "hardware-qualification")]
#[test]
fn producer_launch_active_three_binding_rechecks_final_authority() {
    let mut fixture = ScriptedActiveProducerFixtureV1::new(true);
    let admitted =
        crate::qualification_gfx942_r57_n3_v1::admit_gfx942_r57_n3_qualification_v1().unwrap();
    let observation = admitted.observation_v1();
    // The exact native gate must reject this synthetic fixture's artifact.
    fixture.backend.launch_gate = KfdRuntimeLaunchGateV1::ExactGfx942R57N3(admitted);
    let destination = fixture.allocations[3];
    let bytes = fixture.backend.allocations[&destination].bytes.clone();
    let consumer = fixture.submit().unwrap();
    assert_eq!(observation.authorization_calls_v1(), 0);
    fixture.backend.release_event_v1(fixture.event).unwrap();
    assert_eq!(
        fixture.backend.poll_v1(consumer).unwrap(),
        BackendPollV1::Pending
    );
    assert_eq!(observation.authorization_calls_v1(), 0);
    assert!(
        matches!(fixture.backend.flush_stream_v1(fixture.launch.stream),
            Err(RuntimeBackendFailureV1::Quiescent(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Native)
    );
    assert_eq!(observation.authorization_calls_v1(), 1);
    assert_eq!(
        fixture.backend.poll_v1(consumer).unwrap(),
        BackendPollV1::Failed { code: -1 }
    );
    assert!(fixture.backend.active.is_none());
    assert!(fixture.backend.pending_compute.is_empty());
    assert!(!fixture.backend.submissions[&consumer].profile_dispatch_published);
    assert_eq!(fixture.backend.allocations[&destination].bytes, bytes);
    assert!(matches!(
        fixture.backend.allocations[&destination].sdma_storage,
        KfdRuntimeSdmaStorageV1::H2dReady(_)
    ));
    assert_eq!(
        fixture
            .backend
            .last_launch_performance_v1()
            .unwrap()
            .user_data_materializations(),
        0
    );
    fixture.finish(Some(consumer));
}

#[test]
fn producer_launch_active_three_binding_parent_terminal_retains_both_rosters() {
    let mut fixture = ScriptedActiveProducerFixtureV1::new(true);
    let original = fixture.published_owners();
    let consumer = fixture.submit().unwrap();
    fixture.backend.release_event_v1(fixture.event).unwrap();
    fixture
        .backend
        .allocations
        .get_mut(&fixture.allocations[0])
        .unwrap()
        .sdma_storage = KfdRuntimeSdmaStorageV1::ComputeInFlight(fixture.producer + 100);
    assert!(matches!(
        fixture.backend.poll_v1(consumer),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(fixture.backend.terminal);
    assert!(fixture.backend.terminal_sdma_custody.is_none());
    assert_eq!(
        fixture.backend.active.as_ref().unwrap().id,
        fixture.producer
    );
    assert_eq!(fixture.published_owners(), original);
    assert_eq!(
        &*fixture.backend.pending_compute[&consumer].explicit_success_dependencies,
        &[fixture.producer]
    );
    assert_eq!(
        fixture.backend.compute_dependency_retain_counts[&fixture.producer],
        1
    );
    assert_eq!(
        fixture.backend.compute_module_retain_counts[&fixture.module],
        2
    );
    assert_eq!(fixture.backend.compute_completion_reservations, 2);
    assert_eq!(
        fixture.backend.pending_compute_streams[&fixture.launch.stream],
        [consumer]
    );
    for (submission, stream, allocations) in [
        (
            fixture.producer,
            fixture.producer_stream,
            &fixture.allocations[..3],
        ),
        (
            consumer,
            fixture.launch.stream,
            &[
                fixture.allocations[2],
                fixture.allocations[1],
                fixture.allocations[3],
            ][..],
        ),
    ] {
        for allocation in allocations {
            assert!(fixture.backend.allocation_retains_exact_owner_v1(
                *allocation,
                RuntimeAllocationCustodyOwnerV1 {
                    submission,
                    stream,
                    kind: RuntimeAllocationCustodyKindV1::Compute,
                }
            ));
        }
    }
    assert!(matches!(
        fixture.backend.cancel_v1(consumer),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    let driver = fixture.backend.scripted_sdma.as_ref().unwrap();
    assert_eq!(driver.live_owner_count(), 4);
    assert_eq!(driver.unexpected_drops(), 0);
    disarm_scripted_drop_after_inspection_v1(&mut fixture.backend);
}

#[test]
fn producer_launch_active_three_binding_failed_receipts_never_publish_child() {
    for cross_stream in [false, true] {
        for observe_only in [false, true] {
            for quiescent in [false, true] {
                let mut fixture = ScriptedActiveProducerFixtureV1::new(cross_stream);
                let consumer = fixture.submit().unwrap();
                assert_eq!(
                    fixture.backend.poll_v1(fixture.producer).unwrap(),
                    BackendPollV1::Succeeded
                );
                // Inject completion-record outcomes only after real scripted
                // owner restoration. This is not a native fault witness.
                fixture
                    .backend
                    .submissions
                    .get_mut(&fixture.producer)
                    .unwrap()
                    .status = BackendPollV1::Failed { code: -9 };
                if quiescent {
                    fixture
                        .backend
                        .quiescent_sdma_submissions
                        .insert(fixture.producer);
                }
                if observe_only {
                    assert_eq!(
                        fixture.backend.poll_v1(consumer).unwrap(),
                        BackendPollV1::Failed { code: -1 }
                    );
                } else {
                    assert!(
                        matches!(fixture.backend.flush_stream_v1(fixture.launch.stream),
                            Err(RuntimeBackendFailureV1::Quiescent(error))
                                if error.kind() == KfdRuntimeBackendErrorKindV1::Native)
                    );
                }
                assert_eq!(
                    fixture.backend.poll_v1(consumer).unwrap(),
                    BackendPollV1::Failed { code: -1 }
                );
                assert!(!fixture.backend.submissions[&consumer].profile_dispatch_published);
                assert!(fixture.backend.active.is_none());
                assert!(fixture.backend.pending_compute.is_empty());
                assert!(fixture.backend.allocation_custody.is_empty());
                assert!(fixture.backend.compute_dependency_retain_counts.is_empty());
                assert!(fixture.backend.compute_module_retain_counts.is_empty());
                assert_eq!(fixture.backend.compute_completion_reservations, 0);
                fixture
                    .backend
                    .quiescent_sdma_submissions
                    .remove(&fixture.producer);
                fixture
                    .backend
                    .submissions
                    .get_mut(&fixture.producer)
                    .unwrap()
                    .status = BackendPollV1::Succeeded;
                fixture.finish(Some(consumer));
            }
        }
    }
}

#[test]
fn scripted_three_binding_launches_twice_and_reuses_prior_output_as_read() {
    let byte_len = 64_usize;
    let (mut backend, stream, allocations) = scripted_three_binding_backend_v1(byte_len);
    let initial_sha256 = allocations.map(|allocation| {
        backend.allocations[&allocation]
            .content_sha256
            .expect("fixture is authenticated")
    });
    let module = backend
        .load_module_v1(7, &synthetic_cov6::three_binding_module())
        .unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();

    let first = submit_scripted_three_binding_v1(
        &mut backend,
        stream,
        kernel,
        allocations,
        byte_len as u64,
    );
    backend.flush_stream_v1(stream).unwrap();
    assert_eq!(
        backend
            .wait_v1(first, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    let first_performance = backend.last_launch_performance_v1().unwrap();
    assert_eq!(
        first_performance.data_path(),
        KfdRuntimeLaunchDataPathV1::PersistentDeviceReused
    );
    assert_eq!(first_performance.user_data_materializations(), 0);
    assert!(!first_performance.persistent_control_reused());
    assert!(backend.retained_persistent_dispatch.is_none());
    assert_eq!(
        backend.allocations[&allocations[0]].content_sha256,
        Some(initial_sha256[0])
    );
    assert_eq!(
        backend.allocations[&allocations[1]].content_sha256,
        Some(initial_sha256[1])
    );
    assert_eq!(backend.allocations[&allocations[2]].content_sha256, None);
    assert!(backend.allocations[&allocations[2]].sdma_shadow_dirty);
    assert!(backend.allocations[&allocations[2]].sdma_initialized);
    assert_runtime_compute_pipeline_empty_v1(&backend);

    let second_allocations = [allocations[2], allocations[1], allocations[0]];
    let second = submit_scripted_three_binding_v1(
        &mut backend,
        stream,
        kernel,
        second_allocations,
        byte_len as u64,
    );
    backend.flush_stream_v1(stream).unwrap();
    assert_eq!(backend.poll_v1(second).unwrap(), BackendPollV1::Succeeded);
    let second_performance = backend.last_launch_performance_v1().unwrap();
    assert_eq!(
        second_performance.data_path(),
        KfdRuntimeLaunchDataPathV1::PersistentDeviceReused
    );
    assert_eq!(second_performance.user_data_materializations(), 0);
    assert!(!second_performance.persistent_control_reused());
    assert!(backend.retained_persistent_dispatch.is_none());
    assert_eq!(
        backend.allocations[&allocations[1]].content_sha256,
        Some(initial_sha256[1])
    );
    assert_eq!(backend.allocations[&allocations[0]].content_sha256, None);
    assert!(backend.allocations[&allocations[0]].sdma_shadow_dirty);
    assert!(backend.allocations[&allocations[0]].sdma_initialized);
    assert_runtime_compute_pipeline_empty_v1(&backend);

    backend.release_submission_v1(first).unwrap();
    backend.release_submission_v1(second).unwrap();
    backend.unload_module_v1(module).unwrap();
    for allocation in allocations {
        backend
            .allocations
            .get_mut(&allocation)
            .unwrap()
            .sdma_backed = false;
        backend.release_allocation_v1(allocation).unwrap();
    }
    backend.destroy_stream_v1(stream).unwrap();
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 0);
    assert_eq!(driver.unexpected_drops(), 0);
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn typed_runtime_context_launch_selects_exact_three_binding_persistent_path() {
    let byte_len = 64_u64;
    let mut context = crate::RuntimeContextV1::open(KfdRuntimeBackendV1::mock()).unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let allocations = std::array::from_fn(|_| {
        context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, byte_len, 8)
            .unwrap()
    });
    {
        let backend = context.backend_mut_for_test_v1();
        let release_steps = [
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ];
        let driver = ScriptedSdmaDriverV1::new(release_steps);
        let [owner_a, owner_b, owner_c] =
            std::array::from_fn(|_| driver.test_device_owner(byte_len as usize));
        let mut backend_allocations: Vec<_> = backend.allocations.keys().copied().collect();
        backend_allocations.sort_unstable();
        assert_eq!(backend_allocations.len(), 3);
        for ((index, allocation), owner) in backend_allocations
            .into_iter()
            .enumerate()
            .zip([owner_a, owner_b, owner_c])
        {
            let record = backend.allocations.get_mut(&allocation).unwrap();
            let bytes: Arc<[u8]> = vec![0x61 + index as u8; byte_len as usize].into();
            let authenticated_sha256 = Sha256::digest(&bytes).into();
            let DirectionalSdmaDeviceOwnerV1::Scripted(device) = owner else {
                unreachable!("scripted factory returned native device custody")
            };
            record.bytes = bytes;
            record.content_sha256 = Some(authenticated_sha256);
            record.sdma_storage =
                KfdRuntimeSdmaStorageV1::H2dReady(Box::new(PersistentComputeReadyStorageV1 {
                    owner: PersistentComputeReadyOwnerV1::Scripted {
                        device,
                        authenticated_sha256,
                    },
                    promotion: None,
                }));
            record.sdma_backed = true;
            record.sdma_initialized = true;
            record.sdma_shadow_dirty = false;
        }
        backend.native_available = true;
        backend.sdma_enabled = true;
        backend.scripted_sdma = Some(driver);
    }

    let module = context
        .load_module(device, &synthetic_cov6::three_binding_module())
        .unwrap();
    let kernel = context
        .resolve_kernel::<ThreeBindingContextArgumentsV1>(module, "vecadd")
        .unwrap();
    let arguments = ThreeBindingContextArgumentsV1 {
        allocations,
        byte_len,
    };
    let mut submission = context
        .launch(
            stream,
            &kernel,
            &arguments,
            crate::RuntimeLaunchGeometryV1 {
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_shared_bytes: 0,
            },
            &[],
        )
        .unwrap();
    assert_eq!(
        context
            .wait(&mut submission, Duration::from_secs(1))
            .unwrap(),
        crate::RuntimePollV1::Succeeded
    );
    let performance = context.backend().last_launch_performance_v1().unwrap();
    assert_eq!(
        performance.data_path(),
        KfdRuntimeLaunchDataPathV1::PersistentDeviceReused
    );
    assert_eq!(performance.user_data_materializations(), 0);
    assert!(!performance.persistent_control_reused());

    context.release_submission(submission).unwrap();
    context.unload_module(module).unwrap();
    for allocation in allocations {
        for record in context.backend_mut_for_test_v1().allocations.values_mut() {
            record.sdma_backed = false;
        }
        context.release_allocation(allocation).unwrap();
    }
    context.destroy_stream(stream).unwrap();
    let mut backend = context.shutdown().unwrap();
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 0);
    assert_eq!(driver.unexpected_drops(), 0);
    backend.shutdown_native_v1().unwrap();
}
