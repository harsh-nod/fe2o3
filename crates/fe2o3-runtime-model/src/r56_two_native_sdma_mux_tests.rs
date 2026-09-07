use alloc::vec::Vec;

use super::*;

fn plan(logical_lanes: u8, cursor: u8) -> R56MuxPlanV1 {
    R56MuxPlanV1 {
        label: R56MuxPresentationLabelV1::LogicalLanesOverTwoNativeQueues,
        owner_occurrence: 11,
        session_occurrence: 22,
        submission_epoch: 33,
        logical_lane_count: logical_lanes,
        cursor,
        native_queues: [
            R56NativeQueueIdentityV1 {
                session_occurrence: 22,
                native_ordinal: 0,
                engine_index: 0,
                queue_id: 40,
                queue_generation: 50,
            },
            R56NativeQueueIdentityV1 {
                session_occurrence: 22,
                native_ordinal: 1,
                engine_index: 1,
                queue_id: 41,
                queue_generation: 51,
            },
        ],
    }
}

fn requests(count: usize) -> Vec<R56MuxRequestV1> {
    (0..count)
        .map(|index| {
            R56MuxRequestV1::new_model_only(R56RequestIdentityV1 {
                owner_occurrence: 11,
                session_occurrence: 22,
                submission_epoch: 33,
                request_token: 100 + index as u64,
                payload_identity: 1_000 + index as u64,
            })
        })
        .collect()
}

fn currentness(opening_current: bool) -> R56MuxCurrentnessV1 {
    R56MuxCurrentnessV1 {
        owner_occurrence: 11,
        session_occurrence: 22,
        submission_epoch: 33,
        queue_generations: [50, 51],
        opening_current,
    }
}

fn prepared(lanes: u8, cursor: u8, count: usize) -> R56PreparedMuxBatchV1 {
    r56_prepare_mux_batch_model_only(plan(lanes, cursor), requests(count)).unwrap()
}

fn published(lanes: u8, cursor: u8, count: usize) -> R56PublishedMuxBatchV1 {
    match r56_publish_mux_batch_model_only(
        prepared(lanes, cursor, count),
        currentness(true),
        R56MuxPublicationScriptV1::Complete,
    ) {
        R56MuxPublishOutcomeV1::Published(published) => published,
        other => panic!("expected published, got {other:?}"),
    }
}

#[test]
fn exact_lane_domain_and_all_request_counts_are_admitted() {
    for lanes in [2, 4, 8, 14, 16] {
        for cursor in 0..lanes {
            for count in R56_MIN_REQUESTS_V1..=R56_MAX_REQUESTS_V1 {
                let prepared = prepared(lanes, cursor, count);
                assert!(prepared.is_exact_model_only());
                assert!(prepared.native_load_model_only(0) <= 63);
                assert!(prepared.native_load_model_only(1) <= 63);
            }
        }
    }
    for lanes in [0, 1, 3, 6, 10, 12, 15, 17] {
        assert!(!R56MuxPlanV1::admits_logical_lane_count_model_only(lanes));
    }
}

#[test]
fn canonical_assignment_and_two_stable_native_filters_are_exact() {
    let prepared = prepared(8, 3, 10);
    let lanes = prepared
        .tickets
        .iter()
        .map(|ticket| ticket.logical_lane)
        .collect::<Vec<_>>();
    assert_eq!(lanes, [3, 4, 5, 6, 7, 0, 1, 2, 3, 4]);
    assert_eq!(prepared.native_orders[0].request_indices, [1, 3, 5, 7, 9]);
    assert_eq!(prepared.native_orders[1].request_indices, [0, 2, 4, 6, 8]);
    for lane in 0..8 {
        let lane_indices = prepared
            .tickets
            .iter()
            .filter(|ticket| ticket.logical_lane == lane)
            .map(|ticket| ticket.request_index)
            .collect::<Vec<_>>();
        assert!(lane_indices.windows(2).all(|pair| pair[0] < pair[1]));
    }
}

#[test]
fn every_ticket_carries_exact_request_lane_native_engine_queue_slot_and_generation() {
    let prepared = prepared(14, 13, 126);
    for (index, ticket) in prepared.tickets.iter().enumerate() {
        let lane = ((13 + index) % 14) as u8;
        let native = lane % 2;
        assert_eq!(ticket.request_index, index as u16);
        assert_eq!(ticket.request_token, 100 + index as u64);
        assert_eq!(ticket.logical_lane, lane);
        assert_eq!(ticket.native_ordinal, native);
        assert_eq!(ticket.engine_index, native);
        assert_eq!(ticket.queue_id, 40 + u32::from(native));
        assert_eq!(ticket.ring_slot, (index / 2) as u8);
        assert_eq!(ticket.queue_generation, 50 + u64::from(native));
        assert_eq!(ticket.packet_identity, 33_001 + index as u64);
    }
}

