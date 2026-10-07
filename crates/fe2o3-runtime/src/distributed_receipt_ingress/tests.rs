use super::*;
use ed25519_dalek::{Signer, SigningKey};
use fe2o3_runtime_model::{
    DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1, DISTRIBUTED_PUBLICATION_RECEIPT_DOMAIN_V1,
    DistributedExecutionPlanIdV1, DistributedMembershipIdV1, DistributedOperationIdV1,
    DistributedParticipantIdV1, DistributedPlacementPlanIdV1, DistributedRunIdV1,
    DistributedRuntimeInstanceIdV1, DistributedTargetDescriptionIdV1, IdentityDigestV1,
    ReportedDistributedPublicationV1, RuntimeArtifactIdV1, RuntimeModelIdV1,
    UntrustedDistributedOperationCoordinatesV1,
};
use std::{cell::RefCell, collections::VecDeque};

use DistributedPublicationContractErrorV1 as ContractError;
use DistributedReceiptIngressErrorV1 as Error;
use ModelDistributedReceiptDispositionV1 as Disposition;
use ReportedDistributedPublicationV1 as Outcome;

type EntropyReads = VecDeque<Result<(usize, u8), ()>>;

thread_local! {
    static ENTROPY_READS: RefCell<Option<EntropyReads>> = const { RefCell::new(None) };
}

// This entire module is cfg(test); production has no entropy injection API.
pub(super) fn entropy_read(bytes: &mut [u8]) -> Result<usize, Error> {
    ENTROPY_READS.with(|slot| {
        let mut slot = slot.borrow_mut();
        match slot.as_mut() {
            Some(reads) => {
                let (count, value) = reads
                    .pop_front()
                    .expect("unexpected additional entropy read")
                    .map_err(|_| Error::EntropyUnavailable)?;
                let filled = count.min(bytes.len());
                bytes[..filled].fill(value);
                Ok(count)
            }
            None => os_entropy(bytes),
        }
    })
}

fn with_entropy<T>(
    reads: impl IntoIterator<Item = Result<(usize, u8), ()>>,
    f: impl FnOnce() -> T,
) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ENTROPY_READS.with(|slot| *slot.borrow_mut() = None);
        }
    }
    ENTROPY_READS.with(|slot| {
        assert!(slot.borrow().is_none());
        *slot.borrow_mut() = Some(reads.into_iter().collect());
    });
    let _reset = Reset;
    let result = f();
    ENTROPY_READS.with(|slot| assert!(slot.borrow().as_ref().unwrap().is_empty()));
    result
}

fn signing_key() -> SigningKey {
    // Fixed test-only key material; not a deployed identity.
    SigningKey::from_bytes(&[17; 32])
}

fn peer_key(key: &SigningKey) -> DistributedReceiptPeerKeyV1 {
    DistributedReceiptPeerKeyV1::from_provisioned_bytes(key.verifying_key().to_bytes()).unwrap()
}

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

fn ingress_with_challenge(value: u8) -> DistributedReceiptIngressV1 {
    with_entropy([Ok((32, value))], || {
        DistributedReceiptIngressV1::open(peer_key(&signing_key()), binding()).unwrap()
    })
}

fn receipt(sequence: u64, outcome: Outcome) -> UntrustedDistributedPublicationReceiptV1 {
    UntrustedDistributedPublicationReceiptV1::new(binding(), sequence, outcome).unwrap()
}

fn signature(
    ingress: &DistributedReceiptIngressV1,
    bytes: &[u8; DISTRIBUTED_PUBLICATION_RECEIPT_BYTES_V1],
) -> [u8; 64] {
    signing_key()
        .sign(&ingress.signing_preimage(bytes))
        .to_bytes()
}

fn accept(
    ingress: &mut DistributedReceiptIngressV1,
    receipt: UntrustedDistributedPublicationReceiptV1,
) -> Result<OriginAuthenticatedDistributedReceiptV1, Error> {
    let bytes = receipt.canonical_description();
    let signature = signature(ingress, &bytes);
    ingress.accept_signed_receipt(&bytes, &signature)
}

