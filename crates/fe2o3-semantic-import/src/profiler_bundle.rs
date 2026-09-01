//! Unified, bounded profiler capture bundle.
//!
//! This schema composes existing closed dispatch/counter/sample formats with
//! exact environment and collector claims. It references ATT/Compute Viewer
//! artifacts but deliberately does not decode thread-trace payloads.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use fe2o3_semantic_trace::{ContentIdentityV1, KernelIrIdentityClaimV1, WaveWidthV1};
use serde::de::IgnoredAny;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ArtifactClaimV1, CaptureIdentityV1, CaptureUnavailableReasonV1, ContentIdentityRecordV1,
    ContentSchemeV1, ImportLimitsV1, LossStateV1, LossStatusV1, MAX_ROCPROF_PROCESSES_V1,
    RocprofCaptureBindingV1, SemanticCaptureV1, TruthOriginV1,
    import_rocprofv3_capture_with_agents_v1,
};

pub const PROFILER_BUNDLE_SCHEMA_VERSION_V4: u16 = 4;
pub const MAX_PROFILER_BUNDLE_BYTES_V4: u64 = 16 * 1024 * 1024;
pub const MAX_PROFILER_SOURCE_BYTES_V4: u64 = 8 * 1024 * 1024;
pub const MAX_PROFILER_DISPATCHES_V4: usize = 16_384;
pub const MAX_PROFILER_DEVICE_BINDINGS_V4: usize = 256;
/// Maximum process-local agent occurrences retained by JSON projection. One
/// physical KFD device may appear under different opaque handles in different
/// traced processes, so this is intentionally distinct from the device bound.
pub const MAX_PROFILER_SOURCE_AGENT_MAPPINGS_V4: usize = MAX_PROFILER_DISPATCHES_V4;
pub const MAX_PROFILER_ATT_REFERENCES_V4: usize = 512;
pub const MAX_PROFILER_REFERENCE_BYTES_V4: usize = 256;
pub const MAX_PROFILER_CSV_COLUMNS_V4: usize = 32;
pub const MAX_PROFILER_CSV_FIELD_BYTES_V4: usize = 256;
/// Kernel names can contain long Rust demanglings. This remains subordinate to
/// the 8 MiB source bound while fixed and numeric fields retain the tight cap.
pub const MAX_PROFILER_CSV_KERNEL_NAME_BYTES_V4: usize = 64 * 1024;
pub const PROFILER_BUNDLE_IDENTITY_DOMAIN_V4: &[u8] = b"fe2o3.semantic-profiler-bundle.v4\0";

