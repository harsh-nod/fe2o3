#[allow(dead_code)]
mod common;

use common::{digest, name, text};
use fe2o3_artifacts::{
    ConfigurationEntry, DigestAlgorithm, DigestBytes, MAX_CONFIGURATION_ENTRIES,
    MAX_PROOF_RECORD_BYTES, MeasuredToolIdentity, PROOF_RECORD_MAGIC, PayloadDigest,
    ProofArtifactIdentity, ProofDecodeError, ProofExecutionIdentity, ProofOutcome, ProofProperty,
    ProofRecordV1, ProofTargetIdentity, SourceContractIdentity, TrustedItem,
    VerificationModelIdentity,
};

fn sha(byte: u8) -> PayloadDigest {
    PayloadDigest::new(DigestAlgorithm::Sha256, digest(byte))
}

fn measured_tool(name: &str, version: &str, byte: u8) -> MeasuredToolIdentity {
    MeasuredToolIdentity::new(text(name), text(version), sha(byte), sha(byte + 1))
}

fn record() -> ProofRecordV1 {
    ProofRecordV1::new(
        ProofTargetIdentity::new(
            ProofArtifactIdentity::new(
                sha(1),
                sha(2),
                sha(3),
                sha(4),
                sha(5),
                sha(6),
                sha(7),
                sha(8),
            ),
            SourceContractIdentity::new(sha(9), sha(10), sha(11), sha(12), sha(13)),
        ),
        vec![
            ConfigurationEntry::new(name("cfg_target"), text("amdgpu")),
            ConfigurationEntry::new(name("feature_bounds"), text("enabled")),
        ],
        ProofExecutionIdentity::new(
            VerificationModelIdentity::new(text("fe2o3-gpu-v1"), sha(14)),
            measured_tool("verus", "0.2026.08.04", 15),
            measured_tool("z3", "4.15.2", 17),
            measured_tool("fe2o3-proof-driver", "0.1.0", 19),
            sha(21),
        ),
        ProofOutcome::Proved,
        vec![
            ProofProperty::Bounds,
            ProofProperty::MemorySafety,
            ProofProperty::RaceFreedom,
        ],
        vec![TrustedItem::new(name("model_axiom"), sha(22))],
    )
    .unwrap()
}

#[test]
fn proof_record_round_trip_is_canonical_and_digest_bound() {
    let original = record();
    let bytes = original.to_bytes();
    let decoded = ProofRecordV1::from_bytes(&bytes).unwrap();

    assert_eq!(decoded, original);
    assert_eq!(decoded.to_bytes(), bytes);
    assert_eq!(
        decoded
            .target()
            .source_contracts()
            .functional_specification_digest(),
        sha(13)
    );
    assert_eq!(
        decoded
            .execution()
            .evidence_recorder()
            .configuration_digest(),
        sha(20)
    );
    assert_eq!(
        decoded.digest(DigestAlgorithm::Sha256),
        DigestAlgorithm::Sha256.calculate(&bytes)
    );
}

#[test]
fn proof_record_v1_encoding_has_a_stable_golden_digest() {
    let encoded = record().to_bytes();
    assert_eq!(&encoded[..8], &PROOF_RECORD_MAGIC);
    assert_eq!(encoded.len(), 880);
    assert_eq!(
        record().digest(DigestAlgorithm::Sha256).bytes(),
        DigestBytes::from_bytes([
            26, 18, 135, 79, 133, 39, 100, 196, 25, 112, 101, 234, 21, 210, 96, 245, 191, 35, 218,
            16, 40, 124, 10, 51, 198, 132, 97, 221, 143, 180, 137, 204,
        ])
    );
}

#[test]
fn malformed_headers_and_envelope_are_rejected() {
    let valid = record().to_bytes();

    let mut bad_magic = valid.clone();
    bad_magic[0] ^= 1;
    assert_eq!(
        ProofRecordV1::from_bytes(&bad_magic),
        Err(ProofDecodeError::InvalidMagic)
    );

    let mut bad_version = valid.clone();
    bad_version[8..10].copy_from_slice(&2u16.to_le_bytes());
    assert_eq!(
        ProofRecordV1::from_bytes(&bad_version),
        Err(ProofDecodeError::UnknownVersion(2))
    );

    let mut bad_flags = valid.clone();
    bad_flags[10..12].copy_from_slice(&1u16.to_le_bytes());
    assert_eq!(
        ProofRecordV1::from_bytes(&bad_flags),
        Err(ProofDecodeError::UnsupportedFlags(1))
    );

    let mut trailing = valid;
    trailing.push(0);
    assert_eq!(
        ProofRecordV1::from_bytes(&trailing),
        Err(ProofDecodeError::TrailingBytes)
    );

    let oversized = vec![0; MAX_PROOF_RECORD_BYTES + 1];
    assert_eq!(
        ProofRecordV1::from_bytes(&oversized),
        Err(ProofDecodeError::TooLarge {
            max: MAX_PROOF_RECORD_BYTES
        })
    );
}

