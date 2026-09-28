//! Public admission with scripted owners; this does not execute a GPU kernel.

use super::super::prepared_cancellation::ScriptedPreparedCancelFaultV1 as Fault;
use super::*;
use std::mem::ManuallyDrop;
use std::os::unix::process::ExitStatusExt;
use std::panic::{AssertUnwindSafe, catch_unwind};

pub(super) fn fixture() -> (KfdRuntimeBackendV1, u64, u64) {
    let (mut backend, stream, _, device) = scripted_direct_backend_v1(
        4096,
        vec![ScriptedSdmaStepV1::PromoteInitializedStorage(
            ScriptedFailureModeV1::Success,
        )],
    );
    let module = backend
        .load_module_v1(7, &synthetic_cov6::module())
        .unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();
    backend.scripted_persistent_publication_retries = 1;
    let submission = submit_scripted_read_v1(&mut backend, stream, kernel, device, 4096, &[]);
    backend.flush_stream_v1(stream).unwrap();
    assert!(matches!(
        backend.active.as_ref().unwrap().execution,
        Some(ActiveComputeExecutionV1::ScriptedPersistentPrepared { .. })
    ));
    (backend, submission, device)
}

pub(super) fn three_fixture() -> (KfdRuntimeBackendV1, u64, Vec<u64>) {
    let steps = (0..3)
        .map(|_| ScriptedSdmaStepV1::PromoteInitializedStorage(ScriptedFailureModeV1::Success));
    let (mut backend, stream, allocations) =
        scripted_persistent_backend_with_steps_v1::<3>(4096, steps);
    for allocation in allocations {
        backend.normalize_h2d_ready_v1(allocation).unwrap();
    }
    let module = backend
        .load_module_v1(7, &synthetic_cov6::three_binding_module())
        .unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();
    backend.scripted_persistent_publication_retries = 1;
    let submission =
        submit_scripted_three_binding_v1(&mut backend, stream, kernel, allocations, 4096);
    backend.flush_stream_v1(stream).unwrap();
    assert!(matches!(
        backend.active.as_ref().unwrap().execution,
        Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared { .. })
    ));
    (backend, submission, allocations.to_vec())
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct InputFacts {
    pub(super) owner: u64,
    pub(super) bytes: usize,
    pub(super) digest: [u8; 32],
    certificate: Option<[u8; 32]>,
    origin: &'static str,
}

#[derive(Debug, Eq, PartialEq)]
struct OwnerFacts {
    input: InputFacts,
    shells: Option<[usize; 4]>,
    restored_box: Option<usize>,
}

pub(super) fn input_facts(input: &KfdRuntimePersistentComputeInputV1) -> InputFacts {
    let (id, bytes, certificate, origin) = match input {
        KfdRuntimePersistentComputeInputV1::ScriptedStorage(device) => (
            device.scripted_owner_id().unwrap(),
            device.scripted_bytes().unwrap(),
            None,
            "storage",
        ),
        KfdRuntimePersistentComputeInputV1::ScriptedReplay(device) => (
            device.scripted_owner_id().unwrap(),
            device.scripted_bytes().unwrap(),
            None,
            "replay",
        ),
        KfdRuntimePersistentComputeInputV1::ScriptedReady(ready) => (
            ready.owner.scripted_owner_id().unwrap(),
            ready.owner.scripted_bytes().unwrap(),
            Some(ready.owner.authenticated_sha256()),
            "ready",
        ),
        _ => panic!("scripted input required"),
    };
    InputFacts {
        owner: id,
        bytes: bytes.as_ptr() as usize,
        digest: Sha256::digest(bytes).into(),
        certificate,
        origin,
    }
}

pub(super) fn shell_facts(shell: &ThreeBindingPersistentRestoreShellV1) -> [usize; 4] {
    [
        shell
            .ready
            .as_ref()
            .map_or(0, |value| value.as_ref() as *const _ as usize),
        shell
            .device
            .as_ref()
            .map_or(0, |value| value.as_ref() as *const _ as usize),
        shell
            .replay
            .as_ref()
            .map_or(0, |value| value.as_ref() as *const _ as usize),
        shell
            .initialized
            .as_ref()
            .map_or(0, |value| value.as_ref() as *const _ as usize),
    ]
}

