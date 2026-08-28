use ed25519_dalek::SigningKey;
use fe2o3_artifact_transaction::{
    INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1, INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
    INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1, InertCompilerExecutionSubjectV1,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1, COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1,
    CompilerExecutionAttestationChallengeV1, CompilerExecutionAttestationErrorV1,
    CompilerExecutionAttestationReceiptV1, CompilerExecutionAttestationRequestV1,
    CompilerExecutionIssuerMeasurementV1, CompilerExecutionIssuerPolicyV1,
};
use sha2::{Digest, Sha256};

const SUBJECT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V1\0";
const COMPILER_CLOSURE_IDENTITY_DOMAIN: &[u8] = b"fe2o3-compiler-closure-identity-v2\0";
const POLICY_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-ISSUER-POLICY/V1\0";
const CHALLENGE_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-CHALLENGE/V1\0";
const REQUEST_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-REQUEST/V1\0";
const RECEIPT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-RECEIPT/V1\0";

const HEADER_BYTES: usize = 24;
const POLICY_PREIMAGE_BYTES: usize = COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1 - 32;
const CHALLENGE_PREIMAGE_BYTES: usize = COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1 - 32;
const REQUEST_PREIMAGE_BYTES: usize = COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1 - 32;
const RECEIPT_PREIMAGE_BYTES: usize = COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1 - 32;

struct Fixture {
    signing_key: SigningKey,
    policy: CompilerExecutionIssuerPolicyV1,
    subject: InertCompilerExecutionSubjectV1,
    challenge: CompilerExecutionAttestationChallengeV1,
    request: CompilerExecutionAttestationRequestV1,
    receipt: CompilerExecutionAttestationReceiptV1,
}

impl Fixture {
    fn new() -> Self {
        let signing_key = SigningKey::from_bytes(&[0x51; 32]);
        let policy = CompilerExecutionIssuerPolicyV1::new(
            7,
            CompilerExecutionIssuerMeasurementV1::new([0x61; 32], 12_345).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x62; 32], 67_890).unwrap(),
            signing_key.verifying_key().to_bytes(),
        )
        .unwrap();
        let subject = subject(0x20);
        let challenge =
            CompilerExecutionAttestationChallengeV1::new(&policy, &subject, [0x71; 32], 1, [0; 32])
                .unwrap();
        let request =
            CompilerExecutionAttestationRequestV1::new(challenge.clone(), subject.clone()).unwrap();
        let receipt =
            CompilerExecutionAttestationReceiptV1::issue(&policy, &request, &signing_key).unwrap();
        Self {
            signing_key,
            policy,
            subject,
            challenge,
            request,
            receipt,
        }
    }
}

#[test]
fn canonical_round_trip_authenticates_only_the_pinned_key() {
    assert_eq!(COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1, 184);
    assert_eq!(COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1, 200);
    assert_eq!(COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1, 946);
    assert_eq!(COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1, 400);

    let fixture = Fixture::new();
    let policy = CompilerExecutionIssuerPolicyV1::decode(fixture.policy.canonical_bytes()).unwrap();
    let challenge =
        CompilerExecutionAttestationChallengeV1::decode(fixture.challenge.canonical_bytes())
            .unwrap();
    let request =
        CompilerExecutionAttestationRequestV1::decode(fixture.request.canonical_bytes()).unwrap();
    let receipt =
        CompilerExecutionAttestationReceiptV1::decode(fixture.receipt.canonical_bytes()).unwrap();

    assert_eq!(policy, fixture.policy);
    assert_eq!(challenge, fixture.challenge);
    assert_eq!(request, fixture.request);
    assert_eq!(receipt, fixture.receipt);
    assert!(
        policy
            .identity()
            .matches_canonical_bytes(policy.canonical_bytes())
    );
    assert!(
        challenge
            .identity()
            .matches_canonical_bytes(challenge.canonical_bytes())
    );
    assert!(
        request
            .identity()
            .matches_canonical_bytes(request.canonical_bytes())
    );
    assert!(
        receipt
            .identity()
            .matches_canonical_bytes(receipt.canonical_bytes())
    );

    let verified = receipt.verify(&policy, &request, [0; 32]).unwrap();
    assert!(verified.authenticates_pinned_signing_key());
    assert!(!verified.authenticates_protected_compiler_execution());
    assert!(!verified.grants_compiler_authority());
    assert!(!verified.grants_load_authority());
    assert!(!verified.grants_launch_authority());
    assert_eq!(verified.into_receipt(), fixture.receipt);
    assert!(!fixture.receipt.grants_compiler_authority());
    assert!(!fixture.receipt.grants_load_authority());
    assert!(!fixture.receipt.grants_launch_authority());
}

