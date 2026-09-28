//! Real pin/recycle and outer-close composition; no native signal or binding claim.

use super::super::super::materialized_submission_attempt::MaterializedSubmissionAttemptV1 as Attempt;
use super::super::super::ordinary_queue_io::{
    CpuIoOperationV1 as Operation, CpuOuterFaultV1 as Fault,
};
use super::super::initial_publication_tests::pending_facts;
use super::*;
use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn publish(f: &mut Fixture, pinned: bool) -> (u64, CpuDispatchIdentityV1) {
    let id = f.submit();
    f.backend
        .cpu_queue
        .as_mut()
        .unwrap()
        .lane_control
        .pin_next_submission = pinned;
    f.flush();
    assert!(
        !f.backend
            .cpu_queue
            .as_ref()
            .unwrap()
            .lane_control
            .pin_next_submission
    );
    (id, f.identity(id))
}

fn unpin(f: &mut Fixture, identity: CpuDispatchIdentityV1) {
    let handle = f.backend.native_compute_lanes[f.lane].unwrap();
    f.backend
        .cpu_queue
        .as_mut()
        .unwrap()
        .fixture
        .with_lane(handle, |lane| lane.release_pin(identity).unwrap())
        .unwrap();
}

fn logical_facts(f: &Fixture) -> String {
    let b = &f.backend;
    let mut pending: Vec<_> = b.pending_compute.values().map(pending_facts).collect();
    pending.sort();
    format!(
        "{:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {} {:?}",
        b.allocation_custody,
        b.compute_module_retain_counts,
        b.compute_dependency_retain_counts,
        b.event_submission_retain_counts,
        b.pending_compute_streams,
        b.stream_submission_tails,
        b.stream_compute_lanes,
        b.submissions,
        b.compute_completion_reservations,
        pending
    )
}

fn completed_events(f: &Fixture, ids: &[u64]) -> Vec<u64> {
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

fn owner_metadata(f: &Fixture, id: u64) -> String {
    let active = owner(&f.backend, f.lane, id);
    format!(
        "{} {} {} {:?} {} {} {:?} {:?} {:?} {:?} {:p} {:p} {:p}",
        active.id,
        active.stream,
        active.kernel,
        active.ordered_predecessor,
        active.deferred_ordered_predecessor_retain,
        active.dependency_depth,
        active.allocations,
        active.resident_descriptors,
        active.writebacks,
        active.dispatch_shape_sha256,
        Arc::as_ptr(active.ordinary_recipe.as_ref().unwrap()),
        active.resident_descriptors.as_ptr(),
        active.writebacks.as_ptr()
    )
}

fn assert_completed(f: &Fixture, id: u64, identity: CpuDispatchIdentityV1) {
    let Some(ActiveComputeExecutionV1::Materialized(MaterializedCompletionReceiptV1::Completed(
        batch,
    ))) = owner(&f.backend, f.lane, id).execution.as_ref()
    else {
        panic!("exact completed receipt remains indexed");
    };
    assert!(identity.matches_completed(batch));
}

pub(super) fn isolated_cases(test: &str, variable: &str, count: usize, body: impl Fn(usize)) {
    if let Ok(case) = std::env::var(variable) {
        let case: usize = case.parse().unwrap();
        assert!(case < count);
        body(case);
        println!("CPU_COMPLETION_VERIFIED_{variable}_{case}");
        return;
    }
    for case in 0..count {
        let output = std::process::Command::new("sh")
            .args(["-c", "ulimit -c 0; exec \"$@\"", "cpu-completion-child"])
            .arg(std::env::current_exe().unwrap())
            .args(["--exact", test, "--nocapture"])
            .env(variable, case.to_string())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{test} case {case}: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .contains(&format!("CPU_COMPLETION_VERIFIED_{variable}_{case}"))
        );
    }
}

