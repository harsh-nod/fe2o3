//! Canonical inert wire and live publication custody for a strict Worker V3 load envelope.

use core::{fmt, mem};
use std::error::Error;
use std::path::Path;

use fe2o3_artifact_transaction::{
    BuildAttempt, DurableCurrentLinkPublicationLeaseV1, DurableLinkPublicationPlanV1,
    DurablePublishedClaimCodecErrorV3, DurablePublishedClaimReacquisitionErrorV3,
    DurablePublishedHsacoClaimV3, MAX_COMPILER_MODULE_HANDOFF_BYTES_V3,
    MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V3, MAX_WORKER_V3_PUBLICATION_INTENT_RECORD_BYTES_V1,
    MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
    MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1, VerifiedWorkerV3LoadEnvelopeAuthorityV1,
    WorkerV3LoadEnvelopeBindingV1, WorkerV3LoadReadinessCodecErrorV1, WorkerV3LoadReadinessErrorV1,
    WorkerV3LoadReadinessReceiptV1, WorkerV3LoadReadinessResultV1,
    WorkerV3PublicationIntentCodecErrorV1, WorkerV3PublicationIntentRecordV1,
    publish_worker_v3_load_readiness_v1, reacquire_current_hsaco_publication_lease_v3,
    recover_worker_v3_load_readiness_for_attempt_v1,
};
use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3;
use fe2o3_hsaco_finalize::{
    ProtectedWorkerV3CompactFinalizerReplayErrorV1, ProtectedWorkerV3CompactFinalizerReplayV2,
    PublishedProtectedWorkerV3HsacoV1, WorkerV3HsacoPublicationErrorV1,
};
use sha2::{Digest, Sha256};

/// Magic for the strict production Worker V3 load-envelope V1 wire.
pub const WORKER_V3_LOAD_ENVELOPE_MAGIC_V1: [u8; 8] = *b"F3LDENV1";
/// The only Worker V3 load-envelope schema accepted by this module.
pub const WORKER_V3_LOAD_ENVELOPE_VERSION_V1: u16 = 1;

const HEADER_BYTES_V1: usize = 60;
const CHECKSUM_BYTES_V1: usize = 32;
const PROVIDER_LENGTH_BYTES_V1: usize = 8;
const LOAD_ENVELOPE_CHECKSUM_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V3/LOAD-ENVELOPE-CHECKSUM/V1\0";
const PROVIDER_ARCHIVE_MAGIC_V1: &[u8] = b"FE2O3-WORKER-V3-PROVIDER-PAYLOADS-V1\0";
const PROVIDER_ARCHIVE_VERSION_V1: u16 = 1;
const PROVIDER_ARCHIVE_CHECKSUM_DOMAIN_V1: &[u8] =
    b"fe2o3.worker-v3-publication-intent.provider-archive-checksum.v1\0";
const PROVIDER_ARCHIVE_FIXED_BYTES_V1: usize = PROVIDER_ARCHIVE_MAGIC_V1.len() + 2 + 4 + 8 + 32;
const PROVIDER_ARCHIVE_ENTRY_BYTES_V1: usize = 8 + 32;

/// Maximum canonical bytes in one strict Worker V3 load envelope.
///
/// Finalized HSACO bytes are deliberately absent. They remain pinned only by a live publication
/// lease and must be reacquired independently after decoding.
pub const MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1: usize = 256 * 1024 * 1024;

/// Default maximum bytes allocated directly by one envelope encode or decode.
pub const MAX_WORKER_V3_LOAD_ENVELOPE_ALLOCATION_BYTES_V1: usize =
    MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1
        + MAX_WORKER_V3_PUBLICATION_INTENT_RECORD_BYTES_V1
        + MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V3;

/// Explicit limits for attacker-controlled V3 envelope wires and retained allocations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3LoadEnvelopeCodecBudgetV1 {
    max_wire_bytes: usize,
    max_allocation_bytes: usize,
    max_provider_count: usize,
}

impl WorkerV3LoadEnvelopeCodecBudgetV1 {
    pub const fn new(
        max_wire_bytes: usize,
        max_allocation_bytes: usize,
        max_provider_count: usize,
    ) -> Self {
        Self {
            max_wire_bytes,
            max_allocation_bytes,
            max_provider_count,
        }
    }

    pub const fn production() -> Self {
        Self::new(
            MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1,
            MAX_WORKER_V3_LOAD_ENVELOPE_ALLOCATION_BYTES_V1,
            MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1,
        )
    }

    pub const fn max_wire_bytes(self) -> usize {
        self.max_wire_bytes
    }

    pub const fn max_allocation_bytes(self) -> usize {
        self.max_allocation_bytes
    }

    pub const fn max_provider_count(self) -> usize {
        self.max_provider_count
    }
}

impl Default for WorkerV3LoadEnvelopeCodecBudgetV1 {
    fn default() -> Self {
        Self::production()
    }
}

/// Cross-component association rejected by strict V3 envelope validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3LoadEnvelopeBindingFieldV1 {
    RecordAttempt,
    DurablePlan,
    PublicationIntentRecord,
    FinalizedOutputHash,
    FinalizedOutputLength,
    OuterHandoffHash,
    OuterHandoffLength,
    ExternalProviderArchiveHash,
    ExternalProviderArchiveLength,
    ExternalProviderCount,
    ExternalProviderPayloadLength,
    TranscriptHash,
    TranscriptLength,
    TranscriptFinalization,
    TranscriptSource,
    CompilerClosure,
    LeasePublication,
    LeaseArtifact,
}

/// Strict construction, encoding, or decoding failure for a Worker V3 load envelope.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3LoadEnvelopeErrorV1 {
    Publication(WorkerV3HsacoPublicationErrorV1),
    IntentRecord(WorkerV3PublicationIntentCodecErrorV1),
    PublishedClaim(DurablePublishedClaimCodecErrorV3),
    PublishedClaimReacquisition(DurablePublishedClaimReacquisitionErrorV3),
    Transcript(ProtectedWorkerV3CompactFinalizerReplayErrorV1),
    LoadReadinessCodec(WorkerV3LoadReadinessCodecErrorV1),
    LoadReadiness(WorkerV3LoadReadinessErrorV1),
    WireTooLarge {
        actual: usize,
        max: usize,
    },
    AllocationBudgetExceeded {
        required: usize,
        max: usize,
    },
    AllocationFailed {
        field: &'static str,
        requested: usize,
    },
    LengthOverflow {
        field: &'static str,
    },
    LengthOutOfRange {
        field: &'static str,
        actual: u64,
        max: usize,
    },
    CountOutOfRange {
        actual: u64,
        max: usize,
    },
    Truncated,
    TrailingBytes,
    BadMagic,
    UnsupportedVersion {
        actual: u16,
    },
    NonZeroReserved,
    InvalidTotalLength {
        declared: u64,
        actual: usize,
    },
    ChecksumMismatch,
    NonCanonicalOuterHandoff {
        field: &'static str,
    },
    BindingMismatch {
        field: WorkerV3LoadEnvelopeBindingFieldV1,
    },
}

