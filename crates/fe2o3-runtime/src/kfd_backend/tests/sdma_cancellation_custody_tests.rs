//! Public cancellation over scripted owners, not GPU execution or unwind proof.

use super::*;
use std::mem::ManuallyDrop;
use std::os::unix::process::ExitStatusExt;

fn submit_step(kind: usize) -> ScriptedSdmaStepV1 {
    submit_step_at(kind, 0)
}

fn submit_step_at(kind: usize, offset: u64) -> ScriptedSdmaStepV1 {
    match kind {
        0 | 1 => scripted_submit_step_v1(
            if kind == 0 {
                Gfx942PersistentSdmaDirectionV1::HostToDevice
            } else {
                Gfx942PersistentSdmaDirectionV1::DeviceToHost
            },
            offset,
            offset,
            8,
            ScriptedFailureModeV1::Success,
        ),
        2 => scripted_same_device_submit_window_step_v1(
            [SameDeviceSdmaCopyRequestV1 {
                source_offset: offset,
                destination_offset: offset,
                copy_bytes: 8,
            }],
            ScriptedFailureModeV1::Success,
        ),
        _ => panic!("unknown copy kind"),
    }
}

fn completion_steps(kind: usize) -> [ScriptedSdmaStepV1; 2] {
    if kind == 2 {
        [
            ScriptedSdmaStepV1::PollSameDevice(ScriptedSameDeviceExecutionOutcomeV1::Completed {
                copy_bytes: None,
                requests: None,
                swap_allocations: false,
            }),
            ScriptedSdmaStepV1::RetireSameDevice(ScriptedFailureModeV1::Success),
        ]
    } else {
        [
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
            ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
        ]
    }
}

fn release_steps(kind: usize) -> Vec<ScriptedSdmaStepV1> {
    if kind == 2 {
        scripted_same_device_release_steps_v1().into()
    } else {
        scripted_release_steps_v1().into()
    }
}

fn clean(backend: &mut KfdRuntimeBackendV1, kind: usize, stream: u64, first: u64, second: u64) {
    if kind == 2 {
        clean_scripted_same_device_backend_v1(backend, stream, first, second, None);
    } else {
        clean_scripted_direct_backend_v1(backend, stream, first, second, None);
    }
}

#[test]
fn asynchronous_copy_cancellation_preserves_active_predecessor_and_retained_successor() {
    for kind in 0..3 {
        for successor in [false, true] {
            let mut steps = vec![submit_step(kind)];
            steps.extend(completion_steps(kind));
            steps.extend(release_steps(kind));
            let (mut backend, stream, first, second) = if kind == 2 {
                scripted_same_device_backend_v1(16, steps)
            } else {
                scripted_direct_backend_v1(16, steps)
            };
            let (source_id, destination_id) = if kind == 1 {
                (second, first)
            } else {
                (first, second)
            };
            let (source, destination) = scripted_copy_regions_v1(source_id, destination_id, 8);
            let predecessor = backend
                .copy_async_v1(stream, source, destination, &[])
                .unwrap();
            let cancelled = backend
                .copy_async_v1(stream, source, destination, &[])
                .unwrap();
            let event = backend.record_event_v1(stream, cancelled).unwrap();
            let dependent = successor.then(|| {
                backend
                    .copy_async_v1(stream, source, destination, &[event])
                    .unwrap()
            });
            let prior_facts = descriptor_facts(&backend.active_sdma[&predecessor]);
            let later_facts = dependent.map(|id| descriptor_facts(&backend.active_sdma[&id]));
            let owners = native_pair_ids(&backend.active_sdma[&predecessor], kind);
            let steps = backend.scripted_sdma.as_ref().unwrap().remaining_steps();
            assert_eq!(
                backend.cancel_v1(predecessor).unwrap(),
                crate::BackendCancellationV1::TooLate
            );
            assert_eq!(
                backend.cancel_v1(cancelled).unwrap(),
                crate::BackendCancellationV1::Cancelled
            );
            assert_eq!(
                backend.submissions[&cancelled].status,
                BackendPollV1::Failed { code: -2 }
            );
            assert!(!backend.quiescent_sdma_submissions.contains(&cancelled));
            assert!(!backend.active_sdma.contains_key(&cancelled));
            assert_eq!(
                backend.sdma_completion_reservations,
                1 + usize::from(successor)
            );
            assert_eq!(
                descriptor_facts(&backend.active_sdma[&predecessor]),
                prior_facts
            );
            assert_eq!(
                native_pair_ids(&backend.active_sdma[&predecessor], kind),
                owners
            );
            assert_eq!(
                backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
                steps
            );
            let retained: Vec<_> = std::iter::once(predecessor).chain(dependent).collect();
            assert_eq!(
                backend.active_sdma_streams[&stream],
                VecDeque::from(retained.clone())
            );
            assert_eq!(
                backend.stream_submission_tails[&stream],
                dependent.unwrap_or(predecessor)
            );
            for allocation in [source_id, destination_id] {
                let custody = &backend.allocation_custody[&allocation];
                assert_eq!(
                    custody
                        .owners
                        .iter()
                        .map(|owner| owner.submission)
                        .collect::<Vec<_>>(),
                    retained
                );
                assert_eq!(custody.owner_counts, [0, retained.len()]);
                assert!(matches!(backend.allocations[&allocation].sdma_storage,
                    KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(id)) if id == predecessor));
            }
            backend.release_event_v1(event).unwrap();
            if let Some(dependent) = dependent {
                assert_eq!(
                    descriptor_facts(&backend.active_sdma[&dependent]),
                    later_facts.unwrap()
                );
                assert_eq!(backend.sdma_dependency_retain_counts[&cancelled], 1);
                assert!(
                    matches!(backend.release_submission_v1(cancelled), Err(RuntimeBackendFailureV1::Rejected(error))
                    if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
                );
                assert!(matches!(
                    backend.poll_v1(dependent).unwrap(),
                    BackendPollV1::Failed { .. }
                ));
                assert_eq!(
                    backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
                    steps
                );
            }
            assert!(backend.sdma_dependency_retain_counts.is_empty());
            assert_eq!(
                backend.poll_v1(predecessor).unwrap(),
                BackendPollV1::Succeeded
            );
            for id in dependent.into_iter().chain([cancelled, predecessor]) {
                backend.release_submission_v1(id).unwrap();
            }
            clean(&mut backend, kind, stream, first, second);
        }
    }
}

