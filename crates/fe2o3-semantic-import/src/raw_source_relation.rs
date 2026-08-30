use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use fe2o3_semantic_trace::{
    ContentIdentitySchemeV1, ContentIdentityV1, KernelIrIdentityClaimV1, OpaqueIdentityV1,
    WaveWidthV1,
};

use super::*;

const BUNDLE_RELATION_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.semantic-import.rocprof-bundle-raw-source-relation.v1\0";

#[derive(Debug)]
pub enum RocprofRawSourceRelationErrorV1 {
    Source(ImportErrorV1),
    RelationMismatch,
}

impl fmt::Display for RocprofRawSourceRelationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "rocprofv3 source admission failed: {error}"),
            Self::RelationMismatch => formatter
                .write_str("valid rocprofv3 JSON does not exactly relate the normalized evidence"),
        }
    }
}

impl Error for RocprofRawSourceRelationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::RelationMismatch => None,
        }
    }
}

impl From<ImportErrorV1> for RocprofRawSourceRelationErrorV1 {
    fn from(value: ImportErrorV1) -> Self {
        Self::Source(value)
    }
}

/// In-process proof that the source-derived Bundle V4 facts are the exact
/// normalization of supplied, bounded rocprofv3 JSON bytes under the retained
/// declarations. Schema-valid trailing unused device entries are ignored.
/// This is content binding, not a signature or external provenance claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RocprofBundleRawSourceRelationV1 {
    source: ContentIdentityRecordV1,
    bundle: ContentIdentityRecordV1,
    dispatch_count: u32,
}

impl RocprofBundleRawSourceRelationV1 {
    pub const fn source(self) -> ContentIdentityRecordV1 {
        self.source
    }

    pub const fn dispatch_count(self) -> u32 {
        self.dispatch_count
    }
}

/// Exact process-local `dispatch_id` association from Counter Capture V2
/// dispatch order to Bundle V4 dispatch order. The V2 wire stays unchanged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RocprofCounterBundleRelationV1 {
    bundle_dispatch_ordinals: Vec<u32>,
}

impl RocprofCounterBundleRelationV1 {
    pub fn bundle_dispatch_ordinals(&self) -> &[u32] {
        &self.bundle_dispatch_ordinals
    }
}

/// Exact process-local `dispatch_id` association from PC Capture V3 dispatch
/// order to Bundle V4 dispatch order. The V3 wire stays unchanged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RocprofPcBundleRelationV1 {
    bundle_dispatch_ordinals: Vec<u32>,
}

impl RocprofPcBundleRelationV1 {
    pub fn bundle_dispatch_ordinals(&self) -> &[u32] {
        &self.bundle_dispatch_ordinals
    }
}

pub fn rocprofv3_json_source_content_identity_v1(
    source: &[u8],
    limits: ImportLimitsV1,
) -> Result<ContentIdentityRecordV1, ImportErrorV1> {
    validate_source_size(source, limits)?;
    source_identity(ROCPROF_JSON_SOURCE_IDENTITY_DOMAIN_V1, source)
        .map(ContentIdentityRecordV1::from)
}