#[test]
fn cpu_receipts_pinned_recycles_keep_exact_completed_owner_until_unpin() {
    for lane in 0..2 {
        for pipeline in [false, true] {
            let mut f = Fixture::new(lane);
            let (a, a_identity) = publish(&mut f, true);
            f.complete(a);
            assert_eq!(f.backend.poll_v1(a).unwrap(), BackendPollV1::Pending);
            assert_completed(&f, a, a_identity);
            let (target, identity) = if pipeline {
                let (b, identity) = publish(&mut f, true);
                f.complete(b);
                assert_eq!(f.backend.poll_v1(b).unwrap(), BackendPollV1::Pending);
                (b, identity)
            } else {
                (a, a_identity)
            };
            let facts = logical_facts(&f);
            let timing = owner(&f.backend, lane, target)
                .performance
                .publish_to_completion;
            let before = f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots();
            for _ in 0..3 {
                let pins = f
                    .backend
                    .cpu_queue
                    .as_ref()
                    .unwrap()
                    .lane_control
                    .pinned_recycles;
                assert_eq!(f.backend.poll_v1(target).unwrap(), BackendPollV1::Pending);
                assert_eq!(
                    f.backend
                        .cpu_queue
                        .as_ref()
                        .unwrap()
                        .lane_control
                        .pinned_recycles,
                    pins + 1 + usize::from(pipeline)
                );
                assert_completed(&f, target, identity);
                assert_eq!(
                    owner(&f.backend, lane, target)
                        .performance
                        .publish_to_completion,
                    timing
                );
                assert_eq!(logical_facts(&f), facts);
                assert_eq!(
                    f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots(),
                    before
                );
                assert!(completed_events(&f, &[a, target]).is_empty());
            }
            unpin(&mut f, identity);
            let status = f.backend.poll_v1(target).unwrap();
            if pipeline {
                assert_eq!(status, BackendPollV1::Pending);
                assert!(matches!(
                    owner(&f.backend, lane, target).execution,
                    Some(ActiveComputeExecutionV1::Materialized(
                        MaterializedCompletionReceiptV1::Retired(_)
                    ))
                ));
                assert_eq!(logical_facts(&f), facts);
                assert_eq!(f.live_epochs(), 1);
                unpin(&mut f, a_identity);
                assert_eq!(f.backend.poll_v1(a).unwrap(), BackendPollV1::Succeeded);
                assert_eq!(f.backend.poll_v1(target).unwrap(), BackendPollV1::Succeeded);
                assert_eq!(completed_events(&f, &[a, target]), [a, target]);
            } else {
                assert_eq!(status, BackendPollV1::Succeeded);
                assert_eq!(completed_events(&f, &[target]), [target]);
            }
            assert_eq!(f.live_epochs(), 0);
            f.cleanup();
        }
    }
}