const PROFILER_SOURCE_CSV_DOMAIN_V4: &[u8] = b"fe2o3.profiler.rocprof-csv.v4\0";
const PROFILER_SOURCE_JSON_DOMAIN_V4: &[u8] = b"fe2o3.profiler.rocprof-json.v4\0";
const PROFILER_SOURCE_ATT_DOMAIN_V4: &[u8] = b"fe2o3.profiler.rocprof-att-manifest.v4\0";
const PROFILER_RUN_IDENTITY_DOMAIN_V4: &[u8] = b"fe2o3.profiler.run.v4\0";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerSourceKindV4 {
    Rocprofv3KernelDispatchJson,
    Rocprofv3KernelDispatchCsv,
    Rocprofv3AttComputeViewerManifest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerIdentityRoleV4 {
    Environment,
    CollectorTool,
    CollectorConfiguration,
    StableDevice,
    SourceEvidence,
    NormalizedProjection,
    KernelArtifact,
    AttReferencedArtifact,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerIdentityFactV4 {
    pub role: ProfilerIdentityRoleV4,
    pub origin: TruthOriginV1,
    pub value: Option<ContentIdentityRecordV1>,
    pub unavailable_reason: Option<CaptureUnavailableReasonV1>,
}

impl ProfilerIdentityFactV4 {
    pub const fn declared(role: ProfilerIdentityRoleV4, value: ContentIdentityRecordV1) -> Self {
        Self {
            role,
            origin: TruthOriginV1::Declared,
            value: Some(value),
            unavailable_reason: None,
        }
    }

    pub const fn observed(role: ProfilerIdentityRoleV4, value: ContentIdentityRecordV1) -> Self {
        Self {
            role,
            origin: TruthOriginV1::Observed,
            value: Some(value),
            unavailable_reason: None,
        }
    }

    pub const fn unavailable(
        role: ProfilerIdentityRoleV4,
        reason: CaptureUnavailableReasonV1,
    ) -> Self {
        Self {
            role,
            origin: TruthOriginV1::Unavailable,
            value: None,
            unavailable_reason: Some(reason),
        }
    }

    fn validate(self, expected: ProfilerIdentityRoleV4) -> Result<(), ProfilerBundleErrorV4> {
        if self.role != expected {
            return Err(ProfilerBundleErrorV4::IdentityRoleMismatch);
        }
        match (self.origin, self.value, self.unavailable_reason) {
            (TruthOriginV1::Declared | TruthOriginV1::Observed, Some(value), None) => {
                validate_content_identity(value)
            }
            (TruthOriginV1::Unavailable, None, Some(_)) => Ok(()),
            _ => Err(ProfilerBundleErrorV4::InvalidIdentityFact),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfilerEnvironmentBindingV4 {
    pub environment: ContentIdentityRecordV1,
    pub collector_tool: ContentIdentityRecordV1,
    pub collector_configuration: ContentIdentityRecordV1,
    pub stable_device_bindings: Vec<ProfilerDeviceBindingV4>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfilerDeviceBindingV4 {
    pub source_agent_id: u64,
    pub stable_identity: ContentIdentityRecordV1,
}

/// Exact rocprof JSON catalog fields required to map an opaque dispatch agent
/// handle to one direct-KFD node. This record remains inert; the caller must
/// compare every hardware field with an independently observed KFD owner.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RocprofJsonGpuAgentBindingV4 {
    pub process_index: u32,
    pub process_id: u64,
    pub source_agent_id: u64,
    pub node_id: u32,
    pub gpu_id: u64,
    pub simd_count: u64,
    pub vendor_id: u64,
    pub device_id: u64,
    pub location_id: u64,
    pub domain: u64,
    pub gfx_target_version: u64,
    pub wave_front_size: u64,
    pub num_xcc: u64,
}

/// Bounded canonical projection that replaces each process-local rocprofiler
/// agent handle with the absolute KFD node admitted through that process's
/// `agents[]` catalog. The raw source remains a separate observed object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RocprofJsonDispatchProjectionV4 {
    dialect: RocprofDispatchSchemaDialectV4,
    canonical_json: Vec<u8>,
    agent_bindings: Vec<RocprofJsonGpuAgentBindingV4>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RocprofCsvSourceAgentBindingV4 {
    pub process_index: u32,
    pub process_id: Option<u64>,
    pub node_id: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RocprofDispatchSchemaDialectV4 {
    InstalledRocprofv3_1_1_97f5574,
    ForwardRocprofv3_848868,
}

impl RocprofJsonDispatchProjectionV4 {
    pub const fn dialect(&self) -> RocprofDispatchSchemaDialectV4 {
        self.dialect
    }

    pub fn canonical_json(&self) -> &[u8] {
        &self.canonical_json
    }

    pub fn agent_bindings(&self) -> &[RocprofJsonGpuAgentBindingV4] {
        &self.agent_bindings
    }
}

#[derive(Clone, Debug)]
pub struct ProfilerDispatchBindingV4 {
    pub environment: ProfilerEnvironmentBindingV4,
    pub kernel_ir_claim: KernelIrIdentityClaimV1,
    pub artifact: Option<ArtifactClaimV1>,
    pub source_map: Option<ContentIdentityV1>,
    pub wave_width: WaveWidthV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfilerAttArtifactBindingV4 {
    pub reference: String,
    pub content: ContentIdentityRecordV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfilerAttBindingV4 {
    pub environment: ProfilerEnvironmentBindingV4,
    pub source_agent_id: u64,
    pub referenced_artifacts: Vec<ProfilerAttArtifactBindingV4>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerDeviceV4 {
    pub ordinal: u32,
    pub stable_identity: ProfilerIdentityFactV4,
    pub source_bound_identity: Option<CaptureIdentityV1>,
    pub source_bound_origin: TruthOriginV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttReferenceKindV4 {
    WaveTimeline,
    ShaderEngineMetadata,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AttArtifactReferenceV4 {
    pub ordinal: u32,
    pub kind: AttReferenceKindV4,
    pub reference: String,
    pub content: ProfilerIdentityFactV4,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AttReferenceCatalogV4 {
    pub manifest: ProfilerIdentityFactV4,
    pub decoder_output_origin: TruthOriginV1,
    pub references: Vec<AttArtifactReferenceV4>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerCompletenessV4 {
    DispatchEnvelopesOnly,
    AttReferencesOnly,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerCoverageV4 {
    pub origin: TruthOriginV1,
    pub completeness: ProfilerCompletenessV4,
    pub imported_dispatches: u64,
    pub att_references: u64,
    pub loss: LossStatusV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerUnavailableFactV4 {
    RuntimeApiEvents,
    CopyEvents,
    CounterRecords,
    PcSamples,
    DecodedAttEvents,
    WaitEvents,
    FullGridWaveCoverage,
    SemanticExecutionHistory,
    SourceIrIsaCorrelation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticProfilerBundleV4 {
    pub schema_version: u16,
    pub source_kind: ProfilerSourceKindV4,
    pub run_identity: CaptureIdentityV1,
    pub run_identity_origin: TruthOriginV1,
    pub source: ProfilerIdentityFactV4,
    pub normalized_projection: ProfilerIdentityFactV4,
    pub environment: ProfilerIdentityFactV4,
    pub collector_tool: ProfilerIdentityFactV4,
    pub collector_configuration: ProfilerIdentityFactV4,
    pub devices: Vec<ProfilerDeviceV4>,
    pub dispatch_capture: Option<SemanticCaptureV1>,
    pub att: Option<AttReferenceCatalogV4>,
    pub coverage: ProfilerCoverageV4,
    pub unavailable: Vec<ProfilerUnavailableFactV4>,
}

impl SemanticProfilerBundleV4 {
    pub fn validate(&self) -> Result<(), ProfilerBundleErrorV4> {
        if self.schema_version != PROFILER_BUNDLE_SCHEMA_VERSION_V4 {
            return Err(ProfilerBundleErrorV4::UnsupportedVersion(
                self.schema_version,
            ));
        }
        self.source
            .validate(ProfilerIdentityRoleV4::SourceEvidence)?;
        self.normalized_projection
            .validate(ProfilerIdentityRoleV4::NormalizedProjection)?;
        self.environment
            .validate(ProfilerIdentityRoleV4::Environment)?;
        self.collector_tool
            .validate(ProfilerIdentityRoleV4::CollectorTool)?;
        self.collector_configuration
            .validate(ProfilerIdentityRoleV4::CollectorConfiguration)?;
        if self.source.origin != TruthOriginV1::Observed
            || !matches!(self.environment.origin, TruthOriginV1::Declared)
            || !matches!(self.collector_tool.origin, TruthOriginV1::Declared)
            || !matches!(self.collector_configuration.origin, TruthOriginV1::Declared)
        {
            return Err(ProfilerBundleErrorV4::EnvironmentMustBeDeclared);
        }
        let expected_run = derive_run_identity(
            self.source
                .value
                .ok_or(ProfilerBundleErrorV4::InvalidIdentityFact)?,
            self.environment
                .value
                .ok_or(ProfilerBundleErrorV4::InvalidIdentityFact)?,
            self.collector_tool
                .value
                .ok_or(ProfilerBundleErrorV4::InvalidIdentityFact)?,
            self.collector_configuration
                .value
                .ok_or(ProfilerBundleErrorV4::InvalidIdentityFact)?,
        )?;
        if self.run_identity_origin != TruthOriginV1::Inferred || self.run_identity != expected_run
        {
            return Err(ProfilerBundleErrorV4::StaleRunIdentity);
        }
        if self.devices.is_empty() || self.devices.len() > MAX_PROFILER_DEVICE_BINDINGS_V4 {
            return Err(ProfilerBundleErrorV4::DeviceCountOutOfRange);
        }
        let mut stable = BTreeSet::new();
        for (ordinal, device) in (0_u32..).zip(&self.devices) {
            let stable_value = device
                .stable_identity
                .value
                .ok_or(ProfilerBundleErrorV4::InvalidDevice)?;
            if device.ordinal != ordinal
                || !matches!(
                    (device.source_bound_identity, device.source_bound_origin),
                    (Some(_), TruthOriginV1::Observed) | (None, TruthOriginV1::Unavailable)
                )
                || !stable.insert(stable_value.digest)
            {
                return Err(ProfilerBundleErrorV4::InvalidDevice);
            }
            device
                .stable_identity
                .validate(ProfilerIdentityRoleV4::StableDevice)?;
            if device.stable_identity.origin != TruthOriginV1::Declared {
                return Err(ProfilerBundleErrorV4::InvalidDevice);
            }
        }
        let expected_loss = LossStatusV1 {
            origin: TruthOriginV1::Unavailable,
            state: LossStateV1::Unknown,
            lost_records: None,
            unavailable_reason: Some(CaptureUnavailableReasonV1::CollectorLossUnknown),
        };
        if self.coverage.origin != TruthOriginV1::Declared || self.coverage.loss != expected_loss {
            return Err(ProfilerBundleErrorV4::InvalidCoverage);
        }
        match self.source_kind {
            ProfilerSourceKindV4::Rocprofv3KernelDispatchJson
            | ProfilerSourceKindV4::Rocprofv3KernelDispatchCsv => {
                let capture = self
                    .dispatch_capture
                    .as_ref()
                    .ok_or(ProfilerBundleErrorV4::MissingDispatchCapture)?;
                capture
                    .validate()
                    .map_err(|_| ProfilerBundleErrorV4::InvalidDispatchCapture)?;
                if self.att.is_some()
                    || self.normalized_projection.origin != TruthOriginV1::Observed
                    || self.coverage.completeness != ProfilerCompletenessV4::DispatchEnvelopesOnly
                    || self.coverage.imported_dispatches != capture.dispatches.len() as u64
                    || self.coverage.att_references != 0
                    || self.devices.len() != capture.devices.len()
                {
                    return Err(ProfilerBundleErrorV4::InvalidCoverage);
                }
                for (device, source) in self.devices.iter().zip(&capture.devices) {
                    if device.source_bound_identity != Some(source.identity) {
                        return Err(ProfilerBundleErrorV4::StaleReference);
                    }
                }
                let expected_source = match self.source_kind {
                    ProfilerSourceKindV4::Rocprofv3KernelDispatchJson
                    | ProfilerSourceKindV4::Rocprofv3KernelDispatchCsv => {
                        self.normalized_projection.value
                    }
                    _ => unreachable!(),
                };
                if capture.runs[0].source
                    != expected_source.ok_or(ProfilerBundleErrorV4::InvalidIdentityFact)?
                {
                    return Err(ProfilerBundleErrorV4::StaleReference);
                }
            }
            ProfilerSourceKindV4::Rocprofv3AttComputeViewerManifest => {
                if self.dispatch_capture.is_some()
                    || self.normalized_projection.origin != TruthOriginV1::Unavailable
                    || self.coverage.completeness != ProfilerCompletenessV4::AttReferencesOnly
                    || self.coverage.imported_dispatches != 0
                    || self.devices.len() != 1
                {
                    return Err(ProfilerBundleErrorV4::InvalidCoverage);
                }
                let att = self
                    .att
                    .as_ref()
                    .ok_or(ProfilerBundleErrorV4::MissingAttCatalog)?;
                att.manifest
                    .validate(ProfilerIdentityRoleV4::SourceEvidence)?;
                if att.manifest != self.source
                    || att.manifest.origin != TruthOriginV1::Observed
                    || att.decoder_output_origin != TruthOriginV1::Unavailable
                    || att.references.is_empty()
                    || att.references.len() > MAX_PROFILER_ATT_REFERENCES_V4
                    || self.coverage.att_references != att.references.len() as u64
                {
                    return Err(ProfilerBundleErrorV4::InvalidAttCatalog);
                }
                for (ordinal, reference) in (0_u32..).zip(&att.references) {
                    if reference.ordinal != ordinal || !valid_reference(&reference.reference) {
                        return Err(ProfilerBundleErrorV4::InvalidAttReference);
                    }
                    reference
                        .content
                        .validate(ProfilerIdentityRoleV4::AttReferencedArtifact)?;
                }
            }
        }
        if self.unavailable.is_empty() || !self.unavailable.windows(2).all(|pair| pair[0] < pair[1])
        {
            return Err(ProfilerBundleErrorV4::NonCanonicalUnavailableFacts);
        }
        Ok(())
    }
}

/// Imports a raw rocprof JSON document through exact per-process agent
/// projection. Stable-device bindings are keyed by projected absolute KFD node
/// ID, never by an opaque process-local rocprof agent handle. The Bundle keeps
/// raw source and normalized projection as distinct observed identities.
pub fn import_rocprofv3_json_profiler_bundle_v4(
    source: &[u8],
    binding: ProfilerDispatchBindingV4,
) -> Result<SemanticProfilerBundleV4, ProfilerBundleErrorV4> {
    let projection = project_rocprofv3_json_dispatch_agents_v4(source)?;
    import_projected_rocprofv3_json_profiler_bundle_v4(source, &projection, binding)
}

/// Imports raw rocprof JSON with a retained exact projection. The projection
/// is rederived from `source` before use. Stable-device bindings are keyed by
/// projected absolute KFD node ID, never by an opaque process-local handle.
pub fn import_projected_rocprofv3_json_profiler_bundle_v4(
    source: &[u8],
    projection: &RocprofJsonDispatchProjectionV4,
    binding: ProfilerDispatchBindingV4,
) -> Result<SemanticProfilerBundleV4, ProfilerBundleErrorV4> {
    validate_source(source)?;
    let expected = project_rocprofv3_json_dispatch_agents_v4(source)?;
    if &expected != projection {
        return Err(ProfilerBundleErrorV4::StaleReference);
    }
    validate_environment(&binding.environment)?;
    let imported = import_rocprofv3_capture_with_agents_v1(
        projection.canonical_json(),
        RocprofCaptureBindingV1 {
            kernel_ir_claim: binding.kernel_ir_claim,
            artifact: binding.artifact,
            source_map: binding.source_map,
            wave_width: binding.wave_width,
        },
        ImportLimitsV1::default(),
    )
    .map_err(map_dispatch_import_error_v4)?;
    let projection_identity = imported.capture.runs[0].source;
    finish_dispatch_bundle(
        ProfilerSourceKindV4::Rocprofv3KernelDispatchJson,
        rocprofv3_json_profiler_source_content_identity_v4(source)?,
        projection_identity,
        imported.capture,
        imported.source_agent_ids,
        binding.environment,
    )
}

/// Exact Bundle V4 raw-source identity. This is intentionally distinct from
/// the V1 rocprof source identity retained by Counter Capture V2 and PC Sample
/// Capture V3 relations.
pub fn rocprofv3_json_profiler_source_content_identity_v4(
    source: &[u8],
) -> Result<ContentIdentityRecordV1, ProfilerBundleErrorV4> {
    validate_source(source)?;
    content_identity(PROFILER_SOURCE_JSON_DOMAIN_V4, 1, source)
}

pub fn project_rocprofv3_json_dispatch_agents_v4(
    source: &[u8],
) -> Result<RocprofJsonDispatchProjectionV4, ProfilerBundleErrorV4> {
    validate_source(source)?;
    let document = parse_dispatch_json_document_v4(source)?;
    if document.processes.is_empty() || document.processes.len() > MAX_ROCPROF_PROCESSES_V1 {
        return Err(ProfilerBundleErrorV4::InvalidRocprofJson);
    }
    let mut bindings = Vec::new();
    let mut projected_processes = Vec::new();
    bindings
        .try_reserve(document.processes.len())
        .map_err(|_| ProfilerBundleErrorV4::AllocationFailure)?;
    projected_processes
        .try_reserve(document.processes.len())
        .map_err(|_| ProfilerBundleErrorV4::AllocationFailure)?;
    let dialect = document.dialect()?;
    let mut process_ids = BTreeSet::new();
    for (process_index, process) in document.processes.into_iter().enumerate() {
        let process_index =
            u32::try_from(process_index).map_err(|_| ProfilerBundleErrorV4::SizeOverflow)?;
        if process.metadata.pid == 0 || !process_ids.insert(process.metadata.pid) {
            return Err(ProfilerBundleErrorV4::InvalidRocprofJson);
        }
        let agents = process.agents;
        let mut process_agents = BTreeMap::new();
        let mut process_nodes = BTreeSet::new();
        let mut catalog_handles = BTreeSet::new();
        for agent in agents {
            if agent.id.handle == 0 || !catalog_handles.insert(agent.id.handle) {
                return Err(ProfilerBundleErrorV4::InvalidDevice);
            }
            if agent.agent_type != 2 || agent.simd_count == 0 {
                continue;
            }
            let binding = RocprofJsonGpuAgentBindingV4 {
                process_index,
                process_id: process.metadata.pid,
                source_agent_id: agent.id.handle,
                node_id: u32::try_from(agent.node_id)
                    .map_err(|_| ProfilerBundleErrorV4::InvalidDevice)?,
                gpu_id: agent.gpu_id,
                simd_count: agent.simd_count,
                vendor_id: agent.vendor_id,
                device_id: agent.device_id,
                location_id: agent.location_id,
                domain: agent.domain,
                gfx_target_version: agent.gfx_target_version,
                wave_front_size: agent.wave_front_size,
                num_xcc: agent.num_xcc,
            };
            if binding.gpu_id == 0
                || binding.wave_front_size == 0
                || !process_nodes.insert(binding.node_id)
                || process_agents
                    .insert(binding.source_agent_id, binding)
                    .is_some()
            {
                return Err(ProfilerBundleErrorV4::InvalidDevice);
            }
        }
        let mut used = BTreeSet::new();
        let mut projected_dispatches = Vec::new();
        projected_dispatches
            .try_reserve(process.buffer_records.kernel_dispatch.len())
            .map_err(|_| ProfilerBundleErrorV4::AllocationFailure)?;
        for record in process.buffer_records.kernel_dispatch {
            let source_agent_id = record.dispatch_info.agent_id.handle;
            let binding = process_agents
                .get(&source_agent_id)
                .copied()
                .ok_or(ProfilerBundleErrorV4::MissingDeviceBinding)?;
            if used.insert(source_agent_id) {
                if bindings.len() == MAX_PROFILER_SOURCE_AGENT_MAPPINGS_V4 {
                    return Err(ProfilerBundleErrorV4::SourceAgentMappingCountOutOfRange);
                }
                bindings.push(binding);
            }
            projected_dispatches.push(DispatchJsonProjectionRecordV4 {
                start_timestamp: record.start_timestamp,
                end_timestamp: record.end_timestamp,
                dispatch_info: DispatchJsonProjectionInfoV4 {
                    agent_id: DispatchJsonHandleV4 {
                        handle: u64::from(binding.node_id),
                    },
                    dispatch_id: Some(record.dispatch_info.dispatch_id),
                    workgroup_size: DispatchJsonProjectionDimensionsV4 {
                        x: u64::from(record.dispatch_info.workgroup_size.x),
                        y: u64::from(record.dispatch_info.workgroup_size.y),
                        z: u64::from(record.dispatch_info.workgroup_size.z),
                    },
                    grid_size: DispatchJsonProjectionDimensionsV4 {
                        x: u64::from(record.dispatch_info.grid_size.x),
                        y: u64::from(record.dispatch_info.grid_size.y),
                        z: u64::from(record.dispatch_info.grid_size.z),
                    },
                },
            });
        }
        projected_processes.push(DispatchJsonProjectionProcessV4 {
            buffer_records: DispatchJsonProjectionBufferRecordsV4 {
                kernel_dispatch: projected_dispatches,
            },
        });
    }
    if bindings.is_empty() {
        return Err(ProfilerBundleErrorV4::DeviceCountOutOfRange);
    }
    let canonical_json = serde_json::to_vec(&DispatchJsonProjectionDocumentV4 {
        processes: projected_processes,
    })
    .map_err(|_| ProfilerBundleErrorV4::JsonEncode)?;
    validate_source(&canonical_json)?;
    Ok(RocprofJsonDispatchProjectionV4 {
        dialect,
        canonical_json,
        agent_bindings: bindings,
    })
}

#[derive(Serialize)]
struct DispatchJsonProjectionDocumentV4 {
    #[serde(rename = "rocprofiler-sdk-tool")]
    processes: Vec<DispatchJsonProjectionProcessV4>,
}

#[derive(Serialize)]
struct DispatchJsonProjectionProcessV4 {
    buffer_records: DispatchJsonProjectionBufferRecordsV4,
}

#[derive(Serialize)]
struct DispatchJsonProjectionBufferRecordsV4 {
    kernel_dispatch: Vec<DispatchJsonProjectionRecordV4>,
}

#[derive(Serialize)]
struct DispatchJsonProjectionRecordV4 {
    start_timestamp: u64,
    end_timestamp: u64,
    dispatch_info: DispatchJsonProjectionInfoV4,
}

#[derive(Serialize)]
struct DispatchJsonProjectionInfoV4 {
    agent_id: DispatchJsonHandleV4,
    #[serde(skip_serializing_if = "Option::is_none")]
    dispatch_id: Option<u64>,
    workgroup_size: DispatchJsonProjectionDimensionsV4,
    grid_size: DispatchJsonProjectionDimensionsV4,
}

#[derive(Serialize)]
struct DispatchJsonProjectionDimensionsV4 {
    x: u64,
    y: u64,
    z: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DispatchJsonParseFailureV4 {
    Allocation,
}

thread_local! {
    static DISPATCH_JSON_PARSE_FAILURE_V4: Cell<Option<DispatchJsonParseFailureV4>> = const { Cell::new(None) };
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DispatchJsonAllocationInjectionSiteV4 {
    Any,
    StringArrayElement,
    ObjectKey,
}

#[cfg(test)]
thread_local! {
    static INJECT_DISPATCH_JSON_ALLOCATION_FAILURE_V4: Cell<Option<DispatchJsonAllocationInjectionSiteV4>> = const { Cell::new(None) };
}

fn parse_dispatch_json_document_v4(
    source: &[u8],
) -> Result<DispatchJsonDocumentV4, ProfilerBundleErrorV4> {
    DISPATCH_JSON_PARSE_FAILURE_V4.with(|state| {
        let prior = state.replace(None);
        let parsed = serde_json::from_slice(source);
        let failure = state.replace(prior);
        match (parsed, failure) {
            (_, Some(DispatchJsonParseFailureV4::Allocation)) => {
                Err(ProfilerBundleErrorV4::AllocationFailure)
            }
            (Ok(document), None) => Ok(document),
            (Err(_), None) => Err(ProfilerBundleErrorV4::InvalidRocprofJson),
        }
    })
}

fn dispatch_json_allocation_error_v4<E: serde::de::Error>() -> E {
    DISPATCH_JSON_PARSE_FAILURE_V4.with(|state| {
        state.set(Some(DispatchJsonParseFailureV4::Allocation));
    });
    E::custom("rocprof dispatch JSON allocation failed")
}

fn reserve_dispatch_json_vec_v4<T, E: serde::de::Error>(
    output: &mut Vec<T>,
    additional: usize,
) -> Result<(), E> {
    #[cfg(test)]
    if take_dispatch_json_allocation_injection_v4(DispatchJsonAllocationInjectionSiteV4::Any) {
        return Err(dispatch_json_allocation_error_v4());
    }
    output
        .try_reserve(additional)
        .map_err(|_| dispatch_json_allocation_error_v4())
}

fn reserve_dispatch_json_string_v4<E: serde::de::Error>(
    output: &mut String,
    additional: usize,
) -> Result<(), E> {
    output
        .try_reserve(additional)
        .map_err(|_| dispatch_json_allocation_error_v4())
}

#[cfg(test)]
fn take_dispatch_json_allocation_injection_v4(site: DispatchJsonAllocationInjectionSiteV4) -> bool {
    INJECT_DISPATCH_JSON_ALLOCATION_FAILURE_V4.with(|inject| match inject.get() {
        Some(DispatchJsonAllocationInjectionSiteV4::Any) => {
            inject.set(None);
            true
        }
        Some(expected) if expected == site => {
            inject.set(None);
            true
        }
        _ => false,
    })
}

fn reserve_dispatch_json_string_array_element_v4<E: serde::de::Error>(
    output: &mut String,
    additional: usize,
) -> Result<(), E> {
    #[cfg(test)]
    if take_dispatch_json_allocation_injection_v4(
        DispatchJsonAllocationInjectionSiteV4::StringArrayElement,
    ) {
        return Err(dispatch_json_allocation_error_v4());
    }
    reserve_dispatch_json_string_v4(output, additional)
}

fn reserve_dispatch_json_object_key_v4<E: serde::de::Error>(
    output: &mut String,
    additional: usize,
) -> Result<(), E> {
    #[cfg(test)]
    if take_dispatch_json_allocation_injection_v4(DispatchJsonAllocationInjectionSiteV4::ObjectKey)
    {
        return Err(dispatch_json_allocation_error_v4());
    }
    reserve_dispatch_json_string_v4(output, additional)
}

#[allow(dead_code)]
enum PresentFieldV4<T> {
    Absent,
    Present(T),
}

impl<T> PresentFieldV4<T> {
    const fn is_present(&self) -> bool {
        matches!(self, Self::Present(_))
    }
}

impl<T> Default for PresentFieldV4<T> {
    fn default() -> Self {
        Self::Absent
    }
}

impl<'de, T> Deserialize<'de> for PresentFieldV4<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(deserializer).map(Self::Present)
    }
}

// This is the closed rocprofv3 dispatch protocol admitted by Bundle V4. Fields
// emitted by the reviewed rocprofv3 schema but not projected into the bundle
// are named explicitly and discarded. A new field therefore requires an
// intentional protocol update instead of being silently accepted by serde.
#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DispatchJsonDocumentV4 {
    #[serde(
        rename = "rocprofiler-sdk-tool",
        deserialize_with = "deserialize_bounded_processes_v4"
    )]
    processes: Vec<DispatchJsonProcessV4>,
}

impl DispatchJsonDocumentV4 {
    fn dialect(&self) -> Result<RocprofDispatchSchemaDialectV4, ProfilerBundleErrorV4> {
        let forward = self
            .processes
            .first()
            .is_some_and(|process| process.buffer_records.hipfile_api.is_present());
        for process in &self.processes {
            if process.agents.iter().any(|agent| !agent.has_valid_shape()) {
                return Err(ProfilerBundleErrorV4::InvalidRocprofJson);
            }
            let buffer_is_forward = process.buffer_records.hipfile_api.is_present()
                && process.buffer_records.kfd.is_present()
                && process.buffer_records.hip_graph.is_present()
                && process.buffer_records.rocshmem_api.is_present();
            let buffer_is_installed = !process.buffer_records.hipfile_api.is_present()
                && !process.buffer_records.kfd.is_present()
                && !process.buffer_records.hip_graph.is_present()
                && !process.buffer_records.rocshmem_api.is_present();
            if (forward && !buffer_is_forward) || (!forward && !buffer_is_installed) {
                return Err(ProfilerBundleErrorV4::InvalidRocprofJson);
            }
            if process.callback_records.spm_counter_collection.is_present() != forward {
                return Err(ProfilerBundleErrorV4::InvalidRocprofJson);
            }
            if process.buffer_records.kernel_dispatch.iter().any(|record| {
                record.graph_exec_id.is_present() != forward
                    || record.graph_node_id.is_present() != forward
            }) {
                return Err(ProfilerBundleErrorV4::InvalidRocprofJson);
            }
        }
        Ok(if forward {
            RocprofDispatchSchemaDialectV4::ForwardRocprofv3_848868
        } else {
            RocprofDispatchSchemaDialectV4::InstalledRocprofv3_1_1_97f5574
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DispatchJsonProcessV4 {
    buffer_records: DispatchJsonBufferRecordsV4,
    metadata: DispatchJsonMetadataV4,
    #[serde(deserialize_with = "deserialize_bounded_agents_v4")]
    agents: Vec<DispatchJsonAgentV4>,
    callback_records: DispatchJsonCallbackRecordsV4,
    counters: BoundedIgnoredArrayV4,
    code_objects: BoundedIgnoredArrayV4,
    kernel_symbols: BoundedIgnoredArrayV4,
    strings: DispatchJsonStringsV4,
    summary: BoundedIgnoredArrayV4,
    host_functions: BoundedIgnoredArrayV4,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DispatchJsonCallbackRecordsV4 {
    counter_collection: BoundedIgnoredArrayV4,
    #[serde(default)]
    spm_counter_collection: PresentFieldV4<BoundedIgnoredArrayV4>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DispatchJsonStringsV4 {
    callback_records: BoundedIgnoredArrayV4,
    buffer_records: BoundedIgnoredArrayV4,
    marker_api: BoundedIgnoredArrayV4,
    correlation_id: DispatchJsonCorrelationStringsV4,
    counters: DispatchJsonCounterStringsV4,
    pc_sample_instructions: BoundedIgnoredArrayV4,
    pc_sample_comments: BoundedIgnoredArrayV4,
    att_filenames: BoundedIgnoredArrayV4,
    code_object_snapshot_filenames: BoundedIgnoredArrayV4,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DispatchJsonCorrelationStringsV4 {
    external: BoundedExternalCorrelationStringsV4,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DispatchJsonCounterStringsV4 {
    dimension_ids: BoundedCounterDimensionStringsV4,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DispatchJsonExternalCorrelationStringV4 {
    key: u64,
    #[serde(deserialize_with = "deserialize_bounded_string_v4")]
    value: String,
}

#[allow(dead_code)]
struct BoundedExternalCorrelationStringsV4(Vec<DispatchJsonExternalCorrelationStringV4>);

impl<'de> Deserialize<'de> for BoundedExternalCorrelationStringsV4 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_bounded_vec_v4(deserializer, 4_096, "rocprof external correlation string")
            .map(Self)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DispatchJsonCounterDimensionStringV4 {
    id: u64,
    instance_size: u64,
    #[serde(deserialize_with = "deserialize_bounded_string_v4")]
    name: String,
}

#[allow(dead_code)]
struct BoundedCounterDimensionStringsV4(Vec<DispatchJsonCounterDimensionStringV4>);

impl<'de> Deserialize<'de> for BoundedCounterDimensionStringsV4 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_bounded_vec_v4(deserializer, 4_096, "rocprof counter dimension string")
            .map(Self)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DispatchJsonMetadataV4 {
    node: DispatchJsonNodeMetadataV4,
    pid: u64,
    init_time: u64,
    fini_time: u64,
    command: BoundedStringArrayV4,
    config: BoundedIgnoredObjectV4,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DispatchJsonNodeMetadataV4 {
    id: u64,
    hash: u64,
    #[serde(deserialize_with = "deserialize_bounded_string_v4")]
    machine_id: String,
    #[serde(deserialize_with = "deserialize_bounded_string_v4")]
    system_name: String,
    #[serde(deserialize_with = "deserialize_bounded_string_v4")]
    hostname: String,
    #[serde(deserialize_with = "deserialize_bounded_string_v4")]
    release: String,
    #[serde(deserialize_with = "deserialize_bounded_string_v4")]
    version: String,
    #[serde(deserialize_with = "deserialize_bounded_string_v4")]
    hardware_name: String,
    #[serde(deserialize_with = "deserialize_bounded_string_v4")]
    domain_name: String,
}

#[allow(dead_code)]
struct BoundedStringArrayV4(Vec<BoundedStringArrayElementV4>);

#[allow(dead_code)]
struct BoundedIgnoredObjectV4(Vec<String>);

#[allow(dead_code)]
struct BoundedStringArrayElementV4(String);

struct BoundedObjectKeyV4(String);

impl<'de> Deserialize<'de> for BoundedStringArrayElementV4 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ElementVisitorV4;

        impl<'de> serde::de::Visitor<'de> for ElementVisitorV4 {
            type Value = BoundedStringArrayElementV4;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a rocprof string of at most 65536 bytes")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if value.len() > 64 * 1024 {
                    return Err(E::custom("rocprof string exceeds limit"));
                }
                let mut output = String::new();
                reserve_dispatch_json_string_array_element_v4(&mut output, value.len())?;
                output.push_str(value);
                Ok(BoundedStringArrayElementV4(output))
            }

            fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_str(value)
            }
        }

        deserializer.deserialize_str(ElementVisitorV4)
    }
}

impl<'de> Deserialize<'de> for BoundedObjectKeyV4 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct KeyVisitorV4;

        impl<'de> serde::de::Visitor<'de> for KeyVisitorV4 {
            type Value = BoundedObjectKeyV4;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a rocprof object key of at most 65536 bytes")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if value.len() > 64 * 1024 {
                    return Err(E::custom("rocprof object key exceeds limit"));
                }
                let mut output = String::new();
                reserve_dispatch_json_object_key_v4(&mut output, value.len())?;
                output.push_str(value);
                Ok(BoundedObjectKeyV4(output))
            }

            fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_str(value)
            }
        }

        deserializer.deserialize_identifier(KeyVisitorV4)
    }
}

impl<'de> Deserialize<'de> for BoundedIgnoredObjectV4 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ObjectVisitorV4;

        impl<'de> serde::de::Visitor<'de> for ObjectVisitorV4 {
            type Value = BoundedIgnoredObjectV4;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "a rocprof object with at most 4096 fields")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                if map.size_hint().is_some_and(|hint| hint > 4_096) {
                    return Err(serde::de::Error::custom(
                        "rocprof object field limit exceeded",
                    ));
                }
                let mut keys = Vec::new();
                while let Some(BoundedObjectKeyV4(key)) = map.next_key()? {
                    if keys.len() == 4_096 || key.len() > 64 * 1024 || keys.contains(&key) {
                        return Err(serde::de::Error::custom("invalid rocprof object key"));
                    }
                    map.next_value::<IgnoredAny>()?;
                    reserve_dispatch_json_vec_v4(&mut keys, 1)?;
                    keys.push(key);
                }
                Ok(BoundedIgnoredObjectV4(keys))
            }
        }

        deserializer.deserialize_map(ObjectVisitorV4)
    }
}

fn deserialize_bounded_string_v4<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringVisitorV4;

    impl<'de> serde::de::Visitor<'de> for StringVisitorV4 {
        type Value = String;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "a rocprof string of at most 65536 bytes")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if value.len() > 64 * 1024 {
                return Err(E::custom("rocprof string exceeds limit"));
            }
            let mut output = String::new();
            reserve_dispatch_json_string_v4(&mut output, value.len())?;
            output.push_str(value);
            Ok(output)
        }

        fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.visit_str(value)
        }
    }

    deserializer.deserialize_str(StringVisitorV4)
}

impl<'de> Deserialize<'de> for BoundedStringArrayV4 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_bounded_vec_v4(deserializer, 4_096, "rocprof string").map(Self)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DispatchJsonAgentV4 {
    id: DispatchJsonHandleV4,
    node_id: u64,
    simd_count: u64,
    gpu_id: u64,
    vendor_id: u64,
    device_id: u64,
    location_id: u64,
    domain: u64,
    gfx_target_version: u64,
    wave_front_size: u64,
    num_xcc: u64,
    size: u64,
    #[serde(rename = "type")]
    agent_type: u32,
    logical_node_id: i32,
    logical_node_type_id: i32,
    cpu_cores_count: u64,
    cpu_core_id_base: u64,
    simd_id_base: u64,
    max_waves_per_simd: u64,
    lds_size_in_kb: u64,
    gds_size_in_kb: u64,
    num_gws: u64,
    cu_count: u64,
    array_count: u64,
    num_shader_banks: u64,
    simd_arrays_per_engine: u64,
    cu_per_simd_array: u64,
    simd_per_cu: u64,
    max_slots_scratch_cu: u64,
    drm_render_minor: u64,
    num_sdma_engines: u64,
    num_sdma_xgmi_engines: u64,
    num_sdma_queues_per_engine: u64,
    num_cp_queues: u64,
    max_engine_clk_ccompute: u64,
    max_engine_clk_fcompute: u64,
    sdma_fw_version: DispatchJsonSdmaFirmwareV4,
    fw_version: DispatchJsonFirmwareV4,
    capability: DispatchJsonCapabilityV4,
    cu_per_engine: u64,
    max_waves_per_cu: u64,
    family_id: u64,
    workgroup_max_size: u64,
    grid_max_size: u64,
    local_mem_size: u64,
    hive_id: u64,
    workgroup_max_dim: DispatchJsonDimensionsV4,
    grid_max_dim: DispatchJsonDimensionsV4,
    #[serde(deserialize_with = "deserialize_bounded_string_v4")]
    name: String,
    #[serde(deserialize_with = "deserialize_bounded_string_v4")]
    vendor_name: String,
    #[serde(deserialize_with = "deserialize_bounded_string_v4")]
    product_name: String,
    #[serde(deserialize_with = "deserialize_bounded_string_v4")]
    model_name: String,
    uuid: DispatchJsonUuidV4,
    mem_banks: BoundedIgnoredArrayV4,
    mem_banks_count: u32,
    caches: BoundedIgnoredArrayV4,
    caches_count: u32,
    io_links: BoundedIgnoredArrayV4,
    io_links_count: u32,
    runtime_visibility: DispatchJsonRuntimeVisibilityV4,
    gpu_index: i64,
}

impl DispatchJsonAgentV4 {
    fn has_valid_shape(&self) -> bool {
        self.sdma_fw_version.has_valid_shape()
            && self.fw_version.has_valid_shape()
            && self.capability.has_valid_shape()
            && self.runtime_visibility.has_valid_shape()
            && usize::try_from(self.mem_banks_count).ok() == Some(self.mem_banks.0.len())
            && usize::try_from(self.caches_count).ok() == Some(self.caches.0.len())
            && usize::try_from(self.io_links_count).ok() == Some(self.io_links.0.len())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code, non_snake_case)]
struct DispatchJsonSdmaFirmwareV4 {
    uCodeSDMA: u16,
    uCodeRes: u16,
}

impl DispatchJsonSdmaFirmwareV4 {
    fn has_valid_shape(&self) -> bool {
        self.uCodeSDMA <= 0x03ff && self.uCodeRes <= 0x03ff
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code, non_snake_case)]
struct DispatchJsonFirmwareV4 {
    uCode: u16,
    Major: u8,
    Minor: u8,
    Stepping: u8,
}

impl DispatchJsonFirmwareV4 {
    fn has_valid_shape(&self) -> bool {
        self.uCode <= 0x03ff && self.Major <= 0x3f
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code, non_snake_case)]
struct DispatchJsonCapabilityV4 {
    HotPluggable: u8,
    HSAMMUPresent: u8,
    SharedWithGraphics: u8,
    QueueSizePowerOfTwo: u8,
    QueueSize32bit: u8,
    QueueIdleEvent: u8,
    VALimit: u8,
    WatchPointsSupported: u8,
    WatchPointsTotalBits: u8,
    DoorbellType: u8,
    AQLQueueDoubleMap: u8,
    DebugTrapSupported: u8,
    WaveLaunchTrapOverrideSupported: u8,
    WaveLaunchModeSupported: u8,
    PreciseMemoryOperationsSupported: u8,
    DEPRECATED_SRAM_EDCSupport: u8,
    Mem_EDCSupport: u8,
    RASEventNotify: u8,
    ASICRevision: u8,
    SRAM_EDCSupport: u8,
    SVMAPISupported: u8,
    CoherentHostAccess: u8,
    DebugSupportedFirmware: u8,
}

impl DispatchJsonCapabilityV4 {
    fn has_valid_shape(&self) -> bool {
        [
            self.HotPluggable,
            self.HSAMMUPresent,
            self.SharedWithGraphics,
            self.QueueSizePowerOfTwo,
            self.QueueSize32bit,
            self.QueueIdleEvent,
            self.VALimit,
            self.WatchPointsSupported,
            self.AQLQueueDoubleMap,
            self.DebugTrapSupported,
            self.WaveLaunchTrapOverrideSupported,
            self.WaveLaunchModeSupported,
            self.PreciseMemoryOperationsSupported,
            self.DEPRECATED_SRAM_EDCSupport,
            self.Mem_EDCSupport,
            self.RASEventNotify,
            self.SRAM_EDCSupport,
            self.SVMAPISupported,
            self.CoherentHostAccess,
            self.DebugSupportedFirmware,
        ]
        .into_iter()
        .all(|value| value <= 1)
            && self.WatchPointsTotalBits <= 0x0f
            && self.DoorbellType <= 0x03
            && self.ASICRevision <= 0x0f
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DispatchJsonUuidV4 {
    bytes: DispatchJsonUuidBytesV4,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DispatchJsonUuidBytesV4 {
    value0: u8,
    value1: u8,
    value2: u8,
    value3: u8,
    value4: u8,
    value5: u8,
    value6: u8,
    value7: u8,
    value8: u8,
    value9: u8,
    value10: u8,
    value11: u8,
    value12: u8,
    value13: u8,
    value14: u8,
    value15: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DispatchJsonRuntimeVisibilityV4 {
    hsa: u8,
    hip: u8,
    rccl: u8,
    rocdecode: u8,
}

impl DispatchJsonRuntimeVisibilityV4 {
    fn has_valid_shape(&self) -> bool {
        [self.hsa, self.hip, self.rccl, self.rocdecode]
            .into_iter()
            .all(|value| value <= 1)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DispatchJsonBufferRecordsV4 {
    #[serde(deserialize_with = "deserialize_bounded_dispatches_v4")]
    kernel_dispatch: Vec<DispatchJsonRecordV4>,
    hip_api: BoundedIgnoredArrayV4,
    hsa_api: BoundedIgnoredArrayV4,
    rccl_api: BoundedIgnoredArrayV4,
    #[serde(default)]
    kfd: PresentFieldV4<BoundedIgnoredArrayV4>,
    rocdecode_api: BoundedIgnoredArrayV4,
    rocjpeg_api: BoundedIgnoredArrayV4,
    #[serde(default)]
    hip_graph: PresentFieldV4<BoundedIgnoredArrayV4>,
    #[serde(default)]
    hipfile_api: PresentFieldV4<BoundedIgnoredArrayV4>,
    #[serde(default)]
    rocshmem_api: PresentFieldV4<BoundedIgnoredArrayV4>,
    marker_api: BoundedIgnoredArrayV4,
    memory_copy: BoundedIgnoredArrayV4,
    memory_allocation: BoundedIgnoredArrayV4,
    scratch_memory: BoundedIgnoredArrayV4,
    pc_sample_host_trap: BoundedIgnoredArrayV4,
    pc_sample_stochastic: BoundedIgnoredArrayV4,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DispatchJsonRecordV4 {
    size: u64,
    kind: u64,
    operation: u64,
    thread_id: u64,
    correlation_id: DispatchJsonCorrelationV4,
    start_timestamp: u64,
    end_timestamp: u64,
    dispatch_info: DispatchJsonInfoV4,
    stream_id: DispatchJsonHandleV4,
    #[serde(default)]
    graph_exec_id: PresentFieldV4<u64>,
    #[serde(default)]
    graph_node_id: PresentFieldV4<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DispatchJsonInfoV4 {
    size: u64,
    agent_id: DispatchJsonHandleV4,
    queue_id: DispatchJsonHandleV4,
    kernel_id: u64,
    dispatch_id: u64,
    private_segment_size: u64,
    group_segment_size: u64,
    workgroup_size: DispatchJsonDimensionsV4,
    grid_size: DispatchJsonDimensionsV4,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DispatchJsonCorrelationV4 {
    internal: u64,
    external: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DispatchJsonHandleV4 {
    handle: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct DispatchJsonDimensionsV4 {
    x: u32,
    y: u32,
    z: u32,
}

fn deserialize_bounded_processes_v4<'de, D>(
    deserializer: D,
) -> Result<Vec<DispatchJsonProcessV4>, D::Error>
where
    D: Deserializer<'de>,
{
    struct ProcessVisitorV4;

    impl<'de> serde::de::Visitor<'de> for ProcessVisitorV4 {
        type Value = Vec<DispatchJsonProcessV4>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "at most {MAX_ROCPROF_PROCESSES_V1} rocprof processes containing at most {MAX_PROFILER_DISPATCHES_V4} total dispatches"
            )
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            if sequence
                .size_hint()
                .is_some_and(|hint| hint > MAX_ROCPROF_PROCESSES_V1)
            {
                return Err(serde::de::Error::custom("rocprof process limit exceeded"));
            }
            let mut output = Vec::new();
            let mut dispatch_count = 0_usize;
            let mut agent_count = 0_usize;
            while let Some(process) = sequence.next_element::<DispatchJsonProcessV4>()? {
                if output.len() == MAX_ROCPROF_PROCESSES_V1 {
                    return Err(serde::de::Error::custom("rocprof process limit exceeded"));
                }
                dispatch_count = dispatch_count
                    .checked_add(process.buffer_records.kernel_dispatch.len())
                    .ok_or_else(|| serde::de::Error::custom("rocprof dispatch count overflow"))?;
                if dispatch_count > MAX_PROFILER_DISPATCHES_V4 {
                    return Err(serde::de::Error::custom("rocprof dispatch limit exceeded"));
                }
                agent_count = agent_count
                    .checked_add(process.agents.len())
                    .ok_or_else(|| serde::de::Error::custom("rocprof agent count overflow"))?;
                if agent_count > MAX_PROFILER_SOURCE_AGENT_MAPPINGS_V4 {
                    return Err(serde::de::Error::custom("rocprof agent limit exceeded"));
                }
                reserve_dispatch_json_vec_v4(&mut output, 1)?;
                output.push(process);
            }
            Ok(output)
        }
    }

    deserializer.deserialize_seq(ProcessVisitorV4)
}

fn deserialize_bounded_agents_v4<'de, D>(
    deserializer: D,
) -> Result<Vec<DispatchJsonAgentV4>, D::Error>
where
    D: Deserializer<'de>,
{
    BoundedAgentVecV4::deserialize(deserializer).map(|value| value.0)
}

struct BoundedAgentVecV4(Vec<DispatchJsonAgentV4>);

impl<'de> Deserialize<'de> for BoundedAgentVecV4 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_bounded_vec_v4(
            deserializer,
            MAX_PROFILER_DEVICE_BINDINGS_V4,
            "rocprof agent",
        )
        .map(Self)
    }
}

#[allow(dead_code)]
struct BoundedIgnoredArrayV4(Vec<IgnoredAny>);

impl<'de> Deserialize<'de> for BoundedIgnoredArrayV4 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_bounded_vec_v4(
            deserializer,
            MAX_PROFILER_DISPATCHES_V4,
            "rocprof ignored buffer",
        )
        .map(Self)
    }
}

fn deserialize_bounded_dispatches_v4<'de, D>(
    deserializer: D,
) -> Result<Vec<DispatchJsonRecordV4>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec_v4(deserializer, MAX_PROFILER_DISPATCHES_V4, "rocprof dispatch")
}

fn deserialize_bounded_vec_v4<'de, D, T>(
    deserializer: D,
    maximum: usize,
    label: &'static str,
) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct BoundedVisitor<T> {
        maximum: usize,
        label: &'static str,
        marker: std::marker::PhantomData<T>,
    }

    impl<'de, T> serde::de::Visitor<'de> for BoundedVisitor<T>
    where
        T: Deserialize<'de>,
    {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "at most {} {} records", self.maximum, self.label)
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            if sequence.size_hint().is_some_and(|hint| hint > self.maximum) {
                return Err(serde::de::Error::custom("bounded sequence exceeds limit"));
            }
            let mut output = Vec::new();
            while let Some(value) = sequence.next_element()? {
                if output.len() == self.maximum {
                    return Err(serde::de::Error::custom("bounded sequence exceeds limit"));
                }
                reserve_dispatch_json_vec_v4(&mut output, 1)?;
                output.push(value);
            }
            Ok(output)
        }
    }

    deserializer.deserialize_seq(BoundedVisitor {
        maximum,
        label,
        marker: std::marker::PhantomData,
    })
}

pub fn import_rocprofv3_csv_profiler_bundle_v4(
    source: &[u8],
    binding: ProfilerDispatchBindingV4,
) -> Result<SemanticProfilerBundleV4, ProfilerBundleErrorV4> {
    validate_source(source)?;
    validate_environment(&binding.environment)?;
    let projection = csv_to_rocprof_json(source)?;
    let imported = import_rocprofv3_capture_with_agents_v1(
        &projection,
        RocprofCaptureBindingV1 {
            kernel_ir_claim: binding.kernel_ir_claim,
            artifact: binding.artifact,
            source_map: binding.source_map,
            wave_width: binding.wave_width,
        },
        ImportLimitsV1::default(),
    )
    .map_err(|error| match error {
        crate::ImportErrorV1::AllocationFailure => ProfilerBundleErrorV4::AllocationFailure,
        _ => ProfilerBundleErrorV4::InvalidRocprofCsv,
    })?;
    let projection = imported.capture.runs[0].source;
    finish_dispatch_bundle(
        ProfilerSourceKindV4::Rocprofv3KernelDispatchCsv,
        content_identity(PROFILER_SOURCE_CSV_DOMAIN_V4, 1, source)?,
        projection,
        imported.capture,
        imported.source_agent_ids,
        binding.environment,
    )
}

pub fn import_rocprofv3_att_profiler_bundle_v4(
    source: &[u8],
    binding: ProfilerAttBindingV4,
) -> Result<SemanticProfilerBundleV4, ProfilerBundleErrorV4> {
    validate_source(source)?;
    validate_environment(&binding.environment)?;
    let stable_device = binding
        .environment
        .stable_device_bindings
        .iter()
        .find(|device| device.source_agent_id == binding.source_agent_id)
        .ok_or(ProfilerBundleErrorV4::MissingDeviceBinding)?
        .stable_identity;
    let references = parse_att_references(source)?;
    let mut supplied = BTreeMap::new();
    for item in binding.referenced_artifacts {
        if !valid_reference(&item.reference)
            || validate_content_identity(item.content).is_err()
            || supplied.insert(item.reference, item.content).is_some()
        {
            return Err(ProfilerBundleErrorV4::InvalidAttBinding);
        }
    }
    let mut records = Vec::new();
    records
        .try_reserve_exact(references.len())
        .map_err(|_| ProfilerBundleErrorV4::AllocationFailure)?;
    for (ordinal, (kind, reference)) in (0_u32..).zip(references) {
        let content = supplied
            .remove(&reference)
            .map(|value| {
                ProfilerIdentityFactV4::declared(
                    ProfilerIdentityRoleV4::AttReferencedArtifact,
                    value,
                )
            })
            .unwrap_or_else(|| {
                ProfilerIdentityFactV4::unavailable(
                    ProfilerIdentityRoleV4::AttReferencedArtifact,
                    CaptureUnavailableReasonV1::NotProvided,
                )
            });
        records.push(AttArtifactReferenceV4 {
            ordinal,
            kind,
            reference,
            content,
        });
    }
    if !supplied.is_empty() {
        return Err(ProfilerBundleErrorV4::UnknownAttBinding);
    }
    let att_reference_count =
        u64::try_from(records.len()).map_err(|_| ProfilerBundleErrorV4::SizeOverflow)?;
    let source_identity = content_identity(PROFILER_SOURCE_ATT_DOMAIN_V4, 1, source)?;
    let run_identity = derive_run_identity(
        source_identity,
        binding.environment.environment,
        binding.environment.collector_tool,
        binding.environment.collector_configuration,
    )?;
    let bundle = SemanticProfilerBundleV4 {
        schema_version: PROFILER_BUNDLE_SCHEMA_VERSION_V4,
        source_kind: ProfilerSourceKindV4::Rocprofv3AttComputeViewerManifest,
        run_identity,
        run_identity_origin: TruthOriginV1::Inferred,
        source: ProfilerIdentityFactV4::observed(
            ProfilerIdentityRoleV4::SourceEvidence,
            source_identity,
        ),
        normalized_projection: ProfilerIdentityFactV4::unavailable(
            ProfilerIdentityRoleV4::NormalizedProjection,
            CaptureUnavailableReasonV1::NotRepresented,
        ),
        environment: ProfilerIdentityFactV4::declared(
            ProfilerIdentityRoleV4::Environment,
            binding.environment.environment,
        ),
        collector_tool: ProfilerIdentityFactV4::declared(
            ProfilerIdentityRoleV4::CollectorTool,
            binding.environment.collector_tool,
        ),
        collector_configuration: ProfilerIdentityFactV4::declared(
            ProfilerIdentityRoleV4::CollectorConfiguration,
            binding.environment.collector_configuration,
        ),
        devices: vec![ProfilerDeviceV4 {
            ordinal: 0,
            stable_identity: ProfilerIdentityFactV4::declared(
                ProfilerIdentityRoleV4::StableDevice,
                stable_device,
            ),
            source_bound_identity: None,
            source_bound_origin: TruthOriginV1::Unavailable,
        }],
        dispatch_capture: None,
        att: Some(AttReferenceCatalogV4 {
            manifest: ProfilerIdentityFactV4::observed(
                ProfilerIdentityRoleV4::SourceEvidence,
                source_identity,
            ),
            decoder_output_origin: TruthOriginV1::Unavailable,
            references: records,
        }),
        coverage: ProfilerCoverageV4 {
            origin: TruthOriginV1::Declared,
            completeness: ProfilerCompletenessV4::AttReferencesOnly,
            imported_dispatches: 0,
            att_references: att_reference_count,
            loss: unknown_loss(),
        },
        unavailable: att_unavailable(),
    };
    bundle.validate()?;
    Ok(bundle)
}

fn finish_dispatch_bundle(
    source_kind: ProfilerSourceKindV4,
    source: ContentIdentityRecordV1,
    projection: ContentIdentityRecordV1,
    capture: SemanticCaptureV1,
    source_agent_ids: Vec<u64>,
    environment: ProfilerEnvironmentBindingV4,
) -> Result<SemanticProfilerBundleV4, ProfilerBundleErrorV4> {
    if source_agent_ids.len() != capture.devices.len() {
        return Err(ProfilerBundleErrorV4::DeviceCountMismatch);
    }
    let stable_devices = environment
        .stable_device_bindings
        .iter()
        .map(|binding| (binding.source_agent_id, binding.stable_identity))
        .collect::<BTreeMap<_, _>>();
    let run_identity = derive_run_identity(
        source,
        environment.environment,
        environment.collector_tool,
        environment.collector_configuration,
    )?;
    let devices = capture
        .devices
        .iter()
        .zip(source_agent_ids)
        .enumerate()
        .map(|(ordinal, (source, source_agent_id))| {
            let stable = stable_devices
                .get(&source_agent_id)
                .copied()
                .ok_or(ProfilerBundleErrorV4::MissingDeviceBinding)?;
            Ok(ProfilerDeviceV4 {
                ordinal: u32::try_from(ordinal).map_err(|_| ProfilerBundleErrorV4::SizeOverflow)?,
                stable_identity: ProfilerIdentityFactV4::declared(
                    ProfilerIdentityRoleV4::StableDevice,
                    stable,
                ),
                source_bound_identity: Some(source.identity),
                source_bound_origin: TruthOriginV1::Observed,
            })
        })
        .collect::<Result<Vec<_>, ProfilerBundleErrorV4>>()?;
    let imported_dispatches =
        u64::try_from(capture.dispatches.len()).map_err(|_| ProfilerBundleErrorV4::SizeOverflow)?;
    let bundle = SemanticProfilerBundleV4 {
        schema_version: PROFILER_BUNDLE_SCHEMA_VERSION_V4,
        source_kind,
        run_identity,
        run_identity_origin: TruthOriginV1::Inferred,
        source: ProfilerIdentityFactV4::observed(ProfilerIdentityRoleV4::SourceEvidence, source),
        normalized_projection: ProfilerIdentityFactV4::observed(
            ProfilerIdentityRoleV4::NormalizedProjection,
            projection,
        ),
        environment: ProfilerIdentityFactV4::declared(
            ProfilerIdentityRoleV4::Environment,
            environment.environment,
        ),
        collector_tool: ProfilerIdentityFactV4::declared(
            ProfilerIdentityRoleV4::CollectorTool,
            environment.collector_tool,
        ),
        collector_configuration: ProfilerIdentityFactV4::declared(
            ProfilerIdentityRoleV4::CollectorConfiguration,
            environment.collector_configuration,
        ),
        devices,
        dispatch_capture: Some(capture),
        att: None,
        coverage: ProfilerCoverageV4 {
            origin: TruthOriginV1::Declared,
            completeness: ProfilerCompletenessV4::DispatchEnvelopesOnly,
            imported_dispatches,
            att_references: 0,
            loss: unknown_loss(),
        },
        unavailable: dispatch_unavailable(),
    };
    bundle.validate()?;
    Ok(bundle)
}

pub fn encode_profiler_bundle_v4(
    bundle: &SemanticProfilerBundleV4,
) -> Result<Vec<u8>, ProfilerBundleErrorV4> {
    bundle.validate()?;
    let bytes = serde_json::to_vec(bundle).map_err(|_| ProfilerBundleErrorV4::JsonEncode)?;
    if bytes.len() as u64 > MAX_PROFILER_BUNDLE_BYTES_V4 {
        return Err(ProfilerBundleErrorV4::BundleTooLarge);
    }
    Ok(bytes)
}

pub fn decode_profiler_bundle_v4(
    bytes: &[u8],
) -> Result<SemanticProfilerBundleV4, ProfilerBundleErrorV4> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_PROFILER_BUNDLE_BYTES_V4 {
        return Err(ProfilerBundleErrorV4::BundleTooLarge);
    }
    let bundle: SemanticProfilerBundleV4 =
        serde_json::from_slice(bytes).map_err(|_| ProfilerBundleErrorV4::JsonDecode)?;
    bundle.validate()?;
    if serde_json::to_vec(&bundle).map_err(|_| ProfilerBundleErrorV4::JsonEncode)? != bytes {
        return Err(ProfilerBundleErrorV4::NonCanonicalEncoding);
    }
    Ok(bundle)
}

pub fn profiler_bundle_content_identity_v4(
    bytes: &[u8],
) -> Result<ContentIdentityRecordV1, ProfilerBundleErrorV4> {
    let _ = decode_profiler_bundle_v4(bytes)?;
    content_identity(
        PROFILER_BUNDLE_IDENTITY_DOMAIN_V4,
        PROFILER_BUNDLE_SCHEMA_VERSION_V4,
        bytes,
    )
}

fn csv_to_rocprof_json(source: &[u8]) -> Result<Vec<u8>, ProfilerBundleErrorV4> {
    csv_projection_v4(source).map(|projection| projection.0)
}

pub fn rocprofv3_csv_source_agent_bindings_v4(
    source: &[u8],
) -> Result<Vec<RocprofCsvSourceAgentBindingV4>, ProfilerBundleErrorV4> {
    validate_source(source)?;
    csv_projection_v4(source).map(|projection| projection.1)
}

fn csv_projection_v4(
    source: &[u8],
) -> Result<(Vec<u8>, Vec<RocprofCsvSourceAgentBindingV4>), ProfilerBundleErrorV4> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .trim(csv::Trim::None)
        .from_reader(source);
    let headers = reader
        .headers()
        .map_err(|_| ProfilerBundleErrorV4::InvalidRocprofCsv)?
        .clone();
    if headers.is_empty() || headers.len() > MAX_PROFILER_CSV_COLUMNS_V4 {
        return Err(ProfilerBundleErrorV4::InvalidRocprofCsv);
    }
    let mut positions = BTreeMap::new();
    for (index, header) in headers.iter().enumerate() {
        if positions.insert(header.to_owned(), index).is_some() {
            return Err(ProfilerBundleErrorV4::InvalidRocprofCsv);
        }
    }
    if !headers.iter().eq(CSV_CURRENT_HEADERS_V4.iter().copied()) {
        return Err(ProfilerBundleErrorV4::InvalidRocprofCsv);
    }
    let mut processes: BTreeMap<Option<u64>, Vec<serde_json::Value>> = BTreeMap::new();
    let mut used_agents: BTreeMap<Option<u64>, BTreeSet<u32>> = BTreeMap::new();
    let mut count = 0_usize;
    for row in reader.records() {
        let row = row.map_err(|_| ProfilerBundleErrorV4::InvalidRocprofCsv)?;
        if row.iter().enumerate().any(|(index, field)| {
            let limit = if headers.get(index) == Some("Kernel_Name") {
                MAX_PROFILER_CSV_KERNEL_NAME_BYTES_V4
            } else {
                MAX_PROFILER_CSV_FIELD_BYTES_V4
            };
            field.len() > limit
        }) {
            return Err(ProfilerBundleErrorV4::InvalidRocprofCsv);
        }
        count = count
            .checked_add(1)
            .ok_or(ProfilerBundleErrorV4::SizeOverflow)?;
        if count > MAX_PROFILER_DISPATCHES_V4
            || field(&row, &positions, "Kind")? != "KERNEL_DISPATCH"
        {
            return Err(ProfilerBundleErrorV4::InvalidRocprofCsv);
        }
        for (name, index) in &positions {
            if !matches!(name.as_str(), "Kind" | "Agent_Id" | "Kernel_Name") {
                let value = row
                    .get(*index)
                    .ok_or(ProfilerBundleErrorV4::InvalidRocprofCsv)?;
                if CSV_CURRENT_U32_HEADERS_V4.contains(&name.as_str()) {
                    parse_u32_integer(value)?;
                } else {
                    parse_integer(value)?;
                }
            }
        }
        parse_integer(field(&row, &positions, "Stream_Id")?)?;
        let process = None;
        let agent = parse_agent_id(field(&row, &positions, "Agent_Id")?)?;
        let agent_node =
            u32::try_from(agent).map_err(|_| ProfilerBundleErrorV4::InvalidRocprofCsv)?;
        used_agents.entry(process).or_default().insert(agent_node);
        let start = parse_integer(field(&row, &positions, "Start_Timestamp")?)?;
        let end = parse_integer(field(&row, &positions, "End_Timestamp")?)?;
        let dispatch_id = parse_integer(field(&row, &positions, "Dispatch_Id")?)?;
        let dimension = |prefix: &str, axis: &str| {
            parse_u32_integer(field(&row, &positions, &format!("{prefix}_{axis}"))?)
        };
        let workgroup = [
            dimension("Workgroup_Size", "X")?,
            dimension("Workgroup_Size", "Y")?,
            dimension("Workgroup_Size", "Z")?,
        ];
        let grid = [
            dimension("Grid_Size", "X")?,
            dimension("Grid_Size", "Y")?,
            dimension("Grid_Size", "Z")?,
        ];
        processes
            .entry(process)
            .or_default()
            .push(serde_json::json!({
                "start_timestamp": start,
                "end_timestamp": end,
                "dispatch_info": {
                    "agent_id": {"handle": agent},
                    "dispatch_id": dispatch_id,
                    "workgroup_size": {"x": workgroup[0], "y": workgroup[1], "z": workgroup[2]},
                    "grid_size": {"x": grid[0], "y": grid[1], "z": grid[2]}
                }
            }));
    }
    if count == 0 {
        return Err(ProfilerBundleErrorV4::InvalidRocprofCsv);
    }
    let processes = processes
        .into_values()
        .map(|kernel_dispatch| serde_json::json!({"buffer_records":{"kernel_dispatch":kernel_dispatch}}))
        .collect::<Vec<_>>();
    let projection = serde_json::to_vec(&serde_json::json!({"rocprofiler-sdk-tool":processes}))
        .map_err(|_| ProfilerBundleErrorV4::JsonEncode)?;
    let mut bindings = Vec::new();
    for (process_index, (process_id, agents)) in used_agents.into_iter().enumerate() {
        let process_index =
            u32::try_from(process_index).map_err(|_| ProfilerBundleErrorV4::SizeOverflow)?;
        for node_id in agents {
            if bindings.len() == MAX_PROFILER_SOURCE_AGENT_MAPPINGS_V4 {
                return Err(ProfilerBundleErrorV4::SourceAgentMappingCountOutOfRange);
            }
            bindings.push(RocprofCsvSourceAgentBindingV4 {
                process_index,
                process_id,
                node_id,
            });
        }
    }
    Ok((projection, bindings))
}

