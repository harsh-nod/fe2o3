use super::sdma_host_write_tests::{discard_scripted_fixture, fixture, host_observation, snapshot};
use super::*;
use crate::kfd_backend::kfd_backend_sdma_seam::{ScriptedTerminalCustodyV1, SdmaOwnerDiagnosticV1};
use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn extract_device(
    backend: &mut KfdRuntimeBackendV1,
    allocation: u64,
) -> Box<DirectionalSdmaDeviceOwnerV1> {
    let storage = &mut backend
        .allocations
        .get_mut(&allocation)
        .unwrap()
        .sdma_storage;
    let KfdRuntimeSdmaStorageV1::Device(device) = std::mem::replace(
        storage,
        KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous),
    ) else {
        panic!("fixture must start with the indexed original device");
    };
    device
}

fn device_observation(device: &DirectionalSdmaDeviceOwnerV1) -> (u64, Vec<u8>) {
    (
        device.scripted_owner_id().unwrap(),
        device.scripted_bytes().unwrap().to_vec(),
    )
}

fn retained_device(backend: &KfdRuntimeBackendV1) -> &DirectionalSdmaDeviceOwnerV1 {
    match &backend.terminal_sdma_custody {
        Some(KfdRuntimeTerminalSdmaCustodyV1::Device(device))
        | Some(KfdRuntimeTerminalSdmaCustodyV1::Scripted(ScriptedTerminalCustodyV1::Device(
            device,
        ))) => device,
        _ => panic!("runtime must retain exact device"),
    }
}

