//! Source-event custody through real runtime publication and lower CPU receipts.

use super::super::super::materialized_source_event::SourceEventPhaseV1 as Phase;
use super::super::super::materialized_submission_attempt::MaterializedSubmissionAttemptV1 as Attempt;
use super::super::super::ordinary_queue_io::{
    CpuIoOperationV1 as Operation, CpuOuterFaultV1 as Fault,
    CpuSourceReleaseFaultV1 as ReleaseFault,
};
use super::*;
use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn record(f: &mut Fixture, id: u64) -> u64 {
    f.backend.record_event_v1(f.streams[f.lane], id).unwrap()
}

fn ledger(f: &mut Fixture) -> (u64, usize, usize) {
    let handle = f.backend.native_compute_lanes[f.lane].unwrap();
    f.backend
        .cpu_queue
        .as_mut()
        .unwrap()
        .fixture
        .with_lane(handle, |lane| lane.event_ledger_counts())
        .unwrap()
}

fn state(f: &Fixture, id: u64) -> (Phase, usize, usize) {
    owner(&f.backend, f.lane, id).source_event.snapshot()
}

#[test]
fn early_logical_events_share_one_native_pin_and_wait_does_not_require_destroy() {
    for lane in 0..2 {
        let mut f = Fixture::new(lane);
        let id = f.submit();
        let events = [record(&mut f, id), record(&mut f, id)];
        let before = ledger(&mut f);
        f.flush();
        assert_eq!(state(&f, id).0, Phase::Owned);
        assert_eq!(state(&f, id).1, 1);
        assert_eq!(ledger(&mut f), (before.0 + 1, 1, 0));
        assert_eq!(f.backend.event_submission_retain_counts[&id], 2);
        assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
        assert_eq!(state(&f, id).0, Phase::Owned);
        f.complete(id);
        assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
        assert_eq!(ledger(&mut f), (before.0 + 1, 0, 0));
        assert_eq!(f.backend.event_submission_retain_counts[&id], 2);
        assert!(f.backend.release_submission_v1(id).is_err());
        for event in events {
            f.backend.release_event_v1(event).unwrap();
        }
        f.cleanup();
    }
}

#[test]
fn absent_late_and_released_logical_events_do_not_mint_native_authority() {
    for lane in 0..2 {
        for released_before in [false, true] {
            let mut f = Fixture::new(lane);
            let id = f.submit();
            if released_before {
                let event = record(&mut f, id);
                f.backend.release_event_v1(event).unwrap();
            }
            let before = ledger(&mut f);
            f.flush();
            assert_eq!(state(&f, id), (Phase::Absent, 0, 0));
            let late = record(&mut f, id);
            assert_eq!(state(&f, id), (Phase::Absent, 0, 0));
            assert_eq!(ledger(&mut f), before);
            f.complete(id);
            assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
            let completed = record(&mut f, id);
            assert_eq!(ledger(&mut f), before);
            f.backend.release_event_v1(late).unwrap();
            f.backend.release_event_v1(completed).unwrap();
            f.cleanup();
        }
    }
}

#[test]
fn prepared_source_request_survives_retries_and_logical_event_release() {
    for lane in 0..2 {
        let mut f = Fixture::new(lane);
        let handle = f.backend.native_compute_lanes[lane].unwrap();
        f.backend
            .cpu_queue
            .as_mut()
            .unwrap()
            .fixture
            .with_lane(handle, |lane| lane.saturate_signals().unwrap())
            .unwrap();
        let id = f.submit();
        f.flush();
        assert_eq!(state(&f, id).0, Phase::Absent);
        let event = record(&mut f, id);
        let generation = f.generation_and_capacity().0;
        let before = ledger(&mut f);
        for retry in 0..3 {
            assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
            assert_eq!(state(&f, id), (Phase::Requested, 0, 0));
            assert_eq!(f.generation_and_capacity(), (generation + retry + 1, 0));
            assert_eq!(ledger(&mut f), before);
            assert_eq!(f.live_epochs(), 0);
            if retry == 0 {
                f.backend.release_event_v1(event).unwrap();
            }
        }
        f.backend
            .cpu_queue
            .as_mut()
            .unwrap()
            .fixture
            .with_lane(handle, |lane| lane.drain_saturation().unwrap())
            .unwrap();
        assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
        assert_eq!(state(&f, id).0, Phase::Owned);
        assert!(!f.backend.event_submission_retain_counts.contains_key(&id));
        f.complete(id);
        assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
        assert_eq!(ledger(&mut f), (before.0 + 1, 0, 0));
        f.cleanup();
    }
}

