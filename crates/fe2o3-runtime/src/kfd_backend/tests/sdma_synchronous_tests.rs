use super::sdma_host_write_tests::{discard_scripted_fixture, fixture, host_observation};
use super::*;
use crate::kfd_backend::kfd_backend_sdma_seam::{ScriptedTerminalCustodyV1, SdmaOwnerDiagnosticV1};
use crate::kfd_backend::sdma_synchronous::{SynchronousSdmaCustodyV1, SynchronousSdmaPhaseV1};
use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};

type PairFacts = (u64, Vec<u8>, (u64, Vec<u8>, Option<[u8; 32]>));

fn pair_facts(pair: &DirectionalSdmaPairOwnerV1) -> PairFacts {
    (
        pair.device.scripted_owner_id().unwrap(),
        pair.device.scripted_bytes().unwrap().to_vec(),
        host_observation(&pair.host),
    )
}

fn root(backend: &KfdRuntimeBackendV1) -> &SynchronousSdmaCustodyV1 {
    let Some(KfdRuntimeTerminalSdmaCustodyV1::Synchronous(root)) = &backend.terminal_sdma_custody
    else {
        panic!("synchronous root must survive");
    };
    root
}

fn pending(owner: &DirectionalSdmaSubmissionOwnerV1) -> &DirectionalSdmaPairOwnerV1 {
    match owner {
        DirectionalSdmaSubmissionOwnerV1::Scripted(submission) => submission.pair(),
        _ => panic!("scripted pending"),
    }
}

fn completed_pair(owner: &DirectionalSdmaCompletedOwnerV1) -> &DirectionalSdmaPairOwnerV1 {
    match owner {
        DirectionalSdmaCompletedOwnerV1::Scripted(completed) => completed.pair(),
        _ => panic!("scripted completed"),
    }
}

fn retained_pair(backend: &KfdRuntimeBackendV1) -> &DirectionalSdmaPairOwnerV1 {
    match &root(backend).phase {
        SynchronousSdmaPhaseV1::Pair(pair) | SynchronousSdmaPhaseV1::RetiredPair(pair) => pair,
        SynchronousSdmaPhaseV1::Pending(owner) => pending(owner),
        SynchronousSdmaPhaseV1::Completed(owner) => completed_pair(owner),
        SynchronousSdmaPhaseV1::Terminal(SdmaTerminalCustodyV1::Scripted(custody)) => match custody
        {
            ScriptedTerminalCustodyV1::Pair(pair) => pair,
            ScriptedTerminalCustodyV1::Submission(owner) => pending(owner),
            ScriptedTerminalCustodyV1::Completed(owner) => completed_pair(owner),
            _ => panic!("terminal copy pair"),
        },
        SynchronousSdmaPhaseV1::LowerOwned => {
            let driver = backend.scripted_sdma.as_ref().unwrap();
            if let Some(submission) = driver.wait_custody() {
                submission.pair()
            } else {
                driver.retirement_custody().unwrap().pair()
            }
        }
        _ => panic!("copy must retain both owners"),
    }
}

fn device_box(backend: &KfdRuntimeBackendV1, device: u64) -> (usize, u64) {
    let KfdRuntimeSdmaStorageV1::Device(owner) = &backend.allocations[&device].sdma_storage else {
        panic!("indexed device")
    };
    (
        &**owner as *const DirectionalSdmaDeviceOwnerV1 as usize,
        owner.scripted_owner_id().unwrap(),
    )
}

fn install(backend: &mut KfdRuntimeBackendV1, device: u64, host: SdmaBufferOwnerV1) {
    backend.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Synchronous(
        SynchronousSdmaCustodyV1 {
            allocation: device,
            direction: Gfx942PersistentSdmaDirectionV1::HostToDevice,
            request: DirectionalSdmaCopyRequestV1 {
                host_offset: 0,
                device_offset: 0,
                copy_bytes: 8,
            },
            operation: "upload",
            shell: None,
            phase: SynchronousSdmaPhaseV1::Host(host),
        },
    ));
    backend.extract_synchronous_device_v1();
}

