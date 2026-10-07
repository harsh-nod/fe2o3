//! Scripted scheduling/custody coverage; native initialization is qualified in KFD.

use super::sdma_host_write_tests::discard_scripted_fixture;
use super::*;
use fe2o3_kfd::Gfx942PersistentComputeStorageIneligibilityV1 as Ineligible;
use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn owner(backend: &KfdRuntimeBackendV1, allocation: u64) -> u64 {
    match &backend.allocations[&allocation].sdma_storage {
        KfdRuntimeSdmaStorageV1::Device(device) => device.scripted_owner_id().unwrap(),
        KfdRuntimeSdmaStorageV1::InitializedStorage(ready) => match ready.as_ref() {
            InitializedStorageOwnerV1::Scripted(device) => device.scripted_owner_id().unwrap(),
            _ => unreachable!(),
        },
        _ => panic!("expected indexed device custody"),
    }
}

fn load(backend: &mut KfdRuntimeBackendV1, three: bool) -> (u64, u64) {
    let image = if three {
        synthetic_cov6::three_binding_module()
    } else {
        synthetic_cov6::module()
    };
    let module = backend.load_module_v1(7, &image).unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();
    (module, kernel)
}

fn complete(backend: &mut KfdRuntimeBackendV1, submission: u64) -> BackendPollV1 {
    for _ in 0..8 {
        let status = backend.poll_v1(submission).unwrap();
        if status != BackendPollV1::Pending {
            return status;
        }
    }
    panic!("scripted compute did not settle");
}

fn submit_gated(
    backend: &mut KfdRuntimeBackendV1,
    stream: u64,
    kernel: u64,
    allocations: &[u64],
) -> u64 {
    assert!(matches!(allocations.len(), 1 | 3));
    let mut kernarg = vec![0; if allocations.len() == 1 { 16 } else { 32 }];
    kernarg[allocations.len() * 8..].copy_from_slice(&1024_u64.to_le_bytes());
    let bindings: Vec<_> = allocations
        .iter()
        .enumerate()
        .map(|(index, allocation)| BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: *allocation,
                access: if index == 2 {
                    RuntimeAccessV1::Write
                } else {
                    RuntimeAccessV1::Read
                },
                byte_offset: 0,
                byte_len: 4096,
            },
            kernarg_byte_offset: (index * 8) as u32,
        })
        .collect();
    let launch = BackendLaunchV1 {
        stream,
        kernel,
        explicit_kernarg: &kernarg,
        bindings: &bindings,
        dependencies: &[],
        geometry: crate::RuntimeLaunchGeometryV1 {
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
            dynamic_shared_bytes: 0,
        },
        semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
    };
    let mut collected = backend
        .preflight_compute_v1(launch, ComputeDependencyRosterV1::Events(&[]))
        .unwrap();
    let id = backend.next_handle;
    // Install the child gate before immediate submit-time progression can run.
    collected.peer_gate = Some(PeerComputeGateV1::waiting(900, id, false));
    assert_eq!(
        backend
            .submit_collected_compute_v1(launch, collected)
            .unwrap(),
        id
    );
    id
}

fn resolve_gate(backend: &mut KfdRuntimeBackendV1, submission: u64, result: PeerComputeResultV1) {
    let pending = backend.pending_compute.get_mut(&submission).unwrap();
    pending.peer_gate = Some(
        pending
            .peer_gate
            .unwrap()
            .resolve(900, submission, result, true)
            .unwrap(),
    );
}

