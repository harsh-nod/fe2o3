use super::sdma_host_write_tests::{
    assert_terminal_retries_inert, discard_scripted_fixture, fixture, host_observation,
    retained_host, snapshot,
};
use super::*;
use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};

const DEVICE_PATTERN: [u8; 8] = [0x61, 0x28, 0xdf, 0x03, 0x9a, 0xf1, 0x42, 0x7e];

fn seed_device(backend: &mut KfdRuntimeBackendV1, device: u64) {
    let KfdRuntimeSdmaStorageV1::Device(owner) =
        &mut backend.allocations.get_mut(&device).unwrap().sdma_storage
    else {
        panic!()
    };
    owner
        .scripted_bytes_mut()
        .unwrap()
        .copy_from_slice(&DEVICE_PATTERN);
}

fn no_host_read(backend: &KfdRuntimeBackendV1) {
    assert!(
        !backend
            .profiler
            .as_ref()
            .unwrap()
            .recorded_events_for_test_v1()
            .iter()
            .any(|entry| matches!(entry.event, KfdRuntimeProfileEventKindV1::HostRead { .. }))
    );
}

fn read_step(mode: u8) -> ScriptedSdmaStepV1 {
    match mode {
        0 | 1 => ScriptedSdmaStepV1::ReadFault {
            offset: 0,
            byte_len: 8,
            panic: mode == 1,
        },
        _ => ScriptedSdmaStepV1::ReadLength {
            offset: 0,
            byte_len: 8,
            returned_len: if mode == 2 { 7 } else { 9 },
        },
    }
}

#[test]
fn sdma_host_read_into_has_no_intermediate_allocation() {
    for (offset, len) in [(0, 8), (2, 3)] {
        let (backend, _, host, _) = fixture(
            8,
            [
                ScriptedSdmaStepV1::Write {
                    offset: 0,
                    byte_len: 8,
                },
                ScriptedSdmaStepV1::Read {
                    offset,
                    byte_len: len,
                },
            ],
        );
        let mut backend = ManuallyDrop::new(backend);
        backend
            .write_allocation_v1(host, 0, &DEVICE_PATTERN)
            .unwrap();
        backend.profiler = None;
        let mut destination = [0xff; 10];
        let (result, allocations) = counted_allocations_for_test_v1(|| {
            backend.read_allocation_v1(host, offset, &mut destination[1..1 + len as usize])
        });
        result.unwrap();
        assert_eq!(
            destination[1..1 + len as usize],
            DEVICE_PATTERN[offset as usize..(offset + len) as usize]
        );
        assert_eq!(destination[0], 0xff);
        assert!(
            destination[1 + len as usize..]
                .iter()
                .all(|byte| *byte == 0xff)
        );
        assert_eq!(allocations, 0, "readback must not allocate a byte copy");
        assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 0);
        discard_scripted_fixture(backend);
    }
}

#[test]
fn sdma_host_read_into_transient_has_no_intermediate_allocation() {
    let (backend, _, _, _) = fixture(
        8,
        [
            ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len: 8,
            },
            ScriptedSdmaStepV1::Read {
                offset: 0,
                byte_len: 8,
            },
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ],
    );
    let mut backend = ManuallyDrop::new(backend);
    let mut stage = backend.scripted_sdma.as_ref().unwrap().test_host_owner(8);
    backend
        .directional_sdma_ops_v1()
        .write_host(&mut stage, 0, &DEVICE_PATTERN)
        .unwrap();
    let mut destination = [0xff; 10];
    let (result, allocations) = counted_allocations_for_test_v1(|| {
        backend.readback_and_recycle_transient_sdma_v1(stage, &mut destination[1..9])
    });
    result.unwrap();
    assert_eq!(destination[1..9], DEVICE_PATTERN);
    assert_eq!((destination[0], destination[9]), (0xff, 0xff));
    assert_eq!(allocations, 0);
    assert!(backend.terminal_sdma_custody.is_none());
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert_eq!(driver.remaining_steps(), 0);
    assert_eq!(driver.live_owner_count(), 2);
    assert_eq!(driver.unexpected_drops(), 0);
    discard_scripted_fixture(backend);
}