#[test]
fn rollback_chain_requires_the_exact_caller_current_anchor() {
    let fixture = Fixture::new();
    let first_next = fixture.receipt.next_rollback_anchor();
    let second_challenge = CompilerExecutionAttestationChallengeV1::new(
        &fixture.policy,
        &fixture.subject,
        [0x72; 32],
        2,
        first_next,
    )
    .unwrap();
    let second_request =
        CompilerExecutionAttestationRequestV1::new(second_challenge, fixture.subject.clone())
            .unwrap();
    let second_receipt = CompilerExecutionAttestationReceiptV1::issue(
        &fixture.policy,
        &second_request,
        &fixture.signing_key,
    )
    .unwrap();

    second_receipt
        .clone()
        .verify(&fixture.policy, &second_request, first_next)
        .unwrap();
    assert!(matches!(
        second_receipt.verify(&fixture.policy, &second_request, [0; 32]),
        Err(CompilerExecutionAttestationErrorV1::RollbackAnchorMismatch)
    ));
    assert!(matches!(
        fixture
            .receipt
            .verify(&fixture.policy, &fixture.request, first_next),
        Err(CompilerExecutionAttestationErrorV1::RollbackAnchorMismatch)
    ));

    assert!(matches!(
        CompilerExecutionAttestationChallengeV1::new(
            &fixture.policy,
            &fixture.subject,
            [0x73; 32],
            2,
            [0; 32],
        ),
        Err(CompilerExecutionAttestationErrorV1::InvalidRollbackPosition)
    ));
    assert!(matches!(
        CompilerExecutionAttestationChallengeV1::new(
            &fixture.policy,
            &fixture.subject,
            [0x73; 32],
            1,
            [0x74; 32],
        ),
        Err(CompilerExecutionAttestationErrorV1::InvalidRollbackPosition)
    ));
}

#[test]
fn policy_subject_challenge_and_key_substitution_fail_closed() {
    let fixture = Fixture::new();
    let wrong_key = SigningKey::from_bytes(&[0x52; 32]);
    assert!(matches!(
        CompilerExecutionAttestationReceiptV1::issue(&fixture.policy, &fixture.request, &wrong_key,),
        Err(CompilerExecutionAttestationErrorV1::SigningKeyMismatch)
    ));

    let other_policy = CompilerExecutionIssuerPolicyV1::new(
        fixture.policy.generation() + 1,
        fixture.policy.executable(),
        fixture.policy.runtime(),
        fixture.signing_key.verifying_key().to_bytes(),
    )
    .unwrap();
    assert!(matches!(
        fixture.receipt.clone().verify(
            &other_policy,
            &fixture.request,
            fixture.receipt.prior_rollback_anchor(),
        ),
        Err(CompilerExecutionAttestationErrorV1::PolicyMismatch)
    ));

    let other_subject = subject(0x30);
    assert!(matches!(
        CompilerExecutionAttestationRequestV1::new(
            fixture.challenge.clone(),
            other_subject.clone(),
        ),
        Err(CompilerExecutionAttestationErrorV1::SubjectMismatch)
    ));
    let other_subject_challenge = CompilerExecutionAttestationChallengeV1::new(
        &fixture.policy,
        &other_subject,
        [0x71; 32],
        1,
        [0; 32],
    )
    .unwrap();
    let other_subject_request =
        CompilerExecutionAttestationRequestV1::new(other_subject_challenge, other_subject).unwrap();
    let other_subject_receipt = CompilerExecutionAttestationReceiptV1::issue(
        &fixture.policy,
        &other_subject_request,
        &fixture.signing_key,
    )
    .unwrap();
    assert!(matches!(
        other_subject_receipt.verify(&fixture.policy, &fixture.request, [0; 32]),
        Err(CompilerExecutionAttestationErrorV1::SubjectMismatch)
    ));

    let other_challenge = CompilerExecutionAttestationChallengeV1::new(
        &fixture.policy,
        &fixture.subject,
        [0x75; 32],
        1,
        [0; 32],
    )
    .unwrap();
    let other_request =
        CompilerExecutionAttestationRequestV1::new(other_challenge, fixture.subject.clone())
            .unwrap();
    let other_receipt = CompilerExecutionAttestationReceiptV1::issue(
        &fixture.policy,
        &other_request,
        &fixture.signing_key,
    )
    .unwrap();
    assert!(matches!(
        other_receipt.verify(&fixture.policy, &fixture.request, [0; 32]),
        Err(CompilerExecutionAttestationErrorV1::ChallengeMismatch)
    ));
}

