//! Public admission/publication over scripted owners, not GPU execution.

use super::*;
use std::mem::ManuallyDrop;
use std::os::unix::process::ExitStatusExt;

fn storage_owner_id(storage: &KfdRuntimeSdmaStorageV1) -> u64 {
    match storage {
        KfdRuntimeSdmaStorageV1::Host(owner) => {
            super::sdma_host_write_tests::host_observation(owner).0
        }
        KfdRuntimeSdmaStorageV1::Device(owner) => owner.scripted_owner_id().unwrap(),
        _ => panic!("expected restored scripted backing"),
    }
}

fn storage_bytes(storage: &KfdRuntimeSdmaStorageV1) -> &[u8] {
    match storage {
        KfdRuntimeSdmaStorageV1::Host(owner) => owner.scripted_bytes().unwrap(),
        KfdRuntimeSdmaStorageV1::Device(owner) => owner.scripted_bytes().unwrap(),
        _ => panic!("expected restored scripted backing"),
    }
}

fn terminal_pair_ids(backend: &KfdRuntimeBackendV1, kind: usize, unwind: bool) -> (u64, u64) {
    use kfd_backend_sdma_seam::ScriptedTerminalCustodyV1;
    let capsule = if unwind {
        assert!(backend.terminal_sdma_custody.is_none());
        backend
            .scripted_sdma
            .as_ref()
            .unwrap()
            .publication_custody()
            .unwrap()
    } else {
        let Some(KfdRuntimeTerminalSdmaCustodyV1::Scripted(capsule)) =
            backend.terminal_sdma_custody.as_ref()
        else {
            panic!("scripted terminal custody")
        };
        capsule
    };
    match capsule {
        ScriptedTerminalCustodyV1::Pair(pair) => {
            let host = super::sdma_host_write_tests::host_observation(&pair.host).0;
            let device = pair.device.scripted_owner_id().unwrap();
            if kind == 0 {
                (host, device)
            } else {
                (device, host)
            }
        }
        ScriptedTerminalCustodyV1::SameDevicePair(pair) => (
            pair.source.scripted_owner_id().unwrap(),
            pair.destination.scripted_owner_id().unwrap(),
        ),
        _ => panic!("exact submitted pair must remain in terminal custody"),
    }
}

fn submit_step(kind: usize, outcome: ScriptedFailureModeV1) -> ScriptedSdmaStepV1 {
    submit_at(kind, 0, outcome)
}

fn submit_at(kind: usize, offset: u64, outcome: ScriptedFailureModeV1) -> ScriptedSdmaStepV1 {
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
            outcome,
        ),
        2 => scripted_same_device_submit_window_step_v1(
            [SameDeviceSdmaCopyRequestV1 {
                source_offset: offset,
                destination_offset: offset,
                copy_bytes: 8,
            }],
            outcome,
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

fn submit_teardown_detail(kind: usize) -> &'static str {
    if kind == 2 {
        "KFD same-device SDMA publication: scripted same-device submission teardown"
    } else {
        "KFD directional SDMA publication: scripted submission teardown"
    }
}

