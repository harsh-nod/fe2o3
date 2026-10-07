use super::*;
use DistributedPublicationContractErrorV1 as E;
use ModelDistributedReceiptDispositionV1 as D;
use ReportedDistributedPublicationV1 as P;
use alloc::vec::Vec;

const OUTCOMES: [P; 6] = [
    P::DefinitelyNotPublished,
    P::Published,
    P::Completed,
    P::FailedBeforePublication,
    P::FailedMayStillExecute,
    P::ParticipantLostWithUnknownPublication,
];

fn digest(value: u8) -> IdentityDigestV1 {
    IdentityDigestV1::from_untrusted_bytes([value; 32])
}

fn coordinates() -> UntrustedDistributedOperationCoordinatesV1 {
    UntrustedDistributedOperationCoordinatesV1 {
        runtime_instance: DistributedRuntimeInstanceIdV1::from_untrusted_digest(digest(1)),
        participant: DistributedParticipantIdV1::from_untrusted_digest(digest(2)),
        participant_incarnation: 0x0102030405060708,
        coordinator: DistributedParticipantIdV1::from_untrusted_digest(digest(3)),
        coordinator_epoch: 0x1112131415161718,
        membership: DistributedMembershipIdV1::from_untrusted_digest(digest(4)),
        membership_epoch: 0x2122232425262728,
        run: DistributedRunIdV1::from_untrusted_digest(digest(5)),
        operation: DistributedOperationIdV1::from_untrusted_digest(digest(6)),
        attempt: 0x3132333435363738,
        artifact: RuntimeArtifactIdV1::from_untrusted_digest(digest(7)),
        execution_plan: DistributedExecutionPlanIdV1::from_untrusted_digest(digest(8)),
        placement_plan: DistributedPlacementPlanIdV1::from_untrusted_digest(digest(9)),
        target: DistributedTargetDescriptionIdV1::from_untrusted_digest(digest(10)),
        runtime_model: RuntimeModelIdV1::from_untrusted_digest(digest(11)),
    }
}

fn binding() -> ModelDistributedOperationBindingV1 {
    ModelDistributedOperationBindingV1::from_untrusted_coordinates(coordinates()).unwrap()
}

fn change_coordinate(
    mut c: UntrustedDistributedOperationCoordinatesV1,
    field: usize,
    value: u8,
) -> UntrustedDistributedOperationCoordinatesV1 {
    match field {
        0 => {
            c.runtime_instance =
                DistributedRuntimeInstanceIdV1::from_untrusted_digest(digest(value))
        }
        1 => c.participant = DistributedParticipantIdV1::from_untrusted_digest(digest(value)),
        2 => c.participant_incarnation = u64::from(value),
        3 => c.coordinator = DistributedParticipantIdV1::from_untrusted_digest(digest(value)),
        4 => c.coordinator_epoch = u64::from(value),
        5 => c.membership = DistributedMembershipIdV1::from_untrusted_digest(digest(value)),
        6 => c.membership_epoch = u64::from(value),
        7 => c.run = DistributedRunIdV1::from_untrusted_digest(digest(value)),
        8 => c.operation = DistributedOperationIdV1::from_untrusted_digest(digest(value)),
        9 => c.attempt = u64::from(value),
        10 => c.artifact = RuntimeArtifactIdV1::from_untrusted_digest(digest(value)),
        11 => c.execution_plan = DistributedExecutionPlanIdV1::from_untrusted_digest(digest(value)),
        12 => c.placement_plan = DistributedPlacementPlanIdV1::from_untrusted_digest(digest(value)),
        13 => c.target = DistributedTargetDescriptionIdV1::from_untrusted_digest(digest(value)),
        14 => c.runtime_model = RuntimeModelIdV1::from_untrusted_digest(digest(value)),
        _ => unreachable!(),
    }
    c
}

fn receipt(sequence: u64, outcome: P) -> UntrustedDistributedPublicationReceiptV1 {
    UntrustedDistributedPublicationReceiptV1::new(binding(), sequence, outcome).unwrap()
}

fn duplicate_record(
    record: &ModelDistributedPublicationRecordV1,
) -> ModelDistributedPublicationRecordV1 {
    ModelDistributedPublicationRecordV1 {
        binding: record.binding,
        last: record.last,
        interrupted: record.interrupted,
    }
}

