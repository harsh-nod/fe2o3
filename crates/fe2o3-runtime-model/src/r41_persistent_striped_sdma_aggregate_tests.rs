use alloc::vec;
use alloc::vec::Vec;

use super::r40_gfx942_striped_sdma_aggregate::{R40Gfx942SdmaPlanKindV1, R40Gfx942SdmaQueuePlanV1};
use super::r41_persistent_striped_sdma_aggregate::*;

fn queue_plan(striped_count: u8) -> R40Gfx942SdmaQueuePlanV1 {
    let queue_ids: Vec<u32> = (100..100 + u32::from(striped_count) + 2).collect();
    R40Gfx942SdmaQueuePlanV1::create_model_only(
        7,
        R40Gfx942SdmaPlanKindV1::CombinedDirectionalStriped,
        striped_count,
        &queue_ids,
    )
    .unwrap()
}

fn pair() -> R41DirectionalPairOccurrenceV1 {
    R41DirectionalPairOccurrenceV1 {
        session_id: 7,
        d2h_queue_id: 100,
        h2d_queue_id: 101,
        queue_generation: 9,
    }
}

fn currentness() -> R41CombinedCurrentnessV1 {
    R41CombinedCurrentnessV1 {
        session_id: 7,
        directional_pair: pair(),
        striped_queue_generation: 10,
        scope_closed: true,
    }
}

fn requests(count: usize) -> Vec<R41PersistentStripedRequestV1> {
    (0..count)
        .map(|index| {
            R41PersistentStripedRequestV1::new_model_only(R41PersistentStripedRequestIdentityV1 {
                request_token: index as u64 + 1,
                direction: if index.is_multiple_of(2) {
                    R41PersistentSdmaDirectionV1::HostToDevice
                } else {
                    R41PersistentSdmaDirectionV1::DeviceToHost
                },
                device: R41PersistentDeviceIdentityV1 {
                    owner_id: 1_000 + index as u64,
                    storage_id: 2_000 + index as u64,
                    session_id: 7,
                    directional_pair: pair(),
                    pool_generation: 11 + index as u64,
                    logical_bytes: 4096,
                    physical_bytes: 8192,
                    kind: R41DeviceStorageKindV1::DeviceLocal,
                },
                host: R41HostStorageIdentityV1 {
                    storage_id: 3_000 + index as u64,
                    session_id: 7,
                    pool_generation: 101 + index as u64,
                    logical_bytes: 4096,
                    physical_bytes: 8192,
                    kind: R41HostStorageKindV1::CoherentGtt,
                },
                device_offset: 64,
                host_offset: 128,
                copy_bytes: 1024,
            })
        })
        .collect()
}

fn prepare_with(
    striped_count: u8,
    request_count: usize,
    first_queue: u8,
    script: R41PreparationScriptV1,
) -> R41PreparationOutcomeV1 {
    R41PersistentStripedPreparedV1::prepare_model_only(
        queue_plan(striped_count),
        pair(),
        currentness(),
        17,
        100,
        200,
        first_queue,
        requests(request_count),
        script,
    )
}

fn prepared(
    striped_count: u8,
    request_count: usize,
    first_queue: u8,
) -> R41PersistentStripedPreparedV1 {
    match prepare_with(
        striped_count,
        request_count,
        first_queue,
        R41PreparationScriptV1::Succeeds,
    ) {
        R41PreparationOutcomeV1::Prepared(prepared) => prepared,
        other => panic!("unexpected preparation: {other:?}"),
    }
}

fn submission(
    striped_count: u8,
    request_count: usize,
    first_queue: u8,
) -> R41PersistentStripedSubmissionV1 {
    match prepared(striped_count, request_count, first_queue)
        .publish_model_only(currentness(), R41PublicationScriptV1::Full)
    {
        R41PublicationOutcomeV1::Published(submission) => submission,
        other => panic!("unexpected publication: {other:?}"),
    }
}

