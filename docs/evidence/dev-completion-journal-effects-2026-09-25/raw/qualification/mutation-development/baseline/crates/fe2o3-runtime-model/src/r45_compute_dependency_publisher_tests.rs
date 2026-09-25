use alloc::{vec, vec::Vec};

use super::r45_compute_dependency_publisher::*;

const SESSION: u64 = 41;
const TARGET_QUEUE: u64 = 900;

fn premise() -> R45EpochMintingPremiseV1 {
    R45EpochMintingPremiseV1 {
        exactly_one_owner_for_session: true,
        all_source_epochs_minted_by_owner: true,
    }
}

fn publisher() -> R45ComputeDependencyPublisherV1 {
    R45ComputeDependencyPublisherV1::new_model_only(501, SESSION, premise()).unwrap()
}

fn occurrence(epoch: u64, queue: u64, mapping: u64, ordinal: u64) -> R45OccurrenceIdentityV1 {
    R45OccurrenceIdentityV1 {
        session_occurrence: SESSION,
        acceptance_epoch: epoch,
        queue_occurrence: queue,
        signal_mapping: mapping,
        batch_id: 1_000 + ordinal,
        slot: ordinal as u16,
        slot_generation: 2_000 + ordinal,
        dispatch_generation: 3_000 + ordinal,
        packet_id: 4_000 + ordinal,
    }
}

fn target(epoch: u64) -> R45SealedTargetBundleV1 {
    let identity = R45OccurrenceIdentityV1 {
        packet_id: 0,
        ..occurrence(epoch, TARGET_QUEUE, 901, 99)
    };
    let completion_signal = R45SignalIdentityV1::from_occurrence_model_only(identity);
    R45SealedTargetBundleV1::seal_model_only(
        identity,
        R45TargetBatchIdentityV1 {
            target: identity,
            retention_id: 71,
            completion_signal,
        },
        R45TargetEventV1 {
            event_id: 73,
            target: identity,
        },
        R45FinalDispatchV1 {
            dispatch_id: 79,
            completion_signal,
            target: identity,
        },
    )
    .unwrap()
}

fn source(
    epoch: u64,
    target_epoch: u64,
    queue: u64,
    mapping: u64,
    ordinal: u64,
) -> R45SourceReaderCustodyV1 {
    let record = source_record(epoch, target_epoch, queue, mapping, ordinal);
    R45SourceReaderCustodyV1::new_model_only(record, 20_000 + ordinal).unwrap()
}

fn source_record(
    epoch: u64,
    target_epoch: u64,
    queue: u64,
    mapping: u64,
    ordinal: u64,
) -> R45SourceRecordV1 {
    let source = occurrence(epoch, queue, mapping, ordinal);
    R45SourceRecordV1 {
        source,
        event_id: 10_000 + ordinal,
        signal_identity: R45SignalIdentityV1::from_occurrence_model_only(source),
        dependent_epoch: target_epoch,
    }
}

fn source_signal(epoch: u64, queue: u64, mapping: u64, ordinal: u64) -> R45SignalIdentityV1 {
    R45SignalIdentityV1::from_occurrence_model_only(occurrence(epoch, queue, mapping, ordinal))
}

fn live_reader(
    epoch: u64,
    target_epoch: u64,
    queue: u64,
    mapping: u64,
    ordinal: u64,
) -> R45LiveReaderRecordV1 {
    R45LiveReaderRecordV1::from_parts_model_only(
        source_record(epoch, target_epoch, queue, mapping, ordinal),
        20_000 + ordinal,
    )
}

fn reserve_through(owner: &mut R45ComputeDependencyPublisherV1, epoch: u64) -> R45AcceptanceV1 {
    let mut acceptance = None;
    for _ in 0..epoch {
        acceptance = Some(owner.reserve_acceptance_epoch_model_only().unwrap());
    }
    acceptance.unwrap()
}

fn prepared(
    owner: &mut R45ComputeDependencyPublisherV1,
    sources: Vec<R45SourceReaderCustodyV1>,
    epoch: u64,
) -> R45PreparedTargetUseV1 {
    let acceptance = reserve_through(owner, epoch);
    owner
        .begin_target_use_model_only(acceptance, target(epoch), sources)
        .unwrap()
}