// Every reachable receipt state, before or after the independent interruption bit.
const STATES: [Option<(u64, P)>; 8] = [
    None,
    Some((1, P::DefinitelyNotPublished)),
    Some((1, P::Published)),
    Some((1, P::FailedBeforePublication)),
    Some((1, P::FailedMayStillExecute)),
    Some((1, P::ParticipantLostWithUnknownPublication)),
    Some((2, P::Completed)),
    Some((2, P::FailedMayStillExecute)),
];

fn seeded(state: Option<(u64, P)>, interrupted: bool) -> ModelDistributedPublicationRecordV1 {
    let mut record = ModelDistributedPublicationRecordV1::new(binding());
    if let Some((sequence, outcome)) = state {
        if sequence == 2 {
            assert_eq!(
                record.record_untrusted_receipt(receipt(1, P::Published)),
                Ok(D::Recorded)
            );
        }
        assert_eq!(
            record.record_untrusted_receipt(receipt(sequence, outcome)),
            Ok(D::Recorded)
        );
    }
    if interrupted {
        record.observe(DistributedObservationEventV1::ConnectionLost);
    }
    record
}

#[test]
fn distributed_binding_rejects_each_zero_and_roundtrips_every_coordinate() {
    let original = binding();
    assert_eq!(original.authority_domain(), AuthorityDomainV1::ModelOnly);
    for field in 0..15 {
        let zero_coordinates = change_coordinate(coordinates(), field, 0);
        let zero = ModelDistributedOperationBindingV1::from_untrusted_coordinates(zero_coordinates);
        let expected = Err(if [2, 4, 6, 9].contains(&field) {
            E::ZeroEpochOrAttempt
        } else {
            E::ZeroIdentity
        });
        assert_eq!(zero, expected);
        let malformed = ModelDistributedOperationBindingV1 {
            coordinates: zero_coordinates,
        };
        assert_eq!(
            ModelDistributedOperationBindingV1::decode_untrusted_description(
                &malformed.canonical_description()
            ),
            expected
        );
        let changed = ModelDistributedOperationBindingV1::from_untrusted_coordinates(
            change_coordinate(coordinates(), field, 99),
        )
        .unwrap();
        assert_ne!(changed, original);
        assert_ne!(
            changed.canonical_description(),
            original.canonical_description()
        );
        assert_eq!(
            ModelDistributedOperationBindingV1::decode_untrusted_description(
                &changed.canonical_description()
            ),
            Ok(changed)
        );
    }
    let mut largest = coordinates();
    largest.participant_incarnation = u64::MAX;
    largest.coordinator_epoch = u64::MAX;
    largest.membership_epoch = u64::MAX;
    largest.attempt = u64::MAX;
    let largest = ModelDistributedOperationBindingV1::from_untrusted_coordinates(largest).unwrap();
    assert_eq!(
        ModelDistributedOperationBindingV1::decode_untrusted_description(
            &largest.canonical_description()
        ),
        Ok(largest)
    );
}

#[test]
fn distributed_canonical_vectors_have_exact_domains_order_and_endianness() {
    let mut expected = Vec::from(&b"FE2O3/DISTRIBUTED-OPERATION-DESCRIPTION/V1\0"[..]);
    expected.extend_from_slice(&[1, 0, 0, 0]);
    expected.extend_from_slice(&[1; 32]);
    expected.extend_from_slice(&[2; 32]);
    expected.extend_from_slice(&[8, 7, 6, 5, 4, 3, 2, 1]);
    expected.extend_from_slice(&[3; 32]);
    expected.extend_from_slice(&[24, 23, 22, 21, 20, 19, 18, 17]);
    expected.extend_from_slice(&[4; 32]);
    expected.extend_from_slice(&[40, 39, 38, 37, 36, 35, 34, 33]);
    expected.extend_from_slice(&[5; 32]);
    expected.extend_from_slice(&[6; 32]);
    expected.extend_from_slice(&[56, 55, 54, 53, 52, 51, 50, 49]);
    for value in 7..=11 {
        expected.extend_from_slice(&[value; 32]);
    }
    assert_eq!(
        binding().canonical_description().as_slice(),
        expected.as_slice()
    );
    assert_eq!(expected.len(), DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1);
    for outcome in OUTCOMES {
        for sequence in [1, 2, u64::MAX] {
            let mut wire = Vec::from(&b"FE2O3/DISTRIBUTED-PUBLICATION-RECEIPT/V1\0"[..]);
            wire.extend_from_slice(&[1, 0, 0, 0]);
            wire.extend_from_slice(&expected);
            wire.extend_from_slice(&sequence.to_le_bytes());
            wire.extend_from_slice(&[outcome as u8, 0, 0, 0]);
            let value = receipt(sequence, outcome);
            assert_eq!(value.canonical_description().as_slice(), wire.as_slice());
            assert_eq!(wire.len(), DISTRIBUTED_PUBLICATION_RECEIPT_BYTES_V1);
            assert_eq!(
                UntrustedDistributedPublicationReceiptV1::decode_untrusted_description(&wire),
                Ok(value)
            );
        }
    }
}

