//! Signed, crash-safe rollback journal for the protected compiler-execution issuer.

use std::error::Error;
use std::fmt;
use std::io;
use std::os::fd::OwnedFd;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use fe2o3_artifact_transaction::{
    INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1, InertCompilerExecutionSubjectV1,
    NoRetainedDurableDirectoryHooksV1, RetainedDurableDirectoryErrorV1,
    RetainedDurableDirectoryHooksV1, RetainedDurableDirectoryV1,
};
use fe2o3_runtime_protocol::{
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1, CompilerExecutionAttestationChallengeV1,
    CompilerExecutionAttestationErrorV1, CompilerExecutionAttestationReceiptIdentityV1,
    CompilerExecutionAttestationReceiptV1, CompilerExecutionAttestationRequestV1,
    CompilerExecutionIssuerPolicyV1,
};
use rustix::fs::{FlockOperation, Mode, OFlags, flock};
use sha2::{Digest, Sha256};

use crate::{
    ProtectedCompilerExecutionIssuerAdmissionErrorV1, ProtectedCompilerExecutionIssuerAdmissionV1,
    ProtectedCompilerExecutionOccurrenceErrorV1, ProtectedCompilerExecutionOccurrenceGuardV1,
    ProtectedCompilerExecutionOccurrenceV1,
};

const RECORD_MAGIC: [u8; 8] = *b"F2O3CEJ1";
const RECORD_VERSION: u16 = 1;
const HEADER_BYTES: usize = 24;
const STAGE_BYTES: usize = 8;
const SHA256_BYTES: usize = 32;
const SIGNATURE_BYTES: usize = 64;
const RECORD_SIGNED_PREFIX_BYTES: usize = HEADER_BYTES
    + STAGE_BYTES
    + SHA256_BYTES
    + 8
    + SHA256_BYTES
    + SHA256_BYTES
    + SHA256_BYTES
    + INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1
    + COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1
    + COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1
    + COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1;
const RECORD_IDENTITY_PREIMAGE_BYTES: usize = RECORD_SIGNED_PREFIX_BYTES + SIGNATURE_BYTES;
/// Exact byte length of one signed protected-issuer durable record.
pub const COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V1: usize =
    RECORD_IDENTITY_PREIMAGE_BYTES + SHA256_BYTES;

const RECORD_SIGNATURE_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-ISSUER-DURABLE-SIGNATURE/V1\0";
const RECORD_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-ISSUER-DURABLE-IDENTITY/V1\0";
const CANONICAL_RECORD: &str = "compiler-execution-issuer-v1.state";
const REDO_RECORD: &str = "compiler-execution-issuer-v1.redo";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum IssuerStageV1 {
    Ready = 1,
    Prepared = 2,
    Issued = 3,
}

impl IssuerStageV1 {
    fn decode(value: u8) -> Result<Self, ProtectedCompilerExecutionIssuerErrorV1> {
        match value {
            1 => Ok(Self::Ready),
            2 => Ok(Self::Prepared),
            3 => Ok(Self::Issued),
            _ => Err(ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
                "issuer journal has an unknown stage",
            )),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Prepared => "prepared",
            Self::Issued => "issued",
        }
    }
}

#[derive(Clone)]
struct IssuerRecordV1 {
    stage: IssuerStageV1,
    sequence: u64,
    prior_anchor: [u8; SHA256_BYTES],
    last_receipt: [u8; SHA256_BYTES],
    occurrence_identity: [u8; SHA256_BYTES],
    subject: Option<InertCompilerExecutionSubjectV1>,
    challenge: Option<CompilerExecutionAttestationChallengeV1>,
    request: Option<CompilerExecutionAttestationRequestV1>,
    receipt: Option<CompilerExecutionAttestationReceiptV1>,
    identity: [u8; SHA256_BYTES],
    canonical: [u8; COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V1],
}