fn field<'a>(
    row: &'a csv::StringRecord,
    positions: &BTreeMap<String, usize>,
    name: &str,
) -> Result<&'a str, ProfilerBundleErrorV4> {
    row.get(
        *positions
            .get(name)
            .ok_or(ProfilerBundleErrorV4::InvalidRocprofCsv)?,
    )
    .ok_or(ProfilerBundleErrorV4::InvalidRocprofCsv)
}

fn parse_integer(value: &str) -> Result<u64, ProfilerBundleErrorV4> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(ProfilerBundleErrorV4::InvalidRocprofCsv);
    }
    let parsed = value
        .parse::<u64>()
        .map_err(|_| ProfilerBundleErrorV4::InvalidRocprofCsv)?;
    if parsed.to_string() != value {
        return Err(ProfilerBundleErrorV4::InvalidRocprofCsv);
    }
    Ok(parsed)
}

fn parse_u32_integer(value: &str) -> Result<u32, ProfilerBundleErrorV4> {
    u32::try_from(parse_integer(value)?).map_err(|_| ProfilerBundleErrorV4::InvalidRocprofCsv)
}

fn parse_agent_id(value: &str) -> Result<u64, ProfilerBundleErrorV4> {
    let Some(agent) = value.strip_prefix("Agent ") else {
        return Err(ProfilerBundleErrorV4::InvalidRocprofCsv);
    };
    let parsed = agent
        .parse::<u64>()
        .map_err(|_| ProfilerBundleErrorV4::InvalidRocprofCsv)?;
    if parsed.to_string() != agent {
        return Err(ProfilerBundleErrorV4::InvalidRocprofCsv);
    }
    Ok(parsed)
}