impl fmt::Display for WorkerV3LoadEnvelopeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Publication(error) => {
                write!(formatter, "V3 publication transfer failed: {error}")
            }
            Self::IntentRecord(error) => write!(formatter, "invalid V3 intent record: {error}"),
            Self::PublishedClaim(error) => write!(formatter, "invalid V3 published claim: {error}"),
            Self::PublishedClaimReacquisition(error) => {
                write!(formatter, "V3 published claim is not current: {error}")
            }
            Self::Transcript(error) => write!(formatter, "invalid compact V2 transcript: {error}"),
            Self::LoadReadinessCodec(error) => {
                write!(
                    formatter,
                    "invalid V3 load-envelope custody binding: {error}"
                )
            }
            Self::LoadReadiness(error) => {
                write!(
                    formatter,
                    "failed to persist V3 load-envelope custody: {error}"
                )
            }
            Self::WireTooLarge { actual, max } => {
                write!(
                    formatter,
                    "Worker V3 load envelope is {actual} bytes; maximum is {max}"
                )
            }
            Self::AllocationBudgetExceeded { required, max } => write!(
                formatter,
                "Worker V3 load envelope requires {required} allocation bytes; budget is {max}"
            ),
            Self::AllocationFailed { field, requested } => {
                write!(
                    formatter,
                    "failed to allocate {requested} bytes for {field}"
                )
            }
            Self::LengthOverflow { field } => write!(formatter, "{field} length overflows"),
            Self::LengthOutOfRange { field, actual, max } => {
                write!(formatter, "{field} length {actual} exceeds {max}")
            }
            Self::CountOutOfRange { actual, max } => {
                write!(formatter, "provider count {actual} exceeds {max}")
            }
            Self::Truncated => formatter.write_str("truncated Worker V3 load envelope"),
            Self::TrailingBytes => {
                formatter.write_str("Worker V3 load envelope has trailing bytes")
            }
            Self::BadMagic => formatter.write_str("Worker V3 load-envelope magic mismatch"),
            Self::UnsupportedVersion { actual } => {
                write!(
                    formatter,
                    "unsupported Worker V3 load-envelope version {actual}"
                )
            }
            Self::NonZeroReserved => {
                formatter.write_str("Worker V3 load-envelope reserved field is nonzero")
            }
            Self::InvalidTotalLength { declared, actual } => write!(
                formatter,
                "Worker V3 load-envelope declared length {declared} does not match {actual} bytes"
            ),
            Self::ChecksumMismatch => {
                formatter.write_str("Worker V3 load-envelope checksum mismatch")
            }
            Self::NonCanonicalOuterHandoff { field } => {
                write!(formatter, "noncanonical V3 outer semantic handoff: {field}")
            }
            Self::BindingMismatch { field } => {
                write!(
                    formatter,
                    "Worker V3 load-envelope binding mismatch: {field:?}"
                )
            }
        }
    }
}

impl Error for WorkerV3LoadEnvelopeErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Publication(error) => Some(error),
            Self::IntentRecord(error) => Some(error),
            Self::PublishedClaim(error) => Some(error),
            Self::PublishedClaimReacquisition(error) => Some(error),
            Self::Transcript(error) => Some(error),
            Self::LoadReadinessCodec(error) => Some(error),
            Self::LoadReadiness(error) => Some(error),
            _ => None,
        }
    }
}

impl From<WorkerV3HsacoPublicationErrorV1> for WorkerV3LoadEnvelopeErrorV1 {
    fn from(value: WorkerV3HsacoPublicationErrorV1) -> Self {
        Self::Publication(value)
    }
}

impl From<WorkerV3LoadReadinessCodecErrorV1> for WorkerV3LoadEnvelopeErrorV1 {
    fn from(value: WorkerV3LoadReadinessCodecErrorV1) -> Self {
        Self::LoadReadinessCodec(value)
    }
}

impl From<WorkerV3LoadReadinessErrorV1> for WorkerV3LoadEnvelopeErrorV1 {
    fn from(value: WorkerV3LoadReadinessErrorV1) -> Self {
        Self::LoadReadiness(value)
    }
}

/// Live, move-only custody for one completed strict-V3 publication and its envelope evidence.
///
/// Construction consumes [`PublishedProtectedWorkerV3HsacoV1`]. It validates and then drops the
/// replay-owned duplicate finalized HSACO, retaining only the exact current-publication lease.
/// This type cannot be cloned and cannot be reconstructed from serialized evidence.
///
/// ```compile_fail
/// use fe2o3_runtime_protocol::WorkerV3LoadEnvelopeV1;
/// fn duplicate(value: WorkerV3LoadEnvelopeV1) {
///     let _copy = value.clone();
/// }
/// ```
pub struct WorkerV3LoadEnvelopeV1 {
    wire: WorkerV3LoadEnvelopeWireV1,
    current_lease: DurableCurrentLinkPublicationLeaseV1,
}

impl fmt::Debug for WorkerV3LoadEnvelopeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkerV3LoadEnvelopeV1")
            .field("wire", &self.wire)
            .field("current_lease", &self.current_lease)
            .finish()
    }
}

impl WorkerV3LoadEnvelopeV1 {
    /// Consumes one completed V3 publication into live load-envelope custody.
    pub fn new(
        published: PublishedProtectedWorkerV3HsacoV1,
    ) -> Result<Self, WorkerV3LoadEnvelopeErrorV1> {
        Self::from_published_hsaco_v1(published)
    }

    /// Named constructor for the only live V3 publication-to-envelope boundary.
    pub fn from_published_hsaco_v1(
        published: PublishedProtectedWorkerV3HsacoV1,
    ) -> Result<Self, WorkerV3LoadEnvelopeErrorV1> {
        let (replay, record, claim, current_lease) =
            published.into_load_envelope_parts_v1()?.into_parts();
        let fe2o3_hsaco_finalize::ProtectedWorkerV3CompactFinalizerReplayPartsV2 {
            outer_handoff,
            external_provider_payloads,
            transcript,
            finalized_hsaco,
        } = replay;

        validate_finalized_artifact(record, &claim, &current_lease, Some(&finalized_hsaco))?;
        drop(finalized_hsaco);

        let wire = WorkerV3LoadEnvelopeWireV1::from_parts(
            record,
            claim,
            outer_handoff,
            external_provider_payloads,
            transcript,
        )?;
        Ok(Self {
            wire,
            current_lease,
        })
    }

    pub const fn wire(&self) -> &WorkerV3LoadEnvelopeWireV1 {
        &self.wire
    }

    pub const fn current_publication_lease(&self) -> &DurableCurrentLinkPublicationLeaseV1 {
        &self.current_lease
    }

    pub fn exact_artifact_bytes(&self) -> &[u8] {
        self.current_lease.exact_artifact_bytes()
    }

    pub fn encode_canonical(&self) -> Result<Vec<u8>, WorkerV3LoadEnvelopeErrorV1> {
        validate_finalized_artifact(
            self.wire.record,
            &self.wire.claim,
            &self.current_lease,
            None,
        )?;
        self.wire.encode_canonical()
    }

    pub fn encode_canonical_with_budget(
        &self,
        budget: WorkerV3LoadEnvelopeCodecBudgetV1,
    ) -> Result<Vec<u8>, WorkerV3LoadEnvelopeErrorV1> {
        validate_finalized_artifact(
            self.wire.record,
            &self.wire.claim,
            &self.current_lease,
            None,
        )?;
        self.wire.encode_canonical_with_budget(budget)
    }

    /// Persists the exact canonical envelope beside its current V3 publication.
    ///
    /// The resulting receipt proves only that the envelope replay components and the separately
    /// published finalized artifact are durably reconstructible. It authenticates no descriptor
    /// source, performs no semantic admission, and grants no HSA load or launch authority.
    pub fn persist_durable_replay_custody_v1(
        &self,
        output_dir: &Path,
    ) -> Result<WorkerV3LoadReadinessResultV1, WorkerV3LoadEnvelopeErrorV1> {
        let exact_envelope = self.encode_canonical()?;
        let binding = WorkerV3LoadEnvelopeBindingV1::from_exact_bytes(&exact_envelope)?;
        let authority = audited_replay_custody_authority_v1(binding, &self.wire.claim)?;
        publish_worker_v3_load_readiness_v1(output_dir, &self.wire.claim, authority, exact_envelope)
            .map_err(Into::into)
    }