fn assert_terminal_inert(backend: &mut KfdRuntimeBackendV1, host: u64, device: u64) {
    assert!(backend.terminal);
    let observe = |backend: &KfdRuntimeBackendV1| {
        let indexed = backend
            .allocations
            .ordinary_iter()
            .map(|(&id, record)| {
                (
                    id,
                    (record.device, record.kind, record.alignment),
                    (
                        Arc::clone(&record.bytes),
                        record.bytes.as_ptr() as usize,
                        record.content_sha256,
                        record.last_full_host_write.clone(),
                        record.native_dirty.clone(),
                    ),
                    (
                        record.sdma_backed,
                        record.sdma_initialized,
                        record.sdma_shadow_dirty,
                        record.scripted_three_binding_replay,
                    ),
                    format!("{:?}", record.sdma_storage),
                )
            })
            .collect::<Vec<_>>();
        let root = root(backend);
        let driver = backend.scripted_sdma.as_ref().unwrap();
        (
            indexed,
            pair_facts(retained_pair(backend)),
            (
                root.shell.as_ref().map(|shell| shell.as_ptr() as usize),
                root.allocation,
                root.direction,
                root.request,
                std::mem::discriminant(&root.phase),
            ),
            (backend.staged_context_bytes, backend.next_handle),
            (
                driver.remaining_steps(),
                driver.live_owner_count(),
                driver.wait_custody().is_some(),
                driver.retirement_custody().is_some(),
            ),
            backend
                .profiler
                .as_ref()
                .unwrap()
                .recorded_events_for_test_v1()
                .to_vec(),
        )
    };
    let before = observe(backend);
    assert!(matches!(
        backend.allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(matches!(
        backend.write_allocation_v1(device, 0, &[1]),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(matches!(
        backend.read_allocation_v1(host, 0, &mut [0; 1]),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(matches!(
        backend.release_allocation_v1(device),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(matches!(
        backend.shutdown_native_v1(),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(matches!(
        backend.finish_profiler_v1(),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert_eq!(observe(backend), before);
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
        0
    );
}

fn submit(outcome: ScriptedFailureModeV1) -> ScriptedSdmaStepV1 {
    scripted_submit_step_v1(
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        0,
        0,
        8,
        outcome,
    )
}

fn completed() -> ScriptedSdmaStepV1 {
    ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Completed {
        direction: None,
        copy_bytes: None,
    })
}

fn execute(
    backend: &mut KfdRuntimeBackendV1,
    device: u64,
    stage: SdmaBufferOwnerV1,
) -> Result<SdmaBufferOwnerV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    backend.execute_synchronous_directional_sdma_v1(
        device,
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        stage,
        0,
        0,
        8,
        "upload",
    )
}

#[test]
fn sdma_synchronous_success_reuses_box_and_exact_offset_owners_in_both_directions() {
    for direction in [
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        Gfx942PersistentSdmaDirectionV1::DeviceToHost,
    ] {
        let (backend, _, host, device) = fixture(
            23,
            [
                ScriptedSdmaStepV1::Write {
                    offset: 0,
                    byte_len: 17,
                },
                scripted_submit_step_v1(direction, 3, 5, 12, ScriptedFailureModeV1::Success),
                completed(),
                ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ],
        );
        let mut backend = ManuallyDrop::new(backend);
        let device_before = device_box(&backend, device);
        let observe_neighbor = |backend: &KfdRuntimeBackendV1| {
            let record = &backend.allocations[&host];
            let KfdRuntimeSdmaStorageV1::Host(owner) = &record.sdma_storage else {
                panic!()
            };
            (
                (record.device, record.kind, record.alignment),
                (
                    Arc::clone(&record.bytes),
                    record.bytes.as_ptr() as usize,
                    record.content_sha256,
                    record.last_full_host_write.clone(),
                    record.native_dirty.clone(),
                ),
                host_observation(owner),
                (
                    record.sdma_backed,
                    record.sdma_initialized,
                    record.sdma_shadow_dirty,
                    record.scripted_three_binding_replay,
                ),
            )
        };
        let neighbor = observe_neighbor(&backend);
        let staged_bytes = backend.staged_context_bytes;
        let mut stage = backend.scripted_sdma.as_ref().unwrap().test_host_owner(17);
        backend
            .directional_sdma_ops_v1()
            .write_host(&mut stage, 0, &[0x71; 17])
            .unwrap();
        let stage_id = host_observation(&stage).0;
        let (returned, allocations) = counted_allocations_for_test_v1(|| {
            backend
                .execute_synchronous_directional_sdma_v1(device, direction, stage, 3, 5, 12, "copy")
        });
        let returned = returned.unwrap();
        assert_eq!(allocations, 0);
        assert_eq!(device_box(&backend, device), device_before);
        assert!(backend.terminal_sdma_custody.is_none());
        assert!(!backend.terminal);
        let mut host_bytes = vec![0x71; 17];
        let mut device_bytes = vec![0; 23];
        match direction {
            Gfx942PersistentSdmaDirectionV1::HostToDevice => device_bytes[5..17].fill(0x71),
            Gfx942PersistentSdmaDirectionV1::DeviceToHost => host_bytes[3..15].fill(0),
        }
        assert_eq!(host_observation(&returned), (stage_id, host_bytes, None));
        let KfdRuntimeSdmaStorageV1::Device(owner) = &backend.allocations[&device].sdma_storage
        else {
            panic!()
        };
        assert_eq!(owner.scripted_bytes().unwrap(), device_bytes);
        assert_eq!(observe_neighbor(&backend), neighbor);
        assert_eq!(backend.staged_context_bytes, staged_bytes);
        backend
            .recycle_transient_sdma_buffer_v1(returned, "copy")
            .unwrap();
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

#[test]
fn sdma_synchronous_retry_restores_original_box_before_cleanup_and_remains_healthy() {
    let (backend, _, _, device) = fixture(
        8,
        [
            submit(ScriptedFailureModeV1::Retryable),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            submit(ScriptedFailureModeV1::Success),
            completed(),
            ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ],
    );
    let mut backend = ManuallyDrop::new(backend);
    let original = device_box(&backend, device);
    let stage = backend.scripted_sdma.as_ref().unwrap().test_host_owner(8);
    let Err(RuntimeBackendFailureV1::Rejected(error)) = execute(&mut backend, device, stage) else {
        panic!("retryable publication")
    };
    assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Native);
    assert_eq!(
        error.detail(),
        "KFD upload publication: scripted submission retryable"
    );
    assert_eq!(device_box(&backend, device), original);
    assert!(!backend.terminal);
    assert!(backend.terminal_sdma_custody.is_none());
    let stage = backend.scripted_sdma.as_ref().unwrap().test_host_owner(8);
    let returned = execute(&mut backend, device, stage).unwrap();
    assert_eq!(device_box(&backend, device), original);
    backend
        .recycle_transient_sdma_buffer_v1(returned, "upload")
        .unwrap();
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
        0
    );
    discard_scripted_fixture(backend);
}

#[test]
fn sdma_synchronous_timeout_and_teardown_retain_exact_pair_and_shell() {
    for (outcome, phase, message) in [
        (
            ScriptedExecutionOutcomeV1::Pending,
            "pending",
            "KFD upload completion became ambiguous: scripted wait timed out",
        ),
        (
            ScriptedExecutionOutcomeV1::Retryable,
            "terminal",
            "KFD upload execution became ambiguous: directional SDMA synchronous wait returned non-timeout retryable custody: scripted wait retryable",
        ),
        (
            ScriptedExecutionOutcomeV1::ProcessTeardown,
            "terminal",
            "KFD upload execution became ambiguous: scripted wait teardown",
        ),
    ] {
        let (backend, _, host, device) = fixture(
            8,
            [
                submit(ScriptedFailureModeV1::Success),
                ScriptedSdmaStepV1::Wait(outcome),
                ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
            ],
        );
        let mut backend = ManuallyDrop::new(backend);
        let (address, owner) = device_box(&backend, device);
        let stage = backend.scripted_sdma.as_ref().unwrap().test_host_owner(8);
        let expected = (owner, vec![0; 8], host_observation(&stage));
        let Err(RuntimeBackendFailureV1::Terminal(error)) = execute(&mut backend, device, stage)
        else {
            panic!("terminal copy")
        };
        assert_eq!(error.detail(), message);
        assert_eq!(
            matches!(root(&backend).phase, SynchronousSdmaPhaseV1::Pending(_)),
            phase == "pending"
        );
        assert_eq!(
            root(&backend).shell.as_ref().unwrap().as_ptr() as usize,
            address
        );
        assert_eq!(pair_facts(retained_pair(&backend)), expected);
        assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 1);
        assert_terminal_inert(&mut backend, host, device);
        discard_scripted_fixture(backend);
    }
}

#[test]
fn sdma_synchronous_metadata_corruption_precedes_retirement_and_keeps_completed_pair() {
    let request = DirectionalSdmaCopyRequestV1 {
        host_offset: 0,
        device_offset: 0,
        copy_bytes: 8,
    };
    for outcome in [
        ScriptedExecutionOutcomeV1::Completed {
            direction: Some(Gfx942PersistentSdmaDirectionV1::DeviceToHost),
            copy_bytes: None,
        },
        ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: Some(7),
        },
        ScriptedExecutionOutcomeV1::CompletedWindow {
            direction: None,
            copy_bytes: None,
            requests: Some(vec![DirectionalSdmaCopyRequestV1 {
                host_offset: 1,
                ..request
            }]),
        },
        ScriptedExecutionOutcomeV1::CompletedWindow {
            direction: None,
            copy_bytes: None,
            requests: Some(vec![DirectionalSdmaCopyRequestV1 {
                device_offset: 1,
                ..request
            }]),
        },
        ScriptedExecutionOutcomeV1::CompletedWindow {
            direction: None,
            copy_bytes: None,
            requests: Some(vec![]),
        },
        ScriptedExecutionOutcomeV1::CompletedWindow {
            direction: None,
            copy_bytes: None,
            requests: Some(vec![request, request]),
        },
    ] {
        let (backend, _, host, device) = fixture(
            8,
            [
                submit(ScriptedFailureModeV1::Success),
                ScriptedSdmaStepV1::Wait(outcome),
                ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
            ],
        );
        let mut backend = ManuallyDrop::new(backend);
        let (address, owner) = device_box(&backend, device);
        let stage = backend.scripted_sdma.as_ref().unwrap().test_host_owner(8);
        let expected = (owner, vec![0; 8], host_observation(&stage));
        let Err(RuntimeBackendFailureV1::Terminal(error)) = execute(&mut backend, device, stage)
        else {
            panic!()
        };
        assert_eq!(
            error.detail(),
            "KFD upload completion metadata changed unexpectedly"
        );
        assert!(matches!(
            root(&backend).phase,
            SynchronousSdmaPhaseV1::Completed(_)
        ));
        assert_eq!(
            root(&backend).shell.as_ref().unwrap().as_ptr() as usize,
            address
        );
        assert_eq!(pair_facts(retained_pair(&backend)), expected);
        assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 1);
        assert_terminal_inert(&mut backend, host, device);
        discard_scripted_fixture(backend);
    }
}

#[test]
fn sdma_synchronous_retirement_rejection_teardown_and_panic_preserve_owners() {
    for case in 0..3 {
        let step = match case {
            0 => ScriptedSdmaStepV1::RetireCompletedRetry,
            1 => ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::ProcessTeardown),
            _ => ScriptedSdmaStepV1::RetirePanic,
        };
        let (backend, _, host, device) = fixture(
            8,
            [
                submit(ScriptedFailureModeV1::Success),
                completed(),
                step,
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ],
        );
        let mut backend = ManuallyDrop::new(backend);
        let (address, owner) = device_box(&backend, device);
        let stage = backend.scripted_sdma.as_ref().unwrap().test_host_owner(8);
        let expected = (owner, vec![0; 8], host_observation(&stage));
        match catch_unwind(AssertUnwindSafe(|| execute(&mut backend, device, stage))) {
            Ok(Err(RuntimeBackendFailureV1::Terminal(error))) if case < 2 => {
                assert_eq!(error.detail(), "KFD upload frontier retirement failed")
            }
            Err(payload) if case == 2 => assert_eq!(
                payload.downcast_ref::<&str>(),
                Some(&"scripted SDMA retirement panic")
            ),
            _ => panic!("retirement failure"),
        }
        match (&root(&backend).phase, case) {
            (SynchronousSdmaPhaseV1::Completed(_), 0)
            | (SynchronousSdmaPhaseV1::Terminal(_), 1)
            | (SynchronousSdmaPhaseV1::LowerOwned, 2) => {}
            _ => panic!("retained retirement phase"),
        }
        assert_eq!(
            root(&backend).shell.as_ref().unwrap().as_ptr() as usize,
            address
        );
        assert_eq!(pair_facts(retained_pair(&backend)), expected);
        assert_terminal_inert(&mut backend, host, device);
        discard_scripted_fixture(backend);
    }
}

#[test]
fn sdma_synchronous_driver_selection_and_actual_scripted_copy_panics_keep_custody() {
    for missing_driver in [true, false] {
        let count = if missing_driver { 8 } else { 9 };
        let (backend, _, host, device) = fixture(
            8,
            [
                scripted_submit_step_v1(
                    Gfx942PersistentSdmaDirectionV1::HostToDevice,
                    0,
                    0,
                    count,
                    ScriptedFailureModeV1::Success,
                ),
                completed(),
                ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
            ],
        );
        let mut backend = ManuallyDrop::new(backend);
        let (address, owner) = device_box(&backend, device);
        let stage = backend.scripted_sdma.as_ref().unwrap().test_host_owner(8);
        let expected = (owner, vec![0; 8], host_observation(&stage));
        let driver = if missing_driver {
            backend.scripted_sdma.take()
        } else {
            None
        };
        // Oversized copying deliberately bypasses public range admission at this private seam.
        let result = catch_unwind(AssertUnwindSafe(|| {
            backend.execute_synchronous_directional_sdma_v1(
                device,
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                stage,
                0,
                0,
                count,
                "upload",
            )
        }));
        assert!(result.is_err());
        if missing_driver {
            backend.scripted_sdma = driver;
        } else {
            assert_eq!(
                backend
                    .scripted_sdma
                    .as_ref()
                    .unwrap()
                    .wait_custody()
                    .unwrap()
                    .requests()[0]
                    .copy_bytes,
                9
            );
        }
        assert_eq!(pair_facts(retained_pair(&backend)), expected);
        assert_eq!(
            root(&backend).shell.as_ref().unwrap().as_ptr() as usize,
            address
        );
        assert_terminal_inert(&mut backend, host, device);
        discard_scripted_fixture(backend);
    }
}

#[test]
fn sdma_synchronous_restoration_rejects_changed_slots_before_recycling_or_diagnostics() {
    for case in 0..6 {
        let (backend, _, host, device) = fixture(
            8,
            [
                submit(ScriptedFailureModeV1::Retryable),
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ],
        );
        let mut backend = ManuallyDrop::new(backend);
        let (address, owner) = device_box(&backend, device);
        let stage = backend.scripted_sdma.as_ref().unwrap().test_host_owner(8);
        let expected = (owner, vec![0; 8], host_observation(&stage));
        install(&mut backend, device, stage);
        let result = backend.run_synchronous_lower_v1();
        let removed = if case == 0 {
            backend.allocations.remove(&device)
        } else {
            None
        };
        match case {
            1 => {
                backend.allocations.get_mut(&device).unwrap().kind =
                    RuntimeMemoryKindV1::HostVisible
            }
            2 => {
                let replacement = backend.scripted_sdma.as_ref().unwrap().test_host_owner(8);
                backend.allocations.get_mut(&device).unwrap().sdma_storage =
                    KfdRuntimeSdmaStorageV1::Host(replacement);
            }
            3 => backend.synchronous_copy_root_v1().allocation = u64::MAX,
            4 => {
                backend.allocations.get_mut(&device).unwrap().sdma_storage =
                    KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(827))
            }
            5 => {
                let replacement = backend.scripted_sdma.as_ref().unwrap().test_device_owner(8);
                backend.allocations.get_mut(&device).unwrap().sdma_storage =
                    KfdRuntimeSdmaStorageV1::Device(Box::new(replacement));
            }
            _ => {}
        }
        let replacement_host = if case == 2 {
            let KfdRuntimeSdmaStorageV1::Host(owner) = &backend.allocations[&device].sdma_storage
            else {
                panic!()
            };
            Some(host_observation(owner))
        } else {
            None
        };
        let replacement_device = if case == 5 {
            Some(device_box(&backend, device))
        } else {
            None
        };
        let Err(RuntimeBackendFailureV1::Terminal(error)) =
            backend.settle_synchronous_execution_v1(result, |_| panic!("diagnostic must not run"))
        else {
            panic!()
        };
        assert_eq!(
            error.detail(),
            "synchronous directional SDMA restoration slot changed unexpectedly"
        );
        assert_eq!(pair_facts(retained_pair(&backend)), expected);
        assert_eq!(
            root(&backend).shell.as_ref().unwrap().as_ptr() as usize,
            address
        );
        assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 1);
        if let Some(expected) = replacement_host {
            let KfdRuntimeSdmaStorageV1::Host(owner) = &backend.allocations[&device].sdma_storage
            else {
                panic!()
            };
            assert_eq!(host_observation(owner), expected);
        }
        if let Some(expected) = replacement_device {
            assert_eq!(device_box(&backend, device), expected);
            let KfdRuntimeSdmaStorageV1::Device(owner) = &backend.allocations[&device].sdma_storage
            else {
                panic!()
            };
            assert_eq!(owner.scripted_bytes().unwrap(), &[0; 8]);
        }
        assert_terminal_inert(&mut backend, host, device);
        drop(removed);
        discard_scripted_fixture(backend);
    }
}

#[test]
fn sdma_synchronous_typed_diagnostic_panic_keeps_first_payload_and_retained_terminal_owners() {
    for teardown in [false, true] {
        let (backend, _, host, device) = fixture(
            8,
            [
                submit(ScriptedFailureModeV1::Success),
                ScriptedSdmaStepV1::Wait(if teardown {
                    ScriptedExecutionOutcomeV1::ProcessTeardown
                } else {
                    ScriptedExecutionOutcomeV1::Pending
                }),
            ],
        );
        let mut backend = ManuallyDrop::new(backend);
        let stage = backend.scripted_sdma.as_ref().unwrap().test_host_owner(8);
        install(&mut backend, device, stage);
        let expected = pair_facts(retained_pair(&backend));
        let detail = SdmaOwnerDiagnosticV1::Native(
            fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract("typed synchronous cause"),
        );
        let failure = match backend.run_synchronous_lower_v1() {
            Err(DirectionalSdmaSynchronousExecutionFailureV1::RetryableTimeout {
                submission,
                ..
            }) if !teardown => DirectionalSdmaSynchronousExecutionFailureV1::RetryableTimeout {
                detail,
                submission,
            },
            Err(DirectionalSdmaSynchronousExecutionFailureV1::ProcessTeardown {
                custody, ..
            }) if teardown => {
                DirectionalSdmaSynchronousExecutionFailureV1::ProcessTeardown { detail, custody }
            }
            _ => panic!("expected exact scripted failure"),
        };
        let payload = Box::new(517_u64);
        let address = &*payload as *const u64;
        let caught = catch_unwind(AssertUnwindSafe(|| {
            backend.settle_synchronous_execution_v1(Err(failure), |detail| {
                assert!(matches!(
                    detail,
                    SdmaOwnerDiagnosticV1::Native(
                        fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract(
                            "typed synchronous cause"
                        )
                    )
                ));
                std::panic::resume_unwind(payload)
            })
        }))
        .unwrap_err()
        .downcast::<u64>()
        .unwrap();
        assert_eq!(&*caught as *const u64, address);
        assert_eq!(*caught, 517);
        assert_eq!(pair_facts(retained_pair(&backend)), expected);
        assert_terminal_inert(&mut backend, host, device);
        discard_scripted_fixture(backend);
    }
}

#[test]
fn sdma_synchronous_normalizes_genuine_h2d_ready_without_changing_device_identity() {
    let len = HOST_VISIBLE_MEMORY_PAGE_BYTES_V1 as usize;
    let mut steps = vec![
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len: len,
        },
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            len as u32,
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
    ];
    steps.extend([
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
            0,
            0,
            8,
            ScriptedFailureModeV1::Success,
        ),
        completed(),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]);
    let (backend, stream, host, device) = fixture(len, steps);
    let mut backend = ManuallyDrop::new(backend);
    let original = device_box(&backend, device).1;
    backend
        .write_allocation_v1(host, 0, &vec![0x5a; len])
        .unwrap();
    let (source, destination) = scripted_copy_regions_v1(host, device, len as u64);
    let submission = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(
        backend.poll_v1(submission).unwrap(),
        BackendPollV1::Succeeded
    );
    assert!(matches!(
        backend.allocations[&device].sdma_storage,
        KfdRuntimeSdmaStorageV1::H2dReady(_)
    ));
    let digest = backend.allocations[&device].content_sha256;
    let shadow = Arc::clone(&backend.allocations[&device].bytes);
    let stage = backend.scripted_sdma.as_ref().unwrap().test_host_owner(8);
    let returned = backend
        .execute_synchronous_directional_sdma_v1(
            device,
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
            stage,
            0,
            0,
            8,
            "download",
        )
        .unwrap();
    assert_eq!(device_box(&backend, device).1, original);
    assert_eq!(host_observation(&returned).1, vec![0x5a; 8]);
    assert_eq!(backend.allocations[&device].content_sha256, digest);
    assert!(Arc::ptr_eq(&backend.allocations[&device].bytes, &shadow));
    backend
        .recycle_transient_sdma_buffer_v1(returned, "download")
        .unwrap();
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
        0
    );
    discard_scripted_fixture(backend);
}

