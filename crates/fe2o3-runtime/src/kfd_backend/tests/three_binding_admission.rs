use super::*;

#[test]
fn three_binding_host_visible_validates_and_prepares_materialized_launch() {
    let (mut backend, launch) = host_visible_three_binding_launch_v1();
    assert!(
        backend
            .three_binding_persistent_admission_for_launch_v1(launch.borrowed())
            .is_none()
    );
    backend
        .validate_compute_launch_v1(&launch.borrowed(), &[], ComputeInputAdmissionV1::Ready)
        .unwrap();
    for reuse_bound_recipe in [false, true] {
        let prepared = backend
            .prepare_launch(launch.borrowed(), false, reuse_bound_recipe)
            .unwrap();
        let PreparedLaunchStorageV1::Materialized(data) = &prepared.storage else {
            panic!("host-visible R/R/W must select ordinary materialization")
        };
        assert_eq!(data.len(), 3);
        for (index, (data, binding)) in data.iter().zip(&launch.bindings).enumerate() {
            assert_eq!(data.allocation, binding.region.allocation);
            assert_eq!(data.kind, RuntimeMemoryKindV1::HostVisible);
            assert_eq!(data.bytes(), &[0x31 + index as u8; 64]);
        }
        assert_eq!(prepared.writebacks.len(), 1);
        assert_eq!(
            prepared.writebacks[0].allocation,
            launch.bindings[2].region.allocation
        );
        let program =
            build_program_v1(&prepared.program, prepared.signature, &prepared.abi_rows).unwrap();
        assert!(program.dispatch_abi_identity().is_some());
    }
    assert_runtime_compute_pipeline_empty_v1(&backend);
    assert!(backend.allocation_custody.is_empty());
    assert_eq!(backend.compute_completion_reservations, 0);
    for binding in launch.bindings.iter().rev() {
        backend
            .release_allocation_v1(binding.region.allocation)
            .unwrap();
    }
    backend
        .unload_module_v1(backend.kernels[&launch.kernel].module)
        .unwrap();
    backend.destroy_stream_v1(launch.stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn three_binding_host_visible_exemption_rejects_mixed_device_local_and_unknown_rosters() {
    let (mut backend, mut launch) = host_visible_three_binding_launch_v1();
    for device_mask in 1..8 {
        for (index, binding) in launch.bindings.iter().enumerate() {
            backend
                .allocations
                .get_mut(&binding.region.allocation)
                .unwrap()
                .kind = if device_mask & (1 << index) != 0 {
                RuntimeMemoryKindV1::DeviceLocal
            } else {
                RuntimeMemoryKindV1::HostVisible
            };
        }
        assert!(matches!(
            backend.validate_compute_launch_v1(&launch.borrowed(), &[], ComputeInputAdmissionV1::Ready),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
                    && error.detail().contains("exact R/R/W admission")
        ));
        for reuse_bound_recipe in [false, true] {
            assert!(matches!(
                backend.prepare_launch(launch.borrowed(), false, reuse_bound_recipe),
                Err(RuntimeBackendFailureV1::Rejected(error))
                    if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
                        && error.detail().contains("exact R/R/W admission")
            ));
        }
    }
    for binding in &launch.bindings {
        backend
            .allocations
            .get_mut(&binding.region.allocation)
            .unwrap()
            .kind = RuntimeMemoryKindV1::HostVisible;
    }
    for index in 0..3 {
        let allocation = launch.bindings[index].region.allocation;
        launch.bindings[index].region.allocation = u64::MAX;
        assert!(matches!(
            backend.validate_compute_launch_v1(
                &launch.borrowed(),
                &[],
                ComputeInputAdmissionV1::Ready
            ),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert!(matches!(
            backend.prepare_launch(launch.borrowed(), false, false),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
                    && error.detail().contains("exact R/R/W admission")
        ));
        launch.bindings[index].region.allocation = allocation;
    }
    assert_runtime_compute_pipeline_empty_v1(&backend);
    assert!(backend.allocation_custody.is_empty());
    assert_eq!(backend.compute_completion_reservations, 0);
    for binding in launch.bindings.iter().rev() {
        backend
            .release_allocation_v1(binding.region.allocation)
            .unwrap();
    }
    backend
        .unload_module_v1(backend.kernels[&launch.kernel].module)
        .unwrap();
    backend.destroy_stream_v1(launch.stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn three_binding_admission_requires_exact_authenticated_initialized_roster() {
    let byte_len = 64_usize;
    let (mut backend, stream, allocations) = scripted_three_binding_backend_v1(byte_len);
    let bindings = |allocations: [u64; 3],
                    lengths: [u64; 3],
                    accesses: [RuntimeAccessV1; 3]|
     -> [BackendBindingV1; 3] {
        std::array::from_fn(|index| BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: allocations[index],
                access: accesses[index],
                byte_offset: 0,
                byte_len: lengths[index],
            },
            kernarg_byte_offset: (index * 8) as u32,
        })
    };
    let exact = bindings(
        allocations,
        [byte_len as u64; 3],
        [
            RuntimeAccessV1::Read,
            RuntimeAccessV1::Read,
            RuntimeAccessV1::Write,
        ],
    );
    let admitted = three_binding_persistent_compute_admission_v1(
        KfdRuntimeSemanticLaunchV1::Ordinary,
        &exact,
        7,
        &backend.allocations,
    )
    .expect("three authenticated H2D full extents must be reachable");
    assert_eq!(
        admitted.bindings.map(|admission| admission.source),
        [PersistentFullRangeComputeSourceV1::AuthenticatedH2d; 3]
    );

    let unequal = bindings(
        allocations,
        [byte_len as u64, byte_len as u64, byte_len as u64 - 1],
        [
            RuntimeAccessV1::Read,
            RuntimeAccessV1::Read,
            RuntimeAccessV1::Write,
        ],
    );
    assert!(
        three_binding_persistent_compute_admission_v1(
            KfdRuntimeSemanticLaunchV1::Ordinary,
            &unequal,
            7,
            &backend.allocations,
        )
        .is_none()
    );
    let wrong_roles = bindings(
        allocations,
        [byte_len as u64; 3],
        [
            RuntimeAccessV1::Read,
            RuntimeAccessV1::Write,
            RuntimeAccessV1::Write,
        ],
    );
    assert!(
        three_binding_persistent_compute_admission_v1(
            KfdRuntimeSemanticLaunchV1::Ordinary,
            &wrong_roles,
            7,
            &backend.allocations,
        )
        .is_none()
    );
    let aliased = bindings(
        [allocations[0], allocations[0], allocations[2]],
        [byte_len as u64; 3],
        [
            RuntimeAccessV1::Read,
            RuntimeAccessV1::Read,
            RuntimeAccessV1::Write,
        ],
    );
    assert!(
        three_binding_persistent_compute_admission_v1(
            KfdRuntimeSemanticLaunchV1::Ordinary,
            &aliased,
            7,
            &backend.allocations,
        )
        .is_none()
    );

    for input in allocations.into_iter().take(2) {
        backend
            .allocations
            .get_mut(&input)
            .unwrap()
            .sdma_initialized = false;
        assert!(
            three_binding_persistent_compute_admission_v1(
                KfdRuntimeSemanticLaunchV1::Ordinary,
                &exact,
                7,
                &backend.allocations,
            )
            .is_none(),
            "each read input requires established full initialization"
        );
        backend
            .allocations
            .get_mut(&input)
            .unwrap()
            .sdma_initialized = true;
    }

    let c = backend.allocations.get_mut(&allocations[2]).unwrap();
    let storage = core::mem::replace(&mut c.sdma_storage, KfdRuntimeSdmaStorageV1::Synthetic);
    let KfdRuntimeSdmaStorageV1::H2dReady(ready) = storage else {
        unreachable!("fixture output starts H2D-authenticated")
    };
    c.sdma_storage = KfdRuntimeSdmaStorageV1::Device(Box::new(ready.owner.normalize()));
    c.sdma_initialized = false;
    assert!(
        three_binding_persistent_compute_admission_v1(
            KfdRuntimeSemanticLaunchV1::Ordinary,
            &exact,
            7,
            &backend.allocations,
        )
        .is_none(),
        "write metadata cannot certify full initialization"
    );
    assert!(matches!(
        backend.allocations[&allocations[2]].sdma_storage,
        KfdRuntimeSdmaStorageV1::Device(_)
    ));
    assert!(!backend.allocations[&allocations[2]].sdma_initialized);

    for allocation in allocations {
        backend
            .allocations
            .get_mut(&allocation)
            .unwrap()
            .sdma_backed = false;
        backend.release_allocation_v1(allocation).unwrap();
    }
    backend.destroy_stream_v1(stream).unwrap();
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
        0
    );
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn public_three_read_binding_launch_preserves_the_generic_materialized_path() {
    let byte_len = 64_u64;
    let (mut context, stream, allocations, backend_allocations) =
        scripted_three_binding_context_v1(byte_len);
    let device = context.devices()[0].id();
    let module = context
        .load_module(device, &synthetic_cov6::three_binding_module())
        .unwrap();
    let kernel = context
        .resolve_kernel::<ThreeBindingCandidateContextArgumentsV1>(module, "vecadd")
        .unwrap();
    let geometry = crate::RuntimeLaunchGeometryV1 {
        grid: [64, 1, 1],
        workgroup: [64, 1, 1],
        dynamic_shared_bytes: 0,
    };
    let before = three_binding_prelaunch_snapshot_v1(context.backend(), backend_allocations);
    let mut expected_after = before.clone();
    for snapshot in expected_after.iter_mut().take(2) {
        let ThreeBindingPrelaunchStorageV1::H2dReady { owner_id, .. } = snapshot.storage else {
            unreachable!("fixture begins with authenticated ready custody")
        };
        snapshot.storage = ThreeBindingPrelaunchStorageV1::Device { owner_id };
    }
    let arguments = ThreeBindingCandidateContextArgumentsV1 {
        allocations: [allocations[0], allocations[0], allocations[1]],
        byte_offsets: [0, byte_len / 2, byte_len / 4],
        byte_lens: [byte_len / 2, byte_len / 4, byte_len / 2],
        accesses: [RuntimeAccessV1::Read; 3],
    };

    let mut submission = context
        .launch(stream, &kernel, &arguments, geometry, &[])
        .unwrap();
    assert_eq!(
        context
            .wait(&mut submission, Duration::from_secs(1))
            .unwrap(),
        crate::RuntimePollV1::Succeeded
    );
    assert_eq!(
        three_binding_prelaunch_snapshot_v1(context.backend(), backend_allocations),
        expected_after
    );
    let performance = context.backend().last_launch_performance_v1().unwrap();
    assert_eq!(
        performance.data_path(),
        KfdRuntimeLaunchDataPathV1::Materialized
    );
    assert_eq!(performance.user_data_materializations(), 2);
    assert_eq!(
        context
            .backend()
            .scripted_sdma
            .as_ref()
            .unwrap()
            .live_owner_count(),
        3
    );
    assert!(context.backend().pending_compute.is_empty());
    assert!(context.backend().active.is_none());
    assert_eq!(context.backend().compute_completion_reservations, 0);

    context.release_submission(submission).unwrap();
    assert!(context.backend().allocation_custody.is_empty());
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

#[test]
fn public_three_binding_candidate_rejection_preserves_custody_and_retries() {
    let byte_len = 64_u64;
    let (mut context, stream, allocations, backend_allocations) =
        scripted_three_binding_context_v1(byte_len);
    let device = context.devices()[0].id();

    let module = context
        .load_module(device, &synthetic_cov6::three_binding_module())
        .unwrap();
    let kernel = context
        .resolve_kernel::<ThreeBindingCandidateContextArgumentsV1>(module, "vecadd")
        .unwrap();
    let geometry = crate::RuntimeLaunchGeometryV1 {
        grid: [64, 1, 1],
        workgroup: [64, 1, 1],
        dynamic_shared_bytes: 0,
    };
    let exact_accesses = [
        RuntimeAccessV1::Read,
        RuntimeAccessV1::Read,
        RuntimeAccessV1::Write,
    ];
    let exact_lengths = [byte_len; 3];
    for (index, allocation) in backend_allocations.into_iter().enumerate() {
        let bytes: Arc<[u8]> = vec![0x91 + index as u8; byte_len as usize].into();
        let sha256 = Sha256::digest(&bytes).into();
        context
            .backend_mut_for_test_v1()
            .allocations
            .get_mut(&allocation)
            .unwrap()
            .last_full_host_write = Some((bytes, sha256));
    }
    let all_ready_snapshot =
        three_binding_prelaunch_snapshot_v1(context.backend(), backend_allocations);
    assert_runtime_compute_pipeline_empty_v1(context.backend());

    let c_witness = {
        let c = context
            .backend_mut_for_test_v1()
            .allocations
            .get_mut(&backend_allocations[2])
            .unwrap();
        // Device-local bytes alone are not an authenticated persistent input witness.
        let witness = remove_three_binding_ready_witness_v1(c);
        c.sdma_initialized = true;
        witness
    };
    let unwitnessed_c_snapshot =
        three_binding_prelaunch_snapshot_v1(context.backend(), backend_allocations);
    let exact_arguments = ThreeBindingCandidateContextArgumentsV1 {
        allocations,
        byte_offsets: [0; 3],
        byte_lens: exact_lengths,
        accesses: exact_accesses,
    };
    for _ in 0..2 {
        // Raw storage can be admitted for deferred native conversion, but
        // bytes alone cannot grant publication when conversion is refused.
        let mut submission = context
            .launch(stream, &kernel, &exact_arguments, geometry, &[])
            .unwrap();
        assert_eq!(
            context
                .wait(&mut submission, Duration::from_secs(1))
                .unwrap(),
            crate::RuntimePollV1::Failed { code: -1 }
        );
        assert!(context.backend().last_launch_performance_v1().is_none());
        context.release_submission(submission).unwrap();
        assert_eq!(
            three_binding_prelaunch_snapshot_v1(context.backend(), backend_allocations,),
            unwitnessed_c_snapshot
        );
        assert_eq!(
            context
                .backend()
                .scripted_sdma
                .as_ref()
                .unwrap()
                .live_owner_count(),
            3
        );
        assert!(context.backend().allocation_custody.is_empty());
        assert!(context.backend().pending_compute.is_empty());
        assert!(context.backend().active.is_none());
        assert_eq!(context.backend().compute_completion_reservations, 0);
        assert_runtime_compute_pipeline_empty_v1(context.backend());
    }

    {
        let c = context
            .backend_mut_for_test_v1()
            .allocations
            .get_mut(&backend_allocations[2])
            .unwrap();
        restore_three_binding_ready_witness_v1(c, c_witness);
    }
    assert_eq!(
        three_binding_prelaunch_snapshot_v1(context.backend(), backend_allocations),
        all_ready_snapshot
    );

    for index in 0..2 {
        let witness = {
            let input = context
                .backend_mut_for_test_v1()
                .allocations
                .get_mut(&backend_allocations[index])
                .unwrap();
            remove_three_binding_ready_witness_v1(input)
        };
        let unwitnessed_input_snapshot =
            three_binding_prelaunch_snapshot_v1(context.backend(), backend_allocations);
        let mut submission = context
            .launch(stream, &kernel, &exact_arguments, geometry, &[])
            .unwrap();
        assert_eq!(
            context
                .wait(&mut submission, Duration::from_secs(1))
                .unwrap(),
            crate::RuntimePollV1::Failed { code: -1 }
        );
        assert!(context.backend().last_launch_performance_v1().is_none());
        context.release_submission(submission).unwrap();
        assert_eq!(
            three_binding_prelaunch_snapshot_v1(context.backend(), backend_allocations),
            unwitnessed_input_snapshot
        );
        assert_eq!(
            context
                .backend()
                .scripted_sdma
                .as_ref()
                .unwrap()
                .live_owner_count(),
            3
        );
        assert!(context.backend().allocation_custody.is_empty());
        assert!(context.backend().pending_compute.is_empty());
        assert!(context.backend().active.is_none());
        assert_eq!(context.backend().compute_completion_reservations, 0);
        assert_runtime_compute_pipeline_empty_v1(context.backend());
        restore_three_binding_ready_witness_v1(
            context
                .backend_mut_for_test_v1()
                .allocations
                .get_mut(&backend_allocations[index])
                .unwrap(),
            witness,
        );
        assert_eq!(
            three_binding_prelaunch_snapshot_v1(context.backend(), backend_allocations),
            all_ready_snapshot
        );
    }

    let invalid_arguments = [
        ThreeBindingCandidateContextArgumentsV1 {
            allocations,
            byte_offsets: [0; 3],
            byte_lens: [byte_len, byte_len, byte_len - 1],
            accesses: exact_accesses,
        },
        ThreeBindingCandidateContextArgumentsV1 {
            allocations: [allocations[0], allocations[0], allocations[2]],
            byte_offsets: [0; 3],
            byte_lens: exact_lengths,
            accesses: exact_accesses,
        },
        ThreeBindingCandidateContextArgumentsV1 {
            allocations: [allocations[0], allocations[1], allocations[0]],
            byte_offsets: [0; 3],
            byte_lens: exact_lengths,
            accesses: exact_accesses,
        },
    ];
    for arguments in &invalid_arguments {
        assert!(matches!(
            context.launch(stream, &kernel, arguments, geometry, &[]),
            Err(crate::RuntimeErrorV1::BackendRejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
                    && error.detail().contains("exact R/R/W admission")
        ));
        assert_eq!(
            three_binding_prelaunch_snapshot_v1(context.backend(), backend_allocations,),
            all_ready_snapshot
        );
        assert_eq!(
            context
                .backend()
                .scripted_sdma
                .as_ref()
                .unwrap()
                .live_owner_count(),
            3
        );
        assert_runtime_compute_pipeline_empty_v1(context.backend());
    }
    for index in 0..2 {
        context
            .backend_mut_for_test_v1()
            .allocations
            .get_mut(&backend_allocations[index])
            .unwrap()
            .sdma_initialized = false;
        let uninitialized_snapshot =
            three_binding_prelaunch_snapshot_v1(context.backend(), backend_allocations);
        assert!(matches!(
            context.launch(stream, &kernel, &exact_arguments, geometry, &[]),
            Err(crate::RuntimeErrorV1::BackendRejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
        ));
        assert_eq!(
            three_binding_prelaunch_snapshot_v1(context.backend(), backend_allocations,),
            uninitialized_snapshot
        );
        assert_runtime_compute_pipeline_empty_v1(context.backend());
        context
            .backend_mut_for_test_v1()
            .allocations
            .get_mut(&backend_allocations[index])
            .unwrap()
            .sdma_initialized = true;
    }
    assert_eq!(
        three_binding_prelaunch_snapshot_v1(context.backend(), backend_allocations),
        all_ready_snapshot
    );

    let mut submission = context
        .launch(stream, &kernel, &exact_arguments, geometry, &[])
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
    assert_runtime_compute_pipeline_empty_v1(context.backend());

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

#[cfg(feature = "hardware-qualification")]
#[test]
fn r57_n3_qualification_refuses_unwitnessed_c_before_authority() {
    let byte_len = 64_u64;
    let (mut context, stream, allocations, backend_allocations) =
        scripted_three_binding_context_v1(byte_len);
    let admitted =
        crate::qualification_gfx942_r57_n3_v1::admit_gfx942_r57_n3_qualification_v1().unwrap();
    let observation = admitted.observation_v1();
    context.backend_mut_for_test_v1().launch_gate =
        KfdRuntimeLaunchGateV1::ExactGfx942R57N3(admitted);
    let device = context.devices()[0].id();
    let module = context
        .load_module(device, &synthetic_cov6::three_binding_module())
        .unwrap();
    let kernel = context
        .resolve_kernel::<ThreeBindingCandidateContextArgumentsV1>(module, "vecadd")
        .unwrap();
    let c_witness = remove_three_binding_ready_witness_v1(
        context
            .backend_mut_for_test_v1()
            .allocations
            .get_mut(&backend_allocations[2])
            .unwrap(),
    );
    let before = three_binding_prelaunch_snapshot_v1(context.backend(), backend_allocations);
    let arguments = ThreeBindingCandidateContextArgumentsV1 {
        allocations,
        byte_offsets: [0; 3],
        byte_lens: [byte_len; 3],
        accesses: [
            RuntimeAccessV1::Read,
            RuntimeAccessV1::Read,
            RuntimeAccessV1::Write,
        ],
    };
    let geometry = crate::RuntimeLaunchGeometryV1 {
        grid: [64, 1, 1],
        workgroup: [64, 1, 1],
        dynamic_shared_bytes: 0,
    };
    let mut submission = context
        .launch(stream, &kernel, &arguments, geometry, &[])
        .unwrap();
    assert_eq!(
        context
            .wait(&mut submission, Duration::from_secs(1))
            .unwrap(),
        crate::RuntimePollV1::Failed { code: -1 }
    );
    assert_eq!(observation.authorization_calls_v1(), 0);
    assert!(context.backend().last_launch_performance_v1().is_none());
    context.release_submission(submission).unwrap();
    assert_eq!(
        three_binding_prelaunch_snapshot_v1(context.backend(), backend_allocations),
        before
    );

    restore_three_binding_ready_witness_v1(
        context
            .backend_mut_for_test_v1()
            .allocations
            .get_mut(&backend_allocations[2])
            .unwrap(),
        c_witness,
    );
    context.unload_module(module).unwrap();
    for record in context.backend_mut_for_test_v1().allocations.values_mut() {
        record.sdma_backed = false;
    }
    for allocation in allocations {
        context.release_allocation(allocation).unwrap();
    }
    context.destroy_stream(stream).unwrap();
    let mut backend = context.shutdown().unwrap();
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
        0
    );
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn three_binding_restore_preflight_prevents_partial_owner_restoration() {
    let byte_len = 64_usize;
    let (mut backend, _stream, allocations) = scripted_three_binding_backend_v1(byte_len);
    let bindings: [BackendBindingV1; 3] = std::array::from_fn(|index| BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation: allocations[index],
            access: [
                RuntimeAccessV1::Read,
                RuntimeAccessV1::Read,
                RuntimeAccessV1::Write,
            ][index],
            byte_offset: 0,
            byte_len: byte_len as u64,
        },
        kernarg_byte_offset: (index * 8) as u32,
    });
    let admission = three_binding_persistent_compute_admission_v1(
        KfdRuntimeSemanticLaunchV1::Ordinary,
        &bindings,
        7,
        &backend.allocations,
    )
    .unwrap();
    let mut shells = backend
        .prepare_three_binding_restore_shells_v1(admission.bindings)
        .unwrap();
    let (inputs, promotions) = backend
        .take_three_binding_persistent_inputs_v1(admission.bindings, 701)
        .unwrap();
    shells[1].ready = None;

    assert!(matches!(
        backend.restore_three_binding_persistent_inputs_v1(
            admission.bindings,
            701,
            inputs,
            promotions,
            shells,
        ),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(backend.terminal);
    assert!(allocations.iter().all(|allocation| matches!(
        backend.allocations[allocation].sdma_storage,
        KfdRuntimeSdmaStorageV1::ComputeInFlight(701)
    )));
    assert!(matches!(
        backend.terminal_sdma_custody,
        Some(KfdRuntimeTerminalSdmaCustodyV1::ThreeBindingPersistentInputs(_))
    ));
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert_eq!(driver.live_owner_count(), 3);
    assert_eq!(driver.unexpected_drops(), 0);
    disarm_scripted_drop_after_inspection_v1(&mut backend);
}

#[test]
fn three_binding_prepublication_restore_returns_all_authenticated_owners() {
    let byte_len = 64_usize;
    let (mut backend, stream, allocations) = scripted_three_binding_backend_v1(byte_len);
    let bindings: [BackendBindingV1; 3] = std::array::from_fn(|index| BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation: allocations[index],
            access: [
                RuntimeAccessV1::Read,
                RuntimeAccessV1::Read,
                RuntimeAccessV1::Write,
            ][index],
            byte_offset: 0,
            byte_len: byte_len as u64,
        },
        kernarg_byte_offset: (index * 8) as u32,
    });
    let admission = three_binding_persistent_compute_admission_v1(
        KfdRuntimeSemanticLaunchV1::Ordinary,
        &bindings,
        7,
        &backend.allocations,
    )
    .unwrap();
    let shells = backend
        .prepare_three_binding_restore_shells_v1(admission.bindings)
        .unwrap();
    let (inputs, promotions) = backend
        .take_three_binding_persistent_inputs_v1(admission.bindings, 702)
        .unwrap();
    backend
        .restore_three_binding_persistent_inputs_v1(
            admission.bindings,
            702,
            inputs,
            promotions,
            shells,
        )
        .unwrap();
    assert!(allocations.iter().all(|allocation| matches!(
        backend.allocations[allocation].sdma_storage,
        KfdRuntimeSdmaStorageV1::H2dReady(_)
    )));
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
        3
    );

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
