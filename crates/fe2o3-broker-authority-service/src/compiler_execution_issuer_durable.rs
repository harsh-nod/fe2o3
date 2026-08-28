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
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1, CompilerExecutionAttestationChallengeV1,
    CompilerExecutionAttestationErrorV1, CompilerExecutionAttestationReceiptV1,
    CompilerExecutionAttestationRequestV1, CompilerExecutionCurrentRecordAttestationV3,
    CompilerExecutionCurrentRecordVerificationErrorV3,
    CompilerExecutionCurrentRecordVerificationV3, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionReceiptCarriageV1, CompilerExecutionReceiptPublicationAckV1,
    CompilerExecutionReceiptPublicationErrorV1, CompilerExecutionReceiptPublicationV1,
};
use rustix::fs::{FlockOperation, Mode, OFlags, flock};
use sha2::{Digest, Sha256};

use crate::compiler_execution_worker_ledger::{
    ProtectedCompilerExecutionWorkerLedgerErrorV1, ReacquiredWorkerReceiptRecordV2,
    WorkerExternalAnchorPublicationPlanV1, WorkerReceiptLedgerV1,
};
use crate::{
    ProtectedCompilerExecutionExternalAnchorErrorV1,
    ProtectedCompilerExecutionIssuerAdmissionErrorV1, ProtectedCompilerExecutionIssuerAdmissionV1,
    ProtectedCompilerExecutionOccurrenceErrorV1, ProtectedCompilerExecutionOccurrenceGuardV1,
    ProtectedCompilerExecutionOccurrenceV1,
};

const RECORD_MAGIC: [u8; 8] = *b"F2O3CEJ2";
const RECORD_VERSION: u16 = 2;
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
    + COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1
    + SHA256_BYTES
    + INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1
    + COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1
    + COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1
    + COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1;
const RECORD_IDENTITY_PREIMAGE_BYTES: usize = RECORD_SIGNED_PREFIX_BYTES + SIGNATURE_BYTES;
/// Exact byte length of one signed protected-issuer durable record.
pub const COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V2: usize =
    RECORD_IDENTITY_PREIMAGE_BYTES + SHA256_BYTES;

const RECORD_SIGNATURE_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-ISSUER-DURABLE-SIGNATURE/V2\0";
const RECORD_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-ISSUER-DURABLE-IDENTITY/V2\0";
const CANONICAL_RECORD: &str = "compiler-execution-issuer-v2.state";
const REDO_RECORD: &str = "compiler-execution-issuer-v2.redo";
const LEGACY_V1_CANONICAL_RECORD: &str = "compiler-execution-issuer-v1.state";
const LEGACY_V1_REDO_RECORD: &str = "compiler-execution-issuer-v1.redo";
const LEGACY_V1_RECORD_BYTES: usize = 2500;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum IssuerStageV2 {
    Ready = 1,
    Prepared = 2,
    Issued = 3,
}

