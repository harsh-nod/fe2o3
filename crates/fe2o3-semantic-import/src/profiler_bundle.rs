//! Unified, bounded profiler capture bundle.
//!
//! This schema composes existing closed dispatch/counter/sample formats with
//! exact environment and collector claims. It references ATT/Compute Viewer
//! artifacts but deliberately does not decode thread-trace payloads.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use fe2o3_semantic_trace::{ContentIdentityV1, KernelIrIdentityClaimV1, WaveWidthV1};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ArtifactClaimV1, CaptureIdentityV1, CaptureUnavailableReasonV1, ContentIdentityRecordV1,
    ContentSchemeV1, ImportLimitsV1, LossStateV1, LossStatusV1, RocprofCaptureBindingV1,
    SemanticCaptureV1, TruthOriginV1, import_rocprofv3_capture_with_agents_v1,
};

pub const PROFILER_BUNDLE_SCHEMA_VERSION_V4: u16 = 4;
pub const MAX_PROFILER_BUNDLE_BYTES_V4: u64 = 16 * 1024 * 1024;
pub const MAX_PROFILER_SOURCE_BYTES_V4: u64 = 8 * 1024 * 1024;
pub const MAX_PROFILER_DISPATCHES_V4: usize = 16_384;
pub const MAX_PROFILER_DEVICE_BINDINGS_V4: usize = 256;
pub const MAX_PROFILER_ATT_REFERENCES_V4: usize = 512;
pub const MAX_PROFILER_REFERENCE_BYTES_V4: usize = 256;
pub const MAX_PROFILER_CSV_COLUMNS_V4: usize = 32;
pub const MAX_PROFILER_CSV_FIELD_BYTES_V4: usize = 256;
pub const PROFILER_BUNDLE_IDENTITY_DOMAIN_V4: &[u8] = b"fe2o3.semantic-profiler-bundle.v4\0";

const PROFILER_SOURCE_CSV_DOMAIN_V4: &[u8] = b"fe2o3.profiler.rocprof-csv.v4\0";
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
        if self.devices.is_empty() || self.devices.len() > MAX_PROFILER_DISPATCHES_V4 {
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
                    ProfilerSourceKindV4::Rocprofv3KernelDispatchJson => self.source.value,
                    ProfilerSourceKindV4::Rocprofv3KernelDispatchCsv => {
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

pub fn import_rocprofv3_json_profiler_bundle_v4(
    source: &[u8],
    binding: ProfilerDispatchBindingV4,
) -> Result<SemanticProfilerBundleV4, ProfilerBundleErrorV4> {
    validate_source(source)?;
    validate_environment(&binding.environment)?;
    let imported = import_rocprofv3_capture_with_agents_v1(
        source,
        RocprofCaptureBindingV1 {
            kernel_ir_claim: binding.kernel_ir_claim,
            artifact: binding.artifact,
            source_map: binding.source_map,
            wave_width: binding.wave_width,
        },
        ImportLimitsV1::default(),
    )
    .map_err(|_| ProfilerBundleErrorV4::InvalidRocprofJson)?;
    let source = imported.capture.runs[0].source;
    finish_dispatch_bundle(
        ProfilerSourceKindV4::Rocprofv3KernelDispatchJson,
        source,
        source,
        imported.capture,
        imported.source_agent_ids,
        binding.environment,
    )
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
    .map_err(|_| ProfilerBundleErrorV4::InvalidRocprofCsv)?;
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
        if !CSV_ALLOWED_HEADERS.contains(&header)
            || positions.insert(header.to_owned(), index).is_some()
        {
            return Err(ProfilerBundleErrorV4::InvalidRocprofCsv);
        }
    }
    for required in CSV_REQUIRED_HEADERS {
        if !positions.contains_key(*required) {
            return Err(ProfilerBundleErrorV4::InvalidRocprofCsv);
        }
    }
    let mut processes: BTreeMap<u64, Vec<serde_json::Value>> = BTreeMap::new();
    let mut count = 0_usize;
    for row in reader.records() {
        let row = row.map_err(|_| ProfilerBundleErrorV4::InvalidRocprofCsv)?;
        if row
            .iter()
            .any(|field| field.len() > MAX_PROFILER_CSV_FIELD_BYTES_V4)
        {
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
        let process = parse_integer(field(&row, &positions, "Process_Id")?)?;
        let agent = parse_integer(field(&row, &positions, "Agent_Id")?)?;
        let start = parse_integer(field(&row, &positions, "Start_Timestamp")?)?;
        let end = parse_integer(field(&row, &positions, "End_Timestamp")?)?;
        let dimension = |prefix: &str, axis: &str| {
            parse_integer(field(&row, &positions, &format!("{prefix}_{axis}"))?)
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
    serde_json::to_vec(&serde_json::json!({"rocprofiler-sdk-tool":processes}))
        .map_err(|_| ProfilerBundleErrorV4::JsonEncode)
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
    let parsed = value
        .strip_prefix("0x")
        .map(|value| u64::from_str_radix(value, 16))
        .unwrap_or_else(|| value.parse());
    parsed.map_err(|_| ProfilerBundleErrorV4::InvalidRocprofCsv)
}

const CSV_REQUIRED_HEADERS: &[&str] = &[
    "Kind",
    "Agent_Id",
    "Process_Id",
    "Workgroup_Size_X",
    "Workgroup_Size_Y",
    "Workgroup_Size_Z",
    "Grid_Size_X",
    "Grid_Size_Y",
    "Grid_Size_Z",
    "Start_Timestamp",
    "End_Timestamp",
];

const CSV_ALLOWED_HEADERS: &[&str] = &[
    "Kind",
    "Agent_Id",
    "Queue_Id",
    "Process_Id",
    "Thread_Id",
    "Correlation_Id",
    "Kernel_Id",
    "Dispatch_Id",
    "Kernel_Name",
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
    "Start_Timestamp",
    "End_Timestamp",
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