#[test]
fn oversized_count_is_rejected_before_allocation() {
    let mut bytes = record().to_bytes();
    let configuration_count_offset = 12 + 13 * 33;
    bytes[configuration_count_offset..configuration_count_offset + 2]
        .copy_from_slice(&((MAX_CONFIGURATION_ENTRIES + 1) as u16).to_le_bytes());
    assert_eq!(
        ProofRecordV1::from_bytes(&bytes),
        Err(ProofDecodeError::CountOutOfRange {
            field: "proof configuration",
            count: MAX_CONFIGURATION_ENTRIES + 1,
            min: 0,
            max: MAX_CONFIGURATION_ENTRIES,
        })
    );
}

#[test]
fn unknown_tags_and_noncanonical_sets_are_rejected() {
    let valid = record().to_bytes();
    let invocation_offset = tagged_digest_offset(&valid, 21);
    let outcome_offset = invocation_offset + 33;
    let first_property_offset = outcome_offset + 3;

    let mut unknown_digest = valid.clone();
    unknown_digest[12] = 0xff;
    assert_eq!(
        ProofRecordV1::from_bytes(&unknown_digest),
        Err(ProofDecodeError::UnknownTag {
            kind: "digest algorithm",
            tag: 0xff,
        })
    );

    let mut unknown_outcome = valid.clone();
    unknown_outcome[outcome_offset] = 0xff;
    assert_eq!(
        ProofRecordV1::from_bytes(&unknown_outcome),
        Err(ProofDecodeError::UnknownTag {
            kind: "proof outcome",
            tag: 0xff,
        })
    );

    let mut unknown_property = valid.clone();
    unknown_property[first_property_offset] = 0xff;
    assert_eq!(
        ProofRecordV1::from_bytes(&unknown_property),
        Err(ProofDecodeError::UnknownTag {
            kind: "proof property",
            tag: 0xff,
        })
    );

    let mut reversed_properties = valid.clone();
    reversed_properties.swap(first_property_offset, first_property_offset + 1);
    assert_eq!(
        ProofRecordV1::from_bytes(&reversed_properties),
        Err(ProofDecodeError::NonCanonicalOrder {
            field: "proved properties"
        })
    );

    let configuration_start = 12 + 13 * 33 + 2;
    let first_configuration_len = 20;
    let second_configuration_len = 25;
    let first = valid[configuration_start..configuration_start + first_configuration_len].to_vec();
    let second = valid[configuration_start + first_configuration_len
        ..configuration_start + first_configuration_len + second_configuration_len]
        .to_vec();
    let mut reversed_configuration = valid;
    reversed_configuration[configuration_start..configuration_start + second_configuration_len]
        .copy_from_slice(&second);
    reversed_configuration[configuration_start + second_configuration_len
        ..configuration_start + second_configuration_len + first_configuration_len]
        .copy_from_slice(&first);
    assert_eq!(
        ProofRecordV1::from_bytes(&reversed_configuration),
        Err(ProofDecodeError::NonCanonicalOrder {
            field: "proof configuration"
        })
    );
}

fn tagged_digest_offset(bytes: &[u8], byte: u8) -> usize {
    bytes
        .windows(33)
        .position(|window| window[0] == 0 && window[1..].iter().all(|value| *value == byte))
        .expect("fixture digest must have a unique tagged encoding")
}

#[test]
fn every_truncation_is_rejected_without_panicking() {
    let valid = record().to_bytes();
    for end in 0..valid.len() {
        let result = std::panic::catch_unwind(|| ProofRecordV1::from_bytes(&valid[..end]));
        assert!(result.is_ok(), "decoder panicked at truncation {end}");
        assert!(result.unwrap().is_err(), "accepted truncation {end}");
    }
}
