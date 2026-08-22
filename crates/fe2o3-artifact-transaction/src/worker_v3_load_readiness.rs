//! Durable custody for one exact opaque Worker V3 load envelope.
//!
//! This module deliberately knows nothing about the load-envelope schema, the worker bundle, or
//! COMGR. An unsafe boundary attests only that the supplied opaque envelope bytes, together with
//! the exact current V3 publication named by the accompanying durable claim, retain every compact
//! replay preimage. Their joint durable custody permits retirement of the duplicate V3 publication
//! intent. The terminal receipt does not authenticate descriptor-source evidence, perform semantic
//! load admission, establish HSA readiness, or grant load or launch authority.

use crate::attempt::{AttemptPhase, BackendReceiptV1};
use crate::durable_published_claim::validate_current_hsaco_publication_locked_v3;
use crate::{
    AttemptCodecError, BackendPublicationReceiptV3, BuildAttempt,
    DurablePublishedClaimCodecErrorV3, DurablePublishedClaimReacquisitionErrorV3,
    DurablePublishedHsacoClaimV3, EmitError, MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V3,
    MAX_OUTPUT_ENTRIES, PinnedOutput, commit_attempt_registry_direct, read_attempt_registry,
};
use rustix::fs::{
    AtFlags, FileType, Mode, OFlags, RenameFlags, fstat, fsync, openat, renameat_with, statat,
    unlinkat,
};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const BINDING_MAGIC_V1: &[u8] = b"FE2O3-WORKER-V3-LOAD-ENVELOPE-BINDING-V1\0";
const BINDING_VERSION_V1: u16 = 1;
const BINDING_CHECKSUM_DOMAIN_V1: &[u8] = b"fe2o3.worker-v3-load-envelope-binding.checksum.v1\0";
const BINDING_BODY_BYTES_V1: usize = BINDING_MAGIC_V1.len() + 2 + 32 + 8;
const BINDING_BYTES_V1: usize = BINDING_BODY_BYTES_V1 + 32;

const RECEIPT_MAGIC_V1: &[u8] = b"FE2O3-WORKER-V3-LOAD-READINESS-RECEIPT-V1\0";
const RECEIPT_VERSION_V1: u16 = 1;
const RECEIPT_CHECKSUM_DOMAIN_V1: &[u8] = b"fe2o3.worker-v3-load-readiness-receipt.checksum.v1\0";
const RECEIPT_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.worker-v3-load-readiness-receipt.identity.v1\0";
const BACKEND_RECEIPT_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.worker-v3-load-readiness.backend-receipt.v1\0";
const NAMESPACE_KEY_DOMAIN_V1: &[u8] = b"fe2o3.worker-v3-load-readiness.namespace-key.v1\0";
const ATTEMPT_BYTES: usize = 8 + 16 + 32;
const RECEIPT_BODY_BYTES_V1: usize =
    RECEIPT_MAGIC_V1.len() + 2 + ATTEMPT_BYTES + 32 + 32 + 8 + 32 + 8 + (8 * 14);

/// Exact canonical size of one terminal Worker V3 load-readiness receipt.
pub const MAX_WORKER_V3_LOAD_READINESS_RECEIPT_BYTES_V1: usize = RECEIPT_BODY_BYTES_V1 + 32;

/// Independent hard ceiling for opaque Worker V3 load-envelope bytes retained by this crate.
pub const MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1: usize = 256 * 1024 * 1024;

const FILE_PREFIX_V1: &str = ".fe2o3-worker-v3-load-readiness-v1-";
const ENVELOPE_SUFFIX_V1: &str = ".envelope";
const CLAIM_SUFFIX_V1: &str = ".claim";
const RECEIPT_SUFFIX_V1: &str = ".receipt";
const TEMP_MARKER_V1: &str = ".tmp-";
const MAX_TEMP_ATTEMPTS: u64 = 64;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

/// Schema-neutral identity of exact opaque Worker V3 load-envelope bytes.
///
/// The digest is ordinary SHA-256 over the exact bytes and is paired with their nonzero length.
/// It authenticates no schema and grants no descriptor, admission, load, or launch authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerV3LoadEnvelopeBindingV1 {
    sha256: [u8; 32],
    byte_length: u64,
}

impl WorkerV3LoadEnvelopeBindingV1 {
    /// Constructs the fixed binding from already-computed exact-byte fields.
    pub fn new(
        sha256: [u8; 32],
        byte_length: u64,
    ) -> Result<Self, WorkerV3LoadReadinessCodecErrorV1> {
        validate_envelope_length(byte_length)?;
        Ok(Self {
            sha256,
            byte_length,
        })
    }

    /// Computes ordinary SHA-256 and the exact length without interpreting the bytes.
    pub fn from_exact_bytes(bytes: &[u8]) -> Result<Self, WorkerV3LoadReadinessCodecErrorV1> {
        let byte_length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        validate_envelope_length(byte_length)?;
        Ok(Self {
            sha256: Sha256::digest(bytes).into(),
            byte_length,
        })
    }

    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_length(self) -> u64 {
        self.byte_length
    }

    pub fn encode_canonical(&self) -> Result<Vec<u8>, WorkerV3LoadReadinessCodecErrorV1> {
        validate_envelope_length(self.byte_length)?;
        let mut bytes = fallible_vec(BINDING_BYTES_V1)?;
        bytes.extend_from_slice(BINDING_MAGIC_V1);
        bytes.extend_from_slice(&BINDING_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&self.sha256);
        bytes.extend_from_slice(&self.byte_length.to_le_bytes());
        let checksum = domain_hash(BINDING_CHECKSUM_DOMAIN_V1, &bytes);
        bytes.extend_from_slice(&checksum);
        debug_assert_eq!(bytes.len(), BINDING_BYTES_V1);
        Ok(bytes)
    }

    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, WorkerV3LoadReadinessCodecErrorV1> {
        if bytes.len() != BINDING_BYTES_V1 {
            return Err(WorkerV3LoadReadinessCodecErrorV1::NoncanonicalLength {
                actual: bytes.len(),
                expected: BINDING_BYTES_V1,
            });
        }
        let (body, checksum) = bytes.split_at(BINDING_BODY_BYTES_V1);
        if domain_hash(BINDING_CHECKSUM_DOMAIN_V1, body) != checksum {
            return Err(WorkerV3LoadReadinessCodecErrorV1::ChecksumMismatch);
        }
        let mut decoder = FixedDecoder::new(body);
        if decoder.take(BINDING_MAGIC_V1.len())? != BINDING_MAGIC_V1 {
            return Err(WorkerV3LoadReadinessCodecErrorV1::MagicMismatch);
        }
        let version = decoder.u16()?;
        if version != BINDING_VERSION_V1 {
            return Err(WorkerV3LoadReadinessCodecErrorV1::UnsupportedVersion { actual: version });
        }
        let binding = Self::new(decoder.fixed()?, decoder.u64()?)?;
        if !decoder.finished() {
            return Err(WorkerV3LoadReadinessCodecErrorV1::TrailingBytes);
        }
        Ok(binding)
    }

    pub const fn authenticates_descriptor_source(self) -> bool {
        false
    }

    pub const fn grants_semantic_load_admission(self) -> bool {
        false
    }

    pub const fn establishes_hsa_readiness(self) -> bool {
        false
    }

    pub const fn grants_load_authority(self) -> bool {
        false
    }

    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

/// Move-only authority asserting replay-complete custody properties of exact envelope bytes.
///
/// This capability does not assert descriptor-source authentication, semantic load admission, HSA
/// readiness, load authority, or launch authority. Safe code cannot construct it.
#[derive(Debug)]
pub struct VerifiedWorkerV3LoadEnvelopeAuthorityV1 {
    binding: WorkerV3LoadEnvelopeBindingV1,
}

impl VerifiedWorkerV3LoadEnvelopeAuthorityV1 {
    /// Bridges an independently reconstructed exact envelope into durable custody.
    ///
    /// # Safety
    ///
    /// The caller must have verified that the exact bytes named by `binding` contain every
    /// non-artifact compact replay preimage and bind the same current V3 publication whose durable
    /// claim retains the exact finalized artifact. This assertion is only sufficient to persist
    /// that joint custody and later retire the duplicate replay files. It must not be made from
    /// native V3 structural finalization alone and does not authenticate compiler-produced
    /// descriptor-source evidence or admit the envelope for HSA loading.
    #[doc(hidden)]
    pub unsafe fn from_complete_compact_replay_preimages_unchecked(
        binding: WorkerV3LoadEnvelopeBindingV1,
    ) -> Self {
        Self { binding }
    }

    pub const fn envelope_binding(&self) -> WorkerV3LoadEnvelopeBindingV1 {
        self.binding
    }

    pub const fn authenticates_descriptor_source(&self) -> bool {
        false
    }

    pub const fn grants_semantic_load_admission(&self) -> bool {
        false
    }

    pub const fn establishes_hsa_readiness(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ExactFileIdentityV1 {
    device: u64,
    inode: u64,
}

/// Terminal fixed receipt proving durable custody of one exact opaque envelope file.
///
/// The receipt is inert coordination evidence. Its only positive consequence is that exact
/// durable validation can authorize deletion of the duplicate compact replay-intent files.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkerV3LoadReadinessReceiptV1 {
    attempt: BuildAttempt,
    backend_receipt_sha256: [u8; 32],
    envelope: WorkerV3LoadEnvelopeBindingV1,
    durable_claim_sha256: [u8; 32],
    durable_claim_length: u64,
    output_directory: ExactFileIdentityV1,
    envelope_file: ExactFileIdentityV1,
    envelope_mtime_seconds: i64,
    envelope_mtime_nanoseconds: u64,
    envelope_ctime_seconds: i64,
    envelope_ctime_nanoseconds: u64,
    durable_claim_file: ExactFileIdentityV1,
    durable_claim_mtime_seconds: i64,
    durable_claim_mtime_nanoseconds: u64,
    durable_claim_ctime_seconds: i64,
    durable_claim_ctime_nanoseconds: u64,
}

impl WorkerV3LoadReadinessReceiptV1 {
    pub const fn attempt(self) -> BuildAttempt {
        self.attempt
    }

    pub const fn envelope_binding(self) -> WorkerV3LoadEnvelopeBindingV1 {
        self.envelope
    }

    pub const fn output_directory_device(self) -> u64 {
        self.output_directory.device
    }

    pub const fn output_directory_inode(self) -> u64 {
        self.output_directory.inode
    }

    pub const fn envelope_file_device(self) -> u64 {
        self.envelope_file.device
    }

    pub const fn envelope_file_inode(self) -> u64 {
        self.envelope_file.inode
    }