fn retryable(
    owner: &mut R45ComputeDependencyPublisherV1,
    sources: Vec<R45SourceReaderCustodyV1>,
    epoch: u64,
) -> R45RetryableTargetUseV1 {
    let prepared = prepared(owner, sources, epoch);
    match owner.publish_native_model_only(prepared, R45NativeFaultV1::RingOccupied) {
        Err(R45PublicationFailureV1::Retryable(retryable)) => retryable,
        other => panic!("unexpected publication result: {other:?}"),
    }
}

#[test]
fn owner_requires_explicit_premise_and_documents_duplicate_owner_limit() {
    for invalid in [
        R45EpochMintingPremiseV1 {
            exactly_one_owner_for_session: false,
            all_source_epochs_minted_by_owner: true,
        },
        R45EpochMintingPremiseV1 {
            exactly_one_owner_for_session: true,
            all_source_epochs_minted_by_owner: false,
        },
    ] {
        assert_eq!(
            R45ComputeDependencyPublisherV1::new_model_only(501, SESSION, invalid).unwrap_err(),
            R45DependencyPublisherErrorV1::MissingEpochMintingPremise
        );
    }
    assert_eq!(
        R45ComputeDependencyPublisherV1::new_model_only(501, 0, premise()).unwrap_err(),
        R45DependencyPublisherErrorV1::InvalidSessionOccurrence
    );
    assert_eq!(
        R45ComputeDependencyPublisherV1::new_model_only(0, SESSION, premise()).unwrap_err(),
        R45DependencyPublisherErrorV1::InvalidOwnerOccurrence
    );

    // The premise is contracted, not globally enforced: two model owners can
    // each claim it and mint epoch one for the same session.
    let mut first = publisher();
    let mut duplicate =
        R45ComputeDependencyPublisherV1::new_model_only(502, SESSION, premise()).unwrap();
    assert_eq!(
        first
            .reserve_acceptance_epoch_model_only()
            .unwrap()
            .epoch_model_only(),
        1
    );
    assert_eq!(
        duplicate
            .reserve_acceptance_epoch_model_only()
            .unwrap()
            .epoch_model_only(),
        1
    );
}

#[test]
fn begin_rejects_a_token_minted_by_a_distinct_owner_without_mutation() {
    let mut primary = publisher();
    let mut decoy =
        R45ComputeDependencyPublisherV1::new_model_only(502, SESSION, premise()).unwrap();
    let acceptance = decoy.reserve_acceptance_epoch_model_only().unwrap();
    let before = primary.snapshot_model_only();
    let failure = primary
        .begin_target_use_model_only(acceptance, target(1), vec![source(1, 1, 101, 201, 1)])
        .unwrap_err();
    assert_eq!(
        failure.error,
        R45DependencyPublisherErrorV1::AcceptanceSubstitution
    );
    assert_eq!(failure.acceptance.owner_occurrence_model_only(), 502);
    assert_eq!(failure.readers.len(), 1);
    assert_eq!(primary.snapshot_model_only(), before);
}

#[test]
fn epochs_are_per_owner_monotonic_and_burned_after_retry_rollback() {
    let mut owner = publisher();
    let sources = vec![source(1, 2, 101, 201, 1)];
    let retryable = retryable(&mut owner, sources, 2);
    let target_bundle = target(2);
    let mut source_owner = R45SourceArenaV1::new_model_only(
        R45ArenaIdentityV1 {
            queue_occurrence: 101,
            signal_mapping: 201,
        },
        vec![live_reader(1, 2, 101, 201, 1)],
    )
    .unwrap();
    let mut source_owners = [&mut source_owner];
    let mut target_owner = R45TargetArenaV1::from_bundle_model_only(&target_bundle);
    owner
        .rollback_retryable_model_only(retryable, &mut source_owners, &mut target_owner)
        .unwrap();
    assert_eq!(
        owner
            .reserve_acceptance_epoch_model_only()
            .unwrap()
            .epoch_model_only(),
        3
    );
}

