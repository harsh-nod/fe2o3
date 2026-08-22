//! Cross-process reacquisition of one exact durable publication.
//!
//! A claim is inert, cloneable coordination data. It binds the complete publication plan and
//! backend receipt to the output directory, canonical record, and content-addressed artifact
//! identities observed when publication completed. It contains no descriptor, lock, lease, load
//! authority, or launch authority.
//!
//! Reacquisition is deliberately local. It validates the current filesystem and attempt registry
//! under the cooperative output lock, but it does not detect rollback of that complete local state.

use super::attempt::{
    AttemptPhase, BackendReceiptV1, COMPILER_CLOSURE_BYTES_V2, decode_compiler_closure_v2,
    push_compiler_closure_v2,
};
use super::attempt_scoped_hsaco_publication::{
    producer_receipt_identity_v1, producer_receipt_identity_v2, producer_receipt_identity_v3,
    publication_receipt_for_producer_identity, publication_receipt_for_producer_identity_v2,
    publication_receipt_for_producer_identity_v3,
};
use super::durable_link_publication::{
    DurableCurrentLinkPublicationLeaseV1, DurableFileIdentityV1, DurableLinkPublicationError,
    DurableLinkPublicationPlanV1, DurablePublishedFileBindingV1,
    MAX_DURABLE_FINALIZED_ARTIFACT_BYTES, reacquire_current_publication_lease_locked,
    recover_durable_published_file_binding_locked,
};
use super::{
    AtomicPublicationIdentityV1, BackendPublicationReceiptV1, BackendPublicationReceiptV2,
    BackendPublicationReceiptV3, BuildAttempt, BuildInvocation, BuildSession,
    CanonicalLinkRequestIdentityV1, EmitError, FinalizationIdentityV1, FinalizedOutputIdentityV1,
    KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1, PackageIdentityV1,
    PinnedOutput, PinnedWorkerIdentityV1, TargetIdentityV1, UpstreamCodeObjectEvidenceIdentityV1,
    ValidatedResponseIdentityV1, WorkerV3PublicationBindingErrorV1, WorkerV3PublicationBindingV1,
    read_attempt_registry,
};
use fe2o3_build_authority::{CompilerClosureErrorV2, CompilerClosureV2};
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::Path;

const CLAIM_MAGIC: &[u8] = b"FE2O3-PUBLISHED-HSACO-CLAIM-V1\0";
const CLAIM_VERSION: u16 = 1;
const CLAIM_CHECKSUM_DOMAIN: &[u8] = b"fe2o3.published-hsaco-claim.checksum.v1\0";
const CLAIM_FIXED_BODY_BYTES: usize =
    CLAIM_MAGIC.len() + 2 + 8 + 16 + 32 + 3 * 32 + 7 * 32 + 32 + 7 * 32 + 7 * 8;
const CLAIM_CANONICAL_BYTES: usize = CLAIM_FIXED_BODY_BYTES + 32;
const CLAIM_MAGIC_V2: &[u8] = b"FE2O3-PUBLISHED-HSACO-CLAIM-V2\0";
const CLAIM_VERSION_V2: u16 = 2;
const CLAIM_CHECKSUM_DOMAIN_V2: &[u8] = b"fe2o3.published-hsaco-claim.checksum.v2\0";
const CLAIM_FIXED_BODY_BYTES_V2: usize = CLAIM_FIXED_BODY_BYTES + COMPILER_CLOSURE_BYTES_V2;
const CLAIM_CANONICAL_BYTES_V2: usize = CLAIM_FIXED_BODY_BYTES_V2 + 32;
const CLAIM_MAGIC_V3: &[u8] = b"FE2O3-PUBLISHED-HSACO-CLAIM-V3\0";
const CLAIM_VERSION_V3: u16 = 3;
const CLAIM_CHECKSUM_DOMAIN_V3: &[u8] = b"fe2o3.published-hsaco-claim.checksum.v3\0";
const WORKER_V3_BINDING_PREIMAGE_BYTES_V1: usize = COMPILER_CLOSURE_BYTES_V2 + (7 * 32) + (2 * 8);
const CLAIM_FIXED_BODY_BYTES_V3: usize =
    CLAIM_FIXED_BODY_BYTES + WORKER_V3_BINDING_PREIMAGE_BYTES_V1;
const CLAIM_CANONICAL_BYTES_V3: usize = CLAIM_FIXED_BODY_BYTES_V3 + 32;

/// Maximum accepted wire size for one durable published-HSACO claim.
pub const MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES: usize = 1_024;

/// Maximum accepted wire size for one durable protected published-HSACO claim.
pub const MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V2: usize = 1_024;

/// Maximum accepted wire size for one strict Worker V3 published-HSACO claim.
pub const MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V3: usize = 1_280;

/// Receipt field that disagrees with the complete plan or upstream identity in a claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DurablePublishedClaimReceiptFieldV1 {
    Attempt,
    Scope,
    Plan,
    UpstreamEvidence,
    FinalizedOutput,
    Publication,
}

/// Protected receipt field that disagrees with complete V2 claim evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DurablePublishedClaimReceiptFieldV2 {
    Attempt,
    Scope,
    Plan,
    UpstreamEvidence,
    FinalizedOutput,
    Publication,
}

/// Strict Worker V3 receipt field that disagrees with complete claim evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DurablePublishedClaimReceiptFieldV3 {
    Attempt,
    Scope,
    Plan,
    UpstreamEvidence,
    FinalizedOutput,
    Publication,
}

/// Worker V3 binding axis that disagrees with the durable publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DurablePublishedClaimWorkerV3BindingFieldV1 {
    RawOutput,
    FinalizedOutput,
    ArtifactLength,
}

/// Bounded canonical claim codec failure.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DurablePublishedClaimCodecErrorV1 {
    TooLarge {
        actual: usize,
        maximum: usize,
    },
    Truncated,
    TrailingBytes,
    BadMagic,
    UnsupportedVersion {
        actual: u16,
    },
    ChecksumMismatch,
    InvalidAttempt,
    InvalidArtifactLength {
        actual: u64,
    },
    ReceiptMismatch {
        field: DurablePublishedClaimReceiptFieldV1,
    },
    NonCanonical,
}

/// Bounded canonical protected-claim codec failure.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DurablePublishedClaimCodecErrorV2 {
    TooLarge {
        actual: usize,
        maximum: usize,
    },
    Truncated,
    TrailingBytes,
    BadMagic,
    UnsupportedVersion {
        actual: u16,
    },
    ChecksumMismatch,
    InvalidAttempt,
    InvalidArtifactLength {
        actual: u64,
    },
    InvalidCompilerClosure(CompilerClosureErrorV2),
    ReceiptMismatch {
        field: DurablePublishedClaimReceiptFieldV2,
    },
    NonCanonical,
}

/// Bounded canonical strict Worker V3 claim codec failure.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DurablePublishedClaimCodecErrorV3 {
    TooLarge {
        actual: usize,
        maximum: usize,
    },
    Truncated,
    TrailingBytes,
    BadMagic,
    UnsupportedVersion {
        actual: u16,
    },
    ChecksumMismatch,
    InvalidAttempt,
    InvalidArtifactLength {
        actual: u64,
    },
    InvalidWorkerV3Binding(WorkerV3PublicationBindingErrorV1),
    ReceiptMismatch {
        field: DurablePublishedClaimReceiptFieldV3,
    },
    WorkerV3BindingMismatch {
        field: DurablePublishedClaimWorkerV3BindingFieldV1,
    },
    AllocationFailed {
        requested: usize,
    },
    NonCanonical,
}

impl fmt::Display for DurablePublishedClaimCodecErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { actual, maximum } => {
                write!(formatter, "published claim size {actual} exceeds {maximum}")
            }
            Self::Truncated => formatter.write_str("truncated published claim"),
            Self::TrailingBytes => formatter.write_str("trailing published claim bytes"),
            Self::BadMagic => formatter.write_str("bad published claim magic"),
            Self::UnsupportedVersion { actual } => {
                write!(formatter, "unsupported published claim version {actual}")
            }
            Self::ChecksumMismatch => formatter.write_str("published claim checksum mismatch"),
            Self::InvalidAttempt => formatter.write_str("invalid published claim build attempt"),
            Self::InvalidArtifactLength { actual } => {
                write!(
                    formatter,
                    "invalid published claim artifact length {actual}"
                )
            }
            Self::ReceiptMismatch { field } => {
                write!(
                    formatter,
                    "published claim receipt {field:?} does not match"
                )
            }
            Self::NonCanonical => formatter.write_str("noncanonical published claim"),
        }
    }
}

impl std::error::Error for DurablePublishedClaimCodecErrorV1 {}

impl fmt::Display for DurablePublishedClaimCodecErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { actual, maximum } => {
                write!(
                    formatter,
                    "protected published claim size {actual} exceeds {maximum}"
                )
            }
            Self::Truncated => formatter.write_str("truncated protected published claim"),
            Self::TrailingBytes => formatter.write_str("trailing protected published claim bytes"),
            Self::BadMagic => formatter.write_str("bad protected published claim magic"),
            Self::UnsupportedVersion { actual } => {
                write!(
                    formatter,
                    "unsupported protected published claim version {actual}"
                )
            }
            Self::ChecksumMismatch => {
                formatter.write_str("protected published claim checksum mismatch")
            }
            Self::InvalidAttempt => {
                formatter.write_str("invalid protected published claim build attempt")
            }
            Self::InvalidArtifactLength { actual } => write!(
                formatter,
                "invalid protected published claim artifact length {actual}"
            ),
            Self::InvalidCompilerClosure(error) => {
                write!(formatter, "invalid protected compiler closure: {error}")
            }
            Self::ReceiptMismatch { field } => write!(
                formatter,
                "protected published claim receipt {field:?} does not match"
            ),
            Self::NonCanonical => formatter.write_str("noncanonical protected published claim"),
        }
    }
}

