use alloc::vec::Vec;

use super::r40_gfx942_striped_sdma_aggregate::{
    R40AggregateObservationStateV1, R40AggregatePresentationV1, R40AggregateSubmissionV1,
    R40Gfx942SdmaPlanKindV1, R40Gfx942SdmaQueuePlanV1,
};
use super::r46_gfx942_striped_sdma_tail_wait::*;

fn r40_submission(
    striped_count: u8,
    request_count: u16,
    deadline_ns: u64,
) -> (R40AggregateSubmissionV1, R40AggregatePresentationV1) {
    let queue_ids: Vec<u32> = (100..100 + u32::from(striped_count) + 2).collect();
    let plan = R40Gfx942SdmaQueuePlanV1::create_model_only(
        7,
        R40Gfx942SdmaPlanKindV1::CombinedDirectionalStriped,
        striped_count,
        &queue_ids,
    )
    .unwrap();
    R40AggregateSubmissionV1::new_with_presentation_model_only(
        plan,
        11,
        100,
        deadline_ns,
        request_count,
        13,
    )
    .unwrap()
}

fn tails_for(submission: &R40AggregateSubmissionV1) -> Vec<R46AuthenticatedTailFenceV1> {
    let plan = submission.plan_model_only();
    (0..plan.queue_plan.striped_queue_count)
        .filter_map(|queue_slot| {
            let last = submission
                .shards_model_only()
                .iter()
                .rev()
                .find(|shard| shard.ticket.queue_slot == queue_slot)?;
            let queue = plan
                .queue_plan
                .striped_queue_model_only(queue_slot)
                .unwrap();
            Some(R46AuthenticatedTailFenceV1 {
                session_id: plan.queue_plan.session_id,
                submission_id: plan.submission_id,
                engine_index: queue.engine_index,
                queue_slot,
                queue_id: queue.queue_id,
                queue_generation: plan.queue_generation,
                last_request_index: last.ticket.request_index,
                fence_occurrence: 1_000 + u64::from(queue_slot),
                signal_slot: u16::from(queue_slot),
                signal_generation: 2_000 + u64::from(queue_slot),
                contract: R46TailFenceContractV1::contracted_model_only(),
            })
        })
        .collect()
}

fn bind(striped_count: u8, request_count: u16, deadline_ns: u64) -> R46Gfx942StripedSdmaTailWaitV1 {
    let (submission, presentation) = r40_submission(striped_count, request_count, deadline_ns);
    let tails = tails_for(&submission);
    match R46Gfx942StripedSdmaTailWaitV1::bind_model_only(submission, presentation, tails) {
        R46TailWaitBindOutcomeV1::Bound(owner) => owner,
        other => panic!("unexpected bind outcome: {other:?}"),
    }
}

fn currentness(owner: &R46Gfx942StripedSdmaTailWaitV1) -> R46TailCurrentnessV1 {
    let plan = owner.submission_model_only().plan_model_only();
    R46TailCurrentnessV1 {
        session_id: plan.queue_plan.session_id,
        submission_id: plan.submission_id,
        queue_generation: plan.queue_generation,
        scope_closed: true,
    }
}

#[test]
fn binding_builds_exact_active_shards_order_partitions_and_tails() {
    for (striped_count, request_count, expected_active) in [(4, 2, 2), (4, 17, 4), (14, 882, 14)] {
        let owner = bind(striped_count, request_count, 500);
        assert!(owner.is_exact_model_only());
        assert_eq!(owner.active_shards_model_only().len(), expected_active);
        assert_eq!(owner.post_bind_allocation_count_model_only(), 0);
        for (slot, shard) in owner.active_shards_model_only().iter().enumerate() {
            assert_eq!(usize::from(shard.queue_slot), slot);
            assert_eq!(shard.engine_index, shard.queue_slot % 2);
            assert!(shard.tail.contract.is_exact_model_only());
            assert_eq!(shard.tail.queue_slot, shard.queue_slot);
            assert_eq!(shard.tail.queue_id, shard.queue_id);
            assert_eq!(shard.tail.queue_generation, shard.queue_generation);
            assert_eq!(
                shard.request_indices.last().copied(),
                Some(shard.tail.last_request_index)
            );
            assert!(
                shard
                    .request_indices
                    .windows(2)
                    .all(|pair| pair[1] - pair[0] == u16::from(striped_count))
            );
        }
    }
}

