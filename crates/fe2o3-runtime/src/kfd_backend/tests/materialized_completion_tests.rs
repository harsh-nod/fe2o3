//! Public completion ingress with scripted outcomes, not native receipt evidence.

use super::super::super::materialized_completion::{
    MaterializedConsumeV1, ScriptedCompletionStepV1 as Step,
};
use super::*;

#[path = "ordered_publication_tests.rs"]
mod ordered;

const FAULTS: [Step; 12] = [
    Step::PollError,
    Step::PollUnwind,
    Step::PollOuterErrorPending,
    Step::PollOuterUnwindPending,
    Step::PollOuterErrorReady,
    Step::PollOuterUnwindReady,
    Step::RecycleError,
    Step::RecycleUnwind,
    Step::RecycleOuterErrorRetry,
    Step::RecycleOuterUnwindRetry,
    Step::RecycleOuterErrorReady,
    Step::RecycleOuterUnwindReady,
];

fn published(lane: usize, write: bool) -> Fixture {
    let mut f = Fixture::new(lane, write);
    f.backend.scripted_materialized_preparation =
        Some((MaterializedPreparationOriginV1::NewBinding, 0));
    f.backend.flush_stream_v1(f.stream).unwrap();
    assert_eq!(f.backend.poll_v1(f.first).unwrap(), BackendPollV1::Pending);
    f.assert_handoff();
    f.backend.scripted_materialized_completion = Some(Default::default());
    f
}

fn steps(f: &mut Fixture, entries: &[(u64, Step)]) {
    assert!(
        f.backend
            .scripted_materialized_completion
            .as_ref()
            .is_none_or(|steps| steps.is_empty()),
        "unconsumed scripted outcome"
    );
    f.backend.scripted_materialized_completion = Some(entries.iter().copied().collect());
}

fn mutate(f: &mut Fixture, id: u64, operation: impl FnOnce(&mut ActiveSubmissionV1)) {
    f.backend.with_compute_lane_state_v1(f.lane, |b| {
        if b.active.as_ref().is_some_and(|a| a.id == id) {
            operation(b.active.as_mut().unwrap());
        } else {
            let identity = b.compute_pipeline.identity_for_submission_v1(id).unwrap();
            operation(&mut b.compute_pipeline.entry_mut_v1(identity).unwrap().active);
        }
    });
}

fn facts(f: &Fixture) -> String {
    let b = &f.backend;
    let (frontier, pipeline) = if f.lane == 0 {
        (&b.active, &b.compute_pipeline)
    } else {
        let lane = &b.auxiliary_compute_lanes[f.lane - 1];
        (&lane.active, &lane.pipeline)
    };
    let owners: Vec<_> = frontier
        .iter()
        .chain(pipeline.iter())
        .map(|a| {
            format!(
                "{:?} {:p} {:p} {:?} {:?} {:p}",
                a,
                a.resident_descriptors.as_ptr(),
                a.writebacks.as_ptr(),
                a.resident_descriptors,
                a.writebacks,
                Arc::as_ptr(a.ordinary_recipe.as_ref().unwrap())
            )
        })
        .collect();
    let caches: Vec<_> = core::iter::once(&b.recycled_dispatch)
        .chain(
            b.auxiliary_compute_lanes
                .iter()
                .map(|lane| &lane.recycled_dispatch),
        )
        .map(|cache| {
            cache.as_ref().map(|c| {
                (
                    c.kernel,
                    c.dispatch_shape_sha256,
                    c.descriptors.as_ptr(),
                    &c.descriptors,
                )
            })
        })
        .collect();
    let extra = format!(
        "{:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
        b.selected_compute_lane,
        b.active,
        b.auxiliary_compute_lanes
            .iter()
            .map(|lane| lane.owner_stream)
            .collect::<Vec<_>>(),
        b.compute_pipeline.iter().collect::<Vec<_>>(),
        caches,
        b.pending_compute_streams,
        b.pending_compute
            .iter()
            .map(|(id, pending)| (*id, pending_facts(pending)))
            .collect::<Vec<_>>(),
        b.allocations[&f.host].content_sha256
    );
    format!(
        "{owners:?} {extra} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:p} {:?} {:?} {:?} {:?}",
        b.compute_module_retain_counts,
        b.compute_dependency_retain_counts,
        b.event_submission_retain_counts,
        b.allocation_custody,
        b.compute_completion_reservations,
        b.stream_compute_lanes,
        b.stream_submission_tails,
        b.allocations[&f.host].bytes,
        Arc::as_ptr(&b.allocations[&f.host].bytes),
        b.allocations[&f.host].native_dirty,
        b.native_dirty_extents,
        b.pending_compute.keys().copied().collect::<Vec<_>>(),
        pipeline
            .iter()
            .map(|a| pipeline.identity_for_submission_v1(a.id))
            .collect::<Vec<_>>()
    )
}