impl std::error::Error for DurablePublishedClaimCodecErrorV2 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidCompilerClosure(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for DurablePublishedClaimCodecErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { actual, maximum } => write!(
                formatter,
                "strict Worker V3 published claim size {actual} exceeds {maximum}"
            ),
            Self::Truncated => formatter.write_str("truncated strict Worker V3 published claim"),
            Self::TrailingBytes => {
                formatter.write_str("trailing strict Worker V3 published claim bytes")
            }
            Self::BadMagic => formatter.write_str("bad strict Worker V3 published claim magic"),
            Self::UnsupportedVersion { actual } => write!(
                formatter,
                "unsupported strict Worker V3 published claim version {actual}"
            ),
            Self::ChecksumMismatch => {
                formatter.write_str("strict Worker V3 published claim checksum mismatch")
            }
            Self::InvalidAttempt => {
                formatter.write_str("invalid strict Worker V3 published claim build attempt")
            }
            Self::InvalidArtifactLength { actual } => write!(
                formatter,
                "invalid strict Worker V3 published claim artifact length {actual}"
            ),
            Self::InvalidWorkerV3Binding(error) => {
                write!(
                    formatter,
                    "invalid strict Worker V3 publication binding: {error}"
                )
            }
            Self::ReceiptMismatch { field } => write!(
                formatter,
                "strict Worker V3 published claim receipt {field:?} does not match"
            ),
            Self::WorkerV3BindingMismatch { field } => write!(
                formatter,
                "strict Worker V3 published claim binding {field:?} does not match"
            ),
            Self::AllocationFailed { requested } => write!(
                formatter,
                "strict Worker V3 published claim allocation of {requested} bytes failed"
            ),
            Self::NonCanonical => {
                formatter.write_str("noncanonical strict Worker V3 published claim")
            }
        }
    }
}

impl std::error::Error for DurablePublishedClaimCodecErrorV3 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidWorkerV3Binding(error) => Some(error),
            _ => None,
        }
    }
}

/// Why an inert claim could not be reacquired as a fresh current-publication lease.
#[derive(Debug)]
#[non_exhaustive]
pub enum DurablePublishedClaimReacquisitionErrorV1 {
    Busy,
    InvalidClaim(DurablePublishedClaimCodecErrorV1),
    Filesystem(EmitError),
    AttemptNotFound,
    AttemptState,
    ReceiptMismatch,
    ProducerIdentityMismatch,
    Publication(DurableLinkPublicationError),
}

/// Why an inert protected claim could not be reacquired as a fresh current-publication lease.
#[derive(Debug)]
#[non_exhaustive]
pub enum DurablePublishedClaimReacquisitionErrorV2 {
    Busy,
    InvalidClaim(DurablePublishedClaimCodecErrorV2),
    Filesystem(EmitError),
    AttemptNotFound,
    AttemptState,
    ReceiptMismatch,
    ProducerIdentityMismatch,
    Publication(DurableLinkPublicationError),
}

/// Why an inert strict Worker V3 claim could not be reacquired as a fresh current lease.
#[derive(Debug)]
#[non_exhaustive]
pub enum DurablePublishedClaimReacquisitionErrorV3 {
    Busy,
    InvalidClaim(DurablePublishedClaimCodecErrorV3),
    Filesystem(EmitError),
    AttemptNotFound,
    AttemptState,
    ReceiptMismatch,
    ProducerIdentityMismatch,
    Publication(DurableLinkPublicationError),
}

impl fmt::Display for DurablePublishedClaimReacquisitionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => formatter.write_str("published-claim output lock is busy"),
            Self::InvalidClaim(error) => write!(formatter, "invalid published claim: {error}"),
            Self::Filesystem(error) => {
                write!(formatter, "published-claim filesystem failure: {error}")
            }
            Self::AttemptNotFound => {
                formatter.write_str("published claim build attempt is not durably present")
            }
            Self::AttemptState => formatter
                .write_str("published claim build attempt is not backend-claimed or completed"),
            Self::ReceiptMismatch => formatter
                .write_str("published claim receipt does not match the durable attempt registry"),
            Self::ProducerIdentityMismatch => formatter.write_str(
                "published claim producer identity does not match the durable attempt owner",
            ),
            Self::Publication(error) => {
                write!(formatter, "published claim is not current: {error}")
            }
        }
    }
}

impl std::error::Error for DurablePublishedClaimReacquisitionErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidClaim(error) => Some(error),
            Self::Filesystem(error) => Some(error),
            Self::Publication(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for DurablePublishedClaimReacquisitionErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => formatter.write_str("protected published-claim output lock is busy"),
            Self::InvalidClaim(error) => {
                write!(formatter, "invalid protected published claim: {error}")
            }
            Self::Filesystem(error) => write!(
                formatter,
                "protected published-claim filesystem failure: {error}"
            ),
            Self::AttemptNotFound => formatter
                .write_str("protected published claim build attempt is not durably present"),
            Self::AttemptState => formatter
                .write_str("protected published claim attempt is not backend-claimed or completed"),
            Self::ReceiptMismatch => formatter.write_str(
                "protected published claim receipt does not match durable V2 attempt state",
            ),
            Self::ProducerIdentityMismatch => formatter.write_str(
                "protected published claim producer does not match the durable attempt owner",
            ),
            Self::Publication(error) => {
                write!(
                    formatter,
                    "protected published claim is not current: {error}"
                )
            }
        }
    }
}

impl std::error::Error for DurablePublishedClaimReacquisitionErrorV2 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidClaim(error) => Some(error),
            Self::Filesystem(error) => Some(error),
            Self::Publication(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for DurablePublishedClaimReacquisitionErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => {
                formatter.write_str("strict Worker V3 published-claim output lock is busy")
            }
            Self::InvalidClaim(error) => {
                write!(formatter, "invalid strict Worker V3 published claim: {error}")
            }
            Self::Filesystem(error) => write!(
                formatter,
                "strict Worker V3 published-claim filesystem failure: {error}"
            ),
            Self::AttemptNotFound => formatter
                .write_str("strict Worker V3 published claim attempt is not durably present"),
            Self::AttemptState => formatter.write_str(
                "strict Worker V3 published claim attempt is not backend-claimed or completed",
            ),
            Self::ReceiptMismatch => formatter.write_str(
                "strict Worker V3 published claim receipt does not match durable V3 attempt state",
            ),
            Self::ProducerIdentityMismatch => formatter.write_str(
                "strict Worker V3 published claim producer does not match the durable attempt owner",
            ),
            Self::Publication(error) => write!(
                formatter,
                "strict Worker V3 published claim is not current: {error}"
            ),
        }
    }
}

impl std::error::Error for DurablePublishedClaimReacquisitionErrorV3 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidClaim(error) => Some(error),
            Self::Filesystem(error) => Some(error),
            Self::Publication(error) => Some(error),
            _ => None,
        }
    }
}

/// Canonical inert claim for one exact attempt-scoped HSACO publication.
///
/// The private file bindings prevent callers from forging a claim from free-standing identities.
/// A claim is intentionally cloneable and serializable because it grants no authority. Only
/// [`reacquire_current_hsaco_publication_lease_v1`] can exchange it for a fresh non-`Clone` lease,
/// after revalidating all durable state under the publication lock.
///
/// ```compile_fail
/// use fe2o3_artifact_transaction::DurablePublishedHsacoClaimV1;
///
/// fn cannot_extract_private_binding(claim: DurablePublishedHsacoClaimV1) {
///     let DurablePublishedHsacoClaimV1 { files, .. } = claim;
///     let _ = files;
/// }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurablePublishedHsacoClaimV1 {
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    receipt: BackendPublicationReceiptV1,
    files: DurablePublishedFileBindingV1,
}

impl DurablePublishedHsacoClaimV1 {
    pub(crate) fn new(
        plan: DurableLinkPublicationPlanV1,
        upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
        receipt: BackendPublicationReceiptV1,
        files: DurablePublishedFileBindingV1,
    ) -> Self {
        let claim = Self {
            plan,
            upstream_evidence,
            receipt,
            files,
        };
        debug_assert!(claim.validate().is_ok());
        claim
    }

    /// Returns the complete typed publication plan carried as inert data.
    pub const fn plan(&self) -> DurableLinkPublicationPlanV1 {
        self.plan
    }

    /// Returns the caller-supplied upstream evidence identity bound by the receipt.
    pub const fn upstream_evidence(&self) -> UpstreamCodeObjectEvidenceIdentityV1 {
        self.upstream_evidence
    }

    /// Returns the exact provenance receipt persisted for the build attempt.
    pub const fn receipt(&self) -> BackendPublicationReceiptV1 {
        self.receipt
    }

    /// Encodes this claim as one checksummed, fixed-schema canonical record.
    pub fn encode_canonical(&self) -> Result<Vec<u8>, DurablePublishedClaimCodecErrorV1> {
        self.validate()?;
        let mut bytes = Vec::with_capacity(CLAIM_CANONICAL_BYTES);
        bytes.extend_from_slice(CLAIM_MAGIC);
        bytes.extend_from_slice(&CLAIM_VERSION.to_le_bytes());
        push_attempt(&mut bytes, self.plan.attempt());
        push_scope(&mut bytes, self.plan.scope());
        for identity in [
            *self.plan.request().as_bytes(),
            *self.plan.worker().as_bytes(),
            *self.plan.response().as_bytes(),
            *self.plan.linked_output().as_bytes(),
            *self.plan.finalization().as_bytes(),
            *self.plan.finalized_output().as_bytes(),
            *self.plan.publication().as_bytes(),
            self.upstream_evidence.as_bytes(),
        ] {
            bytes.extend_from_slice(&identity);
        }
        push_receipt(&mut bytes, self.receipt);
        push_file_identity(&mut bytes, self.files.output_identity);
        push_file_identity(&mut bytes, self.files.record_identity);
        push_file_identity(&mut bytes, self.files.artifact_identity);
        bytes.extend_from_slice(&(self.files.artifact_length as u64).to_le_bytes());
        debug_assert_eq!(bytes.len(), CLAIM_FIXED_BODY_BYTES);
        bytes.extend_from_slice(&claim_checksum(&bytes));
        debug_assert_eq!(bytes.len(), CLAIM_CANONICAL_BYTES);
        Ok(bytes)
    }