const CSV_CURRENT_HEADERS_V4: &[&str] = &[
    "Kind",
    "Agent_Id",
    "Queue_Id",
    "Stream_Id",
    "Thread_Id",
    "Dispatch_Id",
    "Kernel_Id",
    "Kernel_Name",
    "Correlation_Id",
    "Start_Timestamp",
    "End_Timestamp",
    "LDS_Block_Size",
    "Scratch_Size",
    "VGPR_Count",
    "Accum_VGPR_Count",
    "SGPR_Count",
    "Workgroup_Size_X",
    "Workgroup_Size_Y",
    "Workgroup_Size_Z",
    "Grid_Size_X",
    "Grid_Size_Y",
    "Grid_Size_Z",
];

const CSV_CURRENT_U32_HEADERS_V4: &[&str] = &[
    "LDS_Block_Size",
    "Scratch_Size",
    "VGPR_Count",
    "Accum_VGPR_Count",
    "SGPR_Count",
    "Workgroup_Size_X",
    "Workgroup_Size_Y",
    "Workgroup_Size_Z",
    "Grid_Size_X",
    "Grid_Size_Y",
    "Grid_Size_Z",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AttManifestDocumentV4 {
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    thread_trace: Option<bool>,
    wave_filenames: serde_json::Value,
    #[serde(default)]
    se_filenames: Option<serde_json::Value>,
    #[serde(default)]
    global_begin_time: Option<u64>,
    #[serde(default)]
    gfxv: Option<String>,
    #[serde(default)]
    gfxip: Option<u64>,
    #[serde(default)]
    counter_names: Option<Vec<String>>,
    #[serde(default)]
    is_pcs_stochastic: Option<bool>,
    #[serde(default)]
    pc_sampling: Option<bool>,
}

fn parse_att_references(
    source: &[u8],
) -> Result<Vec<(AttReferenceKindV4, String)>, ProfilerBundleErrorV4> {
    let document: AttManifestDocumentV4 =
        serde_json::from_slice(source).map_err(|_| ProfilerBundleErrorV4::InvalidAttManifest)?;
    let current = document.thread_trace == Some(true)
        && document.version.as_deref().is_some_and(valid_bounded_text);
    let installed = document.thread_trace.is_none()
        && document.version.is_none()
        && document.global_begin_time.is_some()
        && document.gfxv.as_deref().is_some_and(valid_bounded_text)
        && document.se_filenames.is_some();
    if !(current || installed)
        || document.gfxip.is_some_and(|value| value == 0)
        || document
            .counter_names
            .as_ref()
            .is_some_and(|names| names.len() > MAX_PROFILER_ATT_REFERENCES_V4)
        || document.is_pcs_stochastic == Some(true)
        || document.pc_sampling == Some(true)
    {
        return Err(ProfilerBundleErrorV4::InvalidAttManifest);
    }
    let mut references = Vec::new();
    collect_wave_references(&document.wave_filenames, 0, &mut references)?;
    if let Some(se) = document.se_filenames {
        collect_se_references(&se, &mut references)?;
    }
    if references.is_empty() || references.len() > MAX_PROFILER_ATT_REFERENCES_V4 {
        return Err(ProfilerBundleErrorV4::InvalidAttManifest);
    }
    references.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    if references.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ProfilerBundleErrorV4::InvalidAttManifest);
    }
    Ok(references)
}