fn terminal_retries(backend: &mut KfdRuntimeBackendV1, device: u64) {
    assert!(backend.terminal);
    let indexed = |backend: &KfdRuntimeBackendV1| {
        backend
            .allocations
            .ordinary_iter()
            .map(|(&id, record)| {
                let owner = match &record.sdma_storage {
                    KfdRuntimeSdmaStorageV1::Device(owner) => Some(device_observation(owner)),
                    KfdRuntimeSdmaStorageV1::Host(owner) => {
                        let (id, bytes, _) = host_observation(owner);
                        Some((id, bytes))
                    }
                    KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous) => {
                        None
                    }
                    other => panic!("unexpected fixture storage: {other:?}"),
                };
                (
                    id,
                    record.kind,
                    owner,
                    Arc::clone(&record.bytes),
                    record.bytes.as_ptr() as usize,
                    record.content_sha256,
                    record.sdma_backed,
                    record.sdma_initialized,
                    record.sdma_shadow_dirty,
                )
            })
            .collect::<Vec<_>>()
    };
    let index_before = indexed(backend);
    let events = backend
        .profiler
        .as_ref()
        .unwrap()
        .recorded_events_for_test_v1()
        .to_vec();
    let before = (
        backend.staged_context_bytes,
        backend.allocations.len(),
        backend.next_handle,
        backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
        backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
    );
    let runtime = backend
        .terminal_sdma_custody
        .as_ref()
        .map(|_| device_observation(retained_device(backend)));
    let lower = backend
        .scripted_sdma
        .as_ref()
        .unwrap()
        .demotion_custody()
        .map(|(id, bytes)| (id, bytes.to_vec()));
    assert!(matches!(
        backend.release_allocation_v1(device),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(matches!(
        backend.write_allocation_v1(device, 0, &[1]),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(matches!(
        backend.allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8),
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
    assert_eq!(
        (
            backend.staged_context_bytes,
            backend.allocations.len(),
            backend.next_handle,
            backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
            backend.scripted_sdma.as_ref().unwrap().live_owner_count()
        ),
        before
    );
    assert_eq!(
        backend
            .terminal_sdma_custody
            .as_ref()
            .map(|_| device_observation(retained_device(backend))),
        runtime
    );
    assert_eq!(
        backend
            .scripted_sdma
            .as_ref()
            .unwrap()
            .demotion_custody()
            .map(|(id, bytes)| (id, bytes.to_vec())),
        lower
    );
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
        0
    );
    assert_eq!(indexed(backend), index_before);
    assert_eq!(
        backend
            .profiler
            .as_ref()
            .unwrap()
            .recorded_events_for_test_v1(),
        events
    );
}

#[test]
fn sdma_demotion_restore_shell_reuses_allocation_and_drops_exactly_once() {
    use std::cell::Cell;
    use std::rc::Rc;
    #[repr(align(128))]
    struct Tracked {
        drops: Rc<Cell<usize>>,
        bytes: Box<[u8]>,
    }
    impl Drop for Tracked {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }
    for mode in 0..4 {
        let drops = Rc::new(Cell::new(0));
        let boxed = Box::new(Tracked {
            drops: Rc::clone(&drops),
            bytes: Box::new([0x4b; 17]),
        });
        let address = &*boxed as *const Tracked;
        let bytes = boxed.bytes.as_ptr();
        let (value, shell) = take_restore_shell_v1(boxed);
        assert_eq!(drops.get(), 0);
        assert_eq!(
            (&*shell as *const MaybeUninit<Tracked>).cast::<Tracked>(),
            address
        );
        assert_eq!(value.bytes.as_ptr(), bytes);
        match mode {
            0 => {
                let boxed = fill_restore_shell_v1(shell, value);
                assert_eq!(&*boxed as *const Tracked, address);
                assert_eq!(boxed.bytes.as_ptr(), bytes);
                assert_eq!(&*boxed.bytes, &[0x4b; 17]);
                drop(boxed);
            }
            1 => {
                drop(shell);
                assert_eq!(drops.get(), 0);
                drop(value);
            }
            2 => {
                drop(value);
                assert_eq!(drops.get(), 1);
                drop(shell);
            }
            _ => {
                let root = Some(value);
                assert!(
                    catch_unwind(AssertUnwindSafe(move || {
                        let _shell = shell;
                        std::panic::panic_any("after owner rooted");
                    }))
                    .is_err()
                );
                assert_eq!(drops.get(), 0);
                assert_eq!(root.as_ref().unwrap().bytes.as_ptr(), bytes);
                drop(root);
            }
        }
        assert_eq!(drops.get(), 1);
    }
    thread_local! { static ZST_DROPS: Cell<usize> = const { Cell::new(0) }; }
    #[repr(align(256))]
    struct AlignedZst;
    impl Drop for AlignedZst {
        fn drop(&mut self) {
            ZST_DROPS.with(|n| n.set(n.get() + 1));
        }
    }
    for refill in [false, true] {
        ZST_DROPS.with(|n| n.set(0));
        let boxed = Box::new(AlignedZst);
        let address = &*boxed as *const AlignedZst;
        assert_eq!((address as usize) % 256, 0);
        let (value, shell) = take_restore_shell_v1(boxed);
        if refill {
            let boxed = fill_restore_shell_v1(shell, value);
            assert_eq!(&*boxed as *const AlignedZst, address);
            drop(boxed);
        } else {
            drop(shell);
            ZST_DROPS.with(|n| assert_eq!(n.get(), 0));
            drop(value);
        }
        ZST_DROPS.with(|n| assert_eq!(n.get(), 1));
    }
}

#[test]
fn sdma_demotion_retry_preserves_original_box_neighbor_and_refunds_once() {
    let (backend, _, host, device) = fixture(
        8,
        [
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Retryable),
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ],
    );
    let mut backend = ManuallyDrop::new(backend);
    backend.allocations.get_mut(&device).unwrap().sdma_backed = false;
    let before = snapshot(&backend, host, device);
    let KfdRuntimeSdmaStorageV1::Device(owner) = &backend.allocations[&device].sdma_storage else {
        unreachable!()
    };
    let box_address = &**owner as *const DirectionalSdmaDeviceOwnerV1;
    let Err(RuntimeBackendFailureV1::Quiescent(error)) = backend.release_allocation_v1(device)
    else {
        panic!("demotion rejection must remain quiescent")
    };
    assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Native);
    assert_eq!(
        error.detail(),
        "KFD persistent device demotion: scripted demotion retryable"
    );
    assert_eq!(snapshot(&backend, host, device), before);
    let KfdRuntimeSdmaStorageV1::Device(owner) = &backend.allocations[&device].sdma_storage else {
        unreachable!()
    };
    assert_eq!(&**owner as *const DirectionalSdmaDeviceOwnerV1, box_address);
    assert!(!backend.terminal && backend.terminal_sdma_custody.is_none());
    assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 2);
    backend.release_allocation_v1(device).unwrap();
    assert_eq!(
        (backend.allocations.len(), backend.staged_context_bytes),
        (1, 8)
    );
    assert!(!backend.allocations.contains_key(&device));
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
        1
    );
    assert!(backend.scripted_sdma.as_ref().unwrap().is_exhausted());
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
        0
    );
    discard_scripted_fixture(backend);
}

