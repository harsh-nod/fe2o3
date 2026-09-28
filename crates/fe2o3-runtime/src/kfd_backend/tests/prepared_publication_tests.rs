//! Public admission and consuming-transition faults, without GPU execution.

use super::super::prepared_publication::ScriptedPreparedPublicationFaultV1 as Fault;
use super::prepared_cancellation_tests::{InputFacts, input_facts, profile_facts, shell_facts};
use super::*;
use std::mem::ManuallyDrop;
use std::os::unix::process::ExitStatusExt;
use std::panic::{AssertUnwindSafe, catch_unwind};

pub(super) fn inputs(backend: &KfdRuntimeBackendV1) -> Vec<InputFacts> {
    match backend.active.as_ref().unwrap().execution.as_ref().unwrap() {
        ActiveComputeExecutionV1::ScriptedPersistentPrepared {
            input: PreparedReceiptV1::Armed(input),
            ..
        } => vec![input_facts(input)],
        ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared {
            inputs: PreparedReceiptV1::Armed(inputs),
            ..
        } => inputs.iter().map(input_facts).collect(),
        _ => match backend.terminal_sdma_custody.as_ref().unwrap() {
            KfdRuntimeTerminalSdmaCustodyV1::PersistentRuntimeInput(input) => {
                vec![input_facts(input)]
            }
            KfdRuntimeTerminalSdmaCustodyV1::ThreeBindingPersistentInputs(inputs) => {
                inputs.iter().map(input_facts).collect()
            }
            _ => panic!("retained publication input custody"),
        },
    }
}

pub(super) fn shells(backend: &KfdRuntimeBackendV1) -> Vec<[usize; 4]> {
    match backend.active.as_ref().unwrap().execution.as_ref().unwrap() {
        ActiveComputeExecutionV1::ScriptedPersistentPrepared { allocation, .. }
        | ActiveComputeExecutionV1::ScriptedPersistent { allocation, .. } => backend.allocations
            [allocation]
            .persistent_storage_restore
            .as_ref()
            .map(shell_facts)
            .into_iter()
            .collect(),
        ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared {
            restore_shells, ..
        }
        | ActiveComputeExecutionV1::ScriptedThreeBindingPersistent { restore_shells, .. } => {
            restore_shells.iter().map(shell_facts).collect()
        }
        _ => panic!("scripted persistent phase"),
    }
}

// Publication changes only timing and the native phase, not this logical ledger.
pub(super) fn ledger(backend: &KfdRuntimeBackendV1) -> Vec<String> {
    let active = backend.active.as_ref().unwrap();
    let mut result = vec![
        format!(
            "active {} {} {:?} {} {} {} {:?} {:?} {:?} {:?} {} {} {:?}",
            active.id,
            active.stream,
            active.ordered_predecessor,
            active.deferred_ordered_predecessor_retain,
            active.kernel,
            active.dependency_depth,
            active.writebacks,
            active.resident_descriptors,
            active.ordinary_recipe,
            active.dispatch_shape_sha256,
            active.writebacks.as_ptr() as usize,
            active.resident_descriptors.as_ptr() as usize,
            backend.retained_persistent_dispatch
        ),
        format!(
            "ledger {} {} {:?} {:?} {:?} {:?} {:?}",
            backend.compute_completion_reservations,
            backend.staged_context_bytes,
            backend.event_submission_retain_counts,
            backend.stream_submission_tails,
            backend.stream_compute_lanes,
            backend.compute_module_retain_counts,
            backend.compute_dependency_retain_counts
        ),
    ];
    result.extend(
        active
            .allocations
            .iter()
            .map(|id| format!("active allocation {id}")),
    );
    for (id, custody) in &backend.allocation_custody {
        result.push(format!(
            "custody {id} {:?} {:?} {:?}",
            custody.owners, custody.owner_counts, custody.sole_stream
        ));
    }
    for (id, record) in backend.allocations.ordinary_iter() {
        result.push(format!(
            "allocation {id} {} {:?} {:?} {:?} {:?} {:?}",
            record.bytes.as_ptr() as usize,
            Sha256::digest(&record.bytes),
            record.content_sha256,
            record.native_dirty,
            record.sdma_shadow_dirty,
            record.sdma_storage
        ));
    }
    for (id, record) in &backend.submissions {
        result.push(format!(
            "result {id} {} {:?} {} {}",
            record.stream,
            record.status,
            record.dependency_depth,
            record.profile_dispatch_published
        ));
    }
    result.sort();
    result
}