fn collect_wave_references(
    value: &serde_json::Value,
    depth: u8,
    output: &mut Vec<(AttReferenceKindV4, String)>,
) -> Result<(), ProfilerBundleErrorV4> {
    if output.len() == MAX_PROFILER_ATT_REFERENCES_V4 {
        return Err(ProfilerBundleErrorV4::InvalidAttManifest);
    }
    if depth == 4 {
        let leaf = value
            .as_array()
            .filter(|leaf| leaf.len() == 3)
            .ok_or(ProfilerBundleErrorV4::InvalidAttManifest)?;
        let reference = leaf[0]
            .as_str()
            .filter(|value| valid_reference(value))
            .ok_or(ProfilerBundleErrorV4::InvalidAttReference)?;
        if !leaf[1].is_u64() || !leaf[2].is_u64() {
            return Err(ProfilerBundleErrorV4::InvalidAttManifest);
        }
        output.push((AttReferenceKindV4::WaveTimeline, reference.to_owned()));
        return Ok(());
    }
    let object = value
        .as_object()
        .filter(|object| !object.is_empty())
        .ok_or(ProfilerBundleErrorV4::InvalidAttManifest)?;
    for (key, nested) in object {
        if key.is_empty() || key.len() > 20 || !key.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(ProfilerBundleErrorV4::InvalidAttManifest);
        }
        collect_wave_references(nested, depth + 1, output)?;
    }
    Ok(())
}