#[test]
fn distributed_wire_refuses_all_truncations_extensions_headers_and_tags() {
    let binding_wire = binding().canonical_description();
    let receipt_wire = receipt(1, P::Published).canonical_description();
    for length in 0..binding_wire.len() {
        assert_eq!(
            ModelDistributedOperationBindingV1::decode_untrusted_description(
                &binding_wire[..length]
            ),
            Err(E::WrongLength)
        );
    }
    for length in 0..receipt_wire.len() {
        assert_eq!(
            UntrustedDistributedPublicationReceiptV1::decode_untrusted_description(
                &receipt_wire[..length]
            ),
            Err(E::WrongLength)
        );
    }
    let mut extended = binding_wire.to_vec();
    extended.push(0);
    assert_eq!(
        ModelDistributedOperationBindingV1::decode_untrusted_description(&extended),
        Err(E::WrongLength)
    );
    let mut extended = receipt_wire.to_vec();
    extended.push(0);
    assert_eq!(
        UntrustedDistributedPublicationReceiptV1::decode_untrusted_description(&extended),
        Err(E::WrongLength)
    );
    for (start, domain) in [
        (0, DISTRIBUTED_PUBLICATION_RECEIPT_DOMAIN_V1),
        (
            DISTRIBUTED_PUBLICATION_RECEIPT_DOMAIN_V1.len() + 4,
            DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1,
        ),
    ] {
        for offset in 0..domain.len() + 4 {
            let mut changed = receipt_wire;
            changed[start + offset] ^= 1;
            let error = if offset < domain.len() {
                E::WrongDomain
            } else if offset < domain.len() + 2 {
                E::WrongSchema
            } else {
                E::NonzeroReserved
            };
            assert_eq!(
                UntrustedDistributedPublicationReceiptV1::decode_untrusted_description(&changed),
                Err(error)
            );
        }
    }
    for tag in 0..=u8::MAX {
        let mut changed = receipt_wire;
        changed[receipt_wire.len() - 4] = tag;
        let result =
            UntrustedDistributedPublicationReceiptV1::decode_untrusted_description(&changed);
        if (1..=6).contains(&tag) {
            assert_eq!(result.unwrap().reported_outcome() as u8, tag);
        } else {
            assert_eq!(result, Err(E::InvalidOutcome));
        }
    }
    for offset in receipt_wire.len() - 3..receipt_wire.len() {
        let mut changed = receipt_wire;
        changed[offset] = 1;
        assert_eq!(
            UntrustedDistributedPublicationReceiptV1::decode_untrusted_description(&changed),
            Err(E::NonzeroReserved)
        );
    }
    let mut zero = receipt_wire;
    let sequence = receipt_wire.len() - 12;
    zero[sequence..sequence + 8].fill(0);
    assert_eq!(
        UntrustedDistributedPublicationReceiptV1::decode_untrusted_description(&zero),
        Err(E::ZeroSequence)
    );
    assert_eq!(
        UntrustedDistributedPublicationReceiptV1::new(binding(), 0, P::Published),
        Err(E::ZeroSequence)
    );
}

#[test]
fn distributed_successful_decode_is_canonical_after_each_single_byte_mutation() {
    let original = receipt(1, P::Published).canonical_description();
    for offset in 0..original.len() {
        for replacement in [0, 1, 127, 255] {
            let mut changed = original;
            changed[offset] = replacement;
            if let Ok(decoded) =
                UntrustedDistributedPublicationReceiptV1::decode_untrusted_description(&changed)
            {
                assert_eq!(decoded.canonical_description(), changed);
                assert_eq!(
                    decoded.binding().authority_domain(),
                    AuthorityDomainV1::ModelOnly
                );
            }
        }
    }
}