#[test]
fn failed_explicit_dependency_releases_only_unstarted_successor_custody() {
    let mut steps = vec![
        submit_step(0, ScriptedFailureModeV1::Success),
        submit_step(0, ScriptedFailureModeV1::Success),
    ];
    steps.extend(completion_steps(0));
    steps.push(submit_step(0, ScriptedFailureModeV1::Retryable));
    steps.extend(completion_steps(0));
    steps.push(submit_step(0, ScriptedFailureModeV1::Success));
    steps.extend(completion_steps(0));
    steps.extend(scripted_release_steps_v1());
    steps.extend(scripted_release_steps_v1());
    let (backend, stream, host, device) = scripted_direct_backend_v1(16, steps);
    let mut backend = ManuallyDrop::new(backend);
    let other_stream = backend.create_stream_v1(7).unwrap();
    let (other_host, other_device) = add_scripted_direct_pair_v1(&mut backend, 16);
    let owners = (
        storage_owner_id(&backend.allocations[&host].sdma_storage),
        storage_owner_id(&backend.allocations[&device].sdma_storage),
    );
    let (source, destination) = scripted_copy_regions_v1(host, device, 8);
    let predecessor = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    let (other_source, other_destination) = scripted_copy_regions_v1(other_host, other_device, 8);
    let first_other = backend
        .copy_async_v1(other_stream, other_source, other_destination, &[])
        .unwrap();
    let failed_dependency = backend
        .copy_async_v1(other_stream, other_source, other_destination, &[])
        .unwrap();
    let event = backend
        .record_event_v1(other_stream, failed_dependency)
        .unwrap();
    let successor = backend
        .copy_async_v1(stream, source, destination, &[event])
        .unwrap();
    assert_eq!(
        backend.active_sdma[&successor].dependencies,
        [failed_dependency, predecessor]
    );
    assert_eq!(
        backend.poll_v1(first_other).unwrap(),
        BackendPollV1::Succeeded
    );
    assert!(matches!(
        backend.flush_stream_v1(other_stream),
        Err(RuntimeBackendFailureV1::Quiescent(_))
    ));
    let steps_before = backend.scripted_sdma.as_ref().unwrap().remaining_steps();
    assert!(matches!(
        backend.poll_v1(successor),
        Ok(BackendPollV1::Failed { .. })
    ));
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
        steps_before
    );
    assert!(!backend.terminal);
    assert!(!backend.active_sdma.contains_key(&successor));
    assert!(backend.sdma_dependency_retain_counts.is_empty());
    assert_eq!(backend.sdma_completion_reservations, 1);
    assert_eq!(
        backend.active_sdma_streams[&stream],
        VecDeque::from([predecessor])
    );
    assert_eq!(backend.published_sdma_submissions.len(), 1);
    assert!(backend.published_sdma_submissions.contains(&predecessor));
    let ActiveSdmaPhaseV1::DirectionalPublished(native) = &backend.active_sdma[&predecessor].phase
    else {
        panic!("predecessor must remain published");
    };
    let DirectionalSdmaSubmissionOwnerV1::Scripted(native) = native.as_ref() else {
        panic!("scripted predecessor");
    };
    assert_eq!(
        (
            super::sdma_host_write_tests::host_observation(&native.pair().host).0,
            native.pair().device.scripted_owner_id().unwrap()
        ),
        owners
    );
    for allocation in [host, device] {
        assert!(matches!(backend.allocations[&allocation].sdma_storage,
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(id)) if id == predecessor));
        let custody = &backend.allocation_custody[&allocation];
        assert_eq!(
            custody.owners,
            VecDeque::from([RuntimeAllocationCustodyOwnerV1 {
                submission: predecessor,
                stream,
                kind: RuntimeAllocationCustodyKindV1::Sdma,
            }])
        );
        assert_eq!(custody.owner_counts, [0, 1]);
    }
    backend.release_submission_v1(successor).unwrap();
    assert_eq!(
        backend.stream_submission_tails.get(&stream),
        Some(&predecessor),
        "releasing a failed tail must preserve the unfinished stream prefix"
    );
    let next = backend
        .copy_async_v1(stream, other_source, other_destination, &[])
        .unwrap();
    assert_eq!(backend.active_sdma[&next].dependencies, [predecessor]);
    assert!(matches!(
        backend.active_sdma[&next].phase,
        ActiveSdmaPhaseV1::Ready
    ));
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
        steps_before
    );
    assert_eq!(
        backend.poll_v1(predecessor).unwrap(),
        BackendPollV1::Succeeded
    );
    backend.flush_stream_v1(stream).unwrap();
    assert_eq!(backend.poll_v1(next).unwrap(), BackendPollV1::Succeeded);
    backend.release_event_v1(event).unwrap();
    for submission in [next, failed_dependency, first_other, predecessor] {
        backend.release_submission_v1(submission).unwrap();
    }
    backend.release_allocation_v1(other_host).unwrap();
    backend
        .allocations
        .get_mut(&other_device)
        .unwrap()
        .sdma_backed = false;
    backend.release_allocation_v1(other_device).unwrap();
    backend.destroy_stream_v1(other_stream).unwrap();
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, None);
    drop(ManuallyDrop::into_inner(backend));
}