#[test]
fn sdma_host_read_into_partial_failure_preserves_custody_and_visible_prefix() {
    for transient in [false, true] {
        for copied_prefix in [0, 3, 8] {
            for panic in [false, true] {
                let fault = ScriptedSdmaStepV1::ReadPartialFault {
                    offset: 0,
                    byte_len: 8,
                    copied_prefix,
                    panic,
                };
                let steps = if transient {
                    let mut steps = scripted_sync_copy_steps_v1(
                        Gfx942PersistentSdmaDirectionV1::DeviceToHost,
                        0,
                        8,
                        ScriptedFailureModeV1::Success,
                    );
                    let index = steps.len() - 2;
                    steps[index] = fault;
                    steps
                } else {
                    vec![
                        ScriptedSdmaStepV1::Write {
                            offset: 0,
                            byte_len: 8,
                        },
                        fault,
                        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
                    ]
                };
                let (backend, _, host, device) = fixture(8, steps);
                let mut backend = ManuallyDrop::new(backend);
                if transient {
                    seed_device(&mut backend, device);
                } else {
                    backend
                        .write_allocation_v1(host, 0, &DEVICE_PATTERN)
                        .unwrap();
                }
                let before = snapshot(&backend, host, device);
                let mut destination = [0xff; 10];
                let result = catch_unwind(AssertUnwindSafe(|| {
                    backend.read_allocation_v1(
                        if transient { device } else { host },
                        0,
                        &mut destination[1..9],
                    )
                }));
                if panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<&str>(),
                        Some(&"scripted SDMA host read panic")
                    );
                } else {
                    let Err(RuntimeBackendFailureV1::Terminal(error)) = result.unwrap() else {
                        panic!()
                    };
                    assert_eq!(
                        error.detail(),
                        if transient {
                            "KFD download readback: scripted SDMA host read failure"
                        } else {
                            "KFD persistent host read: scripted SDMA host read failure"
                        }
                    );
                }
                assert_eq!(destination[0], 0xff);
                assert_eq!(
                    destination[1..1 + copied_prefix],
                    DEVICE_PATTERN[..copied_prefix]
                );
                assert!(
                    destination[1 + copied_prefix..]
                        .iter()
                        .all(|byte| *byte == 0xff)
                );
                assert_eq!(snapshot(&backend, host, device), before);
                no_host_read(&backend);
                if transient && panic {
                    assert_eq!(
                        host_observation(retained_host(&backend)),
                        (3, DEVICE_PATTERN.to_vec(), None)
                    );
                } else {
                    assert!(backend.terminal_sdma_custody.is_none());
                }
                let driver = backend.scripted_sdma.as_ref().unwrap();
                assert_eq!(driver.remaining_steps(), usize::from(!transient || panic));
                assert_eq!(
                    driver.live_owner_count(),
                    if transient && panic { 3 } else { 2 }
                );
                assert_eq!(driver.unexpected_drops(), 0);
                assert_terminal_retries_inert(&mut backend, host, device);
                discard_scripted_fixture(backend);
            }
        }
    }
}