fn inspect_case(case: usize) {
    let three = case >= 10;
    let case = case % 10;
    let (backend, submission, allocations) = if three {
        super::prepared_cancellation_tests::three_fixture()
    } else {
        let (backend, submission, allocation) = super::prepared_cancellation_tests::fixture();
        (backend, submission, vec![allocation])
    };
    let mut backend = ManuallyDrop::new(backend);
    let stream = backend.active.as_ref().unwrap().stream;
    let module = backend.kernels[&backend.active.as_ref().unwrap().kernel].module;
    let event = backend.record_event_v1(stream, submission).unwrap();
    let fault = match case {
        0 => Fault::Terminal,
        1 => Fault::Unwind,
        2 => Fault::ProfileUnwind,
        _ => Fault::Unwind,
    };
    backend.scripted_prepared_publication_fault = Some(fault);
    match case {
        3 => {
            backend
                .allocations
                .get_mut(&allocations[0])
                .unwrap()
                .sdma_storage = KfdRuntimeSdmaStorageV1::ComputeInFlight(0)
        }
        4 => {
            if three {
                let ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared {
                    restore_shells,
                    ..
                } = backend.active.as_mut().unwrap().execution.as_mut().unwrap()
                else {
                    unreachable!()
                };
                restore_shells[2].initialized = None;
            } else {
                backend
                    .allocations
                    .get_mut(&allocations[0])
                    .unwrap()
                    .persistent_storage_restore
                    .as_mut()
                    .unwrap()
                    .initialized = None;
            }
        }
        5 => {
            backend.compute_module_retain_counts.insert(module, 0);
        }
        6 => {
            backend.stream_compute_lanes.insert(stream, 1);
        }
        7 => {
            // Simulate a retained lower obligation, not a fabricated retry receipt.
            let custody = match backend.active.as_mut().unwrap().execution.as_mut().unwrap() {
                ActiveComputeExecutionV1::ScriptedPersistentPrepared { input, .. } => {
                    KfdRuntimeTerminalSdmaCustodyV1::PersistentRuntimeInput(
                        *core::mem::replace(input, PreparedReceiptV1::NativeOwned)
                            .into_armed()
                            .unwrap(),
                    )
                }
                ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared {
                    inputs, ..
                } => KfdRuntimeTerminalSdmaCustodyV1::ThreeBindingPersistentInputs(
                    core::mem::replace(inputs, PreparedReceiptV1::NativeOwned)
                        .into_armed()
                        .unwrap(),
                ),
                _ => unreachable!(),
            };
            backend.terminal_sdma_custody = Some(custody);
        }
        8 => {
            backend.stream_submission_tails.remove(&stream);
        }
        9 => backend.compute_completion_reservations = 0,
        _ => {}
    }
    let before = ledger(&backend);
    let before_inputs = inputs(&backend);
    let before_shells = shells(&backend);
    let before_profile = profile_facts(&backend);
    let before_live = backend.scripted_sdma.as_ref().unwrap().live_owner_count();
    let before_steps = backend.scripted_sdma.as_ref().unwrap().remaining_steps();
    let result = catch_unwind(AssertUnwindSafe(|| backend.poll_v1(submission)));
    if matches!(case, 1 | 2) {
        let expected = if case == 1 {
            "scripted consuming publication unwind"
        } else {
            "scripted persistent publication profile unwind"
        };
        assert_eq!(result.unwrap_err().downcast_ref::<&str>(), Some(&expected));
    } else {
        assert!(matches!(
            result,
            Ok(Err(RuntimeBackendFailureV1::Terminal(_)))
        ));
    }
    assert!(backend.terminal);
    assert_eq!(
        backend.active.as_ref().map(|active| active.id),
        Some(submission)
    );
    assert_eq!(ledger(&backend), before);
    assert_eq!(shells(&backend), before_shells);
    assert_eq!(backend.events[&event].submission, submission);
    if case == 2 {
        let devices: Vec<_> = match backend.active.as_ref().unwrap().execution.as_ref().unwrap() {
            ActiveComputeExecutionV1::ScriptedPersistent { device, .. } if !three => {
                vec![device.as_ref()]
            }
            ActiveComputeExecutionV1::ScriptedThreeBindingPersistent { devices, .. } if three => {
                devices.iter().collect()
            }
            _ => panic!("publication must be indexed before its observer"),
        };
        for (device, input) in devices.iter().zip(&before_inputs) {
            assert_eq!(device.scripted_owner_id().unwrap(), input.owner);
            assert_eq!(
                device.scripted_bytes().unwrap().as_ptr() as usize,
                input.bytes
            );
            assert_eq!(
                <[u8; 32]>::from(Sha256::digest(device.scripted_bytes().unwrap())),
                input.digest
            );
        }
        assert!(backend.terminal_sdma_custody.is_none());
    } else {
        assert_eq!(inputs(&backend), before_inputs);
        assert_eq!(profile_facts(&backend), before_profile);
        assert_eq!(
            backend.persistent_prepared_is_armed_v1(),
            !matches!(case, 0 | 1 | 7)
        );
    }
    assert_eq!(
        backend.scripted_prepared_publication_fault,
        if case < 3 { None } else { Some(fault) }
    );
    for operation in [
        backend.poll_v1(submission).map(|_| ()),
        backend.cancel_v1(submission).map(|_| ()),
        backend.shutdown_native_v1(),
    ] {
        assert!(matches!(
            operation,
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
    }
    assert_eq!(ledger(&backend), before);
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert_eq!(driver.live_owner_count(), before_live);
    assert_eq!(driver.remaining_steps(), before_steps);
    assert_eq!(driver.unexpected_drops(), 0);
    eprintln!("prepared publication custody inspected; dropping unrepaired backend");
    drop(ManuallyDrop::into_inner(backend));
    panic!("prepared publication Drop returned");
}

#[test]
fn prepared_publication_retains_exact_custody_across_faults() {
    const CHILD: &str = "FE2O3_TEST_PREPARED_PUBLICATION_CUSTODY";
    const TEST: &str = "kfd_backend::tests::prepared_publication_tests::prepared_publication_retains_exact_custody_across_faults";
    if let Ok(case) = std::env::var(CHILD) {
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        inspect_case(case.parse().unwrap());
        unreachable!();
    }
    for case in 0..20 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, case.to_string())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "case {case}: {stderr}");
        assert!(
            stderr.contains("prepared publication custody inspected; dropping unrepaired backend"),
            "case {case}: {stderr}"
        );
        eprintln!("verified prepared publication case {case}: retained custody; Drop SIGABRT");
    }
}