#[test]
fn sdma_synchronous_busy_and_publication_rejection_recycle_before_original_error() {
    for busy in [false, true] {
        for recycle in [
            ScriptedRecycleOutcomeV1::Success,
            ScriptedRecycleOutcomeV1::Recovered,
            ScriptedRecycleOutcomeV1::Panic,
        ] {
            let mut steps = Vec::new();
            if !busy {
                steps.push(submit(ScriptedFailureModeV1::Retryable));
            }
            steps.push(ScriptedSdmaStepV1::Recycle(recycle));
            let (backend, _, host, device) = fixture(8, steps);
            let mut backend = ManuallyDrop::new(backend);
            let original = device_box(&backend, device);
            let stage = backend.scripted_sdma.as_ref().unwrap().test_host_owner(8);
            let expected = host_observation(&stage);
            let allocation = if busy { u64::MAX } else { device };
            let result = catch_unwind(AssertUnwindSafe(|| {
                execute(&mut backend, allocation, stage)
            }));
            match recycle {
                ScriptedRecycleOutcomeV1::Success => {
                    let Err(RuntimeBackendFailureV1::Rejected(error)) = result.unwrap() else {
                        panic!()
                    };
                    assert_eq!(
                        error.kind(),
                        if busy {
                            KfdRuntimeBackendErrorKindV1::Busy
                        } else {
                            KfdRuntimeBackendErrorKindV1::Native
                        }
                    );
                    assert_eq!(
                        error.detail(),
                        if busy {
                            "persistent device allocation is retained by pending work"
                        } else {
                            "KFD upload publication: scripted submission retryable"
                        }
                    );
                    assert!(!backend.terminal);
                    assert!(backend.terminal_sdma_custody.is_none());
                    assert_eq!(
                        backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
                        2
                    );
                }
                ScriptedRecycleOutcomeV1::Recovered => {
                    let Err(RuntimeBackendFailureV1::Terminal(error)) = result.unwrap() else {
                        panic!()
                    };
                    assert_eq!(
                        error.detail(),
                        "KFD upload transient release became ambiguous: scripted recycle recovered"
                    );
                    assert_eq!(
                        host_observation(super::sdma_host_write_tests::retained_host(&backend)),
                        expected
                    );
                }
                ScriptedRecycleOutcomeV1::Panic => {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<&str>(),
                        Some(&"scripted recycle panic")
                    );
                    let retained = backend
                        .scripted_sdma
                        .as_ref()
                        .unwrap()
                        .recycle_custody()
                        .unwrap()
                        .observation();
                    assert_eq!((retained.0, retained.1.to_vec(), retained.2), expected);
                }
                _ => unreachable!(),
            }
            assert_eq!(device_box(&backend, device), original);
            assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 0);
            assert_eq!(
                backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
                0
            );
            if recycle != ScriptedRecycleOutcomeV1::Success {
                super::sdma_host_write_tests::assert_terminal_retries_inert(
                    &mut backend,
                    host,
                    device,
                );
            }
            discard_scripted_fixture(backend);
        }
    }
}

