use ed25519_dalek::{Signer, SigningKey};
use fe2o3_external_anchor_protocol::{
    ANCHOR_TRANSITION_RECEIPT_BYTES_V1, AnchorChallengeV1, AnchorPositionV1, AnchorProtocolErrorV1,
    AnchorTransitionReceiptV1, AnchoredStateV1, CallerNonceV1, HashChainHeadV1, PinnedAnchorKeyV1,
    UnsignedAnchorObservationV1, derive_transaction_digest_v1,
};

struct Fixture {
    signing_key: SigningKey,
    pinned_key: PinnedAnchorKeyV1,
    challenge: AnchorChallengeV1,
}

impl Fixture {
    fn new(seed: u8, nonce: [u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(&[seed; 32]);
        let pinned_key =
            PinnedAnchorKeyV1::from_bytes(signing_key.verifying_key().to_bytes()).unwrap();
        let stable = AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32]));
        let transaction = derive_transaction_digest_v1(&[seed; 32]).unwrap();
        let prepared = stable.prepare(transaction, &pinned_key).unwrap();
        let pending = prepared
            .begin_advance(CallerNonceV1::from_bytes(nonce), &pinned_key)
            .unwrap();
        Self {
            signing_key,
            pinned_key,
            challenge: pending.challenge().clone(),
        }
    }

    fn observation(&self, position: AnchorPositionV1) -> [u8; 288] {
        let unsigned = UnsignedAnchorObservationV1::from_challenge(&self.challenge, position);
        let signature = self.signing_key.sign(&unsigned.signing_bytes()).to_bytes();
        unsigned.attach_signature(signature)
    }

    fn receipt(&self, position: AnchorPositionV1) -> AnchorTransitionReceiptV1 {
        AnchorTransitionReceiptV1::new(
            self.challenge.clone(),
            &self.observation(position),
            &self.pinned_key,
        )
        .unwrap()
    }
}

#[test]
fn proposed_transition_receipt_round_trips_and_freezes_identity() {
    assert_eq!(ANCHOR_TRANSITION_RECEIPT_BYTES_V1, 528);
    let fixture = Fixture::new(0x11, [0x21; 32]);
    let receipt = fixture.receipt(AnchorPositionV1::Proposed);
    let decoded =
        AnchorTransitionReceiptV1::decode(receipt.canonical_bytes(), &fixture.pinned_key).unwrap();

    assert_eq!(decoded, receipt);
    assert_eq!(decoded.challenge(), &fixture.challenge);
    assert_eq!(
        decoded.observation_bytes(),
        &fixture.observation(AnchorPositionV1::Proposed)
    );
    assert_eq!(decoded.position(), AnchorPositionV1::Proposed);
    assert_eq!(decoded.anchor_key_identity(), fixture.pinned_key.identity());
    assert!(decoded.observes_proposed_position());
    assert!(decoded.authenticates_pinned_key_and_exact_challenge());
    assert!(!decoded.grants_authority());
    assert!(
        decoded
            .identity()
            .matches_canonical_bytes(decoded.canonical_bytes())
    );
    assert_eq!(
        decoded.identity().as_bytes(),
        &[
            0xab, 0x5b, 0xba, 0x32, 0x34, 0x4c, 0xb4, 0x32, 0x9a, 0xe4, 0x62, 0x91, 0x04, 0x69,
            0x62, 0xc6, 0xf7, 0xd4, 0xb2, 0x5f, 0xa0, 0x5e, 0x02, 0xaa, 0x58, 0x11, 0xee, 0x89,
            0x0c, 0x68, 0xb3, 0xdf,
        ]
    );
}

#[test]
fn prior_position_is_explicitly_an_abort_receipt() {
    let fixture = Fixture::new(0x31, [0x41; 32]);
    let receipt = fixture.receipt(AnchorPositionV1::Prior);
    assert_eq!(receipt.position(), AnchorPositionV1::Prior);
    assert!(!receipt.observes_proposed_position());
    assert_ne!(
        receipt.identity(),
        fixture.receipt(AnchorPositionV1::Proposed).identity()
    );
}

#[test]
fn wrong_key_and_challenge_substitution_fail_closed() {
    let fixture = Fixture::new(0x51, [0x61; 32]);
    let other_key = Fixture::new(0x52, [0x61; 32]);
    let other_challenge = Fixture::new(0x51, [0x62; 32]);
    let observation = fixture.observation(AnchorPositionV1::Proposed);

    assert!(matches!(
        AnchorTransitionReceiptV1::new(
            fixture.challenge.clone(),
            &observation,
            &other_key.pinned_key,
        ),
        Err(AnchorProtocolErrorV1::AnchorKeyIdentityMismatch)
    ));
    assert!(matches!(
        AnchorTransitionReceiptV1::new(
            other_challenge.challenge,
            &observation,
            &fixture.pinned_key,
        ),
        Err(AnchorProtocolErrorV1::ChallengeMismatch)
    ));
}

#[test]
fn every_transition_receipt_byte_mutation_and_wrong_length_rejects() {
    let fixture = Fixture::new(0x71, [0x72; 32]);
    let receipt = fixture.receipt(AnchorPositionV1::Proposed);
    let bytes = receipt.canonical_bytes();
    for index in 0..bytes.len() {
        let mut mutated = bytes.to_vec();
        mutated[index] ^= 0x80;
        assert!(
            AnchorTransitionReceiptV1::decode(&mutated, &fixture.pinned_key).is_err(),
            "mutation at byte {index} was accepted"
        );
    }
    assert!(
        AnchorTransitionReceiptV1::decode(&bytes[..bytes.len() - 1], &fixture.pinned_key).is_err()
    );
    let mut extended = bytes.to_vec();
    extended.push(0);
    assert!(AnchorTransitionReceiptV1::decode(&extended, &fixture.pinned_key).is_err());
}