#[test]
fn ordered_source_retires_out_of_order_without_authorizing_its_dependent() {
    for lane in 0..2 {
        let mut f = Fixture::new(lane);
        let a = f.submit();
        let ea = record(&mut f, a);
        f.flush();
        let b = f.submit();
        let eb = record(&mut f, b);
        f.flush();
        assert_eq!(state(&f, a).0, Phase::Owned);
        assert_eq!(state(&f, b).0, Phase::Owned);
        let c = launch(
            &mut f.backend,
            f.streams[lane],
            f.kernel,
            f.hosts[lane],
            &[eb],
        );
        f.ids.push(c);
        assert!(f.backend.pending_compute.contains_key(&c));
        f.complete(b);
        assert_eq!(f.backend.poll_v1(b).unwrap(), BackendPollV1::Pending);
        assert_eq!(state(&f, b).0, Phase::Released);
        assert_eq!(state(&f, a).0, Phase::Owned);
        assert_eq!(ledger(&mut f).1, 1);
        assert!(f.backend.pending_compute.contains_key(&c));
        assert_eq!(f.backend.pending_compute[&c].explicit_dependency_cursor, 0);
        assert_eq!(f.publication_count(c), 0);
        f.complete(a);
        assert_eq!(f.backend.poll_v1(a).unwrap(), BackendPollV1::Succeeded);
        assert_eq!(f.backend.poll_v1(b).unwrap(), BackendPollV1::Succeeded);
        // This CPU provider only replaces its inert cache when both lanes are idle.
        if let Some(blocker) = f.blocker.take() {
            assert_eq!(
                f.primary_snapshot.as_ref().unwrap(),
                &f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots()[0]
            );
            complete(&mut f.backend, 0, blocker);
            assert_eq!(
                f.backend.poll_v1(blocker).unwrap(),
                BackendPollV1::Succeeded
            );
            f.ids.push(blocker);
        }
        f.flush();
        let target_lane = f.backend.stream_compute_lanes[&f.streams[lane]];
        complete(&mut f.backend, target_lane, c);
        assert_eq!(f.backend.poll_v1(c).unwrap(), BackendPollV1::Succeeded);
        for event in [ea, eb] {
            f.backend.release_event_v1(event).unwrap();
        }
        f.cleanup();
    }
}