#[test]
fn sdma_synchronous_publication_diagnostic_panic_follows_restoration_and_cleanup() {
    let (backend, _, host, device) = fixture(
        8,
        [
            submit(ScriptedFailureModeV1::Retryable),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ],
    );
    let mut backend = ManuallyDrop::new(backend);
    let original = device_box(&backend, device);
    let stage = backend.scripted_sdma.as_ref().unwrap().test_host_owner(8);
    install(&mut backend, device, stage);
    let Err(DirectionalSdmaSynchronousExecutionFailureV1::RetryableBeforePublication {
        pair, ..
    }) = backend.run_synchronous_lower_v1()
    else {
        panic!()
    };
    let payload = Box::new(619_u64);
    let address = &*payload as *const u64;
    let caught = catch_unwind(AssertUnwindSafe(|| {
        backend.settle_synchronous_execution_v1(
            Err(
                DirectionalSdmaSynchronousExecutionFailureV1::RetryableBeforePublication {
                    detail: SdmaOwnerDiagnosticV1::Native(
                        fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract(
                            "typed publication cause",
                        ),
                    ),
                    pair,
                },
            ),
            |detail| {
                assert!(matches!(
                    detail,
                    SdmaOwnerDiagnosticV1::Native(
                        fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract(
                            "typed publication cause"
                        )
                    )
                ));
                std::panic::resume_unwind(payload)
            },
        )
    }))
    .unwrap_err()
    .downcast::<u64>()
    .unwrap();
    assert_eq!(&*caught as *const u64, address);
    assert_eq!(*caught, 619);
    assert_eq!(device_box(&backend, device), original);
    assert!(backend.terminal_sdma_custody.is_none());
    assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 0);
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
        2
    );
    super::sdma_host_write_tests::assert_terminal_retries_inert(&mut backend, host, device);
    discard_scripted_fixture(backend);
}