fn inspect_bounded_prefix(kind: usize, fault: usize) {
    let terminal = matches!(fault, 0 | 3 | 4 | 6 | 7);
    let mut steps = vec![submit_step(kind, ScriptedFailureModeV1::Success)];
    steps.extend(completion_steps(kind));
    match fault {
        0 | 1 | 6 | 7 => {}
        2 => steps.push(submit_at(kind, 8, ScriptedFailureModeV1::Retryable)),
        3 => steps.push(submit_at(kind, 8, ScriptedFailureModeV1::ProcessTeardown)),
        4 => steps.push(if kind == 2 {
            ScriptedSdmaStepV1::SubmitSameDevicePanic
        } else {
            ScriptedSdmaStepV1::SubmitPanic
        }),
        5 => {
            steps.push(submit_at(kind, 8, ScriptedFailureModeV1::Success));
            steps.extend(completion_steps(kind));
        }
        _ => panic!("unknown prefix scenario"),
    }
    if !terminal {
        if kind == 2 {
            steps.extend(scripted_same_device_release_steps_v1());
        } else {
            steps.extend(scripted_release_steps_v1());
        }
    }
    let (backend, stream, first, second) = if kind == 2 {
        scripted_same_device_backend_v1(32, steps)
    } else {
        scripted_direct_backend_v1(32, steps)
    };
    let mut backend = ManuallyDrop::new(backend);
    // This bounded test policy changes window geometry, not completion state.
    // Both the prefix and continuation still run the real admission/settlement code.
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
    let (source, destination) = scripted_copy_regions_v1(source_id, destination_id, 16);
    let source_bytes: Vec<_> = (1..=32).collect();
    let destination_bytes = [0xa5; 32];
    for (allocation, bytes) in [
        (source_id, source_bytes.as_slice()),
        (destination_id, &destination_bytes),
    ] {
        let record = backend.allocations.get_mut(&allocation).unwrap();
        match &mut record.sdma_storage {
            KfdRuntimeSdmaStorageV1::Host(owner) => {
                owner.scripted_bytes_mut().unwrap().copy_from_slice(bytes)
            }
            KfdRuntimeSdmaStorageV1::Device(owner) => {
                owner.scripted_bytes_mut().unwrap().copy_from_slice(bytes)
            }
            _ => panic!("expected initial scripted backing"),
        }
        record.sdma_shadow_dirty = true;
    }
    let owner_ids = (
        storage_owner_id(&backend.allocations[&source_id].sdma_storage),
        storage_owner_id(&backend.allocations[&destination_id].sdma_storage),
    );
    let copy = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    let event = backend.record_event_v1(stream, copy).unwrap();
    assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Pending);
    assert_eq!(backend.active_sdma[&copy].completed_bytes, 8);
    assert!(matches!(
        backend.active_sdma[&copy].phase,
        ActiveSdmaPhaseV1::Ready
    ));
    let mut expected = destination_bytes;
    expected[..8].copy_from_slice(&source_bytes[..8]);
    assert_eq!(
        storage_bytes(&backend.allocations[&source_id].sdma_storage),
        source_bytes
    );
    assert_eq!(
        storage_bytes(&backend.allocations[&destination_id].sdma_storage),
        expected
    );
    let mut retained_destination = destination_id;
    if fault == 0 {
        // Deliberate preparation-state corruption after genuine prefix completion.
        backend
            .allocations
            .get_mut(&source_id)
            .unwrap()
            .native_dirty
            .push(NativeDirtyExtentV1 {
                compute_lane: 0,
                data_index: 0,
                allocation_offset: 0,
                data_offset: 0,
                byte_len: 1,
            });
    } else if fault == 1 {
        // Inject a failed plan through the bounded test policy, not an OOM claim.
        backend
            .scripted_sdma
            .as_mut()
            .unwrap()
            .publication_byte_limit = Some(0);
    } else if fault == 6 {
        // Corrupt the lookup identity without dropping or extracting a native owner.
        let record = backend.allocations.remove(&destination_id).unwrap();
        retained_destination = u64::MAX;
        assert!(!backend.allocations.contains_key(&retained_destination));
        backend.allocations.insert(retained_destination, record);
        assert!(!backend.sdma_release_custody_is_intact_v1(copy));
    } else if fault == 7 {
        backend.allocations.get_mut(&source_id).unwrap().kind = RuntimeMemoryKindV1::HostVisible;
        backend.allocations.get_mut(&destination_id).unwrap().kind =
            RuntimeMemoryKindV1::HostVisible;
        assert!(!backend.sdma_release_custody_is_intact_v1(copy));
    }
    let facts = |backend: &KfdRuntimeBackendV1| {
        (
            backend.active_sdma_streams.clone(),
            backend.stream_submission_tails.clone(),
            backend.sdma_dependency_retain_counts.clone(),
            backend.event_submission_retain_counts.clone(),
            backend.sdma_completion_reservations,
            backend
                .allocation_custody
                .iter()
                .map(|(&id, custody)| {
                    (
                        id,
                        (
                            custody.owners.clone(),
                            custody.sole_stream,
                            custody.owner_counts,
                        ),
                    )
                })
                .collect::<HashMap<_, _>>(),
        )
    };
    let before = facts(&backend);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        backend.flush_stream_v1(stream)
    }));
    if fault == 4 {
        assert_eq!(
            outcome
                .expect_err("rooted submit panic")
                .downcast_ref::<&str>()
                .copied(),
            Some("scripted SDMA publication panic")
        );
    } else if terminal {
        let Ok(Err(RuntimeBackendFailureV1::Terminal(error))) = outcome else {
            panic!("expected terminal publication result");
        };
        if fault == 3 {
            assert_eq!(error.detail(), submit_teardown_detail(kind));
        }
    } else if fault == 5 {
        assert!(matches!(outcome, Ok(Ok(()))));
        assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
    } else {
        assert!(matches!(
            outcome,
            Ok(Err(RuntimeBackendFailureV1::Quiescent(_)))
        ));
        assert!(backend.quiescent_sdma_submissions.contains(&copy));
        assert!(matches!(
            backend.poll_v1(copy),
            Err(RuntimeBackendFailureV1::Quiescent(_))
        ));
    }
    if terminal {
        assert!(backend.terminal);
        assert_eq!(facts(&backend), before);
        assert_eq!(backend.active_sdma[&copy].completed_bytes, 8);
        assert!(matches!(
            backend.active_sdma[&copy].phase,
            ActiveSdmaPhaseV1::Quarantined
        ));
        assert!(!backend.submissions.contains_key(&copy));
        assert!(!backend.quiescent_sdma_submissions.contains(&copy));
        if matches!(fault, 0 | 6 | 7) {
            assert!(backend.terminal_sdma_custody.is_none());
            assert!(
                backend
                    .scripted_sdma
                    .as_ref()
                    .unwrap()
                    .publication_custody()
                    .is_none()
            );
            assert_eq!(
                (
                    storage_owner_id(&backend.allocations[&source_id].sdma_storage),
                    storage_owner_id(&backend.allocations[&retained_destination].sdma_storage)
                ),
                owner_ids
            );
        } else {
            assert_eq!(backend.active_sdma[&copy].window_bytes, 8);
            assert_eq!(
                backend.active_sdma[&copy]
                    .window_requests
                    .as_ref()
                    .unwrap()
                    .as_slice(),
                &[DirectSdmaCopyRequestV1 {
                    source_offset: 8,
                    destination_offset: 8,
                    copy_bytes: 8
                }]
            );
            assert_eq!(terminal_pair_ids(&backend, kind, fault == 4), owner_ids);
            for allocation in [source_id, destination_id] {
                assert!(matches!(backend.allocations[&allocation].sdma_storage,
                    KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(id)) if id == copy));
            }
        }
        assert!(matches!(
            backend.release_event_v1(event),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            backend.flush_stream_v1(stream),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert_eq!(facts(&backend), before);
        let driver = backend.scripted_sdma.as_ref().unwrap();
        assert_eq!(driver.live_owner_count(), 2);
        assert_eq!(driver.unexpected_drops(), 0);
        assert_eq!(driver.remaining_steps(), 0);
        eprintln!("bounded prefix custody inspected; dropping terminal backend");
        drop(ManuallyDrop::into_inner(backend));
        panic!("unfinished prefix Drop returned");
    }
    assert!(!backend.terminal);
    assert!(!backend.active_sdma.contains_key(&copy));
    assert!(backend.allocation_custody.is_empty());
    assert!(backend.active_sdma_streams.is_empty());
    assert_eq!(backend.sdma_completion_reservations, 0);
    if fault == 5 {
        expected[..16].copy_from_slice(&source_bytes[..16]);
    }
    assert_eq!(
        storage_bytes(&backend.allocations[&source_id].sdma_storage),
        source_bytes
    );
    assert_eq!(
        storage_bytes(&backend.allocations[&destination_id].sdma_storage),
        expected
    );
    assert_eq!(
        (
            storage_owner_id(&backend.allocations[&source_id].sdma_storage),
            storage_owner_id(&backend.allocations[&destination_id].sdma_storage)
        ),
        owner_ids
    );
    backend.release_event_v1(event).unwrap();
    if kind == 2 {
        clean_scripted_same_device_backend_v1(&mut backend, stream, first, second, Some(copy));
    } else {
        clean_scripted_direct_backend_v1(&mut backend, stream, first, second, Some(copy));
    }
    drop(ManuallyDrop::into_inner(backend));
    eprintln!("bounded prefix settled and cleaned without terminal custody");
}

