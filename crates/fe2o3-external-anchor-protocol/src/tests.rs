use ed25519_dalek::{Signer, SigningKey};

use crate::{
    ANCHOR_CHALLENGE_WIRE_LEN_V1, ANCHOR_OBSERVATION_SIGNED_LEN_V1, ANCHOR_OBSERVATION_WIRE_LEN_V1,
    AnchorChallengeV1, AnchorDecisionV1, AnchorPositionV1, AnchorProtocolErrorV1, AnchoredStateV1,
    CallerNonceV1, ChallengeKindV1, EXTERNAL_ANCHOR_AUTHORITY_V1,
    EXTERNAL_ANCHOR_PROTOCOL_VERSION_V1, HashChainHeadV1, PinnedAnchorKeyV1,
    PreparedAnchorAdvanceV1, TransactionDigestV1, UnsignedAnchorObservationV1,
};

fn keys(seed: u8) -> (SigningKey, PinnedAnchorKeyV1) {
    let signing = SigningKey::from_bytes(&[seed; 32]);
    let pinned = PinnedAnchorKeyV1::from_bytes(signing.verifying_key().to_bytes()).unwrap();
    (signing, pinned)
}

fn prepare(
    sequence: u64,
    head: [u8; 32],
    transaction: [u8; 32],
    key: &PinnedAnchorKeyV1,
) -> PreparedAnchorAdvanceV1 {
    AnchoredStateV1::from_local_state(sequence, HashChainHeadV1::from_bytes(head))
        .prepare(TransactionDigestV1::from_bytes(transaction), key)
        .unwrap()
}

fn signed_observation(
    challenge: &AnchorChallengeV1,
    position: AnchorPositionV1,
    signing: &SigningKey,
) -> [u8; ANCHOR_OBSERVATION_WIRE_LEN_V1] {
    let unsigned = UnsignedAnchorObservationV1::from_challenge(challenge, position);
    let signature = signing.sign(&unsigned.signing_bytes()).to_bytes();
    unsigned.attach_signature(signature)
}

#[test]
fn constants_and_authority_are_frozen() {
    assert_eq!(EXTERNAL_ANCHOR_PROTOCOL_VERSION_V1, 1);
    assert_eq!(EXTERNAL_ANCHOR_AUTHORITY_V1, "none");
    assert_eq!(ANCHOR_CHALLENGE_WIRE_LEN_V1, 184);
    assert_eq!(ANCHOR_OBSERVATION_SIGNED_LEN_V1, 224);
    assert_eq!(ANCHOR_OBSERVATION_WIRE_LEN_V1, 288);
}

#[test]
fn canonical_challenge_binds_every_transition_field() {
    let (_, key) = keys(7);
    let pending = prepare(41, [1; 32], [2; 32], &key)
        .begin_advance(CallerNonceV1::from_bytes([3; 32]), &key)
        .unwrap();
    let challenge = pending.challenge();
    assert_eq!(challenge.kind(), ChallengeKindV1::Advance);
    assert_eq!(challenge.nonce(), [3; 32]);
    assert_eq!(challenge.expected_sequence(), 42);
    assert_eq!(challenge.prior_head().to_bytes(), [1; 32]);
    assert_eq!(challenge.transaction().to_bytes(), [2; 32]);
    assert_eq!(challenge.anchor_key_identity(), key.identity());
    assert_ne!(challenge.proposed_head(), challenge.prior_head());
    assert_eq!(
        AnchorChallengeV1::decode(challenge.as_bytes()).unwrap(),
        *challenge
    );
    assert_eq!(&challenge.as_bytes()[..8], b"F2ARBA1\0");
    assert_eq!(&challenge.as_bytes()[8..10], &1_u16.to_le_bytes());
    assert_eq!(&challenge.as_bytes()[12..16], &[0; 4]);
}