fn completion_events(f: &Fixture, ids: &[u64]) -> Vec<u64> {
    f.backend
        .profiler
        .as_ref()
        .unwrap()
        .recorded_events_for_test_v1()
        .iter()
        .filter_map(|event| {
            let KfdRuntimeProfileEventKindV1::DispatchCompleted { dispatch, .. } = event.event
            else {
                return None;
            };
            ids.iter().copied().find(|id| {
                f.backend
                    .profile_resource_v1(KfdProfileResourceKindV1::Dispatch, *id)
                    == Some(dispatch)
            })
        })
        .collect()
}

// These owners start as genuine public Pending admissions, then this fixture
// constructs only their completion state. It does NOT test successor publication.
fn pipeline(f: &mut Fixture) -> [u64; 2] {
    assert_eq!(
        f.backend.cancel_v1(f.trailing).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert_eq!(f.backend.stream_submission_tails[&f.stream], f.first);
    let a = f.first;
    steps(f, &[(a, Step::PollPending)]);
    let b = f.backend.submit_v1(f.recipe.borrowed()).unwrap();
    let c = f.backend.submit_v1(f.recipe.borrowed()).unwrap();
    let ids = [b, c];
    for id in ids {
        let pending = f.backend.pending_compute.remove(&id).unwrap();
        assert!(pending.quiescence_dependencies.is_empty());
        assert!(pending.explicit_success_dependencies.is_empty());
        f.backend.with_compute_lane_state_v1(f.lane, |b| {
            let mut prepared = b
                .prepare_launch(pending.launch.borrowed(), false, true)
                .unwrap();
            prepared.performance.data_path = KfdRuntimeLaunchDataPathV1::ResidentReused;
            prepared.performance.native_binding = Duration::ZERO;
            prepared.performance.user_data_materializations = 0;
            let PreparedLaunchStorageV1::Materialized(data) = prepared.storage else {
                panic!("ordinary fixture");
            };
            let descriptors = resident_descriptors_v1(&data).unwrap();
            for writeback in &prepared.writebacks {
                b.allocations
                    .get_mut(&writeback.allocation)
                    .unwrap()
                    .native_dirty
                    .reserve(prepared.writebacks.len());
            }
            let active = ActiveSubmissionV1 {
                source_event: Default::default(),
                id: pending.id,
                stream: pending.launch.stream,
                ordered_predecessor: pending.ordered_predecessor,
                deferred_ordered_predecessor_retain: true,
                kernel: pending.launch.kernel,
                dependency_depth: pending.dependency_depth,
                allocations: prepared.allocations,
                writebacks: prepared.writebacks,
                resident_descriptors: descriptors,
                ordinary_recipe: Some(Arc::clone(&pending.launch)),
                dispatch_shape_sha256: prepared.dispatch_shape_sha256,
                published_at: Instant::now(),
                performance: prepared.performance,
                execution: Some(ActiveComputeExecutionV1::ScriptedMaterialized),
            };
            b.compute_pipeline.insert_published(active).unwrap();
            b.observe_materialized_dispatch_published_v1(
                pending.id,
                f.stream,
                pending.launch.kernel,
                prepared.dispatch_shape_sha256,
                PersistentPublicationProfileV1 {
                    launch: prepared.profile_launch,
                    semantic_contract: prepared.profile_semantic_contract,
                    bindings: prepared.profile_bindings,
                },
            );
        });
        f.backend
            .remove_pending_compute_from_stream_v1(f.stream, id);
        f.backend
            .release_compute_dependency_retains_v1(&pending.explicit_success_dependencies);
        f.backend
            .release_compute_dependency_retains_v1(&pending.quiescence_dependencies);
        // The ordered predecessor retain remains attached to this pipeline owner.
    }
    assert_eq!(f.backend.compute_completion_reservations, 3 + f.lane);
    assert_eq!(
        f.backend.compute_module_retain_counts[&f.module],
        3 + f.lane
    );
    assert_eq!(f.backend.compute_dependency_retain_counts[&a], 1);
    assert_eq!(f.backend.compute_dependency_retain_counts[&b], 1);
    assert_eq!(f.backend.stream_submission_tails[&f.stream], c);
    assert_eq!(f.backend.allocation_custody[&f.host].owners.len(), 3);
    assert!(
        f.backend
            .scripted_materialized_completion
            .as_ref()
            .unwrap()
            .is_empty()
    );
    ids
}

fn fault_case(lane: usize, pipelined: bool, fault: Step, unrepaired_drop: bool) {
    let mut f = published(lane, true);
    let id = if pipelined {
        pipeline(&mut f)[1]
    } else {
        f.first
    };
    let before = facts(&f);
    let recycle = FAULTS.iter().position(|step| *step == fault).unwrap() >= 6;
    let entries = if recycle {
        vec![(id, Step::PollReady), (id, fault)]
    } else {
        vec![(id, fault)]
    };
    steps(&mut f, &entries);
    let outcome = catch_unwind(AssertUnwindSafe(|| f.backend.poll_v1(id)));
    if matches!(
        fault,
        Step::PollUnwind
            | Step::RecycleUnwind
            | Step::PollOuterUnwindPending
            | Step::PollOuterUnwindReady
            | Step::RecycleOuterUnwindRetry
            | Step::RecycleOuterUnwindReady
    ) {
        assert!(outcome.is_err());
    } else {
        assert!(matches!(
            outcome,
            Ok(Err(RuntimeBackendFailureV1::Terminal(_)))
        ));
    }
    assert!(f.backend.terminal);
    assert_eq!(facts(&f), before);
    assert!(completion_events(&f, &[id]).is_empty());
    assert!(!f.backend.submissions.contains_key(&id));
    let execution = f
        .backend
        .active_compute_submission_v1(id)
        .unwrap()
        .execution
        .as_ref()
        .unwrap();
    assert!(match fault {
        Step::PollError | Step::PollUnwind => matches!(
            execution,
            ActiveComputeExecutionV1::Materialized(MaterializedCompletionReceiptV1::Consuming(
                MaterializedConsumeV1::Poll
            ))
        ),
        Step::RecycleError | Step::RecycleUnwind => matches!(
            execution,
            ActiveComputeExecutionV1::Materialized(MaterializedCompletionReceiptV1::Consuming(
                MaterializedConsumeV1::Recycle
            ))
        ),
        Step::PollOuterErrorPending | Step::PollOuterUnwindPending =>
            matches!(execution, ActiveComputeExecutionV1::ScriptedMaterialized),
        Step::PollOuterErrorReady
        | Step::PollOuterUnwindReady
        | Step::RecycleOuterErrorRetry
        | Step::RecycleOuterUnwindRetry => matches!(
            execution,
            ActiveComputeExecutionV1::ScriptedMaterializedCompleted
        ),
        Step::RecycleOuterErrorReady | Step::RecycleOuterUnwindReady => matches!(
            execution,
            ActiveComputeExecutionV1::ScriptedMaterializedRetired
        ),
        _ => false,
    });
    for outcome in [
        f.backend.poll_v1(id).map(|_| ()),
        f.backend.cancel_v1(id).map(|_| ()),
        f.backend.shutdown_native_v1(),
    ] {
        assert!(matches!(outcome, Err(RuntimeBackendFailureV1::Terminal(_))));
    }
    assert_eq!(facts(&f), before);
    assert!(
        f.backend
            .scripted_materialized_completion
            .as_ref()
            .unwrap()
            .is_empty()
    );
    if unrepaired_drop {
        eprintln!("ordinary completion indexed custody inspected; dropping unrepaired backend");
        drop(ManuallyDrop::into_inner(f.backend));
        panic!("unrepaired ordinary completion Drop returned");
    }
    discard_scripted_fixture(f.backend);
}

#[test]
fn materialized_completion_faults_preserve_indexed_frontier_and_pipeline() {
    for lane in 0..2 {
        for pipelined in [false, true] {
            for fault in FAULTS {
                fault_case(lane, pipelined, fault, false);
            }
        }
    }
}

#[test]
fn materialized_completion_unrepaired_drop_aborts() {
    const CHILD: &str = "FE2O3_TEST_MATERIALIZED_COMPLETION_DROP";
    const TEST: &str = "kfd_backend::tests::materialized_publication_tests::completion::materialized_completion_unrepaired_drop_aborts";
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
        fault_case(
            case / 24,
            !(case / 12).is_multiple_of(2),
            FAULTS[case % 12],
            true,
        );
        unreachable!();
    }
    for case in 0..48 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, case.to_string())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "case {case}: {stderr}");
        assert!(
            stderr.contains(
                "ordinary completion indexed custody inspected; dropping unrepaired backend"
            ),
            "case {case}: {stderr}"
        );
    }
}

