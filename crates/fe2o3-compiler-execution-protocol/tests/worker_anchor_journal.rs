use ed25519_dalek::{Signer, SigningKey};
use fe2o3_artifact_transaction::{
    INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1, INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
    INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1, InertCompilerExecutionSubjectV1,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V1, CompilerExecutionAttestationChallengeV1,
    CompilerExecutionAttestationReceiptV1, CompilerExecutionAttestationRequestV1,
    CompilerExecutionExternalAnchorTransactionV1, CompilerExecutionIssuerMeasurementV1,
    CompilerExecutionIssuerPolicyV1, CompilerExecutionReceiptPublicationV1,
    CompilerExecutionWorkerAnchorJournalErrorV1, CompilerExecutionWorkerAnchorJournalStageV1,
    CompilerExecutionWorkerAnchorJournalV1,
};
use fe2o3_external_anchor_protocol::{
    AnchorChallengeV1, AnchorPositionV1, AnchorTransitionReceiptV1, AnchoredStateV1, CallerNonceV1,
    HashChainHeadV1, PinnedAnchorKeyV1, UnsignedAnchorObservationV1,
};
use sha2::{Digest, Sha256};

const SUBJECT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V1\0";
const COMPILER_CLOSURE_IDENTITY_DOMAIN: &[u8] = b"fe2o3-compiler-closure-identity-v2\0";

struct Fixture {
    anchor_signing_key: SigningKey,
    pinned_anchor_key: PinnedAnchorKeyV1,
    transaction: CompilerExecutionExternalAnchorTransactionV1,
    challenge: AnchorChallengeV1,
}

impl Fixture {
    fn new(
        seed: u8,
        sequence: u64,
        prior_rollback_anchor: [u8; 32],
        prior_external_head: [u8; 32],
    ) -> Self {
        let issuer_signing_key = SigningKey::from_bytes(&[seed; 32]);
        let anchor_signing_key = SigningKey::from_bytes(&[seed.wrapping_add(1); 32]);
        let policy = CompilerExecutionIssuerPolicyV1::new(
            u64::from(seed),
            CompilerExecutionIssuerMeasurementV1::new([seed + 1; 32], 12_345).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed + 2; 32], 67_890).unwrap(),
            issuer_signing_key.verifying_key().to_bytes(),
            anchor_signing_key.verifying_key().to_bytes(),
        )
        .unwrap();
        let compiler_subject = subject(seed + 3);
        let issuer_challenge = CompilerExecutionAttestationChallengeV1::new(
            &policy,
            &compiler_subject,
            [seed + 4; 32],
            sequence,
            prior_rollback_anchor,
        )
        .unwrap();
        let request =
            CompilerExecutionAttestationRequestV1::new(issuer_challenge, compiler_subject).unwrap();
        let receipt =
            CompilerExecutionAttestationReceiptV1::issue(&policy, &request, &issuer_signing_key)
                .unwrap();
        let publication =
            CompilerExecutionReceiptPublicationV1::new([seed + 5; 32], [seed + 6; 32], receipt)
                .unwrap();
        let transaction =
            CompilerExecutionExternalAnchorTransactionV1::new(policy, request, publication)
                .unwrap();
        let pinned_anchor_key =
            PinnedAnchorKeyV1::from_bytes(anchor_signing_key.verifying_key().to_bytes()).unwrap();
        let stable = AnchoredStateV1::from_local_state(
            sequence - 1,
            HashChainHeadV1::from_bytes(prior_external_head),
        );
        let prepared = stable
            .prepare(transaction.external_anchor_digest(), &pinned_anchor_key)
            .unwrap();
        let pending = prepared
            .begin_advance(
                CallerNonceV1::from_bytes([seed.wrapping_add(7); 32]),
                &pinned_anchor_key,
            )
            .unwrap();
        Self {
            anchor_signing_key,
            pinned_anchor_key,
            transaction,
            challenge: pending.challenge().clone(),
        }
    }

    fn prepared(&self) -> CompilerExecutionWorkerAnchorJournalV1 {
        CompilerExecutionWorkerAnchorJournalV1::prepared(
            self.transaction.clone(),
            self.challenge.clone(),
        )
        .unwrap()
    }

    fn receipt(&self, position: AnchorPositionV1) -> AnchorTransitionReceiptV1 {
        let unsigned = UnsignedAnchorObservationV1::from_challenge(&self.challenge, position);
        let signature = self
            .anchor_signing_key
            .sign(&unsigned.signing_bytes())
            .to_bytes();
        let observation = unsigned.attach_signature(signature);
        AnchorTransitionReceiptV1::new(
            self.challenge.clone(),
            &observation,
            &self.pinned_anchor_key,
        )
        .unwrap()
    }
}