    pub fn into_wire_and_current_lease(
        self,
    ) -> (
        WorkerV3LoadEnvelopeWireV1,
        DurableCurrentLinkPublicationLeaseV1,
    ) {
        (self.wire, self.current_lease)
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[allow(
    unsafe_code,
    reason = "one audited custody bridge follows exact live-envelope and publication validation"
)]
fn audited_replay_custody_authority_v1(
    binding: WorkerV3LoadEnvelopeBindingV1,
    claim: &DurablePublishedHsacoClaimV3,
) -> Result<VerifiedWorkerV3LoadEnvelopeAuthorityV1, WorkerV3LoadEnvelopeErrorV1> {
    // SAFETY: only `WorkerV3LoadEnvelopeV1::persist_durable_replay_custody_v1` calls this helper.
    // The live owner can only be constructed by consuming a completed V3 publication; construction
    // validates the exact intent record, claim, current lease, outer handoff, ordered providers,
    // compact transcript, and finalized artifact. Its encoder revalidates those associations. The
    // canonical envelope therefore retains every non-artifact replay component while the claim and
    // current publication retain the exact finalized artifact checked by the persistence layer.
    unsafe {
        VerifiedWorkerV3LoadEnvelopeAuthorityV1::from_complete_compact_replay_preimages_unchecked(
            binding, claim,
        )
    }
    .map_err(WorkerV3LoadEnvelopeErrorV1::PublishedClaim)
}

/// Separately decoded inert V1 wire owner.
///
/// This value contains no finalized-HSACO bytes and no publication lease. A host must independently
/// call `reacquire_current_hsaco_publication_lease_v3` with [`Self::published_claim`] and then
/// validate that lease before any later admission boundary.
pub struct WorkerV3LoadEnvelopeWireV1 {
    record: WorkerV3PublicationIntentRecordV1,
    claim: DurablePublishedHsacoClaimV3,
    outer_handoff: Vec<u8>,
    external_provider_payloads: Vec<Vec<u8>>,
    transcript: Vec<u8>,
}

impl fmt::Debug for WorkerV3LoadEnvelopeWireV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let provider_bytes = self
            .external_provider_payloads
            .iter()
            .fold(0_usize, |total, payload| {
                total.saturating_add(payload.len())
            });
        formatter
            .debug_struct("WorkerV3LoadEnvelopeWireV1")
            .field("record_identity", &self.record.identity())
            .field("outer_handoff_bytes", &self.outer_handoff.len())
            .field(
                "external_provider_count",
                &self.external_provider_payloads.len(),
            )
            .field("external_provider_bytes", &provider_bytes)
            .field("transcript_bytes", &self.transcript.len())
            .finish_non_exhaustive()
    }
}

impl WorkerV3LoadEnvelopeWireV1 {
    fn from_parts(
        record: WorkerV3PublicationIntentRecordV1,
        claim: DurablePublishedHsacoClaimV3,
        outer_handoff: Vec<u8>,
        external_provider_payloads: Vec<Vec<u8>>,
        transcript: Vec<u8>,
    ) -> Result<Self, WorkerV3LoadEnvelopeErrorV1> {
        let external_provider_payloads = validate_components(
            record,
            &claim,
            &outer_handoff,
            external_provider_payloads,
            &transcript,
        )?;
        Ok(Self {
            record,
            claim,
            outer_handoff,
            external_provider_payloads,
            transcript,
        })
    }

    pub const fn publication_intent_record(&self) -> WorkerV3PublicationIntentRecordV1 {
        self.record
    }

    pub const fn published_claim(&self) -> &DurablePublishedHsacoClaimV3 {
        &self.claim
    }

    pub fn outer_handoff(&self) -> &[u8] {
        &self.outer_handoff
    }

    pub fn external_provider_payloads(&self) -> impl ExactSizeIterator<Item = &[u8]> {
        self.external_provider_payloads.iter().map(Vec::as_slice)
    }

    pub fn transcript(&self) -> &[u8] {
        &self.transcript
    }

    pub fn encode_canonical(&self) -> Result<Vec<u8>, WorkerV3LoadEnvelopeErrorV1> {
        self.encode_canonical_with_budget(WorkerV3LoadEnvelopeCodecBudgetV1::production())
    }

    pub fn encode_canonical_with_budget(
        &self,
        budget: WorkerV3LoadEnvelopeCodecBudgetV1,
    ) -> Result<Vec<u8>, WorkerV3LoadEnvelopeErrorV1> {
        validate_components_borrowed(
            self.record,
            &self.claim,
            &self.outer_handoff,
            &self.external_provider_payloads,
            &self.transcript,
        )?;
        let record = self
            .record
            .encode_canonical()
            .map_err(WorkerV3LoadEnvelopeErrorV1::IntentRecord)?;
        let claim = self
            .claim
            .encode_canonical()
            .map_err(WorkerV3LoadEnvelopeErrorV1::PublishedClaim)?;
        let provider_payload_bytes = provider_payload_length(&self.external_provider_payloads)?;
        let total_len = canonical_wire_length(
            record.len(),
            claim.len(),
            self.outer_handoff.len(),
            self.external_provider_payloads.len(),
            provider_payload_bytes,
            self.transcript.len(),
        )?;
        require_wire_bound(total_len, budget)?;
        let live_allocation = total_len
            .checked_add(record.len())
            .and_then(|value| value.checked_add(claim.len()))
            .ok_or(WorkerV3LoadEnvelopeErrorV1::LengthOverflow {
                field: "envelope encoding allocation",
            })?;
        require_allocation_budget(live_allocation, budget)?;

        let mut bytes = try_vec_bytes(total_len, "canonical V3 load envelope")?;
        bytes.extend_from_slice(&WORKER_V3_LOAD_ENVELOPE_MAGIC_V1);
        bytes.extend_from_slice(&WORKER_V3_LOAD_ENVELOPE_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&as_u64(total_len, "envelope")?.to_le_bytes());
        bytes.extend_from_slice(&as_u32(record.len(), "intent record")?.to_le_bytes());
        bytes.extend_from_slice(&as_u32(claim.len(), "published claim")?.to_le_bytes());
        bytes.extend_from_slice(&as_u64(self.outer_handoff.len(), "outer handoff")?.to_le_bytes());
        bytes.extend_from_slice(
            &as_u32(self.external_provider_payloads.len(), "provider count")?.to_le_bytes(),
        );
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes
            .extend_from_slice(&as_u64(provider_payload_bytes, "provider payloads")?.to_le_bytes());
        bytes.extend_from_slice(&as_u64(self.transcript.len(), "transcript")?.to_le_bytes());
        debug_assert_eq!(bytes.len(), HEADER_BYTES_V1);

        bytes.extend_from_slice(&record);
        bytes.extend_from_slice(&claim);
        bytes.extend_from_slice(&self.outer_handoff);
        for payload in &self.external_provider_payloads {
            bytes.extend_from_slice(&as_u64(payload.len(), "provider payload")?.to_le_bytes());
            bytes.extend_from_slice(payload);
        }
        bytes.extend_from_slice(&self.transcript);
        let checksum = checksum(&bytes);
        bytes.extend_from_slice(&checksum);
        debug_assert_eq!(bytes.len(), total_len);
        Ok(bytes)
    }

    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, WorkerV3LoadEnvelopeErrorV1> {
        Self::decode_canonical_with_budget(bytes, WorkerV3LoadEnvelopeCodecBudgetV1::production())
    }