pub fn validate_rocprofv3_bundle_raw_source_relation_v1(
    source: &[u8],
    bundle: &SemanticProfilerBundleV4,
    limits: ImportLimitsV1,
) -> Result<RocprofBundleRawSourceRelationV1, RocprofRawSourceRelationErrorV1> {
    validate_source_size(source, limits)?;
    bundle
        .validate()
        .map_err(|_| RocprofRawSourceRelationErrorV1::RelationMismatch)?;
    if bundle.source_kind != ProfilerSourceKindV4::Rocprofv3KernelDispatchJson {
        return Err(RocprofRawSourceRelationErrorV1::RelationMismatch);
    }
    let document: RocprofDocument = serde_json::from_slice(source)
        .map_err(|_| RocprofRawSourceRelationErrorV1::Source(ImportErrorV1::InvalidRocprofJson))?;
    let capture = bundle
        .dispatch_capture
        .as_ref()
        .ok_or(RocprofRawSourceRelationErrorV1::RelationMismatch)?;
    let first = capture
        .dispatches
        .first()
        .ok_or(RocprofRawSourceRelationErrorV1::RelationMismatch)?;
    let source_identity = rocprofv3_json_source_content_identity_v1(source, limits)?;
    if capture.runs.len() != 1
        || capture.runs[0].source != source_identity
        || bundle.source.value != Some(source_identity)
    {
        return Err(RocprofRawSourceRelationErrorV1::RelationMismatch);
    }

    let source_agent_ids = source_agent_ids(&document)?;
    if source_agent_ids.len() > bundle.devices.len() {
        return Err(RocprofRawSourceRelationErrorV1::RelationMismatch);
    }
    let stable_device_bindings = source_agent_ids
        .into_iter()
        .zip(&bundle.devices)
        .map(|(source_agent_id, device)| {
            Ok(ProfilerDeviceBindingV4 {
                source_agent_id,
                stable_identity: device
                    .stable_identity
                    .value
                    .ok_or(RocprofRawSourceRelationErrorV1::RelationMismatch)?,
            })
        })
        .collect::<Result<Vec<_>, RocprofRawSourceRelationErrorV1>>()?;
    let binding = ProfilerDispatchBindingV4 {
        environment: ProfilerEnvironmentBindingV4 {
            environment: available_profiler_identity(bundle.environment)?,
            collector_tool: available_profiler_identity(bundle.collector_tool)?,
            collector_configuration: available_profiler_identity(bundle.collector_configuration)?,
            stable_device_bindings,
        },
        kernel_ir_claim: kernel_ir_claim(first.kernel_ir)?,
        artifact: artifact_claim(first.artifact)?,
        source_map: content_claim(first.source_map)?,
        wave_width: wave_width(first.launch.wave_width)?,
    };
    let normalized = import_rocprofv3_json_profiler_bundle_v4(source, binding)
        .map_err(|_| RocprofRawSourceRelationErrorV1::RelationMismatch)?;
    if !bundle_matches_allowing_trailing_unused_devices(&normalized, bundle) {
        return Err(RocprofRawSourceRelationErrorV1::RelationMismatch);
    }
    Ok(RocprofBundleRawSourceRelationV1 {
        source: source_identity,
        bundle: bundle_relation_identity(bundle)?,
        dispatch_count: u32::try_from(capture.dispatches.len())
            .map_err(|_| ImportErrorV1::SizeOverflow)?,
    })
}

fn bundle_matches_allowing_trailing_unused_devices(
    normalized: &SemanticProfilerBundleV4,
    supplied: &SemanticProfilerBundleV4,
) -> bool {
    let Some(normalized_capture) = &normalized.dispatch_capture else {
        return false;
    };
    let Some(supplied_capture) = &supplied.dispatch_capture else {
        return false;
    };
    if supplied.devices.len() < normalized.devices.len()
        || supplied_capture.devices.len() < normalized_capture.devices.len()
    {
        return false;
    }
    let used = supplied_capture
        .dispatches
        .iter()
        .map(|dispatch| dispatch.device_identity)
        .collect::<BTreeSet<_>>();
    if supplied_capture.devices[normalized_capture.devices.len()..]
        .iter()
        .any(|device| used.contains(&device.identity))
    {
        return false;
    }
    let mut projected = supplied.clone();
    projected.devices.truncate(normalized.devices.len());
    projected
        .dispatch_capture
        .as_mut()
        .expect("checked above")
        .devices
        .truncate(normalized_capture.devices.len());
    &projected == normalized
}

