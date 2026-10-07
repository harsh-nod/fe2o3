//! CPU custody witnesses; these do not fabricate native completion receipts.

use super::*;
use crate::kfd_backend::tests::{initial_publication_tests, prepared_publication_tests};

fn submit(f: &mut Fixture, access: RuntimeAccessV1) -> u64 {
    let mut kernarg = [0_u8; 16];
    kernarg[8..].copy_from_slice(&13_u64.to_le_bytes());
    f.backend
        .submit_v1(BackendLaunchV1 {
            stream: f.stream,
            kernel: f.kernel,
            explicit_kernarg: &kernarg,
            bindings: &[BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation: f.device,
                    access,
                    byte_offset: 0,
                    byte_len: 4096,
                },
                kernarg_byte_offset: 0,
            }],
            dependencies: &[f.event],
            geometry: crate::RuntimeLaunchGeometryV1 {
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_shared_bytes: 0,
            },
            semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
        })
        .unwrap()
}

fn published(f: &Fixture) -> (u64, usize, usize, [u8; 32], usize) {
    let Some(ActiveComputeExecutionV1::ScriptedPersistent {
        device, completion, ..
    }) = f.backend.active.as_ref().unwrap().execution.as_ref()
    else {
        panic!("scalar scripted publication");
    };
    let bytes = device.scripted_bytes().unwrap();
    (
        device.scripted_owner_id().unwrap(),
        bytes.as_ptr() as usize,
        device.as_ref() as *const _ as usize,
        Sha256::digest(bytes).into(),
        completion.as_ref() as *const _ as usize,
    )
}

#[test]
fn scalar_completion_reserves_only_missing_native_shells() {
    for origin in 0..3 {
        for access in [
            RuntimeAccessV1::Read,
            RuntimeAccessV1::Write,
            RuntimeAccessV1::ReadWrite,
        ] {
            let mut f = Fixture::new(origin);
            let before = f.stored();
            let admission = PersistentFullRangeComputeAdmissionV1 {
                access,
                ..f.admission()
            };
            let (root, allocations) = counted_allocations_for_test_v1(|| {
                f.backend.reserve_persistent_completion_v1(admission)
            });
            let root = root.unwrap();
            let needs_replay = origin == 0 && access != RuntimeAccessV1::Read;
            assert_eq!(allocations, 1 + usize::from(needs_replay));
            assert_eq!(root.shell.is_some(), needs_replay);
            if let Some(shell) = &root.shell {
                assert!(
                    shell.ready.is_none() && shell.device.is_none() && shell.initialized.is_none()
                );
                assert!(shell.replay.is_some());
            }
            assert_eq!(f.stored(), before);
            drop(root);
            f.finish(&[]);
        }
    }
}

#[test]
fn scalar_completion_reuses_published_box_without_allocating() {
    for origin in 0..3 {
        for access in [
            RuntimeAccessV1::Read,
            RuntimeAccessV1::Write,
            RuntimeAccessV1::ReadWrite,
        ] {
            if origin == 2 && access != RuntimeAccessV1::Read {
                continue;
            }
            let mut f = Fixture::new(origin);
            let mut completed = Vec::new();
            for _ in 0..3 {
                let original_box = f.stored().1;
                let id = submit(&mut f, access);
                f.backend.flush_stream_v1(f.stream).unwrap();
                let before = published(&f);
                let Some(ActiveComputeExecutionV1::ScriptedPersistent { completion, .. }) =
                    f.backend.active.as_ref().unwrap().execution.as_ref()
                else {
                    unreachable!()
                };
                if let Some(shell) = &completion.shell {
                    let boxes = shell_facts(shell);
                    let inherited = if completion.admission.source
                        == PersistentFullRangeComputeSourceV1::AuthenticatedH2d
                    {
                        boxes[0]
                    } else {
                        boxes[1]
                    };
                    assert_eq!(inherited, original_box);
                }
                let (result, allocations) =
                    counted_allocations_for_test_v1(|| f.backend.poll_v1(id));
                assert_eq!(result.unwrap(), BackendPollV1::Succeeded);
                assert_eq!(allocations, 0, "origin {origin}, access {access:?}");
                let after = f.stored();
                assert_eq!(
                    (after.0.owner, after.0.bytes, after.1, after.0.digest),
                    (before.0, before.1, before.2, before.3)
                );
                assert!(f.backend.active.is_none());
                assert_eq!(f.backend.compute_completion_reservations, 0);
                assert!(
                    f.backend.allocations[&f.device]
                        .persistent_storage_restore
                        .is_none()
                );
                if access != RuntimeAccessV1::Read {
                    assert_eq!(f.backend.allocations[&f.device].content_sha256, None);
                    assert!(f.backend.allocations[&f.device].sdma_shadow_dirty);
                }
                completed.push(id);
            }
            f.finish(&completed);
        }
    }
}

