//! Canonical packet protocol for the protected compiler-execution service.
//!
//! These records are inert wire data. They grant no signing, publication, load, or launch
//! authority. The protected service must independently bind its retained peer, client pidfd,
//! policy, durable issuer journal, and Worker ledger before acting on a decoded request.

use std::{error::Error, fmt};

use fe2o3_artifact_transaction::{
    CompilerExecutionSubjectErrorV1, INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1,
    InertCompilerExecutionSubjectV1,
};
use sha2::{Digest, Sha256};

use crate::{
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1,
    COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V2,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1, CompilerExecutionAttestationChallengeV1,
    CompilerExecutionAttestationErrorV1, CompilerExecutionAttestationRequestV1,
    CompilerExecutionCurrentRecordAttestationV2, CompilerExecutionCurrentRecordVerificationErrorV2,
    CompilerExecutionCurrentRecordVerificationV2, CompilerExecutionIssuerPolicyIdentityV1,
    CompilerExecutionIssuerPolicyV1, CompilerExecutionReceiptCarriageV1,
    CompilerExecutionReceiptPublicationAckV1, CompilerExecutionReceiptPublicationErrorV1,
    CompilerExecutionReceiptPublicationV1,
};

const SHA256_BYTES: usize = 32;
const HEADER_BYTES: usize = 24;
const VERSION_V1: u16 = 1;
const REQUEST_MAGIC: [u8; 8] = *b"F2O3CSQ1";
const RESPONSE_MAGIC: [u8; 8] = *b"F2O3CSP1";
const REQUEST_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-SERVICE-REQUEST/V1\0";
const RESPONSE_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-SERVICE-RESPONSE/V1\0";

const REQUEST_COMMON_PREIMAGE_BYTES: usize = HEADER_BYTES + SHA256_BYTES + 8 + SHA256_BYTES;
const REQUEST_BASE_BYTES: usize = REQUEST_COMMON_PREIMAGE_BYTES + SHA256_BYTES;
/// Exact byte length of an inspect or cancel request.
pub const COMPILER_EXECUTION_SERVICE_CONTROL_REQUEST_BYTES_V1: usize = REQUEST_BASE_BYTES;
/// Exact byte length of a prepare request.
pub const COMPILER_EXECUTION_SERVICE_PREPARE_REQUEST_BYTES_V1: usize = REQUEST_BASE_BYTES;
/// Exact byte length of an issue request.
pub const COMPILER_EXECUTION_SERVICE_ISSUE_REQUEST_BYTES_V1: usize =
    REQUEST_BASE_BYTES + COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1;
/// Exact byte length of a publish request.
pub const COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_BYTES_V1: usize = REQUEST_BASE_BYTES
    + COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1
    + COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1;
/// Exact byte length of a subject-bound durable receipt recovery request.
pub const COMPILER_EXECUTION_SERVICE_RECOVER_REQUEST_BYTES_V1: usize =
    REQUEST_BASE_BYTES + INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1;
/// Exact byte length of an exact-carriage current-record verification request.
pub const COMPILER_EXECUTION_SERVICE_VERIFY_CURRENT_REQUEST_BYTES_V1: usize =
    REQUEST_BASE_BYTES + COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1 + SHA256_BYTES;
/// Maximum packet accepted by the protected compiler-execution service.
pub const MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1: usize =
    if COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_BYTES_V1
        > COMPILER_EXECUTION_SERVICE_VERIFY_CURRENT_REQUEST_BYTES_V1
    {
        COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_BYTES_V1
    } else {
        COMPILER_EXECUTION_SERVICE_VERIFY_CURRENT_REQUEST_BYTES_V1
    };

const RESPONSE_COMMON_PREIMAGE_BYTES: usize =
    HEADER_BYTES + SHA256_BYTES + SHA256_BYTES + 8 + SHA256_BYTES;
const RESPONSE_BASE_BYTES: usize = RESPONSE_COMMON_PREIMAGE_BYTES + SHA256_BYTES;
const PUBLISHED_DISPOSITION_BYTES: usize = 8;
/// Exact byte length of a ready or cancelled response.
pub const COMPILER_EXECUTION_SERVICE_CONTROL_RESPONSE_BYTES_V1: usize = RESPONSE_BASE_BYTES;
/// Exact byte length of a prepared response.
pub const COMPILER_EXECUTION_SERVICE_PREPARED_RESPONSE_BYTES_V1: usize =
    RESPONSE_BASE_BYTES + COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1;
/// Exact byte length of an issued response.
pub const COMPILER_EXECUTION_SERVICE_ISSUED_RESPONSE_BYTES_V1: usize =
    RESPONSE_BASE_BYTES + COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1;
/// Exact byte length of a published response.
pub const COMPILER_EXECUTION_SERVICE_PUBLISHED_RESPONSE_BYTES_V1: usize = RESPONSE_BASE_BYTES
    + PUBLISHED_DISPOSITION_BYTES
    + COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1;
/// Exact byte length of a complete recovered compiler-receipt carriage response.
pub const COMPILER_EXECUTION_SERVICE_RECOVERED_RESPONSE_BYTES_V1: usize =
    RESPONSE_BASE_BYTES + COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1;
/// Exact byte length of a protected current-record verification response.
pub const COMPILER_EXECUTION_SERVICE_VERIFIED_CURRENT_RESPONSE_BYTES_V1: usize =
    RESPONSE_BASE_BYTES + COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V2;
/// Maximum packet emitted by the protected compiler-execution service.
pub const MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V1: usize =
    if COMPILER_EXECUTION_SERVICE_RECOVERED_RESPONSE_BYTES_V1
        > COMPILER_EXECUTION_SERVICE_VERIFIED_CURRENT_RESPONSE_BYTES_V1
    {
        COMPILER_EXECUTION_SERVICE_RECOVERED_RESPONSE_BYTES_V1
    } else {
        COMPILER_EXECUTION_SERVICE_VERIFIED_CURRENT_RESPONSE_BYTES_V1
    };

/// Domain-separated identity of one exact canonical service request packet.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionServiceRequestIdentityV1([u8; SHA256_BYTES]);

impl CompilerExecutionServiceRequestIdentityV1 {
    /// Returns the exact request identity bytes.
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }
}

impl fmt::Debug for CompilerExecutionServiceRequestIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CompilerExecutionServiceRequestIdentityV1")
            .field(&self.0)
            .finish()
    }
}

/// Domain-separated identity of one exact canonical service response packet.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionServiceResponseIdentityV1([u8; SHA256_BYTES]);

impl CompilerExecutionServiceResponseIdentityV1 {
    /// Returns the exact response identity bytes.
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }
}

impl fmt::Debug for CompilerExecutionServiceResponseIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CompilerExecutionServiceResponseIdentityV1")
            .field(&self.0)
            .finish()
    }
}

/// Operation selected by one canonical service request packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum CompilerExecutionServiceRequestKindV1 {
    Inspect = 1,
    Prepare = 2,
    Issue = 3,
    Publish = 4,
    Cancel = 5,
    Recover = 6,
    VerifyCurrent = 7,
}

impl CompilerExecutionServiceRequestKindV1 {
    fn decode(value: u16) -> Result<Self, CompilerExecutionServiceProtocolErrorV1> {
        match value {
            1 => Ok(Self::Inspect),
            2 => Ok(Self::Prepare),
            3 => Ok(Self::Issue),
            4 => Ok(Self::Publish),
            5 => Ok(Self::Cancel),
            6 => Ok(Self::Recover),
            7 => Ok(Self::VerifyCurrent),
            other => Err(CompilerExecutionServiceProtocolErrorV1::UnknownRequestKind(
                other,
            )),
        }
    }

    const fn packet_bytes(self) -> usize {
        match self {
            Self::Inspect | Self::Cancel => COMPILER_EXECUTION_SERVICE_CONTROL_REQUEST_BYTES_V1,
            Self::Prepare => COMPILER_EXECUTION_SERVICE_PREPARE_REQUEST_BYTES_V1,
            Self::Issue => COMPILER_EXECUTION_SERVICE_ISSUE_REQUEST_BYTES_V1,
            Self::Publish => COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_BYTES_V1,
            Self::Recover => COMPILER_EXECUTION_SERVICE_RECOVER_REQUEST_BYTES_V1,
            Self::VerifyCurrent => COMPILER_EXECUTION_SERVICE_VERIFY_CURRENT_REQUEST_BYTES_V1,
        }
    }
}