    /// Decodes one bounded canonical claim and rejects malformed or trailing input.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, DurablePublishedClaimCodecErrorV1> {
        if bytes.len() > MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES {
            return Err(DurablePublishedClaimCodecErrorV1::TooLarge {
                actual: bytes.len(),
                maximum: MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES,
            });
        }
        if bytes.len() < CLAIM_CANONICAL_BYTES {
            return Err(DurablePublishedClaimCodecErrorV1::Truncated);
        }
        if bytes.len() > CLAIM_CANONICAL_BYTES {
            return Err(DurablePublishedClaimCodecErrorV1::TrailingBytes);
        }
        let (body, checksum) = bytes.split_at(CLAIM_FIXED_BODY_BYTES);
        if claim_checksum(body) != checksum {
            return Err(DurablePublishedClaimCodecErrorV1::ChecksumMismatch);
        }

        let mut decoder = ClaimDecoder::new(body);
        if decoder.take(CLAIM_MAGIC.len())? != CLAIM_MAGIC {
            return Err(DurablePublishedClaimCodecErrorV1::BadMagic);
        }
        let version = decoder.u16()?;
        if version != CLAIM_VERSION {
            return Err(DurablePublishedClaimCodecErrorV1::UnsupportedVersion { actual: version });
        }
        let attempt = decoder.attempt()?;
        let scope = decoder.scope()?;
        let plan = DurableLinkPublicationPlanV1::new(
            attempt,
            scope,
            CanonicalLinkRequestIdentityV1::from_bytes(decoder.identity()?),
            PinnedWorkerIdentityV1::from_bytes(decoder.identity()?),
            ValidatedResponseIdentityV1::from_bytes(decoder.identity()?),
            LinkedOutputIdentityV1::from_bytes(decoder.identity()?),
            FinalizationIdentityV1::from_bytes(decoder.identity()?),
            FinalizedOutputIdentityV1::from_bytes(decoder.identity()?),
            AtomicPublicationIdentityV1::from_bytes(decoder.identity()?),
        );
        let upstream_evidence =
            UpstreamCodeObjectEvidenceIdentityV1::from_bytes(decoder.identity()?);
        let receipt = decoder.receipt()?;
        let files = DurablePublishedFileBindingV1 {
            output_identity: decoder.file_identity()?,
            record_identity: decoder.file_identity()?,
            artifact_identity: decoder.file_identity()?,
            artifact_length: decoder.artifact_length()?,
        };
        if !decoder.finished() {
            return Err(DurablePublishedClaimCodecErrorV1::TrailingBytes);
        }
        let claim = Self {
            plan,
            upstream_evidence,
            receipt,
            files,
        };
        claim.validate()?;
        if claim.encode_canonical()?.as_slice() != bytes {
            return Err(DurablePublishedClaimCodecErrorV1::NonCanonical);
        }
        Ok(claim)
    }

    /// An inert claim grants no code-object loading authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// An inert claim grants no kernel-launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    fn validate(&self) -> Result<(), DurablePublishedClaimCodecErrorV1> {
        let length = u64::try_from(self.files.artifact_length).unwrap_or(u64::MAX);
        if self.files.artifact_length == 0
            || self.files.artifact_length > MAX_DURABLE_FINALIZED_ARTIFACT_BYTES
        {
            return Err(DurablePublishedClaimCodecErrorV1::InvalidArtifactLength {
                actual: length,
            });
        }
        let expected = publication_receipt_for_producer_identity(
            self.plan.attempt(),
            self.plan,
            self.upstream_evidence,
            self.receipt.producer_identity(),
        );
        for (matches, field) in [
            (
                self.receipt.attempt_identity() == expected.attempt_identity(),
                DurablePublishedClaimReceiptFieldV1::Attempt,
            ),
            (
                self.receipt.scope_identity() == expected.scope_identity(),
                DurablePublishedClaimReceiptFieldV1::Scope,
            ),
            (
                self.receipt.plan_commitment() == expected.plan_commitment(),
                DurablePublishedClaimReceiptFieldV1::Plan,
            ),
            (
                self.receipt.upstream_evidence_identity() == expected.upstream_evidence_identity(),
                DurablePublishedClaimReceiptFieldV1::UpstreamEvidence,
            ),
            (
                self.receipt.finalized_output_identity() == expected.finalized_output_identity(),
                DurablePublishedClaimReceiptFieldV1::FinalizedOutput,
            ),
            (
                self.receipt.publication_identity() == expected.publication_identity(),
                DurablePublishedClaimReceiptFieldV1::Publication,
            ),
        ] {
            if !matches {
                return Err(DurablePublishedClaimCodecErrorV1::ReceiptMismatch { field });
            }
        }
        Ok(())
    }
}

/// Canonical inert protected claim retaining one exact `CompilerClosureV2`.
///
/// The private file bindings prevent construction from free-standing identities. The complete
/// closure and all other fields remain coordination evidence and grant no compiler, proof,
/// publication, load, or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurablePublishedHsacoClaimV2 {
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    receipt: BackendPublicationReceiptV2,
    files: DurablePublishedFileBindingV1,
}

impl DurablePublishedHsacoClaimV2 {
    pub(crate) fn new(
        plan: DurableLinkPublicationPlanV1,
        upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
        receipt: BackendPublicationReceiptV2,
        files: DurablePublishedFileBindingV1,
    ) -> Self {
        let claim = Self {
            plan,
            upstream_evidence,
            receipt,
            files,
        };
        debug_assert!(claim.validate().is_ok());
        claim
    }

    /// Returns the complete typed publication plan carried as inert data.
    pub const fn plan(&self) -> DurableLinkPublicationPlanV1 {
        self.plan
    }

    /// Returns the caller-supplied upstream evidence identity.
    pub const fn upstream_evidence(&self) -> UpstreamCodeObjectEvidenceIdentityV1 {
        self.upstream_evidence
    }

    /// Returns the exact V2 receipt persisted for the build attempt.
    pub const fn receipt(&self) -> BackendPublicationReceiptV2 {
        self.receipt
    }

    /// Returns the complete canonical compiler-closure preimage.
    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.receipt.compiler_closure()
    }

    /// Encodes this protected claim under the independent V2 schema.
    pub fn encode_canonical(&self) -> Result<Vec<u8>, DurablePublishedClaimCodecErrorV2> {
        self.validate()?;
        let mut bytes = Vec::with_capacity(CLAIM_CANONICAL_BYTES_V2);
        bytes.extend_from_slice(CLAIM_MAGIC_V2);
        bytes.extend_from_slice(&CLAIM_VERSION_V2.to_le_bytes());
        push_attempt(&mut bytes, self.plan.attempt());
        push_scope(&mut bytes, self.plan.scope());
        for identity in [
            *self.plan.request().as_bytes(),
            *self.plan.worker().as_bytes(),
            *self.plan.response().as_bytes(),
            *self.plan.linked_output().as_bytes(),
            *self.plan.finalization().as_bytes(),
            *self.plan.finalized_output().as_bytes(),
            *self.plan.publication().as_bytes(),
            self.upstream_evidence.as_bytes(),
        ] {
            bytes.extend_from_slice(&identity);
        }
        push_receipt_v2(&mut bytes, self.receipt);
        push_file_identity(&mut bytes, self.files.output_identity);
        push_file_identity(&mut bytes, self.files.record_identity);
        push_file_identity(&mut bytes, self.files.artifact_identity);
        bytes.extend_from_slice(&(self.files.artifact_length as u64).to_le_bytes());
        debug_assert_eq!(bytes.len(), CLAIM_FIXED_BODY_BYTES_V2);
        bytes.extend_from_slice(&claim_checksum_v2(&bytes));
        debug_assert_eq!(bytes.len(), CLAIM_CANONICAL_BYTES_V2);
        Ok(bytes)
    }

    /// Decodes only the bounded canonical V2 schema; V1 bytes are never reinterpreted.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, DurablePublishedClaimCodecErrorV2> {
        if bytes.len() > MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V2 {
            return Err(DurablePublishedClaimCodecErrorV2::TooLarge {
                actual: bytes.len(),
                maximum: MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V2,
            });
        }
        if bytes.len() < CLAIM_CANONICAL_BYTES_V2 {
            return Err(DurablePublishedClaimCodecErrorV2::Truncated);
        }
        if bytes.len() > CLAIM_CANONICAL_BYTES_V2 {
            return Err(DurablePublishedClaimCodecErrorV2::TrailingBytes);
        }
        let (body, checksum) = bytes.split_at(CLAIM_FIXED_BODY_BYTES_V2);
        if claim_checksum_v2(body) != checksum {
            return Err(DurablePublishedClaimCodecErrorV2::ChecksumMismatch);
        }

        let mut decoder = ClaimDecoder::new(body);
        if decode_v2(decoder.take(CLAIM_MAGIC_V2.len()))? != CLAIM_MAGIC_V2 {
            return Err(DurablePublishedClaimCodecErrorV2::BadMagic);
        }
        let version = decode_v2(decoder.u16())?;
        if version != CLAIM_VERSION_V2 {
            return Err(DurablePublishedClaimCodecErrorV2::UnsupportedVersion { actual: version });
        }
        let attempt = decode_v2(decoder.attempt())?;
        let scope = decode_v2(decoder.scope())?;
        let plan = DurableLinkPublicationPlanV1::new(
            attempt,
            scope,
            CanonicalLinkRequestIdentityV1::from_bytes(decode_v2(decoder.identity())?),
            PinnedWorkerIdentityV1::from_bytes(decode_v2(decoder.identity())?),
            ValidatedResponseIdentityV1::from_bytes(decode_v2(decoder.identity())?),
            LinkedOutputIdentityV1::from_bytes(decode_v2(decoder.identity())?),
            FinalizationIdentityV1::from_bytes(decode_v2(decoder.identity())?),
            FinalizedOutputIdentityV1::from_bytes(decode_v2(decoder.identity())?),
            AtomicPublicationIdentityV1::from_bytes(decode_v2(decoder.identity())?),
        );
        let upstream_evidence =
            UpstreamCodeObjectEvidenceIdentityV1::from_bytes(decode_v2(decoder.identity())?);
        let receipt = decode_receipt_v2(&mut decoder)?;
        let files = DurablePublishedFileBindingV1 {
            output_identity: decode_v2(decoder.file_identity())?,
            record_identity: decode_v2(decoder.file_identity())?,
            artifact_identity: decode_v2(decoder.file_identity())?,
            artifact_length: decode_v2(decoder.artifact_length())?,
        };
        if !decoder.finished() {
            return Err(DurablePublishedClaimCodecErrorV2::TrailingBytes);
        }
        let claim = Self {
            plan,
            upstream_evidence,
            receipt,
            files,
        };
        claim.validate()?;
        if claim.encode_canonical()?.as_slice() != bytes {
            return Err(DurablePublishedClaimCodecErrorV2::NonCanonical);
        }
        Ok(claim)
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_proof_authority(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    fn validate(&self) -> Result<(), DurablePublishedClaimCodecErrorV2> {
        let length = u64::try_from(self.files.artifact_length).unwrap_or(u64::MAX);
        if self.files.artifact_length == 0
            || self.files.artifact_length > MAX_DURABLE_FINALIZED_ARTIFACT_BYTES
        {
            return Err(DurablePublishedClaimCodecErrorV2::InvalidArtifactLength {
                actual: length,
            });
        }
        let expected = publication_receipt_for_producer_identity_v2(
            self.plan.attempt(),
            self.plan,
            self.upstream_evidence,
            self.receipt.compiler_closure(),
            self.receipt.producer_identity(),
        );
        for (matches, field) in [
            (
                self.receipt.attempt_identity() == expected.attempt_identity(),
                DurablePublishedClaimReceiptFieldV2::Attempt,
            ),
            (
                self.receipt.scope_identity() == expected.scope_identity(),
                DurablePublishedClaimReceiptFieldV2::Scope,
            ),
            (
                self.receipt.plan_commitment() == expected.plan_commitment(),
                DurablePublishedClaimReceiptFieldV2::Plan,
            ),
            (
                self.receipt.upstream_evidence_identity() == expected.upstream_evidence_identity(),
                DurablePublishedClaimReceiptFieldV2::UpstreamEvidence,
            ),
            (
                self.receipt.finalized_output_identity() == expected.finalized_output_identity(),
                DurablePublishedClaimReceiptFieldV2::FinalizedOutput,
            ),
            (
                self.receipt.publication_identity() == expected.publication_identity(),
                DurablePublishedClaimReceiptFieldV2::Publication,
            ),
        ] {
            if !matches {
                return Err(DurablePublishedClaimCodecErrorV2::ReceiptMismatch { field });
            }
        }
        Ok(())
    }
}