    pub fn decode_canonical_with_budget(
        bytes: &[u8],
        budget: WorkerV3LoadEnvelopeCodecBudgetV1,
    ) -> Result<Self, WorkerV3LoadEnvelopeErrorV1> {
        let sections = decode_sections(bytes, budget)?;
        let record = WorkerV3PublicationIntentRecordV1::decode_canonical(sections.record)
            .map_err(WorkerV3LoadEnvelopeErrorV1::IntentRecord)?;
        let claim = DurablePublishedHsacoClaimV3::decode_canonical(sections.claim)
            .map_err(WorkerV3LoadEnvelopeErrorV1::PublishedClaim)?;

        let retained_bytes = sections
            .outer_handoff
            .len()
            .checked_add(sections.transcript.len())
            .and_then(|value| value.checked_add(sections.provider_payload_bytes))
            .and_then(|value| {
                value.checked_add(
                    sections
                        .providers
                        .len()
                        .checked_mul(mem::size_of::<Vec<u8>>() + mem::size_of::<&[u8]>())?,
                )
            })
            .ok_or(WorkerV3LoadEnvelopeErrorV1::LengthOverflow {
                field: "decoded envelope allocation",
            })?;
        require_allocation_budget(retained_bytes, budget)?;

        validate_record_claim(record, &claim)?;
        validate_component_hash_length(
            sections.outer_handoff,
            record.outer_handoff_sha256(),
            record.outer_handoff_length(),
            WorkerV3LoadEnvelopeBindingFieldV1::OuterHandoffHash,
            WorkerV3LoadEnvelopeBindingFieldV1::OuterHandoffLength,
        )?;
        validate_provider_slices(record, &sections.providers)?;
        validate_component_hash_length(
            sections.transcript,
            record.transcript_sha256(),
            record.transcript_length(),
            WorkerV3LoadEnvelopeBindingFieldV1::TranscriptHash,
            WorkerV3LoadEnvelopeBindingFieldV1::TranscriptLength,
        )?;
        let outer_compiler_closure = validate_outer_handoff(sections.outer_handoff)?;
        let transcript =
            ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(sections.transcript)
                .map_err(WorkerV3LoadEnvelopeErrorV1::Transcript)?;
        validate_transcript_binding(&transcript, &claim)?;
        if outer_compiler_closure != claim.compiler_closure() {
            return binding_mismatch(WorkerV3LoadEnvelopeBindingFieldV1::CompilerClosure);
        }

        let mut allocation = AllocationTracker::new(budget);
        allocation.account(
            sections
                .providers
                .len()
                .checked_mul(mem::size_of::<Vec<u8>>())
                .ok_or(WorkerV3LoadEnvelopeErrorV1::LengthOverflow {
                    field: "provider owner list",
                })?,
        )?;
        let outer_handoff = allocation.copy(sections.outer_handoff, "outer semantic handoff")?;
        let mut external_provider_payloads = Vec::new();
        external_provider_payloads
            .try_reserve_exact(sections.providers.len())
            .map_err(|_| WorkerV3LoadEnvelopeErrorV1::AllocationFailed {
                field: "provider owner list",
                requested: sections.providers.len(),
            })?;
        for payload in sections.providers {
            external_provider_payloads.push(allocation.copy(payload, "provider payload")?);
        }
        allocation.account(sections.transcript.len())?;
        let transcript_bytes = transcript.into_canonical_bytes();

        Ok(Self {
            record,
            claim,
            outer_handoff,
            external_provider_payloads,
            transcript: transcript_bytes,
        })
    }

    /// Checks a separately reacquired V3 lease against this inert wire.
    ///
    /// This does not retain the lease, acquire a currentness token, or grant load authority.
    pub fn validate_reacquired_publication_lease_v1(
        &self,
        lease: &DurableCurrentLinkPublicationLeaseV1,
    ) -> Result<(), WorkerV3LoadEnvelopeErrorV1> {
        validate_finalized_artifact(self.record, &self.claim, lease, None)
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
}

/// Restart-recovered live custody for one exact V3 envelope and current finalized artifact.
///
/// This owner is intentionally move-only. It proves durable replay custody and current artifact
/// identity, but authenticates no descriptor source and grants no HSA load or launch authority.
pub struct RecoveredWorkerV3LoadEnvelopeV1 {
    wire: WorkerV3LoadEnvelopeWireV1,
    current_lease: DurableCurrentLinkPublicationLeaseV1,
    receipt: WorkerV3LoadReadinessReceiptV1,
}

impl fmt::Debug for RecoveredWorkerV3LoadEnvelopeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecoveredWorkerV3LoadEnvelopeV1")
            .field("wire", &self.wire)
            .field("current_lease", &self.current_lease)
            .field("receipt", &self.receipt)
            .finish()
    }
}

impl RecoveredWorkerV3LoadEnvelopeV1 {
    pub const fn wire(&self) -> &WorkerV3LoadEnvelopeWireV1 {
        &self.wire
    }

    pub const fn current_publication_lease(&self) -> &DurableCurrentLinkPublicationLeaseV1 {
        &self.current_lease
    }

    pub const fn receipt(&self) -> WorkerV3LoadReadinessReceiptV1 {
        self.receipt
    }

    pub fn exact_artifact_bytes(&self) -> &[u8] {
        self.current_lease.exact_artifact_bytes()
    }