pub(super) fn inspect_partial_cancellation(kind: usize) {
    let mut steps = vec![submit_step(kind)];
    steps.extend(completion_steps(kind));
    steps.push(submit_step_at(kind, 8));
    steps.extend(completion_steps(kind));
    steps.extend(release_steps(kind));
    let (mut backend, stream, first, second) = if kind == 2 {
        scripted_same_device_backend_v1(32, steps)
    } else {
        scripted_direct_backend_v1(32, steps)
    };
    backend
        .scripted_sdma
        .as_mut()
        .unwrap()
        .publication_byte_limit = Some(8);
    let (source_id, destination_id) = if kind == 1 {
        (second, first)
    } else {
        (first, second)
    };
    let storage_id = |storage: &KfdRuntimeSdmaStorageV1| match storage {
        KfdRuntimeSdmaStorageV1::Host(owner) => {
            super::sdma_host_write_tests::host_observation(owner).0
        }
        KfdRuntimeSdmaStorageV1::Device(owner) => owner.scripted_owner_id().unwrap(),
        _ => panic!("restored scripted storage"),
    };
    let owners = (
        storage_id(&backend.allocations[&source_id].sdma_storage),
        storage_id(&backend.allocations[&destination_id].sdma_storage),
    );
    let (source, destination) = scripted_copy_regions_v1(source_id, destination_id, 16);
    let copy = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Pending);
    assert_eq!(backend.active_sdma[&copy].completed_bytes, 8);
    assert!(matches!(
        backend.active_sdma[&copy].phase,
        ActiveSdmaPhaseV1::Ready
    ));
    let before = descriptor_facts(&backend.active_sdma[&copy]);
    let steps = backend.scripted_sdma.as_ref().unwrap().remaining_steps();
    assert_eq!(
        backend.cancel_v1(copy).unwrap(),
        crate::BackendCancellationV1::TooLate
    );
    assert_eq!(descriptor_facts(&backend.active_sdma[&copy]), before);
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
        steps
    );
    assert_eq!(
        (
            storage_id(&backend.allocations[&source_id].sdma_storage),
            storage_id(&backend.allocations[&destination_id].sdma_storage)
        ),
        owners
    );
    assert_eq!(backend.sdma_completion_reservations, 1);
    assert!(!backend.submissions.contains_key(&copy));
    backend.flush_stream_v1(stream).unwrap();
    assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
    backend.release_submission_v1(copy).unwrap();
    clean(&mut backend, kind, stream, first, second);
}

#[test]
fn asynchronous_copy_partial_d2h_and_same_device_cancellation_is_too_late() {
    for kind in [1, 2] {
        inspect_partial_cancellation(kind);
    }
}