#[test]
fn materialized_completion_retry_preserves_first_ready_and_commits_without_allocation() {
    for lane in 0..2 {
        for write in [false, true] {
            let mut f = published(lane, write);
            let id = f.first;
            mutate(&mut f, id, |a| {
                a.performance.completed_readback = Duration::from_millis(17);
                a.performance.completion_detach_restore = Duration::from_millis(19);
            });
            let before = facts(&f);
            steps(&mut f, &[(id, Step::PollPending)]);
            assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
            assert_eq!(facts(&f), before);
            steps(&mut f, &[(id, Step::PollReady), (id, Step::RecycleRetry)]);
            assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
            let first = f
                .backend
                .active_compute_submission_v1(id)
                .unwrap()
                .performance
                .publish_to_completion;
            assert_eq!(facts(&f), before);
            mutate(&mut f, id, |a| {
                a.published_at = Instant::now()
                    .checked_sub(Duration::from_secs(3600))
                    .unwrap()
            });
            steps(&mut f, &[(id, Step::RecycleRetry)]);
            let (result, allocations) =
                super::super::super::drain_capture::tests::counted(|| f.backend.poll_v1(id));
            assert_eq!(result.unwrap(), BackendPollV1::Pending);
            assert_eq!(allocations, 0);
            assert_eq!(
                f.backend
                    .active_compute_submission_v1(id)
                    .unwrap()
                    .performance
                    .publish_to_completion,
                first
            );
            steps(&mut f, &[(id, Step::RecycleReady)]);
            let (result, allocations) =
                super::super::super::drain_capture::tests::counted(|| f.backend.poll_v1(id));
            assert_eq!(result.unwrap(), BackendPollV1::Succeeded);
            assert_eq!(allocations, 0);
            assert_eq!(
                f.backend
                    .last_launch_performance
                    .unwrap()
                    .completed_readback,
                Duration::ZERO
            );
            assert_eq!(
                f.backend
                    .last_launch_performance
                    .unwrap()
                    .completion_detach_restore,
                Duration::ZERO
            );
            assert_eq!(
                f.backend
                    .last_launch_performance
                    .unwrap()
                    .publish_to_completion,
                first
            );
            assert_eq!(
                f.backend.allocations[&f.host].native_dirty.len(),
                usize::from(write)
            );
            assert_eq!(f.backend.native_dirty_extents, usize::from(write));
            assert_eq!(f.backend.compute_completion_reservations, 1 + lane);
            assert_eq!(f.backend.allocation_custody[&f.host].owners.len(), 1);
            assert_eq!(completion_events(&f, &[id]), [id]);
            assert_eq!(
                pending_facts(&f.backend.pending_compute[&f.trailing]),
                f.trailing_recipe
            );
            assert!(!f.backend.terminal);
            discard_scripted_fixture(f.backend);
        }
    }
}