#[test]
fn every_single_byte_mutation_is_rejected() {
    let fixture = Fixture::new();
    assert_mutations_rejected(fixture.policy.canonical_bytes(), |bytes| {
        CompilerExecutionIssuerPolicyV1::decode(bytes).is_err()
    });
    assert_mutations_rejected(fixture.challenge.canonical_bytes(), |bytes| {
        CompilerExecutionAttestationChallengeV1::decode(bytes).is_err()
    });
    assert_mutations_rejected(fixture.request.canonical_bytes(), |bytes| {
        CompilerExecutionAttestationRequestV1::decode(bytes).is_err()
    });
    assert_mutations_rejected(fixture.receipt.canonical_bytes(), |bytes| {
        CompilerExecutionAttestationReceiptV1::decode(bytes).is_err()
    });
}

#[test]
fn independently_resealed_noncanonical_records_are_rejected() {
    let fixture = Fixture::new();

    let mut policy = fixture.policy.canonical_bytes().to_vec();
    policy[10] = 1;
    reseal(&mut policy, POLICY_PREIMAGE_BYTES, POLICY_IDENTITY_DOMAIN);
    assert!(matches!(
        CompilerExecutionIssuerPolicyV1::decode(&policy),
        Err(CompilerExecutionAttestationErrorV1::UnsupportedFlags {
            field: "issuer policy",
            flags: 1,
        })
    ));

    let mut challenge = fixture.challenge.canonical_bytes().to_vec();
    challenge[128..136].copy_from_slice(&2_u64.to_le_bytes());
    reseal(
        &mut challenge,
        CHALLENGE_PREIMAGE_BYTES,
        CHALLENGE_IDENTITY_DOMAIN,
    );
    assert!(matches!(
        CompilerExecutionAttestationChallengeV1::decode(&challenge),
        Err(CompilerExecutionAttestationErrorV1::InvalidRollbackPosition)
    ));

    let mut request = fixture.request.canonical_bytes().to_vec();
    let subject_offset = HEADER_BYTES + COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1;
    request[subject_offset..subject_offset + INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1]
        .copy_from_slice(subject(0x30).canonical_bytes());
    reseal(
        &mut request,
        REQUEST_PREIMAGE_BYTES,
        REQUEST_IDENTITY_DOMAIN,
    );
    assert!(matches!(
        CompilerExecutionAttestationRequestV1::decode(&request),
        Err(CompilerExecutionAttestationErrorV1::SubjectMismatch)
    ));

    let mut receipt = fixture.receipt.canonical_bytes().to_vec();
    receipt[240] ^= 1;
    reseal(
        &mut receipt,
        RECEIPT_PREIMAGE_BYTES,
        RECEIPT_IDENTITY_DOMAIN,
    );
    assert!(matches!(
        CompilerExecutionAttestationReceiptV1::decode(&receipt),
        Err(CompilerExecutionAttestationErrorV1::RollbackTransitionMismatch)
    ));

    let mut receipt = fixture.receipt.canonical_bytes().to_vec();
    receipt[304] ^= 1;
    reseal(
        &mut receipt,
        RECEIPT_PREIMAGE_BYTES,
        RECEIPT_IDENTITY_DOMAIN,
    );
    assert!(matches!(
        CompilerExecutionAttestationReceiptV1::decode(&receipt),
        Err(CompilerExecutionAttestationErrorV1::SignatureRejected)
    ));
}