fn observations(
    submission: &R41PersistentStripedSubmissionV1,
    state: R41CompletionObservationStateV1,
) -> Vec<R41CompletionObservationV1> {
    submission
        .completion_roster_model_only()
        .iter()
        .copied()
        .map(|identity| R41CompletionObservationV1 { identity, state })
        .collect()
}

fn tokens(requests: &[R41PersistentStripedRequestV1]) -> Vec<u64> {
    requests
        .iter()
        .map(|request| request.identity_model_only().request_token)
        .collect()
}

fn replace_identity(
    request: &mut R41PersistentStripedRequestV1,
    mutate: impl FnOnce(&mut R41PersistentStripedRequestIdentityV1),
) {
    let mut identity = request.identity_model_only();
    mutate(&mut identity);
    *request = R41PersistentStripedRequestV1::new_model_only(identity);
}

#[test]
fn exact_combined_capacity_and_round_robin_shard_bounds() {
    for striped_count in (2..=14).step_by(2) {
        let count = usize::from(striped_count) * 63;
        let prepared = prepared(striped_count, count, striped_count - 1);
        assert!(prepared.is_fully_prepared_model_only());
        assert_eq!(prepared.plan_model_only().request_count as usize, count);
        assert_eq!(
            prepared.completion_roster_model_only()[0].ticket.queue_slot,
            striped_count - 1
        );
        for slot in 0..striped_count {
            assert_eq!(
                prepared
                    .completion_roster_model_only()
                    .iter()
                    .filter(|identity| identity.ticket.queue_slot == slot)
                    .count(),
                63
            );
        }
    }
    assert_eq!(R41_GFX942_MAX_REQUESTS_V1, 882);
}

#[test]
fn rejects_standalone_malformed_and_over_capacity_plans() {
    let standalone = R40Gfx942SdmaQueuePlanV1::create_model_only(
        7,
        R40Gfx942SdmaPlanKindV1::StandaloneStriped,
        2,
        &[102, 103],
    )
    .unwrap();
    let cases = [
        R41PersistentStripedPreparedV1::prepare_model_only(
            standalone,
            pair(),
            currentness(),
            17,
            100,
            200,
            0,
            requests(1),
            R41PreparationScriptV1::Succeeds,
        ),
        prepare_with(2, 127, 0, R41PreparationScriptV1::Succeeds),
        prepare_with(2, 0, 0, R41PreparationScriptV1::Succeeds),
        prepare_with(2, 1, 2, R41PreparationScriptV1::Succeeds),
    ];
    for outcome in cases {
        assert!(matches!(outcome, R41PreparationOutcomeV1::Retryable(_)));
    }

    let mut malformed = queue_plan(2);
    malformed.striped_queue_count = 3;
    let outcome = R41PersistentStripedPreparedV1::prepare_model_only(
        malformed,
        pair(),
        currentness(),
        17,
        100,
        200,
        0,
        requests(1),
        R41PreparationScriptV1::Succeeds,
    );
    assert!(matches!(outcome, R41PreparationOutcomeV1::Retryable(_)));
}