    pub const fn authenticates_descriptor_source(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Recovers one strict V3 envelope and its exact current publication using durable state alone.
pub fn recover_worker_v3_load_envelope_v1(
    output_dir: &Path,
    attempt: BuildAttempt,
) -> Result<RecoveredWorkerV3LoadEnvelopeV1, WorkerV3LoadEnvelopeErrorV1> {
    let custody = recover_worker_v3_load_readiness_for_attempt_v1(output_dir, attempt)?;
    let expected_claim = custody.published_claim().clone();
    let receipt = custody.receipt();
    let exact_envelope = custody.into_exact_envelope_bytes();
    let wire = WorkerV3LoadEnvelopeWireV1::decode_canonical(&exact_envelope)?;
    drop(exact_envelope);
    if wire.published_claim() != &expected_claim {
        return binding_mismatch(WorkerV3LoadEnvelopeBindingFieldV1::DurablePlan);
    }
    let current_lease = reacquire_current_hsaco_publication_lease_v3(output_dir, &expected_claim)
        .map_err(WorkerV3LoadEnvelopeErrorV1::PublishedClaimReacquisition)?;
    wire.validate_reacquired_publication_lease_v1(&current_lease)?;
    Ok(RecoveredWorkerV3LoadEnvelopeV1 {
        wire,
        current_lease,
        receipt,
    })
}

fn validate_components(
    record: WorkerV3PublicationIntentRecordV1,
    claim: &DurablePublishedHsacoClaimV3,
    outer_handoff: &[u8],
    external_provider_payloads: Vec<Vec<u8>>,
    transcript_bytes: &[u8],
) -> Result<Vec<Vec<u8>>, WorkerV3LoadEnvelopeErrorV1> {
    validate_record_claim(record, claim)?;
    validate_component_hash_length(
        outer_handoff,
        record.outer_handoff_sha256(),
        record.outer_handoff_length(),
        WorkerV3LoadEnvelopeBindingFieldV1::OuterHandoffHash,
        WorkerV3LoadEnvelopeBindingFieldV1::OuterHandoffLength,
    )?;
    validate_provider_slices(record, &external_provider_payloads)?;
    validate_component_hash_length(
        transcript_bytes,
        record.transcript_sha256(),
        record.transcript_length(),
        WorkerV3LoadEnvelopeBindingFieldV1::TranscriptHash,
        WorkerV3LoadEnvelopeBindingFieldV1::TranscriptLength,
    )?;
    let outer_compiler_closure = validate_outer_handoff(outer_handoff)?;
    let transcript = ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(transcript_bytes)
        .map_err(WorkerV3LoadEnvelopeErrorV1::Transcript)?;
    validate_transcript_binding(&transcript, claim)?;
    if outer_compiler_closure != claim.compiler_closure() {
        return binding_mismatch(WorkerV3LoadEnvelopeBindingFieldV1::CompilerClosure);
    }
    Ok(external_provider_payloads)
}

fn validate_components_borrowed(
    record: WorkerV3PublicationIntentRecordV1,
    claim: &DurablePublishedHsacoClaimV3,
    outer_handoff: &[u8],
    external_provider_payloads: &[Vec<u8>],
    transcript_bytes: &[u8],
) -> Result<(), WorkerV3LoadEnvelopeErrorV1> {
    validate_record_claim(record, claim)?;
    validate_component_hash_length(
        outer_handoff,
        record.outer_handoff_sha256(),
        record.outer_handoff_length(),
        WorkerV3LoadEnvelopeBindingFieldV1::OuterHandoffHash,
        WorkerV3LoadEnvelopeBindingFieldV1::OuterHandoffLength,
    )?;
    validate_provider_slices(record, external_provider_payloads)?;
    validate_component_hash_length(
        transcript_bytes,
        record.transcript_sha256(),
        record.transcript_length(),
        WorkerV3LoadEnvelopeBindingFieldV1::TranscriptHash,
        WorkerV3LoadEnvelopeBindingFieldV1::TranscriptLength,
    )?;
    let outer_compiler_closure = validate_outer_handoff(outer_handoff)?;
    let transcript = ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(transcript_bytes)
        .map_err(WorkerV3LoadEnvelopeErrorV1::Transcript)?;
    validate_transcript_binding(&transcript, claim)?;
    if outer_compiler_closure != claim.compiler_closure() {
        return binding_mismatch(WorkerV3LoadEnvelopeBindingFieldV1::CompilerClosure);
    }
    Ok(())
}

fn validate_record_claim(
    record: WorkerV3PublicationIntentRecordV1,
    claim: &DurablePublishedHsacoClaimV3,
) -> Result<(), WorkerV3LoadEnvelopeErrorV1> {
    let plan = record.plan();
    if record.attempt() != plan.attempt() {
        return binding_mismatch(WorkerV3LoadEnvelopeBindingFieldV1::RecordAttempt);
    }
    if plan != claim.plan() {
        return binding_mismatch(WorkerV3LoadEnvelopeBindingFieldV1::DurablePlan);
    }
    let binding = claim.worker_v3_binding();
    if binding.publication_intent_record_identity() != record.identity().as_bytes() {
        return binding_mismatch(WorkerV3LoadEnvelopeBindingFieldV1::PublicationIntentRecord);
    }
    if binding.finalized_output_sha256() != record.output_sha256()
        || record.output_sha256() != *plan.finalized_output().as_bytes()
    {
        return binding_mismatch(WorkerV3LoadEnvelopeBindingFieldV1::FinalizedOutputHash);
    }
    if usize::try_from(binding.finalized_output_length()).ok() != Some(record.output_length()) {
        return binding_mismatch(WorkerV3LoadEnvelopeBindingFieldV1::FinalizedOutputLength);
    }
    Ok(())
}

fn validate_transcript_binding(
    transcript: &ProtectedWorkerV3CompactFinalizerReplayV2,
    claim: &DurablePublishedHsacoClaimV3,
) -> Result<(), WorkerV3LoadEnvelopeErrorV1> {
    let binding = claim.worker_v3_binding();
    if transcript.expected_finalization_identity() != &binding.finalization_identity() {
        return binding_mismatch(WorkerV3LoadEnvelopeBindingFieldV1::TranscriptFinalization);
    }
    if transcript.source_evidence_identity() != &binding.source_evidence_identity() {
        return binding_mismatch(WorkerV3LoadEnvelopeBindingFieldV1::TranscriptSource);
    }
    Ok(())
}

fn validate_provider_slices<T: AsRef<[u8]>>(
    record: WorkerV3PublicationIntentRecordV1,
    payloads: &[T],
) -> Result<(), WorkerV3LoadEnvelopeErrorV1> {
    let (count, payload_length, canonical_length, canonical_sha256) =
        provider_archive_bindings(payloads)?;
    for (matches, field) in [
        (
            canonical_sha256 == record.external_provider_archive_sha256(),
            WorkerV3LoadEnvelopeBindingFieldV1::ExternalProviderArchiveHash,
        ),
        (
            canonical_length == record.external_provider_archive_length(),
            WorkerV3LoadEnvelopeBindingFieldV1::ExternalProviderArchiveLength,
        ),
        (
            count == record.external_provider_count(),
            WorkerV3LoadEnvelopeBindingFieldV1::ExternalProviderCount,
        ),
        (
            payload_length == record.external_provider_payload_length(),
            WorkerV3LoadEnvelopeBindingFieldV1::ExternalProviderPayloadLength,
        ),
    ] {
        if !matches {
            return binding_mismatch(field);
        }
    }
    Ok(())
}

fn provider_archive_bindings<T: AsRef<[u8]>>(
    payloads: &[T],
) -> Result<(usize, usize, usize, [u8; 32]), WorkerV3LoadEnvelopeErrorV1> {
    let count = payloads.len();
    if count > MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1 {
        return Err(WorkerV3LoadEnvelopeErrorV1::CountOutOfRange {
            actual: count as u64,
            max: MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1,
        });
    }
    let payload_length = payloads.iter().try_fold(0_usize, |total, payload| {
        let payload = payload.as_ref();
        if payload.is_empty() {
            return Err(WorkerV3LoadEnvelopeErrorV1::LengthOutOfRange {
                field: "provider payload",
                actual: 0,
                max: MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
            });
        }
        total
            .checked_add(payload.len())
            .ok_or(WorkerV3LoadEnvelopeErrorV1::LengthOverflow {
                field: "provider payload aggregate",
            })
    })?;
    if payload_length > MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1 {
        return Err(WorkerV3LoadEnvelopeErrorV1::LengthOutOfRange {
            field: "provider payload aggregate",
            actual: payload_length as u64,
            max: MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
        });
    }
    let canonical_length = PROVIDER_ARCHIVE_FIXED_BYTES_V1
        .checked_add(count.checked_mul(PROVIDER_ARCHIVE_ENTRY_BYTES_V1).ok_or(
            WorkerV3LoadEnvelopeErrorV1::LengthOverflow {
                field: "provider archive framing",
            },
        )?)
        .and_then(|value| value.checked_add(payload_length))
        .ok_or(WorkerV3LoadEnvelopeErrorV1::LengthOverflow {
            field: "provider archive",
        })?;

    let mut checksum_digest = Sha256::new();
    checksum_digest.update(PROVIDER_ARCHIVE_CHECKSUM_DOMAIN_V1);
    update_provider_archive_body(&mut checksum_digest, payloads, payload_length);
    let checksum: [u8; 32] = checksum_digest.finalize().into();
    let mut archive_digest = Sha256::new();
    update_provider_archive_body(&mut archive_digest, payloads, payload_length);
    archive_digest.update(checksum);
    Ok((
        count,
        payload_length,
        canonical_length,
        archive_digest.finalize().into(),
    ))
}

fn update_provider_archive_body<T: AsRef<[u8]>>(
    digest: &mut Sha256,
    payloads: &[T],
    payload_length: usize,
) {
    digest.update(PROVIDER_ARCHIVE_MAGIC_V1);
    digest.update(PROVIDER_ARCHIVE_VERSION_V1.to_le_bytes());
    digest.update((payloads.len() as u32).to_le_bytes());
    digest.update((payload_length as u64).to_le_bytes());
    for payload in payloads {
        let payload = payload.as_ref();
        digest.update((payload.len() as u64).to_le_bytes());
        digest.update(Sha256::digest(payload));
    }
    for payload in payloads {
        digest.update(payload.as_ref());
    }
}

fn validate_component_hash_length(
    bytes: &[u8],
    expected_hash: [u8; 32],
    expected_length: usize,
    hash_field: WorkerV3LoadEnvelopeBindingFieldV1,
    length_field: WorkerV3LoadEnvelopeBindingFieldV1,
) -> Result<(), WorkerV3LoadEnvelopeErrorV1> {
    if bytes.len() != expected_length {
        return binding_mismatch(length_field);
    }
    if <[u8; 32]>::from(Sha256::digest(bytes)) != expected_hash {
        return binding_mismatch(hash_field);
    }
    Ok(())
}

fn validate_finalized_artifact(
    record: WorkerV3PublicationIntentRecordV1,
    claim: &DurablePublishedHsacoClaimV3,
    lease: &DurableCurrentLinkPublicationLeaseV1,
    replay_finalized: Option<&[u8]>,
) -> Result<(), WorkerV3LoadEnvelopeErrorV1> {
    validate_record_claim(record, claim)?;
    if !published_matches_plan(lease, claim.plan()) {
        return binding_mismatch(WorkerV3LoadEnvelopeBindingFieldV1::LeasePublication);
    }
    let exact = lease.exact_artifact_bytes();
    validate_component_hash_length(
        exact,
        record.output_sha256(),
        record.output_length(),
        WorkerV3LoadEnvelopeBindingFieldV1::FinalizedOutputHash,
        WorkerV3LoadEnvelopeBindingFieldV1::FinalizedOutputLength,
    )?;
    if replay_finalized.is_some_and(|bytes| bytes != exact) {
        return binding_mismatch(WorkerV3LoadEnvelopeBindingFieldV1::LeaseArtifact);
    }
    Ok(())
}

fn published_matches_plan(
    lease: &DurableCurrentLinkPublicationLeaseV1,
    plan: DurableLinkPublicationPlanV1,
) -> bool {
    let published = lease.published();
    published.attempt() == plan.attempt()
        && published.scope() == plan.scope()
        && published.request() == plan.request()
        && published.worker() == plan.worker()
        && published.response() == plan.response()
        && published.linked_output() == plan.linked_output()
        && published.finalization() == plan.finalization()
        && published.finalized_output() == plan.finalized_output()
        && published.publication() == plan.publication()
}

struct DecodedSections<'a> {
    record: &'a [u8],
    claim: &'a [u8],
    outer_handoff: &'a [u8],
    providers: Vec<&'a [u8]>,
    provider_payload_bytes: usize,
    transcript: &'a [u8],
}

fn decode_sections<'a>(
    bytes: &'a [u8],
    budget: WorkerV3LoadEnvelopeCodecBudgetV1,
) -> Result<DecodedSections<'a>, WorkerV3LoadEnvelopeErrorV1> {
    require_wire_bound(bytes.len(), budget)?;
    if bytes.len() < HEADER_BYTES_V1 {
        return Err(WorkerV3LoadEnvelopeErrorV1::Truncated);
    }
    let mut header = Reader::new(bytes);
    if header.array::<8>()? != WORKER_V3_LOAD_ENVELOPE_MAGIC_V1 {
        return Err(WorkerV3LoadEnvelopeErrorV1::BadMagic);
    }
    let version = header.u16()?;
    if version != WORKER_V3_LOAD_ENVELOPE_VERSION_V1 {
        return Err(WorkerV3LoadEnvelopeErrorV1::UnsupportedVersion { actual: version });
    }
    if header.u16()? != 0 {
        return Err(WorkerV3LoadEnvelopeErrorV1::NonZeroReserved);
    }
    let declared_total = header.u64()?;
    let actual_u64 = u64::try_from(bytes.len())
        .map_err(|_| WorkerV3LoadEnvelopeErrorV1::LengthOverflow { field: "envelope" })?;
    if declared_total < actual_u64 {
        return Err(WorkerV3LoadEnvelopeErrorV1::TrailingBytes);
    }
    if declared_total > actual_u64 {
        return Err(WorkerV3LoadEnvelopeErrorV1::Truncated);
    }
    if declared_total != actual_u64 {
        return Err(WorkerV3LoadEnvelopeErrorV1::InvalidTotalLength {
            declared: declared_total,
            actual: bytes.len(),
        });
    }
    let record_len = bounded_length(
        u64::from(header.u32()?),
        "publication intent record",
        MAX_WORKER_V3_PUBLICATION_INTENT_RECORD_BYTES_V1,
    )?;
    let claim_len = bounded_length(
        u64::from(header.u32()?),
        "V3 published claim",
        MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V3,
    )?;
    let outer_len = bounded_nonzero_length(
        header.u64()?,
        "outer semantic handoff",
        MAX_COMPILER_MODULE_HANDOFF_BYTES_V3,
    )?;
    let provider_count_u64 = u64::from(header.u32()?);
    let provider_max = budget
        .max_provider_count
        .min(MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1);
    if provider_count_u64 > provider_max as u64 {
        return Err(WorkerV3LoadEnvelopeErrorV1::CountOutOfRange {
            actual: provider_count_u64,
            max: provider_max,
        });
    }
    let provider_count = provider_count_u64 as usize;
    if header.u32()? != 0 {
        return Err(WorkerV3LoadEnvelopeErrorV1::NonZeroReserved);
    }
    let provider_payload_bytes = bounded_length(
        header.u64()?,
        "provider payload aggregate",
        MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
    )?;
    let transcript_len = bounded_nonzero_length(
        header.u64()?,
        "compact V2 transcript",
        fe2o3_hsaco_finalize::MAX_PROTECTED_WORKER_V3_COMPACT_FINALIZER_REPLAY_BYTES_V1,
    )?;
    debug_assert_eq!(header.consumed(), HEADER_BYTES_V1);

    let body_len = bytes
        .len()
        .checked_sub(CHECKSUM_BYTES_V1)
        .ok_or(WorkerV3LoadEnvelopeErrorV1::Truncated)?;
    if body_len < HEADER_BYTES_V1 {
        return Err(WorkerV3LoadEnvelopeErrorV1::Truncated);
    }
    let (body, actual_checksum) = bytes.split_at(body_len);
    if checksum(body) != actual_checksum {
        return Err(WorkerV3LoadEnvelopeErrorV1::ChecksumMismatch);
    }

    let framing_bytes = provider_count.checked_mul(PROVIDER_LENGTH_BYTES_V1).ok_or(
        WorkerV3LoadEnvelopeErrorV1::LengthOverflow {
            field: "provider framing",
        },
    )?;
    let expected = HEADER_BYTES_V1
        .checked_add(record_len)
        .and_then(|value| value.checked_add(claim_len))
        .and_then(|value| value.checked_add(outer_len))
        .and_then(|value| value.checked_add(framing_bytes))
        .and_then(|value| value.checked_add(provider_payload_bytes))
        .and_then(|value| value.checked_add(transcript_len))
        .and_then(|value| value.checked_add(CHECKSUM_BYTES_V1))
        .ok_or(WorkerV3LoadEnvelopeErrorV1::LengthOverflow { field: "envelope" })?;
    if expected != bytes.len() {
        return Err(WorkerV3LoadEnvelopeErrorV1::InvalidTotalLength {
            declared: declared_total,
            actual: expected,
        });
    }

    let mut reader = Reader::new(&body[HEADER_BYTES_V1..]);
    let record = reader.take(record_len)?;
    let claim = reader.take(claim_len)?;
    let outer_handoff = reader.take(outer_len)?;
    let provider_list_bytes = provider_count.checked_mul(mem::size_of::<&[u8]>()).ok_or(
        WorkerV3LoadEnvelopeErrorV1::LengthOverflow {
            field: "provider slice list",
        },
    )?;
    require_allocation_budget(provider_list_bytes, budget)?;
    let mut providers = Vec::new();
    providers.try_reserve_exact(provider_count).map_err(|_| {
        WorkerV3LoadEnvelopeErrorV1::AllocationFailed {
            field: "provider slice list",
            requested: provider_count,
        }
    })?;
    let mut observed_provider_bytes = 0_usize;
    for _ in 0..provider_count {
        let payload_len = bounded_nonzero_length(
            reader.u64()?,
            "provider payload",
            MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
        )?;
        observed_provider_bytes = observed_provider_bytes.checked_add(payload_len).ok_or(
            WorkerV3LoadEnvelopeErrorV1::LengthOverflow {
                field: "provider payload aggregate",
            },
        )?;
        if observed_provider_bytes > provider_payload_bytes {
            return binding_mismatch(
                WorkerV3LoadEnvelopeBindingFieldV1::ExternalProviderPayloadLength,
            );
        }
        providers.push(reader.take(payload_len)?);
    }
    if observed_provider_bytes != provider_payload_bytes {
        return binding_mismatch(WorkerV3LoadEnvelopeBindingFieldV1::ExternalProviderPayloadLength);
    }
    let transcript = reader.take(transcript_len)?;
    if !reader.is_empty() {
        return Err(WorkerV3LoadEnvelopeErrorV1::TrailingBytes);
    }
    Ok(DecodedSections {
        record,
        claim,
        outer_handoff,
        providers,
        provider_payload_bytes,
        transcript,
    })
}