impl fmt::Debug for IssuerRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IssuerRecordV1")
            .field("stage", &self.stage)
            .field("sequence", &self.sequence)
            .field("prior_anchor", &self.prior_anchor)
            .field("last_receipt", &self.last_receipt)
            .field("occurrence_identity", &self.occurrence_identity)
            .field(
                "request_identity",
                &self.request.as_ref().map(|request| request.identity()),
            )
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl IssuerRecordV1 {
    fn genesis(
        policy: &CompilerExecutionIssuerPolicyV1,
        signing_key: &SigningKey,
    ) -> Result<Self, ProtectedCompilerExecutionIssuerErrorV1> {
        Self::encode(
            IssuerStageV1::Ready,
            1,
            [0; SHA256_BYTES],
            [0; SHA256_BYTES],
            [0; SHA256_BYTES],
            None,
            None,
            None,
            None,
            policy,
            signing_key,
        )
    }

    fn prepare(
        &self,
        occurrence: &ProtectedCompilerExecutionOccurrenceGuardV1<'_>,
        nonce: [u8; SHA256_BYTES],
        policy: &CompilerExecutionIssuerPolicyV1,
        signing_key: &SigningKey,
    ) -> Result<Self, ProtectedCompilerExecutionIssuerErrorV1> {
        if self.stage != IssuerStageV1::Ready {
            return Err(ProtectedCompilerExecutionIssuerErrorV1::WrongStage {
                expected: "ready",
                actual: self.stage.name(),
            });
        }
        let challenge = CompilerExecutionAttestationChallengeV1::new(
            policy,
            occurrence.subject(),
            nonce,
            self.sequence,
            self.prior_anchor,
        )?;
        Self::encode(
            IssuerStageV1::Prepared,
            self.sequence,
            self.prior_anchor,
            self.last_receipt,
            *occurrence.identity(),
            Some(occurrence.subject().clone()),
            Some(challenge),
            None,
            None,
            policy,
            signing_key,
        )
    }

    fn issue(
        &self,
        occurrence: &ProtectedCompilerExecutionOccurrenceGuardV1<'_>,
        request_bytes: &[u8],
        policy: &CompilerExecutionIssuerPolicyV1,
        signing_key: &SigningKey,
    ) -> Result<Self, ProtectedCompilerExecutionIssuerErrorV1> {
        if self.stage != IssuerStageV1::Prepared {
            return Err(ProtectedCompilerExecutionIssuerErrorV1::WrongStage {
                expected: "prepared",
                actual: self.stage.name(),
            });
        }
        let request = CompilerExecutionAttestationRequestV1::decode(request_bytes)?;
        if self.occurrence_identity != *occurrence.identity()
            || self.subject.as_ref() != Some(occurrence.subject())
            || request.subject() != occurrence.subject()
        {
            return Err(ProtectedCompilerExecutionIssuerErrorV1::OccurrenceMismatch);
        }
        if self.challenge.as_ref() != Some(request.challenge()) {
            return Err(ProtectedCompilerExecutionIssuerErrorV1::RequestMismatch);
        }
        occurrence.revalidate_immediately_before_signing()?;
        let receipt = CompilerExecutionAttestationReceiptV1::issue(policy, &request, signing_key)?;
        Self::encode(
            IssuerStageV1::Issued,
            self.sequence,
            self.prior_anchor,
            self.last_receipt,
            self.occurrence_identity,
            self.subject.clone(),
            self.challenge.clone(),
            Some(request),
            Some(receipt),
            policy,
            signing_key,
        )
    }

    fn acknowledge(
        &self,
        receipt_identity: CompilerExecutionAttestationReceiptIdentityV1,
        policy: &CompilerExecutionIssuerPolicyV1,
        signing_key: &SigningKey,
    ) -> Result<(Self, CompilerExecutionIssuerAckV1), ProtectedCompilerExecutionIssuerErrorV1> {
        match self.stage {
            IssuerStageV1::Issued => {
                let receipt = self.receipt.as_ref().ok_or(
                    ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
                        "issued state has no receipt",
                    ),
                )?;
                if receipt.identity() != receipt_identity {
                    return Err(ProtectedCompilerExecutionIssuerErrorV1::ReceiptMismatch);
                }
                let next_sequence = self
                    .sequence
                    .checked_add(1)
                    .ok_or(ProtectedCompilerExecutionIssuerErrorV1::SequenceExhausted)?;
                let next = Self::encode(
                    IssuerStageV1::Ready,
                    next_sequence,
                    receipt.next_rollback_anchor(),
                    *receipt.identity().as_bytes(),
                    [0; SHA256_BYTES],
                    None,
                    None,
                    None,
                    None,
                    policy,
                    signing_key,
                )?;
                Ok((next, CompilerExecutionIssuerAckV1::Advanced))
            }
            IssuerStageV1::Ready
                if self.sequence > 1 && self.last_receipt == *receipt_identity.as_bytes() =>
            {
                Ok((
                    self.clone(),
                    CompilerExecutionIssuerAckV1::AlreadyAcknowledged,
                ))
            }
            _ => Err(ProtectedCompilerExecutionIssuerErrorV1::WrongStage {
                expected: "issued or matching acknowledged receipt",
                actual: self.stage.name(),
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn encode(
        stage: IssuerStageV1,
        sequence: u64,
        prior_anchor: [u8; SHA256_BYTES],
        last_receipt: [u8; SHA256_BYTES],
        occurrence_identity: [u8; SHA256_BYTES],
        subject: Option<InertCompilerExecutionSubjectV1>,
        challenge: Option<CompilerExecutionAttestationChallengeV1>,
        request: Option<CompilerExecutionAttestationRequestV1>,
        receipt: Option<CompilerExecutionAttestationReceiptV1>,
        policy: &CompilerExecutionIssuerPolicyV1,
        signing_key: &SigningKey,
    ) -> Result<Self, ProtectedCompilerExecutionIssuerErrorV1> {
        validate_position(sequence, prior_anchor, last_receipt)?;
        validate_stage_payload(
            stage,
            sequence,
            prior_anchor,
            occurrence_identity,
            subject.as_ref(),
            challenge.as_ref(),
            request.as_ref(),
            receipt.as_ref(),
            policy,
        )?;
        if signing_key.verifying_key().as_bytes() != policy.verifying_key() {
            return Err(ProtectedCompilerExecutionIssuerErrorV1::SigningKeyMismatch);
        }

        let mut canonical = [0_u8; COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V1];
        let mut offset = encode_header(&mut canonical);
        canonical[offset] = stage as u8;
        offset += STAGE_BYTES;
        put(&mut canonical, &mut offset, policy.identity().as_bytes());
        put(&mut canonical, &mut offset, &sequence.to_le_bytes());
        put(&mut canonical, &mut offset, &prior_anchor);
        put(&mut canonical, &mut offset, &last_receipt);
        put(&mut canonical, &mut offset, &occurrence_identity);
        put_optional(
            &mut canonical,
            &mut offset,
            subject
                .as_ref()
                .map(|value| value.canonical_bytes().as_slice()),
            INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1,
        );
        put_optional(
            &mut canonical,
            &mut offset,
            challenge
                .as_ref()
                .map(|value| value.canonical_bytes().as_slice()),
            COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1,
        );
        put_optional(
            &mut canonical,
            &mut offset,
            request
                .as_ref()
                .map(|value| value.canonical_bytes().as_slice()),
            COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1,
        );
        put_optional(
            &mut canonical,
            &mut offset,
            receipt
                .as_ref()
                .map(|value| value.canonical_bytes().as_slice()),
            COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1,
        );
        debug_assert_eq!(offset, RECORD_SIGNED_PREFIX_BYTES);
        let message = record_digest(RECORD_SIGNATURE_DOMAIN, &canonical[..offset]);
        let signature = signing_key.sign(&message).to_bytes();
        put(&mut canonical, &mut offset, &signature);
        debug_assert_eq!(offset, RECORD_IDENTITY_PREIMAGE_BYTES);
        let identity = record_digest(RECORD_IDENTITY_DOMAIN, &canonical[..offset]);
        put(&mut canonical, &mut offset, &identity);
        debug_assert_eq!(offset, canonical.len());
        Ok(Self {
            stage,
            sequence,
            prior_anchor,
            last_receipt,
            occurrence_identity,
            subject,
            challenge,
            request,
            receipt,
            identity,
            canonical,
        })
    }

    fn decode(
        bytes: &[u8],
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> Result<Self, ProtectedCompilerExecutionIssuerErrorV1> {
        if bytes.len() != COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V1 {
            return Err(ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
                "issuer journal has the wrong byte length",
            ));
        }
        let signature: [u8; SIGNATURE_BYTES] = bytes
            [RECORD_SIGNED_PREFIX_BYTES..RECORD_IDENTITY_PREIMAGE_BYTES]
            .try_into()
            .expect("fixed issuer record signature range");
        let verifying_key = VerifyingKey::from_bytes(policy.verifying_key())
            .map_err(|_| ProtectedCompilerExecutionIssuerErrorV1::SignatureRejected)?;
        let message = record_digest(
            RECORD_SIGNATURE_DOMAIN,
            &bytes[..RECORD_SIGNED_PREFIX_BYTES],
        );
        verifying_key
            .verify_strict(&message, &Signature::from_bytes(&signature))
            .map_err(|_| ProtectedCompilerExecutionIssuerErrorV1::SignatureRejected)?;

        let mut reader = Reader::new(bytes);
        decode_header(&mut reader)?;
        let stage = IssuerStageV1::decode(reader.u8()?)?;
        if reader.fixed::<7>()? != [0; 7] {
            return Err(ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
                "issuer journal stage padding is nonzero",
            ));
        }
        if reader.fixed::<32>()? != *policy.identity().as_bytes() {
            return Err(ProtectedCompilerExecutionIssuerErrorV1::PolicyMismatch);
        }
        let sequence = reader.u64()?;
        let prior_anchor = reader.fixed::<32>()?;
        let last_receipt = reader.fixed::<32>()?;
        let occurrence_identity = reader.fixed::<32>()?;
        let subject_bytes = reader.take(INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1)?;
        let challenge_bytes = reader.take(COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1)?;
        let request_bytes = reader.take(COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1)?;
        let receipt_bytes = reader.take(COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1)?;
        let observed_signature = reader.fixed::<SIGNATURE_BYTES>()?;
        let declared_identity = reader.fixed::<SHA256_BYTES>()?;
        if observed_signature != signature || !reader.is_empty() {
            return Err(ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
                "issuer journal signature position or trailing bytes changed",
            ));
        }
        let expected_identity = record_digest(
            RECORD_IDENTITY_DOMAIN,
            &bytes[..RECORD_IDENTITY_PREIMAGE_BYTES],
        );
        if declared_identity != expected_identity || declared_identity == [0; SHA256_BYTES] {
            return Err(ProtectedCompilerExecutionIssuerErrorV1::IdentityMismatch);
        }

        let (subject, challenge, request, receipt) = match stage {
            IssuerStageV1::Ready => {
                require_zero(subject_bytes, "ready subject")?;
                require_zero(challenge_bytes, "ready challenge")?;
                require_zero(request_bytes, "ready request")?;
                require_zero(receipt_bytes, "ready receipt")?;
                (None, None, None, None)
            }
            IssuerStageV1::Prepared => {
                let subject =
                    InertCompilerExecutionSubjectV1::decode(subject_bytes).map_err(|error| {
                        ProtectedCompilerExecutionIssuerErrorV1::Subject(error.to_string())
                    })?;
                let challenge = CompilerExecutionAttestationChallengeV1::decode(challenge_bytes)?;
                require_zero(request_bytes, "prepared request")?;
                require_zero(receipt_bytes, "prepared receipt")?;
                (Some(subject), Some(challenge), None, None)
            }
            IssuerStageV1::Issued => {
                let subject =
                    InertCompilerExecutionSubjectV1::decode(subject_bytes).map_err(|error| {
                        ProtectedCompilerExecutionIssuerErrorV1::Subject(error.to_string())
                    })?;
                let challenge = CompilerExecutionAttestationChallengeV1::decode(challenge_bytes)?;
                let request = CompilerExecutionAttestationRequestV1::decode(request_bytes)?;
                let receipt = CompilerExecutionAttestationReceiptV1::decode(receipt_bytes)?;
                (Some(subject), Some(challenge), Some(request), Some(receipt))
            }
        };
        validate_position(sequence, prior_anchor, last_receipt)?;
        validate_stage_payload(
            stage,
            sequence,
            prior_anchor,
            occurrence_identity,
            subject.as_ref(),
            challenge.as_ref(),
            request.as_ref(),
            receipt.as_ref(),
            policy,
        )?;
        Ok(Self {
            stage,
            sequence,
            prior_anchor,
            last_receipt,
            occurrence_identity,
            subject,
            challenge,
            request,
            receipt,
            identity: declared_identity,
            canonical: bytes
                .try_into()
                .expect("issuer journal length was checked before decoding"),
        })
    }

    fn is_legal_successor_of(&self, prior: &Self) -> bool {
        match (prior.stage, self.stage) {
            (IssuerStageV1::Ready, IssuerStageV1::Prepared) => {
                self.sequence == prior.sequence
                    && self.prior_anchor == prior.prior_anchor
                    && self.last_receipt == prior.last_receipt
            }
            (IssuerStageV1::Prepared, IssuerStageV1::Issued) => {
                self.sequence == prior.sequence
                    && self.prior_anchor == prior.prior_anchor
                    && self.last_receipt == prior.last_receipt
                    && self.subject == prior.subject
                    && self.occurrence_identity == prior.occurrence_identity
                    && self.challenge == prior.challenge
            }
            (IssuerStageV1::Issued, IssuerStageV1::Ready) => {
                prior.receipt.as_ref().is_some_and(|receipt| {
                    self.sequence == prior.sequence.checked_add(1).unwrap_or(0)
                        && self.prior_anchor == receipt.next_rollback_anchor()
                        && self.last_receipt == *receipt.identity().as_bytes()
                })
            }
            _ => false,
        }
    }
}

fn validate_position(
    sequence: u64,
    prior_anchor: [u8; SHA256_BYTES],
    last_receipt: [u8; SHA256_BYTES],
) -> Result<(), ProtectedCompilerExecutionIssuerErrorV1> {
    if sequence == 0
        || (sequence == 1 && (prior_anchor != [0; 32] || last_receipt != [0; 32]))
        || (sequence > 1 && (prior_anchor == [0; 32] || last_receipt == [0; 32]))
    {
        return Err(ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
            "issuer journal has a noncanonical rollback position",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_stage_payload(
    stage: IssuerStageV1,
    sequence: u64,
    prior_anchor: [u8; SHA256_BYTES],
    occurrence_identity: [u8; SHA256_BYTES],
    subject: Option<&InertCompilerExecutionSubjectV1>,
    challenge: Option<&CompilerExecutionAttestationChallengeV1>,
    request: Option<&CompilerExecutionAttestationRequestV1>,
    receipt: Option<&CompilerExecutionAttestationReceiptV1>,
    policy: &CompilerExecutionIssuerPolicyV1,
) -> Result<(), ProtectedCompilerExecutionIssuerErrorV1> {
    match stage {
        IssuerStageV1::Ready
            if occurrence_identity == [0; SHA256_BYTES]
                && subject.is_none()
                && challenge.is_none()
                && request.is_none()
                && receipt.is_none() =>
        {
            Ok(())
        }
        IssuerStageV1::Prepared | IssuerStageV1::Issued => {
            if occurrence_identity == [0; SHA256_BYTES] {
                return Err(ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
                    "active issuer journal has no compiler occurrence identity",
                ));
            }
            let subject = subject.ok_or(ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
                "active issuer journal has no subject",
            ))?;
            let challenge =
                challenge.ok_or(ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
                    "active issuer journal has no challenge",
                ))?;
            if challenge.policy_identity() != policy.identity()
                || !challenge.subject().matches_subject(subject)
                || challenge.sequence() != sequence
                || challenge.prior_rollback_anchor() != prior_anchor
            {
                return Err(ProtectedCompilerExecutionIssuerErrorV1::ChallengeMismatch);
            }
            if stage == IssuerStageV1::Prepared && request.is_none() && receipt.is_none() {
                return Ok(());
            }
            if stage != IssuerStageV1::Issued {
                return Err(ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
                    "prepared issuer journal contains terminal payload",
                ));
            }
            let request = request.ok_or(ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
                "issued issuer journal has no request",
            ))?;
            let receipt = receipt.ok_or(ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
                "issued issuer journal has no receipt",
            ))?;
            if request.challenge() != challenge || request.subject() != subject {
                return Err(ProtectedCompilerExecutionIssuerErrorV1::RequestMismatch);
            }
            receipt
                .clone()
                .verify(policy, request, prior_anchor)
                .map_err(ProtectedCompilerExecutionIssuerErrorV1::Protocol)?;
            Ok(())
        }
        _ => Err(ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
            "issuer journal stage and payload disagree",
        )),
    }
}