/// Canonical request packet bound to one policy and rollback position.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerExecutionServiceRequestV1 {
    kind: CompilerExecutionServiceRequestKindV1,
    policy_identity: CompilerExecutionIssuerPolicyIdentityV1,
    expected_sequence: u64,
    expected_rollback_anchor: [u8; SHA256_BYTES],
    subject: Option<InertCompilerExecutionSubjectV1>,
    request: Option<CompilerExecutionAttestationRequestV1>,
    publication: Option<CompilerExecutionReceiptPublicationV1>,
    carriage: Option<CompilerExecutionReceiptCarriageV1>,
    verification_challenge: Option<[u8; SHA256_BYTES]>,
    identity: CompilerExecutionServiceRequestIdentityV1,
    canonical_bytes: [u8; MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1],
    canonical_len: usize,
}

impl CompilerExecutionServiceRequestV1 {
    /// Constructs a state-inspection packet. Inspection carries no caller-asserted position.
    pub fn inspect(policy: &CompilerExecutionIssuerPolicyV1) -> Self {
        Self::encode(
            CompilerExecutionServiceRequestKindV1::Inspect,
            policy.identity(),
            0,
            [0; SHA256_BYTES],
            None,
            None,
            None,
            None,
            None,
        )
        .expect("fixed inspect request is canonical")
    }

    /// Constructs a prepare packet for one exact caller-current rollback position.
    pub fn prepare(
        policy: &CompilerExecutionIssuerPolicyV1,
        expected_sequence: u64,
        expected_rollback_anchor: [u8; SHA256_BYTES],
    ) -> Result<Self, CompilerExecutionServiceProtocolErrorV1> {
        Self::encode(
            CompilerExecutionServiceRequestKindV1::Prepare,
            policy.identity(),
            expected_sequence,
            expected_rollback_anchor,
            None,
            None,
            None,
            None,
            None,
        )
    }

    /// Constructs an issue packet. Its expected position is derived from the exact request.
    pub fn issue(
        policy: &CompilerExecutionIssuerPolicyV1,
        request: CompilerExecutionAttestationRequestV1,
    ) -> Result<Self, CompilerExecutionServiceProtocolErrorV1> {
        if request.challenge().policy_identity() != policy.identity() {
            return Err(CompilerExecutionServiceProtocolErrorV1::PolicyMismatch);
        }
        let sequence = request.challenge().sequence();
        let anchor = request.challenge().prior_rollback_anchor();
        Self::encode(
            CompilerExecutionServiceRequestKindV1::Issue,
            policy.identity(),
            sequence,
            anchor,
            None,
            Some(request),
            None,
            None,
            None,
        )
    }

    /// Constructs a publish packet after independently verifying its signed receipt.
    pub fn publish(
        policy: &CompilerExecutionIssuerPolicyV1,
        request: CompilerExecutionAttestationRequestV1,
        publication: CompilerExecutionReceiptPublicationV1,
    ) -> Result<Self, CompilerExecutionServiceProtocolErrorV1> {
        if request.challenge().policy_identity() != policy.identity()
            || publication.policy_identity() != policy.identity()
        {
            return Err(CompilerExecutionServiceProtocolErrorV1::PolicyMismatch);
        }
        let sequence = request.challenge().sequence();
        let anchor = request.challenge().prior_rollback_anchor();
        publication
            .receipt()
            .clone()
            .verify(policy, &request, anchor)?;
        Self::encode(
            CompilerExecutionServiceRequestKindV1::Publish,
            policy.identity(),
            sequence,
            anchor,
            None,
            Some(request),
            Some(publication),
            None,
            None,
        )
    }

    /// Constructs a request to recover the exact current durable receipt for one subject.
    pub fn recover(
        policy: &CompilerExecutionIssuerPolicyV1,
        subject: InertCompilerExecutionSubjectV1,
    ) -> Result<Self, CompilerExecutionServiceProtocolErrorV1> {
        Self::encode(
            CompilerExecutionServiceRequestKindV1::Recover,
            policy.identity(),
            0,
            [0; SHA256_BYTES],
            Some(subject),
            None,
            None,
            None,
            None,
        )
    }

    /// Constructs a terminal request to verify one exact carriage against protected current state.
    pub fn verify_current(
        policy: &CompilerExecutionIssuerPolicyV1,
        carriage: CompilerExecutionReceiptCarriageV1,
        verification_challenge: [u8; SHA256_BYTES],
    ) -> Result<Self, CompilerExecutionServiceProtocolErrorV1> {
        if carriage.policy() != policy {
            return Err(CompilerExecutionServiceProtocolErrorV1::PolicyMismatch);
        }
        if verification_challenge == [0; SHA256_BYTES] {
            return Err(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch);
        }
        let sequence = carriage.acknowledgment().sequence();
        let current_rollback_anchor = carriage.acknowledgment().current_rollback_anchor();
        Self::encode(
            CompilerExecutionServiceRequestKindV1::VerifyCurrent,
            policy.identity(),
            sequence,
            current_rollback_anchor,
            None,
            None,
            None,
            Some(carriage),
            Some(verification_challenge),
        )
    }

    /// Constructs an explicit cancellation packet. Cancellation carries no asserted position.
    pub fn cancel(policy: &CompilerExecutionIssuerPolicyV1) -> Self {
        Self::encode(
            CompilerExecutionServiceRequestKindV1::Cancel,
            policy.identity(),
            0,
            [0; SHA256_BYTES],
            None,
            None,
            None,
            None,
            None,
        )
        .expect("fixed cancel request is canonical")
    }

    /// Strictly decodes one complete `SOCK_SEQPACKET` payload.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionServiceProtocolErrorV1> {
        if bytes.len() < COMPILER_EXECUTION_SERVICE_CONTROL_REQUEST_BYTES_V1
            || bytes.len() > MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1
        {
            return Err(CompilerExecutionServiceProtocolErrorV1::RequestLength);
        }
        let mut reader = Reader::new(bytes);
        let kind = CompilerExecutionServiceRequestKindV1::decode(decode_header(
            &mut reader,
            REQUEST_MAGIC,
            bytes.len(),
        )?)?;
        if bytes.len() != kind.packet_bytes() {
            return Err(CompilerExecutionServiceProtocolErrorV1::RequestLength);
        }
        let policy_identity = CompilerExecutionIssuerPolicyIdentityV1::from_bytes_for_protocol(
            reader.fixed::<SHA256_BYTES>()?,
        );
        let expected_sequence = reader.u64()?;
        let expected_rollback_anchor = reader.fixed::<SHA256_BYTES>()?;
        let (subject, request, publication, carriage, verification_challenge) = match kind {
            CompilerExecutionServiceRequestKindV1::Inspect
            | CompilerExecutionServiceRequestKindV1::Prepare
            | CompilerExecutionServiceRequestKindV1::Cancel => (None, None, None, None, None),
            CompilerExecutionServiceRequestKindV1::Issue => (
                None,
                Some(CompilerExecutionAttestationRequestV1::decode(
                    reader.take(COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1)?,
                )?),
                None,
                None,
                None,
            ),
            CompilerExecutionServiceRequestKindV1::Publish => (
                None,
                Some(CompilerExecutionAttestationRequestV1::decode(
                    reader.take(COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1)?,
                )?),
                Some(CompilerExecutionReceiptPublicationV1::decode(
                    reader.take(COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1)?,
                )?),
                None,
                None,
            ),
            CompilerExecutionServiceRequestKindV1::Recover => (
                Some(InertCompilerExecutionSubjectV1::decode(
                    reader.take(INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1)?,
                )?),
                None,
                None,
                None,
                None,
            ),
            CompilerExecutionServiceRequestKindV1::VerifyCurrent => (
                None,
                None,
                None,
                Some(CompilerExecutionReceiptCarriageV1::decode(
                    reader.take(COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1)?,
                )?),
                Some(reader.fixed::<SHA256_BYTES>()?),
            ),
        };
        let declared_identity = reader.fixed::<SHA256_BYTES>()?;
        if !reader.is_empty() {
            return Err(CompilerExecutionServiceProtocolErrorV1::TrailingBytes);
        }
        let decoded = Self::encode(
            kind,
            policy_identity,
            expected_sequence,
            expected_rollback_anchor,
            subject,
            request,
            publication,
            carriage,
            verification_challenge,
        )?;
        if declared_identity != decoded.identity.0 || decoded.canonical_bytes() != bytes {
            return Err(CompilerExecutionServiceProtocolErrorV1::IdentityMismatch);
        }
        Ok(decoded)
    }

    #[allow(clippy::too_many_arguments)]
    fn encode(
        kind: CompilerExecutionServiceRequestKindV1,
        policy_identity: CompilerExecutionIssuerPolicyIdentityV1,
        expected_sequence: u64,
        expected_rollback_anchor: [u8; SHA256_BYTES],
        subject: Option<InertCompilerExecutionSubjectV1>,
        request: Option<CompilerExecutionAttestationRequestV1>,
        publication: Option<CompilerExecutionReceiptPublicationV1>,
        carriage: Option<CompilerExecutionReceiptCarriageV1>,
        verification_challenge: Option<[u8; SHA256_BYTES]>,
    ) -> Result<Self, CompilerExecutionServiceProtocolErrorV1> {
        validate_request_fields(
            kind,
            policy_identity,
            expected_sequence,
            expected_rollback_anchor,
            subject.as_ref(),
            request.as_ref(),
            publication.as_ref(),
            carriage.as_ref(),
            verification_challenge,
        )?;
        let canonical_len = kind.packet_bytes();
        let mut canonical_bytes = [0_u8; MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1];
        let mut offset = encode_header(
            &mut canonical_bytes,
            REQUEST_MAGIC,
            kind as u16,
            canonical_len,
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            policy_identity.as_bytes(),
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            &expected_sequence.to_le_bytes(),
        );
        put(&mut canonical_bytes, &mut offset, &expected_rollback_anchor);
        if let Some(subject) = subject.as_ref() {
            put(&mut canonical_bytes, &mut offset, subject.canonical_bytes());
        }
        if let Some(request) = request.as_ref() {
            put(&mut canonical_bytes, &mut offset, request.canonical_bytes());
        }
        if let Some(publication) = publication.as_ref() {
            put(
                &mut canonical_bytes,
                &mut offset,
                publication.canonical_bytes(),
            );
        }
        if let Some(carriage) = carriage.as_ref() {
            put(
                &mut canonical_bytes,
                &mut offset,
                carriage.canonical_bytes(),
            );
        }
        if let Some(verification_challenge) = verification_challenge {
            put(&mut canonical_bytes, &mut offset, &verification_challenge);
        }
        debug_assert_eq!(offset + SHA256_BYTES, canonical_len);
        let identity = CompilerExecutionServiceRequestIdentityV1(derive_identity(
            REQUEST_IDENTITY_DOMAIN,
            &canonical_bytes[..offset],
        ));
        put(&mut canonical_bytes, &mut offset, &identity.0);
        debug_assert_eq!(offset, canonical_len);
        Ok(Self {
            kind,
            policy_identity,
            expected_sequence,
            expected_rollback_anchor,
            subject,
            request,
            publication,
            carriage,
            verification_challenge,
            identity,
            canonical_bytes,
            canonical_len,
        })
    }

    pub const fn kind(&self) -> CompilerExecutionServiceRequestKindV1 {
        self.kind
    }

    pub const fn policy_identity(&self) -> CompilerExecutionIssuerPolicyIdentityV1 {
        self.policy_identity
    }

    pub const fn expected_sequence(&self) -> u64 {
        self.expected_sequence
    }

    pub const fn expected_rollback_anchor(&self) -> [u8; SHA256_BYTES] {
        self.expected_rollback_anchor
    }

    pub const fn subject(&self) -> Option<&InertCompilerExecutionSubjectV1> {
        self.subject.as_ref()
    }

    pub const fn request(&self) -> Option<&CompilerExecutionAttestationRequestV1> {
        self.request.as_ref()
    }

    pub const fn publication(&self) -> Option<&CompilerExecutionReceiptPublicationV1> {
        self.publication.as_ref()
    }

    pub const fn carriage(&self) -> Option<&CompilerExecutionReceiptCarriageV1> {
        self.carriage.as_ref()
    }

    pub const fn verification_challenge(&self) -> Option<[u8; SHA256_BYTES]> {
        self.verification_challenge
    }

    pub const fn identity(&self) -> CompilerExecutionServiceRequestIdentityV1 {
        self.identity
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes[..self.canonical_len]
    }

    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for CompilerExecutionServiceRequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionServiceRequestV1")
            .field("kind", &self.kind)
            .field("policy_identity", &self.policy_identity)
            .field("expected_sequence", &self.expected_sequence)
            .field("expected_rollback_anchor", &self.expected_rollback_anchor)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

