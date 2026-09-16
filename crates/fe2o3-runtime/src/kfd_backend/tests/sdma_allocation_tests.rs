use super::sdma_host_write_tests::{
    assert_terminal_retries_inert, discard_scripted_fixture, fixture, host_observation, snapshot,
};
use super::*;
use crate::kfd_backend::kfd_backend_sdma_seam::{SdmaAllocationFailureV1, SdmaOwnerDiagnosticV1};
use fe2o3_kfd::{Gfx942SdmaAllocationDispositionV1, Gfx942SdmaErrorV1, MemorySessionError};
use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};

pub(super) fn capacity(kind: RuntimeMemoryKindV1, terminal: bool) -> SdmaAllocationFailureV1 {
    let credit = fe2o3_resource_accounting::ResourceCreditErrorV1::Capacity;
    let memory = match kind {
        RuntimeMemoryKindV1::HostVisible => MemorySessionError::HostVisibleBackingCredits(credit),
        RuntimeMemoryKindV1::DeviceLocal => MemorySessionError::DeviceBackingCredits(credit),
    };
    SdmaAllocationFailureV1 {
        detail: SdmaOwnerDiagnosticV1::Native(fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Sdma(
            Gfx942SdmaErrorV1::Memory(memory),
        )),
        disposition: if terminal {
            Gfx942SdmaAllocationDispositionV1::ProcessTeardown
        } else {
            Gfx942SdmaAllocationDispositionV1::RetryableCapacity
        },
    }
}

fn scripted_kind(kind: RuntimeMemoryKindV1) -> ScriptedBufferKindV1 {
    match kind {
        RuntimeMemoryKindV1::HostVisible => ScriptedBufferKindV1::Host,
        RuntimeMemoryKindV1::DeviceLocal => ScriptedBufferKindV1::Device,
    }
}

pub(super) fn reject(
    kind: RuntimeMemoryKindV1,
    byte_len: usize,
    terminal: bool,
) -> ScriptedSdmaStepV1 {
    ScriptedSdmaStepV1::AllocateFailure {
        kind: scripted_kind(kind),
        byte_len,
        failure: capacity(kind, terminal),
    }
}

pub(super) fn success(kind: RuntimeMemoryKindV1) -> Vec<ScriptedSdmaStepV1> {
    let mut steps = vec![ScriptedSdmaStepV1::Allocate {
        kind: scripted_kind(kind),
        byte_len: 8,
    }];
    match kind {
        RuntimeMemoryKindV1::HostVisible => steps.push(ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len: 8,
        }),
        RuntimeMemoryKindV1::DeviceLocal => {
            steps.push(ScriptedSdmaStepV1::Promote(ScriptedFailureModeV1::Success));
            steps.extend(scripted_sync_copy_steps_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                8,
                ScriptedFailureModeV1::Success,
            ));
        }
    }
    steps
}

#[test]
fn sdma_allocation_capacity_preserves_neighbors_and_retries_after_warm_or_cold_admission() {
    for kind in [
        RuntimeMemoryKindV1::HostVisible,
        RuntimeMemoryKindV1::DeviceLocal,
    ] {
        for warm in [false, true] {
            let expected_detail = format!(
                "KFD persistent SDMA allocation: {}",
                capacity(kind, false).detail
            );
            let mut steps = vec![reject(kind, 8, false)];
            steps.extend(success(kind));
            let (backend, _, host, device) = fixture(8, steps);
            let mut backend = ManuallyDrop::new(backend);
            // Cold is a scripted provenance model, not native queue-creation evidence.
            backend.sdma_enabled = warm;
            let candidate = backend.next_handle;
            let mut before = snapshot(&backend, host, device);
            before.next_handle += 1;
            let error = match backend.allocate_v1(7, kind, 8, 8).unwrap_err() {
                RuntimeBackendFailureV1::Rejected(error) if warm => error,
                RuntimeBackendFailureV1::Quiescent(error) if !warm => error,
                _ => panic!("wrong capacity disposition"),
            };
            assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Capacity);
            assert_eq!(error.detail(), expected_detail);
            assert_eq!(snapshot(&backend, host, device), before);
            assert!(!backend.terminal && backend.sdma_enabled);
            assert!(!backend.allocations.contains_key(&candidate));
            assert!(backend.terminal_sdma_custody.is_none());
            assert_eq!(
                backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
                2
            );
            let allocated = backend.allocate_v1(7, kind, 8, 8).unwrap();
            assert_eq!(allocated, candidate + 1);
            assert!(backend.allocations[&allocated].sdma_initialized);
            assert_eq!(&*backend.allocations[&allocated].bytes, &[0; 8]);
            assert_eq!(backend.staged_context_bytes, 24);
            let driver = backend.scripted_sdma.as_ref().unwrap();
            assert_eq!(driver.remaining_steps(), 0);
            assert_eq!(driver.live_owner_count(), 3);
            assert_eq!(driver.unexpected_drops(), 0);
            discard_scripted_fixture(backend);
        }
    }
}