    pub const fn envelope_file_mtime(self) -> (i64, u64) {
        (self.envelope_mtime_seconds, self.envelope_mtime_nanoseconds)
    }

    pub const fn envelope_file_ctime(self) -> (i64, u64) {
        (self.envelope_ctime_seconds, self.envelope_ctime_nanoseconds)
    }

    pub fn identity(self) -> Result<[u8; 32], WorkerV3LoadReadinessCodecErrorV1> {
        Ok(domain_hash(
            RECEIPT_IDENTITY_DOMAIN_V1,
            &self.encode_canonical()?,
        ))
    }

    pub fn matches_backend_receipt(
        self,
        receipt: BackendPublicationReceiptV3,
    ) -> Result<bool, WorkerV3LoadReadinessCodecErrorV1> {
        Ok(self.backend_receipt_sha256 == backend_receipt_identity(receipt)?)
    }

    pub fn matches_durable_claim(
        self,
        claim: &DurablePublishedHsacoClaimV3,
    ) -> Result<bool, WorkerV3LoadReadinessErrorV1> {
        let (sha256, length) = durable_claim_binding(claim)?;
        Ok(self.durable_claim_sha256 == sha256 && self.durable_claim_length == length)
    }

    pub fn encode_canonical(&self) -> Result<Vec<u8>, WorkerV3LoadReadinessCodecErrorV1> {
        validate_envelope_length(self.envelope.byte_length)?;
        if self.durable_claim_length == 0 {
            return Err(WorkerV3LoadReadinessCodecErrorV1::InvalidDurableClaimLength);
        }
        let mut bytes = fallible_vec(MAX_WORKER_V3_LOAD_READINESS_RECEIPT_BYTES_V1)?;
        bytes.extend_from_slice(RECEIPT_MAGIC_V1);
        bytes.extend_from_slice(&RECEIPT_VERSION_V1.to_le_bytes());
        push_attempt(&mut bytes, self.attempt);
        bytes.extend_from_slice(&self.backend_receipt_sha256);
        bytes.extend_from_slice(&self.envelope.sha256);
        bytes.extend_from_slice(&self.envelope.byte_length.to_le_bytes());
        bytes.extend_from_slice(&self.durable_claim_sha256);
        bytes.extend_from_slice(&self.durable_claim_length.to_le_bytes());
        for value in [
            self.output_directory.device,
            self.output_directory.inode,
            self.envelope_file.device,
            self.envelope_file.inode,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&self.envelope_mtime_seconds.to_le_bytes());
        bytes.extend_from_slice(&self.envelope_mtime_nanoseconds.to_le_bytes());
        bytes.extend_from_slice(&self.envelope_ctime_seconds.to_le_bytes());
        bytes.extend_from_slice(&self.envelope_ctime_nanoseconds.to_le_bytes());
        for value in [
            self.durable_claim_file.device,
            self.durable_claim_file.inode,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&self.durable_claim_mtime_seconds.to_le_bytes());
        bytes.extend_from_slice(&self.durable_claim_mtime_nanoseconds.to_le_bytes());
        bytes.extend_from_slice(&self.durable_claim_ctime_seconds.to_le_bytes());
        bytes.extend_from_slice(&self.durable_claim_ctime_nanoseconds.to_le_bytes());
        let checksum = domain_hash(RECEIPT_CHECKSUM_DOMAIN_V1, &bytes);
        bytes.extend_from_slice(&checksum);
        debug_assert_eq!(bytes.len(), MAX_WORKER_V3_LOAD_READINESS_RECEIPT_BYTES_V1);
        Ok(bytes)
    }

    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, WorkerV3LoadReadinessCodecErrorV1> {
        if bytes.len() != MAX_WORKER_V3_LOAD_READINESS_RECEIPT_BYTES_V1 {
            return Err(WorkerV3LoadReadinessCodecErrorV1::NoncanonicalLength {
                actual: bytes.len(),
                expected: MAX_WORKER_V3_LOAD_READINESS_RECEIPT_BYTES_V1,
            });
        }
        let (body, checksum) = bytes.split_at(RECEIPT_BODY_BYTES_V1);
        if domain_hash(RECEIPT_CHECKSUM_DOMAIN_V1, body) != checksum {
            return Err(WorkerV3LoadReadinessCodecErrorV1::ChecksumMismatch);
        }
        let mut decoder = FixedDecoder::new(body);
        if decoder.take(RECEIPT_MAGIC_V1.len())? != RECEIPT_MAGIC_V1 {
            return Err(WorkerV3LoadReadinessCodecErrorV1::MagicMismatch);
        }
        let version = decoder.u16()?;
        if version != RECEIPT_VERSION_V1 {
            return Err(WorkerV3LoadReadinessCodecErrorV1::UnsupportedVersion { actual: version });
        }
        let attempt = decoder.attempt()?;
        let backend_receipt_sha256 = decoder.fixed()?;
        let envelope = WorkerV3LoadEnvelopeBindingV1::new(decoder.fixed()?, decoder.u64()?)?;
        let durable_claim_sha256 = decoder.fixed()?;
        let durable_claim_length = decoder.u64()?;
        if durable_claim_length == 0 {
            return Err(WorkerV3LoadReadinessCodecErrorV1::InvalidDurableClaimLength);
        }
        let output_directory = ExactFileIdentityV1 {
            device: decoder.u64()?,
            inode: decoder.u64()?,
        };
        let envelope_file = ExactFileIdentityV1 {
            device: decoder.u64()?,
            inode: decoder.u64()?,
        };
        let envelope_mtime_seconds = decoder.i64()?;
        let envelope_mtime_nanoseconds = decoder.u64()?;
        let envelope_ctime_seconds = decoder.i64()?;
        let envelope_ctime_nanoseconds = decoder.u64()?;
        let durable_claim_file = ExactFileIdentityV1 {
            device: decoder.u64()?,
            inode: decoder.u64()?,
        };
        let durable_claim_mtime_seconds = decoder.i64()?;
        let durable_claim_mtime_nanoseconds = decoder.u64()?;
        let durable_claim_ctime_seconds = decoder.i64()?;
        let durable_claim_ctime_nanoseconds = decoder.u64()?;
        if !decoder.finished() {
            return Err(WorkerV3LoadReadinessCodecErrorV1::TrailingBytes);
        }
        Ok(Self {
            attempt,
            backend_receipt_sha256,
            envelope,
            durable_claim_sha256,
            durable_claim_length,
            output_directory,
            envelope_file,
            envelope_mtime_seconds,
            envelope_mtime_nanoseconds,
            envelope_ctime_seconds,
            envelope_ctime_nanoseconds,
            durable_claim_file,
            durable_claim_mtime_seconds,
            durable_claim_mtime_nanoseconds,
            durable_claim_ctime_seconds,
            durable_claim_ctime_nanoseconds,
        })
    }

    /// The inert receipt alone is not retirement authority; the retirement API must revalidate it.
    pub const fn grants_replay_intent_retirement_authority(self) -> bool {
        false
    }

    /// The receipt is evidence accepted only after exact durable registry and file revalidation.
    pub const fn is_replay_intent_retirement_evidence(self) -> bool {
        true
    }

    pub const fn authenticates_descriptor_source(self) -> bool {
        false
    }

    pub const fn grants_semantic_load_admission(self) -> bool {
        false
    }

    pub const fn establishes_hsa_readiness(self) -> bool {
        false
    }

    pub const fn grants_load_authority(self) -> bool {
        false
    }

    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

/// Strict fixed-codec failure for the schema-neutral binding or terminal receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3LoadReadinessCodecErrorV1 {
    NoncanonicalLength { actual: usize, expected: usize },
    MagicMismatch,
    UnsupportedVersion { actual: u16 },
    ChecksumMismatch,
    InvalidEnvelopeLength { actual: u64, maximum: usize },
    InvalidDurableClaimLength,
    InvalidAttempt,
    Truncated,
    TrailingBytes,
    AllocationFailed { requested: usize },
}

impl fmt::Display for WorkerV3LoadReadinessCodecErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoncanonicalLength { actual, expected } => write!(
                formatter,
                "Worker V3 load-readiness length {actual} is not canonical {expected}"
            ),
            Self::MagicMismatch => formatter.write_str("Worker V3 load-readiness magic mismatch"),
            Self::UnsupportedVersion { actual } => {
                write!(
                    formatter,
                    "unsupported Worker V3 load-readiness version {actual}"
                )
            }
            Self::ChecksumMismatch => {
                formatter.write_str("Worker V3 load-readiness checksum mismatch")
            }
            Self::InvalidEnvelopeLength { actual, maximum } => write!(
                formatter,
                "Worker V3 load-envelope length {actual} is outside 1..={maximum}"
            ),
            Self::InvalidDurableClaimLength => {
                formatter.write_str("Worker V3 durable-claim binding has zero length")
            }
            Self::InvalidAttempt => {
                formatter.write_str("invalid Worker V3 load-readiness build attempt")
            }
            Self::Truncated => formatter.write_str("truncated Worker V3 load-readiness field"),
            Self::TrailingBytes => formatter.write_str("trailing Worker V3 load-readiness bytes"),
            Self::AllocationFailed { requested } => write!(
                formatter,
                "Worker V3 load-readiness allocation of {requested} bytes failed"
            ),
        }
    }
}

impl std::error::Error for WorkerV3LoadReadinessCodecErrorV1 {}