#[test]
fn initialized_storage_runtime_single_progress_uses_native_bytes_without_materialization() {
    for dirty in [false, true] {
        let mut steps = vec![ScriptedSdmaStepV1::PromoteInitializedStorage(
            ScriptedFailureModeV1::Success,
        )];
        steps.extend(scripted_release_steps_v1());
        let (backend, stream, host, device) = scripted_direct_backend_v1(4096, steps);
        let mut backend = ManuallyDrop::new(backend);
        let original = owner(&backend, device);
        let record = backend.allocations.get_mut(&device).unwrap();
        record.sdma_shadow_dirty = dirty;
        if dirty {
            let KfdRuntimeSdmaStorageV1::Device(native) = &mut record.sdma_storage else {
                unreachable!()
            };
            native.scripted_bytes_mut().unwrap().fill(0x79);
            record.bytes = vec![0x31; 4096].into();
        }
        let shadow_digest = Sha256::digest(&record.bytes).into();
        record.content_sha256 = Some(shadow_digest);
        record.last_full_host_write = Some((Arc::clone(&record.bytes), shadow_digest));
        let (module, kernel) = load(&mut backend, false);
        let submission = submit_scripted_read_v1(&mut backend, stream, kernel, device, 4096, &[]);
        backend.flush_stream_v1(stream).unwrap();
        assert!(matches!(backend.allocations[&device].sdma_storage,
            KfdRuntimeSdmaStorageV1::ComputeInFlight(id) if id == submission));
        assert!(
            backend
                .active
                .as_ref()
                .unwrap()
                .resident_descriptors
                .iter()
                .all(|descriptor| descriptor.host_content_sha256.is_none())
        );
        let Some(ActiveComputeExecutionV1::ScriptedPersistent { device: native, .. }) = backend
            .active
            .as_ref()
            .and_then(|active| active.execution.as_ref())
        else {
            panic!("converted storage must publish persistently")
        };
        assert_eq!(native.scripted_owner_id(), Some(original));
        assert!(
            native
                .scripted_bytes()
                .unwrap()
                .iter()
                .all(|byte| *byte == if dirty { 0x79 } else { 0 })
        );
        assert!(
            backend.allocations[&device]
                .persistent_storage_restore
                .is_some()
        );
        assert_eq!(complete(&mut backend, submission), BackendPollV1::Succeeded);
        assert_eq!(owner(&backend, device), original);
        assert!(
            backend.allocations[&device]
                .persistent_storage_restore
                .is_none()
        );
        assert!(backend.allocations[&device].content_sha256.is_none());
        assert!(backend.allocations[&device].last_full_host_write.is_none());
        assert_eq!(backend.allocations[&device].sdma_shadow_dirty, dirty);
        assert_eq!(
            backend
                .last_launch_performance_v1()
                .unwrap()
                .user_data_materializations(),
            0
        );
        backend.release_submission_v1(submission).unwrap();
        backend.unload_module_v1(module).unwrap();
        clean_scripted_direct_backend_v1(&mut backend, stream, host, device, None);
        drop(ManuallyDrop::into_inner(backend));
    }
}

#[test]
fn initialized_storage_runtime_cancellation_preserves_storage_origin_and_restore_shells() {
    let mut steps = vec![ScriptedSdmaStepV1::PromoteInitializedStorage(
        ScriptedFailureModeV1::Success,
    )];
    steps.extend(scripted_release_steps_v1());
    let (backend, stream, host, device) = scripted_direct_backend_v1(4096, steps);
    let mut backend = ManuallyDrop::new(backend);
    backend.scripted_persistent_publication_retries = 10;
    let original = owner(&backend, device);
    let (module, kernel) = load(&mut backend, false);
    let submission = submit_scripted_read_v1(&mut backend, stream, kernel, device, 4096, &[]);
    backend.flush_stream_v1(stream).unwrap();
    assert!(matches!(
        backend
            .active
            .as_ref()
            .and_then(|active| active.execution.as_ref()),
        Some(ActiveComputeExecutionV1::ScriptedPersistentPrepared { .. })
    ));
    assert_eq!(
        backend.cancel_v1(submission).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert!(matches!(
        backend.allocations[&device].sdma_storage,
        KfdRuntimeSdmaStorageV1::InitializedStorage(_)
    ));
    assert_eq!(owner(&backend, device), original);
    assert!(
        backend.allocations[&device]
            .persistent_storage_restore
            .is_none()
    );
    backend.release_submission_v1(submission).unwrap();
    backend.unload_module_v1(module).unwrap();
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, None);
    drop(ManuallyDrop::into_inner(backend));
}