#[test]
fn advance_can_produce_commit_only_after_valid_proposed_observation() {
    let (signing, key) = keys(8);
    let pending = prepare(9, [4; 32], [5; 32], &key)
        .begin_advance(CallerNonceV1::from_bytes([6; 32]), &key)
        .unwrap();
    let proposed = pending.challenge().proposed_head();
    let wire = signed_observation(pending.challenge(), AnchorPositionV1::Proposed, &signing);
    let AnchorDecisionV1::Commit(commit) = pending.verify(&wire).unwrap() else {
        panic!("proposed observation must commit");
    };
    assert_eq!(commit.sequence(), 10);
    assert_eq!(commit.head(), proposed);
    assert_eq!(commit.prior_head().to_bytes(), [4; 32]);
    assert_eq!(commit.transaction().to_bytes(), [5; 32]);
    assert_eq!(commit.observed_nonce(), &[6; 32]);
    assert_eq!(commit.into_stable_state().sequence(), 10);
}

#[test]
fn exact_prior_observation_deterministically_aborts() {
    let (signing, key) = keys(9);
    let pending = prepare(19, [10; 32], [11; 32], &key)
        .begin_advance(CallerNonceV1::from_bytes([12; 32]), &key)
        .unwrap();
    let proposed = pending.challenge().proposed_head();
    let wire = signed_observation(pending.challenge(), AnchorPositionV1::Prior, &signing);
    let AnchorDecisionV1::Abort(abort) = pending.verify(&wire).unwrap() else {
        panic!("prior observation must abort");
    };
    assert_eq!(abort.sequence(), 19);
    assert_eq!(abort.head().to_bytes(), [10; 32]);
    assert_eq!(abort.proposed_head(), proposed);
    assert_eq!(abort.transaction().to_bytes(), [11; 32]);
    assert_eq!(abort.observed_nonce(), &[12; 32]);
    assert_eq!(abort.into_stable_state().sequence(), 19);
}

#[test]
fn crash_recovery_reconstructs_and_resolves_both_positions() {
    let (signing, key) = keys(13);
    let (expected, prior, transaction, proposed) = {
        let original = prepare(22, [14; 32], [15; 32], &key);
        (
            original.expected_sequence(),
            original.prior_head(),
            original.transaction(),
            original.proposed_head(),
        )
    };

    for (position, expect_commit, nonce) in [
        (AnchorPositionV1::Prior, false, [16; 32]),
        (AnchorPositionV1::Proposed, true, [17; 32]),
    ] {
        let recovered = PreparedAnchorAdvanceV1::recover_from_local_state(
            expected,
            prior,
            transaction,
            proposed,
            &key,
        )
        .unwrap();
        let pending = recovered
            .begin_recovery(CallerNonceV1::from_bytes(nonce), &key)
            .unwrap();
        assert_eq!(pending.challenge().kind(), ChallengeKindV1::Recover);
        let wire = signed_observation(pending.challenge(), position, &signing);
        assert_eq!(
            matches!(pending.verify(&wire).unwrap(), AnchorDecisionV1::Commit(_)),
            expect_commit
        );
    }
}

#[test]
fn recovery_rejects_mutated_local_transaction_or_head() {
    let (_, key) = keys(18);
    let original = prepare(29, [19; 32], [20; 32], &key);
    assert_eq!(
        PreparedAnchorAdvanceV1::recover_from_local_state(
            original.expected_sequence(),
            original.prior_head(),
            TransactionDigestV1::from_bytes([21; 32]),
            original.proposed_head(),
            &key,
        ),
        Err(AnchorProtocolErrorV1::InvalidProposedHead)
    );
    assert_eq!(
        PreparedAnchorAdvanceV1::recover_from_local_state(
            original.expected_sequence(),
            HashChainHeadV1::from_bytes([22; 32]),
            original.transaction(),
            original.proposed_head(),
            &key,
        ),
        Err(AnchorProtocolErrorV1::InvalidProposedHead)
    );
}

