//! Public asynchronous copy observation over scripted owners, not GPU execution.

use super::*;
use std::mem::ManuallyDrop;
use std::os::unix::process::ExitStatusExt;

#[test]
fn asynchronous_copy_completion_preserves_later_same_stream_owners() {
    let mut steps = (0..3)
        .flat_map(|_| {
            [
                scripted_submit_step_v1(
                    Gfx942PersistentSdmaDirectionV1::HostToDevice,
                    0,
                    0,
                    8,
                    ScriptedFailureModeV1::Success,
                ),
                ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                    direction: None,
                    copy_bytes: None,
                }),
                ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
            ]
        })
        .collect::<Vec<_>>();
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(16, steps);
    let (source, destination) = scripted_copy_regions_v1(host, device, 8);
    let copies: [u64; 3] = std::array::from_fn(|_| {
        backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap()
    });
    for (index, &copy) in copies.iter().enumerate() {
        let later = &copies[index + 1..];
        let rosters = later
            .iter()
            .map(|id| {
                let active = &backend.active_sdma[id];
                (
                    *id,
                    active.dependencies.clone(),
                    active.dependencies.as_ptr() as usize,
                    active.dependency_cursor,
                )
            })
            .collect::<Vec<_>>();
        backend.flush_stream_v1(stream).unwrap();
        assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
        assert!(!backend.active_sdma.contains_key(&copy));
        assert_eq!(backend.sdma_completion_reservations, later.len());
        assert_eq!(
            backend
                .active_sdma_streams
                .get(&stream)
                .map(|queue| queue.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default(),
            later
        );
        assert_eq!(backend.stream_submission_tails[&stream], copies[2]);
        for allocation in [host, device] {
            let owners = backend
                .allocation_custody
                .get(&allocation)
                .map(|custody| {
                    assert_eq!(custody.sole_stream, Some(stream));
                    assert_eq!(custody.owner_counts, [0, later.len()]);
                    custody
                        .owners
                        .iter()
                        .map(|owner| owner.submission)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            assert_eq!(owners, later);
        }
        for (id, dependencies, pointer, cursor) in rosters {
            let active = &backend.active_sdma[&id];
            assert!(matches!(active.phase, ActiveSdmaPhaseV1::Ready));
            assert_eq!(active.dependencies, dependencies);
            assert_eq!(active.dependencies.as_ptr() as usize, pointer);
            assert_eq!(active.dependency_cursor, cursor);
        }
    }
    assert!(backend.sdma_dependency_retain_counts.is_empty());
    for copy in copies.into_iter().rev() {
        backend.release_submission_v1(copy).unwrap();
    }
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, None);
}

fn inspect_completion_preflight_and_drop(mutation: usize) {
    let steps = (0..3).flat_map(|_| {
        [
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                8,
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
            ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
        ]
    });
    let (backend, stream, host, device) = scripted_direct_backend_v1(16, steps);
    let mut backend = ManuallyDrop::new(backend);
    let (source, destination) = scripted_copy_regions_v1(host, device, 8);
    let mut events = Vec::new();
    for _ in 0..2 {
        let prior = backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();
        assert_eq!(backend.poll_v1(prior).unwrap(), BackendPollV1::Succeeded);
        events.push(backend.record_event_v1(stream, prior).unwrap());
    }
    let copy = backend
        .copy_async_v1(stream, source, destination, &events)
        .unwrap();
    let dependencies = backend.active_sdma[&copy].dependencies.clone();
    assert_eq!(dependencies.len(), 2);
    let ActiveSdmaPhaseV1::DirectionalPublished(owner) = &backend.active_sdma[&copy].phase else {
        panic!("published copy")
    };
    let DirectionalSdmaSubmissionOwnerV1::Scripted(owner) = &**owner else {
        panic!("scripted owner")
    };
    let owner_ids = (
        super::sdma_host_write_tests::host_observation(&owner.pair().host).0,
        owner.pair().device.scripted_owner_id().unwrap(),
    );
    // Deliberate private-state corruption exercises zero-prefix preflight failure;
    // these mutations are not presented as reachable through legal admission.
    match mutation {
        0 => {
            backend
                .sdma_dependency_retain_counts
                .remove(&dependencies[1]);
        }
        1 => backend
            .allocation_custody
            .get_mut(&device)
            .unwrap()
            .owners
            .clear(),
        2 => {
            backend
                .allocation_custody
                .get_mut(&host)
                .unwrap()
                .owner_counts[RuntimeAllocationCustodyKindV1::Sdma.index()] = 0
        }
        3 => backend
            .active_sdma_streams
            .get_mut(&stream)
            .unwrap()
            .clear(),
        4 => backend.sdma_completion_reservations = 0,
        5 => backend
            .active_sdma
            .get_mut(&copy)
            .unwrap()
            .dependencies
            .push(dependencies[0]),
        6 => backend
            .active_sdma_streams
            .get_mut(&stream)
            .unwrap()
            .push_back(copy),
        7 => {
            let custody = backend.allocation_custody.get_mut(&device).unwrap();
            custody.owners.push_back(custody.owners[0]);
            custody.owner_counts[RuntimeAllocationCustodyKindV1::Sdma.index()] += 1;
        }
        8 => {
            backend
                .submissions
                .get_mut(&dependencies[1])
                .unwrap()
                .status = BackendPollV1::Pending
        }
        _ => panic!("unknown completion mutation"),
    }
    let facts = |backend: &KfdRuntimeBackendV1| {
        let active = &backend.active_sdma[&copy];
        (
            backend.sdma_dependency_retain_counts.clone(),
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
            backend.sdma_completion_reservations,
            backend.event_submission_retain_counts.clone(),
            backend.stream_submission_tails.clone(),
            (
                active.dependencies.clone(),
                active.dependencies.as_ptr() as usize,
                active.completed_bytes,
                active.window_bytes,
                active.dependency_cursor,
            ),
        )
    };
    let before = facts(&backend);
    assert!(matches!(
        backend.poll_v1(copy),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(backend.terminal);
    assert_eq!(facts(&backend), before);
    assert!(matches!(
        backend.active_sdma[&copy].phase,
        ActiveSdmaPhaseV1::Quarantined
    ));
    assert!(!backend.submissions.contains_key(&copy));
    assert!(!backend.published_sdma_submissions.contains(&copy));
    assert!(backend.terminal_sdma_custody.is_none());
    let KfdRuntimeSdmaStorageV1::Host(host_owner) = &backend.allocations[&host].sdma_storage else {
        panic!("host restored before logical preflight")
    };
    let KfdRuntimeSdmaStorageV1::Device(device_owner) = &backend.allocations[&device].sdma_storage
    else {
        panic!("device restored before logical preflight")
    };
    assert_eq!(
        (
            super::sdma_host_write_tests::host_observation(host_owner).0,
            device_owner.scripted_owner_id().unwrap()
        ),
        owner_ids
    );
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert_eq!(driver.remaining_steps(), 0);
    assert_eq!(driver.live_owner_count(), 2);
    assert_eq!(driver.unexpected_drops(), 0);
    assert!(matches!(
        backend.poll_v1(copy),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert_eq!(facts(&backend), before);
    eprintln!("completion preflight custody inspected; dropping unfinished backend");
    drop(ManuallyDrop::into_inner(backend));
    panic!("unfinished completion Drop returned");
}

#[test]
fn asynchronous_copy_completion_preflight_releases_no_prefix_on_invalid_custody() {
    const CHILD: &str = "FE2O3_TEST_SDMA_COMPLETION_PREFLIGHT";
    const TEST: &str = "kfd_backend::tests::sdma_observation_custody_tests::asynchronous_copy_completion_preflight_releases_no_prefix_on_invalid_custody";
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
        inspect_completion_preflight_and_drop(case.parse().unwrap());
        unreachable!();
    }
    for case in 0..9 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, case.to_string())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "mutation {case}: {stderr}");
        assert!(
            stderr.contains("completion preflight custody inspected; dropping unfinished backend"),
            "mutation {case}: {stderr}"
        );
        assert!(!stderr.contains("unfinished completion Drop returned"));
        eprintln!(
            "verified completion mutation {case}: no logical release prefix; exact native owners restored; Drop SIGABRT"
        );
    }
}

fn inspect_retirement_failure_and_drop(h2d: bool, wait: bool, nested: bool, fault: usize) {
    let direction = if h2d {
        Gfx942PersistentSdmaDirectionV1::HostToDevice
    } else {
        Gfx942PersistentSdmaDirectionV1::DeviceToHost
    };
    let completed = match fault {
        4 => ScriptedExecutionOutcomeV1::Retryable,
        5 => ScriptedExecutionOutcomeV1::ProcessTeardown,
        _ => ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: (fault == 3).then_some(4),
        },
    };
    let mut steps = vec![
        scripted_submit_step_v1(direction, 0, 0, 8, ScriptedFailureModeV1::Success),
        if wait && !nested {
            ScriptedSdmaStepV1::Wait(completed)
        } else {
            ScriptedSdmaStepV1::Poll(completed)
        },
    ];
    match fault {
        0 => steps.push(ScriptedSdmaStepV1::RetirePanic),
        1 => steps.push(ScriptedSdmaStepV1::RetireCompletedRetry),
        2 => steps.push(ScriptedSdmaStepV1::Retire(
            ScriptedFailureModeV1::ProcessTeardown,
        )),
        3..=5 => {}
        _ => panic!("unknown failure scenario"),
    }
    let (backend, stream, host, device) = scripted_direct_backend_v1(16, steps);
    let mut backend = ManuallyDrop::new(backend);
    let (source, destination) = if h2d { (host, device) } else { (device, host) };
    let (source, destination) = scripted_copy_regions_v1(source, destination, 8);
    let copy = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    backend.flush_stream_v1(stream).unwrap();
    let event = backend.record_event_v1(stream, copy).unwrap();
    let mut target = copy;
    if nested {
        // These are genuinely admitted same-stream ancestors of the observation,
        // not fabricated retained records. Only the earliest copy is published.
        for _ in 0..2 {
            target = backend
                .copy_async_v1(stream, source, destination, &[])
                .unwrap();
        }
    }
    let descriptors = |backend: &KfdRuntimeBackendV1| {
        backend
            .active_sdma
            .iter()
            .map(|(&id, active)| {
                (
                    id,
                    (
                        active.stream,
                        active.prior_stream_submission,
                        active.source,
                        active.destination,
                        active.dependencies.clone(),
                        active.dependencies.as_ptr() as usize,
                        active.dependency_cursor,
                        active.dependency_depth,
                    ),
                )
            })
            .collect::<HashMap<_, _>>()
    };
    let descriptors_before = descriptors(&backend);
    let tails_before = backend.stream_submission_tails.clone();
    let facts = |backend: &KfdRuntimeBackendV1| {
        let active = &backend.active_sdma[&copy];
        (
            (
                active.id,
                active.stream,
                active.prior_stream_submission,
                active.source,
                active.destination,
            ),
            (
                active.source_offset,
                active.destination_offset,
                active.byte_len,
                active.completed_bytes,
                active.window_bytes,
            ),
            (
                active.dependencies.clone(),
                active.dependencies.as_ptr() as usize,
                active.dependency_cursor,
                active.dependency_depth,
            ),
            active
                .window_requests
                .as_ref()
                .map(|requests| (requests.packet_count(), *requests.first())),
            backend.active_sdma_streams.clone(),
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
    let pair_ids = |pair: &DirectionalSdmaPairOwnerV1| {
        (
            pair.device.scripted_owner_id().unwrap(),
            super::sdma_host_write_tests::host_observation(&pair.host).0,
        )
    };
    let ActiveSdmaPhaseV1::DirectionalPublished(native) = &backend.active_sdma[&copy].phase else {
        panic!("copy must be published before the retirement fault");
    };
    let DirectionalSdmaSubmissionOwnerV1::Scripted(native) = &**native else {
        panic!("scripted owner required");
    };
    let owners = pair_ids(native.pair());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if wait {
            backend.wait_v1(target, Instant::now() + Duration::from_secs(1))
        } else {
            backend.poll_v1(target)
        }
    }));
    if fault == 0 {
        let panic = result.expect_err("retirement panic must propagate");
        assert_eq!(
            panic.downcast_ref::<&str>().copied(),
            Some("scripted SDMA retirement panic")
        );
    } else {
        assert!(matches!(
            result,
            Ok(Err(RuntimeBackendFailureV1::Terminal(_)))
        ));
    }
    assert!(
        backend.active_sdma.contains_key(&copy),
        "retirement unwind lost the accepted copy descriptor"
    );
    assert_eq!(facts(&backend), before);
    assert_eq!(descriptors(&backend), descriptors_before);
    assert_eq!(backend.stream_submission_tails, tails_before);
    assert!(matches!(
        backend.active_sdma[&copy].phase,
        ActiveSdmaPhaseV1::Quarantined
    ));
    for (&id, active) in &backend.active_sdma {
        if id != copy {
            assert!(matches!(active.phase, ActiveSdmaPhaseV1::Ready));
        }
    }
    for allocation in [source.allocation, destination.allocation] {
        assert!(matches!(backend.allocations[&allocation].sdma_storage,
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(id)) if id == copy));
    }
    assert!(backend.terminal);
    assert!(!backend.published_sdma_submissions.contains(&copy));
    assert!(!backend.submissions.contains_key(&copy));
    let driver = backend.scripted_sdma.as_ref().unwrap();
    let pair = if fault == 0 {
        driver.retirement_custody().unwrap().pair()
    } else {
        use kfd_backend_sdma_seam::ScriptedTerminalCustodyV1;
        match backend.terminal_sdma_custody.as_ref().unwrap() {
            KfdRuntimeTerminalSdmaCustodyV1::Completed(
                DirectionalSdmaCompletedOwnerV1::Scripted(owner),
            )
            | KfdRuntimeTerminalSdmaCustodyV1::Scripted(ScriptedTerminalCustodyV1::Completed(
                DirectionalSdmaCompletedOwnerV1::Scripted(owner),
            )) => owner.pair(),
            KfdRuntimeTerminalSdmaCustodyV1::Pending(
                DirectionalSdmaSubmissionOwnerV1::Scripted(owner),
            )
            | KfdRuntimeTerminalSdmaCustodyV1::Scripted(ScriptedTerminalCustodyV1::Submission(
                DirectionalSdmaSubmissionOwnerV1::Scripted(owner),
            )) => owner.pair(),
            _ => panic!("exact directional pair must remain in terminal custody"),
        }
    };
    assert_eq!(pair_ids(pair), owners);
    assert_eq!(driver.live_owner_count(), 2);
    assert_eq!(driver.unexpected_drops(), 0);
    assert_eq!(driver.remaining_steps(), 0);
    for _ in 0..2 {
        assert!(matches!(
            backend.poll_v1(copy),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            backend.flush_stream_v1(stream),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            backend.wait_v1(target, Instant::now()),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            backend.cancel_v1(target),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            backend.release_event_v1(event),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert_eq!(facts(&backend), before);
        assert_eq!(descriptors(&backend), descriptors_before);
        assert_eq!(backend.stream_submission_tails, tails_before);
        assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 0);
    }
    eprintln!("async retirement custody inspected; dropping unfinished backend");
    drop(ManuallyDrop::into_inner(backend));
    panic!("unfinished asynchronous copy Drop returned");
}

