use std::collections::BTreeMap;

use ed25519_dalek::{Signer, SigningKey};
use fe2o3_external_anchor_protocol::{
    ANCHOR_CHALLENGE_WIRE_LEN_V1, ANCHOR_OBSERVATION_WIRE_LEN_V1, AnchorChallengeV1,
    AnchorDecisionV1, AnchorPositionV1, AnchorProtocolErrorV1, AnchoredStateV1, CallerNonceV1,
    ChallengeKindV1, HashChainHeadV1, PendingAnchorTransitionV1, PinnedAnchorKeyV1,
    TRANSACTION_IDENTITY_MAX_LEN_V1, TransactionDigestV1, UnsignedAnchorObservationV1,
    derive_transaction_digest_v1,
};
use sha2::{Digest, Sha256};

const VECTOR_TEXT: &str = include_str!("vectors/external_anchor_v1.txt");
const MUTATED_FIELD_OFFSETS: [usize; 14] = [
    0,   // magic
    8,   // version
    10,  // challenge kind
    11,  // anchor position
    12,  // reserved
    16,  // nonce
    48,  // expected sequence
    56,  // prior head
    88,  // transaction digest
    120, // proposed head
    152, // anchor key identity
    184, // observed sequence
    192, // observed head
    224, // signature
];

fn vectors() -> BTreeMap<&'static str, &'static str> {
    VECTOR_TEXT
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                None
            } else {
                Some(line.split_once('=').expect("vector line is key=value"))
            }
        })
        .collect()
}

fn decode_hex(input: &str) -> Vec<u8> {
    assert_eq!(input.len() % 2, 0, "hex input must contain whole bytes");
    input
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = char::from(pair[0]).to_digit(16).expect("hex digit");
            let low = char::from(pair[1]).to_digit(16).expect("hex digit");
            u8::try_from((high << 4) | low).expect("decoded byte")
        })
        .collect()
}

fn fixed_hex<const N: usize>(input: &str) -> [u8; N] {
    decode_hex(input)
        .try_into()
        .expect("vector has fixed length")
}

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn signing_and_pinned(vector: &BTreeMap<&str, &str>) -> (SigningKey, PinnedAnchorKeyV1) {
    let signing = SigningKey::from_bytes(&fixed_hex(vector["signing_seed"]));
    let pinned = PinnedAnchorKeyV1::from_bytes(signing.verifying_key().to_bytes()).unwrap();
    (signing, pinned)
}

fn transaction(vector: &BTreeMap<&str, &str>) -> TransactionDigestV1 {
    derive_transaction_digest_v1(&decode_hex(vector["canonical_transaction_identity"])).unwrap()
}

fn pending(
    kind: ChallengeKindV1,
    vector: &BTreeMap<&str, &str>,
    key: &PinnedAnchorKeyV1,
) -> PendingAnchorTransitionV1 {
    let stable_sequence = vector["stable_sequence"].parse().unwrap();
    let prepared = AnchoredStateV1::from_local_state(
        stable_sequence,
        HashChainHeadV1::from_bytes(fixed_hex(vector["prior_head"])),
    )
    .prepare(transaction(vector), key)
    .unwrap();
    let nonce = match kind {
        ChallengeKindV1::Advance => fixed_hex(vector["advance_nonce"]),
        ChallengeKindV1::Recover => fixed_hex(vector["recovery_nonce"]),
    };
    match kind {
        ChallengeKindV1::Advance => prepared.begin_advance(CallerNonceV1::from_bytes(nonce), key),
        ChallengeKindV1::Recover => prepared.begin_recovery(CallerNonceV1::from_bytes(nonce), key),
    }
    .unwrap()
}

fn names(kind: ChallengeKindV1, position: AnchorPositionV1) -> (&'static str, &'static str) {
    let kind = match kind {
        ChallengeKindV1::Advance => "advance",
        ChallengeKindV1::Recover => "recovery",
    };
    let position = match position {
        AnchorPositionV1::Prior => "prior",
        AnchorPositionV1::Proposed => "proposed",
    };
    (kind, position)
}