pub fn validate_rocprofv3_counter_bundle_relation_v1(
    source: &[u8],
    bundle: &SemanticProfilerBundleV4,
    admitted_source: RocprofBundleRawSourceRelationV1,
    counters: &SemanticCounterCaptureV2,
    limits: ImportLimitsV1,
) -> Result<RocprofCounterBundleRelationV1, RocprofRawSourceRelationErrorV1> {
    let source_identity = rocprofv3_json_source_content_identity_v1(source, limits)?;
    if source_identity != admitted_source.source
        || bundle_relation_identity(bundle)? != admitted_source.bundle
        || counters.runs.len() != 1
        || counters.runs[0].source != source_identity
    {
        return Err(RocprofRawSourceRelationErrorV1::RelationMismatch);
    }
    let first = counters
        .dispatches
        .first()
        .ok_or(RocprofRawSourceRelationErrorV1::RelationMismatch)?;
    let normalized = import_rocprofv3_counter_capture_v2(
        source,
        RocprofCaptureBindingV1 {
            kernel_ir_claim: kernel_ir_claim(first.kernel_ir)?,
            artifact: artifact_claim(first.artifact)?,
            source_map: content_claim(first.source_map)?,
            wave_width: wave_width(first.launch.wave_width)?,
        },
        limits,
    )?;
    if &normalized != counters {
        return Err(RocprofRawSourceRelationErrorV1::RelationMismatch);
    }

    let document: RocprofDocument = serde_json::from_slice(source)
        .map_err(|_| RocprofRawSourceRelationErrorV1::Source(ImportErrorV1::InvalidRocprofJson))?;
    let mut mapping = Vec::new();
    let mut flattened_base = 0_u32;
    for process in document.processes.iter() {
        let mut dispatch_ids = BTreeMap::new();
        for (dispatch_index, dispatch) in process.buffer_records.kernel_dispatch.iter().enumerate()
        {
            let dispatch_id = dispatch
                .dispatch_info
                .dispatch_id
                .ok_or(RocprofRawSourceRelationErrorV1::RelationMismatch)?;
            let ordinal = flattened_base
                .checked_add(
                    u32::try_from(dispatch_index).map_err(|_| ImportErrorV1::SizeOverflow)?,
                )
                .ok_or(ImportErrorV1::SizeOverflow)?;
            if dispatch_ids
                .insert(dispatch_id, (ordinal, dispatch))
                .is_some()
            {
                return Err(RocprofRawSourceRelationErrorV1::RelationMismatch);
            }
        }
        for collection in process.callback_records.counter_collection.iter() {
            let dispatch_id = collection
                .dispatch_data
                .dispatch_info
                .dispatch_id
                .ok_or(RocprofRawSourceRelationErrorV1::RelationMismatch)?;
            let (ordinal, dispatch) = dispatch_ids
                .get(&dispatch_id)
                .copied()
                .ok_or(RocprofRawSourceRelationErrorV1::RelationMismatch)?;
            if !same_raw_dispatch(collection.dispatch_data, *dispatch) {
                return Err(RocprofRawSourceRelationErrorV1::RelationMismatch);
            }
            mapping.push(ordinal);
        }
        flattened_base = flattened_base
            .checked_add(
                u32::try_from(process.buffer_records.kernel_dispatch.len())
                    .map_err(|_| ImportErrorV1::SizeOverflow)?,
            )
            .ok_or(ImportErrorV1::SizeOverflow)?;
    }
    if mapping.len() != counters.dispatches.len()
        || mapping.len()
            != usize::try_from(admitted_source.dispatch_count)
                .map_err(|_| ImportErrorV1::SizeOverflow)?
        || mapping.iter().copied().collect::<BTreeSet<_>>().len() != mapping.len()
    {
        return Err(RocprofRawSourceRelationErrorV1::RelationMismatch);
    }
    verify_counter_bundle_axes(bundle, counters, &mapping)?;
    Ok(RocprofCounterBundleRelationV1 {
        bundle_dispatch_ordinals: mapping,
    })
}

pub fn validate_rocprofv3_pc_bundle_relation_v1(
    source: &[u8],
    bundle: &SemanticProfilerBundleV4,
    admitted_source: RocprofBundleRawSourceRelationV1,
    pc: &SemanticPcSampleCaptureV3,
    limits: ImportLimitsV1,
) -> Result<RocprofPcBundleRelationV1, RocprofRawSourceRelationErrorV1> {
    let source_identity = rocprofv3_json_source_content_identity_v1(source, limits)?;
    if source_identity != admitted_source.source
        || bundle_relation_identity(bundle)? != admitted_source.bundle
        || pc.runs.len() != 1
        || pc.runs[0].source != source_identity
    {
        return Err(RocprofRawSourceRelationErrorV1::RelationMismatch);
    }
    let first = pc
        .dispatches
        .first()
        .ok_or(RocprofRawSourceRelationErrorV1::RelationMismatch)?;
    let normalized = import_rocprofv3_pc_sample_capture_v3(
        source,
        RocprofPcSampleCaptureBindingV3 {
            capture: RocprofCaptureBindingV1 {
                kernel_ir_claim: kernel_ir_claim(first.kernel_ir)?,
                artifact: artifact_claim(first.artifact)?,
                source_map: content_claim(first.source_map)?,
                wave_width: wave_width(first.launch.wave_width)?,
            },
            sampling_interval_cycles: pc.coverage.sampling.interval,
        },
        limits,
    )?;
    if &normalized != pc {
        return Err(RocprofRawSourceRelationErrorV1::RelationMismatch);
    }

    let document: RocprofPcDocument = serde_json::from_slice(source)
        .map_err(|_| RocprofRawSourceRelationErrorV1::Source(ImportErrorV1::InvalidRocprofJson))?;
    let mut mapping = Vec::new();
    let mut flattened = 0_u32;
    for process in document.processes.iter() {
        let sampled = process
            .buffer_records
            .pc_sample_stochastic
            .iter()
            .map(|sample| sample.record.dispatch_id)
            .collect::<BTreeSet<_>>();
        let mut seen = BTreeSet::new();
        for dispatch in process.buffer_records.kernel_dispatch.iter() {
            let id = dispatch.dispatch_info.dispatch_id;
            if sampled.contains(&id) {
                if !seen.insert(id) {
                    return Err(RocprofRawSourceRelationErrorV1::RelationMismatch);
                }
                mapping.push(flattened);
            }
            flattened = flattened
                .checked_add(1)
                .ok_or(ImportErrorV1::SizeOverflow)?;
        }
        if sampled != seen {
            return Err(RocprofRawSourceRelationErrorV1::RelationMismatch);
        }
    }
    if mapping.len() != pc.dispatches.len() {
        return Err(RocprofRawSourceRelationErrorV1::RelationMismatch);
    }
    verify_pc_bundle_axes(bundle, pc, &mapping)?;
    Ok(RocprofPcBundleRelationV1 {
        bundle_dispatch_ordinals: mapping,
    })
}