/// State or terminal result carried by one canonical service response packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum CompilerExecutionServiceResponseKindV1 {
    Ready = 1,
    Prepared = 2,
    Issued = 3,
    Published = 4,
    Cancelled = 5,
    Recovered = 6,
    ReceiptAbsent = 7,
    VerifiedCurrent = 8,
}

impl CompilerExecutionServiceResponseKindV1 {
    fn decode(value: u16) -> Result<Self, CompilerExecutionServiceProtocolErrorV1> {
        match value {
            1 => Ok(Self::Ready),
            2 => Ok(Self::Prepared),
            3 => Ok(Self::Issued),
            4 => Ok(Self::Published),
            5 => Ok(Self::Cancelled),
            6 => Ok(Self::Recovered),
            7 => Ok(Self::ReceiptAbsent),
            8 => Ok(Self::VerifiedCurrent),
            other => Err(CompilerExecutionServiceProtocolErrorV1::UnknownResponseKind(other)),
        }
    }

    const fn packet_bytes(self) -> usize {
        match self {
            Self::Ready | Self::Cancelled | Self::ReceiptAbsent => {
                COMPILER_EXECUTION_SERVICE_CONTROL_RESPONSE_BYTES_V1
            }
            Self::Prepared => COMPILER_EXECUTION_SERVICE_PREPARED_RESPONSE_BYTES_V1,
            Self::Issued => COMPILER_EXECUTION_SERVICE_ISSUED_RESPONSE_BYTES_V1,
            Self::Published => COMPILER_EXECUTION_SERVICE_PUBLISHED_RESPONSE_BYTES_V1,
            Self::Recovered => COMPILER_EXECUTION_SERVICE_RECOVERED_RESPONSE_BYTES_V1,
            Self::VerifiedCurrent => COMPILER_EXECUTION_SERVICE_VERIFIED_CURRENT_RESPONSE_BYTES_V1,
        }
    }
}

/// Whether a publish operation advanced state or replayed its exact durable result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CompilerExecutionServicePublishDispositionV1 {
    Advanced = 1,
    AlreadyAcknowledged = 2,
}

impl CompilerExecutionServicePublishDispositionV1 {
    fn decode(value: u8) -> Result<Self, CompilerExecutionServiceProtocolErrorV1> {
        match value {
            1 => Ok(Self::Advanced),
            2 => Ok(Self::AlreadyAcknowledged),
            other => Err(CompilerExecutionServiceProtocolErrorV1::UnknownDisposition(
                other,
            )),
        }
    }
}

/// Canonical response packet correlated to one exact request identity.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerExecutionServiceResponseV1 {
    kind: CompilerExecutionServiceResponseKindV1,
    request_identity: CompilerExecutionServiceRequestIdentityV1,
    policy_identity: CompilerExecutionIssuerPolicyIdentityV1,
    sequence: u64,
    rollback_anchor: [u8; SHA256_BYTES],
    challenge: Option<CompilerExecutionAttestationChallengeV1>,
    publication: Option<CompilerExecutionReceiptPublicationV1>,
    carriage: Option<CompilerExecutionReceiptCarriageV1>,
    current_record_attestation: Option<CompilerExecutionCurrentRecordAttestationV2>,
    acknowledgment: Option<CompilerExecutionReceiptPublicationAckV1>,
    disposition: Option<CompilerExecutionServicePublishDispositionV1>,
    identity: CompilerExecutionServiceResponseIdentityV1,
    canonical_bytes: [u8; MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V1],
    canonical_len: usize,
}

impl CompilerExecutionServiceResponseV1 {
    pub fn ready(
        request_identity: CompilerExecutionServiceRequestIdentityV1,
        policy: &CompilerExecutionIssuerPolicyV1,
        sequence: u64,
        rollback_anchor: [u8; SHA256_BYTES],
    ) -> Result<Self, CompilerExecutionServiceProtocolErrorV1> {
        Self::encode(
            CompilerExecutionServiceResponseKindV1::Ready,
            request_identity,
            policy.identity(),
            sequence,
            rollback_anchor,
            None,
            None,
            None,
            None,
            None,
            None,
        )
    }

    pub fn prepared(
        request_identity: CompilerExecutionServiceRequestIdentityV1,
        policy: &CompilerExecutionIssuerPolicyV1,
        challenge: CompilerExecutionAttestationChallengeV1,
    ) -> Result<Self, CompilerExecutionServiceProtocolErrorV1> {
        let sequence = challenge.sequence();
        let rollback_anchor = challenge.prior_rollback_anchor();
        Self::encode(
            CompilerExecutionServiceResponseKindV1::Prepared,
            request_identity,
            policy.identity(),
            sequence,
            rollback_anchor,
            Some(challenge),
            None,
            None,
            None,
            None,
            None,
        )
    }

    pub fn issued(
        request_identity: CompilerExecutionServiceRequestIdentityV1,
        policy: &CompilerExecutionIssuerPolicyV1,
        publication: CompilerExecutionReceiptPublicationV1,
    ) -> Result<Self, CompilerExecutionServiceProtocolErrorV1> {
        let sequence = publication.receipt().sequence();
        let rollback_anchor = publication.receipt().prior_rollback_anchor();
        Self::encode(
            CompilerExecutionServiceResponseKindV1::Issued,
            request_identity,
            policy.identity(),
            sequence,
            rollback_anchor,
            None,
            Some(publication),
            None,
            None,
            None,
            None,
        )
    }

