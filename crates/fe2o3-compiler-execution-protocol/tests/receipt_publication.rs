use ed25519_dalek::{Signer, SigningKey};
use fe2o3_artifact_transaction::{
    INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1, INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
    INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1, InertCompilerExecutionSubjectV1,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V3,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1, CompilerExecutionAttestationChallengeV1,
    CompilerExecutionAttestationReceiptV1, CompilerExecutionAttestationRequestV1,
    CompilerExecutionCurrentRecordAttestationV3, CompilerExecutionCurrentRecordVerificationErrorV3,
    CompilerExecutionCurrentRecordVerificationV3, CompilerExecutionExternalAnchorTransactionV1,
    CompilerExecutionIssuerMeasurementV1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionReceiptCarriageV1, CompilerExecutionReceiptPublicationAckV1,
    CompilerExecutionReceiptPublicationErrorV1, CompilerExecutionReceiptPublicationV1,
};
use fe2o3_external_anchor_protocol::{
    AnchorPositionV1, AnchorTransitionReceiptV1, AnchoredStateV1, CallerNonceV1, HashChainHeadV1,
    PinnedAnchorKeyV1, UnsignedAnchorObservationV1,
};
use sha2::{Digest, Sha256};

const SUBJECT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V1\0";
const COMPILER_CLOSURE_IDENTITY_DOMAIN: &[u8] = b"fe2o3-compiler-closure-identity-v2\0";

struct Fixture {
    signing_key: SigningKey,
    anchor_signing_key: SigningKey,
    policy: CompilerExecutionIssuerPolicyV1,
    request: CompilerExecutionAttestationRequestV1,
    receipt: CompilerExecutionAttestationReceiptV1,
}

impl Fixture {
    fn new(seed: u8) -> Self {
        let signing_key = SigningKey::from_bytes(&[seed; 32]);
        let anchor_signing_key = SigningKey::from_bytes(&[seed.wrapping_add(1); 32]);
        let policy = CompilerExecutionIssuerPolicyV1::new(
            u64::from(seed),
            CompilerExecutionIssuerMeasurementV1::new([seed + 1; 32], 12_345).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed + 2; 32], 67_890).unwrap(),
            signing_key.verifying_key().to_bytes(),
            anchor_signing_key.verifying_key().to_bytes(),
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
            signing_key,
            anchor_signing_key,
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
        self.carriage_with_worker_identity([0x83; 32])
    }

    fn carriage_with_worker_identity(
        &self,
        worker_identity: [u8; 32],
    ) -> CompilerExecutionReceiptCarriageV1 {
        let publication = self.publication();
        let acknowledgment =
            CompilerExecutionReceiptPublicationAckV1::new(&publication, worker_identity).unwrap();
        CompilerExecutionReceiptCarriageV1::new(
            self.policy.clone(),
            self.request.clone(),
            publication,
            acknowledgment,
        )
        .unwrap()
    }

    fn carriage_with_publication_bindings(
        &self,
        compiler_invocation: [u8; 32],
        artifact_transaction: [u8; 32],
    ) -> CompilerExecutionReceiptCarriageV1 {
        let publication = CompilerExecutionReceiptPublicationV1::new(
            compiler_invocation,
            artifact_transaction,
            self.receipt.clone(),
        )
        .unwrap();
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

    fn anchor_receipt(
        &self,
        carriage: &CompilerExecutionReceiptCarriageV1,
        nonce: [u8; 32],
    ) -> AnchorTransitionReceiptV1 {
        self.anchor_receipt_at_position(carriage, nonce, AnchorPositionV1::Proposed)
    }

    fn anchor_receipt_at_position(
        &self,
        carriage: &CompilerExecutionReceiptCarriageV1,
        nonce: [u8; 32],
        position: AnchorPositionV1,
    ) -> AnchorTransitionReceiptV1 {
        let transaction = CompilerExecutionExternalAnchorTransactionV1::new(
            carriage.policy().clone(),
            carriage.request().clone(),
            carriage.publication().clone(),
        )
        .unwrap();
        let key = PinnedAnchorKeyV1::from_bytes(self.anchor_signing_key.verifying_key().to_bytes())
            .unwrap();
        let pending = AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32]))
            .prepare(transaction.external_anchor_digest(), &key)
            .unwrap()
            .begin_advance(CallerNonceV1::from_bytes(nonce), &key)
            .unwrap();
        let unsigned = UnsignedAnchorObservationV1::from_challenge(pending.challenge(), position);
        let signature = self.anchor_signing_key.sign(&unsigned.signing_bytes());
        AnchorTransitionReceiptV1::new(
            pending.challenge().clone(),
            &unsigned.attach_signature(signature.to_bytes()),
            &key,
        )
        .unwrap()
    }

    fn currentness_receipt(
        &self,
        carriage: &CompilerExecutionReceiptCarriageV1,
        commit_receipt: &AnchorTransitionReceiptV1,
        verification_challenge: [u8; 32],
        position: AnchorPositionV1,
    ) -> AnchorTransitionReceiptV1 {
        let challenge =
            CompilerExecutionCurrentRecordVerificationV3::external_anchor_currentness_challenge(
                carriage,
                commit_receipt,
                verification_challenge,
            )
            .unwrap();
        let key = PinnedAnchorKeyV1::from_bytes(self.anchor_signing_key.verifying_key().to_bytes())
            .unwrap();
        let unsigned = UnsignedAnchorObservationV1::from_challenge(&challenge, position);
        let signature = self.anchor_signing_key.sign(&unsigned.signing_bytes());
        AnchorTransitionReceiptV1::new(
            challenge,
            &unsigned.attach_signature(signature.to_bytes()),
            &key,
        )
        .unwrap()
    }
}

