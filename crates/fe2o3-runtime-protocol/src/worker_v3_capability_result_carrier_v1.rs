//! Durable side-by-side carriage for one completed production capability result.
//!
//! Worker V3 load-envelope V2 is frozen. This record therefore names the exact V2 byte string
//! and retains the complete V5 result beside it instead of changing the V2 identity grammar.
//! Decoding establishes canonical structure and exact-byte associations only. Protected compiler
//! currentness, target/launch interpretation, and runtime authority remain host admission work.

use core::fmt;
use std::error::Error;

use fe2o3_artifact_transaction::{
    BuildAttempt, NoRetainedDurableDirectoryHooksV1, RetainedDurableDirectoryErrorV1,
    RetainedDurableDirectoryHooksV1, RetainedDurableDirectoryV1, WorkerV3LoadEnvelopeBindingV1,
    WorkerV3LoadReadinessCodecErrorV1, WorkerV3LoadReadinessResultV1,
};
use fe2o3_compiler_ffi::{
    InertProductionCapabilityHandoffErrorV5, InertProductionCapabilityResultIdentityV5,
    InertProductionCapabilityResultV5, MAX_INERT_PRODUCTION_CAPABILITY_RESULT_BYTES_V5,
};
use sha2::{Digest, Sha256};

use crate::{RecoveredWorkerV3LoadEnvelopeV2, WorkerV3LoadEnvelopeErrorV2, WorkerV3LoadEnvelopeV2};

/// Magic for the side-by-side completed-result carrier.
pub const WORKER_V3_CAPABILITY_RESULT_CARRIER_MAGIC_V1: [u8; 8] = *b"F3CPRS01";
/// Frozen wire version for the completed-result carrier.
pub const WORKER_V3_CAPABILITY_RESULT_CARRIER_VERSION_V1: u16 = 1;

const HEADER_BYTES: usize = 32;
const ATTEMPT_BYTES: usize = 8 + 16 + 32;
const BINDING_BYTES: usize = 32 + 8;
const FIXED_COORDINATE_BYTES: usize = BINDING_BYTES // exact V2 envelope
    + 32 // load-readiness receipt
    + ATTEMPT_BYTES
    + BINDING_BYTES // raw compiler object
    + BINDING_BYTES // finalized HSACO
    + 32 // target model
    + 32 // launch roster
    + 32 // subject/root roster
    + 32 // dynamic-precondition roster
    + 32 // compiler subject
    + 32 // compiler carriage
    + 32 // compiler policy
    + 32 // compiler occurrence
    + 32 // current Worker ledger record
    + 8 // compiler sequence
    + 32 // compiler rollback head
    + BINDING_BYTES // source-refinement receipt
    + BINDING_BYTES // machine-refinement receipt
    + BINDING_BYTES; // completed V5 result
const TERMINAL_IDENTITY_BYTES: usize = 32;
const CHECKSUM_BYTES: usize = 32;
const FIXED_OVERHEAD_BYTES: usize =
    HEADER_BYTES + FIXED_COORDINATE_BYTES + TERMINAL_IDENTITY_BYTES + CHECKSUM_BYTES;
/// Maximum complete sidecar bytes under the frozen V1 grammar.
pub const MAX_WORKER_V3_CAPABILITY_RESULT_CARRIER_BYTES_V1: usize =
    MAX_INERT_PRODUCTION_CAPABILITY_RESULT_BYTES_V5 + FIXED_OVERHEAD_BYTES;

const CARRIER_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/WORKER-V3/COMPLETED-CAPABILITY-RESULT-CARRIER-IDENTITY/V1\0";
const CARRIER_CHECKSUM_DOMAIN: &[u8] =
    b"FE2O3/WORKER-V3/COMPLETED-CAPABILITY-RESULT-CARRIER-CHECKSUM/V1\0";
const SUBJECT_ROSTER_DOMAIN: &[u8] =
    b"FE2O3/WORKER-V3/COMPLETED-CAPABILITY-SUBJECT-ROOT-ROSTER/V1\0";
const DYNAMIC_PRECONDITION_ROSTER_DOMAIN: &[u8] =
    b"FE2O3/WORKER-V3/DYNAMIC-LAUNCH-PRECONDITION-ROSTER/V1\0";
const DURABLE_NAMESPACE_DOMAIN: &[u8] =
    b"FE2O3/WORKER-V3/COMPLETED-CAPABILITY-DURABLE-NAMESPACE/V1\0";
const CONSUMPTION_MAGIC: [u8; 8] = *b"F3CPCN01";
const CONSUMPTION_VERSION: u16 = 1;
const CONSUMPTION_BYTES: usize = 8 + 2 + 2 + 32 + 8 + 32 + 32;
const CONSUMPTION_CHECKSUM_DOMAIN: &[u8] =
    b"FE2O3/WORKER-V3/COMPLETED-CAPABILITY-CONSUMPTION-CHECKSUM/V1\0";
const PENDING_MAGIC: [u8; 8] = *b"F3CPPN01";
const PENDING_VERSION: u16 = 1;
const PENDING_HEADER_BYTES: usize = 8 + 2 + 2 + 8 + BINDING_BYTES + ATTEMPT_BYTES + 8;
const PENDING_IDENTITY_BYTES: usize = 32;
const PENDING_CHECKSUM_BYTES: usize = 32;
const PENDING_FIXED_OVERHEAD_BYTES: usize =
    PENDING_HEADER_BYTES + PENDING_IDENTITY_BYTES + PENDING_CHECKSUM_BYTES;
/// Maximum durable bytes in the pre-readiness completion journal.
pub const MAX_WORKER_V3_PENDING_CAPABILITY_RESULT_BYTES_V1: usize =
    MAX_INERT_PRODUCTION_CAPABILITY_RESULT_BYTES_V5 + PENDING_FIXED_OVERHEAD_BYTES;
const PENDING_IDENTITY_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/PENDING-CAPABILITY-RESULT-IDENTITY/V1\0";
const PENDING_CHECKSUM_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/PENDING-CAPABILITY-RESULT-CHECKSUM/V1\0";

/// Exact canonical carrier identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerV3CapabilityResultCarrierIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl WorkerV3CapabilityResultCarrierIdentityV1 {
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Identity of the exact ordered dynamic-launch requirement roster.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerV3DynamicPreconditionRosterIdentityV1([u8; 32]);

impl WorkerV3DynamicPreconditionRosterIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Exact association rejected while joining the carrier to V2 custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3CapabilityResultBindingFieldV1 {
    LoadEnvelope,
    LoadReadinessReceipt,
    Attempt,
    LegacyCompilerHandoff,
    RawCompilerObject,
    FinalizedArtifact,
    TargetModel,
    LaunchRoster,
    SubjectRootRoster,
    DynamicPreconditions,
    CompilerSubject,
    CompilerCarriage,
    CompilerPolicy,
    CompilerOccurrence,
    CompilerCurrentness,
    SourceRefinement,
    MachineRefinement,
    ProductionResult,
}

/// Strict codec, durable-I/O, recovery, or association failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3CapabilityResultCarrierErrorV1 {
    Result(InertProductionCapabilityHandoffErrorV5),
    LoadEnvelope(WorkerV3LoadEnvelopeErrorV2),
    LoadReadiness(WorkerV3LoadReadinessCodecErrorV1),
    Durable(RetainedDurableDirectoryErrorV1),
    WireLength { actual: usize, maximum: usize },
    ResultLength { actual: u64, maximum: usize },
    LengthOverflow,
    AllocationFailed { requested: usize },
    Truncated,
    TrailingBytes,
    BadMagic,
    UnsupportedVersion { actual: u16 },
    UnsupportedFlags { actual: u16 },
    InvalidTotalLength { declared: u64, actual: usize },
    ChecksumMismatch,
    IdentityMismatch,
    NoncanonicalResult,
    BindingMismatch(WorkerV3CapabilityResultBindingFieldV1),
    MissingDurableCarrier,
    MissingDurablePendingResult,
    DurableCarrierConflict,
    AlreadyConsumed,
    InvalidConsumptionRecord,
    ZeroSealedAdmissionIdentity,
}

impl fmt::Display for WorkerV3CapabilityResultCarrierErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Result(error) => write!(formatter, "invalid completed V5 result: {error}"),
            Self::LoadEnvelope(error) => write!(formatter, "invalid V2 load envelope: {error}"),
            Self::LoadReadiness(error) => {
                write!(formatter, "invalid load-readiness receipt: {error}")
            }
            Self::Durable(error) => write!(formatter, "durable carrier I/O failed: {error}"),
            Self::WireLength { actual, maximum } => write!(
                formatter,
                "capability carrier is {actual} bytes; maximum is {maximum}"
            ),
            Self::ResultLength { actual, maximum } => write!(
                formatter,
                "completed V5 result is {actual} bytes; maximum is {maximum}"
            ),
            Self::LengthOverflow => formatter.write_str("capability carrier length overflows"),
            Self::AllocationFailed { requested } => write!(
                formatter,
                "failed to allocate {requested} capability-carrier bytes"
            ),
            Self::Truncated => formatter.write_str("truncated completed-result carrier"),
            Self::TrailingBytes => {
                formatter.write_str("completed-result carrier has trailing bytes")
            }
            Self::BadMagic => formatter.write_str("completed-result carrier magic mismatch"),
            Self::UnsupportedVersion { actual } => write!(
                formatter,
                "unsupported completed-result carrier version {actual}"
            ),
            Self::UnsupportedFlags { actual } => write!(
                formatter,
                "unsupported completed-result carrier flags {actual:#06x}"
            ),
            Self::InvalidTotalLength { declared, actual } => write!(
                formatter,
                "completed-result carrier declares {declared} bytes but contains {actual}"
            ),
            Self::ChecksumMismatch => {
                formatter.write_str("completed-result carrier checksum mismatch")
            }
            Self::IdentityMismatch => {
                formatter.write_str("completed-result carrier identity mismatch")
            }
            Self::NoncanonicalResult => {
                formatter.write_str("completed V5 result bytes are not canonical")
            }
            Self::BindingMismatch(field) => write!(
                formatter,
                "completed-result carrier binding mismatch: {field:?}"
            ),
            Self::MissingDurableCarrier => {
                formatter.write_str("durable completed-result carrier is missing")
            }
            Self::MissingDurablePendingResult => {
                formatter.write_str("durable pre-readiness capability-result journal is missing")
            }
            Self::DurableCarrierConflict => formatter.write_str(
                "different completed-result carrier already occupies this build occurrence",
            ),
            Self::AlreadyConsumed => formatter.write_str(
                "completed-result carrier was already consumed by sealed host admission",
            ),
            Self::InvalidConsumptionRecord => formatter
                .write_str("completed-result consumption record is malformed or substituted"),
            Self::ZeroSealedAdmissionIdentity => {
                formatter.write_str("sealed host-admission identity is zero")
            }
        }
    }
}

