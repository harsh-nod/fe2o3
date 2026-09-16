use super::*;
use crate::kfd_backend::kfd_backend_sdma_seam::ScriptedTerminalCustodyV1;
use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};

pub(super) fn host_observation(buffer: &SdmaBufferOwnerV1) -> (u64, Vec<u8>, Option<[u8; 32]>) {
    let SdmaBufferOwnerV1::Scripted(buffer) = buffer else {
        panic!("expected scripted host owner");
    };
    let (id, bytes, certificate) = buffer.observation();
    (id, bytes.to_vec(), certificate)
}

fn indexed_host(backend: &KfdRuntimeBackendV1, host: u64) -> &SdmaBufferOwnerV1 {
    let KfdRuntimeSdmaStorageV1::Host(buffer) = &backend.allocations[&host].sdma_storage else {
        panic!("original host owner must remain indexed");
    };
    buffer
}

pub(super) fn retained_host(backend: &KfdRuntimeBackendV1) -> &SdmaBufferOwnerV1 {
    match &backend.terminal_sdma_custody {
        Some(KfdRuntimeTerminalSdmaCustodyV1::Buffer(buffer))
        | Some(KfdRuntimeTerminalSdmaCustodyV1::Scripted(ScriptedTerminalCustodyV1::Buffer(
            buffer,
        ))) => buffer,
        _ => panic!("failed operation must retain its buffer"),
    }
}

#[derive(Debug, PartialEq)]
pub(super) struct Snapshot {
    pub(super) next_handle: u64,
    pub(super) staged_bytes: u64,
    pub(super) allocations: usize,
    host: (u64, Vec<u8>, Option<[u8; 32]>),
    pub(super) device: (u64, Vec<u8>),
    host_shadow: Arc<[u8]>,
    device_shadow: Arc<[u8]>,
    host_shadow_address: usize,
    device_shadow_address: usize,
    host_digest: Option<[u8; 32]>,
    events: usize,
}

pub(super) fn snapshot(backend: &KfdRuntimeBackendV1, host: u64, device: u64) -> Snapshot {
    let KfdRuntimeSdmaStorageV1::Device(owner) = &backend.allocations[&device].sdma_storage else {
        panic!("original device owner must remain indexed, not InFlight");
    };
    Snapshot {
        next_handle: backend.next_handle,
        staged_bytes: backend.staged_context_bytes,
        allocations: backend.allocations.len(),
        host: host_observation(indexed_host(backend, host)),
        device: (
            owner.scripted_owner_id().unwrap(),
            owner.scripted_bytes().unwrap().to_vec(),
        ),
        host_shadow: Arc::clone(&backend.allocations[&host].bytes),
        device_shadow: Arc::clone(&backend.allocations[&device].bytes),
        host_shadow_address: Arc::as_ptr(&backend.allocations[&host].bytes) as *const u8 as usize,
        device_shadow_address: Arc::as_ptr(&backend.allocations[&device].bytes) as *const u8
            as usize,
        host_digest: backend.allocations[&host].content_sha256,
        events: backend
            .profiler
            .as_ref()
            .unwrap()
            .recorded_events_for_test_v1()
            .len(),
    }
}

pub(super) fn fixture(
    len: usize,
    steps: impl IntoIterator<Item = ScriptedSdmaStepV1>,
) -> (KfdRuntimeBackendV1, u64, u64, u64) {
    scripted_direct_backend_configured_v1(len, steps, |backend| {
        backend
            .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([0x83; 32], 64).unwrap())
            .unwrap();
    })
}

fn assert_failure<T: fmt::Debug>(
    result: std::thread::Result<Result<T, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>>,
    panic: bool,
    operation: &str,
) {
    if panic {
        let payload = result.unwrap_err();
        assert_eq!(
            payload.downcast_ref::<&'static str>(),
            Some(&"scripted SDMA host write panic")
        );
    } else {
        let Err(RuntimeBackendFailureV1::Terminal(error)) = result.unwrap() else {
            panic!("write failure must be terminal");
        };
        assert!(error.to_string().contains(operation));
        assert!(
            error
                .to_string()
                .contains("scripted SDMA host write failure")
        );
    }
}