#[test]
fn scalar_completion_timeout_preserves_owner_root_and_ledger() {
    for origin in 0..3 {
        let mut f = Fixture::new(origin);
        let id = f.submit();
        f.backend.flush_stream_v1(f.stream).unwrap();
        let before = published(&f);
        let ledger = prepared_publication_tests::ledger(&f.backend);
        f.backend.scripted_persistent_wait_pending_observations = 1;
        let (result, allocations) =
            counted_allocations_for_test_v1(|| f.backend.wait_v1(id, Instant::now()));
        assert_eq!(result.unwrap(), BackendPollV1::Pending);
        assert_eq!(allocations, 0);
        assert_eq!(published(&f), before);
        assert_eq!(prepared_publication_tests::ledger(&f.backend), ledger);
        assert_eq!(
            f.backend
                .wait_v1(id, Instant::now() + Duration::from_secs(1))
                .unwrap(),
            BackendPollV1::Succeeded
        );
        f.finish(&[id]);
    }
}

#[test]
fn scalar_completion_reservation_failure_precedes_input_extraction() {
    for origin in 0..3 {
        let mut f = Fixture::new(origin);
        let before = f.stored();
        let metadata = f.metadata();
        let steps = f.backend.scripted_sdma.as_ref().unwrap().remaining_steps();
        let id = f.submit();
        f.backend.scripted_persistent_transition_failure =
            Some(ScriptedPersistentTransitionFailureV1::ReserveCompletion);
        assert!(matches!(
            f.backend.flush_stream_v1(f.stream),
            Err(RuntimeBackendFailureV1::Quiescent(_))
        ));
        assert!(!f.backend.terminal);
        assert!(f.backend.active.is_none());
        assert_eq!(f.stored(), before);
        assert_eq!(f.metadata(), metadata);
        assert_eq!(
            f.backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
            steps
        );
        assert_eq!(f.backend.compute_completion_reservations, 0);
        assert_eq!(
            f.backend.poll_v1(id).unwrap(),
            BackendPollV1::Failed { code: -1 }
        );
        f.finish(&[id]);
    }
}