#[test]
fn challenge_bound_current_record_attestation_round_trips_and_verifies() {
    assert_eq!(COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V3, 1624);
    let fixture = Fixture::new(0x51);
    let challenge = [0xa1; 32];
    let (carriage, verification) =
        current_record_verification(&fixture, challenge, [0x91; 32], [0x92; 32]);
    let attestation = CompilerExecutionCurrentRecordAttestationV3::issue(
        &fixture.policy,
        &carriage,
        verification.clone(),
        challenge,
        &fixture.signing_key,
    )
    .unwrap();
    let decoded =
        CompilerExecutionCurrentRecordAttestationV3::decode(attestation.canonical_bytes()).unwrap();
    assert_eq!(decoded, attestation);
    assert_eq!(decoded.challenge(), challenge);
    assert_eq!(decoded.verification(), &verification);
    assert_eq!(decoded.verifying_key(), *fixture.policy.verifying_key());
    assert!(!decoded.grants_authority());

    let verified = decoded
        .verify(&fixture.policy, &carriage, challenge)
        .unwrap();
    assert_eq!(verified.verification(), &verification);
    assert!(verified.authenticates_pinned_signing_key());
    assert!(verified.authenticates_expected_challenge());
    assert!(!verified.authenticates_protected_current_record());
    assert!(verified.authenticates_external_anchor_commit());
    assert!(verified.authenticates_external_rollback_currentness());
    assert_eq!(
        verified.external_rollback_verification_identity(),
        *verification
            .external_anchor_currentness_receipt()
            .identity()
            .as_bytes()
    );
    assert!(!verified.grants_authority());
}

#[test]
fn current_record_attestation_rejects_key_policy_challenge_and_record_substitution() {
    let fixture = Fixture::new(0x51);
    let other = Fixture::new(0x61);
    let challenge = [0xb1; 32];
    let (carriage, verification) =
        current_record_verification(&fixture, challenge, [0x91; 32], [0x92; 32]);
    let other_carriage = other.carriage();
    let substituted_carriage = fixture.carriage_with_worker_identity([0x84; 32]);
    let attestation = CompilerExecutionCurrentRecordAttestationV3::issue(
        &fixture.policy,
        &carriage,
        verification.clone(),
        challenge,
        &fixture.signing_key,
    )
    .unwrap();

    assert!(matches!(
        CompilerExecutionCurrentRecordAttestationV3::issue(
            &fixture.policy,
            &carriage,
            verification.clone(),
            challenge,
            &other.signing_key,
        ),
        Err(CompilerExecutionCurrentRecordVerificationErrorV3::SigningKeyMismatch)
    ));
    assert!(matches!(
        attestation
            .clone()
            .verify(&other.policy, &other_carriage, challenge),
        Err(CompilerExecutionCurrentRecordVerificationErrorV3::PolicyMismatch)
    ));
    assert!(matches!(
        attestation
            .clone()
            .verify(&fixture.policy, &carriage, [0xb2; 32]),
        Err(CompilerExecutionCurrentRecordVerificationErrorV3::ChallengeMismatch)
    ));
    assert!(matches!(
        attestation.verify(&fixture.policy, &substituted_carriage, challenge),
        Err(CompilerExecutionCurrentRecordVerificationErrorV3::VerificationMismatch)
    ));
}