/// Canonical inert claim retaining one exact strict Worker V3 publication binding.
///
/// The V3 binding is preserved directly and is never projected into the V2 compiler-closure
/// schema. The private file bindings prevent construction from free-standing identities. This
/// value remains coordination evidence and grants no compiler, proof, publication, load, or
/// launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurablePublishedHsacoClaimV3 {
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    receipt: BackendPublicationReceiptV3,
    files: DurablePublishedFileBindingV1,
}

impl DurablePublishedHsacoClaimV3 {
    pub(crate) fn new(
        plan: DurableLinkPublicationPlanV1,
        upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
        receipt: BackendPublicationReceiptV3,
        files: DurablePublishedFileBindingV1,
    ) -> Result<Self, DurablePublishedClaimCodecErrorV3> {
        let claim = Self {
            plan,
            upstream_evidence,
            receipt,
            files,
        };
        claim.validate()?;
        Ok(claim)
    }

    /// Returns the complete typed publication plan carried as inert data.
    pub const fn plan(&self) -> DurableLinkPublicationPlanV1 {
        self.plan
    }

    /// Returns the caller-supplied upstream evidence identity.
    pub const fn upstream_evidence(&self) -> UpstreamCodeObjectEvidenceIdentityV1 {
        self.upstream_evidence
    }

    /// Returns the exact V3 receipt persisted for the build attempt.
    pub const fn receipt(&self) -> BackendPublicationReceiptV3 {
        self.receipt
    }

    /// Returns the complete strict Worker V3 publication binding.
    pub const fn worker_v3_binding(&self) -> WorkerV3PublicationBindingV1 {
        self.receipt.publication_binding()
    }

    /// Returns the complete canonical compiler-closure preimage retained by V3.
    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.worker_v3_binding().compiler_closure()
    }

    /// Encodes this claim under its independent fixed V3 schema.
    pub fn encode_canonical(&self) -> Result<Vec<u8>, DurablePublishedClaimCodecErrorV3> {
        self.validate()?;
        let mut bytes = claim_encoding_vec_v3(CLAIM_CANONICAL_BYTES_V3)?;
        bytes.extend_from_slice(CLAIM_MAGIC_V3);
        bytes.extend_from_slice(&CLAIM_VERSION_V3.to_le_bytes());
        push_attempt(&mut bytes, self.plan.attempt());
        push_scope(&mut bytes, self.plan.scope());
        for identity in [
            *self.plan.request().as_bytes(),
            *self.plan.worker().as_bytes(),
            *self.plan.response().as_bytes(),
            *self.plan.linked_output().as_bytes(),
            *self.plan.finalization().as_bytes(),
            *self.plan.finalized_output().as_bytes(),
            *self.plan.publication().as_bytes(),
            self.upstream_evidence.as_bytes(),
        ] {
            bytes.extend_from_slice(&identity);
        }
        push_receipt_v3(&mut bytes, self.receipt);
        push_file_identity(&mut bytes, self.files.output_identity);
        push_file_identity(&mut bytes, self.files.record_identity);
        push_file_identity(&mut bytes, self.files.artifact_identity);
        bytes.extend_from_slice(&(self.files.artifact_length as u64).to_le_bytes());
        debug_assert_eq!(bytes.len(), CLAIM_FIXED_BODY_BYTES_V3);
        bytes.extend_from_slice(&claim_checksum_v3(&bytes));
        debug_assert_eq!(bytes.len(), CLAIM_CANONICAL_BYTES_V3);
        Ok(bytes)
    }

    /// Decodes only the bounded canonical V3 schema; V1/V2 bytes are never reinterpreted.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, DurablePublishedClaimCodecErrorV3> {
        if bytes.len() > MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V3 {
            return Err(DurablePublishedClaimCodecErrorV3::TooLarge {
                actual: bytes.len(),
                maximum: MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V3,
            });
        }
        if bytes.len() < CLAIM_CANONICAL_BYTES_V3 {
            return Err(DurablePublishedClaimCodecErrorV3::Truncated);
        }
        if bytes.len() > CLAIM_CANONICAL_BYTES_V3 {
            return Err(DurablePublishedClaimCodecErrorV3::TrailingBytes);
        }
        let (body, checksum) = bytes.split_at(CLAIM_FIXED_BODY_BYTES_V3);
        if claim_checksum_v3(body) != checksum {
            return Err(DurablePublishedClaimCodecErrorV3::ChecksumMismatch);
        }

        let mut decoder = ClaimDecoder::new(body);
        if decode_v3(decoder.take(CLAIM_MAGIC_V3.len()))? != CLAIM_MAGIC_V3 {
            return Err(DurablePublishedClaimCodecErrorV3::BadMagic);
        }
        let version = decode_v3(decoder.u16())?;
        if version != CLAIM_VERSION_V3 {
            return Err(DurablePublishedClaimCodecErrorV3::UnsupportedVersion { actual: version });
        }
        let attempt = decode_v3(decoder.attempt())?;
        let scope = decode_v3(decoder.scope())?;
        let plan = DurableLinkPublicationPlanV1::new(
            attempt,
            scope,
            CanonicalLinkRequestIdentityV1::from_bytes(decode_v3(decoder.identity())?),
            PinnedWorkerIdentityV1::from_bytes(decode_v3(decoder.identity())?),
            ValidatedResponseIdentityV1::from_bytes(decode_v3(decoder.identity())?),
            LinkedOutputIdentityV1::from_bytes(decode_v3(decoder.identity())?),
            FinalizationIdentityV1::from_bytes(decode_v3(decoder.identity())?),
            FinalizedOutputIdentityV1::from_bytes(decode_v3(decoder.identity())?),
            AtomicPublicationIdentityV1::from_bytes(decode_v3(decoder.identity())?),
        );
        let upstream_evidence =
            UpstreamCodeObjectEvidenceIdentityV1::from_bytes(decode_v3(decoder.identity())?);
        let receipt = decode_receipt_v3(&mut decoder)?;
        let files = DurablePublishedFileBindingV1 {
            output_identity: decode_v3(decoder.file_identity())?,
            record_identity: decode_v3(decoder.file_identity())?,
            artifact_identity: decode_v3(decoder.file_identity())?,
            artifact_length: decode_v3(decoder.artifact_length())?,
        };
        if !decoder.finished() {
            return Err(DurablePublishedClaimCodecErrorV3::TrailingBytes);
        }
        let claim = Self {
            plan,
            upstream_evidence,
            receipt,
            files,
        };
        claim.validate()?;
        if claim.encode_canonical()?.as_slice() != bytes {
            return Err(DurablePublishedClaimCodecErrorV3::NonCanonical);
        }
        Ok(claim)
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_proof_authority(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    fn validate(&self) -> Result<(), DurablePublishedClaimCodecErrorV3> {
        let length = u64::try_from(self.files.artifact_length).unwrap_or(u64::MAX);
        if self.files.artifact_length == 0
            || self.files.artifact_length > MAX_DURABLE_FINALIZED_ARTIFACT_BYTES
        {
            return Err(DurablePublishedClaimCodecErrorV3::InvalidArtifactLength {
                actual: length,
            });
        }
        let binding = self.worker_v3_binding();
        validate_worker_v3_binding(binding)?;
        if binding.raw_output_sha256() != *self.plan.linked_output().as_bytes() {
            return Err(DurablePublishedClaimCodecErrorV3::WorkerV3BindingMismatch {
                field: DurablePublishedClaimWorkerV3BindingFieldV1::RawOutput,
            });
        }
        if binding.finalized_output_sha256() != *self.plan.finalized_output().as_bytes() {
            return Err(DurablePublishedClaimCodecErrorV3::WorkerV3BindingMismatch {
                field: DurablePublishedClaimWorkerV3BindingFieldV1::FinalizedOutput,
            });
        }
        if binding.finalized_output_length() != length {
            return Err(DurablePublishedClaimCodecErrorV3::WorkerV3BindingMismatch {
                field: DurablePublishedClaimWorkerV3BindingFieldV1::ArtifactLength,
            });
        }
        let expected = publication_receipt_for_producer_identity_v3(
            self.plan.attempt(),
            self.plan,
            self.upstream_evidence,
            binding,
            self.receipt.producer_identity(),
        );
        for (matches, field) in [
            (
                self.receipt.attempt_identity() == expected.attempt_identity(),
                DurablePublishedClaimReceiptFieldV3::Attempt,
            ),
            (
                self.receipt.scope_identity() == expected.scope_identity(),
                DurablePublishedClaimReceiptFieldV3::Scope,
            ),
            (
                self.receipt.plan_commitment() == expected.plan_commitment(),
                DurablePublishedClaimReceiptFieldV3::Plan,
            ),
            (
                self.receipt.upstream_evidence_identity() == expected.upstream_evidence_identity(),
                DurablePublishedClaimReceiptFieldV3::UpstreamEvidence,
            ),
            (
                self.receipt.finalized_output_identity() == expected.finalized_output_identity(),
                DurablePublishedClaimReceiptFieldV3::FinalizedOutput,
            ),
            (
                self.receipt.publication_identity() == expected.publication_identity(),
                DurablePublishedClaimReceiptFieldV3::Publication,
            ),
        ] {
            if !matches {
                return Err(DurablePublishedClaimCodecErrorV3::ReceiptMismatch { field });
            }
        }
        Ok(())
    }
}