#[test]
fn binding_rejects_missing_duplicate_substituted_and_unauthenticated_tails() {
    let mutations: [fn(&mut Vec<R46AuthenticatedTailFenceV1>); 10] = [
        |tails| {
            tails.pop();
        },
        |tails| tails.push(tails[0]),
        |tails| tails[0].submission_id += 1,
        |tails| tails[0].engine_index ^= 1,
        |tails| tails[0].queue_slot += 1,
        |tails| tails[0].queue_id += 1,
        |tails| tails[0].queue_generation += 1,
        |tails| tails[0].last_request_index += 1,
        |tails| tails[0].fence_occurrence = 0,
        |tails| tails[0].signal_generation = 0,
    ];
    for mutate in mutations {
        let (submission, presentation) = r40_submission(4, 17, 500);
        let mut tails = tails_for(&submission);
        mutate(&mut tails);
        let expected_tails = tails.clone();
        let (reason, unbound) = match R46Gfx942StripedSdmaTailWaitV1::bind_model_only(
            submission,
            presentation,
            tails,
        ) {
            R46TailWaitBindOutcomeV1::Rejected { reason, unbound } => (reason, unbound),
            other => panic!("unexpected hostile bind outcome: {other:?}"),
        };
        assert!(matches!(
            reason,
            R46TailWaitBindErrorV1::TailCountMismatch
                | R46TailWaitBindErrorV1::TailSubstitution { .. }
        ));
        assert_eq!(unbound.tails_model_only(), expected_tails);
        assert_eq!(
            unbound.submission_model_only().shards_model_only().len(),
            17
        );
    }

    let (submission, presentation) = r40_submission(4, 17, 500);
    let mut tails = tails_for(&submission);
    tails[1].fence_occurrence = tails[0].fence_occurrence;
    assert!(matches!(
        R46Gfx942StripedSdmaTailWaitV1::bind_model_only(submission, presentation, tails),
        R46TailWaitBindOutcomeV1::Rejected {
            reason: R46TailWaitBindErrorV1::TailFenceOccurrenceNotUnique { .. },
            ..
        }
    ));

    let (submission, presentation) = r40_submission(4, 17, 500);
    let mut tails = tails_for(&submission);
    tails[1].signal_slot = tails[0].signal_slot;
    tails[1].signal_generation = tails[0].signal_generation;
    assert!(matches!(
        R46Gfx942StripedSdmaTailWaitV1::bind_model_only(submission, presentation, tails),
        R46TailWaitBindOutcomeV1::Rejected {
            reason: R46TailWaitBindErrorV1::TailSignalNotUnique { .. },
            ..
        }
    ));

    let contract_mutations: [fn(&mut R46TailFenceContractV1); 2] = [
        |contract: &mut R46TailFenceContractV1| {
            contract.completion_implies_preceding_visible = false;
        },
        |contract: &mut R46TailFenceContractV1| {
            contract.signal_read_is_bound_to_named_fence = false;
        },
    ];
    for mutate in contract_mutations {
        let (submission, presentation) = r40_submission(4, 17, 500);
        let mut tails = tails_for(&submission);
        mutate(&mut tails[2].contract);
        assert!(matches!(
            R46Gfx942StripedSdmaTailWaitV1::bind_model_only(submission, presentation, tails),
            R46TailWaitBindOutcomeV1::Rejected {
                reason: R46TailWaitBindErrorV1::TailContractRejected { .. },
                ..
            }
        ));
    }
}

#[test]
fn pending_rounds_observe_only_tails_and_retain_exact_whole_custody() {
    let owner = bind(4, 17, 500);
    let expected_roster = owner
        .submission_model_only()
        .completion_validation_roster_model_only()
        .to_vec();
    let current = currentness(&owner);
    let tails = r46_tail_observations_model_only(&owner, R46TailObservationStateV1::Pending);
    let owner = match owner.wait_round_model_only(current, &tails, &[], 200) {
        R46TailWaitOutcomeV1::Pending(waiting) => waiting.into_owner_model_only(),
        other => panic!("unexpected first wait: {other:?}"),
    };
    assert_eq!(owner.work_model_only().tail_rounds, 1);
    assert_eq!(owner.work_model_only().tail_observations, 4);
    assert_eq!(owner.work_model_only().final_audit_observations, 0);
    assert_eq!(
        owner
            .submission_model_only()
            .completion_validation_roster_model_only(),
        expected_roster
    );
    assert!(
        owner
            .submission_model_only()
            .prepared_completed_output_is_empty_model_only()
    );
    assert_eq!(owner.post_bind_allocation_count_model_only(), 0);

    let current = currentness(&owner);
    let tails = r46_tail_observations_model_only(&owner, R46TailObservationStateV1::Pending);
    match owner.wait_round_model_only(current, &tails, &[], 300) {
        R46TailWaitOutcomeV1::Pending(waiting) => {
            let work = waiting.owner_model_only().work_model_only();
            assert_eq!(work.tail_rounds, 2);
            assert_eq!(work.tail_observations, 8);
            assert!(work.exact_complexity_model_only());
        }
        other => panic!("unexpected second wait: {other:?}"),
    }
}