#[test]
fn sdma_allocation_terminal_capacity_protocol_error_and_panic_seal_without_new_owner() {
    for kind in [
        RuntimeMemoryKindV1::HostVisible,
        RuntimeMemoryKindV1::DeviceLocal,
    ] {
        for mode in 0..3 {
            let step = match mode {
                0 => reject(kind, 8, true),
                1 => ScriptedSdmaStepV1::Read {
                    offset: 0,
                    byte_len: 8,
                },
                _ => ScriptedSdmaStepV1::AllocatePanic {
                    kind: scripted_kind(kind),
                    byte_len: 8,
                },
            };
            let (backend, _, host, device) = fixture(
                8,
                [
                    step,
                    ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
                ],
            );
            let mut backend = ManuallyDrop::new(backend);
            let mut before = snapshot(&backend, host, device);
            before.next_handle += 1;
            let result = catch_unwind(AssertUnwindSafe(|| backend.allocate_v1(7, kind, 8, 8)));
            if mode == 2 {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"scripted SDMA allocation panic")
                );
            } else {
                let Err(RuntimeBackendFailureV1::Terminal(error)) = result.unwrap() else {
                    panic!()
                };
                assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Terminal);
                if mode == 0 {
                    assert_eq!(
                        error.detail(),
                        format!(
                            "KFD persistent SDMA allocation: {}",
                            capacity(kind, true).detail
                        )
                    );
                } else {
                    assert!(error.detail().starts_with(
                        "KFD persistent SDMA allocation: scripted SDMA allocation mismatch:"
                    ));
                }
            }
            assert_eq!(snapshot(&backend, host, device), before);
            assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 1);
            assert_eq!(
                backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
                2
            );
            assert_terminal_retries_inert(&mut backend, host, device);
            discard_scripted_fixture(backend);
        }
    }
}

#[test]
fn sdma_allocation_diagnostic_panic_preserves_original_payload_and_all_prior_owners() {
    for terminal in [false, true] {
        for warm in [false, true] {
            let (backend, _, host, device) = fixture(8, []);
            let mut backend = ManuallyDrop::new(backend);
            let before = snapshot(&backend, host, device);
            let payload = Box::new(981_u64);
            let address = &*payload as *const u64;
            let caught = catch_unwind(AssertUnwindSafe(|| {
                backend.settle_sdma_allocation_failure_v1(
                    capacity(RuntimeMemoryKindV1::HostVisible, terminal),
                    warm,
                    "KFD persistent SDMA allocation",
                    |detail| {
                        assert!(matches!(detail, SdmaOwnerDiagnosticV1::Native(_)));
                        std::panic::resume_unwind(payload)
                    },
                )
            }))
            .unwrap_err()
            .downcast::<u64>()
            .unwrap();
            assert_eq!(&*caught as *const u64, address);
            assert_eq!(*caught, 981);
            assert_eq!(snapshot(&backend, host, device), before);
            assert_terminal_retries_inert(&mut backend, host, device);
            discard_scripted_fixture(backend);
        }
    }
}