#[test]
fn exact_bounds_pack_b37_barriers_and_preserve_source_order() {
    for (count, barrier_count) in [(1, 1), (5, 1), (6, 2), (255, 51), (256, 52)] {
        let target_epoch = count as u64 + 1;
        let sources = (0..count)
            .map(|index| {
                source(
                    index as u64 + 1,
                    target_epoch,
                    100 + (index % 3) as u64,
                    200 + (index % 3) as u64,
                    index as u64 + 1,
                )
            })
            .collect::<Vec<_>>();
        let expected_signals = sources
            .iter()
            .map(|source| source.record_model_only().signal_identity)
            .collect::<Vec<_>>();
        let mut owner = publisher();
        let prepared = prepared(&mut owner, sources, target_epoch);
        assert_eq!(prepared.source_count_model_only(), count);
        assert_eq!(prepared.barrier_count_model_only(), barrier_count);
        let publication = owner
            .publish_native_model_only(prepared, R45NativeFaultV1::None)
            .unwrap();
        assert_eq!(publication.plan.barriers.len(), barrier_count);
        assert!(
            publication
                .plan
                .barriers
                .iter()
                .all(|barrier| !barrier.signals.is_empty() && barrier.signals.len() <= 5)
        );
        assert_eq!(
            publication
                .plan
                .barriers
                .iter()
                .flat_map(|barrier| barrier.signals.iter().copied())
                .collect::<Vec<_>>(),
            expected_signals
        );
    }
    assert_eq!(R45_MAX_BARRIERS_V1, 52);
}

#[test]
fn admission_rejections_are_inert_and_return_exact_custody() {
    let mut cross_record = source_record(1, 2, 101, 201, 1);
    cross_record.source.session_occurrence = 99;
    let cases = [
        (
            R45DependencyPublisherErrorV1::CrossSession,
            R45SourceReaderCustodyV1::new_model_only(cross_record, 20_001).unwrap(),
        ),
        (
            R45DependencyPublisherErrorV1::SameQueue,
            source(1, 2, TARGET_QUEUE, 201, 1),
        ),
        (
            R45DependencyPublisherErrorV1::SelfDependency,
            source(2, 2, 101, 201, 1),
        ),
        (
            R45DependencyPublisherErrorV1::DependencyCycle,
            source(3, 2, 101, 201, 1),
        ),
    ];
    for (expected, hostile) in cases {
        let mut owner = publisher();
        let acceptance = reserve_through(&mut owner, 2);
        let target = target(2);
        let before = owner.snapshot_model_only();
        let expected_target_identity = target.target_model_only();
        let expected_source = hostile.record_model_only();
        let expected_acceptance = (
            acceptance.owner_occurrence_model_only(),
            acceptance.session_occurrence_model_only(),
            acceptance.epoch_model_only(),
        );
        let failure = owner
            .begin_target_use_model_only(acceptance, target, vec![hostile])
            .unwrap_err();
        assert_eq!(failure.error, expected);
        assert_eq!(
            (
                failure.acceptance.owner_occurrence_model_only(),
                failure.acceptance.session_occurrence_model_only(),
                failure.acceptance.epoch_model_only(),
            ),
            expected_acceptance
        );
        assert_eq!(failure.target.target_model_only(), expected_target_identity);
        assert_eq!(failure.readers.len(), 1);
        assert_eq!(failure.readers[0].record_model_only(), expected_source);
        assert_eq!(owner.snapshot_model_only(), before);
    }

    let mut owner = publisher();
    let acceptance = reserve_through(&mut owner, 3);
    let first = source(1, 3, 101, 201, 1);
    let duplicate = source(1, 3, 101, 201, 1);
    let before = owner.snapshot_model_only();
    let failure = owner
        .begin_target_use_model_only(acceptance, target(3), vec![first, duplicate])
        .unwrap_err();
    assert_eq!(
        failure.error,
        R45DependencyPublisherErrorV1::DuplicateSource
    );
    assert_eq!(failure.readers.len(), 2);
    assert_eq!(
        failure.readers[0].record_model_only(),
        failure.readers[1].record_model_only()
    );
    assert_eq!(owner.snapshot_model_only(), before);
}

