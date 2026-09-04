//! Receipt-bearing production Worker V3 load envelope.
//!
//! This schema nests the complete canonical V1 replay envelope without reinterpretation and adds
//! the complete compiler-execution receipt carriage. V1 remains a replay codec; production callers
//! can require this top-level schema without maintaining two selectable compilation pipelines.

use core::fmt;
use std::{error::Error, path::Path};

use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerExecutionSubjectErrorV1, CompilerModuleHandoffSlotV3,
    DurableCurrentLinkPublicationLeaseV1, DurablePublishedClaimReacquisitionErrorV3,
    DurablePublishedHsacoClaimV3, InertCompilerExecutionSubjectV1,
    MAX_WORKER_V3_LOAD_ENVELOPE_CUSTODY_BYTES_V2 as MAX_DURABLE_LOAD_ENVELOPE_BYTES,
    MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1, RetainedDurableDirectoryV1,
    VerifiedWorkerV3LoadEnvelopeAuthorityV1, WorkerV3LoadEnvelopeBindingV1,
    WorkerV3LoadReadinessErrorV1, WorkerV3LoadReadinessReceiptV1, WorkerV3LoadReadinessResultV1,
    publish_worker_v3_load_readiness_v1,
    reacquire_current_hsaco_publication_lease_from_retained_directory_v3,
    reacquire_current_hsaco_publication_lease_v3,
    recover_worker_v3_load_readiness_for_attempt_from_retained_directory_v1,
    recover_worker_v3_load_readiness_for_attempt_v1,
};
use fe2o3_compiler_ffi::{
    InertSemanticCompilerModuleHandoffErrorV3, InertSemanticCompilerModuleHandoffV3,
};
use fe2o3_hsaco_finalize::{
    ProtectedWorkerV3CompactFinalizerReplayErrorV1, ProtectedWorkerV3CompactFinalizerReplayV2,
    PublishedProtectedWorkerV3HsacoV1,
};
use sha2::{Digest, Sha256};

use crate::{
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1, CompilerExecutionReceiptCarriageV1,
    CompilerExecutionReceiptPublicationErrorV1, MAX_WORKER_V3_LOAD_ENVELOPE_ALLOCATION_BYTES_V1,
    MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1, WorkerV3LoadEnvelopeCodecBudgetV1,
    WorkerV3LoadEnvelopeErrorV1, WorkerV3LoadEnvelopeV1, WorkerV3LoadEnvelopeWireV1,
};

/// Magic for the receipt-bearing production Worker V3 load envelope.
pub const WORKER_V3_LOAD_ENVELOPE_MAGIC_V2: [u8; 8] = *b"F3LDENV2";
/// Version of the receipt-bearing production Worker V3 load envelope.
pub const WORKER_V3_LOAD_ENVELOPE_VERSION_V2: u16 = 2;

const HEADER_BYTES_V2: usize = 24;
const CHECKSUM_BYTES_V2: usize = 32;
const FIXED_OVERHEAD_BYTES_V2: usize =
    HEADER_BYTES_V2 + COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1 + CHECKSUM_BYTES_V2;
const CHECKSUM_DOMAIN_V2: &[u8] = b"FE2O3/WORKER-V3/LOAD-ENVELOPE-CHECKSUM/V2\0";

/// Maximum complete V2 wire size, including its nested replay and receipt carriage.
///
/// The complete V1 limit remains representable without truncation or a reduced provider budget.
pub const MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2: usize =
    MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1 + FIXED_OVERHEAD_BYTES_V2;
/// Maximum nested canonical replay bytes accepted by one complete V2 envelope.
pub const MAX_WORKER_V3_LOAD_ENVELOPE_REPLAY_BYTES_V2: usize = MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1;
/// Maximum transient allocation across nested decode and canonical byte comparison.
pub const MAX_WORKER_V3_LOAD_ENVELOPE_ALLOCATION_BYTES_V2: usize =
    (2 * MAX_WORKER_V3_LOAD_ENVELOPE_ALLOCATION_BYTES_V1) + FIXED_OVERHEAD_BYTES_V2;
const _: () = assert!(MAX_DURABLE_LOAD_ENVELOPE_BYTES == MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2);

/// Explicit limits for attacker-controlled receipt-bearing envelope wires and allocations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3LoadEnvelopeCodecBudgetV2 {
    max_wire_bytes: usize,
    max_allocation_bytes: usize,
}