struct IssuerLedgerV1 {
    store: RetainedDurableDirectoryV1,
    _singleton_lock: SingletonLockV1,
    record: IssuerRecordV1,
    poisoned: bool,
}

impl IssuerLedgerV1 {
    fn recover(
        service_root: OwnedFd,
        policy: &CompilerExecutionIssuerPolicyV1,
        signing_key: &SigningKey,
    ) -> Result<Self, ProtectedCompilerExecutionIssuerErrorV1> {
        let mut hooks = NoRetainedDurableDirectoryHooksV1;
        Self::recover_with_hooks(service_root, policy, signing_key, &mut hooks)
    }

    fn recover_with_hooks(
        service_root: OwnedFd,
        policy: &CompilerExecutionIssuerPolicyV1,
        signing_key: &SigningKey,
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
    ) -> Result<Self, ProtectedCompilerExecutionIssuerErrorV1> {
        let singleton_lock = acquire_singleton_lock(&service_root)?;
        let store = RetainedDurableDirectoryV1::admit_service_owned(service_root)?;
        let canonical_bytes = store.read_private(
            CANONICAL_RECORD,
            COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V1,
        )?;
        let redo_bytes = store.read_private(
            REDO_RECORD,
            COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V1,
        )?;

        let record = match (canonical_bytes, redo_bytes) {
            (None, None) => {
                let genesis = IssuerRecordV1::genesis(policy, signing_key)?;
                store.commit_record(
                    CANONICAL_RECORD,
                    REDO_RECORD,
                    &genesis.canonical,
                    COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V1,
                    hooks,
                )?;
                genesis
            }
            (canonical, Some(redo_bytes)) => {
                let redo = IssuerRecordV1::decode(&redo_bytes, policy)?;
                let canonical_record = canonical
                    .as_deref()
                    .map(|bytes| IssuerRecordV1::decode(bytes, policy))
                    .transpose()?;
                let legal = canonical_record.as_ref().map_or_else(
                    || {
                        redo.stage == IssuerStageV1::Ready
                            && redo.sequence == 1
                            && redo.prior_anchor == [0; 32]
                    },
                    |prior| redo.is_legal_successor_of(prior),
                );
                if !legal {
                    return Err(ProtectedCompilerExecutionIssuerErrorV1::IllegalSuccessor);
                }
                store.promote_validated_redo(
                    CANONICAL_RECORD,
                    REDO_RECORD,
                    canonical.as_deref(),
                    &redo_bytes,
                    COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V1,
                    hooks,
                )?;
                redo
            }
            (Some(canonical_bytes), None) => {
                let record = IssuerRecordV1::decode(&canonical_bytes, policy)?;
                store.establish_recovered_record_durability(
                    CANONICAL_RECORD,
                    REDO_RECORD,
                    &canonical_bytes,
                    COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V1,
                    hooks,
                )?;
                record
            }
        };
        Ok(Self {
            store,
            _singleton_lock: singleton_lock,
            record,
            poisoned: false,
        })
    }