#[test]
fn admission_requires_exact_pair_currentness_ranges_and_distinct_storage() {
    let mut wrong_pair = pair();
    wrong_pair.d2h_queue_id += 1;
    let mut open = currentness();
    open.scope_closed = false;
    let mut invalid_range = requests(2);
    replace_identity(&mut invalid_range[1], |identity| identity.copy_bytes = 4097);
    let mut duplicate_owner = requests(2);
    let owner_id = duplicate_owner[0].identity_model_only().device.owner_id;
    replace_identity(&mut duplicate_owner[1], |identity| {
        identity.device.owner_id = owner_id;
    });
    let mut duplicate_device = requests(2);
    let device_storage_id = duplicate_device[0].identity_model_only().device.storage_id;
    replace_identity(&mut duplicate_device[1], |identity| {
        identity.device.storage_id = device_storage_id;
    });
    let mut duplicate_host = requests(2);
    let host_storage_id = duplicate_host[0].identity_model_only().host.storage_id;
    replace_identity(&mut duplicate_host[1], |identity| {
        identity.host.storage_id = host_storage_id;
    });
    let mut zero_device_generation = requests(2);
    replace_identity(&mut zero_device_generation[1], |identity| {
        identity.device.pool_generation = 0;
    });
    let mut zero_host_generation = requests(2);
    replace_identity(&mut zero_host_generation[1], |identity| {
        identity.host.pool_generation = 0;
    });
    let mut invalid_host_extent = requests(2);
    replace_identity(&mut invalid_host_extent[1], |identity| {
        identity.host.physical_bytes = identity.host.logical_bytes - 1;
    });
    let mut noncoherent_host = requests(2);
    replace_identity(&mut noncoherent_host[1], |identity| {
        identity.host.kind = R41HostStorageKindV1::Other;
    });
    let mut unaligned_device = requests(2);
    replace_identity(&mut unaligned_device[1], |identity| {
        identity.device.physical_bytes = 8193;
    });
    let mut oversized_device = requests(2);
    replace_identity(&mut oversized_device[1], |identity| {
        identity.device.logical_bytes = R41_PERSISTENT_DEVICE_MAX_ALLOCATION_BYTES_V1 + 4096;
        identity.device.physical_bytes = R41_PERSISTENT_DEVICE_MAX_ALLOCATION_BYTES_V1 + 4096;
    });
    let mut nonlocal_device = requests(2);
    replace_identity(&mut nonlocal_device[1], |identity| {
        identity.device.kind = R41DeviceStorageKindV1::Other;
    });

    let inputs = [
        (wrong_pair, currentness(), requests(2)),
        (pair(), open, requests(2)),
        (pair(), currentness(), invalid_range),
        (pair(), currentness(), duplicate_owner),
        (pair(), currentness(), duplicate_device),
        (pair(), currentness(), duplicate_host),
        (pair(), currentness(), zero_device_generation),
        (pair(), currentness(), zero_host_generation),
        (pair(), currentness(), invalid_host_extent),
        (pair(), currentness(), noncoherent_host),
        (pair(), currentness(), unaligned_device),
        (pair(), currentness(), oversized_device),
        (pair(), currentness(), nonlocal_device),
    ];
    for (pair, currentness, requests) in inputs {
        let outcome = R41PersistentStripedPreparedV1::prepare_model_only(
            queue_plan(2),
            pair,
            currentness,
            17,
            100,
            200,
            0,
            requests,
            R41PreparationScriptV1::Succeeds,
        );
        match outcome {
            R41PreparationOutcomeV1::Retryable(recovered) => {
                assert_eq!(tokens(recovered.requests_model_only()), vec![1, 2]);
                assert!(recovered.recovered_every_request_in_order_model_only());
            }
            other => panic!("unexpected admission result: {other:?}"),
        }
    }
}

#[test]
fn heterogeneous_buffer_generations_are_preserved_exactly() {
    let prepared = prepared(4, 4, 0);
    assert_eq!(
        prepared
            .completion_roster_model_only()
            .iter()
            .map(|identity| identity.request.device.pool_generation)
            .collect::<Vec<_>>(),
        vec![11, 12, 13, 14]
    );
    assert_eq!(
        prepared
            .completion_roster_model_only()
            .iter()
            .map(|identity| identity.request.host.pool_generation)
            .collect::<Vec<_>>(),
        vec![101, 102, 103, 104]
    );
    assert!(
        prepared
            .completion_roster_model_only()
            .iter()
            .all(|identity| {
                identity.request.host.kind == R41HostStorageKindV1::CoherentGtt
                    && identity.request.host.logical_bytes <= identity.request.host.physical_bytes
            })
    );

    let submission = match prepared.publish_model_only(currentness(), R41PublicationScriptV1::Full)
    {
        R41PublicationOutcomeV1::Published(submission) => submission,
        other => panic!("unexpected publication: {other:?}"),
    };
    let presentation = submission.presentation_model_only();
    let observed = observations(&submission, R41CompletionObservationStateV1::Ready);
    match submission.poll_model_only(
        &presentation,
        &observed,
        150,
        &[true; 4],
        R41RestorationScriptV1::AllSucceed,
    ) {
        R41PollOutcomeV1::Completed(completed) => {
            assert_eq!(
                completed
                    .completed_model_only()
                    .iter()
                    .map(|item| {
                        let identity = item.request_model_only().identity_model_only();
                        (
                            identity.device.pool_generation,
                            identity.host.pool_generation,
                        )
                    })
                    .collect::<Vec<_>>(),
                vec![(11, 101), (12, 102), (13, 103), (14, 104)]
            );
        }
        other => panic!("unexpected completion: {other:?}"),
    }
}

