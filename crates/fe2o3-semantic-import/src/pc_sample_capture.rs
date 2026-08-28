use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use fe2o3_semantic_trace::{LaunchGeometryV1, WaveWidthV1};

use crate::{
    CaptureDeviceV1, CaptureIdentityV1, CaptureRunV1, CaptureUnavailableReasonV1,
    CompletenessScopeV1, IdentityFactV1, KernelIrClaimRecordV1, LaunchRecordV1, LossStateV1,
    LossStatusV1, TruthOriginV1,
};

pub const PC_SAMPLE_CAPTURE_SCHEMA_VERSION_V3: u16 = 3;
pub const MAX_PC_SAMPLE_CAPTURE_BYTES_V3: u64 = 8 * 1024 * 1024;
pub const MAX_PC_SAMPLE_DISPATCHES_V3: usize = 16_384;
pub const MAX_PC_SAMPLE_RECORDS_V3: usize = 65_536;
pub const MAX_PC_SAMPLE_CODE_OBJECTS_V3: usize = 16_384;
pub const PC_SAMPLE_CAPTURE_IDENTITY_DOMAIN_V3: &[u8] = b"fe2o3.semantic-pc-sample-capture.v3\0";
pub(crate) const PC_SAMPLE_CODE_OBJECT_IDENTITY_DOMAIN_V3: &[u8] =
    b"fe2o3.semantic-pc-sample-capture.code-object.v3\0";
pub(crate) const PC_SAMPLE_DISPATCH_IDENTITY_DOMAIN_V3: &[u8] =
    b"fe2o3.semantic-pc-sample-capture.dispatch.v3\0";