fn claim_encoding_vec_v3(requested: usize) -> Result<Vec<u8>, DurablePublishedClaimCodecErrorV3> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(requested)
        .map_err(|_| DurablePublishedClaimCodecErrorV3::AllocationFailed { requested })?;
    Ok(bytes)
}

/// Reacquires a fresh non-`Clone` lease from one inert cross-process claim.
///
/// The operation takes the existing output directory's cooperative lock without blocking, then
/// atomically revalidates claim coherence, the exact completed provenance receipt and producer,
/// the complete publication plan, current generation, output path identity, canonical record,
/// and content-addressed artifact. The lock is released after the new descriptor-owning lease has
/// been minted. Neither the claim nor the returned lease establishes compiler provenance,
/// semantic validity, load authority, launch authority, or rollback resistance.
pub fn reacquire_current_hsaco_publication_lease_v1(
    output_dir: &Path,
    claim: &DurablePublishedHsacoClaimV1,
) -> Result<DurableCurrentLinkPublicationLeaseV1, DurablePublishedClaimReacquisitionErrorV1> {
    reacquire_current_hsaco_publication_lease::<ClaimSchemaV1>(output_dir, claim)
        .map_err(reacquisition_error_v1)
}

/// Reacquires a fresh lease only from exact protected V2 claim and registry state.
pub fn reacquire_current_hsaco_publication_lease_v2(
    output_dir: &Path,
    claim: &DurablePublishedHsacoClaimV2,
) -> Result<DurableCurrentLinkPublicationLeaseV1, DurablePublishedClaimReacquisitionErrorV2> {
    reacquire_current_hsaco_publication_lease::<ClaimSchemaV2>(output_dir, claim)
        .map_err(reacquisition_error_v2)
}

/// Reacquires a fresh lease only from exact strict Worker V3 claim and registry state.
pub fn reacquire_current_hsaco_publication_lease_v3(
    output_dir: &Path,
    claim: &DurablePublishedHsacoClaimV3,
) -> Result<DurableCurrentLinkPublicationLeaseV1, DurablePublishedClaimReacquisitionErrorV3> {
    reacquire_current_hsaco_publication_lease::<ClaimSchemaV3>(output_dir, claim)
        .map_err(reacquisition_error_v3)
}

pub(crate) fn validate_current_hsaco_publication_locked_v3(
    output: &PinnedOutput,
    claim: &DurablePublishedHsacoClaimV3,
) -> Result<DurableCurrentLinkPublicationLeaseV1, DurablePublishedClaimReacquisitionErrorV3> {
    reacquire_current_hsaco_publication_lease_locked::<ClaimSchemaV3>(output, claim)
        .map_err(reacquisition_error_v3)
}

pub(crate) fn validate_current_hsaco_publication_receipt_locked_v3(
    output: &PinnedOutput,
    plan: DurableLinkPublicationPlanV1,
    receipt: BackendPublicationReceiptV3,
) -> Result<
    (
        DurablePublishedHsacoClaimV3,
        DurableCurrentLinkPublicationLeaseV1,
    ),
    DurablePublishedClaimReacquisitionErrorV3,
> {
    let files = recover_durable_published_file_binding_locked(output, plan)
        .map_err(ReacquisitionError::Publication)
        .map_err(reacquisition_error_v3)?
        .ok_or(DurablePublishedClaimReacquisitionErrorV3::ReceiptMismatch)?;
    let upstream =
        UpstreamCodeObjectEvidenceIdentityV1::from_bytes(receipt.upstream_evidence_identity());
    let claim = DurablePublishedHsacoClaimV3::new(plan, upstream, receipt, files)
        .map_err(DurablePublishedClaimReacquisitionErrorV3::InvalidClaim)?;
    let lease = validate_current_hsaco_publication_locked_v3(output, &claim)?;
    Ok((claim, lease))
}

enum ReacquisitionError<C> {
    Busy,
    InvalidClaim(C),
    Filesystem(EmitError),
    AttemptNotFound,
    AttemptState,
    ReceiptMismatch,
    ProducerIdentityMismatch,
    Publication(DurableLinkPublicationError),
}

trait ClaimSchema {
    type Claim;
    type CodecError;

    fn validate(claim: &Self::Claim) -> Result<(), Self::CodecError>;
    fn plan(claim: &Self::Claim) -> DurableLinkPublicationPlanV1;
    fn files(claim: &Self::Claim) -> DurablePublishedFileBindingV1;
    fn receipt_matches(receipt: Option<BackendReceiptV1>, claim: &Self::Claim) -> bool;
    fn producer_matches(stable_source: &str, crate_name: &str, claim: &Self::Claim) -> bool;
}

struct ClaimSchemaV1;

impl ClaimSchema for ClaimSchemaV1 {
    type Claim = DurablePublishedHsacoClaimV1;
    type CodecError = DurablePublishedClaimCodecErrorV1;

    fn validate(claim: &Self::Claim) -> Result<(), Self::CodecError> {
        claim.validate()
    }

    fn plan(claim: &Self::Claim) -> DurableLinkPublicationPlanV1 {
        claim.plan
    }

    fn files(claim: &Self::Claim) -> DurablePublishedFileBindingV1 {
        claim.files
    }

    fn receipt_matches(receipt: Option<BackendReceiptV1>, claim: &Self::Claim) -> bool {
        matches!(receipt, Some(BackendReceiptV1::Provenance(actual)) if actual == claim.receipt)
    }

    fn producer_matches(stable_source: &str, crate_name: &str, claim: &Self::Claim) -> bool {
        producer_receipt_identity_v1(stable_source, crate_name) == claim.receipt.producer_identity()
    }
}

struct ClaimSchemaV2;

impl ClaimSchema for ClaimSchemaV2 {
    type Claim = DurablePublishedHsacoClaimV2;
    type CodecError = DurablePublishedClaimCodecErrorV2;

    fn validate(claim: &Self::Claim) -> Result<(), Self::CodecError> {
        claim.validate()
    }

    fn plan(claim: &Self::Claim) -> DurableLinkPublicationPlanV1 {
        claim.plan
    }

    fn files(claim: &Self::Claim) -> DurablePublishedFileBindingV1 {
        claim.files
    }

    fn receipt_matches(receipt: Option<BackendReceiptV1>, claim: &Self::Claim) -> bool {
        matches!(receipt, Some(BackendReceiptV1::ProvenanceV2(actual)) if actual == claim.receipt)
    }

    fn producer_matches(stable_source: &str, crate_name: &str, claim: &Self::Claim) -> bool {
        producer_receipt_identity_v2(stable_source, crate_name) == claim.receipt.producer_identity()
    }
}

struct ClaimSchemaV3;

impl ClaimSchema for ClaimSchemaV3 {
    type Claim = DurablePublishedHsacoClaimV3;
    type CodecError = DurablePublishedClaimCodecErrorV3;

    fn validate(claim: &Self::Claim) -> Result<(), Self::CodecError> {
        claim.validate()
    }

    fn plan(claim: &Self::Claim) -> DurableLinkPublicationPlanV1 {
        claim.plan
    }

