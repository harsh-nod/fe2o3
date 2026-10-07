use alloc::vec::Vec;
use core::num::NonZeroU64;

use super::r48_retryable_striped_sdma_tail_wait::*;

const OWNER: u64 = 41;
const SESSION: u64 = 43;
const QUEUE_GENERATION: u64 = 47;

fn requests_for(
    epoch: u64,
    queue_count: usize,
    request_count: usize,
    first_queue: usize,
) -> Vec<R48PublishedRequestV1> {
    (0..request_count)
        .map(|index| {
            let queue_ordinal = (first_queue + index) % queue_count;
            let signal = R48SignalIdentityV1 {
                mapping_identity: 10_000 + index as u64,
                slot: index as u16,
                generation: index as u32 + 1,
            };
            let ticket = R48StripedTicketV1 {
                owner_occurrence: OWNER,
                session_occurrence: SESSION,
                submission_epoch: epoch,
                request_index: index as u16,
                queue_ordinal: queue_ordinal as u8,
                queue_id: 100 + queue_ordinal as u32,
                engine: R48PhysicalEngineLabelV1::for_queue_ordinal_model_only(queue_ordinal),
                queue_generation: QUEUE_GENERATION,
                ring_slot: ((R48_RING_SLOT_COUNT_V1 - 1 + index / queue_count)
                    % R48_RING_SLOT_COUNT_V1) as u16,
                signal,
            };
            R48PublishedRequestV1 {
                ticket,
                publication: R48PacketPublicationEvidenceV1 {
                    ticket,
                    packet_occurrence: index as u64 + 1,
                    fence_header: R48_SYSTEM_SNOOP_FENCE_HEADER_V1,
                    packet_signal: signal,
                    complete_packet_written: true,
                    complete_packet_before_write_pointer_release: true,
                    record_retained_before_write_pointer_release: true,
                    write_pointer_published_release: true,
                    doorbell_published_release: true,
                    write_pointer_release_before_doorbell_release: true,
                    admitted_gfx942_engine: true,
                    system_scope: true,
                    snoop: true,
                    signal_read_names_fence: true,
                    completion_implies_preceding_visible: true,
                },
                payload_identity: 20_000 + index as u64,
            }
        })
        .collect()
}

fn submission(
    profile: R48StripedQueueProfileV1,
    queue_count: usize,
    request_count: usize,
    first_queue: usize,
) -> R48PublishedStripedSubmissionV1 {
    let mut issuer = R48SubmissionEpochOwnerV1::new_model_only(OWNER, SESSION).unwrap();
    let occurrence = issuer.mint_model_only().unwrap();
    let requests = requests_for(
        occurrence.epoch_model_only(),
        queue_count,
        request_count,
        first_queue,
    );
    R48PublishedStripedSubmissionV1::seal_model_only(
        occurrence,
        profile,
        QUEUE_GENERATION,
        first_queue,
        (0..queue_count).map(|index| 100 + index as u32).collect(),
        requests,
    )
    .unwrap()
}

fn wait_owner(
    queue_count: usize,
    request_count: usize,
    first_queue: usize,
) -> R48RetryableStripedTailWaitV1 {
    R48RetryableStripedTailWaitV1::bind_model_only(submission(
        R48StripedQueueProfileV1::Standalone,
        queue_count,
        request_count,
        first_queue,
    ))
}

fn currentness(owner: &R48RetryableStripedTailWaitV1) -> R48CurrentnessEnvelopeV1 {
    let plan = owner.submission_model_only().plan_model_only();
    R48CurrentnessEnvelopeV1 {
        owner_occurrence: plan.owner_occurrence,
        session_occurrence: plan.session_occurrence,
        submission_epoch: plan.submission_epoch,
        queue_generation: plan.queue_generation,
        wait_epoch: owner.work_model_only().wait_epoch,
        opening_current: true,
        closing_current: true,
    }
}

fn tails(
    owner: &R48RetryableStripedTailWaitV1,
    state: R48CompletionStateV1,
) -> Vec<R48TailObservationV1> {
    owner
        .submission_model_only()
        .tails_model_only()
        .iter()
        .copied()
        .map(|tail| R48TailObservationV1 { tail, state })
        .collect()
}