fn inspect_preflight(case: usize) {
    let mut f = Fixture::new(case / 7);
    let fault = case % 7;
    let id = f.submit();
    f.backend.flush_stream_v1(f.stream).unwrap();
    let before = published(&f);
    let performance = f.backend.last_launch_performance_v1();
    let active = f.backend.active.as_mut().unwrap();
    let Some(ActiveComputeExecutionV1::ScriptedPersistent { completion, .. }) =
        active.execution.as_mut()
    else {
        unreachable!()
    };
    match fault {
        0 => completion.admission.allocation = f.host,
        1 => completion.admission.access = RuntimeAccessV1::Write,
        2 => completion.receipt = PersistentCompletionReceiptV1::NativeOwned,
        3 => active.allocations.clear(),
        4 => f.backend.compute_completion_reservations = 0,
        5 => {
            f.backend.allocation_custody.remove(&f.device);
        }
        6 => {
            f.backend.scripted_persistent_transition_failure =
                Some(ScriptedPersistentTransitionFailureV1::CompletionCommitRosterMismatch)
        }
        _ => unreachable!(),
    }
    let mut ledger = prepared_publication_tests::ledger(&f.backend);
    if fault == 6 {
        ledger.retain(|line| {
            !line.starts_with("active allocation ") && !line.starts_with("allocation ")
        });
    }
    assert!(matches!(
        f.backend.poll_v1(id),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    let mut after = prepared_publication_tests::ledger(&f.backend);
    if fault == 6 {
        after.retain(|line| {
            !line.starts_with("active allocation ") && !line.starts_with("allocation ")
        });
        assert!(f.backend.active.as_ref().unwrap().allocations.is_empty());
        assert!(matches!(
            f.backend.persistent_completion_root_v1().receipt,
            PersistentCompletionReceiptV1::Restored
        ));
        let stored = f.stored();
        assert_eq!(
            (stored.0.owner, stored.0.bytes, stored.1, stored.0.digest),
            (before.0, before.1, before.2, before.3)
        );
    } else {
        assert_eq!(published(&f), before);
        assert!(
            matches!(f.backend.allocations[&f.device].sdma_storage, KfdRuntimeSdmaStorageV1::ComputeInFlight(actual) if actual == id)
        );
    }
    assert_eq!(after, ledger);
    assert_eq!(f.backend.last_launch_performance_v1(), performance);
    assert!(!f.backend.submissions.contains_key(&id));
    assert!(f.backend.terminal_sdma_custody.is_none());
    assert!(f.backend.terminal);
    assert_eq!(
        f.backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
        2
    );
    assert_eq!(
        f.backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
        0
    );
    eprintln!("scalar preflight custody inspected; dropping unrepaired backend");
    drop(ManuallyDrop::into_inner(f.backend));
    panic!("scalar preflight Drop returned");
}

#[test]
fn scalar_completion_preflights_preserve_malformed_custody() {
    const CHILD: &str = "FE2O3_TEST_SCALAR_COMPLETION_PREFLIGHT";
    const TEST: &str = "kfd_backend::tests::bind_recovery_tests::completion::scalar_completion_preflights_preserve_malformed_custody";
    if let Ok(case) = std::env::var(CHILD) {
        // Piped crash handlers ignore RLIMIT_CORE; keep deliberate aborts local.
        rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
            .unwrap();
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        inspect_preflight(case.parse().unwrap());
        unreachable!();
    }
    for case in 0..21 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, case.to_string())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "case {case}: {stderr}");
        assert!(
            stderr.contains("scalar preflight custody inspected; dropping unrepaired backend"),
            "case {case}: {stderr}"
        );
        eprintln!("verified scalar preflight case {case}: retained custody; Drop SIGABRT");
    }
}