    fn files(claim: &Self::Claim) -> DurablePublishedFileBindingV1 {
        claim.files
    }

    fn receipt_matches(receipt: Option<BackendReceiptV1>, claim: &Self::Claim) -> bool {
        matches!(
            receipt,
            Some(BackendReceiptV1::ProvenanceV3(actual))
                | Some(BackendReceiptV1::EnvelopeCustodyV3(actual, _))
                if actual == claim.receipt
        )
    }

    fn producer_matches(stable_source: &str, crate_name: &str, claim: &Self::Claim) -> bool {
        producer_receipt_identity_v3(stable_source, crate_name) == claim.receipt.producer_identity()
    }
}

fn reacquire_current_hsaco_publication_lease<S: ClaimSchema>(
    output_dir: &Path,
    claim: &S::Claim,
) -> Result<DurableCurrentLinkPublicationLeaseV1, ReacquisitionError<S::CodecError>> {
    S::validate(claim).map_err(ReacquisitionError::InvalidClaim)?;
    let output = PinnedOutput::open_existing(output_dir).map_err(ReacquisitionError::Filesystem)?;
    let lock = output
        .try_lock()
        .map_err(ReacquisitionError::Filesystem)?
        .ok_or(ReacquisitionError::Busy)?;
    output
        .verify_path_identity()
        .map_err(ReacquisitionError::Filesystem)?;
    let lease = reacquire_current_hsaco_publication_lease_locked::<S>(&output, claim)?;
    drop(lock);
    Ok(lease)
}

fn reacquire_current_hsaco_publication_lease_locked<S: ClaimSchema>(
    output: &PinnedOutput,
    claim: &S::Claim,
) -> Result<DurableCurrentLinkPublicationLeaseV1, ReacquisitionError<S::CodecError>> {
    S::validate(claim).map_err(ReacquisitionError::InvalidClaim)?;
    output
        .verify_path_identity()
        .map_err(ReacquisitionError::Filesystem)?;
    validate_persisted_receipt::<S>(output, claim)?;
    let lease = reacquire_current_publication_lease_locked(output, S::plan(claim), S::files(claim))
        .map_err(ReacquisitionError::Publication)?;
    validate_persisted_receipt::<S>(output, claim)?;
    Ok(lease)
}

fn validate_persisted_receipt<S: ClaimSchema>(
    output: &PinnedOutput,
    claim: &S::Claim,
) -> Result<(), ReacquisitionError<S::CodecError>> {
    let attempts = read_attempt_registry(output).map_err(ReacquisitionError::Filesystem)?;
    let (stable_source, record) = attempts
        .record_for_attempt(S::plan(claim).attempt())
        .ok_or(ReacquisitionError::AttemptNotFound)?;
    if !matches!(
        record.phase,
        AttemptPhase::BackendClaimed | AttemptPhase::Completed
    ) {
        return Err(ReacquisitionError::AttemptState);
    }
    if !S::receipt_matches(record.backend_receipt, claim) {
        return Err(ReacquisitionError::ReceiptMismatch);
    }
    if !S::producer_matches(stable_source, &record.crate_name, claim) {
        return Err(ReacquisitionError::ProducerIdentityMismatch);
    }
    Ok(())
}

fn reacquisition_error_v1(
    error: ReacquisitionError<DurablePublishedClaimCodecErrorV1>,
) -> DurablePublishedClaimReacquisitionErrorV1 {
    match error {
        ReacquisitionError::Busy => DurablePublishedClaimReacquisitionErrorV1::Busy,
        ReacquisitionError::InvalidClaim(error) => {
            DurablePublishedClaimReacquisitionErrorV1::InvalidClaim(error)
        }
        ReacquisitionError::Filesystem(error) => {
            DurablePublishedClaimReacquisitionErrorV1::Filesystem(error)
        }
        ReacquisitionError::AttemptNotFound => {
            DurablePublishedClaimReacquisitionErrorV1::AttemptNotFound
        }
        ReacquisitionError::AttemptState => DurablePublishedClaimReacquisitionErrorV1::AttemptState,
        ReacquisitionError::ReceiptMismatch => {
            DurablePublishedClaimReacquisitionErrorV1::ReceiptMismatch
        }
        ReacquisitionError::ProducerIdentityMismatch => {
            DurablePublishedClaimReacquisitionErrorV1::ProducerIdentityMismatch
        }
        ReacquisitionError::Publication(error) => {
            DurablePublishedClaimReacquisitionErrorV1::Publication(error)
        }
    }
}

fn reacquisition_error_v2(
    error: ReacquisitionError<DurablePublishedClaimCodecErrorV2>,
) -> DurablePublishedClaimReacquisitionErrorV2 {
    match error {
        ReacquisitionError::Busy => DurablePublishedClaimReacquisitionErrorV2::Busy,
        ReacquisitionError::InvalidClaim(error) => {
            DurablePublishedClaimReacquisitionErrorV2::InvalidClaim(error)
        }
        ReacquisitionError::Filesystem(error) => {
            DurablePublishedClaimReacquisitionErrorV2::Filesystem(error)
        }
        ReacquisitionError::AttemptNotFound => {
            DurablePublishedClaimReacquisitionErrorV2::AttemptNotFound
        }
        ReacquisitionError::AttemptState => DurablePublishedClaimReacquisitionErrorV2::AttemptState,
        ReacquisitionError::ReceiptMismatch => {
            DurablePublishedClaimReacquisitionErrorV2::ReceiptMismatch
        }
        ReacquisitionError::ProducerIdentityMismatch => {
            DurablePublishedClaimReacquisitionErrorV2::ProducerIdentityMismatch
        }
        ReacquisitionError::Publication(error) => {
            DurablePublishedClaimReacquisitionErrorV2::Publication(error)
        }
    }
}

fn reacquisition_error_v3(
    error: ReacquisitionError<DurablePublishedClaimCodecErrorV3>,
) -> DurablePublishedClaimReacquisitionErrorV3 {
    match error {
        ReacquisitionError::Busy => DurablePublishedClaimReacquisitionErrorV3::Busy,
        ReacquisitionError::InvalidClaim(error) => {
            DurablePublishedClaimReacquisitionErrorV3::InvalidClaim(error)
        }
        ReacquisitionError::Filesystem(error) => {
            DurablePublishedClaimReacquisitionErrorV3::Filesystem(error)
        }
        ReacquisitionError::AttemptNotFound => {
            DurablePublishedClaimReacquisitionErrorV3::AttemptNotFound
        }
        ReacquisitionError::AttemptState => DurablePublishedClaimReacquisitionErrorV3::AttemptState,
        ReacquisitionError::ReceiptMismatch => {
            DurablePublishedClaimReacquisitionErrorV3::ReceiptMismatch
        }
        ReacquisitionError::ProducerIdentityMismatch => {
            DurablePublishedClaimReacquisitionErrorV3::ProducerIdentityMismatch
        }
        ReacquisitionError::Publication(error) => {
            DurablePublishedClaimReacquisitionErrorV3::Publication(error)
        }
    }
}

fn push_attempt(bytes: &mut Vec<u8>, attempt: BuildAttempt) {
    bytes.extend_from_slice(&attempt.generation().to_le_bytes());
    bytes.extend_from_slice(attempt.session().as_bytes());
    bytes.extend_from_slice(attempt.invocation().as_bytes());
}

fn push_scope(bytes: &mut Vec<u8>, scope: LinkPublicationScopeV1) {
    bytes.extend_from_slice(scope.package().as_bytes());
    bytes.extend_from_slice(scope.kernel_set().as_bytes());
    bytes.extend_from_slice(scope.target().as_bytes());
}

fn push_receipt(bytes: &mut Vec<u8>, receipt: BackendPublicationReceiptV1) {
    for identity in [
        receipt.attempt_identity(),
        receipt.producer_identity(),
        receipt.scope_identity(),
        receipt.plan_commitment(),
        receipt.upstream_evidence_identity(),
        receipt.finalized_output_identity(),
        receipt.publication_identity(),
    ] {
        bytes.extend_from_slice(&identity);
    }
}

fn push_receipt_v2(bytes: &mut Vec<u8>, receipt: BackendPublicationReceiptV2) {
    for identity in [
        receipt.attempt_identity(),
        receipt.producer_identity(),
        receipt.scope_identity(),
        receipt.plan_commitment(),
        receipt.upstream_evidence_identity(),
        receipt.finalized_output_identity(),
        receipt.publication_identity(),
    ] {
        bytes.extend_from_slice(&identity);
    }
    push_compiler_closure_v2(bytes, receipt.compiler_closure());
}

fn push_receipt_v3(bytes: &mut Vec<u8>, receipt: BackendPublicationReceiptV3) {
    for identity in [
        receipt.attempt_identity(),
        receipt.producer_identity(),
        receipt.scope_identity(),
        receipt.plan_commitment(),
        receipt.upstream_evidence_identity(),
        receipt.finalized_output_identity(),
        receipt.publication_identity(),
    ] {
        bytes.extend_from_slice(&identity);
    }
    push_worker_v3_binding_preimage(bytes, receipt.publication_binding());
}

fn push_worker_v3_binding_preimage(bytes: &mut Vec<u8>, binding: WorkerV3PublicationBindingV1) {
    push_compiler_closure_v2(bytes, binding.compiler_closure());
    for identity in [
        binding.publication_intent_record_identity(),
        binding.finalization_identity(),
        binding.source_evidence_identity(),
        binding.compiler_handoff_binding_identity(),
        binding.raw_inspection_identity(),
        binding.raw_output_sha256(),
    ] {
        bytes.extend_from_slice(&identity);
    }
    bytes.extend_from_slice(&binding.raw_output_length().to_le_bytes());
    bytes.extend_from_slice(&binding.finalized_output_sha256());
    bytes.extend_from_slice(&binding.finalized_output_length().to_le_bytes());
}

