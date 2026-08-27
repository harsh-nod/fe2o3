#![forbid(unsafe_code)]
//! Truth-preserving, bounded adapters into Semantic Trace V1.
//!
//! This crate parses inert evidence. It does not load a runtime, enumerate a
//! device, resolve a KIR identity claim, or grant authority to source handles,
//! addresses, artifact identities, or profiler identifiers.

use std::error::Error;
use std::fmt;

use fe2o3_semantic_trace::*;
use serde::de::{IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use sha2::{Digest, Sha256};

pub const MAX_IMPORT_SOURCE_BYTES_V1: u64 = 8 * 1024 * 1024;
pub const MAX_IMPORT_OUTPUT_BYTES_V1: u64 = 64 * 1024;
pub const MAX_ROCPROF_PROCESSES_V1: usize = 4_096;
pub const MAX_ROCPROF_DISPATCHES_PER_PROCESS_V1: usize = 65_536;
pub const ROCPROF_JSON_SOURCE_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.rocprofv3-json.source.v1\0";
pub const ROCPROF_ATT_SOURCE_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.rocprofv3-att-manifest.source.v1\0";

const SOURCE_FORMAT_VERSION_V1: u16 = 1;
const IMPORT_EVENT_LIMIT_V1: u64 = 2;
const IMPORT_EVIDENCE_LIMIT_V1: u16 = 2;
const MAX_TOOL_VERSION_BYTES_V1: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImportLimitsV1 {
    max_source_bytes: u64,
}

impl ImportLimitsV1 {
    pub fn new(max_source_bytes: u64) -> Result<Self, ImportErrorV1> {
        if max_source_bytes == 0 || max_source_bytes > MAX_IMPORT_SOURCE_BYTES_V1 {
            return Err(ImportErrorV1::SourceLimitOutOfRange {
                actual: max_source_bytes,
                max: MAX_IMPORT_SOURCE_BYTES_V1,
            });
        }
        Ok(Self { max_source_bytes })
    }

    pub const fn max_source_bytes(self) -> u64 {
        self.max_source_bytes
    }
}

impl Default for ImportLimitsV1 {
    fn default() -> Self {
        Self {
            max_source_bytes: MAX_IMPORT_SOURCE_BYTES_V1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactClaimV1 {
    pub identity: OpaqueIdentityV1,
    pub canonical_len: u64,
    pub format_version: u16,
}

impl ArtifactClaimV1 {
    pub fn content_identity(self) -> Result<ContentIdentityV1, ImportErrorV1> {
        ContentIdentityV1::new(
            ContentIdentitySchemeV1::RawCanonicalSha256,
            self.format_version,
            self.identity,
            self.canonical_len,
        )
        .map_err(ImportErrorV1::Trace)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RocprofDispatchSelectionV1 {
    pub process_index: usize,
    pub dispatch_index: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RocprofBindingV1 {
    pub kernel_ir_claim: KernelIrIdentityClaimV1,
    pub artifact: Option<ArtifactClaimV1>,
    pub wave_width: WaveWidthV1,
    pub selection: RocprofDispatchSelectionV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SparseImportBindingV1 {
    pub kernel_ir_claim: KernelIrIdentityClaimV1,
    pub artifact: Option<ArtifactClaimV1>,
    pub launch: LaunchGeometryV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportSourceKindV1 {
    Rocprofv3Json,
    Rocprofv3AttManifest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedFactV1 {
    DispatchEnvelope,
    AttCaptureManifest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnavailableImportFactV1 {
    DispatchTiming,
    InvocationHistory,
    WorkgroupHistory,
    WaveHistory,
    LaneHistory,
    KirSiteHistory,
    MemoryHistory,
    RegisterAndValueState,
    DiagnosticAndFaultHistory,
}

const ROCPROF_UNAVAILABLE: [UnavailableImportFactV1; 8] = [
    UnavailableImportFactV1::InvocationHistory,
    UnavailableImportFactV1::WorkgroupHistory,
    UnavailableImportFactV1::WaveHistory,
    UnavailableImportFactV1::LaneHistory,
    UnavailableImportFactV1::KirSiteHistory,
    UnavailableImportFactV1::MemoryHistory,
    UnavailableImportFactV1::RegisterAndValueState,
    UnavailableImportFactV1::DiagnosticAndFaultHistory,
];

const SPARSE_UNAVAILABLE: [UnavailableImportFactV1; 9] = [
    UnavailableImportFactV1::DispatchTiming,
    UnavailableImportFactV1::InvocationHistory,
    UnavailableImportFactV1::WorkgroupHistory,
    UnavailableImportFactV1::WaveHistory,
    UnavailableImportFactV1::LaneHistory,
    UnavailableImportFactV1::KirSiteHistory,
    UnavailableImportFactV1::MemoryHistory,
    UnavailableImportFactV1::RegisterAndValueState,
    UnavailableImportFactV1::DiagnosticAndFaultHistory,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedTraceV1 {
    trace: TraceV1,
    source_kind: ImportSourceKindV1,
    source_identity: ContentIdentityV1,
    imported_facts: &'static [ImportedFactV1],
    unavailable_facts: &'static [UnavailableImportFactV1],
    selected_record_ordinal: Option<u64>,
}

impl ImportedTraceV1 {
    pub const fn trace(&self) -> &TraceV1 {
        &self.trace
    }

    pub fn into_trace(self) -> TraceV1 {
        self.trace
    }

    pub const fn source_kind(&self) -> ImportSourceKindV1 {
        self.source_kind
    }

    pub const fn source_identity(&self) -> ContentIdentityV1 {
        self.source_identity
    }

    pub const fn imported_facts(&self) -> &'static [ImportedFactV1] {
        self.imported_facts
    }

    pub const fn unavailable_facts(&self) -> &'static [UnavailableImportFactV1] {
        self.unavailable_facts
    }

    pub const fn selected_record_ordinal(&self) -> Option<u64> {
        self.selected_record_ordinal
    }
}

/// Imports one selected `buffer_records.kernel_dispatch` record from the
/// documented rocprofv3 programmatic JSON format.
pub fn import_rocprofv3_json_v1(
    source: &[u8],
    binding: RocprofBindingV1,
    limits: ImportLimitsV1,
) -> Result<ImportedTraceV1, ImportErrorV1> {
    validate_source_size(source, limits)?;
    let source_identity = source_identity(ROCPROF_JSON_SOURCE_IDENTITY_DOMAIN_V1, source)?;
    let document: RocprofDocument =
        serde_json::from_slice(source).map_err(|_| ImportErrorV1::InvalidRocprofJson)?;
    let process = document
        .processes
        .get(binding.selection.process_index)
        .ok_or(ImportErrorV1::ProcessNotFound)?;
    let record = process
        .buffer_records
        .kernel_dispatch
        .get(binding.selection.dispatch_index)
        .ok_or(ImportErrorV1::DispatchNotFound)?;
    if record.start_timestamp > record.end_timestamp {
        return Err(ImportErrorV1::TimestampOrder);
    }

    let launch = launch_from_dispatch(record.dispatch_info, binding.wave_width)?;
    let selected_record_ordinal = flattened_record_ordinal(
        &document,
        binding.selection.process_index,
        binding.selection.dispatch_index,
    )?;
    let source_digest = source_identity.digest();
    let dispatch = imported_dispatch_identity(
        b"fe2o3.rocprofv3-json.dispatch.v1\0",
        source_digest,
        selected_record_ordinal,
    )?;
    let clock = derived_identity(
        b"fe2o3.rocprofv3-json.clock.v1\0",
        source_digest,
        selected_record_ordinal,
    )?;
    let evidence = observed_evidence(source_digest, binding.artifact);
    let scope = ExecutionScopeV1::dispatch(dispatch);
    let events = vec![
        TraceEventV1::new(
            0,
            TimestampV1::Clock {
                domain: clock,
                ticks: record.start_timestamp,
            },
            FactProvenanceV1::Observed,
            scope,
            None,
            TraceEventKindV1::Dispatch(DispatchEventV1::Begin),
            evidence.clone(),
        )
        .map_err(ImportErrorV1::Trace)?,
        TraceEventV1::new(
            1,
            TimestampV1::Clock {
                domain: clock,
                ticks: record.end_timestamp,
            },
            FactProvenanceV1::Observed,
            scope,
            None,
            // `Completed` records lifecycle completion at rocprof's documented
            // end timestamp; it is not a kernel-correctness or fault-free claim.
            TraceEventKindV1::Dispatch(DispatchEventV1::End(DispatchOutcomeV1::Completed)),
            evidence,
        )
        .map_err(ImportErrorV1::Trace)?,
    ];
    let trace = build_trace(
        ProducerKindV1::RocprofImporter,
        ExecutionKindV1::RocprofImport,
        "rocprofv3-json-import",
        binding.kernel_ir_claim,
        binding.artifact,
        dispatch,
        launch,
        events,
        CaptureBoundariesV1::FULL_DISPATCH,
    )?;
    Ok(ImportedTraceV1 {
        trace,
        source_kind: ImportSourceKindV1::Rocprofv3Json,
        source_identity,
        imported_facts: &[ImportedFactV1::DispatchEnvelope],
        unavailable_facts: &ROCPROF_UNAVAILABLE,
        selected_record_ordinal: Some(selected_record_ordinal),
    })
}

/// Imports the installed rocprofv3 ATT `filenames.json` manifest as sparse
/// evidence. The manifest has no authenticated KIR-site or launch-event stream,
/// so this adapter intentionally emits no semantic events.
pub fn import_rocprofv3_att_manifest_v1(
    source: &[u8],
    binding: SparseImportBindingV1,
    limits: ImportLimitsV1,
) -> Result<ImportedTraceV1, ImportErrorV1> {
    validate_source_size(source, limits)?;
    let source_identity = source_identity(ROCPROF_ATT_SOURCE_IDENTITY_DOMAIN_V1, source)?;
    let document: AttManifest =
        serde_json::from_slice(source).map_err(|_| ImportErrorV1::InvalidAttManifest)?;
    let current_shape = document.thread_trace == Some(true)
        && document.version.as_deref().is_some_and(valid_tool_text);
    let installed_v1_1_shape = document.thread_trace.is_none()
        && document.version.is_none()
        && document
            .se_filenames
            .as_ref()
            .is_some_and(|value| value.nonempty)
        && document.global_begin_time.is_some()
        && document.gfxv.as_deref().is_some_and(valid_tool_text);
    if !document.wave_filenames.nonempty || !(current_shape || installed_v1_1_shape) {
        return Err(ImportErrorV1::InvalidAttManifest);
    }
    let dispatch = imported_dispatch_identity(
        b"fe2o3.rocprofv3-att-manifest.dispatch.v1\0",
        source_identity.digest(),
        0,
    )?;
    let trace = build_trace(
        ProducerKindV1::RocprofImporter,
        ExecutionKindV1::RocprofImport,
        "rocprofv3-att-manifest-import",
        binding.kernel_ir_claim,
        binding.artifact,
        dispatch,
        binding.launch,
        Vec::new(),
        partial_boundaries(),
    )?;
    Ok(ImportedTraceV1 {
        trace,
        source_kind: ImportSourceKindV1::Rocprofv3AttManifest,
        source_identity,
        imported_facts: &[ImportedFactV1::AttCaptureManifest],
        unavailable_facts: &SPARSE_UNAVAILABLE,
        selected_record_ordinal: None,
    })
}

fn validate_source_size(source: &[u8], limits: ImportLimitsV1) -> Result<(), ImportErrorV1> {
    let actual = u64::try_from(source.len()).map_err(|_| ImportErrorV1::SizeOverflow)?;
    if actual == 0 {
        return Err(ImportErrorV1::EmptySource);
    }
    if actual > limits.max_source_bytes {
        return Err(ImportErrorV1::SourceTooLarge {
            actual,
            max: limits.max_source_bytes,
        });
    }
    Ok(())
}

fn source_identity(domain: &[u8], source: &[u8]) -> Result<ContentIdentityV1, ImportErrorV1> {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(source);
    let digest = OpaqueIdentityV1::new(hasher.finalize().into()).map_err(ImportErrorV1::Trace)?;
    ContentIdentityV1::new(
        ContentIdentitySchemeV1::DomainSeparatedSha256,
        SOURCE_FORMAT_VERSION_V1,
        digest,
        u64::try_from(source.len()).map_err(|_| ImportErrorV1::SizeOverflow)?,
    )
    .map_err(ImportErrorV1::Trace)
}

fn flattened_record_ordinal(
    document: &RocprofDocument,
    process_index: usize,
    dispatch_index: usize,
) -> Result<u64, ImportErrorV1> {
    let prior = document.processes[..process_index]
        .iter()
        .try_fold(0_u64, |total, process| {
            let count = u64::try_from(process.buffer_records.kernel_dispatch.len()).ok()?;
            total.checked_add(count)
        })
        .ok_or(ImportErrorV1::SizeOverflow)?;
    prior
        .checked_add(u64::try_from(dispatch_index).map_err(|_| ImportErrorV1::SizeOverflow)?)
        .ok_or(ImportErrorV1::SizeOverflow)
}

fn launch_from_dispatch(
    dispatch: RocprofDispatchInfo,
    wave_width: WaveWidthV1,
) -> Result<LaunchGeometryV1, ImportErrorV1> {
    let grid = dispatch.grid_size.array();
    let workgroup = dispatch.workgroup_size.array_u32()?;
    if grid.contains(&0) || workgroup.contains(&0) {
        return Err(ImportErrorV1::InvalidLaunchGeometry);
    }
    let mut groups = [0_u32; 3];
    for axis in 0..3 {
        let count = grid[axis]
            .checked_add(u64::from(workgroup[axis]) - 1)
            .ok_or(ImportErrorV1::InvalidLaunchGeometry)?
            / u64::from(workgroup[axis]);
        groups[axis] = u32::try_from(count).map_err(|_| ImportErrorV1::InvalidLaunchGeometry)?;
    }
    LaunchGeometryV1::new_exact(grid, groups, workgroup, wave_width)
        .map_err(|_| ImportErrorV1::InvalidLaunchGeometry)
}

fn observed_evidence(
    source_digest: OpaqueIdentityV1,
    artifact: Option<ArtifactClaimV1>,
) -> Vec<EvidenceRefV1> {
    let mut evidence = Vec::with_capacity(if artifact.is_some() { 2 } else { 1 });
    evidence.push(EvidenceRefV1::new(
        EvidenceKindV1::RuntimeObservation,
        source_digest,
    ));
    if let Some(artifact) = artifact {
        evidence.push(EvidenceRefV1::new(
            EvidenceKindV1::Artifact,
            artifact.identity,
        ));
    }
    evidence
}

#[allow(clippy::too_many_arguments)]
fn build_trace(
    producer_kind: ProducerKindV1,
    execution_kind: ExecutionKindV1,
    producer_name: &str,
    kernel_ir_claim: KernelIrIdentityClaimV1,
    artifact: Option<ArtifactClaimV1>,
    dispatch: DispatchIdentityV1,
    launch: LaunchGeometryV1,
    events: Vec<TraceEventV1>,
    boundaries: CaptureBoundariesV1,
) -> Result<TraceV1, ImportErrorV1> {
    let emitted_events = u64::try_from(events.len()).map_err(|_| ImportErrorV1::SizeOverflow)?;
    let producer = ProducerIdentityV1::new(
        producer_kind,
        ProducerTextV1::new(producer_name).map_err(ImportErrorV1::Trace)?,
        ProducerTextV1::new("v1").map_err(ImportErrorV1::Trace)?,
        None,
    );
    let bounds = TraceBoundsV1::new(
        IMPORT_EVENT_LIMIT_V1,
        MAX_IMPORT_OUTPUT_BYTES_V1,
        IMPORT_EVIDENCE_LIMIT_V1,
    )
    .map_err(ImportErrorV1::Trace)?;
    let header = TraceHeaderV1::new(
        producer,
        execution_kind,
        kernel_ir_claim,
        None,
        None,
        artifact
            .map(ArtifactClaimV1::content_identity)
            .transpose()?,
        dispatch,
        launch,
        bounds,
        TraceCompletenessV1::Truncated {
            reason: TruncationReasonV1::CollectorLoss,
            emitted_events,
            dropped_events: DroppedEventCountV1::Unknown,
        },
        boundaries,
    )
    .map_err(ImportErrorV1::Trace)?;
    TraceV1::new_with_resident_reservation(header, events, 0).map_err(ImportErrorV1::Trace)
}

const fn partial_boundaries() -> CaptureBoundariesV1 {
    CaptureBoundariesV1::new(
        CaptureStartBoundaryV1::DispatchAlreadyActive,
        CaptureEndBoundaryV1::DispatchContinuesAfterCapture,
    )
}

fn imported_dispatch_identity(
    domain: &[u8],
    source: OpaqueIdentityV1,
    ordinal: u64,
) -> Result<DispatchIdentityV1, ImportErrorV1> {
    Ok(DispatchIdentityV1::new(
        DispatchIdentityDomainV1::ImportedCollector,
        derived_identity(domain, source, ordinal)?,
    ))
}

fn derived_identity(
    domain: &[u8],
    source: OpaqueIdentityV1,
    ordinal: u64,
) -> Result<OpaqueIdentityV1, ImportErrorV1> {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(source.as_bytes());
    digest.update(ordinal.to_le_bytes());
    OpaqueIdentityV1::new(digest.finalize().into()).map_err(ImportErrorV1::Trace)
}

fn valid_tool_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_TOOL_VERSION_BYTES_V1 && !value.contains('\0')
}

#[derive(Deserialize)]
struct RocprofDocument {
    #[serde(rename = "rocprofiler-sdk-tool")]
    processes: BoundedVec<RocprofProcess, MAX_ROCPROF_PROCESSES_V1>,
}

#[derive(Deserialize)]
struct RocprofProcess {
    buffer_records: RocprofBufferRecords,
}

#[derive(Deserialize)]
struct RocprofBufferRecords {
    #[serde(default)]
    kernel_dispatch: BoundedVec<RocprofDispatchRecord, MAX_ROCPROF_DISPATCHES_PER_PROCESS_V1>,
}

#[derive(Clone, Copy, Deserialize)]
struct RocprofDispatchRecord {
    start_timestamp: u64,
    end_timestamp: u64,
    dispatch_info: RocprofDispatchInfo,
}

#[derive(Clone, Copy, Deserialize)]
struct RocprofDispatchInfo {
    workgroup_size: JsonDimensions,
    grid_size: JsonDimensions,
}

#[derive(Clone, Copy, Deserialize)]
struct JsonDimensions {
    x: u64,
    y: u64,
    z: u64,
}

impl JsonDimensions {
    const fn array(self) -> [u64; 3] {
        [self.x, self.y, self.z]
    }

    fn array_u32(self) -> Result<[u32; 3], ImportErrorV1> {
        Ok([
            u32::try_from(self.x).map_err(|_| ImportErrorV1::InvalidLaunchGeometry)?,
            u32::try_from(self.y).map_err(|_| ImportErrorV1::InvalidLaunchGeometry)?,
            u32::try_from(self.z).map_err(|_| ImportErrorV1::InvalidLaunchGeometry)?,
        ])
    }
}

#[derive(Deserialize)]
struct AttManifest {
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    thread_trace: Option<bool>,
    wave_filenames: NonEmptyObject,
    #[serde(default)]
    se_filenames: Option<NonEmptyContainer>,
    #[serde(default)]
    global_begin_time: Option<u64>,
    #[serde(default)]
    gfxv: Option<String>,
}

struct BoundedVec<T, const MAX: usize>(Vec<T>);

impl<T, const MAX: usize> BoundedVec<T, MAX> {
    fn get(&self, index: usize) -> Option<&T> {
        self.0.get(index)
    }

    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<T, const MAX: usize> std::ops::Deref for BoundedVec<T, MAX> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T, const MAX: usize> Default for BoundedVec<T, MAX> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<'de, T, const MAX: usize> Deserialize<'de> for BoundedVec<T, MAX>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct BoundedVecVisitor<T, const MAX: usize>(std::marker::PhantomData<T>);

        impl<'de, T, const MAX: usize> Visitor<'de> for BoundedVecVisitor<T, MAX>
        where
            T: Deserialize<'de>,
        {
            type Value = BoundedVec<T, MAX>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "an array with at most {MAX} items")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let initial = sequence.size_hint().unwrap_or(0).min(MAX);
                let mut values = Vec::new();
                values
                    .try_reserve_exact(initial)
                    .map_err(serde::de::Error::custom)?;
                if values.capacity() > MAX {
                    return Err(serde::de::Error::custom(
                        "JSON sequence allocation exceeded its item bound",
                    ));
                }
                while let Some(value) = sequence.next_element()? {
                    if values.len() == MAX {
                        return Err(serde::de::Error::invalid_length(MAX + 1, &self));
                    }
                    if values.len() == values.capacity() {
                        let target = values
                            .capacity()
                            .max(1)
                            .saturating_mul(2)
                            .min(MAX)
                            .max(values.len() + 1);
                        values
                            .try_reserve_exact(target - values.capacity())
                            .map_err(serde::de::Error::custom)?;
                        if values.capacity() > MAX {
                            return Err(serde::de::Error::custom(
                                "JSON sequence allocation exceeded its item bound",
                            ));
                        }
                    }
                    values.push(value);
                }
                Ok(BoundedVec(values))
            }
        }

        deserializer.deserialize_seq(BoundedVecVisitor(std::marker::PhantomData))
    }
}

struct NonEmptyObject {
    nonempty: bool,
}

struct NonEmptyContainer {
    nonempty: bool,
}

impl<'de> Deserialize<'de> for NonEmptyContainer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ContainerVisitor;

        impl<'de> Visitor<'de> for ContainerVisitor {
            type Value = NonEmptyContainer;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a nonempty JSON array or object")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut nonempty = false;
                while sequence.next_element::<IgnoredAny>()?.is_some() {
                    nonempty = true;
                }
                Ok(NonEmptyContainer { nonempty })
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut nonempty = false;
                while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {
                    nonempty = true;
                }
                Ok(NonEmptyContainer { nonempty })
            }
        }

        deserializer.deserialize_any(ContainerVisitor)
    }
}

impl<'de> Deserialize<'de> for NonEmptyObject {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ObjectVisitor;

        impl<'de> Visitor<'de> for ObjectVisitor {
            type Value = NonEmptyObject;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a nonempty JSON object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut nonempty = false;
                while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {
                    nonempty = true;
                }
                Ok(NonEmptyObject { nonempty })
            }
        }

        deserializer.deserialize_map(ObjectVisitor)
    }
}

#[derive(Debug)]
pub enum ImportErrorV1 {
    SourceLimitOutOfRange { actual: u64, max: u64 },
    EmptySource,
    SourceTooLarge { actual: u64, max: u64 },
    SizeOverflow,
    InvalidRocprofJson,
    ProcessNotFound,
    DispatchNotFound,
    TimestampOrder,
    InvalidLaunchGeometry,
    InvalidAttManifest,
    Trace(TraceValidationErrorV1),
}

impl fmt::Display for ImportErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceLimitOutOfRange { actual, max } => {
                write!(formatter, "source limit {actual} is outside 1..={max}")
            }
            Self::EmptySource => formatter.write_str("source evidence is empty"),
            Self::SourceTooLarge { actual, max } => {
                write!(
                    formatter,
                    "source evidence has {actual} bytes; limit is {max}"
                )
            }
            Self::SizeOverflow => formatter.write_str("import size arithmetic overflowed"),
            Self::InvalidRocprofJson => formatter.write_str("invalid rocprofv3 JSON evidence"),
            Self::ProcessNotFound => formatter.write_str("selected rocprofv3 process is absent"),
            Self::DispatchNotFound => formatter.write_str("selected rocprofv3 dispatch is absent"),
            Self::TimestampOrder => formatter.write_str("dispatch timestamps are reversed"),
            Self::InvalidLaunchGeometry => {
                formatter.write_str("dispatch launch geometry is invalid")
            }
            Self::InvalidAttManifest => {
                formatter.write_str("invalid rocprofv3 ATT filenames.json evidence")
            }
            Self::Trace(error) => {
                write!(formatter, "Semantic Trace V1 rejected the import: {error}")
            }
        }
    }
}

impl Error for ImportErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Trace(error) => Some(error),
            _ => None,
        }
    }
}
