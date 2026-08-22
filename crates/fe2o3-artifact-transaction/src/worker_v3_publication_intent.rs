//! Inert durable restart input for one finalized Worker V3 output.
//!
//! This protocol stores one caller-designated copy of each replay attachment beside the exact
//! finalized output. The layout contains one exact outer handoff, each external provider payload
//! once, and compact opaque reconstruction metadata. It deliberately does not retain raw worker
//! output or canonical worker request/response aggregates: a higher-level finalizer must derive raw
//! bytes from the canonical finalized output and reconstruct or stream-hash the worker wires.
//! Receipt-bound retirement moves the record to an inert marker before deleting attachments, so
//! cleanup can resume after interruption or a successor generation without retaining old inputs.
//!
//! This crate validates storage framing and resource bounds only. It does not authenticate a
//! component's producer, establish that metadata is a canonical finalizer transcript, derive
//! semantic identities, or grant publication, loading, or launch authority.

use crate::attempt::{AttemptPhase, BackendReceiptV1};
use crate::attempt_scoped_hsaco_publication::{publication_receipt, publication_receipt_v2};
use crate::{
    AtomicPublicationIdentityV1, BuildAttempt, BuildInvocation, BuildSession,
    CanonicalLinkRequestIdentityV1, DurableLinkPublicationPlanV1, FinalizationIdentityV1,
    FinalizedOutputIdentityV1, KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1,
    MAX_COMPILER_MODULE_HANDOFF_BYTES_V3, MAX_DURABLE_FINALIZED_ARTIFACT_BYTES, MAX_OUTPUT_ENTRIES,
    PackageIdentityV1, PinnedOutput, PinnedWorkerIdentityV1, ProducerIdentity, TargetIdentityV1,
    UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1, read_attempt_registry,
};
use rustix::fs::{
    AtFlags, FileType, Mode, OFlags, RenameFlags, fstat, fsync, openat, renameat, renameat_with,
    statat, unlinkat,
};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const RECORD_MAGIC_V1: &[u8] = b"FE2O3-WORKER-V3-PUBLICATION-INTENT-V1\0";
const RECORD_VERSION_V1: u16 = 1;
const PRODUCER_KEY_DOMAIN_V1: &[u8] = b"fe2o3.worker-v3-publication-intent.producer-key.v1\0";
const OCCURRENCE_KEY_DOMAIN_V1: &[u8] = b"fe2o3.worker-v3-publication-intent.occurrence-key.v1\0";
const RECORD_CHECKSUM_DOMAIN_V1: &[u8] = b"fe2o3.worker-v3-publication-intent.record-checksum.v1\0";
const RECORD_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.worker-v3-publication-intent.record-identity.v1\0";
const FILE_PREFIX_V1: &str = ".fe2o3-worker-v3-publication-intent-v1-";
const OUTER_HANDOFF_SUFFIX: &str = ".handoff";
const EXTERNAL_PROVIDERS_SUFFIX: &str = ".providers";
const OUTPUT_SUFFIX: &str = ".output";
const TRANSCRIPT_SUFFIX: &str = ".transcript";
const RECORD_SUFFIX: &str = ".record";
const REDO_SUFFIX: &str = ".record.redo";
const RETIRING_SUFFIX: &str = ".record.retiring";
const TEMP_SUFFIX: &str = ".tmp-";
const MAX_TEMP_ATTEMPTS: u64 = 64;

/// Canonical final directory entries reserved for one complete V3 intent.
pub const WORKER_V3_PUBLICATION_INTENT_FINAL_ENTRY_HEADROOM_V1: usize = 5;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

// magic, version, occurrence key, attempt, producer key, plan commitment, scope, seven plan
// fields, four attachment bindings, provider count/payload length, and checksum.
const RECORD_BYTES_V1: usize = RECORD_MAGIC_V1.len()
    + 2
    + 32
    + 8
    + 16
    + 32
    + 32
    + 32
    + (3 * 32)
    + (7 * 32)
    + (4 * (32 + 8))
    + 4
    + 8
    + 32;

/// Exact canonical size of one Worker V3 publication-intent record.
pub const MAX_WORKER_V3_PUBLICATION_INTENT_RECORD_BYTES_V1: usize = RECORD_BYTES_V1;

/// Maximum exact finalized output retained by one Worker V3 publication intent.
pub const MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1: usize =
    MAX_DURABLE_FINALIZED_ARTIFACT_BYTES;

/// Maximum external provider payloads in the compact storage layout.
///
/// The direct worker admits 128 total inputs. The compiler module is already present in the outer
/// handoff, leaving at most 127 external providers.
pub const MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1: usize = 127;

/// Maximum aggregate bytes across all external provider payloads.
///
/// This mirrors the direct worker's aggregate input-payload ceiling. It is intentionally applied
/// independently here because this lower-level crate does not parse the compiler module out of the
/// opaque outer handoff. The finalizer must also enforce its tighter compiler-plus-provider total.
pub const MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1: usize = 64 * 1024 * 1024;

const MAX_REPLAY_LINK_OPTIONS_V1: usize = 64;
const MAX_REPLAY_LINK_OPTION_NAME_BYTES_V1: usize = 64;
const MAX_REPLAY_LINK_OPTION_VALUE_BYTES_V1: usize = 256;
const MAX_REPLAY_TARGET_BYTES_V1: usize = 128;
const MAX_REPLAY_TOOLCHAIN_ID_BYTES_V1: usize = 160;
const MAX_REPLAY_SYMBOLS_V1: usize = 4096;
const MAX_REPLAY_SYMBOL_BYTES_V1: usize = 256;
const MAX_REPLAY_PROVIDER_FILES_V1: usize = 16;
const MAX_REPLAY_PROVIDER_BASENAME_BYTES_V1: usize = 128;
const MAX_REPLAY_PROVIDER_IDENTITY_BYTES_V1: usize = 128;
const MAX_REPLAY_DIAGNOSTICS_V1: usize = 64;
const MAX_REPLAY_TOTAL_DIAGNOSTIC_BYTES_V1: usize = 16 * 1024;
const REPLAY_CONTENT_IDENTITY_BYTES_V1: usize = 32 + 8;

// One device-library provider-evidence metadata body from a Worker V3 response. Strict V3 binds
// bootstrap and replay responses independently, so the transcript budget includes two bodies.
const MAX_REPLAY_PROVIDER_EVIDENCE_METADATA_BYTES_V1: usize = MAX_REPLAY_PROVIDER_IDENTITY_BYTES_V1
    + MAX_REPLAY_TARGET_BYTES_V1
    + MAX_REPLAY_SYMBOLS_V1 * (MAX_REPLAY_SYMBOL_BYTES_V1 + 4)
    + MAX_REPLAY_PROVIDER_FILES_V1 * (MAX_REPLAY_PROVIDER_BASENAME_BYTES_V1 + 36)
    + 49;

const MAX_REPLAY_DIAGNOSTIC_BODY_BYTES_V1: usize =
    MAX_REPLAY_TOTAL_DIAGNOSTIC_BYTES_V1 + MAX_REPLAY_DIAGNOSTICS_V1 * 4 + 4;
const MAX_REPLAY_SHARED_WORKER_OPTION_METADATA_BYTES_V1: usize = 2
    * MAX_REPLAY_TOOLCHAIN_ID_BYTES_V1
    + REPLAY_CONTENT_IDENTITY_BYTES_V1
    + MAX_REPLAY_TARGET_BYTES_V1
    + MAX_REPLAY_LINK_OPTIONS_V1
        * (8 + MAX_REPLAY_LINK_OPTION_NAME_BYTES_V1 + MAX_REPLAY_LINK_OPTION_VALUE_BYTES_V1);
// Exact V3 response metadata shell excluding the separately stored output payload: magic, nine
// field headers, three request/closure identities, worker build identity, stage, diagnostics,
// output identity/length shell, provider evidence, and response identity.
const MAX_REPLAY_RESPONSE_METADATA_SHELL_BYTES_V1: usize = 8
    + 9 * (2 + 4)
    + 3 * 32
    + MAX_REPLAY_TOOLCHAIN_ID_BYTES_V1
    + 1
    + MAX_REPLAY_DIAGNOSTIC_BODY_BYTES_V1
    + (1 + REPLAY_CONTENT_IDENTITY_BYTES_V1)
    + MAX_REPLAY_PROVIDER_EVIDENCE_METADATA_BYTES_V1
    + 32;
// Audited bound for versioned transcript framing and fixed request/plan identities not already in
// the two response shells or shared worker/target/option reconstruction metadata.
const MAX_REPLAY_SHARED_FRAMING_AND_IDENTITIES_BYTES_V1: usize = 4_175;

/// Maximum compact opaque finalizer reconstruction metadata retained by one intent.
///
/// The formula admits two independent strict-V3 response metadata shells, including separate
/// provider-evidence and diagnostic bodies, plus shared worker/target/option reconstruction
/// metadata and audited fixed framing/identity bytes. Large handoff, provider, raw-output, and
/// finalized-output bytes have separate attachments and must not be copied into this metadata.
pub const MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1: usize = 2
    * MAX_REPLAY_RESPONSE_METADATA_SHELL_BYTES_V1
    + MAX_REPLAY_SHARED_WORKER_OPTION_METADATA_BYTES_V1
    + MAX_REPLAY_SHARED_FRAMING_AND_IDENTITIES_BYTES_V1;
const _: () = assert!(MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1 == 2_195_505);

const PROVIDER_ARCHIVE_MAGIC_V1: &[u8] = b"FE2O3-WORKER-V3-PROVIDER-PAYLOADS-V1\0";
const PROVIDER_ARCHIVE_VERSION_V1: u16 = 1;
const PROVIDER_ARCHIVE_CHECKSUM_DOMAIN_V1: &[u8] =
    b"fe2o3.worker-v3-publication-intent.provider-archive-checksum.v1\0";
const PROVIDER_ARCHIVE_PREFIX_BYTES_V1: usize = PROVIDER_ARCHIVE_MAGIC_V1.len() + 2 + 4 + 8;
const PROVIDER_ARCHIVE_FIXED_BYTES_V1: usize = PROVIDER_ARCHIVE_MAGIC_V1.len() + 2 + 4 + 8 + 32;
const PROVIDER_ARCHIVE_ENTRY_BYTES_V1: usize = 8 + 32;
const MAX_PROVIDER_ARCHIVE_FRAMING_BYTES_V1: usize = PROVIDER_ARCHIVE_FIXED_BYTES_V1
    + MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1 * PROVIDER_ARCHIVE_ENTRY_BYTES_V1;
// Recovery retains the parsed length/hash table while filling the payload-owner vector. Account
// for both bounded allocations as metadata instead of hiding them in the payload budget.
const MAX_PROVIDER_RECOVERY_BOOKKEEPING_BYTES_V1: usize =
    MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1
        * (std::mem::size_of::<(usize, [u8; 32])>() + std::mem::size_of::<Vec<u8>>());
const RECOVERY_ATTACHMENT_OWNER_BYTES_V1: usize = 4 * std::mem::size_of::<Vec<u8>>();

/// Maximum canonical provider archive bytes, including framing and per-payload hashes.
pub const MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_ARCHIVE_BYTES_V1: usize =
    MAX_PROVIDER_ARCHIVE_FRAMING_BYTES_V1 + MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1;

/// Maximum retained and recovery-only metadata for one V3 intent.
///
/// This is the record, compact replay transcript, provider-archive framing, the parsed provider
/// length/hash table, all provider `Vec` owners, and the four top-level attachment owners. It does
/// not include any handoff, provider payload, or finalized output bytes.
pub const MAX_WORKER_V3_PUBLICATION_INTENT_METADATA_BYTES_V1: usize =
    MAX_WORKER_V3_PUBLICATION_INTENT_RECORD_BYTES_V1
        + MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1
        + MAX_PROVIDER_ARCHIVE_FRAMING_BYTES_V1
        + MAX_PROVIDER_RECOVERY_BOOKKEEPING_BYTES_V1
        + RECOVERY_ATTACHMENT_OWNER_BYTES_V1;

/// Maximum retained bytes allocated by one restart recovery.
///
/// This is the checked sum of storage-layout byte owners plus bounded retained and recovery-only
/// metadata: one outer handoff entry, each provider archive payload entry, one output entry, and
/// [`MAX_WORKER_V3_PUBLICATION_INTENT_METADATA_BYTES_V1`]. Raw output must be deterministically
/// derived from finalized output; no canonical request or response aggregate is included.
pub const MAX_WORKER_V3_PUBLICATION_INTENT_RECOVERY_BYTES_V1: usize =
    MAX_COMPILER_MODULE_HANDOFF_BYTES_V3
        + MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1
        + MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1
        + MAX_WORKER_V3_PUBLICATION_INTENT_METADATA_BYTES_V1;
const _: () = assert!(MAX_WORKER_V3_PUBLICATION_INTENT_RECOVERY_BYTES_V1 < 512 * 1024 * 1024);

/// Hard ceiling for caller-owned `Vec` backing allocations accepted or returned by one operation.
///
/// This is distinct from the logical recovery working-set bound. It adds the capacities of the
/// outer handoff, transcript, finalized output, every provider payload, and the provider-list
/// backing allocation. On a 64-bit target the formula is:
///
/// `handoff max + transcript max + output max + provider-payload max + 127 * size_of::<Vec<u8>>()`.
pub const MAX_WORKER_V3_PUBLICATION_INTENT_CALLER_OWNER_CAPACITY_BYTES_V1: usize =
    MAX_COMPILER_MODULE_HANDOFF_BYTES_V3
        + MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1
        + MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1
        + MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1
        + MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1 * std::mem::size_of::<Vec<u8>>();

/// Failure to encode or strictly decode a Worker V3 publication-intent record.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3PublicationIntentCodecErrorV1 {
    /// The byte slice is shorter or longer than the one canonical V1 representation.
    NoncanonicalLength { actual: usize, expected: usize },
    /// The record belongs to another protocol.
    MagicMismatch,
    /// The record uses an unsupported schema version.
    UnsupportedVersion { actual: u16 },
    /// The domain-separated record checksum does not match.
    ChecksumMismatch,
    /// Fixed-width decoding overflowed or encountered a truncated field.
    TruncatedField,
    /// The encoded build occurrence is not a valid non-direct attempt.
    InvalidAttempt,
    /// The encoded occurrence key is not derived from the encoded producer and attempt.
    OccurrenceKeyMismatch,
    /// The outer-handoff length is outside the V3 storage bound.
    InvalidOuterHandoffLength { actual: u64, maximum: usize },
    /// The provider count is outside the V3 storage bound.
    InvalidExternalProviderCount { actual: u64, maximum: usize },
    /// Aggregate external-provider payload bytes are outside the V3 storage bound.
    InvalidExternalProviderPayloadLength { actual: u64, maximum: usize },
    /// The canonical external-provider archive length is outside the V3 storage bound.
    InvalidExternalProviderArchiveLength { actual: u64, maximum: usize },
    /// The output length is outside the V3 storage bound.
    InvalidOutputLength { actual: u64, maximum: usize },
    /// The transcript length is outside the V3 storage bound.
    InvalidTranscriptLength { actual: u64, maximum: usize },
    /// The outer-handoff owner reserved more backing bytes than the component permits.
    InvalidOuterHandoffCapacity { actual: usize, maximum: usize },
    /// The provider-list owner reserved more entries than the protocol permits.
    InvalidExternalProviderListCapacity { actual: usize, maximum: usize },
    /// One provider owner reserved more bytes than the aggregate provider ceiling.
    InvalidExternalProviderPayloadCapacity {
        index: usize,
        actual: usize,
        maximum: usize,
    },
    /// Provider payload owners collectively reserve more bytes than the provider ceiling.
    InvalidExternalProviderAggregateCapacity { actual: usize, maximum: usize },
    /// The transcript owner reserved more backing bytes than the component permits.
    InvalidTranscriptCapacity { actual: usize, maximum: usize },
    /// The finalized-output owner reserved more backing bytes than the component permits.
    InvalidOutputCapacity { actual: usize, maximum: usize },
    /// Checked caller-owner capacity arithmetic overflowed.
    OwnerCapacityArithmeticOverflow,
    /// Caller-owned backing allocations exceed the independent hard owner ceiling.
    OwnerCapacityBudgetExceeded { required: usize, maximum: usize },
    /// The encoded plan commitment does not match the complete plan fields.
    PlanCommitmentMismatch,
    /// The separately retained output hash does not match the durable plan.
    OutputPlanMismatch,
    /// Checked attachment or working-set arithmetic overflowed.
    LengthArithmeticOverflow,
    /// Unique retained bytes exceed the complete recovery working-set ceiling.
    RecoveryBudgetExceeded { required: usize, maximum: usize },
    /// The provider archive belongs to another protocol.
    ProviderArchiveMagicMismatch,
    /// The provider archive uses an unsupported schema version.
    UnsupportedProviderArchiveVersion { actual: u16 },
    /// The provider archive checksum does not match its complete body.
    ProviderArchiveChecksumMismatch,
    /// One provider payload does not match its archive entry hash.
    ProviderPayloadDigestMismatch { index: usize },
    /// A bounded codec allocation could not be reserved.
    AllocationFailed { requested: usize },
}

impl fmt::Display for WorkerV3PublicationIntentCodecErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoncanonicalLength { actual, expected } => write!(
                formatter,
                "Worker V3 publication-intent record length {actual} is not canonical {expected}"
            ),
            Self::MagicMismatch => {
                formatter.write_str("Worker V3 publication-intent record magic mismatch")
            }
            Self::UnsupportedVersion { actual } => write!(
                formatter,
                "unsupported Worker V3 publication-intent record version {actual}"
            ),
            Self::ChecksumMismatch => {
                formatter.write_str("Worker V3 publication-intent record checksum mismatch")
            }
            Self::TruncatedField => formatter
                .write_str("Worker V3 publication-intent record contains a truncated field"),
            Self::InvalidAttempt => formatter.write_str(
                "Worker V3 publication-intent record contains an invalid build occurrence",
            ),
            Self::OccurrenceKeyMismatch => formatter.write_str(
                "Worker V3 publication-intent occurrence key does not match its producer and attempt",
            ),
            Self::InvalidOuterHandoffLength { actual, maximum } => write!(
                formatter,
                "Worker V3 outer handoff length {actual} is outside 1..={maximum}"
            ),
            Self::InvalidExternalProviderCount { actual, maximum } => write!(
                formatter,
                "Worker V3 external provider count {actual} exceeds {maximum}"
            ),
            Self::InvalidExternalProviderPayloadLength { actual, maximum } => write!(
                formatter,
                "Worker V3 aggregate external provider payload length {actual} exceeds {maximum}"
            ),
            Self::InvalidExternalProviderArchiveLength { actual, maximum } => write!(
                formatter,
                "Worker V3 external provider archive length {actual} is outside its canonical bound {maximum}"
            ),
            Self::InvalidOutputLength { actual, maximum } => write!(
                formatter,
                "Worker V3 publication-intent output length {actual} is outside 1..={maximum}"
            ),
            Self::InvalidTranscriptLength { actual, maximum } => write!(
                formatter,
                "Worker V3 compact finalizer replay metadata length {actual} is outside 1..={maximum}"
            ),
            Self::InvalidOuterHandoffCapacity { actual, maximum } => write!(
                formatter,
                "Worker V3 outer-handoff owner capacity {actual} exceeds {maximum}"
            ),
            Self::InvalidExternalProviderListCapacity { actual, maximum } => write!(
                formatter,
                "Worker V3 provider-list owner capacity {actual} exceeds {maximum}"
            ),
            Self::InvalidExternalProviderPayloadCapacity {
                index,
                actual,
                maximum,
            } => write!(
                formatter,
                "Worker V3 provider payload {index} owner capacity {actual} exceeds {maximum}"
            ),
            Self::InvalidExternalProviderAggregateCapacity { actual, maximum } => write!(
                formatter,
                "Worker V3 aggregate provider owner capacity {actual} exceeds {maximum}"
            ),
            Self::InvalidTranscriptCapacity { actual, maximum } => write!(
                formatter,
                "Worker V3 transcript owner capacity {actual} exceeds {maximum}"
            ),
            Self::InvalidOutputCapacity { actual, maximum } => write!(
                formatter,
                "Worker V3 finalized-output owner capacity {actual} exceeds {maximum}"
            ),
            Self::OwnerCapacityArithmeticOverflow => formatter
                .write_str("Worker V3 caller-owner capacity arithmetic overflowed"),
            Self::OwnerCapacityBudgetExceeded { required, maximum } => write!(
                formatter,
                "Worker V3 caller-owned backing allocations require {required} bytes, exceeding {maximum}"
            ),
            Self::PlanCommitmentMismatch => formatter.write_str(
                "Worker V3 publication-intent plan commitment does not match its plan fields",
            ),
            Self::OutputPlanMismatch => formatter.write_str(
                "Worker V3 publication-intent output hash does not match the durable plan",
            ),
            Self::LengthArithmeticOverflow => formatter
                .write_str("Worker V3 publication-intent length arithmetic overflowed"),
            Self::RecoveryBudgetExceeded { required, maximum } => write!(
                formatter,
                "Worker V3 storage-layout recovery working set {required} exceeds {maximum}"
            ),
            Self::ProviderArchiveMagicMismatch => {
                formatter.write_str("Worker V3 external-provider archive magic mismatch")
            }
            Self::UnsupportedProviderArchiveVersion { actual } => write!(
                formatter,
                "unsupported Worker V3 external-provider archive version {actual}"
            ),
            Self::ProviderArchiveChecksumMismatch => formatter
                .write_str("Worker V3 external-provider archive checksum mismatch"),
            Self::ProviderPayloadDigestMismatch { index } => write!(
                formatter,
                "Worker V3 external-provider payload {index} does not match its archive hash"
            ),
            Self::AllocationFailed { requested } => write!(
                formatter,
                "could not reserve {requested} bytes for a Worker V3 publication-intent record"
            ),
        }
    }
}

impl std::error::Error for WorkerV3PublicationIntentCodecErrorV1 {}

/// SHA-256 identity of one complete canonical checksummed V1 record.
///
/// This identifies storage bytes only. It is not a semantic or authenticity identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerV3PublicationIntentIdentityV1([u8; 32]);