#[test]
fn every_current_record_attestation_byte_mutation_and_wrong_length_rejects() {
    let fixture = Fixture::new(0x51);
    let challenge = [0xc1; 32];
    let (carriage, verification) =
        current_record_verification(&fixture, challenge, [0x91; 32], [0x92; 32]);
    assert_mutations_rejected(verification.canonical_bytes(), |bytes| {
        CompilerExecutionCurrentRecordVerificationV3::decode(bytes).is_err()
    });
    assert_wrong_lengths(verification.canonical_bytes(), |bytes| {
        CompilerExecutionCurrentRecordVerificationV3::decode(bytes).is_err()
    });
    let attestation = CompilerExecutionCurrentRecordAttestationV3::issue(
        &fixture.policy,
        &carriage,
        verification,
        challenge,
        &fixture.signing_key,
    )
    .unwrap();
    assert_mutations_rejected(attestation.canonical_bytes(), |bytes| {
        CompilerExecutionCurrentRecordAttestationV3::decode(bytes).is_err()
    });
    assert_wrong_lengths(attestation.canonical_bytes(), |bytes| {
        CompilerExecutionCurrentRecordAttestationV3::decode(bytes).is_err()
    });
}

#[test]
fn current_record_rejects_wrong_anchor_transaction_and_observation_position() {
    let fixture = Fixture::new(0x51);
    let other_anchor = Fixture::new(0x61);
    let carriage = fixture.carriage();
    let challenge = [0xd1; 32];
    let valid_commit = fixture.anchor_receipt(&carriage, [0xa2; 32]);
    let valid_currentness = fixture.currentness_receipt(
        &carriage,
        &valid_commit,
        challenge,
        AnchorPositionV1::Proposed,
    );
    let wrong_key_receipt = other_anchor.anchor_receipt(&other_anchor.carriage(), [0xa3; 32]);
    assert!(matches!(
        CompilerExecutionCurrentRecordVerificationV3::new(
            &carriage,
            wrong_key_receipt,
            valid_currentness.clone(),
            challenge,
            [0x91; 32],
            [0x92; 32],
        ),
        Err(CompilerExecutionCurrentRecordVerificationErrorV3::ExternalAnchor(_))
    ));

    let substituted_carriage = fixture.carriage_with_publication_bindings([0x85; 32], [0x86; 32]);
    let wrong_transaction_receipt = fixture.anchor_receipt(&substituted_carriage, [0xa4; 32]);
    assert!(matches!(
        CompilerExecutionCurrentRecordVerificationV3::new(
            &carriage,
            wrong_transaction_receipt,
            valid_currentness.clone(),
            challenge,
            [0x91; 32],
            [0x92; 32],
        ),
        Err(CompilerExecutionCurrentRecordVerificationErrorV3::ExternalAnchorReceiptMismatch)
    ));

    let prior_receipt =
        fixture.anchor_receipt_at_position(&carriage, [0xa5; 32], AnchorPositionV1::Prior);
    assert!(matches!(
        CompilerExecutionCurrentRecordVerificationV3::new(
            &carriage,
            prior_receipt,
            valid_currentness,
            challenge,
            [0x91; 32],
            [0x92; 32],
        ),
        Err(CompilerExecutionCurrentRecordVerificationErrorV3::ExternalAnchorReceiptMismatch)
    ));
}

#[test]
fn independently_signed_receipts_for_the_same_transition_are_equivalent_commit_evidence() {
    let fixture = Fixture::new(0x51);
    let carriage = fixture.carriage();
    let challenge = [0xd2; 32];
    let first = fixture.anchor_receipt(&carriage, [0xa6; 32]);
    let second = fixture.anchor_receipt(&carriage, [0xa7; 32]);
    assert_ne!(first.identity(), second.identity());
    for receipt in [first, second] {
        let currentness =
            fixture.currentness_receipt(&carriage, &receipt, challenge, AnchorPositionV1::Proposed);
        let verification = CompilerExecutionCurrentRecordVerificationV3::new(
            &carriage,
            receipt.clone(),
            currentness,
            challenge,
            [0x91; 32],
            [0x92; 32],
        )
        .unwrap();
        assert_eq!(verification.external_anchor_commit_receipt(), &receipt);
    }
}