#[test]
fn packet_and_persistent_extent_boundaries_are_exact() {
    let mut maximum_packet = requests(1);
    replace_identity(&mut maximum_packet[0], |identity| {
        identity.device_offset = 0;
        identity.host_offset = 0;
        identity.copy_bytes = R41_GFX942_MAX_LINEAR_COPY_BYTES_V1;
        identity.device.logical_bytes = R41_GFX942_MAX_LINEAR_COPY_BYTES_V1;
        identity.device.physical_bytes = 4 * 1024 * 1024;
        identity.host.logical_bytes = R41_GFX942_MAX_LINEAR_COPY_BYTES_V1;
        identity.host.physical_bytes = R41_GFX942_MAX_LINEAR_COPY_BYTES_V1;
    });
    assert!(matches!(
        R41PersistentStripedPreparedV1::prepare_model_only(
            queue_plan(2),
            pair(),
            currentness(),
            17,
            100,
            200,
            0,
            maximum_packet,
            R41PreparationScriptV1::Succeeds,
        ),
        R41PreparationOutcomeV1::Prepared(_)
    ));

    let mut packet_plus_one = requests(1);
    replace_identity(&mut packet_plus_one[0], |identity| {
        identity.device_offset = 0;
        identity.host_offset = 0;
        identity.copy_bytes = R41_GFX942_MAX_LINEAR_COPY_BYTES_V1 + 1;
        identity.device.logical_bytes = R41_GFX942_MAX_LINEAR_COPY_BYTES_V1 + 1;
        identity.device.physical_bytes = 4 * 1024 * 1024;
        identity.host.logical_bytes = R41_GFX942_MAX_LINEAR_COPY_BYTES_V1 + 1;
        identity.host.physical_bytes = R41_GFX942_MAX_LINEAR_COPY_BYTES_V1 + 1;
    });
    assert!(matches!(
        R41PersistentStripedPreparedV1::prepare_model_only(
            queue_plan(2),
            pair(),
            currentness(),
            17,
            100,
            200,
            0,
            packet_plus_one,
            R41PreparationScriptV1::Succeeds,
        ),
        R41PreparationOutcomeV1::Retryable(_)
    ));

    let mut maximum_allocation = requests(1);
    replace_identity(&mut maximum_allocation[0], |identity| {
        identity.device.physical_bytes = R41_PERSISTENT_DEVICE_MAX_ALLOCATION_BYTES_V1;
    });
    assert!(matches!(
        R41PersistentStripedPreparedV1::prepare_model_only(
            queue_plan(2),
            pair(),
            currentness(),
            17,
            100,
            200,
            0,
            maximum_allocation,
            R41PreparationScriptV1::Succeeds,
        ),
        R41PreparationOutcomeV1::Prepared(_)
    ));
}

#[test]
fn every_preparation_failure_recovers_the_exact_input_order() {
    for fail_at in 0..8 {
        match prepare_with(
            4,
            8,
            3,
            R41PreparationScriptV1::FailsAt {
                request_index: fail_at,
                recovery: R41PreparationRecoveryV1::Succeeds,
            },
        ) {
            R41PreparationOutcomeV1::Retryable(recovered) => {
                assert_eq!(recovered.prepared_count, usize::from(fail_at));
                assert_eq!(recovered.recovered_count, 8);
                assert_eq!(recovered.cursor_before, 3);
                assert_eq!(
                    tokens(recovered.requests_model_only()),
                    (1..=8).collect::<Vec<_>>()
                );
            }
            other => panic!("unexpected preparation failure: {other:?}"),
        }
    }
}