#[test]
fn initialized_storage_runtime_only_typed_ineligibility_allows_synchronized_read_fallback() {
    for eligible_failure in [false, true] {
        let mut steps = vec![if eligible_failure {
            ScriptedSdmaStepV1::InitializedStorageNotEligible(Ineligible::IncompleteInitialization)
        } else {
            ScriptedSdmaStepV1::PromoteInitializedStorage(ScriptedFailureModeV1::Retryable)
        }];
        if eligible_failure {
            steps.extend(scripted_sync_copy_steps_v1(
                Gfx942PersistentSdmaDirectionV1::DeviceToHost,
                0,
                4096,
                ScriptedFailureModeV1::Success,
            ));
        }
        steps.extend(scripted_release_steps_v1());
        let (backend, stream, host, device) = scripted_direct_backend_v1(4096, steps);
        let mut backend = ManuallyDrop::new(backend);
        let original = owner(&backend, device);
        let record = backend.allocations.get_mut(&device).unwrap();
        record.sdma_shadow_dirty = true;
        let KfdRuntimeSdmaStorageV1::Device(native) = &mut record.sdma_storage else {
            unreachable!()
        };
        native.scripted_bytes_mut().unwrap().fill(0x63);
        let (module, kernel) = load(&mut backend, false);
        let submission = submit_gated(&mut backend, stream, kernel, &[device]);
        resolve_gate(&mut backend, submission, PeerComputeResultV1::Succeeded);
        let publication = backend.flush_stream_v1(stream);
        if eligible_failure {
            publication.unwrap();
        } else {
            assert!(matches!(
                publication,
                Err(RuntimeBackendFailureV1::Quiescent(_))
            ));
        }
        assert_eq!(
            complete(&mut backend, submission),
            if eligible_failure {
                BackendPollV1::Succeeded
            } else {
                BackendPollV1::Failed { code: -1 }
            }
        );
        assert_eq!(owner(&backend, device), original);
        if eligible_failure {
            assert!(
                backend.allocations[&device]
                    .bytes
                    .iter()
                    .all(|byte| *byte == 0x63)
            );
            assert!(!backend.allocations[&device].sdma_shadow_dirty);
            assert_eq!(
                backend
                    .last_launch_performance_v1()
                    .unwrap()
                    .user_data_materializations(),
                1
            );
        } else {
            assert!(
                backend.allocations[&device]
                    .bytes
                    .iter()
                    .all(|byte| *byte == 0)
            );
            assert!(backend.allocations[&device].sdma_shadow_dirty);
            assert!(backend.active.is_none());
        }
        backend.release_submission_v1(submission).unwrap();
        backend.unload_module_v1(module).unwrap();
        clean_scripted_direct_backend_v1(&mut backend, stream, host, device, None);
        drop(ManuallyDrop::into_inner(backend));
    }
}

#[test]
fn initialized_storage_runtime_clean_refusal_restores_original_box_and_metadata() {
    for reason in [
        Ineligible::PartialExtent,
        Ineligible::IncompleteInitialization,
    ] {
        let mut steps = vec![ScriptedSdmaStepV1::InitializedStorageNotEligible(reason)];
        steps.extend(scripted_release_steps_v1());
        let (backend, stream, host, device) = scripted_direct_backend_v1(4096, steps);
        let mut backend = ManuallyDrop::new(backend);
        let record = backend.allocations.get_mut(&device).unwrap();
        let digest = Sha256::digest(&record.bytes).into();
        record.content_sha256 = Some(digest);
        record.last_full_host_write = Some((Arc::clone(&record.bytes), digest));
        record.sdma_shadow_dirty = true;
        let metadata = (
            Arc::clone(&record.bytes),
            record.content_sha256,
            record.last_full_host_write.clone(),
            record.sdma_shadow_dirty,
        );
        let original = owner(&backend, device);
        let KfdRuntimeSdmaStorageV1::Device(native) = &backend.allocations[&device].sdma_storage
        else {
            unreachable!()
        };
        let pointer = &**native as *const _;
        assert_eq!(
            backend.convert_initialized_storage_v1(device).unwrap(),
            Some(reason)
        );
        let KfdRuntimeSdmaStorageV1::Device(native) = &backend.allocations[&device].sdma_storage
        else {
            panic!("refusal changed owner type")
        };
        assert_eq!(&**native as *const _, pointer);
        assert_eq!(owner(&backend, device), original);
        let record = &backend.allocations[&device];
        assert_eq!(
            (
                Arc::clone(&record.bytes),
                record.content_sha256,
                record.last_full_host_write.clone(),
                record.sdma_shadow_dirty
            ),
            metadata
        );
        assert!(!backend.terminal);
        clean_scripted_direct_backend_v1(&mut backend, stream, host, device, None);
        drop(ManuallyDrop::into_inner(backend));
    }
}

