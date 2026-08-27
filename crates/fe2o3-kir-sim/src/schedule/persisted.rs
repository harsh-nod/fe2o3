use std::error::Error;
use std::fmt;
use std::io::{self, Write};

use fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV7;
use serde::de::{self, SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize};

use super::{
    MAX_SCHEDULE_DECISIONS_V1, SimulationScheduleCoverageV1, SimulationScheduleDecisionV1,
    SimulationScheduleIdentityV1, SimulationScheduleRecordV1, record_integrity,
    transcript_identity,
};
use crate::{IndexWidthV1, SimulationLimitsV1, SimulationTargetV1};

/// Maximum canonical bytes accepted for one persisted semantic CPU schedule.
pub const MAX_PERSISTED_SCHEDULE_BYTES_V1: usize = 256 * 1024 * 1024;

const SCHEMA_V1: &str = "fe2o3-simulation-schedule-v1";
const MAX_WIRE_STRING_TOKEN_BYTES_V1: usize = 128;
const DECISION_LIMIT_MARKER: &str = "fe2o3:schedule_decision_limit";
const ALLOCATION_MARKER: &str = "fe2o3:schedule_allocation_failure";
const UNSUPPORTED_SCHEMA_MARKER: &str = "fe2o3:schedule_schema_unsupported";

/// Exact artifact route bound to a persisted simulator schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistedSimulationScheduleArtifactV1 {
    /// Exact canonical Kernel IR V7 was supplied directly.
    CanonicalKirV7,
    /// An authority-free simulation bundle supplied the exact canonical KIR.
    SimulationBundleV1 {
        bundle_sha256: [u8; 32],
        subject_sha256: [u8; 32],
    },
}

/// Exact immutable inputs which supplement the simulator record's semantic context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistedSimulationScheduleBindingV1 {
    artifact: PersistedSimulationScheduleArtifactV1,
    kir_sha256: [u8; 32],
    kir_canonical_bytes: u64,
    request_sha256: [u8; 32],
    request_bytes: u64,
    target: SimulationTargetV1,
    limits: SimulationLimitsV1,
}

impl PersistedSimulationScheduleBindingV1 {
    /// Constructs an exact persisted binding from already admitted simulator inputs.
    pub const fn new(
        artifact: PersistedSimulationScheduleArtifactV1,
        kir: VerifiedCanonicalKernelIrIdentityV7,
        request_sha256: [u8; 32],
        request_bytes: u64,
        target: SimulationTargetV1,
        limits: SimulationLimitsV1,
    ) -> Self {
        Self {
            artifact,
            kir_sha256: *kir.digest(),
            kir_canonical_bytes: kir.canonical_length(),
            request_sha256,
            request_bytes,
            target,
            limits,
        }
    }

    pub const fn artifact(self) -> PersistedSimulationScheduleArtifactV1 {
        self.artifact
    }

    pub const fn kir_sha256(self) -> [u8; 32] {
        self.kir_sha256
    }

    pub const fn kir_canonical_bytes(self) -> u64 {
        self.kir_canonical_bytes
    }

    pub const fn request_sha256(self) -> [u8; 32] {
        self.request_sha256
    }

    pub const fn request_bytes(self) -> u64 {
        self.request_bytes
    }

    pub const fn target(self) -> SimulationTargetV1 {
        self.target
    }

    pub const fn limits(self) -> SimulationLimitsV1 {
        self.limits
    }
}

/// Strictly decoded canonical persisted schedule plus its exact admission binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedSimulationScheduleDocumentV1 {
    binding: PersistedSimulationScheduleBindingV1,
    record: SimulationScheduleRecordV1,
}

impl PersistedSimulationScheduleDocumentV1 {
    /// Constructs a document view over a successful in-process schedule record.
    pub fn new(
        binding: PersistedSimulationScheduleBindingV1,
        record: SimulationScheduleRecordV1,
    ) -> Result<Self, PersistedSimulationScheduleCodecErrorV1> {
        validate_binding(&binding)?;
        validate_record_structure(&record)?;
        Ok(Self { binding, record })
    }