#[test]
fn materialized_completion_out_of_order_retirement_commits_in_stream_order() {
    for lane in 0..2 {
        for fault_at in 0..4 {
            for existing_dirty in [false, true] {
                let mut f = published(lane, true);
                let [b, c] = pipeline(&mut f);
                let a = f.first;
                let ids = [a, b, c];
                if existing_dirty {
                    let active = f.backend.active_compute_submission_v1(a).unwrap();
                    let w = &active.writebacks[0];
                    let dirty = NativeDirtyExtentV1 {
                        compute_lane: lane,
                        data_index: w.data_index,
                        allocation_offset: w.allocation_offset,
                        data_offset: w.data_offset,
                        byte_len: w.byte_len,
                    };
                    let record = f.backend.allocations.get_mut(&f.host).unwrap();
                    record.native_dirty = vec![dirty];
                    assert_eq!(record.native_dirty.len(), record.native_dirty.capacity());
                    record.content_sha256 = None;
                    f.backend.native_dirty_extents = 1;
                }
                let before = facts(&f);
                for id in [c, b] {
                    steps(
                        &mut f,
                        &[
                            (id, Step::PollReady),
                            (id, Step::RecycleReady),
                            (a, Step::PollPending),
                        ],
                    );
                    assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
                    assert!(
                        f.backend
                            .scripted_materialized_completion
                            .as_ref()
                            .unwrap()
                            .is_empty()
                    );
                    assert_eq!(facts(&f), before);
                    assert!(completion_events(&f, &ids).is_empty());
                }
                let mut entries = vec![(a, Step::PollReady), (a, Step::RecycleReady)];
                if fault_at != 0 {
                    entries.push((ids[fault_at - 1], Step::ProfileUnwind));
                }
                steps(&mut f, &entries);
                let outcome = catch_unwind(AssertUnwindSafe(|| f.backend.poll_v1(a)));
                let committed = if fault_at == 0 {
                    assert_eq!(outcome.unwrap().unwrap(), BackendPollV1::Succeeded);
                    assert_eq!(completion_events(&f, &ids), ids);
                    3
                } else {
                    assert!(outcome.is_err());
                    assert!(f.backend.terminal);
                    assert_eq!(completion_events(&f, &ids), ids[..fault_at - 1]);
                    fault_at
                };
                assert!(
                    f.backend
                        .scripted_materialized_completion
                        .as_ref()
                        .unwrap()
                        .is_empty()
                );
                for id in &ids[..committed] {
                    assert_eq!(f.backend.submissions[id].status, BackendPollV1::Succeeded);
                    assert!(f.backend.active_compute_submission_v1(*id).is_none());
                }
                for id in &ids[committed..] {
                    assert!(f.backend.active_compute_submission_v1(*id).is_some());
                }
                let remaining: Vec<_> = f
                    .backend
                    .allocation_custody
                    .get(&f.host)
                    .map(|c| c.owners.iter().map(|owner| owner.submission).collect())
                    .unwrap_or_default();
                assert_eq!(remaining, ids[committed..]);
                assert_eq!(
                    f.backend
                        .compute_module_retain_counts
                        .get(&f.module)
                        .copied()
                        .unwrap_or(0),
                    3 - committed + lane
                );
                assert_eq!(
                    f.backend.compute_completion_reservations,
                    3 - committed + lane
                );
                assert_eq!(
                    f.backend
                        .compute_dependency_retain_counts
                        .get(&a)
                        .copied()
                        .unwrap_or(0),
                    usize::from(committed < 2)
                );
                assert_eq!(
                    f.backend
                        .compute_dependency_retain_counts
                        .get(&b)
                        .copied()
                        .unwrap_or(0),
                    usize::from(committed < 3)
                );
                assert_eq!(
                    f.backend.stream_compute_lanes.contains_key(&f.stream),
                    committed < 3
                );
                assert_eq!(f.backend.native_dirty_extents, 1);
                if let Some(primary) = &f.primary {
                    assert_eq!(
                        format!("{:?}", f.backend.active.as_ref().unwrap()),
                        *primary
                    );
                }
                discard_scripted_fixture(f.backend);
            }
        }
    }
}