impl WorkerV3PublicationIntentIdentityV1 {
    /// Constructs an identity from its exact representation.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the exact representation.
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Ordered external-provider payloads retained exactly once for Worker V3 replay.
///
/// The order and exact bytes are storage data only. This crate does not assign provider kinds,
/// authenticate provider origin, or establish that these payloads match replay metadata.
#[derive(Debug, Eq, PartialEq)]
pub struct WorkerV3ExternalProviderPayloadsV1 {
    payloads: Vec<Vec<u8>>,
    payload_length: usize,
    canonical_length: usize,
    canonical_sha256: [u8; 32],
}

impl WorkerV3ExternalProviderPayloadsV1 {
    /// Validates an ordered set without concatenating or copying its payload bytes.
    pub fn new(payloads: Vec<Vec<u8>>) -> Result<Self, WorkerV3PublicationIntentCodecErrorV1> {
        let count = payloads.len();
        if count > MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1 {
            return Err(
                WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderCount {
                    actual: count as u64,
                    maximum: MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1,
                },
            );
        }
        provider_owner_capacity_bytes(&payloads)?;
        let payload_length = payloads.iter().try_fold(0_usize, |total, payload| {
            if payload.is_empty() {
                return Err(
                    WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderPayloadLength {
                        actual: 0,
                        maximum: MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
                    },
                );
            }
            total.checked_add(payload.len()).ok_or(
                WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderPayloadLength {
                    actual: u64::MAX,
                    maximum: MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
                },
            )
        })?;
        if payload_length > MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1 {
            return Err(
                WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderPayloadLength {
                    actual: payload_length as u64,
                    maximum: MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
                },
            );
        }
        let canonical_length = provider_archive_length(count, payload_length)?;
        let checksum = provider_archive_checksum(&payloads, payload_length);
        let canonical_sha256 = provider_archive_sha256(&payloads, payload_length, checksum);
        Ok(Self {
            payloads,
            payload_length,
            canonical_length,
            canonical_sha256,
        })
    }

    /// Returns the number of ordered provider payloads.
    pub fn len(&self) -> usize {
        self.payloads.len()
    }

    /// Reports whether there are no external provider payloads.
    pub fn is_empty(&self) -> bool {
        self.payloads.is_empty()
    }

    /// Returns one exact provider payload by canonical archive position.
    pub fn get(&self, index: usize) -> Option<&[u8]> {
        self.payloads.get(index).map(Vec::as_slice)
    }

    /// Iterates over exact provider payloads without copying them.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &[u8]> {
        self.payloads.iter().map(Vec::as_slice)
    }

    /// Returns aggregate provider payload bytes, excluding archive framing.
    pub const fn payload_length(&self) -> usize {
        self.payload_length
    }

    /// Returns the exact canonical provider-archive length.
    pub const fn canonical_length(&self) -> usize {
        self.canonical_length
    }

    /// Returns raw SHA-256 of the complete canonical provider archive.
    pub const fn canonical_sha256(&self) -> [u8; 32] {
        self.canonical_sha256
    }

    fn caller_owner_capacity_bytes(&self) -> Result<usize, WorkerV3PublicationIntentCodecErrorV1> {
        provider_owner_capacity_bytes(&self.payloads)
    }

    /// Consumes the archive owner and returns each exact payload without copying it.
    pub fn into_payloads(self) -> Vec<Vec<u8>> {
        self.payloads
    }
}

/// Storage-layout byte owners needed by a higher-level finalizer to replay one Worker V3 result.
///
/// `canonical_replay_transcript` is compact metadata only. It must not contain copies of the outer
/// handoff, provider payloads, finalized output, or complete canonical worker request and response
/// aggregates. Raw output is derived from finalized output by the higher-level finalizer. This
/// lower-level crate enforces the metadata byte ceiling but cannot establish that caller-supplied
/// bytes obey that semantic restriction.
#[derive(Debug, Eq, PartialEq)]
pub struct WorkerV3FinalizerReplayAttachmentsV1 {
    outer_handoff: Vec<u8>,
    external_providers: WorkerV3ExternalProviderPayloadsV1,
    canonical_replay_transcript: Vec<u8>,
}

impl WorkerV3FinalizerReplayAttachmentsV1 {
    /// Validates compact replay attachments without creating request/response aggregates.
    pub fn new(
        outer_handoff: Vec<u8>,
        external_provider_payloads: Vec<Vec<u8>>,
        canonical_replay_transcript: Vec<u8>,
    ) -> Result<Self, WorkerV3PublicationIntentCodecErrorV1> {
        let external_providers =
            WorkerV3ExternalProviderPayloadsV1::new(external_provider_payloads)?;
        validate_payload_lengths(
            outer_handoff.len(),
            external_providers.canonical_length(),
            external_providers.len(),
            external_providers.payload_length(),
            canonical_replay_transcript.len(),
            1,
        )?;
        validate_caller_owner_capacities(
            &outer_handoff,
            &external_providers,
            &canonical_replay_transcript,
            None,
        )?;
        Ok(Self {
            outer_handoff,
            external_providers,
            canonical_replay_transcript,
        })
    }

    /// Borrows the exact outer semantic handoff bytes.
    pub fn outer_handoff(&self) -> &[u8] {
        &self.outer_handoff
    }

    /// Borrows the ordered external provider payload owner.
    pub const fn external_providers(&self) -> &WorkerV3ExternalProviderPayloadsV1 {
        &self.external_providers
    }

    /// Borrows compact opaque metadata used by the finalizer to reconstruct replay wires.
    pub fn canonical_replay_transcript(&self) -> &[u8] {
        &self.canonical_replay_transcript
    }

    /// This storage owner does not authenticate the finalizer transcript or its components.
    pub const fn authenticates_finalizer_transcript(&self) -> bool {
        false
    }

    /// Consumes the owner and returns its three storage-layout component owners.
    pub fn into_parts(self) -> (Vec<u8>, Vec<Vec<u8>>, Vec<u8>) {
        (
            self.outer_handoff,
            self.external_providers.into_payloads(),
            self.canonical_replay_transcript,
        )
    }
}

/// Canonical inert storage record for exact Worker V3 restart inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3PublicationIntentRecordV1 {
    occurrence_key: [u8; 32],
    attempt: BuildAttempt,
    producer_key: [u8; 32],
    plan: DurableLinkPublicationPlanV1,
    outer_handoff_sha256: [u8; 32],
    outer_handoff_length: usize,
    external_provider_archive_sha256: [u8; 32],
    external_provider_archive_length: usize,
    external_provider_count: usize,
    external_provider_payload_length: usize,
    transcript_sha256: [u8; 32],
    transcript_length: usize,
    output_sha256: [u8; 32],
    output_length: usize,
    identity: WorkerV3PublicationIntentIdentityV1,
}

impl WorkerV3PublicationIntentRecordV1 {
    /// Returns the exact build occurrence bound by this storage record.
    pub const fn attempt(self) -> BuildAttempt {
        self.attempt
    }

    /// Returns the producer storage key derived from its exact source and crate-name spelling.
    ///
    /// This key is a namespace binding, not proof of producer authorship.
    pub const fn producer_key(self) -> [u8; 32] {
        self.producer_key
    }

    /// Returns the storage key binding the producer to this exact build occurrence.
    pub const fn occurrence_key(self) -> [u8; 32] {
        self.occurrence_key
    }

    /// Returns the complete durable publication plan carried without reinterpretation.
    pub const fn plan(self) -> DurableLinkPublicationPlanV1 {
        self.plan
    }

    /// Returns raw SHA-256 of the exact retained outer-handoff bytes.
    pub const fn outer_handoff_sha256(self) -> [u8; 32] {
        self.outer_handoff_sha256
    }

    /// Returns the exact retained outer-handoff length.
    pub const fn outer_handoff_length(self) -> usize {
        self.outer_handoff_length
    }

    /// Returns raw SHA-256 of the canonical external-provider archive.
    pub const fn external_provider_archive_sha256(self) -> [u8; 32] {
        self.external_provider_archive_sha256
    }

    /// Returns the exact canonical external-provider archive length.
    pub const fn external_provider_archive_length(self) -> usize {
        self.external_provider_archive_length
    }

    /// Returns the number of provider payloads retained exactly once in archive order.
    pub const fn external_provider_count(self) -> usize {
        self.external_provider_count
    }

    /// Returns aggregate bytes across provider payloads, excluding archive framing.
    pub const fn external_provider_payload_length(self) -> usize {
        self.external_provider_payload_length
    }

    /// Returns raw SHA-256 of the exact finalized output bytes.
    pub const fn output_sha256(self) -> [u8; 32] {
        self.output_sha256
    }

    /// Returns the exact finalized output length.
    pub const fn output_length(self) -> usize {
        self.output_length
    }

    /// Returns raw SHA-256 of the exact compact opaque replay-metadata bytes.
    pub const fn transcript_sha256(self) -> [u8; 32] {
        self.transcript_sha256
    }

    /// Returns the exact compact opaque replay-metadata length.
    pub const fn transcript_length(self) -> usize {
        self.transcript_length
    }

    /// Returns the domain-separated identity of the complete checksummed storage record.
    pub const fn identity(self) -> WorkerV3PublicationIntentIdentityV1 {
        self.identity
    }

    /// A storage record does not establish that its opaque transcript is authentic or canonical.
    pub const fn authenticates_finalizer_transcript(self) -> bool {
        false
    }

    /// A storage record grants no publication authority.
    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    /// A storage record grants no module-loading authority.
    pub const fn grants_load_authority(self) -> bool {
        false
    }

    /// A storage record grants no kernel-launch authority.
    pub const fn grants_launch_authority(self) -> bool {
        false
    }

    fn from_exact_bytes(
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        plan: DurableLinkPublicationPlanV1,
        attachments: &WorkerV3FinalizerReplayAttachmentsV1,
        exact_output: &[u8],
    ) -> Result<Self, WorkerV3PublicationIntentCodecErrorV1> {
        validate_payload_lengths(
            attachments.outer_handoff.len(),
            attachments.external_providers.canonical_length(),
            attachments.external_providers.len(),
            attachments.external_providers.payload_length(),
            attachments.canonical_replay_transcript.len(),
            exact_output.len(),
        )?;
        let producer_key = producer_key(producer);
        let mut record = Self {
            occurrence_key: occurrence_key(producer_key, attempt),
            attempt,
            producer_key,
            plan,
            outer_handoff_sha256: sha256(&attachments.outer_handoff),
            outer_handoff_length: attachments.outer_handoff.len(),
            external_provider_archive_sha256: attachments.external_providers.canonical_sha256(),
            external_provider_archive_length: attachments.external_providers.canonical_length(),
            external_provider_count: attachments.external_providers.len(),
            external_provider_payload_length: attachments.external_providers.payload_length(),
            transcript_sha256: sha256(&attachments.canonical_replay_transcript),
            transcript_length: attachments.canonical_replay_transcript.len(),
            output_sha256: sha256(exact_output),
            output_length: exact_output.len(),
            identity: WorkerV3PublicationIntentIdentityV1([0; 32]),
        };
        if record.output_sha256 != *plan.finalized_output().as_bytes() {
            return Err(WorkerV3PublicationIntentCodecErrorV1::OutputPlanMismatch);
        }
        record.identity = record.encoded_identity()?;
        Ok(record)
    }

    /// Encodes this record into its one fixed-width canonical V1 representation.
    pub fn encode_canonical(self) -> Result<Vec<u8>, WorkerV3PublicationIntentCodecErrorV1> {
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(RECORD_BYTES_V1).map_err(|_| {
            WorkerV3PublicationIntentCodecErrorV1::AllocationFailed {
                requested: RECORD_BYTES_V1,
            }
        })?;
        self.encode_body(&mut bytes);
        let checksum = sha256_parts(&[RECORD_CHECKSUM_DOMAIN_V1, &bytes]);
        bytes.extend_from_slice(&checksum);
        debug_assert_eq!(bytes.len(), RECORD_BYTES_V1);
        Ok(bytes)
    }

    /// Strictly decodes one canonical V1 record without interpreting transcript content.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, WorkerV3PublicationIntentCodecErrorV1> {
        if bytes.len() != RECORD_BYTES_V1 {
            return Err(WorkerV3PublicationIntentCodecErrorV1::NoncanonicalLength {
                actual: bytes.len(),
                expected: RECORD_BYTES_V1,
            });
        }
        let body_length = bytes
            .len()
            .checked_sub(32)
            .ok_or(WorkerV3PublicationIntentCodecErrorV1::TruncatedField)?;
        let (body, checksum) = bytes.split_at(body_length);
        if sha256_parts(&[RECORD_CHECKSUM_DOMAIN_V1, body]).as_slice() != checksum {
            return Err(WorkerV3PublicationIntentCodecErrorV1::ChecksumMismatch);
        }

        let mut decoder = Decoder::new(body);
        if decoder.take(RECORD_MAGIC_V1.len())? != RECORD_MAGIC_V1 {
            return Err(WorkerV3PublicationIntentCodecErrorV1::MagicMismatch);
        }
        let version = decoder.u16()?;
        if version != RECORD_VERSION_V1 {
            return Err(WorkerV3PublicationIntentCodecErrorV1::UnsupportedVersion {
                actual: version,
            });
        }
        let encoded_occurrence_key = decoder.array()?;
        let attempt = BuildAttempt::new(
            decoder.u64()?,
            BuildSession::from_bytes(decoder.array()?),
            BuildInvocation::from_bytes(decoder.array()?),
        )
        .map_err(|_| WorkerV3PublicationIntentCodecErrorV1::InvalidAttempt)?;
        if attempt.session() == BuildSession::DIRECT {
            return Err(WorkerV3PublicationIntentCodecErrorV1::InvalidAttempt);
        }
        let producer_key = decoder.array()?;
        if encoded_occurrence_key != occurrence_key(producer_key, attempt) {
            return Err(WorkerV3PublicationIntentCodecErrorV1::OccurrenceKeyMismatch);
        }
        let committed_plan_identity: [u8; 32] = decoder.array()?;
        let scope = LinkPublicationScopeV1::new(
            PackageIdentityV1::from_bytes(decoder.array()?),
            KernelSetIdentityV1::from_bytes(decoder.array()?),
            TargetIdentityV1::from_bytes(decoder.array()?),
        );
        let plan = DurableLinkPublicationPlanV1::new(
            attempt,
            scope,
            CanonicalLinkRequestIdentityV1::from_bytes(decoder.array()?),
            PinnedWorkerIdentityV1::from_bytes(decoder.array()?),
            ValidatedResponseIdentityV1::from_bytes(decoder.array()?),
            LinkedOutputIdentityV1::from_bytes(decoder.array()?),
            FinalizationIdentityV1::from_bytes(decoder.array()?),
            FinalizedOutputIdentityV1::from_bytes(decoder.array()?),
            AtomicPublicationIdentityV1::from_bytes(decoder.array()?),
        );
        let outer_handoff_sha256 = decoder.array()?;
        let outer_handoff_length_u64 = decoder.u64()?;
        let external_provider_archive_sha256 = decoder.array()?;
        let external_provider_archive_length_u64 = decoder.u64()?;
        let external_provider_count_u32 = decoder.u32()?;
        let external_provider_payload_length_u64 = decoder.u64()?;
        let transcript_sha256 = decoder.array()?;
        let transcript_length_u64 = decoder.u64()?;
        let output_sha256 = decoder.array()?;
        let output_length_u64 = decoder.u64()?;
        if !decoder.finished() {
            return Err(WorkerV3PublicationIntentCodecErrorV1::TruncatedField);
        }
        let outer_handoff_length = bounded_length(
            outer_handoff_length_u64,
            MAX_COMPILER_MODULE_HANDOFF_BYTES_V3,
        )
        .map_err(|()| {
            WorkerV3PublicationIntentCodecErrorV1::InvalidOuterHandoffLength {
                actual: outer_handoff_length_u64,
                maximum: MAX_COMPILER_MODULE_HANDOFF_BYTES_V3,
            }
        })?;
        let external_provider_count = external_provider_count_u32 as usize;
        if external_provider_count > MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1 {
            return Err(
                WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderCount {
                    actual: external_provider_count as u64,
                    maximum: MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1,
                },
            );
        }
        let external_provider_payload_length = bounded_length_allow_zero(
            external_provider_payload_length_u64,
            MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
        )
        .map_err(|()| {
            WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderPayloadLength {
                actual: external_provider_payload_length_u64,
                maximum: MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
            }
        })?;
        if (external_provider_count == 0) != (external_provider_payload_length == 0) {
            return Err(
                WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderPayloadLength {
                    actual: external_provider_payload_length_u64,
                    maximum: MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
                },
            );
        }
        let external_provider_archive_length =
            usize::try_from(external_provider_archive_length_u64).map_err(|_| {
                WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderArchiveLength {
                    actual: external_provider_archive_length_u64,
                    maximum: MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_ARCHIVE_BYTES_V1,
                }
            })?;
        let expected_provider_archive_length =
            provider_archive_length(external_provider_count, external_provider_payload_length)?;
        if external_provider_archive_length != expected_provider_archive_length {
            return Err(
                WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderArchiveLength {
                    actual: external_provider_archive_length_u64,
                    maximum: expected_provider_archive_length,
                },
            );
        }
        let output_length = bounded_length(
            output_length_u64,
            MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1,
        )
        .map_err(
            |()| WorkerV3PublicationIntentCodecErrorV1::InvalidOutputLength {
                actual: output_length_u64,
                maximum: MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1,
            },
        )?;
        let transcript_length = bounded_length(
            transcript_length_u64,
            MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1,
        )
        .map_err(
            |()| WorkerV3PublicationIntentCodecErrorV1::InvalidTranscriptLength {
                actual: transcript_length_u64,
                maximum: MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1,
            },
        )?;
        validate_payload_lengths(
            outer_handoff_length,
            external_provider_archive_length,
            external_provider_count,
            external_provider_payload_length,
            transcript_length,
            output_length,
        )?;
        if committed_plan_identity != plan.identity() {
            return Err(WorkerV3PublicationIntentCodecErrorV1::PlanCommitmentMismatch);
        }
        if output_sha256 != *plan.finalized_output().as_bytes() {
            return Err(WorkerV3PublicationIntentCodecErrorV1::OutputPlanMismatch);
        }
        let mut record = Self {
            occurrence_key: encoded_occurrence_key,
            attempt,
            producer_key,
            plan,
            outer_handoff_sha256,
            outer_handoff_length,
            external_provider_archive_sha256,
            external_provider_archive_length,
            external_provider_count,
            external_provider_payload_length,
            transcript_sha256,
            transcript_length,
            output_sha256,
            output_length,
            identity: WorkerV3PublicationIntentIdentityV1([0; 32]),
        };
        record.identity =
            WorkerV3PublicationIntentIdentityV1(sha256_parts(&[RECORD_IDENTITY_DOMAIN_V1, bytes]));
        Ok(record)
    }

    fn encode_body(self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(RECORD_MAGIC_V1);
        bytes.extend_from_slice(&RECORD_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&self.occurrence_key);
        bytes.extend_from_slice(&self.attempt.generation().to_le_bytes());
        bytes.extend_from_slice(self.attempt.session().as_bytes());
        bytes.extend_from_slice(self.attempt.invocation().as_bytes());
        bytes.extend_from_slice(&self.producer_key);
        bytes.extend_from_slice(&self.plan.identity());
        push_scope(bytes, self.plan.scope());
        bytes.extend_from_slice(self.plan.request().as_bytes());
        bytes.extend_from_slice(self.plan.worker().as_bytes());
        bytes.extend_from_slice(self.plan.response().as_bytes());
        bytes.extend_from_slice(self.plan.linked_output().as_bytes());
        bytes.extend_from_slice(self.plan.finalization().as_bytes());
        bytes.extend_from_slice(self.plan.finalized_output().as_bytes());
        bytes.extend_from_slice(self.plan.publication().as_bytes());
        bytes.extend_from_slice(&self.outer_handoff_sha256);
        bytes.extend_from_slice(&(self.outer_handoff_length as u64).to_le_bytes());
        bytes.extend_from_slice(&self.external_provider_archive_sha256);
        bytes.extend_from_slice(&(self.external_provider_archive_length as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.external_provider_count as u32).to_le_bytes());
        bytes.extend_from_slice(&(self.external_provider_payload_length as u64).to_le_bytes());
        bytes.extend_from_slice(&self.transcript_sha256);
        bytes.extend_from_slice(&(self.transcript_length as u64).to_le_bytes());
        bytes.extend_from_slice(&self.output_sha256);
        bytes.extend_from_slice(&(self.output_length as u64).to_le_bytes());
    }

    fn encoded_identity(
        self,
    ) -> Result<WorkerV3PublicationIntentIdentityV1, WorkerV3PublicationIntentCodecErrorV1> {
        Ok(WorkerV3PublicationIntentIdentityV1(sha256_parts(&[
            RECORD_IDENTITY_DOMAIN_V1,
            &self.encode_canonical()?,
        ])))
    }
}

/// Whether a persistence call created a new intent or recovered an exact prior commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerV3PublicationIntentOutcomeV1 {
    /// This call committed a new record after every exact attachment became durable.
    Persisted,
    /// This call recovered an exact record committed by an earlier process.
    Recovered,
}

/// Result of safely scavenging one exact uncommitted V3 occurrence namespace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerV3PublicationIntentScavengeOutcomeV1 {
    /// The exact namespace contained no uncommitted canonical or temporary entries.
    NotFound,
    /// Private entries in the exact namespace were removed and the directory was synchronized.
    Removed { entries: usize },
}

/// Inert exact bytes recovered under the artifact-store lock.
#[derive(Debug, Eq, PartialEq)]
pub struct RecoveredWorkerV3PublicationIntentV1 {
    outcome: WorkerV3PublicationIntentOutcomeV1,
    record: WorkerV3PublicationIntentRecordV1,
    replay_attachments: WorkerV3FinalizerReplayAttachmentsV1,
    exact_output: Vec<u8>,
}

impl RecoveredWorkerV3PublicationIntentV1 {
    /// Reports whether this call committed or recovered the intent.
    pub const fn outcome(&self) -> WorkerV3PublicationIntentOutcomeV1 {
        self.outcome
    }

    /// Returns the validated canonical storage record.
    pub const fn record(&self) -> WorkerV3PublicationIntentRecordV1 {
        self.record
    }