#[test]
fn sdma_demotion_rejection_after_real_scrub_preserves_shadow_and_retries_scrub() {
    let mut steps = scripted_sync_copy_steps_v1(
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        0,
        8,
        ScriptedFailureModeV1::Success,
    );
    steps.push(ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Retryable));
    steps.extend(scripted_sync_copy_steps_v1(
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        0,
        8,
        ScriptedFailureModeV1::Success,
    ));
    steps.extend([
        ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]);
    let (backend, _, _, device) = fixture(8, steps);
    let mut backend = ManuallyDrop::new(backend);
    let record = backend.allocations.get_mut(&device).unwrap();
    record.bytes = Arc::from([0x7a; 8]);
    let digest = Sha256::digest(&*record.bytes).into();
    record.content_sha256 = Some(digest);
    record.last_full_host_write = Some((Arc::clone(&record.bytes), digest));
    assert!(record.last_full_host_write.is_some());
    let shadow = Arc::clone(&record.bytes);
    let KfdRuntimeSdmaStorageV1::Device(owner) = &mut record.sdma_storage else {
        unreachable!()
    };
    owner.scripted_bytes_mut().unwrap().fill(0x7a);
    let owner_id = owner.scripted_owner_id().unwrap();
    assert!(matches!(
        backend.release_allocation_v1(device),
        Err(RuntimeBackendFailureV1::Quiescent(_))
    ));
    let record = &backend.allocations[&device];
    assert!(Arc::ptr_eq(&record.bytes, &shadow));
    assert_eq!(&*record.bytes, &[0x7a; 8]);
    assert!(
        record.sdma_shadow_dirty
            && record.content_sha256.is_none()
            && record.last_full_host_write.is_none()
    );
    let KfdRuntimeSdmaStorageV1::Device(owner) = &record.sdma_storage else {
        unreachable!()
    };
    assert_eq!(device_observation(owner), (owner_id, vec![0; 8]));
    assert_eq!(backend.staged_context_bytes, 16);
    backend.release_allocation_v1(device).unwrap();
    assert_eq!(
        (backend.allocations.len(), backend.staged_context_bytes),
        (1, 8)
    );
    assert!(backend.scripted_sdma.as_ref().unwrap().is_exhausted());
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
        0
    );
    discard_scripted_fixture(backend);
}

#[test]
fn sdma_demotion_terminal_and_lower_panic_retain_exact_owner_and_charge() {
    for panic in [false, true] {
        let step = if panic {
            ScriptedSdmaStepV1::DemotePanic
        } else {
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::ProcessTeardown)
        };
        let (backend, _, host, device) = fixture(
            8,
            [
                step,
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ],
        );
        let mut backend = ManuallyDrop::new(backend);
        backend.allocations.get_mut(&device).unwrap().sdma_backed = false;
        let KfdRuntimeSdmaStorageV1::Device(owner) = &backend.allocations[&device].sdma_storage
        else {
            unreachable!()
        };
        let before = device_observation(owner);
        let neighbor = Arc::clone(&backend.allocations[&host].bytes);
        let result = catch_unwind(AssertUnwindSafe(|| backend.release_allocation_v1(device)));
        if panic {
            assert_eq!(
                result.unwrap_err().downcast_ref::<&str>(),
                Some(&"scripted SDMA demotion panic")
            );
            let (id, bytes) = backend
                .scripted_sdma
                .as_ref()
                .unwrap()
                .demotion_custody()
                .unwrap();
            assert_eq!((id, bytes.to_vec()), before);
            assert!(backend.terminal_sdma_custody.is_none());
        } else {
            let Err(RuntimeBackendFailureV1::Terminal(error)) = result.unwrap() else {
                panic!("terminal demotion outcome")
            };
            assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Terminal);
            assert_eq!(
                error.detail(),
                "KFD persistent device demotion: scripted demotion teardown"
            );
            assert_eq!(device_observation(retained_device(&backend)), before);
        }
        assert!(matches!(
            backend.allocations[&device].sdma_storage,
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous)
        ));
        assert!(Arc::ptr_eq(&backend.allocations[&host].bytes, &neighbor));
        assert_eq!(backend.staged_context_bytes, 16);
        terminal_retries(&mut backend, device);
        discard_scripted_fixture(backend);
    }
}