fn push_file_identity(bytes: &mut Vec<u8>, identity: DurableFileIdentityV1) {
    bytes.extend_from_slice(&identity.device.to_le_bytes());
    bytes.extend_from_slice(&identity.inode.to_le_bytes());
}

fn claim_checksum(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(CLAIM_CHECKSUM_DOMAIN);
    digest.update(bytes);
    digest.finalize().into()
}

fn claim_checksum_v2(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(CLAIM_CHECKSUM_DOMAIN_V2);
    digest.update(bytes);
    digest.finalize().into()
}

fn claim_checksum_v3(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(CLAIM_CHECKSUM_DOMAIN_V3);
    digest.update(bytes);
    digest.finalize().into()
}

fn decode_v2<T>(
    value: Result<T, DurablePublishedClaimCodecErrorV1>,
) -> Result<T, DurablePublishedClaimCodecErrorV2> {
    value.map_err(|error| match error {
        DurablePublishedClaimCodecErrorV1::TooLarge { actual, maximum } => {
            DurablePublishedClaimCodecErrorV2::TooLarge { actual, maximum }
        }
        DurablePublishedClaimCodecErrorV1::Truncated => {
            DurablePublishedClaimCodecErrorV2::Truncated
        }
        DurablePublishedClaimCodecErrorV1::TrailingBytes => {
            DurablePublishedClaimCodecErrorV2::TrailingBytes
        }
        DurablePublishedClaimCodecErrorV1::BadMagic => DurablePublishedClaimCodecErrorV2::BadMagic,
        DurablePublishedClaimCodecErrorV1::UnsupportedVersion { actual } => {
            DurablePublishedClaimCodecErrorV2::UnsupportedVersion { actual }
        }
        DurablePublishedClaimCodecErrorV1::ChecksumMismatch => {
            DurablePublishedClaimCodecErrorV2::ChecksumMismatch
        }
        DurablePublishedClaimCodecErrorV1::InvalidAttempt => {
            DurablePublishedClaimCodecErrorV2::InvalidAttempt
        }
        DurablePublishedClaimCodecErrorV1::InvalidArtifactLength { actual } => {
            DurablePublishedClaimCodecErrorV2::InvalidArtifactLength { actual }
        }
        DurablePublishedClaimCodecErrorV1::ReceiptMismatch { .. }
        | DurablePublishedClaimCodecErrorV1::NonCanonical => {
            unreachable!("shared decoder primitives do not produce semantic claim errors")
        }
    })
}

fn decode_receipt_v2(
    decoder: &mut ClaimDecoder<'_>,
) -> Result<BackendPublicationReceiptV2, DurablePublishedClaimCodecErrorV2> {
    let attempt_identity = decode_v2(decoder.identity())?;
    let producer_identity = decode_v2(decoder.identity())?;
    let scope_identity = decode_v2(decoder.identity())?;
    let plan_commitment = decode_v2(decoder.identity())?;
    let upstream_evidence_identity = decode_v2(decoder.identity())?;
    let finalized_output_identity = decode_v2(decoder.identity())?;
    let publication_identity = decode_v2(decoder.identity())?;
    let closure_bytes = decode_v2(decoder.take(COMPILER_CLOSURE_BYTES_V2))?;
    let compiler_closure = decode_compiler_closure_v2(closure_bytes)
        .map_err(DurablePublishedClaimCodecErrorV2::InvalidCompilerClosure)?;
    Ok(BackendPublicationReceiptV2::new(
        attempt_identity,
        producer_identity,
        scope_identity,
        plan_commitment,
        upstream_evidence_identity,
        finalized_output_identity,
        publication_identity,
        compiler_closure,
    ))
}

fn decode_v3<T>(
    value: Result<T, DurablePublishedClaimCodecErrorV1>,
) -> Result<T, DurablePublishedClaimCodecErrorV3> {
    value.map_err(|error| match error {
        DurablePublishedClaimCodecErrorV1::TooLarge { actual, maximum } => {
            DurablePublishedClaimCodecErrorV3::TooLarge { actual, maximum }
        }
        DurablePublishedClaimCodecErrorV1::Truncated => {
            DurablePublishedClaimCodecErrorV3::Truncated
        }
        DurablePublishedClaimCodecErrorV1::TrailingBytes => {
            DurablePublishedClaimCodecErrorV3::TrailingBytes
        }
        DurablePublishedClaimCodecErrorV1::BadMagic => DurablePublishedClaimCodecErrorV3::BadMagic,
        DurablePublishedClaimCodecErrorV1::UnsupportedVersion { actual } => {
            DurablePublishedClaimCodecErrorV3::UnsupportedVersion { actual }
        }
        DurablePublishedClaimCodecErrorV1::ChecksumMismatch => {
            DurablePublishedClaimCodecErrorV3::ChecksumMismatch
        }
        DurablePublishedClaimCodecErrorV1::InvalidAttempt => {
            DurablePublishedClaimCodecErrorV3::InvalidAttempt
        }
        DurablePublishedClaimCodecErrorV1::InvalidArtifactLength { actual } => {
            DurablePublishedClaimCodecErrorV3::InvalidArtifactLength { actual }
        }
        DurablePublishedClaimCodecErrorV1::ReceiptMismatch { .. }
        | DurablePublishedClaimCodecErrorV1::NonCanonical => {
            unreachable!("shared decoder primitives do not produce semantic claim errors")
        }
    })
}

fn decode_receipt_v3(
    decoder: &mut ClaimDecoder<'_>,
) -> Result<BackendPublicationReceiptV3, DurablePublishedClaimCodecErrorV3> {
    let attempt_identity = decode_v3(decoder.identity())?;
    let producer_identity = decode_v3(decoder.identity())?;
    let scope_identity = decode_v3(decoder.identity())?;
    let plan_commitment = decode_v3(decoder.identity())?;
    let upstream_evidence_identity = decode_v3(decoder.identity())?;
    let finalized_output_identity = decode_v3(decoder.identity())?;
    let publication_identity = decode_v3(decoder.identity())?;
    let binding = decode_worker_v3_binding_preimage(decoder)?;
    Ok(BackendPublicationReceiptV3::new(
        attempt_identity,
        producer_identity,
        scope_identity,
        plan_commitment,
        upstream_evidence_identity,
        finalized_output_identity,
        publication_identity,
        binding,
    ))
}

fn decode_worker_v3_binding_preimage(
    decoder: &mut ClaimDecoder<'_>,
) -> Result<WorkerV3PublicationBindingV1, DurablePublishedClaimCodecErrorV3> {
    let closure_bytes = decode_v3(decoder.take(COMPILER_CLOSURE_BYTES_V2))?;
    let compiler_closure = decode_compiler_closure_v2(closure_bytes).map_err(|error| {
        DurablePublishedClaimCodecErrorV3::InvalidWorkerV3Binding(
            WorkerV3PublicationBindingErrorV1::InvalidCompilerClosure(error),
        )
    })?;
    WorkerV3PublicationBindingV1::new(
        compiler_closure,
        decode_v3(decoder.identity())?,
        decode_v3(decoder.identity())?,
        decode_v3(decoder.identity())?,
        decode_v3(decoder.identity())?,
        decode_v3(decoder.identity())?,
        decode_v3(decoder.identity())?,
        decode_v3(decoder.u64())?,
        decode_v3(decoder.identity())?,
        decode_v3(decoder.u64())?,
    )
    .map_err(DurablePublishedClaimCodecErrorV3::InvalidWorkerV3Binding)
}

fn validate_worker_v3_binding(
    binding: WorkerV3PublicationBindingV1,
) -> Result<(), DurablePublishedClaimCodecErrorV3> {
    WorkerV3PublicationBindingV1::new(
        binding.compiler_closure(),
        binding.publication_intent_record_identity(),
        binding.finalization_identity(),
        binding.source_evidence_identity(),
        binding.compiler_handoff_binding_identity(),
        binding.raw_inspection_identity(),
        binding.raw_output_sha256(),
        binding.raw_output_length(),
        binding.finalized_output_sha256(),
        binding.finalized_output_length(),
    )
    .map(|_| ())
    .map_err(DurablePublishedClaimCodecErrorV3::InvalidWorkerV3Binding)
}

