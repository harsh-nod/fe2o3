//! Public admission/flush with scripted submit outcomes, not native receipts.

use super::super::super::super::ordered_publication::ScriptedOrderedPublicationV1 as Submit;
use super::*;

const FAULTS: [Submit; 10] = [
    Submit::BeforeAttemptError,
    Submit::BeforeAttemptUnwind,
    Submit::Reject,
    Submit::Terminal,
    Submit::SubmitUnwind,
    Submit::OuterErrorRetry,
    Submit::OuterUnwindRetry,
    Submit::OuterErrorPublish,
    Submit::OuterUnwindPublish,
    Submit::ProfileUnwind,
];

fn publication_steps(f: &mut Fixture, entries: &[(u64, Submit)]) {
    assert!(
        f.backend
            .scripted_ordered_publication
            .as_ref()
            .is_none_or(VecDeque::is_empty)
    );
    f.backend.scripted_ordered_publication = Some(entries.iter().copied().collect());
}

fn pending_chain(lane: usize, write: bool) -> (Fixture, [u64; 3]) {
    let mut f = published(lane, write);
    f.backend.cancel_v1(f.trailing).unwrap();
    let event = f.backend.record_event_v1(f.stream, f.predecessor).unwrap();
    let mut ids = [0; 3];
    for id in &mut ids {
        *id = f
            .backend
            .submit_v1(BackendLaunchV1 {
                dependencies: &[event],
                ..f.recipe.borrowed()
            })
            .unwrap();
        assert!(f.backend.pending_compute.contains_key(id));
    }
    publication_steps(&mut f, &[]);
    (f, ids)
}

fn observe_pending_head(f: &mut Fixture, id: u64) {
    let a = f.first;
    assert_eq!(f.backend.free_compute_lane_v1().is_none(), f.lane != 0);
    let mut completion = Vec::new();
    if f.lane != 0 {
        // With both lanes occupied, public poll first makes bounded progress on
        // both frontiers, then observes A again as this pending head's predecessor.
        completion.extend([
            (f.backend.active.as_ref().unwrap().id, Step::PollPending),
            (a, Step::PollPending),
        ]);
    }
    completion.push((a, Step::PollPending));
    steps(f, &completion);
    assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
    assert!(f.backend.pending_compute.contains_key(&id));
    let pending = &f.backend.pending_compute[&id];
    assert_eq!(pending.explicit_success_dependencies.len(), 1);
    assert_eq!(
        pending.explicit_dependency_cursor,
        pending.explicit_success_dependencies.len()
    );
    assert!(
        f.backend
            .scripted_materialized_completion
            .as_ref()
            .unwrap()
            .is_empty()
    );
}

fn publish(f: &mut Fixture, id: u64) {
    publication_steps(f, &[(id, Submit::Publish)]);
    f.backend.flush_stream_v1(f.stream).unwrap();
    assert!(!f.backend.pending_compute.contains_key(&id));
    assert!(f.backend.active_compute_submission_v1(id).is_some());
    assert!(
        f.backend
            .scripted_ordered_publication
            .as_ref()
            .unwrap()
            .is_empty()
    );
    assert!(
        f.backend
            .scripted_materialized_completion
            .as_ref()
            .unwrap()
            .is_empty()
    );
}

fn publications(f: &Fixture, id: u64) -> usize {
    let dispatch = f
        .backend
        .profile_resource_v1(KfdProfileResourceKindV1::Dispatch, id)
        .unwrap();
    f.backend.profiler.as_ref().unwrap().recorded_events_for_test_v1().iter()
        .filter(|event| matches!(event.event, KfdRuntimeProfileEventKindV1::DispatchPublished { dispatch: observed, .. } if observed == dispatch)).count()
}