impl Error for WorkerV3CapabilityResultCarrierErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Result(error) => Some(error),
            Self::LoadEnvelope(error) => Some(error),
            Self::LoadReadiness(error) => Some(error),
            Self::Durable(error) => Some(error),
            _ => None,
        }
    }
}

impl From<InertProductionCapabilityHandoffErrorV5> for WorkerV3CapabilityResultCarrierErrorV1 {
    fn from(error: InertProductionCapabilityHandoffErrorV5) -> Self {
        Self::Result(error)
    }
}

impl From<WorkerV3LoadEnvelopeErrorV2> for WorkerV3CapabilityResultCarrierErrorV1 {
    fn from(error: WorkerV3LoadEnvelopeErrorV2) -> Self {
        Self::LoadEnvelope(error)
    }
}

impl From<WorkerV3LoadReadinessCodecErrorV1> for WorkerV3CapabilityResultCarrierErrorV1 {
    fn from(error: WorkerV3LoadReadinessCodecErrorV1) -> Self {
        Self::LoadReadiness(error)
    }
}

impl From<RetainedDurableDirectoryErrorV1> for WorkerV3CapabilityResultCarrierErrorV1 {
    fn from(error: RetainedDurableDirectoryErrorV1) -> Self {
        Self::Durable(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CarrierCoordinatesV1 {
    load_envelope: ExactBindingV1,
    readiness_receipt: [u8; 32],
    generation: u64,
    session: [u8; 16],
    invocation: [u8; 32],
    raw_object: ExactBindingV1,
    finalized_artifact: ExactBindingV1,
    target_model: [u8; 32],
    launch_roster: [u8; 32],
    subject_root_roster: [u8; 32],
    dynamic_preconditions: [u8; 32],
    compiler_subject: [u8; 32],
    compiler_carriage: [u8; 32],
    compiler_policy: [u8; 32],
    compiler_occurrence: [u8; 32],
    worker_ledger_record: [u8; 32],
    compiler_sequence: u64,
    current_rollback_anchor: [u8; 32],
    source_refinement: ExactBindingV1,
    machine_refinement: ExactBindingV1,
    result: ExactBindingV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExactBindingV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

/// Durable pre-readiness journal for one completed V5 result.
///
/// This record is committed before load readiness. It binds the complete result to the exact V2
/// envelope and build attempt, allowing a restart to finish the carrier after the readiness receipt
/// exists. It is authority-free and cannot be consumed by a host directly.
#[derive(Debug)]
#[must_use = "native V5 completion must reach the final carrier or remain recoverable"]
pub struct WorkerV3PendingCapabilityResultV1 {
    envelope: ExactBindingV1,
    attempt: BuildAttempt,
    result: InertProductionCapabilityResultV5,
    canonical_bytes: Box<[u8]>,
}

impl WorkerV3PendingCapabilityResultV1 {
    /// Binds a completed result to a live, current V2 envelope before readiness is published.
    pub fn new(
        envelope: &WorkerV3LoadEnvelopeV2,
        result: InertProductionCapabilityResultV5,
    ) -> Result<Self, WorkerV3CapabilityResultCarrierErrorV1> {
        let exact = envelope.encode_canonical()?;
        let binding = envelope_binding(&exact)?;
        let attempt = envelope.wire().published_claim().plan().attempt();
        validate_result_for_envelope(envelope.wire(), &result)?;
        Self::from_parts(binding, attempt, result)
    }

    fn from_parts(
        envelope: ExactBindingV1,
        attempt: BuildAttempt,
        result: InertProductionCapabilityResultV5,
    ) -> Result<Self, WorkerV3CapabilityResultCarrierErrorV1> {
        let canonical_bytes = encode_pending(envelope, attempt, result.canonical_bytes())?;
        Ok(Self {
            envelope,
            attempt,
            result,
            canonical_bytes: canonical_bytes.into_boxed_slice(),
        })
    }

    /// Commits the pre-readiness journal with redo recovery and exact retry matching.
    pub fn persist_durable_journal_v1(
        &self,
        directory: &RetainedDurableDirectoryV1,
    ) -> Result<(), WorkerV3CapabilityResultCarrierErrorV1> {
        let names = DurableNames::new(AttemptKey::from_attempt(self.attempt));
        persist_immutable_record(
            directory,
            &names.pending,
            &names.pending_redo,
            &self.canonical_bytes,
            MAX_WORKER_V3_PENDING_CAPABILITY_RESULT_BYTES_V1,
        )
    }

    /// Finishes the final carrier after readiness is durable for the same live envelope.
    pub fn finish_live(
        self,
        envelope: &WorkerV3LoadEnvelopeV2,
        readiness: &WorkerV3LoadReadinessResultV1,
    ) -> Result<WorkerV3CapabilityResultCarrierV1, WorkerV3CapabilityResultCarrierErrorV1> {
        let exact = envelope.encode_canonical()?;
        self.require_binding(envelope_binding(&exact)?, envelope.wire())?;
        WorkerV3CapabilityResultCarrierV1::new(envelope, readiness, self.result)
    }

    /// Finishes the final carrier from restart-recovered readiness and current publication custody.
    pub fn finish_recovered(
        self,
        envelope: &RecoveredWorkerV3LoadEnvelopeV2,
    ) -> Result<WorkerV3CapabilityResultCarrierV1, WorkerV3CapabilityResultCarrierErrorV1> {
        let evidence = envelope.canonical_evidence_view();
        let binding = ExactBindingV1 {
            sha256: evidence.binding().sha256(),
            byte_len: evidence.binding().byte_length(),
        };
        self.require_binding(binding, envelope.wire())?;
        let coordinates = derive_coordinates(
            envelope.wire(),
            evidence.binding(),
            envelope.receipt(),
            &self.result,
        )?;
        Ok(WorkerV3CapabilityResultCarrierV1 {
            wire: WorkerV3CapabilityResultCarrierWireV1::from_coordinates(
                coordinates,
                self.result,
            )?,
        })
    }

    fn require_binding(
        &self,
        envelope: ExactBindingV1,
        wire: &crate::WorkerV3LoadEnvelopeWireV2,
    ) -> Result<(), WorkerV3CapabilityResultCarrierErrorV1> {
        if self.envelope != envelope || self.attempt != wire.published_claim().plan().attempt() {
            return binding_mismatch(WorkerV3CapabilityResultBindingFieldV1::LoadEnvelope);
        }
        validate_result_for_envelope(wire, &self.result)
    }

    pub const fn production_result(&self) -> &InertProductionCapabilityResultV5 {
        &self.result
    }

    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn envelope_binding(
    exact: &[u8],
) -> Result<ExactBindingV1, WorkerV3CapabilityResultCarrierErrorV1> {
    let binding = WorkerV3LoadEnvelopeBindingV1::from_exact_bytes(exact)?;
    Ok(ExactBindingV1 {
        sha256: binding.sha256(),
        byte_len: binding.byte_length(),
    })
}

/// Authority-free decoded sidecar retaining the exact completed result bytes.
#[derive(Debug)]
pub struct WorkerV3CapabilityResultCarrierWireV1 {
    coordinates: CarrierCoordinatesV1,
    result: InertProductionCapabilityResultV5,
    identity: WorkerV3CapabilityResultCarrierIdentityV1,
    canonical_bytes: Box<[u8]>,
}

impl WorkerV3CapabilityResultCarrierWireV1 {
    /// Constructs the sole carrier form from an already persisted exact V2 envelope.
    pub fn new(
        envelope: &WorkerV3LoadEnvelopeV2,
        readiness: &WorkerV3LoadReadinessResultV1,
        result: InertProductionCapabilityResultV5,
    ) -> Result<Self, WorkerV3CapabilityResultCarrierErrorV1> {
        let exact_v2 = envelope.encode_canonical()?;
        if readiness.exact_envelope_bytes() != exact_v2 {
            return binding_mismatch(WorkerV3CapabilityResultBindingFieldV1::LoadEnvelope);
        }
        let binding = WorkerV3LoadEnvelopeBindingV1::from_exact_bytes(&exact_v2)?;
        if readiness.receipt().envelope_binding() != binding {
            return binding_mismatch(WorkerV3CapabilityResultBindingFieldV1::LoadReadinessReceipt);
        }
        let coordinates =
            derive_coordinates(envelope.wire(), binding, readiness.receipt(), &result)?;
        Self::from_coordinates(coordinates, result)
    }

    fn from_coordinates(
        coordinates: CarrierCoordinatesV1,
        result: InertProductionCapabilityResultV5,
    ) -> Result<Self, WorkerV3CapabilityResultCarrierErrorV1> {
        validate_result_coordinates(&coordinates, &result)?;
        let canonical_bytes = encode_carrier(coordinates, result.canonical_bytes())?;
        let identity = carrier_identity(&canonical_bytes)?;
        Ok(Self {
            coordinates,
            result,
            identity,
            canonical_bytes: canonical_bytes.into_boxed_slice(),
        })
    }

    /// Strictly decodes the complete sidecar and independently rechecks all result-owned fields.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, WorkerV3CapabilityResultCarrierErrorV1> {
        let (coordinates, result_bytes) = decode_carrier_sections(bytes)?;
        let result = InertProductionCapabilityResultV5::decode(result_bytes)?;
        if result.canonical_bytes() != result_bytes {
            return Err(WorkerV3CapabilityResultCarrierErrorV1::NoncanonicalResult);
        }
        let decoded = Self::from_coordinates(coordinates, result)?;
        if decoded.canonical_bytes() != bytes {
            return Err(WorkerV3CapabilityResultCarrierErrorV1::IdentityMismatch);
        }
        Ok(decoded)
    }

    pub const fn identity(&self) -> WorkerV3CapabilityResultCarrierIdentityV1 {
        self.identity
    }

    pub const fn production_result(&self) -> &InertProductionCapabilityResultV5 {
        &self.result
    }

    pub const fn production_result_identity(&self) -> InertProductionCapabilityResultIdentityV5 {
        self.result.identity()
    }

    /// Returns the exact managed build attempt encoded by this carrier.
    ///
    /// This coordinate lets an external recovery owner reacquire the matching frozen V2 load
    /// envelope before consuming the carrier. It is inert and grants no build, load, or launch
    /// authority.
    pub fn attempt(&self) -> BuildAttempt {
        BuildAttempt::from_env_value(&format!(
            "{}:{}:{}",
            self.coordinates.generation,
            hex(&self.coordinates.session),
            hex(&self.coordinates.invocation),
        ))
        .expect("a decoded carrier contains a valid canonical build attempt")
    }

    pub const fn dynamic_precondition_roster_identity(
        &self,
    ) -> WorkerV3DynamicPreconditionRosterIdentityV1 {
        WorkerV3DynamicPreconditionRosterIdentityV1(self.coordinates.dynamic_preconditions)
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn grants_verification_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn decode_carrier_sections(
    bytes: &[u8],
) -> Result<(CarrierCoordinatesV1, &[u8]), WorkerV3CapabilityResultCarrierErrorV1> {
    if bytes.len() < FIXED_OVERHEAD_BYTES + 1 {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::Truncated);
    }
    if bytes.len() > MAX_WORKER_V3_CAPABILITY_RESULT_CARRIER_BYTES_V1 {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::WireLength {
            actual: bytes.len(),
            maximum: MAX_WORKER_V3_CAPABILITY_RESULT_CARRIER_BYTES_V1,
        });
    }
    let checksum_offset = bytes
        .len()
        .checked_sub(CHECKSUM_BYTES)
        .ok_or(WorkerV3CapabilityResultCarrierErrorV1::Truncated)?;
    let identity_offset = checksum_offset
        .checked_sub(TERMINAL_IDENTITY_BYTES)
        .ok_or(WorkerV3CapabilityResultCarrierErrorV1::Truncated)?;
    let (body_and_identity, declared_checksum) = bytes.split_at(checksum_offset);
    let (body, declared_identity) = body_and_identity.split_at(identity_offset);
    if domain_hash(CARRIER_CHECKSUM_DOMAIN, body_and_identity).as_slice() != declared_checksum {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::ChecksumMismatch);
    }
    if domain_hash(CARRIER_IDENTITY_DOMAIN, body).as_slice() != declared_identity {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::IdentityMismatch);
    }
    let mut reader = Reader::new(body);
    if reader.array::<8>()? != WORKER_V3_CAPABILITY_RESULT_CARRIER_MAGIC_V1 {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::BadMagic);
    }
    let version = reader.u16()?;
    if version != WORKER_V3_CAPABILITY_RESULT_CARRIER_VERSION_V1 {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::UnsupportedVersion { actual: version });
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::UnsupportedFlags { actual: flags });
    }
    let total = reader.u64()?;
    if total != bytes.len() as u64 {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::InvalidTotalLength {
            declared: total,
            actual: bytes.len(),
        });
    }
    let result_len = reader.u64()?;
    let reserved = reader.u32()?;
    if reserved != 0 {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::UnsupportedFlags { actual: u16::MAX });
    }
    require_result_length(result_len)?;
    let coordinates = decode_coordinates(&mut reader)?;
    let result_len = usize::try_from(result_len)
        .map_err(|_| WorkerV3CapabilityResultCarrierErrorV1::LengthOverflow)?;
    let result_bytes = reader.take(result_len)?;
    if !reader.is_empty() {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::TrailingBytes);
    }
    Ok((coordinates, result_bytes))
}