#[test]
fn sdma_host_read_indexed_failures_retain_bytes_certificate_and_unchanged_destination() {
    for mode in 0..4 {
        let (backend, _, host, device) = fixture(
            8,
            [
                ScriptedSdmaStepV1::Write {
                    offset: 0,
                    byte_len: 8,
                },
                read_step(mode),
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ],
        );
        let mut backend = ManuallyDrop::new(backend);
        backend.write_allocation_v1(host, 0, &[0x73; 8]).unwrap();
        let KfdRuntimeSdmaStorageV1::Host(owner) = &backend.allocations[&host].sdma_storage else {
            panic!()
        };
        assert_eq!(
            host_observation(owner).2,
            Some(Sha256::digest([0x73; 8]).into())
        );
        let steps = backend.scripted_sdma.as_ref().unwrap().remaining_steps();
        backend.read_allocation_v1(host, 0, &mut []).unwrap();
        assert!(matches!(
            backend.read_allocation_v1(host, 1, &mut [0; 8]),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert_eq!(
            backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
            steps
        );
        let before = snapshot(&backend, host, device);
        let events = backend
            .profiler
            .as_ref()
            .unwrap()
            .recorded_events_for_test_v1()
            .to_vec();
        let mut destination = [0xff; 8];
        let result = catch_unwind(AssertUnwindSafe(|| {
            backend.read_allocation_v1(host, 0, &mut destination)
        }));
        if mode == 0 {
            let Err(RuntimeBackendFailureV1::Terminal(error)) = result.unwrap() else {
                panic!()
            };
            assert_eq!(
                error.detail(),
                "KFD persistent host read: scripted SDMA host read failure"
            );
        } else {
            let payload = result.unwrap_err();
            if mode == 1 {
                assert_eq!(
                    payload.downcast_ref::<&str>(),
                    Some(&"scripted SDMA host read panic")
                );
            }
        }
        assert_eq!(destination, [0xff; 8]);
        assert_eq!(snapshot(&backend, host, device), before);
        assert!(backend.terminal_sdma_custody.is_none());
        assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 1);
        assert_eq!(
            backend
                .profiler
                .as_ref()
                .unwrap()
                .recorded_events_for_test_v1(),
            events
        );
        assert_terminal_retries_inert(&mut backend, host, device);
        discard_scripted_fixture(backend);
    }
}

#[test]
fn sdma_host_read_transient_errors_preserve_cleanup_precedence_and_copy_visibility() {
    for (read_fails, copied_prefix) in [(false, 8), (true, 0), (true, 3), (true, 8)] {
        for recycle in [
            ScriptedRecycleOutcomeV1::Success,
            ScriptedRecycleOutcomeV1::Recovered,
            ScriptedRecycleOutcomeV1::Ambiguous,
            ScriptedRecycleOutcomeV1::Panic,
        ] {
            let mut steps = scripted_sync_copy_steps_v1(
                Gfx942PersistentSdmaDirectionV1::DeviceToHost,
                0,
                8,
                ScriptedFailureModeV1::Success,
            );
            let n = steps.len();
            if read_fails {
                steps[n - 2] = ScriptedSdmaStepV1::ReadPartialFault {
                    offset: 0,
                    byte_len: 8,
                    copied_prefix,
                    panic: false,
                };
            }
            steps[n - 1] = ScriptedSdmaStepV1::Recycle(recycle);
            let (backend, _, host, device) = fixture(8, steps);
            let mut backend = ManuallyDrop::new(backend);
            seed_device(&mut backend, device);
            let before = snapshot(&backend, host, device);
            let expected_id = backend.scripted_sdma.as_ref().unwrap().live_owner_count() as u64 + 1;
            let mut destination = [0xff; 8];
            let result = catch_unwind(AssertUnwindSafe(|| {
                backend.read_allocation_v1(device, 0, &mut destination)
            }));
            assert_eq!(
                destination[..copied_prefix],
                DEVICE_PATTERN[..copied_prefix]
            );
            assert!(
                destination[copied_prefix..]
                    .iter()
                    .all(|byte| *byte == 0xff)
            );
            match recycle {
                ScriptedRecycleOutcomeV1::Panic => assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"scripted recycle panic")
                ),
                ScriptedRecycleOutcomeV1::Success if !read_fails => result.unwrap().unwrap(),
                _ => {
                    let Err(RuntimeBackendFailureV1::Terminal(error)) = result.unwrap() else {
                        panic!()
                    };
                    let expected = match recycle {
                        ScriptedRecycleOutcomeV1::Success => {
                            "KFD download readback: scripted SDMA host read failure"
                        }
                        ScriptedRecycleOutcomeV1::Recovered => {
                            "KFD download transient release became ambiguous: scripted recycle recovered"
                        }
                        ScriptedRecycleOutcomeV1::Ambiguous => {
                            "KFD download transient release became ambiguous: scripted recycle ambiguous"
                        }
                        ScriptedRecycleOutcomeV1::Panic => unreachable!(),
                    };
                    assert_eq!(error.detail(), expected);
                }
            }
            let driver = backend.scripted_sdma.as_ref().unwrap();
            assert_eq!(driver.remaining_steps(), 0);
            assert_eq!(driver.unexpected_drops(), 0);
            if recycle == ScriptedRecycleOutcomeV1::Recovered {
                assert_eq!(
                    host_observation(retained_host(&backend)),
                    (expected_id, DEVICE_PATTERN.to_vec(), None)
                );
            } else if matches!(
                recycle,
                ScriptedRecycleOutcomeV1::Ambiguous | ScriptedRecycleOutcomeV1::Panic
            ) {
                assert_eq!(
                    driver.recycle_custody().unwrap().observation(),
                    (expected_id, DEVICE_PATTERN.as_slice(), None)
                );
            }
            assert_eq!(
                driver.live_owner_count(),
                if recycle == ScriptedRecycleOutcomeV1::Success {
                    2
                } else {
                    3
                }
            );
            if read_fails || recycle != ScriptedRecycleOutcomeV1::Success {
                assert_eq!(snapshot(&backend, host, device), before);
                no_host_read(&backend);
                assert_terminal_retries_inert(&mut backend, host, device);
            } else {
                assert!(!backend.terminal);
                assert!(backend.terminal_sdma_custody.is_none());
                assert_eq!(
                    backend
                        .profiler
                        .as_ref()
                        .unwrap()
                        .recorded_events_for_test_v1()
                        .iter()
                        .filter(|entry| matches!(
                            entry.event,
                            KfdRuntimeProfileEventKindV1::HostRead { .. }
                        ))
                        .count(),
                    1
                );
            }
            discard_scripted_fixture(backend);
        }
    }
}