    pub const fn binding(&self) -> PersistedSimulationScheduleBindingV1 {
        self.binding
    }

    pub const fn record(&self) -> &SimulationScheduleRecordV1 {
        &self.record
    }

    /// Encodes the unique whitespace-free canonical JSON representation.
    pub fn to_canonical_bytes(&self) -> Result<Vec<u8>, PersistedSimulationScheduleCodecErrorV1> {
        encode_parts(&self.binding, &self.record)
    }

    /// Encodes a successful in-process record without cloning its decisions.
    pub fn encode_record(
        binding: PersistedSimulationScheduleBindingV1,
        record: &SimulationScheduleRecordV1,
    ) -> Result<Vec<u8>, PersistedSimulationScheduleCodecErrorV1> {
        encode_parts(&binding, record)
    }

    /// Strictly decodes canonical JSON, rejecting alternate spellings and layout.
    pub fn from_canonical_bytes(
        bytes: &[u8],
    ) -> Result<Self, PersistedSimulationScheduleCodecErrorV1> {
        if bytes.is_empty() {
            return Err(PersistedSimulationScheduleCodecErrorV1::Empty);
        }
        if bytes.len() > MAX_PERSISTED_SCHEDULE_BYTES_V1 {
            return Err(PersistedSimulationScheduleCodecErrorV1::ByteLimit {
                actual: bytes.len(),
                limit: MAX_PERSISTED_SCHEDULE_BYTES_V1,
            });
        }
        validate_string_token_bounds(bytes)?;
        let wire: ScheduleDocumentWireV1 = serde_json::from_slice(bytes).map_err(json_error)?;
        let document = Self::try_from(wire)?;
        validate_canonical_bytes(&document.binding, &document.record, bytes)?;
        Ok(document)
    }
}

/// Closed persisted-schedule codec failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistedSimulationScheduleCodecErrorV1 {
    Empty,
    ByteLimit { actual: usize, limit: usize },
    JsonSyntax,
    JsonStructure,
    StringTokenLimit { actual: usize, limit: usize },
    DecisionLimit { actual: usize, limit: usize },
    NumericOverflow(&'static str),
    AllocationFailure,
    UnsupportedSchema,
    InvalidBinding,
    InvalidTarget,
    InvalidLimits,
    InvalidCoverage,
    InvalidRecordIntegrity,
    NonCanonical,
    EncodingFailure,
}

impl fmt::Display for PersistedSimulationScheduleCodecErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("persisted schedule is empty"),
            Self::ByteLimit { actual, limit } => {
                write!(
                    formatter,
                    "persisted schedule has {actual} bytes; maximum is {limit}"
                )
            }
            Self::JsonSyntax => formatter.write_str("persisted schedule JSON syntax is invalid"),
            Self::JsonStructure => {
                formatter.write_str("persisted schedule JSON structure is invalid")
            }
            Self::StringTokenLimit { actual, limit } => write!(
                formatter,
                "persisted schedule JSON string has at least {actual} encoded bytes; maximum is {limit}"
            ),
            Self::DecisionLimit { actual, limit } => write!(
                formatter,
                "persisted schedule has {actual} decisions; maximum is {limit}"
            ),
            Self::NumericOverflow(field) => {
                write!(
                    formatter,
                    "persisted schedule field {field} exceeds this host"
                )
            }
            Self::AllocationFailure => {
                formatter.write_str("cannot allocate bounded persisted schedule storage")
            }
            Self::UnsupportedSchema => {
                formatter.write_str("persisted schedule schema is unsupported")
            }
            Self::InvalidBinding => {
                formatter.write_str("persisted schedule artifact or request binding is invalid")
            }
            Self::InvalidTarget => formatter.write_str("persisted schedule target is invalid"),
            Self::InvalidLimits => formatter.write_str("persisted schedule limits are invalid"),
            Self::InvalidCoverage => formatter.write_str("persisted schedule coverage is invalid"),
            Self::InvalidRecordIntegrity => {
                formatter.write_str("persisted schedule record integrity is invalid")
            }
            Self::NonCanonical => formatter.write_str("persisted schedule bytes are not canonical"),
            Self::EncodingFailure => formatter.write_str("cannot encode persisted schedule"),
        }
    }
}