#[test]
fn all_ready_tails_trigger_one_full_audit_and_ordered_r40_retirement() {
    for now_ns in [300, 500, u64::MAX] {
        let owner = bind(4, 17, 500);
        let current = currentness(&owner);
        let tails = r46_tail_observations_model_only(&owner, R46TailObservationStateV1::Ready);
        let audit = r46_final_audit_model_only(&owner, R40AggregateObservationStateV1::Ready);
        match owner.wait_round_model_only(current, &tails, &audit, now_ns) {
            R46TailWaitOutcomeV1::Completed(completed) => {
                assert!(completed.is_exact_model_only());
                assert_eq!(completed.work.tail_rounds, 1);
                assert_eq!(completed.work.tail_observations, 4);
                assert_eq!(completed.work.final_audit_observations, 17);
                assert_eq!(completed.work.total_observations_model_only(), Some(21));
                assert_eq!(completed.post_bind_allocation_count, 0);
            }
            other => panic!("unexpected completion: {other:?}"),
        }
    }
}

#[test]
fn deadline_timeout_requires_a_full_audit_with_pending_work() {
    let owner = bind(4, 17, 500);
    let current = currentness(&owner);
    let tails = r46_tail_observations_model_only(&owner, R46TailObservationStateV1::Pending);
    let mut audit = r46_final_audit_model_only(&owner, R40AggregateObservationStateV1::Ready);
    audit[6].observation.state = R40AggregateObservationStateV1::Pending;
    match owner.wait_round_model_only(current, &tails, &audit, 500) {
        R46TailWaitOutcomeV1::TimedOut(timeout) => {
            let owner = timeout.owner_model_only();
            assert_eq!(owner.work_model_only().tail_observations, 4);
            assert_eq!(owner.work_model_only().final_audit_observations, 17);
            assert!(
                owner
                    .submission_model_only()
                    .prepared_completed_output_is_empty_model_only()
            );
            assert_eq!(owner.post_bind_allocation_count_model_only(), 0);
            assert!(timeout.is_exact_terminal_custody_model_only());
        }
        other => panic!("unexpected timeout: {other:?}"),
    }
}

#[test]
fn ready_tail_with_pending_prefix_is_a_terminal_contract_violation() {
    for pending_index in [0, 8, 16] {
        let owner = bind(4, 17, 500);
        let current = currentness(&owner);
        let tails = r46_tail_observations_model_only(&owner, R46TailObservationStateV1::Ready);
        let mut audit = r46_final_audit_model_only(&owner, R40AggregateObservationStateV1::Ready);
        audit[pending_index].observation.state = R40AggregateObservationStateV1::Pending;
        match owner.wait_round_model_only(current, &tails, &audit, 300) {
            R46TailWaitOutcomeV1::Terminal(terminal) => {
                assert_eq!(
                    terminal.reason,
                    R46TailWaitTerminalReasonV1::TailReadyWithPendingPrefix {
                        request_index: pending_index as u16,
                    }
                );
                assert_eq!(terminal.work.final_audit_observations, 17);
                assert_eq!(
                    terminal
                        .owner_model_only()
                        .submission_model_only()
                        .shards_model_only()
                        .len(),
                    17
                );
            }
            other => panic!("unexpected contract violation: {other:?}"),
        }
    }
}