#[test]
fn all_four_stages_round_trip_and_freeze_the_canonical_shape() {
    assert_eq!(COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V1, 2_682);
    let fixture = Fixture::new(0x31, 1, [0; 32], [0; 32]);
    let prepared = fixture.prepared();
    let committed = prepared
        .clone()
        .record_anchor_receipt(fixture.receipt(AnchorPositionV1::Proposed))
        .unwrap();
    let published = committed.clone().mark_published([0x91; 32]).unwrap();
    let aborted = prepared
        .clone()
        .record_anchor_receipt(fixture.receipt(AnchorPositionV1::Prior))
        .unwrap();

    for record in [&prepared, &committed, &published, &aborted] {
        let decoded =
            CompilerExecutionWorkerAnchorJournalV1::decode(record.canonical_bytes()).unwrap();
        assert_eq!(&decoded, record);
        assert_eq!(decoded.transaction(), &fixture.transaction);
        assert_eq!(decoded.challenge(), &fixture.challenge);
        assert!(
            decoded
                .identity()
                .matches_canonical_bytes(decoded.canonical_bytes())
        );
        assert!(!decoded.grants_authority());
    }

    assert_eq!(
        prepared.identity().as_bytes(),
        &[
            0x13, 0xd4, 0x9f, 0x74, 0xec, 0x6e, 0xf2, 0x1b, 0x30, 0x7f, 0x1e, 0xeb, 0x51, 0x08,
            0xe7, 0x2b, 0x2f, 0x6e, 0x3b, 0x77, 0x77, 0xd3, 0x10, 0xf3, 0x97, 0xcc, 0x4a, 0x54,
            0xa0, 0xfb, 0x75, 0x41,
        ]
    );
    assert_eq!(
        committed.identity().as_bytes(),
        &[
            0x57, 0x76, 0xfd, 0x26, 0x52, 0x2e, 0x21, 0x41, 0x47, 0x88, 0xf5, 0x7b, 0x06, 0x6f,
            0x68, 0x5b, 0xdc, 0xd3, 0x1b, 0xae, 0x26, 0x6e, 0xc6, 0xe3, 0x3d, 0xdf, 0xce, 0x25,
            0xf1, 0xb7, 0x83, 0x91,
        ]
    );
    assert_eq!(
        published.identity().as_bytes(),
        &[
            0xdb, 0xc5, 0xd6, 0xd9, 0x76, 0x7e, 0xb7, 0xda, 0x75, 0x00, 0xeb, 0xa2, 0xd2, 0x8f,
            0xe8, 0x9d, 0x35, 0xc2, 0x32, 0x63, 0xc6, 0xd7, 0x99, 0x96, 0x84, 0xf0, 0xee, 0xe5,
            0xee, 0x6f, 0x94, 0x1d,
        ]
    );
    assert_eq!(
        aborted.identity().as_bytes(),
        &[
            0xd7, 0xa5, 0x82, 0x51, 0x5d, 0xf2, 0xab, 0xa7, 0xcc, 0xbb, 0xa3, 0x54, 0xdf, 0xe9,
            0xd7, 0xa8, 0x31, 0x61, 0x78, 0xec, 0x65, 0xb1, 0xdd, 0x84, 0x01, 0x88, 0x8a, 0xcf,
            0xe7, 0x31, 0xa1, 0xf6,
        ]
    );

    assert_eq!(
        prepared.stage(),
        CompilerExecutionWorkerAnchorJournalStageV1::PreparedAnchor
    );
    assert!(prepared.receipt().is_none());
    assert!(prepared.is_genesis_prepared());
    assert_eq!(
        committed.stage(),
        CompilerExecutionWorkerAnchorJournalStageV1::AnchorCommitted
    );
    assert_eq!(
        committed.receipt().unwrap().position(),
        AnchorPositionV1::Proposed
    );
    assert_eq!(
        published.stage(),
        CompilerExecutionWorkerAnchorJournalStageV1::Published
    );
    assert_eq!(published.worker_record_identity(), [0x91; 32]);
    assert_eq!(
        aborted.stage(),
        CompilerExecutionWorkerAnchorJournalStageV1::Aborted
    );
    assert_eq!(
        aborted.receipt().unwrap().position(),
        AnchorPositionV1::Prior
    );
}