impl WorkerV3LoadEnvelopeCodecBudgetV2 {
    pub const fn new(max_wire_bytes: usize, max_allocation_bytes: usize) -> Self {
        Self {
            max_wire_bytes,
            max_allocation_bytes,
        }
    }

    pub const fn production() -> Self {
        Self::new(
            MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2,
            MAX_WORKER_V3_LOAD_ENVELOPE_ALLOCATION_BYTES_V2,
        )
    }

    pub const fn max_wire_bytes(self) -> usize {
        self.max_wire_bytes
    }

    pub const fn max_allocation_bytes(self) -> usize {
        self.max_allocation_bytes
    }
}

impl Default for WorkerV3LoadEnvelopeCodecBudgetV2 {
    fn default() -> Self {
        Self::production()
    }
}

/// Exact association rejected by receipt-bearing envelope validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3LoadEnvelopeBindingFieldV2 {
    ProductionHandoffSlot,
    CompilerExecutionSubject,
    DurablePublishedClaim,
    ExactEnvelopeBytes,
}

/// Construction, canonical codec, persistence, or recovery failure for the V2 envelope.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3LoadEnvelopeErrorV2 {
    Replay(WorkerV3LoadEnvelopeErrorV1),
    Carriage(CompilerExecutionReceiptPublicationErrorV1),
    CompilerExecutionSubject(CompilerExecutionSubjectErrorV1),
    OuterHandoff(InertSemanticCompilerModuleHandoffErrorV3),
    Transcript(ProtectedWorkerV3CompactFinalizerReplayErrorV1),
    LoadReadiness(WorkerV3LoadReadinessErrorV1),
    PublishedClaimReacquisition(DurablePublishedClaimReacquisitionErrorV3),
    WireLengthOutOfRange {
        actual: usize,
        minimum: usize,
        maximum: usize,
    },
    ReplayLengthOutOfRange {
        actual: u64,
        maximum: usize,
    },
    LengthOverflow,
    AllocationBudgetExceeded {
        required: usize,
        maximum: usize,
    },
    AllocationFailed {
        requested: usize,
    },
    Truncated,
    TrailingBytes,
    BadMagic,
    UnsupportedVersion {
        actual: u16,
    },
    UnsupportedFlags {
        actual: u16,
    },
    InvalidTotalLength {
        declared: u64,
        actual: usize,
    },
    ChecksumMismatch,
    NoncanonicalNestedReplay,
    BindingMismatch {
        field: WorkerV3LoadEnvelopeBindingFieldV2,
    },
}

impl fmt::Display for WorkerV3LoadEnvelopeErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Replay(error) => write!(formatter, "invalid nested Worker V3 replay: {error}"),
            Self::Carriage(error) => {
                write!(
                    formatter,
                    "invalid compiler-execution receipt carriage: {error}"
                )
            }
            Self::CompilerExecutionSubject(error) => {
                write!(
                    formatter,
                    "invalid reconstructed compiler-execution subject: {error}"
                )
            }
            Self::OuterHandoff(error) => {
                write!(formatter, "invalid compiler outer handoff: {error}")
            }
            Self::Transcript(error) => write!(formatter, "invalid finalizer transcript: {error}"),
            Self::LoadReadiness(error) => {
                write!(
                    formatter,
                    "failed to recover Worker V3 load readiness: {error}"
                )
            }
            Self::PublishedClaimReacquisition(error) => {
                write!(
                    formatter,
                    "Worker V3 published claim is not current: {error}"
                )
            }
            Self::WireLengthOutOfRange {
                actual,
                minimum,
                maximum,
            } => write!(
                formatter,
                "Worker V3 V2 load envelope is {actual} bytes; expected {minimum}..={maximum}"
            ),
            Self::ReplayLengthOutOfRange { actual, maximum } => write!(
                formatter,
                "nested Worker V3 replay is {actual} bytes; maximum is {maximum}"
            ),
            Self::LengthOverflow => formatter.write_str("Worker V3 V2 envelope length overflows"),
            Self::AllocationBudgetExceeded { required, maximum } => write!(
                formatter,
                "Worker V3 V2 envelope requires {required} allocation bytes; budget is {maximum}"
            ),
            Self::AllocationFailed { requested } => write!(
                formatter,
                "failed to allocate {requested} bytes for Worker V3 V2 envelope"
            ),
            Self::Truncated => formatter.write_str("truncated Worker V3 V2 load envelope"),
            Self::TrailingBytes => {
                formatter.write_str("Worker V3 V2 load envelope has trailing bytes")
            }
            Self::BadMagic => formatter.write_str("Worker V3 V2 load-envelope magic mismatch"),
            Self::UnsupportedVersion { actual } => {
                write!(
                    formatter,
                    "unsupported Worker V3 load-envelope version {actual}"
                )
            }
            Self::UnsupportedFlags { actual } => {
                write!(formatter, "unsupported Worker V3 V2 flags {actual:#06x}")
            }
            Self::InvalidTotalLength { declared, actual } => write!(
                formatter,
                "Worker V3 V2 envelope declares {declared} bytes but contains {actual}"
            ),
            Self::ChecksumMismatch => {
                formatter.write_str("Worker V3 V2 load-envelope checksum mismatch")
            }
            Self::NoncanonicalNestedReplay => {
                formatter.write_str("nested Worker V3 replay is not canonical")
            }
            Self::BindingMismatch { field } => {
                write!(formatter, "Worker V3 V2 binding mismatch: {field:?}")
            }
        }
    }
}