fn assert_rejection_unchanged(
    ingress: &mut DistributedReceiptIngressV1,
    bytes: &[u8],
    signature: &[u8; 64],
    expected: Error,
) {
    let before = (ingress.last_receipt(), ingress.connection_interrupted());
    assert_eq!(
        ingress.accept_signed_receipt(bytes, signature),
        Err(expected)
    );
    assert_eq!(
        before,
        (ingress.last_receipt(), ingress.connection_interrupted())
    );
    assert_eq!(ingress.expected_binding(), binding());
}

#[test]
fn public_open_fails_closed_on_entropy_errors_and_invalid_counts() {
    for reads in [
        vec![Err(())],
        vec![Ok((0, 1))],
        vec![Ok((33, 1))],
        vec![Ok((8, 1)), Err(())],
    ] {
        with_entropy(reads, || {
            assert_eq!(
                DistributedReceiptIngressV1::open(peer_key(&signing_key()), binding()).err(),
                Some(Error::EntropyUnavailable)
            );
        });
    }
}

#[test]
fn public_open_uses_os_entropy_without_test_injection() {
    ENTROPY_READS.with(|slot| assert!(slot.borrow().is_none()));
    let key = peer_key(&signing_key());
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let mut ingress = DistributedReceiptIngressV1::open(key, binding()).unwrap();
        assert_eq!(ingress.peer_key(), key);
        assert_eq!(ingress.expected_binding(), binding());
        assert_eq!(ingress.last_receipt(), None);
        assert!(!ingress.connection_interrupted());
        assert_ne!(ingress.challenge().to_bytes(), [0; 32]);
        let published = receipt(1, Outcome::Published);
        let claim = accept(&mut ingress, published).unwrap();
        assert_eq!(claim.receipt(), published);
        assert_eq!(claim.challenge(), ingress.challenge());
        assert_eq!(claim.disposition(), Disposition::Recorded);
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        assert_eq!(
            DistributedReceiptIngressV1::open(key, binding()).err(),
            Some(Error::EntropyUnavailable),
        );
    }
    ENTROPY_READS.with(|slot| assert!(slot.borrow().is_none()));
}

#[test]
fn entropy_partial_reads_preserve_all_bytes_and_zero_attempts_are_bounded() {
    with_entropy([Ok((7, 1)), Ok((13, 2)), Ok((12, 3))], || {
        let ingress =
            DistributedReceiptIngressV1::open(peer_key(&signing_key()), binding()).unwrap();
        let challenge = ingress.challenge().to_bytes();
        assert_eq!(&challenge[..7], &[1; 7]);
        assert_eq!(&challenge[7..20], &[2; 13]);
        assert_eq!(&challenge[20..], &[3; 12]);
    });
    with_entropy([Ok((32, 0)); 4], || {
        assert_eq!(fresh_challenge(), Err(Error::EntropyUnavailable));
    });
    with_entropy([Ok((32, 0)), Ok((32, 0)), Ok((32, 0)), Ok((32, 9))], || {
        assert_eq!(fresh_challenge().unwrap().to_bytes(), [9; 32]);
    });
}

#[test]
fn weak_provisioned_keys_are_rejected() {
    let mut identity = [0; 32];
    identity[0] = 1;
    for bytes in [[0; 32], identity] {
        assert_eq!(
            DistributedReceiptPeerKeyV1::from_provisioned_bytes(bytes),
            Err(Error::InvalidPeerKey)
        );
    }
    let valid = peer_key(&signing_key());
    assert_eq!(valid.to_bytes(), signing_key().verifying_key().to_bytes());
}

#[test]
fn signature_preimage_has_independent_exact_fixed_layout() {
    let ingress = ingress_with_challenge(29);
    let bytes = receipt(1, Outcome::Published).canonical_description();
    let expected = [
        b"FE2O3/DISTRIBUTED-RECEIPT-ORIGIN/V1\0".as_slice(),
        signing_key().verifying_key().as_bytes().as_slice(),
        &[29; 32],
        &binding().canonical_description(),
        &bytes,
    ]
    .concat();
    assert_eq!(ingress.signing_preimage(&bytes).as_slice(), expected);
    assert_eq!(DISTRIBUTED_RECEIPT_ORIGIN_MESSAGE_BYTES_V1, expected.len());
    assert!(ingress.last_receipt().is_none());
}