fn audit(
    owner: &R48RetryableStripedTailWaitV1,
    state: R48CompletionStateV1,
) -> Vec<R48AuditObservationV1> {
    owner
        .submission_model_only()
        .requests_model_only()
        .iter()
        .map(|request| R48AuditObservationV1 {
            ticket: request.ticket,
            state,
            retirement_ready: true,
        })
        .collect()
}

#[test]
fn queue_profiles_cover_exact_even_combined_and_standalone_domains() {
    for count in 2..=R48_MAX_STRIPED_QUEUES_V1 {
        assert_eq!(
            R48StripedQueueProfileV1::Combined.admits_queue_count_model_only(count),
            count.is_multiple_of(2) && count <= R48_MAX_COMBINED_STRIPED_QUEUES_V1
        );
        assert_eq!(
            R48StripedQueueProfileV1::Standalone.admits_queue_count_model_only(count),
            count.is_multiple_of(2)
        );
    }
    assert!(!R48StripedQueueProfileV1::Combined.admits_queue_count_model_only(16));
    assert!(submission(R48StripedQueueProfileV1::Combined, 14, 882, 13).is_exact_model_only());
    assert!(submission(R48StripedQueueProfileV1::Standalone, 16, 1008, 15).is_exact_model_only());
}

#[test]
fn rotating_cursor_normalizes_tails_but_preserves_physical_engine_labels() {
    let submission = submission(R48StripedQueueProfileV1::Standalone, 8, 10, 5);
    let plan = submission.plan_model_only();
    let expected_queues = [5, 6, 7, 0, 1, 2, 3, 4, 5, 6];
    for (index, expected) in expected_queues.into_iter().enumerate() {
        assert_eq!(plan.queue_for_request_model_only(index), Some(expected));
        assert_eq!(plan.normalized_slot_model_only(expected), Some(index % 8));
        assert_eq!(
            submission.requests_model_only()[index].ticket.engine,
            R48PhysicalEngineLabelV1::for_queue_ordinal_model_only(expected)
        );
    }
    let expected_last = [8, 9, 2, 3, 4, 5, 6, 7];
    for (slot, tail) in submission.tails_model_only().iter().enumerate() {
        assert_eq!(tail.normalized_queue_slot as usize, slot);
        assert_eq!(tail.ticket.request_index as usize, expected_last[slot]);
    }
}

#[test]
fn submission_epochs_are_nonzero_monotonic_burned_and_authenticated() {
    let mut issuer = R48SubmissionEpochOwnerV1::new_model_only(OWNER, SESSION).unwrap();
    let first = issuer.mint_model_only().unwrap();
    assert_eq!(first.epoch_model_only(), 1);
    let forged = first.substitute_epoch_model_only(7);
    let forged_requests = requests_for(7, 2, 2, 0);
    assert!(
        R48PublishedStripedSubmissionV1::seal_model_only(
            forged,
            R48StripedQueueProfileV1::Standalone,
            QUEUE_GENERATION,
            0,
            Vec::from([100, 101]),
            forged_requests,
        )
        .is_err()
    );
    let second = issuer.mint_model_only().unwrap();
    assert_eq!(second.epoch_model_only(), 2);
    assert_eq!(issuer.next_epoch_model_only(), 3);
}

#[test]
fn seal_rejects_each_false_packet_publication_premise_without_losing_roster() {
    let mutations: [fn(&mut R48PublishedRequestV1); 13] = [
        |request| request.publication.fence_header ^= 1,
        |request| request.publication.packet_signal.generation += 1,
        |request| request.publication.complete_packet_written = false,
        |request| {
            request
                .publication
                .complete_packet_before_write_pointer_release = false
        },
        |request| {
            request
                .publication
                .record_retained_before_write_pointer_release = false
        },
        |request| request.publication.write_pointer_published_release = false,
        |request| request.publication.doorbell_published_release = false,
        |request| {
            request
                .publication
                .write_pointer_release_before_doorbell_release = false
        },
        |request| request.publication.admitted_gfx942_engine = false,
        |request| request.publication.system_scope = false,
        |request| request.publication.snoop = false,
        |request| request.publication.signal_read_names_fence = false,
        |request| request.publication.completion_implies_preceding_visible = false,
    ];
    for mutate in mutations {
        let mut issuer = R48SubmissionEpochOwnerV1::new_model_only(OWNER, SESSION).unwrap();
        let occurrence = issuer.mint_model_only().unwrap();
        let mut requests = requests_for(1, 4, 9, 3);
        mutate(&mut requests[4]);
        let expected = requests;
        let (_, returned) = R48PublishedStripedSubmissionV1::seal_model_only(
            occurrence,
            R48StripedQueueProfileV1::Standalone,
            QUEUE_GENERATION,
            3,
            Vec::from([100, 101, 102, 103]),
            expected.clone(),
        )
        .unwrap_err();
        assert_eq!(returned, expected);
    }
}