fn replace_digest(
    mut c: UntrustedDistributedOperationCoordinatesV1,
    field: usize,
    bytes: [u8; 32],
) -> UntrustedDistributedOperationCoordinatesV1 {
    let value = IdentityDigestV1::from_untrusted_bytes(bytes);
    match field {
        0 => c.runtime_instance = DistributedRuntimeInstanceIdV1::from_untrusted_digest(value),
        1 => c.participant = DistributedParticipantIdV1::from_untrusted_digest(value),
        2 => c.coordinator = DistributedParticipantIdV1::from_untrusted_digest(value),
        3 => c.membership = DistributedMembershipIdV1::from_untrusted_digest(value),
        4 => c.run = DistributedRunIdV1::from_untrusted_digest(value),
        5 => c.operation = DistributedOperationIdV1::from_untrusted_digest(value),
        6 => c.artifact = RuntimeArtifactIdV1::from_untrusted_digest(value),
        7 => c.execution_plan = DistributedExecutionPlanIdV1::from_untrusted_digest(value),
        8 => c.placement_plan = DistributedPlacementPlanIdV1::from_untrusted_digest(value),
        9 => c.target = DistributedTargetDescriptionIdV1::from_untrusted_digest(value),
        10 => c.runtime_model = RuntimeModelIdV1::from_untrusted_digest(value),
        _ => unreachable!(),
    }
    c
}