#[test]
fn cpu_receipts_completion_outer_fault_preserves_deposited_phase() {
    isolated_cases(
        "kfd_backend::tests::cpu_receipt_tests::completion::cpu_receipts_completion_outer_fault_preserves_deposited_phase",
        "FE2O3_CPU_COMPLETION_OUTER_CHILD",
        48,
        |case| {
            let lane = case & 1;
            let pipeline = case & 2 != 0;
            let fault = if case & 4 == 0 {
                Fault::Error
            } else {
                Fault::Unwind
            };
            let phase = case / 8;
            let mut f = ManuallyDrop::new(Fixture::new(lane));
            let predecessor = pipeline.then(|| publish(&mut f, false));
            let (target, identity) = publish(&mut f, matches!(phase, 2 | 4 | 5));
            let trailing = f.submit();
            let trailing_facts = pending_facts(&f.backend.pending_compute[&trailing]);
            if phase != 0 {
                f.complete(target);
            }
            if phase >= 4 {
                assert_eq!(f.backend.poll_v1(target).unwrap(), BackendPollV1::Pending);
                assert_completed(&f, target, identity);
                if phase == 5 {
                    unpin(&mut f, identity);
                }
            }
            f.backend.with_compute_lane_state_v1(lane, |b| {
                let a = if b.active.as_ref().is_some_and(|a| a.id == target) {
                    b.active.as_mut().unwrap()
                } else {
                    let identity = b
                        .compute_pipeline
                        .identity_for_submission_v1(target)
                        .unwrap();
                    &mut b.compute_pipeline.entry_mut_v1(identity).unwrap().active
                };
                if phase < 4 {
                    a.performance.publish_to_completion = Duration::from_secs(123);
                }
                a.performance.completion_signal_recycle = Duration::from_secs(17);
            });
            let facts = logical_facts(&f);
            let metadata = owner_metadata(&f, target);
            let before = f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots();
            let pin_retries = f
                .backend
                .cpu_queue
                .as_ref()
                .unwrap()
                .lane_control
                .pinned_recycles;
            let operation = if phase < 2 {
                Operation::Poll
            } else {
                Operation::Recycle
            };
            f.backend.cpu_queue.as_mut().unwrap().next_outer_fault = Some((operation, fault));
            let ready_before = owner(&f.backend, lane, target)
                .performance
                .publish_to_completion;
            let published_at = owner(&f.backend, lane, target).published_at;
            let elapsed_before = published_at.elapsed();
            let result = catch_unwind(AssertUnwindSafe(|| f.backend.poll_v1(target)));
            let elapsed_after = published_at.elapsed();
            match fault {
                Fault::Error => assert!(matches!(
                    result,
                    Ok(Err(RuntimeBackendFailureV1::Terminal(_)))
                )),
                Fault::Unwind => assert_eq!(
                    *result.unwrap_err().downcast::<Fault>().unwrap(),
                    Fault::Unwind
                ),
            }
            assert_eq!(logical_facts(&f), facts);
            assert_eq!(owner_metadata(&f, target), metadata);
            assert_eq!(
                pending_facts(&f.backend.pending_compute[&trailing]),
                trailing_facts
            );
            assert!(f.backend.terminal);
            assert_eq!(f.backend.selected_compute_lane, 0);
            let active = owner(&f.backend, lane, target);
            let Some(ActiveComputeExecutionV1::Materialized(receipt)) = active.execution.as_ref()
            else {
                panic!("indexed completion receipt");
            };
            match (phase, receipt) {
                (0, MaterializedCompletionReceiptV1::Published(batch)) => {
                    assert!(identity.matches_published(batch))
                }
                (1 | 2 | 4, MaterializedCompletionReceiptV1::Completed(batch)) => {
                    assert!(identity.matches_completed(batch))
                }
                (3 | 5, MaterializedCompletionReceiptV1::Retired(observation)) => {
                    assert_eq!(observation.packet_count(), 1)
                }
                _ => panic!("unexpected deposited completion phase"),
            }
            assert_eq!(active.published_at, published_at);
            if phase == 0 || phase >= 4 {
                assert_eq!(active.performance.publish_to_completion, ready_before);
            } else {
                assert!(active.performance.publish_to_completion >= elapsed_before);
                assert!(active.performance.publish_to_completion <= elapsed_after);
            }
            assert_eq!(
                active.performance.completion_signal_recycle,
                Duration::from_secs(17)
            );
            assert_eq!(f.publication_count(target), 1);
            assert!(completed_events(&f, &[target]).is_empty());
            let cpu = f.backend.cpu_queue.as_ref().unwrap();
            assert!(cpu.fixture.is_terminal());
            assert!(cpu.next_outer_fault.is_none());
            assert_eq!(
                cpu.lane_control.pinned_recycles,
                pin_retries + usize::from(matches!(phase, 2 | 4))
            );
            let after = cpu.fixture.snapshots();
            assert!(after[lane].same_custody(cpu.before_outer_fault.as_ref().unwrap()));
            assert!(after[1 - lane].same_custody(&before[1 - lane]));
            if let Some((id, identity)) = predecessor {
                let Some(ActiveComputeExecutionV1::Materialized(
                    MaterializedCompletionReceiptV1::Published(batch),
                )) = owner(&f.backend, lane, id).execution.as_ref()
                else {
                    panic!("published predecessor");
                };
                assert!(identity.matches_published(batch));
                let pipeline = if lane == 0 {
                    &f.backend.compute_pipeline
                } else {
                    &f.backend.auxiliary_compute_lanes[lane - 1].pipeline
                };
                assert_eq!(
                    pipeline
                        .entry_v1(pipeline.identity_for_submission_v1(target).unwrap())
                        .unwrap()
                        .phase,
                    RuntimeComputePipelinePhaseV1::Quarantined
                );
            }
            if lane == 1 {
                assert_eq!(
                    f.primary_owner.as_ref().unwrap(),
                    &format!("{:?}", f.backend.active)
                );
            }
            let phase_before = format!("{:?}", owner(&f.backend, lane, target));
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
            assert_eq!(logical_facts(&f), facts);
            assert_eq!(
                format!("{:?}", owner(&f.backend, lane, target)),
                phase_before
            );
            assert_eq!(
                f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots(),
                after
            );
        },
    );
}