    fn commit(
        &mut self,
        next: IssuerRecordV1,
    ) -> Result<(), ProtectedCompilerExecutionIssuerErrorV1> {
        let mut hooks = NoRetainedDurableDirectoryHooksV1;
        self.commit_with_hooks(next, &mut hooks)
    }

    fn commit_with_hooks(
        &mut self,
        next: IssuerRecordV1,
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
    ) -> Result<(), ProtectedCompilerExecutionIssuerErrorV1> {
        if self.poisoned {
            return Err(ProtectedCompilerExecutionIssuerErrorV1::Poisoned);
        }
        if !next.is_legal_successor_of(&self.record) {
            return Err(ProtectedCompilerExecutionIssuerErrorV1::IllegalSuccessor);
        }
        if let Err(error) = self.store.commit_record(
            CANONICAL_RECORD,
            REDO_RECORD,
            &next.canonical,
            COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V1,
            hooks,
        ) {
            self.poisoned = true;
            return Err(error.into());
        }
        self.record = next;
        Ok(())
    }

    fn recovery(&self) -> CompilerExecutionIssuerRecoveryV1 {
        match self.record.stage {
            IssuerStageV1::Ready => CompilerExecutionIssuerRecoveryV1::Ready {
                next_sequence: self.record.sequence,
                current_rollback_anchor: self.record.prior_anchor,
            },
            IssuerStageV1::Prepared => CompilerExecutionIssuerRecoveryV1::Prepared {
                challenge: self
                    .record
                    .challenge
                    .clone()
                    .expect("validated prepared record has a challenge"),
            },
            IssuerStageV1::Issued => CompilerExecutionIssuerRecoveryV1::Issued {
                receipt: self
                    .record
                    .receipt
                    .clone()
                    .expect("validated issued record has a receipt"),
            },
        }
    }
}

struct SingletonLockV1 {
    descriptor: OwnedFd,
}

impl Drop for SingletonLockV1 {
    fn drop(&mut self) {
        // Explicitly unlock the shared open-file description before close, including transient
        // fork inheritance that has not reached close-on-exec yet.
        let _ = flock(&self.descriptor, FlockOperation::Unlock);
    }
}

fn acquire_singleton_lock(
    service_root: &OwnedFd,
) -> Result<SingletonLockV1, ProtectedCompilerExecutionIssuerErrorV1> {
    let lock = rustix::fs::openat(
        service_root,
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|error| {
        ProtectedCompilerExecutionIssuerErrorV1::SingletonLock(io::Error::from(error))
    })?;
    let root_stat = rustix::fs::fstat(service_root).map_err(|error| {
        ProtectedCompilerExecutionIssuerErrorV1::SingletonLock(io::Error::from(error))
    })?;
    let lock_stat = rustix::fs::fstat(&lock).map_err(|error| {
        ProtectedCompilerExecutionIssuerErrorV1::SingletonLock(io::Error::from(error))
    })?;
    if (root_stat.st_dev, root_stat.st_ino) != (lock_stat.st_dev, lock_stat.st_ino) {
        return Err(ProtectedCompilerExecutionIssuerErrorV1::SingletonLock(
            io::Error::other("issuer lock descriptor does not name the retained service root"),
        ));
    }
    let descriptor_flags = rustix::io::fcntl_getfd(&lock).map_err(|error| {
        ProtectedCompilerExecutionIssuerErrorV1::SingletonLock(io::Error::from(error))
    })?;
    if !descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC) {
        return Err(ProtectedCompilerExecutionIssuerErrorV1::SingletonLock(
            io::Error::other("issuer lock descriptor lacks FD_CLOEXEC"),
        ));
    }
    flock(&lock, FlockOperation::NonBlockingLockExclusive).map_err(|error| {
        ProtectedCompilerExecutionIssuerErrorV1::SingletonLock(io::Error::from(error))
    })?;
    Ok(SingletonLockV1 { descriptor: lock })
}

/// Move-only challenge released only after its complete prepared record is durable.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedCompilerExecutionChallengeV1;
/// fn duplicate(value: ProtectedCompilerExecutionChallengeV1) { let _ = value.clone(); }
/// ```
pub struct ProtectedCompilerExecutionChallengeV1 {
    challenge: CompilerExecutionAttestationChallengeV1,
}

impl ProtectedCompilerExecutionChallengeV1 {
    /// Returns the exact canonical challenge.
    pub const fn challenge(&self) -> &CompilerExecutionAttestationChallengeV1 {
        &self.challenge
    }
}

/// Move-only protected receipt released only after its complete issued record is durable.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedCompilerExecutionReceiptV1;
/// fn duplicate(value: ProtectedCompilerExecutionReceiptV1) { let _ = value.clone(); }
/// ```
pub struct ProtectedCompilerExecutionReceiptV1 {
    receipt: CompilerExecutionAttestationReceiptV1,
}

impl ProtectedCompilerExecutionReceiptV1 {
    /// Returns the exact canonical signed receipt.
    pub const fn receipt(&self) -> &CompilerExecutionAttestationReceiptV1 {
        &self.receipt
    }

    /// Reports that this wrapper was produced from a protected supervised-occurrence token.
    pub const fn authenticates_protected_compiler_execution(&self) -> bool {
        true
    }

    /// Compiler authority still requires the Worker V3 sealed-verifier join.
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
}

/// Inert restart state. Prepared challenges and issued receipts are exact re-emissions.
/// The bounded values remain inline so recovery introduces no additional fallible allocation.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompilerExecutionIssuerRecoveryV1 {
    Ready {
        next_sequence: u64,
        current_rollback_anchor: [u8; SHA256_BYTES],
    },
    Prepared {
        challenge: CompilerExecutionAttestationChallengeV1,
    },
    Issued {
        receipt: CompilerExecutionAttestationReceiptV1,
    },
}

/// Result of durably acknowledging an issued receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerExecutionIssuerAckV1 {
    Advanced,
    AlreadyAcknowledged,
}

/// Single crash-safe protected compiler-execution issuer.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedCompilerExecutionIssuerV1;
/// fn duplicate(value: ProtectedCompilerExecutionIssuerV1) { let _ = value.clone(); }
/// ```
pub struct ProtectedCompilerExecutionIssuerV1 {
    admission: ProtectedCompilerExecutionIssuerAdmissionV1,
    ledger: IssuerLedgerV1,
}