#[test]
fn initialized_storage_runtime_terminal_and_unwind_keep_pending_consumer_and_owner() {
    for panic in [false, true] {
        let step = if panic {
            ScriptedSdmaStepV1::InitializedStoragePanic
        } else {
            ScriptedSdmaStepV1::PromoteInitializedStorage(ScriptedFailureModeV1::ProcessTeardown)
        };
        let (backend, stream, _, device) = scripted_direct_backend_v1(4096, [step]);
        let mut backend = ManuallyDrop::new(backend);
        let original = owner(&backend, device);
        let (_, kernel) = load(&mut backend, false);
        let submission = submit_gated(&mut backend, stream, kernel, &[device]);
        let launch = Arc::as_ptr(&backend.pending_compute[&submission].launch);
        let reservations = backend.compute_completion_reservations;
        resolve_gate(&mut backend, submission, PeerComputeResultV1::Succeeded);
        let result = catch_unwind(AssertUnwindSafe(|| backend.flush_stream_v1(stream)));
        if panic {
            assert_eq!(
                result.unwrap_err().downcast_ref::<&str>(),
                Some(&"scripted initialized-storage conversion panic")
            );
            assert_eq!(
                backend
                    .scripted_sdma
                    .as_ref()
                    .unwrap()
                    .storage_conversion_custody(),
                Some(original)
            );
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
            assert!(backend.terminal_sdma_custody.is_some());
        }
        assert!(backend.terminal);
        assert_eq!(
            Arc::as_ptr(&backend.pending_compute[&submission].launch),
            launch
        );
        assert_eq!(
            backend.pending_compute_streams[&stream].front(),
            Some(&submission)
        );
        assert_eq!(backend.compute_completion_reservations, reservations);
        assert!(matches!(
            backend.allocations[&device].sdma_storage,
            KfdRuntimeSdmaStorageV1::InFlight(_)
        ));
        assert_eq!(
            backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
            2
        );
        assert_eq!(
            backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
            0
        );
        discard_scripted_fixture(backend);
    }
}

fn three_release_steps() -> Vec<ScriptedSdmaStepV1> {
    (0..3)
        .flat_map(|_| {
            [
                ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ]
        })
        .collect()
}

fn raw_three(steps: Vec<ScriptedSdmaStepV1>) -> (ManuallyDrop<KfdRuntimeBackendV1>, u64, [u64; 3]) {
    let (backend, stream, allocations) =
        scripted_persistent_backend_with_steps_v1::<3>(4096, steps);
    let mut backend = ManuallyDrop::new(backend);
    for allocation in allocations {
        backend.normalize_h2d_ready_v1(allocation).unwrap();
    }
    (backend, stream, allocations)
}

#[test]
fn initialized_storage_runtime_public_three_binding_progress_converts_every_binding() {
    let mut steps = (0..3)
        .map(|_| ScriptedSdmaStepV1::PromoteInitializedStorage(ScriptedFailureModeV1::Success))
        .collect::<Vec<_>>();
    steps.extend(three_release_steps());
    let (mut backend, stream, allocations) = raw_three(steps);
    let original = allocations.map(|allocation| owner(&backend, allocation));
    let (module, kernel) = load(&mut backend, true);
    let submission =
        submit_scripted_three_binding_v1(&mut backend, stream, kernel, allocations, 4096);
    backend.flush_stream_v1(stream).unwrap();
    let Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistent {
        admissions,
        restore_shells,
        ..
    }) = backend
        .active
        .as_ref()
        .and_then(|active| active.execution.as_ref())
    else {
        panic!("all-storage R/R/W must publish persistently")
    };
    assert!(admissions.iter().all(
        |admission| admission.source == PersistentFullRangeComputeSourceV1::InitializedStorage
    ));
    assert!(
        restore_shells
            .iter()
            .all(|shell| shell.initialized.is_some() && shell.replay.is_some())
    );
    assert_eq!(complete(&mut backend, submission), BackendPollV1::Succeeded);
    assert_eq!(
        allocations.map(|allocation| owner(&backend, allocation)),
        original
    );
    assert_eq!(
        backend
            .last_launch_performance_v1()
            .unwrap()
            .user_data_materializations(),
        0
    );
    backend.release_submission_v1(submission).unwrap();
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
    drop(ManuallyDrop::into_inner(backend));
}

