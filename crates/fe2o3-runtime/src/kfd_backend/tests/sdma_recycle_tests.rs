use super::sdma_host_write_tests::{
    discard_scripted_fixture, fixture, host_observation, retained_host, snapshot,
};
use super::*;
use crate::kfd_backend::kfd_backend_sdma_seam::{ScriptedTerminalCustodyV1, SdmaOwnerDiagnosticV1};
use crate::kfd_backend::sdma_recycle::SdmaRecycleTargetV1 as Target;
use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(Debug, PartialEq)]
enum StorageObservation {
    Host(u64, Vec<u8>, Option<[u8; 32]>),
    Device(u64, Vec<u8>),
    InFlight(KfdRuntimeSdmaInFlightV1),
}

#[derive(Debug, PartialEq)]
struct AllocationObservation {
    handle: u64,
    kind: RuntimeMemoryKindV1,
    storage: StorageObservation,
    shadow: Arc<[u8]>,
    shadow_address: usize,
    digest: Option<[u8; 32]>,
    backed: bool,
    initialized: bool,
    dirty: bool,
}

fn allocation_observations(backend: &KfdRuntimeBackendV1) -> Vec<AllocationObservation> {
    backend
        .allocations
        .ordinary_iter()
        .map(|(&handle, record)| {
            let storage = match &record.sdma_storage {
                KfdRuntimeSdmaStorageV1::Host(buffer) => {
                    let (id, bytes, certificate) = host_observation(buffer);
                    StorageObservation::Host(id, bytes, certificate)
                }
                KfdRuntimeSdmaStorageV1::Device(buffer) => StorageObservation::Device(
                    buffer.scripted_owner_id().unwrap(),
                    buffer.scripted_bytes().unwrap().to_vec(),
                ),
                KfdRuntimeSdmaStorageV1::InFlight(state) => StorageObservation::InFlight(*state),
                other => panic!("unexpected fixture storage: {other:?}"),
            };
            AllocationObservation {
                handle,
                kind: record.kind,
                storage,
                shadow: Arc::clone(&record.bytes),
                shadow_address: Arc::as_ptr(&record.bytes) as *const u8 as usize,
                digest: record.content_sha256,
                backed: record.sdma_backed,
                initialized: record.sdma_initialized,
                dirty: record.sdma_shadow_dirty,
            }
        })
        .collect()
}

fn extract_host(backend: &mut KfdRuntimeBackendV1, host: u64) -> SdmaBufferOwnerV1 {
    let storage = &mut backend.allocations.get_mut(&host).unwrap().sdma_storage;
    let KfdRuntimeSdmaStorageV1::Host(buffer) = std::mem::replace(
        storage,
        KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous),
    ) else {
        panic!("fixture must begin with an indexed host owner");
    };
    buffer
}