#[test]
fn failed_preparation_recovery_quarantines_the_whole_batch() {
    match prepare_with(
        4,
        8,
        3,
        R41PreparationScriptV1::FailsAt {
            request_index: 4,
            recovery: R41PreparationRecoveryV1::Fails { terminal_token: 99 },
        },
    ) {
        R41PreparationOutcomeV1::Terminal(terminal) => {
            assert_eq!(terminal.stage, R41TerminalStageV1::Preparation);
            assert_eq!(terminal.entries_model_only().len(), 8);
            assert!(terminal.quarantine_is_monotonic_model_only());
            assert_eq!(terminal.cursor_before, 3);
            assert_eq!(terminal.resulting_cursor, 3);
            assert!(!terminal.cursor_committed);
            assert!(terminal.cursor_history_is_consistent_model_only());
        }
        other => panic!("unexpected recovery failure: {other:?}"),
    }
}

#[test]
fn partial_publication_has_an_exact_shard_partition_and_never_commits_cursor() {
    for confirmed in 0..4 {
        for indeterminate in [false, true] {
            let outcome = prepared(4, 10, 3).publish_model_only(
                currentness(),
                R41PublicationScriptV1::StopsAfter {
                    confirmed_shards: confirmed,
                    next_shard_indeterminate: indeterminate,
                    terminal_token: 71,
                },
            );
            match outcome {
                R41PublicationOutcomeV1::Terminal(terminal) => {
                    let partition = terminal.partition.as_ref().unwrap();
                    assert_eq!(
                        partition.confirmed_count_model_only(),
                        usize::from(confirmed)
                    );
                    assert_eq!(
                        partition.indeterminate_count_model_only(),
                        usize::from(indeterminate)
                    );
                    assert_eq!(
                        partition.untouched_count_model_only(),
                        4 - usize::from(confirmed) - usize::from(indeterminate)
                    );
                    assert!(
                        partition.is_exact_for_model_only(
                            terminal.plan.as_ref().unwrap(),
                            &terminal
                                .entries_model_only()
                                .iter()
                                .map(|entry| R41CompletionIdentityV1 {
                                    ticket: entry.ticket.unwrap(),
                                    request: entry.request_model_only().identity_model_only(),
                                })
                                .collect::<Vec<_>>()
                        )
                    );
                    assert!(terminal.quarantine_is_monotonic_model_only());
                    assert_eq!(terminal.cursor_before, 3);
                    assert_eq!(terminal.resulting_cursor, 3);
                    assert!(!terminal.cursor_committed);
                    assert!(terminal.cursor_history_is_consistent_model_only());
                }
                other => panic!("unexpected partial publication: {other:?}"),
            }
        }
    }
}

#[test]
fn cursor_commits_only_after_full_publication_and_exact_close() {
    let published = submission(4, 10, 3);
    assert_eq!(published.committed_cursor_model_only(), 1);

    let mut stale = currentness();
    stale.striped_queue_generation += 1;
    match prepared(4, 10, 3).publish_model_only(stale, R41PublicationScriptV1::Full) {
        R41PublicationOutcomeV1::Terminal(terminal) => {
            assert_eq!(
                terminal.reason,
                R41TerminalReasonV1::PublicationCurrentnessCloseFailed
            );
            assert!(!terminal.cursor_committed);
            assert_eq!(terminal.cursor_before, 3);
            assert_eq!(terminal.resulting_cursor, 3);
            assert!(terminal.quarantine_is_monotonic_model_only());
            assert!(terminal.cursor_history_is_consistent_model_only());
        }
        other => panic!("unexpected stale close: {other:?}"),
    }
}