fn owners(backend: &KfdRuntimeBackendV1) -> Vec<OwnerFacts> {
    let active = backend.active.as_ref().unwrap();
    match active.execution.as_ref().unwrap() {
        ActiveComputeExecutionV1::ScriptedPersistentPrepared {
            allocation, input, ..
        } => vec![OwnerFacts {
            input: input_facts(input.armed().unwrap()),
            shells: Some(shell_facts(
                backend.allocations[allocation]
                    .persistent_storage_restore
                    .as_ref()
                    .unwrap(),
            )),
            restored_box: None,
        }],
        ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared {
            inputs,
            restore_shells,
            ..
        } => inputs
            .armed()
            .unwrap()
            .iter()
            .zip(restore_shells)
            .map(|(input, shell)| OwnerFacts {
                input: input_facts(input),
                shells: Some(shell_facts(shell)),
                restored_box: None,
            })
            .collect(),
        ActiveComputeExecutionV1::PersistentCancelling(root) => root
            .slots
            .iter()
            .flatten()
            .map(|slot| {
                if slot.restored {
                    let storage = &backend.allocations[&slot.admission.allocation].sdma_storage;
                    let KfdRuntimeSdmaStorageV1::InitializedStorage(owner) = storage else {
                        panic!("restored storage origin");
                    };
                    let InitializedStorageOwnerV1::Scripted(device) = owner.as_ref() else {
                        panic!("scripted owner");
                    };
                    OwnerFacts {
                        input: InputFacts {
                            owner: device.scripted_owner_id().unwrap(),
                            bytes: device.scripted_bytes().unwrap().as_ptr() as usize,
                            digest: Sha256::digest(device.scripted_bytes().unwrap()).into(),
                            certificate: None,
                            origin: "storage",
                        },
                        shells: None,
                        restored_box: Some(owner.as_ref() as *const _ as usize),
                    }
                } else {
                    OwnerFacts {
                        input: input_facts(slot.input.as_ref().unwrap()),
                        shells: Some(shell_facts(slot.shell.as_ref().unwrap())),
                        restored_box: None,
                    }
                }
            })
            .collect(),
        _ => panic!("prepared or cancelling required"),
    }
}

pub(super) fn profile_facts(backend: &KfdRuntimeBackendV1) -> String {
    let profile = match backend.active.as_ref().unwrap().execution.as_ref().unwrap() {
        ActiveComputeExecutionV1::ScriptedPersistentPrepared { profile, .. }
        | ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared { profile, .. } => {
            profile
        }
        ActiveComputeExecutionV1::PersistentCancelling(root) => &root._profile,
        _ => panic!("prepared profile"),
    };
    format!(
        "{:?} {:?} {:?}",
        profile.launch, profile.semantic_contract, profile.bindings
    )
}