#[test]
fn request_lane_native_engine_packet_queue_slot_and_generation_substitution_fail_closed() {
    type Mutation = fn(&mut R56MuxTicketV1);
    let mutations: [Mutation; 8] = [
        |ticket| ticket.request_token += 1,
        |ticket| ticket.logical_lane ^= 2,
        |ticket| ticket.native_ordinal ^= 1,
        |ticket| ticket.engine_index ^= 1,
        |ticket| ticket.packet_identity += 1,
        |ticket| ticket.queue_id += 1,
        |ticket| ticket.ring_slot += 1,
        |ticket| ticket.queue_generation += 1,
    ];
    for mutation in mutations {
        let mut prepared = prepared(8, 3, 10);
        mutation(&mut prepared.tickets[4]);
        assert!(!prepared.is_exact_model_only());
        let expected = prepared.request_identities_model_only();
        match r56_publish_mux_batch_model_only(
            prepared,
            currentness(true),
            R56MuxPublicationScriptV1::Complete,
        ) {
            R56MuxPublishOutcomeV1::Quarantined(quarantined) => {
                assert_eq!(
                    quarantined.reason,
                    R56QuarantineReasonV1::InvalidPreparedPresentation
                );
                assert_eq!(quarantined.request_identities_model_only(), expected);
                assert!(!quarantined.cursor_committed);
            }
            other => panic!("expected quarantine, got {other:?}"),
        }
    }
}

#[test]
fn missing_duplicate_and_unstable_native_order_are_rejected() {
    let mut missing = prepared(8, 0, 10);
    missing.native_orders[0].request_indices.pop();
    assert!(!missing.is_exact_model_only());

    let mut duplicate = prepared(8, 0, 10);
    let duplicate_index = duplicate.native_orders[0].request_indices[0];
    duplicate.native_orders[0]
        .request_indices
        .push(duplicate_index);
    assert!(!duplicate.is_exact_model_only());

    let mut unstable = prepared(8, 0, 10);
    unstable.native_orders[0].request_indices.swap(0, 1);
    assert!(!unstable.is_exact_model_only());
}

#[test]
fn duplicate_request_identities_and_127_requests_retain_exact_inputs() {
    let mut duplicates = requests(3);
    let duplicate_token = duplicates[0].identity_model_only().request_token;
    let identity = duplicates[1].identity_model_only();
    duplicates[1] = R56MuxRequestV1::new_model_only(R56RequestIdentityV1 {
        request_token: duplicate_token,
        ..identity
    });
    let expected = duplicates
        .iter()
        .map(R56MuxRequestV1::identity_model_only)
        .collect::<Vec<_>>();
    let failure = r56_prepare_mux_batch_model_only(plan(8, 0), duplicates).unwrap_err();
    assert_eq!(
        failure.reason,
        R56PreparationErrorV1::DuplicateRequestToken { request_index: 1 }
    );
    assert_eq!(failure.request_identities_model_only(), expected);

    let too_many = requests(127);
    let expected = too_many
        .iter()
        .map(R56MuxRequestV1::identity_model_only)
        .collect::<Vec<_>>();
    let failure = r56_prepare_mux_batch_model_only(plan(16, 0), too_many).unwrap_err();
    assert_eq!(
        failure.reason,
        R56PreparationErrorV1::RequestCapacityExceeded
    );
    assert_eq!(failure.request_identities_model_only(), expected);
}

#[test]
fn false_physical_striped_two_relabel_is_rejected() {
    let mut false_plan = plan(16, 0);
    false_plan.label = R56MuxPresentationLabelV1::FalsePhysicalStriped2;
    let failure = r56_prepare_mux_batch_model_only(false_plan, requests(16)).unwrap_err();
    assert_eq!(failure.reason, R56PreparationErrorV1::InvalidPlan);
    assert_eq!(
        failure.request_identities_model_only(),
        requests(16)
            .iter()
            .map(R56MuxRequestV1::identity_model_only)
            .collect::<Vec<_>>()
    );
}