#[test]
fn distributed_construction_accepts_each_full_digest_single_bit_without_substitution() {
    let mut cases = 0;
    for field in 0..11 {
        for byte in 0..32 {
            for bit in 0..8 {
                let mut payload = [0; 32];
                payload[byte] = 1 << bit;
                let c = replace_digest(coordinates(), field, payload);
                let value =
                    ModelDistributedOperationBindingV1::from_untrusted_coordinates(c).unwrap();
                assert_eq!(value.coordinates(), c);
                assert_eq!(value.authority_domain(), AuthorityDomainV1::ModelOnly);
                assert_eq!(
                    ModelDistributedOperationBindingV1::decode_untrusted_description(
                        &value.canonical_description()
                    ),
                    Ok(value),
                );
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 2816);
}

#[test]
fn distributed_construction_zero_identity_precedes_every_zero_integer() {
    for identity in 0..11 {
        for integer in [2, 4, 6, 9] {
            let c = replace_digest(
                change_coordinate(coordinates(), integer, 0),
                identity,
                [0; 32],
            );
            assert_eq!(
                ModelDistributedOperationBindingV1::from_untrusted_coordinates(c),
                Err(E::ZeroIdentity),
            );
        }
    }
    for mask in 1..16 {
        let mut c = coordinates();
        for (bit, field) in [2, 4, 6, 9].into_iter().enumerate() {
            if mask & (1 << bit) != 0 {
                c = change_coordinate(c, field, 0);
            }
        }
        assert_eq!(
            ModelDistributedOperationBindingV1::from_untrusted_coordinates(c),
            Err(E::ZeroEpochOrAttempt),
        );
    }
}

#[test]
fn distributed_construction_receipt_preserves_inputs_without_lifecycle_validation() {
    let malformed = ModelDistributedOperationBindingV1 {
        coordinates: change_coordinate(coordinates(), 0, 0),
    };
    for value in [binding(), malformed] {
        for sequence in [0, 1, 2, u64::MAX] {
            for outcome in OUTCOMES {
                let result =
                    UntrustedDistributedPublicationReceiptV1::new(value, sequence, outcome);
                if sequence == 0 {
                    assert_eq!(result, Err(E::ZeroSequence));
                } else {
                    let result = result.unwrap();
                    assert_eq!(result.binding(), value);
                    assert_eq!(result.sequence(), sequence);
                    assert_eq!(result.reported_outcome(), outcome);
                }
            }
        }
    }
    let completed =
        UntrustedDistributedPublicationReceiptV1::new(binding(), 1, P::Completed).unwrap();
    let mut record = ModelDistributedPublicationRecordV1::new(binding());
    assert_eq!(
        record.record_untrusted_receipt(completed),
        Err(E::InvalidTransition)
    );
    assert_eq!(record.last_receipt(), None);
}

#[test]
fn distributed_construction_trailer_matrix_and_nested_decode_precedence() {
    let original = receipt(1, P::Published).canonical_description();
    for tag in 0..=u8::MAX {
        for reserved_bit in 0..25 {
            let mut trailer = [tag, 0, 0, 0];
            if reserved_bit != 0 {
                trailer[1 + (reserved_bit - 1) / 8] = 1 << ((reserved_bit - 1) % 8);
            }
            let expected = if reserved_bit != 0 {
                Err(E::NonzeroReserved)
            } else if (1..=6).contains(&tag) {
                Ok(OUTCOMES[usize::from(tag - 1)])
            } else {
                Err(E::InvalidOutcome)
            };
            assert_eq!(decode_outcome_trailer(trailer), expected);
            let mut wire = original;
            let offset = wire.len() - 4;
            wire[offset..].copy_from_slice(&trailer);
            assert_eq!(
                UntrustedDistributedPublicationReceiptV1::decode_untrusted_description(&wire)
                    .map(|value| value.reported_outcome()),
                expected,
            );
        }
    }
    let mut wire = original;
    let sequence = wire.len() - 12;
    wire[sequence..sequence + 8].fill(0);
    let trailer = wire.len() - 4;
    wire[trailer] = 0;
    wire[trailer + 1] = 1;
    assert_eq!(
        UntrustedDistributedPublicationReceiptV1::decode_untrusted_description(&wire),
        Err(E::NonzeroReserved)
    );
    wire[trailer + 1] = 0;
    assert_eq!(
        UntrustedDistributedPublicationReceiptV1::decode_untrusted_description(&wire),
        Err(E::InvalidOutcome)
    );
    wire[trailer] = 2;
    assert_eq!(
        UntrustedDistributedPublicationReceiptV1::decode_untrusted_description(&wire),
        Err(E::ZeroSequence)
    );
    let first_digest = DISTRIBUTED_PUBLICATION_RECEIPT_DOMAIN_V1.len()
        + 4
        + DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1.len()
        + 4;
    wire[first_digest..first_digest + 32].fill(0);
    wire[trailer + 1] = 1;
    assert_eq!(
        UntrustedDistributedPublicationReceiptV1::decode_untrusted_description(&wire),
        Err(E::ZeroIdentity)
    );
}

fn expected(state: Option<(u64, P)>, interrupted: bool, sequence: u64, outcome: P) -> Result<D, E> {
    if let Some((last_sequence, last_outcome)) = state {
        if sequence == last_sequence {
            return if outcome == last_outcome {
                Ok(D::ExactDuplicate)
            } else {
                Err(E::ConflictingDuplicate)
            };
        }
        if sequence < last_sequence {
            return Err(E::StaleReceipt);
        }
    }
    if interrupted {
        return Err(E::Interrupted);
    }
    let expected_sequence = state.map_or(1, |(sequence, _)| sequence + 1);
    if sequence != expected_sequence {
        return Err(E::SequenceGap);
    }
    // Independent numeric oracle: initial tags 1,2,4,5,6; only 2 -> 3 or 5.
    let accepted = match state.map(|(_, outcome)| outcome as u8) {
        None => [1, 2, 4, 5, 6].contains(&(outcome as u8)),
        Some(2) => [3, 5].contains(&(outcome as u8)),
        _ => false,
    };
    if accepted {
        Ok(D::Recorded)
    } else {
        Err(E::InvalidTransition)
    }
}

#[test]
fn distributed_exhaustive_receipt_matrix_is_failure_atomic_and_idempotent() {
    let mut cases = 0;
    for state in STATES {
        for interrupted in [false, true] {
            for sequence in [1, 2, 3, u64::MAX] {
                for outcome in OUTCOMES {
                    let mut record = seeded(state, interrupted);
                    let before = duplicate_record(&record);
                    let incoming = receipt(sequence, outcome);
                    let result = record.record_untrusted_receipt(incoming);
                    assert_eq!(result, expected(state, interrupted, sequence, outcome));
                    if result == Ok(D::Recorded) {
                        assert_eq!(record.last_receipt(), Some(incoming));
                        assert_eq!(record.binding(), before.binding());
                        assert_eq!(
                            record.connection_interrupted(),
                            before.connection_interrupted()
                        );
                    } else {
                        assert_eq!(record, before);
                    }
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 384);
}

#[test]
fn distributed_every_scope_substitution_rejects_before_receipt_or_interruption_rules() {
    for field in 0..15 {
        let other = ModelDistributedOperationBindingV1::from_untrusted_coordinates(
            change_coordinate(coordinates(), field, 99),
        )
        .unwrap();
        for state in STATES {
            for interrupted in [false, true] {
                for sequence in [1, 2, u64::MAX] {
                    for outcome in OUTCOMES {
                        let mut record = seeded(state, interrupted);
                        let before = duplicate_record(&record);
                        let incoming =
                            UntrustedDistributedPublicationReceiptV1::new(other, sequence, outcome)
                                .unwrap();
                        assert_eq!(
                            record.record_untrusted_receipt(incoming),
                            Err(E::BindingMismatch)
                        );
                        assert_eq!(record, before);
                    }
                }
            }
        }
    }
}

#[test]
fn distributed_observation_events_never_fabricate_receipts_or_reopen_admission() {
    for state in STATES {
        let mut record = seeded(state, false);
        assert_eq!(record.authority_domain(), AuthorityDomainV1::ModelOnly);
        let before = duplicate_record(&record);
        for event in [
            DistributedObservationEventV1::ObservationTimedOut,
            DistributedObservationEventV1::ObserverDropped,
        ] {
            record.observe(event);
            assert_eq!(record, before);
        }
        record.observe(DistributedObservationEventV1::ConnectionLost);
        assert!(record.connection_interrupted());
        assert_eq!(record.last_receipt(), before.last_receipt());
        let interrupted = duplicate_record(&record);
        for event in [
            DistributedObservationEventV1::ConnectionLost,
            DistributedObservationEventV1::ObservationTimedOut,
            DistributedObservationEventV1::ObserverDropped,
        ] {
            record.observe(event);
            assert_eq!(record, interrupted);
        }
        if let Some(last) = record.last_receipt() {
            assert_eq!(record.record_untrusted_receipt(last), Ok(D::ExactDuplicate));
            assert_eq!(record, interrupted);
        }
    }
}

#[test]
fn distributed_interrupted_exact_duplicate_is_not_conflict_or_new_receipt_permission() {
    let mut record = seeded(Some((1, P::Published)), true);
    let before = duplicate_record(&record);
    assert_eq!(
        record.record_untrusted_receipt(receipt(1, P::Published)),
        Ok(D::ExactDuplicate)
    );
    assert_eq!(record, before);
    assert_eq!(
        record.record_untrusted_receipt(receipt(1, P::Completed)),
        Err(E::ConflictingDuplicate)
    );
    assert_eq!(record, before);
    assert_eq!(
        record.record_untrusted_receipt(receipt(2, P::Completed)),
        Err(E::Interrupted)
    );
    assert_eq!(record, before);
    let foreign = ModelDistributedOperationBindingV1::from_untrusted_coordinates(
        change_coordinate(coordinates(), 4, 99),
    )
    .unwrap();
    let foreign_duplicate =
        UntrustedDistributedPublicationReceiptV1::new(foreign, 1, P::Published).unwrap();
    assert_eq!(
        record.record_untrusted_receipt(foreign_duplicate),
        Err(E::BindingMismatch)
    );
    assert_eq!(record, before);
}

#[test]
fn distributed_short_traces_keep_terminal_records_and_unknown_outcomes() {
    fn visit(record: &ModelDistributedPublicationRecordV1, changes: u8, remaining: usize) {
        assert!(changes <= 2);
        if let Some(last) = record.last_receipt() {
            assert!((1..=2).contains(&last.sequence()));
            if last.sequence() == 2 {
                assert!(matches!(
                    last.reported_outcome(),
                    P::Completed | P::FailedMayStillExecute
                ));
            }
        }
        if remaining == 0 {
            return;
        }
        for action in 0..15 {
            let mut next = duplicate_record(record);
            let mut next_changes = changes;
            if action < 12 {
                let outcome = OUTCOMES[action % 6];
                let sequence = if action < 6 { 1 } else { 2 };
                if next.record_untrusted_receipt(receipt(sequence, outcome)) == Ok(D::Recorded) {
                    next_changes += 1;
                }
            } else {
                next.observe(match action {
                    12 => DistributedObservationEventV1::ConnectionLost,
                    13 => DistributedObservationEventV1::ObservationTimedOut,
                    _ => DistributedObservationEventV1::ObserverDropped,
                });
            }
            assert_eq!(next.binding(), record.binding());
            if record.connection_interrupted() {
                assert!(next.connection_interrupted());
                assert_eq!(next.last_receipt(), record.last_receipt());
            }
            if let Some(last) = record.last_receipt() {
                assert!(next.last_receipt().is_some());
                if last.reported_outcome() != P::Published {
                    assert_eq!(next.last_receipt(), Some(last));
                }
            }
            visit(&next, next_changes, remaining - 1);
        }
    }
    visit(&ModelDistributedPublicationRecordV1::new(binding()), 0, 4);
}
