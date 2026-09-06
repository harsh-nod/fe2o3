use super::r40_gfx942_striped_sdma_aggregate::*;
use alloc::vec;
use alloc::vec::Vec;

fn queue_ids(count: u32) -> Vec<u32> {
    (100..100 + count).collect()
}

fn combined(striped: u8) -> R40Gfx942SdmaQueuePlanV1 {
    R40Gfx942SdmaQueuePlanV1::create_model_only(
        7,
        R40Gfx942SdmaPlanKindV1::CombinedDirectionalStriped,
        striped,
        &queue_ids(u32::from(striped) + 2),
    )
    .unwrap()
}

fn standalone(striped: u8) -> R40Gfx942SdmaQueuePlanV1 {
    R40Gfx942SdmaQueuePlanV1::create_model_only(
        7,
        R40Gfx942SdmaPlanKindV1::StandaloneStriped,
        striped,
        &queue_ids(u32::from(striped)),
    )
    .unwrap()
}

fn submission(request_count: u16, deadline_ns: u64) -> R40AggregateSubmissionV1 {
    submission_with_presentation(request_count, deadline_ns).0
}

fn submission_with_presentation(
    request_count: u16,
    deadline_ns: u64,
) -> (R40AggregateSubmissionV1, R40AggregatePresentationV1) {
    R40AggregateSubmissionV1::new_with_presentation_model_only(
        combined(4),
        11,
        100,
        deadline_ns,
        request_count,
        13,
    )
    .unwrap()
}

fn assert_exact_submission(
    submission: &R40AggregateSubmissionV1,
    presentation: &R40AggregatePresentationV1,
) {
    assert_eq!(submission.plan_model_only(), &presentation.plan);
    assert_eq!(
        submission.shards_model_only(),
        presentation.shards.as_slice()
    );
    assert_eq!(
        submission.completion_validation_roster_model_only(),
        presentation.completion_validation_roster.as_slice()
    );
    assert!(submission.is_fully_prepared_model_only());
}

fn observations(
    submission: &R40AggregateSubmissionV1,
    state: R40AggregateObservationStateV1,
) -> Vec<R40AggregateObservationV1> {
    submission
        .shards_model_only()
        .iter()
        .map(|shard| R40AggregateObservationV1 {
            ticket: shard.ticket,
            state,
        })
        .collect()
}

#[test]
fn exact_gfx942_capacity_domain_is_balanced_and_session_scoped() {
    for striped in [2, 4, 6, 8, 10, 12, 14] {
        let plan = combined(striped);
        assert!(plan.is_exact_model_only());
        assert_eq!(
            plan.queues_on_engine_model_only(0),
            usize::from(striped / 2 + 1)
        );
        assert_eq!(
            plan.queues_on_engine_model_only(1),
            usize::from(striped / 2 + 1)
        );
        assert_eq!(plan.request_capacity_model_only(), u16::from(striped) * 63);
        assert_eq!(plan.queues[0].role, R40Gfx942SdmaQueueRoleV1::Directional);
        assert_eq!(plan.queues[0].engine_index, 0);
        assert_eq!(plan.queues[1].role, R40Gfx942SdmaQueueRoleV1::Directional);
        assert_eq!(plan.queues[1].engine_index, 1);
    }
    for striped in [2, 4, 6, 8, 10, 12, 14, 16] {
        let plan = standalone(striped);
        assert!(plan.is_exact_model_only());
        assert_eq!(
            plan.queues_on_engine_model_only(0),
            usize::from(striped / 2)
        );
        assert_eq!(
            plan.queues_on_engine_model_only(1),
            usize::from(striped / 2)
        );
    }
    assert_eq!(combined(14).request_capacity_model_only(), 882);
    assert_eq!(standalone(16).request_capacity_model_only(), 1008);
}