/// Live authority-free carrier ready for descriptor-relative durable publication.
#[derive(Debug)]
#[must_use = "the completed capability result must remain joined to V2 durable custody"]
pub struct WorkerV3CapabilityResultCarrierV1 {
    wire: WorkerV3CapabilityResultCarrierWireV1,
}

impl WorkerV3CapabilityResultCarrierV1 {
    pub fn new(
        envelope: &WorkerV3LoadEnvelopeV2,
        readiness: &WorkerV3LoadReadinessResultV1,
        result: InertProductionCapabilityResultV5,
    ) -> Result<Self, WorkerV3CapabilityResultCarrierErrorV1> {
        Ok(Self {
            wire: WorkerV3CapabilityResultCarrierWireV1::new(envelope, readiness, result)?,
        })
    }

    pub const fn wire(&self) -> &WorkerV3CapabilityResultCarrierWireV1 {
        &self.wire
    }

    /// Durably commits the exact sidecar in the supervisor-retained private output root.
    pub fn persist_durable_sidecar_v1(
        &self,
        directory: &RetainedDurableDirectoryV1,
    ) -> Result<WorkerV3CapabilityResultCarrierIdentityV1, WorkerV3CapabilityResultCarrierErrorV1>
    {
        let attempt = self.wire.coordinates.attempt_key();
        let names = DurableNames::new(attempt);
        persist_immutable_record(
            directory,
            &names.carrier,
            &names.carrier_redo,
            self.wire.canonical_bytes(),
            MAX_WORKER_V3_CAPABILITY_RESULT_CARRIER_BYTES_V1,
        )?;
        Ok(self.wire.identity())
    }
}

/// Restart-recovered, not-yet-consumed carrier custody.
#[derive(Debug)]
#[must_use = "sealed host admission must consume recovered completed-result custody exactly once"]
pub struct RecoveredWorkerV3CapabilityResultCarrierV1 {
    wire: WorkerV3CapabilityResultCarrierWireV1,
}

impl RecoveredWorkerV3CapabilityResultCarrierV1 {
    pub const fn wire(&self) -> &WorkerV3CapabilityResultCarrierWireV1 {
        &self.wire
    }

    pub const fn production_result(&self) -> &InertProductionCapabilityResultV5 {
        self.wire.production_result()
    }

    pub const fn identity(&self) -> WorkerV3CapabilityResultCarrierIdentityV1 {
        self.wire.identity()
    }

    pub const fn dynamic_precondition_roster_identity(
        &self,
    ) -> WorkerV3DynamicPreconditionRosterIdentityV1 {
        self.wire.dynamic_precondition_roster_identity()
    }

    /// Commits the durable one-shot tombstone selected by successful sealed host admission.
    ///
    /// The returned owner remains authority-free; only the host's sealed typestate may interpret
    /// the supplied admission identity. Failure retains this complete recovery owner.
    pub fn commit_sealed_host_consumption_v1(
        self,
        directory: &RetainedDurableDirectoryV1,
        sealed_host_admission_identity: [u8; 32],
    ) -> Result<
        ConsumedWorkerV3CapabilityResultCarrierV1,
        RecoverableWorkerV3CapabilityResultConsumptionErrorV1,
    > {
        self.commit_consumption_v1(directory, sealed_host_admission_identity)
    }

    /// Commits the same durable one-shot tombstone for compiler-owned tutorial qualification.
    ///
    /// Host admission and qualification intentionally share one marker: a completed result cannot
    /// be consumed once for launch and again for archival publication (or vice versa).
    pub fn commit_tutorial_qualification_consumption_v1(
        self,
        directory: &RetainedDurableDirectoryV1,
        request_binding_identity: [u8; 32],
    ) -> Result<
        ConsumedWorkerV3CapabilityResultCarrierV1,
        RecoverableWorkerV3CapabilityResultConsumptionErrorV1,
    > {
        self.commit_consumption_v1(directory, request_binding_identity)
    }