#[test]
fn prepared_publication_profiler_records_once_and_exhaustion_does_not_change_execution() {
    for capacity in [2, 128] {
        let byte_len = 4096;
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
        ];
        steps.extend(scripted_release_steps_v1());
        let (backend, stream, host, device) =
            scripted_direct_backend_configured_v1(byte_len, steps, |backend| {
                backend
                    .enable_profiler_v1(
                        KfdRuntimeProfilerConfigV1::new([0xd3; 32], capacity).unwrap(),
                    )
                    .unwrap();
            });
        let mut backend = ManuallyDrop::new(backend);
        backend
            .write_allocation_v1(host, 0, &vec![0xd3; byte_len])
            .unwrap();
        let (source, destination) = scripted_copy_regions_v1(host, device, byte_len as u64);
        let copy = backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();
        assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
        let module = backend
            .load_module_v1(7, &synthetic_cov6::module())
            .unwrap();
        let kernel = backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        backend.scripted_persistent_publication_retries = 1;
        let submission =
            submit_scripted_read_v1(&mut backend, stream, kernel, device, byte_len as u64, &[]);
        backend.flush_stream_v1(stream).unwrap();
        let before = inputs(&backend);
        for _ in 0..2 {
            backend.scripted_prepared_publication_fault = Some(Fault::Retryable);
            assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
            assert_eq!(inputs(&backend), before);
        }
        assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
        assert!(matches!(
            backend.active.as_ref().unwrap().execution,
            Some(ActiveComputeExecutionV1::ScriptedPersistent { .. })
        ));
        assert_eq!(
            backend.poll_v1(submission).unwrap(),
            BackendPollV1::Succeeded
        );
        backend.release_submission_v1(submission).unwrap();
        backend.release_submission_v1(copy).unwrap();
        backend.unload_module_v1(module).unwrap();
        release_scripted_direct_pair_v1(&mut backend, host, device);
        backend.destroy_stream_v1(stream).unwrap();
        let driver = backend.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted());
        assert_eq!(driver.live_owner_count(), 0);
        assert_eq!(driver.unexpected_drops(), 0);
        backend.shutdown_native_v1().unwrap();
        let capture = backend.finish_profiler_v1().unwrap();
        capture.validate().unwrap();
        let publications = capture
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event.event,
                    KfdRuntimeProfileEventKindV1::DispatchPublished { .. }
                )
            })
            .count();
        assert_eq!(publications, usize::from(capacity == 128));
        assert_eq!(
            capture.coverage.complete_runtime_operation_history,
            capacity == 128
        );
        assert_eq!(capture.coverage.dropped_events == 0, capacity == 128);
        drop(ManuallyDrop::into_inner(backend));
    }
}