fn bundle_relation_identity(
    bundle: &SemanticProfilerBundleV4,
) -> Result<ContentIdentityRecordV1, RocprofRawSourceRelationErrorV1> {
    let encoded = serde_json::to_vec(bundle)
        .map_err(|_| RocprofRawSourceRelationErrorV1::RelationMismatch)?;
    source_identity(BUNDLE_RELATION_IDENTITY_DOMAIN_V1, &encoded)
        .map(ContentIdentityRecordV1::from)
        .map_err(RocprofRawSourceRelationErrorV1::Source)
}

fn source_agent_ids(
    document: &RocprofDocument,
) -> Result<Vec<u64>, RocprofRawSourceRelationErrorV1> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for process in document.processes.iter() {
        for dispatch in process.buffer_records.kernel_dispatch.iter() {
            let agent = dispatch
                .dispatch_info
                .agent_id
                .ok_or(ImportErrorV1::MissingCaptureDeviceIdentity)?
                .handle;
            if seen.insert(agent) {
                output.push(agent);
            }
        }
    }
    Ok(output)
}

fn same_raw_dispatch(counter: RocprofCounterDispatchData, dispatch: RocprofDispatchRecord) -> bool {
    counter.start_timestamp == dispatch.start_timestamp
        && counter.end_timestamp == dispatch.end_timestamp
        && counter.dispatch_info.agent_id.map(|value| value.handle)
            == dispatch.dispatch_info.agent_id.map(|value| value.handle)
        && counter.dispatch_info.dispatch_id == dispatch.dispatch_info.dispatch_id
        && counter.dispatch_info.workgroup_size.array()
            == dispatch.dispatch_info.workgroup_size.array()
        && counter.dispatch_info.grid_size.array() == dispatch.dispatch_info.grid_size.array()
}

fn verify_counter_bundle_axes(
    bundle: &SemanticProfilerBundleV4,
    counters: &SemanticCounterCaptureV2,
    mapping: &[u32],
) -> Result<(), RocprofRawSourceRelationErrorV1> {
    let dispatches = &bundle
        .dispatch_capture
        .as_ref()
        .ok_or(RocprofRawSourceRelationErrorV1::RelationMismatch)?
        .dispatches;
    for (counter, ordinal) in counters.dispatches.iter().zip(mapping) {
        let dispatch = dispatches
            .get(usize::try_from(*ordinal).map_err(|_| ImportErrorV1::SizeOverflow)?)
            .ok_or(RocprofRawSourceRelationErrorV1::RelationMismatch)?;
        if counter.kernel_ir != dispatch.kernel_ir
            || counter.process_index != dispatch.process_index
            || counter.device_identity != dispatch.device_identity
            || counter.artifact != dispatch.artifact
            || counter.source_map != dispatch.source_map
            || counter.launch != dispatch.launch
            || counter.start_timestamp != dispatch.start_timestamp
            || counter.end_timestamp != dispatch.end_timestamp
            || counter.duration_ticks != dispatch.duration_ticks
        {
            return Err(RocprofRawSourceRelationErrorV1::RelationMismatch);
        }
    }
    Ok(())
}