#[test]
fn sdma_allocation_context_refunds_only_warm_capacity_and_preserves_cold_quarantine() {
    for configured in [false, true] {
        for warm in [false, true] {
            for kind in [
                RuntimeMemoryKindV1::HostVisible,
                RuntimeMemoryKindV1::DeviceLocal,
            ] {
                let mut steps = vec![reject(kind, 8, false)];
                steps.extend(success(kind));
                let mut backend = KfdRuntimeBackendV1::mock();
                backend.native_available = true;
                backend.sdma_enabled = warm;
                backend.scripted_sdma = Some(ScriptedSdmaDriverV1::new(steps));
                let mut context =
                    ManuallyDrop::new(crate::RuntimeContextV1::open(backend).unwrap());
                let device = context.devices()[0].id();
                if configured {
                    context
                        .configure_allocation_admission_v1(device, 8, 1)
                        .unwrap();
                }
                match context.allocate(device, kind, 8, 8).unwrap_err() {
                    crate::RuntimeErrorV1::BackendRejected(error) if warm => {
                        assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Capacity)
                    }
                    crate::RuntimeErrorV1::BackendQuiescent(error) if !warm => {
                        assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Capacity)
                    }
                    _ => panic!("wrong Context disposition"),
                }
                assert!(!context.is_terminal());
                assert!(context.backend_mut_for_test_v1().allocations.is_empty());
                if configured {
                    let usage = context
                        .allocation_admission_usage_v1(device)
                        .unwrap()
                        .unwrap();
                    assert_eq!(
                        usage
                            .used
                            .get(crate::RuntimeResourceKindV1::RequestedAllocationBytes),
                        if warm { 0 } else { 8 }
                    );
                    assert_eq!(
                        usage
                            .used
                            .get(crate::RuntimeResourceKindV1::AllocationRecords),
                        u64::from(!warm)
                    );
                    assert_eq!(usage.quarantined_records, usize::from(!warm));
                }
                let steps = context
                    .backend_mut_for_test_v1()
                    .scripted_sdma
                    .as_ref()
                    .unwrap()
                    .remaining_steps();
                if configured && !warm {
                    let usage = context.allocation_admission_usage_v1(device).unwrap();
                    assert!(matches!(
                        context.allocate(device, kind, 8, 8),
                        Err(crate::RuntimeErrorV1::Validation(
                            crate::RuntimeValidationErrorV1::Capacity
                        ))
                    ));
                    assert_eq!(
                        context.allocation_admission_usage_v1(device).unwrap(),
                        usage
                    );
                    assert_eq!(
                        context
                            .backend_mut_for_test_v1()
                            .scripted_sdma
                            .as_ref()
                            .unwrap()
                            .remaining_steps(),
                        steps
                    );
                    assert_eq!(context.cleanup().allocation_credit_records_v1(), 1);
                } else {
                    context.allocate(device, kind, 8, 8).unwrap();
                    assert_eq!(
                        context
                            .backend_mut_for_test_v1()
                            .scripted_sdma
                            .as_ref()
                            .unwrap()
                            .remaining_steps(),
                        0
                    );
                }
                let backend = context.backend_mut_for_test_v1();
                assert!(!backend.terminal);
                assert_eq!(
                    backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
                    0
                );
                disarm_scripted_drop_after_inspection_v1(backend);
                drop(ManuallyDrop::into_inner(context));
            }
        }
    }
}