fn accounting(backend: &KfdRuntimeBackendV1) -> Vec<String> {
    let active = backend.active.as_ref().unwrap();
    let mut result = vec![
        format!(
            "{active:?} {:?} {:?} {:?} {:?} {:?} {} {} {:?}",
            active.resident_descriptors,
            active.performance,
            active.dispatch_shape_sha256,
            active.published_at,
            active.ordinary_recipe,
            active.writebacks.as_ptr() as usize,
            active.resident_descriptors.as_ptr() as usize,
            backend.retained_persistent_dispatch
        ),
        format!(
            "reservations {} staged {} events {:?} tails {:?} lanes {:?} modules {:?} dependencies {:?}",
            backend.compute_completion_reservations,
            backend.staged_context_bytes,
            backend.event_submission_retain_counts,
            backend.stream_submission_tails,
            backend.stream_compute_lanes,
            backend.compute_module_retain_counts,
            backend.compute_dependency_retain_counts
        ),
    ];
    for (allocation, custody) in &backend.allocation_custody {
        result.push(format!(
            "custody {allocation} {:?} {:?} {:?}",
            custody.owners, custody.owner_counts, custody.sole_stream
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
    for (id, record) in backend.allocations.ordinary_iter() {
        result.push(format!(
            "allocation {id} {} {:?} {:?} {:?} {:?}",
            record.bytes.as_ptr() as usize,
            Sha256::digest(&record.bytes),
            record.content_sha256,
            record.native_dirty,
            record.sdma_shadow_dirty
        ));
    }
    result.sort();
    result
}

fn inspect_case(case: usize) {
    let three = case >= 22;
    let case = case % 22;
    let (backend, submission, allocations) = if three {
        three_fixture()
    } else {
        let (backend, submission, allocation) = fixture();
        (backend, submission, vec![allocation])
    };
    let mut backend = ManuallyDrop::new(backend);
    let stream = backend.active.as_ref().unwrap().stream;
    let module = backend.kernels[&backend.active.as_ref().unwrap().kernel].module;
    let event = backend.record_event_v1(stream, submission).unwrap();
    let first = allocations[0];
    let fault = if case < 16 {
        Fault::Unwind
    } else {
        match case {
            16 => Fault::Terminal,
            17 => Fault::Unwind,
            18 => Fault::ReturnedSlotMismatch(allocations.len() - 1),
            19 => Fault::AfterRestore(0),
            20 => Fault::AfterRestore(allocations.len() - 1),
            21 => Fault::BeforeCommit,
            _ => unreachable!(),
        }
    };
    backend.scripted_prepared_cancel_fault = Some(fault);
    match case {
        0 => {
            backend.allocations.get_mut(&first).unwrap().sdma_storage =
                KfdRuntimeSdmaStorageV1::ComputeInFlight(submission + 1)
        }
        1 => {
            backend.compute_module_retain_counts.insert(module, 0);
        }
        2 => {
            backend.allocation_custody.remove(&first);
        }
        3 => {
            backend
                .allocation_custody
                .get_mut(&first)
                .unwrap()
                .owner_counts[0] = 0
        }
        4 => backend.compute_completion_reservations = 0,
        5 => {
            backend.stream_compute_lanes.insert(stream, 1);
        }
        6 => {
            backend.active.as_mut().unwrap().allocations.remove(&first);
        }
        7 => {
            backend.submissions.insert(
                submission,
                SubmissionRecordV1 {
                    stream,
                    status: BackendPollV1::Succeeded,
                    dependency_depth: 0,
                    profile_dispatch_published: true,
                },
            );
        }
        8 => {
            let kernel = backend.active.as_ref().unwrap().kernel;
            backend.kernels.get_mut(&kernel).unwrap().module = u64::MAX;
        }
        9 => {
            let custody = backend.allocation_custody.get_mut(&first).unwrap();
            let owner = custody.owners[0];
            custody.owners.push_back(owner);
            custody.owner_counts[0] += 1;
        }
        10 => backend.allocation_custody.get_mut(&first).unwrap().owners[0].stream = stream + 1,
        11 => {
            backend.retained_persistent_dispatch = Some(RetainedPersistentDispatchV1 {
                allocation: first,
                dispatch_shape_sha256: [0; 32],
            })
        }
        12 => {
            backend.stream_submission_tails.remove(&stream);
        }
        13 => {
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
                    .get_mut(&first)
                    .unwrap()
                    .persistent_storage_restore
                    .as_mut()
                    .unwrap()
                    .initialized = None;
            }
        }
        14 => match backend.active.as_mut().unwrap().execution.as_mut().unwrap() {
            ActiveComputeExecutionV1::ScriptedPersistentPrepared { source, .. } => {
                *source = PersistentFullRangeComputeSourceV1::AuthenticatedH2d
            }
            ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared {
                admissions, ..
            } => admissions[2].source = PersistentFullRangeComputeSourceV1::AuthenticatedH2d,
            _ => unreachable!(),
        },
        15 => {
            backend
                .active
                .as_mut()
                .unwrap()
                .deferred_ordered_predecessor_retain = true
        }
        _ => {}
    }
    let before = accounting(&backend);
    let before_owners = owners(&backend);
    let profile = profile_facts(&backend);
    let remaining = backend.scripted_sdma.as_ref().unwrap().remaining_steps();
    let live = backend.scripted_sdma.as_ref().unwrap().live_owner_count();
    let result = catch_unwind(AssertUnwindSafe(|| backend.cancel_v1(submission)));
    if case < 16 || matches!(case, 16 | 18) {
        assert!(matches!(
            result,
            Ok(Err(RuntimeBackendFailureV1::Terminal(_)))
        ));
    } else {
        let payload = result.unwrap_err();
        let expected = match case {
            17 => "scripted consuming cancellation unwind",
            19 | 20 => "scripted cancellation restoration unwind",
            _ => "scripted cancellation commit unwind",
        };
        assert_eq!(payload.downcast_ref::<&str>(), Some(&expected));
    }
    assert_eq!(
        backend.active.as_ref().map(|active| active.id),
        Some(submission)
    );
    assert_eq!(accounting(&backend), before);
    assert_eq!(profile_facts(&backend), profile);
    assert_eq!(backend.events[&event].submission, submission);
    let after_owners = owners(&backend);
    let restored = match case {
        19 => 1,
        20 | 21 => allocations.len(),
        _ => 0,
    };
    let execution = backend.active.as_ref().unwrap().execution.as_ref().unwrap();
    if case < 16 {
        assert!(if three {
            matches!(
                execution,
                ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared { .. }
            )
        } else {
            matches!(
                execution,
                ActiveComputeExecutionV1::ScriptedPersistentPrepared { .. }
            )
        });
    } else {
        let ActiveComputeExecutionV1::PersistentCancelling(root) = execution else {
            panic!("indexed cancellation phase");
        };
        assert!(matches!(
            root.receipt,
            super::super::prepared_cancellation::PreparedCancellationReceiptV1::InputsReturned
        ));
        for index in 0..3 {
            if index >= allocations.len() {
                assert!(root.slots[index].is_none());
                continue;
            }
            let slot = root.slots[index].as_ref().unwrap();
            assert_eq!(slot.admission.allocation, allocations[index]);
            assert_eq!(slot.restored, index < restored);
            assert_eq!(slot.input.is_some(), index >= restored);
            assert_eq!(slot.shell.is_some(), index >= restored);
        }
    }
    for index in 0..allocations.len() {
        if index < restored {
            assert_eq!(after_owners[index].input, before_owners[index].input);
            assert_eq!(after_owners[index].shells, None);
            assert_eq!(
                after_owners[index].restored_box,
                Some(before_owners[index].shells.unwrap()[3])
            );
        } else {
            assert_eq!(after_owners[index], before_owners[index]);
            let marker = if case == 0 && index == 0 {
                submission + 1
            } else if case == 18 && index == allocations.len() - 1 {
                0
            } else {
                submission
            };
            assert!(
                matches!(backend.allocations[&allocations[index]].sdma_storage,
                KfdRuntimeSdmaStorageV1::ComputeInFlight(actual) if actual == marker)
            );
        }
    }
    assert_eq!(
        backend.scripted_prepared_cancel_fault,
        if case < 16 { Some(fault) } else { None }
    );
    for operation in [
        backend.cancel_v1(submission).map(|_| ()),
        backend.poll_v1(submission).map(|_| ()),
        backend.shutdown_native_v1(),
    ] {
        assert!(matches!(
            operation,
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
    }
    assert_eq!(accounting(&backend), before);
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert_eq!(driver.remaining_steps(), remaining);
    assert_eq!(driver.live_owner_count(), live);
    assert_eq!(driver.unexpected_drops(), 0);
    eprintln!("prepared cancellation custody inspected; dropping unrepaired backend");
    drop(ManuallyDrop::into_inner(backend));
    panic!("prepared cancellation Drop returned");
}

#[test]
fn prepared_cancellation_invalid_slot_keeps_outer_descriptor() {
    const CHILD: &str = "FE2O3_TEST_PREPARED_CANCELLATION_CUSTODY";
    const TEST: &str = "kfd_backend::tests::prepared_cancellation_tests::prepared_cancellation_invalid_slot_keeps_outer_descriptor";
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
    for case in 0..44 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, case.to_string())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "case {case}: {stderr}");
        assert!(
            stderr.contains("prepared cancellation custody inspected; dropping unrepaired backend"),
            "case {case}: {stderr}"
        );
        eprintln!("verified prepared cancellation case {case}: retained custody; Drop SIGABRT");
    }
}

fn healthy<const N: usize>(origin: usize, publish: bool) {
    let mut steps = Vec::new();
    let promotions =
        usize::from(origin == 1 || (N == 3 && origin >= 2)) * N + usize::from(origin == 3) * N;
    steps
        .extend((0..promotions).map(|_| {
            ScriptedSdmaStepV1::PromoteInitializedStorage(ScriptedFailureModeV1::Success)
        }));
    steps.extend((0..N).flat_map(|_| {
        [
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ]
    }));
    let (backend, stream, allocations) =
        scripted_persistent_backend_with_steps_v1::<N>(4096, steps);
    let mut backend = ManuallyDrop::new(backend);
    if origin == 1 || (N == 3 && origin >= 2) {
        for allocation in allocations {
            backend.normalize_h2d_ready_v1(allocation).unwrap();
        }
    }
    let module = backend
        .load_module_v1(
            7,
            &if N == 1 {
                synthetic_cov6::module()
            } else {
                synthetic_cov6::three_binding_module()
            },
        )
        .unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();
    let submit = |backend: &mut KfdRuntimeBackendV1| {
        if N == 1 {
            submit_scripted_read_v1(backend, stream, kernel, allocations[0], 4096, &[])
        } else {
            submit_scripted_three_binding_v1(
                backend,
                stream,
                kernel,
                [allocations[0], allocations[1], allocations[2]],
                4096,
            )
        }
    };
    let mut completed = None;
    if origin >= 2 {
        let first = submit(&mut backend);
        backend.flush_stream_v1(stream).unwrap();
        assert_eq!(backend.poll_v1(first).unwrap(), BackendPollV1::Succeeded);
        completed = Some(first);
    }
    backend.scripted_persistent_publication_retries = 1;
    let submission = submit(&mut backend);
    let event = backend.record_event_v1(stream, submission).unwrap();
    backend.flush_stream_v1(stream).unwrap();
    let (before_inputs, admissions): (Vec<_>, Vec<_>) =
        match backend.active.as_ref().unwrap().execution.as_ref().unwrap() {
            ActiveComputeExecutionV1::ScriptedPersistentPrepared {
                allocation,
                access,
                source,
                input,
                ..
            } => (
                vec![input_facts(input.armed().unwrap())],
                vec![PersistentFullRangeComputeAdmissionV1 {
                    allocation: *allocation,
                    access: *access,
                    source: *source,
                }],
            ),
            ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared {
                inputs,
                admissions,
                ..
            } => (
                inputs.armed().unwrap().iter().map(input_facts).collect(),
                admissions.to_vec(),
            ),
            _ => panic!("prepared before cancellation/publication"),
        };
    let source = match origin {
        0 => PersistentFullRangeComputeSourceV1::AuthenticatedH2d,
        1 | 3 => PersistentFullRangeComputeSourceV1::InitializedStorage,
        _ => PersistentFullRangeComputeSourceV1::RetainedControlReplay,
    };
    assert!(
        admissions
            .iter()
            .all(|admission| admission.source == source),
        "N={N} origin={origin}: {admissions:?}"
    );
    if origin == 3 && N == 1 {
        assert!(
            backend
                .active
                .as_ref()
                .unwrap()
                .performance
                .persistent_control_reused()
        );
    }
    let profile = profile_facts(&backend);
    let ledger = super::prepared_publication_tests::ledger(&backend);
    let shells = super::prepared_publication_tests::shells(&backend);
    for _ in 0..2 {
        backend.scripted_prepared_publication_fault =
            Some(super::super::prepared_publication::ScriptedPreparedPublicationFaultV1::Retryable);
        assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
        assert!(backend.persistent_prepared_is_armed_v1());
        assert_eq!(
            super::prepared_publication_tests::inputs(&backend),
            before_inputs
        );
        assert_eq!(profile_facts(&backend), profile);
        assert_eq!(super::prepared_publication_tests::ledger(&backend), ledger);
        assert_eq!(super::prepared_publication_tests::shells(&backend), shells);
        assert!(!backend.terminal);
    }
    if publish {
        assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
        assert_eq!(
            backend.poll_v1(submission).unwrap(),
            BackendPollV1::Succeeded
        );
    } else {
        assert_eq!(
            backend.cancel_v1(submission).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        assert_eq!(
            backend.poll_v1(submission).unwrap(),
            BackendPollV1::Failed { code: -2 }
        );
        for (index, allocation) in allocations.into_iter().enumerate() {
            let record = &backend.allocations[&allocation];
            let actual = match &record.sdma_storage {
                KfdRuntimeSdmaStorageV1::H2dReady(ready) => InputFacts {
                    owner: ready.owner.scripted_owner_id().unwrap(),
                    bytes: ready.owner.scripted_bytes().unwrap().as_ptr() as usize,
                    digest: Sha256::digest(ready.owner.scripted_bytes().unwrap()).into(),
                    certificate: Some(ready.owner.authenticated_sha256()),
                    origin: "ready",
                },
                KfdRuntimeSdmaStorageV1::InitializedStorage(owner) => {
                    let InitializedStorageOwnerV1::Scripted(device) = owner.as_ref() else {
                        unreachable!()
                    };
                    InputFacts {
                        owner: device.scripted_owner_id().unwrap(),
                        bytes: device.scripted_bytes().unwrap().as_ptr() as usize,
                        digest: Sha256::digest(device.scripted_bytes().unwrap()).into(),
                        certificate: None,
                        origin: "storage",
                    }
                }
                KfdRuntimeSdmaStorageV1::Device(device) => InputFacts {
                    owner: device.scripted_owner_id().unwrap(),
                    bytes: device.scripted_bytes().unwrap().as_ptr() as usize,
                    digest: Sha256::digest(device.scripted_bytes().unwrap()).into(),
                    certificate: None,
                    origin: "replay",
                },
                _ => panic!("typed restored backing"),
            };
            assert_eq!(actual, before_inputs[index]);
            assert!(record.persistent_storage_restore.is_none());
        }
        assert!(backend.retained_persistent_dispatch.is_none());
    }
    assert!(backend.active.is_none());
    assert!(backend.allocation_custody.is_empty());
    assert!(backend.compute_module_retain_counts.is_empty());
    assert!(backend.compute_dependency_retain_counts.is_empty());
    assert!(!backend.stream_compute_lanes.contains_key(&stream));
    if !publish {
        assert!(!backend.stream_submission_tails.contains_key(&stream));
    }
    assert_eq!(backend.compute_completion_reservations, 0);
    backend.release_event_v1(event).unwrap();
    backend.release_submission_v1(submission).unwrap();
    if let Some(first) = completed {
        backend.release_submission_v1(first).unwrap();
    }
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
fn prepared_cancellation_preserves_single_and_three_binding_origins() {
    for origin in 0..4 {
        healthy::<1>(origin, false);
        healthy::<3>(origin, false);
    }
}

#[test]
fn prepared_three_binding_poll_still_publishes_and_completes() {
    for origin in 0..4 {
        healthy::<1>(origin, true);
        healthy::<3>(origin, true);
    }
}