pub(crate) const PC_SAMPLE_RECORD_IDENTITY_DOMAIN_V3: &[u8] =
    b"fe2o3.semantic-pc-sample-capture.record.v3\0";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcSampleCaptureSourceKindV3 {
    Rocprofv3StochasticPcSamplingJson,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PcSampleCodeObjectV3 {
    pub identity: CaptureIdentityV1,
    pub identity_origin: TruthOriginV1,
    pub source_code_object_ordinal: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum PcInstructionTypeV3 {
    None = 0,
    Valu = 1,
    Matrix = 2,
    Scalar = 3,
    Tex = 4,
    Lds = 5,
    LdsDirect = 6,
    Flat = 7,
    Export = 8,
    Message = 9,
    Barrier = 10,
    BranchNotTaken = 11,
    BranchTaken = 12,
    Jump = 13,
    Other = 14,
    NoInstruction = 15,
    DualValu = 16,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcPositionUnavailableReasonV3 {
    NativeVirtualAddressRedacted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PcPositionV3 {
    pub origin: TruthOriginV1,
    pub code_object_identity: Option<CaptureIdentityV1>,
    pub code_object_offset: Option<u64>,
    pub unavailable_reason: Option<PcPositionUnavailableReasonV3>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcTimestampDomainV3 {
    RocprofilerOpaqueCollectorClock,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PcSampleTimestampV3 {
    pub origin: TruthOriginV1,
    pub domain: PcTimestampDomainV3,
    pub ticks: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PcSampleWaveLocationV3 {
    pub origin: TruthOriginV1,
    pub workgroup: [u64; 3],
    pub wave_in_group: u8,
    pub chiplet: u8,
    pub shader_engine: u8,
    pub shader_array: u8,
    pub cu_or_wgp: u8,
    pub simd: u8,
    pub wave_slot: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PcSampleRecordV3 {
    pub identity: CaptureIdentityV1,
    pub origin: TruthOriginV1,
    pub source_record_ordinal: u64,
    pub dispatch_identity: CaptureIdentityV1,
    pub pc: PcPositionV3,
    pub timestamp: PcSampleTimestampV3,
    /// Logical active-lane mask copied from rocprofiler. It does not prove that
    /// each set lane executed the sampled instruction.
    pub exec_mask: u64,
    pub wave: PcSampleWaveLocationV3,
    pub wave_issued: bool,
    pub instruction_type: PcInstructionTypeV3,
    pub active_wave_count_on_cu: u32,
    pub memory_counters_present_but_not_imported: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcSourceAndIsaCorrelationV3 {
    UnavailableNoAuthenticatedSourceOrIsaMap,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PcSampleDispatchV3 {
    pub identity: CaptureIdentityV1,
    pub identity_origin: TruthOriginV1,
    pub run_identity: CaptureIdentityV1,
    pub device_identity: CaptureIdentityV1,
    pub process_index: u32,
    pub dispatch_index: u32,
    pub source_dispatch_ordinal: u64,
    pub kernel_ir: KernelIrClaimRecordV1,
    pub artifact: IdentityFactV1,
    pub source_map: IdentityFactV1,
    pub source_and_isa_correlation: PcSourceAndIsaCorrelationV3,
    pub launch_origin: TruthOriginV1,
    pub launch: LaunchRecordV1,
    pub timing_origin: TruthOriginV1,
    pub start_timestamp: u64,
    pub end_timestamp: u64,
    pub duration_ticks: u64,
    pub sample_count: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcSamplingMethodV3 {
    Stochastic,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcSamplingUnitV3 {
    Cycles,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PcSamplingConfigurationV3 {
    pub method_origin: TruthOriginV1,
    pub method: PcSamplingMethodV3,
    pub unit_origin: TruthOriginV1,
    pub unit: PcSamplingUnitV3,
    pub interval_origin: TruthOriginV1,
    pub interval: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcSampleScopeV3 {
    StochasticSamplesOnly,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcExecMaskMeaningV3 {
    RocprofilerActiveLaneMaskNoPerLaneInstructionExecutionProof,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PcExecMaskSemanticsV3 {
    pub origin: TruthOriginV1,
    pub meaning: PcExecMaskMeaningV3,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PcSampleCaptureCoverageV3 {
    pub origin: TruthOriginV1,
    pub scope: CompletenessScopeV1,
    pub pc_sample_scope: PcSampleScopeV3,
    pub source_dispatch_records: u64,
    pub captured_dispatch_records: u64,
    pub source_pc_sample_records: u64,
    pub captured_pc_sample_records: u64,
    pub sampling: PcSamplingConfigurationV3,
    pub exec_mask_semantics: PcExecMaskSemanticsV3,
    pub loss: LossStatusV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPcSampleCaptureV3 {
    pub schema_version: u16,
    pub source_kind: PcSampleCaptureSourceKindV3,
    pub runs: Vec<CaptureRunV1>,
    pub devices: Vec<CaptureDeviceV1>,
    pub code_objects: Vec<PcSampleCodeObjectV3>,
    pub dispatches: Vec<PcSampleDispatchV3>,
    pub samples: Vec<PcSampleRecordV3>,
    pub coverage: PcSampleCaptureCoverageV3,
}

impl SemanticPcSampleCaptureV3 {
    pub fn validate(&self) -> Result<(), PcSampleCaptureErrorV3> {
        if self.schema_version != PC_SAMPLE_CAPTURE_SCHEMA_VERSION_V3 {
            return Err(PcSampleCaptureErrorV3::UnsupportedVersion(
                self.schema_version,
            ));
        }
        if self.runs.len() != 1 {
            return Err(PcSampleCaptureErrorV3::InvalidRunCount);
        }
        if self.devices.is_empty() || self.devices.len() > self.dispatches.len() {
            return Err(PcSampleCaptureErrorV3::InvalidDeviceCatalog);
        }
        if self.code_objects.len() > MAX_PC_SAMPLE_CODE_OBJECTS_V3 {
            return Err(PcSampleCaptureErrorV3::InvalidCodeObjectCatalog);
        }
        if self.dispatches.is_empty() || self.dispatches.len() > MAX_PC_SAMPLE_DISPATCHES_V3 {
            return Err(PcSampleCaptureErrorV3::InvalidDispatchCount);
        }
        if self.samples.is_empty() || self.samples.len() > MAX_PC_SAMPLE_RECORDS_V3 {
            return Err(PcSampleCaptureErrorV3::InvalidSampleCount);
        }

        let run = &self.runs[0];
        if run.source.scheme != crate::ContentSchemeV1::DomainSeparatedSha256
            || run.source.format_version != 1
            || run.source.canonical_len == 0
        {
            return Err(PcSampleCaptureErrorV3::InvalidSourceIdentity);
        }
        if run.identity_origin != TruthOriginV1::Observed
            || run.identity
                != crate::capture::derive_identity(
                    crate::capture::RUN_IDENTITY_DOMAIN_V1,
                    run.source.digest,
                    0,
                )
                .map_err(|_| PcSampleCaptureErrorV3::IdentityFailure)?
        {
            return Err(PcSampleCaptureErrorV3::StaleRunIdentity);
        }
        let mut device_ids = BTreeSet::new();
        for (ordinal, device) in (0_u64..).zip(&self.devices) {
            if device.source_device_ordinal != ordinal
                || device.identity_origin != TruthOriginV1::Observed
                || device.identity
                    != crate::capture::derive_identity(
                        crate::capture::DEVICE_IDENTITY_DOMAIN_V1,
                        run.source.digest,
                        ordinal,
                    )
                    .map_err(|_| PcSampleCaptureErrorV3::IdentityFailure)?
                || !device_ids.insert(device.identity)
            {
                return Err(PcSampleCaptureErrorV3::InvalidDeviceCatalog);
            }
        }
        let mut code_object_ids = BTreeSet::new();
        for (ordinal, code_object) in (0_u64..).zip(&self.code_objects) {
            if code_object.source_code_object_ordinal != ordinal
                || code_object.identity_origin != TruthOriginV1::Observed
                || code_object.identity
                    != crate::capture::derive_identity(
                        PC_SAMPLE_CODE_OBJECT_IDENTITY_DOMAIN_V3,
                        run.source.digest,
                        ordinal,
                    )
                    .map_err(|_| PcSampleCaptureErrorV3::IdentityFailure)?
                || !code_object_ids.insert(code_object.identity)
            {
                return Err(PcSampleCaptureErrorV3::InvalidCodeObjectCatalog);
            }
        }

        let mut dispatch_ids = BTreeSet::new();
        let mut dispatch_launches = BTreeMap::new();
        let mut dispatch_sample_counts = BTreeMap::new();
        let mut prior_selector = None;
        let mut prior_source_ordinal = None;
        for dispatch in &self.dispatches {
            if !valid_launch(dispatch.launch) {
                return Err(PcSampleCaptureErrorV3::InvalidDispatchEnvelope);
            }
            if dispatch.run_identity != run.identity
                || !device_ids.contains(&dispatch.device_identity)
                || dispatch.identity_origin != TruthOriginV1::Observed
                || dispatch.identity
                    != derive_pc_dispatch_identity_v3(
                        run.source.digest,
                        PcDispatchIdentityFieldsV3 {
                            source_ordinal: dispatch.source_dispatch_ordinal,
                            process_index: dispatch.process_index,
                            dispatch_index: dispatch.dispatch_index,
                            device: dispatch.device_identity,
                            launch: dispatch.launch,
                            start: dispatch.start_timestamp,
                            end: dispatch.end_timestamp,
                        },
                    )?
                || !dispatch_ids.insert(dispatch.identity)
            {
                return Err(PcSampleCaptureErrorV3::InvalidDispatchIdentity);
            }
            validate_filtered_selector(
                prior_selector,
                dispatch.process_index,
                dispatch.dispatch_index,
            )?;
            prior_selector = Some((dispatch.process_index, dispatch.dispatch_index));
            if prior_source_ordinal.is_some_and(|prior| prior >= dispatch.source_dispatch_ordinal) {
                return Err(PcSampleCaptureErrorV3::NonCanonicalSourceSelector);
            }
            prior_source_ordinal = Some(dispatch.source_dispatch_ordinal);
            if dispatch.start_timestamp > dispatch.end_timestamp
                || dispatch.end_timestamp - dispatch.start_timestamp != dispatch.duration_ticks
                || dispatch.launch_origin != TruthOriginV1::Observed
                || dispatch.timing_origin != TruthOriginV1::Observed
                || dispatch.sample_count == 0
                || dispatch.source_and_isa_correlation
                    != PcSourceAndIsaCorrelationV3::UnavailableNoAuthenticatedSourceOrIsaMap
                || dispatch.kernel_ir.origin != TruthOriginV1::Declared
                || dispatch.kernel_ir.wire_version != 7
                || dispatch.kernel_ir.identity_policy != 1
                || dispatch.kernel_ir.canonical_len == 0
            {
                return Err(PcSampleCaptureErrorV3::InvalidDispatchEnvelope);
            }
            dispatch
                .artifact
                .validate()
                .map_err(|_| PcSampleCaptureErrorV3::InvalidContentClaim)?;
            dispatch
                .source_map
                .validate()
                .map_err(|_| PcSampleCaptureErrorV3::InvalidContentClaim)?;
            dispatch_sample_counts.insert(dispatch.identity, 0_u64);
            dispatch_launches.insert(dispatch.identity, dispatch.launch);
        }

        for (ordinal, sample) in (0_u64..).zip(&self.samples) {
            validate_sample(
                sample,
                ordinal,
                run.source.digest,
                &dispatch_ids,
                &dispatch_launches,
                &code_object_ids,
            )?;
            let count = dispatch_sample_counts
                .get_mut(&sample.dispatch_identity)
                .ok_or(PcSampleCaptureErrorV3::InvalidDispatchReference)?;
            *count = count
                .checked_add(1)
                .ok_or(PcSampleCaptureErrorV3::SizeOverflow)?;
        }
        for dispatch in &self.dispatches {
            if dispatch_sample_counts.get(&dispatch.identity).copied()
                != Some(dispatch.sample_count)
            {
                return Err(PcSampleCaptureErrorV3::InvalidDispatchSampleCount);
            }
        }

        let dispatch_count = u64::try_from(self.dispatches.len())
            .map_err(|_| PcSampleCaptureErrorV3::SizeOverflow)?;
        let sample_count =
            u64::try_from(self.samples.len()).map_err(|_| PcSampleCaptureErrorV3::SizeOverflow)?;
        if self.coverage.origin != TruthOriginV1::Declared
            || self.coverage.scope != CompletenessScopeV1::PartialSemanticExecutionHistory
            || self.coverage.pc_sample_scope != PcSampleScopeV3::StochasticSamplesOnly
            || self.coverage.source_dispatch_records < dispatch_count
            || self.coverage.captured_dispatch_records != dispatch_count
            || self.coverage.source_pc_sample_records != sample_count
            || self.coverage.captured_pc_sample_records != sample_count
            || self.coverage.sampling.method_origin != TruthOriginV1::Observed
            || self.coverage.sampling.method != PcSamplingMethodV3::Stochastic
            || self.coverage.sampling.unit_origin != TruthOriginV1::Declared
            || self.coverage.sampling.unit != PcSamplingUnitV3::Cycles
            || self.coverage.sampling.interval_origin != TruthOriginV1::Declared
            || self.coverage.sampling.interval == 0
            || self.coverage.exec_mask_semantics != (PcExecMaskSemanticsV3 {
                origin: TruthOriginV1::Declared,
                meaning:
                    PcExecMaskMeaningV3::RocprofilerActiveLaneMaskNoPerLaneInstructionExecutionProof,
            })
            || self.coverage.loss
                != (LossStatusV1 {
                    origin: TruthOriginV1::Unavailable,
                    state: LossStateV1::Unknown,
                    lost_records: None,
                    unavailable_reason: Some(CaptureUnavailableReasonV1::CollectorLossUnknown),
                })
        {
            return Err(PcSampleCaptureErrorV3::InvalidCoverage);
        }
        Ok(())
    }
}

fn validate_sample(
    sample: &PcSampleRecordV3,
    ordinal: u64,
    source: CaptureIdentityV1,
    dispatch_ids: &BTreeSet<CaptureIdentityV1>,
    dispatch_launches: &BTreeMap<CaptureIdentityV1, LaunchRecordV1>,
    code_object_ids: &BTreeSet<CaptureIdentityV1>,
) -> Result<(), PcSampleCaptureErrorV3> {
    // These are the field widths of rocprofiler_pc_sampling_hw_id_v0_t and
    // rocprofiler_pc_sampling_record_stochastic_v0_t in the 1.1 SDK contract,
    // not topology assumptions about one GPU generation.
    if sample.source_record_ordinal != ordinal
        || sample.origin != TruthOriginV1::Observed
        || !dispatch_ids.contains(&sample.dispatch_identity)
        || sample.timestamp.origin != TruthOriginV1::Observed
        || sample.timestamp.domain != PcTimestampDomainV3::RocprofilerOpaqueCollectorClock
        || sample.wave.origin != TruthOriginV1::Observed
        || sample.wave.wave_in_group > 15
        || sample.wave.chiplet > 63
        || sample.wave.shader_engine > 31
        || sample.wave.shader_array > 1
        || sample.wave.cu_or_wgp > 15
        || sample.wave.simd > 3
        || sample.wave.wave_slot > 127
        || dispatch_launches
            .get(&sample.dispatch_identity)
            .is_none_or(|launch| !wave_location_within_launch(sample.wave, *launch))
    {
        return Err(PcSampleCaptureErrorV3::InvalidSampleRecord);
    }
    match (
        sample.pc.origin,
        sample.pc.code_object_identity,
        sample.pc.code_object_offset,
        sample.pc.unavailable_reason,
    ) {
        (TruthOriginV1::Observed, Some(identity), Some(_), None)
            if code_object_ids.contains(&identity) => {}
        (
            TruthOriginV1::Unavailable,
            None,
            None,
            Some(PcPositionUnavailableReasonV3::NativeVirtualAddressRedacted),
        ) => {}
        _ => return Err(PcSampleCaptureErrorV3::InvalidPcPosition),
    }
    if sample.identity != derive_pc_sample_identity_v3(source, sample)? {
        return Err(PcSampleCaptureErrorV3::InvalidSampleIdentity);
    }
    Ok(())
}

fn wave_location_within_launch(wave: PcSampleWaveLocationV3, launch: LaunchRecordV1) -> bool {
    if (0..3).any(|axis| wave.workgroup[axis] >= u64::from(launch.grid_workgroups[axis])) {
        return false;
    }
    let workgroup_threads = launch
        .workgroup_size
        .into_iter()
        .fold(1_u128, |total, dimension| total * u128::from(dimension));
    let wave_width = u128::from(launch.wave_width);
    let waves = workgroup_threads.div_ceil(wave_width);
    u128::from(wave.wave_in_group) < waves
}

fn valid_launch(launch: LaunchRecordV1) -> bool {
    let wave_width = match launch.wave_width {
        32 => WaveWidthV1::Wave32,
        64 => WaveWidthV1::Wave64,
        _ => return false,
    };
    LaunchGeometryV1::new_exact(
        launch.logical_grid,
        launch.grid_workgroups,
        launch.workgroup_size,
        wave_width,
    )
    .is_ok()
}

fn validate_filtered_selector(
    prior: Option<(u32, u32)>,
    process: u32,
    dispatch: u32,
) -> Result<(), PcSampleCaptureErrorV3> {
    match prior {
        None => Ok(()),
        Some((prior_process, prior_dispatch)) if process == prior_process => {
            if dispatch > prior_dispatch {
                Ok(())
            } else {
                Err(PcSampleCaptureErrorV3::NonCanonicalSourceSelector)
            }
        }
        Some((prior_process, _)) if process > prior_process => Ok(()),
        Some(_) => Err(PcSampleCaptureErrorV3::NonCanonicalSourceSelector),
    }
}

pub(crate) struct PcDispatchIdentityFieldsV3 {
    pub source_ordinal: u64,
    pub process_index: u32,
    pub dispatch_index: u32,
    pub device: CaptureIdentityV1,
    pub launch: LaunchRecordV1,
    pub start: u64,
    pub end: u64,
}

pub(crate) fn derive_pc_dispatch_identity_v3(
    source: CaptureIdentityV1,
    fields: PcDispatchIdentityFieldsV3,
) -> Result<CaptureIdentityV1, PcSampleCaptureErrorV3> {
    let mut digest = Sha256::new();
    digest.update(PC_SAMPLE_DISPATCH_IDENTITY_DOMAIN_V3);
    digest.update(source.as_bytes());
    digest.update(fields.source_ordinal.to_le_bytes());
    digest.update(fields.process_index.to_le_bytes());
    digest.update(fields.dispatch_index.to_le_bytes());
    digest.update(fields.device.as_bytes());
    for value in fields.launch.logical_grid {
        digest.update(value.to_le_bytes());
    }
    for value in fields.launch.grid_workgroups {
        digest.update(value.to_le_bytes());
    }
    for value in fields.launch.workgroup_size {
        digest.update(value.to_le_bytes());
    }
    digest.update(fields.launch.wave_width.to_le_bytes());
    digest.update(fields.start.to_le_bytes());
    digest.update(fields.end.to_le_bytes());
    CaptureIdentityV1::new(digest.finalize().into())
        .map_err(|_| PcSampleCaptureErrorV3::IdentityFailure)
}

pub(crate) fn derive_pc_sample_identity_v3(
    source: CaptureIdentityV1,
    sample: &PcSampleRecordV3,
) -> Result<CaptureIdentityV1, PcSampleCaptureErrorV3> {
    let mut digest = Sha256::new();
    digest.update(PC_SAMPLE_RECORD_IDENTITY_DOMAIN_V3);
    digest.update(source.as_bytes());
    digest.update(sample.source_record_ordinal.to_le_bytes());
    digest.update(sample.dispatch_identity.as_bytes());
    digest.update([match sample.pc.origin {
        TruthOriginV1::Declared => 0,
        TruthOriginV1::Proved => 1,
        TruthOriginV1::Observed => 2,
        TruthOriginV1::Inferred => 3,
        TruthOriginV1::Unavailable => 4,
    }]);
    digest.update(
        sample
            .pc
            .code_object_identity
            .map(CaptureIdentityV1::as_bytes)
            .unwrap_or([0; 32]),
    );
    digest.update(sample.pc.code_object_offset.unwrap_or(0).to_le_bytes());
    digest.update([sample
        .pc
        .unavailable_reason
        .map_or(0, |value| value as u8 + 1)]);
    digest.update(sample.timestamp.ticks.to_le_bytes());
    digest.update(sample.exec_mask.to_le_bytes());
    for value in sample.wave.workgroup {
        digest.update(value.to_le_bytes());
    }
    digest.update([
        sample.wave.wave_in_group,
        sample.wave.chiplet,
        sample.wave.shader_engine,
        sample.wave.shader_array,
        sample.wave.cu_or_wgp,
        sample.wave.simd,
        sample.wave.wave_slot,
        u8::from(sample.wave_issued),
        sample.instruction_type as u8,
        u8::from(sample.memory_counters_present_but_not_imported),
    ]);
    digest.update(sample.active_wave_count_on_cu.to_le_bytes());
    CaptureIdentityV1::new(digest.finalize().into())
        .map_err(|_| PcSampleCaptureErrorV3::IdentityFailure)
}

pub fn encode_pc_sample_capture_v3(
    capture: &SemanticPcSampleCaptureV3,
) -> Result<Vec<u8>, PcSampleCaptureErrorV3> {
    capture.validate()?;
    let bytes = serde_json::to_vec(capture).map_err(|_| PcSampleCaptureErrorV3::JsonEncode)?;
    if bytes.len() as u64 > MAX_PC_SAMPLE_CAPTURE_BYTES_V3 {
        return Err(PcSampleCaptureErrorV3::CaptureTooLarge {
            actual: bytes.len() as u64,
            max: MAX_PC_SAMPLE_CAPTURE_BYTES_V3,
        });
    }
    Ok(bytes)
}

pub fn decode_pc_sample_capture_v3(
    bytes: &[u8],
) -> Result<SemanticPcSampleCaptureV3, PcSampleCaptureErrorV3> {
    let actual = u64::try_from(bytes.len()).map_err(|_| PcSampleCaptureErrorV3::SizeOverflow)?;
    if actual == 0 || actual > MAX_PC_SAMPLE_CAPTURE_BYTES_V3 {
        return Err(PcSampleCaptureErrorV3::CaptureTooLarge {
            actual,
            max: MAX_PC_SAMPLE_CAPTURE_BYTES_V3,
        });
    }
    let capture: SemanticPcSampleCaptureV3 =
        serde_json::from_slice(bytes).map_err(|_| PcSampleCaptureErrorV3::JsonDecode)?;
    capture.validate()?;
    if serde_json::to_vec(&capture).map_err(|_| PcSampleCaptureErrorV3::JsonEncode)? != bytes {
        return Err(PcSampleCaptureErrorV3::NonCanonicalEncoding);
    }
    Ok(capture)
}

pub fn pc_sample_capture_content_identity_v3(
    bytes: &[u8],
) -> Result<crate::ContentIdentityRecordV1, PcSampleCaptureErrorV3> {
    let _ = decode_pc_sample_capture_v3(bytes)?;
    let mut digest = Sha256::new();
    digest.update(PC_SAMPLE_CAPTURE_IDENTITY_DOMAIN_V3);
    digest.update(bytes);
    Ok(crate::ContentIdentityRecordV1 {
        scheme: crate::ContentSchemeV1::DomainSeparatedSha256,
        format_version: PC_SAMPLE_CAPTURE_SCHEMA_VERSION_V3,
        digest: CaptureIdentityV1::new(digest.finalize().into())
            .map_err(|_| PcSampleCaptureErrorV3::IdentityFailure)?,
        canonical_len: u64::try_from(bytes.len())
            .map_err(|_| PcSampleCaptureErrorV3::SizeOverflow)?,
    })
}

#[derive(Debug)]
pub enum PcSampleCaptureErrorV3 {
    UnsupportedVersion(u16),
    InvalidRunCount,
    InvalidSourceIdentity,
    InvalidDeviceCatalog,
    InvalidCodeObjectCatalog,
    InvalidDispatchCount,
    InvalidSampleCount,
    StaleRunIdentity,
    InvalidDispatchIdentity,
    NonCanonicalSourceSelector,
    InvalidDispatchEnvelope,
    InvalidContentClaim,
    InvalidDispatchReference,
    InvalidDispatchSampleCount,
    InvalidSampleRecord,
    InvalidPcPosition,
    InvalidSampleIdentity,
    InvalidCoverage,
    IdentityFailure,
    SizeOverflow,
    CaptureTooLarge { actual: u64, max: u64 },
    NonCanonicalEncoding,
    JsonEncode,
    JsonDecode,
}

impl fmt::Display for PcSampleCaptureErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "semantic PC sample capture rejected: {self:?}")
    }
}

impl Error for PcSampleCaptureErrorV3 {}