fn collect_se_references(
    value: &serde_json::Value,
    output: &mut Vec<(AttReferenceKindV4, String)>,
) -> Result<(), ProfilerBundleErrorV4> {
    let array = value
        .as_array()
        .filter(|values| !values.is_empty())
        .ok_or(ProfilerBundleErrorV4::InvalidAttManifest)?;
    for value in array {
        if output.len() == MAX_PROFILER_ATT_REFERENCES_V4 {
            return Err(ProfilerBundleErrorV4::InvalidAttManifest);
        }
        let reference = value
            .as_str()
            .filter(|value| valid_reference(value))
            .ok_or(ProfilerBundleErrorV4::InvalidAttReference)?;
        output.push((
            AttReferenceKindV4::ShaderEngineMetadata,
            reference.to_owned(),
        ));
    }
    Ok(())
}

fn valid_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PROFILER_REFERENCE_BYTES_V4
        && !value.contains('\0')
        && !value.starts_with('/')
        && !value.starts_with('\\')
        && !value.contains(':')
        && value
            .split(['/', '\\'])
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

fn valid_bounded_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_PROFILER_REFERENCE_BYTES_V4 && !value.contains('\0')
}

fn validate_source(source: &[u8]) -> Result<(), ProfilerBundleErrorV4> {
    let actual = u64::try_from(source.len()).map_err(|_| ProfilerBundleErrorV4::SizeOverflow)?;
    if actual == 0 || actual > MAX_PROFILER_SOURCE_BYTES_V4 {
        return Err(ProfilerBundleErrorV4::SourceSizeOutOfRange);
    }
    Ok(())
}