#[test]
fn seal_rejects_cursor_engine_signal_and_packet_occurrence_substitution() {
    for mutation in 0..4 {
        let mut issuer = R48SubmissionEpochOwnerV1::new_model_only(OWNER, SESSION).unwrap();
        let occurrence = issuer.mint_model_only().unwrap();
        let mut requests = requests_for(1, 4, 8, 3);
        match mutation {
            0 => requests[0].ticket.queue_ordinal = 0,
            1 => requests[0].ticket.engine = R48PhysicalEngineLabelV1::Engine0,
            2 => requests[1].ticket.signal = requests[0].ticket.signal,
            _ => {
                requests[4].publication.packet_occurrence =
                    requests[0].publication.packet_occurrence
            }
        }
        assert!(
            R48PublishedStripedSubmissionV1::seal_model_only(
                occurrence,
                R48StripedQueueProfileV1::Standalone,
                QUEUE_GENERATION,
                3,
                Vec::from([100, 101, 102, 103]),
                requests,
            )
            .is_err()
        );
    }
}

#[test]
fn ring_slots_cover_zero_through_sixty_three_and_reject_same_queue_aliases() {
    let valid = submission(R48StripedQueueProfileV1::Standalone, 2, 126, 1);
    assert!(
        valid
            .requests_model_only()
            .iter()
            .any(|request| usize::from(request.ticket.ring_slot) == 63)
    );

    let mut issuer = R48SubmissionEpochOwnerV1::new_model_only(OWNER, SESSION).unwrap();
    let occurrence = issuer.mint_model_only().unwrap();
    let mut requests = requests_for(1, 4, 9, 3);
    requests[4].ticket.ring_slot = requests[0].ticket.ring_slot;
    requests[4].publication.ticket.ring_slot = requests[4].ticket.ring_slot;
    assert!(
        R48PublishedStripedSubmissionV1::seal_model_only(
            occurrence,
            R48StripedQueueProfileV1::Standalone,
            QUEUE_GENERATION,
            3,
            Vec::from([100, 101, 102, 103]),
            requests,
        )
        .is_err()
    );
}

#[test]
fn public_plan_queries_are_total_for_malformed_public_values() {
    let empty = R48StripedPlanV1 {
        profile: R48StripedQueueProfileV1::Standalone,
        owner_occurrence: 1,
        session_occurrence: 1,
        submission_epoch: 1,
        queue_generation: 1,
        first_queue: 0,
        queue_ids: Vec::new(),
        request_count: 1,
    };
    assert_eq!(empty.queue_for_request_model_only(0), None);
    assert_eq!(empty.normalized_slot_model_only(0), None);

    let invalid_cursor = R48StripedPlanV1 {
        queue_ids: Vec::from([1, 2]),
        first_queue: u8::MAX,
        ..empty
    };
    assert_eq!(invalid_cursor.queue_for_request_model_only(0), None);
    assert_eq!(invalid_cursor.normalized_slot_model_only(0), None);
}

#[test]
fn completed_tail_rounds_have_exact_shard_work_and_no_audit() {
    let owner = wait_owner(6, 19, 4);
    let current = currentness(&owner);
    let pending = tails(&owner, R48CompletionStateV1::Pending);
    let owner = match owner.wait_round_model_only(
        current,
        &pending,
        &[],
        false,
        R48ModelRetakeV1::Succeeds,
    ) {
        R48TailWaitOutcomeV1::Pending(waiting) => waiting.into_owner_model_only(),
        other => panic!("unexpected first pending result: {other:?}"),
    };
    let current = currentness(&owner);
    let pending = tails(&owner, R48CompletionStateV1::Pending);
    match owner.wait_round_model_only(current, &pending, &[], false, R48ModelRetakeV1::Succeeds) {
        R48TailWaitOutcomeV1::Pending(waiting) => {
            let work = waiting.into_owner_model_only().work_model_only();
            assert_eq!(work.completed_tail_rounds, 2);
            assert_eq!(work.tail_load_attempts, 12);
            assert_eq!(work.audit_load_attempts, 0);
            assert_eq!(work.retired_count, 0);
        }
        other => panic!("unexpected second pending result: {other:?}"),
    }
}