#[test]
fn tail_and_final_audit_errors_are_terminal_with_whole_custody() {
    let owner = bind(4, 17, 500);
    let current = currentness(&owner);
    let mut tails = r46_tail_observations_model_only(&owner, R46TailObservationStateV1::Pending);
    tails[1].state = R46TailObservationStateV1::Error { terminal_token: 71 };
    match owner.wait_round_model_only(current, &tails, &[], 300) {
        R46TailWaitOutcomeV1::Terminal(terminal) => {
            assert_eq!(
                terminal.reason,
                R46TailWaitTerminalReasonV1::TailObservationError {
                    active_shard_index: 1,
                    terminal_token: 71,
                }
            );
            assert_eq!(terminal.work.tail_observations, 4);
            assert_eq!(terminal.work.final_audit_observations, 0);
        }
        other => panic!("unexpected tail error: {other:?}"),
    }

    let owner = bind(4, 17, 500);
    let current = currentness(&owner);
    let tails = r46_tail_observations_model_only(&owner, R46TailObservationStateV1::Ready);
    let mut audit = r46_final_audit_model_only(&owner, R40AggregateObservationStateV1::Pending);
    audit[2].observation.state = R40AggregateObservationStateV1::Error { terminal_token: 73 };
    match owner.wait_round_model_only(current, &tails, &audit, 300) {
        R46TailWaitOutcomeV1::Terminal(terminal) => {
            assert_eq!(
                terminal.reason,
                R46TailWaitTerminalReasonV1::FinalAuditError {
                    request_index: 2,
                    terminal_token: 73,
                }
            );
            assert_eq!(terminal.work.final_audit_observations, 17);
            assert_eq!(
                terminal
                    .owner_model_only()
                    .submission_model_only()
                    .shards_model_only()
                    .len(),
                17
            );
        }
        other => panic!("unexpected audit error: {other:?}"),
    }
}

#[test]
fn currentness_and_tail_roster_failures_are_terminal_before_observation() {
    let current_mutations: [fn(&mut R46TailCurrentnessV1); 4] = [
        |value| value.session_id += 1,
        |value| value.submission_id += 1,
        |value| value.queue_generation += 1,
        |value| value.scope_closed = false,
    ];
    for mutate in current_mutations {
        let owner = bind(4, 17, 500);
        let mut current = currentness(&owner);
        mutate(&mut current);
        let tails = r46_tail_observations_model_only(&owner, R46TailObservationStateV1::Ready);
        match owner.wait_round_model_only(current, &tails, &[], 300) {
            R46TailWaitOutcomeV1::Terminal(terminal) => {
                assert_eq!(
                    terminal.reason,
                    R46TailWaitTerminalReasonV1::CurrentnessFailure
                );
                assert_eq!(terminal.work.tail_observations, 0);
            }
            other => panic!("unexpected currentness result: {other:?}"),
        }
    }

    let roster_mutations: [fn(&mut Vec<R46TailObservationV1>); 5] = [
        |items| {
            items.pop();
        },
        |items| items.push(items[0]),
        |items| items[0].tail.queue_slot += 1,
        |items| items[0].tail.queue_id += 1,
        |items| items[0].tail.queue_generation += 1,
    ];
    for mutate in roster_mutations {
        let owner = bind(4, 17, 500);
        let current = currentness(&owner);
        let mut tails = r46_tail_observations_model_only(&owner, R46TailObservationStateV1::Ready);
        mutate(&mut tails);
        match owner.wait_round_model_only(current, &tails, &[], 300) {
            R46TailWaitOutcomeV1::Terminal(terminal) => {
                assert_eq!(
                    terminal.reason,
                    R46TailWaitTerminalReasonV1::TailObservationRosterMismatch
                );
                assert_eq!(terminal.work.tail_observations, 0);
            }
            other => panic!("unexpected tail roster result: {other:?}"),
        }
    }
}

#[test]
fn late_identity_mismatch_rejects_before_committing_abstract_observations() {
    let owner = bind(4, 17, 500);
    let current = currentness(&owner);
    let mut tails = r46_tail_observations_model_only(&owner, R46TailObservationStateV1::Ready);
    tails.last_mut().unwrap().tail.queue_id += 1;
    match owner.wait_round_model_only(current, &tails, &[], 300) {
        R46TailWaitOutcomeV1::Terminal(terminal) => {
            assert_eq!(
                terminal.reason,
                R46TailWaitTerminalReasonV1::TailObservationRosterMismatch
            );
            assert_eq!(terminal.work.tail_observations, 0);
            assert_eq!(terminal.work.final_audit_observations, 0);
            assert!(terminal.is_exact_terminal_custody_model_only());
        }
        other => panic!("unexpected late tail substitution: {other:?}"),
    }

    let owner = bind(4, 17, 500);
    let current = currentness(&owner);
    let tails = r46_tail_observations_model_only(&owner, R46TailObservationStateV1::Ready);
    let mut audit = r46_final_audit_model_only(&owner, R40AggregateObservationStateV1::Ready);
    audit
        .last_mut()
        .unwrap()
        .observation
        .ticket
        .queue_generation += 1;
    match owner.wait_round_model_only(current, &tails, &audit, 300) {
        R46TailWaitOutcomeV1::Terminal(terminal) => {
            assert_eq!(
                terminal.reason,
                R46TailWaitTerminalReasonV1::FinalAuditRosterMismatch
            );
            assert_eq!(terminal.work.tail_observations, 4);
            assert_eq!(terminal.work.final_audit_observations, 0);
            assert!(terminal.is_exact_terminal_custody_model_only());
        }
        other => panic!("unexpected late final-audit substitution: {other:?}"),
    }
}