    pub fn published(
        request_identity: CompilerExecutionServiceRequestIdentityV1,
        policy: &CompilerExecutionIssuerPolicyV1,
        acknowledgment: CompilerExecutionReceiptPublicationAckV1,
        disposition: CompilerExecutionServicePublishDispositionV1,
    ) -> Result<Self, CompilerExecutionServiceProtocolErrorV1> {
        let sequence = acknowledgment.sequence();
        let rollback_anchor = acknowledgment.current_rollback_anchor();
        Self::encode(
            CompilerExecutionServiceResponseKindV1::Published,
            request_identity,
            policy.identity(),
            sequence,
            rollback_anchor,
            None,
            None,
            None,
            None,
            Some(acknowledgment),
            Some(disposition),
        )
    }

    pub fn recovered(
        request_identity: CompilerExecutionServiceRequestIdentityV1,
        carriage: CompilerExecutionReceiptCarriageV1,
    ) -> Result<Self, CompilerExecutionServiceProtocolErrorV1> {
        let policy_identity = carriage.policy().identity();
        let sequence = carriage.acknowledgment().sequence();
        let rollback_anchor = carriage.acknowledgment().current_rollback_anchor();
        Self::encode(
            CompilerExecutionServiceResponseKindV1::Recovered,
            request_identity,
            policy_identity,
            sequence,
            rollback_anchor,
            None,
            None,
            Some(carriage),
            None,
            None,
            None,
        )
    }

    /// Returns a challenge-bound signature over the protected service's exact-current record.
    pub fn verified_current(
        request_identity: CompilerExecutionServiceRequestIdentityV1,
        attestation: CompilerExecutionCurrentRecordAttestationV2,
    ) -> Result<Self, CompilerExecutionServiceProtocolErrorV1> {
        let verification = attestation.verification();
        Self::encode(
            CompilerExecutionServiceResponseKindV1::VerifiedCurrent,
            request_identity,
            CompilerExecutionIssuerPolicyIdentityV1::from_bytes_for_protocol(
                verification.policy_identity(),
            ),
            verification.sequence(),
            verification.current_rollback_anchor(),
            None,
            None,
            None,
            Some(attestation),
            None,
            None,
        )
    }

    /// Reports that no current Worker receipt exists for the requested subject.
    ///
    /// This is nonterminal service state, not evidence that a previously published receipt was
    /// lost. A current record for a different subject remains a fail-closed subject mismatch.
    pub fn receipt_absent(
        request_identity: CompilerExecutionServiceRequestIdentityV1,
        policy: &CompilerExecutionIssuerPolicyV1,
        sequence: u64,
        rollback_anchor: [u8; SHA256_BYTES],
    ) -> Result<Self, CompilerExecutionServiceProtocolErrorV1> {
        Self::encode(
            CompilerExecutionServiceResponseKindV1::ReceiptAbsent,
            request_identity,
            policy.identity(),
            sequence,
            rollback_anchor,
            None,
            None,
            None,
            None,
            None,
            None,
        )
    }

