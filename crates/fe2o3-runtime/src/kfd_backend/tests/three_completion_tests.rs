//! Three-binding completion custody, using public scripted admission.

use super::*;
use crate::kfd_backend::tests::prepared_cancellation_tests::{input_facts, shell_facts};
use crate::kfd_backend::three_binding_completion::ScriptedThreeCompletionFaultV1 as CompletionFault;
use std::mem::ManuallyDrop;

#[test]
fn three_completion_slot_failure_retains_active() {
    let (backend, submission, allocations) = prepared_cancellation_tests::three_fixture();
    let mut backend = ManuallyDrop::new(backend);
    assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
    backend
        .allocations
        .get_mut(&allocations[2])
        .unwrap()
        .sdma_storage = KfdRuntimeSdmaStorageV1::ComputeInFlight(u64::MAX);
    assert!(matches!(
        backend.poll_v1(submission),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert_eq!(
        backend.active.as_ref().map(|active| active.id),
        Some(submission)
    );
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
        0
    );
    disarm_scripted_drop_after_inspection_v1(&mut backend);
    drop(ManuallyDrop::into_inner(backend));
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DeviceFacts {
    owner: u64,
    bytes: usize,
    digest: [u8; 32],
}

impl DeviceFacts {
    fn from_input(input: InputFacts) -> Self {
        Self {
            owner: input.owner,
            bytes: input.bytes,
            digest: input.digest,
        }
    }

    fn device(device: &DirectionalSdmaDeviceOwnerV1) -> Self {
        let bytes = device.scripted_bytes().unwrap();
        Self {
            owner: device.scripted_owner_id().unwrap(),
            bytes: bytes.as_ptr() as usize,
            digest: Sha256::digest(bytes).into(),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct PublishedFacts {
    root: usize,
    owners: [DeviceFacts; 3],
    shells: [[usize; 4]; 3],
    admissions: [PersistentFullRangeComputeAdmissionV1; 3],
}

fn published(f: &Fixture<3>) -> PublishedFacts {
    let Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistent {
        devices,
        restore_shells,
        admissions,
        completion,
    }) = f.backend.active.as_ref().unwrap().execution.as_ref()
    else {
        panic!("published three-binding scripted receipt");
    };
    PublishedFacts {
        root: completion.as_ref() as *const _ as usize,
        owners: std::array::from_fn(|index| DeviceFacts::device(&devices[index])),
        shells: std::array::from_fn(|index| shell_facts(&restore_shells[index])),
        admissions: *admissions,
    }
}

fn complete(
    f: &mut Fixture<3>,
    wait: bool,
) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    if wait {
        f.backend
            .wait_v1(f.first, Instant::now() + Duration::from_secs(1))
    } else {
        f.backend.poll_v1(f.first)
    }
}

fn assert_origin(admissions: [PersistentFullRangeComputeAdmissionV1; 3], origin: usize) {
    assert_eq!(
        admissions.map(|admission| admission.source),
        [[
            PersistentFullRangeComputeSourceV1::AuthenticatedH2d,
            PersistentFullRangeComputeSourceV1::InitializedStorage,
            PersistentFullRangeComputeSourceV1::RetainedControlReplay,
        ][origin]; 3]
    );
}

fn assert_restored(f: &Fixture<3>, index: usize, before: &PublishedFacts) {
    assert_eq!(
        DeviceFacts::from_input(stored_input(&f.backend, f.allocations[index])),
        before.owners[index]
    );
    let record = &f.backend.allocations[&f.allocations[index]];
    let (address, shell) = match &record.sdma_storage {
        KfdRuntimeSdmaStorageV1::H2dReady(ready) => {
            assert_eq!(
                record.content_sha256,
                Some(ready.owner.authenticated_sha256())
            );
            (ready.as_ref() as *const _ as usize, 0)
        }
        KfdRuntimeSdmaStorageV1::Device(device) => (device.as_ref() as *const _ as usize, 1),
        _ => panic!("restored scripted input"),
    };
    assert_eq!(address, before.shells[index][shell]);
    assert!(record.persistent_storage_restore.is_none());
    if index == 2 {
        assert_eq!(record.content_sha256, None);
        assert!(record.sdma_shadow_dirty);
    }
}

fn assert_committed(f: &Fixture<3>) {
    let b = &f.backend;
    assert_eq!(b.submissions[&f.first].status, BackendPollV1::Succeeded);
    assert_eq!(b.compute_completion_reservations, 1);
    assert_eq!(b.compute_module_retain_counts[&f.module], 1);
    assert_eq!(b.compute_dependency_retain_counts[&f.copy], 1);
    assert_eq!(b.compute_dependency_retain_counts[&f.first], 1);
    assert!(!b.stream_compute_lanes.contains_key(&f.stream));
    for allocation in f.allocations {
        let owners = &b.allocation_custody[&allocation].owners;
        assert_eq!(owners.len(), 1);
        assert_eq!(owners[0].submission, f.trailing);
    }
}

#[test]
fn three_completion_poll_and_wait_restore_without_allocating() {
    for origin in 0..3 {
        for wait in [false, true] {
            let mut f = Fixture::<3>::new_origin(origin);
            f.backend.flush_stream_v1(f.stream).unwrap();
            f.assert_handoff();
            let before = published(&f);
            assert_origin(before.admissions, origin);
            let trailing = pending_facts(&f.backend.pending_compute[&f.trailing]);
            let (result, allocations) = counted_allocations_for_test_v1(|| complete(&mut f, wait));
            assert_eq!(result.unwrap(), BackendPollV1::Succeeded);
            assert_eq!(allocations, 0, "origin {origin}, wait {wait}");
            assert!(f.backend.active.is_none());
            assert_committed(&f);
            for index in 0..3 {
                assert_restored(&f, index, &before);
            }
            assert_eq!(
                pending_facts(&f.backend.pending_compute[&f.trailing]),
                trailing
            );
            f.finish(false);
        }
    }
}

#[test]
fn three_completion_pending_observations_preserve_exact_root_and_ledger() {
    for origin in 0..3 {
        let mut f = Fixture::<3>::new_origin(origin);
        f.backend.flush_stream_v1(f.stream).unwrap();
        let before = published(&f);
        assert_origin(before.admissions, origin);
        let before_ledger = ledger(&f.backend);
        f.backend.scripted_persistent_poll_pending_observations = 1;
        assert_eq!(f.backend.poll_v1(f.first).unwrap(), BackendPollV1::Pending);
        assert_eq!(published(&f), before);
        assert_eq!(ledger(&f.backend), before_ledger);
        f.backend.scripted_persistent_wait_pending_observations = 1;
        let (result, allocations) =
            counted_allocations_for_test_v1(|| f.backend.wait_v1(f.first, Instant::now()));
        assert_eq!(result.unwrap(), BackendPollV1::Pending);
        assert_eq!(allocations, 0);
        assert_eq!(published(&f), before);
        assert_eq!(ledger(&f.backend), before_ledger);
        assert_eq!(f.backend.scripted_persistent_wait_observations, 1);
        f.finish(false);
    }
}

const FAULTS: [CompletionFault; 21] = [
    CompletionFault::Poll,
    CompletionFault::Detach,
    CompletionFault::Retire,
    CompletionFault::AfterRetirement,
    CompletionFault::Slot(0),
    CompletionFault::Slot(1),
    CompletionFault::Slot(2),
    CompletionFault::Shell(0),
    CompletionFault::Shell(1),
    CompletionFault::Shell(2),
    CompletionFault::Effect(0),
    CompletionFault::Effect(1),
    CompletionFault::Effect(2),
    CompletionFault::AfterRestore(0),
    CompletionFault::AfterRestore(1),
    CompletionFault::AfterRestore(2),
    CompletionFault::CommitRoster,
    CompletionFault::ControlMissing,
    CompletionFault::ControlFailure,
    CompletionFault::ControlUnwind,
    CompletionFault::ProfileUnwind,
];

fn inspect_completion(case: usize) {
    let origin = case / 42;
    let wait = case % 42 >= 21;
    let fault = FAULTS[case % 21];
    let mut f = Fixture::<3>::new_origin(origin);
    f.backend.flush_stream_v1(f.stream).unwrap();
    f.assert_handoff();
    let before = published(&f);
    assert_origin(before.admissions, origin);
    let trailing = pending_facts(&f.backend.pending_compute[&f.trailing]);
    let recipe = Arc::downgrade(&f.backend.pending_compute[&f.trailing].launch);
    let fifo = f.backend.pending_compute_streams.clone();
    let before_ledger = ledger(&f.backend);
    let metadata = f.allocations.map(|id| {
        let r = &f.backend.allocations[&id];
        (
            r.content_sha256,
            r.native_dirty.clone(),
            r.sdma_shadow_dirty,
        )
    });
    let live = f.backend.scripted_sdma.as_ref().unwrap().live_owner_count();
    let steps = f.backend.scripted_sdma.as_ref().unwrap().remaining_steps();
    let performance = f.backend.last_launch_performance_v1();
    f.backend.scripted_three_completion_fault = Some(fault);
    let result = catch_unwind(AssertUnwindSafe(|| complete(&mut f, wait)));
    let panic = match fault {
        CompletionFault::AfterRetirement => Some("scripted three-binding retired-input unwind"),
        CompletionFault::AfterRestore(_) => {
            Some("scripted three-binding restoration-prefix unwind")
        }
        CompletionFault::ControlUnwind => Some("scripted detached-control release unwind"),
        CompletionFault::ProfileUnwind => Some("scripted three-binding completion-report unwind"),
        _ => None,
    };
    if let Some(message) = panic {
        assert_eq!(result.unwrap_err().downcast_ref::<&str>(), Some(&message));
    } else {
        assert!(matches!(
            result.unwrap(),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
    }
    assert!(f.backend.terminal);
    assert_eq!(f.backend.active.as_ref().unwrap().id, f.first);
    assert!(f.backend.persistent_compute_is_active_v1());
    assert!(f.backend.terminal_sdma_custody.is_none());
    let root = f.backend.three_completion_root_v1();
    assert_eq!(root as *const _ as usize, before.root);
    assert_eq!(root.admissions, before.admissions);
    let prefix = match fault {
        CompletionFault::AfterRestore(index) => index + 1,
        CompletionFault::CommitRoster
        | CompletionFault::ControlMissing
        | CompletionFault::ControlFailure
        | CompletionFault::ControlUnwind
        | CompletionFault::ProfileUnwind => 3,
        _ => 0,
    };
    let expected_control = match fault {
        CompletionFault::ControlMissing
        | CompletionFault::ControlFailure
        | CompletionFault::ControlUnwind => ThreeCompletionControlV1::Releasing,
        CompletionFault::ProfileUnwind => ThreeCompletionControlV1::Released,
        _ => ThreeCompletionControlV1::Pending,
    };
    assert_eq!(root.control, expected_control);
    for (index, original_metadata) in metadata.iter().enumerate() {
        if index < prefix {
            assert!(root.shells[index].is_none());
            assert_restored(&f, index, &before);
        } else {
            if fault == CompletionFault::Shell(index) {
                assert!(root.shells[index].is_none());
            } else {
                assert_eq!(
                    shell_facts(root.shells[index].as_ref().unwrap()),
                    before.shells[index]
                );
            }
            let record = &f.backend.allocations[&f.allocations[index]];
            let expected_marker = if fault == CompletionFault::Slot(index) {
                u64::MAX
            } else {
                f.first
            };
            assert!(
                matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::ComputeInFlight(id) if id == expected_marker)
            );
            assert_eq!(
                (
                    record.content_sha256,
                    record.native_dirty.clone(),
                    record.sdma_shadow_dirty
                ),
                *original_metadata
            );
        }
        if index < prefix && index < 2 {
            let record = &f.backend.allocations[&f.allocations[index]];
            assert_eq!(
                (
                    record.content_sha256,
                    record.native_dirty.clone(),
                    record.sdma_shadow_dirty
                ),
                *original_metadata
            );
        }
    }
    match &root.receipt {
        ThreeCompletionReceiptV1::Scripted(ScriptedCompletionOwnersV1::Three(devices)) => {
            assert!(matches!(
                fault,
                CompletionFault::Poll | CompletionFault::Detach | CompletionFault::Retire
            ));
            assert_eq!(
                std::array::from_fn::<_, 3, _>(|i| DeviceFacts::device(&devices[i])),
                before.owners
            );
        }
        ThreeCompletionReceiptV1::Retired(inputs, effects) => {
            assert!(matches!(
                fault,
                CompletionFault::AfterRetirement
                    | CompletionFault::AfterRestore(_)
                    | CompletionFault::Slot(_)
                    | CompletionFault::Shell(_)
                    | CompletionFault::Effect(_)
            ));
            for index in 0..3 {
                if index < prefix {
                    assert!(inputs[index].is_none());
                } else {
                    assert_eq!(
                        DeviceFacts::from_input(input_facts(inputs[index].as_ref().unwrap())),
                        before.owners[index]
                    );
                }
                let effect = if fault == CompletionFault::Effect(index) {
                    persistent_compute_effect_v1(if index == 2 {
                        RuntimeAccessV1::Read
                    } else {
                        RuntimeAccessV1::Write
                    })
                } else {
                    persistent_compute_effect_v1(before.admissions[index].access)
                };
                assert_eq!(effects[index], effect);
            }
        }
        ThreeCompletionReceiptV1::Restored => assert_eq!(prefix, 3),
        _ => panic!("unexpected scripted completion phase"),
    }
    if fault == CompletionFault::ProfileUnwind {
        assert_committed(&f);
        assert_eq!(
            f.backend.last_launch_performance_v1(),
            Some(f.backend.active.as_ref().unwrap().performance)
        );
    } else {
        let filter = |mut lines: Vec<String>| {
            lines.retain(|line| {
                !(line.starts_with("allocation ")
                    || fault == CompletionFault::CommitRoster
                        && line.starts_with("active allocation "))
            });
            lines
        };
        assert_eq!(filter(ledger(&f.backend)), filter(before_ledger));
        assert_eq!(f.backend.last_launch_performance_v1(), performance);
        assert!(!f.backend.submissions.contains_key(&f.first));
    }
    assert_eq!(
        pending_facts(&f.backend.pending_compute[&f.trailing]),
        trailing
    );
    assert!(recipe.upgrade().is_some());
    assert_eq!(f.backend.pending_compute_streams, fifo);
    assert!(matches!(
        f.backend.shutdown_native_v1(),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    let driver = f.backend.scripted_sdma.as_ref().unwrap();
    assert_eq!(driver.remaining_steps(), steps);
    assert_eq!(driver.live_owner_count(), live);
    assert_eq!(driver.unexpected_drops(), 0);
    eprintln!("three completion custody inspected; dropping unrepaired backend");
    drop(ManuallyDrop::into_inner(f.backend));
    panic!("terminal completion Drop returned");
}

fn subprocess_matrix(test: &str, variable: &str, count: usize, inspect: fn(usize), marker: &str) {
    let test = test.strip_prefix("fe2o3_runtime::").unwrap();
    if let Ok(case) = std::env::var(variable) {
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
        inspect(case.parse().unwrap());
        unreachable!();
    }
    for case in 0..count {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test, "--nocapture"])
            .env(variable, case.to_string())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "case {case}: {stderr}");
        assert!(
            !output.status.core_dumped(),
            "case {case}: unexpected core dump"
        );
        assert!(stderr.contains(marker), "case {case}: {stderr}");
        eprintln!("verified {variable} case {case}: custody inspected; Drop SIGABRT");
    }
}

#[test]
fn three_completion_faults_preserve_custody_until_process_teardown() {
    subprocess_matrix(
        concat!(
            module_path!(),
            "::three_completion_faults_preserve_custody_until_process_teardown"
        ),
        "FE2O3_TEST_THREE_COMPLETION_CUSTODY",
        126,
        inspect_completion,
        "three completion custody inspected; dropping unrepaired backend",
    );
}

#[test]
fn three_completion_replays_queued_successor_with_full_cleanup() {
    for origin in 0..3 {
        for wait in [false, true] {
            let mut f = Fixture::<3>::new_origin(origin);
            f.backend.flush_stream_v1(f.stream).unwrap();
            let original = published(&f);
            assert_eq!(complete(&mut f, wait).unwrap(), BackendPollV1::Succeeded);
            f.backend.flush_stream_v1(f.stream).unwrap();
            assert_eq!(f.backend.active.as_ref().unwrap().id, f.trailing);
            let replay = published(&f);
            assert_eq!(replay.owners, original.owners);
            let (result, allocations) = counted_allocations_for_test_v1(|| {
                if wait {
                    f.backend
                        .wait_v1(f.trailing, Instant::now() + Duration::from_secs(1))
                } else {
                    f.backend.poll_v1(f.trailing)
                }
            });
            assert_eq!(result.unwrap(), BackendPollV1::Succeeded);
            assert_eq!(allocations, 0);
            assert!(f.backend.active.is_none());
            for index in 0..3 {
                assert_restored(&f, index, &replay);
            }
            f.cleanup();
        }
    }
}

#[test]
fn three_completion_reservation_failure_leaves_inputs_unextracted() {
    for origin in 0..3 {
        let mut f = Fixture::<3>::new_origin(origin);
        let before = f.allocations.map(|id| stored_input(&f.backend, id));
        let steps = f.backend.scripted_sdma.as_ref().unwrap().remaining_steps();
        let trailing = pending_facts(&f.backend.pending_compute[&f.trailing]);
        f.backend.scripted_three_completion_fault = Some(CompletionFault::Reserve);
        assert!(matches!(
            f.backend.flush_stream_v1(f.stream),
            Err(RuntimeBackendFailureV1::Quiescent(_))
        ));
        assert!(!f.backend.terminal);
        assert!(f.backend.active.is_none());
        assert_eq!(f.allocations.map(|id| stored_input(&f.backend, id)), before);
        // Storage promotion is preparation, before the completion-root reservation.
        assert_eq!(
            f.backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
            steps - usize::from(origin == 1) * 3
        );
        assert_eq!(
            pending_facts(&f.backend.pending_compute[&f.trailing]),
            trailing
        );
        assert_eq!(
            f.backend.poll_v1(f.first).unwrap(),
            BackendPollV1::Failed { code: -1 }
        );
        assert_eq!(f.backend.compute_completion_reservations, 1);
        assert_eq!(
            f.backend.cancel_v1(f.trailing).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        f.cleanup();
    }
}

#[test]
fn three_completion_empty_reserved_root_cancels_cleanly() {
    for origin in 0..3 {
        let mut f = Fixture::<3>::new_origin(origin);
        f.backend.scripted_persistent_publication_retries = 1;
        f.backend.flush_stream_v1(f.stream).unwrap();
        f.assert_handoff();
        let Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared {
            completion,
            ..
        }) = f.backend.active.as_ref().unwrap().execution.as_ref()
        else {
            panic!("prepared three")
        };
        assert!(matches!(
            completion.receipt,
            ThreeCompletionReceiptV1::Reserved
        ));
        assert_origin(completion.admissions, origin);
        assert!(completion.shells.iter().all(Option::is_none));
        f.finish(true);
    }
}

fn inspect_prepared(case: usize) {
    let mut f = Fixture::<3>::new_origin(case / 12);
    f.backend.scripted_persistent_publication_retries = 1;
    f.backend.flush_stream_v1(f.stream).unwrap();
    let before_inputs = inputs(&f.backend);
    let before_shells = shells(&f.backend);
    let before_ledger = ledger(&f.backend);
    let trailing = pending_facts(&f.backend.pending_compute[&f.trailing]);
    let steps = f.backend.scripted_sdma.as_ref().unwrap().remaining_steps();
    let live = f.backend.scripted_sdma.as_ref().unwrap().live_owner_count();
    let Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared {
        completion, ..
    }) = f.backend.active.as_mut().unwrap().execution.as_mut()
    else {
        panic!("prepared three")
    };
    let address = completion.as_ref() as *const _ as usize;
    assert_origin(completion.admissions, case / 12);
    match case % 6 {
        0 => completion.receipt = ThreeCompletionReceiptV1::NativeOwned,
        1 => completion.admissions[0].allocation = f.copy_pair.0,
        2 => completion.admissions[0].access = RuntimeAccessV1::Write,
        3 => {
            completion.shells[0] = Some(ThreeBindingPersistentRestoreShellV1 {
                ready: None,
                device: None,
                replay: None,
                initialized: None,
            })
        }
        4 => completion.control = ThreeCompletionControlV1::Released,
        5 => completion.admissions[2].allocation = completion.admissions[1].allocation,
        _ => unreachable!(),
    }
    let result = if case % 12 < 6 {
        f.backend.poll_v1(f.first).map(|_| ())
    } else {
        f.backend.cancel_v1(f.first).map(|_| ())
    };
    assert!(matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))));
    assert!(f.backend.terminal);
    assert_eq!(inputs(&f.backend), before_inputs);
    assert_eq!(shells(&f.backend), before_shells);
    assert_eq!(ledger(&f.backend), before_ledger);
    assert_eq!(
        pending_facts(&f.backend.pending_compute[&f.trailing]),
        trailing
    );
    let Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared {
        completion, ..
    }) = f.backend.active.as_ref().unwrap().execution.as_ref()
    else {
        panic!("retained prepared three")
    };
    assert_eq!(completion.as_ref() as *const _ as usize, address);
    let driver = f.backend.scripted_sdma.as_ref().unwrap();
    assert_eq!(driver.remaining_steps(), steps);
    assert_eq!(driver.live_owner_count(), live);
    assert_eq!(driver.unexpected_drops(), 0);
    eprintln!("three prepared completion custody inspected; dropping unrepaired backend");
    drop(ManuallyDrop::into_inner(f.backend));
    panic!("terminal prepared Drop returned");
}

#[test]
fn three_completion_prepared_publication_and_cancellation_validate_reserved_root() {
    subprocess_matrix(
        concat!(
            module_path!(),
            "::three_completion_prepared_publication_and_cancellation_validate_reserved_root"
        ),
        "FE2O3_TEST_THREE_COMPLETION_PREPARED",
        36,
        inspect_prepared,
        "three prepared completion custody inspected; dropping unrepaired backend",
    );
}