#[test]
fn target_component_substitution_rejects_before_native_or_active_mutation() {
    for substitution in 0..3 {
        let mut owner = publisher();
        let acceptance = reserve_through(&mut owner, 2);
        let mut hostile = target(2);
        if substitution == 0 {
            hostile = hostile.substitute_final_dispatch_model_only(R45FinalDispatchV1 {
                dispatch_id: 808,
                completion_signal: source_signal(2, 777, 778, 98),
                target: target(2).target_model_only(),
            });
        } else if substitution == 1 {
            hostile = hostile.substitute_event_model_only(R45TargetEventV1 {
                event_id: 909,
                target: occurrence(2, 777, 778, 98),
            });
        } else {
            hostile = hostile.substitute_final_dispatch_model_only(R45FinalDispatchV1 {
                dispatch_id: 808,
                completion_signal: R45SignalIdentityV1::from_occurrence_model_only(
                    target(2).target_model_only(),
                ),
                target: occurrence(2, 777, 778, 98),
            });
        }
        let before = owner.snapshot_model_only();
        let failure = owner
            .begin_target_use_model_only(acceptance, hostile, vec![source(1, 2, 101, 201, 1)])
            .unwrap_err();
        assert_eq!(
            failure.error,
            R45DependencyPublisherErrorV1::TargetComponentSplit
        );
        assert_eq!(failure.acceptance.epoch_model_only(), 2);
        assert_eq!(failure.readers.len(), 1);
        assert_eq!(owner.snapshot_model_only(), before);
    }
}

#[test]
fn empty_over_capacity_and_duplicate_signal_reject_without_mutation() {
    let mut owner = publisher();
    let acceptance = reserve_through(&mut owner, 2);
    let before = owner.snapshot_model_only();
    assert_eq!(
        owner
            .begin_target_use_model_only(acceptance, target(2), Vec::new())
            .unwrap_err()
            .error,
        R45DependencyPublisherErrorV1::EmptyDependencies
    );
    assert_eq!(owner.snapshot_model_only(), before);

    let mut owner = publisher();
    let acceptance = reserve_through(&mut owner, 258);
    let too_many = (0..257)
        .map(|index| source(index as u64 + 1, 258, 101, 201, index as u64 + 1))
        .collect();
    assert_eq!(
        owner
            .begin_target_use_model_only(acceptance, target(258), too_many)
            .unwrap_err()
            .error,
        R45DependencyPublisherErrorV1::TooManyDependencies
    );

    let mut owner = publisher();
    let acceptance = reserve_through(&mut owner, 3);
    let first_record = source_record(1, 3, 101, 201, 1);
    let first = R45SourceReaderCustodyV1::new_model_only(first_record, 20_001).unwrap();
    let mut second_record = first_record;
    second_record.source.acceptance_epoch = 2;
    second_record.source.batch_id = 1_002;
    second_record.source.dispatch_generation = 3_002;
    second_record.source.packet_id = 4_002;
    second_record.event_id = 10_002;
    let second = R45SourceReaderCustodyV1::new_model_only(second_record, 20_002).unwrap();
    assert_eq!(
        owner
            .begin_target_use_model_only(acceptance, target(3), vec![first, second])
            .unwrap_err()
            .error,
        R45DependencyPublisherErrorV1::DuplicateSignal
    );
}

#[test]
fn duplicate_reader_event_and_lease_identities_are_rejected_before_activation() {
    for duplicate_lease in [false, true] {
        let mut owner = publisher();
        let acceptance = reserve_through(&mut owner, 3);
        let first = source(1, 3, 101, 201, 1);
        let mut second_record = source_record(2, 3, 102, 202, 2);
        if !duplicate_lease {
            second_record.event_id = first.record_model_only().event_id;
        }
        let second = R45SourceReaderCustodyV1::new_model_only(
            second_record,
            if duplicate_lease { 20_001 } else { 20_002 },
        )
        .unwrap();
        let before = owner.snapshot_model_only();
        let failure = owner
            .begin_target_use_model_only(acceptance, target(3), vec![first, second])
            .unwrap_err();
        assert_eq!(
            failure.error,
            if duplicate_lease {
                R45DependencyPublisherErrorV1::DuplicateLease
            } else {
                R45DependencyPublisherErrorV1::DuplicateEvent
            }
        );
        assert_eq!(failure.readers.len(), 2);
        assert_eq!(owner.snapshot_model_only(), before);
    }
}

#[test]
fn coordinated_epoch_substitution_fails_exact_mint_authentication() {
    let mut owner = publisher();
    let acceptance = reserve_through(&mut owner, 2).substitute_epoch_model_only(9);
    let mint_id = acceptance.mint_id_model_only();
    let before = owner.snapshot_model_only();
    let failure = owner
        .begin_target_use_model_only(acceptance, target(9), vec![source(1, 9, 101, 201, 1)])
        .unwrap_err();
    assert_eq!(
        failure.error,
        R45DependencyPublisherErrorV1::AcceptanceSubstitution
    );
    assert_eq!(failure.acceptance.mint_id_model_only(), mint_id);
    assert_eq!(failure.acceptance.epoch_model_only(), 9);
    assert_eq!(owner.snapshot_model_only(), before);
}