impl Error for PersistedSimulationScheduleCodecErrorV1 {}

fn validate_string_token_bounds(
    bytes: &[u8],
) -> Result<(), PersistedSimulationScheduleCodecErrorV1> {
    let mut in_string = false;
    let mut escaped = false;
    let mut token_bytes = 0_usize;
    for byte in bytes {
        if !in_string {
            if *byte == b'"' {
                in_string = true;
                token_bytes = 0;
            }
            continue;
        }
        if !escaped && *byte == b'"' {
            in_string = false;
            continue;
        }
        token_bytes = token_bytes.saturating_add(1);
        if token_bytes > MAX_WIRE_STRING_TOKEN_BYTES_V1 {
            return Err(PersistedSimulationScheduleCodecErrorV1::StringTokenLimit {
                actual: MAX_WIRE_STRING_TOKEN_BYTES_V1.saturating_add(1),
                limit: MAX_WIRE_STRING_TOKEN_BYTES_V1,
            });
        }
        if escaped {
            escaped = false;
        } else if *byte == b'\\' {
            escaped = true;
        }
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScheduleDocumentWireV1 {
    schema: ScheduleSchemaWireV1,
    artifact: ArtifactWireV1,
    request: RequestWireV1,
    target: TargetWireV1,
    limits: LimitsWireV1,
    context_sha256: HexIdentityV1,
    transcript_sha256: HexIdentityV1,
    record_sha256: HexIdentityV1,
    schedule: ScheduleWireV1,
    coverage: CoverageWireV1,
    decisions: BoundedDecisionsV1,
}

#[derive(Serialize)]
struct ScheduleDocumentEncodeWireV1<'a> {
    schema: ScheduleSchemaWireV1,
    artifact: ArtifactWireV1,
    request: RequestWireV1,
    target: TargetWireV1,
    limits: LimitsWireV1,
    context_sha256: HexIdentityV1,
    transcript_sha256: HexIdentityV1,
    record_sha256: HexIdentityV1,
    schedule: ScheduleWireV1,
    coverage: CoverageWireV1,
    decisions: DecisionSliceWireV1<'a>,
}

#[derive(Clone, Copy, Serialize)]
enum ScheduleSchemaWireV1 {
    #[serde(rename = "fe2o3-simulation-schedule-v1")]
    V1,
}

impl<'de> Deserialize<'de> for ScheduleSchemaWireV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match <&str>::deserialize(deserializer)? {
            SCHEMA_V1 => Ok(Self::V1),
            _ => Err(de::Error::custom(UNSUPPORTED_SCHEMA_MARKER)),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ArtifactWireV1 {
    CanonicalKirV7 {
        kir_sha256: HexIdentityV1,
        kir_canonical_bytes: u64,
    },
    SimulationBundleV1 {
        bundle_sha256: HexIdentityV1,
        subject_sha256: HexIdentityV1,
        kir_sha256: HexIdentityV1,
        kir_canonical_bytes: u64,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestWireV1 {
    sha256: HexIdentityV1,
    bytes: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetWireV1 {
    identity: TargetIdentityWireV1,
    index_bits: u16,
    max_workgroup_invocations: u64,
}

#[derive(Serialize, Deserialize)]
enum TargetIdentityWireV1 {
    #[serde(rename = "little_endian_index32_v1")]
    LittleEndianIndex32V1,
    #[serde(rename = "amdgpu_64_little_endian_v1")]
    Amdgpu64LittleEndianV1,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LimitsWireV1 {
    max_canonical_bytes: u64,
    max_reachable_functions: u64,
    max_reachable_operations: u64,
    max_invocations: u64,
    max_workgroups: u64,
    max_scheduled_slots: u64,
    max_steps: u64,
    max_call_depth: u64,
    max_ssa_values: u64,
    max_allocations: u64,
    max_allocation_bytes: u64,
    max_total_bytes: u64,
    max_resident_bytes: u64,
    max_events: u64,
    max_memory_access_records: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "identity", rename_all = "snake_case", deny_unknown_fields)]
enum ScheduleWireV1 {
    WorkgroupMajorLocalZyxCooperativeV1,
    WorkgroupMajorSeededRunnableCooperativeV1 { seed: u64 },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoverageWireV1 {
    decisions: u64,
    workgroups: u64,
    barrier_releases: u64,
    complete: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DecisionWireV1 {
    workgroup: [u64; 3],
    phase: u64,
    local: [u32; 3],
}

struct DecisionSliceWireV1<'a>(&'a [SimulationScheduleDecisionV1]);

impl Serialize for DecisionSliceWireV1<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for decision in self.0 {
            sequence.serialize_element(&DecisionWireV1 {
                workgroup: decision.workgroup,
                phase: decision.phase,
                local: decision.local,
            })?;
        }
        sequence.end()
    }
}

struct BoundedDecisionsV1(Vec<SimulationScheduleDecisionV1>);

impl Serialize for BoundedDecisionsV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        DecisionSliceWireV1(&self.0).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BoundedDecisionsV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(DecisionVisitorV1)
    }
}

struct DecisionVisitorV1;

impl<'de> Visitor<'de> for DecisionVisitorV1 {
    type Value = BoundedDecisionsV1;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "at most {MAX_SCHEDULE_DECISIONS_V1} schedule decisions"
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        if sequence
            .size_hint()
            .is_some_and(|hint| hint > MAX_SCHEDULE_DECISIONS_V1)
        {
            return Err(de::Error::custom(DECISION_LIMIT_MARKER));
        }
        let mut decisions = Vec::new();
        decisions
            .try_reserve_exact(
                sequence
                    .size_hint()
                    .unwrap_or(0)
                    .min(MAX_SCHEDULE_DECISIONS_V1),
            )
            .map_err(|_| de::Error::custom(ALLOCATION_MARKER))?;
        while let Some(decision) = sequence.next_element::<DecisionWireV1>()? {
            if decisions.len() == MAX_SCHEDULE_DECISIONS_V1 {
                return Err(de::Error::custom(DECISION_LIMIT_MARKER));
            }
            if decisions.len() == decisions.capacity() {
                decisions
                    .try_reserve(1)
                    .map_err(|_| de::Error::custom(ALLOCATION_MARKER))?;
            }
            decisions.push(SimulationScheduleDecisionV1 {
                workgroup: decision.workgroup,
                phase: decision.phase,
                local: decision.local,
            });
        }
        Ok(BoundedDecisionsV1(decisions))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HexIdentityV1([u8; 32]);

impl Serialize for HexIdentityV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut encoded = [0_u8; 64];
        encode_hex(&self.0, &mut encoded);
        let value = std::str::from_utf8(&encoded).expect("lowercase hexadecimal is UTF-8");
        serializer.serialize_str(value)
    }
}

impl<'de> Deserialize<'de> for HexIdentityV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = <&str>::deserialize(deserializer)?;
        decode_identity(value)
            .map(Self)
            .ok_or_else(|| de::Error::custom("identity must be 64 lowercase hexadecimal digits"))
    }
}