fn verify_pc_bundle_axes(
    bundle: &SemanticProfilerBundleV4,
    pc: &SemanticPcSampleCaptureV3,
    mapping: &[u32],
) -> Result<(), RocprofRawSourceRelationErrorV1> {
    let dispatches = &bundle
        .dispatch_capture
        .as_ref()
        .ok_or(RocprofRawSourceRelationErrorV1::RelationMismatch)?
        .dispatches;
    for (sample, ordinal) in pc.dispatches.iter().zip(mapping) {
        let dispatch = dispatches
            .get(usize::try_from(*ordinal).map_err(|_| ImportErrorV1::SizeOverflow)?)
            .ok_or(RocprofRawSourceRelationErrorV1::RelationMismatch)?;
        if sample.process_index != dispatch.process_index
            || sample.dispatch_index != dispatch.dispatch_index
            || sample.source_dispatch_ordinal != dispatch.source_record_ordinal
            || sample.device_identity != dispatch.device_identity
            || sample.kernel_ir != dispatch.kernel_ir
            || sample.artifact != dispatch.artifact
            || sample.source_map != dispatch.source_map
            || sample.launch != dispatch.launch
            || sample.start_timestamp != dispatch.start_timestamp
            || sample.end_timestamp != dispatch.end_timestamp
            || sample.duration_ticks != dispatch.duration_ticks
        {
            return Err(RocprofRawSourceRelationErrorV1::RelationMismatch);
        }
    }
    Ok(())
}

fn available_profiler_identity(
    fact: ProfilerIdentityFactV4,
) -> Result<ContentIdentityRecordV1, RocprofRawSourceRelationErrorV1> {
    fact.value
        .ok_or(RocprofRawSourceRelationErrorV1::RelationMismatch)
}

fn kernel_ir_claim(
    claim: KernelIrClaimRecordV1,
) -> Result<KernelIrIdentityClaimV1, RocprofRawSourceRelationErrorV1> {
    if claim.origin != TruthOriginV1::Declared
        || claim.wire_version != 7
        || claim.identity_policy != 1
    {
        return Err(RocprofRawSourceRelationErrorV1::RelationMismatch);
    }
    KernelIrIdentityClaimV1::canonical_v7_claim(
        OpaqueIdentityV1::new(claim.digest.as_bytes()).map_err(ImportErrorV1::Trace)?,
        claim.canonical_len,
    )
    .map_err(|error| RocprofRawSourceRelationErrorV1::Source(ImportErrorV1::Trace(error)))
}

fn artifact_claim(
    fact: IdentityFactV1,
) -> Result<Option<ArtifactClaimV1>, RocprofRawSourceRelationErrorV1> {
    match (fact.origin, fact.value, fact.unavailable_reason) {
        (TruthOriginV1::Declared, Some(value), None)
            if value.scheme == ContentSchemeV1::RawCanonicalSha256 =>
        {
            Ok(Some(ArtifactClaimV1 {
                identity: OpaqueIdentityV1::new(value.digest.as_bytes())
                    .map_err(ImportErrorV1::Trace)?,
                canonical_len: value.canonical_len,
                format_version: value.format_version,
            }))
        }
        (TruthOriginV1::Unavailable, None, Some(CaptureUnavailableReasonV1::NotProvided)) => {
            Ok(None)
        }
        _ => Err(RocprofRawSourceRelationErrorV1::RelationMismatch),
    }
}

fn content_claim(
    fact: IdentityFactV1,
) -> Result<Option<ContentIdentityV1>, RocprofRawSourceRelationErrorV1> {
    match (fact.origin, fact.value, fact.unavailable_reason) {
        (TruthOriginV1::Declared, Some(value), None) => {
            let scheme = match value.scheme {
                ContentSchemeV1::RawCanonicalSha256 => ContentIdentitySchemeV1::RawCanonicalSha256,
                ContentSchemeV1::DomainSeparatedSha256 => {
                    ContentIdentitySchemeV1::DomainSeparatedSha256
                }
            };
            ContentIdentityV1::new(
                scheme,
                value.format_version,
                OpaqueIdentityV1::new(value.digest.as_bytes()).map_err(ImportErrorV1::Trace)?,
                value.canonical_len,
            )
            .map(Some)
            .map_err(|error| RocprofRawSourceRelationErrorV1::Source(ImportErrorV1::Trace(error)))
        }
        (TruthOriginV1::Unavailable, None, Some(CaptureUnavailableReasonV1::NotProvided)) => {
            Ok(None)
        }
        _ => Err(RocprofRawSourceRelationErrorV1::RelationMismatch),
    }
}

fn wave_width(width: u16) -> Result<WaveWidthV1, RocprofRawSourceRelationErrorV1> {
    match width {
        32 => Ok(WaveWidthV1::Wave32),
        64 => Ok(WaveWidthV1::Wave64),
        _ => Err(RocprofRawSourceRelationErrorV1::RelationMismatch),
    }
}