#[test]
fn sdma_demotion_typed_diagnostic_panic_roots_both_failure_classes_before_format() {
    for terminal in [false, true] {
        let (backend, _, host, device) = fixture(8, []);
        let mut backend = ManuallyDrop::new(backend);
        let (owner, shell) = take_restore_shell_v1(extract_device(&mut backend, device));
        let before = device_observation(&owner);
        let neighbor = Arc::clone(&backend.allocations[&host].bytes);
        let detail = SdmaOwnerDiagnosticV1::Native(
            fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract("typed demotion cause"),
        );
        let failure = if terminal {
            SdmaTransitionFailureV1::ProcessTeardown {
                detail,
                custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Device(owner)),
            }
        } else {
            SdmaTransitionFailureV1::Retryable {
                detail,
                custody: owner,
            }
        };
        let original = Box::new(617_u64);
        let address = &*original as *const u64;
        let result = catch_unwind(AssertUnwindSafe(|| {
            backend.settle_sdma_demotion_failure_v1(device, shell, failure, |detail| {
                assert!(matches!(
                    detail,
                    SdmaOwnerDiagnosticV1::Native(
                        fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract("typed demotion cause")
                    )
                ));
                std::panic::resume_unwind(original)
            })
        }));
        let payload = result.unwrap_err().downcast::<u64>().unwrap();
        assert_eq!(&*payload as *const u64, address);
        assert_eq!(device_observation(retained_device(&backend)), before);
        assert!(Arc::ptr_eq(&backend.allocations[&host].bytes, &neighbor));
        assert_eq!(backend.staged_context_bytes, 16);
        terminal_retries(&mut backend, device);
        discard_scripted_fixture(backend);
    }
}

#[test]
fn sdma_demotion_restoration_rejects_missing_wrong_kind_and_occupied_slots() {
    for case in 0..3 {
        let (backend, _, host, device) = fixture(8, []);
        let mut backend = ManuallyDrop::new(backend);
        let (owner, shell) = take_restore_shell_v1(extract_device(&mut backend, device));
        let before = device_observation(&owner);
        let replacement = if case == 2 {
            let owner = backend.scripted_sdma.as_ref().unwrap().test_device_owner(8);
            let observation = device_observation(&owner);
            backend.allocations.get_mut(&device).unwrap().sdma_storage =
                KfdRuntimeSdmaStorageV1::Device(Box::new(owner));
            Some(observation)
        } else {
            None
        };
        let target = match case {
            0 => u64::MAX,
            1 => {
                backend.allocations.get_mut(&device).unwrap().kind =
                    RuntimeMemoryKindV1::HostVisible;
                device
            }
            _ => device,
        };
        let KfdRuntimeSdmaStorageV1::Host(host_owner) = &backend.allocations[&host].sdma_storage
        else {
            unreachable!()
        };
        let host_before = host_observation(host_owner);
        let failure = SdmaTransitionFailureV1::Retryable {
            detail: SdmaOwnerDiagnosticV1::Native(
                fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract("retry"),
            ),
            custody: owner,
        };
        assert!(matches!(
            backend.settle_sdma_demotion_failure_v1(target, shell, failure, |d| d.to_string()),
            RuntimeBackendFailureV1::Terminal(_)
        ));
        assert_eq!(device_observation(retained_device(&backend)), before);
        let KfdRuntimeSdmaStorageV1::Host(host_owner) = &backend.allocations[&host].sdma_storage
        else {
            unreachable!()
        };
        assert_eq!(host_observation(host_owner), host_before);
        if let Some(replacement) = replacement {
            let KfdRuntimeSdmaStorageV1::Device(owner) = &backend.allocations[&device].sdma_storage
            else {
                unreachable!()
            };
            assert_eq!(device_observation(owner), replacement);
        } else {
            assert!(matches!(
                backend.allocations[&device].sdma_storage,
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous)
            ));
        }
        terminal_retries(&mut backend, device);
        discard_scripted_fixture(backend);
    }
}

#[test]
fn sdma_demotion_driver_selection_panic_keeps_original_device() {
    let (backend, _, _, device) = fixture(8, []);
    let mut backend = ManuallyDrop::new(backend);
    let owner = extract_device(&mut backend, device);
    let before = device_observation(&owner);
    let driver = backend.scripted_sdma.take();
    assert!(
        catch_unwind(AssertUnwindSafe(
            || backend.demote_sdma_device_v1(device, owner)
        ))
        .is_err()
    );
    assert!(backend.terminal);
    assert_eq!(device_observation(retained_device(&backend)), before);
    assert_eq!(driver.as_ref().unwrap().unexpected_drops(), 0);
    backend.scripted_sdma = driver;
    terminal_retries(&mut backend, device);
    discard_scripted_fixture(backend);
}