fn validate_outer_handoff(
    bytes: &[u8],
) -> Result<fe2o3_hsaco_finalize::CompilerClosureV2, WorkerV3LoadEnvelopeErrorV1> {
    let handoff = InertSemanticCompilerModuleHandoffV3::decode(bytes).map_err(|_| {
        WorkerV3LoadEnvelopeErrorV1::NonCanonicalOuterHandoff {
            field: "shared strict V3 decoder",
        }
    })?;
    if handoff.canonical_bytes() != bytes {
        return Err(WorkerV3LoadEnvelopeErrorV1::NonCanonicalOuterHandoff {
            field: "shared strict V3 canonical bytes",
        });
    }
    Ok(*handoff.capsule().compiler_closure())
}

fn canonical_wire_length(
    record_len: usize,
    claim_len: usize,
    outer_len: usize,
    provider_count: usize,
    provider_payload_bytes: usize,
    transcript_len: usize,
) -> Result<usize, WorkerV3LoadEnvelopeErrorV1> {
    HEADER_BYTES_V1
        .checked_add(record_len)
        .and_then(|value| value.checked_add(claim_len))
        .and_then(|value| value.checked_add(outer_len))
        .and_then(|value| value.checked_add(provider_count.checked_mul(PROVIDER_LENGTH_BYTES_V1)?))
        .and_then(|value| value.checked_add(provider_payload_bytes))
        .and_then(|value| value.checked_add(transcript_len))
        .and_then(|value| value.checked_add(CHECKSUM_BYTES_V1))
        .ok_or(WorkerV3LoadEnvelopeErrorV1::LengthOverflow { field: "envelope" })
}