fn terminal_retries(backend: &mut KfdRuntimeBackendV1, host: u64) {
    let owners = allocation_observations(backend);
    let retained = backend
        .terminal_sdma_custody
        .as_ref()
        .map(|_| host_observation(retained_host(backend)));
    let lower_observation = |backend: &KfdRuntimeBackendV1| {
        backend
            .scripted_sdma
            .as_ref()
            .unwrap()
            .recycle_custody()
            .map(|buffer| {
                let (id, bytes, certificate) = buffer.observation();
                (id, bytes.to_vec(), certificate)
            })
    };
    let lower = lower_observation(backend);
    let before = (
        backend.allocations.len(),
        backend.staged_context_bytes,
        backend.next_handle,
        backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
        backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
    );
    assert!(backend.terminal);
    assert!(matches!(
        backend.release_allocation_v1(host),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(matches!(
        backend.write_allocation_v1(host, 0, &[1]),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(matches!(
        backend.allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8),
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
            backend.allocations.len(),
            backend.staged_context_bytes,
            backend.next_handle,
            backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
            backend.scripted_sdma.as_ref().unwrap().live_owner_count()
        ),
        before
    );
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
        0
    );
    assert_eq!(allocation_observations(backend), owners);
    assert_eq!(
        backend
            .terminal_sdma_custody
            .as_ref()
            .map(|_| host_observation(retained_host(backend))),
        retained
    );
    assert_eq!(lower_observation(backend), lower);
}

#[test]
fn sdma_recycle_indexed_host_recovery_preserves_certificate_and_refunds_only_on_retry() {
    let (backend, _, host, device) = fixture(
        8,
        [
            ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len: 8,
            },
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Recovered),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ],
    );
    let mut backend = ManuallyDrop::new(backend);
    backend.write_allocation_v1(host, 0, &[0x39; 8]).unwrap();
    let KfdRuntimeSdmaStorageV1::Host(buffer) = &backend.allocations[&host].sdma_storage else {
        unreachable!()
    };
    assert_eq!(
        host_observation(buffer).2,
        Some(Sha256::digest([0x39; 8]).into())
    );
    let before = snapshot(&backend, host, device);
    let result = backend.release_allocation_v1(host);
    let Err(RuntimeBackendFailureV1::Quiescent(error)) = result else {
        panic!("indexed recovered release stays quiescent");
    };
    assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Native);
    assert_eq!(
        error.detail(),
        "KFD persistent allocation recycle rejected: scripted recycle recovered"
    );
    assert_eq!(snapshot(&backend, host, device), before);
    assert!(!backend.terminal && backend.terminal_sdma_custody.is_none());
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
        2
    );
    backend.release_allocation_v1(host).unwrap();
    assert!(!backend.allocations.contains_key(&host));
    assert_eq!(
        (backend.allocations.len(), backend.staged_context_bytes),
        (1, 8)
    );
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
fn sdma_recycle_device_retry_preserves_scrub_and_does_not_repeat_demotion() {
    let mut steps = scripted_sync_copy_steps_v1(
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        0,
        8,
        ScriptedFailureModeV1::Success,
    );
    steps.extend([
        ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Recovered),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]);
    let (backend, _, host, device) = fixture(8, steps);
    let mut backend = ManuallyDrop::new(backend);
    let record = backend.allocations.get_mut(&device).unwrap();
    record.bytes = Arc::from([0x7a; 8]);
    record.content_sha256 = Some([0x47; 32]);
    let KfdRuntimeSdmaStorageV1::Device(owner) = &mut record.sdma_storage else {
        unreachable!();
    };
    owner.scripted_bytes_mut().unwrap().fill(0x7a);
    let identity = owner.scripted_owner_id().unwrap();
    let host_before = match &backend.allocations[&host].sdma_storage {
        KfdRuntimeSdmaStorageV1::Host(buffer) => host_observation(buffer),
        _ => unreachable!(),
    };
    assert!(matches!(
        backend.release_allocation_v1(device),
        Err(RuntimeBackendFailureV1::Quiescent(_))
    ));
    let record = &backend.allocations[&device];
    let KfdRuntimeSdmaStorageV1::DemotedDevice(buffer) = &record.sdma_storage else {
        panic!("recovered device remains demoted");
    };
    assert_eq!(host_observation(buffer), (identity, vec![0; 8], None));
    assert!(record.sdma_backed && record.sdma_shadow_dirty && record.content_sha256.is_none());
    assert_eq!(record.bytes.as_ref(), &[0x7a; 8]);
    assert_eq!(
        (backend.allocations.len(), backend.staged_context_bytes),
        (2, 16)
    );
    assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 1);
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
        2
    );
    backend.release_allocation_v1(device).unwrap();
    assert!(!backend.allocations.contains_key(&device));
    assert_eq!(backend.staged_context_bytes, 8);
    let KfdRuntimeSdmaStorageV1::Host(buffer) = &backend.allocations[&host].sdma_storage else {
        unreachable!();
    };
    assert_eq!(host_observation(buffer), host_before);
    assert!(backend.scripted_sdma.as_ref().unwrap().is_exhausted());
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
        0
    );
    discard_scripted_fixture(backend);
}