#[test]
fn asynchronous_copy_retirement_unwind_retains_descriptor_and_pair_until_abort() {
    const CHILD: &str = "FE2O3_TEST_ASYNC_COPY_RETIREMENT_UNWIND";
    const TEST: &str = "kfd_backend::tests::sdma_observation_custody_tests::asynchronous_copy_retirement_unwind_retains_descriptor_and_pair_until_abort";
    const CASES: [(&str, bool, bool, bool); 8] = [
        ("h2d-poll", true, false, false),
        ("h2d-wait", true, true, false),
        ("d2h-poll", false, false, false),
        ("d2h-wait", false, true, false),
        ("h2d-nested-poll", true, false, true),
        ("h2d-nested-wait", true, true, true),
        ("d2h-nested-poll", false, false, true),
        ("d2h-nested-wait", false, true, true),
    ];
    if let Some(mode) = std::env::var_os(CHILD) {
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
        let (_, h2d, wait, nested) = CASES
            .into_iter()
            .find(|case| case.0 == mode.to_str().unwrap())
            .unwrap();
        let fault = std::env::var("FE2O3_TEST_ASYNC_COPY_FAULT")
            .unwrap()
            .parse()
            .unwrap();
        inspect_retirement_failure_and_drop(h2d, wait, nested, fault);
        unreachable!();
    }
    for (mode, _, _, _) in CASES {
        for fault in 0..6 {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(CHILD, mode)
                .env("FE2O3_TEST_ASYNC_COPY_FAULT", fault.to_string())
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(
                output.status.signal(),
                Some(6),
                "{mode} fault {fault}: {stderr}"
            );
            assert!(
                stderr.contains("async retirement custody inspected; dropping unfinished backend"),
                "{mode}: {stderr}"
            );
            assert!(
                !stderr.contains("unfinished asynchronous copy Drop returned"),
                "{mode}: {stderr}"
            );
            eprintln!(
                "verified {mode} fault {fault}: copy descriptor and pair retained; unfinished Drop SIGABRT"
            );
        }
    }
}