#[test]
fn asynchronous_copy_bounded_prefix_distinguishes_terminal_quiescent_and_success() {
    const CHILD: &str = "FE2O3_TEST_SDMA_BOUNDED_PREFIX";
    const TEST: &str = "kfd_backend::tests::sdma_publication_custody_tests::asynchronous_copy_bounded_prefix_distinguishes_terminal_quiescent_and_success";
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
        inspect_bounded_prefix(case / 8, case % 8);
        return;
    }
    for case in 0..24 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, case.to_string())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        if matches!(case % 8, 0 | 3 | 4 | 6 | 7) {
            assert_eq!(output.status.signal(), Some(6), "case {case}: {stderr}");
            assert!(
                stderr.contains("bounded prefix custody inspected; dropping terminal backend"),
                "case {case}: {stderr}"
            );
            assert!(!stderr.contains("unfinished prefix Drop returned"));
        } else {
            assert!(output.status.success(), "case {case}: {stderr}");
            assert!(stderr.contains("bounded prefix settled and cleaned without terminal custody"));
        }
        eprintln!("verified bounded prefix case {case}: exact failure class and cleanup");
    }
}

fn inspect_submit_teardown_and_drop(kind: usize, flush: bool, unwind: bool) {
    let mut steps = Vec::new();
    if flush {
        steps.push(submit_step(kind, ScriptedFailureModeV1::Success));
        if kind == 2 {
            steps.push(ScriptedSdmaStepV1::PollSameDevice(
                ScriptedSameDeviceExecutionOutcomeV1::Completed {
                    copy_bytes: None,
                    requests: None,
                    swap_allocations: false,
                },
            ));
            steps.push(ScriptedSdmaStepV1::RetireSameDevice(
                ScriptedFailureModeV1::Success,
            ));
        } else {
            steps.push(ScriptedSdmaStepV1::Poll(
                ScriptedExecutionOutcomeV1::Completed {
                    direction: None,
                    copy_bytes: None,
                },
            ));
            steps.push(ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success));
        }
    }
    steps.push(if unwind {
        if kind == 2 {
            ScriptedSdmaStepV1::SubmitSameDevicePanic
        } else {
            ScriptedSdmaStepV1::SubmitPanic
        }
    } else {
        submit_step(kind, ScriptedFailureModeV1::ProcessTeardown)
    });
    let (backend, stream, first, second) = if kind == 2 {
        scripted_same_device_backend_v1(16, steps)
    } else {
        scripted_direct_backend_v1(16, steps)
    };
    let mut backend = ManuallyDrop::new(backend);
    let (source_id, destination_id) = if kind == 1 {
        (second, first)
    } else {
        (first, second)
    };
    let owner_ids = (
        storage_owner_id(&backend.allocations[&source_id].sdma_storage),
        storage_owner_id(&backend.allocations[&destination_id].sdma_storage),
    );
    let (source, destination) = scripted_copy_regions_v1(source_id, destination_id, 8);
    let mut predecessor = None;
    let mut roster_pointer = None;
    let copy;
    let outcome = if flush {
        let prior = backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();
        copy = backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();
        assert_eq!(backend.poll_v1(prior).unwrap(), BackendPollV1::Succeeded);
        predecessor = Some(prior);
        roster_pointer = Some(backend.active_sdma[&copy].dependencies.as_ptr() as usize);
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            backend.flush_stream_v1(stream)
        }))
    } else {
        copy = backend.next_handle;
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            backend
                .copy_async_v1(stream, source, destination, &[])
                .map(|_| ())
        }));
        assert_eq!(backend.next_handle, copy + 1);
        outcome
    };
    if unwind {
        let panic = outcome.expect_err("publication panic must propagate");
        assert_eq!(
            panic.downcast_ref::<&str>().copied(),
            Some("scripted SDMA publication panic")
        );
    } else {
        let Ok(Err(RuntimeBackendFailureV1::Terminal(error))) = outcome else {
            panic!("expected terminal publication result");
        };
        assert_eq!(error.detail(), submit_teardown_detail(kind));
    }
    assert!(
        backend.active_sdma.contains_key(&copy),
        "terminal publication lost the admitted descriptor"
    );
    let active = &backend.active_sdma[&copy];
    assert!(matches!(active.phase, ActiveSdmaPhaseV1::Quarantined));
    assert_eq!(active.window_bytes, 8);
    assert_eq!(active.window_requests.as_ref().unwrap().packet_count(), 1);
    assert_eq!((active.source_offset, active.destination_offset), (0, 0));
    assert_eq!(
        active.window_requests.as_ref().unwrap().as_slice(),
        &[DirectSdmaCopyRequestV1 {
            source_offset: 0,
            destination_offset: 0,
            copy_bytes: 8
        }]
    );
    assert_eq!(
        (active.id, active.stream, active.source, active.destination),
        (copy, stream, source_id, destination_id)
    );
    assert_eq!((active.byte_len, active.completed_bytes), (8, 0));
    assert_eq!(active.prior_stream_submission, predecessor);
    assert_eq!(
        active.dependencies,
        predecessor.into_iter().collect::<Vec<_>>()
    );
    assert_eq!(active.dependency_cursor, usize::from(flush));
    if let Some(pointer) = roster_pointer {
        assert_eq!(active.dependencies.as_ptr() as usize, pointer);
    }
    assert!(backend.terminal);
    assert_eq!(backend.active_sdma_streams[&stream], VecDeque::from([copy]));
    assert_eq!(backend.stream_submission_tails[&stream], copy);
    assert_eq!(backend.sdma_completion_reservations, 1);
    assert!(!backend.submissions.contains_key(&copy));
    assert!(!backend.quiescent_sdma_submissions.contains(&copy));
    assert!(backend.published_sdma_submissions.is_empty());
    for allocation in [source_id, destination_id] {
        let custody = &backend.allocation_custody[&allocation];
        assert_eq!(
            custody.owners,
            VecDeque::from([RuntimeAllocationCustodyOwnerV1 {
                submission: copy,
                stream,
                kind: RuntimeAllocationCustodyKindV1::Sdma
            }])
        );
        assert_eq!(custody.owner_counts, [0, 1]);
        assert!(matches!(backend.allocations[&allocation].sdma_storage,
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(id)) if id == copy));
    }
    if let Some(prior) = predecessor {
        assert_eq!(backend.sdma_dependency_retain_counts[&prior], 1);
    }
    assert_eq!(terminal_pair_ids(&backend, kind, unwind), owner_ids);
    for _ in 0..2 {
        assert!(matches!(
            backend.poll_v1(copy),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            backend.wait_v1(copy, Instant::now()),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            backend.flush_stream_v1(stream),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            backend.cancel_v1(copy),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        let driver = backend.scripted_sdma.as_ref().unwrap();
        assert_eq!(driver.live_owner_count(), 2);
        assert_eq!(driver.unexpected_drops(), 0);
        assert_eq!(driver.remaining_steps(), 0);
        assert!(backend.active_sdma.contains_key(&copy));
    }
    eprintln!("publication custody inspected; dropping unfinished backend");
    drop(ManuallyDrop::into_inner(backend));
    panic!("unfinished publication Drop returned");
}

#[test]
fn asynchronous_copy_publication_terminal_retains_exact_descriptor_and_pair_until_abort() {
    const CHILD: &str = "FE2O3_TEST_SDMA_PUBLICATION_CUSTODY";
    const TEST: &str = "kfd_backend::tests::sdma_publication_custody_tests::asynchronous_copy_publication_terminal_retains_exact_descriptor_and_pair_until_abort";
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
        inspect_submit_teardown_and_drop(case / 4, case % 2 == 1, case % 4 >= 2);
        unreachable!();
    }
    for case in 0..12 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, case.to_string())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "case {case}: {stderr}");
        assert!(
            stderr.contains("publication custody inspected; dropping unfinished backend"),
            "case {case}: {stderr}"
        );
        assert!(!stderr.contains("unfinished publication Drop returned"));
        eprintln!(
            "verified publication case {case}: exact descriptor and owners retained; real Drop SIGABRT"
        );
    }
}