#[test]
fn pending_and_timeout_scan_and_retain_the_whole_roster() {
    for pending_at in 0..6 {
        let submission = submission(4, 6, 1);
        let presentation = submission.presentation_model_only();
        let mut observed = observations(&submission, R41CompletionObservationStateV1::Ready);
        observed[pending_at].state = R41CompletionObservationStateV1::Pending;
        match submission.poll_model_only(
            &presentation,
            &observed,
            199,
            &[],
            R41RestorationScriptV1::AllSucceed,
        ) {
            R41PollOutcomeV1::Pending(waiting) => {
                assert_eq!(waiting.observation_count, 6);
                assert_eq!(waiting.restored_count, 0);
                assert_eq!(
                    waiting
                        .submission_model_only()
                        .completion_roster_model_only()
                        .len(),
                    6
                );
            }
            other => panic!("unexpected pending: {other:?}"),
        }
    }

    let submission = submission(4, 6, 1);
    let presentation = submission.presentation_model_only();
    let observed = observations(&submission, R41CompletionObservationStateV1::Pending);
    match submission.poll_model_only(
        &presentation,
        &observed,
        200,
        &[],
        R41RestorationScriptV1::AllSucceed,
    ) {
        R41PollOutcomeV1::TimedOut(waiting) => {
            assert_eq!(waiting.observation_count, 6);
            assert_eq!(waiting.restored_count, 0);
            assert_eq!(
                waiting
                    .submission_model_only()
                    .committed_cursor_model_only(),
                3
            );
        }
        other => panic!("unexpected timeout: {other:?}"),
    }
}

#[test]
fn completion_validates_presentation_identity_and_currentness_before_observation() {
    let mutations: [fn(&mut R41CompletionPresentationV1); 6] = [
        |presented| presented.plan.submission_id += 1,
        |presented| presented.completion_roster[0].ticket.queue_id += 1,
        |presented| presented.completion_roster[0].request.device.storage_id += 1,
        |presented| {
            presented.completion_roster[0]
                .request
                .device
                .pool_generation += 1
        },
        |presented| presented.completion_roster[0].request.host.pool_generation += 1,
        |presented| presented.closing_currentness.scope_closed = false,
    ];
    for mutate in mutations {
        let submission = submission(4, 6, 1);
        let mut presentation = submission.presentation_model_only();
        mutate(&mut presentation);
        let observed = observations(
            &submission,
            R41CompletionObservationStateV1::Error { terminal_token: 88 },
        );
        match submission.poll_model_only(
            &presentation,
            &observed,
            150,
            &[true; 6],
            R41RestorationScriptV1::AllSucceed,
        ) {
            R41PollOutcomeV1::Terminal(terminal) => {
                assert_eq!(terminal.observation_count, 0);
                assert_eq!(terminal.restored_count, 0);
                assert!(terminal.quarantine_is_monotonic_model_only());
                assert!(terminal.cursor_committed);
                assert_eq!(terminal.resulting_cursor, 3);
                assert!(terminal.cursor_history_is_consistent_model_only());
            }
            other => panic!("unexpected presentation mutation: {other:?}"),
        }
    }
}

#[test]
fn completion_identity_preflight_is_all_before_any_restoration() {
    for fail_at in 0..6 {
        let submission = submission(4, 6, 1);
        let presentation = submission.presentation_model_only();
        let observed = observations(&submission, R41CompletionObservationStateV1::Ready);
        let mut preflight = [true; 6];
        preflight[fail_at] = false;
        match submission.poll_model_only(
            &presentation,
            &observed,
            150,
            &preflight,
            R41RestorationScriptV1::AllSucceed,
        ) {
            R41PollOutcomeV1::Terminal(terminal) => {
                assert_eq!(terminal.observation_count, 6);
                assert_eq!(terminal.restored_count, 0);
                assert!(terminal.quarantine_is_monotonic_model_only());
                assert!(terminal.cursor_committed);
                assert_eq!(terminal.resulting_cursor, 3);
                assert!(terminal.cursor_history_is_consistent_model_only());
            }
            other => panic!("unexpected preflight failure: {other:?}"),
        }
    }
}