impl ProtectedCompilerExecutionIssuerV1 {
    /// Acquires the singleton ledger and recovers one exact durable state.
    pub fn admit(
        admission: ProtectedCompilerExecutionIssuerAdmissionV1,
    ) -> Result<(Self, CompilerExecutionIssuerRecoveryV1), ProtectedCompilerExecutionIssuerErrorV1>
    {
        admission.validate_continuity()?;
        let root = admission.try_clone_service_root()?;
        let ledger = IssuerLedgerV1::recover(root, admission.policy(), admission.signing_key())?;
        admission.validate_continuity()?;
        let recovery = ledger.recovery();
        Ok((Self { admission, ledger }, recovery))
    }

    /// Generates and durably commits a fresh subject-bound challenge before returning it.
    pub fn prepare_challenge(
        &mut self,
    ) -> Result<ProtectedCompilerExecutionChallengeV1, ProtectedCompilerExecutionIssuerErrorV1>
    {
        self.admission.validate_continuity()?;
        let occurrence = ProtectedCompilerExecutionOccurrenceV1::observe_current(
            self.admission.service_admission(),
        )?;
        let occurrence_guard = occurrence.acquire_for_issuer()?;
        let nonce = generate_nonce()?;
        occurrence_guard.revalidate_immediately_before_signing()?;
        let next = self.ledger.record.prepare(
            &occurrence_guard,
            nonce,
            self.admission.policy(),
            self.admission.signing_key(),
        )?;
        let challenge = next
            .challenge
            .clone()
            .expect("prepared record construction returns a challenge");
        self.admission.validate_continuity()?;
        self.ledger.commit(next)?;
        self.admission.validate_continuity()?;
        drop(occurrence_guard);
        Ok(ProtectedCompilerExecutionChallengeV1 { challenge })
    }

    /// Compares one exact request with a fresh supervised occurrence, signs it, and durably
    /// commits the complete receipt before release.
    pub fn issue_receipt(
        &mut self,
        request_bytes: &[u8],
    ) -> Result<ProtectedCompilerExecutionReceiptV1, ProtectedCompilerExecutionIssuerErrorV1> {
        self.admission.validate_continuity()?;
        let occurrence = ProtectedCompilerExecutionOccurrenceV1::observe_current(
            self.admission.service_admission(),
        )?;
        let occurrence_guard = occurrence.acquire_for_issuer()?;
        let next = self.ledger.record.issue(
            &occurrence_guard,
            request_bytes,
            self.admission.policy(),
            self.admission.signing_key(),
        )?;
        let receipt = next
            .receipt
            .clone()
            .expect("issued record construction returns a receipt");
        self.admission.validate_continuity()?;
        self.ledger.commit(next)?;
        self.admission.validate_continuity()?;
        drop(occurrence_guard);
        Ok(ProtectedCompilerExecutionReceiptV1 { receipt })
    }

    /// Durably advances to the receipt's next rollback position. Repeating the exact
    /// acknowledgment after a lost response is idempotent.
    pub fn acknowledge_receipt(
        &mut self,
        receipt_identity: CompilerExecutionAttestationReceiptIdentityV1,
    ) -> Result<CompilerExecutionIssuerAckV1, ProtectedCompilerExecutionIssuerErrorV1> {
        self.admission.validate_continuity()?;
        let (next, outcome) = self.ledger.record.acknowledge(
            receipt_identity,
            self.admission.policy(),
            self.admission.signing_key(),
        )?;
        if outcome == CompilerExecutionIssuerAckV1::Advanced {
            self.admission.validate_continuity()?;
            self.ledger.commit(next)?;
        }
        self.admission.validate_continuity()?;
        Ok(outcome)
    }

    /// Returns the current inert restart output without changing durable state.
    pub fn recovery(&self) -> CompilerExecutionIssuerRecoveryV1 {
        self.ledger.recovery()
    }
}

fn generate_nonce() -> Result<[u8; SHA256_BYTES], ProtectedCompilerExecutionIssuerErrorV1> {
    const MAX_ATTEMPTS: usize = 4;
    for _ in 0..MAX_ATTEMPTS {
        let mut nonce = [0_u8; SHA256_BYTES];
        let mut filled = 0;
        while filled < nonce.len() {
            let count = rustix::rand::getrandom(
                &mut nonce[filled..],
                rustix::rand::GetRandomFlags::empty(),
            )
            .map_err(|error| {
                ProtectedCompilerExecutionIssuerErrorV1::Entropy(io::Error::from_raw_os_error(
                    error.raw_os_error(),
                ))
            })?;
            if count == 0 {
                return Err(ProtectedCompilerExecutionIssuerErrorV1::Entropy(
                    io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Linux getrandom returned zero bytes",
                    ),
                ));
            }
            filled += count;
        }
        if nonce != [0; SHA256_BYTES] {
            return Ok(nonce);
        }
    }
    Err(ProtectedCompilerExecutionIssuerErrorV1::Entropy(
        io::Error::other("Linux getrandom returned only inadmissible issuer nonces"),
    ))
}

fn encode_header(output: &mut [u8]) -> usize {
    let mut offset = 0;
    put(output, &mut offset, &RECORD_MAGIC);
    put(output, &mut offset, &RECORD_VERSION.to_le_bytes());
    put(output, &mut offset, &0_u16.to_le_bytes());
    put(
        output,
        &mut offset,
        &(COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V1 as u64).to_le_bytes(),
    );
    put(output, &mut offset, &0_u32.to_le_bytes());
    offset
}

fn decode_header(reader: &mut Reader<'_>) -> Result<(), ProtectedCompilerExecutionIssuerErrorV1> {
    if reader.fixed::<8>()? != RECORD_MAGIC
        || reader.u16()? != RECORD_VERSION
        || reader.u16()? != 0
        || reader.u64()? != COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V1 as u64
        || reader.fixed::<4>()? != [0; 4]
    {
        return Err(ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
            "issuer journal header is not canonical",
        ));
    }
    Ok(())
}

fn put(output: &mut [u8], offset: &mut usize, value: &[u8]) {
    let end = *offset + value.len();
    output[*offset..end].copy_from_slice(value);
    *offset = end;
}

fn put_optional(output: &mut [u8], offset: &mut usize, value: Option<&[u8]>, exact_length: usize) {
    if let Some(value) = value {
        debug_assert_eq!(value.len(), exact_length);
        put(output, offset, value);
    } else {
        *offset += exact_length;
    }
}

fn record_digest(domain: &[u8], bytes: &[u8]) -> [u8; SHA256_BYTES] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn require_zero(
    bytes: &[u8],
    field: &'static str,
) -> Result<(), ProtectedCompilerExecutionIssuerErrorV1> {
    if bytes.iter().any(|byte| *byte != 0) {
        return Err(ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
            field,
        ));
    }
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ProtectedCompilerExecutionIssuerErrorV1> {
        let end = self.offset.checked_add(length).ok_or(
            ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
                "issuer journal offset overflow",
            ),
        )?;
        let value = self.bytes.get(self.offset..end).ok_or(
            ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord("issuer journal is truncated"),
        )?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], ProtectedCompilerExecutionIssuerErrorV1> {
        self.take(N)?.try_into().map_err(|_| {
            ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord("issuer journal is truncated")
        })
    }

    fn u8(&mut self) -> Result<u8, ProtectedCompilerExecutionIssuerErrorV1> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, ProtectedCompilerExecutionIssuerErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, ProtectedCompilerExecutionIssuerErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    const fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