#[test]
fn tail_error_reports_the_exact_early_prefix_after_completed_rounds() {
    let owner = wait_owner(4, 9, 1);
    let current = currentness(&owner);
    let pending = tails(&owner, R48CompletionStateV1::Pending);
    let owner = match owner.wait_round_model_only(
        current,
        &pending,
        &[],
        false,
        R48ModelRetakeV1::Succeeds,
    ) {
        R48TailWaitOutcomeV1::Pending(waiting) => waiting.into_owner_model_only(),
        other => panic!("unexpected pending result: {other:?}"),
    };
    let current = currentness(&owner);
    let mut failing = tails(&owner, R48CompletionStateV1::Ready);
    failing[2].state = R48CompletionStateV1::Error { terminal_token: 71 };
    match owner.wait_round_model_only(current, &failing, &[], false, R48ModelRetakeV1::Succeeds) {
        R48TailWaitOutcomeV1::Terminal(terminal) => {
            assert_eq!(
                terminal.reason,
                R48TailWaitErrorV1::TailObservation {
                    attempted: 3,
                    terminal_token: 71,
                }
            );
            assert_eq!(terminal.work.completed_tail_rounds, 1);
            assert_eq!(terminal.work.tail_load_attempts, 4 + 3);
            assert!(terminal.has_zero_partial_retirement_model_only());
        }
        other => panic!("unexpected tail error: {other:?}"),
    }
}

#[test]
fn final_audit_is_one_ordered_n_scan_with_exact_early_error_prefix() {
    let owner = wait_owner(8, 23, 7);
    let current = currentness(&owner);
    let ready = tails(&owner, R48CompletionStateV1::Ready);
    let mut failing = audit(&owner, R48CompletionStateV1::Ready);
    failing[6].state = R48CompletionStateV1::Error { terminal_token: 73 };
    match owner.wait_round_model_only(current, &ready, &failing, false, R48ModelRetakeV1::Succeeds)
    {
        R48TailWaitOutcomeV1::Terminal(terminal) => {
            assert_eq!(
                terminal.reason,
                R48TailWaitErrorV1::AuditObservation {
                    attempted: 7,
                    terminal_token: 73,
                }
            );
            assert_eq!(terminal.work.completed_tail_rounds, 1);
            assert_eq!(terminal.work.tail_load_attempts, 8);
            assert_eq!(terminal.work.audit_load_attempts, 7);
            assert!(terminal.has_zero_partial_retirement_model_only());
        }
        other => panic!("unexpected audit error: {other:?}"),
    }
}

#[test]
fn timeout_returns_exact_custody_and_retry_advances_only_wait_epoch() {
    let owner = wait_owner(6, 17, 5);
    let current = currentness(&owner);
    let pending_tails = tails(&owner, R48CompletionStateV1::Pending);
    let pending_audit = audit(&owner, R48CompletionStateV1::Pending);
    let timeout = match owner.wait_round_model_only(
        current,
        &pending_tails,
        &pending_audit,
        true,
        R48ModelRetakeV1::Succeeds,
    ) {
        R48TailWaitOutcomeV1::TimedOut(timeout) => timeout,
        other => panic!("unexpected timeout result: {other:?}"),
    };
    assert!(timeout.is_exact_retryable_custody_model_only());
    let retried = timeout.retry_model_only().unwrap();
    let work = retried.work_model_only();
    assert_eq!(work.wait_epoch, 2);
    assert_eq!(work.completed_tail_rounds, 0);
    assert_eq!(work.tail_load_attempts, 0);
    assert_eq!(work.audit_load_attempts, 0);
    assert_eq!(work.cumulative_tail_load_attempts, 6);
    assert_eq!(work.cumulative_audit_load_attempts, 17);
    assert_eq!(work.retired_count, 0);

    let current = currentness(&retried);
    let pending_tails = tails(&retried, R48CompletionStateV1::Pending);
    let pending_audit = audit(&retried, R48CompletionStateV1::Pending);
    let second = match retried.wait_round_model_only(
        current,
        &pending_tails,
        &pending_audit,
        true,
        R48ModelRetakeV1::Succeeds,
    ) {
        R48TailWaitOutcomeV1::TimedOut(timeout) => timeout,
        other => panic!("unexpected second timeout: {other:?}"),
    };
    let work = second.owner_model_only().work_model_only();
    assert_eq!(work.wait_epoch, 2);
    assert_eq!(work.cumulative_tail_load_attempts, 12);
    assert_eq!(work.cumulative_audit_load_attempts, 34);
}

