//! Scripted owner custody across native failure, not GPU or unwind proof evidence.

use super::*;
use std::os::unix::process::ExitStatusExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn assert_context_terminal<T>(result: Result<T, crate::RuntimeErrorV1<KfdRuntimeBackendErrorV1>>) {
    assert!(matches!(
        result,
        Err(crate::RuntimeErrorV1::Validation(
            crate::RuntimeValidationErrorV1::ContextTerminal
        ))
    ));
}

fn inspect_failure_and_drop(
    cross_stream: bool,
    unwind: bool,
    consumer_active: bool,
    flush_middle: bool,
) {
    let mut f = Fixture::new(cross_stream);
    let (a, ai) = f.launch(0, &[]);
    f.context.flush_stream(f.streams[0]).unwrap();
    let x_owner = f.active_owner_id(ai, 2);
    f.active_bytes(ai, 2).fill(0xa1);
    let ae = f.context.record_event(&a).unwrap();
    let (b, bi) = f.launch(1, &[ae]);
    let be = f.context.record_event(&b).unwrap();
    let (mut c, ci) = f.launch(2, &[be]);
    let ce = f.context.record_event(&c).unwrap();
    let callbacks = Arc::new(AtomicUsize::new(0));
    let callback_log = callbacks.clone();
    f.context
        .on_completion(&c, move |_| {
            callback_log.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
    for event in [ae, be] {
        f.context.release_event(event).unwrap();
    }

    // Either fail the ancestor with both descendants still queued, or finish
    // native A/B without Context observation and fail the active consumer.
    let (active_id, active_allocations, inactive_allocations) = if consumer_active {
        f.finish_native(ai);
        f.context.flush_stream(f.streams[1]).unwrap();
        assert_eq!(f.active_owner_id(bi, 2), x_owner);
        f.active_bytes(bi, 2).fill(0xb2);
        f.finish_native(bi);
        f.context.flush_stream(f.streams[2]).unwrap();
        assert_eq!(f.active_owner_id(ci, 0), x_owner);
        assert_eq!(f.active_bytes(ci, 0), &[0xb2; 64]);
        for id in [ai, bi] {
            assert_eq!(
                f.context.backend().submissions[&id].status,
                BackendPollV1::Succeeded
            );
        }
        (ci, [2, 1, 4], [0, 3])
    } else {
        let backend = f.context.backend();
        assert_eq!(backend.compute_dependency_retain_counts[&ai], 2);
        assert_eq!(backend.compute_dependency_retain_counts[&bi], 1);
        assert_eq!(backend.compute_completion_reservations, 3);
        (ai, [0, 1, 2], [3, 4])
    };
    let active_owners = [0, 1, 2].map(|binding| f.active_owner_id(active_id, binding));
    for submission in [&a, &b, &c] {
        assert_eq!(
            f.context.query_submission(submission).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
    }
    assert_eq!(f.context.version_journal_read_records_v1(), Some(6));
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(3));
    let streams_before = f.streams.map(|s| f.context.query_stream(s).unwrap());
    let device = f.context.devices()[0].id();
    let usage_before = f
        .context
        .allocation_admission_usage_v1(device)
        .unwrap()
        .unwrap();
    assert_eq!(usage_before.reserved_records, 0);
    assert_eq!(usage_before.retained_records, 5);
    assert_eq!(usage_before.quarantined_records, 0);
    assert_eq!(usage_before.record_capacity, 5);
    assert_eq!(usage_before.used, usage_before.capacity);
    assert_eq!(
        usage_before
            .used
            .get(crate::RuntimeResourceKindV1::RequestedAllocationBytes),
        320
    );
    assert_eq!(
        usage_before
            .used
            .get(crate::RuntimeResourceKindV1::AllocationRecords),
        5
    );
    assert!(!usage_before.poisoned);
    assert_eq!(Arc::strong_count(&callbacks), 2);
    let pending_recipes = [bi, ci].map(|id| {
        f.context
            .backend()
            .pending_compute
            .get(&id)
            .map(|pending| Arc::downgrade(&pending.launch))
    });

    let retention = |f: &Fixture| {
        let backend = f.context.backend();
        (
            backend.compute_dependency_retain_counts.clone(),
            backend.compute_module_retain_counts.clone(),
            backend.event_submission_retain_counts.clone(),
            backend.compute_completion_reservations,
            backend.selected_compute_lane,
            backend.stream_compute_lanes.clone(),
            backend.pending_compute_streams.clone(),
            [ai, bi, ci].map(|id| {
                backend.submissions.get(&id).map(|record| {
                    (
                        record.stream,
                        record.status,
                        record.dependency_depth,
                        record.profile_dispatch_published,
                    )
                })
            }),
            [ai, bi, ci].map(|id| {
                backend.pending_compute.get(&id).map(|pending| {
                    (
                        pending.id,
                        pending.module,
                        pending.launch.unaccounted_copy_for_test(),
                        pending.retained_allocations.clone(),
                        pending.ordered_predecessor,
                        pending.explicit_success_dependencies.clone(),
                        pending.explicit_dependency_cursor,
                        pending.quiescence_dependencies.clone(),
                        pending.quiescence_cursor,
                        pending.dependency_depth,
                        (
                            Arc::as_ptr(&pending.launch) as usize,
                            pending.retained_allocations.as_ptr() as usize,
                            pending.explicit_success_dependencies.as_ptr() as usize,
                            pending.quiescence_dependencies.as_ptr() as usize,
                        ),
                    )
                })
            }),
            f.native_allocations.map(|id| {
                backend.allocation_custody.get(&id).map(|custody| {
                    (
                        custody.owners.clone(),
                        custody.sole_stream,
                        custody.owner_counts,
                    )
                })
            }),
            inactive_allocations.map(|index| {
                let id = f.native_allocations[index];
                let KfdRuntimeSdmaStorageV1::H2dReady(ready) =
                    &backend.allocations[&id].sdma_storage
                else {
                    panic!("unrelated read input must retain its authenticated owner");
                };
                ready.owner.scripted_owner_id().unwrap()
            }),
            backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
        )
    };
    let retained_before = retention(&f);
    f.assert_owners(5);
    {
        let backend = f.context.backend_mut_for_test_v1();
        backend.scripted_persistent_poll_pending_observations = 0;
        if unwind {
            backend.scripted_persistent_transition_failure =
                Some(ScriptedPersistentTransitionFailureV1::UnwindBeforeTake);
        } else {
            // Corrupt only the marker, not an owner. The actual restoration
            // check must retain all three owners in terminal custody.
            backend
                .allocations
                .get_mut(&f.native_allocations[active_allocations[2]])
                .unwrap()
                .sdma_storage = KfdRuntimeSdmaStorageV1::ComputeInFlight(active_id + 1);
        }
    }
    let observe = |f: &mut Fixture, c: &mut Submission| {
        if flush_middle {
            f.context.flush_stream(f.streams[1]).map(|_| ())
        } else {
            f.context.poll(c).map(|_| ())
        }
    };
    if unwind {
        let panic =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observe(&mut f, &mut c)))
                .expect_err("the injected native unwind must propagate");
        assert_eq!(
            panic.downcast_ref::<&str>().copied(),
            Some("scripted three-binding unwind before active take")
        );
    } else {
        let Err(crate::RuntimeErrorV1::BackendTerminal(error)) = observe(&mut f, &mut c) else {
            panic!("restoration slot mismatch must be terminal");
        };
        assert_eq!(
            error.detail(),
            "three-binding completion custody changed before native effects"
        );
    }
    assert!(f.context.is_terminal());
    let mut expected_usage = usage_before;
    expected_usage.retained_records = 0;
    expected_usage.quarantined_records = 5;

    // Repeated public ingress must be inert, including cleanup. Local queries
    // retain Pending because no conclusive Context completion was observed.
    for _ in 0..2 {
        for submission in [&a, &b, &c] {
            assert_eq!(
                f.context.query_submission(submission).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
        }
        assert_eq!(
            f.context.query_event(ce).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
        assert_eq!(
            f.streams.map(|s| f.context.query_stream(s).unwrap()),
            streams_before
        );
        assert_eq!(
            f.context.allocation_admission_usage_v1(device).unwrap(),
            Some(expected_usage)
        );
        assert_eq!(f.context.version_journal_read_records_v1(), Some(6));
        assert_eq!(f.context.version_journal_writer_records_v1(), Some(3));
        assert_eq!(callbacks.load(Ordering::SeqCst), 0);
        assert_eq!(f.context.completion_callback_panic_count(), 0);
        assert_context_terminal(f.context.poll(&mut c));
        assert_context_terminal(f.context.wait(&mut c, Duration::ZERO));
        assert_context_terminal(f.context.poll_event(ce));
        assert_context_terminal(f.context.wait_event(ce, Duration::ZERO));
        assert_context_terminal(f.context.cancel(&mut c));
        assert_context_terminal(
            f.context
                .on_completion(&c, |_| panic!("terminal callback ran")),
        );
        assert_context_terminal(f.context.flush_stream(f.streams[2]));
        assert_context_terminal(f.context.release_event(ce));
        assert_context_terminal(f.context.release_allocation(f.allocations[2]));
        assert_context_terminal(f.context.unload_module(f.module));
        assert_context_terminal(f.context.destroy_stream(f.streams[2]));
        let report = f.context.cleanup();
        assert!(report.is_terminal());
        assert!(!report.is_complete());
        assert!(!report.is_graph_reserved());
        assert!(report.failures().is_empty());
        assert_eq!(
            report.retained(),
            crate::RuntimeRetainedResourcesV1 {
                streams: if cross_stream { 3 } else { 1 },
                events: 1,
                submissions: 3,
                modules: 1,
                allocations: 5,
            }
        );
        assert_eq!(report.allocation_credit_records_v1(), 5);
        assert_eq!(report.allocation_journal_records_v1(), 5);
        assert_eq!(report.writer_journal_records_v1(), 3);
        assert_eq!(report.reader_journal_records_v1(), 6);
        assert_eq!(report.producer_launch_records_v1(), 3);
        assert_eq!(report.scalar_peer_copy_records_v1(), 0);
        assert_eq!(retention(&f), retained_before);
        for recipe in pending_recipes.iter().flatten() {
            assert!(
                recipe.upgrade().is_some(),
                "pending recipe must remain backend-owned"
            );
        }
        assert_eq!(Arc::strong_count(&callbacks), 2);
        f.assert_owners(5);
        let backend = f.context.backend();
        assert_eq!(backend.terminal, !unwind || flush_middle);
        assert_eq!(
            backend.pending_compute.len(),
            if consumer_active { 0 } else { 2 }
        );
        if unwind {
            assert_eq!(backend.scripted_persistent_transition_failure, None);
            assert!(backend.terminal_sdma_custody.is_none());
            assert_eq!(
                [0, 1, 2].map(|binding| f.active_owner_id(active_id, binding)),
                active_owners
            );
        } else {
            assert_eq!(
                backend.active.as_ref().map(|active| active.id),
                Some(active_id)
            );
            assert!(backend.terminal_sdma_custody.is_none());
            assert_eq!(
                [0, 1, 2].map(|binding| f.active_owner_id(active_id, binding)),
                active_owners
            );
        }
        for index in active_allocations {
            let expected = if !unwind && index == active_allocations[2] {
                active_id + 1
            } else {
                active_id
            };
            assert!(
                matches!(backend.allocations[&f.native_allocations[index]].sdma_storage,
                KfdRuntimeSdmaStorageV1::ComputeInFlight(id) if id == expected)
            );
        }
    }
    eprintln!("queued native failure assertions complete; dropping unfinished Context");
    drop(ManuallyDrop::into_inner(f.context));
    panic!("unfinished queued native Context Drop returned");
}

#[test]
fn queued_native_context_terminal_and_unwind_preserve_custody_until_abort() {
    const CHILD: &str = "FE2O3_TEST_QUEUED_NATIVE_FAILURE_DROP";
    const TEST: &str = "kfd_backend::tests::queued_producer_context_tests::failure_tests::queued_native_context_terminal_and_unwind_preserve_custody_until_abort";
    const CASES: [(&str, bool, bool, bool, bool); 10] = [
        ("same-terminal-ancestor", false, false, false, false),
        ("cross-terminal-ancestor", true, false, false, false),
        ("same-unwind-ancestor", false, true, false, false),
        ("cross-unwind-ancestor", true, true, false, false),
        ("same-terminal-consumer", false, false, true, false),
        ("cross-terminal-consumer", true, false, true, false),
        ("same-unwind-consumer", false, true, true, false),
        ("cross-unwind-consumer", true, true, true, false),
        ("same-unwind-middle-flush", false, true, false, true),
        ("same-terminal-middle-flush", false, false, false, true),
    ];
    if let Some(mode) = std::env::var_os(CHILD) {
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        let mode = mode.to_str().unwrap();
        let (_, cross_stream, unwind, consumer_active, flush_middle) = CASES
            .into_iter()
            .find(|case| case.0 == mode)
            .expect("known failure case");
        inspect_failure_and_drop(cross_stream, unwind, consumer_active, flush_middle);
        unreachable!();
    }
    for (mode, _, _, _, _) in CASES {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, mode)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "{mode}: {stderr}");
        assert!(
            stderr
                .contains("queued native failure assertions complete; dropping unfinished Context"),
            "{mode}: {stderr}"
        );
        assert!(
            !stderr.contains("unfinished queued native Context Drop returned"),
            "{mode}: {stderr}"
        );
        eprintln!("verified {mode}: exact custody retained; unfinished Drop SIGABRT");
    }
}