#[test]
fn sdma_host_read_transient_panics_retain_staging_without_attempting_recycle() {
    for mode in 1..4 {
        let mut steps = scripted_sync_copy_steps_v1(
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
            0,
            8,
            ScriptedFailureModeV1::Success,
        );
        let n = steps.len();
        steps[n - 2] = read_step(mode);
        let (backend, _, host, device) = fixture(8, steps);
        let mut backend = ManuallyDrop::new(backend);
        seed_device(&mut backend, device);
        let before = snapshot(&backend, host, device);
        let mut destination = [0xff; 8];
        let result = catch_unwind(AssertUnwindSafe(|| {
            backend.read_allocation_v1(device, 0, &mut destination)
        }));
        let payload = result.unwrap_err();
        if mode == 1 {
            assert_eq!(
                payload.downcast_ref::<&str>(),
                Some(&"scripted SDMA host read panic")
            );
        }
        assert_eq!(destination, [0xff; 8]);
        assert_eq!(snapshot(&backend, host, device), before);
        assert_eq!(
            host_observation(retained_host(&backend)),
            (3, DEVICE_PATTERN.to_vec(), None)
        );
        assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 1);
        assert_eq!(
            backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
            3
        );
        no_host_read(&backend);
        assert_terminal_retries_inert(&mut backend, host, device);
        discard_scripted_fixture(backend);
    }
}

