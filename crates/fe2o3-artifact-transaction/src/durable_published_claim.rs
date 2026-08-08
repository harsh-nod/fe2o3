//! Cross-process reacquisition of one exact durable publication.
//!
//! A claim is inert, cloneable coordination data. It binds the complete publication plan and
//! backend receipt to the output directory, canonical record, and content-addressed artifact
//! identities observed when publication completed. It contains no descriptor, lock, lease, load
//! authority, or launch authority.
//!
//! Reacquisition is deliberately local. It validates the current filesystem and attempt registry
//! under the cooperative output lock, but it does not detect rollback of that complete local state.

use super::attempt::{AttemptPhase, BackendReceiptV1};
use super::attempt_scoped_hsaco_publication::{
    producer_receipt_identity_v1, publication_receipt_for_producer_identity,
};
use super::durable_link_publication::{
    DurableCurrentLinkPublicationLeaseV1, DurableFileIdentityV1, DurableLinkPublicationError,
    DurableLinkPublicationPlanV1, DurablePublishedFileBindingV1,
    MAX_DURABLE_FINALIZED_ARTIFACT_BYTES, reacquire_current_publication_lease_locked,
};
use super::{
    AtomicPublicationIdentityV1, BackendPublicationReceiptV1, BuildAttempt, BuildInvocation,
    BuildSession, CanonicalLinkRequestIdentityV1, EmitError, FinalizationIdentityV1,
    FinalizedOutputIdentityV1, KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1,
    PackageIdentityV1, PinnedOutput, PinnedWorkerIdentityV1, TargetIdentityV1,
    UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1, read_attempt_registry,
};
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::Path;

const CLAIM_MAGIC: &[u8] = b"FE2O3-PUBLISHED-HSACO-CLAIM-V1\0";
const CLAIM_VERSION: u16 = 1;
const CLAIM_CHECKSUM_DOMAIN: &[u8] = b"fe2o3.published-hsaco-claim.checksum.v1\0";
const CLAIM_FIXED_BODY_BYTES: usize =
    CLAIM_MAGIC.len() + 2 + 8 + 16 + 32 + 3 * 32 + 7 * 32 + 32 + 7 * 32 + 7 * 8;
const CLAIM_CANONICAL_BYTES: usize = CLAIM_FIXED_BODY_BYTES + 32;

/// Maximum accepted wire size for one durable published-HSACO claim.
pub const MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES: usize = 1_024;

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
    claim
        .validate()
        .map_err(DurablePublishedClaimReacquisitionErrorV1::InvalidClaim)?;
    let output = PinnedOutput::open_existing(output_dir)
        .map_err(DurablePublishedClaimReacquisitionErrorV1::Filesystem)?;
    let lock = output
        .try_lock()
        .map_err(DurablePublishedClaimReacquisitionErrorV1::Filesystem)?
        .ok_or(DurablePublishedClaimReacquisitionErrorV1::Busy)?;
    output
        .verify_path_identity()
        .map_err(DurablePublishedClaimReacquisitionErrorV1::Filesystem)?;
    validate_persisted_receipt(&output, claim)?;
    let lease = reacquire_current_publication_lease_locked(&output, claim.plan, claim.files)
        .map_err(DurablePublishedClaimReacquisitionErrorV1::Publication)?;
    validate_persisted_receipt(&output, claim)?;
    drop(lock);
    Ok(lease)
}

fn validate_persisted_receipt(
    output: &PinnedOutput,
    claim: &DurablePublishedHsacoClaimV1,
) -> Result<(), DurablePublishedClaimReacquisitionErrorV1> {
    let attempts = read_attempt_registry(output)
        .map_err(DurablePublishedClaimReacquisitionErrorV1::Filesystem)?;
    let (stable_source, record) = attempts
        .record_for_attempt(claim.plan.attempt())
        .ok_or(DurablePublishedClaimReacquisitionErrorV1::AttemptNotFound)?;
    if !matches!(
        record.phase,
        AttemptPhase::BackendClaimed | AttemptPhase::Completed
    ) {
        return Err(DurablePublishedClaimReacquisitionErrorV1::AttemptState);
    }
    match record.backend_receipt {
        Some(BackendReceiptV1::Provenance(receipt)) if receipt == claim.receipt => {}
        _ => return Err(DurablePublishedClaimReacquisitionErrorV1::ReceiptMismatch),
    }
    if producer_receipt_identity_v1(stable_source, &record.crate_name)
        != claim.receipt.producer_identity()
    {
        return Err(DurablePublishedClaimReacquisitionErrorV1::ProducerIdentityMismatch);
    }
    Ok(())
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