#[test]
fn initialized_storage_runtime_three_restore_preflights_all_shells_before_owner_movement() {
    for bad in 0..3 {
        let steps = (0..3)
            .map(|_| ScriptedSdmaStepV1::PromoteInitializedStorage(ScriptedFailureModeV1::Success))
            .collect();
        let (mut backend, _, allocations) = raw_three(steps);
        for allocation in allocations {
            assert_eq!(
                backend.convert_initialized_storage_v1(allocation).unwrap(),
                None
            );
        }
        let original = allocations.map(|allocation| owner(&backend, allocation));
        let admissions = std::array::from_fn(|index| PersistentFullRangeComputeAdmissionV1 {
            allocation: allocations[index],
            access: if index == 2 {
                RuntimeAccessV1::Write
            } else {
                RuntimeAccessV1::Read
            },
            source: PersistentFullRangeComputeSourceV1::InitializedStorage,
        });
        let mut shells = backend
            .prepare_three_binding_restore_shells_v1(admissions)
            .unwrap();
        let (inputs, promotions) = backend
            .take_three_binding_persistent_inputs_v1(admissions, 777)
            .unwrap();
        shells[bad].initialized = None;
        assert!(matches!(
            backend.restore_three_binding_persistent_inputs_v1(
                admissions, 777, inputs, promotions, shells
            ),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(allocations.iter().all(|allocation| matches!(
            backend.allocations[allocation].sdma_storage,
            KfdRuntimeSdmaStorageV1::ComputeInFlight(777)
        )));
        let Some(KfdRuntimeTerminalSdmaCustodyV1::ThreeBindingPersistentInputs(inputs)) =
            backend.terminal_sdma_custody.as_ref()
        else {
            panic!("complete roster must be rooted")
        };
        for (index, input) in inputs.iter().enumerate() {
            let KfdRuntimePersistentComputeInputV1::ScriptedStorage(device) = input else {
                panic!("cancellation must preserve source")
            };
            assert_eq!(device.scripted_owner_id(), Some(original[index]));
        }
        assert_eq!(
            backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
            0
        );
        discard_scripted_fixture(backend);
    }
}

#[test]
fn initialized_storage_runtime_partial_roster_conversion_failure_preserves_every_owner() {
    for bad in 0..3 {
        for mode in 0..4 {
            let mut steps = (0..bad)
                .map(|_| {
                    ScriptedSdmaStepV1::PromoteInitializedStorage(ScriptedFailureModeV1::Success)
                })
                .collect::<Vec<_>>();
            steps.push(match mode {
                0 => {
                    ScriptedSdmaStepV1::PromoteInitializedStorage(ScriptedFailureModeV1::Retryable)
                }
                1 => ScriptedSdmaStepV1::InitializedStorageNotEligible(
                    Ineligible::IncompleteInitialization,
                ),
                2 => ScriptedSdmaStepV1::PromoteInitializedStorage(
                    ScriptedFailureModeV1::ProcessTeardown,
                ),
                _ => ScriptedSdmaStepV1::InitializedStoragePanic,
            });
            if mode == 1 {
                steps.extend((bad + 1..3).map(|_| {
                    ScriptedSdmaStepV1::PromoteInitializedStorage(ScriptedFailureModeV1::Success)
                }));
            }
            if mode < 2 {
                steps.extend(three_release_steps());
            }
            let (mut backend, stream, allocations) = raw_three(steps);
            #[cfg(feature = "hardware-qualification")]
            let authority = {
                let admitted =
                    crate::qualification_gfx942_r57_n3_v1::admit_gfx942_r57_n3_qualification_v1()
                        .unwrap();
                let observation = admitted.observation_v1();
                backend.launch_gate = KfdRuntimeLaunchGateV1::ExactGfx942R57N3(admitted);
                observation
            };
            let original = allocations.map(|allocation| owner(&backend, allocation));
            let (module, kernel) = load(&mut backend, true);
            let submission = submit_gated(&mut backend, stream, kernel, &allocations);
            resolve_gate(&mut backend, submission, PeerComputeResultV1::Succeeded);
            let result = catch_unwind(AssertUnwindSafe(|| backend.flush_stream_v1(stream)));
            if mode == 3 {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"scripted initialized-storage conversion panic")
                );
                assert_eq!(
                    backend
                        .scripted_sdma
                        .as_ref()
                        .unwrap()
                        .storage_conversion_custody(),
                    Some(original[bad])
                );
            } else if mode == 2 {
                assert!(matches!(
                    result.unwrap(),
                    Err(RuntimeBackendFailureV1::Terminal(_))
                ));
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(RuntimeBackendFailureV1::Quiescent(_))
                ));
                assert_eq!(
                    complete(&mut backend, submission),
                    BackendPollV1::Failed { code: -1 }
                );
            }
            for (index, allocation) in allocations.into_iter().enumerate() {
                assert!(
                    backend.allocations[&allocation]
                        .persistent_storage_restore
                        .is_none()
                );
                if mode >= 2 && index == bad {
                    assert!(matches!(
                        backend.allocations[&allocation].sdma_storage,
                        KfdRuntimeSdmaStorageV1::InFlight(_)
                    ));
                } else {
                    assert_eq!(owner(&backend, allocation), original[index]);
                    assert_eq!(
                        matches!(
                            backend.allocations[&allocation].sdma_storage,
                            KfdRuntimeSdmaStorageV1::InitializedStorage(_)
                        ),
                        index < bad || (mode == 1 && index != bad)
                    );
                }
            }
            assert!(backend.active.is_none());
            assert!(backend.last_launch_performance_v1().is_none());
            #[cfg(feature = "hardware-qualification")]
            assert_eq!(authority.authorization_calls_v1(), 0);
            assert_eq!(
                backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
                3
            );
            assert_eq!(
                backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
                0
            );
            if mode >= 2 {
                assert!(backend.pending_compute.contains_key(&submission));
                assert_eq!(
                    backend.pending_compute_streams[&stream].front(),
                    Some(&submission)
                );
                discard_scripted_fixture(backend);
            } else {
                assert!(backend.pending_compute.is_empty());
                assert_eq!(backend.compute_completion_reservations, 0);
                backend.release_submission_v1(submission).unwrap();
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
                assert!(backend.scripted_sdma.as_ref().unwrap().is_exhausted());
                assert_eq!(
                    backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
                    0
                );
                backend.shutdown_native_v1().unwrap();
                drop(ManuallyDrop::into_inner(backend));
            }
        }
    }
}