#[test]
fn sdma_host_read_missing_driver_and_wrong_recycle_phase_preserve_original_root() {
    for transient in [false, true] {
        let (backend, _, host, device) = fixture(8, []);
        let mut backend = ManuallyDrop::new(backend);
        let stage = if transient {
            Some(backend.scripted_sdma.as_ref().unwrap().test_host_owner(8))
        } else {
            None
        };
        let expected = stage.as_ref().map(host_observation);
        let driver = backend.scripted_sdma.take();
        let mut destination = [0xff; 8];
        let result = catch_unwind(AssertUnwindSafe(|| match stage {
            Some(stage) => backend.readback_and_recycle_transient_sdma_v1(stage, &mut destination),
            None => backend.read_indexed_sdma_host_into_v1(host, 0, &mut destination),
        }));
        assert!(result.is_err());
        backend.scripted_sdma = driver;
        assert_eq!(destination, [0xff; 8]);
        if let Some(expected) = expected {
            assert_eq!(host_observation(retained_host(&backend)), expected);
        }
        assert_terminal_retries_inert(&mut backend, host, device);
        discard_scripted_fixture(backend);
    }
    let (backend, _, _, _) = fixture(8, []);
    let mut backend = ManuallyDrop::new(backend);
    let owner = backend.scripted_sdma.as_ref().unwrap().test_device_owner(8);
    let id = owner.scripted_owner_id();
    backend.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Device(owner));
    assert!(
        catch_unwind(AssertUnwindSafe(|| backend.recycle_rooted_sdma_owner_v1(
            crate::kfd_backend::sdma_recycle::SdmaRecycleTargetV1::Transient("download")
        )))
        .is_err()
    );
    let Some(KfdRuntimeTerminalSdmaCustodyV1::Device(owner)) = &backend.terminal_sdma_custody
    else {
        panic!("wrong-phase owner must stay rooted")
    };
    assert_eq!(owner.scripted_owner_id(), id);
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
        0
    );
    assert!(backend.terminal);
    discard_scripted_fixture(backend);
}

#[test]
fn sdma_host_read_second_chunk_error_or_panic_preserves_exact_visible_prefix() {
    let chunk = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize;
    for panic in [false, true] {
        let (backend, _, host, device) = fixture(
            chunk + 11,
            [
                ScriptedSdmaStepV1::Read {
                    offset: 3,
                    byte_len: chunk as u64,
                },
                ScriptedSdmaStepV1::ReadFault {
                    offset: 3 + chunk as u64,
                    byte_len: 8,
                    panic,
                },
            ],
        );
        let mut backend = ManuallyDrop::new(backend);
        let before = snapshot(&backend, host, device);
        let mut destination = vec![0xff; chunk + 8];
        let result = catch_unwind(AssertUnwindSafe(|| {
            backend.read_allocation_v1(host, 3, &mut destination)
        }));
        if panic {
            assert_eq!(
                result.unwrap_err().downcast_ref::<&str>(),
                Some(&"scripted SDMA host read panic")
            );
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
        }
        assert!(destination[..chunk].iter().all(|&byte| byte == 0));
        assert_eq!(&destination[chunk..], &[0xff; 8]);
        assert_eq!(snapshot(&backend, host, device), before);
        no_host_read(&backend);
        assert_terminal_retries_inert(&mut backend, host, device);
        discard_scripted_fixture(backend);
    }
}