#[test]
fn full_active_batches_publish_exactly_two_pointers_doorbells_and_tails() {
    for lanes in [2, 4, 8, 14, 16] {
        for cursor in 0..lanes {
            for count in [2, 3, 63, 126] {
                let published = published(lanes, cursor, count);
                assert!(published.is_exact_model_only());
                assert_eq!(published.write_pointer_publication_count_model_only(), 2);
                assert_eq!(published.doorbell_count_model_only(), 2);
                assert_eq!(published.tail_count_model_only(), 2);
                assert_eq!(published.publications[0].native_ordinal, cursor % 2);
                assert_eq!(
                    published.publications[1].native_ordinal,
                    (cursor % 2 + 1) % 2
                );
                assert!(published.cursor_committed);
                assert_eq!(
                    published.cursor_after,
                    ((usize::from(cursor) + count) % usize::from(lanes)) as u8
                );
            }
        }
    }
}

#[test]
fn zero_and_one_request_batches_reject_before_preparation_and_retain_inputs() {
    let empty = r56_prepare_mux_batch_model_only(plan(16, 7), Vec::new()).unwrap_err();
    assert_eq!(empty.reason, R56PreparationErrorV1::EmptySubmission);
    assert!(empty.request_identities_model_only().is_empty());

    let singleton = requests(1);
    let expected = singleton
        .iter()
        .map(R56MuxRequestV1::identity_model_only)
        .collect::<Vec<_>>();
    let failure = r56_prepare_mux_batch_model_only(plan(16, 7), singleton).unwrap_err();
    assert_eq!(failure.reason, R56PreparationErrorV1::RequestCountTooSmall);
    assert_eq!(failure.request_identities_model_only(), expected);
}

#[test]
fn timeout_retains_exact_published_custody_and_committed_cursor() {
    let published = published(14, 9, 126);
    let expected = published.request_identities_model_only();
    let expected_publications = published.publications.clone();
    let completions = published.tickets_model_only().to_vec();
    let expected_cursor = published.cursor_after;
    match r56_wait_mux_batch_model_only(published, R56MuxWaitObservationV1::Timeout) {
        R56MuxWaitOutcomeV1::Pending(pending) => {
            assert!(pending.is_exact_model_only());
            assert_eq!(pending.request_identities_model_only(), expected);
            assert_eq!(pending.publications, expected_publications);
            assert_eq!(pending.write_pointer_publication_count_model_only(), 2);
            assert_eq!(pending.doorbell_count_model_only(), 2);
            assert_eq!(pending.tail_count_model_only(), 2);
            assert!(pending.cursor_committed);
            assert_eq!(pending.cursor_before, 9);
            assert_eq!(pending.cursor_after, expected_cursor);
            match r56_wait_mux_batch_model_only(
                pending,
                R56MuxWaitObservationV1::Completed(completions),
            ) {
                R56MuxWaitOutcomeV1::Completed(released) => assert_eq!(
                    released
                        .iter()
                        .map(R56MuxRequestV1::identity_model_only)
                        .collect::<Vec<_>>(),
                    expected
                ),
                other => panic!("expected completion after timeout, got {other:?}"),
            }
        }
        other => panic!("expected pending published custody, got {other:?}"),
    }
}

#[test]
fn recoverable_prepublication_failure_is_distinct_exact_and_native_effect_free() {
    let prepared = prepared(14, 9, 126);
    let expected = prepared.request_identities_model_only();
    match r56_publish_mux_batch_model_only(
        prepared,
        currentness(true),
        R56MuxPublicationScriptV1::RecoverableFirstNativeNoEffect,
    ) {
        R56MuxPublishOutcomeV1::Retained(retained) => {
            assert_eq!(
                retained.reason,
                R56RetentionReasonV1::RecoverableNoNativeEffect
            );
            assert_eq!(retained.published_native_prefix, 0);
            assert_eq!(retained.request_identities_model_only(), expected);
            assert_eq!(retained.cursor_before, 9);
            assert!(!retained.cursor_committed);
            assert!(retained.is_exact_retryable_model_only());
            assert_eq!(retained.into_requests_model_only().len(), 126);
        }
        other => panic!("expected retained, got {other:?}"),
    }
}