#[test]
fn published_completed_and_all_outcome_tags_remain_peer_claims() {
    for outcome in [
        Outcome::DefinitelyNotPublished,
        Outcome::Published,
        Outcome::Completed,
        Outcome::FailedBeforePublication,
        Outcome::FailedMayStillExecute,
        Outcome::ParticipantLostWithUnknownPublication,
    ] {
        let mut ingress = ingress_with_challenge(29);
        let sequence = if outcome == Outcome::Completed {
            accept(&mut ingress, receipt(1, Outcome::Published)).unwrap();
            2
        } else {
            1
        };
        let original = receipt(sequence, outcome);
        let claim = accept(&mut ingress, original).unwrap();
        assert_eq!(claim.receipt(), original);
        assert_eq!(claim.peer_key(), ingress.peer_key());
        assert_eq!(claim.challenge(), ingress.challenge());
        assert_eq!(claim.disposition(), Disposition::Recorded);
        assert_eq!(claim.receipt().reported_outcome(), outcome);
        assert_eq!(ingress.last_receipt(), Some(original));
        assert_eq!(
            accept(&mut ingress, original).unwrap().disposition(),
            Disposition::ExactDuplicate
        );
    }
}

#[test]
fn signatures_bind_peer_key_domain_challenge_and_expected_operation() {
    let mut ingress = ingress_with_challenge(29);
    let bytes = receipt(1, Outcome::Published).canonical_description();
    let other_key = SigningKey::from_bytes(&[18; 32]);
    let wrong_key = other_key.sign(&ingress.signing_preimage(&bytes)).to_bytes();
    assert_rejection_unchanged(&mut ingress, &bytes, &wrong_key, Error::InvalidSignature);
    let mut message = ingress.signing_preimage(&bytes);
    message[0] ^= 1;
    let wrong_domain = signing_key().sign(&message).to_bytes();
    assert_rejection_unchanged(&mut ingress, &bytes, &wrong_domain, Error::InvalidSignature);
    let other_session = ingress_with_challenge(30);
    let wrong_challenge = signature(&other_session, &bytes);
    assert_rejection_unchanged(
        &mut ingress,
        &bytes,
        &wrong_challenge,
        Error::InvalidSignature,
    );
    let mut message = ingress.signing_preimage(&bytes);
    message[DISTRIBUTED_RECEIPT_ORIGIN_SIGNING_DOMAIN_V1.len()] ^= 1;
    let wrong_key_context = signing_key().sign(&message).to_bytes();
    assert_rejection_unchanged(
        &mut ingress,
        &bytes,
        &wrong_key_context,
        Error::InvalidSignature,
    );
    let mut message = ingress.signing_preimage(&bytes);
    message[DISTRIBUTED_RECEIPT_ORIGIN_SIGNING_DOMAIN_V1.len() + 64] ^= 1;
    let wrong_expected = signing_key().sign(&message).to_bytes();
    assert_rejection_unchanged(
        &mut ingress,
        &bytes,
        &wrong_expected,
        Error::InvalidSignature,
    );
}

#[test]
fn every_receipt_byte_and_signature_byte_is_authenticated() {
    let mut ingress = ingress_with_challenge(29);
    let bytes = receipt(1, Outcome::Published).canonical_description();
    let valid = signature(&ingress, &bytes);
    for index in 0..bytes.len() {
        let mut changed = bytes;
        changed[index] ^= 1;
        assert_rejection_unchanged(&mut ingress, &changed, &valid, Error::InvalidSignature);
    }
    for index in 0..valid.len() {
        let mut changed = valid;
        changed[index] ^= 1;
        assert_rejection_unchanged(&mut ingress, &bytes, &changed, Error::InvalidSignature);
    }
    let mut noncanonical = valid;
    noncanonical[32..].fill(255);
    assert_rejection_unchanged(&mut ingress, &bytes, &noncanonical, Error::InvalidSignature);
}