impl TryFrom<ScheduleDocumentWireV1> for PersistedSimulationScheduleDocumentV1 {
    type Error = PersistedSimulationScheduleCodecErrorV1;

    fn try_from(wire: ScheduleDocumentWireV1) -> Result<Self, Self::Error> {
        let ScheduleSchemaWireV1::V1 = wire.schema;
        let (artifact, kir_sha256, kir_canonical_bytes) = match wire.artifact {
            ArtifactWireV1::CanonicalKirV7 {
                kir_sha256,
                kir_canonical_bytes,
            } => (
                PersistedSimulationScheduleArtifactV1::CanonicalKirV7,
                kir_sha256.0,
                kir_canonical_bytes,
            ),
            ArtifactWireV1::SimulationBundleV1 {
                bundle_sha256,
                subject_sha256,
                kir_sha256,
                kir_canonical_bytes,
            } => (
                PersistedSimulationScheduleArtifactV1::SimulationBundleV1 {
                    bundle_sha256: bundle_sha256.0,
                    subject_sha256: subject_sha256.0,
                },
                kir_sha256.0,
                kir_canonical_bytes,
            ),
        };
        let target = target_from_wire(&wire.target)?;
        let limits = limits_from_wire(wire.limits)?;
        let (schedule, seed) = match wire.schedule {
            ScheduleWireV1::WorkgroupMajorLocalZyxCooperativeV1 => (
                SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1,
                None,
            ),
            ScheduleWireV1::WorkgroupMajorSeededRunnableCooperativeV1 { seed } => (
                SimulationScheduleIdentityV1::WorkgroupMajorSeededRunnableCooperativeV1,
                Some(seed),
            ),
        };
        if !wire.coverage.complete {
            return Err(PersistedSimulationScheduleCodecErrorV1::InvalidCoverage);
        }
        let decisions = wire.decisions.0;
        let record = SimulationScheduleRecordV1 {
            context_identity: wire.context_sha256.0,
            transcript_identity: wire.transcript_sha256.0,
            record_integrity: wire.record_sha256.0,
            schedule,
            seed,
            decisions,
            coverage: SimulationScheduleCoverageV1 {
                decisions: wire.coverage.decisions,
                workgroups: wire.coverage.workgroups,
                barrier_releases: wire.coverage.barrier_releases,
            },
        };
        let binding = PersistedSimulationScheduleBindingV1 {
            artifact,
            kir_sha256,
            kir_canonical_bytes,
            request_sha256: wire.request.sha256.0,
            request_bytes: wire.request.bytes,
            target,
            limits,
        };
        validate_binding(&binding)?;
        validate_record_structure(&record)?;
        Ok(Self { binding, record })
    }
}