#[test]
fn successful_restoration_returns_every_owner_in_original_order() {
    let submission = submission(4, 10, 3);
    let presentation = submission.presentation_model_only();
    let observed = observations(&submission, R41CompletionObservationStateV1::Ready);
    match submission.poll_model_only(
        &presentation,
        &observed,
        150,
        &[true; 10],
        R41RestorationScriptV1::AllSucceed,
    ) {
        R41PollOutcomeV1::Completed(completed) => {
            assert!(completed.restoration_is_complete_and_ordered_model_only());
            assert_eq!(completed.committed_cursor, 1);
            assert_eq!(
                completed
                    .completed_model_only()
                    .iter()
                    .map(|item| item
                        .request_model_only()
                        .identity_model_only()
                        .request_token)
                    .collect::<Vec<_>>(),
                (1..=10).collect::<Vec<_>>()
            );
        }
        other => panic!("unexpected completion: {other:?}"),
    }
}

#[test]
fn every_restoration_failure_is_whole_batch_terminal_and_ordered() {
    for fail_at in 0..8 {
        let submission = submission(4, 8, 3);
        let presentation = submission.presentation_model_only();
        let observed = observations(&submission, R41CompletionObservationStateV1::Ready);
        match submission.poll_model_only(
            &presentation,
            &observed,
            150,
            &[true; 8],
            R41RestorationScriptV1::FailsAt {
                request_index: fail_at,
                terminal_token: 91,
            },
        ) {
            R41PollOutcomeV1::Terminal(terminal) => {
                assert_eq!(terminal.restored_count, usize::from(fail_at));
                assert_eq!(terminal.entries_model_only().len(), 8);
                assert_eq!(
                    terminal
                        .entries_model_only()
                        .iter()
                        .filter(|entry| entry.restored_before_terminal)
                        .count(),
                    usize::from(fail_at)
                );
                assert_eq!(
                    terminal
                        .entries_model_only()
                        .iter()
                        .map(|entry| entry
                            .request_model_only()
                            .identity_model_only()
                            .request_token)
                        .collect::<Vec<_>>(),
                    (1..=8).collect::<Vec<_>>()
                );
                assert!(terminal.quarantine_is_monotonic_model_only());
                assert!(terminal.cursor_committed);
                assert_eq!(terminal.cursor_before, 3);
                assert_eq!(terminal.resulting_cursor, 3);
                assert!(terminal.cursor_history_is_consistent_model_only());
            }
            other => panic!("unexpected restoration failure: {other:?}"),
        }
    }
}

#[test]
fn observation_error_is_terminal_and_pending_can_resume() {
    let submission = submission(2, 4, 0);
    let presentation = submission.presentation_model_only();
    let mut observed = observations(&submission, R41CompletionObservationStateV1::Ready);
    observed[2].state = R41CompletionObservationStateV1::Pending;
    let resumed = match submission.poll_model_only(
        &presentation,
        &observed,
        150,
        &[],
        R41RestorationScriptV1::AllSucceed,
    ) {
        R41PollOutcomeV1::Pending(waiting) => waiting.into_submission_model_only(),
        other => panic!("unexpected pending: {other:?}"),
    };
    let presentation = resumed.presentation_model_only();
    let mut observed = observations(&resumed, R41CompletionObservationStateV1::Ready);
    observed[2].state = R41CompletionObservationStateV1::Error { terminal_token: 44 };
    match resumed.poll_model_only(
        &presentation,
        &observed,
        151,
        &[true; 4],
        R41RestorationScriptV1::AllSucceed,
    ) {
        R41PollOutcomeV1::Terminal(terminal) => {
            assert_eq!(terminal.observation_count, 3);
            assert_eq!(terminal.restored_count, 0);
            assert!(terminal.quarantine_is_monotonic_model_only());
            assert!(terminal.cursor_committed);
            assert_eq!(terminal.resulting_cursor, 0);
            assert!(terminal.cursor_history_is_consistent_model_only());
        }
        other => panic!("unexpected observation error: {other:?}"),
    }
}