/// Protected issuer service or durable-state failure.
#[derive(Debug)]
pub enum ProtectedCompilerExecutionIssuerErrorV1 {
    Admission(ProtectedCompilerExecutionIssuerAdmissionErrorV1),
    Durable(RetainedDurableDirectoryErrorV1),
    Protocol(CompilerExecutionAttestationErrorV1),
    Occurrence(ProtectedCompilerExecutionOccurrenceErrorV1),
    SingletonLock(io::Error),
    Entropy(io::Error),
    Subject(String),
    InvalidRecord(&'static str),
    SignatureRejected,
    IdentityMismatch,
    PolicyMismatch,
    SigningKeyMismatch,
    ChallengeMismatch,
    RequestMismatch,
    ReceiptMismatch,
    OccurrenceMismatch,
    IllegalSuccessor,
    SequenceExhausted,
    Poisoned,
    WrongStage {
        expected: &'static str,
        actual: &'static str,
    },
}

impl fmt::Display for ProtectedCompilerExecutionIssuerErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(error) => write!(formatter, "issuer admission failed: {error}"),
            Self::Durable(error) => write!(formatter, "issuer durable state failed: {error}"),
            Self::Protocol(error) => write!(formatter, "issuer protocol failed: {error}"),
            Self::Occurrence(error) => {
                write!(formatter, "compiler occurrence validation failed: {error}")
            }
            Self::SingletonLock(error) => {
                write!(formatter, "issuer singleton lock failed: {error}")
            }
            Self::Entropy(error) => write!(formatter, "issuer entropy failed: {error}"),
            Self::Subject(error) => write!(formatter, "issuer subject failed: {error}"),
            Self::InvalidRecord(reason) => write!(formatter, "invalid issuer journal: {reason}"),
            Self::SignatureRejected => formatter.write_str("issuer journal signature rejected"),
            Self::IdentityMismatch => formatter.write_str("issuer journal identity mismatch"),
            Self::PolicyMismatch => formatter.write_str("issuer journal policy mismatch"),
            Self::SigningKeyMismatch => formatter.write_str("issuer signing key mismatch"),
            Self::ChallengeMismatch => formatter.write_str("issuer challenge mismatch"),
            Self::RequestMismatch => formatter.write_str("issuer request mismatch"),
            Self::ReceiptMismatch => formatter.write_str("issuer receipt mismatch"),
            Self::OccurrenceMismatch => {
                formatter.write_str("issuer occurrence identity or subject mismatch")
            }
            Self::IllegalSuccessor => {
                formatter.write_str("issuer journal has an illegal successor")
            }
            Self::SequenceExhausted => formatter.write_str("issuer rollback sequence is exhausted"),
            Self::Poisoned => formatter.write_str("issuer ledger is poisoned and requires restart"),
            Self::WrongStage { expected, actual } => {
                write!(
                    formatter,
                    "issuer expected {expected} state, found {actual}"
                )
            }
        }
    }
}