fn encode_parts(
    binding: &PersistedSimulationScheduleBindingV1,
    record: &SimulationScheduleRecordV1,
) -> Result<Vec<u8>, PersistedSimulationScheduleCodecErrorV1> {
    validate_binding(binding)?;
    validate_record_structure(record)?;
    let wire = wire_from_parts(binding, record)?;
    let mut writer = BoundedVecWriterV1::default();
    serde_json::to_writer(&mut writer, &wire).map_err(|_| {
        writer
            .failure
            .unwrap_or(PersistedSimulationScheduleCodecErrorV1::EncodingFailure)
    })?;
    Ok(writer.bytes)
}

fn validate_binding(
    binding: &PersistedSimulationScheduleBindingV1,
) -> Result<(), PersistedSimulationScheduleCodecErrorV1> {
    if binding.kir_canonical_bytes == 0 || binding.request_bytes == 0 {
        return Err(PersistedSimulationScheduleCodecErrorV1::InvalidBinding);
    }
    binding
        .limits
        .validate()
        .map(|_| ())
        .map_err(|_| PersistedSimulationScheduleCodecErrorV1::InvalidLimits)
}

fn validate_canonical_bytes(
    binding: &PersistedSimulationScheduleBindingV1,
    record: &SimulationScheduleRecordV1,
    expected: &[u8],
) -> Result<(), PersistedSimulationScheduleCodecErrorV1> {
    let wire = wire_from_parts(binding, record)?;
    let mut writer = ExactBytesWriterV1 {
        expected,
        offset: 0,
        mismatch: false,
    };
    serde_json::to_writer(&mut writer, &wire)
        .map_err(|_| PersistedSimulationScheduleCodecErrorV1::EncodingFailure)?;
    if writer.mismatch || writer.offset != expected.len() {
        return Err(PersistedSimulationScheduleCodecErrorV1::NonCanonical);
    }
    Ok(())
}