#[test]
fn fixed_length_decoders_reject_truncation_and_extension() {
    let fixture = Fixture::new();
    assert_wrong_lengths(fixture.policy.canonical_bytes(), |bytes| {
        CompilerExecutionIssuerPolicyV1::decode(bytes).is_err()
    });
    assert_wrong_lengths(fixture.challenge.canonical_bytes(), |bytes| {
        CompilerExecutionAttestationChallengeV1::decode(bytes).is_err()
    });
    assert_wrong_lengths(fixture.request.canonical_bytes(), |bytes| {
        CompilerExecutionAttestationRequestV1::decode(bytes).is_err()
    });
    assert_wrong_lengths(fixture.receipt.canonical_bytes(), |bytes| {
        CompilerExecutionAttestationReceiptV1::decode(bytes).is_err()
    });
}

#[test]
fn constructors_reject_zero_and_weak_security_inputs() {
    assert!(matches!(
        CompilerExecutionIssuerMeasurementV1::new([0; 32], 1),
        Err(CompilerExecutionAttestationErrorV1::ZeroValue(
            "issuer measurement"
        ))
    ));
    assert!(matches!(
        CompilerExecutionIssuerMeasurementV1::new([1; 32], 0),
        Err(CompilerExecutionAttestationErrorV1::ZeroValue(
            "issuer measurement"
        ))
    ));

    let fixture = Fixture::new();
    assert!(matches!(
        CompilerExecutionIssuerPolicyV1::new(
            0,
            fixture.policy.executable(),
            fixture.policy.runtime(),
            fixture.signing_key.verifying_key().to_bytes(),
        ),
        Err(CompilerExecutionAttestationErrorV1::ZeroValue(
            "issuer policy generation"
        ))
    ));
    let mut weak_key = [0; 32];
    weak_key[0] = 1;
    assert!(matches!(
        CompilerExecutionIssuerPolicyV1::new(
            1,
            fixture.policy.executable(),
            fixture.policy.runtime(),
            weak_key,
        ),
        Err(CompilerExecutionAttestationErrorV1::WeakVerifyingKey)
    ));
    assert!(matches!(
        CompilerExecutionAttestationChallengeV1::new(
            &fixture.policy,
            &fixture.subject,
            [0; 32],
            1,
            [0; 32],
        ),
        Err(CompilerExecutionAttestationErrorV1::ZeroValue(
            "challenge nonce"
        ))
    ));
    assert!(matches!(
        CompilerExecutionAttestationChallengeV1::new(
            &fixture.policy,
            &fixture.subject,
            [1; 32],
            0,
            [0; 32],
        ),
        Err(CompilerExecutionAttestationErrorV1::ZeroValue(
            "attestation sequence"
        ))
    ));
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
    let identity = identity(SUBJECT_IDENTITY_DOMAIN, &bytes[..offset]);
    put(&mut bytes, &mut offset, &identity);
    assert_eq!(offset, bytes.len());
    InertCompilerExecutionSubjectV1::decode(&bytes).unwrap()
}

fn assert_mutations_rejected(bytes: &[u8], rejects: impl Fn(&[u8]) -> bool) {
    for index in 0..bytes.len() {
        let mut mutated = bytes.to_vec();
        mutated[index] ^= 0x80;
        assert!(rejects(&mutated), "mutation at byte {index} was accepted");
    }
}

fn assert_wrong_lengths(bytes: &[u8], rejects: impl Fn(&[u8]) -> bool) {
    assert!(rejects(&bytes[..bytes.len() - 1]));
    let mut extended = bytes.to_vec();
    extended.push(0);
    assert!(rejects(&extended));
}

fn reseal(bytes: &mut [u8], preimage_len: usize, domain: &[u8]) {
    let identity = identity(domain, &bytes[..preimage_len]);
    bytes[preimage_len..].copy_from_slice(&identity);
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