    fn commit_consumption_v1(
        self,
        directory: &RetainedDurableDirectoryV1,
        consumer_identity: [u8; 32],
    ) -> Result<
        ConsumedWorkerV3CapabilityResultCarrierV1,
        RecoverableWorkerV3CapabilityResultConsumptionErrorV1,
    > {
        if consumer_identity == [0; 32] {
            return Err(RecoverableWorkerV3CapabilityResultConsumptionErrorV1::new(
                WorkerV3CapabilityResultCarrierErrorV1::ZeroSealedAdmissionIdentity,
                self,
            ));
        }
        let names = DurableNames::new(self.wire.coordinates.attempt_key());
        let operation = (|| {
            directory.verify_exact(&names.carrier, self.wire.canonical_bytes())?;
            if let Some(consumption) = directory.read_private(&names.consumed, CONSUMPTION_BYTES)? {
                if decode_consumption(&consumption)? != self.identity() {
                    return Err(WorkerV3CapabilityResultCarrierErrorV1::InvalidConsumptionRecord);
                }
                return Err(WorkerV3CapabilityResultCarrierErrorV1::AlreadyConsumed);
            }
            if let Some(consumption) =
                directory.read_private(&names.consumed_redo, CONSUMPTION_BYTES)?
            {
                if decode_consumption(&consumption)? != self.identity() {
                    return Err(WorkerV3CapabilityResultCarrierErrorV1::InvalidConsumptionRecord);
                }
                return Err(WorkerV3CapabilityResultCarrierErrorV1::AlreadyConsumed);
            }
            let consumption = encode_consumption(self.identity(), consumer_identity);
            let mut hooks = NoRetainedDurableDirectoryHooksV1;
            directory.commit_record(
                &names.consumed,
                &names.consumed_redo,
                &consumption,
                CONSUMPTION_BYTES,
                &mut hooks,
            )?;
            Ok(())
        })();
        match operation {
            Ok(()) => Ok(ConsumedWorkerV3CapabilityResultCarrierV1 {
                wire: self.wire,
                sealed_host_admission_identity: consumer_identity,
            }),
            Err(error) => Err(RecoverableWorkerV3CapabilityResultConsumptionErrorV1::new(
                error, self,
            )),
        }
    }
}

/// Durable one-shot carrier state retained by the sealed host typestate.
#[derive(Debug)]
#[must_use = "consumed result custody must remain with the authenticated host admission"]
pub struct ConsumedWorkerV3CapabilityResultCarrierV1 {
    wire: WorkerV3CapabilityResultCarrierWireV1,
    sealed_host_admission_identity: [u8; 32],
}

impl ConsumedWorkerV3CapabilityResultCarrierV1 {
    pub const fn wire(&self) -> &WorkerV3CapabilityResultCarrierWireV1 {
        &self.wire
    }

    pub const fn production_result(&self) -> &InertProductionCapabilityResultV5 {
        self.wire.production_result()
    }