fn native_pair_ids(active: &ActiveSdmaCopyV1, kind: usize) -> (u64, u64) {
    match &active.phase {
        ActiveSdmaPhaseV1::DirectionalPublished(owner) => {
            let DirectionalSdmaSubmissionOwnerV1::Scripted(owner) = owner.as_ref() else {
                panic!("scripted directional owner");
            };
            let host = super::sdma_host_write_tests::host_observation(&owner.pair().host).0;
            let device = owner.pair().device.scripted_owner_id().unwrap();
            if kind == 0 {
                (host, device)
            } else {
                (device, host)
            }
        }
        ActiveSdmaPhaseV1::SameDevicePublished(owner) => {
            let SameDeviceSdmaSubmissionOwnerV1::Scripted(owner) = owner.as_ref() else {
                panic!("scripted same-device owner");
            };
            (
                owner.pair().source.scripted_owner_id().unwrap(),
                owner.pair().destination.scripted_owner_id().unwrap(),
            )
        }
        _ => panic!("predecessor must remain published"),
    }
}

#[derive(Debug, Eq, PartialEq)]
struct DescriptorFacts {
    identity: (u64, u64, u64, u64),
    range: (u64, u64, u64),
    prior: Option<u64>,
    progress: (u64, u64),
    requests: Option<Vec<DirectSdmaCopyRequestV1>>,
    dependencies: Vec<u64>,
    roster_address: usize,
    cursor: usize,
    depth: usize,
    phase: u8,
    native_address: usize,
}

fn descriptor_facts(active: &ActiveSdmaCopyV1) -> DescriptorFacts {
    let (phase, native_address) = match &active.phase {
        ActiveSdmaPhaseV1::Ready => (0, 0),
        ActiveSdmaPhaseV1::Quarantined => (1, 0),
        ActiveSdmaPhaseV1::DirectionalPublished(owner) => (2, owner.as_ref() as *const _ as usize),
        ActiveSdmaPhaseV1::SameDevicePublished(owner) => (3, owner.as_ref() as *const _ as usize),
    };
    DescriptorFacts {
        identity: (active.id, active.stream, active.source, active.destination),
        range: (
            active.source_offset,
            active.destination_offset,
            active.byte_len,
        ),
        prior: active.prior_stream_submission,
        progress: (active.completed_bytes, active.window_bytes),
        requests: active
            .window_requests
            .as_ref()
            .map(|requests| requests.as_slice().to_vec()),
        dependencies: active.dependencies.clone(),
        roster_address: active.dependencies.as_ptr() as usize,
        cursor: active.dependency_cursor,
        depth: active.dependency_depth,
        phase,
        native_address,
    }
}