#[test]
fn initialized_storage_runtime_ordering_blocks_conversion_then_replaces_foreign_control() {
    let steps = [
        ScriptedSdmaStepV1::PromoteInitializedStorage(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ];
    let (backend, stream, [a, b]) = scripted_persistent_backend_with_steps_v1::<2>(4096, steps);
    let mut backend = ManuallyDrop::new(backend);
    backend.normalize_h2d_ready_v1(b).unwrap();
    let original = owner(&backend, b);
    let (module, kernel) = load(&mut backend, false);
    backend.scripted_persistent_publication_retries = 10;
    let first = submit_scripted_read_v1(&mut backend, stream, kernel, a, 4096, &[]);
    backend.flush_stream_v1(stream).unwrap();
    let second = submit_scripted_read_v1(&mut backend, stream, kernel, b, 4096, &[]);
    let remaining = backend.scripted_sdma.as_ref().unwrap().remaining_steps();
    assert_eq!(backend.poll_v1(second).unwrap(), BackendPollV1::Pending);
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
        remaining
    );
    assert_eq!(owner(&backend, b), original);
    assert!(matches!(
        backend.allocations[&b].sdma_storage,
        KfdRuntimeSdmaStorageV1::Device(_)
    ));
    assert!(backend.pending_compute.contains_key(&second));
    backend.scripted_persistent_publication_retries = 0;
    assert_eq!(complete(&mut backend, first), BackendPollV1::Succeeded);
    assert_eq!(backend.retained_persistent_dispatch.unwrap().allocation, a);
    assert_eq!(backend.convert_initialized_storage_v1(b).unwrap(), None);
    let launch = Arc::clone(&backend.pending_compute[&second].launch);
    let prepared = backend
        .prepare_launch(launch.borrowed(), true, false)
        .unwrap();
    assert!(
        backend.retained_persistent_dispatch.is_none(),
        "incompatible control must be released before publication can overwrite its identity"
    );
    drop(prepared);
    backend.flush_stream_v1(stream).unwrap();
    assert_eq!(complete(&mut backend, second), BackendPollV1::Succeeded);
    assert_eq!(backend.retained_persistent_dispatch.unwrap().allocation, b);
    assert!(
        !backend
            .last_launch_performance_v1()
            .unwrap()
            .persistent_control_reused()
    );
    assert_eq!(owner(&backend, b), original);
    for submission in [first, second] {
        backend.release_submission_v1(submission).unwrap();
    }
    backend.unload_module_v1(module).unwrap();
    for allocation in [a, b] {
        backend
            .allocations
            .get_mut(&allocation)
            .unwrap()
            .sdma_backed = false;
        backend.release_allocation_v1(allocation).unwrap();
    }
    backend.destroy_stream_v1(stream).unwrap();
    assert!(backend.scripted_sdma.as_ref().unwrap().is_exhausted());
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
        0
    );
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
        0
    );
    backend.shutdown_native_v1().unwrap();
    drop(ManuallyDrop::into_inner(backend));
}