impl IssuerStageV2 {
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
struct IssuerRecordV2 {
    stage: IssuerStageV2,
    sequence: u64,
    prior_anchor: [u8; SHA256_BYTES],
    last_receipt: [u8; SHA256_BYTES],
    last_ack: Option<CompilerExecutionReceiptPublicationAckV1>,
    occurrence_identity: [u8; SHA256_BYTES],
    subject: Option<InertCompilerExecutionSubjectV1>,
    challenge: Option<CompilerExecutionAttestationChallengeV1>,
    request: Option<CompilerExecutionAttestationRequestV1>,
    receipt: Option<CompilerExecutionAttestationReceiptV1>,
    identity: [u8; SHA256_BYTES],
    canonical: [u8; COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V2],
}

impl fmt::Debug for IssuerRecordV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IssuerRecordV2")
            .field("stage", &self.stage)
            .field("sequence", &self.sequence)
            .field("prior_anchor", &self.prior_anchor)
            .field("last_receipt", &self.last_receipt)
            .field(
                "last_ack_identity",
                &self.last_ack.as_ref().map(|ack| ack.identity()),
            )
            .field("occurrence_identity", &self.occurrence_identity)
            .field(
                "request_identity",
                &self.request.as_ref().map(|request| request.identity()),
            )
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl IssuerRecordV2 {
    fn genesis(
        policy: &CompilerExecutionIssuerPolicyV1,
        signing_key: &SigningKey,
    ) -> Result<Self, ProtectedCompilerExecutionIssuerErrorV1> {
        Self::encode(
            IssuerStageV2::Ready,
            1,
            [0; SHA256_BYTES],
            [0; SHA256_BYTES],
            None,
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
        if self.stage != IssuerStageV2::Ready {
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
            IssuerStageV2::Prepared,
            self.sequence,
            self.prior_anchor,
            self.last_receipt,
            self.last_ack.clone(),
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
        if self.stage != IssuerStageV2::Prepared {
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
            IssuerStageV2::Issued,
            self.sequence,
            self.prior_anchor,
            self.last_receipt,
            self.last_ack.clone(),
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
        ack: &CompilerExecutionReceiptPublicationAckV1,
        policy: &CompilerExecutionIssuerPolicyV1,
        signing_key: &SigningKey,
    ) -> Result<(Self, CompilerExecutionIssuerAckV1), ProtectedCompilerExecutionIssuerErrorV1> {
        match self.stage {
            IssuerStageV2::Issued => {
                let receipt = self.receipt.as_ref().ok_or(
                    ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
                        "issued state has no receipt",
                    ),
                )?;
                let publication = self.receipt_publication()?;
                ack.matches_publication(&publication)?;
                let next_sequence = self
                    .sequence
                    .checked_add(1)
                    .ok_or(ProtectedCompilerExecutionIssuerErrorV1::SequenceExhausted)?;
                let next = Self::encode(
                    IssuerStageV2::Ready,
                    next_sequence,
                    receipt.next_rollback_anchor(),
                    *receipt.identity().as_bytes(),
                    Some(ack.clone()),
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
            IssuerStageV2::Ready if self.sequence > 1 && self.last_ack.as_ref() == Some(ack) => {
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

    fn receipt_publication(
        &self,
    ) -> Result<CompilerExecutionReceiptPublicationV1, ProtectedCompilerExecutionIssuerErrorV1>
    {
        if self.stage != IssuerStageV2::Issued {
            return Err(ProtectedCompilerExecutionIssuerErrorV1::WrongStage {
                expected: "issued",
                actual: self.stage.name(),
            });
        }
        let receipt =
            self.receipt
                .as_ref()
                .ok_or(ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
                    "issued state has no receipt",
                ))?;
        CompilerExecutionReceiptPublicationV1::new(
            self.identity,
            self.occurrence_identity,
            receipt.clone(),
        )
        .map_err(Into::into)
    }

    #[allow(clippy::too_many_arguments)]
    fn encode(
        stage: IssuerStageV2,
        sequence: u64,
        prior_anchor: [u8; SHA256_BYTES],
        last_receipt: [u8; SHA256_BYTES],
        last_ack: Option<CompilerExecutionReceiptPublicationAckV1>,
        occurrence_identity: [u8; SHA256_BYTES],
        subject: Option<InertCompilerExecutionSubjectV1>,
        challenge: Option<CompilerExecutionAttestationChallengeV1>,
        request: Option<CompilerExecutionAttestationRequestV1>,
        receipt: Option<CompilerExecutionAttestationReceiptV1>,
        policy: &CompilerExecutionIssuerPolicyV1,
        signing_key: &SigningKey,
    ) -> Result<Self, ProtectedCompilerExecutionIssuerErrorV1> {
        validate_position(
            sequence,
            prior_anchor,
            last_receipt,
            last_ack.as_ref(),
            policy,
        )?;
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

        let mut canonical = [0_u8; COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V2];
        let mut offset = encode_header(&mut canonical);
        canonical[offset] = stage as u8;
        offset += STAGE_BYTES;
        put(&mut canonical, &mut offset, policy.identity().as_bytes());
        put(&mut canonical, &mut offset, &sequence.to_le_bytes());
        put(&mut canonical, &mut offset, &prior_anchor);
        put(&mut canonical, &mut offset, &last_receipt);
        put_optional(
            &mut canonical,
            &mut offset,
            last_ack
                .as_ref()
                .map(|value| value.canonical_bytes().as_slice()),
            COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1,
        );
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
            last_ack,
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
        if bytes.len() != COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V2 {
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
        let stage = IssuerStageV2::decode(reader.u8()?)?;
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
        let last_ack_bytes = reader.take(COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1)?;
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

        let last_ack = if last_ack_bytes.iter().all(|byte| *byte == 0) {
            None
        } else {
            Some(CompilerExecutionReceiptPublicationAckV1::decode(
                last_ack_bytes,
            )?)
        };

        let (subject, challenge, request, receipt) = match stage {
            IssuerStageV2::Ready => {
                require_zero(subject_bytes, "ready subject")?;
                require_zero(challenge_bytes, "ready challenge")?;
                require_zero(request_bytes, "ready request")?;
                require_zero(receipt_bytes, "ready receipt")?;
                (None, None, None, None)
            }
            IssuerStageV2::Prepared => {
                let subject =
                    InertCompilerExecutionSubjectV1::decode(subject_bytes).map_err(|error| {
                        ProtectedCompilerExecutionIssuerErrorV1::Subject(error.to_string())
                    })?;
                let challenge = CompilerExecutionAttestationChallengeV1::decode(challenge_bytes)?;
                require_zero(request_bytes, "prepared request")?;
                require_zero(receipt_bytes, "prepared receipt")?;
                (Some(subject), Some(challenge), None, None)
            }
            IssuerStageV2::Issued => {
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
        validate_position(
            sequence,
            prior_anchor,
            last_receipt,
            last_ack.as_ref(),
            policy,
        )?;
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
            last_ack,
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
            (IssuerStageV2::Ready, IssuerStageV2::Prepared) => {
                self.sequence == prior.sequence
                    && self.prior_anchor == prior.prior_anchor
                    && self.last_receipt == prior.last_receipt
                    && self.last_ack == prior.last_ack
            }
            (IssuerStageV2::Prepared, IssuerStageV2::Issued) => {
                self.sequence == prior.sequence
                    && self.prior_anchor == prior.prior_anchor
                    && self.last_receipt == prior.last_receipt
                    && self.last_ack == prior.last_ack
                    && self.subject == prior.subject
                    && self.occurrence_identity == prior.occurrence_identity
                    && self.challenge == prior.challenge
            }
            (IssuerStageV2::Issued, IssuerStageV2::Ready) => {
                prior.receipt.as_ref().is_some_and(|receipt| {
                    self.last_ack.as_ref().is_some_and(|ack| {
                        prior.receipt_publication().is_ok_and(|publication| {
                            self.sequence == prior.sequence.checked_add(1).unwrap_or(0)
                                && self.prior_anchor == receipt.next_rollback_anchor()
                                && self.last_receipt == *receipt.identity().as_bytes()
                                && ack.matches_publication(&publication).is_ok()
                        })
                    })
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
    last_ack: Option<&CompilerExecutionReceiptPublicationAckV1>,
    policy: &CompilerExecutionIssuerPolicyV1,
) -> Result<(), ProtectedCompilerExecutionIssuerErrorV1> {
    let valid = match sequence {
        0 => false,
        1 => prior_anchor == [0; 32] && last_receipt == [0; 32] && last_ack.is_none(),
        _ => last_ack.is_some_and(|ack| {
            prior_anchor != [0; 32]
                && last_receipt != [0; 32]
                && ack.policy_identity() == policy.identity()
                && ack.sequence().checked_add(1) == Some(sequence)
                && ack.current_rollback_anchor() == prior_anchor
                && *ack.receipt_identity().as_bytes() == last_receipt
        }),
    };
    if !valid {
        return Err(ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
            "issuer journal has a noncanonical rollback position",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_stage_payload(
    stage: IssuerStageV2,
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
        IssuerStageV2::Ready
            if occurrence_identity == [0; SHA256_BYTES]
                && subject.is_none()
                && challenge.is_none()
                && request.is_none()
                && receipt.is_none() =>
        {
            Ok(())
        }
        IssuerStageV2::Prepared | IssuerStageV2::Issued => {
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
            if stage == IssuerStageV2::Prepared && request.is_none() && receipt.is_none() {
                return Ok(());
            }
            if stage != IssuerStageV2::Issued {
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

struct IssuerLedgerV2 {
    store: RetainedDurableDirectoryV1,
    _singleton_lock: SingletonLockV1,
    record: IssuerRecordV2,
    poisoned: bool,
}

impl IssuerLedgerV2 {
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
        if store
            .read_private(LEGACY_V1_CANONICAL_RECORD, LEGACY_V1_RECORD_BYTES)?
            .is_some()
            || store
                .read_private(LEGACY_V1_REDO_RECORD, LEGACY_V1_RECORD_BYTES)?
                .is_some()
        {
            return Err(ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
                "legacy V1 journal requires explicit fail-closed migration",
            ));
        }
        let canonical_bytes = store.read_private(
            CANONICAL_RECORD,
            COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V2,
        )?;
        let redo_bytes = store.read_private(
            REDO_RECORD,
            COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V2,
        )?;

        let record = match (canonical_bytes, redo_bytes) {
            (None, None) => {
                let genesis = IssuerRecordV2::genesis(policy, signing_key)?;
                store.commit_record(
                    CANONICAL_RECORD,
                    REDO_RECORD,
                    &genesis.canonical,
                    COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V2,
                    hooks,
                )?;
                genesis
            }
            (canonical, Some(redo_bytes)) => {
                let redo = IssuerRecordV2::decode(&redo_bytes, policy)?;
                let canonical_record = canonical
                    .as_deref()
                    .map(|bytes| IssuerRecordV2::decode(bytes, policy))
                    .transpose()?;
                let legal = canonical_record.as_ref().map_or_else(
                    || {
                        redo.stage == IssuerStageV2::Ready
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
                    COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V2,
                    hooks,
                )?;
                redo
            }
            (Some(canonical_bytes), None) => {
                let record = IssuerRecordV2::decode(&canonical_bytes, policy)?;
                store.establish_recovered_record_durability(
                    CANONICAL_RECORD,
                    REDO_RECORD,
                    &canonical_bytes,
                    COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V2,
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
        next: IssuerRecordV2,
    ) -> Result<(), ProtectedCompilerExecutionIssuerErrorV1> {
        let mut hooks = NoRetainedDurableDirectoryHooksV1;
        self.commit_with_hooks(next, &mut hooks)
    }

    fn commit_with_hooks(
        &mut self,
        next: IssuerRecordV2,
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
            COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V2,
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
            IssuerStageV2::Ready => CompilerExecutionIssuerRecoveryV1::Ready {
                next_sequence: self.record.sequence,
                current_rollback_anchor: self.record.prior_anchor,
            },
            IssuerStageV2::Prepared => CompilerExecutionIssuerRecoveryV1::Prepared {
                challenge: self
                    .record
                    .challenge
                    .clone()
                    .expect("validated prepared record has a challenge"),
            },
            IssuerStageV2::Issued => CompilerExecutionIssuerRecoveryV1::Issued {
                publication: self
                    .record
                    .receipt_publication()
                    .expect("validated issued record forms its exact receipt sidecar"),
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
    publication: CompilerExecutionReceiptPublicationV1,
}

impl ProtectedCompilerExecutionReceiptV1 {
    /// Returns the exact canonical signed receipt.
    pub const fn receipt(&self) -> &CompilerExecutionAttestationReceiptV1 {
        self.publication.receipt()
    }

    /// Returns the exact authority-free sidecar bound to the durable issued journal record.
    pub const fn publication(&self) -> &CompilerExecutionReceiptPublicationV1 {
        &self.publication
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
        publication: CompilerExecutionReceiptPublicationV1,
    },
}

/// Move-only proof that the protected Worker ledger durably committed one exact receipt sidecar.
///
/// No public constructor exists. The bounded service may form this value only after independent
/// descriptor-relative Worker-ledger recovery and exact ACK matching.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::CommittedCompilerExecutionReceiptPublicationV1;
/// fn duplicate(value: CommittedCompilerExecutionReceiptPublicationV1) { let _ = value.clone(); }
/// ```
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::CommittedCompilerExecutionReceiptPublicationV1;
/// use fe2o3_runtime_protocol::CompilerExecutionReceiptPublicationAckV1;
/// fn forge(ack: CompilerExecutionReceiptPublicationAckV1) {
///     let _ = CommittedCompilerExecutionReceiptPublicationV1 { ack };
/// }
/// ```
pub struct CommittedCompilerExecutionReceiptPublicationV1 {
    ack: CompilerExecutionReceiptPublicationAckV1,
}

impl CommittedCompilerExecutionReceiptPublicationV1 {
    pub(super) fn from_reacquired_worker_record(record: ReacquiredWorkerReceiptRecordV2) -> Self {
        Self {
            ack: record.into_acknowledgment(),
        }
    }

    /// Returns the exact canonical ACK bound to the reacquired Worker ledger record.
    pub const fn acknowledgment(&self) -> &CompilerExecutionReceiptPublicationAckV1 {
        &self.ack
    }

    /// Reports the only fact conferred by this service-owned wrapper.
    pub const fn proves_durable_receipt_publication(&self) -> bool {
        true
    }

    /// Durable receipt publication alone does not close compiler authority.
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
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
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::{
///     CommittedCompilerExecutionReceiptPublicationV1, ProtectedCompilerExecutionIssuerV1,
/// };
/// fn bypass(
///     issuer: &mut ProtectedCompilerExecutionIssuerV1,
///     committed: CommittedCompilerExecutionReceiptPublicationV1,
/// ) {
///     let _ = issuer.acknowledge_published_receipt(committed);
/// }
/// ```
///
/// The durable transition methods are service-private; an external caller cannot prepare, issue,
/// publish, or inspect recovery state without using the canonical packet service:
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedCompilerExecutionIssuerV1;
/// fn bypass(issuer: &mut ProtectedCompilerExecutionIssuerV1) {
///     let _ = issuer.prepare_challenge();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedCompilerExecutionIssuerV1;
/// fn bypass(issuer: &mut ProtectedCompilerExecutionIssuerV1, request: &[u8]) {
///     let _ = issuer.issue_receipt(request);
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedCompilerExecutionIssuerV1;
/// fn bypass(
///     issuer: &mut ProtectedCompilerExecutionIssuerV1,
///     request: &[u8],
///     publication: &[u8],
/// ) {
///     let _ = issuer.publish_receipt_to_worker(request, publication);
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedCompilerExecutionIssuerV1;
/// fn bypass(issuer: &ProtectedCompilerExecutionIssuerV1) {
///     let _ = issuer.recovery();
/// }
/// ```
pub struct ProtectedCompilerExecutionIssuerV1 {
    admission: ProtectedCompilerExecutionIssuerAdmissionV1,
    ledger: IssuerLedgerV2,
    worker_ledger: WorkerReceiptLedgerV1,
}

impl ProtectedCompilerExecutionIssuerV1 {
    /// Acquires the singleton ledger and recovers one exact durable state.
    pub fn admit(
        admission: ProtectedCompilerExecutionIssuerAdmissionV1,
    ) -> Result<(Self, CompilerExecutionIssuerRecoveryV1), ProtectedCompilerExecutionIssuerErrorV1>
    {
        admission.validate_continuity()?;
        let issuer_root = admission.try_clone_service_root()?;
        let worker_root = admission.try_clone_service_root()?;
        let ledger =
            IssuerLedgerV2::recover(issuer_root, admission.policy(), admission.signing_key())?;
        let worker_ledger = WorkerReceiptLedgerV1::recover(worker_root, admission.policy())?;
        validate_worker_ledger_join(&ledger.record, &worker_ledger)?;
        admission.validate_continuity()?;
        let recovery = ledger.recovery();
        Ok((
            Self {
                admission,
                ledger,
                worker_ledger,
            },
            recovery,
        ))
    }

    /// Generates and durably commits a fresh subject-bound challenge before returning it.
    pub(super) fn prepare_challenge(
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
    pub(super) fn issue_receipt(
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
        let publication = next.receipt_publication()?;
        self.admission.validate_continuity()?;
        self.ledger.commit(next)?;
        self.admission.validate_continuity()?;
        drop(occurrence_guard);
        Ok(ProtectedCompilerExecutionReceiptV1 { publication })
    }

    fn acknowledge_published_receipt(
        &mut self,
        committed: CommittedCompilerExecutionReceiptPublicationV1,
    ) -> Result<CompilerExecutionIssuerAckV1, ProtectedCompilerExecutionIssuerErrorV1> {
        self.admission.validate_continuity()?;
        let (next, outcome) = self.ledger.record.acknowledge(
            &committed.ack,
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

    /// Verifies and externally anchors one exact issued receipt, durably publishes it in the
    /// protected Worker ledger, reacquires the canonical Worker record, and only then advances the
    /// issuer journal. Repeating the same request and sidecar after a lost response recovers the
    /// persisted challenge or finishes an already anchor-committed local publication.
    pub(super) fn publish_receipt_to_worker(
        &mut self,
        request_bytes: &[u8],
        publication_bytes: &[u8],
    ) -> Result<CompilerExecutionIssuerAckV1, ProtectedCompilerExecutionIssuerErrorV1> {
        self.admission.validate_continuity()?;
        validate_worker_ledger_join(&self.ledger.record, &self.worker_ledger)?;
        let request = CompilerExecutionAttestationRequestV1::decode(request_bytes)?;
        let publication = CompilerExecutionReceiptPublicationV1::decode(publication_bytes)?;
        validate_publication_input(
            &self.ledger.record,
            &self.worker_ledger,
            &request,
            &publication,
        )?;
        let worker_ledger = &mut self.worker_ledger;
        let external_anchor = self.admission.external_anchor_mut();
        let reacquired = commit_externally_anchored_worker_publication(
            worker_ledger,
            external_anchor,
            request,
            publication,
        )?;
        self.admission.validate_continuity()?;
        validate_worker_ledger_join(&self.ledger.record, &self.worker_ledger)?;
        let committed =
            CommittedCompilerExecutionReceiptPublicationV1::from_reacquired_worker_record(
                reacquired,
            );
        let outcome = self.acknowledge_published_receipt(committed)?;
        validate_worker_ledger_join(&self.ledger.record, &self.worker_ledger)?;
        self.admission.validate_continuity()?;
        Ok(outcome)
    }

    pub(super) fn prepare_challenge_for_service(
        &mut self,
        expected_sequence: u64,
        expected_rollback_anchor: [u8; SHA256_BYTES],
    ) -> Result<ProtectedCompilerExecutionChallengeV1, ProtectedCompilerExecutionIssuerErrorV1>
    {
        self.admission.validate_continuity()?;
        if self.ledger.record.sequence != expected_sequence
            || self.ledger.record.prior_anchor != expected_rollback_anchor
        {
            return Err(ProtectedCompilerExecutionIssuerErrorV1::ServicePositionMismatch);
        }
        match self.ledger.record.stage {
            IssuerStageV2::Ready => self.prepare_challenge(),
            IssuerStageV2::Prepared => {
                let challenge = self.ledger.record.challenge.clone().ok_or(
                    ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
                        "prepared issuer state has no challenge",
                    ),
                )?;
                self.admission.validate_continuity()?;
                Ok(ProtectedCompilerExecutionChallengeV1 { challenge })
            }
            stage => Err(ProtectedCompilerExecutionIssuerErrorV1::WrongStage {
                expected: "ready or matching prepared challenge",
                actual: stage.name(),
            }),
        }
    }

    pub(super) fn issue_receipt_for_service(
        &mut self,
        request_bytes: &[u8],
    ) -> Result<ProtectedCompilerExecutionReceiptV1, ProtectedCompilerExecutionIssuerErrorV1> {
        self.admission.validate_continuity()?;
        match self.ledger.record.stage {
            IssuerStageV2::Prepared => self.issue_receipt(request_bytes),
            IssuerStageV2::Issued => {
                let request = CompilerExecutionAttestationRequestV1::decode(request_bytes)?;
                if self.ledger.record.request.as_ref() != Some(&request) {
                    return Err(ProtectedCompilerExecutionIssuerErrorV1::RequestMismatch);
                }
                let publication = self.ledger.record.receipt_publication()?;
                self.admission.validate_continuity()?;
                Ok(ProtectedCompilerExecutionReceiptV1 { publication })
            }
            stage => Err(ProtectedCompilerExecutionIssuerErrorV1::WrongStage {
                expected: "prepared or matching issued request",
                actual: stage.name(),
            }),
        }
    }

    pub(super) fn publish_receipt_for_service(
        &mut self,
        request_bytes: &[u8],
        publication_bytes: &[u8],
    ) -> Result<
        (
            CompilerExecutionIssuerAckV1,
            CompilerExecutionReceiptPublicationAckV1,
        ),
        ProtectedCompilerExecutionIssuerErrorV1,
    > {
        let outcome = self.publish_receipt_to_worker(request_bytes, publication_bytes)?;
        let acknowledgment = self
            .worker_ledger
            .last_record()
            .ok_or(ProtectedCompilerExecutionIssuerErrorV1::WorkerLedgerJoin(
                "published issuer state has no Worker record",
            ))?
            .acknowledgment()?;
        self.admission.validate_continuity()?;
        Ok((outcome, acknowledgment))
    }

    /// Reacquires the current protected Worker record and returns its exact complete carriage.
    pub(super) fn recover_current_carriage_for_service(
        &self,
        expected_subject: &InertCompilerExecutionSubjectV1,
    ) -> Result<Option<CompilerExecutionReceiptCarriageV1>, ProtectedCompilerExecutionIssuerErrorV1>
    {
        self.admission.validate_continuity()?;
        validate_worker_ledger_join(&self.ledger.record, &self.worker_ledger)?;
        let carriage = match self
            .worker_ledger
            .recover_current_carriage(expected_subject)
        {
            Ok(carriage) => carriage,
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::MissingCanonicalRecord) => {
                self.admission.validate_continuity()?;
                validate_worker_ledger_join(&self.ledger.record, &self.worker_ledger)?;
                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        };
        if carriage.policy() != self.admission.policy()
            || carriage.request().subject() != expected_subject
        {
            return Err(ProtectedCompilerExecutionIssuerErrorV1::WorkerLedgerJoin(
                "recovered carriage differs from the current policy or requested subject",
            ));
        }
        self.admission.validate_continuity()?;
        validate_worker_ledger_join(&self.ledger.record, &self.worker_ledger)?;
        Ok(Some(carriage))
    }

    /// Reacquires one exact commit, obtains a fresh external observation, and reacquires it again.
    pub(super) fn verify_current_carriage_for_service(
        &mut self,
        expected_carriage: &CompilerExecutionReceiptCarriageV1,
        verification_challenge: [u8; 32],
    ) -> Result<CompilerExecutionCurrentRecordVerificationV3, ProtectedCompilerExecutionIssuerErrorV1>
    {
        self.admission.validate_continuity()?;
        validate_worker_ledger_join(&self.ledger.record, &self.worker_ledger)?;
        let external_challenge = self
            .worker_ledger
            .external_anchor_currentness_challenge(expected_carriage, verification_challenge)?;
        let external_currentness_receipt = self
            .admission
            .external_anchor_mut()
            .exchange(&external_challenge)?;
        self.admission.validate_continuity()?;
        validate_worker_ledger_join(&self.ledger.record, &self.worker_ledger)?;
        let verification = self.worker_ledger.verify_current_carriage(
            expected_carriage,
            external_currentness_receipt,
            verification_challenge,
        )?;
        if verification.policy_identity() != *self.admission.policy().identity().as_bytes()
            || verification.subject_identity()
                != *expected_carriage.request().subject().identity().sha256()
            || verification.carriage_identity() != *expected_carriage.identity().as_bytes()
        {
            return Err(ProtectedCompilerExecutionIssuerErrorV1::WorkerLedgerJoin(
                "current-record verification differs from protected policy or expected carriage",
            ));
        }
        self.admission.validate_continuity()?;
        validate_worker_ledger_join(&self.ledger.record, &self.worker_ledger)?;
        Ok(verification)
    }

    /// Reacquires one exact current carriage and signs the caller's fresh endpoint challenge.
    pub(super) fn attest_current_carriage_for_service(
        &mut self,
        expected_carriage: &CompilerExecutionReceiptCarriageV1,
        verification_challenge: [u8; 32],
    ) -> Result<CompilerExecutionCurrentRecordAttestationV3, ProtectedCompilerExecutionIssuerErrorV1>
    {
        let verification =
            self.verify_current_carriage_for_service(expected_carriage, verification_challenge)?;
        let attestation = CompilerExecutionCurrentRecordAttestationV3::issue(
            self.admission.policy(),
            expected_carriage,
            verification,
            verification_challenge,
            self.admission.signing_key(),
        )?;
        self.admission.validate_continuity()?;
        validate_worker_ledger_join(&self.ledger.record, &self.worker_ledger)?;
        Ok(attestation)
    }

    pub(super) fn validate_service_continuity(
        &self,
    ) -> Result<(), ProtectedCompilerExecutionIssuerErrorV1> {
        self.admission.validate_continuity()?;
        validate_worker_ledger_join(&self.ledger.record, &self.worker_ledger)
    }

    pub(super) const fn service_policy(&self) -> &CompilerExecutionIssuerPolicyV1 {
        self.admission.policy()
    }

    pub(super) fn service_peer(&self) -> std::os::fd::BorrowedFd<'_> {
        self.admission.service_peer()
    }

    pub(super) fn client_pidfd(&self) -> std::os::fd::BorrowedFd<'_> {
        self.admission.client_pidfd()
    }

    /// Returns the current inert restart output without changing durable state.
    pub(super) fn recovery(&self) -> CompilerExecutionIssuerRecoveryV1 {
        self.ledger.recovery()
    }
}

fn commit_externally_anchored_worker_publication(
    worker_ledger: &mut WorkerReceiptLedgerV1,
    external_anchor: &mut crate::ProtectedCompilerExecutionExternalAnchorV1,
    request: CompilerExecutionAttestationRequestV1,
    publication: CompilerExecutionReceiptPublicationV1,
) -> Result<ReacquiredWorkerReceiptRecordV2, ProtectedCompilerExecutionIssuerErrorV1> {
    let plan = worker_ledger.prepare_external_anchor_publication(request, publication)?;
    if let WorkerExternalAnchorPublicationPlanV1::Exchange(challenge) = plan {
        let receipt = external_anchor.exchange(&challenge)?;
        worker_ledger.record_external_anchor_observation(receipt.observation_bytes())?;
    }
    worker_ledger
        .commit_anchored_publication()
        .map_err(Into::into)
}

fn validate_worker_ledger_join(
    issuer: &IssuerRecordV2,
    worker: &WorkerReceiptLedgerV1,
) -> Result<(), ProtectedCompilerExecutionIssuerErrorV1> {
    let Some(worker_record) = worker.last_record() else {
        if issuer.sequence == 1
            && issuer.prior_anchor == [0; SHA256_BYTES]
            && issuer.last_ack.is_none()
        {
            return Ok(());
        }
        return Err(ProtectedCompilerExecutionIssuerErrorV1::WorkerLedgerJoin(
            "non-genesis issuer state has no Worker record",
        ));
    };

    let worker_ack = worker_record.acknowledgment()?;
    let worker_is_prior = worker_record.sequence().checked_add(1) == Some(issuer.sequence)
        && worker_record.current_rollback_anchor() == issuer.prior_anchor
        && issuer.last_ack.as_ref() == Some(&worker_ack);
    if worker_is_prior {
        return Ok(());
    }

    if issuer.stage == IssuerStageV2::Issued
        && worker_record.sequence() == issuer.sequence
        && worker_record.prior_rollback_anchor() == issuer.prior_anchor
        && issuer.request.as_ref() == Some(worker_record.request())
        && issuer
            .receipt_publication()
            .is_ok_and(|publication| &publication == worker_record.publication())
    {
        return Ok(());
    }

    Err(ProtectedCompilerExecutionIssuerErrorV1::WorkerLedgerJoin(
        "durable sequences, anchors, ACK, request, or publication do not form one crash position",
    ))
}

fn validate_publication_input(
    issuer: &IssuerRecordV2,
    worker: &WorkerReceiptLedgerV1,
    request: &CompilerExecutionAttestationRequestV1,
    publication: &CompilerExecutionReceiptPublicationV1,
) -> Result<(), ProtectedCompilerExecutionIssuerErrorV1> {
    match issuer.stage {
        IssuerStageV2::Issued => {
            let expected_request = issuer.request.as_ref().ok_or(
                ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
                    "issued state has no request",
                ),
            )?;
            let expected_publication = issuer.receipt_publication()?;
            if request != expected_request || publication != &expected_publication {
                return Err(ProtectedCompilerExecutionIssuerErrorV1::WorkerLedgerJoin(
                    "publication input does not equal the current issued record",
                ));
            }
            Ok(())
        }
        IssuerStageV2::Ready if issuer.sequence > 1 => {
            let worker_record = worker.last_record().ok_or(
                ProtectedCompilerExecutionIssuerErrorV1::WorkerLedgerJoin(
                    "acknowledged issuer state has no Worker record",
                ),
            )?;
            if request != worker_record.request()
                || publication != worker_record.publication()
                || issuer.last_ack.as_ref() != Some(&worker_record.acknowledgment()?)
            {
                return Err(ProtectedCompilerExecutionIssuerErrorV1::WorkerLedgerJoin(
                    "replayed publication does not equal both durable journals",
                ));
            }
            Ok(())
        }
        _ => Err(ProtectedCompilerExecutionIssuerErrorV1::WrongStage {
            expected: "issued or matching acknowledged receipt",
            actual: issuer.stage.name(),
        }),
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
        &(COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V2 as u64).to_le_bytes(),
    );
    put(output, &mut offset, &0_u32.to_le_bytes());
    offset
}

fn decode_header(reader: &mut Reader<'_>) -> Result<(), ProtectedCompilerExecutionIssuerErrorV1> {
    if reader.fixed::<8>()? != RECORD_MAGIC
        || reader.u16()? != RECORD_VERSION
        || reader.u16()? != 0
        || reader.u64()? != COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V2 as u64
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
    ReceiptPublication(CompilerExecutionReceiptPublicationErrorV1),
    CurrentRecord(CompilerExecutionCurrentRecordVerificationErrorV3),
    WorkerLedger(ProtectedCompilerExecutionWorkerLedgerErrorV1),
    ExternalAnchor(ProtectedCompilerExecutionExternalAnchorErrorV1),
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
    OccurrenceMismatch,
    ServicePositionMismatch,
    WorkerLedgerJoin(&'static str),
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
            Self::ReceiptPublication(error) => {
                write!(formatter, "issuer receipt publication failed: {error}")
            }
            Self::CurrentRecord(error) => {
                write!(
                    formatter,
                    "issuer current-record attestation failed: {error}"
                )
            }
            Self::WorkerLedger(error) => write!(formatter, "issuer Worker ledger failed: {error}"),
            Self::ExternalAnchor(error) => {
                write!(formatter, "issuer external-anchor exchange failed: {error}")
            }
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
            Self::OccurrenceMismatch => {
                formatter.write_str("issuer occurrence identity or subject mismatch")
            }
            Self::ServicePositionMismatch => {
                formatter.write_str("issuer service request names the wrong durable position")
            }
            Self::WorkerLedgerJoin(reason) => {
                write!(formatter, "issuer and Worker ledger disagree: {reason}")
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
            Self::ReceiptPublication(error) => Some(error),
            Self::CurrentRecord(error) => Some(error),
            Self::WorkerLedger(error) => Some(error),
            Self::ExternalAnchor(error) => Some(error),
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

impl From<CompilerExecutionReceiptPublicationErrorV1> for ProtectedCompilerExecutionIssuerErrorV1 {
    fn from(error: CompilerExecutionReceiptPublicationErrorV1) -> Self {
        Self::ReceiptPublication(error)
    }
}

impl From<CompilerExecutionCurrentRecordVerificationErrorV3>
    for ProtectedCompilerExecutionIssuerErrorV1
{
    fn from(error: CompilerExecutionCurrentRecordVerificationErrorV3) -> Self {
        Self::CurrentRecord(error)
    }
}

impl From<ProtectedCompilerExecutionWorkerLedgerErrorV1>
    for ProtectedCompilerExecutionIssuerErrorV1
{
    fn from(error: ProtectedCompilerExecutionWorkerLedgerErrorV1) -> Self {
        Self::WorkerLedger(error)
    }
}

impl From<ProtectedCompilerExecutionExternalAnchorErrorV1>
    for ProtectedCompilerExecutionIssuerErrorV1
{
    fn from(error: ProtectedCompilerExecutionExternalAnchorErrorV1) -> Self {
        Self::ExternalAnchor(error)
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::os::fd::{AsRawFd, FromRawFd, RawFd};
    use std::os::unix::fs::PermissionsExt;
    use std::thread;
    use std::time::{Duration, Instant};

    use fe2o3_artifact_transaction::{
        INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1, INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1,
        RetainedDurableFaultTimingV1, RetainedDurableRecordBoundaryV1,
    };
    use fe2o3_external_anchor_protocol::{
        ANCHOR_CHALLENGE_WIRE_LEN_V1, ANCHOR_OBSERVATION_WIRE_LEN_V1, AnchorChallengeV1,
        AnchorPositionV1, ChallengeKindV1, UnsignedAnchorObservationV1,
    };
    use fe2o3_runtime_protocol::{
        CompilerExecutionExternalAnchorServiceIdentityV1,
        CompilerExecutionWorkerAnchorJournalStageV1,
    };
    use rustix::net::{AddressFamily, SocketFlags, SocketType, socketpair};
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
                SigningKey::from_bytes(&[0x52; 32])
                    .verifying_key()
                    .to_bytes(),
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
            record: &IssuerRecordV2,
            nonce: [u8; SHA256_BYTES],
        ) -> Result<IssuerRecordV2, ProtectedCompilerExecutionIssuerErrorV1> {
            let occurrence = self.occurrence();
            let guard = occurrence.acquire_for_issuer()?;
            record.prepare(&guard, nonce, &self.policy, &self.signing_key)
        }

        fn issue(
            &self,
            record: &IssuerRecordV2,
            request_bytes: &[u8],
        ) -> Result<IssuerRecordV2, ProtectedCompilerExecutionIssuerErrorV1> {
            let occurrence = self.occurrence();
            let guard = occurrence.acquire_for_issuer()?;
            record.issue(&guard, request_bytes, &self.policy, &self.signing_key)
        }

        fn ack(&self, record: &IssuerRecordV2) -> CompilerExecutionReceiptPublicationAckV1 {
            CompilerExecutionReceiptPublicationAckV1::new(
                &record.receipt_publication().unwrap(),
                [0x93; SHA256_BYTES],
            )
            .unwrap()
        }
    }

    fn pidfd_for_current_process() -> OwnedFd {
        // SAFETY: pidfd_open receives the current positive PID and zero flags. Success returns one
        // fresh owned close-on-exec descriptor.
        let descriptor = unsafe {
            libc::syscall(
                libc::SYS_pidfd_open,
                libc::pid_t::try_from(std::process::id()).unwrap(),
                0,
            )
        };
        assert!(
            descriptor >= 0,
            "pidfd_open failed: {}",
            io::Error::last_os_error()
        );
        // SAFETY: successful pidfd_open returned a fresh owned descriptor.
        unsafe { OwnedFd::from_raw_fd(descriptor as RawFd) }
    }

    fn external_anchor_transport(
        fixture: &Fixture,
    ) -> (crate::ProtectedCompilerExecutionExternalAnchorV1, OwnedFd) {
        let (peer, service) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .unwrap();
        let identity = CompilerExecutionExternalAnchorServiceIdentityV1::new(
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
        .unwrap();
        let admission =
            crate::ProtectedExternalAnchorServiceAdmissionV1::admit_non_authoritative_same_uid_test(
                peer,
                pidfd_for_current_process(),
                identity,
            )
            .unwrap();
        let transport = crate::ProtectedCompilerExecutionExternalAnchorV1::from_issuer_policy(
            admission,
            &fixture.policy,
        )
        .unwrap();
        (transport, service)
    }

    fn issued_publication(
        fixture: &Fixture,
        nonce: [u8; SHA256_BYTES],
    ) -> (
        CompilerExecutionAttestationRequestV1,
        CompilerExecutionReceiptPublicationV1,
    ) {
        let ready = IssuerRecordV2::genesis(&fixture.policy, &fixture.signing_key).unwrap();
        let prepared = fixture.prepare(&ready, nonce).unwrap();
        let request = CompilerExecutionAttestationRequestV1::new(
            prepared.challenge.clone().unwrap(),
            fixture.subject.clone(),
        )
        .unwrap();
        let issued = fixture.issue(&prepared, request.canonical_bytes()).unwrap();
        let publication = issued.receipt_publication().unwrap();
        (request, publication)
    }

    fn receive_anchor_challenge(service: &OwnedFd) -> AnchorChallengeV1 {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut bytes = [0_u8; ANCHOR_CHALLENGE_WIRE_LEN_V1];
        loop {
            // SAFETY: the fixed byte array is writable and `service` retains the descriptor.
            let received = unsafe {
                libc::recv(
                    service.as_raw_fd(),
                    bytes.as_mut_ptr().cast(),
                    bytes.len(),
                    libc::MSG_DONTWAIT,
                )
            };
            if received == bytes.len() as isize {
                return AnchorChallengeV1::decode(&bytes).unwrap();
            }
            if received < 0 && io::Error::last_os_error().kind() == io::ErrorKind::WouldBlock {
                assert!(Instant::now() < deadline, "challenge receive timed out");
                thread::yield_now();
                continue;
            }
            panic!("unexpected challenge receive result: {received}");
        }
    }

    fn signed_anchor_observation(
        challenge: &AnchorChallengeV1,
        position: AnchorPositionV1,
        signing_key: &SigningKey,
    ) -> [u8; ANCHOR_OBSERVATION_WIRE_LEN_V1] {
        let unsigned = UnsignedAnchorObservationV1::from_challenge(challenge, position);
        let signature = signing_key.sign(&unsigned.signing_bytes()).to_bytes();
        unsigned.attach_signature(signature)
    }

    fn commit_worker_publication(
        worker: &mut WorkerReceiptLedgerV1,
        request: CompilerExecutionAttestationRequestV1,
        publication: CompilerExecutionReceiptPublicationV1,
    ) -> Result<ReacquiredWorkerReceiptRecordV2, ProtectedCompilerExecutionWorkerLedgerErrorV1>
    {
        match worker.prepare_external_anchor_publication(request, publication)? {
            WorkerExternalAnchorPublicationPlanV1::Exchange(challenge) => {
                let observation = signed_anchor_observation(
                    &challenge,
                    AnchorPositionV1::Proposed,
                    &SigningKey::from_bytes(&[0x52; 32]),
                );
                worker.record_external_anchor_observation(&observation)?;
            }
            WorkerExternalAnchorPublicationPlanV1::CommitLocally => {}
        }
        worker.commit_anchored_publication()
    }

    fn spawn_anchor_response(
        service: OwnedFd,
        expected: Option<AnchorChallengeV1>,
        position: AnchorPositionV1,
        signing_key: SigningKey,
    ) -> thread::JoinHandle<OwnedFd> {
        thread::spawn(move || {
            let challenge = receive_anchor_challenge(&service);
            if let Some(expected) = expected {
                assert_eq!(challenge, expected);
            }
            let response = signed_anchor_observation(&challenge, position, &signing_key);
            // SAFETY: `response` is readable for its fixed length and `service` remains owned.
            let sent = unsafe {
                libc::send(
                    service.as_raw_fd(),
                    response.as_ptr().cast(),
                    response.len(),
                    libc::MSG_NOSIGNAL,
                )
            };
            assert_eq!(sent, response.len() as isize);
            service
        })
    }

    fn assert_no_anchor_challenge(service: &OwnedFd) {
        let mut bytes = [0_u8; ANCHOR_CHALLENGE_WIRE_LEN_V1];
        // SAFETY: the fixed byte array is writable and `service` retains the descriptor.
        let received = unsafe {
            libc::recv(
                service.as_raw_fd(),
                bytes.as_mut_ptr().cast(),
                bytes.len(),
                libc::MSG_DONTWAIT,
            )
        };
        assert_eq!(received, -1);
        assert_eq!(io::Error::last_os_error().kind(), io::ErrorKind::WouldBlock);
    }

    #[test]
    fn client_bound_currentness_recovery_crosses_retained_anchor_transport() {
        let fixture = Fixture::new();
        let (request, publication) = issued_publication(&fixture, [0x9a; SHA256_BYTES]);
        let mut worker = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        commit_worker_publication(&mut worker, request, publication).unwrap();
        let carriage = worker.recover_current_carriage(&fixture.subject).unwrap();
        let verification_challenge = [0x9b; SHA256_BYTES];
        let expected = worker
            .external_anchor_currentness_challenge(&carriage, verification_challenge)
            .unwrap();
        assert_eq!(expected.kind(), ChallengeKindV1::Recover);

        let (mut external_anchor, service) = external_anchor_transport(&fixture);
        let responder = spawn_anchor_response(
            service,
            Some(expected.clone()),
            AnchorPositionV1::Proposed,
            SigningKey::from_bytes(&[0x52; 32]),
        );
        let currentness_receipt = external_anchor.exchange(&expected).unwrap();
        let _service = responder.join().unwrap();
        assert_eq!(currentness_receipt.challenge(), &expected);
        assert_eq!(currentness_receipt.position(), AnchorPositionV1::Proposed);

        let verification = worker
            .verify_current_carriage(&carriage, currentness_receipt, verification_challenge)
            .unwrap();
        let attestation = CompilerExecutionCurrentRecordAttestationV3::issue(
            &fixture.policy,
            &carriage,
            verification,
            verification_challenge,
            &fixture.signing_key,
        )
        .unwrap();
        let verified = attestation
            .verify(&fixture.policy, &carriage, verification_challenge)
            .unwrap();
        assert!(verified.authenticates_external_anchor_commit());
        assert!(verified.authenticates_external_rollback_currentness());
        assert!(!verified.grants_authority());
    }

    #[test]
    fn externally_anchored_worker_publication_commits_before_replay_ack() {
        let fixture = Fixture::new();
        let anchor_signing_key = SigningKey::from_bytes(&[0x52; 32]);
        let (request, publication) = issued_publication(&fixture, [0xa1; SHA256_BYTES]);
        let mut worker = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        let (mut anchor, service) = external_anchor_transport(&fixture);
        let response = spawn_anchor_response(
            service,
            None,
            AnchorPositionV1::Proposed,
            anchor_signing_key,
        );

        let first = commit_externally_anchored_worker_publication(
            &mut worker,
            &mut anchor,
            request.clone(),
            publication.clone(),
        )
        .unwrap();
        let service = response.join().unwrap();
        let first_ack = first.into_acknowledgment();
        assert_eq!(
            worker.anchor_journal().unwrap().stage(),
            CompilerExecutionWorkerAnchorJournalStageV1::Published
        );
        assert_eq!(
            worker.last_record().unwrap().acknowledgment().unwrap(),
            first_ack
        );

        let replay = commit_externally_anchored_worker_publication(
            &mut worker,
            &mut anchor,
            request,
            publication,
        )
        .unwrap()
        .into_acknowledgment();
        assert_eq!(replay, first_ack);
        assert_no_anchor_challenge(&service);
    }

    #[test]
    fn prepared_anchor_restart_reuses_durable_challenge_before_publication() {
        let fixture = Fixture::new();
        let (request, publication) = issued_publication(&fixture, [0xa2; SHA256_BYTES]);
        let mut worker = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        let expected = match worker
            .prepare_external_anchor_publication(request.clone(), publication.clone())
            .unwrap()
        {
            WorkerExternalAnchorPublicationPlanV1::Exchange(challenge) => challenge,
            WorkerExternalAnchorPublicationPlanV1::CommitLocally => {
                panic!("fresh publication unexpectedly skipped anchor exchange")
            }
        };
        assert!(worker.last_record().is_none());
        drop(worker);

        let mut recovered =
            WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        let (mut anchor, service) = external_anchor_transport(&fixture);
        let response = spawn_anchor_response(
            service,
            Some(expected),
            AnchorPositionV1::Proposed,
            SigningKey::from_bytes(&[0x52; 32]),
        );
        commit_externally_anchored_worker_publication(
            &mut recovered,
            &mut anchor,
            request,
            publication,
        )
        .unwrap();
        drop(response.join().unwrap());
        assert!(recovered.last_record().is_some());
        assert_eq!(
            recovered.anchor_journal().unwrap().stage(),
            CompilerExecutionWorkerAnchorJournalStageV1::Published
        );
    }

    #[test]
    fn anchor_committed_restart_finishes_without_reexchange() {
        let fixture = Fixture::new();
        let anchor_signing_key = SigningKey::from_bytes(&[0x52; 32]);
        let (request, publication) = issued_publication(&fixture, [0xa3; SHA256_BYTES]);
        let mut worker = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        let challenge = match worker
            .prepare_external_anchor_publication(request.clone(), publication.clone())
            .unwrap()
        {
            WorkerExternalAnchorPublicationPlanV1::Exchange(challenge) => challenge,
            WorkerExternalAnchorPublicationPlanV1::CommitLocally => {
                panic!("fresh publication unexpectedly skipped anchor exchange")
            }
        };
        let observation =
            signed_anchor_observation(&challenge, AnchorPositionV1::Proposed, &anchor_signing_key);
        worker
            .record_external_anchor_observation(&observation)
            .unwrap();
        assert!(worker.last_record().is_none());
        drop(worker);

        let mut recovered =
            WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        let (mut anchor, service) = external_anchor_transport(&fixture);
        commit_externally_anchored_worker_publication(
            &mut recovered,
            &mut anchor,
            request,
            publication,
        )
        .unwrap();
        assert_no_anchor_challenge(&service);
        assert!(recovered.last_record().is_some());
    }

    #[test]
    fn prior_anchor_observation_aborts_without_worker_ack() {
        let fixture = Fixture::new();
        let (request, publication) = issued_publication(&fixture, [0xa4; SHA256_BYTES]);
        let mut worker = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        let (mut anchor, service) = external_anchor_transport(&fixture);
        let response = spawn_anchor_response(
            service,
            None,
            AnchorPositionV1::Prior,
            SigningKey::from_bytes(&[0x52; 32]),
        );

        assert!(matches!(
            commit_externally_anchored_worker_publication(
                &mut worker,
                &mut anchor,
                request.clone(),
                publication.clone(),
            ),
            Err(ProtectedCompilerExecutionIssuerErrorV1::WorkerLedger(
                ProtectedCompilerExecutionWorkerLedgerErrorV1::ExternalAnchorNotCommitted
            ))
        ));
        let service = response.join().unwrap();
        assert!(worker.last_record().is_none());
        assert_eq!(
            worker.anchor_journal().unwrap().stage(),
            CompilerExecutionWorkerAnchorJournalStageV1::Aborted
        );
        assert!(matches!(
            commit_externally_anchored_worker_publication(
                &mut worker,
                &mut anchor,
                request,
                publication,
            ),
            Err(ProtectedCompilerExecutionIssuerErrorV1::WorkerLedger(
                ProtectedCompilerExecutionWorkerLedgerErrorV1::ExternalAnchorNotCommitted
            ))
        ));
        assert_no_anchor_challenge(&service);
    }

    #[test]
    fn external_anchor_transport_failure_cannot_create_worker_ack() {
        let fixture = Fixture::new();
        let (request, publication) = issued_publication(&fixture, [0xa5; SHA256_BYTES]);
        let mut worker = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        let (mut anchor, service) = external_anchor_transport(&fixture);
        drop(service);

        assert!(matches!(
            commit_externally_anchored_worker_publication(
                &mut worker,
                &mut anchor,
                request,
                publication,
            ),
            Err(ProtectedCompilerExecutionIssuerErrorV1::ExternalAnchor(_))
        ));
        assert!(worker.last_record().is_none());
        assert_eq!(
            worker.anchor_journal().unwrap().stage(),
            CompilerExecutionWorkerAnchorJournalStageV1::PreparedAnchor
        );
        assert!(matches!(
            worker.commit_anchored_publication(),
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::ExternalAnchorNotCommitted)
        ));
    }

    #[test]
    fn record_sizes_and_all_stages_round_trip() {
        assert_eq!(COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V2, 2788);
        let fixture = Fixture::new();
        let ready = IssuerRecordV2::genesis(&fixture.policy, &fixture.signing_key).unwrap();
        let prepared = fixture.prepare(&ready, [0x71; 32]).unwrap();
        let request = CompilerExecutionAttestationRequestV1::new(
            prepared.challenge.clone().unwrap(),
            fixture.subject.clone(),
        )
        .unwrap();
        let issued = fixture.issue(&prepared, request.canonical_bytes()).unwrap();
        let ack = fixture.ack(&issued);
        let (next, outcome) = issued
            .acknowledge(&ack, &fixture.policy, &fixture.signing_key)
            .unwrap();
        assert_eq!(outcome, CompilerExecutionIssuerAckV1::Advanced);
        assert_eq!(next.last_ack.as_ref(), Some(&ack));

        for record in [&ready, &prepared, &issued, &next] {
            let decoded = IssuerRecordV2::decode(&record.canonical, &fixture.policy).unwrap();
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
    fn worker_ledger_join_accepts_only_the_three_exact_crash_positions() {
        let fixture = Fixture::new();
        let ready = IssuerRecordV2::genesis(&fixture.policy, &fixture.signing_key).unwrap();
        let prepared = fixture.prepare(&ready, [0x70; 32]).unwrap();
        let request = CompilerExecutionAttestationRequestV1::new(
            prepared.challenge.clone().unwrap(),
            fixture.subject.clone(),
        )
        .unwrap();
        let issued = fixture.issue(&prepared, request.canonical_bytes()).unwrap();
        let publication = issued.receipt_publication().unwrap();
        let mut worker = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();

        validate_worker_ledger_join(&ready, &worker).unwrap();
        validate_worker_ledger_join(&prepared, &worker).unwrap();
        validate_worker_ledger_join(&issued, &worker).unwrap();

        let reacquired =
            commit_worker_publication(&mut worker, request.clone(), publication.clone()).unwrap();
        validate_worker_ledger_join(&issued, &worker).unwrap();
        let committed =
            CommittedCompilerExecutionReceiptPublicationV1::from_reacquired_worker_record(
                reacquired,
            );
        let (acknowledged, outcome) = issued
            .acknowledge(
                committed.acknowledgment(),
                &fixture.policy,
                &fixture.signing_key,
            )
            .unwrap();
        assert_eq!(outcome, CompilerExecutionIssuerAckV1::Advanced);
        validate_worker_ledger_join(&acknowledged, &worker).unwrap();
        validate_publication_input(&acknowledged, &worker, &request, &publication).unwrap();

        let replay = commit_worker_publication(&mut worker, request, publication).unwrap();
        let replay =
            CommittedCompilerExecutionReceiptPublicationV1::from_reacquired_worker_record(replay);
        let (_, outcome) = acknowledged
            .acknowledge(
                replay.acknowledgment(),
                &fixture.policy,
                &fixture.signing_key,
            )
            .unwrap();
        assert_eq!(outcome, CompilerExecutionIssuerAckV1::AlreadyAcknowledged);
    }

    #[test]
    fn publication_substitution_fails_before_worker_state_changes() {
        let fixture = Fixture::new();
        let ready = IssuerRecordV2::genesis(&fixture.policy, &fixture.signing_key).unwrap();
        let prepared = fixture.prepare(&ready, [0x70; 32]).unwrap();
        let request = CompilerExecutionAttestationRequestV1::new(
            prepared.challenge.clone().unwrap(),
            fixture.subject.clone(),
        )
        .unwrap();
        let issued = fixture.issue(&prepared, request.canonical_bytes()).unwrap();
        let publication = issued.receipt_publication().unwrap();
        let worker = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        let substituted = CompilerExecutionReceiptPublicationV1::new(
            [0x94; SHA256_BYTES],
            publication.compiler_occurrence_identity(),
            publication.receipt().clone(),
        )
        .unwrap();

        assert!(matches!(
            validate_publication_input(&issued, &worker, &request, &substituted),
            Err(ProtectedCompilerExecutionIssuerErrorV1::WorkerLedgerJoin(_))
        ));
        assert!(worker.last_record().is_none());
    }

    #[test]
    fn cross_journal_gap_or_unrelated_worker_record_fails_closed() {
        let fixture = Fixture::new();
        let ready = IssuerRecordV2::genesis(&fixture.policy, &fixture.signing_key).unwrap();
        let prepared = fixture.prepare(&ready, [0x70; 32]).unwrap();
        let request = CompilerExecutionAttestationRequestV1::new(
            prepared.challenge.clone().unwrap(),
            fixture.subject.clone(),
        )
        .unwrap();
        let issued = fixture.issue(&prepared, request.canonical_bytes()).unwrap();
        let publication = issued.receipt_publication().unwrap();
        let ack = fixture.ack(&issued);
        let acknowledged = issued
            .acknowledge(&ack, &fixture.policy, &fixture.signing_key)
            .unwrap()
            .0;
        let mut worker = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        assert!(matches!(
            validate_worker_ledger_join(&acknowledged, &worker),
            Err(ProtectedCompilerExecutionIssuerErrorV1::WorkerLedgerJoin(_))
        ));

        commit_worker_publication(&mut worker, request, publication).unwrap();
        assert!(matches!(
            validate_worker_ledger_join(&acknowledged, &worker),
            Err(ProtectedCompilerExecutionIssuerErrorV1::WorkerLedgerJoin(_))
        ));
        assert!(matches!(
            validate_worker_ledger_join(&ready, &worker),
            Err(ProtectedCompilerExecutionIssuerErrorV1::WorkerLedgerJoin(_))
        ));
    }

    #[test]
    fn every_issued_record_byte_mutation_rejects() {
        let fixture = Fixture::new();
        let ready = IssuerRecordV2::genesis(&fixture.policy, &fixture.signing_key).unwrap();
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
                IssuerRecordV2::decode(&mutated, &fixture.policy).is_err(),
                "mutation at byte {index} was accepted"
            );
        }
    }

    #[test]
    fn every_acknowledged_record_byte_mutation_rejects() {
        let fixture = Fixture::new();
        let ready = IssuerRecordV2::genesis(&fixture.policy, &fixture.signing_key).unwrap();
        let prepared = fixture.prepare(&ready, [0x71; 32]).unwrap();
        let request = CompilerExecutionAttestationRequestV1::new(
            prepared.challenge.clone().unwrap(),
            fixture.subject.clone(),
        )
        .unwrap();
        let issued = fixture.issue(&prepared, request.canonical_bytes()).unwrap();
        let ack = fixture.ack(&issued);
        let acknowledged = issued
            .acknowledge(&ack, &fixture.policy, &fixture.signing_key)
            .unwrap()
            .0;
        for index in 0..acknowledged.canonical.len() {
            let mut mutated = acknowledged.canonical;
            mutated[index] ^= 0x80;
            assert!(
                IssuerRecordV2::decode(&mutated, &fixture.policy).is_err(),
                "mutation at byte {index} was accepted"
            );
        }
    }

    #[test]
    fn truncation_extension_policy_and_key_substitution_reject() {
        let fixture = Fixture::new();
        let record = IssuerRecordV2::genesis(&fixture.policy, &fixture.signing_key).unwrap();
        assert!(
            IssuerRecordV2::decode(
                &record.canonical[..record.canonical.len() - 1],
                &fixture.policy
            )
            .is_err()
        );
        let mut extended = record.canonical.to_vec();
        extended.push(0);
        assert!(IssuerRecordV2::decode(&extended, &fixture.policy).is_err());

        let wrong_key = SigningKey::from_bytes(&[0x53; 32]);
        let measurement =
            fe2o3_runtime_protocol::CompilerExecutionIssuerMeasurementV1::new([0x61; 32], 12_345)
                .unwrap();
        let wrong_policy = CompilerExecutionIssuerPolicyV1::new(
            7,
            measurement,
            measurement,
            wrong_key.verifying_key().to_bytes(),
            *fixture.policy.external_anchor_verifying_key(),
        )
        .unwrap();
        assert!(IssuerRecordV2::decode(&record.canonical, &wrong_policy).is_err());
        assert!(matches!(
            IssuerRecordV2::genesis(&fixture.policy, &wrong_key),
            Err(ProtectedCompilerExecutionIssuerErrorV1::SigningKeyMismatch)
        ));
    }

    #[test]
    fn legacy_v1_journal_presence_fails_closed_before_v2_genesis() {
        for entry in [LEGACY_V1_CANONICAL_RECORD, LEGACY_V1_REDO_RECORD] {
            let fixture = Fixture::new();
            let path = fixture.directory.path().join(entry);
            fs::write(&path, vec![0_u8; LEGACY_V1_RECORD_BYTES]).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

            assert!(matches!(
                IssuerLedgerV2::recover(fixture.root(), &fixture.policy, &fixture.signing_key),
                Err(ProtectedCompilerExecutionIssuerErrorV1::InvalidRecord(
                    "legacy V1 journal requires explicit fail-closed migration"
                ))
            ));
            assert!(!fixture.directory.path().join(CANONICAL_RECORD).exists());
        }
    }

    #[test]
    fn every_genesis_crash_boundary_recovers_one_exact_ready_record() {
        for boundary in RECORD_BOUNDARIES {
            for timing in FAULT_TIMINGS {
                let fixture = Fixture::new();
                let mut fault = RecordFault::new(boundary, timing);
                assert!(
                    IssuerLedgerV2::recover_with_hooks(
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
                    IssuerLedgerV2::recover(fixture.root(), &fixture.policy, &fixture.signing_key)
                        .unwrap();
                assert_eq!(recovered.record.stage, IssuerStageV2::Ready);
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
                    let mut ledger = IssuerLedgerV2::recover(
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
                                    let ack = fixture.ack(&ledger.record);
                                    ledger
                                        .record
                                        .acknowledge(&ack, &fixture.policy, &fixture.signing_key)
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

                    let recovered = IssuerLedgerV2::recover(
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
            IssuerLedgerV2::recover(fixture.root(), &fixture.policy, &fixture.signing_key).unwrap();
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
            IssuerLedgerV2::recover(fixture.root(), &fixture.policy, &fixture.signing_key).unwrap();
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
        let publication = issued.receipt_publication().unwrap();
        let receipt = publication.receipt().clone();
        ledger.commit(issued).unwrap();
        drop(ledger);

        let mut ledger =
            IssuerLedgerV2::recover(fixture.root(), &fixture.policy, &fixture.signing_key).unwrap();
        assert_eq!(
            ledger.recovery(),
            CompilerExecutionIssuerRecoveryV1::Issued {
                publication: publication.clone()
            }
        );
        receipt
            .clone()
            .verify(&fixture.policy, &request, [0; 32])
            .unwrap();
        let ack = fixture.ack(&ledger.record);
        let (ready, outcome) = ledger
            .record
            .acknowledge(&ack, &fixture.policy, &fixture.signing_key)
            .unwrap();
        assert_eq!(outcome, CompilerExecutionIssuerAckV1::Advanced);
        ledger.commit(ready).unwrap();
        let (same, outcome) = ledger
            .record
            .acknowledge(&ack, &fixture.policy, &fixture.signing_key)
            .unwrap();
        assert_eq!(outcome, CompilerExecutionIssuerAckV1::AlreadyAcknowledged);
        assert_eq!(same.canonical, ledger.record.canonical);

        let substituted_ack =
            CompilerExecutionReceiptPublicationAckV1::new(&publication, [0x94; SHA256_BYTES])
                .unwrap();
        assert!(matches!(
            ledger
                .record
                .acknowledge(&substituted_ack, &fixture.policy, &fixture.signing_key,),
            Err(ProtectedCompilerExecutionIssuerErrorV1::WrongStage { .. })
        ));
    }

    #[test]
    fn singleton_lock_rejects_a_second_live_issuer() {
        let fixture = Fixture::new();
        let _first =
            IssuerLedgerV2::recover(fixture.root(), &fixture.policy, &fixture.signing_key).unwrap();
        assert!(matches!(
            IssuerLedgerV2::recover(fixture.root(), &fixture.policy, &fixture.signing_key),
            Err(ProtectedCompilerExecutionIssuerErrorV1::SingletonLock(_))
        ));
    }

    #[test]
    fn explicit_unlock_releases_a_transient_duplicate_description() {
        let fixture = Fixture::new();
        let first =
            IssuerLedgerV2::recover(fixture.root(), &fixture.policy, &fixture.signing_key).unwrap();
        let inherited = rustix::io::dup(&first._singleton_lock.descriptor).unwrap();
        drop(first);

        let second =
            IssuerLedgerV2::recover(fixture.root(), &fixture.policy, &fixture.signing_key).unwrap();
        drop(inherited);
        drop(second);
    }

    #[test]
    fn signed_non_successor_redo_fails_closed() {
        let fixture = Fixture::new();
        let mut ledger =
            IssuerLedgerV2::recover(fixture.root(), &fixture.policy, &fixture.signing_key).unwrap();
        let prepared = fixture.prepare(&ledger.record, [0x77; 32]).unwrap();
        ledger.commit(prepared.clone()).unwrap();
        ledger
            .store
            .stage_record_redo(
                REDO_RECORD,
                &prepared.canonical,
                COMPILER_EXECUTION_ISSUER_DURABLE_RECORD_BYTES_V2,
                &mut NoRetainedDurableDirectoryHooksV1,
            )
            .unwrap();
        drop(ledger);

        assert!(matches!(
            IssuerLedgerV2::recover(fixture.root(), &fixture.policy, &fixture.signing_key),
            Err(ProtectedCompilerExecutionIssuerErrorV1::IllegalSuccessor)
        ));
    }

    #[test]
    fn substitutions_fail_before_a_receipt_exists() {
        let fixture = Fixture::new();
        let ready = IssuerRecordV2::genesis(&fixture.policy, &fixture.signing_key).unwrap();
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
        let ready = IssuerRecordV2::genesis(&fixture.policy, &fixture.signing_key).unwrap();
        let prepared = fixture.prepare(&ready, [0x75; 32]).unwrap();
        let recovered = IssuerRecordV2::decode(&prepared.canonical, &fixture.policy).unwrap();
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