    pub const fn sealed_host_admission_identity(&self) -> [u8; 32] {
        self.sealed_host_admission_identity
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Consumption failure retaining the exact unconsumed owner for retry or quarantine.
#[derive(Debug)]
pub struct RecoverableWorkerV3CapabilityResultConsumptionErrorV1 {
    error: WorkerV3CapabilityResultCarrierErrorV1,
    custody: RecoveredWorkerV3CapabilityResultCarrierV1,
}

impl RecoverableWorkerV3CapabilityResultConsumptionErrorV1 {
    fn new(
        error: WorkerV3CapabilityResultCarrierErrorV1,
        custody: RecoveredWorkerV3CapabilityResultCarrierV1,
    ) -> Self {
        Self { error, custody }
    }

    pub const fn error(&self) -> &WorkerV3CapabilityResultCarrierErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        WorkerV3CapabilityResultCarrierErrorV1,
        RecoveredWorkerV3CapabilityResultCarrierV1,
    ) {
        (self.error, self.custody)
    }
}

/// Recovers the sidecar only for the exact current V2 envelope and rejects consumed custody.
pub fn recover_worker_v3_capability_result_carrier_v1(
    directory: &RetainedDurableDirectoryV1,
    envelope: &RecoveredWorkerV3LoadEnvelopeV2,
) -> Result<RecoveredWorkerV3CapabilityResultCarrierV1, WorkerV3CapabilityResultCarrierErrorV1> {
    envelope
        .wire()
        .validate_reacquired_publication_lease_v2(envelope.current_publication_lease())?;
    let attempt = envelope.wire().published_claim().plan().attempt();
    let names = DurableNames::new(AttemptKey::from_attempt(attempt));
    let current = directory.read_private(
        &names.carrier,
        MAX_WORKER_V3_CAPABILITY_RESULT_CARRIER_BYTES_V1,
    )?;
    let redo = directory.read_private(
        &names.carrier_redo,
        MAX_WORKER_V3_CAPABILITY_RESULT_CARRIER_BYTES_V1,
    )?;
    let exact = match (current.as_deref(), redo.as_deref()) {
        (Some(current), Some(redo)) if current != redo => {
            return Err(WorkerV3CapabilityResultCarrierErrorV1::DurableCarrierConflict);
        }
        (Some(current), _) => current,
        (None, Some(redo)) => redo,
        (None, None) => {
            return Err(WorkerV3CapabilityResultCarrierErrorV1::MissingDurableCarrier);
        }
    };
    let wire = WorkerV3CapabilityResultCarrierWireV1::decode_canonical(exact)?;
    validate_recovered_association(&wire, envelope)?;
    // Admit and bind the redo bytes before promotion. A crash after redo fsync but before rename
    // must resume the same carrier, while a substituted or conflicting redo fails closed.
    persist_immutable_record(
        directory,
        &names.carrier,
        &names.carrier_redo,
        exact,
        MAX_WORKER_V3_CAPABILITY_RESULT_CARRIER_BYTES_V1,
    )?;
    if let Some(consumption) = directory.read_private(&names.consumed, CONSUMPTION_BYTES)? {
        let consumed_identity = decode_consumption(&consumption)?;
        if consumed_identity != wire.identity() {
            return Err(WorkerV3CapabilityResultCarrierErrorV1::InvalidConsumptionRecord);
        }
        return Err(WorkerV3CapabilityResultCarrierErrorV1::AlreadyConsumed);
    }
    if let Some(consumption) = directory.read_private(&names.consumed_redo, CONSUMPTION_BYTES)? {
        if decode_consumption(&consumption)? != wire.identity() {
            return Err(WorkerV3CapabilityResultCarrierErrorV1::InvalidConsumptionRecord);
        }
        return Err(WorkerV3CapabilityResultCarrierErrorV1::AlreadyConsumed);
    }
    Ok(RecoveredWorkerV3CapabilityResultCarrierV1 { wire })
}

/// Recovers the exact pre-readiness journal for a live current envelope.
pub fn recover_worker_v3_pending_capability_result_for_live_v1(
    directory: &RetainedDurableDirectoryV1,
    envelope: &WorkerV3LoadEnvelopeV2,
) -> Result<WorkerV3PendingCapabilityResultV1, WorkerV3CapabilityResultCarrierErrorV1> {
    let exact = envelope.encode_canonical()?;
    recover_pending(
        directory,
        envelope.wire().published_claim().plan().attempt(),
        envelope_binding(&exact)?,
        envelope.wire(),
    )
}

/// Recovers the exact pre-readiness journal for restart-recovered envelope custody.
pub fn recover_worker_v3_pending_capability_result_v1(
    directory: &RetainedDurableDirectoryV1,
    envelope: &RecoveredWorkerV3LoadEnvelopeV2,
) -> Result<WorkerV3PendingCapabilityResultV1, WorkerV3CapabilityResultCarrierErrorV1> {
    let evidence = envelope.canonical_evidence_view();
    recover_pending(
        directory,
        envelope.wire().published_claim().plan().attempt(),
        ExactBindingV1 {
            sha256: evidence.binding().sha256(),
            byte_len: evidence.binding().byte_length(),
        },
        envelope.wire(),
    )
}

fn recover_pending(
    directory: &RetainedDurableDirectoryV1,
    attempt: BuildAttempt,
    envelope: ExactBindingV1,
    wire: &crate::WorkerV3LoadEnvelopeWireV2,
) -> Result<WorkerV3PendingCapabilityResultV1, WorkerV3CapabilityResultCarrierErrorV1> {
    let names = DurableNames::new(AttemptKey::from_attempt(attempt));
    let current = directory.read_private(
        &names.pending,
        MAX_WORKER_V3_PENDING_CAPABILITY_RESULT_BYTES_V1,
    )?;
    let redo = directory.read_private(
        &names.pending_redo,
        MAX_WORKER_V3_PENDING_CAPABILITY_RESULT_BYTES_V1,
    )?;
    let bytes = match (current.as_deref(), redo.as_deref()) {
        (Some(current), Some(redo)) if current != redo => {
            return Err(WorkerV3CapabilityResultCarrierErrorV1::DurableCarrierConflict);
        }
        (Some(current), _) => current,
        (None, Some(redo)) => redo,
        (None, None) => {
            return Err(WorkerV3CapabilityResultCarrierErrorV1::MissingDurablePendingResult);
        }
    };
    let pending = decode_pending(bytes, attempt)?;
    pending.require_binding(envelope, wire)?;
    // Validate before promoting a redo. Once validated, the common immutable-record recovery path
    // resolves redo-only, canonical-only, and equal canonical+redo crash states durably.
    persist_immutable_record(
        directory,
        &names.pending,
        &names.pending_redo,
        bytes,
        MAX_WORKER_V3_PENDING_CAPABILITY_RESULT_BYTES_V1,
    )?;
    Ok(pending)
}

fn validate_result_for_envelope(
    envelope: &crate::WorkerV3LoadEnvelopeWireV2,
    result: &InertProductionCapabilityResultV5,
) -> Result<(), WorkerV3CapabilityResultCarrierErrorV1> {
    let replay = envelope.replay();
    let attempt = replay.published_claim().plan().attempt();
    let handoff = result.handoff();
    if handoff.legacy_handoff().canonical_bytes() != replay.outer_handoff() {
        return binding_mismatch(WorkerV3CapabilityResultBindingFieldV1::LegacyCompilerHandoff);
    }
    let publication = replay.published_claim().worker_v3_binding();
    if result.object_output().output_sha256() != publication.raw_output_sha256()
        || result.object_output().output_bytes() != publication.raw_output_length()
    {
        return binding_mismatch(WorkerV3CapabilityResultBindingFieldV1::RawCompilerObject);
    }
    let reconstructed = envelope.reconstructed_compiler_execution_subject_v1()?;
    if reconstructed.attempt() != attempt
        || envelope.compiler_execution_receipt().request().subject() != &reconstructed
    {
        return binding_mismatch(WorkerV3CapabilityResultBindingFieldV1::CompilerSubject);
    }
    if handoff.inputs().compiler_policy()
        != *envelope
            .compiler_execution_receipt()
            .policy()
            .identity()
            .as_bytes()
    {
        return binding_mismatch(WorkerV3CapabilityResultBindingFieldV1::CompilerPolicy);
    }
    Ok(())
}

fn derive_coordinates(
    envelope: &crate::WorkerV3LoadEnvelopeWireV2,
    load_envelope: WorkerV3LoadEnvelopeBindingV1,
    readiness: fe2o3_artifact_transaction::WorkerV3LoadReadinessReceiptV1,
    result: &InertProductionCapabilityResultV5,
) -> Result<CarrierCoordinatesV1, WorkerV3CapabilityResultCarrierErrorV1> {
    validate_result_for_envelope(envelope, result)?;
    let replay = envelope.replay();
    let attempt = replay.published_claim().plan().attempt();
    if readiness.attempt() != attempt {
        return binding_mismatch(WorkerV3CapabilityResultBindingFieldV1::Attempt);
    }
    let result_handoff = result.handoff();
    let publication = replay.published_claim().worker_v3_binding();
    let reconstructed = envelope.reconstructed_compiler_execution_subject_v1()?;
    let carriage = envelope.compiler_execution_receipt();
    let ack = carriage.acknowledgment();
    let result_identity = result.identity();
    Ok(CarrierCoordinatesV1 {
        load_envelope: ExactBindingV1 {
            sha256: load_envelope.sha256(),
            byte_len: load_envelope.byte_length(),
        },
        readiness_receipt: readiness.identity()?,
        generation: attempt.generation(),
        session: *attempt.session().as_bytes(),
        invocation: *attempt.invocation().as_bytes(),
        raw_object: ExactBindingV1 {
            sha256: publication.raw_output_sha256(),
            byte_len: publication.raw_output_length(),
        },
        finalized_artifact: ExactBindingV1 {
            sha256: publication.finalized_output_sha256(),
            byte_len: publication.finalized_output_length(),
        },
        target_model: *result_handoff
            .target_closure()
            .target_model()
            .digest()
            .as_bytes(),
        launch_roster: *result_handoff
            .target_closure()
            .launch_contract()
            .digest()
            .as_bytes(),
        subject_root_roster: derive_subject_root_roster(result),
        dynamic_preconditions: derive_dynamic_preconditions(result),
        compiler_subject: *reconstructed.identity().sha256(),
        compiler_carriage: *carriage.identity().as_bytes(),
        compiler_policy: *carriage.policy().identity().as_bytes(),
        compiler_occurrence: ack.compiler_occurrence_identity(),
        worker_ledger_record: ack.worker_ledger_record_identity(),
        compiler_sequence: ack.sequence(),
        current_rollback_anchor: ack.current_rollback_anchor(),
        source_refinement: ExactBindingV1 {
            sha256: result_handoff.source_refinement().identity().sha256(),
            byte_len: result_handoff.source_refinement().identity().byte_len(),
        },
        machine_refinement: ExactBindingV1 {
            sha256: result.machine_refinement().identity().sha256(),
            byte_len: result.machine_refinement().identity().byte_len(),
        },
        result: ExactBindingV1 {
            sha256: result_identity.sha256(),
            byte_len: result_identity.byte_len(),
        },
    })
}

fn validate_recovered_association(
    wire: &WorkerV3CapabilityResultCarrierWireV1,
    envelope: &RecoveredWorkerV3LoadEnvelopeV2,
) -> Result<(), WorkerV3CapabilityResultCarrierErrorV1> {
    let evidence = envelope.canonical_evidence_view();
    let receipt = envelope.receipt();
    let expected = derive_coordinates(
        envelope.wire(),
        evidence.binding(),
        receipt,
        wire.production_result(),
    )?;
    if evidence.exact_canonical_bytes().len() as u64 != wire.coordinates.load_envelope.byte_len
        || expected != wire.coordinates
    {
        return binding_mismatch(WorkerV3CapabilityResultBindingFieldV1::LoadEnvelope);
    }
    Ok(())
}

fn validate_result_coordinates(
    coordinates: &CarrierCoordinatesV1,
    result: &InertProductionCapabilityResultV5,
) -> Result<(), WorkerV3CapabilityResultCarrierErrorV1> {
    let handoff = result.handoff();
    let checks = [
        (
            coordinates.raw_object
                == ExactBindingV1 {
                    sha256: result.object_output().output_sha256(),
                    byte_len: result.object_output().output_bytes(),
                },
            WorkerV3CapabilityResultBindingFieldV1::RawCompilerObject,
        ),
        (
            coordinates.target_model
                == *handoff.target_closure().target_model().digest().as_bytes(),
            WorkerV3CapabilityResultBindingFieldV1::TargetModel,
        ),
        (
            coordinates.launch_roster
                == *handoff
                    .target_closure()
                    .launch_contract()
                    .digest()
                    .as_bytes(),
            WorkerV3CapabilityResultBindingFieldV1::LaunchRoster,
        ),
        (
            coordinates.subject_root_roster == derive_subject_root_roster(result),
            WorkerV3CapabilityResultBindingFieldV1::SubjectRootRoster,
        ),
        (
            coordinates.dynamic_preconditions == derive_dynamic_preconditions(result),
            WorkerV3CapabilityResultBindingFieldV1::DynamicPreconditions,
        ),
        (
            coordinates.compiler_policy == handoff.inputs().compiler_policy(),
            WorkerV3CapabilityResultBindingFieldV1::CompilerPolicy,
        ),
        (
            coordinates.source_refinement
                == ExactBindingV1 {
                    sha256: handoff.source_refinement().identity().sha256(),
                    byte_len: handoff.source_refinement().identity().byte_len(),
                },
            WorkerV3CapabilityResultBindingFieldV1::SourceRefinement,
        ),
        (
            coordinates.machine_refinement
                == ExactBindingV1 {
                    sha256: result.machine_refinement().identity().sha256(),
                    byte_len: result.machine_refinement().identity().byte_len(),
                },
            WorkerV3CapabilityResultBindingFieldV1::MachineRefinement,
        ),
        (
            coordinates.result
                == ExactBindingV1 {
                    sha256: result.identity().sha256(),
                    byte_len: result.identity().byte_len(),
                },
            WorkerV3CapabilityResultBindingFieldV1::ProductionResult,
        ),
    ];
    for (matches, field) in checks {
        if !matches {
            return binding_mismatch(field);
        }
    }
    Ok(())
}

fn derive_subject_root_roster(result: &InertProductionCapabilityResultV5) -> [u8; 32] {
    let subjects = result.handoff().subjects();
    let mut digest = Sha256::new();
    digest.update(SUBJECT_ROSTER_DOMAIN);
    digest.update((subjects.len() as u64).to_le_bytes());
    for subject in subjects {
        for identity in [
            subject.kernel().digest(),
            subject.root().digest(),
            subject.executable_kir().digest(),
            subject.target_model().digest(),
            subject.launch_contract().digest(),
        ] {
            digest.update(identity.as_bytes());
        }
        digest.update(subject.executable_kir_epoch().to_le_bytes());
    }
    digest.finalize().into()
}

fn derive_dynamic_preconditions(result: &InertProductionCapabilityResultV5) -> [u8; 32] {
    let obligations = result.handoff().obligation_roster();
    let mut digest = Sha256::new();
    digest.update(DYNAMIC_PRECONDITION_ROSTER_DOMAIN);
    digest.update((obligations.len() as u64).to_le_bytes());
    for set in obligations {
        // The complete obligation-set bytes are retained because this layer must not reinterpret
        // or statically discharge per-dispatch requirements. The domain makes this identity the
        // exact dynamic-precondition input roster consumed by later W7 checks.
        digest.update(set.identity().digest().as_bytes());
        digest.update((set.canonical_bytes().len() as u64).to_le_bytes());
        digest.update(set.canonical_bytes());
    }
    digest.finalize().into()
}

fn encode_carrier(
    coordinates: CarrierCoordinatesV1,
    result: &[u8],
) -> Result<Vec<u8>, WorkerV3CapabilityResultCarrierErrorV1> {
    require_result_length(result.len() as u64)?;
    let total = result
        .len()
        .checked_add(FIXED_OVERHEAD_BYTES)
        .ok_or(WorkerV3CapabilityResultCarrierErrorV1::LengthOverflow)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(total).map_err(|_| {
        WorkerV3CapabilityResultCarrierErrorV1::AllocationFailed { requested: total }
    })?;
    bytes.extend_from_slice(&WORKER_V3_CAPABILITY_RESULT_CARRIER_MAGIC_V1);
    bytes.extend_from_slice(&WORKER_V3_CAPABILITY_RESULT_CARRIER_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&(total as u64).to_le_bytes());
    bytes.extend_from_slice(&(result.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    encode_coordinates(&mut bytes, coordinates);
    bytes.extend_from_slice(result);
    debug_assert_eq!(
        bytes.len(),
        total - TERMINAL_IDENTITY_BYTES - CHECKSUM_BYTES
    );
    let identity = domain_hash(CARRIER_IDENTITY_DOMAIN, &bytes);
    bytes.extend_from_slice(&identity);
    let checksum = domain_hash(CARRIER_CHECKSUM_DOMAIN, &bytes);
    bytes.extend_from_slice(&checksum);
    debug_assert_eq!(bytes.len(), total);
    Ok(bytes)
}

fn encode_pending(
    envelope: ExactBindingV1,
    attempt: BuildAttempt,
    result: &[u8],
) -> Result<Vec<u8>, WorkerV3CapabilityResultCarrierErrorV1> {
    require_result_length(result.len() as u64)?;
    let total = result
        .len()
        .checked_add(PENDING_FIXED_OVERHEAD_BYTES)
        .ok_or(WorkerV3CapabilityResultCarrierErrorV1::LengthOverflow)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(total).map_err(|_| {
        WorkerV3CapabilityResultCarrierErrorV1::AllocationFailed { requested: total }
    })?;
    bytes.extend_from_slice(&PENDING_MAGIC);
    bytes.extend_from_slice(&PENDING_VERSION.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&(total as u64).to_le_bytes());
    push_binding(&mut bytes, envelope);
    bytes.extend_from_slice(&attempt.generation().to_le_bytes());
    bytes.extend_from_slice(attempt.session().as_bytes());
    bytes.extend_from_slice(attempt.invocation().as_bytes());
    bytes.extend_from_slice(&(result.len() as u64).to_le_bytes());
    bytes.extend_from_slice(result);
    debug_assert_eq!(
        bytes.len(),
        total - PENDING_IDENTITY_BYTES - PENDING_CHECKSUM_BYTES
    );
    let identity = domain_hash(PENDING_IDENTITY_DOMAIN, &bytes);
    bytes.extend_from_slice(&identity);
    let checksum = domain_hash(PENDING_CHECKSUM_DOMAIN, &bytes);
    bytes.extend_from_slice(&checksum);
    debug_assert_eq!(bytes.len(), total);
    Ok(bytes)
}

fn decode_pending(
    bytes: &[u8],
    expected_attempt: BuildAttempt,
) -> Result<WorkerV3PendingCapabilityResultV1, WorkerV3CapabilityResultCarrierErrorV1> {
    let (envelope, result_bytes) = decode_pending_sections(bytes, expected_attempt)?;
    let result = InertProductionCapabilityResultV5::decode(result_bytes)?;
    if result.canonical_bytes() != result_bytes {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::NoncanonicalResult);
    }
    let decoded =
        WorkerV3PendingCapabilityResultV1::from_parts(envelope, expected_attempt, result)?;
    if decoded.canonical_bytes.as_ref() != bytes {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::IdentityMismatch);
    }
    Ok(decoded)
}

fn decode_pending_sections(
    bytes: &[u8],
    expected_attempt: BuildAttempt,
) -> Result<(ExactBindingV1, &[u8]), WorkerV3CapabilityResultCarrierErrorV1> {
    if bytes.len() < PENDING_FIXED_OVERHEAD_BYTES + 1 {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::Truncated);
    }
    if bytes.len() > MAX_WORKER_V3_PENDING_CAPABILITY_RESULT_BYTES_V1 {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::WireLength {
            actual: bytes.len(),
            maximum: MAX_WORKER_V3_PENDING_CAPABILITY_RESULT_BYTES_V1,
        });
    }
    let checksum_offset = bytes
        .len()
        .checked_sub(PENDING_CHECKSUM_BYTES)
        .ok_or(WorkerV3CapabilityResultCarrierErrorV1::Truncated)?;
    let identity_offset = checksum_offset
        .checked_sub(PENDING_IDENTITY_BYTES)
        .ok_or(WorkerV3CapabilityResultCarrierErrorV1::Truncated)?;
    let (body_and_identity, declared_checksum) = bytes.split_at(checksum_offset);
    let (body, declared_identity) = body_and_identity.split_at(identity_offset);
    if domain_hash(PENDING_CHECKSUM_DOMAIN, body_and_identity).as_slice() != declared_checksum {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::ChecksumMismatch);
    }
    if domain_hash(PENDING_IDENTITY_DOMAIN, body).as_slice() != declared_identity {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::IdentityMismatch);
    }

    let mut reader = Reader::new(body);
    if reader.array::<8>()? != PENDING_MAGIC {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::BadMagic);
    }
    let version = reader.u16()?;
    if version != PENDING_VERSION {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::UnsupportedVersion { actual: version });
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::UnsupportedFlags { actual: flags });
    }
    let total = reader.u64()?;
    if total != bytes.len() as u64 {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::InvalidTotalLength {
            declared: total,
            actual: bytes.len(),
        });
    }
    let envelope = reader.binding()?;
    let generation = reader.u64()?;
    let session = reader.array::<16>()?;
    let invocation = reader.array::<32>()?;
    if generation != expected_attempt.generation()
        || session != *expected_attempt.session().as_bytes()
        || invocation != *expected_attempt.invocation().as_bytes()
    {
        return binding_mismatch(WorkerV3CapabilityResultBindingFieldV1::Attempt);
    }
    let result_len = reader.u64()?;
    require_result_length(result_len)?;
    let result_len = usize::try_from(result_len)
        .map_err(|_| WorkerV3CapabilityResultCarrierErrorV1::LengthOverflow)?;
    let result = reader.take(result_len)?;
    if !reader.is_empty() {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::TrailingBytes);
    }
    Ok((envelope, result))
}