#[test]
fn stale_nonce_and_phase_confusion_are_rejected_even_when_resigned() {
    let (signing, key) = keys(23);
    let advance = prepare(31, [24; 32], [25; 32], &key)
        .begin_advance(CallerNonceV1::from_bytes([26; 32]), &key)
        .unwrap();
    let stale = prepare(31, [24; 32], [25; 32], &key)
        .begin_recovery(CallerNonceV1::from_bytes([27; 32]), &key)
        .unwrap();
    let stale_wire = signed_observation(stale.challenge(), AnchorPositionV1::Proposed, &signing);
    assert_eq!(
        advance.verify(&stale_wire),
        Err(AnchorProtocolErrorV1::ChallengeMismatch)
    );

    let advance = prepare(31, [24; 32], [25; 32], &key)
        .begin_advance(CallerNonceV1::from_bytes([28; 32]), &key)
        .unwrap();
    let recovery = prepare(31, [24; 32], [25; 32], &key)
        .begin_recovery(CallerNonceV1::from_bytes([28; 32]), &key)
        .unwrap();
    let wrong_phase =
        signed_observation(recovery.challenge(), AnchorPositionV1::Proposed, &signing);
    assert_eq!(
        advance.verify(&wrong_phase),
        Err(AnchorProtocolErrorV1::ChallengeMismatch)
    );
}

#[test]
fn transaction_and_chain_mutation_are_rejected_even_when_resigned() {
    let (signing, key) = keys(29);
    let expected = prepare(37, [30; 32], [31; 32], &key)
        .begin_recovery(CallerNonceV1::from_bytes([32; 32]), &key)
        .unwrap();
    let mutated = prepare(37, [30; 32], [33; 32], &key)
        .begin_recovery(CallerNonceV1::from_bytes([32; 32]), &key)
        .unwrap();
    let wire = signed_observation(mutated.challenge(), AnchorPositionV1::Proposed, &signing);
    assert_eq!(
        expected.verify(&wire),
        Err(AnchorProtocolErrorV1::ChallengeMismatch)
    );
}

#[test]
fn wrong_key_weak_key_and_signature_malleation_are_rejected() {
    let (signing, key) = keys(34);
    let (wrong_signing, wrong_key) = keys(35);
    let prepared = prepare(40, [36; 32], [37; 32], &key);
    assert_eq!(
        prepared.begin_advance(CallerNonceV1::from_bytes([38; 32]), &wrong_key),
        Err(AnchorProtocolErrorV1::AnchorKeyIdentityMismatch)
    );
    assert!(matches!(
        PinnedAnchorKeyV1::from_bytes([0; 32]),
        Err(AnchorProtocolErrorV1::WeakVerifyingKey)
            | Err(AnchorProtocolErrorV1::InvalidVerifyingKey)
    ));

    let pending = prepare(40, [36; 32], [37; 32], &key)
        .begin_advance(CallerNonceV1::from_bytes([38; 32]), &key)
        .unwrap();
    let wrong_key_wire = signed_observation(
        pending.challenge(),
        AnchorPositionV1::Proposed,
        &wrong_signing,
    );
    assert_eq!(
        pending.verify(&wrong_key_wire),
        Err(AnchorProtocolErrorV1::SignatureRejected)
    );

    let pending = prepare(40, [36; 32], [37; 32], &key)
        .begin_advance(CallerNonceV1::from_bytes([38; 32]), &key)
        .unwrap();
    let unsigned = UnsignedAnchorObservationV1::from_challenge(
        pending.challenge(),
        AnchorPositionV1::Proposed,
    );
    let wrong_domain_signature = signing.sign(pending.challenge().as_bytes()).to_bytes();
    assert_eq!(
        pending.verify(&unsigned.attach_signature(wrong_domain_signature)),
        Err(AnchorProtocolErrorV1::SignatureRejected)
    );

    let pending = prepare(40, [36; 32], [37; 32], &key)
        .begin_advance(CallerNonceV1::from_bytes([38; 32]), &key)
        .unwrap();
    let mut wire = signed_observation(pending.challenge(), AnchorPositionV1::Proposed, &signing);
    wire[ANCHOR_OBSERVATION_WIRE_LEN_V1 - 32..].fill(0xff);
    assert_eq!(
        pending.verify(&wire),
        Err(AnchorProtocolErrorV1::SignatureRejected)
    );
}