#[test]
fn all_lengths_except_exact_receipt_are_rejected_before_signature() {
    let mut ingress = ingress_with_challenge(29);
    let bytes = receipt(1, Outcome::Published).canonical_description();
    for length in 0..bytes.len() {
        assert_rejection_unchanged(&mut ingress, &bytes[..length], &[0; 64], Error::WrongLength);
    }
    let extended = [bytes.as_slice(), &[0]].concat();
    assert_rejection_unchanged(&mut ingress, &extended, &[0; 64], Error::WrongLength);
}

fn change_coordinate(
    mut c: UntrustedDistributedOperationCoordinatesV1,
    field: usize,
) -> UntrustedDistributedOperationCoordinatesV1 {
    match field {
        0 => c.runtime_instance = DistributedRuntimeInstanceIdV1::from_untrusted_digest(digest(99)),
        1 => c.participant = DistributedParticipantIdV1::from_untrusted_digest(digest(99)),
        2 => c.participant_incarnation += 1,
        3 => c.coordinator = DistributedParticipantIdV1::from_untrusted_digest(digest(99)),
        4 => c.coordinator_epoch += 1,
        5 => c.membership = DistributedMembershipIdV1::from_untrusted_digest(digest(99)),
        6 => c.membership_epoch += 1,
        7 => c.run = DistributedRunIdV1::from_untrusted_digest(digest(99)),
        8 => c.operation = DistributedOperationIdV1::from_untrusted_digest(digest(99)),
        9 => c.attempt += 1,
        10 => c.artifact = RuntimeArtifactIdV1::from_untrusted_digest(digest(99)),
        11 => c.execution_plan = DistributedExecutionPlanIdV1::from_untrusted_digest(digest(99)),
        12 => c.placement_plan = DistributedPlacementPlanIdV1::from_untrusted_digest(digest(99)),
        13 => c.target = DistributedTargetDescriptionIdV1::from_untrusted_digest(digest(99)),
        14 => c.runtime_model = RuntimeModelIdV1::from_untrusted_digest(digest(99)),
        _ => unreachable!(),
    }
    c
}

#[test]
fn authentic_peer_cannot_replace_any_expected_coordinate() {
    for field in 0..15 {
        let mut ingress = ingress_with_challenge(29);
        let changed = ModelDistributedOperationBindingV1::from_untrusted_coordinates(
            change_coordinate(coordinates(), field),
        )
        .unwrap();
        let bytes = UntrustedDistributedPublicationReceiptV1::new(changed, 1, Outcome::Published)
            .unwrap()
            .canonical_description();
        let signed = signature(&ingress, &bytes);
        assert_rejection_unchanged(
            &mut ingress,
            &bytes,
            &signed,
            Error::Receipt(ContractError::BindingMismatch),
        );
        accept(&mut ingress, receipt(1, Outcome::Published)).unwrap();
    }
}

#[test]
fn stale_incarnation_epochs_and_attempt_reject_without_replacement() {
    for field in [2, 4, 6, 9] {
        let mut c = coordinates();
        match field {
            2 => c.participant_incarnation -= 1,
            4 => c.coordinator_epoch -= 1,
            6 => c.membership_epoch -= 1,
            9 => c.attempt -= 1,
            _ => unreachable!(),
        }
        let stale = ModelDistributedOperationBindingV1::from_untrusted_coordinates(c).unwrap();
        let bytes = UntrustedDistributedPublicationReceiptV1::new(stale, 1, Outcome::Published)
            .unwrap()
            .canonical_description();
        let mut ingress = ingress_with_challenge(29);
        let signed = signature(&ingress, &bytes);
        assert_rejection_unchanged(
            &mut ingress,
            &bytes,
            &signed,
            Error::Receipt(ContractError::BindingMismatch),
        );
    }
}