fn encode_coordinates(bytes: &mut Vec<u8>, coordinates: CarrierCoordinatesV1) {
    push_binding(bytes, coordinates.load_envelope);
    bytes.extend_from_slice(&coordinates.readiness_receipt);
    bytes.extend_from_slice(&coordinates.generation.to_le_bytes());
    bytes.extend_from_slice(&coordinates.session);
    bytes.extend_from_slice(&coordinates.invocation);
    push_binding(bytes, coordinates.raw_object);
    push_binding(bytes, coordinates.finalized_artifact);
    for identity in [
        coordinates.target_model,
        coordinates.launch_roster,
        coordinates.subject_root_roster,
        coordinates.dynamic_preconditions,
        coordinates.compiler_subject,
        coordinates.compiler_carriage,
        coordinates.compiler_policy,
        coordinates.compiler_occurrence,
        coordinates.worker_ledger_record,
    ] {
        bytes.extend_from_slice(&identity);
    }
    bytes.extend_from_slice(&coordinates.compiler_sequence.to_le_bytes());
    bytes.extend_from_slice(&coordinates.current_rollback_anchor);
    push_binding(bytes, coordinates.source_refinement);
    push_binding(bytes, coordinates.machine_refinement);
    push_binding(bytes, coordinates.result);
}

fn decode_coordinates(
    reader: &mut Reader<'_>,
) -> Result<CarrierCoordinatesV1, WorkerV3CapabilityResultCarrierErrorV1> {
    Ok(CarrierCoordinatesV1 {
        load_envelope: reader.binding()?,
        readiness_receipt: reader.array()?,
        generation: reader.u64()?,
        session: reader.array()?,
        invocation: reader.array()?,
        raw_object: reader.binding()?,
        finalized_artifact: reader.binding()?,
        target_model: reader.array()?,
        launch_roster: reader.array()?,
        subject_root_roster: reader.array()?,
        dynamic_preconditions: reader.array()?,
        compiler_subject: reader.array()?,
        compiler_carriage: reader.array()?,
        compiler_policy: reader.array()?,
        compiler_occurrence: reader.array()?,
        worker_ledger_record: reader.array()?,
        compiler_sequence: reader.u64()?,
        current_rollback_anchor: reader.array()?,
        source_refinement: reader.binding()?,
        machine_refinement: reader.binding()?,
        result: reader.binding()?,
    })
}

fn push_binding(bytes: &mut Vec<u8>, binding: ExactBindingV1) {
    bytes.extend_from_slice(&binding.sha256);
    bytes.extend_from_slice(&binding.byte_len.to_le_bytes());
}

fn carrier_identity(
    canonical_bytes: &[u8],
) -> Result<WorkerV3CapabilityResultCarrierIdentityV1, WorkerV3CapabilityResultCarrierErrorV1> {
    let terminal = canonical_bytes
        .len()
        .checked_sub(CHECKSUM_BYTES + TERMINAL_IDENTITY_BYTES)
        .ok_or(WorkerV3CapabilityResultCarrierErrorV1::Truncated)?;
    Ok(WorkerV3CapabilityResultCarrierIdentityV1 {
        sha256: domain_hash(CARRIER_IDENTITY_DOMAIN, &canonical_bytes[..terminal]),
        byte_len: canonical_bytes.len() as u64,
    })
}

fn require_result_length(length: u64) -> Result<(), WorkerV3CapabilityResultCarrierErrorV1> {
    if length == 0 || length > MAX_INERT_PRODUCTION_CAPABILITY_RESULT_BYTES_V5 as u64 {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::ResultLength {
            actual: length,
            maximum: MAX_INERT_PRODUCTION_CAPABILITY_RESULT_BYTES_V5,
        });
    }
    Ok(())
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    digest.finalize().into()
}

fn binding_mismatch<T>(
    field: WorkerV3CapabilityResultBindingFieldV1,
) -> Result<T, WorkerV3CapabilityResultCarrierErrorV1> {
    Err(WorkerV3CapabilityResultCarrierErrorV1::BindingMismatch(
        field,
    ))
}

#[derive(Clone, Copy)]
struct AttemptKey([u8; 32]);

impl AttemptKey {
    fn from_attempt(attempt: BuildAttempt) -> Self {
        let mut digest = Sha256::new();
        digest.update(DURABLE_NAMESPACE_DOMAIN);
        digest.update(attempt.generation().to_le_bytes());
        digest.update(attempt.session().as_bytes());
        digest.update(attempt.invocation().as_bytes());
        Self(digest.finalize().into())
    }
}

impl CarrierCoordinatesV1 {
    fn attempt_key(self) -> AttemptKey {
        let mut digest = Sha256::new();
        digest.update(DURABLE_NAMESPACE_DOMAIN);
        digest.update(self.generation.to_le_bytes());
        digest.update(self.session);
        digest.update(self.invocation);
        AttemptKey(digest.finalize().into())
    }
}

struct DurableNames {
    carrier: String,
    carrier_redo: String,
    pending: String,
    pending_redo: String,
    consumed: String,
    consumed_redo: String,
}