fn inspect_cancel_failure_and_drop(kind: usize, successor: bool, mutation: usize) {
    let (backend, stream, first, second) = if kind == 2 {
        scripted_same_device_backend_v1(16, [submit_step(kind)])
    } else {
        scripted_direct_backend_v1(16, [submit_step(kind)])
    };
    let mut backend = ManuallyDrop::new(backend);
    let (source_id, destination_id) = if kind == 1 {
        (second, first)
    } else {
        (first, second)
    };
    let (source, destination) = scripted_copy_regions_v1(source_id, destination_id, 8);
    let predecessor = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    let cancelled = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    let event = backend.record_event_v1(stream, cancelled).unwrap();
    let dependent = successor.then(|| {
        backend
            .copy_async_v1(stream, source, destination, &[event])
            .unwrap()
    });
    let native_ids = native_pair_ids(&backend.active_sdma[&predecessor], kind);
    assert_eq!(backend.active_sdma[&cancelled].dependencies, [predecessor]);
    if let Some(dependent) = dependent {
        assert_eq!(backend.active_sdma[&dependent].dependencies, [cancelled]);
    }
    // Deliberate private invariant faults after real admission; no native owner
    // is extracted, cloned or dropped by these metadata mutations or snapshots.
    match mutation {
        0 => {
            backend.allocation_custody.remove(&destination_id);
        }
        1 => {
            backend.sdma_dependency_retain_counts.remove(&predecessor);
        }
        2 => {
            backend
                .allocation_custody
                .get_mut(&source_id)
                .unwrap()
                .owner_counts[1] = 0
        }
        3 => backend
            .active_sdma_streams
            .get_mut(&stream)
            .unwrap()
            .retain(|id| *id != cancelled),
        4 => backend.sdma_completion_reservations = 0,
        5 => {
            backend.active_sdma.get_mut(&cancelled).unwrap().phase = ActiveSdmaPhaseV1::Quarantined
        }
        6 => {
            let active = backend.active_sdma.get_mut(&cancelled).unwrap();
            active.window_bytes = 8;
            active.window_requests =
                Some(DirectSdmaRequestPlanV1::Single(DirectSdmaCopyRequestV1 {
                    source_offset: 0,
                    destination_offset: 0,
                    copy_bytes: 8,
                }));
        }
        7 => {
            backend
                .active_sdma
                .get_mut(&cancelled)
                .unwrap()
                .dependencies
                .push(predecessor);
            *backend
                .sdma_dependency_retain_counts
                .get_mut(&predecessor)
                .unwrap() += 1;
        }
        8 => {
            let queue = backend.active_sdma_streams.get_mut(&stream).unwrap();
            let position = queue.iter().position(|id| *id == cancelled).unwrap();
            queue.insert(position, cancelled);
        }
        9 => {
            let custody = backend.allocation_custody.get_mut(&destination_id).unwrap();
            let position = custody
                .owners
                .iter()
                .position(|owner| owner.submission == cancelled)
                .unwrap();
            custody.owners.insert(position, custody.owners[position]);
            custody.owner_counts[1] += 1;
        }
        10 => {
            backend.submissions.insert(
                cancelled,
                SubmissionRecordV1 {
                    stream,
                    status: BackendPollV1::Failed { code: -2 },
                    dependency_depth: 1,
                    profile_dispatch_published: false,
                },
            );
        }
        _ => panic!("unknown cancellation fault"),
    }
    let facts = |backend: &KfdRuntimeBackendV1| {
        (
            backend
                .active_sdma
                .iter()
                .map(|(&id, active)| (id, descriptor_facts(active)))
                .collect::<HashMap<_, _>>(),
            backend
                .allocation_custody
                .iter()
                .map(|(&id, custody)| {
                    (
                        id,
                        (
                            custody.owners.clone(),
                            custody.owner_counts,
                            custody.sole_stream,
                        ),
                    )
                })
                .collect::<HashMap<_, _>>(),
            backend.active_sdma_streams.clone(),
            backend.sdma_dependency_retain_counts.clone(),
            backend.event_submission_retain_counts.clone(),
            backend.sdma_completion_reservations,
            backend.stream_submission_tails.clone(),
            backend.published_sdma_submissions.clone(),
            backend.quiescent_sdma_submissions.clone(),
            backend
                .submissions
                .iter()
                .map(|(&id, record)| {
                    (
                        id,
                        (
                            record.stream,
                            record.status,
                            record.dependency_depth,
                            record.profile_dispatch_published,
                        ),
                    )
                })
                .collect::<HashMap<_, _>>(),
        )
    };
    let before = facts(&backend);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        backend.cancel_v1(cancelled)
    }));
    assert!(
        backend.active_sdma.contains_key(&cancelled),
        "cancellation detached the accepted descriptor before validating custody"
    );
    assert_eq!(
        facts(&backend),
        before,
        "cancellation released a logical prefix before validating custody"
    );
    assert!(matches!(
        outcome,
        Ok(Err(RuntimeBackendFailureV1::Terminal(_)))
    ));
    assert!(backend.terminal);
    assert!(backend.terminal_sdma_custody.is_none());
    assert_eq!(
        native_pair_ids(&backend.active_sdma[&predecessor], kind),
        native_ids
    );
    for allocation in [source_id, destination_id] {
        assert!(matches!(backend.allocations[&allocation].sdma_storage,
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(id)) if id == predecessor));
    }
    for _ in 0..2 {
        assert!(matches!(
            backend.cancel_v1(cancelled),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            backend.poll_v1(predecessor),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            backend.wait_v1(cancelled, Instant::now()),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            backend.flush_stream_v1(stream),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            backend.release_event_v1(event),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert_eq!(facts(&backend), before);
    }
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert_eq!(driver.remaining_steps(), 0);
    assert_eq!(driver.live_owner_count(), 2);
    assert_eq!(driver.unexpected_drops(), 0);
    eprintln!("cancellation custody inspected; dropping unrepaired backend");
    drop(ManuallyDrop::into_inner(backend));
    panic!("unfinished cancellation Drop returned");
}

#[test]
fn asynchronous_copy_cancellation_preflight_preserves_exact_custody() {
    const CHILD: &str = "FE2O3_TEST_SDMA_CANCELLATION_CUSTODY";
    const TEST: &str = "kfd_backend::tests::sdma_cancellation_custody_tests::asynchronous_copy_cancellation_preflight_preserves_exact_custody";
    if let Ok(case) = std::env::var(CHILD) {
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        let case: usize = case.parse().unwrap();
        inspect_cancel_failure_and_drop(case / 22, case % 22 >= 11, case % 11);
        unreachable!();
    }
    for case in 0..66 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, case.to_string())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "case {case}: {stderr}");
        assert!(
            stderr.contains("cancellation custody inspected; dropping unrepaired backend"),
            "case {case}: {stderr}"
        );
        assert!(!stderr.contains("unfinished cancellation Drop returned"));
        eprintln!("verified cancellation case {case}: exact custody retained; Drop SIGABRT");
    }
}