#[test]
fn capacity_rejects_out_of_range_odd_wrong_count_and_duplicate_ids() {
    for striped in [0, 1, 3, 15, 16, 17] {
        assert!(
            R40Gfx942SdmaQueuePlanV1::create_model_only(
                7,
                R40Gfx942SdmaPlanKindV1::CombinedDirectionalStriped,
                striped,
                &queue_ids(u32::from(striped) + 2),
            )
            .is_err()
        );
    }
    for striped in [0, 1, 3, 15, 17] {
        assert!(
            R40Gfx942SdmaQueuePlanV1::create_model_only(
                7,
                R40Gfx942SdmaPlanKindV1::StandaloneStriped,
                striped,
                &queue_ids(u32::from(striped)),
            )
            .is_err()
        );
    }
    assert_eq!(
        R40Gfx942SdmaQueuePlanV1::create_model_only(
            7,
            R40Gfx942SdmaPlanKindV1::CombinedDirectionalStriped,
            2,
            &[1, 2, 3],
        ),
        Err(R40Gfx942SdmaPlanErrorV1::WrongQueueIdCount)
    );
    assert_eq!(
        R40Gfx942SdmaQueuePlanV1::create_model_only(
            7,
            R40Gfx942SdmaPlanKindV1::CombinedDirectionalStriped,
            2,
            &[1, 2, 3, 3],
        ),
        Err(R40Gfx942SdmaPlanErrorV1::DuplicateSessionQueueId)
    );
    assert!(
        R40Gfx942SdmaQueuePlanV1::create_model_only(
            8,
            R40Gfx942SdmaPlanKindV1::CombinedDirectionalStriped,
            2,
            &[1, 2, 3, 4],
        )
        .is_ok()
    );
}

#[test]
fn aggregate_construction_is_bounded_and_binds_every_ticket_coordinate() {
    let mut invalid_plan = combined(2);
    invalid_plan.queues[2].engine_index = 1;
    assert_eq!(
        R40AggregateSubmissionV1::new_model_only(invalid_plan, 1, 2, 3, 1, 4),
        Err(R40AggregateCreateErrorV1::InvalidQueuePlan)
    );

    let plan = combined(2);
    assert_eq!(
        R40AggregateSubmissionV1::new_model_only(plan.clone(), 1, 2, 3, 0, 4),
        Err(R40AggregateCreateErrorV1::EmptySubmission)
    );
    assert_eq!(
        R40AggregateSubmissionV1::new_model_only(plan.clone(), 1, 2, 3, 127, 4),
        Err(R40AggregateCreateErrorV1::CapacityExceeded)
    );
    assert_eq!(
        R40AggregateSubmissionV1::new_model_only(plan.clone(), 1, 2, 3, 1, 0),
        Err(R40AggregateCreateErrorV1::ZeroQueueGeneration)
    );
    assert_eq!(
        R40AggregateSubmissionV1::new_model_only(plan, 1, 3, 2, 1, 4),
        Err(R40AggregateCreateErrorV1::InvalidDeadline)
    );

    let owner = submission(9, 200);
    for (index, shard) in owner.shards_model_only().iter().enumerate() {
        assert_eq!(usize::from(shard.ticket.request_index), index);
        assert_eq!(usize::from(shard.ticket.queue_slot), index % 4);
        assert_eq!(shard.ticket.queue_generation, 13);
        assert_eq!(shard.ticket.submission_id, 11);
        assert_eq!(shard.ticket.session_id, 7);
        assert_eq!(shard.payload_token, index as u64 + 1);
    }
}