fn untouched(f: &Fixture, target: u64) -> String {
    let mut owners = Vec::new();
    for lane in 0..f.backend.native_compute_lanes.len() {
        let (frontier, pipeline) = if lane == 0 {
            (&f.backend.active, &f.backend.compute_pipeline)
        } else {
            let state = &f.backend.auxiliary_compute_lanes[lane - 1];
            (&state.active, &state.pipeline)
        };
        for active in frontier
            .iter()
            .chain(pipeline.iter())
            .filter(|a| a.id != target)
        {
            owners.push((
                active.id,
                format!(
                    "{lane} {active:?} {:?} {:?} {:?} {:?} {:p} {:?} {:p} {:?} {:?}",
                    pipeline.identity_for_submission_v1(active.id),
                    active.ordinary_recipe.as_ref().map(Arc::as_ptr),
                    active.execution.as_ref().map(std::mem::discriminant),
                    active.resident_descriptors,
                    active.resident_descriptors.as_ptr(),
                    active.writebacks,
                    active.writebacks.as_ptr(),
                    active.performance,
                    active.published_at,
                ),
            ));
        }
    }
    owners.sort_by_key(|(id, _)| *id);
    let mut pending: Vec<_> = f
        .backend
        .pending_compute
        .iter()
        .filter(|(id, _)| **id != target)
        .map(|(id, pending)| (*id, pending_facts(pending)))
        .collect();
    pending.sort_by_key(|(id, _)| *id);
    format!("{owners:?} {pending:?}")
}