#[test]
fn valid_signature_does_not_make_malformed_receipt_canonical() {
    let header = DISTRIBUTED_PUBLICATION_RECEIPT_DOMAIN_V1.len();
    let operation = header + 4;
    let op_header = operation + DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1.len();
    let sequence = operation + DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1;
    let trailer = sequence + 8;
    for (offset, value, error) in [
        (0, 0, ContractError::WrongDomain),
        (header, 2, ContractError::WrongSchema),
        (header + 2, 1, ContractError::NonzeroReserved),
        (header + 3, 1, ContractError::NonzeroReserved),
        (operation, 0, ContractError::WrongDomain),
        (op_header, 2, ContractError::WrongSchema),
        (op_header + 2, 1, ContractError::NonzeroReserved),
        (op_header + 3, 1, ContractError::NonzeroReserved),
        (sequence, 0, ContractError::ZeroSequence),
        (trailer, 0, ContractError::InvalidOutcome),
        (trailer, 7, ContractError::InvalidOutcome),
        (trailer + 1, 1, ContractError::NonzeroReserved),
        (trailer + 2, 1, ContractError::NonzeroReserved),
        (trailer + 3, 1, ContractError::NonzeroReserved),
    ] {
        let mut ingress = ingress_with_challenge(29);
        let mut bytes = receipt(1, Outcome::Published).canonical_description();
        bytes[offset] = value;
        let signed = signature(&ingress, &bytes);
        assert_rejection_unchanged(&mut ingress, &bytes, &signed, Error::Receipt(error));
        assert_rejection_unchanged(&mut ingress, &bytes, &[0; 64], Error::InvalidSignature);
    }
}

#[test]
fn authenticated_zero_identity_and_epoch_are_rejected_by_existing_decoder() {
    let coordinates_start = DISTRIBUTED_PUBLICATION_RECEIPT_DOMAIN_V1.len()
        + 4
        + DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1.len()
        + 4;
    for (offset, length, error) in [
        (coordinates_start, 32, ContractError::ZeroIdentity),
        (coordinates_start + 64, 8, ContractError::ZeroEpochOrAttempt),
    ] {
        let mut bytes = receipt(1, Outcome::Published).canonical_description();
        bytes[offset..offset + length].fill(0);
        let mut ingress = ingress_with_challenge(29);
        let signed = signature(&ingress, &bytes);
        assert_rejection_unchanged(&mut ingress, &bytes, &signed, Error::Receipt(error));
    }
}

#[test]
fn ordering_conflicts_and_terminal_outcomes_preserve_last_claim() {
    let mut ingress = ingress_with_challenge(29);
    for (sequence, outcome, error) in [
        (1, Outcome::Completed, ContractError::InvalidTransition),
        (2, Outcome::Published, ContractError::SequenceGap),
    ] {
        let bytes = receipt(sequence, outcome).canonical_description();
        let signed = signature(&ingress, &bytes);
        assert_rejection_unchanged(&mut ingress, &bytes, &signed, Error::Receipt(error));
    }
    accept(&mut ingress, receipt(1, Outcome::Published)).unwrap();
    for (sequence, outcome, error) in [
        (1, Outcome::Completed, ContractError::ConflictingDuplicate),
        (3, Outcome::Completed, ContractError::SequenceGap),
        (2, Outcome::Published, ContractError::InvalidTransition),
    ] {
        let bytes = receipt(sequence, outcome).canonical_description();
        let signed = signature(&ingress, &bytes);
        assert_rejection_unchanged(&mut ingress, &bytes, &signed, Error::Receipt(error));
    }
    accept(&mut ingress, receipt(2, Outcome::Completed)).unwrap();
    for (sequence, outcome, error) in [
        (1, Outcome::Published, ContractError::StaleReceipt),
        (3, Outcome::Completed, ContractError::InvalidTransition),
    ] {
        let bytes = receipt(sequence, outcome).canonical_description();
        let signed = signature(&ingress, &bytes);
        assert_rejection_unchanged(&mut ingress, &bytes, &signed, Error::Receipt(error));
    }
}