#[test]
fn sdma_recycle_transient_failures_retain_runtime_or_lower_owner_and_seal_backend() {
    for outcome in [
        ScriptedRecycleOutcomeV1::Recovered,
        ScriptedRecycleOutcomeV1::Ambiguous,
        ScriptedRecycleOutcomeV1::Panic,
    ] {
        let (backend, _, host, device) = fixture(
            8,
            [
                ScriptedSdmaStepV1::Recycle(outcome),
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ],
        );
        let mut backend = ManuallyDrop::new(backend);
        let before = snapshot(&backend, host, device);
        let buffer = backend.scripted_sdma.as_ref().unwrap().test_host_owner(8);
        let observation = host_observation(&buffer);
        let result = catch_unwind(AssertUnwindSafe(|| {
            backend.recycle_transient_sdma_buffer_v1(buffer, "test")
        }));
        if outcome == ScriptedRecycleOutcomeV1::Panic {
            assert_eq!(
                result.unwrap_err().downcast_ref::<&str>(),
                Some(&"scripted recycle panic")
            );
        } else {
            let Err(RuntimeBackendFailureV1::Terminal(error)) = result.unwrap() else {
                panic!("transient must be terminal");
            };
            assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Terminal);
            assert!(
                error
                    .detail()
                    .starts_with("KFD test transient release became ambiguous:")
            );
        }
        if outcome == ScriptedRecycleOutcomeV1::Recovered {
            assert_eq!(host_observation(retained_host(&backend)), observation);
        } else {
            assert!(backend.terminal_sdma_custody.is_none());
            let owner = backend
                .scripted_sdma
                .as_ref()
                .unwrap()
                .recycle_custody()
                .unwrap()
                .observation();
            assert_eq!((owner.0, owner.1.to_vec(), owner.2), observation);
        }
        assert_eq!(snapshot(&backend, host, device), before);
        terminal_retries(&mut backend, host);
        discard_scripted_fixture(backend);
    }
}

#[test]
fn sdma_recycle_indexed_lower_panic_keeps_placeholder_charge_and_original_lower_owner() {
    let (backend, _, host, device) = fixture(
        8,
        [
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Panic),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ],
    );
    let mut backend = ManuallyDrop::new(backend);
    let before = match &backend.allocations[&host].sdma_storage {
        KfdRuntimeSdmaStorageV1::Host(buffer) => host_observation(buffer),
        _ => unreachable!(),
    };
    let neighbor = Arc::clone(&backend.allocations[&device].bytes);
    let result = catch_unwind(AssertUnwindSafe(|| backend.release_allocation_v1(host)));
    assert_eq!(
        result.unwrap_err().downcast_ref::<&str>(),
        Some(&"scripted recycle panic")
    );
    assert!(matches!(
        backend.allocations[&host].sdma_storage,
        KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous)
    ));
    assert!(Arc::ptr_eq(&neighbor, &backend.allocations[&device].bytes));
    assert_eq!(
        (backend.allocations.len(), backend.staged_context_bytes),
        (2, 16)
    );
    let owner = backend
        .scripted_sdma
        .as_ref()
        .unwrap()
        .recycle_custody()
        .unwrap()
        .observation();
    assert_eq!((owner.0, owner.1.to_vec(), owner.2), before);
    terminal_retries(&mut backend, host);
    discard_scripted_fixture(backend);
}