fn provider_payload_length(payloads: &[Vec<u8>]) -> Result<usize, WorkerV3LoadEnvelopeErrorV1> {
    payloads.iter().try_fold(0_usize, |total, payload| {
        total
            .checked_add(payload.len())
            .ok_or(WorkerV3LoadEnvelopeErrorV1::LengthOverflow {
                field: "provider payload aggregate",
            })
    })
}

fn bounded_length(
    actual: u64,
    field: &'static str,
    max: usize,
) -> Result<usize, WorkerV3LoadEnvelopeErrorV1> {
    if actual > max as u64 {
        return Err(WorkerV3LoadEnvelopeErrorV1::LengthOutOfRange { field, actual, max });
    }
    usize::try_from(actual).map_err(|_| WorkerV3LoadEnvelopeErrorV1::LengthOverflow { field })
}

fn bounded_nonzero_length(
    actual: u64,
    field: &'static str,
    max: usize,
) -> Result<usize, WorkerV3LoadEnvelopeErrorV1> {
    if actual == 0 {
        return Err(WorkerV3LoadEnvelopeErrorV1::LengthOutOfRange { field, actual, max });
    }
    bounded_length(actual, field, max)
}

fn as_u32(value: usize, field: &'static str) -> Result<u32, WorkerV3LoadEnvelopeErrorV1> {
    u32::try_from(value).map_err(|_| WorkerV3LoadEnvelopeErrorV1::LengthOverflow { field })
}

fn as_u64(value: usize, field: &'static str) -> Result<u64, WorkerV3LoadEnvelopeErrorV1> {
    u64::try_from(value).map_err(|_| WorkerV3LoadEnvelopeErrorV1::LengthOverflow { field })
}

fn require_wire_bound(
    actual: usize,
    budget: WorkerV3LoadEnvelopeCodecBudgetV1,
) -> Result<(), WorkerV3LoadEnvelopeErrorV1> {
    let max = budget
        .max_wire_bytes
        .min(MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1);
    if actual > max {
        return Err(WorkerV3LoadEnvelopeErrorV1::WireTooLarge { actual, max });
    }
    Ok(())
}

fn require_allocation_budget(
    required: usize,
    budget: WorkerV3LoadEnvelopeCodecBudgetV1,
) -> Result<(), WorkerV3LoadEnvelopeErrorV1> {
    let max = budget
        .max_allocation_bytes
        .min(MAX_WORKER_V3_LOAD_ENVELOPE_ALLOCATION_BYTES_V1);
    if required > max {
        return Err(WorkerV3LoadEnvelopeErrorV1::AllocationBudgetExceeded { required, max });
    }
    Ok(())
}

fn try_vec_bytes(
    capacity: usize,
    field: &'static str,
) -> Result<Vec<u8>, WorkerV3LoadEnvelopeErrorV1> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(capacity).map_err(|_| {
        WorkerV3LoadEnvelopeErrorV1::AllocationFailed {
            field,
            requested: capacity,
        }
    })?;
    Ok(bytes)
}

fn binding_mismatch<T>(
    field: WorkerV3LoadEnvelopeBindingFieldV1,
) -> Result<T, WorkerV3LoadEnvelopeErrorV1> {
    Err(WorkerV3LoadEnvelopeErrorV1::BindingMismatch { field })
}

fn checksum(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(LOAD_ENVELOPE_CHECKSUM_DOMAIN_V1);
    digest.update(bytes);
    digest.finalize().into()
}

struct AllocationTracker {
    budget: WorkerV3LoadEnvelopeCodecBudgetV1,
    used: usize,
}

impl AllocationTracker {
    const fn new(budget: WorkerV3LoadEnvelopeCodecBudgetV1) -> Self {
        Self { budget, used: 0 }
    }

    fn account(&mut self, bytes: usize) -> Result<(), WorkerV3LoadEnvelopeErrorV1> {
        self.used =
            self.used
                .checked_add(bytes)
                .ok_or(WorkerV3LoadEnvelopeErrorV1::LengthOverflow {
                    field: "decoded envelope allocation",
                })?;
        require_allocation_budget(self.used, self.budget)
    }