#[test]
fn sdma_allocation_context_terminal_error_and_panic_keep_prior_owner_and_credits() {
    for configured in [false, true] {
        for panic in [false, true] {
            for kind in [
                RuntimeMemoryKindV1::HostVisible,
                RuntimeMemoryKindV1::DeviceLocal,
            ] {
                let mut steps = success(RuntimeMemoryKindV1::HostVisible);
                steps.extend([
                    if panic {
                        ScriptedSdmaStepV1::AllocatePanic {
                            kind: scripted_kind(kind),
                            byte_len: 8,
                        }
                    } else {
                        reject(kind, 8, true)
                    },
                    ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
                ]);
                let mut backend = KfdRuntimeBackendV1::mock();
                backend.native_available = true;
                backend.scripted_sdma = Some(ScriptedSdmaDriverV1::new(steps));
                let mut context =
                    ManuallyDrop::new(crate::RuntimeContextV1::open(backend).unwrap());
                let device = context.devices()[0].id();
                if configured {
                    context
                        .configure_allocation_admission_v1(device, 16, 2)
                        .unwrap();
                }
                let allocation = context
                    .allocate(device, RuntimeMemoryKindV1::HostVisible, 8, 8)
                    .unwrap();
                let backend = context.backend_mut_for_test_v1();
                let host = *backend.allocations.keys().next().unwrap();
                let observe = |backend: &KfdRuntimeBackendV1| {
                    let record = &backend.allocations[&host];
                    let KfdRuntimeSdmaStorageV1::Host(owner) = &record.sdma_storage else {
                        panic!("prior Context allocation must retain its original host owner");
                    };
                    (
                        host_observation(owner),
                        Arc::as_ptr(&record.bytes) as *const u8 as usize,
                        record.bytes.to_vec(),
                        record.content_sha256,
                        backend.allocations.len(),
                        backend.staged_context_bytes,
                    )
                };
                let before = observe(backend);
                let candidate = backend.next_handle;
                let result =
                    catch_unwind(AssertUnwindSafe(|| context.allocate(device, kind, 8, 8)));
                if panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<&str>(),
                        Some(&"scripted SDMA allocation panic")
                    );
                    assert_eq!(context.is_terminal(), configured);
                } else {
                    let Err(crate::RuntimeErrorV1::BackendTerminal(error)) = result.unwrap() else {
                        panic!("terminal lower disposition must reach Context");
                    };
                    assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Terminal);
                    assert_eq!(
                        error.detail(),
                        format!(
                            "KFD persistent SDMA allocation: {}",
                            capacity(kind, true).detail
                        )
                    );
                    assert!(context.is_terminal());
                }
                let usage = context.allocation_admission_usage_v1(device).unwrap();
                if configured {
                    let usage = usage.as_ref().unwrap();
                    assert_eq!(
                        usage
                            .used
                            .get(crate::RuntimeResourceKindV1::RequestedAllocationBytes),
                        16
                    );
                    assert_eq!(
                        usage
                            .used
                            .get(crate::RuntimeResourceKindV1::AllocationRecords),
                        2
                    );
                    assert_eq!(usage.quarantined_records, 1);
                } else {
                    assert!(usage.is_none());
                }
                assert!(context.allocate(device, kind, 8, 8).is_err());
                assert!(context.is_terminal());
                assert!(context.release_allocation(allocation).is_err());
                assert_eq!(
                    context.allocation_admission_usage_v1(device).unwrap(),
                    usage
                );
                let backend = context.backend_mut_for_test_v1();
                assert!(backend.terminal && backend.terminal_sdma_custody.is_none());
                assert_eq!(backend.next_handle, candidate + 1);
                assert_eq!(observe(backend), before);
                let driver = backend.scripted_sdma.as_ref().unwrap();
                assert_eq!(driver.remaining_steps(), 1);
                assert_eq!(driver.live_owner_count(), 1);
                assert_eq!(driver.unexpected_drops(), 0);
                disarm_scripted_drop_after_inspection_v1(backend);
                drop(ManuallyDrop::into_inner(context));
            }
        }
    }
}