#[test]
fn ready_tail_with_pending_prefix_is_terminal_without_retirement() {
    let owner = wait_owner(4, 11, 2);
    let current = currentness(&owner);
    let ready = tails(&owner, R48CompletionStateV1::Ready);
    let mut pending = audit(&owner, R48CompletionStateV1::Ready);
    pending[3].state = R48CompletionStateV1::Pending;
    match owner.wait_round_model_only(current, &ready, &pending, false, R48ModelRetakeV1::Succeeds)
    {
        R48TailWaitOutcomeV1::Terminal(terminal) => {
            assert_eq!(
                terminal.reason,
                R48TailWaitErrorV1::TailReadyWithPendingPrefix { request_index: 3 }
            );
            assert_eq!(terminal.work.audit_load_attempts, 11);
            assert!(terminal.has_zero_partial_retirement_model_only());
        }
        other => panic!("unexpected ordering result: {other:?}"),
    }
}

#[test]
fn mixed_tail_readiness_uses_the_pending_requests_physical_queue() {
    let owner = wait_owner(4, 12, 1);
    let current = currentness(&owner);
    let mut mixed_tails = tails(&owner, R48CompletionStateV1::Pending);
    mixed_tails[0].state = R48CompletionStateV1::Ready;
    let ready_queue = mixed_tails[0].tail.ticket.queue_ordinal;
    let mut pending_audit = audit(&owner, R48CompletionStateV1::Ready);
    let offending = pending_audit
        .iter_mut()
        .find(|observation| {
            observation.ticket.queue_ordinal == ready_queue
                && !owner
                    .submission_model_only()
                    .tails_model_only()
                    .iter()
                    .any(|tail| tail.ticket == observation.ticket)
        })
        .unwrap();
    offending.state = R48CompletionStateV1::Pending;
    let offending_index = offending.ticket.request_index;
    match owner.wait_round_model_only(
        current,
        &mixed_tails,
        &pending_audit,
        true,
        R48ModelRetakeV1::Succeeds,
    ) {
        R48TailWaitOutcomeV1::Terminal(terminal) => assert_eq!(
            terminal.reason,
            R48TailWaitErrorV1::TailReadyWithPendingPrefix {
                request_index: offending_index,
            }
        ),
        other => panic!("unexpected mixed-ready ordering result: {other:?}"),
    }
}

#[test]
fn pending_audit_on_a_still_pending_tail_queue_is_retryable() {
    let owner = wait_owner(4, 12, 1);
    let current = currentness(&owner);
    let mut mixed_tails = tails(&owner, R48CompletionStateV1::Ready);
    mixed_tails[1].state = R48CompletionStateV1::Pending;
    let pending_queue = mixed_tails[1].tail.ticket.queue_ordinal;
    let mut pending_audit = audit(&owner, R48CompletionStateV1::Ready);
    for observation in &mut pending_audit {
        if observation.ticket.queue_ordinal == pending_queue {
            observation.state = R48CompletionStateV1::Pending;
        }
    }
    match owner.wait_round_model_only(
        current,
        &mixed_tails,
        &pending_audit,
        true,
        R48ModelRetakeV1::Succeeds,
    ) {
        R48TailWaitOutcomeV1::TimedOut(timeout) => {
            assert!(timeout.is_exact_retryable_custody_model_only())
        }
        other => panic!("unexpected mixed-pending timeout result: {other:?}"),
    }
}

