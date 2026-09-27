//! Public admission plus private custody faults; no native kernel execution.

use super::*;
use std::mem::ManuallyDrop;
use std::os::unix::process::ExitStatusExt;

fn fixture(
    failed_dependency: bool,
    clean: bool,
) -> (KfdRuntimeBackendV1, u64, u64, u64, u64, u64, u64) {
    let mut steps = vec![scripted_submit_step_v1(
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        0,
        0,
        8,
        ScriptedFailureModeV1::Success,
    )];
    if clean {
        steps.extend([
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
            ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
        ]);
        steps.extend(scripted_release_steps_v1());
    }
    let (mut backend, copy_stream, host, device) = scripted_direct_backend_v1(16, steps);
    let (source, destination) = scripted_copy_regions_v1(host, device, 8);
    let active = backend
        .copy_async_v1(copy_stream, source, destination, &[])
        .unwrap();
    let dependency = backend
        .copy_async_v1(copy_stream, source, destination, &[])
        .unwrap();
    let dependency_event = backend.record_event_v1(copy_stream, dependency).unwrap();
    let stream = if failed_dependency {
        backend.create_stream_v1(7).unwrap()
    } else {
        copy_stream
    };
    let module = backend
        .load_module_v1(7, &synthetic_cov6::module())
        .unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();
    let pending =
        submit_scripted_read_v1(&mut backend, stream, kernel, device, 8, &[dependency_event]);
    let event = backend.record_event_v1(stream, pending).unwrap();
    backend.release_event_v1(dependency_event).unwrap();
    assert_eq!(
        &*backend.pending_compute[&pending].explicit_success_dependencies,
        &[dependency]
    );
    assert_eq!(
        &*backend.pending_compute[&pending].retained_allocations,
        &[device]
    );
    assert_eq!(backend.compute_dependency_retain_counts[&dependency], 1);
    if failed_dependency {
        assert_eq!(
            backend.cancel_v1(dependency).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
    }
    (backend, stream, active, dependency, pending, event, device)
}

#[derive(Debug, PartialEq, Eq)]
struct CustodyFacts {
    owners: VecDeque<RuntimeAllocationCustodyOwnerV1>,
    counts: [usize; 2],
    sole_stream: Option<u64>,
}

#[derive(Debug, PartialEq, Eq)]
struct Facts {
    pending: HashMap<u64, (String, [usize; 4])>,
    custody: HashMap<u64, CustodyFacts>,
    fifo: HashMap<u64, VecDeque<u64>>,
    tails: HashMap<u64, u64>,
    modules: HashMap<u64, usize>,
    dependencies: HashMap<u64, usize>,
    events: HashMap<u64, usize>,
    reservations: usize,
    terminal: HashMap<u64, (u64, BackendPollV1, usize, bool)>,
}

fn facts(backend: &KfdRuntimeBackendV1) -> Facts {
    Facts {
        pending: backend
            .pending_compute
            .iter()
            .map(|(&id, pending)| {
                (
                    id,
                    (
                        format!("{pending:?}"),
                        [
                            Arc::as_ptr(&pending.launch) as usize,
                            pending.retained_allocations.as_ptr() as usize,
                            pending.explicit_success_dependencies.as_ptr() as usize,
                            pending.quiescence_dependencies.as_ptr() as usize,
                        ],
                    ),
                )
            })
            .collect(),
        custody: backend
            .allocation_custody
            .iter()
            .map(|(&id, custody)| {
                (
                    id,
                    CustodyFacts {
                        owners: custody.owners.clone(),
                        counts: custody.owner_counts,
                        sole_stream: custody.sole_stream,
                    },
                )
            })
            .collect(),
        fifo: backend.pending_compute_streams.clone(),
        tails: backend.stream_submission_tails.clone(),
        modules: backend.compute_module_retain_counts.clone(),
        dependencies: backend.compute_dependency_retain_counts.clone(),
        events: backend.event_submission_retain_counts.clone(),
        reservations: backend.compute_completion_reservations,
        terminal: backend
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
            .collect(),
    }
}

fn native_facts(backend: &KfdRuntimeBackendV1, active: u64) -> (usize, u64, u64, u64, u64) {
    let copy = &backend.active_sdma[&active];
    let ActiveSdmaPhaseV1::DirectionalPublished(owner) = &copy.phase else {
        panic!("published ancestor")
    };
    let DirectionalSdmaSubmissionOwnerV1::Scripted(scripted) = owner.as_ref() else {
        panic!("scripted owner")
    };
    for allocation in [copy.source, copy.destination] {
        assert!(matches!(backend.allocations[&allocation].sdma_storage,
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(id)) if id == active));
    }
    (
        owner.as_ref() as *const _ as usize,
        super::sdma_host_write_tests::host_observation(&scripted.pair().host).0,
        scripted.pair().device.scripted_owner_id().unwrap(),
        copy.source,
        copy.destination,
    )
}