#[test]
fn exact_lengths_trailing_bytes_and_noncanonical_headers_are_rejected() {
    let (_, key) = keys(39);
    let pending = prepare(0, [40; 32], [41; 32], &key)
        .begin_advance(CallerNonceV1::from_bytes([42; 32]), &key)
        .unwrap();
    let challenge = pending.challenge().as_bytes();
    for length in [0, 1, ANCHOR_CHALLENGE_WIRE_LEN_V1 - 1] {
        assert!(matches!(
            AnchorChallengeV1::decode(&challenge[..length]),
            Err(AnchorProtocolErrorV1::InvalidLength { .. })
        ));
    }
    let mut oversized = challenge.to_vec();
    oversized.extend_from_slice(&[0; 4096]);
    assert!(matches!(
        AnchorChallengeV1::decode(&oversized),
        Err(AnchorProtocolErrorV1::InvalidLength { .. })
    ));

    for (offset, expected) in [
        (0, AnchorProtocolErrorV1::InvalidMagic),
        (8, AnchorProtocolErrorV1::UnsupportedVersion { actual: 0 }),
        (
            10,
            AnchorProtocolErrorV1::UnknownChallengeKind { actual: 0 },
        ),
        (11, AnchorProtocolErrorV1::NonzeroReserved),
        (12, AnchorProtocolErrorV1::NonzeroReserved),
    ] {
        let mut bytes = *challenge;
        bytes[offset] = 0;
        if offset >= 11 {
            bytes[offset] = 1;
        }
        assert_eq!(AnchorChallengeV1::decode(&bytes), Err(expected));
    }
}

#[test]
fn sequence_overflow_regression_gap_and_head_mismatch_are_rejected() {
    let (signing, key) = keys(43);
    assert_eq!(
        AnchoredStateV1::from_local_state(u64::MAX, HashChainHeadV1::from_bytes([44; 32]))
            .prepare(TransactionDigestV1::from_bytes([45; 32]), &key),
        Err(AnchorProtocolErrorV1::SequenceOverflow)
    );

    let pending = prepare(50, [46; 32], [47; 32], &key)
        .begin_advance(CallerNonceV1::from_bytes([48; 32]), &key)
        .unwrap();
    let mut regression =
        signed_observation(pending.challenge(), AnchorPositionV1::Proposed, &signing);
    regression[184..192].copy_from_slice(&49_u64.to_le_bytes());
    assert_eq!(
        pending.verify(&regression),
        Err(AnchorProtocolErrorV1::InvalidObservedPosition)
    );

    let pending = prepare(50, [46; 32], [47; 32], &key)
        .begin_advance(CallerNonceV1::from_bytes([48; 32]), &key)
        .unwrap();
    let mut gap = signed_observation(pending.challenge(), AnchorPositionV1::Proposed, &signing);
    gap[184..192].copy_from_slice(&52_u64.to_le_bytes());
    assert_eq!(
        pending.verify(&gap),
        Err(AnchorProtocolErrorV1::InvalidObservedPosition)
    );

    let pending = prepare(50, [46; 32], [47; 32], &key)
        .begin_advance(CallerNonceV1::from_bytes([48; 32]), &key)
        .unwrap();
    let mut wrong_head =
        signed_observation(pending.challenge(), AnchorPositionV1::Proposed, &signing);
    wrong_head[192] ^= 1;
    assert_eq!(
        pending.verify(&wrong_head),
        Err(AnchorProtocolErrorV1::InvalidObservedPosition)
    );
}

#[test]
fn obvious_nonfresh_zero_nonce_is_rejected() {
    let (_, key) = keys(58);
    assert_eq!(
        prepare(0, [59; 32], [60; 32], &key)
            .begin_advance(CallerNonceV1::from_bytes([0; 32]), &key),
        Err(AnchorProtocolErrorV1::ZeroNonce)
    );
}