#[test]
fn transaction_digest_v1_has_a_bounded_frozen_derivation() {
    let vector = vectors();
    assert_eq!(TRANSACTION_IDENTITY_MAX_LEN_V1, 4096);
    assert_eq!(
        sha256(&decode_hex(vector["transaction_digest_preimage"])),
        fixed_hex(vector["transaction_digest"])
    );
    assert_eq!(
        transaction(&vector).to_bytes(),
        fixed_hex(vector["transaction_digest"])
    );
    assert!(matches!(
        derive_transaction_digest_v1(&[]),
        Err(AnchorProtocolErrorV1::InvalidTransactionIdentityLength { actual: 0, .. })
    ));
    assert!(derive_transaction_digest_v1(&vec![0x5a; TRANSACTION_IDENTITY_MAX_LEN_V1]).is_ok());
    assert!(matches!(
        derive_transaction_digest_v1(&vec![0x5a; TRANSACTION_IDENTITY_MAX_LEN_V1 + 1]),
        Err(AnchorProtocolErrorV1::InvalidTransactionIdentityLength { .. })
    ));

    let mut mutation = decode_hex(vector["canonical_transaction_identity"]);
    mutation[0] ^= 1;
    assert_ne!(
        derive_transaction_digest_v1(&mutation).unwrap(),
        transaction(&vector)
    );
}

#[test]
fn all_v1_challenge_and_observation_encodings_match_frozen_vectors() {
    let vector = vectors();
    let (signing, key) = signing_and_pinned(&vector);
    assert_eq!(
        signing.verifying_key().to_bytes(),
        fixed_hex(vector["public_key"])
    );
    assert_eq!(
        key.identity().to_bytes(),
        fixed_hex(vector["anchor_key_identity"])
    );
    assert_eq!(
        sha256(&decode_hex(vector["anchor_key_identity_preimage"])),
        key.identity().to_bytes()
    );
    assert_eq!(
        sha256(&decode_hex(vector["proposed_head_preimage"])),
        fixed_hex(vector["proposed_head"])
    );

    for kind in [ChallengeKindV1::Advance, ChallengeKindV1::Recover] {
        let kind_name = names(kind, AnchorPositionV1::Prior).0;
        let challenge_name = format!("{kind_name}_challenge");
        let transition = pending(kind, &vector, &key);
        let expected_challenge: [u8; ANCHOR_CHALLENGE_WIRE_LEN_V1] =
            fixed_hex(vector[challenge_name.as_str()]);
        assert_eq!(transition.challenge().as_bytes(), &expected_challenge);
        assert_eq!(
            AnchorChallengeV1::decode(&expected_challenge).unwrap(),
            *transition.challenge()
        );
        assert_eq!(
            transition.challenge().proposed_head().to_bytes(),
            fixed_hex(vector["proposed_head"])
        );
        assert_eq!(
            transition.challenge().expected_sequence(),
            vector["expected_sequence"].parse().unwrap()
        );

        for position in [AnchorPositionV1::Prior, AnchorPositionV1::Proposed] {
            let (kind_name, position_name) = names(kind, position);
            let signing_bytes_name = format!("{kind_name}_{position_name}_signing_bytes");
            let signature_name = format!("{kind_name}_{position_name}_signature");
            let wire_name = format!("{kind_name}_{position_name}_wire");

            let transition = pending(kind, &vector, &key);
            let unsigned =
                UnsignedAnchorObservationV1::from_challenge(transition.challenge(), position);
            assert_eq!(
                encode_hex(&unsigned.signing_bytes()),
                vector[signing_bytes_name.as_str()]
            );
            let signature = signing.sign(&unsigned.signing_bytes()).to_bytes();
            assert_eq!(encode_hex(&signature), vector[signature_name.as_str()]);
            let wire = unsigned.attach_signature(signature);
            assert_eq!(encode_hex(&wire), vector[wire_name.as_str()]);
            assert_eq!(
                wire,
                fixed_hex::<ANCHOR_OBSERVATION_WIRE_LEN_V1>(vector[wire_name.as_str()])
            );
            assert_eq!(
                matches!(
                    transition.verify(&wire).unwrap(),
                    AnchorDecisionV1::Commit(_)
                ),
                position == AnchorPositionV1::Proposed
            );
        }
    }
}

#[test]
fn every_frozen_observation_rejects_a_single_field_mutation() {
    let vector = vectors();
    let (_, key) = signing_and_pinned(&vector);
    for kind in [ChallengeKindV1::Advance, ChallengeKindV1::Recover] {
        for position in [AnchorPositionV1::Prior, AnchorPositionV1::Proposed] {
            let (kind_name, position_name) = names(kind, position);
            let wire_name = format!("{kind_name}_{position_name}_wire");
            let canonical: [u8; ANCHOR_OBSERVATION_WIRE_LEN_V1] =
                fixed_hex(vector[wire_name.as_str()]);
            for offset in MUTATED_FIELD_OFFSETS {
                let transition = pending(kind, &vector, &key);
                let mut mutated = canonical;
                mutated[offset] ^= 1;
                assert!(
                    transition.verify(&mutated).is_err(),
                    "accepted {wire_name} field mutation at byte {offset}"
                );
            }
        }
    }
}