#[test]
fn publication_has_one_claim_bodies_before_headers_and_one_doorbell_without_prepoll() {
    let mut owner = publisher();
    let sources = (1..=6)
        .map(|ordinal| source(ordinal, 7, 101, 201, ordinal))
        .collect();
    let prepared = prepared(&mut owner, sources, 7);
    owner
        .publish_native_model_only(prepared, R45NativeFaultV1::None)
        .unwrap();
    assert_eq!(
        owner.snapshot_model_only().native_effects,
        vec![
            R45NativeEffectV1::Reservation { packet_count: 3 },
            R45NativeEffectV1::ClaimAttempt { packet_count: 3 },
            R45NativeEffectV1::BarrierBody { barrier_index: 0 },
            R45NativeEffectV1::BarrierBody { barrier_index: 1 },
            R45NativeEffectV1::FinalDispatchBody,
            R45NativeEffectV1::BarrierHeader { barrier_index: 0 },
            R45NativeEffectV1::BarrierHeader { barrier_index: 1 },
            R45NativeEffectV1::FinalDispatchHeader,
            R45NativeEffectV1::Doorbell,
        ]
    );
    assert_eq!(owner.snapshot_model_only().completion_loads, 0);
}

#[test]
fn ring_occupancy_is_only_retryable_outcome_and_has_no_effect() {
    let mut owner = publisher();
    let prepared = prepared(&mut owner, vec![source(1, 2, 101, 201, 1)], 2);
    let before = owner.snapshot_model_only();
    let retryable = match owner
        .publish_native_model_only(prepared, R45NativeFaultV1::RingOccupied)
        .unwrap_err()
    {
        R45PublicationFailureV1::Retryable(retryable) => retryable,
        other => panic!("expected retryable custody, got {other:?}"),
    };
    assert_eq!(owner.snapshot_model_only(), before);
    assert_eq!(retryable.source_count_model_only(), 1);
}

#[test]
fn preclaim_and_every_claim_or_later_fault_are_terminal_with_exact_custody() {
    let faults = [
        R45NativeFaultV1::PreClaimInvariant,
        R45NativeFaultV1::ClaimAttempt,
        R45NativeFaultV1::BarrierBody(0),
        R45NativeFaultV1::FinalDispatchBody,
        R45NativeFaultV1::BarrierHeader(0),
        R45NativeFaultV1::FinalDispatchHeader,
        R45NativeFaultV1::Doorbell,
    ];
    for fault in faults {
        let mut owner = publisher();
        let original_record = source_record(1, 2, 101, 201, 1);
        let prepared = prepared(&mut owner, vec![source(1, 2, 101, 201, 1)], 2);
        let terminal = match owner
            .publish_native_model_only(prepared, fault)
            .unwrap_err()
        {
            R45PublicationFailureV1::Terminal(terminal) => terminal,
            other => panic!("fault became retryable: {other:?}"),
        };
        assert_eq!(terminal.fault, fault);
        assert_eq!(
            terminal.claim_attempted,
            fault != R45NativeFaultV1::PreClaimInvariant
        );
        assert_eq!(terminal.acceptance.owner_occurrence_model_only(), 501);
        assert_eq!(terminal.acceptance.session_occurrence_model_only(), SESSION);
        assert_eq!(terminal.acceptance.epoch_model_only(), 2);
        assert_eq!(
            terminal.target.target_model_only(),
            target(2).target_model_only()
        );
        assert_eq!(terminal.readers.len(), 1);
        assert_eq!(terminal.readers[0].record_model_only(), original_record);
        assert_eq!(
            owner.snapshot_model_only().phase,
            R45PublisherPhaseV1::Poisoned
        );
        assert_eq!(owner.snapshot_model_only().completion_loads, 0);
        assert_eq!(
            owner.reserve_acceptance_epoch_model_only(),
            Err(R45DependencyPublisherErrorV1::Poisoned)
        );
    }
}