#[test]
fn final_audit_tail_readiness_participates_in_ordering_classification() {
    let owner = wait_owner(4, 12, 1);
    let current = currentness(&owner);
    let pending_tails = tails(&owner, R48CompletionStateV1::Pending);
    let queue = pending_tails[2].tail.ticket.queue_ordinal;
    let tail_ticket = pending_tails[2].tail.ticket;
    let mut pending_audit = audit(&owner, R48CompletionStateV1::Ready);
    let predecessor = pending_audit
        .iter_mut()
        .find(|observation| {
            observation.ticket.queue_ordinal == queue && observation.ticket != tail_ticket
        })
        .unwrap();
    predecessor.state = R48CompletionStateV1::Pending;
    let predecessor_index = predecessor.ticket.request_index;
    match owner.wait_round_model_only(
        current,
        &pending_tails,
        &pending_audit,
        true,
        R48ModelRetakeV1::Succeeds,
    ) {
        R48TailWaitOutcomeV1::Terminal(terminal) => assert_eq!(
            terminal.reason,
            R48TailWaitErrorV1::TailReadyWithPendingPrefix {
                request_index: predecessor_index,
            }
        ),
        other => panic!("unexpected final-tail ordering result: {other:?}"),
    }
}

#[test]
fn opening_and_closing_currentness_fail_terminally_with_exact_work_boundaries() {
    let owner = wait_owner(4, 7, 3);
    let mut stale = currentness(&owner);
    stale.opening_current = false;
    match owner.wait_round_model_only(stale, &[], &[], false, R48ModelRetakeV1::Succeeds) {
        R48TailWaitOutcomeV1::Terminal(terminal) => {
            assert_eq!(terminal.reason, R48TailWaitErrorV1::InvalidCurrentness);
            assert_eq!(terminal.work.tail_load_attempts, 0);
        }
        other => panic!("unexpected opening-currentness result: {other:?}"),
    }

    let owner = wait_owner(4, 7, 3);
    let mut stale = currentness(&owner);
    stale.closing_current = false;
    let pending_tails = tails(&owner, R48CompletionStateV1::Pending);
    let pending_audit = audit(&owner, R48CompletionStateV1::Pending);
    match owner.wait_round_model_only(
        stale,
        &pending_tails,
        &pending_audit,
        true,
        R48ModelRetakeV1::Succeeds,
    ) {
        R48TailWaitOutcomeV1::Terminal(terminal) => {
            assert_eq!(terminal.reason, R48TailWaitErrorV1::ClosingCurrentness);
            assert_eq!(terminal.work.tail_load_attempts, 4);
            assert_eq!(terminal.work.audit_load_attempts, 7);
            assert!(terminal.has_zero_partial_retirement_model_only());
        }
        other => panic!("unexpected closing-currentness result: {other:?}"),
    }
}

#[test]
fn retirement_preflight_is_all_or_zero() {
    let owner = wait_owner(6, 15, 0);
    let current = currentness(&owner);
    let ready = tails(&owner, R48CompletionStateV1::Ready);
    let mut audit = audit(&owner, R48CompletionStateV1::Ready);
    audit[12].retirement_ready = false;
    match owner.wait_round_model_only(current, &ready, &audit, false, R48ModelRetakeV1::Succeeds) {
        R48TailWaitOutcomeV1::Terminal(terminal) => {
            assert_eq!(
                terminal.reason,
                R48TailWaitErrorV1::RetirementPreflight { request_index: 12 }
            );
            assert!(terminal.has_zero_partial_retirement_model_only());
        }
        other => panic!("unexpected preflight result: {other:?}"),
    }
}

#[test]
fn all_ready_retires_exact_n_in_original_order() {
    let owner = wait_owner(8, 21, 6);
    let current = currentness(&owner);
    let ready_tails = tails(&owner, R48CompletionStateV1::Ready);
    let ready_audit = audit(&owner, R48CompletionStateV1::Ready);
    match owner.wait_round_model_only(
        current,
        &ready_tails,
        &ready_audit,
        false,
        R48ModelRetakeV1::Succeeds,
    ) {
        R48TailWaitOutcomeV1::Completed(completed) => {
            assert!(completed.is_exact_model_only());
            assert_eq!(completed.work.completed_tail_rounds, 1);
            assert_eq!(completed.work.tail_load_attempts, 8);
            assert_eq!(completed.work.audit_load_attempts, 21);
            assert_eq!(completed.work.retired_count, 21);
            assert_eq!(completed.completed_payloads[0], 20_000);
            assert_eq!(completed.completed_payloads[20], 20_020);
        }
        other => panic!("unexpected completion result: {other:?}"),
    }
}