impl Error for WorkerV3LoadEnvelopeErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Replay(error) => Some(error),
            Self::Carriage(error) => Some(error),
            Self::CompilerExecutionSubject(error) => Some(error),
            Self::OuterHandoff(error) => Some(error),
            Self::Transcript(error) => Some(error),
            Self::LoadReadiness(error) => Some(error),
            Self::PublishedClaimReacquisition(error) => Some(error),
            _ => None,
        }
    }
}

impl From<WorkerV3LoadEnvelopeErrorV1> for WorkerV3LoadEnvelopeErrorV2 {
    fn from(error: WorkerV3LoadEnvelopeErrorV1) -> Self {
        Self::Replay(error)
    }
}

/// Inert canonical V2 wire owner containing complete replay and compiler receipt evidence.
pub struct WorkerV3LoadEnvelopeWireV2 {
    replay: WorkerV3LoadEnvelopeWireV1,
    compiler_execution: CompilerExecutionReceiptCarriageV1,
}

impl fmt::Debug for WorkerV3LoadEnvelopeWireV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkerV3LoadEnvelopeWireV2")
            .field(
                "publication_intent",
                &self.replay.publication_intent_record().identity(),
            )
            .field("compiler_execution", &self.compiler_execution.identity())
            .finish_non_exhaustive()
    }
}

impl WorkerV3LoadEnvelopeWireV2 {
    /// Joins one complete replay with the exact receipt for the same compiler occurrence.
    pub fn new(
        replay: WorkerV3LoadEnvelopeWireV1,
        compiler_execution: CompilerExecutionReceiptCarriageV1,
    ) -> Result<Self, WorkerV3LoadEnvelopeErrorV2> {
        validate_compiler_execution_binding(&replay, &compiler_execution)?;
        Ok(Self {
            replay,
            compiler_execution,
        })
    }

    pub const fn replay(&self) -> &WorkerV3LoadEnvelopeWireV1 {
        &self.replay
    }

    pub const fn compiler_execution_receipt(&self) -> &CompilerExecutionReceiptCarriageV1 {
        &self.compiler_execution
    }

    pub const fn published_claim(&self) -> &DurablePublishedHsacoClaimV3 {
        self.replay.published_claim()
    }

    /// Reconstructs the complete authority-free compiler occurrence retained by the replay.
    pub fn reconstructed_compiler_execution_subject_v1(
        &self,
    ) -> Result<InertCompilerExecutionSubjectV1, WorkerV3LoadEnvelopeErrorV2> {
        reconstruct_compiler_execution_subject(&self.replay)
    }

    pub fn encode_canonical(&self) -> Result<Vec<u8>, WorkerV3LoadEnvelopeErrorV2> {
        self.encode_canonical_with_budget(WorkerV3LoadEnvelopeCodecBudgetV2::production())
    }