#[test]
fn every_single_byte_wire_mutation_is_rejected() {
    let (signing, key) = keys(49);
    let canonical_pending = prepare(61, [50; 32], [51; 32], &key)
        .begin_recovery(CallerNonceV1::from_bytes([52; 32]), &key)
        .unwrap();
    let canonical = signed_observation(
        canonical_pending.challenge(),
        AnchorPositionV1::Proposed,
        &signing,
    );
    assert!(matches!(
        canonical_pending.verify(&canonical),
        Ok(AnchorDecisionV1::Commit(_))
    ));

    for index in 0..ANCHOR_OBSERVATION_WIRE_LEN_V1 {
        let pending = prepare(61, [50; 32], [51; 32], &key)
            .begin_recovery(CallerNonceV1::from_bytes([52; 32]), &key)
            .unwrap();
        let mut mutated = canonical;
        mutated[index] ^= 1;
        assert!(
            pending.verify(&mutated).is_err(),
            "accepted mutation at {index}"
        );
    }
}

#[test]
fn bounded_property_corpus_round_trips_and_rejects_signed_field_mutation() {
    let (signing, key) = keys(53);
    let mut generator = 0x9e37_79b9_7f4a_7c15_u64;
    for case in 0..512_u64 {
        let sequence = next(&mut generator) % (u64::MAX - 1);
        let head = generated_bytes(&mut generator);
        let transaction = generated_bytes(&mut generator);
        let nonce = generated_bytes(&mut generator);
        let position = if case % 2 == 0 {
            AnchorPositionV1::Prior
        } else {
            AnchorPositionV1::Proposed
        };
        let pending = prepare(sequence, head, transaction, &key)
            .begin_recovery(CallerNonceV1::from_bytes(nonce), &key)
            .unwrap();
        let canonical = signed_observation(pending.challenge(), position, &signing);
        assert_eq!(
            matches!(
                pending.verify(&canonical).unwrap(),
                AnchorDecisionV1::Commit(_)
            ),
            position == AnchorPositionV1::Proposed
        );

        let pending = prepare(sequence, head, transaction, &key)
            .begin_recovery(CallerNonceV1::from_bytes(nonce), &key)
            .unwrap();
        let mut mutation = canonical;
        let index = next(&mut generator) as usize % ANCHOR_OBSERVATION_SIGNED_LEN_V1;
        mutation[index] ^= ((case as u8).wrapping_mul(17)) | 1;
        assert!(pending.verify(&mutation).is_err());
    }
}

#[test]
fn observation_rejects_truncation_oversize_and_unknown_position() {
    let (signing, key) = keys(54);
    let pending = prepare(70, [55; 32], [56; 32], &key)
        .begin_recovery(CallerNonceV1::from_bytes([57; 32]), &key)
        .unwrap();
    let canonical = signed_observation(pending.challenge(), AnchorPositionV1::Proposed, &signing);
    assert!(matches!(
        pending.verify(&canonical[..canonical.len() - 1]),
        Err(AnchorProtocolErrorV1::InvalidLength { .. })
    ));

    let pending = prepare(70, [55; 32], [56; 32], &key)
        .begin_recovery(CallerNonceV1::from_bytes([57; 32]), &key)
        .unwrap();
    let mut oversized = canonical.to_vec();
    oversized.extend_from_slice(&[0; 8192]);
    assert!(matches!(
        pending.verify(&oversized),
        Err(AnchorProtocolErrorV1::InvalidLength { .. })
    ));

    let pending = prepare(70, [55; 32], [56; 32], &key)
        .begin_recovery(CallerNonceV1::from_bytes([57; 32]), &key)
        .unwrap();
    let mut unknown = canonical;
    unknown[11] = 3;
    assert_eq!(
        pending.verify(&unknown),
        Err(AnchorProtocolErrorV1::UnknownAnchorPosition { actual: 3 })
    );
}

fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn generated_bytes(state: &mut u64) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    for chunk in bytes.chunks_exact_mut(8) {
        chunk.copy_from_slice(&next(state).to_le_bytes());
    }
    bytes
}