    /// Borrows the exact finalized output bytes retained beside the record.
    pub fn exact_output(&self) -> &[u8] {
        &self.exact_output
    }

    /// Borrows all stored finalizer replay attachments.
    pub const fn replay_attachments(&self) -> &WorkerV3FinalizerReplayAttachmentsV1 {
        &self.replay_attachments
    }

    /// Borrows the exact outer V3 handoff bytes.
    pub fn outer_handoff(&self) -> &[u8] {
        self.replay_attachments.outer_handoff()
    }

    /// Borrows each exact external provider payload in canonical archive order.
    pub const fn external_providers(&self) -> &WorkerV3ExternalProviderPayloadsV1 {
        self.replay_attachments.external_providers()
    }

    /// Borrows the exact compact opaque finalizer reconstruction metadata.
    pub fn finalizer_replay_transcript(&self) -> &[u8] {
        self.replay_attachments.canonical_replay_transcript()
    }

    /// Consumes this inert result without copying any large attachment.
    pub fn into_parts(
        self,
    ) -> (
        WorkerV3PublicationIntentRecordV1,
        WorkerV3FinalizerReplayAttachmentsV1,
        Vec<u8>,
    ) {
        (self.record, self.replay_attachments, self.exact_output)
    }

    /// Recovery does not establish transcript authenticity or canonicality.
    pub const fn authenticates_finalizer_transcript(&self) -> bool {
        false
    }

    /// Recovery grants no publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// Recovery grants no module-loading authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Recovery grants no kernel-launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Filesystem or codec invariant violated by a named V3 intent entry.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3PublicationIntentInvalidReasonV1 {
    /// A canonical and replayable redo record coexist for one occurrence.
    CanonicalAndRedoCoexist,
    /// A normal committed record and a retirement marker coexist.
    CommittedAndRetiringCoexist,
    /// A protocol entry is not a private single-link regular file.
    EntryNotPrivate,
    /// A file does not have the exact length committed by the protocol.
    FileLengthMismatch {
        actual: Option<u64>,
        expected: usize,
    },
    /// A pinned file or its name changed while it was being validated.
    FileChangedWhileRead,
    /// The canonical record failed strict decoding.
    RecordCodec(WorkerV3PublicationIntentCodecErrorV1),
    /// The canonical provider archive failed strict decoding.
    ProviderArchiveCodec(WorkerV3PublicationIntentCodecErrorV1),
    /// The decoded record does not belong to the requested producer occurrence.
    RecordBindingMismatch,
    /// A record disappeared after its commit operation completed.
    RecordDisappearedAfterCommit,
    /// Temporary cleanup encountered too many artifact-directory entries.
    DirectoryEntryLimitExceeded { maximum: usize },
    /// The artifact directory cannot accommodate every missing final intent entry.
    DirectoryEntryHeadroomInsufficient {
        actual: usize,
        required: usize,
        maximum: usize,
    },
    /// One occurrence accumulated too many temporary entries.
    TemporaryEntryLimitExceeded { maximum: usize },
    /// All bounded private temporary names were already occupied.
    TemporaryNameExhausted,
}

impl fmt::Display for WorkerV3PublicationIntentInvalidReasonV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CanonicalAndRedoCoexist => {
                formatter.write_str("canonical and redo records coexist")
            }
            Self::CommittedAndRetiringCoexist => {
                formatter.write_str("a committed record and retirement marker coexist")
            }
            Self::EntryNotPrivate => formatter
                .write_str("entry is not a private single-link regular file with mode 0600"),
            Self::FileLengthMismatch { actual, expected } => write!(
                formatter,
                "entry length {:?} does not match canonical length {expected}",
                actual
            ),
            Self::FileChangedWhileRead => {
                formatter.write_str("entry changed while its pinned descriptor was read")
            }
            Self::RecordCodec(error) => error.fmt(formatter),
            Self::ProviderArchiveCodec(error) => error.fmt(formatter),
            Self::RecordBindingMismatch => {
                formatter.write_str("record does not match the requested producer build occurrence")
            }
            Self::RecordDisappearedAfterCommit => {
                formatter.write_str("record disappeared after commit")
            }
            Self::DirectoryEntryLimitExceeded { maximum } => write!(
                formatter,
                "artifact directory exceeds the cleanup scan bound {maximum}"
            ),
            Self::DirectoryEntryHeadroomInsufficient {
                actual,
                required,
                maximum,
            } => write!(
                formatter,
                "artifact directory has {actual} entries and cannot reserve {required} missing Worker V3 final entries within {maximum}"
            ),
            Self::TemporaryEntryLimitExceeded { maximum } => write!(
                formatter,
                "publication occurrence exceeds the temporary-entry bound {maximum}"
            ),
            Self::TemporaryNameExhausted => {
                formatter.write_str("could not reserve a bounded private temporary name")
            }
        }
    }
}

/// Durable operation at which tests may simulate abrupt process termination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerV3PublicationIntentBoundaryV1 {
    CreateOuterHandoffTemp,
    WriteOuterHandoffTemp,
    SyncOuterHandoffTemp,
    RenameOuterHandoff,
    SyncOuterHandoffName,
    CreateExternalProvidersTemp,
    WriteExternalProvidersTemp,
    SyncExternalProvidersTemp,
    RenameExternalProviders,
    SyncExternalProvidersName,
    CreateTranscriptTemp,
    WriteTranscriptTemp,
    SyncTranscriptTemp,
    RenameTranscript,
    SyncTranscriptName,
    CreateOutputTemp,
    WriteOutputTemp,
    SyncOutputTemp,
    RenameOutput,
    SyncOutputName,
    CreateRecordTemp,
    WriteRecordTemp,
    SyncRecordTemp,
    RenameRecordToRedo,
    SyncRedoName,
    RenameRedoToCanonical,
    SyncCanonicalName,
    RenameRecordToRetiring,
    SyncRetiringName,
    RenameOuterHandoffToQuarantine,
    RemoveOuterHandoff,
    RenameExternalProvidersToQuarantine,
    RemoveExternalProviders,
    RenameTranscriptToQuarantine,
    RemoveTranscript,
    RenameOutputToQuarantine,
    RemoveOutput,
    SyncRetiredAttachments,
    RenameRetiringRecordToQuarantine,
    RemoveRetiringRecord,
    SyncRetirement,
}

/// Side of one durable operation on which a test interruption occurs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerV3PublicationIntentFaultTimingV1 {
    Before,
    After,
}

/// Exact deterministic crash point used by durability tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3PublicationIntentFaultPointV1 {
    pub boundary: WorkerV3PublicationIntentBoundaryV1,
    pub timing: WorkerV3PublicationIntentFaultTimingV1,
}

/// Fault-injection options. Production callers use [`Default::default`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkerV3PublicationIntentOptionsV1 {
    injected_crash: Option<WorkerV3PublicationIntentFaultPointV1>,
}

impl WorkerV3PublicationIntentOptionsV1 {
    /// Simulates abrupt termination at one exact durability boundary.
    pub const fn inject_crash(point: WorkerV3PublicationIntentFaultPointV1) -> Self {
        Self {
            injected_crash: Some(point),
        }
    }
}

/// Failure to persist or inertly recover exact Worker V3 restart inputs.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3PublicationIntentErrorV1 {
    /// The shared artifact store rejected an operation or changed identity.
    Store(crate::EmitError),
    /// A descriptor-relative filesystem operation failed.
    Io(std::io::Error),
    /// Record construction or decoding failed closed.
    Codec(WorkerV3PublicationIntentCodecErrorV1),
    /// The publication plan names another build occurrence.
    PlanAttemptMismatch,
    /// Exact output bytes do not match the plan's finalized-output hash.
    OutputDigestMismatch,
    /// Exact outer-handoff bytes do not match the committed hash.
    OuterHandoffDigestMismatch,
    /// The canonical provider archive does not match the committed hash or structure.
    ExternalProviderArchiveMismatch,
    /// Exact compact transcript bytes do not match the committed transcript hash.
    TranscriptDigestMismatch,
    /// Checked aggregate recovery-size accounting overflowed.
    WorkingSetArithmeticOverflow,
    /// Aggregate record and payload bytes exceed the independent recovery budget.
    WorkingSetBudgetExceeded { required: usize, maximum: usize },
    /// The bounded record or payload allocation failed.
    AllocationFailed {
        component: &'static str,
        requested: usize,
    },
    /// The current attempt registry rejects this producer occurrence.
    Attempt { reason: String },
    /// The attempt registry does not authorize cleanup of the requested occurrence namespace.
    ScavengeNotAuthorized,
    /// A current committed canonical or replayable record cannot be scavenged without its receipt.
    CommittedIntentCannotBeScavenged,
    /// The supplied record identity does not name the exact committed intent.
    IdentityMismatch,
    /// The current occurrence has no exact completed backend publication receipt.
    ReceiptNotDurable,
    /// Durable cleanup has started and normal restart recovery is no longer available.
    RetirementInProgress,
    /// Marker-only retirement resume was requested before a durable retirement marker existed.
    RetirementNotInProgress,
    /// No committed canonical or replayable V3 intent exists.
    NotFound,
    /// Different exact inputs are already retained for this occurrence.
    ConflictingIntent,
    /// A named protocol entry is corrupt, substituted, or unsafe to follow.
    InvalidIntent {
        path: PathBuf,
        reason: WorkerV3PublicationIntentInvalidReasonV1,
    },
    /// Test-only deterministic interruption.
    InjectedCrash {
        point: WorkerV3PublicationIntentFaultPointV1,
    },
}

impl fmt::Display for WorkerV3PublicationIntentErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(
                formatter,
                "artifact store rejected Worker V3 publication intent: {error}"
            ),
            Self::Io(error) => error.fmt(formatter),
            Self::Codec(error) => error.fmt(formatter),
            Self::PlanAttemptMismatch => formatter
                .write_str("Worker V3 publication plan does not match the build occurrence"),
            Self::OutputDigestMismatch => formatter.write_str(
                "Worker V3 publication-intent output bytes do not match the committed hash",
            ),
            Self::OuterHandoffDigestMismatch => formatter.write_str(
                "Worker V3 publication-intent outer-handoff bytes do not match the committed hash",
            ),
            Self::ExternalProviderArchiveMismatch => formatter.write_str(
                "Worker V3 publication-intent provider archive does not match the committed record",
            ),
            Self::TranscriptDigestMismatch => formatter.write_str(
                "Worker V3 compact finalizer replay metadata does not match the committed hash",
            ),
            Self::WorkingSetArithmeticOverflow => formatter
                .write_str("Worker V3 publication-intent recovery-size accounting overflowed"),
            Self::WorkingSetBudgetExceeded { required, maximum } => write!(
                formatter,
                "Worker V3 publication-intent recovery requires {required} bytes, exceeding {maximum}"
            ),
            Self::AllocationFailed {
                component,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} bytes for Worker V3 publication-intent {component}"
            ),
            Self::Attempt { reason } => write!(
                formatter,
                "invalid Worker V3 publication-intent producer occurrence: {reason}"
            ),
            Self::ScavengeNotAuthorized => formatter.write_str(
                "the durable attempt registry does not authorize scavenging this Worker V3 occurrence",
            ),
            Self::CommittedIntentCannotBeScavenged => formatter.write_str(
                "a committed Worker V3 publication intent cannot be scavenged",
            ),
            Self::IdentityMismatch => formatter.write_str(
                "Worker V3 retirement named a different publication-intent identity",
            ),
            Self::ReceiptNotDurable => formatter.write_str(
                "the exact backend publication receipt is not durable for this Worker V3 intent",
            ),
            Self::RetirementInProgress => formatter.write_str(
                "Worker V3 publication-intent retirement is already in progress",
            ),
            Self::RetirementNotInProgress => formatter.write_str(
                "Worker V3 publication-intent retirement has no durable marker to resume",
            ),
            Self::NotFound => formatter.write_str("Worker V3 publication intent was not found"),
            Self::ConflictingIntent => formatter.write_str(
                "a different Worker V3 publication intent is already retained for this occurrence",
            ),
            Self::InvalidIntent { path, reason } => write!(
                formatter,
                "invalid Worker V3 publication intent {}: {reason}",
                path.display()
            ),
            Self::InjectedCrash { point } => write!(
                formatter,
                "injected Worker V3 publication-intent crash at {point:?}"
            ),
        }
    }
}

impl std::error::Error for WorkerV3PublicationIntentErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::Codec(error) => Some(error),
            _ => None,
        }
    }
}

impl From<crate::EmitError> for WorkerV3PublicationIntentErrorV1 {
    fn from(error: crate::EmitError) -> Self {
        Self::Store(error)
    }
}

impl From<std::io::Error> for WorkerV3PublicationIntentErrorV1 {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<WorkerV3PublicationIntentCodecErrorV1> for WorkerV3PublicationIntentErrorV1 {
    fn from(error: WorkerV3PublicationIntentCodecErrorV1) -> Self {
        Self::Codec(error)
    }
}

/// Persists one compact replay storage layout and exact finalized output for restart.
///
/// A fresh intent is accepted only while the exact producer occurrence remains in `Building`.
/// Exact recovery remains available after backend claim or completion. Inputs are accepted by value
/// so success returns the same large allocations without rereading durable files.
pub fn persist_worker_v3_publication_intent_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    replay_attachments: WorkerV3FinalizerReplayAttachmentsV1,
    exact_output: Vec<u8>,
) -> Result<RecoveredWorkerV3PublicationIntentV1, WorkerV3PublicationIntentErrorV1> {
    persist_worker_v3_publication_intent_v1_with_options(
        output_dir,
        producer,
        attempt,
        plan,
        replay_attachments,
        exact_output,
        WorkerV3PublicationIntentOptionsV1::default(),
    )
}

/// Fault-injectable form of [`persist_worker_v3_publication_intent_v1`].
pub fn persist_worker_v3_publication_intent_v1_with_options(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    replay_attachments: WorkerV3FinalizerReplayAttachmentsV1,
    exact_output: Vec<u8>,
    options: WorkerV3PublicationIntentOptionsV1,
) -> Result<RecoveredWorkerV3PublicationIntentV1, WorkerV3PublicationIntentErrorV1> {
    validate_persistence_inputs(attempt, plan, &replay_attachments, &exact_output)?;
    let expected = WorkerV3PublicationIntentRecordV1::from_exact_bytes(
        producer,
        attempt,
        plan,
        &replay_attachments,
        &exact_output,
    )?;
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    let phase = authorize_occurrence(&output, producer, attempt)?;
    let names = IntentNames::new(expected.producer_key(), expected.occurrence_key())?;
    if cleanup_temps(&output, &names)?
        .quarantined_retiring_record
        .is_some()
    {
        return Err(WorkerV3PublicationIntentErrorV1::RetirementInProgress);
    }

    if let Some(record) = recover_exact_locked(
        &output,
        &names,
        producer,
        attempt,
        expected,
        &replay_attachments,
        &exact_output,
    )? {
        return Ok(RecoveredWorkerV3PublicationIntentV1 {
            outcome: WorkerV3PublicationIntentOutcomeV1::Recovered,
            record,
            replay_attachments,
            exact_output,
        });
    }
    if phase != AttemptPhase::Building {
        return Err(WorkerV3PublicationIntentErrorV1::Attempt {
            reason: "a fresh intent may be created only before backend authority is claimed"
                .to_string(),
        });
    }
    reconcile_uncommitted_attachments(&output, &names, &replay_attachments, &exact_output)?;
    require_final_entry_headroom(&output, &names)?;

    let mut faults = FaultInjector::new(options.injected_crash);
    persist_payload(
        &output,
        &names,
        PayloadKind::OuterHandoff,
        replay_attachments.outer_handoff(),
        &mut faults,
    )?;
    persist_provider_archive(
        &output,
        &names,
        replay_attachments.external_providers(),
        &mut faults,
    )?;
    persist_payload(
        &output,
        &names,
        PayloadKind::Transcript,
        replay_attachments.canonical_replay_transcript(),
        &mut faults,
    )?;
    persist_payload(
        &output,
        &names,
        PayloadKind::Output,
        &exact_output,
        &mut faults,
    )?;
    persist_record(&output, &names, expected, &mut faults)?;
    let record = recover_exact_locked(
        &output,
        &names,
        producer,
        attempt,
        expected,
        &replay_attachments,
        &exact_output,
    )?
    .ok_or_else(|| {
        invalid(
            &output,
            &names.record,
            WorkerV3PublicationIntentInvalidReasonV1::RecordDisappearedAfterCommit,
        )
    })?;
    Ok(RecoveredWorkerV3PublicationIntentV1 {
        outcome: WorkerV3PublicationIntentOutcomeV1::Persisted,
        record,
        replay_attachments,
        exact_output,
    })
}

/// Durably retires one exact committed Worker V3 restart intent.
///
/// The current occurrence requires its exact completed backend receipt in the durable attempt
/// registry. A strictly newer same-producer generation may instead retire a superseded intent.
/// Cleanup first renames and synchronizes the validated record as an inert retirement marker,
/// removes and synchronizes all attachments, and removes the marker last. Repeating this call
/// resumes an interrupted retirement without making restart bytes recoverable again.
pub fn clear_worker_v3_publication_intent_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    identity: WorkerV3PublicationIntentIdentityV1,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    clear_worker_v3_publication_intent_v1_with_options(
        output_dir,
        producer,
        attempt,
        identity,
        WorkerV3PublicationIntentOptionsV1::default(),
    )
}

/// Fault-injectable form of [`clear_worker_v3_publication_intent_v1`].
pub fn clear_worker_v3_publication_intent_v1_with_options(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    identity: WorkerV3PublicationIntentIdentityV1,
    options: WorkerV3PublicationIntentOptionsV1,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    if attempt.session() == BuildSession::DIRECT {
        return Err(WorkerV3PublicationIntentErrorV1::ReceiptNotDurable);
    }
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    let producer_key = producer_key(producer);
    let names = IntentNames::new(producer_key, occurrence_key(producer_key, attempt))?;
    let mut faults = FaultInjector::new(options.injected_crash);
    retire_committed_occurrence_locked(
        &output,
        &names,
        producer,
        attempt,
        Some(identity),
        RetirementAuthorizationV1::ReceiptOrSuccessor,
        &mut faults,
    )?;
    Ok(())
}

/// Resumes receipt-authorized cleanup from an inert durable retirement marker.
///
/// This operation intentionally accepts no caller-retained record identity. It decodes and pins
/// the exact `.record.retiring` marker or its terminal quarantine temp, binds it to the requested
/// producer occurrence, and reconstructs the exact backend-receipt authorization from durable
/// state. Finding and removing the terminal quarantine completes this call successfully; if no
/// marker evidence exists, the call returns [`WorkerV3PublicationIntentErrorV1::NotFound`].
/// Canonical and redo records are rejected: callers must start their retirement with
/// [`clear_worker_v3_publication_intent_v1`], which requires the exact record identity.
pub fn resume_worker_v3_publication_intent_retirement_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    if attempt.session() == BuildSession::DIRECT {
        return Err(WorkerV3PublicationIntentErrorV1::ReceiptNotDurable);
    }
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    let producer_key = producer_key(producer);
    let names = IntentNames::new(producer_key, occurrence_key(producer_key, attempt))?;
    let cleanup = cleanup_temps(&output, &names)?;
    if let Some(candidate) = cleanup.quarantined_retiring_record {
        complete_quarantined_retirement_locked(
            &output,
            &names,
            producer,
            attempt,
            candidate,
            None,
            RetirementAuthorizationV1::ReceiptOrSuccessor,
        )?;
        return Ok(());
    }
    let Some(pinned_record) = inspect_retirement_record_locked(&output, &names, producer, attempt)?
    else {
        return Err(WorkerV3PublicationIntentErrorV1::NotFound);
    };
    if !matches!(pinned_record.state, RetirementRecordStateV1::Retiring) {
        return Err(WorkerV3PublicationIntentErrorV1::RetirementNotInProgress);
    }
    let mut faults = FaultInjector::new(None);
    retire_pinned_occurrence_locked(
        &output,
        &names,
        pinned_record,
        RetirementAuthorityV1 {
            producer,
            attempt,
            expected_identity: None,
            authorization: RetirementAuthorizationV1::ReceiptOrSuccessor,
        },
        &mut faults,
    )?;
    Ok(())
}

/// Inertly recovers exact V3 output and transcript bytes after restart.
///
/// Recovery independently checks that the producer occurrence remains current in the durable
/// attempt registry. It does not parse or authenticate the transcript.
pub fn recover_worker_v3_publication_intent_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<RecoveredWorkerV3PublicationIntentV1, WorkerV3PublicationIntentErrorV1> {
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    authorize_occurrence(&output, producer, attempt)?;
    let producer_key = producer_key(producer);
    let names = IntentNames::new(producer_key, occurrence_key(producer_key, attempt))?;
    if cleanup_temps(&output, &names)?
        .quarantined_retiring_record
        .is_some()
    {
        return Err(WorkerV3PublicationIntentErrorV1::RetirementInProgress);
    }
    if let Some(recovered) = recover_locked(&output, &names, producer, attempt)? {
        return Ok(recovered);
    }
    remove_uncommitted_occurrence_entries(&output, &names, false)?;
    Err(WorkerV3PublicationIntentErrorV1::NotFound)
}