impl DurableNames {
    fn new(key: AttemptKey) -> Self {
        let key = hex(&key.0);
        Self {
            carrier: format!(".fe2o3-worker-v3-capability-result-v1-{key}.carrier"),
            carrier_redo: format!(".fe2o3-worker-v3-capability-result-v1-{key}.carrier.redo"),
            pending: format!(".fe2o3-worker-v3-capability-result-v1-{key}.pending"),
            pending_redo: format!(".fe2o3-worker-v3-capability-result-v1-{key}.pending.redo"),
            consumed: format!(".fe2o3-worker-v3-capability-result-v1-{key}.consumed"),
            consumed_redo: format!(".fe2o3-worker-v3-capability-result-v1-{key}.consumed.redo"),
        }
    }
}

fn persist_immutable_record(
    directory: &RetainedDurableDirectoryV1,
    canonical: &str,
    redo: &str,
    bytes: &[u8],
    maximum: usize,
) -> Result<(), WorkerV3CapabilityResultCarrierErrorV1> {
    let mut hooks = NoRetainedDurableDirectoryHooksV1;
    persist_immutable_record_with_hooks(directory, canonical, redo, bytes, maximum, &mut hooks)
}

fn persist_immutable_record_with_hooks(
    directory: &RetainedDurableDirectoryV1,
    canonical: &str,
    redo: &str,
    bytes: &[u8],
    maximum: usize,
    hooks: &mut impl RetainedDurableDirectoryHooksV1,
) -> Result<(), WorkerV3CapabilityResultCarrierErrorV1> {
    let current = directory.read_private(canonical, maximum)?;
    let pending = directory.read_private(redo, maximum)?;
    match (current.as_deref(), pending.as_deref()) {
        (Some(existing), _) if existing != bytes => {
            return Err(WorkerV3CapabilityResultCarrierErrorV1::DurableCarrierConflict);
        }
        (_, Some(existing)) if existing != bytes => {
            return Err(WorkerV3CapabilityResultCarrierErrorV1::DurableCarrierConflict);
        }
        _ => {}
    }
    match (current.as_deref(), pending.as_deref()) {
        (None, None) => directory.commit_record(canonical, redo, bytes, maximum, hooks)?,
        (Some(existing), None) => {
            directory
                .establish_recovered_record_durability(canonical, redo, existing, maximum, hooks)?;
        }
        (canonical_before, Some(pending)) => {
            directory.promote_validated_redo(
                canonical,
                redo,
                canonical_before,
                pending,
                maximum,
                hooks,
            )?;
        }
    }
    directory.verify_exact(canonical, bytes)?;
    Ok(())
}

fn encode_consumption(
    carrier: WorkerV3CapabilityResultCarrierIdentityV1,
    sealed_host_admission_identity: [u8; 32],
) -> [u8; CONSUMPTION_BYTES] {
    let mut bytes = [0_u8; CONSUMPTION_BYTES];
    bytes[..8].copy_from_slice(&CONSUMPTION_MAGIC);
    bytes[8..10].copy_from_slice(&CONSUMPTION_VERSION.to_le_bytes());
    bytes[12..44].copy_from_slice(&carrier.sha256());
    bytes[44..52].copy_from_slice(&carrier.byte_len().to_le_bytes());
    bytes[52..84].copy_from_slice(&sealed_host_admission_identity);
    let checksum = domain_hash(CONSUMPTION_CHECKSUM_DOMAIN, &bytes[..84]);
    bytes[84..].copy_from_slice(&checksum);
    bytes
}