#[test]
fn opening_currentness_rejects_before_publication_without_commit() {
    let prepared = prepared(16, 11, 64);
    let expected = prepared.request_identities_model_only();
    match r56_publish_mux_batch_model_only(
        prepared,
        currentness(false),
        R56MuxPublicationScriptV1::Complete,
    ) {
        R56MuxPublishOutcomeV1::Quarantined(quarantined) => {
            assert_eq!(
                quarantined.reason,
                R56QuarantineReasonV1::OpeningCurrentnessRejected
            );
            assert!(quarantined.is_exact_progress_model_only());
            assert_eq!(quarantined.published_native_prefix, 0);
            assert!(quarantined.confirmed_publications.is_empty());
            assert!(quarantined.indeterminate_publication.is_none());
            assert_eq!(
                quarantined.untouched_request_indices,
                (0..64).collect::<Vec<_>>()
            );
            assert_eq!(quarantined.cursor_before, 11);
            assert!(!quarantined.cursor_committed);
            assert_eq!(quarantined.request_identities_model_only(), expected);
        }
        other => panic!("expected opening-currentness quarantine, got {other:?}"),
    }
}

#[test]
fn first_native_indeterminate_retains_exact_rotated_and_untouched_shards() {
    let prepared = prepared(16, 11, 65);
    let expected = prepared.request_identities_model_only();
    match r56_publish_mux_batch_model_only(
        prepared,
        currentness(true),
        R56MuxPublicationScriptV1::IndeterminateFirstNative,
    ) {
        R56MuxPublishOutcomeV1::Quarantined(quarantined) => {
            assert_eq!(
                quarantined.reason,
                R56QuarantineReasonV1::IndeterminateFirstNative
            );
            assert!(quarantined.is_exact_progress_model_only());
            assert_eq!(quarantined.published_native_prefix, 0);
            assert!(quarantined.confirmed_publications.is_empty());
            let indeterminate = quarantined
                .indeterminate_publication
                .as_ref()
                .expect("first-native indeterminate publication");
            assert_eq!(indeterminate.native_ordinal, 1);
            assert_eq!(
                indeterminate.request_indices,
                (0..65).step_by(2).collect::<Vec<_>>()
            );
            assert_eq!(
                quarantined.untouched_request_indices,
                (1..65).step_by(2).collect::<Vec<_>>()
            );
            assert_eq!(
                quarantined
                    .indeterminate_tickets_model_only()
                    .iter()
                    .map(|ticket| ticket.request_index)
                    .collect::<Vec<_>>(),
                indeterminate.request_indices
            );
            assert_eq!(
                quarantined
                    .untouched_tickets_model_only()
                    .iter()
                    .map(|ticket| ticket.request_index)
                    .collect::<Vec<_>>(),
                quarantined.untouched_request_indices
            );
            assert_eq!(quarantined.request_identities_model_only(), expected);
            assert_eq!(quarantined.cursor_before, 11);
            assert!(!quarantined.cursor_committed);
        }
        other => panic!("expected first-native indeterminate quarantine, got {other:?}"),
    }
}

#[test]
fn second_native_failures_retain_cursor_rotated_exact_coordinates() {
    let cases = [
        (
            R56MuxPublicationScriptV1::RecoverableSecondNativeAfterConfirmedFirst,
            R56QuarantineReasonV1::RecoverableSecondNativeAfterConfirmedFirst,
            false,
        ),
        (
            R56MuxPublicationScriptV1::IndeterminateSecondNativeAfterConfirmedFirst,
            R56QuarantineReasonV1::IndeterminateSecondNativeAfterConfirmedFirst,
            true,
        ),
    ];
    for (script, reason, indeterminate) in cases {
        let prepared = prepared(16, 11, 64);
        let expected = prepared.request_identities_model_only();
        match r56_publish_mux_batch_model_only(prepared, currentness(true), script) {
            R56MuxPublishOutcomeV1::Quarantined(quarantined) => {
                assert_eq!(quarantined.reason, reason);
                assert!(quarantined.is_exact_progress_model_only());
                assert_eq!(quarantined.published_native_prefix, 1);
                assert_eq!(quarantined.confirmed_publications[0].native_ordinal, 1);
                assert_eq!(
                    quarantined
                        .indeterminate_publication
                        .as_ref()
                        .map(|publication| publication.native_ordinal),
                    indeterminate.then_some(0)
                );
                if indeterminate {
                    assert!(quarantined.untouched_request_indices.is_empty());
                } else {
                    assert_eq!(
                        quarantined.untouched_request_indices,
                        (1..64).step_by(2).collect::<Vec<_>>()
                    );
                }
                assert!(!quarantined.cursor_committed);
                assert_eq!(quarantined.request_identities_model_only(), expected);
            }
            other => panic!("expected second-native quarantine, got {other:?}"),
        }
    }
}