fn wire_from_parts<'a>(
    binding: &PersistedSimulationScheduleBindingV1,
    record: &'a SimulationScheduleRecordV1,
) -> Result<ScheduleDocumentEncodeWireV1<'a>, PersistedSimulationScheduleCodecErrorV1> {
    let artifact = match binding.artifact {
        PersistedSimulationScheduleArtifactV1::CanonicalKirV7 => ArtifactWireV1::CanonicalKirV7 {
            kir_sha256: HexIdentityV1(binding.kir_sha256),
            kir_canonical_bytes: binding.kir_canonical_bytes,
        },
        PersistedSimulationScheduleArtifactV1::SimulationBundleV1 {
            bundle_sha256,
            subject_sha256,
        } => ArtifactWireV1::SimulationBundleV1 {
            bundle_sha256: HexIdentityV1(bundle_sha256),
            subject_sha256: HexIdentityV1(subject_sha256),
            kir_sha256: HexIdentityV1(binding.kir_sha256),
            kir_canonical_bytes: binding.kir_canonical_bytes,
        },
    };
    let schedule = match (record.schedule, record.seed) {
        (SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1, None) => {
            ScheduleWireV1::WorkgroupMajorLocalZyxCooperativeV1
        }
        (SimulationScheduleIdentityV1::WorkgroupMajorSeededRunnableCooperativeV1, Some(seed)) => {
            ScheduleWireV1::WorkgroupMajorSeededRunnableCooperativeV1 { seed }
        }
        _ => return Err(PersistedSimulationScheduleCodecErrorV1::InvalidRecordIntegrity),
    };
    Ok(ScheduleDocumentEncodeWireV1 {
        schema: ScheduleSchemaWireV1::V1,
        artifact,
        request: RequestWireV1 {
            sha256: HexIdentityV1(binding.request_sha256),
            bytes: binding.request_bytes,
        },
        target: target_to_wire(binding.target),
        limits: limits_to_wire(binding.limits)?,
        context_sha256: HexIdentityV1(record.context_identity),
        transcript_sha256: HexIdentityV1(record.transcript_identity),
        record_sha256: HexIdentityV1(record.record_integrity),
        schedule,
        coverage: CoverageWireV1 {
            decisions: record.coverage.decisions,
            workgroups: record.coverage.workgroups,
            barrier_releases: record.coverage.barrier_releases,
            complete: true,
        },
        decisions: DecisionSliceWireV1(&record.decisions),
    })
}

fn validate_record_structure(
    record: &SimulationScheduleRecordV1,
) -> Result<(), PersistedSimulationScheduleCodecErrorV1> {
    if record.decisions.is_empty()
        || record.coverage.decisions != record.decisions.len() as u64
        || record.coverage.workgroups == 0
        || record.decisions.len() > MAX_SCHEDULE_DECISIONS_V1
    {
        return Err(PersistedSimulationScheduleCodecErrorV1::InvalidCoverage);
    }
    if transcript_identity(
        record.context_identity,
        record.schedule,
        record.seed,
        record.coverage,
    ) != record.transcript_identity
        || record_integrity(record.transcript_identity, &record.decisions)
            != record.record_integrity
    {
        return Err(PersistedSimulationScheduleCodecErrorV1::InvalidRecordIntegrity);
    }
    Ok(())
}

fn target_to_wire(target: SimulationTargetV1) -> TargetWireV1 {
    let (identity, index_bits) = match target.index_width() {
        IndexWidthV1::Bits32 => (TargetIdentityWireV1::LittleEndianIndex32V1, 32),
        IndexWidthV1::Bits64 => (TargetIdentityWireV1::Amdgpu64LittleEndianV1, 64),
    };
    TargetWireV1 {
        identity,
        index_bits,
        max_workgroup_invocations: target.max_workgroup_invocations(),
    }
}