fn map_dispatch_import_error_v4(error: crate::ImportErrorV1) -> ProfilerBundleErrorV4 {
    match error {
        crate::ImportErrorV1::AllocationFailure => ProfilerBundleErrorV4::AllocationFailure,
        _ => ProfilerBundleErrorV4::InvalidRocprofJson,
    }
}

fn validate_environment(
    environment: &ProfilerEnvironmentBindingV4,
) -> Result<(), ProfilerBundleErrorV4> {
    validate_content_identity(environment.environment)?;
    validate_content_identity(environment.collector_tool)?;
    validate_content_identity(environment.collector_configuration)?;
    if environment.stable_device_bindings.is_empty()
        || environment.stable_device_bindings.len() > MAX_PROFILER_DEVICE_BINDINGS_V4
    {
        return Err(ProfilerBundleErrorV4::DeviceCountOutOfRange);
    }
    let mut unique_agents = BTreeSet::new();
    let mut unique_devices = BTreeSet::new();
    for binding in &environment.stable_device_bindings {
        validate_content_identity(binding.stable_identity)?;
        if !unique_agents.insert(binding.source_agent_id) {
            return Err(ProfilerBundleErrorV4::DuplicateSourceAgentBinding);
        }
        if !unique_devices.insert(binding.stable_identity.digest) {
            return Err(ProfilerBundleErrorV4::DuplicateStableDevice);
        }
    }
    Ok(())
}

