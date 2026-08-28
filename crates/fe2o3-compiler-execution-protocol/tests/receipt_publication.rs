use ed25519_dalek::SigningKey;
use fe2o3_artifact_transaction::{
    INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1, INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
    INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1, InertCompilerExecutionSubjectV1,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1, CompilerExecutionAttestationChallengeV1,
    CompilerExecutionAttestationReceiptV1, CompilerExecutionAttestationRequestV1,
    CompilerExecutionIssuerMeasurementV1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionReceiptCarriageV1, CompilerExecutionReceiptPublicationAckV1,
    CompilerExecutionReceiptPublicationErrorV1, CompilerExecutionReceiptPublicationV1,
};
use sha2::{Digest, Sha256};

const SUBJECT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V1\0";
const COMPILER_CLOSURE_IDENTITY_DOMAIN: &[u8] = b"fe2o3-compiler-closure-identity-v2\0";

struct Fixture {
    policy: CompilerExecutionIssuerPolicyV1,
    request: CompilerExecutionAttestationRequestV1,
    receipt: CompilerExecutionAttestationReceiptV1,
}

impl Fixture {
    fn new(seed: u8) -> Self {
        let signing_key = SigningKey::from_bytes(&[seed; 32]);
        let policy = CompilerExecutionIssuerPolicyV1::new(
            u64::from(seed),
            CompilerExecutionIssuerMeasurementV1::new([seed + 1; 32], 12_345).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed + 2; 32], 67_890).unwrap(),
            signing_key.verifying_key().to_bytes(),
        )
        .unwrap();
        let subject = subject(seed + 3);
        let challenge = CompilerExecutionAttestationChallengeV1::new(
            &policy,
            &subject,
            [seed + 4; 32],
            1,
            [0; 32],
        )
        .unwrap();
        let request = CompilerExecutionAttestationRequestV1::new(challenge, subject).unwrap();
        let receipt =
            CompilerExecutionAttestationReceiptV1::issue(&policy, &request, &signing_key).unwrap();
        Self {
            policy,
            request,
            receipt,
        }
    }

    fn publication(&self) -> CompilerExecutionReceiptPublicationV1 {
        CompilerExecutionReceiptPublicationV1::new([0x81; 32], [0x82; 32], self.receipt.clone())
            .unwrap()
    }

    fn carriage(&self) -> CompilerExecutionReceiptCarriageV1 {
        let publication = self.publication();
        let acknowledgment =
            CompilerExecutionReceiptPublicationAckV1::new(&publication, [0x83; 32]).unwrap();
        CompilerExecutionReceiptCarriageV1::new(
            self.policy.clone(),
            self.request.clone(),
            publication,
            acknowledgment,
        )
        .unwrap()
    }
}

#[test]
fn complete_carriage_round_trips_without_granting_authority() {
    assert_eq!(COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1, 2058);
    let fixture = Fixture::new(0x51);
    let carriage = fixture.carriage();
    let decoded = CompilerExecutionReceiptCarriageV1::decode(carriage.canonical_bytes()).unwrap();
    assert_eq!(decoded, carriage);
    assert_eq!(decoded.policy(), &fixture.policy);
    assert_eq!(decoded.request(), &fixture.request);
    assert_eq!(decoded.publication(), &fixture.publication());
    decoded
        .acknowledgment()
        .matches_publication(decoded.publication())
        .unwrap();
    assert!(
        decoded
            .identity()
            .matches_canonical_bytes(decoded.canonical_bytes())
    );
    assert!(decoded.requires_protected_policy_verification());
    assert!(!decoded.grants_compiler_authority());
    assert!(!decoded.grants_load_authority());
    assert!(!decoded.grants_launch_authority());
}

#[test]
fn every_carriage_byte_mutation_and_wrong_length_rejects() {
    let carriage = Fixture::new(0x51).carriage();
    assert_mutations_rejected(carriage.canonical_bytes(), |bytes| {
        CompilerExecutionReceiptCarriageV1::decode(bytes).is_err()
    });
    assert_wrong_lengths(carriage.canonical_bytes(), |bytes| {
        CompilerExecutionReceiptCarriageV1::decode(bytes).is_err()
    });
}

#[test]
fn carriage_rejects_independently_valid_nested_substitutions() {
    let first = Fixture::new(0x51);
    let second = Fixture::new(0x61);
    let first_publication = first.publication();
    let first_ack =
        CompilerExecutionReceiptPublicationAckV1::new(&first_publication, [0x83; 32]).unwrap();
    assert!(
        CompilerExecutionReceiptCarriageV1::new(
            second.policy.clone(),
            first.request.clone(),
            first_publication.clone(),
            first_ack.clone(),
        )
        .is_err()
    );
    assert!(
        CompilerExecutionReceiptCarriageV1::new(
            first.policy.clone(),
            second.request.clone(),
            first_publication.clone(),
            first_ack.clone(),
        )
        .is_err()
    );
    let second_publication = second.publication();
    let second_ack =
        CompilerExecutionReceiptPublicationAckV1::new(&second_publication, [0x93; 32]).unwrap();
    assert!(
        CompilerExecutionReceiptCarriageV1::new(
            first.policy.clone(),
            first.request.clone(),
            first_publication,
            second_ack,
        )
        .is_err()
    );
}