/// Durable protocol boundary available to focused crash testing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3LoadReadinessBoundaryV1 {
    CreateEnvelopeTemp,
    WriteEnvelopeTemp,
    SyncEnvelopeTemp,
    RenameEnvelope,
    SyncEnvelopeName,
    CreateClaimTemp,
    WriteClaimTemp,
    SyncClaimTemp,
    RenameClaim,
    SyncClaimName,
    CreateReceiptTemp,
    WriteReceiptTemp,
    SyncReceiptTemp,
    RenameReceipt,
    SyncReceiptName,
    CommitAttemptRegistry,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerV3LoadReadinessFaultTimingV1 {
    Before,
    After,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3LoadReadinessFaultPointV1 {
    pub boundary: WorkerV3LoadReadinessBoundaryV1,
    pub timing: WorkerV3LoadReadinessFaultTimingV1,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkerV3LoadReadinessOptionsV1 {
    pub injected_crash: Option<WorkerV3LoadReadinessFaultPointV1>,
}

impl WorkerV3LoadReadinessOptionsV1 {
    pub const fn inject_crash(point: WorkerV3LoadReadinessFaultPointV1) -> Self {
        Self {
            injected_crash: Some(point),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerV3LoadReadinessOutcomeV1 {
    Published,
    Recovered,
}

#[derive(Debug, Eq, PartialEq)]
pub struct WorkerV3LoadReadinessResultV1 {
    outcome: WorkerV3LoadReadinessOutcomeV1,
    receipt: WorkerV3LoadReadinessReceiptV1,
    claim: DurablePublishedHsacoClaimV3,
    exact_envelope: Vec<u8>,
    envelope_path: PathBuf,
}

impl WorkerV3LoadReadinessResultV1 {
    pub const fn outcome(&self) -> WorkerV3LoadReadinessOutcomeV1 {
        self.outcome
    }

    pub const fn receipt(&self) -> WorkerV3LoadReadinessReceiptV1 {
        self.receipt
    }

    pub const fn published_claim(&self) -> &DurablePublishedHsacoClaimV3 {
        &self.claim
    }

    /// Returns exact bytes read from the revalidated pinned envelope file.
    pub fn exact_envelope_bytes(&self) -> &[u8] {
        &self.exact_envelope
    }

    /// Consumes the custody result and transfers ownership of the exact validated envelope bytes.
    pub fn into_exact_envelope_bytes(self) -> Vec<u8> {
        self.exact_envelope
    }

    /// Diagnostic path only. Durable validation always uses the pinned directory and exact inode.
    pub fn envelope_path(&self) -> &Path {
        &self.envelope_path
    }

    pub const fn authenticates_descriptor_source(&self) -> bool {
        false
    }

    pub const fn grants_semantic_load_admission(&self) -> bool {
        false
    }

    pub const fn establishes_hsa_readiness(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Fail-closed durable publication or recovery failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3LoadReadinessErrorV1 {
    Codec(WorkerV3LoadReadinessCodecErrorV1),
    ClaimCodec(DurablePublishedClaimCodecErrorV3),
    Claim(DurablePublishedClaimReacquisitionErrorV3),
    Store(EmitError),
    Io(std::io::Error),
    Attempt(AttemptCodecError),
    AuthorityMismatch,
    EnvelopeCapacityExceeded { actual: usize, maximum: usize },
    AttemptState,
    ReceiptMismatch,
    EnvelopeMismatch,
    MissingEnvelope,
    MissingClaim,
    MissingReceipt,
    InvalidPrivateEntry { entry: PathBuf },
    FileChanged { entry: PathBuf },
    DirectoryEntryLimitExceeded { maximum: usize },
    TemporaryEntryLimitExceeded { maximum: usize },
    TemporaryNameExhausted,
    InjectedCrash(WorkerV3LoadReadinessFaultPointV1),
}

impl fmt::Display for WorkerV3LoadReadinessErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(error) => write!(formatter, "invalid Worker V3 load readiness: {error}"),
            Self::ClaimCodec(error) => {
                write!(formatter, "invalid Worker V3 durable claim: {error}")
            }
            Self::Claim(error) => write!(formatter, "Worker V3 claim is not current: {error}"),
            Self::Store(error) => {
                write!(formatter, "Worker V3 load-readiness store failure: {error}")
            }
            Self::Io(error) => write!(formatter, "Worker V3 load-readiness I/O failure: {error}"),
            Self::Attempt(error) => write!(
                formatter,
                "Worker V3 load-readiness attempt failure: {error}"
            ),
            Self::AuthorityMismatch => formatter.write_str(
                "Worker V3 load-envelope authority does not name the supplied exact bytes",
            ),
            Self::EnvelopeCapacityExceeded { actual, maximum } => write!(
                formatter,
                "Worker V3 load-envelope owner capacity {actual} exceeds {maximum}"
            ),
            Self::AttemptState => formatter.write_str(
                "Worker V3 load readiness requires exact completed V3 provenance or readiness",
            ),
            Self::ReceiptMismatch => {
                formatter.write_str("durable Worker V3 load-readiness receipt does not match")
            }
            Self::EnvelopeMismatch => {
                formatter.write_str("durable Worker V3 load envelope does not match")
            }
            Self::MissingEnvelope => {
                formatter.write_str("durable Worker V3 load envelope is missing")
            }
            Self::MissingClaim => {
                formatter.write_str("durable Worker V3 published claim is missing")
            }
            Self::MissingReceipt => {
                formatter.write_str("durable Worker V3 load-readiness receipt is missing")
            }
            Self::InvalidPrivateEntry { entry } => write!(
                formatter,
                "Worker V3 load-readiness entry is not a private single-link file: {}",
                entry.display()
            ),
            Self::FileChanged { entry } => write!(
                formatter,
                "Worker V3 load-readiness entry changed while pinned: {}",
                entry.display()
            ),
            Self::DirectoryEntryLimitExceeded { maximum } => write!(
                formatter,
                "Worker V3 load-readiness directory exceeds {maximum} entries"
            ),
            Self::TemporaryEntryLimitExceeded { maximum } => write!(
                formatter,
                "Worker V3 load-readiness namespace exceeds {maximum} temporary entries"
            ),
            Self::TemporaryNameExhausted => {
                formatter.write_str("Worker V3 load-readiness temporary names exhausted")
            }
            Self::InjectedCrash(point) => write!(
                formatter,
                "injected Worker V3 load-readiness crash at {:?} {:?}",
                point.boundary, point.timing
            ),
        }
    }
}

impl std::error::Error for WorkerV3LoadReadinessErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Codec(error) => Some(error),
            Self::ClaimCodec(error) => Some(error),
            Self::Claim(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::Attempt(error) => Some(error),
            _ => None,
        }
    }
}

impl From<WorkerV3LoadReadinessCodecErrorV1> for WorkerV3LoadReadinessErrorV1 {
    fn from(error: WorkerV3LoadReadinessCodecErrorV1) -> Self {
        Self::Codec(error)
    }
}

impl From<DurablePublishedClaimCodecErrorV3> for WorkerV3LoadReadinessErrorV1 {
    fn from(error: DurablePublishedClaimCodecErrorV3) -> Self {
        Self::ClaimCodec(error)
    }
}

impl From<EmitError> for WorkerV3LoadReadinessErrorV1 {
    fn from(error: EmitError) -> Self {
        Self::Store(error)
    }
}

impl From<std::io::Error> for WorkerV3LoadReadinessErrorV1 {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<AttemptCodecError> for WorkerV3LoadReadinessErrorV1 {
    fn from(error: AttemptCodecError) -> Self {
        Self::Attempt(error)
    }
}

/// Publishes exact opaque envelope bytes and terminally records durable replay custody.
pub fn publish_worker_v3_load_readiness_v1(
    output_dir: &Path,
    claim: &DurablePublishedHsacoClaimV3,
    authority: VerifiedWorkerV3LoadEnvelopeAuthorityV1,
    exact_envelope: Vec<u8>,
) -> Result<WorkerV3LoadReadinessResultV1, WorkerV3LoadReadinessErrorV1> {
    publish_worker_v3_load_readiness_v1_with_options(
        output_dir,
        claim,
        authority,
        exact_envelope,
        WorkerV3LoadReadinessOptionsV1::default(),
    )
}

/// Fault-injectable form of [`publish_worker_v3_load_readiness_v1`].
pub fn publish_worker_v3_load_readiness_v1_with_options(
    output_dir: &Path,
    claim: &DurablePublishedHsacoClaimV3,
    authority: VerifiedWorkerV3LoadEnvelopeAuthorityV1,
    exact_envelope: Vec<u8>,
    options: WorkerV3LoadReadinessOptionsV1,
) -> Result<WorkerV3LoadReadinessResultV1, WorkerV3LoadReadinessErrorV1> {
    if exact_envelope.capacity() > MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1 {
        return Err(WorkerV3LoadReadinessErrorV1::EnvelopeCapacityExceeded {
            actual: exact_envelope.capacity(),
            maximum: MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1,
        });
    }
    let envelope = WorkerV3LoadEnvelopeBindingV1::from_exact_bytes(&exact_envelope)?;
    if authority.envelope_binding() != envelope {
        return Err(WorkerV3LoadReadinessErrorV1::AuthorityMismatch);
    }

    let backend = claim.receipt();
    let names = ReadinessNames::new(backend)?;
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    let _publication = validate_current_hsaco_publication_locked_v3(&output, claim)
        .map_err(WorkerV3LoadReadinessErrorV1::Claim)?;
    let mut faults = FaultInjector::new(options.injected_crash);
    let output_entry_count = cleanup_temps(&output, &names)?;

    let (stable_source, registry_state) =
        exact_registry_state(&output, claim.plan().attempt(), backend)?;
    let receipt_present = private_snapshot_optional(&output, &names.receipt)?.is_some();
    let envelope_present = private_snapshot_optional(&output, &names.envelope)?.is_some();
    let claim_present = private_snapshot_optional(&output, &names.claim)?.is_some();

    if let RegistryReadinessState::EnvelopeCustody(durable_readiness) = registry_state {
        if !receipt_present {
            return Err(WorkerV3LoadReadinessErrorV1::MissingReceipt);
        }
        if !envelope_present {
            return Err(WorkerV3LoadReadinessErrorV1::MissingEnvelope);
        }
        if !claim_present {
            return Err(WorkerV3LoadReadinessErrorV1::MissingClaim);
        }
        let receipt = validate_terminal_receipt(
            &output,
            &names,
            claim.plan().attempt(),
            backend,
            Some(envelope),
        )?;
        if receipt != durable_readiness || !receipt.matches_durable_claim(claim)? {
            return Err(WorkerV3LoadReadinessErrorV1::ReceiptMismatch);
        }
        compare_exact_envelope(&output, &names.envelope, &exact_envelope, receipt)?;
        let _publication = validate_current_hsaco_publication_locked_v3(&output, claim)
            .map_err(WorkerV3LoadReadinessErrorV1::Claim)?;
        return result(
            &output,
            &names,
            WorkerV3LoadReadinessOutcomeV1::Recovered,
            receipt,
        );
    }

    ensure_publication_headroom(
        output_entry_count,
        envelope_present,
        claim_present,
        receipt_present,
    )?;

    let receipt = if receipt_present {
        if !envelope_present {
            return Err(WorkerV3LoadReadinessErrorV1::MissingEnvelope);
        }
        if !claim_present {
            return Err(WorkerV3LoadReadinessErrorV1::MissingClaim);
        }
        let receipt = validate_terminal_receipt(
            &output,
            &names,
            claim.plan().attempt(),
            backend,
            Some(envelope),
        )?;
        if !receipt.matches_durable_claim(claim)? {
            return Err(WorkerV3LoadReadinessErrorV1::ReceiptMismatch);
        }
        compare_exact_envelope(&output, &names.envelope, &exact_envelope, receipt)?;
        receipt
    } else {
        let envelope_snapshot = if envelope_present {
            let snapshot = private_snapshot_required(&output, &names.envelope)?;
            compare_bytes_and_snapshot(&output, &names.envelope, &exact_envelope, snapshot)?;
            snapshot
        } else {
            persist_envelope(&output, &names, &exact_envelope, &mut faults)?
        };
        let claim_bytes = claim.encode_canonical()?;
        let claim_snapshot = if claim_present {
            let snapshot = private_snapshot_required(&output, &names.claim)?;
            compare_bytes_and_snapshot(&output, &names.claim, &claim_bytes, snapshot)?;
            snapshot
        } else {
            persist_claim(&output, &names, &claim_bytes, &mut faults)?
        };
        let receipt = make_receipt(
            &output,
            claim,
            claim.plan().attempt(),
            backend,
            envelope,
            envelope_snapshot,
            claim_snapshot,
        )?;
        persist_receipt(&output, &names, receipt, &mut faults)?;
        receipt
    };

    let _publication = validate_current_hsaco_publication_locked_v3(&output, claim)
        .map_err(WorkerV3LoadReadinessErrorV1::Claim)?;
    faults.hit(
        WorkerV3LoadReadinessBoundaryV1::CommitAttemptRegistry,
        WorkerV3LoadReadinessFaultTimingV1::Before,
    )?;
    let mut attempts = read_attempt_registry(&output)?;
    attempts.record_worker_v3_load_readiness(
        &stable_source,
        claim.plan().attempt(),
        backend,
        receipt,
    )?;
    commit_attempt_registry_direct(&output, &attempts)?;
    faults.hit(
        WorkerV3LoadReadinessBoundaryV1::CommitAttemptRegistry,
        WorkerV3LoadReadinessFaultTimingV1::After,
    )?;
    validate_durable_worker_v3_load_readiness_locked_v1(&output, backend, receipt)?;
    let _publication = validate_current_hsaco_publication_locked_v3(&output, claim)
        .map_err(WorkerV3LoadReadinessErrorV1::Claim)?;
    result(
        &output,
        &names,
        WorkerV3LoadReadinessOutcomeV1::Published,
        receipt,
    )
}

/// Revalidates exact terminal readiness after process restart without granting loading authority.
pub fn recover_worker_v3_load_readiness_v1(
    output_dir: &Path,
    claim: &DurablePublishedHsacoClaimV3,
) -> Result<WorkerV3LoadReadinessResultV1, WorkerV3LoadReadinessErrorV1> {
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    let _publication = validate_current_hsaco_publication_locked_v3(&output, claim)
        .map_err(WorkerV3LoadReadinessErrorV1::Claim)?;
    let names = ReadinessNames::new(claim.receipt())?;
    cleanup_temps(&output, &names)?;
    let (_, state) = exact_registry_state(&output, claim.plan().attempt(), claim.receipt())?;
    let RegistryReadinessState::EnvelopeCustody(receipt) = state else {
        return Err(WorkerV3LoadReadinessErrorV1::AttemptState);
    };
    validate_durable_worker_v3_load_readiness_locked_v1(&output, claim.receipt(), receipt)?;
    if !receipt.matches_durable_claim(claim)? {
        return Err(WorkerV3LoadReadinessErrorV1::ReceiptMismatch);
    }
    let _publication = validate_current_hsaco_publication_locked_v3(&output, claim)
        .map_err(WorkerV3LoadReadinessErrorV1::Claim)?;
    result(
        &output,
        &names,
        WorkerV3LoadReadinessOutcomeV1::Recovered,
        receipt,
    )
}

/// Recovers terminal envelope custody using only the output directory and exact build attempt.
///
/// The registry selects one backend receipt; its terminal receipt authenticates a canonical claim
/// sidecar, which then re-enters the ordinary exact-claim recovery path. No envelope schema is
/// decoded here and no semantic, load, or launch authority is granted.
pub fn recover_worker_v3_load_readiness_for_attempt_v1(
    output_dir: &Path,
    attempt: BuildAttempt,
) -> Result<WorkerV3LoadReadinessResultV1, WorkerV3LoadReadinessErrorV1> {
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    let attempts = read_attempt_registry(&output)?;
    let (_, record) = attempts
        .record_for_attempt(attempt)
        .ok_or(WorkerV3LoadReadinessErrorV1::AttemptState)?;
    let Some(BackendReceiptV1::EnvelopeCustodyV3(backend, expected)) = record.backend_receipt
    else {
        return Err(WorkerV3LoadReadinessErrorV1::AttemptState);
    };
    let names = ReadinessNames::new(backend)?;
    cleanup_temps(&output, &names)?;
    let actual = validate_terminal_receipt(&output, &names, attempt, backend, None)?;
    if actual != expected {
        return Err(WorkerV3LoadReadinessErrorV1::ReceiptMismatch);
    }
    let claim = read_validated_claim_file(&output, &names, actual)?;
    if claim.plan().attempt() != attempt
        || claim.receipt() != backend
        || !actual.matches_durable_claim(&claim)?
    {
        return Err(WorkerV3LoadReadinessErrorV1::ReceiptMismatch);
    }
    let _publication = validate_current_hsaco_publication_locked_v3(&output, &claim)
        .map_err(WorkerV3LoadReadinessErrorV1::Claim)?;
    result(
        &output,
        &names,
        WorkerV3LoadReadinessOutcomeV1::Recovered,
        actual,
    )
}

/// Validates the exact registry tuple, terminal receipt inode, and exact envelope inode and bytes.
pub(crate) fn validate_durable_worker_v3_load_readiness_locked_v1(
    output: &PinnedOutput,
    backend: BackendPublicationReceiptV3,
    expected: WorkerV3LoadReadinessReceiptV1,
) -> Result<(), WorkerV3LoadReadinessErrorV1> {
    output.verify_path_identity()?;
    if !expected.matches_backend_receipt(backend)? {
        return Err(WorkerV3LoadReadinessErrorV1::ReceiptMismatch);
    }
    let names = ReadinessNames::new(backend)?;
    let (_, state) = exact_registry_state(output, expected.attempt(), backend)?;
    if !matches!(state, RegistryReadinessState::EnvelopeCustody(actual) if actual == expected) {
        return Err(WorkerV3LoadReadinessErrorV1::ReceiptMismatch);
    }
    let actual = validate_terminal_receipt(
        output,
        &names,
        expected.attempt(),
        backend,
        Some(expected.envelope),
    )?;
    if actual != expected {
        return Err(WorkerV3LoadReadinessErrorV1::ReceiptMismatch);
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum RegistryReadinessState {
    Provenance,
    EnvelopeCustody(WorkerV3LoadReadinessReceiptV1),
}

fn exact_registry_state(
    output: &PinnedOutput,
    attempt: BuildAttempt,
    backend: BackendPublicationReceiptV3,
) -> Result<(String, RegistryReadinessState), WorkerV3LoadReadinessErrorV1> {
    let attempts = read_attempt_registry(output)?;
    let (stable_source, record) = attempts
        .record_for_attempt(attempt)
        .ok_or(WorkerV3LoadReadinessErrorV1::AttemptState)?;
    if !matches!(
        record.phase,
        AttemptPhase::BackendClaimed | AttemptPhase::Completed
    ) {
        return Err(WorkerV3LoadReadinessErrorV1::AttemptState);
    }
    let state = match record.backend_receipt {
        Some(BackendReceiptV1::ProvenanceV3(actual)) if actual == backend => {
            attempts.ensure_worker_v3_load_readiness_fits(stable_source, attempt, backend)?;
            RegistryReadinessState::Provenance
        }
        Some(BackendReceiptV1::EnvelopeCustodyV3(actual, readiness)) if actual == backend => {
            RegistryReadinessState::EnvelopeCustody(readiness)
        }
        _ => return Err(WorkerV3LoadReadinessErrorV1::AttemptState),
    };
    let mut owned_source = String::new();
    owned_source
        .try_reserve_exact(stable_source.len())
        .map_err(|_| WorkerV3LoadReadinessCodecErrorV1::AllocationFailed {
            requested: stable_source.len(),
        })?;
    owned_source.push_str(stable_source);
    Ok((owned_source, state))
}

fn make_receipt(
    output: &PinnedOutput,
    claim: &DurablePublishedHsacoClaimV3,
    attempt: BuildAttempt,
    backend: BackendPublicationReceiptV3,
    envelope: WorkerV3LoadEnvelopeBindingV1,
    envelope_snapshot: rustix::fs::Stat,
    claim_snapshot: rustix::fs::Stat,
) -> Result<WorkerV3LoadReadinessReceiptV1, WorkerV3LoadReadinessErrorV1> {
    let (durable_claim_sha256, durable_claim_length) = durable_claim_binding(claim)?;
    Ok(WorkerV3LoadReadinessReceiptV1 {
        attempt,
        backend_receipt_sha256: backend_receipt_identity(backend)?,
        envelope,
        durable_claim_sha256,
        durable_claim_length,
        output_directory: ExactFileIdentityV1 {
            device: output.device,
            inode: output.inode,
        },
        envelope_file: ExactFileIdentityV1 {
            device: envelope_snapshot.st_dev,
            inode: envelope_snapshot.st_ino,
        },
        envelope_mtime_seconds: envelope_snapshot.st_mtime,
        envelope_mtime_nanoseconds: envelope_snapshot.st_mtime_nsec,
        envelope_ctime_seconds: envelope_snapshot.st_ctime,
        envelope_ctime_nanoseconds: envelope_snapshot.st_ctime_nsec,
        durable_claim_file: ExactFileIdentityV1 {
            device: claim_snapshot.st_dev,
            inode: claim_snapshot.st_ino,
        },
        durable_claim_mtime_seconds: claim_snapshot.st_mtime,
        durable_claim_mtime_nanoseconds: claim_snapshot.st_mtime_nsec,
        durable_claim_ctime_seconds: claim_snapshot.st_ctime,
        durable_claim_ctime_nanoseconds: claim_snapshot.st_ctime_nsec,
    })
}

fn persist_envelope(
    output: &PinnedOutput,
    names: &ReadinessNames,
    bytes: &[u8],
    faults: &mut FaultInjector,
) -> Result<rustix::fs::Stat, WorkerV3LoadReadinessErrorV1> {
    let (temp, mut file) = create_temp(
        output,
        names,
        "envelope",
        WorkerV3LoadReadinessBoundaryV1::CreateEnvelopeTemp,
        faults,
    )?;
    faults.around(WorkerV3LoadReadinessBoundaryV1::WriteEnvelopeTemp, || {
        file.write_all(bytes).map_err(Into::into)
    })?;
    faults.around(WorkerV3LoadReadinessBoundaryV1::SyncEnvelopeTemp, || {
        file.sync_all().map_err(Into::into)
    })?;
    let before = fstat(&file).map_err(std::io::Error::from)?;
    if !is_private_file(&before) || usize::try_from(before.st_size).ok() != Some(bytes.len()) {
        return Err(WorkerV3LoadReadinessErrorV1::FileChanged {
            entry: PathBuf::from(&temp),
        });
    }
    faults.hit(
        WorkerV3LoadReadinessBoundaryV1::RenameEnvelope,
        WorkerV3LoadReadinessFaultTimingV1::Before,
    )?;
    output.verify_path_identity()?;
    renameat_with(
        &output.fd,
        &temp,
        &output.fd,
        &names.envelope,
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    let named = private_snapshot_required(output, &names.envelope)?;
    let descriptor = fstat(&file).map_err(std::io::Error::from)?;
    if !same_private_inode(&before, &named)
        || !same_private_inode(&before, &descriptor)
        || !same_snapshot(&named, &descriptor)
    {
        return Err(WorkerV3LoadReadinessErrorV1::FileChanged {
            entry: PathBuf::from(&names.envelope),
        });
    }
    faults.hit(
        WorkerV3LoadReadinessBoundaryV1::RenameEnvelope,
        WorkerV3LoadReadinessFaultTimingV1::After,
    )?;
    faults.around(WorkerV3LoadReadinessBoundaryV1::SyncEnvelopeName, || {
        fsync(&output.fd)
            .map_err(std::io::Error::from)
            .map_err(Into::into)
    })?;
    output.verify_path_identity()?;
    Ok(named)
}

fn persist_claim(
    output: &PinnedOutput,
    names: &ReadinessNames,
    bytes: &[u8],
    faults: &mut FaultInjector,
) -> Result<rustix::fs::Stat, WorkerV3LoadReadinessErrorV1> {
    if bytes.is_empty() || bytes.len() > MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V3 {
        return Err(WorkerV3LoadReadinessErrorV1::ReceiptMismatch);
    }
    let (temp, mut file) = create_temp(
        output,
        names,
        "claim",
        WorkerV3LoadReadinessBoundaryV1::CreateClaimTemp,
        faults,
    )?;
    faults.around(WorkerV3LoadReadinessBoundaryV1::WriteClaimTemp, || {
        file.write_all(bytes).map_err(Into::into)
    })?;
    faults.around(WorkerV3LoadReadinessBoundaryV1::SyncClaimTemp, || {
        file.sync_all().map_err(Into::into)
    })?;
    let before = fstat(&file).map_err(std::io::Error::from)?;
    if !is_private_file(&before) || usize::try_from(before.st_size).ok() != Some(bytes.len()) {
        return Err(WorkerV3LoadReadinessErrorV1::FileChanged {
            entry: PathBuf::from(&temp),
        });
    }
    faults.hit(
        WorkerV3LoadReadinessBoundaryV1::RenameClaim,
        WorkerV3LoadReadinessFaultTimingV1::Before,
    )?;
    output.verify_path_identity()?;
    renameat_with(
        &output.fd,
        &temp,
        &output.fd,
        &names.claim,
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    let named = private_snapshot_required(output, &names.claim)?;
    let descriptor = fstat(&file).map_err(std::io::Error::from)?;
    if !same_private_inode(&before, &named)
        || !same_private_inode(&before, &descriptor)
        || !same_snapshot(&named, &descriptor)
    {
        return Err(WorkerV3LoadReadinessErrorV1::FileChanged {
            entry: PathBuf::from(&names.claim),
        });
    }
    faults.hit(
        WorkerV3LoadReadinessBoundaryV1::RenameClaim,
        WorkerV3LoadReadinessFaultTimingV1::After,
    )?;
    faults.around(WorkerV3LoadReadinessBoundaryV1::SyncClaimName, || {
        fsync(&output.fd)
            .map_err(std::io::Error::from)
            .map_err(Into::into)
    })?;
    output.verify_path_identity()?;
    Ok(named)
}

fn persist_receipt(
    output: &PinnedOutput,
    names: &ReadinessNames,
    receipt: WorkerV3LoadReadinessReceiptV1,
    faults: &mut FaultInjector,
) -> Result<(), WorkerV3LoadReadinessErrorV1> {
    let bytes = receipt.encode_canonical()?;
    let (temp, mut file) = create_temp(
        output,
        names,
        "receipt",
        WorkerV3LoadReadinessBoundaryV1::CreateReceiptTemp,
        faults,
    )?;
    faults.around(WorkerV3LoadReadinessBoundaryV1::WriteReceiptTemp, || {
        file.write_all(&bytes).map_err(Into::into)
    })?;
    faults.around(WorkerV3LoadReadinessBoundaryV1::SyncReceiptTemp, || {
        file.sync_all().map_err(Into::into)
    })?;
    let before = fstat(&file).map_err(std::io::Error::from)?;
    if !is_private_file(&before)
        || usize::try_from(before.st_size).ok()
            != Some(MAX_WORKER_V3_LOAD_READINESS_RECEIPT_BYTES_V1)
    {
        return Err(WorkerV3LoadReadinessErrorV1::FileChanged {
            entry: PathBuf::from(&temp),
        });
    }
    faults.hit(
        WorkerV3LoadReadinessBoundaryV1::RenameReceipt,
        WorkerV3LoadReadinessFaultTimingV1::Before,
    )?;
    output.verify_path_identity()?;
    renameat_with(
        &output.fd,
        &temp,
        &output.fd,
        &names.receipt,
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    let named = private_snapshot_required(output, &names.receipt)?;
    let descriptor = fstat(&file).map_err(std::io::Error::from)?;
    if !same_private_inode(&before, &named)
        || !same_private_inode(&before, &descriptor)
        || !same_snapshot(&named, &descriptor)
    {
        return Err(WorkerV3LoadReadinessErrorV1::FileChanged {
            entry: PathBuf::from(&names.receipt),
        });
    }
    faults.hit(
        WorkerV3LoadReadinessBoundaryV1::RenameReceipt,
        WorkerV3LoadReadinessFaultTimingV1::After,
    )?;
    faults.around(WorkerV3LoadReadinessBoundaryV1::SyncReceiptName, || {
        fsync(&output.fd)
            .map_err(std::io::Error::from)
            .map_err(Into::into)
    })?;
    output.verify_path_identity()?;
    Ok(())
}

fn create_temp(
    output: &PinnedOutput,
    names: &ReadinessNames,
    purpose: &str,
    boundary: WorkerV3LoadReadinessBoundaryV1,
    faults: &mut FaultInjector,
) -> Result<(String, fs::File), WorkerV3LoadReadinessErrorV1> {
    let start = NEXT_TEMP_ID.fetch_add(MAX_TEMP_ATTEMPTS, Ordering::Relaxed);
    for offset in 0..MAX_TEMP_ATTEMPTS {
        let name = format!(
            "{}{}-{}-{}-{}",
            names.temp_prefix,
            purpose,
            std::process::id(),
            start.wrapping_add(offset),
            offset
        );
        faults.hit(boundary, WorkerV3LoadReadinessFaultTimingV1::Before)?;
        match openat(
            &output.fd,
            &name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        ) {
            Ok(fd) => {
                faults.hit(boundary, WorkerV3LoadReadinessFaultTimingV1::After)?;
                return Ok((name, fs::File::from(fd)));
            }
            Err(error) if error == rustix::io::Errno::EXIST => {}
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
    }
    Err(WorkerV3LoadReadinessErrorV1::TemporaryNameExhausted)
}

fn validate_terminal_receipt(
    output: &PinnedOutput,
    names: &ReadinessNames,
    expected_attempt: BuildAttempt,
    backend: BackendPublicationReceiptV3,
    expected_envelope: Option<WorkerV3LoadEnvelopeBindingV1>,
) -> Result<WorkerV3LoadReadinessReceiptV1, WorkerV3LoadReadinessErrorV1> {
    let (mut file, before) = open_private_file(
        output,
        &names.receipt,
        MAX_WORKER_V3_LOAD_READINESS_RECEIPT_BYTES_V1,
    )?;
    let mut bytes = [0_u8; MAX_WORKER_V3_LOAD_READINESS_RECEIPT_BYTES_V1];
    file.read_exact(&mut bytes)?;
    finish_private_read(output, &names.receipt, &file, &before)?;
    let receipt = WorkerV3LoadReadinessReceiptV1::decode_canonical(&bytes)?;
    if receipt.output_directory
        != (ExactFileIdentityV1 {
            device: output.device,
            inode: output.inode,
        })
        || receipt.attempt != expected_attempt
        || !receipt.matches_backend_receipt(backend)?
        || expected_envelope.is_some_and(|expected| expected != receipt.envelope)
    {
        return Err(WorkerV3LoadReadinessErrorV1::ReceiptMismatch);
    }
    validate_envelope_file(output, names, receipt)?;
    read_validated_claim_file(output, names, receipt)?;
    Ok(receipt)
}

fn read_validated_claim_file(
    output: &PinnedOutput,
    names: &ReadinessNames,
    receipt: WorkerV3LoadReadinessReceiptV1,
) -> Result<DurablePublishedHsacoClaimV3, WorkerV3LoadReadinessErrorV1> {
    let expected_length = usize::try_from(receipt.durable_claim_length)
        .ok()
        .filter(|length| *length != 0 && *length <= MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V3)
        .ok_or(WorkerV3LoadReadinessErrorV1::ReceiptMismatch)?;
    let (mut file, before) = open_private_file(output, &names.claim, expected_length)?;
    if before.st_dev != receipt.durable_claim_file.device
        || before.st_ino != receipt.durable_claim_file.inode
        || before.st_mtime != receipt.durable_claim_mtime_seconds
        || before.st_mtime_nsec != receipt.durable_claim_mtime_nanoseconds
        || before.st_ctime != receipt.durable_claim_ctime_seconds
        || before.st_ctime_nsec != receipt.durable_claim_ctime_nanoseconds
    {
        return Err(WorkerV3LoadReadinessErrorV1::ReceiptMismatch);
    }
    let mut exact = fallible_vec(expected_length)?;
    exact.resize(expected_length, 0);
    file.read_exact(&mut exact)?;
    finish_private_read(output, &names.claim, &file, &before)?;
    if <[u8; 32]>::from(Sha256::digest(&exact)) != receipt.durable_claim_sha256 {
        return Err(WorkerV3LoadReadinessErrorV1::ReceiptMismatch);
    }
    let claim = DurablePublishedHsacoClaimV3::decode_canonical(&exact)?;
    if !receipt.matches_durable_claim(&claim)? {
        return Err(WorkerV3LoadReadinessErrorV1::ReceiptMismatch);
    }
    Ok(claim)
}

fn validate_envelope_file(
    output: &PinnedOutput,
    names: &ReadinessNames,
    receipt: WorkerV3LoadReadinessReceiptV1,
) -> Result<(), WorkerV3LoadReadinessErrorV1> {
    inspect_validated_envelope_file(output, names, receipt, |_| Ok(()))
}

fn read_validated_envelope_file(
    output: &PinnedOutput,
    names: &ReadinessNames,
    receipt: WorkerV3LoadReadinessReceiptV1,
) -> Result<Vec<u8>, WorkerV3LoadReadinessErrorV1> {
    let expected_length = usize::try_from(receipt.envelope.byte_length)
        .map_err(|_| WorkerV3LoadReadinessErrorV1::EnvelopeMismatch)?;
    let mut exact = fallible_vec(expected_length)?;
    inspect_validated_envelope_file(output, names, receipt, |chunk| {
        exact.extend_from_slice(chunk);
        Ok(())
    })?;
    Ok(exact)
}

fn inspect_validated_envelope_file(
    output: &PinnedOutput,
    names: &ReadinessNames,
    receipt: WorkerV3LoadReadinessReceiptV1,
    mut inspect_chunk: impl FnMut(&[u8]) -> Result<(), WorkerV3LoadReadinessErrorV1>,
) -> Result<(), WorkerV3LoadReadinessErrorV1> {
    let expected_length = usize::try_from(receipt.envelope.byte_length)
        .map_err(|_| WorkerV3LoadReadinessErrorV1::EnvelopeMismatch)?;
    let (mut file, before) = open_private_file(output, &names.envelope, expected_length)?;
    if before.st_dev != receipt.envelope_file.device
        || before.st_ino != receipt.envelope_file.inode
        || before.st_mtime != receipt.envelope_mtime_seconds
        || before.st_mtime_nsec != receipt.envelope_mtime_nanoseconds
        || before.st_ctime != receipt.envelope_ctime_seconds
        || before.st_ctime_nsec != receipt.envelope_ctime_nanoseconds
    {
        return Err(WorkerV3LoadReadinessErrorV1::EnvelopeMismatch);
    }
    let mut digest = Sha256::new();
    let mut remaining = expected_length;
    let mut buffer = [0_u8; 64 * 1024];
    while remaining != 0 {
        let length = remaining.min(buffer.len());
        file.read_exact(&mut buffer[..length])?;
        digest.update(&buffer[..length]);
        inspect_chunk(&buffer[..length])?;
        remaining -= length;
    }
    finish_private_read(output, &names.envelope, &file, &before)?;
    if <[u8; 32]>::from(digest.finalize()) != receipt.envelope.sha256 {
        return Err(WorkerV3LoadReadinessErrorV1::EnvelopeMismatch);
    }
    Ok(())
}

fn compare_exact_envelope(
    output: &PinnedOutput,
    entry: &str,
    expected: &[u8],
    receipt: WorkerV3LoadReadinessReceiptV1,
) -> Result<(), WorkerV3LoadReadinessErrorV1> {
    let snapshot = private_snapshot_required(output, entry)?;
    compare_bytes_and_snapshot(output, entry, expected, snapshot)?;
    if snapshot.st_dev != receipt.envelope_file.device
        || snapshot.st_ino != receipt.envelope_file.inode
        || snapshot.st_mtime != receipt.envelope_mtime_seconds
        || snapshot.st_mtime_nsec != receipt.envelope_mtime_nanoseconds
        || snapshot.st_ctime != receipt.envelope_ctime_seconds
        || snapshot.st_ctime_nsec != receipt.envelope_ctime_nanoseconds
    {
        return Err(WorkerV3LoadReadinessErrorV1::EnvelopeMismatch);
    }
    Ok(())
}

fn compare_bytes_and_snapshot(
    output: &PinnedOutput,
    entry: &str,
    expected: &[u8],
    snapshot: rustix::fs::Stat,
) -> Result<(), WorkerV3LoadReadinessErrorV1> {
    if usize::try_from(snapshot.st_size).ok() != Some(expected.len()) {
        return Err(WorkerV3LoadReadinessErrorV1::EnvelopeMismatch);
    }
    let (mut file, before) = open_private_file(output, entry, expected.len())?;
    if !same_snapshot(&snapshot, &before) {
        return Err(WorkerV3LoadReadinessErrorV1::FileChanged {
            entry: PathBuf::from(entry),
        });
    }
    let mut buffer = [0_u8; 64 * 1024];
    for expected_chunk in expected.chunks(buffer.len()) {
        let actual = &mut buffer[..expected_chunk.len()];
        file.read_exact(actual)?;
        if actual != expected_chunk {
            return Err(WorkerV3LoadReadinessErrorV1::EnvelopeMismatch);
        }
    }
    finish_private_read(output, entry, &file, &before)
}

fn open_private_file(
    output: &PinnedOutput,
    entry: &str,
    exact_length: usize,
) -> Result<(fs::File, rustix::fs::Stat), WorkerV3LoadReadinessErrorV1> {
    let fd = match openat(
        &output.fd,
        entry,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(error) if error == rustix::io::Errno::NOENT => {
            return Err(if entry.ends_with(ENVELOPE_SUFFIX_V1) {
                WorkerV3LoadReadinessErrorV1::MissingEnvelope
            } else if entry.ends_with(CLAIM_SUFFIX_V1) {
                WorkerV3LoadReadinessErrorV1::MissingClaim
            } else {
                WorkerV3LoadReadinessErrorV1::MissingReceipt
            });
        }
        Err(error) => return Err(std::io::Error::from(error).into()),
    };
    let stat = fstat(&fd).map_err(std::io::Error::from)?;
    if !is_private_file(&stat) || usize::try_from(stat.st_size).ok() != Some(exact_length) {
        return Err(WorkerV3LoadReadinessErrorV1::InvalidPrivateEntry {
            entry: PathBuf::from(entry),
        });
    }
    Ok((fs::File::from(fd), stat))
}

fn finish_private_read(
    output: &PinnedOutput,
    entry: &str,
    file: &fs::File,
    before: &rustix::fs::Stat,
) -> Result<(), WorkerV3LoadReadinessErrorV1> {
    let descriptor = fstat(file).map_err(std::io::Error::from)?;
    let named =
        statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    if !same_snapshot(before, &descriptor) || !same_snapshot(before, &named) {
        return Err(WorkerV3LoadReadinessErrorV1::FileChanged {
            entry: PathBuf::from(entry),
        });
    }
    output.verify_path_identity()?;
    Ok(())
}

fn private_snapshot_optional(
    output: &PinnedOutput,
    entry: &str,
) -> Result<Option<rustix::fs::Stat>, WorkerV3LoadReadinessErrorV1> {
    match statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) if is_private_file(&stat) => Ok(Some(stat)),
        Ok(_) => Err(WorkerV3LoadReadinessErrorV1::InvalidPrivateEntry {
            entry: PathBuf::from(entry),
        }),
        Err(error) if error == rustix::io::Errno::NOENT => Ok(None),
        Err(error) => Err(std::io::Error::from(error).into()),
    }
}

fn private_snapshot_required(
    output: &PinnedOutput,
    entry: &str,
) -> Result<rustix::fs::Stat, WorkerV3LoadReadinessErrorV1> {
    private_snapshot_optional(output, entry)?.ok_or_else(|| {
        if entry.ends_with(ENVELOPE_SUFFIX_V1) {
            WorkerV3LoadReadinessErrorV1::MissingEnvelope
        } else if entry.ends_with(CLAIM_SUFFIX_V1) {
            WorkerV3LoadReadinessErrorV1::MissingClaim
        } else {
            WorkerV3LoadReadinessErrorV1::MissingReceipt
        }
    })
}

fn cleanup_temps(
    output: &PinnedOutput,
    names: &ReadinessNames,
) -> Result<usize, WorkerV3LoadReadinessErrorV1> {
    let directory = rustix::io::fcntl_dupfd_cloexec(&output.fd, 0).map_err(std::io::Error::from)?;
    let mut entries = rustix::fs::Dir::read_from(&directory).map_err(std::io::Error::from)?;
    let mut scanned = 0_usize;
    let mut candidates = Vec::new();
    candidates
        .try_reserve_exact(MAX_TEMP_ATTEMPTS as usize)
        .map_err(|_| WorkerV3LoadReadinessCodecErrorV1::AllocationFailed {
            requested: MAX_TEMP_ATTEMPTS as usize,
        })?;
    for entry in &mut entries {
        let entry = entry.map_err(std::io::Error::from)?;
        let name = entry.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        scanned = scanned.checked_add(1).ok_or(
            WorkerV3LoadReadinessErrorV1::DirectoryEntryLimitExceeded {
                maximum: MAX_OUTPUT_ENTRIES,
            },
        )?;
        if scanned > MAX_OUTPUT_ENTRIES {
            return Err(WorkerV3LoadReadinessErrorV1::DirectoryEntryLimitExceeded {
                maximum: MAX_OUTPUT_ENTRIES,
            });
        }
        if !name.starts_with(names.temp_prefix.as_bytes()) {
            continue;
        }
        if candidates.len() == MAX_TEMP_ATTEMPTS as usize {
            return Err(WorkerV3LoadReadinessErrorV1::TemporaryEntryLimitExceeded {
                maximum: MAX_TEMP_ATTEMPTS as usize,
            });
        }
        let path = PathBuf::from(std::ffi::OsString::from_vec(name.to_vec()));
        let snapshot = private_snapshot_optional(
            output,
            path.to_str()
                .ok_or_else(|| WorkerV3LoadReadinessErrorV1::InvalidPrivateEntry {
                    entry: path.clone(),
                })?,
        )?
        .ok_or_else(|| WorkerV3LoadReadinessErrorV1::FileChanged {
            entry: path.clone(),
        })?;
        candidates.push((path, snapshot));
    }
    output.verify_path_identity()?;
    for (entry, snapshot) in candidates {
        quarantine_and_unlink_temp(output, names, &entry, &snapshot)?;
        scanned -= 1;
    }
    fsync(&output.fd).map_err(std::io::Error::from)?;
    output.verify_path_identity()?;
    Ok(scanned)
}

fn quarantine_and_unlink_temp(
    output: &PinnedOutput,
    names: &ReadinessNames,
    entry: &Path,
    snapshot: &rustix::fs::Stat,
) -> Result<(), WorkerV3LoadReadinessErrorV1> {
    let exact_length = usize::try_from(snapshot.st_size)
        .ok()
        .filter(|length| *length <= MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1)
        .ok_or_else(|| WorkerV3LoadReadinessErrorV1::InvalidPrivateEntry {
            entry: entry.to_path_buf(),
        })?;
    let entry_name =
        entry
            .to_str()
            .ok_or_else(|| WorkerV3LoadReadinessErrorV1::InvalidPrivateEntry {
                entry: entry.to_path_buf(),
            })?;
    let (file, pinned) = open_private_file(output, entry_name, exact_length)?;
    if !same_snapshot(snapshot, &pinned) {
        return Err(WorkerV3LoadReadinessErrorV1::FileChanged {
            entry: entry.to_path_buf(),
        });
    }

    let quarantine = reserve_cleanup_quarantine_name(output, names, entry)?;
    let named =
        statat(&output.fd, &quarantine, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    let descriptor = fstat(&file).map_err(std::io::Error::from)?;
    if !same_private_inode(snapshot, &named)
        || !same_private_inode(snapshot, &descriptor)
        || !same_snapshot(&named, &descriptor)
    {
        return Err(WorkerV3LoadReadinessErrorV1::FileChanged { entry: quarantine });
    }

    unlinkat(&output.fd, &quarantine, AtFlags::empty()).map_err(std::io::Error::from)?;
    let unlinked = fstat(&file).map_err(std::io::Error::from)?;
    if unlinked.st_dev != snapshot.st_dev
        || unlinked.st_ino != snapshot.st_ino
        || unlinked.st_nlink != 0
    {
        return Err(WorkerV3LoadReadinessErrorV1::FileChanged { entry: quarantine });
    }
    Ok(())
}

fn reserve_cleanup_quarantine_name(
    output: &PinnedOutput,
    names: &ReadinessNames,
    source: &Path,
) -> Result<PathBuf, WorkerV3LoadReadinessErrorV1> {
    let start = NEXT_TEMP_ID.fetch_add(MAX_TEMP_ATTEMPTS, Ordering::Relaxed);
    for offset in 0..MAX_TEMP_ATTEMPTS {
        let name = format!(
            "{}cleanup-{}-{}-{}",
            names.temp_prefix,
            std::process::id(),
            start.wrapping_add(offset),
            offset
        );
        match renameat_with(
            &output.fd,
            source,
            &output.fd,
            &name,
            RenameFlags::NOREPLACE,
        ) {
            Ok(()) => return Ok(PathBuf::from(name)),
            Err(error) if error == rustix::io::Errno::EXIST => {}
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
    }
    Err(WorkerV3LoadReadinessErrorV1::TemporaryNameExhausted)
}

fn ensure_publication_headroom(
    current_entries: usize,
    envelope_present: bool,
    claim_present: bool,
    receipt_present: bool,
) -> Result<(), WorkerV3LoadReadinessErrorV1> {
    let missing_finals = usize::from(!envelope_present)
        + usize::from(!claim_present)
        + usize::from(!receipt_present);
    let required = missing_finals
        .checked_add(1)
        .and_then(|additional| current_entries.checked_add(additional))
        .ok_or(WorkerV3LoadReadinessErrorV1::DirectoryEntryLimitExceeded {
            maximum: MAX_OUTPUT_ENTRIES,
        })?;
    if required > MAX_OUTPUT_ENTRIES {
        return Err(WorkerV3LoadReadinessErrorV1::DirectoryEntryLimitExceeded {
            maximum: MAX_OUTPUT_ENTRIES,
        });
    }
    Ok(())
}

fn is_private_file(stat: &rustix::fs::Stat) -> bool {
    FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile
        && stat.st_nlink == 1
        && stat.st_mode & 0o077 == 0
}

fn same_snapshot(left: &rustix::fs::Stat, right: &rustix::fs::Stat) -> bool {
    same_private_inode(left, right)
        && left.st_mtime == right.st_mtime
        && left.st_mtime_nsec == right.st_mtime_nsec
        && left.st_ctime == right.st_ctime
        && left.st_ctime_nsec == right.st_ctime_nsec
}

fn same_private_inode(left: &rustix::fs::Stat, right: &rustix::fs::Stat) -> bool {
    is_private_file(left)
        && is_private_file(right)
        && left.st_dev == right.st_dev
        && left.st_ino == right.st_ino
        && left.st_size == right.st_size
}

fn result(
    output: &PinnedOutput,
    names: &ReadinessNames,
    outcome: WorkerV3LoadReadinessOutcomeV1,
    receipt: WorkerV3LoadReadinessReceiptV1,
) -> Result<WorkerV3LoadReadinessResultV1, WorkerV3LoadReadinessErrorV1> {
    let claim = read_validated_claim_file(output, names, receipt)?;
    let exact_envelope = read_validated_envelope_file(output, names, receipt)?;
    Ok(WorkerV3LoadReadinessResultV1 {
        outcome,
        receipt,
        claim,
        exact_envelope,
        envelope_path: output.display_path.join(&names.envelope),
    })
}

struct ReadinessNames {
    envelope: String,
    claim: String,
    receipt: String,
    temp_prefix: String,
}

impl ReadinessNames {
    fn new(
        backend: BackendPublicationReceiptV3,
    ) -> Result<Self, WorkerV3LoadReadinessCodecErrorV1> {
        let backend_identity = backend_receipt_identity(backend)?;
        let key = domain_hash(NAMESPACE_KEY_DOMAIN_V1, &backend_identity);
        let base = format!("{FILE_PREFIX_V1}{}", crate::encode_hex(&key));
        Ok(Self {
            envelope: format!("{base}{ENVELOPE_SUFFIX_V1}"),
            claim: format!("{base}{CLAIM_SUFFIX_V1}"),
            receipt: format!("{base}{RECEIPT_SUFFIX_V1}"),
            temp_prefix: format!("{base}{TEMP_MARKER_V1}"),
        })
    }
}

struct FaultInjector {
    point: Option<WorkerV3LoadReadinessFaultPointV1>,
}

impl FaultInjector {
    const fn new(point: Option<WorkerV3LoadReadinessFaultPointV1>) -> Self {
        Self { point }
    }

    fn hit(
        &mut self,
        boundary: WorkerV3LoadReadinessBoundaryV1,
        timing: WorkerV3LoadReadinessFaultTimingV1,
    ) -> Result<(), WorkerV3LoadReadinessErrorV1> {
        let point = WorkerV3LoadReadinessFaultPointV1 { boundary, timing };
        if self.point == Some(point) {
            self.point = None;
            return Err(WorkerV3LoadReadinessErrorV1::InjectedCrash(point));
        }
        Ok(())
    }

    fn around<T>(
        &mut self,
        boundary: WorkerV3LoadReadinessBoundaryV1,
        operation: impl FnOnce() -> Result<T, WorkerV3LoadReadinessErrorV1>,
    ) -> Result<T, WorkerV3LoadReadinessErrorV1> {
        self.hit(boundary, WorkerV3LoadReadinessFaultTimingV1::Before)?;
        let value = operation()?;
        self.hit(boundary, WorkerV3LoadReadinessFaultTimingV1::After)?;
        Ok(value)
    }
}

fn backend_receipt_identity(
    receipt: BackendPublicationReceiptV3,
) -> Result<[u8; 32], WorkerV3LoadReadinessCodecErrorV1> {
    let bytes = crate::attempt::encode_backend_publication_receipt_v3(receipt).map_err(
        |error| match error {
            AttemptCodecError::AllocationFailed { requested } => {
                WorkerV3LoadReadinessCodecErrorV1::AllocationFailed { requested }
            }
            _ => WorkerV3LoadReadinessCodecErrorV1::InvalidAttempt,
        },
    )?;
    Ok(domain_hash(BACKEND_RECEIPT_IDENTITY_DOMAIN_V1, &bytes))
}

fn durable_claim_binding(
    claim: &DurablePublishedHsacoClaimV3,
) -> Result<([u8; 32], u64), WorkerV3LoadReadinessErrorV1> {
    let bytes = claim.encode_canonical()?;
    let length = u64::try_from(bytes.len())
        .map_err(|_| WorkerV3LoadReadinessCodecErrorV1::InvalidDurableClaimLength)?;
    if length == 0 {
        return Err(WorkerV3LoadReadinessCodecErrorV1::InvalidDurableClaimLength.into());
    }
    Ok((Sha256::digest(&bytes).into(), length))
}

fn push_attempt(bytes: &mut Vec<u8>, attempt: BuildAttempt) {
    bytes.extend_from_slice(&attempt.generation().to_le_bytes());
    bytes.extend_from_slice(attempt.session().as_bytes());
    bytes.extend_from_slice(attempt.invocation().as_bytes());
}

fn validate_envelope_length(length: u64) -> Result<(), WorkerV3LoadReadinessCodecErrorV1> {
    if length == 0 || length > MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1 as u64 {
        return Err(WorkerV3LoadReadinessCodecErrorV1::InvalidEnvelopeLength {
            actual: length,
            maximum: MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1,
        });
    }
    Ok(())
}

fn fallible_vec(capacity: usize) -> Result<Vec<u8>, WorkerV3LoadReadinessCodecErrorV1> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(capacity).map_err(|_| {
        WorkerV3LoadReadinessCodecErrorV1::AllocationFailed {
            requested: capacity,
        }
    })?;
    Ok(bytes)
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    digest.finalize().into()
}

struct FixedDecoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> FixedDecoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], WorkerV3LoadReadinessCodecErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(WorkerV3LoadReadinessCodecErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(WorkerV3LoadReadinessCodecErrorV1::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], WorkerV3LoadReadinessCodecErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| WorkerV3LoadReadinessCodecErrorV1::Truncated)
    }

    fn u16(&mut self) -> Result<u16, WorkerV3LoadReadinessCodecErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, WorkerV3LoadReadinessCodecErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn i64(&mut self) -> Result<i64, WorkerV3LoadReadinessCodecErrorV1> {
        Ok(i64::from_le_bytes(self.fixed()?))
    }

    fn attempt(&mut self) -> Result<BuildAttempt, WorkerV3LoadReadinessCodecErrorV1> {
        BuildAttempt::new(
            self.u64()?,
            crate::BuildSession::from_bytes(self.fixed()?),
            crate::BuildInvocation::from_bytes(self.fixed()?),
        )
        .map_err(|_| WorkerV3LoadReadinessCodecErrorV1::InvalidAttempt)
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WorkerV3PublicationBindingV1;
    use crate::attempt::{AttemptRegistry, BackendReceiptV1, StartAttemptOutcome};
    use fe2o3_build_authority::CompilerClosureV2;

    fn attempt() -> BuildAttempt {
        BuildAttempt::new(
            7,
            crate::BuildSession::from_bytes([0x21; 16]),
            crate::BuildInvocation::from_bytes([0x22; 32]),
        )
        .unwrap()
    }

    fn backend_receipt() -> BackendPublicationReceiptV3 {
        let closure = CompilerClosureV2::new(
            [0x31; 32], [0x32; 32], [0x33; 32], [0x34; 32], [0x35; 32], [0x36; 32],
        )
        .unwrap();
        let binding = WorkerV3PublicationBindingV1::new(
            closure, [0x41; 32], [0x42; 32], [0x43; 32], [0x44; 32], [0x45; 32], [0x46; 32], 19,
            [0x47; 32], 23,
        )
        .unwrap();
        BackendPublicationReceiptV3::new(
            [0x51; 32],
            [0x52; 32],
            [0x53; 32],
            [0x54; 32],
            [0x55; 32],
            binding.finalized_output_sha256(),
            [0x56; 32],
            binding,
        )
    }

    fn receipt() -> WorkerV3LoadReadinessReceiptV1 {
        let backend = backend_receipt();
        WorkerV3LoadReadinessReceiptV1 {
            attempt: attempt(),
            backend_receipt_sha256: backend_receipt_identity(backend).unwrap(),
            envelope: WorkerV3LoadEnvelopeBindingV1::from_exact_bytes(b"opaque V3 envelope")
                .unwrap(),
            durable_claim_sha256: [0x71; 32],
            durable_claim_length: 0x72,
            output_directory: ExactFileIdentityV1 {
                device: 0x1112_1314_1516_1718,
                inode: 0x2122_2324_2526_2728,
            },
            envelope_file: ExactFileIdentityV1 {
                device: 0x3132_3334_3536_3738,
                inode: 0x4142_4344_4546_4748,
            },
            envelope_mtime_seconds: 0x0102_0304_0506_0708,
            envelope_mtime_nanoseconds: 0x1112_1314_1516_1718,
            envelope_ctime_seconds: 0x2122_2324_2526_2728,
            envelope_ctime_nanoseconds: 0x3132_3334_3536_3738,
            durable_claim_file: ExactFileIdentityV1 {
                device: 0x5152_5354_5556_5758,
                inode: 0x6162_6364_6566_6768,
            },
            durable_claim_mtime_seconds: 0x4142_4344_4546_4748,
            durable_claim_mtime_nanoseconds: 0x5152_5354_5556_5758,
            durable_claim_ctime_seconds: 0x6162_6364_6566_6768,
            durable_claim_ctime_nanoseconds: 0x7172_7374_7576_7778,
        }
    }

    fn rewrite_checksum(bytes: &mut [u8], body_length: usize, domain: &[u8]) {
        let checksum = domain_hash(domain, &bytes[..body_length]);
        bytes[body_length..].copy_from_slice(&checksum);
    }

    #[test]
    fn schema_neutral_binding_uses_raw_sha256_and_strict_independent_codec() {
        let exact = b"opaque V3 envelope";
        let binding = WorkerV3LoadEnvelopeBindingV1::from_exact_bytes(exact).unwrap();
        assert_eq!(binding.sha256(), <[u8; 32]>::from(Sha256::digest(exact)));
        assert_eq!(binding.byte_length(), exact.len() as u64);
        let encoded = binding.encode_canonical().unwrap();
        assert_eq!(
            WorkerV3LoadEnvelopeBindingV1::decode_canonical(&encoded).unwrap(),
            binding
        );
        assert!(WorkerV3LoadReadinessReceiptV1::decode_canonical(&encoded).is_err());

        for length in 0..encoded.len() {
            assert!(
                WorkerV3LoadEnvelopeBindingV1::decode_canonical(&encoded[..length]).is_err(),
                "accepted hostile binding prefix {length}"
            );
        }
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(WorkerV3LoadEnvelopeBindingV1::decode_canonical(&trailing).is_err());
        let mut other_version = encoded;
        other_version[BINDING_MAGIC_V1.len()..BINDING_MAGIC_V1.len() + 2]
            .copy_from_slice(&2_u16.to_le_bytes());
        rewrite_checksum(
            &mut other_version,
            BINDING_BODY_BYTES_V1,
            BINDING_CHECKSUM_DOMAIN_V1,
        );
        assert_eq!(
            WorkerV3LoadEnvelopeBindingV1::decode_canonical(&other_version),
            Err(WorkerV3LoadReadinessCodecErrorV1::UnsupportedVersion { actual: 2 })
        );
    }

    #[test]
    fn terminal_receipt_is_fixed_canonical_and_cross_version_fail_closed() {
        let receipt = receipt();
        let encoded = receipt.encode_canonical().unwrap();
        assert_eq!(encoded.len(), MAX_WORKER_V3_LOAD_READINESS_RECEIPT_BYTES_V1);
        assert_eq!(
            WorkerV3LoadReadinessReceiptV1::decode_canonical(&encoded).unwrap(),
            receipt
        );
        assert!(WorkerV3LoadEnvelopeBindingV1::decode_canonical(&encoded).is_err());

        for length in 0..encoded.len() {
            assert!(
                WorkerV3LoadReadinessReceiptV1::decode_canonical(&encoded[..length]).is_err(),
                "accepted hostile receipt prefix {length}"
            );
        }
        let mut corrupt = encoded.clone();
        corrupt[RECEIPT_MAGIC_V1.len() + 5] ^= 1;
        assert_eq!(
            WorkerV3LoadReadinessReceiptV1::decode_canonical(&corrupt),
            Err(WorkerV3LoadReadinessCodecErrorV1::ChecksumMismatch)
        );
        let mut other_version = encoded;
        other_version[RECEIPT_MAGIC_V1.len()..RECEIPT_MAGIC_V1.len() + 2]
            .copy_from_slice(&2_u16.to_le_bytes());
        rewrite_checksum(
            &mut other_version,
            RECEIPT_BODY_BYTES_V1,
            RECEIPT_CHECKSUM_DOMAIN_V1,
        );
        assert_eq!(
            WorkerV3LoadReadinessReceiptV1::decode_canonical(&other_version),
            Err(WorkerV3LoadReadinessCodecErrorV1::UnsupportedVersion { actual: 2 })
        );
    }

    #[test]
    fn codecs_reserve_fallibly_and_reject_unbounded_lengths() {
        assert_eq!(
            fallible_vec(usize::MAX),
            Err(WorkerV3LoadReadinessCodecErrorV1::AllocationFailed {
                requested: usize::MAX,
            })
        );
        assert!(matches!(
            WorkerV3LoadEnvelopeBindingV1::new([0; 32], 0),
            Err(WorkerV3LoadReadinessCodecErrorV1::InvalidEnvelopeLength { .. })
        ));
        assert!(matches!(
            WorkerV3LoadEnvelopeBindingV1::new(
                [0; 32],
                MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1 as u64 + 1
            ),
            Err(WorkerV3LoadReadinessCodecErrorV1::InvalidEnvelopeLength { .. })
        ));
    }

    #[test]
    fn tag_eight_contains_exact_v3_backend_then_exact_readiness_receipt() {
        let mut registry = AttemptRegistry::default();
        let expected_attempt = match registry
            .start_or_resume(
                "path:/src/load-ready.rs",
                "load_ready",
                attempt().invocation(),
                attempt().session(),
            )
            .unwrap()
        {
            StartAttemptOutcome::New(attempt) => attempt,
            outcome => panic!("unexpected attempt outcome: {outcome:?}"),
        };
        let backend = backend_receipt();
        let mut readiness = receipt();
        readiness.attempt = expected_attempt;
        registry
            .transition_building("path:/src/load-ready.rs", expected_attempt)
            .unwrap();
        registry
            .claim_backend_with_pending_receipt_v3(
                "path:/src/load-ready.rs",
                expected_attempt,
                backend,
            )
            .unwrap();
        registry
            .record_backend_publication_receipt_v3(
                "path:/src/load-ready.rs",
                expected_attempt,
                backend,
            )
            .unwrap();
        registry
            .record_worker_v3_load_readiness(
                "path:/src/load-ready.rs",
                expected_attempt,
                backend,
                readiness,
            )
            .unwrap();

        let encoded = registry.encode().unwrap();
        let backend_bytes = crate::attempt::encode_backend_publication_receipt_v3(backend).unwrap();
        let offset = encoded
            .windows(backend_bytes.len())
            .position(|window| window == backend_bytes)
            .unwrap();
        assert_eq!(encoded[offset - 1], 8);
        assert_eq!(
            &encoded[offset + backend_bytes.len()..],
            readiness.encode_canonical().unwrap()
        );
        assert_eq!(AttemptRegistry::decode(&encoded).unwrap(), registry);
        assert_eq!(
            registry
                .record_exact("path:/src/load-ready.rs", expected_attempt)
                .unwrap()
                .backend_receipt,
            Some(BackendReceiptV1::EnvelopeCustodyV3(backend, readiness))
        );
    }

    #[test]
    fn durable_custody_is_not_descriptor_authentication_or_load_admission() {
        let binding = WorkerV3LoadEnvelopeBindingV1::from_exact_bytes(b"opaque").unwrap();
        let authority = unsafe {
            VerifiedWorkerV3LoadEnvelopeAuthorityV1::from_complete_compact_replay_preimages_unchecked(
                binding,
            )
        };
        let receipt = receipt();
        assert!(!authority.authenticates_descriptor_source());
        assert!(!authority.grants_semantic_load_admission());
        assert!(!authority.establishes_hsa_readiness());
        assert!(!authority.grants_load_authority());
        assert!(!authority.grants_launch_authority());
        assert!(!receipt.grants_replay_intent_retirement_authority());
        assert!(receipt.is_replay_intent_retirement_evidence());
        assert!(!receipt.authenticates_descriptor_source());
        assert!(!receipt.grants_semantic_load_admission());
        assert!(!receipt.establishes_hsa_readiness());
        assert!(!receipt.grants_load_authority());
        assert!(!receipt.grants_launch_authority());
    }

    #[test]
    fn publication_headroom_reserves_three_finals_and_one_transient_entry() {
        assert!(ensure_publication_headroom(MAX_OUTPUT_ENTRIES - 4, false, false, false).is_ok());
        assert!(matches!(
            ensure_publication_headroom(MAX_OUTPUT_ENTRIES - 3, false, false, false),
            Err(WorkerV3LoadReadinessErrorV1::DirectoryEntryLimitExceeded {
                maximum: MAX_OUTPUT_ENTRIES
            })
        ));
        assert!(ensure_publication_headroom(MAX_OUTPUT_ENTRIES - 1, true, true, true).is_ok());
        assert!(matches!(
            ensure_publication_headroom(MAX_OUTPUT_ENTRIES, true, true, true),
            Err(WorkerV3LoadReadinessErrorV1::DirectoryEntryLimitExceeded {
                maximum: MAX_OUTPUT_ENTRIES
            })
        ));
    }
}
