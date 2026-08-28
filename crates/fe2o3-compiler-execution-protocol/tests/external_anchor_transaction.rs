use ed25519_dalek::SigningKey;
use fe2o3_artifact_transaction::{
    INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1, INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
    INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1, InertCompilerExecutionSubjectV1,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V1,
    CompilerExecutionAttestationChallengeV1, CompilerExecutionAttestationReceiptV1,
    CompilerExecutionAttestationRequestV1, CompilerExecutionExternalAnchorTransactionErrorV1,
    CompilerExecutionExternalAnchorTransactionV1, CompilerExecutionIssuerMeasurementV1,
    CompilerExecutionIssuerPolicyV1, CompilerExecutionReceiptPublicationV1,
};
use sha2::{Digest, Sha256};

const SUBJECT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V1\0";
const COMPILER_CLOSURE_IDENTITY_DOMAIN: &[u8] = b"fe2o3-compiler-closure-identity-v2\0";

struct Fixture {
    policy: CompilerExecutionIssuerPolicyV1,
    request: CompilerExecutionAttestationRequestV1,
    publication: CompilerExecutionReceiptPublicationV1,
}

impl Fixture {
    fn new(seed: u8, sequence: u64, prior_rollback_anchor: [u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(&[seed; 32]);
        let policy = CompilerExecutionIssuerPolicyV1::new(
            u64::from(seed),
            CompilerExecutionIssuerMeasurementV1::new([seed + 1; 32], 12_345).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed + 2; 32], 67_890).unwrap(),
            signing_key.verifying_key().to_bytes(),
        )
        .unwrap();
        let compiler_subject = subject(seed + 3);
        let challenge = CompilerExecutionAttestationChallengeV1::new(
            &policy,
            &compiler_subject,
            [seed + 4; 32],
            sequence,
            prior_rollback_anchor,
        )
        .unwrap();
        let request =
            CompilerExecutionAttestationRequestV1::new(challenge, compiler_subject).unwrap();
        let receipt =
            CompilerExecutionAttestationReceiptV1::issue(&policy, &request, &signing_key).unwrap();
        let publication =
            CompilerExecutionReceiptPublicationV1::new([seed + 5; 32], [seed + 6; 32], receipt)
                .unwrap();
        Self {
            policy,
            request,
            publication,
        }
    }

    fn transaction(&self) -> CompilerExecutionExternalAnchorTransactionV1 {
        CompilerExecutionExternalAnchorTransactionV1::new(
            self.policy.clone(),
            self.request.clone(),
            self.publication.clone(),
        )
        .unwrap()
    }
}

#[test]
fn exact_compiler_anchor_transaction_round_trips_and_freezes_identity() {
    assert_eq!(
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V1,
        1_842
    );
    let fixture = Fixture::new(0x11, 1, [0; 32]);
    let transaction = fixture.transaction();
    let decoded =
        CompilerExecutionExternalAnchorTransactionV1::decode(transaction.canonical_bytes())
            .unwrap();

    assert_eq!(decoded, transaction);
    assert_eq!(decoded.policy(), &fixture.policy);
    assert_eq!(decoded.request(), &fixture.request);
    assert_eq!(decoded.publication(), &fixture.publication);
    assert_eq!(decoded.sequence(), 1);
    assert_eq!(decoded.prior_rollback_anchor(), [0; 32]);
    assert_eq!(
        decoded.current_rollback_anchor(),
        fixture.publication.receipt().next_rollback_anchor()
    );
    assert!(
        decoded
            .identity()
            .matches_canonical_bytes(decoded.canonical_bytes())
    );
    assert_eq!(
        decoded.identity().as_bytes(),
        &[
            0xeb, 0x81, 0xbc, 0xd9, 0x10, 0xb4, 0x83, 0x96, 0x5a, 0x84, 0xdf, 0xe0, 0xf1, 0xf3,
            0x74, 0xdb, 0xb4, 0xb9, 0x82, 0x2c, 0xaa, 0xd5, 0x95, 0x98, 0x15, 0xff, 0x2d, 0xab,
            0x6e, 0xc3, 0xb0, 0x71,
        ]
    );
    assert_eq!(
        decoded.external_anchor_digest().to_bytes(),
        [
            0x97, 0x1f, 0xe0, 0x53, 0x4b, 0xe0, 0x1f, 0x5f, 0xa0, 0xba, 0xa9, 0x2e, 0xad, 0x94,
            0xec, 0x2b, 0x1b, 0xd0, 0xca, 0x32, 0xfd, 0x31, 0xd9, 0x44, 0x64, 0x34, 0x2d, 0x8a,
            0x17, 0xf0, 0xa9, 0x82,
        ]
    );
    assert!(!decoded.grants_authority());
}