fn validate_content_identity(
    identity: ContentIdentityRecordV1,
) -> Result<(), ProfilerBundleErrorV4> {
    if identity.format_version == 0 || identity.canonical_len == 0 {
        return Err(ProfilerBundleErrorV4::InvalidContentIdentity);
    }
    Ok(())
}

fn content_identity(
    domain: &[u8],
    format_version: u16,
    bytes: &[u8],
) -> Result<ContentIdentityRecordV1, ProfilerBundleErrorV4> {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version,
        digest: CaptureIdentityV1::new(hasher.finalize().into())
            .map_err(|_| ProfilerBundleErrorV4::IdentityFailure)?,
        canonical_len: u64::try_from(bytes.len())
            .map_err(|_| ProfilerBundleErrorV4::SizeOverflow)?,
    })
}

fn derive_run_identity(
    source: ContentIdentityRecordV1,
    environment: ContentIdentityRecordV1,
    tool: ContentIdentityRecordV1,
    configuration: ContentIdentityRecordV1,
) -> Result<CaptureIdentityV1, ProfilerBundleErrorV4> {
    let mut hasher = Sha256::new();
    hasher.update(PROFILER_RUN_IDENTITY_DOMAIN_V4);
    for identity in [source, environment, tool, configuration] {
        hasher.update([match identity.scheme {
            ContentSchemeV1::RawCanonicalSha256 => 1,
            ContentSchemeV1::DomainSeparatedSha256 => 2,
        }]);
        hasher.update(identity.digest.as_bytes());
        hasher.update(identity.canonical_len.to_le_bytes());
        hasher.update(identity.format_version.to_le_bytes());
    }
    CaptureIdentityV1::new(hasher.finalize().into())
        .map_err(|_| ProfilerBundleErrorV4::IdentityFailure)
}

const fn unknown_loss() -> LossStatusV1 {
    LossStatusV1 {
        origin: TruthOriginV1::Unavailable,
        state: LossStateV1::Unknown,
        lost_records: None,
        unavailable_reason: Some(CaptureUnavailableReasonV1::CollectorLossUnknown),
    }
}

fn dispatch_unavailable() -> Vec<ProfilerUnavailableFactV4> {
    vec![
        ProfilerUnavailableFactV4::RuntimeApiEvents,
        ProfilerUnavailableFactV4::CopyEvents,
        ProfilerUnavailableFactV4::CounterRecords,
        ProfilerUnavailableFactV4::PcSamples,
        ProfilerUnavailableFactV4::DecodedAttEvents,
        ProfilerUnavailableFactV4::WaitEvents,
        ProfilerUnavailableFactV4::FullGridWaveCoverage,
        ProfilerUnavailableFactV4::SemanticExecutionHistory,
        ProfilerUnavailableFactV4::SourceIrIsaCorrelation,
    ]
}

fn att_unavailable() -> Vec<ProfilerUnavailableFactV4> {
    vec![
        ProfilerUnavailableFactV4::RuntimeApiEvents,
        ProfilerUnavailableFactV4::CopyEvents,
        ProfilerUnavailableFactV4::CounterRecords,
        ProfilerUnavailableFactV4::PcSamples,
        ProfilerUnavailableFactV4::DecodedAttEvents,
        ProfilerUnavailableFactV4::WaitEvents,
        ProfilerUnavailableFactV4::FullGridWaveCoverage,
        ProfilerUnavailableFactV4::SemanticExecutionHistory,
        ProfilerUnavailableFactV4::SourceIrIsaCorrelation,
    ]
}

#[derive(Debug)]
#[non_exhaustive]
pub enum ProfilerBundleErrorV4 {
    UnsupportedVersion(u16),
    SourceSizeOutOfRange,
    BundleTooLarge,
    InvalidContentIdentity,
    InvalidIdentityFact,
    IdentityRoleMismatch,
    EnvironmentMustBeDeclared,
    DuplicateStableDevice,
    DuplicateSourceAgentBinding,
    DeviceCountOutOfRange,
    SourceAgentMappingCountOutOfRange,
    DeviceCountMismatch,
    MissingDeviceBinding,
    InvalidDevice,
    StaleRunIdentity,
    StaleReference,
    MissingDispatchCapture,
    InvalidDispatchCapture,
    MissingAttCatalog,
    InvalidAttCatalog,
    InvalidAttManifest,
    InvalidAttReference,
    InvalidAttBinding,
    UnknownAttBinding,
    InvalidCoverage,
    NonCanonicalUnavailableFacts,
    InvalidRocprofJson,
    InvalidRocprofCsv,
    IdentityFailure,
    SizeOverflow,
    AllocationFailure,
    JsonEncode,
    JsonDecode,
    NonCanonicalEncoding,
}

impl fmt::Display for ProfilerBundleErrorV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "semantic profiler bundle rejected: {self:?}")
    }
}

impl Error for ProfilerBundleErrorV4 {}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_semantic_trace::OpaqueIdentityV1;
    use std::sync::{Arc, Barrier};

    fn content(byte: u8) -> ContentIdentityRecordV1 {
        ContentIdentityRecordV1 {
            scheme: ContentSchemeV1::DomainSeparatedSha256,
            format_version: 1,
            digest: CaptureIdentityV1::new([byte; 32]).expect("nonzero test identity"),
            canonical_len: 1,
        }
    }

    fn binding(node_id: u32) -> ProfilerDispatchBindingV4 {
        ProfilerDispatchBindingV4 {
            environment: ProfilerEnvironmentBindingV4 {
                environment: content(1),
                collector_tool: content(2),
                collector_configuration: content(3),
                stable_device_bindings: vec![ProfilerDeviceBindingV4 {
                    source_agent_id: u64::from(node_id),
                    stable_identity: content(4),
                }],
            },
            kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(
                OpaqueIdentityV1::new([5; 32]).expect("nonzero test identity"),
                1,
            )
            .expect("valid KIR claim"),
            artifact: None,
            source_map: None,
            wave_width: WaveWidthV1::Wave64,
        }
    }

    fn with_injected_dispatch_json_allocation_failure<T>(
        site: DispatchJsonAllocationInjectionSiteV4,
        operation: impl FnOnce() -> T,
    ) -> T {
        INJECT_DISPATCH_JSON_ALLOCATION_FAILURE_V4.with(|inject| {
            assert!(
                inject.replace(Some(site)).is_none(),
                "nested allocation injection"
            );
        });
        let result = operation();
        INJECT_DISPATCH_JSON_ALLOCATION_FAILURE_V4.with(|inject| {
            inject.set(None);
        });
        result
    }

    #[test]
    fn dispatch_json_public_boundaries_preserve_typed_allocation_failure() {
        let fixture = include_bytes!(
            "../tests/fixtures/rocprofv3-installed-97f5574-kernel-dispatch-schema.json"
        );
        let mut source: serde_json::Value = serde_json::from_slice(fixture).expect("fixture JSON");
        source["rocprofiler-sdk-tool"][0]["metadata"]["command"] = serde_json::json!(["command"]);
        source["rocprofiler-sdk-tool"][0]["metadata"]["config"] = serde_json::json!({"key": 1});
        let source = serde_json::to_vec(&source).expect("test source");
        let projection =
            project_rocprofv3_json_dispatch_agents_v4(&source).expect("reviewed source projects");
        let node_id = projection.agent_bindings()[0].node_id;

        assert!(matches!(
            with_injected_dispatch_json_allocation_failure(
                DispatchJsonAllocationInjectionSiteV4::Any,
                || { project_rocprofv3_json_dispatch_agents_v4(&source) },
            ),
            Err(ProfilerBundleErrorV4::AllocationFailure)
        ));
        assert!(matches!(
            with_injected_dispatch_json_allocation_failure(
                DispatchJsonAllocationInjectionSiteV4::Any,
                || { import_rocprofv3_json_profiler_bundle_v4(&source, binding(node_id),) },
            ),
            Err(ProfilerBundleErrorV4::AllocationFailure)
        ));
        let alias_result = with_injected_dispatch_json_allocation_failure(
            DispatchJsonAllocationInjectionSiteV4::Any,
            || {
                import_projected_rocprofv3_json_profiler_bundle_v4(
                    &source,
                    &projection,
                    binding(node_id),
                )
            },
        );
        assert!(matches!(
            alias_result,
            Err(ProfilerBundleErrorV4::AllocationFailure)
        ));
    }

    #[test]
    fn string_array_and_object_key_allocations_are_typed_at_public_boundary() {
        let fixture = include_bytes!(
            "../tests/fixtures/rocprofv3-installed-97f5574-kernel-dispatch-schema.json"
        );
        let mut source: serde_json::Value = serde_json::from_slice(fixture).expect("fixture JSON");
        source["rocprofiler-sdk-tool"][0]["metadata"]["command"] = serde_json::json!(["command"]);
        source["rocprofiler-sdk-tool"][0]["metadata"]["config"] = serde_json::json!({"key": 1});
        let source = serde_json::to_vec(&source).expect("test source");
        for site in [
            DispatchJsonAllocationInjectionSiteV4::StringArrayElement,
            DispatchJsonAllocationInjectionSiteV4::ObjectKey,
        ] {
            assert!(matches!(
                with_injected_dispatch_json_allocation_failure(site, || {
                    project_rocprofv3_json_dispatch_agents_v4(&source)
                }),
                Err(ProfilerBundleErrorV4::AllocationFailure)
            ));
        }
    }

    #[test]
    fn dispatch_json_allocation_marker_is_thread_local() {
        let source: &'static [u8] = include_bytes!(
            "../tests/fixtures/rocprofv3-installed-97f5574-kernel-dispatch-schema.json"
        );
        let barrier = Arc::new(Barrier::new(2));
        let failing_barrier = Arc::clone(&barrier);
        let failing = std::thread::spawn(move || {
            INJECT_DISPATCH_JSON_ALLOCATION_FAILURE_V4
                .with(|inject| inject.set(Some(DispatchJsonAllocationInjectionSiteV4::Any)));
            failing_barrier.wait();
            project_rocprofv3_json_dispatch_agents_v4(source)
        });
        let succeeding = std::thread::spawn(move || {
            barrier.wait();
            project_rocprofv3_json_dispatch_agents_v4(source)
        });
        assert!(matches!(
            failing.join().expect("failing projector thread"),
            Err(ProfilerBundleErrorV4::AllocationFailure)
        ));
        assert!(
            succeeding
                .join()
                .expect("independent projector thread")
                .is_ok()
        );
    }
}