fn fault_case(lane: usize, pipelined: bool, fault: Submit, drop_unrepaired: bool) {
    let (mut f, [b, c, d]) = pending_chain(lane, true);
    if pipelined {
        publish(&mut f, b);
    }
    let target = if pipelined { c } else { b };
    let predecessor = if pipelined { b } else { f.first };
    let untouched_before = untouched(&f, target);
    let recipe = Arc::clone(&f.backend.pending_compute[&target].launch);
    let tail = pending_facts(&f.backend.pending_compute[&d]);
    let frontier = format!(
        "{:?}",
        f.backend.active_compute_submission_v1(f.first).unwrap()
    );
    let retained = (
        f.backend.compute_completion_reservations,
        f.backend.compute_module_retain_counts.clone(),
        format!("{:?}", f.backend.allocation_custody),
        f.backend.stream_compute_lanes.clone(),
        f.backend.stream_submission_tails.clone(),
        f.backend.event_submission_retain_counts.clone(),
    );
    let explicit_before = f.backend.compute_dependency_retain_counts[&f.predecessor];
    let ordered_before = f.backend.compute_dependency_retain_counts[&predecessor];
    let heads_before = f.backend.with_compute_lane_state_v1(lane, |backend| {
        backend.compute_pipeline.publication_heads_v1()
    });
    let script = if fault == Submit::ProfileUnwind {
        vec![(target, Submit::Publish), (target, fault)]
    } else {
        vec![(target, fault)]
    };
    publication_steps(&mut f, &script);
    let result = catch_unwind(AssertUnwindSafe(|| f.backend.flush_stream_v1(f.stream)));
    let panic = match fault {
        Submit::BeforeAttemptUnwind => Some("scripted ordered lane unwind"),
        Submit::SubmitUnwind => Some("scripted ordered submit unwind"),
        Submit::OuterUnwindRetry | Submit::OuterUnwindPublish => {
            Some("scripted ordered outer unwind")
        }
        Submit::ProfileUnwind => Some("scripted ordered profile unwind"),
        _ => None,
    };
    if let Some(message) = panic {
        assert_eq!(result.unwrap_err().downcast_ref::<&str>(), Some(&message));
    } else {
        assert!(
            matches!(result, Ok(Err(RuntimeBackendFailureV1::Terminal(_)))),
            "{result:?}"
        );
    }
    assert!(f.backend.terminal);
    assert_eq!(untouched(&f, target), untouched_before);
    f.backend.with_compute_lane_state_v1(lane, |backend| {
        let identity = backend
            .compute_pipeline
            .identity_for_submission_v1(target)
            .unwrap();
        let heads = backend.compute_pipeline.publication_heads_v1();
        if fault == Submit::ProfileUnwind {
            assert_eq!(
                heads,
                (
                    heads_before.0.and_then(|epoch| epoch.checked_add(1)),
                    heads_before.1.or(heads_before.0),
                    None,
                )
            );
            assert!(backend.compute_pipeline.checked_frontier_v1().is_ok());
        } else {
            assert_eq!(heads, (heads_before.0, heads_before.1, Some(identity)));
            assert!(backend.compute_pipeline.checked_frontier_v1().is_err());
        }
        for active in backend.compute_pipeline.iter() {
            assert_eq!(
                backend.compute_pipeline.phase(active.id),
                Some(RuntimeComputePipelinePhaseV1::Quarantined)
            );
        }
    });
    assert!(!f.backend.pending_compute.contains_key(&target));
    assert!(!f.backend.pending_compute_streams[&f.stream].contains(&target));
    assert!(!f.backend.submissions.contains_key(&target));
    assert_eq!(
        f.backend.compute_dependency_retain_counts[&f.predecessor],
        explicit_before - 1
    );
    assert_eq!(
        f.backend.compute_dependency_retain_counts[&predecessor],
        ordered_before
    );
    assert_eq!(
        (
            f.backend.compute_completion_reservations,
            f.backend.compute_module_retain_counts.clone(),
            format!("{:?}", f.backend.allocation_custody),
            f.backend.stream_compute_lanes.clone(),
            f.backend.stream_submission_tails.clone(),
            f.backend.event_submission_retain_counts.clone()
        ),
        retained
    );
    assert_eq!(pending_facts(&f.backend.pending_compute[&d]), tail);
    assert_eq!(
        format!(
            "{:?}",
            f.backend.active_compute_submission_v1(f.first).unwrap()
        ),
        frontier
    );
    let active = f.backend.active_compute_submission_v1(target).unwrap();
    assert!(Arc::ptr_eq(
        active.ordinary_recipe.as_ref().unwrap(),
        &recipe
    ));
    assert_eq!(active.ordered_predecessor, Some(predecessor));
    assert!(active.deferred_ordered_predecessor_retain);
    match active.execution.as_ref().unwrap() {
        ActiveComputeExecutionV1::MaterializedSuccessorPublication(root) => {
            assert!(matches!(root.profile.bindings, Some(Ok(_))));
            assert!(match fault {
                Submit::BeforeAttemptError | Submit::BeforeAttemptUnwind =>
                    matches!(root.attempt, MaterializedSubmissionAttemptV1::Unattempted),
                Submit::Reject | Submit::Terminal | Submit::SubmitUnwind =>
                    matches!(root.attempt, MaterializedSubmissionAttemptV1::NativeOwned),
                Submit::OuterErrorRetry | Submit::OuterUnwindRetry =>
                    matches!(root.attempt, MaterializedSubmissionAttemptV1::Retryable),
                Submit::OuterErrorPublish | Submit::OuterUnwindPublish => matches!(
                    root.attempt,
                    MaterializedSubmissionAttemptV1::ScriptedPublished
                ),
                _ => false,
            });
        }
        ActiveComputeExecutionV1::ScriptedMaterialized => assert_eq!(fault, Submit::ProfileUnwind),
        _ => panic!("wrong indexed ordered publication state"),
    }
    assert_eq!(publications(&f, target), 0);
    assert_eq!(f.backend.selected_compute_lane, 0);
    assert!(
        f.backend
            .scripted_ordered_publication
            .as_ref()
            .unwrap()
            .is_empty()
    );
    assert!(
        f.backend
            .scripted_materialized_completion
            .as_ref()
            .unwrap()
            .is_empty()
    );
    let before = facts(&f);
    for _ in 0..2 {
        assert!(matches!(
            f.backend.poll_v1(target),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            f.backend.cancel_v1(target),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            f.backend.shutdown_native_v1(),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert_eq!(facts(&f), before);
    }
    if let Some(primary) = &f.primary {
        assert_eq!(
            format!("{:?}", f.backend.active.as_ref().unwrap()),
            *primary
        );
    }
    if drop_unrepaired {
        eprintln!("ordered-publication-custody-retained");
        drop(ManuallyDrop::into_inner(f.backend));
    } else {
        discard_scripted_fixture(f.backend);
    }
}

#[test]
fn ordered_publication_faults_retain_indexed_owner_and_settle_pending_once() {
    for lane in 0..2 {
        for pipelined in [false, true] {
            for fault in FAULTS {
                fault_case(lane, pipelined, fault, false);
            }
        }
    }
}

#[test]
fn ordered_publication_unrepaired_drop_aborts() {
    const ENV: &str = "FE2O3_ORDERED_PUBLICATION_DROP";
    if let Ok(value) = std::env::var(ENV) {
        let case: usize = value.parse().unwrap();
        fault_case(
            case / 20,
            !(case / 10).is_multiple_of(2),
            FAULTS[case % 10],
            true,
        );
        panic!("unrepaired ordered publication Drop returned");
    }
    for case in 0..40 {
        let output = std::process::Command::new("sh")
            .args(["-c", "ulimit -c 0; exec \"$1\" --exact \"$2\" --nocapture", "sh"])
            .arg(std::env::current_exe().unwrap())
            .arg("kfd_backend::tests::materialized_publication_tests::completion::ordered::ordered_publication_unrepaired_drop_aborts")
            .env(ENV, case.to_string()).output().unwrap();
        assert_eq!(output.status.signal(), Some(6), "case {case}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("ordered-publication-custody-retained"),
            "case {case}: {output:?}"
        );
    }
}

#[test]
fn ordered_publication_corrupt_custody_refuses_before_staging_or_submit() {
    for lane in 0..2 {
        for case in 0..(6 + lane) {
            let (mut f, [b, _, _]) = pending_chain(lane, true);
            let a = f.first;
            publication_steps(&mut f, &[(b, Submit::SubmitUnwind)]);
            // Poll may observe dependencies but cannot enter successor preparation.
            observe_pending_head(&mut f, b);
            match case {
                0 => {
                    f.backend.compute_module_retain_counts.remove(&f.module);
                }
                1 => f.backend.compute_completion_reservations = 0,
                2 => {
                    f.backend.stream_submission_tails.insert(f.stream, 0);
                }
                3 => {
                    f.backend.compute_dependency_retain_counts.remove(&a);
                }
                4 => {
                    f.backend
                        .allocation_custody
                        .get_mut(&f.host)
                        .unwrap()
                        .owner_counts[RuntimeAllocationCustodyKindV1::Compute.index()] = 0
                }
                5 => {
                    let custody = f.backend.allocation_custody.get_mut(&f.host).unwrap();
                    let index = custody
                        .owners
                        .binary_search_by_key(&b, |owner| owner.submission)
                        .unwrap();
                    custody.owners[index].stream += 1;
                }
                6 => f.backend.auxiliary_compute_lanes[lane - 1].owner_stream = None,
                _ => unreachable!(),
            }
            let before = facts(&f);
            assert!(
                matches!(f.backend.flush_stream_v1(f.stream), Err(RuntimeBackendFailureV1::Terminal(error)) if error.detail().contains("lost exact accepted custody"))
            );
            assert_eq!(facts(&f), before);
            assert!(f.backend.active_compute_submission_v1(b).is_none());
            assert_eq!(
                f.backend
                    .scripted_ordered_publication
                    .as_ref()
                    .unwrap()
                    .front(),
                Some(&(b, Submit::SubmitUnwind))
            );
            assert_eq!(publications(&f, b), 0);
            discard_scripted_fixture(f.backend);
        }
    }
}

#[test]
fn ordered_publication_exhausted_identity_keeps_pending_and_submit_untouched() {
    for lane in 0..2 {
        for slots in [false, true] {
            let (mut f, [b, _, _]) = pending_chain(lane, true);
            let a = f.first;
            publication_steps(&mut f, &[(b, Submit::SubmitUnwind)]);
            observe_pending_head(&mut f, b);
            f.backend.with_compute_lane_state_v1(lane, |backend| {
                if slots {
                    backend
                        .compute_pipeline
                        .exhaust_vacant_identities_for_test_v1();
                } else {
                    backend
                        .compute_pipeline
                        .exhaust_logical_epochs_for_test_v1();
                }
            });
            let before = facts(&f);
            steps(&mut f, &[(a, Step::PollPending)]);
            f.backend.flush_stream_v1(f.stream).unwrap();
            assert!(!f.backend.terminal);
            assert_eq!(facts(&f), before);
            assert!(f.backend.active_compute_submission_v1(b).is_none());
            assert_eq!(
                f.backend
                    .scripted_ordered_publication
                    .as_ref()
                    .unwrap()
                    .front(),
                Some(&(b, Submit::SubmitUnwind))
            );
            assert!(
                f.backend
                    .scripted_materialized_completion
                    .as_ref()
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(publications(&f, b), 0);
            discard_scripted_fixture(f.backend);
        }
    }
}

#[test]
fn ordered_publication_retries_preserve_pending_then_commit_contiguous_successors() {
    for lane in 0..2 {
        for write in [false, true] {
            for predecessor_phase in 0..3 {
                let (mut f, [b, c, d]) = pending_chain(lane, write);
                let a = f.first;
                publish(&mut f, b);
                if predecessor_phase != 0 {
                    let recycle = if predecessor_phase == 1 {
                        Step::RecycleRetry
                    } else {
                        Step::RecycleReady
                    };
                    steps(
                        &mut f,
                        &[(b, Step::PollReady), (b, recycle), (a, Step::PollPending)],
                    );
                    assert_eq!(f.backend.poll_v1(b).unwrap(), BackendPollV1::Pending);
                }
                let recipe = Arc::clone(&f.backend.pending_compute[&c].launch);
                let retained = (
                    f.backend.compute_dependency_retain_counts.clone(),
                    f.backend.compute_module_retain_counts.clone(),
                    f.backend.compute_completion_reservations,
                    format!("{:?}", f.backend.allocation_custody),
                );
                let mut after_first = None;
                for _ in 0..3 {
                    publication_steps(&mut f, &[(c, Submit::Retry)]);
                    let mut completion = Vec::new();
                    if predecessor_phase == 0 {
                        completion.push((b, Step::PollPending));
                    }
                    if predecessor_phase == 1 {
                        completion.push((b, Step::RecycleRetry));
                    }
                    completion.push((a, Step::PollPending));
                    steps(&mut f, &completion);
                    f.backend.flush_stream_v1(f.stream).unwrap();
                    assert!(f.backend.active_compute_submission_v1(c).is_none());
                    let pending = &f.backend.pending_compute[&c];
                    assert!(Arc::ptr_eq(&pending.launch, &recipe));
                    if let Some(before) = &after_first {
                        assert_eq!(&pending_facts(pending), before);
                    }
                    after_first = Some(pending_facts(pending));
                    assert_eq!(
                        (
                            f.backend.compute_dependency_retain_counts.clone(),
                            f.backend.compute_module_retain_counts.clone(),
                            f.backend.compute_completion_reservations,
                            format!("{:?}", f.backend.allocation_custody)
                        ),
                        retained
                    );
                    assert_eq!(publications(&f, c), 0);
                    assert!(
                        f.backend
                            .scripted_ordered_publication
                            .as_ref()
                            .unwrap()
                            .is_empty()
                    );
                    assert!(
                        f.backend
                            .scripted_materialized_completion
                            .as_ref()
                            .unwrap()
                            .is_empty()
                    );
                }
                publish(&mut f, c);
                publish(&mut f, d);
                for id in [b, c, d] {
                    assert_eq!(publications(&f, id), 1);
                    let active = f.backend.active_compute_submission_v1(id).unwrap();
                    assert_eq!(active.performance.native_binding, Duration::ZERO);
                    assert_eq!(active.performance.user_data_materializations, 0);
                    assert_eq!(
                        active.performance.data_path,
                        KfdRuntimeLaunchDataPathV1::ResidentReused
                    );
                }
                // Retire the suffix before A, then check exact contiguous logical commit.
                for id in [d, c] {
                    steps(
                        &mut f,
                        &[
                            (id, Step::PollReady),
                            (id, Step::RecycleReady),
                            (a, Step::PollPending),
                        ],
                    );
                    assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
                }
                if predecessor_phase != 2 {
                    let mut completion = Vec::new();
                    if predecessor_phase == 0 {
                        completion.push((b, Step::PollReady));
                    }
                    completion.extend([(b, Step::RecycleReady), (a, Step::PollPending)]);
                    steps(&mut f, &completion);
                    assert_eq!(f.backend.poll_v1(b).unwrap(), BackendPollV1::Pending);
                }
                steps(&mut f, &[(a, Step::PollReady), (a, Step::RecycleReady)]);
                assert_eq!(f.backend.poll_v1(a).unwrap(), BackendPollV1::Succeeded);
                assert_eq!(completion_events(&f, &[a, b, c, d]), [a, b, c, d]);
                for id in [a, b, c, d] {
                    assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
                }
                assert_eq!(f.backend.compute_completion_reservations, lane);
                assert!(!f.backend.allocation_custody.contains_key(&f.host));
                assert!(!f.backend.stream_compute_lanes.contains_key(&f.stream));
                discard_scripted_fixture(f.backend);
            }
        }
    }
}