    pub fn encode_canonical_with_budget(
        &self,
        budget: WorkerV3LoadEnvelopeCodecBudgetV2,
    ) -> Result<Vec<u8>, WorkerV3LoadEnvelopeErrorV2> {
        validate_compiler_execution_binding(&self.replay, &self.compiler_execution)?;
        let replay = self
            .replay
            .encode_canonical_with_budget(nested_budget(budget))?;
        require_replay_length(replay.len() as u64)?;
        let total = canonical_length(replay.len())?;
        require_wire_budget(total, budget)?;
        require_allocation_budget(
            replay
                .len()
                .checked_add(total)
                .ok_or(WorkerV3LoadEnvelopeErrorV2::LengthOverflow)?,
            budget,
        )?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total)
            .map_err(|_| WorkerV3LoadEnvelopeErrorV2::AllocationFailed { requested: total })?;
        bytes.extend_from_slice(&WORKER_V3_LOAD_ENVELOPE_MAGIC_V2);
        bytes.extend_from_slice(&WORKER_V3_LOAD_ENVELOPE_VERSION_V2.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&(total as u64).to_le_bytes());
        let replay_length =
            u32::try_from(replay.len()).map_err(|_| WorkerV3LoadEnvelopeErrorV2::LengthOverflow)?;
        bytes.extend_from_slice(&replay_length.to_le_bytes());
        debug_assert_eq!(bytes.len(), HEADER_BYTES_V2);
        bytes.extend_from_slice(&replay);
        bytes.extend_from_slice(self.compiler_execution.canonical_bytes());
        let checksum = checksum(&bytes);
        bytes.extend_from_slice(&checksum);
        debug_assert_eq!(bytes.len(), total);
        Ok(bytes)
    }

    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, WorkerV3LoadEnvelopeErrorV2> {
        Self::decode_canonical_with_budget(bytes, WorkerV3LoadEnvelopeCodecBudgetV2::production())
    }

    pub fn decode_canonical_with_budget(
        bytes: &[u8],
        budget: WorkerV3LoadEnvelopeCodecBudgetV2,
    ) -> Result<Self, WorkerV3LoadEnvelopeErrorV2> {
        let minimum = FIXED_OVERHEAD_BYTES_V2 + 1;
        let maximum = budget
            .max_wire_bytes()
            .min(MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2);
        if bytes.len() < minimum || bytes.len() > maximum {
            return Err(WorkerV3LoadEnvelopeErrorV2::WireLengthOutOfRange {
                actual: bytes.len(),
                minimum,
                maximum,
            });
        }
        let checksum_offset = bytes
            .len()
            .checked_sub(CHECKSUM_BYTES_V2)
            .ok_or(WorkerV3LoadEnvelopeErrorV2::Truncated)?;
        let (body, declared_checksum) = bytes.split_at(checksum_offset);
        let mut reader = Reader::new(body);
        if reader.array::<8>()? != WORKER_V3_LOAD_ENVELOPE_MAGIC_V2 {
            return Err(WorkerV3LoadEnvelopeErrorV2::BadMagic);
        }
        let version = reader.u16()?;
        if version != WORKER_V3_LOAD_ENVELOPE_VERSION_V2 {
            return Err(WorkerV3LoadEnvelopeErrorV2::UnsupportedVersion { actual: version });
        }
        let flags = reader.u16()?;
        if flags != 0 {
            return Err(WorkerV3LoadEnvelopeErrorV2::UnsupportedFlags { actual: flags });
        }
        let declared_total = reader.u64()?;
        if declared_total != bytes.len() as u64 {
            return Err(WorkerV3LoadEnvelopeErrorV2::InvalidTotalLength {
                declared: declared_total,
                actual: bytes.len(),
            });
        }
        let replay_length = usize::try_from(reader.u32()?)
            .map_err(|_| WorkerV3LoadEnvelopeErrorV2::LengthOverflow)?;
        require_replay_length(replay_length as u64)?;
        require_allocation_budget(
            replay_length
                .checked_mul(2)
                .ok_or(WorkerV3LoadEnvelopeErrorV2::LengthOverflow)?,
            budget,
        )?;
        if canonical_length(replay_length)? != bytes.len() {
            return Err(WorkerV3LoadEnvelopeErrorV2::InvalidTotalLength {
                declared: declared_total,
                actual: bytes.len(),
            });
        }
        let replay = reader.take(replay_length)?;
        let carriage = reader.take(COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1)?;
        if !reader.is_empty() {
            return Err(WorkerV3LoadEnvelopeErrorV2::TrailingBytes);
        }
        if checksum(body).as_slice() != declared_checksum {
            return Err(WorkerV3LoadEnvelopeErrorV2::ChecksumMismatch);
        }
        let nested_budget = nested_budget(budget);
        let decoded_replay =
            WorkerV3LoadEnvelopeWireV1::decode_canonical_with_budget(replay, nested_budget)?;
        let canonical_replay = decoded_replay.encode_canonical_with_budget(nested_budget)?;
        if canonical_replay.as_slice() != replay {
            return Err(WorkerV3LoadEnvelopeErrorV2::NoncanonicalNestedReplay);
        }
        drop(canonical_replay);
        let compiler_execution = CompilerExecutionReceiptCarriageV1::decode(carriage)
            .map_err(WorkerV3LoadEnvelopeErrorV2::Carriage)?;
        Self::new(decoded_replay, compiler_execution)
    }

    pub fn validate_reacquired_publication_lease_v2(
        &self,
        lease: &DurableCurrentLinkPublicationLeaseV1,
    ) -> Result<(), WorkerV3LoadEnvelopeErrorV2> {
        self.replay
            .validate_reacquired_publication_lease_v1(lease)
            .map_err(Into::into)
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn requires_protected_compiler_verification(&self) -> bool {
        true
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Live move-only custody for one receipt-bearing production publication.
pub struct WorkerV3LoadEnvelopeV2 {
    wire: WorkerV3LoadEnvelopeWireV2,
    current_lease: DurableCurrentLinkPublicationLeaseV1,
}

impl fmt::Debug for WorkerV3LoadEnvelopeV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkerV3LoadEnvelopeV2")
            .field("wire", &self.wire)
            .field("current_lease", &self.current_lease)
            .finish()
    }
}