#[test]
fn exact_presentation_is_validated_before_any_status_observation() {
    let cases: [fn(&mut R40AggregatePresentationV1); 6] = [
        |presented| presented.plan.submission_id += 1,
        |presented| presented.plan.queue_plan.session_id += 1,
        |presented| presented.shards.pop().map_or((), |_| ()),
        |presented| presented.shards[0].ticket.request_index += 1,
        |presented| presented.shards[0].ticket.queue_slot += 1,
        |presented| presented.completion_validation_roster[0].queue_generation += 1,
    ];
    for mutate in cases {
        let (owner, mut presented) = submission_with_presentation(4, 200);
        mutate(&mut presented);
        let ready = observations(&owner, R40AggregateObservationStateV1::Ready);
        match owner.poll_model_only(&presented, &ready, 150, &[true; 4]) {
            R40AggregatePollOutcomeV1::Terminal(terminal) => {
                assert_eq!(terminal.observation_count, 0);
                assert_eq!(terminal.retired_count, 0);
                assert_eq!(
                    terminal.reason,
                    R40AggregateTerminalReasonV1::PresentationSubstitution
                );
                assert_eq!(
                    terminal.submission_model_only().shards_model_only().len(),
                    4
                );
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    let (owner, mut presented) = submission_with_presentation(4, 200);
    presented.currentness_closed = false;
    let ready = observations(&owner, R40AggregateObservationStateV1::Ready);
    match owner.poll_model_only(&presented, &ready, 150, &[true; 4]) {
        R40AggregatePollOutcomeV1::Terminal(terminal) => {
            assert_eq!(terminal.observation_count, 0);
            assert_eq!(
                terminal.reason,
                R40AggregateTerminalReasonV1::CurrentnessNotClosed
            );
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn exact_observation_roster_is_validated_before_status_scan() {
    let cases: [fn(&mut Vec<R40AggregateObservationV1>); 5] = [
        |items| items.pop().map_or((), |_| ()),
        |items| items[0].ticket.request_index += 1,
        |items| items[0].ticket.queue_slot += 1,
        |items| items[0].ticket.queue_id += 1,
        |items| items[0].ticket.queue_generation += 1,
    ];
    for mutate in cases {
        let (owner, presented) = submission_with_presentation(4, 200);
        let mut observed = observations(
            &owner,
            R40AggregateObservationStateV1::Error { terminal_token: 99 },
        );
        mutate(&mut observed);
        match owner.poll_model_only(&presented, &observed, 150, &[true; 4]) {
            R40AggregatePollOutcomeV1::Terminal(terminal) => {
                assert_eq!(terminal.observation_count, 0);
                assert_eq!(terminal.retired_count, 0);
                assert_eq!(
                    terminal.reason,
                    R40AggregateTerminalReasonV1::ObservationRosterMismatch
                );
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }
}

#[test]
fn pending_scans_entire_roster_and_returns_exact_whole_submission() {
    for pending_index in 0..4 {
        let (owner, presented) = submission_with_presentation(4, 200);
        let mut observed = observations(&owner, R40AggregateObservationStateV1::Ready);
        observed[pending_index].state = R40AggregateObservationStateV1::Pending;
        match owner.poll_model_only(&presented, &observed, 199, &[]) {
            R40AggregatePollOutcomeV1::Pending(waiting) => {
                assert_eq!(waiting.observation_count, 4);
                assert_eq!(waiting.retired_count, 0);
                assert_eq!(
                    waiting
                        .submission_model_only()
                        .completion_validation_roster_model_only()
                        .len(),
                    4
                );
                assert!(
                    waiting
                        .submission_model_only()
                        .prepared_completed_output_capacity_model_only()
                        >= 4
                );
                assert!(
                    waiting
                        .submission_model_only()
                        .prepared_completed_output_is_empty_model_only()
                );
                assert_exact_submission(waiting.submission_model_only(), &presented);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }
}

#[test]
fn shared_absolute_deadline_times_out_only_after_full_scan() {
    for now_ns in [200, 201, u64::MAX] {
        let (owner, presented) = submission_with_presentation(4, 200);
        let observed = observations(&owner, R40AggregateObservationStateV1::Pending);
        match owner.poll_model_only(&presented, &observed, now_ns, &[]) {
            R40AggregatePollOutcomeV1::TimedOut(waiting) => {
                assert_eq!(waiting.observation_count, 4);
                assert_eq!(waiting.retired_count, 0);
                assert_eq!(
                    waiting
                        .submission_model_only()
                        .completion_validation_roster_model_only()
                        .len(),
                    4
                );
                assert!(
                    waiting
                        .submission_model_only()
                        .prepared_completed_output_capacity_model_only()
                        >= 4
                );
                assert!(
                    waiting
                        .submission_model_only()
                        .prepared_completed_output_is_empty_model_only()
                );
                assert_exact_submission(waiting.submission_model_only(), &presented);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }
}

#[test]
fn observation_error_stops_and_retains_terminal_custody() {
    for error_index in 0..4 {
        let (owner, presented) = submission_with_presentation(4, 200);
        let mut observed = observations(&owner, R40AggregateObservationStateV1::Pending);
        observed[error_index].state = R40AggregateObservationStateV1::Error { terminal_token: 97 };
        match owner.poll_model_only(&presented, &observed, 150, &[]) {
            R40AggregatePollOutcomeV1::Terminal(terminal) => {
                assert_eq!(terminal.observation_count, error_index + 1);
                assert_eq!(terminal.retired_count, 0);
                assert_eq!(
                    terminal.reason,
                    R40AggregateTerminalReasonV1::ObservationError { terminal_token: 97 }
                );
                assert_exact_submission(terminal.submission_model_only(), &presented);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }
}

#[test]
fn retirement_preflight_failure_moves_nothing_and_retains_terminal_custody() {
    let (owner, presented) = submission_with_presentation(4, 200);
    let ready = observations(&owner, R40AggregateObservationStateV1::Ready);
    match owner.poll_model_only(&presented, &ready, 150, &[true, true]) {
        R40AggregatePollOutcomeV1::Terminal(terminal) => {
            assert_eq!(terminal.observation_count, 4);
            assert_eq!(terminal.retired_count, 0);
            assert_eq!(
                terminal.reason,
                R40AggregateTerminalReasonV1::RetirementPreflightMismatch
            );
            assert_exact_submission(terminal.submission_model_only(), &presented);
        }
        other => panic!("unexpected outcome: {other:?}"),
    }

    for failed_index in 0..4 {
        let (owner, presented) = submission_with_presentation(4, 200);
        let ready = observations(&owner, R40AggregateObservationStateV1::Ready);
        let mut preflight = vec![true; 4];
        preflight[failed_index] = false;
        match owner.poll_model_only(&presented, &ready, 150, &preflight) {
            R40AggregatePollOutcomeV1::Terminal(terminal) => {
                assert_eq!(terminal.observation_count, 4);
                assert_eq!(terminal.retired_count, 0);
                assert_eq!(
                    terminal.reason,
                    R40AggregateTerminalReasonV1::RetirementPreflightFailed {
                        request_index: failed_index as u16,
                    }
                );
                assert_exact_submission(terminal.submission_model_only(), &presented);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }
}

#[test]
fn completed_retirement_moves_every_shard_in_request_order() {
    for request_count in [1, 2, 4, 17, 252] {
        let (owner, presented) = submission_with_presentation(request_count, 200);
        let ready = observations(&owner, R40AggregateObservationStateV1::Ready);
        let preflight = vec![true; usize::from(request_count)];
        match owner.poll_model_only(&presented, &ready, 150, &preflight) {
            R40AggregatePollOutcomeV1::Completed(completed) => {
                assert_eq!(completed.observation_count, usize::from(request_count));
                assert_eq!(completed.retired_count, usize::from(request_count));
                assert!(completed.retirement_is_complete_and_ordered_model_only());
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }
}

#[test]
fn post_retirement_retake_failure_retains_every_shard_in_terminal_custody() {
    let (owner, presented) = submission_with_presentation(17, 200);
    let ready = observations(&owner, R40AggregateObservationStateV1::Ready);
    match owner.poll_with_retake_model_only(
        &presented,
        &ready,
        150,
        &[true; 17],
        R40AggregateModelRetakeV1::Fails { terminal_token: 71 },
    ) {
        R40AggregatePollOutcomeV1::RetakeTerminal(terminal) => {
            assert_eq!(terminal.observation_count, 17);
            assert_eq!(terminal.retired_count, 17);
            assert_eq!(terminal.retained_shards_model_only().len(), 17);
            assert!(terminal.retains_every_retired_shard_in_order_model_only());
            assert_eq!(
                terminal.reason,
                R40AggregateTerminalReasonV1::ModelRetakeFailed { terminal_token: 71 }
            );
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn invalid_time_is_terminal_before_observation_and_waiting_can_resume_once() {
    let (owner, presented) = submission_with_presentation(2, 200);
    let ready = observations(&owner, R40AggregateObservationStateV1::Ready);
    match owner.poll_model_only(&presented, &ready, 99, &[true; 2]) {
        R40AggregatePollOutcomeV1::Terminal(terminal) => {
            assert_eq!(terminal.observation_count, 0);
            assert_eq!(terminal.retired_count, 0);
            assert_eq!(
                terminal.reason,
                R40AggregateTerminalReasonV1::InvalidObservationTime
            );
        }
        other => panic!("unexpected outcome: {other:?}"),
    }

    let (owner, presented) = submission_with_presentation(2, 200);
    let pending = observations(&owner, R40AggregateObservationStateV1::Pending);
    let owner = match owner.poll_model_only(&presented, &pending, 150, &[]) {
        R40AggregatePollOutcomeV1::Pending(waiting) => waiting.into_submission_model_only(),
        other => panic!("unexpected outcome: {other:?}"),
    };
    let ready = observations(&owner, R40AggregateObservationStateV1::Ready);
    assert!(matches!(
        owner.poll_model_only(&presented, &ready, 160, &[true; 2]),
        R40AggregatePollOutcomeV1::Completed(_)
    ));
}