    pub fn cancelled(
        request_identity: CompilerExecutionServiceRequestIdentityV1,
        policy: &CompilerExecutionIssuerPolicyV1,
        sequence: u64,
        rollback_anchor: [u8; SHA256_BYTES],
    ) -> Result<Self, CompilerExecutionServiceProtocolErrorV1> {
        Self::encode(
            CompilerExecutionServiceResponseKindV1::Cancelled,
            request_identity,
            policy.identity(),
            sequence,
            rollback_anchor,
            None,
            None,
            None,
            None,
            None,
            None,
        )
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionServiceProtocolErrorV1> {
        if bytes.len() < COMPILER_EXECUTION_SERVICE_CONTROL_RESPONSE_BYTES_V1
            || bytes.len() > MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V1
        {
            return Err(CompilerExecutionServiceProtocolErrorV1::ResponseLength);
        }
        let mut reader = Reader::new(bytes);
        let kind = CompilerExecutionServiceResponseKindV1::decode(decode_header(
            &mut reader,
            RESPONSE_MAGIC,
            bytes.len(),
        )?)?;
        if bytes.len() != kind.packet_bytes() {
            return Err(CompilerExecutionServiceProtocolErrorV1::ResponseLength);
        }
        let request_identity =
            CompilerExecutionServiceRequestIdentityV1(reader.fixed::<SHA256_BYTES>()?);
        let policy_identity = CompilerExecutionIssuerPolicyIdentityV1::from_bytes_for_protocol(
            reader.fixed::<SHA256_BYTES>()?,
        );
        let sequence = reader.u64()?;
        let rollback_anchor = reader.fixed::<SHA256_BYTES>()?;
        let (
            challenge,
            publication,
            carriage,
            current_record_attestation,
            acknowledgment,
            disposition,
        ) = match kind {
            CompilerExecutionServiceResponseKindV1::Ready
            | CompilerExecutionServiceResponseKindV1::Cancelled
            | CompilerExecutionServiceResponseKindV1::ReceiptAbsent => {
                (None, None, None, None, None, None)
            }
            CompilerExecutionServiceResponseKindV1::Prepared => (
                Some(CompilerExecutionAttestationChallengeV1::decode(
                    reader.take(COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1)?,
                )?),
                None,
                None,
                None,
                None,
                None,
            ),
            CompilerExecutionServiceResponseKindV1::Issued => (
                None,
                Some(CompilerExecutionReceiptPublicationV1::decode(
                    reader.take(COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1)?,
                )?),
                None,
                None,
                None,
                None,
            ),
            CompilerExecutionServiceResponseKindV1::Published => {
                let disposition =
                    CompilerExecutionServicePublishDispositionV1::decode(reader.u8()?)?;
                if reader.fixed::<7>()? != [0; 7] {
                    return Err(CompilerExecutionServiceProtocolErrorV1::NonzeroReserved);
                }
                (
                    None,
                    None,
                    None,
                    None,
                    Some(CompilerExecutionReceiptPublicationAckV1::decode(
                        reader.take(COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1)?,
                    )?),
                    Some(disposition),
                )
            }
            CompilerExecutionServiceResponseKindV1::Recovered => (
                None,
                None,
                Some(CompilerExecutionReceiptCarriageV1::decode(
                    reader.take(COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1)?,
                )?),
                None,
                None,
                None,
            ),
            CompilerExecutionServiceResponseKindV1::VerifiedCurrent => (
                None,
                None,
                None,
                Some(CompilerExecutionCurrentRecordAttestationV2::decode(
                    reader.take(COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V2)?,
                )?),
                None,
                None,
            ),
        };
        let declared_identity = reader.fixed::<SHA256_BYTES>()?;
        if !reader.is_empty() {
            return Err(CompilerExecutionServiceProtocolErrorV1::TrailingBytes);
        }
        let decoded = Self::encode(
            kind,
            request_identity,
            policy_identity,
            sequence,
            rollback_anchor,
            challenge,
            publication,
            carriage,
            current_record_attestation,
            acknowledgment,
            disposition,
        )?;
        if declared_identity != decoded.identity.0 || decoded.canonical_bytes() != bytes {
            return Err(CompilerExecutionServiceProtocolErrorV1::IdentityMismatch);
        }
        Ok(decoded)
    }

    #[allow(clippy::too_many_arguments)]
    fn encode(
        kind: CompilerExecutionServiceResponseKindV1,
        request_identity: CompilerExecutionServiceRequestIdentityV1,
        policy_identity: CompilerExecutionIssuerPolicyIdentityV1,
        sequence: u64,
        rollback_anchor: [u8; SHA256_BYTES],
        challenge: Option<CompilerExecutionAttestationChallengeV1>,
        publication: Option<CompilerExecutionReceiptPublicationV1>,
        carriage: Option<CompilerExecutionReceiptCarriageV1>,
        current_record_attestation: Option<CompilerExecutionCurrentRecordAttestationV2>,
        acknowledgment: Option<CompilerExecutionReceiptPublicationAckV1>,
        disposition: Option<CompilerExecutionServicePublishDispositionV1>,
    ) -> Result<Self, CompilerExecutionServiceProtocolErrorV1> {
        validate_response_fields(
            kind,
            request_identity,
            policy_identity,
            sequence,
            rollback_anchor,
            challenge.as_ref(),
            publication.as_ref(),
            carriage.as_ref(),
            current_record_attestation.as_ref(),
            acknowledgment.as_ref(),
            disposition,
        )?;
        let canonical_len = kind.packet_bytes();
        let mut canonical_bytes = [0_u8; MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V1];
        let mut offset = encode_header(
            &mut canonical_bytes,
            RESPONSE_MAGIC,
            kind as u16,
            canonical_len,
        );
        put(&mut canonical_bytes, &mut offset, &request_identity.0);
        put(
            &mut canonical_bytes,
            &mut offset,
            policy_identity.as_bytes(),
        );
        put(&mut canonical_bytes, &mut offset, &sequence.to_le_bytes());
        put(&mut canonical_bytes, &mut offset, &rollback_anchor);
        if let Some(challenge) = challenge.as_ref() {
            put(
                &mut canonical_bytes,
                &mut offset,
                challenge.canonical_bytes(),
            );
        }
        if let Some(publication) = publication.as_ref() {
            put(
                &mut canonical_bytes,
                &mut offset,
                publication.canonical_bytes(),
            );
        }
        if let Some(carriage) = carriage.as_ref() {
            put(
                &mut canonical_bytes,
                &mut offset,
                carriage.canonical_bytes(),
            );
        }
        if let Some(verification) = current_record_attestation.as_ref() {
            put(
                &mut canonical_bytes,
                &mut offset,
                verification.canonical_bytes(),
            );
        }
        if let Some(disposition) = disposition {
            canonical_bytes[offset] = disposition as u8;
            offset += PUBLISHED_DISPOSITION_BYTES;
        }
        if let Some(acknowledgment) = acknowledgment.as_ref() {
            put(
                &mut canonical_bytes,
                &mut offset,
                acknowledgment.canonical_bytes(),
            );
        }
        debug_assert_eq!(offset + SHA256_BYTES, canonical_len);
        let identity = CompilerExecutionServiceResponseIdentityV1(derive_identity(
            RESPONSE_IDENTITY_DOMAIN,
            &canonical_bytes[..offset],
        ));
        put(&mut canonical_bytes, &mut offset, &identity.0);
        debug_assert_eq!(offset, canonical_len);
        Ok(Self {
            kind,
            request_identity,
            policy_identity,
            sequence,
            rollback_anchor,
            challenge,
            publication,
            carriage,
            current_record_attestation,
            acknowledgment,
            disposition,
            identity,
            canonical_bytes,
            canonical_len,
        })
    }

    pub const fn kind(&self) -> CompilerExecutionServiceResponseKindV1 {
        self.kind
    }

    pub const fn request_identity(&self) -> CompilerExecutionServiceRequestIdentityV1 {
        self.request_identity
    }

    pub const fn policy_identity(&self) -> CompilerExecutionIssuerPolicyIdentityV1 {
        self.policy_identity
    }

    /// Returns the active sequence, or the completed receipt sequence for `Published`.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Returns the prior anchor, or the resulting current anchor for `Published`.
    pub const fn rollback_anchor(&self) -> [u8; SHA256_BYTES] {
        self.rollback_anchor
    }

    pub const fn challenge(&self) -> Option<&CompilerExecutionAttestationChallengeV1> {
        self.challenge.as_ref()
    }

    pub const fn publication(&self) -> Option<&CompilerExecutionReceiptPublicationV1> {
        self.publication.as_ref()
    }

    pub const fn carriage(&self) -> Option<&CompilerExecutionReceiptCarriageV1> {
        self.carriage.as_ref()
    }

    pub const fn current_record_attestation(
        &self,
    ) -> Option<&CompilerExecutionCurrentRecordAttestationV2> {
        self.current_record_attestation.as_ref()
    }

    pub const fn current_record_verification(
        &self,
    ) -> Option<&CompilerExecutionCurrentRecordVerificationV2> {
        match self.current_record_attestation.as_ref() {
            Some(attestation) => Some(attestation.verification()),
            None => None,
        }
    }

    pub const fn acknowledgment(&self) -> Option<&CompilerExecutionReceiptPublicationAckV1> {
        self.acknowledgment.as_ref()
    }

    pub const fn disposition(&self) -> Option<CompilerExecutionServicePublishDispositionV1> {
        self.disposition
    }

    pub const fn identity(&self) -> CompilerExecutionServiceResponseIdentityV1 {
        self.identity
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes[..self.canonical_len]
    }

    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for CompilerExecutionServiceResponseV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionServiceResponseV1")
            .field("kind", &self.kind)
            .field("request_identity", &self.request_identity)
            .field("policy_identity", &self.policy_identity)
            .field("sequence", &self.sequence)
            .field("rollback_anchor", &self.rollback_anchor)
            .field("disposition", &self.disposition)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_request_fields(
    kind: CompilerExecutionServiceRequestKindV1,
    policy_identity: CompilerExecutionIssuerPolicyIdentityV1,
    expected_sequence: u64,
    expected_rollback_anchor: [u8; SHA256_BYTES],
    subject: Option<&InertCompilerExecutionSubjectV1>,
    request: Option<&CompilerExecutionAttestationRequestV1>,
    publication: Option<&CompilerExecutionReceiptPublicationV1>,
    carriage: Option<&CompilerExecutionReceiptCarriageV1>,
    verification_challenge: Option<[u8; SHA256_BYTES]>,
) -> Result<(), CompilerExecutionServiceProtocolErrorV1> {
    if *policy_identity.as_bytes() == [0; SHA256_BYTES] {
        return Err(CompilerExecutionServiceProtocolErrorV1::PolicyMismatch);
    }
    match kind {
        CompilerExecutionServiceRequestKindV1::Inspect
        | CompilerExecutionServiceRequestKindV1::Cancel => {
            if expected_sequence != 0
                || expected_rollback_anchor != [0; SHA256_BYTES]
                || subject.is_some()
                || request.is_some()
                || publication.is_some()
                || carriage.is_some()
                || verification_challenge.is_some()
            {
                return Err(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch);
            }
        }
        CompilerExecutionServiceRequestKindV1::Prepare => {
            validate_prior_position(expected_sequence, expected_rollback_anchor)?;
            if subject.is_some()
                || request.is_some()
                || publication.is_some()
                || carriage.is_some()
                || verification_challenge.is_some()
            {
                return Err(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch);
            }
        }
        CompilerExecutionServiceRequestKindV1::Issue => {
            validate_prior_position(expected_sequence, expected_rollback_anchor)?;
            let request =
                request.ok_or(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch)?;
            if subject.is_some()
                || publication.is_some()
                || carriage.is_some()
                || verification_challenge.is_some()
                || request.challenge().policy_identity() != policy_identity
                || request.challenge().sequence() != expected_sequence
                || request.challenge().prior_rollback_anchor() != expected_rollback_anchor
            {
                return Err(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch);
            }
        }
        CompilerExecutionServiceRequestKindV1::Publish => {
            validate_prior_position(expected_sequence, expected_rollback_anchor)?;
            let request =
                request.ok_or(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch)?;
            let publication =
                publication.ok_or(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch)?;
            if subject.is_some()
                || carriage.is_some()
                || verification_challenge.is_some()
                || request.challenge().policy_identity() != policy_identity
                || publication.policy_identity() != policy_identity
                || request.challenge().sequence() != expected_sequence
                || publication.receipt().sequence() != expected_sequence
                || request.challenge().prior_rollback_anchor() != expected_rollback_anchor
                || publication.receipt().prior_rollback_anchor() != expected_rollback_anchor
                || publication.receipt().request_sha256() != request.identity().as_bytes()
                || publication.receipt().challenge_identity() != request.challenge().identity()
            {
                return Err(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch);
            }
        }
        CompilerExecutionServiceRequestKindV1::Recover => {
            if expected_sequence != 0
                || expected_rollback_anchor != [0; SHA256_BYTES]
                || subject.is_none()
                || request.is_some()
                || publication.is_some()
                || carriage.is_some()
                || verification_challenge.is_some()
            {
                return Err(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch);
            }
        }
        CompilerExecutionServiceRequestKindV1::VerifyCurrent => {
            let carriage =
                carriage.ok_or(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch)?;
            let verification_challenge = verification_challenge
                .ok_or(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch)?;
            if subject.is_some()
                || request.is_some()
                || publication.is_some()
                || verification_challenge == [0; SHA256_BYTES]
                || carriage.policy().identity() != policy_identity
                || carriage.acknowledgment().sequence() != expected_sequence
                || carriage.acknowledgment().current_rollback_anchor() != expected_rollback_anchor
            {
                return Err(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch);
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_response_fields(
    kind: CompilerExecutionServiceResponseKindV1,
    request_identity: CompilerExecutionServiceRequestIdentityV1,
    policy_identity: CompilerExecutionIssuerPolicyIdentityV1,
    sequence: u64,
    rollback_anchor: [u8; SHA256_BYTES],
    challenge: Option<&CompilerExecutionAttestationChallengeV1>,
    publication: Option<&CompilerExecutionReceiptPublicationV1>,
    carriage: Option<&CompilerExecutionReceiptCarriageV1>,
    current_record_attestation: Option<&CompilerExecutionCurrentRecordAttestationV2>,
    acknowledgment: Option<&CompilerExecutionReceiptPublicationAckV1>,
    disposition: Option<CompilerExecutionServicePublishDispositionV1>,
) -> Result<(), CompilerExecutionServiceProtocolErrorV1> {
    if request_identity.0 == [0; SHA256_BYTES] || *policy_identity.as_bytes() == [0; SHA256_BYTES] {
        return Err(CompilerExecutionServiceProtocolErrorV1::IdentityMismatch);
    }
    match kind {
        CompilerExecutionServiceResponseKindV1::Ready
        | CompilerExecutionServiceResponseKindV1::Cancelled
        | CompilerExecutionServiceResponseKindV1::ReceiptAbsent => {
            validate_prior_position(sequence, rollback_anchor)?;
            if challenge.is_some()
                || publication.is_some()
                || carriage.is_some()
                || current_record_attestation.is_some()
                || acknowledgment.is_some()
                || disposition.is_some()
            {
                return Err(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch);
            }
        }
        CompilerExecutionServiceResponseKindV1::Prepared => {
            validate_prior_position(sequence, rollback_anchor)?;
            let challenge =
                challenge.ok_or(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch)?;
            if publication.is_some()
                || carriage.is_some()
                || current_record_attestation.is_some()
                || acknowledgment.is_some()
                || disposition.is_some()
                || challenge.policy_identity() != policy_identity
                || challenge.sequence() != sequence
                || challenge.prior_rollback_anchor() != rollback_anchor
            {
                return Err(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch);
            }
        }
        CompilerExecutionServiceResponseKindV1::Issued => {
            validate_prior_position(sequence, rollback_anchor)?;
            let publication =
                publication.ok_or(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch)?;
            if challenge.is_some()
                || carriage.is_some()
                || current_record_attestation.is_some()
                || acknowledgment.is_some()
                || disposition.is_some()
                || publication.policy_identity() != policy_identity
                || publication.receipt().sequence() != sequence
                || publication.receipt().prior_rollback_anchor() != rollback_anchor
            {
                return Err(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch);
            }
        }
        CompilerExecutionServiceResponseKindV1::Published => {
            let acknowledgment =
                acknowledgment.ok_or(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch)?;
            if sequence == 0
                || rollback_anchor == [0; SHA256_BYTES]
                || challenge.is_some()
                || publication.is_some()
                || carriage.is_some()
                || current_record_attestation.is_some()
                || disposition.is_none()
                || acknowledgment.policy_identity() != policy_identity
                || acknowledgment.sequence() != sequence
                || acknowledgment.current_rollback_anchor() != rollback_anchor
            {
                return Err(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch);
            }
        }
        CompilerExecutionServiceResponseKindV1::Recovered => {
            let carriage =
                carriage.ok_or(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch)?;
            if sequence == 0
                || rollback_anchor == [0; SHA256_BYTES]
                || challenge.is_some()
                || publication.is_some()
                || current_record_attestation.is_some()
                || acknowledgment.is_some()
                || disposition.is_some()
                || carriage.policy().identity() != policy_identity
                || carriage.acknowledgment().sequence() != sequence
                || carriage.acknowledgment().current_rollback_anchor() != rollback_anchor
            {
                return Err(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch);
            }
        }
        CompilerExecutionServiceResponseKindV1::VerifiedCurrent => {
            let attestation = current_record_attestation
                .ok_or(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch)?;
            let verification = attestation.verification();
            if sequence == 0
                || rollback_anchor == [0; SHA256_BYTES]
                || challenge.is_some()
                || publication.is_some()
                || carriage.is_some()
                || acknowledgment.is_some()
                || disposition.is_some()
                || verification.policy_identity() != *policy_identity.as_bytes()
                || verification.sequence() != sequence
                || verification.current_rollback_anchor() != rollback_anchor
            {
                return Err(CompilerExecutionServiceProtocolErrorV1::PayloadMismatch);
            }
        }
    }
    Ok(())
}

fn validate_prior_position(
    sequence: u64,
    rollback_anchor: [u8; SHA256_BYTES],
) -> Result<(), CompilerExecutionServiceProtocolErrorV1> {
    if sequence == 0 || (sequence == 1) != (rollback_anchor == [0; SHA256_BYTES]) {
        return Err(CompilerExecutionServiceProtocolErrorV1::PositionMismatch);
    }
    Ok(())
}

fn encode_header(output: &mut [u8], magic: [u8; 8], kind: u16, total: usize) -> usize {
    let mut offset = 0;
    put(output, &mut offset, &magic);
    put(output, &mut offset, &VERSION_V1.to_le_bytes());
    put(output, &mut offset, &kind.to_le_bytes());
    put(output, &mut offset, &(total as u64).to_le_bytes());
    put(output, &mut offset, &0_u32.to_le_bytes());
    offset
}

fn decode_header(
    reader: &mut Reader<'_>,
    magic: [u8; 8],
    actual_len: usize,
) -> Result<u16, CompilerExecutionServiceProtocolErrorV1> {
    if reader.fixed::<8>()? != magic || reader.u16()? != VERSION_V1 {
        return Err(CompilerExecutionServiceProtocolErrorV1::Header);
    }
    let kind = reader.u16()?;
    if reader.u64()? != actual_len as u64 {
        return Err(CompilerExecutionServiceProtocolErrorV1::Header);
    }
    if reader.fixed::<4>()? != [0; 4] {
        return Err(CompilerExecutionServiceProtocolErrorV1::NonzeroReserved);
    }
    Ok(kind)
}

fn put(output: &mut [u8], offset: &mut usize, value: &[u8]) {
    let end = *offset + value.len();
    output[*offset..end].copy_from_slice(value);
    *offset = end;
}

fn derive_identity(domain: &[u8], bytes: &[u8]) -> [u8; SHA256_BYTES] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], CompilerExecutionServiceProtocolErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(CompilerExecutionServiceProtocolErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(CompilerExecutionServiceProtocolErrorV1::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], CompilerExecutionServiceProtocolErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| CompilerExecutionServiceProtocolErrorV1::Truncated)
    }

    fn u8(&mut self) -> Result<u8, CompilerExecutionServiceProtocolErrorV1> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, CompilerExecutionServiceProtocolErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, CompilerExecutionServiceProtocolErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    const fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

/// Canonical compiler-execution service packet failure.
#[derive(Debug)]
pub enum CompilerExecutionServiceProtocolErrorV1 {
    RequestLength,
    ResponseLength,
    Header,
    UnknownRequestKind(u16),
    UnknownResponseKind(u16),
    UnknownDisposition(u8),
    NonzeroReserved,
    Truncated,
    TrailingBytes,
    PolicyMismatch,
    PositionMismatch,
    PayloadMismatch,
    IdentityMismatch,
    Subject(CompilerExecutionSubjectErrorV1),
    Attestation(CompilerExecutionAttestationErrorV1),
    Publication(CompilerExecutionReceiptPublicationErrorV1),
    CurrentRecord(CompilerExecutionCurrentRecordVerificationErrorV2),
}

impl fmt::Display for CompilerExecutionServiceProtocolErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestLength => {
                formatter.write_str("compiler-execution service request length is invalid")
            }
            Self::ResponseLength => {
                formatter.write_str("compiler-execution service response length is invalid")
            }
            Self::Header => formatter.write_str("compiler-execution service header is invalid"),
            Self::UnknownRequestKind(kind) => write!(
                formatter,
                "unknown compiler-execution service request kind {kind}"
            ),
            Self::UnknownResponseKind(kind) => write!(
                formatter,
                "unknown compiler-execution service response kind {kind}"
            ),
            Self::UnknownDisposition(value) => write!(
                formatter,
                "unknown compiler-execution publish disposition {value}"
            ),
            Self::NonzeroReserved => {
                formatter.write_str("compiler-execution service reserved bytes are nonzero")
            }
            Self::Truncated => {
                formatter.write_str("compiler-execution service packet is truncated")
            }
            Self::TrailingBytes => {
                formatter.write_str("compiler-execution service packet has trailing bytes")
            }
            Self::PolicyMismatch => {
                formatter.write_str("compiler-execution service policy mismatch")
            }
            Self::PositionMismatch => {
                formatter.write_str("compiler-execution service rollback position is invalid")
            }
            Self::PayloadMismatch => {
                formatter.write_str("compiler-execution service payload disagrees with its packet")
            }
            Self::IdentityMismatch => {
                formatter.write_str("compiler-execution service packet identity mismatch")
            }
            Self::Subject(error) => {
                write!(
                    formatter,
                    "compiler-execution service subject failed: {error}"
                )
            }
            Self::Attestation(error) => write!(
                formatter,
                "compiler-execution attestation packet failed: {error}"
            ),
            Self::Publication(error) => write!(
                formatter,
                "compiler-execution publication packet failed: {error}"
            ),
            Self::CurrentRecord(error) => write!(
                formatter,
                "compiler-execution current-record verification failed: {error}"
            ),
        }
    }
}

impl Error for CompilerExecutionServiceProtocolErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Subject(error) => Some(error),
            Self::Attestation(error) => Some(error),
            Self::Publication(error) => Some(error),
            Self::CurrentRecord(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CompilerExecutionSubjectErrorV1> for CompilerExecutionServiceProtocolErrorV1 {
    fn from(error: CompilerExecutionSubjectErrorV1) -> Self {
        Self::Subject(error)
    }
}

impl From<CompilerExecutionAttestationErrorV1> for CompilerExecutionServiceProtocolErrorV1 {
    fn from(error: CompilerExecutionAttestationErrorV1) -> Self {
        Self::Attestation(error)
    }
}

impl From<CompilerExecutionReceiptPublicationErrorV1> for CompilerExecutionServiceProtocolErrorV1 {
    fn from(error: CompilerExecutionReceiptPublicationErrorV1) -> Self {
        Self::Publication(error)
    }
}

impl From<CompilerExecutionCurrentRecordVerificationErrorV2>
    for CompilerExecutionServiceProtocolErrorV1
{
    fn from(error: CompilerExecutionCurrentRecordVerificationErrorV2) -> Self {
        Self::CurrentRecord(error)
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};
    use fe2o3_artifact_transaction::{
        INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1, INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
        INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1, InertCompilerExecutionSubjectV1,
    };
    use fe2o3_external_anchor_protocol::{
        AnchorPositionV1, AnchorTransitionReceiptV1, AnchoredStateV1, CallerNonceV1,
        HashChainHeadV1, PinnedAnchorKeyV1, UnsignedAnchorObservationV1,
    };

    use crate::{
        CompilerExecutionAttestationReceiptV1, CompilerExecutionExternalAnchorTransactionV1,
        CompilerExecutionIssuerMeasurementV1,
    };

    use super::*;

    const SUBJECT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V1\0";
    const COMPILER_CLOSURE_IDENTITY_DOMAIN: &[u8] = b"fe2o3-compiler-closure-identity-v2\0";

    struct Fixture {
        signing_key: SigningKey,
        anchor_signing_key: SigningKey,
        policy: CompilerExecutionIssuerPolicyV1,
        request: CompilerExecutionAttestationRequestV1,
        publication: CompilerExecutionReceiptPublicationV1,
        acknowledgment: CompilerExecutionReceiptPublicationAckV1,
    }

    impl Fixture {
        fn new() -> Self {
            let key = SigningKey::from_bytes(&[0x41; 32]);
            let anchor_signing_key = SigningKey::from_bytes(&[0x42; 32]);
            let policy = CompilerExecutionIssuerPolicyV1::new(
                9,
                CompilerExecutionIssuerMeasurementV1::new([0x31; 32], 123).unwrap(),
                CompilerExecutionIssuerMeasurementV1::new([0x32; 32], 456).unwrap(),
                key.verifying_key().to_bytes(),
                anchor_signing_key.verifying_key().to_bytes(),
            )
            .unwrap();
            let challenge = CompilerExecutionAttestationChallengeV1::new(
                &policy,
                &subject(0x20),
                [0x33; 32],
                1,
                [0; 32],
            )
            .unwrap();
            let request =
                CompilerExecutionAttestationRequestV1::new(challenge, subject(0x20)).unwrap();
            let receipt =
                CompilerExecutionAttestationReceiptV1::issue(&policy, &request, &key).unwrap();
            let publication =
                CompilerExecutionReceiptPublicationV1::new([0x34; 32], [0x35; 32], receipt)
                    .unwrap();
            let acknowledgment =
                CompilerExecutionReceiptPublicationAckV1::new(&publication, [0x36; 32]).unwrap();
            Self {
                signing_key: key,
                anchor_signing_key,
                policy,
                request,
                publication,
                acknowledgment,
            }
        }

        fn anchor_receipt(
            &self,
            carriage: &CompilerExecutionReceiptCarriageV1,
        ) -> AnchorTransitionReceiptV1 {
            let transaction = CompilerExecutionExternalAnchorTransactionV1::new(
                carriage.policy().clone(),
                carriage.request().clone(),
                carriage.publication().clone(),
            )
            .unwrap();
            let key =
                PinnedAnchorKeyV1::from_bytes(self.anchor_signing_key.verifying_key().to_bytes())
                    .unwrap();
            let pending =
                AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32]))
                    .prepare(transaction.external_anchor_digest(), &key)
                    .unwrap()
                    .begin_advance(CallerNonceV1::from_bytes([0x73; 32]), &key)
                    .unwrap();
            let unsigned = UnsignedAnchorObservationV1::from_challenge(
                pending.challenge(),
                AnchorPositionV1::Proposed,
            );
            let signature = self.anchor_signing_key.sign(&unsigned.signing_bytes());
            AnchorTransitionReceiptV1::new(
                pending.challenge().clone(),
                &unsigned.attach_signature(signature.to_bytes()),
                &key,
            )
            .unwrap()
        }
    }

    #[test]
    fn exact_packet_sizes_and_all_variants_round_trip() {
        assert_eq!(COMPILER_EXECUTION_SERVICE_CONTROL_REQUEST_BYTES_V1, 128);
        assert_eq!(COMPILER_EXECUTION_SERVICE_PREPARE_REQUEST_BYTES_V1, 128);
        assert_eq!(COMPILER_EXECUTION_SERVICE_ISSUE_REQUEST_BYTES_V1, 1074);
        assert_eq!(COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_BYTES_V1, 1658);
        assert_eq!(COMPILER_EXECUTION_SERVICE_RECOVER_REQUEST_BYTES_V1, 818);
        assert_eq!(
            COMPILER_EXECUTION_SERVICE_VERIFY_CURRENT_REQUEST_BYTES_V1,
            2250
        );
        assert_eq!(COMPILER_EXECUTION_SERVICE_CONTROL_RESPONSE_BYTES_V1, 160);
        assert_eq!(COMPILER_EXECUTION_SERVICE_PREPARED_RESPONSE_BYTES_V1, 360);
        assert_eq!(COMPILER_EXECUTION_SERVICE_ISSUED_RESPONSE_BYTES_V1, 744);
        assert_eq!(COMPILER_EXECUTION_SERVICE_PUBLISHED_RESPONSE_BYTES_V1, 456);
        assert_eq!(COMPILER_EXECUTION_SERVICE_RECOVERED_RESPONSE_BYTES_V1, 2250);
        assert_eq!(
            COMPILER_EXECUTION_SERVICE_VERIFIED_CURRENT_RESPONSE_BYTES_V1,
            1256
        );
        assert_eq!(
            crate::COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V2,
            912
        );

        let fixture = Fixture::new();
        let inspect = CompilerExecutionServiceRequestV1::inspect(&fixture.policy);
        let prepare =
            CompilerExecutionServiceRequestV1::prepare(&fixture.policy, 1, [0; 32]).unwrap();
        let issue =
            CompilerExecutionServiceRequestV1::issue(&fixture.policy, fixture.request.clone())
                .unwrap();
        let publish = CompilerExecutionServiceRequestV1::publish(
            &fixture.policy,
            fixture.request.clone(),
            fixture.publication.clone(),
        )
        .unwrap();
        let cancel = CompilerExecutionServiceRequestV1::cancel(&fixture.policy);
        let recover = CompilerExecutionServiceRequestV1::recover(
            &fixture.policy,
            fixture.request.subject().clone(),
        )
        .unwrap();
        let carriage = CompilerExecutionReceiptCarriageV1::new(
            fixture.policy.clone(),
            fixture.request.clone(),
            fixture.publication.clone(),
            fixture.acknowledgment.clone(),
        )
        .unwrap();
        let verify_current = CompilerExecutionServiceRequestV1::verify_current(
            &fixture.policy,
            carriage.clone(),
            [0x61; 32],
        )
        .unwrap();
        for request in [
            &inspect,
            &prepare,
            &issue,
            &publish,
            &cancel,
            &recover,
            &verify_current,
        ] {
            let decoded =
                CompilerExecutionServiceRequestV1::decode(request.canonical_bytes()).unwrap();
            assert_eq!(&decoded, request);
            assert!(!decoded.grants_authority());
        }

        let ready = CompilerExecutionServiceResponseV1::ready(
            inspect.identity(),
            &fixture.policy,
            1,
            [0; 32],
        )
        .unwrap();
        let prepared = CompilerExecutionServiceResponseV1::prepared(
            prepare.identity(),
            &fixture.policy,
            fixture.request.challenge().clone(),
        )
        .unwrap();
        let issued = CompilerExecutionServiceResponseV1::issued(
            issue.identity(),
            &fixture.policy,
            fixture.publication.clone(),
        )
        .unwrap();
        let published = CompilerExecutionServiceResponseV1::published(
            publish.identity(),
            &fixture.policy,
            fixture.acknowledgment.clone(),
            CompilerExecutionServicePublishDispositionV1::Advanced,
        )
        .unwrap();
        let cancelled = CompilerExecutionServiceResponseV1::cancelled(
            cancel.identity(),
            &fixture.policy,
            1,
            [0; 32],
        )
        .unwrap();
        let recovered =
            CompilerExecutionServiceResponseV1::recovered(recover.identity(), carriage.clone())
                .unwrap();
        let verification = CompilerExecutionCurrentRecordVerificationV2::new(
            &carriage,
            fixture.anchor_receipt(&carriage),
            [0x71; 32],
            [0x72; 32],
        )
        .unwrap();
        assert_every_mutation_rejects(verification.canonical_bytes(), |bytes| {
            CompilerExecutionCurrentRecordVerificationV2::decode(bytes).is_err()
        });
        let verified_current = CompilerExecutionServiceResponseV1::verified_current(
            verify_current.identity(),
            CompilerExecutionCurrentRecordAttestationV2::issue(
                &fixture.policy,
                &carriage,
                verification,
                verify_current.verification_challenge().unwrap(),
                &fixture.signing_key,
            )
            .unwrap(),
        )
        .unwrap();
        let absent = CompilerExecutionServiceResponseV1::receipt_absent(
            recover.identity(),
            &fixture.policy,
            1,
            [0; 32],
        )
        .unwrap();
        for response in [
            &ready,
            &prepared,
            &issued,
            &published,
            &cancelled,
            &recovered,
            &absent,
            &verified_current,
        ] {
            let decoded =
                CompilerExecutionServiceResponseV1::decode(response.canonical_bytes()).unwrap();
            assert_eq!(&decoded, response);
            assert!(!decoded.grants_authority());
        }
    }

    #[test]
    fn every_request_and_response_byte_mutation_rejects() {
        let fixture = Fixture::new();
        let carriage = CompilerExecutionReceiptCarriageV1::new(
            fixture.policy.clone(),
            fixture.request.clone(),
            fixture.publication.clone(),
            fixture.acknowledgment.clone(),
        )
        .unwrap();
        let requests = [
            CompilerExecutionServiceRequestV1::inspect(&fixture.policy),
            CompilerExecutionServiceRequestV1::prepare(&fixture.policy, 1, [0; 32]).unwrap(),
            CompilerExecutionServiceRequestV1::issue(&fixture.policy, fixture.request.clone())
                .unwrap(),
            CompilerExecutionServiceRequestV1::publish(
                &fixture.policy,
                fixture.request.clone(),
                fixture.publication.clone(),
            )
            .unwrap(),
            CompilerExecutionServiceRequestV1::verify_current(
                &fixture.policy,
                carriage.clone(),
                [0x61; 32],
            )
            .unwrap(),
            CompilerExecutionServiceRequestV1::cancel(&fixture.policy),
            CompilerExecutionServiceRequestV1::recover(
                &fixture.policy,
                fixture.request.subject().clone(),
            )
            .unwrap(),
        ];
        for request in &requests {
            assert_every_mutation_rejects(request.canonical_bytes(), |bytes| {
                CompilerExecutionServiceRequestV1::decode(bytes).is_err()
            });
        }

        let responses = [
            CompilerExecutionServiceResponseV1::ready(
                requests[0].identity(),
                &fixture.policy,
                1,
                [0; 32],
            )
            .unwrap(),
            CompilerExecutionServiceResponseV1::prepared(
                requests[1].identity(),
                &fixture.policy,
                fixture.request.challenge().clone(),
            )
            .unwrap(),
            CompilerExecutionServiceResponseV1::issued(
                requests[2].identity(),
                &fixture.policy,
                fixture.publication.clone(),
            )
            .unwrap(),
            CompilerExecutionServiceResponseV1::published(
                requests[3].identity(),
                &fixture.policy,
                fixture.acknowledgment.clone(),
                CompilerExecutionServicePublishDispositionV1::AlreadyAcknowledged,
            )
            .unwrap(),
            CompilerExecutionServiceResponseV1::cancelled(
                requests[5].identity(),
                &fixture.policy,
                1,
                [0; 32],
            )
            .unwrap(),
            CompilerExecutionServiceResponseV1::recovered(
                requests[6].identity(),
                CompilerExecutionReceiptCarriageV1::new(
                    fixture.policy.clone(),
                    fixture.request.clone(),
                    fixture.publication.clone(),
                    fixture.acknowledgment.clone(),
                )
                .unwrap(),
            )
            .unwrap(),
            CompilerExecutionServiceResponseV1::receipt_absent(
                requests[6].identity(),
                &fixture.policy,
                1,
                [0; 32],
            )
            .unwrap(),
            CompilerExecutionServiceResponseV1::verified_current(
                requests[4].identity(),
                CompilerExecutionCurrentRecordAttestationV2::issue(
                    &fixture.policy,
                    &carriage,
                    CompilerExecutionCurrentRecordVerificationV2::new(
                        &carriage,
                        fixture.anchor_receipt(&carriage),
                        [0x71; 32],
                        [0x72; 32],
                    )
                    .unwrap(),
                    requests[4].verification_challenge().unwrap(),
                    &fixture.signing_key,
                )
                .unwrap(),
            )
            .unwrap(),
        ];
        for response in &responses {
            assert_every_mutation_rejects(response.canonical_bytes(), |bytes| {
                CompilerExecutionServiceResponseV1::decode(bytes).is_err()
            });
        }
    }

    #[test]
    fn wrong_lengths_positions_policy_and_nested_pairs_reject() {
        let fixture = Fixture::new();
        let request = CompilerExecutionServiceRequestV1::publish(
            &fixture.policy,
            fixture.request.clone(),
            fixture.publication.clone(),
        )
        .unwrap();
        assert!(
            CompilerExecutionServiceRequestV1::decode(
                &request.canonical_bytes()[..request.canonical_bytes().len() - 1]
            )
            .is_err()
        );
        let mut extended = request.canonical_bytes().to_vec();
        extended.push(0);
        assert!(CompilerExecutionServiceRequestV1::decode(&extended).is_err());
        assert!(CompilerExecutionServiceRequestV1::prepare(&fixture.policy, 0, [0; 32]).is_err());
        assert!(CompilerExecutionServiceRequestV1::prepare(&fixture.policy, 2, [0; 32]).is_err());
        let recover = CompilerExecutionServiceRequestV1::recover(
            &fixture.policy,
            fixture.request.subject().clone(),
        )
        .unwrap();
        assert_eq!(recover.subject(), Some(fixture.request.subject()));
        assert_eq!(recover.expected_sequence(), 0);
        assert_eq!(recover.expected_rollback_anchor(), [0; 32]);

        let wrong_key = SigningKey::from_bytes(&[0x42; 32]);
        let wrong_policy = CompilerExecutionIssuerPolicyV1::new(
            10,
            CompilerExecutionIssuerMeasurementV1::new([0x31; 32], 123).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x32; 32], 456).unwrap(),
            wrong_key.verifying_key().to_bytes(),
            SigningKey::from_bytes(&[0x43; 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap();
        assert!(
            CompilerExecutionServiceRequestV1::issue(&wrong_policy, fixture.request.clone())
                .is_err()
        );

        let other_challenge = CompilerExecutionAttestationChallengeV1::new(
            &fixture.policy,
            &subject(0x21),
            [0x43; 32],
            1,
            [0; 32],
        )
        .unwrap();
        let other_request =
            CompilerExecutionAttestationRequestV1::new(other_challenge, subject(0x21)).unwrap();
        assert!(
            CompilerExecutionServiceRequestV1::publish(
                &fixture.policy,
                other_request,
                fixture.publication.clone()
            )
            .is_err()
        );
        assert!(
            CompilerExecutionServiceResponseV1::published(
                request.identity(),
                &wrong_policy,
                fixture.acknowledgment,
                CompilerExecutionServicePublishDispositionV1::Advanced,
            )
            .is_err()
        );
    }

    fn assert_every_mutation_rejects(bytes: &[u8], rejects: impl Fn(&[u8]) -> bool) {
        for index in 0..bytes.len() {
            let mut mutated = bytes.to_vec();
            mutated[index] ^= 0x80;
            assert!(rejects(&mutated), "mutation at byte {index} was accepted");
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
        let identity = subject_digest(&bytes[..offset]);
        put(&mut bytes, &mut offset, &identity);
        assert_eq!(offset, bytes.len());
        InertCompilerExecutionSubjectV1::decode(&bytes).unwrap()
    }

    fn subject_digest(bytes: &[u8]) -> [u8; SHA256_BYTES] {
        let mut digest = Sha256::new();
        digest.update(SUBJECT_IDENTITY_DOMAIN);
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
        digest.finalize().into()
    }
}