#[test]
fn sdma_demotion_public_context_preserves_retry_credits_and_quarantines_terminal_failure() {
    for configured in [false, true] {
        for mode in 0..3 {
            let mut steps = vec![
                ScriptedSdmaStepV1::Allocate {
                    kind: ScriptedBufferKindV1::Device,
                    byte_len: 8,
                },
                ScriptedSdmaStepV1::Promote(ScriptedFailureModeV1::Success),
            ];
            steps.extend(scripted_sync_copy_steps_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                8,
                ScriptedFailureModeV1::Success,
            ));
            steps.extend(scripted_sync_copy_steps_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                8,
                ScriptedFailureModeV1::Success,
            ));
            steps.push(match mode {
                0 => ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Retryable),
                1 => ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::ProcessTeardown),
                _ => ScriptedSdmaStepV1::DemotePanic,
            });
            let mut retry = scripted_sync_copy_steps_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                8,
                ScriptedFailureModeV1::Success,
            );
            retry.extend([
                ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ]);
            let retry_steps = retry.len();
            steps.extend(retry);
            let mut backend = KfdRuntimeBackendV1::mock();
            backend.native_available = true;
            backend.scripted_sdma = Some(ScriptedSdmaDriverV1::new(steps));
            let mut context = ManuallyDrop::new(crate::RuntimeContextV1::open(backend).unwrap());
            let device = context.devices()[0].id();
            if configured {
                context
                    .configure_allocation_admission_v1(device, 8, 1)
                    .unwrap();
            }
            let allocation = context
                .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
                .unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| context.release_allocation(allocation)));
            match mode {
                0 => assert!(matches!(
                    result.unwrap(),
                    Err(crate::RuntimeErrorV1::BackendQuiescent(_))
                )),
                1 => assert!(matches!(
                    result.unwrap(),
                    Err(crate::RuntimeErrorV1::BackendTerminal(_))
                )),
                _ => assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"scripted SDMA demotion panic")
                ),
            }
            if configured {
                let usage = context
                    .allocation_admission_usage_v1(device)
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    usage
                        .used
                        .get(crate::RuntimeResourceKindV1::RequestedAllocationBytes),
                    8
                );
                assert_eq!(
                    usage
                        .used
                        .get(crate::RuntimeResourceKindV1::AllocationRecords),
                    1
                );
                assert_eq!(usage.quarantined_records, usize::from(mode != 0));
            }
            if mode == 0 {
                context.release_allocation(allocation).unwrap();
            } else {
                assert!(context.release_allocation(allocation).is_err());
                assert!(context.is_terminal());
            }
            if configured {
                let usage = context
                    .allocation_admission_usage_v1(device)
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    usage
                        .used
                        .get(crate::RuntimeResourceKindV1::RequestedAllocationBytes),
                    if mode == 0 { 0 } else { 8 }
                );
                assert_eq!(
                    usage
                        .used
                        .get(crate::RuntimeResourceKindV1::AllocationRecords),
                    u64::from(mode != 0)
                );
                assert_eq!(usage.quarantined_records, usize::from(mode != 0));
            }
            let backend = context.backend_mut_for_test_v1();
            let driver = backend.scripted_sdma.as_ref().unwrap();
            assert_eq!(
                driver.remaining_steps(),
                if mode == 0 { 0 } else { retry_steps }
            );
            assert_eq!(driver.live_owner_count(), usize::from(mode != 0));
            assert_eq!(driver.unexpected_drops(), 0);
            disarm_scripted_drop_after_inspection_v1(backend);
            drop(ManuallyDrop::into_inner(context));
        }
    }
}

#[test]
fn sdma_demotion_terminal_drop_aborts_with_runtime_and_lower_roots() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_TEST_RUNTIME_DEMOTION_DROP";
    const TEST: &str = "kfd_backend::tests::sdma_demotion_tests::sdma_demotion_terminal_drop_aborts_with_runtime_and_lower_roots";
    if let Some(mode) = std::env::var_os(CHILD) {
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        let step = if mode == "runtime" {
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::ProcessTeardown)
        } else {
            ScriptedSdmaStepV1::DemotePanic
        };
        let (mut backend, _, _, device) = fixture(8, [step]);
        backend.allocations.get_mut(&device).unwrap().sdma_backed = false;
        let result = catch_unwind(AssertUnwindSafe(|| backend.release_allocation_v1(device)));
        if mode == "runtime" {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
        } else {
            assert!(result.is_err());
        }
        assert!(backend.terminal);
        assert_eq!(
            backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
            2
        );
        assert_eq!(
            backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
            0
        );
        eprintln!("demotion retained; dropping terminal backend");
        drop(backend);
        panic!("terminal demotion Drop returned");
    }
    for mode in ["runtime", "lower"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, mode)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "{stderr}");
        assert!(stderr.contains("demotion retained; dropping terminal backend"));
        assert!(!stderr.contains("terminal demotion Drop returned"));
    }
}