fn inspect_failure(failed_dependency: bool, mutation: usize) {
    let (backend, stream, active, dependency, pending, event, device) =
        fixture(failed_dependency, false);
    let mut backend = ManuallyDrop::new(backend);
    let module = backend.pending_compute[&pending].module;
    match mutation {
        0 => {
            backend.allocation_custody.remove(&device);
        }
        1 => {
            backend.compute_module_retain_counts.remove(&module);
        }
        2 => {
            backend.compute_dependency_retain_counts.remove(&dependency);
        }
        3 => {
            backend
                .allocation_custody
                .get_mut(&device)
                .unwrap()
                .owner_counts[0] = 0
        }
        4 => backend.compute_completion_reservations = 0,
        5 => {
            let queue = backend.pending_compute_streams.get_mut(&stream).unwrap();
            queue.push_back(pending);
        }
        6 => {
            let custody = backend.allocation_custody.get_mut(&device).unwrap();
            let owner = *custody.owners.back().unwrap();
            assert_eq!(owner.submission, pending);
            custody.owners.push_back(owner);
            custody.owner_counts[0] += 1;
        }
        7 => {
            backend
                .pending_compute
                .get_mut(&pending)
                .unwrap()
                .retained_allocations = Box::new([])
        }
        8 => {
            backend
                .pending_compute
                .get_mut(&pending)
                .unwrap()
                .retained_allocations = Box::new([device, device])
        }
        9 => {
            backend
                .pending_compute
                .get_mut(&pending)
                .unwrap()
                .explicit_success_dependencies = Box::new([dependency, dependency]);
            *backend
                .compute_dependency_retain_counts
                .get_mut(&dependency)
                .unwrap() += 1;
        }
        10 => {
            backend
                .pending_compute
                .get_mut(&pending)
                .unwrap()
                .quiescence_dependencies = Box::new([dependency])
        }
        11 => {
            backend
                .pending_compute
                .get_mut(&pending)
                .unwrap()
                .explicit_dependency_cursor = 2
        }
        12 => {
            backend
                .pending_compute
                .get_mut(&pending)
                .unwrap()
                .quiescence_cursor = usize::MAX
        }
        13 => {
            backend.submissions.insert(
                pending,
                SubmissionRecordV1 {
                    stream,
                    status: BackendPollV1::Failed { code: -2 },
                    dependency_depth: 1,
                    profile_dispatch_published: false,
                },
            );
        }
        14 => {
            backend.pending_compute_streams.remove(&stream);
        }
        15 => {
            backend
                .pending_compute_streams
                .get_mut(&stream)
                .unwrap()
                .clear();
        }
        16 => {
            backend
                .allocation_custody
                .get_mut(&device)
                .unwrap()
                .owners
                .back_mut()
                .unwrap()
                .kind = RuntimeAllocationCustodyKindV1::Sdma;
        }
        17 => {
            backend
                .allocation_custody
                .get_mut(&device)
                .unwrap()
                .owners
                .back_mut()
                .unwrap()
                .stream = 0;
        }
        18 => {
            backend.pending_compute.get_mut(&pending).unwrap().module = 0;
        }
        _ => panic!("unknown mutation"),
    }
    let before = facts(&backend);
    let recipe = Arc::downgrade(&backend.pending_compute[&pending].launch);
    let native = native_facts(&backend, active);
    let sdma = (
        backend.active_sdma_streams.clone(),
        backend.sdma_dependency_retain_counts.clone(),
        backend.sdma_completion_reservations,
        backend.published_sdma_submissions.clone(),
        backend.quiescent_sdma_submissions.clone(),
    );
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if failed_dependency {
            backend.poll_v1(pending).map(|_| ())
        } else {
            backend.cancel_v1(pending).map(|_| ())
        }
    }));
    assert!(
        backend.pending_compute.contains_key(&pending),
        "settlement detached the original recipe"
    );
    assert_eq!(
        facts(&backend),
        before,
        "settlement released a logical prefix"
    );
    assert_eq!(recipe.strong_count(), 1);
    assert!(matches!(
        outcome,
        Ok(Err(RuntimeBackendFailureV1::Terminal(_)))
    ));
    assert!(backend.terminal);
    for _ in 0..2 {
        assert!(matches!(
            backend.cancel_v1(pending),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            backend.poll_v1(pending),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            backend.wait_v1(pending, Instant::now()),
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
    assert_eq!(native_facts(&backend, active), native);
    assert_eq!(
        (
            backend.active_sdma_streams.clone(),
            backend.sdma_dependency_retain_counts.clone(),
            backend.sdma_completion_reservations,
            backend.published_sdma_submissions.clone(),
            backend.quiescent_sdma_submissions.clone(),
        ),
        sdma
    );
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert_eq!(driver.remaining_steps(), 0);
    assert_eq!(driver.live_owner_count(), 2);
    assert_eq!(driver.unexpected_drops(), 0);
    eprintln!("pending compute custody inspected; dropping unrepaired backend");
    drop(ManuallyDrop::into_inner(backend));
    panic!("pending compute Drop returned");
}

fn inspect_reindex_collision() {
    let (backend, stream, active, _, pending, event, device) = fixture(false, false);
    let mut backend = ManuallyDrop::new(backend);
    let kernel = backend.pending_compute[&pending].launch.kernel;
    let second = submit_scripted_read_v1(&mut backend, stream, kernel, device, 8, &[event]);
    let original = backend.pending_compute.remove(&pending).unwrap();
    let recipe = Arc::downgrade(&original.launch);
    let original_debug = format!("{original:?}");
    let original_ptrs = [
        Arc::as_ptr(&original.launch) as usize,
        original.retained_allocations.as_ptr() as usize,
        original.explicit_success_dependencies.as_ptr() as usize,
        original.quiescence_dependencies.as_ptr() as usize,
    ];
    // Move the other admitted recipe under the detached recipe's key. Neither
    // is cloned, and the collision must preserve both independent owners.
    let other = backend.pending_compute.remove(&second).unwrap();
    let other_recipe = Arc::downgrade(&other.launch);
    backend.pending_compute.insert(pending, other);
    let before = facts(&backend);
    let native = native_facts(&backend, active);
    assert!(matches!(
        backend.settle_failed_unpublished_compute_v1(original, -1),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    let retained = backend.terminal_pending_compute.as_ref().unwrap();
    assert_eq!(format!("{retained:?}"), original_debug);
    assert_eq!(
        [
            Arc::as_ptr(&retained.launch) as usize,
            retained.retained_allocations.as_ptr() as usize,
            retained.explicit_success_dependencies.as_ptr() as usize,
            retained.quiescence_dependencies.as_ptr() as usize
        ],
        original_ptrs
    );
    assert_eq!(recipe.strong_count(), 1);
    assert_eq!(other_recipe.strong_count(), 1);
    assert_eq!(facts(&backend), before);
    assert_eq!(native_facts(&backend, active), native);
    assert!(matches!(
        backend.cancel_v1(pending),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(matches!(
        backend.poll_v1(second),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(matches!(
        backend.shutdown_native_v1(),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert_eq!(facts(&backend), before);
    assert_eq!(
        backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
        0
    );
    eprintln!("pending compute custody inspected; dropping unrepaired backend");
    drop(ManuallyDrop::into_inner(backend));
    panic!("pending compute Drop returned");
}

#[test]
fn pending_compute_settlement_releases_only_the_unpublished_consumer() {
    for failed_dependency in [false, true] {
        let (backend, stream, active, dependency, pending, event, device) =
            fixture(failed_dependency, true);
        let mut backend = ManuallyDrop::new(backend);
        let remaining = backend.scripted_sdma.as_ref().unwrap().remaining_steps();
        let native = native_facts(&backend, active);
        let copy_stream = backend.active_sdma[&active].stream;
        let host = backend.active_sdma[&active].source;
        let module = backend.pending_compute[&pending].module;
        let recipe = Arc::downgrade(&backend.pending_compute[&pending].launch);
        let status = if failed_dependency {
            backend.poll_v1(pending).unwrap()
        } else {
            assert_eq!(
                backend.cancel_v1(pending).unwrap(),
                crate::BackendCancellationV1::Cancelled
            );
            backend.poll_v1(pending).unwrap()
        };
        assert_eq!(
            status,
            BackendPollV1::Failed {
                code: if failed_dependency { -1 } else { -2 }
            }
        );
        assert_eq!(recipe.strong_count(), 0);
        assert!(backend.pending_compute.is_empty());
        assert!(backend.terminal_pending_compute.is_none());
        assert!(backend.pending_compute_streams.is_empty());
        assert!(backend.compute_module_retain_counts.is_empty());
        assert!(backend.compute_dependency_retain_counts.is_empty());
        assert_eq!(backend.compute_completion_reservations, 0);
        assert_eq!(backend.allocation_custody[&device].owner_counts[0], 0);
        assert!(matches!(
            backend.active_sdma[&active].phase,
            ActiveSdmaPhaseV1::DirectionalPublished(_)
        ));
        assert_eq!(
            backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
            remaining
        );
        assert_eq!(native_facts(&backend, active), native);
        backend.release_event_v1(event).unwrap();
        backend.release_submission_v1(pending).unwrap();
        if !failed_dependency {
            assert_eq!(
                backend.cancel_v1(dependency).unwrap(),
                crate::BackendCancellationV1::Cancelled
            );
        }
        assert_eq!(backend.poll_v1(active).unwrap(), BackendPollV1::Succeeded);
        for id in [dependency, active] {
            backend.release_submission_v1(id).unwrap();
        }
        backend.unload_module_v1(module).unwrap();
        if stream != copy_stream {
            backend.destroy_stream_v1(stream).unwrap();
        }
        clean_scripted_direct_backend_v1(&mut backend, copy_stream, host, device, None);
        drop(ManuallyDrop::into_inner(backend));
    }
}

#[test]
fn pending_compute_settlement_preflight_preserves_exact_custody() {
    const CHILD: &str = "FE2O3_TEST_PENDING_COMPUTE_SETTLEMENT_CUSTODY";
    const TEST: &str = "kfd_backend::tests::compute_settlement_custody_tests::pending_compute_settlement_preflight_preserves_exact_custody";
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
        if case == 38 {
            inspect_reindex_collision();
        } else {
            inspect_failure(case >= 19, case % 19);
        }
        unreachable!();
    }
    for case in 0..39 {
        // These faults bypass public polling's dependency observation rather
        // than reach failure settlement; cancellation still covers both.
        if [30, 32, 33, 34].contains(&case) {
            continue;
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, case.to_string())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "case {case}: {stderr}");
        assert!(
            stderr.contains("pending compute custody inspected; dropping unrepaired backend"),
            "case {case}: {stderr}"
        );
        assert!(!stderr.contains("pending compute Drop returned"));
        eprintln!("verified pending compute case {case}: exact custody retained; Drop SIGABRT");
    }
}