#[test]
fn currentness_receipt_rejects_prior_position_stale_challenge_and_commit_substitution() {
    let fixture = Fixture::new(0x51);
    let carriage = fixture.carriage();
    let challenge = [0xd4; 32];
    let commit = fixture.anchor_receipt(&carriage, [0xa8; 32]);

    let prior = fixture.currentness_receipt(&carriage, &commit, challenge, AnchorPositionV1::Prior);
    assert!(matches!(
        CompilerExecutionCurrentRecordVerificationV3::new(
            &carriage,
            commit.clone(),
            prior,
            challenge,
            [0x91; 32],
            [0x92; 32],
        ),
        Err(
            CompilerExecutionCurrentRecordVerificationErrorV3::ExternalAnchorCurrentnessReceiptMismatch
        )
    ));

    let stale_challenge = [0xd5; 32];
    let stale = fixture.currentness_receipt(
        &carriage,
        &commit,
        stale_challenge,
        AnchorPositionV1::Proposed,
    );
    assert!(matches!(
        CompilerExecutionCurrentRecordVerificationV3::new(
            &carriage,
            commit.clone(),
            stale,
            challenge,
            [0x91; 32],
            [0x92; 32],
        ),
        Err(
            CompilerExecutionCurrentRecordVerificationErrorV3::ExternalAnchorCurrentnessReceiptMismatch
        )
    ));

    let alternate_commit = fixture.anchor_receipt(&carriage, [0xa9; 32]);
    let current_for_first =
        fixture.currentness_receipt(&carriage, &commit, challenge, AnchorPositionV1::Proposed);
    assert!(matches!(
        CompilerExecutionCurrentRecordVerificationV3::new(
            &carriage,
            alternate_commit,
            current_for_first,
            challenge,
            [0x91; 32],
            [0x92; 32],
        ),
        Err(
            CompilerExecutionCurrentRecordVerificationErrorV3::ExternalAnchorCurrentnessReceiptMismatch
        )
    ));

    assert!(matches!(
        CompilerExecutionCurrentRecordVerificationV3::new(
            &carriage,
            commit.clone(),
            commit,
            challenge,
            [0x91; 32],
            [0x92; 32],
        ),
        Err(
            CompilerExecutionCurrentRecordVerificationErrorV3::ExternalAnchorCurrentnessReceiptMismatch
        )
    ));
}

#[test]
fn zero_current_record_attestation_challenge_rejects_before_signing() {
    let fixture = Fixture::new(0x51);
    let (carriage, verification) =
        current_record_verification(&fixture, [0xd3; 32], [0x91; 32], [0x92; 32]);
    assert!(matches!(
        CompilerExecutionCurrentRecordAttestationV3::issue(
            &fixture.policy,
            &carriage,
            verification,
            [0; 32],
            &fixture.signing_key,
        ),
        Err(CompilerExecutionCurrentRecordVerificationErrorV3::ZeroChallenge)
    ));
}

fn current_record_verification(
    fixture: &Fixture,
    verification_challenge: [u8; 32],
    protected_policy: [u8; 32],
    protected_ledger: [u8; 32],
) -> (
    CompilerExecutionReceiptCarriageV1,
    CompilerExecutionCurrentRecordVerificationV3,
) {
    let carriage = fixture.carriage();
    let commit = fixture.anchor_receipt(&carriage, [0x93; 32]);
    let currentness = fixture.currentness_receipt(
        &carriage,
        &commit,
        verification_challenge,
        AnchorPositionV1::Proposed,
    );
    let verification = CompilerExecutionCurrentRecordVerificationV3::new(
        &carriage,
        commit,
        currentness,
        verification_challenge,
        protected_policy,
        protected_ledger,
    )
    .unwrap();
    (carriage, verification)
}

#[test]
fn complete_carriage_round_trips_without_granting_authority() {
    assert_eq!(COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1, 2090);
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