#[test]
fn materialized_completion_promotes_published_and_completed_successors() {
    for lane in 0..2 {
        for completed in [false, true] {
            let mut f = published(lane, true);
            let [b, c] = pipeline(&mut f);
            let a = f.first;
            let before_ready = if completed {
                steps(
                    &mut f,
                    &[
                        (b, Step::PollReady),
                        (b, Step::RecycleRetry),
                        (a, Step::PollPending),
                    ],
                );
                assert_eq!(f.backend.poll_v1(b).unwrap(), BackendPollV1::Pending);
                Some(
                    f.backend
                        .active_compute_submission_v1(b)
                        .unwrap()
                        .performance
                        .publish_to_completion,
                )
            } else {
                None
            };
            let b_recipe = Arc::clone(
                f.backend
                    .active_compute_submission_v1(b)
                    .unwrap()
                    .ordinary_recipe
                    .as_ref()
                    .unwrap(),
            );
            let b_descriptors = f
                .backend
                .active_compute_submission_v1(b)
                .unwrap()
                .resident_descriptors
                .as_ptr();
            steps(&mut f, &[(a, Step::PollReady), (a, Step::RecycleReady)]);
            assert_eq!(f.backend.poll_v1(a).unwrap(), BackendPollV1::Succeeded);
            let active = f.backend.active_compute_submission_v1(b).unwrap();
            assert!(Arc::ptr_eq(
                active.ordinary_recipe.as_ref().unwrap(),
                &b_recipe
            ));
            assert_eq!(active.resident_descriptors.as_ptr(), b_descriptors);
            let entries = if completed {
                vec![(b, Step::RecycleReady)]
            } else {
                vec![(b, Step::PollReady), (b, Step::RecycleReady)]
            };
            steps(&mut f, &entries);
            assert_eq!(f.backend.poll_v1(b).unwrap(), BackendPollV1::Succeeded);
            if let Some(first) = before_ready {
                assert_eq!(
                    f.backend
                        .last_launch_performance
                        .unwrap()
                        .publish_to_completion,
                    first
                );
            }
            steps(&mut f, &[(c, Step::PollReady), (c, Step::RecycleReady)]);
            assert_eq!(f.backend.poll_v1(c).unwrap(), BackendPollV1::Succeeded);
            assert_eq!(completion_events(&f, &[a, b, c]), [a, b, c]);
            assert!(!f.backend.allocation_custody.contains_key(&f.host));
            assert_eq!(f.backend.compute_completion_reservations, lane);
            discard_scripted_fixture(f.backend);
        }
    }
}