#[test]
fn complete_publication_then_closing_failure_has_prefix_two_without_commit() {
    let prepared = prepared(16, 11, 64);
    match r56_publish_mux_batch_model_only(
        prepared,
        currentness(true),
        R56MuxPublicationScriptV1::CompleteThenClosingCurrentnessFailure,
    ) {
        R56MuxPublishOutcomeV1::Quarantined(quarantined) => {
            assert_eq!(
                quarantined.reason,
                R56QuarantineReasonV1::ClosingCurrentnessFailure
            );
            assert!(quarantined.is_exact_progress_model_only());
            assert_eq!(quarantined.published_native_prefix, 2);
            assert_eq!(
                quarantined
                    .confirmed_publications
                    .iter()
                    .map(|publication| publication.native_ordinal)
                    .collect::<Vec<_>>(),
                [1, 0]
            );
            assert!(quarantined.indeterminate_publication.is_none());
            assert!(quarantined.untouched_request_indices.is_empty());
            assert!(!quarantined.cursor_committed);
        }
        other => panic!("expected closing-currentness quarantine, got {other:?}"),
    }
}

#[test]
fn partial_coordinate_substitution_invalidates_progress() {
    let first_prepared = prepared(16, 11, 64);
    let mut quarantined = match r56_publish_mux_batch_model_only(
        first_prepared,
        currentness(true),
        R56MuxPublicationScriptV1::IndeterminateSecondNativeAfterConfirmedFirst,
    ) {
        R56MuxPublishOutcomeV1::Quarantined(quarantined) => quarantined,
        other => panic!("expected partial quarantine, got {other:?}"),
    };
    assert!(quarantined.is_exact_progress_model_only());
    quarantined
        .indeterminate_publication
        .as_mut()
        .expect("indeterminate coordinate")
        .native_ordinal = 1;
    assert!(!quarantined.is_exact_progress_model_only());

    let second_prepared = prepared(16, 11, 64);
    let mut recoverable = match r56_publish_mux_batch_model_only(
        second_prepared,
        currentness(true),
        R56MuxPublicationScriptV1::RecoverableSecondNativeAfterConfirmedFirst,
    ) {
        R56MuxPublishOutcomeV1::Quarantined(quarantined) => quarantined,
        other => panic!("expected recoverable partial quarantine, got {other:?}"),
    };
    assert!(recoverable.is_exact_progress_model_only());
    recoverable.untouched_request_indices[0] += 2;
    assert!(!recoverable.is_exact_progress_model_only());
}

#[test]
fn exact_completion_releases_every_request_once_in_canonical_order() {
    let published = published(16, 5, 126);
    let completions = published.tickets_model_only().to_vec();
    let expected = published.request_identities_model_only();
    let released = r56_release_completed_mux_batch_model_only(published, &completions).unwrap();
    assert_eq!(
        released
            .iter()
            .map(R56MuxRequestV1::identity_model_only)
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn missing_duplicate_or_substituted_completion_retains_whole_published_custody() {
    for mutation in 0..3 {
        let published = published(8, 2, 10);
        let expected = published.request_identities_model_only();
        let mut completions = published.tickets_model_only().to_vec();
        match mutation {
            0 => {
                completions.pop();
            }
            1 => completions[4] = completions[3],
            2 => completions[4].queue_generation += 1,
            _ => unreachable!(),
        }
        let failure =
            r56_release_completed_mux_batch_model_only(published, &completions).unwrap_err();
        assert_eq!(failure.request_identities_model_only(), expected);
        assert!(failure.into_published_model_only().is_exact_model_only());
    }
}

#[test]
fn publication_record_substitution_invalidates_the_published_batch() {
    type Mutation = fn(&mut R56NativePublicationV1);
    let mutations: [Mutation; 6] = [
        |publication| publication.engine_index ^= 1,
        |publication| publication.queue_id += 1,
        |publication| publication.queue_generation += 1,
        |publication| publication.tail.packet_identity += 1,
        |publication| publication.write_pointer_published_release = false,
        |publication| publication.doorbell_published_release = false,
    ];
    for mutation in mutations {
        let mut published = published(14, 3, 50);
        mutation(&mut published.publications[0]);
        assert!(!published.is_exact_model_only());
    }
}