impl WorkerV3LoadEnvelopeV2 {
    /// Consumes one completed publication and its exact compiler-execution receipt.
    pub fn from_published_hsaco_v1(
        published: PublishedProtectedWorkerV3HsacoV1,
        compiler_execution: CompilerExecutionReceiptCarriageV1,
    ) -> Result<Self, WorkerV3LoadEnvelopeErrorV2> {
        let replay = WorkerV3LoadEnvelopeV1::from_published_hsaco_v1(published)?;
        let (replay, current_lease) = replay.into_wire_and_current_lease();
        let wire = WorkerV3LoadEnvelopeWireV2::new(replay, compiler_execution)?;
        Ok(Self {
            wire,
            current_lease,
        })
    }

    pub const fn wire(&self) -> &WorkerV3LoadEnvelopeWireV2 {
        &self.wire
    }

    pub const fn current_publication_lease(&self) -> &DurableCurrentLinkPublicationLeaseV1 {
        &self.current_lease
    }

    pub fn exact_artifact_bytes(&self) -> &[u8] {
        self.current_lease.exact_artifact_bytes()
    }

    pub fn encode_canonical(&self) -> Result<Vec<u8>, WorkerV3LoadEnvelopeErrorV2> {
        self.wire
            .validate_reacquired_publication_lease_v2(&self.current_lease)?;
        self.wire.encode_canonical()
    }

    /// Durably persists the exact receipt-bearing envelope beside the current HSACO publication.
    pub fn persist_durable_replay_custody_v2(
        &self,
        output_dir: &Path,
    ) -> Result<WorkerV3LoadReadinessResultV1, WorkerV3LoadEnvelopeErrorV2> {
        let exact_envelope = self.encode_canonical()?;
        let binding =
            WorkerV3LoadEnvelopeBindingV1::from_exact_bytes(&exact_envelope).map_err(|error| {
                WorkerV3LoadEnvelopeErrorV2::Replay(
                    WorkerV3LoadEnvelopeErrorV1::LoadReadinessCodec(error),
                )
            })?;
        let authority = audited_receipt_bearing_replay_custody_authority_v2(
            binding,
            self.wire.published_claim(),
        )?;
        publish_worker_v3_load_readiness_v1(
            output_dir,
            self.wire.published_claim(),
            authority,
            exact_envelope,
        )
        .map_err(WorkerV3LoadEnvelopeErrorV2::LoadReadiness)
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Restart-recovered move-only custody for one exact V2 envelope and current artifact.
pub struct RecoveredWorkerV3LoadEnvelopeV2 {
    wire: WorkerV3LoadEnvelopeWireV2,
    exact_canonical_envelope: Vec<u8>,
    current_lease: DurableCurrentLinkPublicationLeaseV1,
    receipt: WorkerV3LoadReadinessReceiptV1,
}

impl fmt::Debug for RecoveredWorkerV3LoadEnvelopeV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecoveredWorkerV3LoadEnvelopeV2")
            .field("wire", &self.wire)
            .field(
                "exact_canonical_envelope_length",
                &self.exact_canonical_envelope.len(),
            )
            .field("current_lease", &self.current_lease)
            .field("receipt", &self.receipt)
            .finish()
    }
}