struct ClaimDecoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ClaimDecoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], DurablePublishedClaimCodecErrorV1> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(DurablePublishedClaimCodecErrorV1::Truncated)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(DurablePublishedClaimCodecErrorV1::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }

    fn u16(&mut self) -> Result<u16, DurablePublishedClaimCodecErrorV1> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("two bytes"),
        ))
    }

    fn u64(&mut self) -> Result<u64, DurablePublishedClaimCodecErrorV1> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("eight bytes"),
        ))
    }

    fn identity(&mut self) -> Result<[u8; 32], DurablePublishedClaimCodecErrorV1> {
        Ok(self.take(32)?.try_into().expect("32-byte identity"))
    }

    fn attempt(&mut self) -> Result<BuildAttempt, DurablePublishedClaimCodecErrorV1> {
        let generation = self.u64()?;
        let session = BuildSession::from_bytes(self.take(16)?.try_into().expect("16-byte session"));
        let invocation =
            BuildInvocation::from_bytes(self.take(32)?.try_into().expect("32-byte invocation"));
        BuildAttempt::from_env_value(&format!("{generation}:{session}:{invocation}"))
            .map_err(|_| DurablePublishedClaimCodecErrorV1::InvalidAttempt)
    }

    fn scope(&mut self) -> Result<LinkPublicationScopeV1, DurablePublishedClaimCodecErrorV1> {
        Ok(LinkPublicationScopeV1::new(
            PackageIdentityV1::from_bytes(self.identity()?),
            KernelSetIdentityV1::from_bytes(self.identity()?),
            TargetIdentityV1::from_bytes(self.identity()?),
        ))
    }

    fn receipt(
        &mut self,
    ) -> Result<BackendPublicationReceiptV1, DurablePublishedClaimCodecErrorV1> {
        Ok(BackendPublicationReceiptV1::new(
            self.identity()?,
            self.identity()?,
            self.identity()?,
            self.identity()?,
            self.identity()?,
            self.identity()?,
            self.identity()?,
        ))
    }

    fn file_identity(
        &mut self,
    ) -> Result<DurableFileIdentityV1, DurablePublishedClaimCodecErrorV1> {
        Ok(DurableFileIdentityV1 {
            device: self.u64()?,
            inode: self.u64()?,
        })
    }

    fn artifact_length(&mut self) -> Result<usize, DurablePublishedClaimCodecErrorV1> {
        let actual = self.u64()?;
        let length = usize::try_from(actual)
            .map_err(|_| DurablePublishedClaimCodecErrorV1::InvalidArtifactLength { actual })?;
        if length == 0 || length > MAX_DURABLE_FINALIZED_ARTIFACT_BYTES {
            return Err(DurablePublishedClaimCodecErrorV1::InvalidArtifactLength { actual });
        }
        Ok(length)
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_v3_claim() -> DurablePublishedHsacoClaimV3 {
        let attempt = BuildAttempt::new(
            0x0102_0304_0506_0708,
            BuildSession::from_bytes([0x10; 16]),
            BuildInvocation::from_bytes([0x11; 32]),
        )
        .unwrap();
        let plan = DurableLinkPublicationPlanV1::new(
            attempt,
            LinkPublicationScopeV1::new(
                PackageIdentityV1::from_bytes([0x20; 32]),
                KernelSetIdentityV1::from_bytes([0x21; 32]),
                TargetIdentityV1::from_bytes([0x22; 32]),
            ),
            CanonicalLinkRequestIdentityV1::from_bytes([0x30; 32]),
            PinnedWorkerIdentityV1::from_bytes([0x31; 32]),
            ValidatedResponseIdentityV1::from_bytes([0x32; 32]),
            LinkedOutputIdentityV1::from_bytes([0x33; 32]),
            FinalizationIdentityV1::from_bytes([0x34; 32]),
            FinalizedOutputIdentityV1::from_bytes([0x35; 32]),
            AtomicPublicationIdentityV1::from_bytes([0x36; 32]),
        );
        let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0x40; 32]);
        let binding = WorkerV3PublicationBindingV1::new(
            CompilerClosureV2::new([1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]).unwrap(),
            [0x42; 32],
            [0x43; 32],
            [0x44; 32],
            [0x45; 32],
            [0x46; 32],
            [0x33; 32],
            0x1200,
            [0x35; 32],
            0x1234,
        )
        .unwrap();
        let receipt = publication_receipt_for_producer_identity_v3(
            attempt, plan, upstream, binding, [0x41; 32],
        );
        DurablePublishedHsacoClaimV3::new(
            plan,
            upstream,
            receipt,
            DurablePublishedFileBindingV1 {
                output_identity: DurableFileIdentityV1 {
                    device: 0x5051_5253_5455_5657,
                    inode: 0x6061_6263_6465_6667,
                },
                record_identity: DurableFileIdentityV1 {
                    device: 0x7071_7273_7475_7677,
                    inode: 0x8081_8283_8485_8687,
                },
                artifact_identity: DurableFileIdentityV1 {
                    device: 0x9091_9293_9495_9697,
                    inode: 0xa0a1_a2a3_a4a5_a6a7,
                },
                artifact_length: 0x1234,
            },
        )
        .unwrap()
    }

    #[test]
    fn full_v1_claim_golden_is_stable() {
        let attempt = BuildAttempt::new(
            0x0102_0304_0506_0708,
            BuildSession::from_bytes([0x10; 16]),
            BuildInvocation::from_bytes([0x11; 32]),
        )
        .unwrap();
        let plan = DurableLinkPublicationPlanV1::new(
            attempt,
            LinkPublicationScopeV1::new(
                PackageIdentityV1::from_bytes([0x20; 32]),
                KernelSetIdentityV1::from_bytes([0x21; 32]),
                TargetIdentityV1::from_bytes([0x22; 32]),
            ),
            CanonicalLinkRequestIdentityV1::from_bytes([0x30; 32]),
            PinnedWorkerIdentityV1::from_bytes([0x31; 32]),
            ValidatedResponseIdentityV1::from_bytes([0x32; 32]),
            LinkedOutputIdentityV1::from_bytes([0x33; 32]),
            FinalizationIdentityV1::from_bytes([0x34; 32]),
            FinalizedOutputIdentityV1::from_bytes([0x35; 32]),
            AtomicPublicationIdentityV1::from_bytes([0x36; 32]),
        );
        let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0x40; 32]);
        let receipt =
            publication_receipt_for_producer_identity(attempt, plan, upstream, [0x41; 32]);
        let claim = DurablePublishedHsacoClaimV1::new(
            plan,
            upstream,
            receipt,
            DurablePublishedFileBindingV1 {
                output_identity: DurableFileIdentityV1 {
                    device: 0x5051_5253_5455_5657,
                    inode: 0x6061_6263_6465_6667,
                },
                record_identity: DurableFileIdentityV1 {
                    device: 0x7071_7273_7475_7677,
                    inode: 0x8081_8283_8485_8687,
                },
                artifact_identity: DurableFileIdentityV1 {
                    device: 0x9091_9293_9495_9697,
                    inode: 0xa0a1_a2a3_a4a5_a6a7,
                },
                artifact_length: 0x1234,
            },
        );
        let encoded = claim.encode_canonical().unwrap();
        assert_eq!(
            crate::encode_hex(&encoded),
            "4645324f332d5055424c49534845442d485341434f2d434c41494d2d5631000100080706050403020110101010101010101010101010101010111111111111111111111111111111111111111111111111111111111111111120202020202020202020202020202020202020202020202020202020202020202121212121212121212121212121212121212121212121212121212121212121222222222222222222222222222222222222222222222222222222222222222230303030303030303030303030303030303030303030303030303030303030303131313131313131313131313131313131313131313131313131313131313131323232323232323232323232323232323232323232323232323232323232323233333333333333333333333333333333333333333333333333333333333333333434343434343434343434343434343434343434343434343434343434343434353535353535353535353535353535353535353535353535353535353535353536363636363636363636363636363636363636363636363636363636363636364040404040404040404040404040404040404040404040404040404040404040bbbdf144cd89f76fab798e82340aaef319555795cd22711b329be43f251839214141414141414141414141414141414141414141414141414141414141414141924fc76df772a71a7f660bc1ce6025ff37c2e40eacb783fc605d72ab9456da53d12545061156d6c857e219791d7a932960767db3565811f6513dbe37b7121e1d40404040404040404040404040404040404040404040404040404040404040403535353535353535353535353535353535353535353535353535353535353535363636363636363636363636363636363636363636363636363636363636363657565554535251506766656463626160777675747372717087868584838281809796959493929190a7a6a5a4a3a2a1a03412000000000000f20ca7e76760fc5d7b5195f403bc719310eed6c0fb1315612e8f85b8229d9d64"
        );
        assert_eq!(
            DurablePublishedHsacoClaimV1::decode_canonical(&encoded).unwrap(),
            claim
        );
    }

    #[test]
    fn full_v3_claim_round_trips_under_only_the_v3_schema() {
        let claim = full_v3_claim();
        let encoded = claim.encode_canonical().unwrap();
        assert_eq!(encoded.len(), CLAIM_CANONICAL_BYTES_V3);
        assert_eq!(
            DurablePublishedHsacoClaimV3::decode_canonical(&encoded).unwrap(),
            claim
        );
        assert!(matches!(
            DurablePublishedHsacoClaimV1::decode_canonical(&encoded),
            Err(DurablePublishedClaimCodecErrorV1::TooLarge { .. })
        ));
        assert!(matches!(
            DurablePublishedHsacoClaimV2::decode_canonical(&encoded),
            Err(DurablePublishedClaimCodecErrorV2::TooLarge { .. })
        ));
    }

    #[test]
    fn v3_claim_codec_rejects_truncation_trailing_and_corruption() {
        let encoded = full_v3_claim().encode_canonical().unwrap();
        assert_eq!(
            DurablePublishedHsacoClaimV3::decode_canonical(&encoded[..encoded.len() - 1]),
            Err(DurablePublishedClaimCodecErrorV3::Truncated)
        );

        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            DurablePublishedHsacoClaimV3::decode_canonical(&trailing),
            Err(DurablePublishedClaimCodecErrorV3::TrailingBytes)
        );

        let mut corrupt = encoded;
        let last = corrupt.len() - 1;
        corrupt[last] ^= 1;
        assert_eq!(
            DurablePublishedHsacoClaimV3::decode_canonical(&corrupt),
            Err(DurablePublishedClaimCodecErrorV3::ChecksumMismatch)
        );
    }

    #[test]
    fn v3_claim_encoding_uses_fallible_exact_reservation_without_changing_bytes() {
        let claim = full_v3_claim();
        let encoded = claim.encode_canonical().unwrap();
        assert_eq!(encoded.len(), CLAIM_CANONICAL_BYTES_V3);
        assert_eq!(encoded, claim.encode_canonical().unwrap());
        assert_eq!(
            claim_encoding_vec_v3(usize::MAX),
            Err(DurablePublishedClaimCodecErrorV3::AllocationFailed {
                requested: usize::MAX,
            })
        );
    }

    #[test]
    fn v3_claim_binds_finalized_length_to_the_published_file() {
        let claim = full_v3_claim();
        let mismatched = DurablePublishedHsacoClaimV3 {
            files: DurablePublishedFileBindingV1 {
                artifact_length: claim.files.artifact_length + 1,
                ..claim.files
            },
            ..claim
        };
        assert_eq!(
            mismatched.validate(),
            Err(DurablePublishedClaimCodecErrorV3::WorkerV3BindingMismatch {
                field: DurablePublishedClaimWorkerV3BindingFieldV1::ArtifactLength,
            })
        );
    }
}