fn target_from_wire(
    target: &TargetWireV1,
) -> Result<SimulationTargetV1, PersistedSimulationScheduleCodecErrorV1> {
    let expected_bits = match target.identity {
        TargetIdentityWireV1::LittleEndianIndex32V1 => 32,
        TargetIdentityWireV1::Amdgpu64LittleEndianV1 => 64,
    };
    if target.index_bits != expected_bits || target.max_workgroup_invocations != 1_024 {
        return Err(PersistedSimulationScheduleCodecErrorV1::InvalidTarget);
    }
    Ok(SimulationTargetV1::little_endian(if expected_bits == 32 {
        IndexWidthV1::Bits32
    } else {
        IndexWidthV1::Bits64
    }))
}

fn limits_to_wire(
    limits: SimulationLimitsV1,
) -> Result<LimitsWireV1, PersistedSimulationScheduleCodecErrorV1> {
    Ok(LimitsWireV1 {
        max_canonical_bytes: as_u64(limits.max_canonical_bytes, "max_canonical_bytes")?,
        max_reachable_functions: as_u64(limits.max_reachable_functions, "max_reachable_functions")?,
        max_reachable_operations: as_u64(
            limits.max_reachable_operations,
            "max_reachable_operations",
        )?,
        max_invocations: limits.max_invocations,
        max_workgroups: limits.max_workgroups,
        max_scheduled_slots: limits.max_scheduled_slots,
        max_steps: limits.max_steps,
        max_call_depth: as_u64(limits.max_call_depth, "max_call_depth")?,
        max_ssa_values: as_u64(limits.max_ssa_values, "max_ssa_values")?,
        max_allocations: as_u64(limits.max_allocations, "max_allocations")?,
        max_allocation_bytes: as_u64(limits.max_allocation_bytes, "max_allocation_bytes")?,
        max_total_bytes: as_u64(limits.max_total_bytes, "max_total_bytes")?,
        max_resident_bytes: as_u64(limits.max_resident_bytes, "max_resident_bytes")?,
        max_events: limits.max_events,
        max_memory_access_records: as_u64(
            limits.max_memory_access_records,
            "max_memory_access_records",
        )?,
    })
}

fn limits_from_wire(
    limits: LimitsWireV1,
) -> Result<SimulationLimitsV1, PersistedSimulationScheduleCodecErrorV1> {
    let limits = SimulationLimitsV1 {
        max_canonical_bytes: as_usize(limits.max_canonical_bytes, "max_canonical_bytes")?,
        max_reachable_functions: as_usize(
            limits.max_reachable_functions,
            "max_reachable_functions",
        )?,
        max_reachable_operations: as_usize(
            limits.max_reachable_operations,
            "max_reachable_operations",
        )?,
        max_invocations: limits.max_invocations,
        max_workgroups: limits.max_workgroups,
        max_scheduled_slots: limits.max_scheduled_slots,
        max_steps: limits.max_steps,
        max_call_depth: as_usize(limits.max_call_depth, "max_call_depth")?,
        max_ssa_values: as_usize(limits.max_ssa_values, "max_ssa_values")?,
        max_allocations: as_usize(limits.max_allocations, "max_allocations")?,
        max_allocation_bytes: as_usize(limits.max_allocation_bytes, "max_allocation_bytes")?,
        max_total_bytes: as_usize(limits.max_total_bytes, "max_total_bytes")?,
        max_resident_bytes: as_usize(limits.max_resident_bytes, "max_resident_bytes")?,
        max_events: limits.max_events,
        max_memory_access_records: as_usize(
            limits.max_memory_access_records,
            "max_memory_access_records",
        )?,
    };
    limits
        .validate()
        .map_err(|_| PersistedSimulationScheduleCodecErrorV1::InvalidLimits)
}

fn as_u64(
    value: usize,
    field: &'static str,
) -> Result<u64, PersistedSimulationScheduleCodecErrorV1> {
    u64::try_from(value)
        .map_err(|_| PersistedSimulationScheduleCodecErrorV1::NumericOverflow(field))
}