impl RecoveredWorkerV3LoadEnvelopeV2 {
    pub const fn wire(&self) -> &WorkerV3LoadEnvelopeWireV2 {
        &self.wire
    }

    pub const fn current_publication_lease(&self) -> &DurableCurrentLinkPublicationLeaseV1 {
        &self.current_lease
    }

    pub const fn receipt(&self) -> WorkerV3LoadReadinessReceiptV1 {
        self.receipt
    }

    /// Borrows the exact canonical envelope bytes admitted from durable custody.
    ///
    /// This view names the original validated byte string rather than a reserialization of host
    /// projections. It is lifetime-bound to this move-only recovery owner and grants no authority.
    pub fn canonical_evidence_view(&self) -> WorkerV3LoadEnvelopeEvidenceViewV2<'_> {
        WorkerV3LoadEnvelopeEvidenceViewV2 {
            exact_canonical_envelope: &self.exact_canonical_envelope,
            binding: self.receipt.envelope_binding(),
        }
    }

    pub fn exact_artifact_bytes(&self) -> &[u8] {
        self.current_lease.exact_artifact_bytes()
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Lifetime-bound exact canonical bytes from one recovered Worker V3 V2 envelope.
///
/// The view cannot be cloned and cannot outlive its move-only recovery owner. Its byte binding is
/// ordinary inert evidence and does not grant compiler, verification, publication, currentness,
/// load, or launch authority.
pub struct WorkerV3LoadEnvelopeEvidenceViewV2<'evidence> {
    exact_canonical_envelope: &'evidence [u8],
    binding: WorkerV3LoadEnvelopeBindingV1,
}

impl fmt::Debug for WorkerV3LoadEnvelopeEvidenceViewV2<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkerV3LoadEnvelopeEvidenceViewV2")
            .field("binding", &self.binding)
            .field("authority", &"none")
            .finish()
    }
}