pub(super) fn assert_terminal_retries_inert(
    backend: &mut KfdRuntimeBackendV1,
    host: u64,
    device: u64,
) {
    assert!(backend.terminal);
    let before = snapshot(backend, host, device);
    let driver = backend.scripted_sdma.as_ref().unwrap();
    let counts = (
        driver.remaining_steps(),
        driver.live_owner_count(),
        driver.unexpected_drops(),
    );
    assert_eq!(counts.2, 0);
    let retained = backend
        .terminal_sdma_custody
        .as_ref()
        .map(|_| host_observation(retained_host(backend)));
    assert!(matches!(
        backend.allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(matches!(
        backend.write_allocation_v1(host, 0, &[7]),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(matches!(
        backend.read_allocation_v1(host, 0, &mut [0; 1]),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(matches!(
        backend.release_allocation_v1(host),
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
    assert_eq!(snapshot(backend, host, device), before);
    assert_eq!(
        backend
            .terminal_sdma_custody
            .as_ref()
            .map(|_| host_observation(retained_host(backend))),
        retained
    );
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert_eq!(
        (
            driver.remaining_steps(),
            driver.live_owner_count(),
            driver.unexpected_drops()
        ),
        counts
    );
}

pub(super) fn discard_scripted_fixture(mut backend: ManuallyDrop<KfdRuntimeBackendV1>) {
    // Observation ends here; this is not native cleanup or terminal refund evidence.
    disarm_scripted_drop_after_inspection_v1(&mut backend);
    drop(ManuallyDrop::into_inner(backend));
}

#[test]
fn host_write_fresh_initialization_retains_exact_prefix_before_any_commit() {
    for panic in [false, true] {
        for written_prefix in [0, 3, 8] {
            let steps = [
                ScriptedSdmaStepV1::AllocateHostFilled {
                    byte_len: 8,
                    fill: 0xa5,
                },
                ScriptedSdmaStepV1::WriteFault {
                    offset: 0,
                    byte_len: 8,
                    written_prefix,
                    panic,
                },
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ];
            let (backend, _, host, device) = fixture(16, steps);
            let mut backend = ManuallyDrop::new(backend);
            let mut expected = snapshot(&backend, host, device);
            let expected_owner = expected.device.0 + 1;
            expected.next_handle += 1;
            let result = catch_unwind(AssertUnwindSafe(|| {
                backend.allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            }));
            assert_failure(
                result,
                panic,
                "KFD persistent host allocation initialization",
            );
            assert_eq!(snapshot(&backend, host, device), expected);
            let mut bytes = vec![0xa5; 8];
            bytes[..written_prefix].fill(0);
            assert_eq!(
                host_observation(retained_host(&backend)),
                (expected_owner, bytes, None)
            );
            let driver = backend.scripted_sdma.as_ref().unwrap();
            assert_eq!(driver.remaining_steps(), 1);
            assert_eq!(driver.live_owner_count(), 3);
            assert_terminal_retries_inert(&mut backend, host, device);
            discard_scripted_fixture(backend);
        }
    }
}

#[test]
fn host_write_upload_staging_failure_never_consumes_device_or_recycles() {
    for panic in [false, true] {
        for written_prefix in [0, 3, 8] {
            let steps = [
                ScriptedSdmaStepV1::AllocateHostFilled {
                    byte_len: 8,
                    fill: 0xa5,
                },
                ScriptedSdmaStepV1::WriteFault {
                    offset: 0,
                    byte_len: 8,
                    written_prefix,
                    panic,
                },
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ];
            let (backend, _, host, device) = fixture(16, steps);
            let mut backend = ManuallyDrop::new(backend);
            let expected = snapshot(&backend, host, device);
            let result = catch_unwind(AssertUnwindSafe(|| {
                backend.write_allocation_v1(device, 2, &[9; 8])
            }));
            assert_failure(result, panic, "KFD upload staging write");
            assert_eq!(snapshot(&backend, host, device), expected);
            let mut bytes = vec![0xa5; 8];
            bytes[..written_prefix].fill(9);
            assert_eq!(
                host_observation(retained_host(&backend)),
                (expected.device.0 + 1, bytes, None)
            );
            let driver = backend.scripted_sdma.as_ref().unwrap();
            assert_eq!(driver.remaining_steps(), 1);
            assert_eq!(driver.live_owner_count(), 3);
            assert_terminal_retries_inert(&mut backend, host, device);
            discard_scripted_fixture(backend);
        }
    }
}

#[test]
fn host_write_indexed_partial_and_authenticated_faults_keep_owner_and_old_shadow() {
    for full in [false, true] {
        let (offset, byte_len) = if full { (0, 16) } else { (2, 8) };
        for panic in [false, true] {
            for written_prefix in [0, 3, byte_len] {
                let steps = [
                    ScriptedSdmaStepV1::Write {
                        offset: 0,
                        byte_len: 16,
                    },
                    ScriptedSdmaStepV1::WriteFault {
                        offset,
                        byte_len,
                        written_prefix,
                        panic,
                    },
                    ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
                ];
                let (backend, _, host, device) = fixture(16, steps);
                let mut backend = ManuallyDrop::new(backend);
                backend.write_allocation_v1(host, 0, &[4; 16]).unwrap();
                let mut expected = snapshot(&backend, host, device);
                assert!(expected.host.2.is_some());
                let prior_image = Arc::clone(&backend.allocations[&host].bytes);
                let prior_full = backend.allocations[&host]
                    .last_full_host_write
                    .clone()
                    .unwrap();
                let result = catch_unwind(AssertUnwindSafe(|| {
                    backend.write_allocation_v1(host, offset, &vec![9; byte_len])
                }));
                assert_failure(
                    result,
                    panic,
                    if full {
                        "KFD persistent authenticated host write"
                    } else {
                        "KFD persistent host write"
                    },
                );
                expected.host.1[offset as usize..offset as usize + written_prefix].fill(9);
                expected.host.2 = None;
                assert_eq!(snapshot(&backend, host, device), expected);
                let record = &backend.allocations[&host];
                assert!(Arc::ptr_eq(&record.bytes, &prior_image));
                let full_image = record.last_full_host_write.as_ref().unwrap();
                assert!(Arc::ptr_eq(&full_image.0, &prior_full.0));
                assert_eq!(full_image.1, prior_full.1);
                assert!(backend.terminal_sdma_custody.is_none());
                let driver = backend.scripted_sdma.as_ref().unwrap();
                assert_eq!(driver.remaining_steps(), 1);
                assert_eq!(driver.live_owner_count(), 2);
                assert_terminal_retries_inert(&mut backend, host, device);
                discard_scripted_fixture(backend);
            }
        }
    }
}

#[test]
fn host_write_success_initializes_and_uploads_then_releases_original_owners() {
    let mut steps = vec![
        ScriptedSdmaStepV1::AllocateHostFilled {
            byte_len: 8,
            fill: 0xa5,
        },
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len: 8,
        },
    ];
    steps.extend(scripted_sync_copy_steps_v1(
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        2,
        8,
        ScriptedFailureModeV1::Success,
    ));
    steps.push(ScriptedSdmaStepV1::Recycle(
        ScriptedRecycleOutcomeV1::Success,
    ));
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) = fixture(16, steps);
    let created = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    assert_eq!(
        host_observation(indexed_host(&backend, created)),
        (3, vec![0; 8], None)
    );
    assert_eq!(backend.staged_context_bytes, 40);
    backend.write_allocation_v1(device, 2, &[9; 8]).unwrap();
    let mut expected = vec![0; 16];
    expected[2..10].fill(9);
    assert_eq!(snapshot(&backend, host, device).device.1, expected);
    assert_eq!(backend.allocations[&device].bytes.as_ref(), expected);
    assert!(!backend.terminal && backend.terminal_sdma_custody.is_none());
    backend.release_allocation_v1(created).unwrap();
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, None);
    assert_eq!(backend.staged_context_bytes, 0);
    backend.finish_profiler_v1().unwrap().validate().unwrap();
}

#[test]
fn host_write_second_upload_chunk_failure_retains_new_staging_and_completed_device_prefix() {
    let chunk = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize;
    for panic in [false, true] {
        let mut steps = scripted_sync_copy_steps_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            chunk as u32,
            ScriptedFailureModeV1::Success,
        );
        steps.extend([
            ScriptedSdmaStepV1::AllocateHostFilled {
                byte_len: 8,
                fill: 0xa5,
            },
            ScriptedSdmaStepV1::WriteFault {
                offset: 0,
                byte_len: 8,
                written_prefix: 3,
                panic,
            },
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ]);
        let (backend, _, host, device) = fixture(chunk + 8, steps);
        let mut backend = ManuallyDrop::new(backend);
        let before = snapshot(&backend, host, device);
        let result = catch_unwind(AssertUnwindSafe(|| {
            backend.write_allocation_v1(device, 0, &vec![9; chunk + 8])
        }));
        assert_failure(result, panic, "KFD upload staging write");
        let mut expected = before;
        expected.device.1[..chunk].fill(9);
        assert_eq!(snapshot(&backend, host, device), expected);
        assert_eq!(backend.allocations[&device].sdma_shadow_dirty, !panic);
        let mut retained = vec![0xa5; 8];
        retained[..3].fill(9);
        assert_eq!(
            host_observation(retained_host(&backend)),
            (4, retained, None)
        );
        assert_eq!(
            backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
            3
        );
        assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 1);
        assert_terminal_retries_inert(&mut backend, host, device);
        discard_scripted_fixture(backend);
    }
}

#[test]
fn host_write_second_indexed_chunk_failure_never_certifies_or_commits_partial_image() {
    let chunk = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize;
    for full in [false, true] {
        let offset = if full { 0 } else { 2 };
        let len = chunk + 8 + offset;
        for panic in [false, true] {
            let steps = [
                ScriptedSdmaStepV1::Write {
                    offset: 0,
                    byte_len: chunk,
                },
                ScriptedSdmaStepV1::Write {
                    offset: chunk as u64,
                    byte_len: len - chunk,
                },
                ScriptedSdmaStepV1::Write {
                    offset: offset as u64,
                    byte_len: chunk,
                },
                ScriptedSdmaStepV1::WriteFault {
                    offset: (offset + chunk) as u64,
                    byte_len: 8,
                    written_prefix: 3,
                    panic,
                },
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ];
            let (backend, _, host, device) = fixture(len, steps);
            let mut backend = ManuallyDrop::new(backend);
            backend.write_allocation_v1(host, 0, &vec![4; len]).unwrap();
            let mut expected = snapshot(&backend, host, device);
            assert!(expected.host.2.is_some());
            let old_image = Arc::clone(&backend.allocations[&host].bytes);
            let result = catch_unwind(AssertUnwindSafe(|| {
                backend.write_allocation_v1(host, offset as u64, &vec![9; chunk + 8])
            }));
            assert_failure(
                result,
                panic,
                if full {
                    "KFD persistent authenticated host write"
                } else {
                    "KFD persistent host write"
                },
            );
            expected.host.1[offset..offset + chunk + 3].fill(9);
            expected.host.2 = None;
            assert_eq!(snapshot(&backend, host, device), expected);
            assert!(Arc::ptr_eq(&backend.allocations[&host].bytes, &old_image));
            assert!(backend.terminal_sdma_custody.is_none());
            let driver = backend.scripted_sdma.as_ref().unwrap();
            assert_eq!(driver.live_owner_count(), 2);
            assert_eq!(driver.remaining_steps(), 1);
            assert_terminal_retries_inert(&mut backend, host, device);
            discard_scripted_fixture(backend);
        }
    }
}

#[test]
fn host_write_public_context_seals_on_error_or_following_caught_backend_panic() {
    for initialization in [false, true] {
        for panic in [false, true] {
            let mut steps = vec![ScriptedSdmaStepV1::AllocateHostFilled {
                byte_len: 8,
                fill: 0xa5,
            }];
            if !initialization {
                steps.push(ScriptedSdmaStepV1::Write {
                    offset: 0,
                    byte_len: 8,
                });
            }
            steps.push(ScriptedSdmaStepV1::WriteFault {
                offset: 0,
                byte_len: 8,
                written_prefix: 3,
                panic,
            });
            steps.push(ScriptedSdmaStepV1::Recycle(
                ScriptedRecycleOutcomeV1::Success,
            ));
            let mut backend = KfdRuntimeBackendV1::mock();
            backend.native_available = true;
            backend.scripted_sdma = Some(ScriptedSdmaDriverV1::new(steps));
            let mut context = ManuallyDrop::new(crate::RuntimeContextV1::open(backend).unwrap());
            let device = context.devices()[0].id();
            let result = if initialization {
                catch_unwind(AssertUnwindSafe(|| {
                    context
                        .allocate(device, RuntimeMemoryKindV1::HostVisible, 8, 8)
                        .map(|_| ())
                }))
            } else {
                let allocation = context
                    .allocate(device, RuntimeMemoryKindV1::HostVisible, 8, 8)
                    .unwrap();
                catch_unwind(AssertUnwindSafe(|| {
                    context.write_allocation(allocation, 0, &[9; 8])
                }))
            };
            if panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<&'static str>(),
                    Some(&"scripted SDMA host write panic")
                );
                assert!(!context.is_terminal());
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(crate::RuntimeErrorV1::BackendTerminal(_))
                ));
                assert!(context.is_terminal());
            }
            assert!(context.backend_mut_for_test_v1().terminal);
            assert!(
                context
                    .allocate(device, RuntimeMemoryKindV1::HostVisible, 8, 8)
                    .is_err()
            );
            assert!(context.is_terminal());
            let backend = context.backend_mut_for_test_v1();
            assert_eq!(backend.terminal_sdma_custody.is_some(), initialization);
            assert_eq!(backend.allocations.len(), usize::from(!initialization));
            let driver = backend.scripted_sdma.as_ref().unwrap();
            assert_eq!(driver.remaining_steps(), 1);
            assert_eq!(driver.live_owner_count(), 1);
            assert_eq!(driver.unexpected_drops(), 0);
            disarm_scripted_drop_after_inspection_v1(backend);
            drop(ManuallyDrop::into_inner(context));
        }
    }
}