#[test]
fn sidecar_and_ack_are_exact_bounded_canonical_records() {
    assert_eq!(COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1, 584);
    assert_eq!(COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1, 288);
    let fixture = Fixture::new(0x51);
    let publication = fixture.publication();
    let ack = CompilerExecutionReceiptPublicationAckV1::new(&publication, [0x83; 32]).unwrap();

    let decoded_publication =
        CompilerExecutionReceiptPublicationV1::decode(publication.canonical_bytes()).unwrap();
    let decoded_ack =
        CompilerExecutionReceiptPublicationAckV1::decode(ack.canonical_bytes()).unwrap();
    assert_eq!(decoded_publication, publication);
    assert_eq!(decoded_ack, ack);
    assert!(
        publication
            .identity()
            .matches_canonical_bytes(publication.canonical_bytes())
    );
    assert!(
        ack.identity()
            .matches_canonical_bytes(ack.canonical_bytes())
    );
    assert_eq!(publication.policy_identity(), fixture.policy.identity());
    assert_eq!(publication.issuer_journal_identity(), [0x81; 32]);
    assert_eq!(publication.compiler_occurrence_identity(), [0x82; 32]);
    assert_eq!(publication.receipt_identity(), fixture.receipt.identity());
    assert_eq!(ack.publication_identity(), publication.identity());
    assert_eq!(ack.worker_ledger_record_identity(), [0x83; 32]);
    assert_eq!(ack.sequence(), fixture.receipt.sequence());
    assert_eq!(
        ack.current_rollback_anchor(),
        fixture.receipt.next_rollback_anchor()
    );
    assert!(!publication.proves_durable_publication());
    assert!(!publication.grants_compiler_authority());
    assert!(!ack.proves_durable_publication());
    assert!(!ack.grants_compiler_authority());
}

#[test]
fn every_sidecar_and_ack_byte_mutation_rejects() {
    let fixture = Fixture::new(0x51);
    let publication = fixture.publication();
    let ack = CompilerExecutionReceiptPublicationAckV1::new(&publication, [0x83; 32]).unwrap();

    assert_mutations_rejected(publication.canonical_bytes(), |bytes| {
        CompilerExecutionReceiptPublicationV1::decode(bytes).is_err()
    });
    assert_mutations_rejected(ack.canonical_bytes(), |bytes| {
        CompilerExecutionReceiptPublicationAckV1::decode(bytes).is_err()
    });
    assert_wrong_lengths(publication.canonical_bytes(), |bytes| {
        CompilerExecutionReceiptPublicationV1::decode(bytes).is_err()
    });
    assert_wrong_lengths(ack.canonical_bytes(), |bytes| {
        CompilerExecutionReceiptPublicationAckV1::decode(bytes).is_err()
    });
}

#[test]
fn independently_valid_substitutions_fail_exact_joins() {
    let fixture = Fixture::new(0x51);
    let other = Fixture::new(0x61);
    let publication = fixture.publication();

    publication
        .matches_issued_record(
            fixture.policy.identity(),
            [0x81; 32],
            [0x82; 32],
            fixture.receipt.identity(),
        )
        .unwrap();
    assert!(matches!(
        publication.matches_issued_record(
            other.policy.identity(),
            [0x81; 32],
            [0x82; 32],
            fixture.receipt.identity(),
        ),
        Err(CompilerExecutionReceiptPublicationErrorV1::PolicyMismatch)
    ));
    assert!(matches!(
        publication.matches_issued_record(
            fixture.policy.identity(),
            [0x91; 32],
            [0x82; 32],
            fixture.receipt.identity(),
        ),
        Err(CompilerExecutionReceiptPublicationErrorV1::IssuerJournalMismatch)
    ));
    assert!(matches!(
        publication.matches_issued_record(
            fixture.policy.identity(),
            [0x81; 32],
            [0x92; 32],
            fixture.receipt.identity(),
        ),
        Err(CompilerExecutionReceiptPublicationErrorV1::OccurrenceMismatch)
    ));
    assert!(matches!(
        publication.matches_issued_record(
            fixture.policy.identity(),
            [0x81; 32],
            [0x82; 32],
            other.receipt.identity(),
        ),
        Err(CompilerExecutionReceiptPublicationErrorV1::ReceiptMismatch)
    ));

    let ack = CompilerExecutionReceiptPublicationAckV1::new(&publication, [0x83; 32]).unwrap();
    ack.matches_publication(&publication).unwrap();
    ack.matches_worker_ledger_record([0x83; 32]).unwrap();
    assert!(matches!(
        ack.matches_worker_ledger_record([0x93; 32]),
        Err(CompilerExecutionReceiptPublicationErrorV1::WorkerLedgerMismatch)
    ));

    let substituted =
        CompilerExecutionReceiptPublicationV1::new([0x91; 32], [0x82; 32], fixture.receipt.clone())
            .unwrap();
    assert!(matches!(
        ack.matches_publication(&substituted),
        Err(CompilerExecutionReceiptPublicationErrorV1::IssuerJournalMismatch)
    ));
}

#[test]
fn zero_authority_bindings_reject_before_encoding() {
    let fixture = Fixture::new(0x51);
    assert!(matches!(
        CompilerExecutionReceiptPublicationV1::new([0; 32], [0x82; 32], fixture.receipt.clone(),),
        Err(CompilerExecutionReceiptPublicationErrorV1::ZeroValue(
            "issuer journal"
        ))
    ));
    assert!(matches!(
        CompilerExecutionReceiptPublicationV1::new([0x81; 32], [0; 32], fixture.receipt.clone(),),
        Err(CompilerExecutionReceiptPublicationErrorV1::ZeroValue(
            "compiler occurrence"
        ))
    ));
    let publication = fixture.publication();
    assert!(matches!(
        CompilerExecutionReceiptPublicationAckV1::new(&publication, [0; 32]),
        Err(CompilerExecutionReceiptPublicationErrorV1::ZeroValue(
            "Worker ledger record"
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