#[test]
fn sdma_recycle_diagnostics_panic_only_after_returned_custody_is_rooted() {
    for indexed in [false, true] {
        for recovered in [false, true] {
            let (backend, _, host, _) = fixture(8, []);
            let mut backend = ManuallyDrop::new(backend);
            let buffer = if indexed {
                extract_host(&mut backend, host)
            } else {
                backend.scripted_sdma.as_ref().unwrap().test_host_owner(8)
            };
            let before = allocation_observations(&backend);
            let observation = host_observation(&buffer);
            let detail = SdmaOwnerDiagnosticV1::Native(
                fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract("typed recycle cause"),
            );
            let failure = if recovered {
                SdmaRecycleFailureV1::Recovered { detail, buffer }
            } else {
                SdmaRecycleFailureV1::ProcessTeardown {
                    detail,
                    custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Buffer(
                        buffer,
                    )),
                }
            };
            let target = if indexed {
                Target::Indexed {
                    allocation: host,
                    kind: RuntimeMemoryKindV1::HostVisible,
                }
            } else {
                Target::Transient("test")
            };
            let original = Box::new(617_u64);
            let address = &*original as *const u64;
            let result = catch_unwind(AssertUnwindSafe(|| {
                backend.settle_sdma_recycle_failure_v1(
                    target,
                    failure,
                    |detail, actual_recovered| {
                        assert_eq!(actual_recovered, recovered);
                        assert!(matches!(
                            detail,
                            SdmaOwnerDiagnosticV1::Native(
                                fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract(
                                    "typed recycle cause"
                                )
                            )
                        ));
                        std::panic::resume_unwind(original)
                    },
                )
            }));
            let payload = result.unwrap_err().downcast::<u64>().unwrap();
            assert_eq!(&*payload as *const u64, address);
            assert_eq!(host_observation(retained_host(&backend)), observation);
            assert_eq!(allocation_observations(&backend), before);
            assert_eq!(
                (backend.allocations.len(), backend.staged_context_bytes),
                (2, 16)
            );
            terminal_retries(&mut backend, host);
            discard_scripted_fixture(backend);
        }
    }
}

#[test]
fn sdma_recycle_restoration_rejects_missing_wrong_kind_and_occupied_slots_without_overwrite() {
    for case in 0..3 {
        let (backend, _, host, _) = fixture(8, []);
        let mut backend = ManuallyDrop::new(backend);
        let buffer = if case == 1 {
            extract_host(&mut backend, host)
        } else {
            backend.scripted_sdma.as_ref().unwrap().test_host_owner(8)
        };
        let before = allocation_observations(&backend);
        let observation = host_observation(&buffer);
        let target = Target::Indexed {
            allocation: if case == 0 { 999 } else { host },
            kind: if case == 1 {
                RuntimeMemoryKindV1::DeviceLocal
            } else {
                RuntimeMemoryKindV1::HostVisible
            },
        };
        let failure = backend.settle_sdma_recycle_failure_v1(
            target,
            SdmaRecycleFailureV1::Recovered {
                detail: SdmaOwnerDiagnosticV1::Scripted("test recovery".to_owned()),
                buffer,
            },
            |detail, _| detail.to_string(),
        );
        assert!(
            matches!(failure, RuntimeBackendFailureV1::Terminal(error) if error.detail() == "recovered recycle allocation slot changed unexpectedly")
        );
        assert_eq!(host_observation(retained_host(&backend)), observation);
        assert_eq!(allocation_observations(&backend), before);
        assert_eq!(
            (backend.allocations.len(), backend.staged_context_bytes),
            (2, 16)
        );
        assert_eq!(
            backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
            if case == 1 { 2 } else { 3 }
        );
        terminal_retries(&mut backend, host);
        discard_scripted_fixture(backend);
    }
}

#[test]
fn sdma_recycle_missing_driver_retains_input_before_selection_panics() {
    let mut backend = ManuallyDrop::new(KfdRuntimeBackendV1::mock());
    let driver = ScriptedSdmaDriverV1::new([]);
    let buffer = driver.test_host_owner(8);
    let before = host_observation(&buffer);
    let result = catch_unwind(AssertUnwindSafe(|| {
        backend.recycle_transient_sdma_buffer_v1(buffer, "selection")
    }));
    assert!(result.is_err() && backend.terminal);
    assert_eq!(host_observation(retained_host(&backend)), before);
    assert_eq!(
        (driver.live_owner_count(), driver.unexpected_drops()),
        (1, 0)
    );
    backend.scripted_sdma = Some(driver);
    discard_scripted_fixture(backend);
}