/// Removes abandoned files from one exact V3 occurrence namespace.
///
/// Authorization comes only from the durable build-attempt registry: the requested occurrence
/// must be the current same-producer attempt or be superseded by a strictly newer generation for
/// the same stable source and exact crate name. Current committed intents remain protected; a
/// superseded committed or partially retired intent is durably retired through the same marker
/// protocol as [`clear_worker_v3_publication_intent_v1`]. This operation grants no publication,
/// loading, launch, transcript, or semantic-identity authority.
pub fn scavenge_worker_v3_publication_intent_occurrence_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    occurrence: BuildAttempt,
) -> Result<WorkerV3PublicationIntentScavengeOutcomeV1, WorkerV3PublicationIntentErrorV1> {
    if occurrence.session() == BuildSession::DIRECT {
        return Err(WorkerV3PublicationIntentErrorV1::ScavengeNotAuthorized);
    }
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    let authorization = authorize_occurrence_scavenge(&output, producer, occurrence)?;
    let producer_key = producer_key(producer);
    let names = IntentNames::new(producer_key, occurrence_key(producer_key, occurrence))?;
    let cleanup = cleanup_temps(&output, &names)?;
    if let Some(candidate) = cleanup.quarantined_retiring_record {
        if authorization != OccurrenceScavengeAuthorizationV1::Superseded {
            return Err(WorkerV3PublicationIntentErrorV1::CommittedIntentCannotBeScavenged);
        }
        let removed = complete_quarantined_retirement_locked(
            &output,
            &names,
            producer,
            occurrence,
            candidate,
            None,
            RetirementAuthorizationV1::SuccessorOnly,
        )?
        .checked_add(cleanup.removed_entries)
        .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?;
        return Ok(WorkerV3PublicationIntentScavengeOutcomeV1::Removed { entries: removed });
    }
    let committed = entry_exists(&output, &names.record)?
        || entry_exists(&output, &names.redo)?
        || entry_exists(&output, &names.retiring)?;
    let removed = if committed {
        if authorization != OccurrenceScavengeAuthorizationV1::Superseded {
            return Err(WorkerV3PublicationIntentErrorV1::CommittedIntentCannotBeScavenged);
        }
        let mut faults = FaultInjector::new(None);
        retire_committed_occurrence_locked(
            &output,
            &names,
            producer,
            occurrence,
            None,
            RetirementAuthorizationV1::SuccessorOnly,
            &mut faults,
        )?
    } else {
        remove_uncommitted_occurrence_entries(&output, &names, true)?
    };
    if removed == 0 {
        Ok(WorkerV3PublicationIntentScavengeOutcomeV1::NotFound)
    } else {
        Ok(WorkerV3PublicationIntentScavengeOutcomeV1::Removed { entries: removed })
    }
}

fn validate_persistence_inputs(
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    replay_attachments: &WorkerV3FinalizerReplayAttachmentsV1,
    exact_output: &Vec<u8>,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    if plan.attempt() != attempt {
        return Err(WorkerV3PublicationIntentErrorV1::PlanAttemptMismatch);
    }
    if attempt.session() == BuildSession::DIRECT {
        return Err(WorkerV3PublicationIntentErrorV1::Attempt {
            reason: "the direct compiler token cannot own restart state".to_string(),
        });
    }
    validate_payload_lengths(
        replay_attachments.outer_handoff().len(),
        replay_attachments.external_providers().canonical_length(),
        replay_attachments.external_providers().len(),
        replay_attachments.external_providers().payload_length(),
        replay_attachments.canonical_replay_transcript().len(),
        exact_output.len(),
    )?;
    validate_caller_owner_capacities(
        &replay_attachments.outer_handoff,
        &replay_attachments.external_providers,
        &replay_attachments.canonical_replay_transcript,
        Some(exact_output),
    )?;
    if sha256(exact_output) != *plan.finalized_output().as_bytes() {
        return Err(WorkerV3PublicationIntentErrorV1::OutputDigestMismatch);
    }
    Ok(())
}

fn validate_recovery_working_set(
    outer_handoff_length: usize,
    external_provider_archive_length: usize,
    external_provider_count: usize,
    transcript_length: usize,
    output_length: usize,
) -> Result<usize, WorkerV3PublicationIntentErrorV1> {
    let provider_bookkeeping = external_provider_count
        .checked_mul(std::mem::size_of::<(usize, [u8; 32])>() + std::mem::size_of::<Vec<u8>>())
        .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?;
    let required = RECORD_BYTES_V1
        .checked_add(outer_handoff_length)
        .and_then(|value| value.checked_add(external_provider_archive_length))
        .and_then(|value| value.checked_add(provider_bookkeeping))
        .and_then(|value| value.checked_add(RECOVERY_ATTACHMENT_OWNER_BYTES_V1))
        .and_then(|value| value.checked_add(transcript_length))
        .and_then(|value| value.checked_add(output_length))
        .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?;
    if required > MAX_WORKER_V3_PUBLICATION_INTENT_RECOVERY_BYTES_V1 {
        return Err(WorkerV3PublicationIntentErrorV1::WorkingSetBudgetExceeded {
            required,
            maximum: MAX_WORKER_V3_PUBLICATION_INTENT_RECOVERY_BYTES_V1,
        });
    }
    Ok(required)
}

fn authorize_occurrence(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<AttemptPhase, WorkerV3PublicationIntentErrorV1> {
    if attempt.session() == BuildSession::DIRECT {
        return Err(WorkerV3PublicationIntentErrorV1::Attempt {
            reason: "the direct compiler token cannot own restart state".to_string(),
        });
    }
    let attempts = read_attempt_registry(output)?;
    let record = attempts
        .record_exact(&producer.stable_source, attempt)
        .map_err(|error| WorkerV3PublicationIntentErrorV1::Attempt {
            reason: error.to_string(),
        })?;
    if record.crate_name != producer.crate_name {
        return Err(WorkerV3PublicationIntentErrorV1::Attempt {
            reason: "build occurrence crate name does not match the producer".to_string(),
        });
    }
    if !matches!(
        record.phase,
        AttemptPhase::Building | AttemptPhase::BackendClaimed | AttemptPhase::Completed
    ) {
        return Err(WorkerV3PublicationIntentErrorV1::Attempt {
            reason: "build occurrence cannot recover restart state in its current phase"
                .to_string(),
        });
    }
    Ok(record.phase)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OccurrenceScavengeAuthorizationV1 {
    Exact,
    Superseded,
}

fn authorize_occurrence_scavenge(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    occurrence: BuildAttempt,
) -> Result<OccurrenceScavengeAuthorizationV1, WorkerV3PublicationIntentErrorV1> {
    let attempts = read_attempt_registry(output)?;
    let Some(current) = attempts.record(&producer.stable_source) else {
        return Err(WorkerV3PublicationIntentErrorV1::ScavengeNotAuthorized);
    };
    let exact = current.generation == occurrence.generation()
        && current.session == occurrence.session()
        && current.invocation == occurrence.invocation();
    let superseded = current.generation > occurrence.generation();
    if current.crate_name != producer.crate_name || (!exact && !superseded) {
        return Err(WorkerV3PublicationIntentErrorV1::ScavengeNotAuthorized);
    }
    Ok(if exact {
        OccurrenceScavengeAuthorizationV1::Exact
    } else {
        OccurrenceScavengeAuthorizationV1::Superseded
    })
}

#[derive(Clone, Copy)]
enum RetirementAuthorizationV1 {
    ReceiptOrSuccessor,
    SuccessorOnly,
}

#[derive(Clone, Copy)]
struct RetirementAuthorityV1<'a> {
    producer: &'a ProducerIdentity,
    attempt: BuildAttempt,
    expected_identity: Option<WorkerV3PublicationIntentIdentityV1>,
    authorization: RetirementAuthorizationV1,
}

#[derive(Clone, Copy)]
enum RetirementRecordStateV1 {
    Canonical,
    Redo,
    Retiring,
}

struct PinnedBoundRecordV1 {
    record: WorkerV3PublicationIntentRecordV1,
    file: fs::File,
    snapshot: rustix::fs::Stat,
}

struct PinnedRetirementRecordV1 {
    record: WorkerV3PublicationIntentRecordV1,
    file: fs::File,
    snapshot: rustix::fs::Stat,
    state: RetirementRecordStateV1,
}

impl RetirementRecordStateV1 {
    fn entry(self, names: &IntentNames) -> &str {
        match self {
            Self::Canonical => &names.record,
            Self::Redo => &names.redo,
            Self::Retiring => &names.retiring,
        }
    }
}

fn retire_committed_occurrence_locked(
    output: &PinnedOutput,
    names: &IntentNames,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    expected_identity: Option<WorkerV3PublicationIntentIdentityV1>,
    authorization: RetirementAuthorizationV1,
    faults: &mut FaultInjector,
) -> Result<usize, WorkerV3PublicationIntentErrorV1> {
    let cleanup = cleanup_temps(output, names)?;
    if let Some(candidate) = cleanup.quarantined_retiring_record {
        return complete_quarantined_retirement_locked(
            output,
            names,
            producer,
            attempt,
            candidate,
            expected_identity,
            authorization,
        )?
        .checked_add(cleanup.removed_entries)
        .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow);
    }
    let Some(pinned_record) = inspect_retirement_record_locked(output, names, producer, attempt)?
    else {
        return Err(WorkerV3PublicationIntentErrorV1::NotFound);
    };
    retire_pinned_occurrence_locked(
        output,
        names,
        pinned_record,
        RetirementAuthorityV1 {
            producer,
            attempt,
            expected_identity,
            authorization,
        },
        faults,
    )?
    .checked_add(cleanup.removed_entries)
    .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)
}

fn complete_quarantined_retirement_locked(
    output: &PinnedOutput,
    names: &IntentNames,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    candidate: CleanupCandidate,
    expected_identity: Option<WorkerV3PublicationIntentIdentityV1>,
    authorization: RetirementAuthorizationV1,
) -> Result<usize, WorkerV3PublicationIntentErrorV1> {
    for entry in [
        &names.record,
        &names.redo,
        &names.retiring,
        &names.outer_handoff,
        &names.external_providers,
        &names.transcript,
        &names.output,
    ] {
        if entry_exists(output, entry)? {
            return Err(invalid(
                output,
                entry,
                WorkerV3PublicationIntentInvalidReasonV1::CommittedAndRetiringCoexist,
            ));
        }
    }
    let entry = candidate.name.to_str().ok_or_else(|| {
        invalid(
            output,
            &candidate.name,
            WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
        )
    })?;
    let pinned = read_bound_record_pinned(output, names, entry, producer, attempt)?;
    if !same_private_snapshot(&candidate.snapshot, &pinned.snapshot) {
        return Err(invalid(
            output,
            &candidate.name,
            WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
        ));
    }
    if expected_identity.is_some_and(|identity| identity != pinned.record.identity()) {
        return Err(WorkerV3PublicationIntentErrorV1::IdentityMismatch);
    }
    authorize_retirement(output, producer, attempt, pinned.record, authorization)?;
    let mut faults = FaultInjector::new(None);
    unlink_pinned_private_candidate(
        output,
        names,
        &candidate,
        "retire-record",
        CleanupBoundariesV1 {
            quarantine: WorkerV3PublicationIntentBoundaryV1::RenameRetiringRecordToQuarantine,
            remove: WorkerV3PublicationIntentBoundaryV1::RemoveRetiringRecord,
        },
        &pinned.file,
        &mut faults,
    )?;
    fsync(&output.fd).map_err(std::io::Error::from)?;
    output.verify_path_identity()?;
    Ok(1)
}

fn retire_pinned_occurrence_locked(
    output: &PinnedOutput,
    names: &IntentNames,
    mut pinned_record: PinnedRetirementRecordV1,
    authority: RetirementAuthorityV1<'_>,
    faults: &mut FaultInjector,
) -> Result<usize, WorkerV3PublicationIntentErrorV1> {
    if authority
        .expected_identity
        .is_some_and(|identity| identity != pinned_record.record.identity())
    {
        return Err(WorkerV3PublicationIntentErrorV1::IdentityMismatch);
    }
    authorize_retirement(
        output,
        authority.producer,
        authority.attempt,
        pinned_record.record,
        authority.authorization,
    )?;

    let require_complete = !matches!(pinned_record.state, RetirementRecordStateV1::Retiring);
    let candidates =
        collect_retirement_candidates(output, names, pinned_record.record, require_complete)?;
    if require_complete {
        rename_record_to_retiring(output, names, &mut pinned_record, faults)?;
    } else {
        // A prior process may have stopped after the rename but before its directory sync.
        faults.around(
            WorkerV3PublicationIntentBoundaryV1::SyncRetiringName,
            || {
                fsync(&output.fd)
                    .map_err(std::io::Error::from)
                    .map_err(Into::into)
            },
        )?;
        output.verify_path_identity()?;
    }

    let mut removed = 0_usize;
    for (candidate, purpose, quarantine_boundary, remove_boundary) in [
        (
            candidates.outer_handoff,
            "retire-handoff",
            WorkerV3PublicationIntentBoundaryV1::RenameOuterHandoffToQuarantine,
            WorkerV3PublicationIntentBoundaryV1::RemoveOuterHandoff,
        ),
        (
            candidates.external_providers,
            "retire-providers",
            WorkerV3PublicationIntentBoundaryV1::RenameExternalProvidersToQuarantine,
            WorkerV3PublicationIntentBoundaryV1::RemoveExternalProviders,
        ),
        (
            candidates.transcript,
            "retire-transcript",
            WorkerV3PublicationIntentBoundaryV1::RenameTranscriptToQuarantine,
            WorkerV3PublicationIntentBoundaryV1::RemoveTranscript,
        ),
        (
            candidates.output,
            "retire-output",
            WorkerV3PublicationIntentBoundaryV1::RenameOutputToQuarantine,
            WorkerV3PublicationIntentBoundaryV1::RemoveOutput,
        ),
    ] {
        if let Some(candidate) = candidate {
            unlink_exact_private_candidate(
                output,
                names,
                &candidate,
                purpose,
                CleanupBoundariesV1 {
                    quarantine: quarantine_boundary,
                    remove: remove_boundary,
                },
                faults,
            )?;
            removed = removed
                .checked_add(1)
                .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?;
        }
    }
    faults.around(
        WorkerV3PublicationIntentBoundaryV1::SyncRetiredAttachments,
        || {
            fsync(&output.fd)
                .map_err(std::io::Error::from)
                .map_err(Into::into)
        },
    )?;
    output.verify_path_identity()?;

    unlink_pinned_private_candidate(
        output,
        names,
        &CleanupCandidate {
            name: PathBuf::from(&names.retiring),
            snapshot: pinned_record.snapshot,
        },
        "retire-record",
        CleanupBoundariesV1 {
            quarantine: WorkerV3PublicationIntentBoundaryV1::RenameRetiringRecordToQuarantine,
            remove: WorkerV3PublicationIntentBoundaryV1::RemoveRetiringRecord,
        },
        &pinned_record.file,
        faults,
    )?;
    removed = removed
        .checked_add(1)
        .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?;
    faults.around(WorkerV3PublicationIntentBoundaryV1::SyncRetirement, || {
        fsync(&output.fd)
            .map_err(std::io::Error::from)
            .map_err(Into::into)
    })?;
    output.verify_path_identity()?;
    Ok(removed)
}