#[test]
fn sdma_allocation_driver_selection_panic_keeps_neighbors_and_seals() {
    for kind in [
        RuntimeMemoryKindV1::HostVisible,
        RuntimeMemoryKindV1::DeviceLocal,
    ] {
        let (backend, _, host, device) = fixture(8, []);
        let mut backend = ManuallyDrop::new(backend);
        let before = snapshot(&backend, host, device);
        let driver = backend.scripted_sdma.take().unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            backend.allocate_sdma_owner_v1(kind, 8, 8, true, "allocation selection")
        }));
        let payload = result.unwrap_err();
        assert_eq!(
            payload.downcast_ref::<String>().map(String::as_str),
            Some("native directional SDMA ownership retains its queue")
        );
        backend.scripted_sdma = Some(driver);
        assert_eq!(snapshot(&backend, host, device), before);
        assert!(backend.terminal_sdma_custody.is_none());
        assert_terminal_retries_inert(&mut backend, host, device);
        discard_scripted_fixture(backend);
    }
}

#[test]
fn sdma_allocation_zero_initialization_capacity_is_quiescent_only_after_hidden_cleanup() {
    for cleanup in [
        ScriptedRecycleOutcomeV1::Success,
        ScriptedRecycleOutcomeV1::Recovered,
        ScriptedRecycleOutcomeV1::Ambiguous,
        ScriptedRecycleOutcomeV1::Panic,
    ] {
        let (backend, _, host, device) = fixture(
            8,
            [
                ScriptedSdmaStepV1::Allocate {
                    kind: ScriptedBufferKindV1::Device,
                    byte_len: 8,
                },
                ScriptedSdmaStepV1::Promote(ScriptedFailureModeV1::Success),
                reject(RuntimeMemoryKindV1::HostVisible, 8, false),
                ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
                ScriptedSdmaStepV1::Recycle(cleanup),
            ],
        );
        let mut backend = ManuallyDrop::new(backend);
        let hidden = backend.next_handle;
        let mut before = snapshot(&backend, host, device);
        let hidden_owner = before.device.0 + 1;
        before.next_handle += 1;
        let result = catch_unwind(AssertUnwindSafe(|| {
            backend.allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
        }));
        if cleanup == ScriptedRecycleOutcomeV1::Success {
            let Err(RuntimeBackendFailureV1::Quiescent(error)) = result.unwrap() else {
                panic!()
            };
            assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Capacity);
            assert_eq!(
                error.detail(),
                format!(
                    "KFD upload staging: {}",
                    capacity(RuntimeMemoryKindV1::HostVisible, false).detail
                )
            );
            assert_eq!(snapshot(&backend, host, device), before);
            assert!(!backend.allocations.contains_key(&hidden) && !backend.terminal);
            assert_eq!(
                backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
                2
            );
        } else {
            if cleanup == ScriptedRecycleOutcomeV1::Panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"scripted recycle panic")
                );
            } else {
                let Err(RuntimeBackendFailureV1::Terminal(error)) = result.unwrap() else {
                    panic!("hidden cleanup failure must be terminal");
                };
                assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Terminal);
                assert_eq!(
                    error.detail(),
                    if cleanup == ScriptedRecycleOutcomeV1::Recovered {
                        "hidden KFD allocation cleanup retained unreachable native custody"
                    } else {
                        "KFD persistent allocation recycle became ambiguous: scripted recycle ambiguous"
                    }
                );
            }
            before.staged_bytes += 8;
            before.allocations += 1;
            assert_eq!(snapshot(&backend, host, device), before);
            let hidden_record = &backend.allocations[&hidden];
            assert_eq!(hidden_record.kind, RuntimeMemoryKindV1::DeviceLocal);
            assert_eq!(&*hidden_record.bytes, &[0; 8]);
            assert!(hidden_record.sdma_backed && !hidden_record.sdma_initialized);
            assert!(backend.terminal_sdma_custody.is_none());
            let driver = backend.scripted_sdma.as_ref().unwrap();
            if cleanup == ScriptedRecycleOutcomeV1::Recovered {
                let KfdRuntimeSdmaStorageV1::DemotedDevice(buffer) = &hidden_record.sdma_storage
                else {
                    panic!("recovered hidden owner must remain indexed");
                };
                assert_eq!(host_observation(buffer), (hidden_owner, vec![0; 8], None));
                assert!(driver.recycle_custody().is_none());
            } else {
                assert!(matches!(
                    hidden_record.sdma_storage,
                    KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous)
                ));
                assert_eq!(
                    driver.recycle_custody().unwrap().observation(),
                    (hidden_owner, &[0; 8][..], None)
                );
            }
            assert_eq!(
                backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
                3
            );
            assert_terminal_retries_inert(&mut backend, host, device);
        }
        assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 0);
        assert_eq!(
            backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
            0
        );
        discard_scripted_fixture(backend);
    }
}