#[test]
fn sdma_recycle_public_context_distinguishes_retryable_credits_from_quarantine() {
    for configured in [false, true] {
        for outcome in [
            ScriptedRecycleOutcomeV1::Recovered,
            ScriptedRecycleOutcomeV1::Ambiguous,
            ScriptedRecycleOutcomeV1::Panic,
        ] {
            let mut backend = KfdRuntimeBackendV1::mock();
            backend.native_available = true;
            backend.scripted_sdma = Some(ScriptedSdmaDriverV1::new([
                ScriptedSdmaStepV1::Allocate {
                    kind: ScriptedBufferKindV1::Host,
                    byte_len: 8,
                },
                ScriptedSdmaStepV1::Write {
                    offset: 0,
                    byte_len: 8,
                },
                ScriptedSdmaStepV1::Recycle(outcome),
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ]));
            let mut context = ManuallyDrop::new(crate::RuntimeContextV1::open(backend).unwrap());
            let device = context.devices()[0].id();
            if configured {
                context
                    .configure_allocation_admission_v1(device, 8, 1)
                    .unwrap();
            }
            let allocation = context
                .allocate(device, RuntimeMemoryKindV1::HostVisible, 8, 8)
                .unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| context.release_allocation(allocation)));
            let retryable = outcome == ScriptedRecycleOutcomeV1::Recovered;
            if outcome == ScriptedRecycleOutcomeV1::Panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"scripted recycle panic")
                );
            } else if retryable {
                assert!(matches!(
                    result.unwrap(),
                    Err(crate::RuntimeErrorV1::BackendQuiescent(_))
                ));
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(crate::RuntimeErrorV1::BackendTerminal(_))
                ));
            }
            assert_eq!(
                context.is_terminal(),
                !retryable && (configured || outcome != ScriptedRecycleOutcomeV1::Panic)
            );
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
                assert_eq!(usage.quarantined_records, usize::from(!retryable));
            }
            if retryable {
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
                    if retryable { 0 } else { 8 }
                );
                assert_eq!(
                    usage
                        .used
                        .get(crate::RuntimeResourceKindV1::AllocationRecords),
                    u64::from(!retryable)
                );
                assert_eq!(usage.quarantined_records, usize::from(!retryable));
            }
            let backend = context.backend_mut_for_test_v1();
            let driver = backend.scripted_sdma.as_ref().unwrap();
            assert_eq!(driver.remaining_steps(), usize::from(!retryable));
            assert_eq!(driver.live_owner_count(), usize::from(!retryable));
            assert_eq!(driver.unexpected_drops(), 0);
            disarm_scripted_drop_after_inspection_v1(backend);
            drop(ManuallyDrop::into_inner(context));
        }
    }
}

#[test]
fn sdma_recycle_terminal_drop_aborts_with_runtime_and_lower_custody() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_TEST_RUNTIME_RECYCLE_DROP";
    const TEST: &str = "kfd_backend::tests::sdma_recycle_tests::sdma_recycle_terminal_drop_aborts_with_runtime_and_lower_custody";
    if let Some(mode) = std::env::var_os(CHILD) {
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        let driver =
            ScriptedSdmaDriverV1::new([ScriptedSdmaStepV1::Recycle(if mode == "runtime" {
                ScriptedRecycleOutcomeV1::Recovered
            } else {
                ScriptedRecycleOutcomeV1::Ambiguous
            })]);
        let buffer = driver.test_host_owner(8);
        let mut backend = KfdRuntimeBackendV1::mock();
        backend.scripted_sdma = Some(driver);
        assert!(matches!(
            backend.recycle_transient_sdma_buffer_v1(buffer, "drop"),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert_eq!(
            backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
            1
        );
        assert_eq!(
            backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
            0
        );
        eprintln!("recycle retained; dropping terminal backend");
        drop(backend);
        panic!("terminal recycle Drop returned");
    }
    for mode in ["runtime", "lower"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, mode)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "{stderr}");
        assert!(stderr.contains("recycle retained; dropping terminal backend"));
        assert!(!stderr.contains("terminal recycle Drop returned"));
    }
}