#[test]
fn exact_legal_successors_are_explicit_and_ordered() {
    let first = Fixture::new(0x41, 1, [0; 32], [0; 32]);
    let prepared = first.prepared();
    let committed = prepared
        .clone()
        .record_anchor_receipt(first.receipt(AnchorPositionV1::Proposed))
        .unwrap();
    let published = committed.clone().mark_published([0xa1; 32]).unwrap();
    let aborted = prepared
        .clone()
        .record_anchor_receipt(first.receipt(AnchorPositionV1::Prior))
        .unwrap();

    assert!(committed.is_legal_successor_of(&prepared));
    assert!(aborted.is_legal_successor_of(&prepared));
    assert!(published.is_legal_successor_of(&committed));
    assert!(!published.is_legal_successor_of(&prepared));
    assert!(!committed.is_legal_successor_of(&published));
    assert!(!published.is_legal_successor_of(&aborted));

    let second = Fixture::new(
        0x41,
        2,
        first.transaction.current_rollback_anchor(),
        first.challenge.proposed_head().to_bytes(),
    );
    let second_prepared = second.prepared();
    assert!(second_prepared.is_legal_successor_of(&published));
    assert!(!second_prepared.is_legal_successor_of(&aborted));
    assert!(!second_prepared.is_genesis_prepared());
}

#[test]
fn challenge_receipt_and_worker_identity_substitution_fail_closed() {
    let fixture = Fixture::new(0x51, 1, [0; 32], [0; 32]);
    let other = Fixture::new(0x61, 1, [0; 32], [0; 32]);
    assert!(matches!(
        CompilerExecutionWorkerAnchorJournalV1::prepared(
            fixture.transaction.clone(),
            other.challenge.clone(),
        ),
        Err(CompilerExecutionWorkerAnchorJournalErrorV1::ChallengeMismatch)
            | Err(CompilerExecutionWorkerAnchorJournalErrorV1::Anchor(_))
    ));
    assert!(matches!(
        fixture
            .prepared()
            .record_anchor_receipt(other.receipt(AnchorPositionV1::Proposed)),
        Err(CompilerExecutionWorkerAnchorJournalErrorV1::Anchor(_))
            | Err(CompilerExecutionWorkerAnchorJournalErrorV1::ReceiptMismatch)
    ));
    assert!(matches!(
        fixture.prepared().mark_published([0x81; 32]),
        Err(CompilerExecutionWorkerAnchorJournalErrorV1::IllegalTransition)
    ));
    let committed = fixture
        .prepared()
        .record_anchor_receipt(fixture.receipt(AnchorPositionV1::Proposed))
        .unwrap();
    assert!(matches!(
        committed.mark_published([0; 32]),
        Err(CompilerExecutionWorkerAnchorJournalErrorV1::IllegalTransition)
    ));
    let aborted = fixture
        .prepared()
        .record_anchor_receipt(fixture.receipt(AnchorPositionV1::Prior))
        .unwrap();
    assert!(matches!(
        aborted.mark_published([0x81; 32]),
        Err(CompilerExecutionWorkerAnchorJournalErrorV1::IllegalTransition)
    ));
}

#[test]
fn every_byte_mutation_and_wrong_length_rejects_for_every_stage() {
    let fixture = Fixture::new(0x71, 1, [0; 32], [0; 32]);
    let prepared = fixture.prepared();
    let committed = prepared
        .clone()
        .record_anchor_receipt(fixture.receipt(AnchorPositionV1::Proposed))
        .unwrap();
    let published = committed.clone().mark_published([0xb1; 32]).unwrap();
    let aborted = prepared
        .clone()
        .record_anchor_receipt(fixture.receipt(AnchorPositionV1::Prior))
        .unwrap();

    for record in [prepared, committed, published, aborted] {
        let bytes = record.canonical_bytes();
        for index in 0..bytes.len() {
            let mut mutated = bytes.to_vec();
            mutated[index] ^= 0x80;
            assert!(
                CompilerExecutionWorkerAnchorJournalV1::decode(&mutated).is_err(),
                "{:?} mutation at byte {index} was accepted",
                record.stage()
            );
        }
        assert!(CompilerExecutionWorkerAnchorJournalV1::decode(&bytes[..bytes.len() - 1]).is_err());
        let mut extended = bytes.to_vec();
        extended.push(0);
        assert!(CompilerExecutionWorkerAnchorJournalV1::decode(&extended).is_err());
    }
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