fn inspect_retirement_record_locked(
    output: &PinnedOutput,
    names: &IntentNames,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<Option<PinnedRetirementRecordV1>, WorkerV3PublicationIntentErrorV1> {
    let canonical = entry_exists(output, &names.record)?;
    let redo = entry_exists(output, &names.redo)?;
    let retiring = entry_exists(output, &names.retiring)?;
    if canonical && redo {
        return Err(invalid(
            output,
            &names.record,
            WorkerV3PublicationIntentInvalidReasonV1::CanonicalAndRedoCoexist,
        ));
    }
    if retiring && (canonical || redo) {
        return Err(invalid(
            output,
            &names.retiring,
            WorkerV3PublicationIntentInvalidReasonV1::CommittedAndRetiringCoexist,
        ));
    }
    let state = if canonical {
        RetirementRecordStateV1::Canonical
    } else if redo {
        RetirementRecordStateV1::Redo
    } else if retiring {
        RetirementRecordStateV1::Retiring
    } else {
        return Ok(None);
    };
    let pinned = read_bound_record_pinned(output, names, state.entry(names), producer, attempt)?;
    Ok(Some(PinnedRetirementRecordV1 {
        record: pinned.record,
        file: pinned.file,
        snapshot: pinned.snapshot,
        state,
    }))
}

fn authorize_retirement(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    intent: WorkerV3PublicationIntentRecordV1,
    authorization: RetirementAuthorizationV1,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    let attempts = read_attempt_registry(output)?;
    let Some(current) = attempts.record(&producer.stable_source) else {
        return Err(WorkerV3PublicationIntentErrorV1::ScavengeNotAuthorized);
    };
    if current.crate_name != producer.crate_name {
        return Err(WorkerV3PublicationIntentErrorV1::ScavengeNotAuthorized);
    }
    let exact = current.generation == attempt.generation()
        && current.session == attempt.session()
        && current.invocation == attempt.invocation();
    if exact {
        if matches!(authorization, RetirementAuthorizationV1::SuccessorOnly) {
            return Err(WorkerV3PublicationIntentErrorV1::ScavengeNotAuthorized);
        }
        if !matches!(
            current.phase,
            AttemptPhase::BackendClaimed | AttemptPhase::Completed
        ) || !has_exact_durable_receipt(current.backend_receipt, producer, attempt, intent)
        {
            return Err(WorkerV3PublicationIntentErrorV1::ReceiptNotDurable);
        }
        return Ok(());
    }
    if current.generation > attempt.generation() {
        return Ok(());
    }
    Err(WorkerV3PublicationIntentErrorV1::ScavengeNotAuthorized)
}

fn has_exact_durable_receipt(
    receipt: Option<BackendReceiptV1>,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    intent: WorkerV3PublicationIntentRecordV1,
) -> bool {
    match receipt {
        Some(BackendReceiptV1::Provenance(receipt)) => {
            let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes(
                receipt.upstream_evidence_identity(),
            );
            receipt == publication_receipt(producer, attempt, intent.plan(), upstream)
        }
        Some(BackendReceiptV1::ProvenanceV2(receipt)) => {
            let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes(
                receipt.upstream_evidence_identity(),
            );
            receipt
                == publication_receipt_v2(
                    producer,
                    attempt,
                    intent.plan(),
                    upstream,
                    receipt.compiler_closure(),
                )
        }
        Some(
            BackendReceiptV1::LegacyCoordination
            | BackendReceiptV1::PendingProvenance(_)
            | BackendReceiptV1::PendingProvenanceV2(_),
        )
        | None => false,
    }
}

fn recover_locked(
    output: &PinnedOutput,
    names: &IntentNames,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<Option<RecoveredWorkerV3PublicationIntentV1>, WorkerV3PublicationIntentErrorV1> {
    let Some((record, redo)) = inspect_committed_record_locked(output, names, producer, attempt)?
    else {
        return Ok(None);
    };
    validate_recovery_working_set(
        record.outer_handoff_length(),
        record.external_provider_archive_length(),
        record.external_provider_count(),
        record.transcript_length(),
        record.output_length(),
    )?;
    let outer_handoff = read_payload(
        output,
        &names.outer_handoff,
        record.outer_handoff_length(),
        record.outer_handoff_sha256(),
        PayloadKind::OuterHandoff,
    )?;
    let external_providers = read_provider_archive(output, names, record)?;
    let canonical_replay_transcript = read_payload(
        output,
        &names.transcript,
        record.transcript_length(),
        record.transcript_sha256(),
        PayloadKind::Transcript,
    )?;
    let exact_output = read_payload(
        output,
        &names.output,
        record.output_length(),
        record.output_sha256(),
        PayloadKind::Output,
    )?;
    let replay_attachments = WorkerV3FinalizerReplayAttachmentsV1 {
        outer_handoff,
        external_providers,
        canonical_replay_transcript,
    };
    validate_caller_owner_capacities(
        &replay_attachments.outer_handoff,
        &replay_attachments.external_providers,
        &replay_attachments.canonical_replay_transcript,
        Some(&exact_output),
    )?;
    finish_committed_record_recovery(output, names, redo)?;
    Ok(Some(RecoveredWorkerV3PublicationIntentV1 {
        outcome: WorkerV3PublicationIntentOutcomeV1::Recovered,
        record,
        replay_attachments,
        exact_output,
    }))
}

fn recover_exact_locked(
    output: &PinnedOutput,
    names: &IntentNames,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    expected_record: WorkerV3PublicationIntentRecordV1,
    expected_attachments: &WorkerV3FinalizerReplayAttachmentsV1,
    expected_output: &[u8],
) -> Result<Option<WorkerV3PublicationIntentRecordV1>, WorkerV3PublicationIntentErrorV1> {
    let Some((record, redo)) = inspect_committed_record_locked(output, names, producer, attempt)?
    else {
        return Ok(None);
    };
    if record != expected_record {
        return Err(WorkerV3PublicationIntentErrorV1::ConflictingIntent);
    }
    validate_private_file_against_bytes(
        output,
        &names.outer_handoff,
        expected_attachments.outer_handoff(),
        PayloadKind::OuterHandoff,
    )?;
    validate_provider_archive_against_payloads(
        output,
        names,
        expected_attachments.external_providers(),
    )?;
    validate_private_file_against_bytes(
        output,
        &names.transcript,
        expected_attachments.canonical_replay_transcript(),
        PayloadKind::Transcript,
    )?;
    validate_private_file_against_bytes(
        output,
        &names.output,
        expected_output,
        PayloadKind::Output,
    )?;
    finish_committed_record_recovery(output, names, redo)?;
    Ok(Some(record))
}

fn inspect_committed_record_locked(
    output: &PinnedOutput,
    names: &IntentNames,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<Option<(WorkerV3PublicationIntentRecordV1, bool)>, WorkerV3PublicationIntentErrorV1> {
    let canonical = entry_exists(output, &names.record)?;
    let redo = entry_exists(output, &names.redo)?;
    let retiring = entry_exists(output, &names.retiring)?;
    if canonical && redo {
        return Err(invalid(
            output,
            &names.record,
            WorkerV3PublicationIntentInvalidReasonV1::CanonicalAndRedoCoexist,
        ));
    }
    if retiring && (canonical || redo) {
        return Err(invalid(
            output,
            &names.retiring,
            WorkerV3PublicationIntentInvalidReasonV1::CommittedAndRetiringCoexist,
        ));
    }
    if retiring {
        return Err(WorkerV3PublicationIntentErrorV1::RetirementInProgress);
    }
    let record_name = if canonical {
        &names.record
    } else if redo {
        &names.redo
    } else {
        return Ok(None);
    };
    let record = read_bound_record(output, names, record_name, producer, attempt)?;
    Ok(Some((record, redo)))
}

fn finish_committed_record_recovery(
    output: &PinnedOutput,
    names: &IntentNames,
    redo: bool,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    if redo {
        output.verify_path_identity()?;
        renameat(&output.fd, &names.redo, &output.fd, &names.record)
            .map_err(std::io::Error::from)?;
    }
    // This also closes the crash window after a canonical rename but before its original sync.
    fsync(&output.fd).map_err(std::io::Error::from)?;
    output.verify_path_identity()?;
    Ok(())
}

fn read_bound_record(
    output: &PinnedOutput,
    names: &IntentNames,
    entry: &str,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<WorkerV3PublicationIntentRecordV1, WorkerV3PublicationIntentErrorV1> {
    Ok(read_bound_record_pinned(output, names, entry, producer, attempt)?.record)
}

fn read_bound_record_pinned(
    output: &PinnedOutput,
    names: &IntentNames,
    entry: &str,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<PinnedBoundRecordV1, WorkerV3PublicationIntentErrorV1> {
    let (mut file, snapshot) = open_private_file(output, entry, RECORD_BYTES_V1)?;
    let mut bytes = [0_u8; RECORD_BYTES_V1];
    file.read_exact(&mut bytes)?;
    finish_private_file_read(output, entry, &mut file, &snapshot, RECORD_BYTES_V1)?;
    let record = WorkerV3PublicationIntentRecordV1::decode_canonical(&bytes).map_err(|error| {
        invalid(
            output,
            entry,
            WorkerV3PublicationIntentInvalidReasonV1::RecordCodec(error),
        )
    })?;
    let expected_producer = producer_key(producer);
    if record.producer_key() != expected_producer
        || record.occurrence_key() != occurrence_key(expected_producer, attempt)
        || record.attempt() != attempt
        || record.plan().attempt() != attempt
        || names.base != IntentNames::new(expected_producer, record.occurrence_key())?.base
    {
        return Err(invalid(
            output,
            entry,
            WorkerV3PublicationIntentInvalidReasonV1::RecordBindingMismatch,
        ));
    }
    Ok(PinnedBoundRecordV1 {
        record,
        file,
        snapshot,
    })
}

fn reconcile_uncommitted_attachments(
    output: &PinnedOutput,
    names: &IntentNames,
    attachments: &WorkerV3FinalizerReplayAttachmentsV1,
    exact_output: &[u8],
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    let mut found = false;
    let mut all_match = true;
    for (entry, expected) in [
        (names.outer_handoff.as_str(), attachments.outer_handoff()),
        (
            names.transcript.as_str(),
            attachments.canonical_replay_transcript(),
        ),
        (names.output.as_str(), exact_output),
    ] {
        if let Some(matches) = private_file_matches_bytes(output, entry, expected)? {
            found = true;
            all_match &= matches;
        }
    }
    if let Some(matches) =
        provider_archive_matches_payloads(output, names, attachments.external_providers())?
    {
        found = true;
        all_match &= matches;
    }
    if found && !all_match {
        remove_uncommitted_occurrence_entries(output, names, false)?;
    }
    Ok(())
}

fn private_file_matches_bytes(
    output: &PinnedOutput,
    entry: &str,
    expected: &[u8],
) -> Result<Option<bool>, WorkerV3PublicationIntentErrorV1> {
    let Some(snapshot) = private_entry_snapshot(output, entry)? else {
        return Ok(None);
    };
    if usize::try_from(snapshot.st_size).ok() != Some(expected.len()) {
        return Ok(Some(false));
    }
    let (mut file, before) = open_private_file(output, entry, expected.len())?;
    let mut matches = true;
    let mut buffer = [0_u8; 64 * 1024];
    for expected_chunk in expected.chunks(buffer.len()) {
        let actual = &mut buffer[..expected_chunk.len()];
        file.read_exact(actual)?;
        matches &= actual == expected_chunk;
    }
    finish_private_file_read(output, entry, &mut file, &before, expected.len())?;
    Ok(Some(matches))
}

fn provider_archive_matches_payloads(
    output: &PinnedOutput,
    names: &IntentNames,
    providers: &WorkerV3ExternalProviderPayloadsV1,
) -> Result<Option<bool>, WorkerV3PublicationIntentErrorV1> {
    let entry = &names.external_providers;
    let Some(snapshot) = private_entry_snapshot(output, entry)? else {
        return Ok(None);
    };
    if usize::try_from(snapshot.st_size).ok() != Some(providers.canonical_length()) {
        return Ok(Some(false));
    }
    let (mut file, before) = open_private_file(output, entry, providers.canonical_length())?;
    let header = provider_archive_header(providers)?;
    let mut matches = compare_reader_bytes_match(&mut file, &header)?;
    for payload in providers.iter() {
        matches &= compare_reader_bytes_match(&mut file, payload)?;
    }
    let checksum = provider_archive_checksum(&providers.payloads, providers.payload_length());
    matches &= compare_reader_bytes_match(&mut file, &checksum)?;
    finish_private_file_read(
        output,
        entry,
        &mut file,
        &before,
        providers.canonical_length(),
    )?;
    Ok(Some(matches))
}

fn compare_reader_bytes_match(
    file: &mut fs::File,
    expected: &[u8],
) -> Result<bool, WorkerV3PublicationIntentErrorV1> {
    let mut matches = true;
    let mut buffer = [0_u8; 64 * 1024];
    for expected_chunk in expected.chunks(buffer.len()) {
        let actual = &mut buffer[..expected_chunk.len()];
        file.read_exact(actual)?;
        matches &= actual == expected_chunk;
    }
    Ok(matches)
}

struct CleanupCandidate {
    name: PathBuf,
    snapshot: rustix::fs::Stat,
}

#[derive(Clone, Copy)]
struct CleanupBoundariesV1 {
    quarantine: WorkerV3PublicationIntentBoundaryV1,
    remove: WorkerV3PublicationIntentBoundaryV1,
}

struct TempCleanupOutcomeV1 {
    removed_entries: usize,
    quarantined_retiring_record: Option<CleanupCandidate>,
}

struct RetirementCandidatesV1 {
    outer_handoff: Option<CleanupCandidate>,
    external_providers: Option<CleanupCandidate>,
    transcript: Option<CleanupCandidate>,
    output: Option<CleanupCandidate>,
}

fn collect_retirement_candidates(
    output: &PinnedOutput,
    names: &IntentNames,
    record: WorkerV3PublicationIntentRecordV1,
    require_complete: bool,
) -> Result<RetirementCandidatesV1, WorkerV3PublicationIntentErrorV1> {
    let outer_handoff = validate_optional_retirement_payload(
        output,
        &names.outer_handoff,
        record.outer_handoff_length(),
        record.outer_handoff_sha256(),
        PayloadKind::OuterHandoff,
        require_complete,
    )?;
    let external_providers =
        if let Some(snapshot) = private_entry_snapshot(output, &names.external_providers)? {
            drop(read_provider_archive(output, names, record)?);
            Some(CleanupCandidate {
                name: PathBuf::from(&names.external_providers),
                snapshot,
            })
        } else if require_complete {
            return Err(invalid(
                output,
                &names.external_providers,
                WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
            ));
        } else {
            None
        };
    let transcript = validate_optional_retirement_payload(
        output,
        &names.transcript,
        record.transcript_length(),
        record.transcript_sha256(),
        PayloadKind::Transcript,
        require_complete,
    )?;
    let output_candidate = validate_optional_retirement_payload(
        output,
        &names.output,
        record.output_length(),
        record.output_sha256(),
        PayloadKind::Output,
        require_complete,
    )?;
    Ok(RetirementCandidatesV1 {
        outer_handoff,
        external_providers,
        transcript,
        output: output_candidate,
    })
}

fn validate_optional_retirement_payload(
    output: &PinnedOutput,
    entry: &str,
    exact_length: usize,
    expected_digest: [u8; 32],
    kind: PayloadKind,
    required: bool,
) -> Result<Option<CleanupCandidate>, WorkerV3PublicationIntentErrorV1> {
    let Some(snapshot) = private_entry_snapshot(output, entry)? else {
        if required {
            return Err(invalid(
                output,
                entry,
                WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
            ));
        }
        return Ok(None);
    };
    let (mut file, before) = open_private_file(output, entry, exact_length)?;
    let mut digest = Sha256::new();
    let mut remaining = exact_length;
    let mut buffer = [0_u8; 64 * 1024];
    while remaining != 0 {
        let chunk = remaining.min(buffer.len());
        file.read_exact(&mut buffer[..chunk])?;
        digest.update(&buffer[..chunk]);
        remaining -= chunk;
    }
    finish_private_file_read(output, entry, &mut file, &before, exact_length)?;
    if <[u8; 32]>::from(digest.finalize()) != expected_digest {
        return Err(kind.digest_mismatch());
    }
    Ok(Some(CleanupCandidate {
        name: PathBuf::from(entry),
        snapshot,
    }))
}

fn rename_record_to_retiring(
    output: &PinnedOutput,
    names: &IntentNames,
    pinned_record: &mut PinnedRetirementRecordV1,
    faults: &mut FaultInjector,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    let source = pinned_record.state.entry(names);
    let current =
        statat(&output.fd, source, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    let descriptor = fstat(&pinned_record.file).map_err(std::io::Error::from)?;
    if !same_private_snapshot(&pinned_record.snapshot, &current)
        || !same_private_snapshot(&pinned_record.snapshot, &descriptor)
    {
        return Err(invalid(
            output,
            source,
            WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
        ));
    }
    faults.hit(
        WorkerV3PublicationIntentBoundaryV1::RenameRecordToRetiring,
        WorkerV3PublicationIntentFaultTimingV1::Before,
    )?;
    output.verify_path_identity()?;
    let current =
        statat(&output.fd, source, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    let descriptor = fstat(&pinned_record.file).map_err(std::io::Error::from)?;
    if !same_private_snapshot(&pinned_record.snapshot, &current)
        || !same_private_snapshot(&pinned_record.snapshot, &descriptor)
    {
        return Err(invalid(
            output,
            source,
            WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
        ));
    }
    renameat_with(
        &output.fd,
        source,
        &output.fd,
        &names.retiring,
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    let named = statat(&output.fd, &names.retiring, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(std::io::Error::from)?;
    let descriptor_after = fstat(&pinned_record.file).map_err(std::io::Error::from)?;
    if !same_private_inode(&pinned_record.snapshot, &named)
        || !same_private_inode(&pinned_record.snapshot, &descriptor_after)
        || !same_private_snapshot(&named, &descriptor_after)
    {
        return Err(invalid(
            output,
            &names.retiring,
            WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
        ));
    }
    pinned_record.snapshot = named;
    pinned_record.state = RetirementRecordStateV1::Retiring;
    faults.hit(
        WorkerV3PublicationIntentBoundaryV1::RenameRecordToRetiring,
        WorkerV3PublicationIntentFaultTimingV1::After,
    )?;
    faults.around(
        WorkerV3PublicationIntentBoundaryV1::SyncRetiringName,
        || {
            fsync(&output.fd)
                .map_err(std::io::Error::from)
                .map_err(Into::into)
        },
    )?;
    output.verify_path_identity()?;
    Ok(())
}

fn unlink_exact_private_candidate(
    output: &PinnedOutput,
    names: &IntentNames,
    candidate: &CleanupCandidate,
    purpose: &str,
    boundaries: CleanupBoundariesV1,
    faults: &mut FaultInjector,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    let exact_length = usize::try_from(candidate.snapshot.st_size).map_err(|_| {
        invalid(
            output,
            &candidate.name,
            WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
        )
    })?;
    let candidate_entry = candidate.name.to_str().ok_or_else(|| {
        invalid(
            output,
            &candidate.name,
            WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
        )
    })?;
    let (file, pinned) = open_private_file(output, candidate_entry, exact_length)?;
    if !same_private_snapshot(&candidate.snapshot, &pinned) {
        return Err(invalid(
            output,
            &candidate.name,
            WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
        ));
    }
    unlink_pinned_private_candidate(output, names, candidate, purpose, boundaries, &file, faults)
}

fn unlink_pinned_private_candidate(
    output: &PinnedOutput,
    names: &IntentNames,
    candidate: &CleanupCandidate,
    purpose: &str,
    boundaries: CleanupBoundariesV1,
    file: &fs::File,
    faults: &mut FaultInjector,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    faults.hit(
        boundaries.quarantine,
        WorkerV3PublicationIntentFaultTimingV1::Before,
    )?;
    output.verify_path_identity()?;
    let current = statat(&output.fd, &candidate.name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(std::io::Error::from)?;
    let descriptor = fstat(file).map_err(std::io::Error::from)?;
    if !same_private_snapshot(&candidate.snapshot, &current)
        || !same_private_snapshot(&candidate.snapshot, &descriptor)
    {
        return Err(invalid(
            output,
            &candidate.name,
            WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
        ));
    }

    let quarantine = reserve_cleanup_quarantine_name(output, names, purpose, &candidate.name)?;
    let named =
        statat(&output.fd, &quarantine, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    let pinned_after = fstat(file).map_err(std::io::Error::from)?;
    if !same_private_inode(&candidate.snapshot, &named)
        || !same_private_inode(&candidate.snapshot, &pinned_after)
        || !same_private_snapshot(&named, &pinned_after)
    {
        return Err(invalid(
            output,
            &quarantine,
            WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
        ));
    }
    faults.hit(
        boundaries.quarantine,
        WorkerV3PublicationIntentFaultTimingV1::After,
    )?;
    faults.hit(
        boundaries.remove,
        WorkerV3PublicationIntentFaultTimingV1::Before,
    )?;
    output.verify_path_identity()?;
    let immediate =
        statat(&output.fd, &quarantine, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    let descriptor_immediate = fstat(file).map_err(std::io::Error::from)?;
    if !same_private_snapshot(&named, &immediate)
        || !same_private_snapshot(&named, &descriptor_immediate)
    {
        return Err(invalid(
            output,
            &quarantine,
            WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
        ));
    }
    unlinkat(&output.fd, &quarantine, AtFlags::empty()).map_err(std::io::Error::from)?;
    let unlinked = fstat(file).map_err(std::io::Error::from)?;
    if unlinked.st_dev != candidate.snapshot.st_dev
        || unlinked.st_ino != candidate.snapshot.st_ino
        || unlinked.st_nlink != 0
    {
        return Err(invalid(
            output,
            &quarantine,
            WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
        ));
    }
    faults.hit(
        boundaries.remove,
        WorkerV3PublicationIntentFaultTimingV1::After,
    )
}

fn reserve_cleanup_quarantine_name(
    output: &PinnedOutput,
    names: &IntentNames,
    purpose: &str,
    source: &Path,
) -> Result<PathBuf, WorkerV3PublicationIntentErrorV1> {
    let start = NEXT_TEMP_ID.fetch_add(MAX_TEMP_ATTEMPTS, Ordering::Relaxed);
    for offset in 0..MAX_TEMP_ATTEMPTS {
        let name = temp_name(
            &names.temp_prefix,
            purpose,
            std::process::id(),
            start.wrapping_add(offset),
        )?;
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
    Err(invalid(
        output,
        source,
        WorkerV3PublicationIntentInvalidReasonV1::TemporaryNameExhausted,
    ))
}

fn remove_uncommitted_occurrence_entries(
    output: &PinnedOutput,
    names: &IntentNames,
    include_temps: bool,
) -> Result<usize, WorkerV3PublicationIntentErrorV1> {
    if entry_exists(output, &names.record)?
        || entry_exists(output, &names.redo)?
        || entry_exists(output, &names.retiring)?
    {
        return Err(WorkerV3PublicationIntentErrorV1::CommittedIntentCannotBeScavenged);
    }
    let candidates = collect_uncommitted_cleanup_candidates(output, names, include_temps)?;
    if candidates.is_empty() {
        return Ok(0);
    }
    if entry_exists(output, &names.record)?
        || entry_exists(output, &names.redo)?
        || entry_exists(output, &names.retiring)?
    {
        return Err(WorkerV3PublicationIntentErrorV1::CommittedIntentCannotBeScavenged);
    }
    output.verify_path_identity()?;
    for candidate in &candidates {
        let current = statat(&output.fd, &candidate.name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(std::io::Error::from)?;
        if !same_private_snapshot(&candidate.snapshot, &current) {
            return Err(invalid(
                output,
                &candidate.name,
                WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
            ));
        }
    }
    let mut faults = FaultInjector::new(None);
    for (index, candidate) in candidates.iter().enumerate() {
        let purpose = format!("scavenge-{index}");
        unlink_exact_private_candidate(
            output,
            names,
            candidate,
            &purpose,
            CleanupBoundariesV1 {
                quarantine: WorkerV3PublicationIntentBoundaryV1::RenameOutputToQuarantine,
                remove: WorkerV3PublicationIntentBoundaryV1::RemoveOutput,
            },
            &mut faults,
        )?;
    }
    fsync(&output.fd).map_err(std::io::Error::from)?;
    output.verify_path_identity()?;
    Ok(candidates.len())
}

fn collect_uncommitted_cleanup_candidates(
    output: &PinnedOutput,
    names: &IntentNames,
    include_temps: bool,
) -> Result<Vec<CleanupCandidate>, WorkerV3PublicationIntentErrorV1> {
    let reserve = 4_usize
        .checked_add(if include_temps {
            MAX_TEMP_ATTEMPTS as usize
        } else {
            0
        })
        .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?;
    let mut candidates = Vec::new();
    candidates.try_reserve_exact(reserve).map_err(|_| {
        WorkerV3PublicationIntentErrorV1::AllocationFailed {
            component: "uncommitted cleanup candidates",
            requested: reserve,
        }
    })?;
    for name in [
        &names.outer_handoff,
        &names.external_providers,
        &names.transcript,
        &names.output,
    ] {
        if let Some(snapshot) = private_entry_snapshot(output, name)? {
            candidates.push(CleanupCandidate {
                name: PathBuf::from(name),
                snapshot,
            });
        }
    }
    if !include_temps {
        return Ok(candidates);
    }

    let directory = rustix::io::fcntl_dupfd_cloexec(&output.fd, 0).map_err(std::io::Error::from)?;
    let mut entries = rustix::fs::Dir::read_from(&directory).map_err(std::io::Error::from)?;
    let mut scanned_entries = 0_usize;
    let mut temp_entries = 0_usize;
    for entry in &mut entries {
        let entry = entry.map_err(std::io::Error::from)?;
        let name_bytes = entry.file_name().to_bytes();
        if name_bytes == b"." || name_bytes == b".." {
            continue;
        }
        scanned_entries = scanned_entries
            .checked_add(1)
            .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?;
        if scanned_entries > MAX_OUTPUT_ENTRIES {
            return Err(invalid(
                output,
                &names.temp_prefix,
                WorkerV3PublicationIntentInvalidReasonV1::DirectoryEntryLimitExceeded {
                    maximum: MAX_OUTPUT_ENTRIES,
                },
            ));
        }
        if !name_bytes.starts_with(names.temp_prefix.as_bytes()) {
            continue;
        }
        temp_entries = temp_entries
            .checked_add(1)
            .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?;
        if temp_entries > MAX_TEMP_ATTEMPTS as usize {
            return Err(invalid(
                output,
                &names.temp_prefix,
                WorkerV3PublicationIntentInvalidReasonV1::TemporaryEntryLimitExceeded {
                    maximum: MAX_TEMP_ATTEMPTS as usize,
                },
            ));
        }
        let name = PathBuf::from(std::ffi::OsStr::from_bytes(name_bytes));
        let snapshot = private_entry_snapshot(output, &name)?.ok_or_else(|| {
            invalid(
                output,
                &name,
                WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
            )
        })?;
        candidates.push(CleanupCandidate { name, snapshot });
    }
    Ok(candidates)
}

fn private_entry_snapshot(
    output: &PinnedOutput,
    entry: impl AsRef<Path>,
) -> Result<Option<rustix::fs::Stat>, WorkerV3PublicationIntentErrorV1> {
    let entry = entry.as_ref();
    match statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) if is_private_file(&stat) => Ok(Some(stat)),
        Ok(_) => Err(invalid(
            output,
            entry,
            WorkerV3PublicationIntentInvalidReasonV1::EntryNotPrivate,
        )),
        Err(error) if error == rustix::io::Errno::NOENT => Ok(None),
        Err(error) => Err(std::io::Error::from(error).into()),
    }
}

fn same_private_snapshot(left: &rustix::fs::Stat, right: &rustix::fs::Stat) -> bool {
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

#[derive(Clone, Copy)]
enum PayloadKind {
    OuterHandoff,
    Output,
    Transcript,
}

impl PayloadKind {
    fn entry(self, names: &IntentNames) -> &str {
        match self {
            Self::OuterHandoff => &names.outer_handoff,
            Self::Output => &names.output,
            Self::Transcript => &names.transcript,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::OuterHandoff => "outer handoff",
            Self::Output => "output",
            Self::Transcript => "transcript",
        }
    }

    const fn create_boundary(self) -> WorkerV3PublicationIntentBoundaryV1 {
        match self {
            Self::OuterHandoff => WorkerV3PublicationIntentBoundaryV1::CreateOuterHandoffTemp,
            Self::Output => WorkerV3PublicationIntentBoundaryV1::CreateOutputTemp,
            Self::Transcript => WorkerV3PublicationIntentBoundaryV1::CreateTranscriptTemp,
        }
    }

    const fn write_boundary(self) -> WorkerV3PublicationIntentBoundaryV1 {
        match self {
            Self::OuterHandoff => WorkerV3PublicationIntentBoundaryV1::WriteOuterHandoffTemp,
            Self::Output => WorkerV3PublicationIntentBoundaryV1::WriteOutputTemp,
            Self::Transcript => WorkerV3PublicationIntentBoundaryV1::WriteTranscriptTemp,
        }
    }

    const fn sync_temp_boundary(self) -> WorkerV3PublicationIntentBoundaryV1 {
        match self {
            Self::OuterHandoff => WorkerV3PublicationIntentBoundaryV1::SyncOuterHandoffTemp,
            Self::Output => WorkerV3PublicationIntentBoundaryV1::SyncOutputTemp,
            Self::Transcript => WorkerV3PublicationIntentBoundaryV1::SyncTranscriptTemp,
        }
    }

    const fn rename_boundary(self) -> WorkerV3PublicationIntentBoundaryV1 {
        match self {
            Self::OuterHandoff => WorkerV3PublicationIntentBoundaryV1::RenameOuterHandoff,
            Self::Output => WorkerV3PublicationIntentBoundaryV1::RenameOutput,
            Self::Transcript => WorkerV3PublicationIntentBoundaryV1::RenameTranscript,
        }
    }

    const fn sync_name_boundary(self) -> WorkerV3PublicationIntentBoundaryV1 {
        match self {
            Self::OuterHandoff => WorkerV3PublicationIntentBoundaryV1::SyncOuterHandoffName,
            Self::Output => WorkerV3PublicationIntentBoundaryV1::SyncOutputName,
            Self::Transcript => WorkerV3PublicationIntentBoundaryV1::SyncTranscriptName,
        }
    }

    const fn digest_mismatch(self) -> WorkerV3PublicationIntentErrorV1 {
        match self {
            Self::OuterHandoff => WorkerV3PublicationIntentErrorV1::OuterHandoffDigestMismatch,
            Self::Output => WorkerV3PublicationIntentErrorV1::OutputDigestMismatch,
            Self::Transcript => WorkerV3PublicationIntentErrorV1::TranscriptDigestMismatch,
        }
    }
}

fn persist_payload(
    output: &PinnedOutput,
    names: &IntentNames,
    kind: PayloadKind,
    exact_bytes: &[u8],
    faults: &mut FaultInjector,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    let entry = kind.entry(names);
    if entry_exists(output, entry)? {
        return validate_and_resync_private_file_against_bytes(output, entry, exact_bytes, kind);
    }
    let (temp_name, mut temp) =
        create_temp(output, names, kind.label(), kind.create_boundary(), faults)?;
    faults.around(kind.write_boundary(), || {
        temp.write_all(exact_bytes).map_err(Into::into)
    })?;
    faults.around(kind.sync_temp_boundary(), || {
        temp.sync_all().map_err(Into::into)
    })?;
    faults.hit(
        kind.rename_boundary(),
        WorkerV3PublicationIntentFaultTimingV1::Before,
    )?;
    output.verify_path_identity()?;
    renameat_with(
        &output.fd,
        &temp_name,
        &output.fd,
        entry,
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    faults.hit(
        kind.rename_boundary(),
        WorkerV3PublicationIntentFaultTimingV1::After,
    )?;
    validate_renamed_file(output, entry, &temp, exact_bytes.len())?;
    faults.around(kind.sync_name_boundary(), || {
        fsync(&output.fd)
            .map_err(std::io::Error::from)
            .map_err(Into::into)
    })
}

fn persist_provider_archive(
    output: &PinnedOutput,
    names: &IntentNames,
    providers: &WorkerV3ExternalProviderPayloadsV1,
    faults: &mut FaultInjector,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    if entry_exists(output, &names.external_providers)? {
        return validate_and_resync_provider_archive_against_payloads(output, names, providers);
    }
    let (temp_name, mut temp) = create_temp(
        output,
        names,
        "providers",
        WorkerV3PublicationIntentBoundaryV1::CreateExternalProvidersTemp,
        faults,
    )?;
    faults.around(
        WorkerV3PublicationIntentBoundaryV1::WriteExternalProvidersTemp,
        || write_provider_archive(&mut temp, providers),
    )?;
    faults.around(
        WorkerV3PublicationIntentBoundaryV1::SyncExternalProvidersTemp,
        || temp.sync_all().map_err(Into::into),
    )?;
    faults.hit(
        WorkerV3PublicationIntentBoundaryV1::RenameExternalProviders,
        WorkerV3PublicationIntentFaultTimingV1::Before,
    )?;
    output.verify_path_identity()?;
    renameat_with(
        &output.fd,
        &temp_name,
        &output.fd,
        &names.external_providers,
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    faults.hit(
        WorkerV3PublicationIntentBoundaryV1::RenameExternalProviders,
        WorkerV3PublicationIntentFaultTimingV1::After,
    )?;
    validate_renamed_file(
        output,
        &names.external_providers,
        &temp,
        providers.canonical_length(),
    )?;
    faults.around(
        WorkerV3PublicationIntentBoundaryV1::SyncExternalProvidersName,
        || {
            fsync(&output.fd)
                .map_err(std::io::Error::from)
                .map_err(Into::into)
        },
    )
}

fn write_provider_archive(
    file: &mut fs::File,
    providers: &WorkerV3ExternalProviderPayloadsV1,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    let header = provider_archive_header(providers)?;
    file.write_all(&header)?;
    for payload in providers.iter() {
        file.write_all(payload)?;
    }
    let checksum = provider_archive_checksum(&providers.payloads, providers.payload_length());
    file.write_all(&checksum)?;
    Ok(())
}

fn provider_archive_header(
    providers: &WorkerV3ExternalProviderPayloadsV1,
) -> Result<Vec<u8>, WorkerV3PublicationIntentErrorV1> {
    let header_length = PROVIDER_ARCHIVE_FIXED_BYTES_V1
        .checked_sub(32)
        .and_then(|length| {
            providers
                .len()
                .checked_mul(PROVIDER_ARCHIVE_ENTRY_BYTES_V1)
                .and_then(|entries| length.checked_add(entries))
        })
        .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?;
    let mut header = Vec::new();
    header.try_reserve_exact(header_length).map_err(|_| {
        WorkerV3PublicationIntentErrorV1::AllocationFailed {
            component: "provider archive header",
            requested: header_length,
        }
    })?;
    header.extend_from_slice(PROVIDER_ARCHIVE_MAGIC_V1);
    header.extend_from_slice(&PROVIDER_ARCHIVE_VERSION_V1.to_le_bytes());
    header.extend_from_slice(&(providers.len() as u32).to_le_bytes());
    header.extend_from_slice(&(providers.payload_length() as u64).to_le_bytes());
    for payload in providers.iter() {
        header.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        header.extend_from_slice(&sha256(payload));
    }
    debug_assert_eq!(header.len(), header_length);
    Ok(header)
}

fn persist_record(
    output: &PinnedOutput,
    names: &IntentNames,
    record: WorkerV3PublicationIntentRecordV1,
    faults: &mut FaultInjector,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    let bytes = record.encode_canonical()?;
    let (temp_name, mut temp) = create_temp(
        output,
        names,
        "record",
        WorkerV3PublicationIntentBoundaryV1::CreateRecordTemp,
        faults,
    )?;
    faults.around(WorkerV3PublicationIntentBoundaryV1::WriteRecordTemp, || {
        temp.write_all(&bytes).map_err(Into::into)
    })?;
    faults.around(WorkerV3PublicationIntentBoundaryV1::SyncRecordTemp, || {
        temp.sync_all().map_err(Into::into)
    })?;
    faults.hit(
        WorkerV3PublicationIntentBoundaryV1::RenameRecordToRedo,
        WorkerV3PublicationIntentFaultTimingV1::Before,
    )?;
    output.verify_path_identity()?;
    renameat_with(
        &output.fd,
        &temp_name,
        &output.fd,
        &names.redo,
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    faults.hit(
        WorkerV3PublicationIntentBoundaryV1::RenameRecordToRedo,
        WorkerV3PublicationIntentFaultTimingV1::After,
    )?;
    validate_renamed_file(output, &names.redo, &temp, bytes.len())?;
    faults.around(WorkerV3PublicationIntentBoundaryV1::SyncRedoName, || {
        fsync(&output.fd)
            .map_err(std::io::Error::from)
            .map_err(Into::into)
    })?;
    faults.hit(
        WorkerV3PublicationIntentBoundaryV1::RenameRedoToCanonical,
        WorkerV3PublicationIntentFaultTimingV1::Before,
    )?;
    output.verify_path_identity()?;
    renameat(&output.fd, &names.redo, &output.fd, &names.record).map_err(std::io::Error::from)?;
    faults.hit(
        WorkerV3PublicationIntentBoundaryV1::RenameRedoToCanonical,
        WorkerV3PublicationIntentFaultTimingV1::After,
    )?;
    faults.around(
        WorkerV3PublicationIntentBoundaryV1::SyncCanonicalName,
        || {
            fsync(&output.fd)
                .map_err(std::io::Error::from)
                .map_err(Into::into)
        },
    )
}

fn read_payload(
    output: &PinnedOutput,
    entry: &str,
    exact_length: usize,
    exact_sha256: [u8; 32],
    kind: PayloadKind,
) -> Result<Vec<u8>, WorkerV3PublicationIntentErrorV1> {
    let bytes = read_private_file(output, entry, exact_length, kind.label())?;
    if sha256(&bytes) != exact_sha256 {
        return Err(kind.digest_mismatch());
    }
    Ok(bytes)
}

fn validate_private_file_against_bytes(
    output: &PinnedOutput,
    entry: &str,
    expected: &[u8],
    kind: PayloadKind,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    validate_private_file_against_bytes_with_sync(output, entry, expected, kind, false)
}

fn validate_and_resync_private_file_against_bytes(
    output: &PinnedOutput,
    entry: &str,
    expected: &[u8],
    kind: PayloadKind,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    validate_private_file_against_bytes_with_sync(output, entry, expected, kind, true)
}

fn validate_private_file_against_bytes_with_sync(
    output: &PinnedOutput,
    entry: &str,
    expected: &[u8],
    kind: PayloadKind,
    resync: bool,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    let (mut file, before) = open_private_file_with_access(output, entry, expected.len(), resync)?;
    let mut buffer = [0_u8; 64 * 1024];
    for expected_chunk in expected.chunks(buffer.len()) {
        let actual = &mut buffer[..expected_chunk.len()];
        file.read_exact(actual)?;
        if actual != expected_chunk {
            return Err(kind.digest_mismatch());
        }
    }
    finish_private_file_read(output, entry, &mut file, &before, expected.len())?;
    if resync {
        resync_validated_private_file(output, entry, &file, expected.len())?;
    }
    Ok(())
}

fn validate_provider_archive_against_payloads(
    output: &PinnedOutput,
    names: &IntentNames,
    providers: &WorkerV3ExternalProviderPayloadsV1,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    validate_provider_archive_against_payloads_with_sync(output, names, providers, false)
}

fn validate_and_resync_provider_archive_against_payloads(
    output: &PinnedOutput,
    names: &IntentNames,
    providers: &WorkerV3ExternalProviderPayloadsV1,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    validate_provider_archive_against_payloads_with_sync(output, names, providers, true)
}

fn validate_provider_archive_against_payloads_with_sync(
    output: &PinnedOutput,
    names: &IntentNames,
    providers: &WorkerV3ExternalProviderPayloadsV1,
    resync: bool,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    let entry = &names.external_providers;
    let (mut file, before) =
        open_private_file_with_access(output, entry, providers.canonical_length(), resync)?;
    let header = provider_archive_header(providers)?;
    compare_reader_bytes(&mut file, &header)?;
    for payload in providers.iter() {
        compare_reader_bytes(&mut file, payload)?;
    }
    let checksum = provider_archive_checksum(&providers.payloads, providers.payload_length());
    compare_reader_bytes(&mut file, &checksum)?;
    finish_private_file_read(
        output,
        entry,
        &mut file,
        &before,
        providers.canonical_length(),
    )?;
    if resync {
        resync_validated_private_file(output, entry, &file, providers.canonical_length())?;
    }
    Ok(())
}

fn compare_reader_bytes(
    file: &mut fs::File,
    expected: &[u8],
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    let mut buffer = [0_u8; 64 * 1024];
    for expected_chunk in expected.chunks(buffer.len()) {
        let actual = &mut buffer[..expected_chunk.len()];
        file.read_exact(actual)?;
        if actual != expected_chunk {
            return Err(WorkerV3PublicationIntentErrorV1::ExternalProviderArchiveMismatch);
        }
    }
    Ok(())
}

fn read_provider_archive(
    output: &PinnedOutput,
    names: &IntentNames,
    record: WorkerV3PublicationIntentRecordV1,
) -> Result<WorkerV3ExternalProviderPayloadsV1, WorkerV3PublicationIntentErrorV1> {
    let entry = &names.external_providers;
    let exact_length = record.external_provider_archive_length();
    let (mut file, before) = open_private_file(output, entry, exact_length)?;
    let mut checksum_digest = Sha256::new();
    checksum_digest.update(PROVIDER_ARCHIVE_CHECKSUM_DOMAIN_V1);
    let mut archive_digest = Sha256::new();

    let mut prefix = [0_u8; PROVIDER_ARCHIVE_PREFIX_BYTES_V1];
    file.read_exact(&mut prefix)?;
    checksum_digest.update(prefix);
    archive_digest.update(prefix);
    let mut decoder = Decoder::new(&prefix);
    if decoder.take(PROVIDER_ARCHIVE_MAGIC_V1.len())? != PROVIDER_ARCHIVE_MAGIC_V1 {
        return Err(provider_archive_invalid(
            output,
            entry,
            WorkerV3PublicationIntentCodecErrorV1::ProviderArchiveMagicMismatch,
        ));
    }
    let version = decoder.u16()?;
    if version != PROVIDER_ARCHIVE_VERSION_V1 {
        return Err(provider_archive_invalid(
            output,
            entry,
            WorkerV3PublicationIntentCodecErrorV1::UnsupportedProviderArchiveVersion {
                actual: version,
            },
        ));
    }
    let count = decoder.u32()? as usize;
    let payload_length_u64 = decoder.u64()?;
    if !decoder.finished() {
        return Err(provider_archive_invalid(
            output,
            entry,
            WorkerV3PublicationIntentCodecErrorV1::TruncatedField,
        ));
    }
    let payload_length = bounded_length_allow_zero(
        payload_length_u64,
        MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
    )
    .map_err(|()| {
        provider_archive_invalid(
            output,
            entry,
            WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderPayloadLength {
                actual: payload_length_u64,
                maximum: MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
            },
        )
    })?;
    if count != record.external_provider_count()
        || payload_length != record.external_provider_payload_length()
    {
        return Err(WorkerV3PublicationIntentErrorV1::ExternalProviderArchiveMismatch);
    }

    let mut entries = Vec::new();
    entries.try_reserve_exact(count).map_err(|_| {
        WorkerV3PublicationIntentErrorV1::AllocationFailed {
            component: "provider archive entries",
            requested: count,
        }
    })?;
    let mut declared_payload_length = 0_usize;
    for index in 0..count {
        let mut encoded = [0_u8; PROVIDER_ARCHIVE_ENTRY_BYTES_V1];
        file.read_exact(&mut encoded)?;
        checksum_digest.update(encoded);
        archive_digest.update(encoded);
        let length_u64 = u64::from_le_bytes(encoded[..8].try_into().map_err(|_| {
            provider_archive_invalid(
                output,
                entry,
                WorkerV3PublicationIntentCodecErrorV1::TruncatedField,
            )
        })?);
        let length = bounded_length(length_u64, MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1)
            .map_err(|()| {
                provider_archive_invalid(
                    output,
                    entry,
                    WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderPayloadLength {
                        actual: length_u64,
                        maximum: MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
                    },
                )
            })?;
        declared_payload_length = declared_payload_length
            .checked_add(length)
            .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?;
        if declared_payload_length > payload_length {
            return Err(WorkerV3PublicationIntentErrorV1::ExternalProviderArchiveMismatch);
        }
        let digest: [u8; 32] = encoded[8..].try_into().map_err(|_| {
            provider_archive_invalid(
                output,
                entry,
                WorkerV3PublicationIntentCodecErrorV1::ProviderPayloadDigestMismatch { index },
            )
        })?;
        entries.push((length, digest));
    }
    if declared_payload_length != payload_length
        || (count == 0) != (payload_length == 0)
        || provider_archive_length(count, payload_length)? != exact_length
    {
        return Err(WorkerV3PublicationIntentErrorV1::ExternalProviderArchiveMismatch);
    }

    let mut payloads = Vec::new();
    payloads.try_reserve_exact(count).map_err(|_| {
        WorkerV3PublicationIntentErrorV1::AllocationFailed {
            component: "provider payload owners",
            requested: count,
        }
    })?;
    for (index, (length, expected_digest)) in entries.into_iter().enumerate() {
        let mut payload = Vec::new();
        payload.try_reserve_exact(length).map_err(|_| {
            WorkerV3PublicationIntentErrorV1::AllocationFailed {
                component: "provider payload",
                requested: length,
            }
        })?;
        payload.resize(length, 0);
        file.read_exact(&mut payload)?;
        checksum_digest.update(&payload);
        archive_digest.update(&payload);
        if sha256(&payload) != expected_digest {
            return Err(provider_archive_invalid(
                output,
                entry,
                WorkerV3PublicationIntentCodecErrorV1::ProviderPayloadDigestMismatch { index },
            ));
        }
        payloads.push(payload);
    }
    let mut checksum = [0_u8; 32];
    file.read_exact(&mut checksum)?;
    if <[u8; 32]>::from(checksum_digest.finalize()) != checksum {
        return Err(provider_archive_invalid(
            output,
            entry,
            WorkerV3PublicationIntentCodecErrorV1::ProviderArchiveChecksumMismatch,
        ));
    }
    archive_digest.update(checksum);
    if <[u8; 32]>::from(archive_digest.finalize()) != record.external_provider_archive_sha256() {
        return Err(WorkerV3PublicationIntentErrorV1::ExternalProviderArchiveMismatch);
    }
    finish_private_file_read(output, entry, &mut file, &before, exact_length)?;
    Ok(WorkerV3ExternalProviderPayloadsV1 {
        payloads,
        payload_length,
        canonical_length: exact_length,
        canonical_sha256: record.external_provider_archive_sha256(),
    })
}

fn provider_archive_invalid(
    output: &PinnedOutput,
    entry: &str,
    error: WorkerV3PublicationIntentCodecErrorV1,
) -> WorkerV3PublicationIntentErrorV1 {
    invalid(
        output,
        entry,
        WorkerV3PublicationIntentInvalidReasonV1::ProviderArchiveCodec(error),
    )
}

fn read_private_file(
    output: &PinnedOutput,
    entry: &str,
    exact_length: usize,
    component: &'static str,
) -> Result<Vec<u8>, WorkerV3PublicationIntentErrorV1> {
    let (mut file, before) = open_private_file(output, entry, exact_length)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(exact_length).map_err(|_| {
        WorkerV3PublicationIntentErrorV1::AllocationFailed {
            component,
            requested: exact_length,
        }
    })?;
    bytes.resize(exact_length, 0);
    file.read_exact(&mut bytes)?;
    finish_private_file_read(output, entry, &mut file, &before, exact_length)?;
    Ok(bytes)
}

fn open_private_file(
    output: &PinnedOutput,
    entry: &str,
    exact_length: usize,
) -> Result<(fs::File, rustix::fs::Stat), WorkerV3PublicationIntentErrorV1> {
    open_private_file_with_access(output, entry, exact_length, false)
}

fn open_private_file_with_access(
    output: &PinnedOutput,
    entry: &str,
    exact_length: usize,
    writable: bool,
) -> Result<(fs::File, rustix::fs::Stat), WorkerV3PublicationIntentErrorV1> {
    let access = if writable {
        OFlags::RDWR
    } else {
        OFlags::RDONLY
    };
    let fd = openat(
        &output.fd,
        entry,
        access | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        invalid(
            output,
            entry,
            if error == rustix::io::Errno::LOOP {
                WorkerV3PublicationIntentInvalidReasonV1::EntryNotPrivate
            } else {
                WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead
            },
        )
    })?;
    let file = fs::File::from(fd);
    let before = fstat(&file).map_err(std::io::Error::from)?;
    if !is_private_file(&before) {
        return Err(invalid(
            output,
            entry,
            WorkerV3PublicationIntentInvalidReasonV1::EntryNotPrivate,
        ));
    }
    if usize::try_from(before.st_size).ok() != Some(exact_length) {
        return Err(invalid(
            output,
            entry,
            WorkerV3PublicationIntentInvalidReasonV1::FileLengthMismatch {
                actual: u64::try_from(before.st_size).ok(),
                expected: exact_length,
            },
        ));
    }
    Ok((file, before))
}

fn resync_validated_private_file(
    output: &PinnedOutput,
    entry: &str,
    file: &fs::File,
    exact_length: usize,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    file.sync_all()?;
    validate_renamed_file(output, entry, file, exact_length)?;
    fsync(&output.fd).map_err(std::io::Error::from)?;
    output.verify_path_identity()?;
    validate_renamed_file(output, entry, file, exact_length)
}

fn finish_private_file_read(
    output: &PinnedOutput,
    entry: &str,
    file: &mut fs::File,
    before: &rustix::fs::Stat,
    exact_length: usize,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    let mut trailing = [0_u8; 1];
    if file.read(&mut trailing)? != 0 {
        return Err(invalid(
            output,
            entry,
            WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
        ));
    }
    let after = fstat(&file).map_err(std::io::Error::from)?;
    let named =
        statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    if !same_private_file(before, &after, exact_length)
        || !same_private_file(before, &named, exact_length)
    {
        return Err(invalid(
            output,
            entry,
            WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
        ));
    }
    Ok(())
}

fn require_final_entry_headroom(
    output: &PinnedOutput,
    names: &IntentNames,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    let directory = rustix::io::fcntl_dupfd_cloexec(&output.fd, 0).map_err(std::io::Error::from)?;
    let mut entries = rustix::fs::Dir::read_from(&directory).map_err(std::io::Error::from)?;
    let mut actual = 0_usize;
    let mut existing_final = 0_usize;
    for entry in &mut entries {
        let entry = entry.map_err(std::io::Error::from)?;
        let name = entry.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        actual = actual
            .checked_add(1)
            .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?;
        if actual > MAX_OUTPUT_ENTRIES {
            return Err(invalid(
                output,
                &names.base,
                WorkerV3PublicationIntentInvalidReasonV1::DirectoryEntryLimitExceeded {
                    maximum: MAX_OUTPUT_ENTRIES,
                },
            ));
        }
        if names.is_final_entry(name) {
            existing_final = existing_final
                .checked_add(1)
                .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?;
        }
    }
    let required = WORKER_V3_PUBLICATION_INTENT_FINAL_ENTRY_HEADROOM_V1
        .checked_sub(existing_final)
        .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?;
    if !final_entry_headroom_available(actual, existing_final)? {
        return Err(invalid(
            output,
            &names.base,
            WorkerV3PublicationIntentInvalidReasonV1::DirectoryEntryHeadroomInsufficient {
                actual,
                required,
                maximum: MAX_OUTPUT_ENTRIES,
            },
        ));
    }
    Ok(())
}

fn final_entry_headroom_available(
    actual: usize,
    existing_final: usize,
) -> Result<bool, WorkerV3PublicationIntentErrorV1> {
    let required = WORKER_V3_PUBLICATION_INTENT_FINAL_ENTRY_HEADROOM_V1
        .checked_sub(existing_final)
        .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?;
    Ok(actual
        .checked_add(required)
        .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?
        <= MAX_OUTPUT_ENTRIES)
}

fn create_temp(
    output: &PinnedOutput,
    names: &IntentNames,
    purpose: &str,
    boundary: WorkerV3PublicationIntentBoundaryV1,
    faults: &mut FaultInjector,
) -> Result<(String, fs::File), WorkerV3PublicationIntentErrorV1> {
    faults.hit(boundary, WorkerV3PublicationIntentFaultTimingV1::Before)?;
    let start = NEXT_TEMP_ID.fetch_add(MAX_TEMP_ATTEMPTS, Ordering::Relaxed);
    for offset in 0..MAX_TEMP_ATTEMPTS {
        let name = temp_name(
            &names.temp_prefix,
            purpose,
            std::process::id(),
            start.wrapping_add(offset),
        )?;
        match openat(
            &output.fd,
            &name,
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        ) {
            Ok(fd) => {
                faults.hit(boundary, WorkerV3PublicationIntentFaultTimingV1::After)?;
                return Ok((name, fs::File::from(fd)));
            }
            Err(error) if error == rustix::io::Errno::EXIST => {}
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
    }
    Err(invalid(
        output,
        &names.temp_prefix,
        WorkerV3PublicationIntentInvalidReasonV1::TemporaryNameExhausted,
    ))
}

fn cleanup_temps(
    output: &PinnedOutput,
    names: &IntentNames,
) -> Result<TempCleanupOutcomeV1, WorkerV3PublicationIntentErrorV1> {
    let directory = rustix::io::fcntl_dupfd_cloexec(&output.fd, 0).map_err(std::io::Error::from)?;
    let mut entries = rustix::fs::Dir::read_from(&directory).map_err(std::io::Error::from)?;
    let mut temps = Vec::new();
    temps
        .try_reserve_exact(MAX_TEMP_ATTEMPTS as usize)
        .map_err(|_| WorkerV3PublicationIntentErrorV1::AllocationFailed {
            component: "temporary-name cleanup",
            requested: MAX_TEMP_ATTEMPTS as usize,
        })?;
    let mut scanned_entries = 0usize;
    let mut temp_entries = 0usize;
    let mut quarantined_retiring_record = None;
    for entry in &mut entries {
        let entry = entry.map_err(std::io::Error::from)?;
        let name_bytes = entry.file_name().to_bytes();
        if name_bytes == b"." || name_bytes == b".." {
            continue;
        }
        scanned_entries = scanned_entries
            .checked_add(1)
            .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?;
        if scanned_entries > MAX_OUTPUT_ENTRIES {
            return Err(invalid(
                output,
                &names.temp_prefix,
                WorkerV3PublicationIntentInvalidReasonV1::DirectoryEntryLimitExceeded {
                    maximum: MAX_OUTPUT_ENTRIES,
                },
            ));
        }
        if !name_bytes.starts_with(names.temp_prefix.as_bytes()) {
            continue;
        }
        temp_entries = temp_entries
            .checked_add(1)
            .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?;
        if temp_entries > MAX_TEMP_ATTEMPTS as usize {
            return Err(invalid(
                output,
                &names.temp_prefix,
                WorkerV3PublicationIntentInvalidReasonV1::TemporaryEntryLimitExceeded {
                    maximum: MAX_TEMP_ATTEMPTS as usize,
                },
            ));
        }
        let name = PathBuf::from(std::ffi::OsStr::from_bytes(name_bytes));
        let snapshot = private_entry_snapshot(output, &name)?.ok_or_else(|| {
            invalid(
                output,
                &name,
                WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
            )
        })?;
        let candidate = CleanupCandidate { name, snapshot };
        if is_retiring_record_quarantine(name_bytes, names) {
            if quarantined_retiring_record.is_some() {
                return Err(invalid(
                    output,
                    &candidate.name,
                    WorkerV3PublicationIntentInvalidReasonV1::CommittedAndRetiringCoexist,
                ));
            }
            quarantined_retiring_record = Some(candidate);
        } else {
            temps.push(candidate);
        }
    }
    if !temps.is_empty() {
        output.verify_path_identity()?;
        let mut faults = FaultInjector::new(None);
        for (index, candidate) in temps.iter().enumerate() {
            let purpose = format!("stale-temp-{index}");
            unlink_exact_private_candidate(
                output,
                names,
                candidate,
                &purpose,
                CleanupBoundariesV1 {
                    quarantine: WorkerV3PublicationIntentBoundaryV1::RenameOutputToQuarantine,
                    remove: WorkerV3PublicationIntentBoundaryV1::RemoveOutput,
                },
                &mut faults,
            )?;
        }
        fsync(&output.fd).map_err(std::io::Error::from)?;
    }
    Ok(TempCleanupOutcomeV1 {
        removed_entries: temps.len(),
        quarantined_retiring_record,
    })
}

fn is_retiring_record_quarantine(name: &[u8], names: &IntentNames) -> bool {
    let Some(suffix) = name
        .strip_prefix(names.temp_prefix.as_bytes())
        .and_then(|suffix| suffix.strip_prefix(b"retire-record-"))
    else {
        return false;
    };
    let mut fields = suffix.split(|byte| *byte == b'-');
    let Some(process_id) = fields.next() else {
        return false;
    };
    let Some(id) = fields.next() else {
        return false;
    };
    fields.next().is_none()
        && !process_id.is_empty()
        && process_id.iter().all(u8::is_ascii_digit)
        && !id.is_empty()
        && id.iter().all(u8::is_ascii_digit)
}

fn validate_renamed_file(
    output: &PinnedOutput,
    entry: &str,
    file: &fs::File,
    length: usize,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    let pinned = fstat(file).map_err(std::io::Error::from)?;
    let named =
        statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    if !same_private_file(&pinned, &named, length) {
        return Err(invalid(
            output,
            entry,
            WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
        ));
    }
    Ok(())
}

fn entry_exists(
    output: &PinnedOutput,
    entry: &str,
) -> Result<bool, WorkerV3PublicationIntentErrorV1> {
    match statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => {
            if !is_private_file(&stat) {
                return Err(invalid(
                    output,
                    entry,
                    WorkerV3PublicationIntentInvalidReasonV1::EntryNotPrivate,
                ));
            }
            Ok(true)
        }
        Err(error) if error == rustix::io::Errno::NOENT => Ok(false),
        Err(error) => Err(std::io::Error::from(error).into()),
    }
}

fn is_private_file(stat: &rustix::fs::Stat) -> bool {
    FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile
        && stat.st_nlink == 1
        && stat.st_mode & 0o777 == 0o600
}

fn same_private_file(left: &rustix::fs::Stat, right: &rustix::fs::Stat, length: usize) -> bool {
    is_private_file(left)
        && is_private_file(right)
        && left.st_dev == right.st_dev
        && left.st_ino == right.st_ino
        && usize::try_from(left.st_size).ok() == Some(length)
        && usize::try_from(right.st_size).ok() == Some(length)
        && left.st_mtime == right.st_mtime
        && left.st_mtime_nsec == right.st_mtime_nsec
        && left.st_ctime == right.st_ctime
        && left.st_ctime_nsec == right.st_ctime_nsec
}

fn invalid(
    output: &PinnedOutput,
    entry: impl AsRef<Path>,
    reason: WorkerV3PublicationIntentInvalidReasonV1,
) -> WorkerV3PublicationIntentErrorV1 {
    WorkerV3PublicationIntentErrorV1::InvalidIntent {
        path: output.display_path.join(entry),
        reason,
    }
}

struct IntentNames {
    base: String,
    outer_handoff: String,
    external_providers: String,
    output: String,
    transcript: String,
    record: String,
    redo: String,
    retiring: String,
    temp_prefix: String,
}

impl IntentNames {
    fn new(
        producer_key: [u8; 32],
        occurrence_key: [u8; 32],
    ) -> Result<Self, WorkerV3PublicationIntentErrorV1> {
        let producer = hex(&producer_key)?;
        let occurrence = hex(&occurrence_key)?;
        let base_capacity = FILE_PREFIX_V1
            .len()
            .checked_add(producer.len())
            .and_then(|value| value.checked_add(1))
            .and_then(|value| value.checked_add(occurrence.len()))
            .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?;
        let mut base = String::new();
        reserve_string(&mut base, base_capacity, "intent base name")?;
        base.push_str(FILE_PREFIX_V1);
        base.push_str(&producer);
        base.push('-');
        base.push_str(&occurrence);
        Ok(Self {
            outer_handoff: append_suffix(&base, OUTER_HANDOFF_SUFFIX, "outer handoff name")?,
            external_providers: append_suffix(
                &base,
                EXTERNAL_PROVIDERS_SUFFIX,
                "external providers name",
            )?,
            output: append_suffix(&base, OUTPUT_SUFFIX, "output name")?,
            transcript: append_suffix(&base, TRANSCRIPT_SUFFIX, "transcript name")?,
            record: append_suffix(&base, RECORD_SUFFIX, "record name")?,
            redo: append_suffix(&base, REDO_SUFFIX, "redo name")?,
            retiring: append_suffix(&base, RETIRING_SUFFIX, "retiring record name")?,
            temp_prefix: append_suffix(&base, TEMP_SUFFIX, "temporary prefix")?,
            base,
        })
    }

    fn is_final_entry(&self, name: &[u8]) -> bool {
        [
            &self.outer_handoff,
            &self.external_providers,
            &self.transcript,
            &self.output,
            &self.record,
        ]
        .into_iter()
        .any(|entry| name == entry.as_bytes())
    }
}

fn append_suffix(
    base: &str,
    suffix: &str,
    component: &'static str,
) -> Result<String, WorkerV3PublicationIntentErrorV1> {
    let capacity = base
        .len()
        .checked_add(suffix.len())
        .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?;
    let mut value = String::new();
    reserve_string(&mut value, capacity, component)?;
    value.push_str(base);
    value.push_str(suffix);
    Ok(value)
}

fn temp_name(
    prefix: &str,
    purpose: &str,
    process_id: u32,
    id: u64,
) -> Result<String, WorkerV3PublicationIntentErrorV1> {
    const NUMERIC_SUFFIX_BOUND: usize = 1 + 10 + 1 + 20;
    let capacity = prefix
        .len()
        .checked_add(purpose.len())
        .and_then(|value| value.checked_add(NUMERIC_SUFFIX_BOUND))
        .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?;
    let mut value = String::new();
    reserve_string(&mut value, capacity, "temporary name")?;
    value.push_str(prefix);
    value.push_str(purpose);
    write!(&mut value, "-{process_id}-{id}").map_err(|_| {
        WorkerV3PublicationIntentErrorV1::AllocationFailed {
            component: "temporary name",
            requested: capacity,
        }
    })?;
    Ok(value)
}

fn reserve_string(
    value: &mut String,
    additional: usize,
    component: &'static str,
) -> Result<(), WorkerV3PublicationIntentErrorV1> {
    value.try_reserve_exact(additional).map_err(|_| {
        WorkerV3PublicationIntentErrorV1::AllocationFailed {
            component,
            requested: additional,
        }
    })
}

fn hex(bytes: &[u8]) -> Result<String, WorkerV3PublicationIntentErrorV1> {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let capacity = bytes
        .len()
        .checked_mul(2)
        .ok_or(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)?;
    let mut encoded = String::new();
    reserve_string(&mut encoded, capacity, "hex storage key")?;
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    Ok(encoded)
}

struct FaultInjector {
    point: Option<WorkerV3PublicationIntentFaultPointV1>,
    fired: bool,
}

impl FaultInjector {
    const fn new(point: Option<WorkerV3PublicationIntentFaultPointV1>) -> Self {
        Self {
            point,
            fired: false,
        }
    }

    fn hit(
        &mut self,
        boundary: WorkerV3PublicationIntentBoundaryV1,
        timing: WorkerV3PublicationIntentFaultTimingV1,
    ) -> Result<(), WorkerV3PublicationIntentErrorV1> {
        let point = WorkerV3PublicationIntentFaultPointV1 { boundary, timing };
        if !self.fired && self.point == Some(point) {
            self.fired = true;
            Err(WorkerV3PublicationIntentErrorV1::InjectedCrash { point })
        } else {
            Ok(())
        }
    }

    fn around(
        &mut self,
        boundary: WorkerV3PublicationIntentBoundaryV1,
        operation: impl FnOnce() -> Result<(), WorkerV3PublicationIntentErrorV1>,
    ) -> Result<(), WorkerV3PublicationIntentErrorV1> {
        self.hit(boundary, WorkerV3PublicationIntentFaultTimingV1::Before)?;
        operation()?;
        self.hit(boundary, WorkerV3PublicationIntentFaultTimingV1::After)
    }
}

fn validate_provider_owner_capacity_values(
    list_capacity: usize,
    payload_capacities: impl IntoIterator<Item = usize>,
) -> Result<usize, WorkerV3PublicationIntentCodecErrorV1> {
    if list_capacity > MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1 {
        return Err(
            WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderListCapacity {
                actual: list_capacity,
                maximum: MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1,
            },
        );
    }
    let mut aggregate = 0_usize;
    for (index, capacity) in payload_capacities.into_iter().enumerate() {
        if capacity > MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1 {
            return Err(
                WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderPayloadCapacity {
                    index,
                    actual: capacity,
                    maximum: MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
                },
            );
        }
        aggregate = aggregate
            .checked_add(capacity)
            .ok_or(WorkerV3PublicationIntentCodecErrorV1::OwnerCapacityArithmeticOverflow)?;
    }
    if aggregate > MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1 {
        return Err(
            WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderAggregateCapacity {
                actual: aggregate,
                maximum: MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
            },
        );
    }
    let list_bytes = list_capacity
        .checked_mul(std::mem::size_of::<Vec<u8>>())
        .ok_or(WorkerV3PublicationIntentCodecErrorV1::OwnerCapacityArithmeticOverflow)?;
    list_bytes
        .checked_add(aggregate)
        .ok_or(WorkerV3PublicationIntentCodecErrorV1::OwnerCapacityArithmeticOverflow)
}

fn provider_owner_capacity_bytes(
    payloads: &Vec<Vec<u8>>,
) -> Result<usize, WorkerV3PublicationIntentCodecErrorV1> {
    validate_provider_owner_capacity_values(payloads.capacity(), payloads.iter().map(Vec::capacity))
}

fn validate_caller_owner_capacities(
    outer_handoff: &Vec<u8>,
    external_providers: &WorkerV3ExternalProviderPayloadsV1,
    transcript: &Vec<u8>,
    output: Option<&Vec<u8>>,
) -> Result<usize, WorkerV3PublicationIntentCodecErrorV1> {
    validate_caller_owner_capacity_values(
        outer_handoff.capacity(),
        external_providers.caller_owner_capacity_bytes()?,
        transcript.capacity(),
        output.map_or(0, Vec::capacity),
    )
}

fn validate_caller_owner_capacity_values(
    outer_handoff_capacity: usize,
    provider_owner_capacity: usize,
    transcript_capacity: usize,
    output_capacity: usize,
) -> Result<usize, WorkerV3PublicationIntentCodecErrorV1> {
    if outer_handoff_capacity > MAX_COMPILER_MODULE_HANDOFF_BYTES_V3 {
        return Err(
            WorkerV3PublicationIntentCodecErrorV1::InvalidOuterHandoffCapacity {
                actual: outer_handoff_capacity,
                maximum: MAX_COMPILER_MODULE_HANDOFF_BYTES_V3,
            },
        );
    }
    if transcript_capacity > MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1 {
        return Err(
            WorkerV3PublicationIntentCodecErrorV1::InvalidTranscriptCapacity {
                actual: transcript_capacity,
                maximum: MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1,
            },
        );
    }
    if output_capacity > MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1 {
        return Err(
            WorkerV3PublicationIntentCodecErrorV1::InvalidOutputCapacity {
                actual: output_capacity,
                maximum: MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1,
            },
        );
    }
    let required = outer_handoff_capacity
        .checked_add(provider_owner_capacity)
        .and_then(|value| value.checked_add(transcript_capacity))
        .and_then(|value| value.checked_add(output_capacity))
        .ok_or(WorkerV3PublicationIntentCodecErrorV1::OwnerCapacityArithmeticOverflow)?;
    if required > MAX_WORKER_V3_PUBLICATION_INTENT_CALLER_OWNER_CAPACITY_BYTES_V1 {
        return Err(
            WorkerV3PublicationIntentCodecErrorV1::OwnerCapacityBudgetExceeded {
                required,
                maximum: MAX_WORKER_V3_PUBLICATION_INTENT_CALLER_OWNER_CAPACITY_BYTES_V1,
            },
        );
    }
    Ok(required)
}

fn validate_payload_lengths(
    outer_handoff_length: usize,
    external_provider_archive_length: usize,
    external_provider_count: usize,
    external_provider_payload_length: usize,
    transcript_length: usize,
    output_length: usize,
) -> Result<(), WorkerV3PublicationIntentCodecErrorV1> {
    if outer_handoff_length == 0 || outer_handoff_length > MAX_COMPILER_MODULE_HANDOFF_BYTES_V3 {
        return Err(
            WorkerV3PublicationIntentCodecErrorV1::InvalidOuterHandoffLength {
                actual: outer_handoff_length as u64,
                maximum: MAX_COMPILER_MODULE_HANDOFF_BYTES_V3,
            },
        );
    }
    if external_provider_count > MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1 {
        return Err(
            WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderCount {
                actual: external_provider_count as u64,
                maximum: MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1,
            },
        );
    }
    if external_provider_payload_length > MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1
        || (external_provider_count == 0) != (external_provider_payload_length == 0)
    {
        return Err(
            WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderPayloadLength {
                actual: external_provider_payload_length as u64,
                maximum: MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
            },
        );
    }
    let expected_archive_length =
        provider_archive_length(external_provider_count, external_provider_payload_length)?;
    if external_provider_archive_length != expected_archive_length {
        return Err(
            WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderArchiveLength {
                actual: external_provider_archive_length as u64,
                maximum: expected_archive_length,
            },
        );
    }
    if output_length == 0 || output_length > MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1 {
        return Err(WorkerV3PublicationIntentCodecErrorV1::InvalidOutputLength {
            actual: output_length as u64,
            maximum: MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1,
        });
    }
    if transcript_length == 0
        || transcript_length > MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1
    {
        return Err(
            WorkerV3PublicationIntentCodecErrorV1::InvalidTranscriptLength {
                actual: transcript_length as u64,
                maximum: MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1,
            },
        );
    }
    let required = MAX_WORKER_V3_PUBLICATION_INTENT_RECORD_BYTES_V1
        .checked_add(outer_handoff_length)
        .and_then(|length| length.checked_add(external_provider_archive_length))
        .and_then(|length| length.checked_add(transcript_length))
        .and_then(|length| length.checked_add(output_length))
        .ok_or(WorkerV3PublicationIntentCodecErrorV1::LengthArithmeticOverflow)?;
    if required > MAX_WORKER_V3_PUBLICATION_INTENT_RECOVERY_BYTES_V1 {
        return Err(
            WorkerV3PublicationIntentCodecErrorV1::RecoveryBudgetExceeded {
                required,
                maximum: MAX_WORKER_V3_PUBLICATION_INTENT_RECOVERY_BYTES_V1,
            },
        );
    }
    Ok(())
}

fn provider_archive_length(
    count: usize,
    payload_length: usize,
) -> Result<usize, WorkerV3PublicationIntentCodecErrorV1> {
    if count > MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1 {
        return Err(
            WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderCount {
                actual: count as u64,
                maximum: MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1,
            },
        );
    }
    if payload_length > MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1 {
        return Err(
            WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderPayloadLength {
                actual: payload_length as u64,
                maximum: MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
            },
        );
    }
    PROVIDER_ARCHIVE_FIXED_BYTES_V1
        .checked_add(
            count
                .checked_mul(PROVIDER_ARCHIVE_ENTRY_BYTES_V1)
                .ok_or(WorkerV3PublicationIntentCodecErrorV1::LengthArithmeticOverflow)?,
        )
        .and_then(|length| length.checked_add(payload_length))
        .ok_or(WorkerV3PublicationIntentCodecErrorV1::LengthArithmeticOverflow)
}

fn update_provider_archive_body(digest: &mut Sha256, payloads: &[Vec<u8>], payload_length: usize) {
    digest.update(PROVIDER_ARCHIVE_MAGIC_V1);
    digest.update(PROVIDER_ARCHIVE_VERSION_V1.to_le_bytes());
    digest.update((payloads.len() as u32).to_le_bytes());
    digest.update((payload_length as u64).to_le_bytes());
    for payload in payloads {
        digest.update((payload.len() as u64).to_le_bytes());
        digest.update(sha256(payload));
    }
    for payload in payloads {
        digest.update(payload);
    }
}

fn provider_archive_checksum(payloads: &[Vec<u8>], payload_length: usize) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(PROVIDER_ARCHIVE_CHECKSUM_DOMAIN_V1);
    update_provider_archive_body(&mut digest, payloads, payload_length);
    digest.finalize().into()
}

fn provider_archive_sha256(
    payloads: &[Vec<u8>],
    payload_length: usize,
    checksum: [u8; 32],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    update_provider_archive_body(&mut digest, payloads, payload_length);
    digest.update(checksum);
    digest.finalize().into()
}

fn bounded_length(encoded: u64, maximum: usize) -> Result<usize, ()> {
    let decoded = usize::try_from(encoded).map_err(|_| ())?;
    if decoded == 0 || decoded > maximum {
        return Err(());
    }
    Ok(decoded)
}

fn bounded_length_allow_zero(encoded: u64, maximum: usize) -> Result<usize, ()> {
    let decoded = usize::try_from(encoded).map_err(|_| ())?;
    if decoded > maximum {
        return Err(());
    }
    Ok(decoded)
}

fn producer_key(producer: &ProducerIdentity) -> [u8; 32] {
    sha256_parts(&[
        PRODUCER_KEY_DOMAIN_V1,
        &(producer.stable_source.len() as u64).to_le_bytes(),
        producer.stable_source.as_bytes(),
        &(producer.crate_name.len() as u64).to_le_bytes(),
        producer.crate_name.as_bytes(),
    ])
}

fn occurrence_key(producer_key: [u8; 32], attempt: BuildAttempt) -> [u8; 32] {
    sha256_parts(&[
        OCCURRENCE_KEY_DOMAIN_V1,
        &producer_key,
        &attempt.generation().to_le_bytes(),
        attempt.session().as_bytes(),
        attempt.invocation().as_bytes(),
    ])
}

fn push_scope(bytes: &mut Vec<u8>, scope: LinkPublicationScopeV1) {
    bytes.extend_from_slice(scope.package().as_bytes());
    bytes.extend_from_slice(scope.kernel_set().as_bytes());
    bytes.extend_from_slice(scope.target().as_bytes());
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn sha256_parts(parts: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    for part in parts {
        digest.update(part);
    }
    digest.finalize().into()
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], WorkerV3PublicationIntentCodecErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(WorkerV3PublicationIntentCodecErrorV1::TruncatedField)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(WorkerV3PublicationIntentCodecErrorV1::TruncatedField)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], WorkerV3PublicationIntentCodecErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| WorkerV3PublicationIntentCodecErrorV1::TruncatedField)
    }

    fn u16(&mut self) -> Result<u16, WorkerV3PublicationIntentCodecErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, WorkerV3PublicationIntentCodecErrorV1> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, WorkerV3PublicationIntentCodecErrorV1> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn attempt() -> BuildAttempt {
        BuildAttempt::new(
            17,
            BuildSession::from_bytes([0x21; 16]),
            BuildInvocation::from_bytes([0x22; 32]),
        )
        .unwrap()
    }

    fn producer() -> ProducerIdentity {
        ProducerIdentity::from_codegen("codec", Some(Path::new("/src/v3.rs"))).unwrap()
    }

    fn plan(output: &[u8]) -> DurableLinkPublicationPlanV1 {
        let attempt = attempt();
        DurableLinkPublicationPlanV1::new(
            attempt,
            LinkPublicationScopeV1::new(
                PackageIdentityV1::from_bytes([1; 32]),
                KernelSetIdentityV1::from_bytes([2; 32]),
                TargetIdentityV1::from_bytes([3; 32]),
            ),
            CanonicalLinkRequestIdentityV1::from_bytes([4; 32]),
            PinnedWorkerIdentityV1::from_bytes([5; 32]),
            ValidatedResponseIdentityV1::from_bytes([6; 32]),
            LinkedOutputIdentityV1::from_bytes([7; 32]),
            FinalizationIdentityV1::from_bytes([8; 32]),
            FinalizedOutputIdentityV1::from_bytes(sha256(output)),
            AtomicPublicationIdentityV1::from_bytes([9; 32]),
        )
    }

    fn record() -> WorkerV3PublicationIntentRecordV1 {
        let output = b"exact finalized output";
        let attachments = WorkerV3FinalizerReplayAttachmentsV1::new(
            b"exact outer V3 handoff".to_vec(),
            vec![b"provider A".to_vec(), b"provider B".to_vec()],
            b"compact canonical replay metadata".to_vec(),
        )
        .unwrap();
        WorkerV3PublicationIntentRecordV1::from_exact_bytes(
            &producer(),
            attempt(),
            plan(output),
            &attachments,
            output,
        )
        .unwrap()
    }

    fn rewrite_checksum(bytes: &mut [u8]) {
        let body_length = bytes.len() - 32;
        let checksum = sha256_parts(&[RECORD_CHECKSUM_DOMAIN_V1, &bytes[..body_length]]);
        bytes[body_length..].copy_from_slice(&checksum);
    }

    #[test]
    fn codec_round_trip_binds_exact_storage_bytes_without_authority_claims() {
        let record = record();
        let encoded = record.encode_canonical().unwrap();
        assert_eq!(
            encoded.len(),
            MAX_WORKER_V3_PUBLICATION_INTENT_RECORD_BYTES_V1
        );
        assert_eq!(
            WorkerV3PublicationIntentRecordV1::decode_canonical(&encoded).unwrap(),
            record
        );
        assert_eq!(record.attempt(), attempt());
        assert_eq!(record.plan(), plan(b"exact finalized output"));
        assert_eq!(
            record.outer_handoff_sha256(),
            sha256(b"exact outer V3 handoff")
        );
        assert_eq!(record.external_provider_count(), 2);
        assert_eq!(record.external_provider_payload_length(), 20);
        assert_eq!(record.output_sha256(), sha256(b"exact finalized output"));
        assert_eq!(
            record.transcript_sha256(),
            sha256(b"compact canonical replay metadata")
        );
        assert!(!record.authenticates_finalizer_transcript());
        assert!(!record.grants_publication_authority());
        assert!(!record.grants_load_authority());
        assert!(!record.grants_launch_authority());
    }

    #[test]
    fn codec_rejects_truncation_trailing_checksum_magic_and_version_mutation() {
        let encoded = record().encode_canonical().unwrap();
        assert!(matches!(
            WorkerV3PublicationIntentRecordV1::decode_canonical(&encoded[..encoded.len() - 1]),
            Err(WorkerV3PublicationIntentCodecErrorV1::NoncanonicalLength { .. })
        ));
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(matches!(
            WorkerV3PublicationIntentRecordV1::decode_canonical(&trailing),
            Err(WorkerV3PublicationIntentCodecErrorV1::NoncanonicalLength { .. })
        ));
        let mut checksum = encoded.clone();
        checksum[100] ^= 1;
        assert_eq!(
            WorkerV3PublicationIntentRecordV1::decode_canonical(&checksum),
            Err(WorkerV3PublicationIntentCodecErrorV1::ChecksumMismatch)
        );
        let mut magic = encoded.clone();
        magic[0] ^= 1;
        rewrite_checksum(&mut magic);
        assert_eq!(
            WorkerV3PublicationIntentRecordV1::decode_canonical(&magic),
            Err(WorkerV3PublicationIntentCodecErrorV1::MagicMismatch)
        );
        let mut version = encoded;
        let version_offset = RECORD_MAGIC_V1.len();
        version[version_offset..version_offset + 2].copy_from_slice(&2_u16.to_le_bytes());
        rewrite_checksum(&mut version);
        assert_eq!(
            WorkerV3PublicationIntentRecordV1::decode_canonical(&version),
            Err(WorkerV3PublicationIntentCodecErrorV1::UnsupportedVersion { actual: 2 })
        );
    }

    #[test]
    fn codec_rejects_mutated_plan_commitment_and_duplicate_output_hash() {
        let encoded = record().encode_canonical().unwrap();
        let plan_commitment_offset = RECORD_MAGIC_V1.len() + 2 + 32 + 8 + 16 + 32 + 32;
        let mut plan_commitment = encoded.clone();
        plan_commitment[plan_commitment_offset] ^= 1;
        rewrite_checksum(&mut plan_commitment);
        assert_eq!(
            WorkerV3PublicationIntentRecordV1::decode_canonical(&plan_commitment),
            Err(WorkerV3PublicationIntentCodecErrorV1::PlanCommitmentMismatch)
        );

        let output_hash_offset = encoded.len() - 32 - 8 - 32;
        let mut output_hash = encoded;
        output_hash[output_hash_offset] ^= 1;
        rewrite_checksum(&mut output_hash);
        assert_eq!(
            WorkerV3PublicationIntentRecordV1::decode_canonical(&output_hash),
            Err(WorkerV3PublicationIntentCodecErrorV1::OutputPlanMismatch)
        );
    }

    #[test]
    fn codec_rederives_the_occurrence_key_from_producer_and_attempt() {
        let mut encoded = record().encode_canonical().unwrap();
        let occurrence_offset = RECORD_MAGIC_V1.len() + 2;
        encoded[occurrence_offset] ^= 1;
        rewrite_checksum(&mut encoded);
        assert_eq!(
            WorkerV3PublicationIntentRecordV1::decode_canonical(&encoded),
            Err(WorkerV3PublicationIntentCodecErrorV1::OccurrenceKeyMismatch)
        );
    }

    #[test]
    fn codec_enforces_payload_bounds_without_allocating_limit_sized_buffers() {
        let empty_archive = provider_archive_length(0, 0).unwrap();
        assert!(validate_payload_lengths(1, empty_archive, 0, 0, 1, 1).is_ok());
        assert!(
            validate_payload_lengths(
                MAX_COMPILER_MODULE_HANDOFF_BYTES_V3,
                MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_ARCHIVE_BYTES_V1,
                MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1,
                MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
                MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1,
                MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1,
            )
            .is_ok()
        );
        assert!(matches!(
            validate_payload_lengths(0, empty_archive, 0, 0, 1, 1),
            Err(WorkerV3PublicationIntentCodecErrorV1::InvalidOuterHandoffLength { actual: 0, .. })
        ));
        assert!(matches!(
            validate_payload_lengths(
                1,
                empty_archive,
                0,
                0,
                1,
                MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1 + 1,
            ),
            Err(WorkerV3PublicationIntentCodecErrorV1::InvalidOutputLength { .. })
        ));
        assert!(matches!(
            validate_payload_lengths(1, empty_archive, 0, 0, 0, 1),
            Err(WorkerV3PublicationIntentCodecErrorV1::InvalidTranscriptLength { actual: 0, .. })
        ));
        assert!(matches!(
            validate_payload_lengths(
                1,
                empty_archive,
                0,
                0,
                MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1 + 1,
                1,
            ),
            Err(WorkerV3PublicationIntentCodecErrorV1::InvalidTranscriptLength { .. })
        ));
    }

    #[test]
    fn recovery_working_set_uses_checked_independent_budget_accounting() {
        assert_eq!(
            validate_recovery_working_set(
                MAX_COMPILER_MODULE_HANDOFF_BYTES_V3,
                MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_ARCHIVE_BYTES_V1,
                MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1,
                MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1,
                MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1,
            )
            .unwrap(),
            MAX_WORKER_V3_PUBLICATION_INTENT_RECOVERY_BYTES_V1
        );
        assert!(matches!(
            validate_recovery_working_set(
                MAX_COMPILER_MODULE_HANDOFF_BYTES_V3,
                MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_ARCHIVE_BYTES_V1,
                MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1,
                MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1 + 1,
                MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1,
            ),
            Err(WorkerV3PublicationIntentErrorV1::WorkingSetBudgetExceeded {
                maximum: MAX_WORKER_V3_PUBLICATION_INTENT_RECOVERY_BYTES_V1,
                ..
            })
        ));
        assert!(matches!(
            validate_recovery_working_set(usize::MAX, 1, 0, 1, 1),
            Err(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)
        ));
    }

    #[test]
    fn transcript_and_owner_capacity_formulas_are_independent_and_checked() {
        assert_eq!(
            MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1,
            2 * MAX_REPLAY_RESPONSE_METADATA_SHELL_BYTES_V1
                + MAX_REPLAY_SHARED_WORKER_OPTION_METADATA_BYTES_V1
                + MAX_REPLAY_SHARED_FRAMING_AND_IDENTITIES_BYTES_V1
        );
        assert_eq!(
            MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1,
            2_195_505
        );
        assert_eq!(
            validate_caller_owner_capacity_values(
                MAX_COMPILER_MODULE_HANDOFF_BYTES_V3,
                MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1
                    + MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1
                        * std::mem::size_of::<Vec<u8>>(),
                MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1,
                MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1,
            )
            .unwrap(),
            MAX_WORKER_V3_PUBLICATION_INTENT_CALLER_OWNER_CAPACITY_BYTES_V1
        );
        #[cfg(target_pointer_width = "64")]
        {
            assert_eq!(
                MAX_WORKER_V3_PUBLICATION_INTENT_RECOVERY_BYTES_V1,
                388_610_319
            );
            assert_eq!(
                MAX_WORKER_V3_PUBLICATION_INTENT_CALLER_OWNER_CAPACITY_BYTES_V1,
                388_599_264
            );
        }
        assert!(matches!(
            validate_caller_owner_capacity_values(
                MAX_COMPILER_MODULE_HANDOFF_BYTES_V3 + 1,
                0,
                1,
                1,
            ),
            Err(WorkerV3PublicationIntentCodecErrorV1::InvalidOuterHandoffCapacity { .. })
        ));
        assert!(matches!(
            validate_caller_owner_capacity_values(
                1,
                0,
                MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1 + 1,
                1,
            ),
            Err(WorkerV3PublicationIntentCodecErrorV1::InvalidTranscriptCapacity { .. })
        ));
        assert!(matches!(
            validate_caller_owner_capacity_values(
                1,
                0,
                1,
                MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1 + 1,
            ),
            Err(WorkerV3PublicationIntentCodecErrorV1::InvalidOutputCapacity { .. })
        ));
        assert!(matches!(
            validate_caller_owner_capacity_values(usize::MAX, usize::MAX, usize::MAX, usize::MAX),
            Err(WorkerV3PublicationIntentCodecErrorV1::InvalidOuterHandoffCapacity { .. })
        ));
    }

    #[test]
    fn oversized_spare_provider_capacities_are_rejected_without_payload_bytes() {
        assert!(matches!(
            validate_provider_owner_capacity_values(
                MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1 + 1,
                [],
            ),
            Err(WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderListCapacity { .. })
        ));
        assert!(matches!(
            validate_provider_owner_capacity_values(
                1,
                [MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1 + 1],
            ),
            Err(
                WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderPayloadCapacity {
                    index: 0,
                    ..
                }
            )
        ));
        assert!(matches!(
            validate_provider_owner_capacity_values(
                2,
                [
                    MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1 / 2 + 1,
                    MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1 / 2 + 1,
                ],
            ),
            Err(
                WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderAggregateCapacity { .. }
            )
        ));

        let mut oversized_list =
            Vec::with_capacity(MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1 + 1);
        oversized_list.push(b"small payload".to_vec());
        assert!(matches!(
            WorkerV3ExternalProviderPayloadsV1::new(oversized_list),
            Err(WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderListCapacity { .. })
        ));

        let mut oversized_transcript =
            Vec::with_capacity(MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1 + 1);
        oversized_transcript.push(1);
        assert!(matches!(
            WorkerV3FinalizerReplayAttachmentsV1::new(vec![1], Vec::new(), oversized_transcript,),
            Err(WorkerV3PublicationIntentCodecErrorV1::InvalidTranscriptCapacity { .. })
        ));
    }

    #[test]
    fn final_entry_headroom_accepts_limit_minus_five_and_rejects_one_more() {
        assert!(
            final_entry_headroom_available(
                MAX_OUTPUT_ENTRIES - WORKER_V3_PUBLICATION_INTENT_FINAL_ENTRY_HEADROOM_V1,
                0,
            )
            .unwrap()
        );
        assert!(
            !final_entry_headroom_available(
                MAX_OUTPUT_ENTRIES - WORKER_V3_PUBLICATION_INTENT_FINAL_ENTRY_HEADROOM_V1 + 1,
                0,
            )
            .unwrap()
        );
        assert!(final_entry_headroom_available(MAX_OUTPUT_ENTRIES, 5).unwrap());
        assert!(matches!(
            final_entry_headroom_available(MAX_OUTPUT_ENTRIES, 6),
            Err(WorkerV3PublicationIntentErrorV1::WorkingSetArithmeticOverflow)
        ));
    }

    #[test]
    fn codec_rejects_zero_and_over_limit_lengths_even_with_a_valid_checksum() {
        let encoded = record().encode_canonical().unwrap();
        let attachment_offset =
            RECORD_MAGIC_V1.len() + 2 + 32 + 8 + 16 + 32 + 32 + 32 + 3 * 32 + 7 * 32;
        let outer_handoff_length_offset = attachment_offset + 32;
        let external_provider_count_offset = outer_handoff_length_offset + 8 + 32 + 8;
        let external_provider_payload_length_offset = external_provider_count_offset + 4;
        let output_length_offset = encoded.len() - 32 - 8;
        let transcript_length_offset = output_length_offset - 32 - 8;

        let mut oversized_handoff = encoded.clone();
        oversized_handoff[outer_handoff_length_offset..outer_handoff_length_offset + 8]
            .copy_from_slice(&((MAX_COMPILER_MODULE_HANDOFF_BYTES_V3 as u64) + 1).to_le_bytes());
        rewrite_checksum(&mut oversized_handoff);
        assert!(matches!(
            WorkerV3PublicationIntentRecordV1::decode_canonical(&oversized_handoff),
            Err(WorkerV3PublicationIntentCodecErrorV1::InvalidOuterHandoffLength { .. })
        ));

        let mut oversized_provider_count = encoded.clone();
        oversized_provider_count
            [external_provider_count_offset..external_provider_count_offset + 4]
            .copy_from_slice(
                &((MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1 as u32) + 1).to_le_bytes(),
            );
        rewrite_checksum(&mut oversized_provider_count);
        assert!(matches!(
            WorkerV3PublicationIntentRecordV1::decode_canonical(&oversized_provider_count),
            Err(WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderCount { .. })
        ));

        let mut oversized_provider_payload = encoded.clone();
        oversized_provider_payload
            [external_provider_payload_length_offset..external_provider_payload_length_offset + 8]
            .copy_from_slice(
                &((MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1 as u64) + 1).to_le_bytes(),
            );
        rewrite_checksum(&mut oversized_provider_payload);
        assert!(matches!(
            WorkerV3PublicationIntentRecordV1::decode_canonical(&oversized_provider_payload),
            Err(WorkerV3PublicationIntentCodecErrorV1::InvalidExternalProviderPayloadLength { .. })
        ));

        let mut zero_output = encoded.clone();
        zero_output[output_length_offset..output_length_offset + 8]
            .copy_from_slice(&0_u64.to_le_bytes());
        rewrite_checksum(&mut zero_output);
        assert!(matches!(
            WorkerV3PublicationIntentRecordV1::decode_canonical(&zero_output),
            Err(WorkerV3PublicationIntentCodecErrorV1::InvalidOutputLength { actual: 0, .. })
        ));

        let mut oversized_transcript = encoded;
        oversized_transcript[transcript_length_offset..transcript_length_offset + 8]
            .copy_from_slice(
                &((MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1 as u64) + 1).to_le_bytes(),
            );
        rewrite_checksum(&mut oversized_transcript);
        assert!(matches!(
            WorkerV3PublicationIntentRecordV1::decode_canonical(&oversized_transcript),
            Err(WorkerV3PublicationIntentCodecErrorV1::InvalidTranscriptLength { .. })
        ));
    }

    #[test]
    fn provider_archive_hash_binds_order_boundaries_and_exact_bytes() {
        let first = WorkerV3ExternalProviderPayloadsV1::new(vec![
            b"provider-a".to_vec(),
            b"provider-b".to_vec(),
        ])
        .unwrap();
        let reordered = WorkerV3ExternalProviderPayloadsV1::new(vec![
            b"provider-b".to_vec(),
            b"provider-a".to_vec(),
        ])
        .unwrap();
        let changed_boundaries = WorkerV3ExternalProviderPayloadsV1::new(vec![
            b"provider-ap".to_vec(),
            b"rovider-b".to_vec(),
        ])
        .unwrap();
        assert_ne!(first.canonical_sha256(), reordered.canonical_sha256());
        assert_ne!(
            first.canonical_sha256(),
            changed_boundaries.canonical_sha256()
        );
        assert_eq!(first.payload_length(), b"provider-aprovider-b".len());
    }

    #[test]
    fn pinned_cleanup_rejects_same_name_replacement() {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "fe2o3-v3-pinned-cleanup-{}-{id}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let output = PinnedOutput::open_existing(&directory).unwrap();
        let names = IntentNames::new([0x51; 32], [0x52; 32]).unwrap();
        let original = b"original-marker";
        let replacement = b"replacement---x";
        assert_eq!(original.len(), replacement.len());
        let fd = openat(
            &output.fd,
            &names.retiring,
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .unwrap();
        let mut writer = fs::File::from(fd);
        writer.write_all(original).unwrap();
        writer.sync_all().unwrap();
        drop(writer);
        let (file, snapshot) = open_private_file(&output, &names.retiring, original.len()).unwrap();

        let displaced = format!("{}.displaced", names.retiring);
        renameat(&output.fd, &names.retiring, &output.fd, &displaced).unwrap();
        let replacement_fd = openat(
            &output.fd,
            &names.retiring,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .unwrap();
        let mut replacement_file = fs::File::from(replacement_fd);
        replacement_file.write_all(replacement).unwrap();
        replacement_file.sync_all().unwrap();
        drop(replacement_file);

        let mut faults = FaultInjector::new(None);
        assert!(matches!(
            unlink_pinned_private_candidate(
                &output,
                &names,
                &CleanupCandidate {
                    name: PathBuf::from(&names.retiring),
                    snapshot,
                },
                "test-record",
                CleanupBoundariesV1 {
                    quarantine:
                        WorkerV3PublicationIntentBoundaryV1::RenameRetiringRecordToQuarantine,
                    remove: WorkerV3PublicationIntentBoundaryV1::RemoveRetiringRecord,
                },
                &file,
                &mut faults,
            ),
            Err(WorkerV3PublicationIntentErrorV1::InvalidIntent {
                reason: WorkerV3PublicationIntentInvalidReasonV1::FileChangedWhileRead,
                ..
            })
        ));
        assert_eq!(
            fs::read(directory.join(&names.retiring)).unwrap(),
            replacement
        );
        assert_eq!(fs::read(directory.join(displaced)).unwrap(), original);
        drop(file);
        drop(output);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn v1_and_v2_worker_intent_wires_cannot_decode_as_worker_v3() {
        let v1_sized = vec![0; super::super::worker_v2_publication_intent::MAX_WORKER_V2_PUBLICATION_INTENT_RECORD_BYTES];
        let v2_sized = vec![0; super::super::worker_v2_publication_intent::MAX_WORKER_V2_PUBLICATION_INTENT_RECORD_BYTES_V2];
        assert!(matches!(
            WorkerV3PublicationIntentRecordV1::decode_canonical(&v1_sized),
            Err(WorkerV3PublicationIntentCodecErrorV1::NoncanonicalLength { .. })
        ));
        assert!(matches!(
            WorkerV3PublicationIntentRecordV1::decode_canonical(&v2_sized),
            Err(WorkerV3PublicationIntentCodecErrorV1::NoncanonicalLength { .. })
        ));
    }
}