#[test]
fn initialized_storage_runtime_peer_gate_precedes_conversion_through_every_ingress() {
    for success in [false, true] {
        let mut steps = vec![ScriptedSdmaStepV1::PromoteInitializedStorage(
            ScriptedFailureModeV1::Success,
        )];
        steps.extend(scripted_release_steps_v1());
        let (backend, stream, host, device) = scripted_direct_backend_v1(4096, steps);
        let mut backend = ManuallyDrop::new(backend);
        let original = owner(&backend, device);
        let (module, kernel) = load(&mut backend, false);
        // Isolate the genuine child gate; this does not open router peer admission.
        let submission = submit_gated(&mut backend, stream, kernel, &[device]);
        let remaining = backend.scripted_sdma.as_ref().unwrap().remaining_steps();
        for ingress in 0..7 {
            match ingress {
                0 => assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending),
                1 => assert_eq!(
                    backend.wait_v1(submission, Instant::now()).unwrap(),
                    BackendPollV1::Pending
                ),
                2 => assert_eq!(
                    backend
                        .wait_v1(submission, Instant::now() + Duration::from_millis(1))
                        .unwrap(),
                    BackendPollV1::Pending
                ),
                3 => backend.flush_stream_v1(stream).unwrap(),
                4 => {
                    let pending = backend.pending_compute.remove(&submission).unwrap();
                    assert_eq!(
                        backend.progress_pending_compute_v1(pending).unwrap(),
                        BackendPollV1::Pending
                    );
                }
                5 => {
                    let pending = backend.pending_compute.remove(&submission).unwrap();
                    assert_eq!(
                        backend.observe_pending_compute_v1(pending).unwrap(),
                        BackendPollV1::Pending
                    );
                }
                _ => assert_eq!(
                    backend
                        .drain_v1(submission, Instant::now() + Duration::from_millis(1))
                        .unwrap(),
                    BackendPollV1::Pending
                ),
            }
            assert_eq!(
                backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
                remaining
            );
            assert_eq!(owner(&backend, device), original);
            assert!(matches!(
                backend.allocations[&device].sdma_storage,
                KfdRuntimeSdmaStorageV1::Device(_)
            ));
            assert!(
                backend.allocations[&device]
                    .persistent_storage_restore
                    .is_none()
            );
            assert!(backend.active.is_none());
        }
        let result = if success {
            PeerComputeResultV1::Succeeded
        } else {
            PeerComputeResultV1::Failed
        };
        resolve_gate(&mut backend, submission, result);
        if success {
            backend.flush_stream_v1(stream).unwrap();
        }
        assert_eq!(
            complete(&mut backend, submission),
            if success {
                BackendPollV1::Succeeded
            } else {
                BackendPollV1::Failed { code: -1 }
            }
        );
        assert_eq!(owner(&backend, device), original);
        if success {
            backend.release_submission_v1(submission).unwrap();
            backend.unload_module_v1(module).unwrap();
            clean_scripted_direct_backend_v1(&mut backend, stream, host, device, None);
            drop(ManuallyDrop::into_inner(backend));
        } else {
            assert_eq!(
                backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
                remaining
            );
            assert!(matches!(
                backend.allocations[&device].sdma_storage,
                KfdRuntimeSdmaStorageV1::Device(_)
            ));
            assert_eq!(
                backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
                0
            );
            discard_scripted_fixture(backend);
        }
    }
}