#[test]
fn host_write_terminal_backend_drop_aborts_without_discarding_retained_owner() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_TEST_HOST_WRITE_DROP";
    const TEST: &str = "kfd_backend::tests::sdma_host_write_tests::host_write_terminal_backend_drop_aborts_without_discarding_retained_owner";
    if let Some(mode) = std::env::var_os(CHILD) {
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        let initialize = mode == "initialize";
        let mut steps = Vec::new();
        if initialize {
            steps.push(ScriptedSdmaStepV1::AllocateHostFilled {
                byte_len: 8,
                fill: 0xa5,
            });
        }
        steps.push(ScriptedSdmaStepV1::WriteFault {
            offset: 0,
            byte_len: 8,
            written_prefix: 3,
            panic: false,
        });
        let (mut backend, _, host, _) = fixture(8, steps);
        let result = if initialize {
            backend
                .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
                .map(|_| ())
        } else {
            backend.write_allocation_v1(host, 0, &[9; 8])
        };
        assert!(matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))));
        assert_eq!(
            backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
            0
        );
        eprintln!("host write retained; dropping terminal backend");
        drop(backend);
        panic!("terminal Drop returned");
    }
    for mode in ["initialize", "indexed"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, mode)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "{stderr}");
        assert!(stderr.contains("host write retained; dropping terminal backend"));
        assert!(!stderr.contains("panicked at"));
    }
}