impl WorkerV3LoadEnvelopeEvidenceViewV2<'_> {
    /// Returns the exact originally admitted canonical envelope bytes.
    pub const fn exact_canonical_bytes(&self) -> &[u8] {
        self.exact_canonical_envelope
    }

    /// Returns the exact-byte digest and nonzero length validated by durable custody.
    pub const fn binding(&self) -> WorkerV3LoadEnvelopeBindingV1 {
        self.binding
    }

    pub const fn grants_authority(&self) -> bool {
        false
    }

    pub const fn grants_verification_authority(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_currentness_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Recovers only the receipt-bearing production envelope schema from durable readiness custody.
pub fn recover_worker_v3_load_envelope_v2(
    output_dir: &Path,
    attempt: BuildAttempt,
) -> Result<RecoveredWorkerV3LoadEnvelopeV2, WorkerV3LoadEnvelopeErrorV2> {
    let custody = recover_worker_v3_load_readiness_for_attempt_v1(output_dir, attempt)
        .map_err(WorkerV3LoadEnvelopeErrorV2::LoadReadiness)?;
    finish_recover_worker_v3_load_envelope_v2(custody, |claim| {
        reacquire_current_hsaco_publication_lease_v3(output_dir, claim)
    })
}

/// Recovers the exact current V2 envelope and publication without resolving an ambient path.
///
/// The retained root is revalidated and locked by descriptor for readiness recovery and current
/// publication reacquisition. The returned owner keeps a close-on-exec duplicate of that exact
/// root inside its publication lease, so later currentness checks cannot fall back to a path.
pub fn recover_worker_v3_load_envelope_from_retained_directory_v2(
    directory: &RetainedDurableDirectoryV1,
    attempt: BuildAttempt,
) -> Result<RecoveredWorkerV3LoadEnvelopeV2, WorkerV3LoadEnvelopeErrorV2> {
    let custody =
        recover_worker_v3_load_readiness_for_attempt_from_retained_directory_v1(directory, attempt)
            .map_err(WorkerV3LoadEnvelopeErrorV2::LoadReadiness)?;
    finish_recover_worker_v3_load_envelope_v2(custody, |claim| {
        reacquire_current_hsaco_publication_lease_from_retained_directory_v3(directory, claim)
    })
}

fn finish_recover_worker_v3_load_envelope_v2(
    custody: WorkerV3LoadReadinessResultV1,
    reacquire: impl FnOnce(
        &DurablePublishedHsacoClaimV3,
    ) -> Result<
        DurableCurrentLinkPublicationLeaseV1,
        DurablePublishedClaimReacquisitionErrorV3,
    >,
) -> Result<RecoveredWorkerV3LoadEnvelopeV2, WorkerV3LoadEnvelopeErrorV2> {
    let expected_claim = custody.published_claim().clone();
    let receipt = custody.receipt();
    let exact_envelope = custody.into_exact_envelope_bytes();
    let exact_binding =
        WorkerV3LoadEnvelopeBindingV1::from_exact_bytes(&exact_envelope).map_err(|error| {
            WorkerV3LoadEnvelopeErrorV2::Replay(WorkerV3LoadEnvelopeErrorV1::LoadReadinessCodec(
                error,
            ))
        })?;
    if exact_binding != receipt.envelope_binding() {
        return binding_mismatch(WorkerV3LoadEnvelopeBindingFieldV2::ExactEnvelopeBytes);
    }
    let wire = WorkerV3LoadEnvelopeWireV2::decode_canonical(&exact_envelope)?;
    if wire.published_claim() != &expected_claim {
        return binding_mismatch(WorkerV3LoadEnvelopeBindingFieldV2::DurablePublishedClaim);
    }
    let current_lease = reacquire(&expected_claim)
        .map_err(WorkerV3LoadEnvelopeErrorV2::PublishedClaimReacquisition)?;
    wire.validate_reacquired_publication_lease_v2(&current_lease)?;
    Ok(RecoveredWorkerV3LoadEnvelopeV2 {
        wire,
        exact_canonical_envelope: exact_envelope,
        current_lease,
        receipt,
    })
}

fn validate_compiler_execution_binding(
    replay: &WorkerV3LoadEnvelopeWireV1,
    compiler_execution: &CompilerExecutionReceiptCarriageV1,
) -> Result<(), WorkerV3LoadEnvelopeErrorV2> {
    let reconstructed = reconstruct_compiler_execution_subject(replay)?;
    if reconstructed.slot() != CompilerModuleHandoffSlotV3::Production {
        return binding_mismatch(WorkerV3LoadEnvelopeBindingFieldV2::ProductionHandoffSlot);
    }
    if compiler_execution.request().subject() != &reconstructed {
        return binding_mismatch(WorkerV3LoadEnvelopeBindingFieldV2::CompilerExecutionSubject);
    }
    Ok(())
}

fn reconstruct_compiler_execution_subject(
    replay: &WorkerV3LoadEnvelopeWireV1,
) -> Result<InertCompilerExecutionSubjectV1, WorkerV3LoadEnvelopeErrorV2> {
    let handoff = InertSemanticCompilerModuleHandoffV3::decode(replay.outer_handoff())
        .map_err(WorkerV3LoadEnvelopeErrorV2::OuterHandoff)?;
    let transcript =
        ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(replay.transcript())
            .map_err(WorkerV3LoadEnvelopeErrorV2::Transcript)?;
    InertCompilerExecutionSubjectV1::from_replay_evidence(
        replay.publication_intent_record().attempt(),
        transcript.handoff_slot(),
        transcript.transaction_identity(),
        &handoff,
    )
    .map_err(WorkerV3LoadEnvelopeErrorV2::CompilerExecutionSubject)
}

#[allow(
    unsafe_code,
    reason = "one V2 custody bridge follows nested replay, receipt, lease, and exact-wire validation"
)]
fn audited_receipt_bearing_replay_custody_authority_v2(
    binding: WorkerV3LoadEnvelopeBindingV1,
    claim: &DurablePublishedHsacoClaimV3,
) -> Result<VerifiedWorkerV3LoadEnvelopeAuthorityV1, WorkerV3LoadEnvelopeErrorV2> {
    // SAFETY: the sole caller first encodes a live V2 owner. Construction consumed a completed V3
    // publication, the nested V1 codec retained and revalidated every non-artifact replay preimage,
    // the carriage was strictly joined to the reconstructed compiler subject, and the live lease
    // was checked against this exact claim. V2 framing preserves the nested bytes without loss.
    unsafe {
        VerifiedWorkerV3LoadEnvelopeAuthorityV1::from_complete_compact_replay_preimages_unchecked(
            binding, claim,
        )
    }
    .map_err(|error| {
        WorkerV3LoadEnvelopeErrorV2::Replay(WorkerV3LoadEnvelopeErrorV1::PublishedClaim(error))
    })
}