    fn copy(
        &mut self,
        source: &[u8],
        field: &'static str,
    ) -> Result<Vec<u8>, WorkerV3LoadEnvelopeErrorV1> {
        self.account(source.len())?;
        let mut value = try_vec_bytes(source.len(), field)?;
        value.extend_from_slice(source);
        Ok(value)
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    const fn consumed(&self) -> usize {
        self.offset
    }

    const fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], WorkerV3LoadEnvelopeErrorV1> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(WorkerV3LoadEnvelopeErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(WorkerV3LoadEnvelopeErrorV1::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], WorkerV3LoadEnvelopeErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| WorkerV3LoadEnvelopeErrorV1::Truncated)
    }

    fn u16(&mut self) -> Result<u16, WorkerV3LoadEnvelopeErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, WorkerV3LoadEnvelopeErrorV1> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, WorkerV3LoadEnvelopeErrorV1> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_artifact_transaction::WorkerV3ExternalProviderPayloadsV1;

    const WORKER_V2_LOAD_ENVELOPE_MAGIC: [u8; 8] = *b"FE2W2B1\0";
    const WORKER_V2_LOAD_ENVELOPE_VERSION: u16 = 1;
    const WORKER_V2_LOAD_ENVELOPE_MAGIC_V2: [u8; 8] = *b"FE2W2B2\0";
    const WORKER_V2_LOAD_ENVELOPE_VERSION_V2: u16 = 2;

    fn framed(
        record: &[u8],
        claim: &[u8],
        outer: &[u8],
        providers: &[&[u8]],
        transcript: &[u8],
    ) -> Vec<u8> {
        let payload_bytes = providers.iter().map(|value| value.len()).sum::<usize>();
        let total = canonical_wire_length(
            record.len(),
            claim.len(),
            outer.len(),
            providers.len(),
            payload_bytes,
            transcript.len(),
        )
        .unwrap();
        let mut bytes = Vec::with_capacity(total);
        bytes.extend_from_slice(&WORKER_V3_LOAD_ENVELOPE_MAGIC_V1);
        bytes.extend_from_slice(&WORKER_V3_LOAD_ENVELOPE_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&(total as u64).to_le_bytes());
        bytes.extend_from_slice(&(record.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(claim.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(outer.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&(providers.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&(payload_bytes as u64).to_le_bytes());
        bytes.extend_from_slice(&(transcript.len() as u64).to_le_bytes());
        bytes.extend_from_slice(record);
        bytes.extend_from_slice(claim);
        bytes.extend_from_slice(outer);
        for provider in providers {
            bytes.extend_from_slice(&(provider.len() as u64).to_le_bytes());
            bytes.extend_from_slice(provider);
        }
        bytes.extend_from_slice(transcript);
        let digest = checksum(&bytes);
        bytes.extend_from_slice(&digest);
        bytes
    }

    fn rechecksum(bytes: &mut [u8]) {
        let checksum_offset = bytes.len() - CHECKSUM_BYTES_V1;
        let digest = checksum(&bytes[..checksum_offset]);
        bytes[checksum_offset..].copy_from_slice(&digest);
    }

    fn structurally_framed() -> Vec<u8> {
        framed(&[1], &[2], &[3], &[b"provider"], &[4])
    }

    #[test]
    fn truncation_and_trailing_bytes_are_rejected_before_nested_decode() {
        let bytes = structurally_framed();
        for length in 0..bytes.len() {
            assert!(WorkerV3LoadEnvelopeWireV1::decode_canonical(&bytes[..length]).is_err());
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(matches!(
            WorkerV3LoadEnvelopeWireV1::decode_canonical(&trailing),
            Err(WorkerV3LoadEnvelopeErrorV1::TrailingBytes)
        ));
    }

    #[test]
    fn magic_version_reserved_and_checksum_are_rejected() {
        let mut magic = structurally_framed();
        magic[0] ^= 1;
        assert!(matches!(
            WorkerV3LoadEnvelopeWireV1::decode_canonical(&magic),
            Err(WorkerV3LoadEnvelopeErrorV1::BadMagic)
        ));

        let mut version = structurally_framed();
        version[8..10].copy_from_slice(&2_u16.to_le_bytes());
        rechecksum(&mut version);
        assert!(matches!(
            WorkerV3LoadEnvelopeWireV1::decode_canonical(&version),
            Err(WorkerV3LoadEnvelopeErrorV1::UnsupportedVersion { actual: 2 })
        ));

        for range in [10..12, 40..44] {
            let mut reserved = structurally_framed();
            let width = range.len();
            reserved[range].copy_from_slice(&1_u32.to_le_bytes()[..width]);
            rechecksum(&mut reserved);
            assert!(matches!(
                WorkerV3LoadEnvelopeWireV1::decode_canonical(&reserved),
                Err(WorkerV3LoadEnvelopeErrorV1::NonZeroReserved)
            ));
        }

        let mut checksum_bytes = structurally_framed();
        let last = checksum_bytes.len() - 1;
        checksum_bytes[last] ^= 1;
        assert!(matches!(
            WorkerV3LoadEnvelopeWireV1::decode_canonical(&checksum_bytes),
            Err(WorkerV3LoadEnvelopeErrorV1::ChecksumMismatch)
        ));
    }

    #[test]
    fn hostile_count_and_lengths_are_rejected_without_allocation() {
        let mut count = structurally_framed();
        count[36..40].copy_from_slice(
            &((MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1 as u32) + 1).to_le_bytes(),
        );
        rechecksum(&mut count);
        assert!(matches!(
            WorkerV3LoadEnvelopeWireV1::decode_canonical(&count),
            Err(WorkerV3LoadEnvelopeErrorV1::CountOutOfRange { .. })
        ));

        for (range, value) in [
            (28..36, (MAX_COMPILER_MODULE_HANDOFF_BYTES_V3 as u64) + 1),
            (
                44..52,
                (MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1 as u64) + 1,
            ),
            (
                52..60,
                (fe2o3_hsaco_finalize::MAX_PROTECTED_WORKER_V3_COMPACT_FINALIZER_REPLAY_BYTES_V1
                    as u64)
                    + 1,
            ),
        ] {
            let mut bytes = structurally_framed();
            bytes[range].copy_from_slice(&value.to_le_bytes());
            rechecksum(&mut bytes);
            assert!(matches!(
                WorkerV3LoadEnvelopeWireV1::decode_canonical(&bytes),
                Err(WorkerV3LoadEnvelopeErrorV1::LengthOutOfRange { .. })
            ));
        }
    }

    #[test]
    fn provider_substitution_and_reordering_are_not_accepted_as_canonical_evidence() {
        let first = framed(&[1], &[2], &[3], &[b"a", b"b"], &[4]);
        let substituted = framed(&[1], &[2], &[3], &[b"a", b"c"], &[4]);
        let reordered = framed(&[1], &[2], &[3], &[b"b", b"a"], &[4]);
        assert_ne!(first, substituted);
        assert_ne!(first, reordered);
        assert!(WorkerV3LoadEnvelopeWireV1::decode_canonical(&substituted).is_err());
        assert!(WorkerV3LoadEnvelopeWireV1::decode_canonical(&reordered).is_err());
    }

    #[test]
    fn worker_v2_wires_have_no_v3_fallback() {
        for (magic, version) in [
            (
                WORKER_V2_LOAD_ENVELOPE_MAGIC,
                WORKER_V2_LOAD_ENVELOPE_VERSION,
            ),
            (
                WORKER_V2_LOAD_ENVELOPE_MAGIC_V2,
                WORKER_V2_LOAD_ENVELOPE_VERSION_V2,
            ),
        ] {
            let mut bytes = structurally_framed();
            bytes[..8].copy_from_slice(&magic);
            bytes[8..10].copy_from_slice(&version.to_le_bytes());
            rechecksum(&mut bytes);
            assert!(matches!(
                WorkerV3LoadEnvelopeWireV1::decode_canonical(&bytes),
                Err(WorkerV3LoadEnvelopeErrorV1::BadMagic)
            ));
        }
    }

    #[test]
    fn codec_budget_rejects_wire_and_allocation_exhaustion() {
        let bytes = structurally_framed();
        let short_wire = WorkerV3LoadEnvelopeCodecBudgetV1::new(
            bytes.len() - 1,
            usize::MAX,
            MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1,
        );
        assert!(matches!(
            WorkerV3LoadEnvelopeWireV1::decode_canonical_with_budget(&bytes, short_wire),
            Err(WorkerV3LoadEnvelopeErrorV1::WireTooLarge { .. })
        ));

        let no_allocation = WorkerV3LoadEnvelopeCodecBudgetV1::new(
            bytes.len(),
            0,
            MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1,
        );
        assert!(matches!(
            WorkerV3LoadEnvelopeWireV1::decode_canonical_with_budget(&bytes, no_allocation),
            Err(WorkerV3LoadEnvelopeErrorV1::AllocationBudgetExceeded { .. })
        ));
    }

    #[test]
    fn streaming_provider_archive_binding_matches_the_transaction_codec() {
        let payloads = vec![b"first-provider".to_vec(), b"second-provider".to_vec()];
        let expected = WorkerV3ExternalProviderPayloadsV1::new(payloads.clone()).unwrap();
        let (count, payload_length, canonical_length, canonical_sha256) =
            provider_archive_bindings(&payloads).unwrap();
        assert_eq!(count, expected.len());
        assert_eq!(payload_length, expected.payload_length());
        assert_eq!(canonical_length, expected.canonical_length());
        assert_eq!(canonical_sha256, expected.canonical_sha256());
    }
}