#[test]
fn materialized_completion_pipeline_refuses_wrong_phase_and_missing_deferred_retain() {
    for lane in 0..2 {
        for phase_mismatch in [false, true] {
            let mut f = published(lane, true);
            let [b, _] = pipeline(&mut f);
            let a = f.first;
            if phase_mismatch {
                f.backend.with_compute_lane_state_v1(lane, |backend| {
                    let identity = backend
                        .compute_pipeline
                        .identity_for_submission_v1(b)
                        .unwrap();
                    backend
                        .compute_pipeline
                        .entry_mut_v1(identity)
                        .unwrap()
                        .phase = RuntimeComputePipelinePhaseV1::Completed;
                });
            } else {
                f.backend.compute_dependency_retain_counts.remove(&a);
            }
            let before = facts(&f);
            steps(&mut f, &[(b, Step::PollUnwind)]);
            assert!(matches!(
                f.backend.poll_v1(b),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
            assert_eq!(facts(&f), before);
            assert_eq!(
                f.backend
                    .scripted_materialized_completion
                    .as_ref()
                    .unwrap()
                    .len(),
                1
            );
            discard_scripted_fixture(f.backend);
        }
    }
}

#[test]
fn materialized_completion_preflight_and_commit_refuse_corruption_without_release() {
    for lane in 0..2 {
        for case in 0..9 {
            let mut f = published(lane, true);
            let id = f.first;
            steps(&mut f, &[(id, Step::PollReady), (id, Step::RecycleRetry)]);
            assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
            match case {
                0 => mutate(&mut f, id, |a| a.writebacks[0].byte_len -= 1),
                1 => {
                    f.backend.compute_module_retain_counts.remove(&f.module);
                }
                2 => {
                    f.backend.compute_completion_reservations = 0;
                }
                3 => {
                    f.backend
                        .allocation_custody
                        .get_mut(&f.host)
                        .unwrap()
                        .owner_counts[RuntimeAllocationCustodyKindV1::Compute.index()] = 0;
                }
                4 => mutate(&mut f, id, |a| a.deferred_ordered_predecessor_retain = true),
                5 => {
                    let record = f.backend.allocations.get_mut(&f.host).unwrap();
                    record.native_dirty = Vec::new();
                }
                6 => {
                    f.backend.native_dirty_extents = usize::MAX;
                }
                7 => {
                    f.backend.submissions.shrink_to_fit();
                    f.backend.compute_completion_reservations =
                        f.backend.submissions.capacity() + 1;
                }
                8 => mutate(&mut f, id, |a| {
                    a.resident_descriptors[0].allocation_offset += 1
                }),
                _ => unreachable!(),
            }
            // Case 4's predecessor is already released by initial publication.
            if case == 4 {
                f.backend
                    .compute_dependency_retain_counts
                    .remove(&f.predecessor);
            }
            let before = facts(&f);
            steps(&mut f, &[(id, Step::RecycleReady)]);
            assert!(matches!(
                f.backend.poll_v1(id),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
            assert_eq!(facts(&f), before);
            assert!(!f.backend.submissions.contains_key(&id));
            assert!(completion_events(&f, &[id]).is_empty());
            let remaining = f
                .backend
                .scripted_materialized_completion
                .as_ref()
                .unwrap()
                .len();
            assert_eq!(remaining, usize::from(!matches!(case, 5..=7)));
            discard_scripted_fixture(f.backend);
        }
    }
}