#[test]
fn final_audit_substitution_and_preflight_failure_retire_nothing() {
    let audit_mutations: [fn(&mut Vec<R46FinalAuditEntryV1>); 4] = [
        |items| {
            items.pop();
        },
        |items| items[0].observation.ticket.queue_slot += 1,
        |items| items[0].observation.ticket.queue_id += 1,
        |items| items[0].observation.ticket.queue_generation += 1,
    ];
    for mutate in audit_mutations {
        let owner = bind(4, 17, 500);
        let current = currentness(&owner);
        let tails = r46_tail_observations_model_only(&owner, R46TailObservationStateV1::Ready);
        let mut audit = r46_final_audit_model_only(&owner, R40AggregateObservationStateV1::Ready);
        mutate(&mut audit);
        match owner.wait_round_model_only(current, &tails, &audit, 300) {
            R46TailWaitOutcomeV1::Terminal(terminal) => {
                assert_eq!(
                    terminal.reason,
                    R46TailWaitTerminalReasonV1::FinalAuditRosterMismatch
                );
                assert_eq!(terminal.work.final_audit_observations, 0);
                assert!(
                    terminal
                        .owner_model_only()
                        .submission_model_only()
                        .prepared_completed_output_is_empty_model_only()
                );
            }
            other => panic!("unexpected audit substitution: {other:?}"),
        }
    }

    for failed_index in [0, 8, 16] {
        let owner = bind(4, 17, 500);
        let current = currentness(&owner);
        let tails = r46_tail_observations_model_only(&owner, R46TailObservationStateV1::Ready);
        let audit = r46_final_audit_model_only(&owner, R40AggregateObservationStateV1::Ready);
        let mut audit = audit;
        audit[failed_index].retirement_ready = false;
        match owner.wait_round_model_only(current, &tails, &audit, 300) {
            R46TailWaitOutcomeV1::Terminal(terminal) => {
                assert_eq!(
                    terminal.reason,
                    R46TailWaitTerminalReasonV1::RetirementPreflightFailed {
                        request_index: failed_index as u16,
                    }
                );
                assert_eq!(terminal.work.final_audit_observations, 17);
                assert!(
                    terminal
                        .owner_model_only()
                        .submission_model_only()
                        .prepared_completed_output_is_empty_model_only()
                );
            }
            other => panic!("unexpected preflight result: {other:?}"),
        }
    }
}

#[test]
fn scale_accounting_is_rounds_times_active_shards_plus_one_final_audit() {
    let mut owner = bind(14, 882, 10_000);
    for now_ns in 200..264 {
        let current = currentness(&owner);
        let tails = r46_tail_observations_model_only(&owner, R46TailObservationStateV1::Pending);
        owner = match owner.wait_round_model_only(current, &tails, &[], now_ns) {
            R46TailWaitOutcomeV1::Pending(waiting) => waiting.into_owner_model_only(),
            other => panic!("unexpected scale pending result: {other:?}"),
        };
    }
    let current = currentness(&owner);
    let tails = r46_tail_observations_model_only(&owner, R46TailObservationStateV1::Ready);
    let audit = r46_final_audit_model_only(&owner, R40AggregateObservationStateV1::Ready);
    match owner.wait_round_model_only(current, &tails, &audit, 300) {
        R46TailWaitOutcomeV1::Completed(completed) => {
            assert_eq!(completed.work.tail_rounds, 65);
            assert_eq!(completed.work.active_shards, 14);
            assert_eq!(completed.work.tail_observations, 65 * 14);
            assert_eq!(completed.work.final_audit_observations, 882);
            assert_eq!(completed.work.total_observations_model_only(), Some(1792));
            assert!(completed.work.total_observations_model_only().unwrap() < 65 * 882);
            assert!(completed.is_exact_model_only());
        }
        other => panic!("unexpected scale completion: {other:?}"),
    }
}