#[test]
fn every_compiler_anchor_transaction_byte_mutation_and_wrong_length_rejects() {
    let transaction = Fixture::new(0x21, 7, [0x31; 32]).transaction();
    let bytes = transaction.canonical_bytes();
    for index in 0..bytes.len() {
        let mut mutated = bytes.to_vec();
        mutated[index] ^= 0x80;
        assert!(
            CompilerExecutionExternalAnchorTransactionV1::decode(&mutated).is_err(),
            "mutation at byte {index} was accepted"
        );
    }
    assert!(
        CompilerExecutionExternalAnchorTransactionV1::decode(&bytes[..bytes.len() - 1]).is_err()
    );
    let mut extended = bytes.to_vec();
    extended.push(0);
    assert!(CompilerExecutionExternalAnchorTransactionV1::decode(&extended).is_err());
}

#[test]
fn compiler_anchor_transaction_rejects_policy_substitution() {
    let fixture = Fixture::new(0x31, 1, [0; 32]);
    let other = Fixture::new(0x41, 1, [0; 32]);
    assert!(matches!(
        CompilerExecutionExternalAnchorTransactionV1::new(
            other.policy,
            fixture.request,
            fixture.publication,
        ),
        Err(CompilerExecutionExternalAnchorTransactionErrorV1::PolicyMismatch)
    ));
}

#[test]
fn compiler_anchor_digest_changes_across_valid_publications_and_positions() {
    let first = Fixture::new(0x51, 1, [0; 32]).transaction();
    let different_publication = Fixture::new(0x61, 1, [0; 32]).transaction();
    let later = Fixture::new(0x51, 2, [0x71; 32]).transaction();

    assert_ne!(first.identity(), different_publication.identity());
    assert_ne!(
        first.external_anchor_digest(),
        different_publication.external_anchor_digest()
    );
    assert_ne!(first.identity(), later.identity());
    assert_ne!(
        first.external_anchor_digest(),
        later.external_anchor_digest()
    );
}

fn subject(seed: u8) -> InertCompilerExecutionSubjectV1 {
    let closure_pins = [
        [seed; 32],
        [seed + 1; 32],
        [seed + 2; 32],
        [seed + 3; 32],
        [seed + 4; 32],
        [seed + 5; 32],
    ];
    let mut closure_digest = Sha256::new();
    closure_digest.update(COMPILER_CLOSURE_IDENTITY_DOMAIN);
    closure_digest.update(1_u16.to_le_bytes());
    for pin in closure_pins {
        closure_digest.update(pin);
    }
    let closure_identity: [u8; 32] = closure_digest.finalize().into();
    let mut bytes = [0_u8; INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1];
    let mut offset = 0;
    put(
        &mut bytes,
        &mut offset,
        &INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
    );
    put(
        &mut bytes,
        &mut offset,
        &INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1.to_le_bytes(),
    );
    put(&mut bytes, &mut offset, &0_u16.to_le_bytes());
    put(
        &mut bytes,
        &mut offset,
        &(INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1 as u64).to_le_bytes(),
    );
    put(&mut bytes, &mut offset, &0_u32.to_le_bytes());
    put(&mut bytes, &mut offset, &9_u64.to_le_bytes());
    put(&mut bytes, &mut offset, &[seed + 6; 16]);
    put(&mut bytes, &mut offset, &[seed + 7; 32]);
    bytes[offset] = 0;
    offset += 8;
    put(&mut bytes, &mut offset, &[seed + 8; 32]);
    put(&mut bytes, &mut offset, &[seed + 9; 32]);
    for pin in closure_pins {
        put(&mut bytes, &mut offset, &pin);
    }
    put(&mut bytes, &mut offset, &1_u16.to_le_bytes());
    put(&mut bytes, &mut offset, &closure_identity);
    for axis in 0_u8..7 {
        put(&mut bytes, &mut offset, &[seed + 10 + axis; 32]);
        put(
            &mut bytes,
            &mut offset,
            &(1_000_u64 + u64::from(axis)).to_le_bytes(),
        );
    }
    assert_eq!(offset, INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1 - 32);
    let subject_identity = identity(SUBJECT_IDENTITY_DOMAIN, &bytes[..offset]);
    put(&mut bytes, &mut offset, &subject_identity);
    assert_eq!(offset, bytes.len());
    InertCompilerExecutionSubjectV1::decode(&bytes).unwrap()
}

fn identity(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn put(output: &mut [u8], offset: &mut usize, value: &[u8]) {
    let end = *offset + value.len();
    output[*offset..end].copy_from_slice(value);
    *offset = end;
}