fn decode_consumption(
    bytes: &[u8],
) -> Result<WorkerV3CapabilityResultCarrierIdentityV1, WorkerV3CapabilityResultCarrierErrorV1> {
    if bytes.len() != CONSUMPTION_BYTES
        || bytes[..8] != CONSUMPTION_MAGIC
        || u16::from_le_bytes(bytes[8..10].try_into().unwrap()) != CONSUMPTION_VERSION
        || bytes[10..12] != [0; 2]
        || domain_hash(CONSUMPTION_CHECKSUM_DOMAIN, &bytes[..84]).as_slice() != &bytes[84..]
        || bytes[52..84] == [0; 32]
    {
        return Err(WorkerV3CapabilityResultCarrierErrorV1::InvalidConsumptionRecord);
    }
    Ok(WorkerV3CapabilityResultCarrierIdentityV1 {
        sha256: bytes[12..44].try_into().unwrap(),
        byte_len: u64::from_le_bytes(bytes[44..52].try_into().unwrap()),
    })
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], WorkerV3CapabilityResultCarrierErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(WorkerV3CapabilityResultCarrierErrorV1::LengthOverflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(WorkerV3CapabilityResultCarrierErrorV1::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], WorkerV3CapabilityResultCarrierErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| WorkerV3CapabilityResultCarrierErrorV1::Truncated)
    }

    fn u16(&mut self) -> Result<u16, WorkerV3CapabilityResultCarrierErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, WorkerV3CapabilityResultCarrierErrorV1> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, WorkerV3CapabilityResultCarrierErrorV1> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn binding(&mut self) -> Result<ExactBindingV1, WorkerV3CapabilityResultCarrierErrorV1> {
        Ok(ExactBindingV1 {
            sha256: self.array()?,
            byte_len: self.u64()?,
        })
    }

    const fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_artifact_transaction::{
        RetainedDurableFaultTimingV1, RetainedDurableRecordBoundaryV1,
    };
    use std::fs;
    use std::io;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory {
        path: std::path::PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "fe2o3-capability-carrier-{}-{}",
                std::process::id(),
                NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            Self { path }
        }

        fn retained(&self) -> RetainedDurableDirectoryV1 {
            RetainedDurableDirectoryV1::admit_service_owned(
                std::fs::File::open(&self.path).unwrap().into(),
            )
            .unwrap()
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.path).unwrap();
        }
    }

    struct CrashAt {
        boundary: RetainedDurableRecordBoundaryV1,
        timing: RetainedDurableFaultTimingV1,
        fired: bool,
    }

    impl RetainedDurableDirectoryHooksV1 for CrashAt {
        fn record(
            &mut self,
            boundary: RetainedDurableRecordBoundaryV1,
            timing: RetainedDurableFaultTimingV1,
        ) -> io::Result<()> {
            if !self.fired && (boundary, timing) == (self.boundary, self.timing) {
                self.fired = true;
                Err(io::Error::other("injected capability-journal crash"))
            } else {
                Ok(())
            }
        }
    }

    fn coordinates(seed: u8) -> CarrierCoordinatesV1 {
        let binding = |offset| ExactBindingV1 {
            sha256: [seed.wrapping_add(offset); 32],
            byte_len: u64::from(seed) + u64::from(offset) + 1,
        };
        CarrierCoordinatesV1 {
            load_envelope: binding(1),
            readiness_receipt: [seed.wrapping_add(2); 32],
            generation: u64::from(seed) + 3,
            session: [seed.wrapping_add(4); 16],
            invocation: [seed.wrapping_add(5); 32],
            raw_object: binding(6),
            finalized_artifact: binding(7),
            target_model: [seed.wrapping_add(8); 32],
            launch_roster: [seed.wrapping_add(9); 32],
            subject_root_roster: [seed.wrapping_add(10); 32],
            dynamic_preconditions: [seed.wrapping_add(11); 32],
            compiler_subject: [seed.wrapping_add(12); 32],
            compiler_carriage: [seed.wrapping_add(13); 32],
            compiler_policy: [seed.wrapping_add(14); 32],
            compiler_occurrence: [seed.wrapping_add(15); 32],
            worker_ledger_record: [seed.wrapping_add(16); 32],
            compiler_sequence: u64::from(seed) + 17,
            current_rollback_anchor: [seed.wrapping_add(18); 32],
            source_refinement: binding(19),
            machine_refinement: binding(20),
            result: binding(21),
        }
    }

    fn reseal(bytes: &mut [u8]) {
        let identity_offset = bytes.len() - CHECKSUM_BYTES - TERMINAL_IDENTITY_BYTES;
        let checksum_offset = bytes.len() - CHECKSUM_BYTES;
        let identity = domain_hash(CARRIER_IDENTITY_DOMAIN, &bytes[..identity_offset]);
        bytes[identity_offset..checksum_offset].copy_from_slice(&identity);
        let checksum = domain_hash(CARRIER_CHECKSUM_DOMAIN, &bytes[..checksum_offset]);
        bytes[checksum_offset..].copy_from_slice(&checksum);
    }

    fn attempt(seed: u8) -> BuildAttempt {
        let value = format!(
            "{}:{}:{}",
            u64::from(seed) + 1,
            hex(&[seed.wrapping_add(1); 16]),
            hex(&[seed.wrapping_add(2); 32])
        );
        BuildAttempt::from_env_value(&value).unwrap()
    }

    fn reseal_pending(bytes: &mut [u8]) {
        let identity_offset = bytes.len() - PENDING_CHECKSUM_BYTES - PENDING_IDENTITY_BYTES;
        let checksum_offset = bytes.len() - PENDING_CHECKSUM_BYTES;
        let identity = domain_hash(PENDING_IDENTITY_DOMAIN, &bytes[..identity_offset]);
        bytes[identity_offset..checksum_offset].copy_from_slice(&identity);
        let checksum = domain_hash(PENDING_CHECKSUM_DOMAIN, &bytes[..checksum_offset]);
        bytes[checksum_offset..].copy_from_slice(&checksum);
    }

    #[test]
    fn pending_framing_round_trips_exact_attempt_binding_and_result() {
        let attempt = attempt(7);
        let envelope = ExactBindingV1 {
            sha256: [0x31; 32],
            byte_len: 901,
        };
        let result = b"exact-pending-result";
        let bytes = encode_pending(envelope, attempt, result).unwrap();
        let (actual_envelope, actual_result) = decode_pending_sections(&bytes, attempt).unwrap();
        assert_eq!(actual_envelope, envelope);
        assert_eq!(actual_result, result);
        assert_eq!(bytes.len(), PENDING_FIXED_OVERHEAD_BYTES + result.len());
    }

    #[test]
    fn pending_rejects_omission_append_downgrade_and_cross_attempt_replay() {
        let expected_attempt = attempt(8);
        let bytes = encode_pending(
            ExactBindingV1 {
                sha256: [0x41; 32],
                byte_len: 902,
            },
            expected_attempt,
            b"pending-result",
        )
        .unwrap();
        for end in 0..bytes.len() {
            assert!(
                decode_pending_sections(&bytes[..end], expected_attempt).is_err(),
                "prefix {end}"
            );
        }
        let mut appended = bytes.clone();
        appended.push(0);
        assert!(decode_pending_sections(&appended, expected_attempt).is_err());

        let mut downgraded = bytes.clone();
        downgraded[8..10].copy_from_slice(&0_u16.to_le_bytes());
        reseal_pending(&mut downgraded);
        assert!(matches!(
            decode_pending_sections(&downgraded, expected_attempt),
            Err(WorkerV3CapabilityResultCarrierErrorV1::UnsupportedVersion { actual: 0 })
        ));
        assert!(matches!(
            decode_pending_sections(&bytes, attempt(9)),
            Err(WorkerV3CapabilityResultCarrierErrorV1::BindingMismatch(
                WorkerV3CapabilityResultBindingFieldV1::Attempt
            ))
        ));
    }

    #[test]
    fn pending_rejects_checksum_identity_and_declared_result_substitution() {
        let attempt = attempt(10);
        let bytes = encode_pending(
            ExactBindingV1 {
                sha256: [0x51; 32],
                byte_len: 903,
            },
            attempt,
            b"pending-result",
        )
        .unwrap();

        let mut checksum = bytes.clone();
        checksum[PENDING_HEADER_BYTES] ^= 1;
        assert!(matches!(
            decode_pending_sections(&checksum, attempt),
            Err(WorkerV3CapabilityResultCarrierErrorV1::ChecksumMismatch)
        ));

        let mut identity = bytes.clone();
        let identity_offset = identity.len() - PENDING_CHECKSUM_BYTES - PENDING_IDENTITY_BYTES;
        identity[identity_offset] ^= 1;
        let checksum_offset = identity.len() - PENDING_CHECKSUM_BYTES;
        let checksum = domain_hash(PENDING_CHECKSUM_DOMAIN, &identity[..checksum_offset]);
        identity[checksum_offset..].copy_from_slice(&checksum);
        assert!(matches!(
            decode_pending_sections(&identity, attempt),
            Err(WorkerV3CapabilityResultCarrierErrorV1::IdentityMismatch)
        ));

        let mut declared = bytes;
        let result_length_offset = 8 + 2 + 2 + 8 + BINDING_BYTES + ATTEMPT_BYTES;
        declared[result_length_offset..result_length_offset + 8]
            .copy_from_slice(&1_u64.to_le_bytes());
        reseal_pending(&mut declared);
        assert!(matches!(
            decode_pending_sections(&declared, attempt),
            Err(WorkerV3CapabilityResultCarrierErrorV1::TrailingBytes)
        ));
    }

    #[test]
    fn pending_journal_resumes_each_durable_record_crash_boundary() {
        let boundaries = [
            RetainedDurableRecordBoundaryV1::CreateTemp,
            RetainedDurableRecordBoundaryV1::WriteTemp,
            RetainedDurableRecordBoundaryV1::SyncTemp,
            RetainedDurableRecordBoundaryV1::RenameTempToRedo,
            RetainedDurableRecordBoundaryV1::SyncRedoName,
            RetainedDurableRecordBoundaryV1::RenameRedoToCanonical,
            RetainedDurableRecordBoundaryV1::SyncCanonicalName,
        ];
        let timings = [
            RetainedDurableFaultTimingV1::Before,
            RetainedDurableFaultTimingV1::After,
        ];

        for boundary in boundaries {
            for timing in timings {
                let root = TestDirectory::new();
                let directory = root.retained();
                let expected_attempt = attempt(11);
                let names = DurableNames::new(AttemptKey::from_attempt(expected_attempt));
                let bytes = encode_pending(
                    ExactBindingV1 {
                        sha256: [0x61; 32],
                        byte_len: 904,
                    },
                    expected_attempt,
                    b"crash-recoverable-pending-result",
                )
                .unwrap();
                let mut crash = CrashAt {
                    boundary,
                    timing,
                    fired: false,
                };
                assert!(
                    persist_immutable_record_with_hooks(
                        &directory,
                        &names.pending,
                        &names.pending_redo,
                        &bytes,
                        MAX_WORKER_V3_PENDING_CAPABILITY_RESULT_BYTES_V1,
                        &mut crash,
                    )
                    .is_err(),
                    "{boundary:?} {timing:?}"
                );
                assert!(crash.fired, "{boundary:?} {timing:?}");

                persist_immutable_record(
                    &directory,
                    &names.pending,
                    &names.pending_redo,
                    &bytes,
                    MAX_WORKER_V3_PENDING_CAPABILITY_RESULT_BYTES_V1,
                )
                .unwrap();
                let recovered = directory
                    .read_private(
                        &names.pending,
                        MAX_WORKER_V3_PENDING_CAPABILITY_RESULT_BYTES_V1,
                    )
                    .unwrap()
                    .unwrap();
                assert_eq!(recovered, bytes, "{boundary:?} {timing:?}");
                assert_eq!(
                    directory
                        .read_private(
                            &names.pending_redo,
                            MAX_WORKER_V3_PENDING_CAPABILITY_RESULT_BYTES_V1,
                        )
                        .unwrap(),
                    None,
                    "{boundary:?} {timing:?}"
                );
                decode_pending_sections(&recovered, expected_attempt).unwrap();
            }
        }
    }

    #[test]
    fn carrier_framing_round_trips_exact_coordinates_and_bytes() {
        let expected = coordinates(7);
        let result = b"exact-result-bytes";
        let bytes = encode_carrier(expected, result).unwrap();
        assert_eq!(bytes.len(), FIXED_OVERHEAD_BYTES + result.len());
        let (actual, decoded_result) = decode_carrier_sections(&bytes).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(decoded_result, result);
    }

    #[test]
    fn carrier_rejects_every_omission_and_appended_bytes() {
        let bytes = encode_carrier(coordinates(3), b"result").unwrap();
        for end in 0..bytes.len() {
            assert!(
                decode_carrier_sections(&bytes[..end]).is_err(),
                "prefix {end}"
            );
        }
        let mut appended = bytes;
        appended.push(0);
        assert!(decode_carrier_sections(&appended).is_err());
    }

    #[test]
    fn carrier_rejects_validly_resealed_downgrade_and_trailing_body_bytes() {
        let mut downgrade = encode_carrier(coordinates(4), b"result").unwrap();
        downgrade[8..10].copy_from_slice(&0_u16.to_le_bytes());
        reseal(&mut downgrade);
        assert!(matches!(
            decode_carrier_sections(&downgrade),
            Err(WorkerV3CapabilityResultCarrierErrorV1::UnsupportedVersion { actual: 0 })
        ));

        let mut trailing = encode_carrier(coordinates(5), b"result").unwrap();
        trailing[20..28].copy_from_slice(&5_u64.to_le_bytes());
        reseal(&mut trailing);
        assert!(matches!(
            decode_carrier_sections(&trailing),
            Err(WorkerV3CapabilityResultCarrierErrorV1::TrailingBytes)
        ));
    }

    #[test]
    fn carrier_rejects_checksum_identity_and_result_substitution() {
        let bytes = encode_carrier(coordinates(6), b"not-a-v5-result").unwrap();

        let mut checksum = bytes.clone();
        checksum[HEADER_BYTES] ^= 1;
        assert!(matches!(
            decode_carrier_sections(&checksum),
            Err(WorkerV3CapabilityResultCarrierErrorV1::ChecksumMismatch)
        ));

        let mut identity = bytes.clone();
        let identity_offset = identity.len() - CHECKSUM_BYTES - TERMINAL_IDENTITY_BYTES;
        identity[identity_offset] ^= 1;
        let checksum_offset = identity.len() - CHECKSUM_BYTES;
        let value = domain_hash(CARRIER_CHECKSUM_DOMAIN, &identity[..checksum_offset]);
        identity[checksum_offset..].copy_from_slice(&value);
        assert!(matches!(
            decode_carrier_sections(&identity),
            Err(WorkerV3CapabilityResultCarrierErrorV1::IdentityMismatch)
        ));

        assert!(matches!(
            WorkerV3CapabilityResultCarrierWireV1::decode_canonical(&bytes),
            Err(WorkerV3CapabilityResultCarrierErrorV1::Result(_))
        ));
    }

    #[test]
    fn consumption_record_binds_carrier_length_hash_and_nonzero_admission() {
        let carrier = WorkerV3CapabilityResultCarrierIdentityV1 {
            sha256: [0x31; 32],
            byte_len: 901,
        };
        let bytes = encode_consumption(carrier, [0x52; 32]);
        assert_eq!(decode_consumption(&bytes).unwrap(), carrier);

        for offset in [12, 44, 52, 84] {
            let mut substituted = bytes;
            substituted[offset] ^= 1;
            assert!(matches!(
                decode_consumption(&substituted),
                Err(WorkerV3CapabilityResultCarrierErrorV1::InvalidConsumptionRecord)
            ));
        }
        assert!(decode_consumption(&bytes[..bytes.len() - 1]).is_err());
        assert!(decode_consumption(&encode_consumption(carrier, [0; 32])).is_err());
    }
}