fn as_usize(
    value: u64,
    field: &'static str,
) -> Result<usize, PersistedSimulationScheduleCodecErrorV1> {
    usize::try_from(value)
        .map_err(|_| PersistedSimulationScheduleCodecErrorV1::NumericOverflow(field))
}

fn json_error(error: serde_json::Error) -> PersistedSimulationScheduleCodecErrorV1 {
    let detail = error.to_string();
    let marker = detail
        .split_once(" at line ")
        .map_or(detail.as_str(), |(marker, _)| marker);
    if marker == DECISION_LIMIT_MARKER {
        PersistedSimulationScheduleCodecErrorV1::DecisionLimit {
            actual: MAX_SCHEDULE_DECISIONS_V1.saturating_add(1),
            limit: MAX_SCHEDULE_DECISIONS_V1,
        }
    } else if marker == ALLOCATION_MARKER {
        PersistedSimulationScheduleCodecErrorV1::AllocationFailure
    } else if marker == UNSUPPORTED_SCHEMA_MARKER {
        PersistedSimulationScheduleCodecErrorV1::UnsupportedSchema
    } else if matches!(
        error.classify(),
        serde_json::error::Category::Syntax | serde_json::error::Category::Eof
    ) {
        PersistedSimulationScheduleCodecErrorV1::JsonSyntax
    } else {
        PersistedSimulationScheduleCodecErrorV1::JsonStructure
    }
}

fn decode_identity(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut decoded = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        decoded[index] = decode_nibble(pair[0])?
            .checked_shl(4)?
            .checked_add(decode_nibble(pair[1])?)?;
    }
    Some(decoded)
}

const fn decode_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn encode_hex(input: &[u8], output: &mut [u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (index, byte) in input.iter().copied().enumerate() {
        output[index * 2] = HEX[usize::from(byte >> 4)];
        output[index * 2 + 1] = HEX[usize::from(byte & 0x0f)];
    }
}

#[derive(Default)]
struct BoundedVecWriterV1 {
    bytes: Vec<u8>,
    failure: Option<PersistedSimulationScheduleCodecErrorV1>,
}

impl Write for BoundedVecWriterV1 {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(total) = self.bytes.len().checked_add(bytes.len()) else {
            self.failure = Some(PersistedSimulationScheduleCodecErrorV1::NumericOverflow(
                "encoded_bytes",
            ));
            return Err(io::Error::other("persisted schedule byte count overflow"));
        };
        if total > MAX_PERSISTED_SCHEDULE_BYTES_V1 {
            self.failure = Some(PersistedSimulationScheduleCodecErrorV1::ByteLimit {
                actual: total,
                limit: MAX_PERSISTED_SCHEDULE_BYTES_V1,
            });
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "persisted schedule byte limit exceeded",
            ));
        }
        if total > self.bytes.capacity() {
            let doubled = self
                .bytes
                .capacity()
                .max(1_024)
                .saturating_mul(2)
                .min(MAX_PERSISTED_SCHEDULE_BYTES_V1);
            let desired = total.max(doubled);
            if self
                .bytes
                .try_reserve_exact(desired.saturating_sub(self.bytes.len()))
                .is_err()
            {
                self.failure = Some(PersistedSimulationScheduleCodecErrorV1::AllocationFailure);
                return Err(io::Error::other(
                    "cannot allocate bounded persisted schedule bytes",
                ));
            }
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct ExactBytesWriterV1<'a> {
    expected: &'a [u8],
    offset: usize,
    mismatch: bool,
}

impl Write for ExactBytesWriterV1<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(end) = self.offset.checked_add(bytes.len()) else {
            self.offset = usize::MAX;
            self.mismatch = true;
            return Ok(bytes.len());
        };
        if end > self.expected.len() || self.expected.get(self.offset..end) != Some(bytes) {
            self.mismatch = true;
        }
        self.offset = end;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