#[test]
fn post_retirement_retake_failure_preserves_opaque_terminal_custody() {
    let owner = wait_owner(2, 3, 1);
    let current = currentness(&owner);
    let ready_tails = tails(&owner, R48CompletionStateV1::Ready);
    let ready_audit = audit(&owner, R48CompletionStateV1::Ready);
    match owner.wait_round_model_only(
        current,
        &ready_tails,
        &ready_audit,
        false,
        R48ModelRetakeV1::Fails {
            terminal_token: NonZeroU64::new(79).unwrap(),
        },
    ) {
        R48TailWaitOutcomeV1::CompletedOpaque(opaque) => {
            assert_eq!(opaque.terminal_token, 79);
            assert!(opaque.is_exact_terminal_custody_model_only());
            assert_eq!(opaque.work.retired_count, 3);
        }
        other => panic!("unexpected retake result: {other:?}"),
    }
}

#[test]
fn panic_guard_conserves_whole_roster_and_records_exact_stage_prefix() {
    let cases = [
        (R48PanicStageV1::BeforeObservation, 0, 0, 0),
        (R48PanicStageV1::TailPrefix { attempted: 3 }, 3, 0, 0),
        (R48PanicStageV1::AuditPrefix { attempted: 5 }, 6, 5, 1),
        (R48PanicStageV1::AfterClosingCurrentness, 6, 13, 1),
    ];
    for (stage, tail_loads, audit_loads, rounds) in cases {
        let guarded = wait_owner(6, 13, 5).panic_guard_model_only(stage).unwrap();
        assert!(guarded.is_exact_guard_custody_model_only());
        assert_eq!(guarded.stage, stage);
        let work = guarded.owner_model_only().work_model_only();
        assert_eq!(work.tail_load_attempts, tail_loads);
        assert_eq!(work.audit_load_attempts, audit_loads);
        assert_eq!(work.completed_tail_rounds, rounds);
        assert_eq!(work.retired_count, 0);
    }
}

#[test]
fn invalid_panic_prefix_is_terminal_whole_custody() {
    let terminal = wait_owner(6, 13, 5)
        .panic_guard_model_only(R48PanicStageV1::TailPrefix { attempted: 7 })
        .unwrap_err();
    assert_eq!(terminal.reason, R48TailWaitErrorV1::TailRoster);
    assert!(terminal.has_zero_partial_retirement_model_only());
    assert_eq!(
        terminal
            .owner_model_only()
            .submission_model_only()
            .requests_model_only()
            .len(),
        13
    );
}

#[test]
fn malformed_tail_and_audit_rosters_fail_closed_at_observed_prefix() {
    let owner = wait_owner(4, 9, 0);
    let current = currentness(&owner);
    let mut substituted = tails(&owner, R48CompletionStateV1::Ready);
    substituted[2].tail.ticket.queue_id += 1;
    match owner.wait_round_model_only(
        current,
        &substituted,
        &[],
        false,
        R48ModelRetakeV1::Succeeds,
    ) {
        R48TailWaitOutcomeV1::Terminal(terminal) => {
            assert_eq!(terminal.reason, R48TailWaitErrorV1::TailRoster);
            assert_eq!(terminal.work.tail_load_attempts, 2);
        }
        other => panic!("unexpected tail roster result: {other:?}"),
    }

    let owner = wait_owner(4, 9, 0);
    let current = currentness(&owner);
    let ready_tails = tails(&owner, R48CompletionStateV1::Ready);
    let mut substituted = audit(&owner, R48CompletionStateV1::Ready);
    substituted[4].ticket.signal.generation += 1;
    match owner.wait_round_model_only(
        current,
        &ready_tails,
        &substituted,
        false,
        R48ModelRetakeV1::Succeeds,
    ) {
        R48TailWaitOutcomeV1::Terminal(terminal) => {
            assert_eq!(terminal.reason, R48TailWaitErrorV1::AuditRoster);
            assert_eq!(terminal.work.audit_load_attempts, 4);
            assert!(terminal.has_zero_partial_retirement_model_only());
        }
        other => panic!("unexpected audit roster result: {other:?}"),
    }
}