fn inspect(origin: usize, fault: usize, wait: bool) {
    let mut f = Fixture::new(origin);
    let id = f.submit();
    f.backend.flush_stream_v1(f.stream).unwrap();
    let before = published(&f);
    let event = f.backend.record_event_v1(f.stream, id).unwrap();
    let successor =
        submit_scripted_read_v1(&mut f.backend, f.stream, f.kernel, f.device, 4096, &[event]);
    let pending = initial_publication_tests::pending_facts(&f.backend.pending_compute[&successor]);
    let fifo = f.backend.pending_compute_streams.clone();
    let mut ledger = prepared_publication_tests::ledger(&f.backend);
    // This fault deliberately changes only the marker; all accounting remains exact.
    if fault == 4 {
        ledger.retain(|line| !line.starts_with("allocation "));
    }
    let performance = f.backend.last_launch_performance_v1();
    let steps = f.backend.scripted_sdma.as_ref().unwrap().remaining_steps();
    f.backend.scripted_persistent_transition_failure = Some(
        [
            ScriptedPersistentTransitionFailureV1::Poll,
            ScriptedPersistentTransitionFailureV1::Recycle,
            ScriptedPersistentTransitionFailureV1::Detach,
            ScriptedPersistentTransitionFailureV1::UnwindAfterRetire,
            ScriptedPersistentTransitionFailureV1::CompletionSlotMismatch,
            ScriptedPersistentTransitionFailureV1::CompletionShellMismatch,
            ScriptedPersistentTransitionFailureV1::CompletionEffectMismatch,
        ][fault],
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if wait {
            f.backend
                .wait_v1(id, Instant::now() + Duration::from_secs(1))
        } else {
            f.backend.poll_v1(id)
        }
    }));
    if fault == 3 {
        let payload = result.unwrap_err();
        assert_eq!(
            payload.downcast_ref::<&str>(),
            Some(&"scripted scalar completion unwind after retirement")
        );
    } else {
        assert!(matches!(
            result.unwrap(),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
    }
    assert!(f.backend.terminal);
    assert_eq!(f.backend.active.as_ref().unwrap().id, id);
    assert!(f.backend.persistent_compute_is_active_v1());
    let root = f.backend.persistent_completion_root_v1();
    assert_eq!(root as *const _ as usize, before.4);
    let (owner, bytes) = if fault < 3 {
        assert!(matches!(
            root.receipt,
            PersistentCompletionReceiptV1::TerminalRooted
        ));
        let Some(KfdRuntimeTerminalSdmaCustodyV1::Device(device)) =
            f.backend.terminal_sdma_custody.as_ref()
        else {
            panic!("terminal owner")
        };
        (
            device.scripted_owner_id().unwrap(),
            device.scripted_bytes().unwrap(),
        )
    } else {
        assert!(f.backend.terminal_sdma_custody.is_none());
        let PersistentCompletionReceiptV1::Retired(
            KfdRuntimePersistentComputeInputV1::ScriptedReplay(device),
            _,
        ) = &root.receipt
        else {
            panic!("indexed retired input")
        };
        (
            device.scripted_owner_id().unwrap(),
            device.scripted_bytes().unwrap(),
        )
    };
    assert_eq!(
        (
            owner,
            bytes.as_ptr() as usize,
            <[u8; 32]>::from(Sha256::digest(bytes))
        ),
        (before.0, before.1, before.3)
    );
    let mut after = prepared_publication_tests::ledger(&f.backend);
    if fault == 4 {
        after.retain(|line| !line.starts_with("allocation "));
    }
    assert_eq!(after, ledger);
    assert_eq!(
        initial_publication_tests::pending_facts(&f.backend.pending_compute[&successor]),
        pending
    );
    assert_eq!(f.backend.pending_compute_streams, fifo);
    assert_eq!(f.backend.last_launch_performance_v1(), performance);
    assert!(!f.backend.submissions.contains_key(&id));
    assert!(
        matches!(f.backend.allocations[&f.device].sdma_storage, KfdRuntimeSdmaStorageV1::ComputeInFlight(actual) if actual == if fault == 4 { u64::MAX } else { id })
    );
    assert!(matches!(
        f.backend.shutdown_native_v1(),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    let driver = f.backend.scripted_sdma.as_ref().unwrap();
    assert_eq!(driver.remaining_steps(), steps);
    assert_eq!(driver.live_owner_count(), 2);
    assert_eq!(driver.unexpected_drops(), 0);
    eprintln!("scalar completion exact custody inspected; dropping unrepaired backend");
    drop(ManuallyDrop::into_inner(f.backend));
    panic!("scalar completion Drop returned");
}

fn inspect_prepared(case: usize) {
    let mut f = Fixture::new(case / 6);
    f.backend.scripted_persistent_publication_retries = 1;
    let id = f.submit();
    f.backend.flush_stream_v1(f.stream).unwrap();
    let Some(ActiveComputeExecutionV1::ScriptedPersistentPrepared {
        input, completion, ..
    }) = f.backend.active.as_mut().unwrap().execution.as_mut()
    else {
        panic!("prepared scalar")
    };
    let before_input = input_facts(input.armed().unwrap());
    let root_address = completion.as_ref() as *const _ as usize;
    match case % 3 {
        0 => completion.receipt = PersistentCompletionReceiptV1::NativeOwned,
        1 => completion.admission.allocation = f.host,
        2 => completion.admission.access = RuntimeAccessV1::Write,
        _ => unreachable!(),
    }
    let ledger = prepared_publication_tests::ledger(&f.backend);
    let steps = f.backend.scripted_sdma.as_ref().unwrap().remaining_steps();
    let result = if case % 6 < 3 {
        f.backend.poll_v1(id).map(|_| ())
    } else {
        f.backend.cancel_v1(id).map(|_| ())
    };
    assert!(matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))));
    assert_eq!(prepared_publication_tests::ledger(&f.backend), ledger);
    let Some(ActiveComputeExecutionV1::ScriptedPersistentPrepared {
        input, completion, ..
    }) = f.backend.active.as_ref().unwrap().execution.as_ref()
    else {
        panic!("original prepared receipt remains indexed")
    };
    assert_eq!(input_facts(input.armed().unwrap()), before_input);
    assert_eq!(completion.as_ref() as *const _ as usize, root_address);
    assert_eq!(
        f.backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
        steps
    );
    assert_eq!(
        f.backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
        0
    );
    assert_eq!(
        f.backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
        2
    );
    assert!(f.backend.terminal);
    eprintln!("scalar prepared completion custody inspected; dropping unrepaired backend");
    drop(ManuallyDrop::into_inner(f.backend));
    panic!("scalar prepared completion Drop returned");
}