#[test]
fn malformed_barrier_fault_coordinates_fail_closed_before_native_effect() {
    for fault in [
        R45NativeFaultV1::BarrierBody(1),
        R45NativeFaultV1::BarrierHeader(1),
    ] {
        let mut owner = publisher();
        let prepared = prepared(&mut owner, vec![source(1, 2, 101, 201, 1)], 2);
        let terminal = match owner
            .publish_native_model_only(prepared, fault)
            .unwrap_err()
        {
            R45PublicationFailureV1::Terminal(terminal) => terminal,
            other => panic!("malformed fault became reusable: {other:?}"),
        };
        assert_eq!(terminal.fault, fault);
        assert!(!terminal.claim_attempted);
        assert_eq!(terminal.readers.len(), 1);
        assert!(owner.snapshot_model_only().native_effects.is_empty());
        assert_eq!(
            owner.snapshot_model_only().phase,
            R45PublisherPhaseV1::Poisoned
        );
    }
}

#[test]
fn multi_owner_retry_rollback_preflights_then_releases_reverse_and_returns_original_order() {
    let sources = vec![
        source(1, 4, 101, 201, 1),
        source(2, 4, 102, 202, 2),
        source(3, 4, 101, 201, 3),
    ];
    let expected_sources = vec![
        source_record(1, 4, 101, 201, 1),
        source_record(2, 4, 102, 202, 2),
        source_record(3, 4, 101, 201, 3),
    ];
    let mut owner = publisher();
    let retryable = retryable(&mut owner, sources, 4);
    let target_bundle = target(4);
    let mut first = R45SourceArenaV1::new_model_only(
        R45ArenaIdentityV1 {
            queue_occurrence: 101,
            signal_mapping: 201,
        },
        vec![
            live_reader(1, 4, 101, 201, 1),
            live_reader(3, 4, 101, 201, 3),
        ],
    )
    .unwrap();
    let mut second = R45SourceArenaV1::new_model_only(
        R45ArenaIdentityV1 {
            queue_occurrence: 102,
            signal_mapping: 202,
        },
        vec![live_reader(2, 4, 102, 202, 2)],
    )
    .unwrap();
    let mut target_owner = R45TargetArenaV1::from_bundle_model_only(&target_bundle);
    let mut source_owners = [&mut second, &mut first];
    let cancelled = owner
        .rollback_retryable_model_only(retryable, &mut source_owners, &mut target_owner)
        .unwrap();
    assert_eq!(cancelled.released_sources, expected_sources);
    assert_eq!(
        cancelled.returned_source_events,
        vec![10_001, 10_002, 10_003]
    );
    assert_eq!(
        first.snapshot_model_only().released_lease_ids,
        vec![20_003, 20_001]
    );
    assert_eq!(
        second.snapshot_model_only().released_lease_ids,
        vec![20_002]
    );
    assert_eq!(target_owner.snapshot_model_only().cancellations, 1);
    assert!(!target_owner.validates_bundle_model_only(&cancelled.target));
    assert_eq!(owner.snapshot_model_only().phase, R45PublisherPhaseV1::Idle);
    assert_eq!(owner.snapshot_model_only().completion_loads, 0);
    let before_replay = owner.snapshot_model_only();
    let replay = owner
        .begin_target_use_model_only(
            cancelled.acceptance,
            cancelled.target,
            vec![source(1, 4, 101, 201, 1)],
        )
        .unwrap_err();
    assert_eq!(
        replay.error,
        R45DependencyPublisherErrorV1::AcceptanceSubstitution
    );
    assert_eq!(owner.snapshot_model_only(), before_replay);
}