#[test]
fn sdma_allocation_staging_capacity_preserves_first_and_second_chunk_visibility() {
    let chunk = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize;
    for upload in [false, true] {
        for second in [false, true] {
            let direction = if upload {
                Gfx942PersistentSdmaDirectionV1::HostToDevice
            } else {
                Gfx942PersistentSdmaDirectionV1::DeviceToHost
            };
            let mut steps = if second {
                scripted_sync_copy_steps_v1(
                    direction,
                    0,
                    chunk as u32,
                    ScriptedFailureModeV1::Success,
                )
            } else {
                Vec::new()
            };
            steps.push(reject(
                RuntimeMemoryKindV1::HostVisible,
                if second { 8 } else { chunk },
                false,
            ));
            let (backend, _, host, device) = fixture(chunk + 8, steps);
            let mut backend = ManuallyDrop::new(backend);
            let KfdRuntimeSdmaStorageV1::Device(owner) =
                &mut backend.allocations.get_mut(&device).unwrap().sdma_storage
            else {
                panic!()
            };
            owner.scripted_bytes_mut().unwrap().fill(0x36);
            let mut before = snapshot(&backend, host, device);
            let events = backend
                .profiler
                .as_ref()
                .unwrap()
                .recorded_events_for_test_v1()
                .to_vec();
            let mut destination = vec![0xed; chunk + 8];
            let result = if upload {
                backend.write_allocation_v1(device, 0, &vec![0x79; chunk + 8])
            } else {
                backend.read_allocation_v1(device, 0, &mut destination)
            };
            let Err(RuntimeBackendFailureV1::Quiescent(error)) = result else {
                panic!("public prior-effect boundary")
            };
            assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Capacity);
            assert!(!backend.terminal);
            let record = &backend.allocations[&device];
            let KfdRuntimeSdmaStorageV1::Device(owner) = &record.sdma_storage else {
                panic!()
            };
            assert_eq!(&*record.bytes, &vec![0; chunk + 8]);
            assert!(record.content_sha256.is_none() && record.last_full_host_write.is_none());
            assert_eq!(record.sdma_shadow_dirty, upload && second);
            if upload && second {
                assert!(
                    owner.scripted_bytes().unwrap()[..chunk]
                        .iter()
                        .all(|&b| b == 0x79)
                );
                assert_eq!(&owner.scripted_bytes().unwrap()[chunk..], &[0x36; 8]);
                before.device.1[..chunk].fill(0x79);
            } else {
                assert!(owner.scripted_bytes().unwrap().iter().all(|&b| b == 0x36));
            }
            assert_eq!(snapshot(&backend, host, device), before);
            if !upload && second {
                assert!(destination[..chunk].iter().all(|&b| b == 0x36));
                assert_eq!(&destination[chunk..], &[0xed; 8]);
            } else {
                assert!(destination.iter().all(|&b| b == 0xed));
            }
            assert_eq!(
                backend
                    .profiler
                    .as_ref()
                    .unwrap()
                    .recorded_events_for_test_v1(),
                events
            );
            let driver = backend.scripted_sdma.as_ref().unwrap();
            assert_eq!(driver.remaining_steps(), 0);
            assert_eq!(driver.live_owner_count(), 2);
            assert_eq!(driver.unexpected_drops(), 0);
            discard_scripted_fixture(backend);
        }
    }
}