#[test]
fn scalar_completion_prepared_publication_and_cancellation_validate_root() {
    const CHILD: &str = "FE2O3_TEST_SCALAR_COMPLETION_PREPARED";
    const TEST: &str = "kfd_backend::tests::bind_recovery_tests::completion::scalar_completion_prepared_publication_and_cancellation_validate_root";
    if let Ok(case) = std::env::var(CHILD) {
        // Piped crash handlers ignore RLIMIT_CORE; keep deliberate aborts local.
        rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
            .unwrap();
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        inspect_prepared(case.parse().unwrap());
        unreachable!();
    }
    for case in 0..18 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, case.to_string())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "case {case}: {stderr}");
        assert!(
            stderr.contains(
                "scalar prepared completion custody inspected; dropping unrepaired backend"
            ),
            "case {case}: {stderr}"
        );
        eprintln!(
            "verified scalar prepared completion case {case}: retained custody; Drop SIGABRT"
        );
    }
}

#[test]
fn scalar_completion_faults_preserve_indexed_custody_until_process_teardown() {
    const CHILD: &str = "FE2O3_TEST_SCALAR_COMPLETION_CUSTODY";
    const TEST: &str = "kfd_backend::tests::bind_recovery_tests::completion::scalar_completion_faults_preserve_indexed_custody_until_process_teardown";
    if let Ok(case) = std::env::var(CHILD) {
        // Piped crash handlers ignore RLIMIT_CORE; keep deliberate aborts local.
        rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
            .unwrap();
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        let case: usize = case.parse().unwrap();
        inspect(case / 14, case % 7, case % 14 >= 7);
        unreachable!();
    }
    for case in 0..42 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, case.to_string())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "case {case}: {stderr}");
        assert!(
            stderr
                .contains("scalar completion exact custody inspected; dropping unrepaired backend"),
            "case {case}: {stderr}"
        );
        eprintln!(
            "verified scalar completion case {case}: exact input and Active retained; Drop SIGABRT"
        );
    }
}