#[test]
fn source_bundle_is_indexed_before_initial_and_ordered_outer_close_faults() {
    super::completion::isolated_cases(
        "kfd_backend::tests::cpu_receipt_tests::source_events::source_bundle_is_indexed_before_initial_and_ordered_outer_close_faults",
        "FE2O3_CPU_SOURCE_OUTER_CHILD",
        16,
        |case| {
            let lane = case / 8;
            let ordered = case % 8 >= 4;
            let retry = case % 4 >= 2;
            let fault = if case % 2 == 0 {
                Fault::Error
            } else {
                Fault::Unwind
            };
            let mut f = ManuallyDrop::new(Fixture::new(lane));
            if ordered {
                let _a = f.submit();
                f.flush();
            }
            let id = f.submit();
            let event = record(&mut f, id);
            f.backend
                .cpu_queue
                .as_mut()
                .unwrap()
                .lane_control
                .source_ring_full = retry;
            f.backend.cpu_queue.as_mut().unwrap().next_outer_fault =
                Some((Operation::Submit, fault));
            let stream = f.streams[lane];
            let outcome = catch_unwind(AssertUnwindSafe(|| f.backend.flush_stream_v1(stream)));
            match fault {
                Fault::Error => assert!(matches!(
                    outcome,
                    Ok(Err(RuntimeBackendFailureV1::Terminal(_)))
                )),
                Fault::Unwind => {
                    assert_eq!(*outcome.unwrap_err().downcast::<Fault>().unwrap(), fault)
                }
            }
            assert!(f.backend.terminal);
            assert!(f.backend.events.contains_key(&event));
            assert_eq!(
                state(&f, id).0,
                if retry {
                    Phase::Requested
                } else {
                    Phase::Owned
                }
            );
            assert_eq!(state(&f, id).1, usize::from(!retry));
            let active = owner(&f.backend, lane, id);
            let attempt = match active.execution.as_ref().unwrap() {
                ActiveComputeExecutionV1::MaterializedBinding(root) => &root.submission,
                ActiveComputeExecutionV1::MaterializedSuccessorPublication(root) => &root.attempt,
                _ => panic!("publication custody settled after outer failure"),
            };
            if retry {
                assert!(matches!(attempt, Attempt::Retryable));
            } else {
                let Attempt::Published(batch) = attempt else {
                    panic!("exact published batch was lost");
                };
                assert!(f.returned_identity().matches_published(batch));
            }
            assert_eq!(f.publication_count(id), 0);
            assert_eq!(f.backend.selected_compute_lane, 0);
            let before = f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots();
            assert!(matches!(
                f.backend.poll_v1(id),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
            assert_eq!(
                f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots(),
                before
            );
        },
    );
}

#[test]
fn ordered_no_effect_withdrawal_resamples_logical_source_requests() {
    for lane in 0..2 {
        for release_request in [false, true] {
            let mut f = Fixture::new(lane);
            let a = f.submit();
            f.flush();
            let b = f.submit();
            let event = record(&mut f, b);
            let before = ledger(&mut f);
            let capacity = f.generation_and_capacity().1;
            for retry in 0..2 {
                f.backend
                    .cpu_queue
                    .as_mut()
                    .unwrap()
                    .lane_control
                    .source_ring_full = true;
                f.flush();
                assert!(f.backend.pending_compute.contains_key(&b));
                assert!(f.backend.active_compute_submission_v1(b).is_none());
                assert_eq!(f.live_epochs(), 1);
                assert_eq!(f.generation_and_capacity().1, capacity);
                assert_eq!(ledger(&mut f), (before.0 + retry + 1, 0, 0));
                assert_eq!(f.publication_count(b), 0);
            }
            if release_request {
                f.backend.release_event_v1(event).unwrap();
            }
            f.flush();
            assert_eq!(
                state(&f, b).0,
                if release_request {
                    Phase::Absent
                } else {
                    Phase::Owned
                }
            );
            assert_eq!(f.publication_count(b), 1);
            f.complete(b);
            assert_eq!(f.backend.poll_v1(b).unwrap(), BackendPollV1::Pending);
            f.complete(a);
            assert_eq!(f.backend.poll_v1(a).unwrap(), BackendPollV1::Succeeded);
            assert_eq!(f.backend.poll_v1(b).unwrap(), BackendPollV1::Succeeded);
            if !release_request {
                f.backend.release_event_v1(event).unwrap();
            }
            f.cleanup();
        }
    }
}

#[test]
fn source_release_failures_preserve_completed_custody_and_never_recycle() {
    super::completion::isolated_cases(
        "kfd_backend::tests::cpu_receipt_tests::source_events::source_release_failures_preserve_completed_custody_and_never_recycle",
        "FE2O3_CPU_SOURCE_RELEASE_CHILD",
        8,
        |case| {
            let lane = case / 4;
            let fault = [
                ReleaseFault::Refuse,
                ReleaseFault::Terminal,
                ReleaseFault::UnwindBefore,
                ReleaseFault::UnwindAfter,
            ][case % 4];
            let mut f = ManuallyDrop::new(Fixture::new(lane));
            let id = f.submit();
            let event = record(&mut f, id);
            f.flush();
            let identity = f.identity(id);
            let capacity = state(&f, id).2;
            f.complete(id);
            let before = f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots();
            f.backend
                .cpu_queue
                .as_mut()
                .unwrap()
                .lane_control
                .source_release_fault = Some(fault);
            let result = catch_unwind(AssertUnwindSafe(|| f.backend.poll_v1(id)));
            if matches!(
                fault,
                ReleaseFault::UnwindBefore | ReleaseFault::UnwindAfter
            ) {
                assert_eq!(
                    *result.unwrap_err().downcast::<ReleaseFault>().unwrap(),
                    fault
                );
            } else {
                assert!(matches!(
                    result,
                    Ok(Err(RuntimeBackendFailureV1::Terminal(_)))
                ));
            }
            assert!(f.backend.terminal);
            assert_eq!(f.backend.selected_compute_lane, 0);
            let active = owner(&f.backend, lane, id);
            let Some(ActiveComputeExecutionV1::Materialized(
                MaterializedCompletionReceiptV1::Completed(batch),
            )) = active.execution.as_ref()
            else {
                panic!("source release lost exact completed dispatch");
            };
            assert!(identity.matches_completed(batch));
            assert_eq!(
                state(&f, id),
                if fault == ReleaseFault::Refuse {
                    (Phase::Owned, 1, capacity)
                } else {
                    (Phase::ConsumingRelease, 0, capacity)
                }
            );
            let after = f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots();
            assert_eq!(after[lane].reset_signals(), before[lane].reset_signals());
            assert_eq!(
                after[lane].event_ledger_counts().1,
                usize::from(fault != ReleaseFault::UnwindAfter)
            );
            assert!(after[1 - lane].same_custody(&before[1 - lane]));
            assert!(f.backend.events.contains_key(&event));
            assert!(!f.backend.submissions.contains_key(&id));
            assert_eq!(
                f.backend
                    .cpu_queue
                    .as_ref()
                    .unwrap()
                    .lane_control
                    .source_release_calls,
                1
            );
            assert!(matches!(
                f.backend.poll_v1(id),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
            assert_eq!(
                f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots(),
                after
            );
        },
    );
}

#[test]
fn completion_outer_faults_keep_source_and_dispatch_phases_paired() {
    super::completion::isolated_cases(
        "kfd_backend::tests::cpu_receipt_tests::source_events::completion_outer_faults_keep_source_and_dispatch_phases_paired",
        "FE2O3_CPU_SOURCE_COMPLETE_CHILD",
        8,
        |case| {
            let lane = case / 4;
            let recycle = case % 4 >= 2;
            let fault = if case % 2 == 0 {
                Fault::Error
            } else {
                Fault::Unwind
            };
            let mut f = ManuallyDrop::new(Fixture::new(lane));
            let id = f.submit();
            let _event = record(&mut f, id);
            f.flush();
            let identity = f.identity(id);
            f.complete(id);
            f.backend.cpu_queue.as_mut().unwrap().next_outer_fault = Some((
                if recycle {
                    Operation::Recycle
                } else {
                    Operation::Poll
                },
                fault,
            ));
            let result = catch_unwind(AssertUnwindSafe(|| f.backend.poll_v1(id)));
            match fault {
                Fault::Error => assert!(matches!(
                    result,
                    Ok(Err(RuntimeBackendFailureV1::Terminal(_)))
                )),
                Fault::Unwind => {
                    assert_eq!(*result.unwrap_err().downcast::<Fault>().unwrap(), fault)
                }
            }
            assert!(f.backend.terminal);
            let active = owner(&f.backend, lane, id);
            match active.execution.as_ref().unwrap() {
                ActiveComputeExecutionV1::Materialized(
                    MaterializedCompletionReceiptV1::Completed(batch),
                ) if !recycle => assert!(identity.matches_completed(batch)),
                ActiveComputeExecutionV1::Materialized(
                    MaterializedCompletionReceiptV1::Retired(observation),
                ) if recycle => assert_eq!(observation.packet_count(), 1),
                _ => panic!("wrong post-close dispatch phase"),
            }
            assert_eq!(
                state(&f, id).0,
                if recycle {
                    Phase::Released
                } else {
                    Phase::Owned
                }
            );
            assert_eq!(
                f.backend
                    .cpu_queue
                    .as_ref()
                    .unwrap()
                    .lane_control
                    .source_release_calls,
                usize::from(recycle)
            );
            assert!(!f.backend.submissions.contains_key(&id));
        },
    );
}

#[test]
fn requested_source_survives_outer_failure_after_genuine_capacity_retry() {
    super::completion::isolated_cases(
        "kfd_backend::tests::cpu_receipt_tests::source_events::requested_source_survives_outer_failure_after_genuine_capacity_retry",
        "FE2O3_CPU_SOURCE_RETRY_CHILD",
        4,
        |case| {
            let lane = case / 2;
            let fault = if case % 2 == 0 {
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
            let id = f.submit();
            let _event = record(&mut f, id);
            f.backend.cpu_queue.as_mut().unwrap().next_outer_fault =
                Some((Operation::Submit, fault));
            let stream = f.streams[lane];
            let result = catch_unwind(AssertUnwindSafe(|| f.backend.flush_stream_v1(stream)));
            match fault {
                Fault::Error => assert!(matches!(
                    result,
                    Ok(Err(RuntimeBackendFailureV1::Terminal(_)))
                )),
                Fault::Unwind => {
                    assert_eq!(*result.unwrap_err().downcast::<Fault>().unwrap(), fault)
                }
            }
            assert!(f.backend.terminal);
            assert_eq!(state(&f, id), (Phase::Requested, 0, 0));
            assert!(
                matches!(owner(&f.backend, lane, id).execution.as_ref(), Some(ActiveComputeExecutionV1::MaterializedBinding(root)) if matches!(root.submission, Attempt::Retryable))
            );
            assert_eq!(f.publication_count(id), 0);
            assert_eq!(
                f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots()[lane]
                    .event_ledger_counts()
                    .1,
                0
            );
        },
    );
}

#[test]
fn source_pin_blocks_lower_recycle_and_wrong_session_returns_exact_event() {
    for auxiliary in [false, true] {
        let mut queue = CpuFixedDispatchFixtureV1::new().unwrap();
        let mut wrong = CpuFixedDispatchFixtureV1::new().unwrap();
        let lane = if auxiliary {
            queue.auxiliary_lane()
        } else {
            queue.primary_lane()
        };
        let source = queue
            .with_lane(lane, |lane| lane.submit_dependency_source().unwrap())
            .unwrap();
        let (batch, mut events) = source.into_parts();
        let identity = queue
            .with_lane(lane, |lane| lane.identity(&batch).unwrap())
            .unwrap();
        let completed = queue
            .with_lane(lane, |lane| {
                lane.complete_signal(&batch).unwrap();
                let Gfx942DispatchPollV1::Ready(completed) = lane.poll(batch).unwrap() else {
                    panic!("real CPU Ready");
                };
                let failure = lane.recycle(completed).unwrap_err();
                failure
                    .into_parts()
                    .1
                    .expect("exact pin-blocked completed receipt")
            })
            .unwrap();
        assert!(identity.matches_completed(&completed));
        let before = queue.snapshots();
        let wrong_before = wrong.snapshots();
        let failure = wrong
            .release_dependency_event(events.pop().unwrap())
            .unwrap_err();
        let returned = failure.into_parts().1.unwrap();
        assert_eq!(queue.snapshots(), before);
        assert_eq!(wrong.snapshots(), wrong_before);
        queue.release_dependency_event(*returned).unwrap();
        queue
            .with_lane(lane, |lane| lane.recycle(completed).unwrap())
            .unwrap();
        queue.ensure_clean().unwrap();
        wrong.ensure_clean().unwrap();
    }
}