impl Error for ProtectedCompilerExecutionIssuerErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error),
            Self::Durable(error) => Some(error),
            Self::Protocol(error) => Some(error),
            Self::Occurrence(error) => Some(error),
            Self::SingletonLock(error) | Self::Entropy(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ProtectedCompilerExecutionOccurrenceErrorV1> for ProtectedCompilerExecutionIssuerErrorV1 {
    fn from(error: ProtectedCompilerExecutionOccurrenceErrorV1) -> Self {
        Self::Occurrence(error)
    }
}

impl From<ProtectedCompilerExecutionIssuerAdmissionErrorV1>
    for ProtectedCompilerExecutionIssuerErrorV1
{
    fn from(error: ProtectedCompilerExecutionIssuerAdmissionErrorV1) -> Self {
        Self::Admission(error)
    }
}

impl From<RetainedDurableDirectoryErrorV1> for ProtectedCompilerExecutionIssuerErrorV1 {
    fn from(error: RetainedDurableDirectoryErrorV1) -> Self {
        Self::Durable(error)
    }
}

impl From<CompilerExecutionAttestationErrorV1> for ProtectedCompilerExecutionIssuerErrorV1 {
    fn from(error: CompilerExecutionAttestationErrorV1) -> Self {
        Self::Protocol(error)
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::os::unix::fs::PermissionsExt;

    use fe2o3_artifact_transaction::{
        INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1, INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1,
        RetainedDurableFaultTimingV1, RetainedDurableRecordBoundaryV1,
    };
    use tempfile::TempDir;

    use super::*;

    const SUBJECT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V1\0";
    const COMPILER_CLOSURE_IDENTITY_DOMAIN: &[u8] = b"fe2o3-compiler-closure-identity-v2\0";
    const RECORD_BOUNDARIES: [RetainedDurableRecordBoundaryV1; 7] = [
        RetainedDurableRecordBoundaryV1::CreateTemp,
        RetainedDurableRecordBoundaryV1::WriteTemp,
        RetainedDurableRecordBoundaryV1::SyncTemp,
        RetainedDurableRecordBoundaryV1::RenameTempToRedo,
        RetainedDurableRecordBoundaryV1::SyncRedoName,
        RetainedDurableRecordBoundaryV1::RenameRedoToCanonical,
        RetainedDurableRecordBoundaryV1::SyncCanonicalName,
    ];
    const FAULT_TIMINGS: [RetainedDurableFaultTimingV1; 2] = [
        RetainedDurableFaultTimingV1::Before,
        RetainedDurableFaultTimingV1::After,
    ];

    struct RecordFault {
        boundary: RetainedDurableRecordBoundaryV1,
        timing: RetainedDurableFaultTimingV1,
        fired: bool,
    }

    impl RecordFault {
        const fn new(
            boundary: RetainedDurableRecordBoundaryV1,
            timing: RetainedDurableFaultTimingV1,
        ) -> Self {
            Self {
                boundary,
                timing,
                fired: false,
            }
        }
    }

    impl RetainedDurableDirectoryHooksV1 for RecordFault {
        fn record(
            &mut self,
            boundary: RetainedDurableRecordBoundaryV1,
            timing: RetainedDurableFaultTimingV1,
        ) -> io::Result<()> {
            if boundary == self.boundary && timing == self.timing {
                self.fired = true;
                Err(io::Error::other("injected issuer journal crash"))
            } else {
                Ok(())
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum Transition {
        Prepare,
        Issue,
        Acknowledge,
    }

    struct Fixture {
        directory: TempDir,
        signing_key: SigningKey,
        policy: CompilerExecutionIssuerPolicyV1,
        subject: InertCompilerExecutionSubjectV1,
    }

    impl Fixture {
        fn new() -> Self {
            let signing_key = SigningKey::from_bytes(&[0x51; 32]);
            let measurement = fe2o3_runtime_protocol::CompilerExecutionIssuerMeasurementV1::new(
                [0x61; 32], 12_345,
            )
            .unwrap();
            let policy = CompilerExecutionIssuerPolicyV1::new(
                7,
                measurement,
                measurement,
                signing_key.verifying_key().to_bytes(),
            )
            .unwrap();
            let directory = TempDir::new().unwrap();
            std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
                .unwrap();
            Self {
                directory,
                signing_key,
                policy,
                subject: subject(0x20),
            }
        }

        fn root(&self) -> OwnedFd {
            File::open(self.directory.path()).unwrap().into()
        }

        fn occurrence(&self) -> ProtectedCompilerExecutionOccurrenceV1 {
            self.occurrence_with_identity([0x91; 32])
        }

        fn occurrence_with_identity(
            &self,
            identity: [u8; SHA256_BYTES],
        ) -> ProtectedCompilerExecutionOccurrenceV1 {
            ProtectedCompilerExecutionOccurrenceV1::from_supervised_subject_for_test(
                self.subject.clone(),
                identity,
            )
            .unwrap()
        }

        fn prepare(
            &self,
            record: &IssuerRecordV1,
            nonce: [u8; SHA256_BYTES],
        ) -> Result<IssuerRecordV1, ProtectedCompilerExecutionIssuerErrorV1> {
            let occurrence = self.occurrence();
            let guard = occurrence.acquire_for_issuer()?;
            record.prepare(&guard, nonce, &self.policy, &self.signing_key)
        }

        fn issue(
            &self,
            record: &IssuerRecordV1,
            request_bytes: &[u8],
        ) -> Result<IssuerRecordV1, ProtectedCompilerExecutionIssuerErrorV1> {
            let occurrence = self.occurrence();
            let guard = occurrence.acquire_for_issuer()?;
            record.issue(&guard, request_bytes, &self.policy, &self.signing_key)
        }
    }

    #[test]
    fn record_sizes_and_all_stages_round_trip() {
        assert_eq!(COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V1, 2500);
        let fixture = Fixture::new();
        let ready = IssuerRecordV1::genesis(&fixture.policy, &fixture.signing_key).unwrap();
        let prepared = fixture.prepare(&ready, [0x71; 32]).unwrap();
        let request = CompilerExecutionAttestationRequestV1::new(
            prepared.challenge.clone().unwrap(),
            fixture.subject.clone(),
        )
        .unwrap();
        let issued = fixture.issue(&prepared, request.canonical_bytes()).unwrap();
        let (next, outcome) = issued
            .acknowledge(
                issued.receipt.as_ref().unwrap().identity(),
                &fixture.policy,
                &fixture.signing_key,
            )
            .unwrap();
        assert_eq!(outcome, CompilerExecutionIssuerAckV1::Advanced);

        for record in [&ready, &prepared, &issued, &next] {
            let decoded = IssuerRecordV1::decode(&record.canonical, &fixture.policy).unwrap();
            assert_eq!(decoded.stage, record.stage);
            assert_eq!(decoded.sequence, record.sequence);
            assert_eq!(decoded.identity, record.identity);
            assert_eq!(decoded.canonical, record.canonical);
        }
        assert_eq!(next.sequence, 2);
        assert_eq!(
            next.prior_anchor,
            issued.receipt.as_ref().unwrap().next_rollback_anchor()
        );
    }

    #[test]
    fn every_issued_record_byte_mutation_rejects() {
        let fixture = Fixture::new();
        let ready = IssuerRecordV1::genesis(&fixture.policy, &fixture.signing_key).unwrap();
        let prepared = fixture.prepare(&ready, [0x71; 32]).unwrap();
        let request = CompilerExecutionAttestationRequestV1::new(
            prepared.challenge.clone().unwrap(),
            fixture.subject.clone(),
        )
        .unwrap();
        let issued = fixture.issue(&prepared, request.canonical_bytes()).unwrap();
        for index in 0..issued.canonical.len() {
            let mut mutated = issued.canonical;
            mutated[index] ^= 0x80;
            assert!(
                IssuerRecordV1::decode(&mutated, &fixture.policy).is_err(),
                "mutation at byte {index} was accepted"
            );
        }
    }

    #[test]
    fn truncation_extension_policy_and_key_substitution_reject() {
        let fixture = Fixture::new();
        let record = IssuerRecordV1::genesis(&fixture.policy, &fixture.signing_key).unwrap();
        assert!(
            IssuerRecordV1::decode(
                &record.canonical[..record.canonical.len() - 1],
                &fixture.policy
            )
            .is_err()
        );
        let mut extended = record.canonical.to_vec();
        extended.push(0);
        assert!(IssuerRecordV1::decode(&extended, &fixture.policy).is_err());

        let wrong_key = SigningKey::from_bytes(&[0x52; 32]);
        let measurement =
            fe2o3_runtime_protocol::CompilerExecutionIssuerMeasurementV1::new([0x61; 32], 12_345)
                .unwrap();
        let wrong_policy = CompilerExecutionIssuerPolicyV1::new(
            7,
            measurement,
            measurement,
            wrong_key.verifying_key().to_bytes(),
        )
        .unwrap();
        assert!(IssuerRecordV1::decode(&record.canonical, &wrong_policy).is_err());
        assert!(matches!(
            IssuerRecordV1::genesis(&fixture.policy, &wrong_key),
            Err(ProtectedCompilerExecutionIssuerErrorV1::SigningKeyMismatch)
        ));
    }

    #[test]
    fn every_genesis_crash_boundary_recovers_one_exact_ready_record() {
        for boundary in RECORD_BOUNDARIES {
            for timing in FAULT_TIMINGS {
                let fixture = Fixture::new();
                let mut fault = RecordFault::new(boundary, timing);
                assert!(
                    IssuerLedgerV1::recover_with_hooks(
                        fixture.root(),
                        &fixture.policy,
                        &fixture.signing_key,
                        &mut fault,
                    )
                    .is_err(),
                    "genesis unexpectedly succeeded at {boundary:?}/{timing:?}"
                );
                assert!(fault.fired, "fault did not fire at {boundary:?}/{timing:?}");

                let recovered =
                    IssuerLedgerV1::recover(fixture.root(), &fixture.policy, &fixture.signing_key)
                        .unwrap();
                assert_eq!(recovered.record.stage, IssuerStageV1::Ready);
                assert_eq!(recovered.record.sequence, 1);
                assert_eq!(recovered.record.prior_anchor, [0; 32]);
            }
        }
    }

    #[test]
    fn every_transition_crash_boundary_recovers_only_prior_or_successor() {
        for transition in [
            Transition::Prepare,
            Transition::Issue,
            Transition::Acknowledge,
        ] {
            for boundary in RECORD_BOUNDARIES {
                for timing in FAULT_TIMINGS {
                    let fixture = Fixture::new();
                    let mut ledger = IssuerLedgerV1::recover(
                        fixture.root(),
                        &fixture.policy,
                        &fixture.signing_key,
                    )
                    .unwrap();
                    let prepared = fixture.prepare(&ledger.record, [0x76; 32]).unwrap();
                    let next = match transition {
                        Transition::Prepare => prepared,
                        Transition::Issue | Transition::Acknowledge => {
                            ledger.commit(prepared).unwrap();
                            let request = CompilerExecutionAttestationRequestV1::new(
                                ledger.record.challenge.clone().unwrap(),
                                fixture.subject.clone(),
                            )
                            .unwrap();
                            let issued = fixture
                                .issue(&ledger.record, request.canonical_bytes())
                                .unwrap();
                            match transition {
                                Transition::Issue => issued,
                                Transition::Acknowledge => {
                                    ledger.commit(issued).unwrap();
                                    ledger
                                        .record
                                        .acknowledge(
                                            ledger.record.receipt.as_ref().unwrap().identity(),
                                            &fixture.policy,
                                            &fixture.signing_key,
                                        )
                                        .unwrap()
                                        .0
                                }
                                Transition::Prepare => unreachable!(),
                            }
                        }
                    };
                    let prior_bytes = ledger.record.canonical;
                    let successor_bytes = next.canonical;
                    let mut fault = RecordFault::new(boundary, timing);
                    assert!(
                        ledger.commit_with_hooks(next.clone(), &mut fault).is_err(),
                        "{transition:?} unexpectedly succeeded at {boundary:?}/{timing:?}"
                    );
                    assert!(fault.fired, "fault did not fire at {boundary:?}/{timing:?}");
                    assert!(ledger.poisoned);
                    assert!(matches!(
                        ledger.commit(next),
                        Err(ProtectedCompilerExecutionIssuerErrorV1::Poisoned)
                    ));
                    drop(ledger);

                    let recovered = IssuerLedgerV1::recover(
                        fixture.root(),
                        &fixture.policy,
                        &fixture.signing_key,
                    )
                    .unwrap();
                    assert!(
                        recovered.record.canonical == prior_bytes
                            || recovered.record.canonical == successor_bytes,
                        "{transition:?} recovered a third state at {boundary:?}/{timing:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn durable_lifecycle_reemits_exact_challenge_and_receipt_after_restart() {
        let fixture = Fixture::new();
        let mut ledger =
            IssuerLedgerV1::recover(fixture.root(), &fixture.policy, &fixture.signing_key).unwrap();
        assert_eq!(
            ledger.recovery(),
            CompilerExecutionIssuerRecoveryV1::Ready {
                next_sequence: 1,
                current_rollback_anchor: [0; 32],
            }
        );
        let prepared = fixture.prepare(&ledger.record, [0x72; 32]).unwrap();
        let challenge = prepared.challenge.clone().unwrap();
        ledger.commit(prepared).unwrap();
        drop(ledger);

        let mut ledger =
            IssuerLedgerV1::recover(fixture.root(), &fixture.policy, &fixture.signing_key).unwrap();
        assert_eq!(
            ledger.recovery(),
            CompilerExecutionIssuerRecoveryV1::Prepared {
                challenge: challenge.clone()
            }
        );
        let request =
            CompilerExecutionAttestationRequestV1::new(challenge, fixture.subject.clone()).unwrap();
        let issued = fixture
            .issue(&ledger.record, request.canonical_bytes())
            .unwrap();
        let receipt = issued.receipt.clone().unwrap();
        ledger.commit(issued).unwrap();
        drop(ledger);

        let mut ledger =
            IssuerLedgerV1::recover(fixture.root(), &fixture.policy, &fixture.signing_key).unwrap();
        assert_eq!(
            ledger.recovery(),
            CompilerExecutionIssuerRecoveryV1::Issued {
                receipt: receipt.clone()
            }
        );
        receipt
            .clone()
            .verify(&fixture.policy, &request, [0; 32])
            .unwrap();
        let (ready, outcome) = ledger
            .record
            .acknowledge(receipt.identity(), &fixture.policy, &fixture.signing_key)
            .unwrap();
        assert_eq!(outcome, CompilerExecutionIssuerAckV1::Advanced);
        ledger.commit(ready).unwrap();
        let (same, outcome) = ledger
            .record
            .acknowledge(receipt.identity(), &fixture.policy, &fixture.signing_key)
            .unwrap();
        assert_eq!(outcome, CompilerExecutionIssuerAckV1::AlreadyAcknowledged);
        assert_eq!(same.canonical, ledger.record.canonical);
    }

    #[test]
    fn singleton_lock_rejects_a_second_live_issuer() {
        let fixture = Fixture::new();
        let _first =
            IssuerLedgerV1::recover(fixture.root(), &fixture.policy, &fixture.signing_key).unwrap();
        assert!(matches!(
            IssuerLedgerV1::recover(fixture.root(), &fixture.policy, &fixture.signing_key),
            Err(ProtectedCompilerExecutionIssuerErrorV1::SingletonLock(_))
        ));
    }

    #[test]
    fn explicit_unlock_releases_a_transient_duplicate_description() {
        let fixture = Fixture::new();
        let first =
            IssuerLedgerV1::recover(fixture.root(), &fixture.policy, &fixture.signing_key).unwrap();
        let inherited = rustix::io::dup(&first._singleton_lock.descriptor).unwrap();
        drop(first);

        let second =
            IssuerLedgerV1::recover(fixture.root(), &fixture.policy, &fixture.signing_key).unwrap();
        drop(inherited);
        drop(second);
    }

    #[test]
    fn signed_non_successor_redo_fails_closed() {
        let fixture = Fixture::new();
        let mut ledger =
            IssuerLedgerV1::recover(fixture.root(), &fixture.policy, &fixture.signing_key).unwrap();
        let prepared = fixture.prepare(&ledger.record, [0x77; 32]).unwrap();
        ledger.commit(prepared.clone()).unwrap();
        ledger
            .store
            .stage_record_redo(
                REDO_RECORD,
                &prepared.canonical,
                COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V1,
                &mut NoRetainedDurableDirectoryHooksV1,
            )
            .unwrap();
        drop(ledger);

        assert!(matches!(
            IssuerLedgerV1::recover(fixture.root(), &fixture.policy, &fixture.signing_key),
            Err(ProtectedCompilerExecutionIssuerErrorV1::IllegalSuccessor)
        ));
    }

    #[test]
    fn substitutions_fail_before_a_receipt_exists() {
        let fixture = Fixture::new();
        let ready = IssuerRecordV1::genesis(&fixture.policy, &fixture.signing_key).unwrap();
        let prepared = fixture.prepare(&ready, [0x73; 32]).unwrap();
        let wrong_subject = subject(0x30);
        let wrong_occurrence =
            ProtectedCompilerExecutionOccurrenceV1::from_supervised_subject_for_test(
                wrong_subject.clone(),
                [0x92; 32],
            )
            .unwrap();
        let request = CompilerExecutionAttestationRequestV1::new(
            CompilerExecutionAttestationChallengeV1::new(
                &fixture.policy,
                &wrong_subject,
                [0x74; 32],
                1,
                [0; 32],
            )
            .unwrap(),
            wrong_subject,
        )
        .unwrap();
        let wrong_guard = wrong_occurrence.acquire_for_issuer().unwrap();
        assert!(matches!(
            prepared.issue(
                &wrong_guard,
                request.canonical_bytes(),
                &fixture.policy,
                &fixture.signing_key,
            ),
            Err(ProtectedCompilerExecutionIssuerErrorV1::OccurrenceMismatch)
                | Err(ProtectedCompilerExecutionIssuerErrorV1::RequestMismatch)
        ));
        assert!(prepared.receipt.is_none());
    }

    #[test]
    fn recovered_prepared_record_rejects_a_subject_equivalent_new_occurrence() {
        let fixture = Fixture::new();
        let ready = IssuerRecordV1::genesis(&fixture.policy, &fixture.signing_key).unwrap();
        let prepared = fixture.prepare(&ready, [0x75; 32]).unwrap();
        let recovered = IssuerRecordV1::decode(&prepared.canonical, &fixture.policy).unwrap();
        let request = CompilerExecutionAttestationRequestV1::new(
            recovered.challenge.clone().unwrap(),
            fixture.subject.clone(),
        )
        .unwrap();
        let replacement = fixture.occurrence_with_identity([0x92; SHA256_BYTES]);
        let replacement_guard = replacement.acquire_for_issuer().unwrap();

        assert!(matches!(
            recovered.issue(
                &replacement_guard,
                request.canonical_bytes(),
                &fixture.policy,
                &fixture.signing_key,
            ),
            Err(ProtectedCompilerExecutionIssuerErrorV1::OccurrenceMismatch)
        ));
        assert!(fixture.issue(&recovered, request.canonical_bytes()).is_ok());
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
        let byte_len = bytes.len() as u64;
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
        put(&mut bytes, &mut offset, &byte_len.to_le_bytes());
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
        let identity = record_digest(SUBJECT_IDENTITY_DOMAIN, &bytes[..offset]);
        put(&mut bytes, &mut offset, &identity);
        assert_eq!(offset, bytes.len());
        InertCompilerExecutionSubjectV1::decode(&bytes).unwrap()
    }
}