#[test]
fn cpu_receipts_retryable_outer_fault_never_confirms_prepared_outcome() {
    isolated_cases(
        "kfd_backend::tests::cpu_receipt_tests::completion::cpu_receipts_retryable_outer_fault_never_confirms_prepared_outcome",
        "FE2O3_CPU_RETRYABLE_OUTER_CHILD",
        8,
        |case| {
            let lane = case & 1;
            let repeat = case & 2 != 0;
            let fault = if case & 4 == 0 {
                Fault::Error
            } else {
                Fault::Unwind
            };
            let mut f = ManuallyDrop::new(Fixture::new(lane));
            let handle = f.backend.native_compute_lanes[lane].unwrap();
            f.backend
                .cpu_queue
                .as_mut()
                .unwrap()
                .fixture
                .with_lane(handle, |lane| lane.saturate_signals().unwrap())
                .unwrap();
            let target = f.submit();
            let recipe = Arc::clone(&f.backend.pending_compute[&target].launch);
            if repeat {
                f.flush();
            }
            let trailing = f.submit();
            let trailing_facts = pending_facts(&f.backend.pending_compute[&trailing]);
            let reservations = f.backend.compute_completion_reservations;
            let retains = f.backend.compute_module_retain_counts.clone();
            let allocations = format!("{:?}", f.backend.allocation_custody);
            let before = f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots();
            let cpu = f.backend.cpu_queue.as_mut().unwrap();
            cpu.lane_control.last_submitted_identity = None;
            cpu.next_outer_fault = Some((Operation::Submit, fault));
            let stream = f.streams[lane];
            let result = catch_unwind(AssertUnwindSafe(|| {
                if repeat {
                    f.backend.poll_v1(target).map(|_| ())
                } else {
                    f.backend.flush_stream_v1(stream)
                }
            }));
            match fault {
                Fault::Error => assert!(matches!(
                    result,
                    Ok(Err(RuntimeBackendFailureV1::Terminal(_)))
                )),
                Fault::Unwind => assert_eq!(
                    *result.unwrap_err().downcast::<Fault>().unwrap(),
                    Fault::Unwind
                ),
            }
            assert!(f.backend.terminal);
            assert_eq!(f.backend.selected_compute_lane, 0);
            assert!(!f.backend.pending_compute.contains_key(&target));
            assert_eq!(f.backend.pending_compute_streams[&stream], [trailing]);
            assert_eq!(
                pending_facts(&f.backend.pending_compute[&trailing]),
                trailing_facts
            );
            assert_eq!(f.backend.compute_completion_reservations, reservations);
            assert_eq!(f.backend.compute_module_retain_counts, retains);
            assert_eq!(format!("{:?}", f.backend.allocation_custody), allocations);
            assert_eq!(
                f.backend.compute_dependency_retain_counts[&f.predecessor],
                1
            );
            assert_eq!(f.backend.compute_dependency_retain_counts[&target], 1);
            let active = owner(&f.backend, lane, target);
            assert!(Arc::ptr_eq(
                active.ordinary_recipe.as_ref().unwrap(),
                &recipe
            ));
            let Some(ActiveComputeExecutionV1::MaterializedBinding(root)) =
                active.execution.as_ref()
            else {
                panic!("unconfirmed binding outcome");
            };
            assert!(matches!(root.submission, Attempt::Retryable));
            assert_eq!(f.publication_count(target), 0);
            assert!(!f.backend.submissions.contains_key(&target));
            assert!(completed_events(&f, &[target]).is_empty());
            let cpu = f.backend.cpu_queue.as_ref().unwrap();
            assert!(cpu.lane_control.last_submitted_identity.is_none());
            assert!(cpu.fixture.is_terminal());
            let after = cpu.fixture.snapshots();
            assert!(after[lane].same_custody(cpu.before_outer_fault.as_ref().unwrap()));
            assert!(after[1 - lane].same_custody(&before[1 - lane]));
            if lane == 1 {
                assert_eq!(
                    f.primary_owner.as_ref().unwrap(),
                    &format!("{:?}", f.backend.active)
                );
            }
            let facts = logical_facts(&f);
            assert!(matches!(
                f.backend.poll_v1(target),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
            assert!(matches!(
                f.backend.shutdown_native_v1(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
            assert_eq!(logical_facts(&f), facts);
            assert_eq!(
                f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots(),
                after
            );
        },
    );
}