#[test]
fn sdma_host_read_context_preserves_credits_and_latches_caught_panic_on_retry() {
    for configured in [false, true] {
        for panic in [false, true] {
            let mut backend = KfdRuntimeBackendV1::mock();
            backend.native_available = true;
            backend.scripted_sdma = Some(ScriptedSdmaDriverV1::new([
                ScriptedSdmaStepV1::AllocateHostFilled {
                    byte_len: 8,
                    fill: 0xa5,
                },
                ScriptedSdmaStepV1::Write {
                    offset: 0,
                    byte_len: 8,
                },
                ScriptedSdmaStepV1::ReadFault {
                    offset: 0,
                    byte_len: 8,
                    panic,
                },
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
            let observe_host = |backend: &KfdRuntimeBackendV1| {
                let (&id, record) = backend.allocations.ordinary_iter().next().unwrap();
                let KfdRuntimeSdmaStorageV1::Host(owner) = &record.sdma_storage else {
                    panic!()
                };
                (
                    id,
                    host_observation(owner),
                    Arc::clone(&record.bytes),
                    record.bytes.as_ptr() as usize,
                    record.content_sha256,
                )
            };
            let original_host = observe_host(context.backend_mut_for_test_v1());
            let mut terminal_usage = context.allocation_admission_usage_v1(device).unwrap();
            if let Some(usage) = &mut terminal_usage {
                assert_eq!(usage.retained_records, 1);
                assert_eq!(usage.quarantined_records, 0);
                usage.retained_records = 0;
                usage.quarantined_records = 1;
            }
            let mut destination = [0xff; 8];
            let result = catch_unwind(AssertUnwindSafe(|| {
                context.read_allocation(allocation, 0, &mut destination)
            }));
            if panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"scripted SDMA host read panic")
                );
                assert_eq!(context.is_terminal(), configured);
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(crate::RuntimeErrorV1::BackendTerminal(_))
                ));
                assert!(context.is_terminal());
            }
            assert_eq!(destination, [0xff; 8]);
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
                assert_eq!(usage.quarantined_records, 1);
                assert_eq!(usage.retained_records, 0);
            }
            assert!(
                context
                    .read_allocation(allocation, 0, &mut destination)
                    .is_err()
            );
            assert!(context.is_terminal());
            assert_eq!(
                context.allocation_admission_usage_v1(device).unwrap(),
                terminal_usage
            );
            let backend = context.backend_mut_for_test_v1();
            assert_eq!(observe_host(backend), original_host);
            assert!(backend.terminal);
            assert_eq!(backend.allocations.len(), 1);
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
fn sdma_host_read_and_synchronous_roots_abort_on_unfinished_drop() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_TEST_RUNTIME_SYNC_READ_DROP";
    const TEST: &str = "kfd_backend::tests::sdma_host_read_tests::sdma_host_read_and_synchronous_roots_abort_on_unfinished_drop";
    if let Some(mode) = std::env::var_os(CHILD) {
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        let (mut backend, _, _, device) = fixture(8, []);
        let stage = backend.scripted_sdma.as_ref().unwrap().test_host_owner(8);
        if mode == "read" {
            let driver = backend.scripted_sdma.take();
            assert!(
                catch_unwind(AssertUnwindSafe(
                    || backend.readback_and_recycle_transient_sdma_v1(stage, &mut [0; 8])
                ))
                .is_err()
            );
            backend.scripted_sdma = driver;
        } else {
            backend.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Synchronous(
                crate::kfd_backend::sdma_synchronous::SynchronousSdmaCustodyV1 {
                    allocation: device,
                    direction: Gfx942PersistentSdmaDirectionV1::HostToDevice,
                    request: DirectionalSdmaCopyRequestV1 {
                        host_offset: 0,
                        device_offset: 0,
                        copy_bytes: 8,
                    },
                    operation: "upload",
                    shell: None,
                    phase: crate::kfd_backend::sdma_synchronous::SynchronousSdmaPhaseV1::Host(
                        stage,
                    ),
                },
            ));
            assert!(!backend.terminal);
        }
        assert_eq!(
            backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
            3
        );
        assert_eq!(
            backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
            0
        );
        eprintln!("retained copy/read owner; dropping unfinished backend");
        drop(backend);
        panic!("unfinished copy/read Drop returned");
    }
    for mode in ["read", "synchronous"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, mode)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "{stderr}");
        assert!(stderr.contains("retained copy/read owner; dropping unfinished backend"));
        assert!(!stderr.contains("unfinished copy/read Drop returned"));
    }
}