fn canonical_length(replay_length: usize) -> Result<usize, WorkerV3LoadEnvelopeErrorV2> {
    let total = replay_length
        .checked_add(FIXED_OVERHEAD_BYTES_V2)
        .ok_or(WorkerV3LoadEnvelopeErrorV2::LengthOverflow)?;
    if total > MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2 {
        return Err(WorkerV3LoadEnvelopeErrorV2::ReplayLengthOutOfRange {
            actual: replay_length as u64,
            maximum: MAX_WORKER_V3_LOAD_ENVELOPE_REPLAY_BYTES_V2,
        });
    }
    Ok(total)
}

fn require_replay_length(length: u64) -> Result<(), WorkerV3LoadEnvelopeErrorV2> {
    if length == 0 || length > MAX_WORKER_V3_LOAD_ENVELOPE_REPLAY_BYTES_V2 as u64 {
        return Err(WorkerV3LoadEnvelopeErrorV2::ReplayLengthOutOfRange {
            actual: length,
            maximum: MAX_WORKER_V3_LOAD_ENVELOPE_REPLAY_BYTES_V2,
        });
    }
    Ok(())
}

fn nested_budget(budget: WorkerV3LoadEnvelopeCodecBudgetV2) -> WorkerV3LoadEnvelopeCodecBudgetV1 {
    WorkerV3LoadEnvelopeCodecBudgetV1::new(
        budget
            .max_wire_bytes()
            .saturating_sub(FIXED_OVERHEAD_BYTES_V2)
            .min(MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1),
        (budget.max_allocation_bytes() / 2).min(MAX_WORKER_V3_LOAD_ENVELOPE_ALLOCATION_BYTES_V1),
        MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1,
    )
}

fn require_wire_budget(
    actual: usize,
    budget: WorkerV3LoadEnvelopeCodecBudgetV2,
) -> Result<(), WorkerV3LoadEnvelopeErrorV2> {
    let maximum = budget
        .max_wire_bytes()
        .min(MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2);
    if actual > maximum {
        Err(WorkerV3LoadEnvelopeErrorV2::WireLengthOutOfRange {
            actual,
            minimum: FIXED_OVERHEAD_BYTES_V2 + 1,
            maximum,
        })
    } else {
        Ok(())
    }
}

fn require_allocation_budget(
    required: usize,
    budget: WorkerV3LoadEnvelopeCodecBudgetV2,
) -> Result<(), WorkerV3LoadEnvelopeErrorV2> {
    let maximum = budget
        .max_allocation_bytes()
        .min(MAX_WORKER_V3_LOAD_ENVELOPE_ALLOCATION_BYTES_V2);
    if required > maximum {
        Err(WorkerV3LoadEnvelopeErrorV2::AllocationBudgetExceeded { required, maximum })
    } else {
        Ok(())
    }
}

fn binding_mismatch<T>(
    field: WorkerV3LoadEnvelopeBindingFieldV2,
) -> Result<T, WorkerV3LoadEnvelopeErrorV2> {
    Err(WorkerV3LoadEnvelopeErrorV2::BindingMismatch { field })
}

fn checksum(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(CHECKSUM_DOMAIN_V2);
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

    fn take(&mut self, length: usize) -> Result<&'a [u8], WorkerV3LoadEnvelopeErrorV2> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(WorkerV3LoadEnvelopeErrorV2::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(WorkerV3LoadEnvelopeErrorV2::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], WorkerV3LoadEnvelopeErrorV2> {
        self.take(N)?
            .try_into()
            .map_err(|_| WorkerV3LoadEnvelopeErrorV2::Truncated)
    }

    fn u16(&mut self) -> Result<u16, WorkerV3LoadEnvelopeErrorV2> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, WorkerV3LoadEnvelopeErrorV2> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, WorkerV3LoadEnvelopeErrorV2> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    const fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