#[test]
fn hostile_owner_routes_and_target_currentness_have_zero_arena_mutation() {
    enum Hostile {
        Missing,
        Substituted,
        Duplicate,
        DuplicateEntry,
        Event,
        StaleLease,
        Target,
    }
    for hostile in [
        Hostile::Missing,
        Hostile::Substituted,
        Hostile::Duplicate,
        Hostile::DuplicateEntry,
        Hostile::Event,
        Hostile::StaleLease,
        Hostile::Target,
    ] {
        let first_reader = if matches!(hostile, Hostile::Event) {
            source(1, 3, 101, 201, 1).substitute_event_model_only(99_999)
        } else {
            source(1, 3, 101, 201, 1)
        };
        let sources = vec![first_reader, source(2, 3, 102, 202, 2)];
        let mut owner = publisher();
        let retryable = retryable(&mut owner, sources, 3);
        let target_bundle = target(3);
        let mut first = R45SourceArenaV1::new_model_only(
            R45ArenaIdentityV1 {
                queue_occurrence: 101,
                signal_mapping: 201,
            },
            vec![live_reader(1, 3, 101, 201, 1)],
        )
        .unwrap();
        let mut second = R45SourceArenaV1::new_model_only(
            R45ArenaIdentityV1 {
                queue_occurrence: 102,
                signal_mapping: 202,
            },
            if matches!(hostile, Hostile::StaleLease) {
                Vec::new()
            } else {
                vec![live_reader(2, 3, 102, 202, 2)]
            },
        )
        .unwrap();
        let mut decoy = R45SourceArenaV1::new_model_only(
            R45ArenaIdentityV1 {
                queue_occurrence: 777,
                signal_mapping: 778,
            },
            vec![R45LiveReaderRecordV1::from_parts_model_only(
                source_record(2, 3, 777, 778, 2),
                20_002,
            )],
        )
        .unwrap();
        let mut duplicate = R45SourceArenaV1::new_model_only(
            R45ArenaIdentityV1 {
                queue_occurrence: 101,
                signal_mapping: 201,
            },
            vec![R45LiveReaderRecordV1::from_parts_model_only(
                source_record(1, 3, 101, 201, 1),
                99_001,
            )],
        )
        .unwrap();
        let mut target_owner = R45TargetArenaV1::from_bundle_model_only(&target_bundle);
        if matches!(hostile, Hostile::DuplicateEntry) {
            first.duplicate_live_reader_model_only(0);
        }
        if matches!(hostile, Hostile::Target) {
            target_owner.substitute_event_model_only(999);
        }
        let first_before = first.snapshot_model_only();
        let second_before = second.snapshot_model_only();
        let decoy_before = decoy.snapshot_model_only();
        let duplicate_before = duplicate.snapshot_model_only();
        let target_before = target_owner.snapshot_model_only();
        let result = match hostile {
            Hostile::Missing => {
                let mut roster = [&mut first];
                owner.rollback_retryable_model_only(retryable, &mut roster, &mut target_owner)
            }
            Hostile::Substituted => {
                let mut roster = [&mut first, &mut decoy];
                owner.rollback_retryable_model_only(retryable, &mut roster, &mut target_owner)
            }
            Hostile::Duplicate => {
                let mut roster = [&mut first, &mut second, &mut duplicate];
                owner.rollback_retryable_model_only(retryable, &mut roster, &mut target_owner)
            }
            Hostile::DuplicateEntry | Hostile::Event | Hostile::StaleLease | Hostile::Target => {
                let mut roster = [&mut first, &mut second];
                owner.rollback_retryable_model_only(retryable, &mut roster, &mut target_owner)
            }
        };
        let failure = result.unwrap_err();
        assert!(failure.released_source_events.is_empty());
        assert_eq!(failure.retryable.source_count_model_only(), 2);
        assert_eq!(first.snapshot_model_only(), first_before);
        assert_eq!(second.snapshot_model_only(), second_before);
        assert_eq!(decoy.snapshot_model_only(), decoy_before);
        assert_eq!(duplicate.snapshot_model_only(), duplicate_before);
        assert_eq!(target_owner.snapshot_model_only(), target_before);
        assert_eq!(
            owner.snapshot_model_only().phase,
            R45PublisherPhaseV1::Poisoned
        );
    }
}

#[test]
fn source_arena_constructor_rejects_duplicate_live_reader_lease_or_event_identity() {
    let identity = R45ArenaIdentityV1 {
        queue_occurrence: 101,
        signal_mapping: 201,
    };
    let first = live_reader(1, 2, 101, 201, 1);
    let second = live_reader(2, 2, 101, 201, 2);
    for records in [
        vec![first, first],
        vec![
            first,
            R45LiveReaderRecordV1 {
                lease_id: first.lease_id,
                ..second
            },
        ],
        vec![
            first,
            R45LiveReaderRecordV1 {
                source: R45SourceRecordV1 {
                    event_id: first.source.event_id,
                    ..second.source
                },
                ..second
            },
        ],
    ] {
        assert_eq!(
            R45SourceArenaV1::new_model_only(identity, records).unwrap_err(),
            R45DependencyPublisherErrorV1::InvalidSourceArena
        );
    }
}