#[test]
fn loss_is_sticky_and_timeout_or_drop_never_synthesizes_a_claim() {
    let mut ingress = ingress_with_challenge(29);
    for event in [
        DistributedObservationEventV1::ObservationTimedOut,
        DistributedObservationEventV1::ObserverDropped,
    ] {
        ingress.observe(event);
        assert_eq!(ingress.last_receipt(), None);
        assert!(!ingress.connection_interrupted());
    }
    let published = receipt(1, Outcome::Published);
    accept(&mut ingress, published).unwrap();
    ingress.observe(DistributedObservationEventV1::ConnectionLost);
    assert!(ingress.connection_interrupted());
    assert_eq!(
        accept(&mut ingress, published).unwrap().disposition(),
        Disposition::ExactDuplicate
    );
    let bytes = receipt(2, Outcome::Completed).canonical_description();
    let signed = signature(&ingress, &bytes);
    assert_rejection_unchanged(
        &mut ingress,
        &bytes,
        &signed,
        Error::Receipt(ContractError::Interrupted),
    );
    for event in [
        DistributedObservationEventV1::ObservationTimedOut,
        DistributedObservationEventV1::ObserverDropped,
    ] {
        ingress.observe(event);
        assert!(ingress.connection_interrupted());
        assert_eq!(ingress.last_receipt(), Some(published));
    }
}

#[test]
fn new_session_rejects_old_signature_but_is_not_durable_deduplication() {
    let mut old = ingress_with_challenge(29);
    let bytes = receipt(1, Outcome::Published).canonical_description();
    let old_signature = signature(&old, &bytes);
    old.accept_signed_receipt(&bytes, &old_signature).unwrap();
    let mut new = ingress_with_challenge(30);
    assert_rejection_unchanged(&mut new, &bytes, &old_signature, Error::InvalidSignature);
    // A newly signed claim can be recorded in a new session: no persistent ledger exists.
    assert_eq!(
        accept(&mut new, receipt(1, Outcome::Published))
            .unwrap()
            .disposition(),
        Disposition::Recorded
    );
}

#[test]
fn authentic_claims_match_model_for_all_reachable_record_states() {
    let outcomes = [
        Outcome::DefinitelyNotPublished,
        Outcome::Published,
        Outcome::Completed,
        Outcome::FailedBeforePublication,
        Outcome::FailedMayStillExecute,
        Outcome::ParticipantLostWithUnknownPublication,
    ];
    let states = [
        None,
        Some((1, Outcome::DefinitelyNotPublished)),
        Some((1, Outcome::Published)),
        Some((1, Outcome::FailedBeforePublication)),
        Some((1, Outcome::FailedMayStillExecute)),
        Some((1, Outcome::ParticipantLostWithUnknownPublication)),
        Some((2, Outcome::Completed)),
        Some((2, Outcome::FailedMayStillExecute)),
    ];
    for state in states {
        for interrupted in [false, true] {
            for outcome in outcomes {
                for sequence in [1, 2, 3, u64::MAX] {
                    let mut ingress = ingress_with_challenge(29);
                    let mut model = ModelDistributedPublicationRecordV1::new(binding());
                    if let Some((previous_sequence, previous_outcome)) = state {
                        if previous_sequence == 2 {
                            let published = receipt(1, Outcome::Published);
                            accept(&mut ingress, published).unwrap();
                            model.record_untrusted_receipt(published).unwrap();
                        }
                        let previous = receipt(previous_sequence, previous_outcome);
                        accept(&mut ingress, previous).unwrap();
                        model.record_untrusted_receipt(previous).unwrap();
                    }
                    if interrupted {
                        ingress.observe(DistributedObservationEventV1::ConnectionLost);
                        model.observe(DistributedObservationEventV1::ConnectionLost);
                    }
                    let candidate = receipt(sequence, outcome);
                    assert_eq!(
                        accept(&mut ingress, candidate).map(|claim| claim.disposition()),
                        model
                            .record_untrusted_receipt(candidate)
                            .map_err(Error::Receipt),
                    );
                    assert_eq!(ingress.last_receipt(), model.last_receipt());
                    assert_eq!(
                        ingress.connection_interrupted(),
                        model.connection_interrupted()
                    );
                }
            }
        }
    }
}
