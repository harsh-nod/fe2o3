//! Canonical admission for externally decoded ROCprofiler ATT callback streams.
//!
//! This module does not load or execute the experimental trace decoder. It admits a bounded,
//! canonical export of the ROCm 7.2.4 callback ABI and binds that self-claimed export to an exact
//! ATT Bundle V4. Decoder fields remain external-decoder declarations, not observed hardware facts.

use std::error::Error;
use std::fmt;
use std::io::Write;

use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    AttReferenceKindV4, CaptureIdentityV1, CaptureUnavailableReasonV1, ContentIdentityRecordV1,
    ContentSchemeV1, IdentityFactV1, ProfilerSourceKindV4, TruthOriginV1,
    decode_profiler_bundle_v4, profiler_bundle_content_identity_v4,
};

pub const DECODED_ATT_SCHEMA_VERSION_V1: u16 = 1;
pub const MAX_DECODED_ATT_EXPORT_BYTES_V1: u64 = 32 * 1024 * 1024;
pub const MAX_DECODED_ATT_INTERCHANGE_BYTES_V1: u64 = 32 * 1024 * 1024;
pub const MAX_DECODED_ATT_CALLBACKS_V1: usize = 65_536;
pub const MAX_DECODED_ATT_RECORDS_V1: usize = 262_144;
pub const MAX_DECODED_ATT_WAVES_V1: usize = 65_536;
pub const MAX_DECODED_ATT_WAVE_STATES_V1: usize = 1_048_576;
pub const MAX_DECODED_ATT_INSTRUCTIONS_V1: usize = 1_048_576;
pub const MAX_DECODED_ATT_CODE_OBJECTS_V1: usize = 16_384;
pub const MAX_DECODED_ATT_REFERENCE_BYTES_V1: usize = 4_096;
pub const DECODED_ATT_INTERCHANGE_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.semantic-decoded-att.v1\0";
const DECODED_ATT_EXPORT_SOURCE_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.rocprofiler-decoded-att-export.source.v1\0";
const DECODED_ATT_CODE_OBJECT_SELECTOR_DOMAIN_V1: &[u8] =
    b"fe2o3.decoded-att.code-object-selector.v1\0";
const DECODED_ATT_CODE_OBJECT_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.decoded-att.code-object.v1\0";
const DECODED_ATT_RECORD_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.decoded-att.record.v1\0";

pub const ROCPROFILER_SDK_7_2_4_TRACE_DECODER_TYPES_HEADER_SHA256_V1: [u8; 32] = [
    0x1d, 0x38, 0x96, 0xcd, 0xe4, 0xe5, 0x33, 0xfb, 0x21, 0xa3, 0xef, 0x74, 0xd3, 0x97, 0x8a, 0x02,
    0x14, 0x65, 0xb7, 0xb0, 0xa4, 0x56, 0xf5, 0xe4, 0x0c, 0xc0, 0x23, 0xc6, 0x8a, 0xf0, 0x61, 0xfa,
];
pub const ROCPROFILER_SDK_7_2_4_TRACE_DECODER_TYPES_HEADER_BYTES_V1: u64 = 10_789;
pub const ROCPROFILER_SDK_7_2_4_TRACE_DECODER_API_HEADER_SHA256_V1: [u8; 32] = [
    0x28, 0x38, 0xcc, 0x93, 0xd5, 0xf7, 0xd2, 0x0a, 0xd6, 0x58, 0x66, 0x79, 0x90, 0xa3, 0xa8, 0x9f,
    0x4e, 0x9b, 0x2e, 0xd2, 0x8c, 0xd2, 0x93, 0x27, 0x8b, 0x7f, 0xd3, 0xe9, 0x47, 0xff, 0x00, 0x6b,
];
pub const ROCPROFILER_SDK_7_2_4_TRACE_DECODER_API_HEADER_BYTES_V1: u64 = 7_500;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodedAttSourceKindV1 {
    RocprofilerSdk724TraceDecoderCallbacks,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodedAttRecordOriginV1 {
    ExternalDecoderDeclared,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodedAttAuthenticityV1 {
    UnavailableSelfClaimedExternalDecoder,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodedAttRawRelationV1 {
    ExternalDecoderDeclaredCompleteExactWaveInputs,
    UnavailableMissingExactRawReferenceIdentity,
    UnavailableIncompleteWaveReferenceCoverage,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodedAttCompletenessV1 {
    UnknownNoPositiveDecoderCompletenessSignal,
    IncompleteInfoReported,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodedAttLossStateV1 {
    Unknown,
    ExternalDecoderReportedDataLost,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodedAttInfoKindV1 {
    None,
    DataLost,
    StitchIncomplete,
    WaveIncomplete,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodedAttWaveStateKindV1 {
    Empty,
    Idle,
    Exec,
    Wait,
    Stall,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodedAttInstructionCategoryV1 {
    None,
    Smem,
    Salu,
    Vmem,
    Flat,
    Lds,
    Valu,
    Jump,
    Next,
    Immed,
    Context,
    Message,
    Bvh,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodedAttPcAvailabilityV1 {
    ElfVirtualAddress,
    UnavailableNativeVirtualAddressRedacted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodedAttCallbackKindV1 {
    Gfxip,
    Occupancy,
    Perfevent,
    Wave,
    Info,
    Shaderdata,
    Realtime,
    RtFrequency,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttCallbackSummaryV1 {
    pub ordinal: u64,
    pub source_reference_ordinal: u32,
    pub kind: DecodedAttCallbackKindV1,
    pub record_count: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttPcV1 {
    pub origin: DecodedAttRecordOriginV1,
    pub availability: DecodedAttPcAvailabilityV1,
    pub code_object: Option<CaptureIdentityV1>,
    pub elf_virtual_address: Option<u64>,
    pub unavailable_reason: Option<CaptureUnavailableReasonV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttRawReferenceV1 {
    pub ordinal: u32,
    pub kind: AttReferenceKindV4,
    pub reference: String,
    pub content: IdentityFactV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttDecoderIdentityV1 {
    pub trace_decoder_types_header: ContentIdentityRecordV1,
    pub trace_decoder_api_header: ContentIdentityRecordV1,
    pub decoder_library: ContentIdentityRecordV1,
    pub exporter_tool: ContentIdentityRecordV1,
    pub identity_origin: TruthOriginV1,
    pub authenticity: DecodedAttAuthenticityV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttRawDecodeRelationV1 {
    pub origin: TruthOriginV1,
    pub state: DecodedAttRawRelationV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttCodeObjectV1 {
    pub identity: CaptureIdentityV1,
    pub selector: CaptureIdentityV1,
    pub origin: DecodedAttRecordOriginV1,
    pub artifact: ContentIdentityRecordV1,
    pub load_size: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttOccupancyV1 {
    pub identity: CaptureIdentityV1,
    pub origin: DecodedAttRecordOriginV1,
    pub source_callback_ordinal: u64,
    pub source_record_ordinal: u64,
    pub source_reference_ordinal: u32,
    pub pc: DecodedAttPcV1,
    pub time: u64,
    pub cu_or_wgp: u8,
    pub simd: u8,
    pub wave_slot: u8,
    pub started: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttPerfEventV1 {
    pub identity: CaptureIdentityV1,
    pub origin: DecodedAttRecordOriginV1,
    pub source_callback_ordinal: u64,
    pub source_record_ordinal: u64,
    pub source_reference_ordinal: u32,
    pub time: i64,
    pub events: [u16; 4],
    pub cu_or_wgp: u8,
    pub bank: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttWaveStateV1 {
    pub identity: CaptureIdentityV1,
    pub origin: DecodedAttRecordOriginV1,
    pub ordinal: u64,
    pub state: DecodedAttWaveStateKindV1,
    pub duration_cycles: i32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttInstructionV1 {
    pub identity: CaptureIdentityV1,
    pub origin: DecodedAttRecordOriginV1,
    pub ordinal: u64,
    pub category: DecodedAttInstructionCategoryV1,
    pub stall_cycles: u32,
    pub duration_cycles: i32,
    pub time: i64,
    pub pc: DecodedAttPcV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttWaveV1 {
    pub identity: CaptureIdentityV1,
    pub origin: DecodedAttRecordOriginV1,
    pub source_callback_ordinal: u64,
    pub source_record_ordinal: u64,
    pub source_reference_ordinal: u32,
    pub cu_or_wgp: u8,
    pub simd: u8,
    pub wave_slot: u8,
    pub contexts: u8,
    pub begin_time: i64,
    pub end_time: i64,
    #[serde(deserialize_with = "deserialize_wave_states")]
    pub timeline: Vec<DecodedAttWaveStateV1>,
    #[serde(deserialize_with = "deserialize_instructions")]
    pub instructions: Vec<DecodedAttInstructionV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttShaderDataV1 {
    pub identity: CaptureIdentityV1,
    pub origin: DecodedAttRecordOriginV1,
    pub source_callback_ordinal: u64,
    pub source_record_ordinal: u64,
    pub source_reference_ordinal: u32,
    pub time: i64,
    pub value: u64,
    pub cu_or_wgp: u8,
    pub simd: u8,
    pub wave_slot: u8,
    pub private_trap_handler: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttRealtimeV1 {
    pub identity: CaptureIdentityV1,
    pub origin: DecodedAttRecordOriginV1,
    pub source_callback_ordinal: u64,
    pub source_record_ordinal: u64,
    pub source_reference_ordinal: u32,
    pub shader_clock: i64,
    pub realtime_clock: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttInfoV1 {
    pub identity: CaptureIdentityV1,
    pub origin: DecodedAttRecordOriginV1,
    pub source_callback_ordinal: u64,
    pub source_record_ordinal: u64,
    pub source_reference_ordinal: u32,
    pub kind: DecodedAttInfoKindV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttCoverageV1 {
    pub origin: DecodedAttRecordOriginV1,
    pub completeness: DecodedAttCompletenessV1,
    pub loss: DecodedAttLossStateV1,
    pub callback_count: u64,
    pub wave_reference_count: u64,
    pub decoded_wave_reference_count: u64,
    pub occupancy_count: u64,
    pub perf_event_count: u64,
    pub wave_count: u64,
    pub wave_state_count: u64,
    pub instruction_count: u64,
    pub shader_data_count: u64,
    pub realtime_count: u64,
    pub info_count: u64,
    pub data_lost_info_count: u64,
    pub stitch_incomplete_info_count: u64,
    pub wave_incomplete_info_count: u64,
    pub debug_record_support: TruthOriginV1,
    pub full_grid_wave_coverage: TruthOriginV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodedAttSourceCorrelationV1 {
    UnavailableNoAdmittedExactArtifactCharacteristicRelation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticDecodedAttV1 {
    pub schema_version: u16,
    pub source_kind: DecodedAttSourceKindV1,
    pub export_source: ContentIdentityRecordV1,
    pub att_bundle: ContentIdentityRecordV1,
    pub att_manifest: ContentIdentityRecordV1,
    pub decoder: DecodedAttDecoderIdentityV1,
    pub raw_decode_relation: DecodedAttRawDecodeRelationV1,
    #[serde(deserialize_with = "deserialize_raw_references")]
    pub raw_references: Vec<DecodedAttRawReferenceV1>,
    #[serde(deserialize_with = "deserialize_decoded_wave_references")]
    pub decoded_wave_references: Vec<u32>,
    #[serde(deserialize_with = "deserialize_callback_summaries")]
    pub callbacks: Vec<DecodedAttCallbackSummaryV1>,
    #[serde(deserialize_with = "deserialize_code_objects")]
    pub code_objects: Vec<DecodedAttCodeObjectV1>,
    pub gfxip_major: u64,
    #[serde(deserialize_with = "deserialize_occupancy")]
    pub occupancy: Vec<DecodedAttOccupancyV1>,
    #[serde(deserialize_with = "deserialize_perf_events")]
    pub perf_events: Vec<DecodedAttPerfEventV1>,
    #[serde(deserialize_with = "deserialize_waves")]
    pub waves: Vec<DecodedAttWaveV1>,
    #[serde(deserialize_with = "deserialize_shader_data")]
    pub shader_data: Vec<DecodedAttShaderDataV1>,
    #[serde(deserialize_with = "deserialize_realtime")]
    pub realtime: Vec<DecodedAttRealtimeV1>,
    pub realtime_frequency_hz: Option<u64>,
    #[serde(deserialize_with = "deserialize_info")]
    pub info: Vec<DecodedAttInfoV1>,
    pub coverage: DecodedAttCoverageV1,
    pub source_correlation: DecodedAttSourceCorrelationV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedAttImportLimitsV1 {
    pub max_export_bytes: u64,
    pub max_interchange_bytes: u64,
}

impl Default for DecodedAttImportLimitsV1 {
    fn default() -> Self {
        Self {
            max_export_bytes: MAX_DECODED_ATT_EXPORT_BYTES_V1,
            max_interchange_bytes: MAX_DECODED_ATT_INTERCHANGE_BYTES_V1,
        }
    }
}

impl DecodedAttImportLimitsV1 {
    pub fn new(export: u64, interchange: u64) -> Result<Self, DecodedAttErrorV1> {
        if export == 0
            || export > MAX_DECODED_ATT_EXPORT_BYTES_V1
            || interchange == 0
            || interchange > MAX_DECODED_ATT_INTERCHANGE_BYTES_V1
        {
            return Err(DecodedAttErrorV1::LimitOutOfRange);
        }
        Ok(Self {
            max_export_bytes: export,
            max_interchange_bytes: interchange,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedAttImportBindingV1 {
    pub trace_decoder_types_header: ContentIdentityRecordV1,
    pub trace_decoder_api_header: ContentIdentityRecordV1,
    pub decoder_library: ContentIdentityRecordV1,
    pub exporter_tool: ContentIdentityRecordV1,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawExportV1 {
    schema_version: u16,
    source_kind: DecodedAttSourceKindV1,
    #[serde(deserialize_with = "deserialize_raw_export_references")]
    raw_references: Vec<RawReferenceV1>,
    #[serde(deserialize_with = "deserialize_raw_code_objects")]
    code_objects: Vec<RawCodeObjectV1>,
    #[serde(deserialize_with = "deserialize_raw_callbacks")]
    callbacks: Vec<RawCallbackV1>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawReferenceV1 {
    ordinal: u32,
    kind: AttReferenceKindV4,
    #[serde(deserialize_with = "deserialize_safe_reference")]
    reference: String,
    content: Option<ContentIdentityRecordV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawCodeObjectV1 {
    load_id: u64,
    load_address: u64,
    load_size: u64,
    artifact: ContentIdentityRecordV1,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "record_type", rename_all = "snake_case", deny_unknown_fields)]
enum RawCallbackV1 {
    Gfxip {
        source_reference_ordinal: u32,
        #[serde(deserialize_with = "deserialize_gfxip_records")]
        records: Vec<u64>,
    },
    Occupancy {
        source_reference_ordinal: u32,
        #[serde(deserialize_with = "deserialize_raw_occupancy")]
        records: Vec<RawOccupancyV1>,
    },
    Perfevent {
        source_reference_ordinal: u32,
        #[serde(deserialize_with = "deserialize_raw_perf_events")]
        records: Vec<RawPerfEventV1>,
    },
    Wave {
        source_reference_ordinal: u32,
        #[serde(deserialize_with = "deserialize_raw_waves")]
        records: Vec<RawWaveV1>,
    },
    Info {
        source_reference_ordinal: u32,
        #[serde(deserialize_with = "deserialize_info_kinds")]
        records: Vec<DecodedAttInfoKindV1>,
    },
    Shaderdata {
        source_reference_ordinal: u32,
        #[serde(deserialize_with = "deserialize_raw_shader_data")]
        records: Vec<RawShaderDataV1>,
    },
    Realtime {
        source_reference_ordinal: u32,
        #[serde(deserialize_with = "deserialize_raw_realtime")]
        records: Vec<RawRealtimeV1>,
    },
    RtFrequency {
        source_reference_ordinal: u32,
        #[serde(deserialize_with = "deserialize_frequency_records")]
        records: Vec<u64>,
    },
}

impl RawCallbackV1 {
    fn len(&self) -> usize {
        match self {
            Self::Gfxip { records, .. } => records.len(),
            Self::Occupancy { records, .. } => records.len(),
            Self::Perfevent { records, .. } => records.len(),
            Self::Wave { records, .. } => records.len(),
            Self::Info { records, .. } => records.len(),
            Self::Shaderdata { records, .. } => records.len(),
            Self::Realtime { records, .. } => records.len(),
            Self::RtFrequency { records, .. } => records.len(),
        }
    }

    const fn source_reference_ordinal(&self) -> u32 {
        match self {
            Self::Gfxip {
                source_reference_ordinal,
                ..
            }
            | Self::Occupancy {
                source_reference_ordinal,
                ..
            }
            | Self::Perfevent {
                source_reference_ordinal,
                ..
            }
            | Self::Wave {
                source_reference_ordinal,
                ..
            }
            | Self::Info {
                source_reference_ordinal,
                ..
            }
            | Self::Shaderdata {
                source_reference_ordinal,
                ..
            }
            | Self::Realtime {
                source_reference_ordinal,
                ..
            }
            | Self::RtFrequency {
                source_reference_ordinal,
                ..
            } => *source_reference_ordinal,
        }
    }

    const fn kind(&self) -> DecodedAttCallbackKindV1 {
        match self {
            Self::Gfxip { .. } => DecodedAttCallbackKindV1::Gfxip,
            Self::Occupancy { .. } => DecodedAttCallbackKindV1::Occupancy,
            Self::Perfevent { .. } => DecodedAttCallbackKindV1::Perfevent,
            Self::Wave { .. } => DecodedAttCallbackKindV1::Wave,
            Self::Info { .. } => DecodedAttCallbackKindV1::Info,
            Self::Shaderdata { .. } => DecodedAttCallbackKindV1::Shaderdata,
            Self::Realtime { .. } => DecodedAttCallbackKindV1::Realtime,
            Self::RtFrequency { .. } => DecodedAttCallbackKindV1::RtFrequency,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawPcV1 {
    address: u64,
    code_object_id: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawOccupancyV1 {
    pc: RawPcV1,
    time: u64,
    reserved: u8,
    cu: u8,
    simd: u8,
    wave_id: u8,
    start: bool,
    start_reserved: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawPerfEventV1 {
    time: i64,
    events0: u16,
    events1: u16,
    events2: u16,
    events3: u16,
    cu: u8,
    bank: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawWaveStateV1 {
    state: DecodedAttWaveStateKindV1,
    duration: i32,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawInstructionV1 {
    category: DecodedAttInstructionCategoryV1,
    stall: u32,
    duration: i32,
    time: i64,
    pc: RawPcV1,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawWaveV1 {
    cu: u8,
    simd: u8,
    wave_id: u8,
    contexts: u8,
    reserved: [u32; 3],
    begin_time: i64,
    end_time: i64,
    #[serde(deserialize_with = "deserialize_raw_wave_states")]
    timeline: Vec<RawWaveStateV1>,
    #[serde(deserialize_with = "deserialize_raw_instructions")]
    instructions: Vec<RawInstructionV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawShaderDataV1 {
    time: i64,
    value: u64,
    cu: u8,
    simd: u8,
    wave_id: u8,
    flags: u8,
    reserved: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawRealtimeV1 {
    shader_clock: i64,
    realtime_clock: u64,
    reserved: u64,
}

pub fn import_rocprofiler_sdk_decoded_att_v1(
    export_source: &[u8],
    att_bundle_bytes: &[u8],
    binding: DecodedAttImportBindingV1,
    limits: DecodedAttImportLimitsV1,
) -> Result<SemanticDecodedAttV1, DecodedAttErrorV1> {
    bounded_input(export_source, limits.max_export_bytes)?;
    validate_decoder_binding(binding)?;
    let raw: RawExportV1 =
        serde_json::from_slice(export_source).map_err(|_| DecodedAttErrorV1::JsonDecode)?;
    let canonical =
        encode_json_bounded(&raw, limits.max_export_bytes).map_err(|error| match error {
            BoundedJsonEncodeErrorV1::TooLarge => DecodedAttErrorV1::InputTooLarge,
            BoundedJsonEncodeErrorV1::Allocation => DecodedAttErrorV1::AllocationFailure,
            BoundedJsonEncodeErrorV1::Json => DecodedAttErrorV1::JsonEncode,
        })?;
    if canonical != export_source {
        return Err(DecodedAttErrorV1::NonCanonicalExport);
    }
    if raw.schema_version != DECODED_ATT_SCHEMA_VERSION_V1
        || raw.source_kind != DecodedAttSourceKindV1::RocprofilerSdk724TraceDecoderCallbacks
    {
        return Err(DecodedAttErrorV1::UnsupportedExportSchema);
    }
    let bundle = decode_profiler_bundle_v4(att_bundle_bytes)
        .map_err(|_| DecodedAttErrorV1::InvalidAttBundle)?;
    if bundle.source_kind != ProfilerSourceKindV4::Rocprofv3AttComputeViewerManifest {
        return Err(DecodedAttErrorV1::InvalidAttBundle);
    }
    let att = bundle
        .att
        .as_ref()
        .ok_or(DecodedAttErrorV1::InvalidAttBundle)?;
    let att_bundle = profiler_bundle_content_identity_v4(att_bundle_bytes)
        .map_err(|_| DecodedAttErrorV1::InvalidAttBundle)?;
    let att_manifest = att
        .manifest
        .value
        .ok_or(DecodedAttErrorV1::InvalidAttBundle)?;
    let export_source_identity = domain_identity(
        DECODED_ATT_EXPORT_SOURCE_IDENTITY_DOMAIN_V1,
        DECODED_ATT_SCHEMA_VERSION_V1,
        export_source,
    )?;

    let (raw_references, raw_decode_relation) =
        bind_raw_references(&raw.raw_references, &att.references)?;
    let code_object_binding = bind_code_objects(&raw.code_objects, export_source_identity)?;
    let mut output = SemanticDecodedAttV1 {
        schema_version: DECODED_ATT_SCHEMA_VERSION_V1,
        source_kind: raw.source_kind,
        export_source: export_source_identity,
        att_bundle,
        att_manifest,
        decoder: DecodedAttDecoderIdentityV1 {
            trace_decoder_types_header: binding.trace_decoder_types_header,
            trace_decoder_api_header: binding.trace_decoder_api_header,
            decoder_library: binding.decoder_library,
            exporter_tool: binding.exporter_tool,
            identity_origin: TruthOriginV1::Declared,
            authenticity: DecodedAttAuthenticityV1::UnavailableSelfClaimedExternalDecoder,
        },
        raw_decode_relation,
        raw_references,
        decoded_wave_references: Vec::new(),
        callbacks: Vec::new(),
        code_objects: code_object_binding.output,
        gfxip_major: 0,
        occupancy: Vec::new(),
        perf_events: Vec::new(),
        waves: Vec::new(),
        shader_data: Vec::new(),
        realtime: Vec::new(),
        realtime_frequency_hz: None,
        info: Vec::new(),
        coverage: empty_coverage(),
        source_correlation:
            DecodedAttSourceCorrelationV1::UnavailableNoAdmittedExactArtifactCharacteristicRelation,
    };
    normalize_callbacks(
        raw.callbacks,
        &code_object_binding.raw_to_identity,
        export_source_identity,
        &mut output,
    )?;
    output.coverage = derive_coverage(&output)?;
    output.validate()?;
    let encoded = encode_decoded_att_v1(&output)?;
    if u64::try_from(encoded.len()).map_err(|_| DecodedAttErrorV1::SizeOverflow)?
        > limits.max_interchange_bytes
    {
        return Err(DecodedAttErrorV1::InterchangeTooLarge);
    }
    Ok(output)
}

pub fn readmit_rocprofiler_sdk_decoded_att_v1(
    export_source: &[u8],
    att_bundle_bytes: &[u8],
    interchange_bytes: &[u8],
    binding: DecodedAttImportBindingV1,
    limits: DecodedAttImportLimitsV1,
) -> Result<SemanticDecodedAttV1, DecodedAttErrorV1> {
    decode_decoded_att_v1(interchange_bytes)?;
    let imported =
        import_rocprofiler_sdk_decoded_att_v1(export_source, att_bundle_bytes, binding, limits)?;
    if encode_decoded_att_v1(&imported)? != interchange_bytes {
        return Err(DecodedAttErrorV1::StaleInterchange);
    }
    Ok(imported)
}

impl SemanticDecodedAttV1 {
    pub fn validate(&self) -> Result<(), DecodedAttErrorV1> {
        if self.schema_version != DECODED_ATT_SCHEMA_VERSION_V1
            || self.source_kind != DecodedAttSourceKindV1::RocprofilerSdk724TraceDecoderCallbacks
            || self.gfxip_major == 0
        {
            return Err(DecodedAttErrorV1::InvalidInterchange);
        }
        validate_content_identity(self.export_source)?;
        validate_content_identity(self.att_bundle)?;
        validate_content_identity(self.att_manifest)?;
        validate_decoder_identity(self.decoder)?;
        validate_reference_catalog(
            &self.raw_references,
            &self.decoded_wave_references,
            self.raw_decode_relation,
        )?;
        validate_callback_summaries(&self.callbacks, &self.decoded_wave_references)?;
        validate_record_sequence(
            &self.callbacks,
            DecodedAttCallbackKindV1::Occupancy,
            self.occupancy.iter().map(|value| {
                (
                    value.source_callback_ordinal,
                    value.source_record_ordinal,
                    value.source_reference_ordinal,
                )
            }),
        )?;
        validate_record_sequence(
            &self.callbacks,
            DecodedAttCallbackKindV1::Perfevent,
            self.perf_events.iter().map(|value| {
                (
                    value.source_callback_ordinal,
                    value.source_record_ordinal,
                    value.source_reference_ordinal,
                )
            }),
        )?;
        validate_record_sequence(
            &self.callbacks,
            DecodedAttCallbackKindV1::Wave,
            self.waves.iter().map(|value| {
                (
                    value.source_callback_ordinal,
                    value.source_record_ordinal,
                    value.source_reference_ordinal,
                )
            }),
        )?;
        validate_record_sequence(
            &self.callbacks,
            DecodedAttCallbackKindV1::Shaderdata,
            self.shader_data.iter().map(|value| {
                (
                    value.source_callback_ordinal,
                    value.source_record_ordinal,
                    value.source_reference_ordinal,
                )
            }),
        )?;
        validate_record_sequence(
            &self.callbacks,
            DecodedAttCallbackKindV1::Realtime,
            self.realtime.iter().map(|value| {
                (
                    value.source_callback_ordinal,
                    value.source_record_ordinal,
                    value.source_reference_ordinal,
                )
            }),
        )?;
        validate_record_sequence(
            &self.callbacks,
            DecodedAttCallbackKindV1::Info,
            self.info.iter().map(|value| {
                (
                    value.source_callback_ordinal,
                    value.source_record_ordinal,
                    value.source_reference_ordinal,
                )
            }),
        )?;
        if self.realtime_frequency_hz.is_some()
            != self
                .callbacks
                .iter()
                .any(|value| value.kind == DecodedAttCallbackKindV1::RtFrequency)
        {
            return Err(DecodedAttErrorV1::InvalidRealtimeFrequency);
        }
        validate_output_code_objects(&self.code_objects)?;
        if self.occupancy.len() > MAX_DECODED_ATT_RECORDS_V1
            || self.perf_events.len() > MAX_DECODED_ATT_RECORDS_V1
            || self.waves.len() > MAX_DECODED_ATT_WAVES_V1
            || self.shader_data.len() > MAX_DECODED_ATT_RECORDS_V1
            || self.realtime.len() > MAX_DECODED_ATT_RECORDS_V1
            || self.info.len() > MAX_DECODED_ATT_RECORDS_V1
            || self.realtime_frequency_hz == Some(0)
        {
            return Err(DecodedAttErrorV1::RecordLimitExceeded);
        }
        for record in &self.occupancy {
            validate_record_source(
                record.source_reference_ordinal,
                &self.decoded_wave_references,
            )?;
            validate_callback_record(
                &self.callbacks,
                DecodedAttCallbackKindV1::Occupancy,
                record.source_callback_ordinal,
                record.source_record_ordinal,
                record.source_reference_ordinal,
            )?;
            validate_record_identity(
                self.export_source,
                b"occupancy",
                record.source_callback_ordinal,
                record.source_record_ordinal,
                record.source_reference_ordinal,
                record.identity,
            )?;
            validate_pc(record.pc, &self.code_objects)?;
            if record.origin != DecodedAttRecordOriginV1::ExternalDecoderDeclared || record.simd > 3
            {
                return Err(DecodedAttErrorV1::InvalidOccupancy);
            }
        }
        for record in &self.perf_events {
            validate_record_source(
                record.source_reference_ordinal,
                &self.decoded_wave_references,
            )?;
            validate_callback_record(
                &self.callbacks,
                DecodedAttCallbackKindV1::Perfevent,
                record.source_callback_ordinal,
                record.source_record_ordinal,
                record.source_reference_ordinal,
            )?;
            validate_record_identity(
                self.export_source,
                b"perfevent",
                record.source_callback_ordinal,
                record.source_record_ordinal,
                record.source_reference_ordinal,
                record.identity,
            )?;
            if record.origin != DecodedAttRecordOriginV1::ExternalDecoderDeclared || record.bank > 1
            {
                return Err(DecodedAttErrorV1::InvalidPerfEvent);
            }
        }
        let mut states = 0_usize;
        let mut instructions = 0_usize;
        for wave in &self.waves {
            validate_record_source(wave.source_reference_ordinal, &self.decoded_wave_references)?;
            validate_callback_record(
                &self.callbacks,
                DecodedAttCallbackKindV1::Wave,
                wave.source_callback_ordinal,
                wave.source_record_ordinal,
                wave.source_reference_ordinal,
            )?;
            validate_record_identity(
                self.export_source,
                b"wave",
                wave.source_callback_ordinal,
                wave.source_record_ordinal,
                wave.source_reference_ordinal,
                wave.identity,
            )?;
            if wave.origin != DecodedAttRecordOriginV1::ExternalDecoderDeclared
                || wave.simd > 3
                || wave.begin_time > wave.end_time
            {
                return Err(DecodedAttErrorV1::InvalidWave);
            }
            states = states
                .checked_add(wave.timeline.len())
                .ok_or(DecodedAttErrorV1::SizeOverflow)?;
            instructions = instructions
                .checked_add(wave.instructions.len())
                .ok_or(DecodedAttErrorV1::SizeOverflow)?;
            validate_wave_children(self.export_source, wave, &self.code_objects)?;
        }
        if states > MAX_DECODED_ATT_WAVE_STATES_V1 || instructions > MAX_DECODED_ATT_INSTRUCTIONS_V1
        {
            return Err(DecodedAttErrorV1::RecordLimitExceeded);
        }
        for record in &self.shader_data {
            validate_record_source(
                record.source_reference_ordinal,
                &self.decoded_wave_references,
            )?;
            validate_callback_record(
                &self.callbacks,
                DecodedAttCallbackKindV1::Shaderdata,
                record.source_callback_ordinal,
                record.source_record_ordinal,
                record.source_reference_ordinal,
            )?;
            validate_record_identity(
                self.export_source,
                b"shaderdata",
                record.source_callback_ordinal,
                record.source_record_ordinal,
                record.source_reference_ordinal,
                record.identity,
            )?;
            if record.origin != DecodedAttRecordOriginV1::ExternalDecoderDeclared || record.simd > 3
            {
                return Err(DecodedAttErrorV1::InvalidShaderData);
            }
        }
        for record in &self.realtime {
            validate_record_source(
                record.source_reference_ordinal,
                &self.decoded_wave_references,
            )?;
            validate_callback_record(
                &self.callbacks,
                DecodedAttCallbackKindV1::Realtime,
                record.source_callback_ordinal,
                record.source_record_ordinal,
                record.source_reference_ordinal,
            )?;
            validate_record_identity(
                self.export_source,
                b"realtime",
                record.source_callback_ordinal,
                record.source_record_ordinal,
                record.source_reference_ordinal,
                record.identity,
            )?;
            if record.origin != DecodedAttRecordOriginV1::ExternalDecoderDeclared {
                return Err(DecodedAttErrorV1::InvalidRealtime);
            }
        }
        for record in &self.info {
            validate_record_source(
                record.source_reference_ordinal,
                &self.decoded_wave_references,
            )?;
            validate_callback_record(
                &self.callbacks,
                DecodedAttCallbackKindV1::Info,
                record.source_callback_ordinal,
                record.source_record_ordinal,
                record.source_reference_ordinal,
            )?;
            validate_record_identity(
                self.export_source,
                b"info",
                record.source_callback_ordinal,
                record.source_record_ordinal,
                record.source_reference_ordinal,
                record.identity,
            )?;
            if record.origin != DecodedAttRecordOriginV1::ExternalDecoderDeclared {
                return Err(DecodedAttErrorV1::InvalidInfo);
            }
        }
        if self.coverage != derive_coverage(self)?
            || self.source_correlation
                != DecodedAttSourceCorrelationV1::UnavailableNoAdmittedExactArtifactCharacteristicRelation
        {
            return Err(DecodedAttErrorV1::InvalidCoverage);
        }
        Ok(())
    }
}

pub fn encode_decoded_att_v1(value: &SemanticDecodedAttV1) -> Result<Vec<u8>, DecodedAttErrorV1> {
    value.validate()?;
    encode_json_bounded(value, MAX_DECODED_ATT_INTERCHANGE_BYTES_V1).map_err(|error| match error {
        BoundedJsonEncodeErrorV1::TooLarge => DecodedAttErrorV1::InterchangeTooLarge,
        BoundedJsonEncodeErrorV1::Allocation => DecodedAttErrorV1::AllocationFailure,
        BoundedJsonEncodeErrorV1::Json => DecodedAttErrorV1::JsonEncode,
    })
}

pub fn decode_decoded_att_v1(bytes: &[u8]) -> Result<SemanticDecodedAttV1, DecodedAttErrorV1> {
    bounded_input(bytes, MAX_DECODED_ATT_INTERCHANGE_BYTES_V1)?;
    let value: SemanticDecodedAttV1 =
        serde_json::from_slice(bytes).map_err(|_| DecodedAttErrorV1::JsonDecode)?;
    value.validate()?;
    if encode_decoded_att_v1(&value)? != bytes {
        return Err(DecodedAttErrorV1::NonCanonicalInterchange);
    }
    Ok(value)
}

pub fn decoded_att_content_identity_v1(
    bytes: &[u8],
) -> Result<ContentIdentityRecordV1, DecodedAttErrorV1> {
    let _ = decode_decoded_att_v1(bytes)?;
    domain_identity(
        DECODED_ATT_INTERCHANGE_IDENTITY_DOMAIN_V1,
        DECODED_ATT_SCHEMA_VERSION_V1,
        bytes,
    )
}

fn validate_decoder_binding(binding: DecodedAttImportBindingV1) -> Result<(), DecodedAttErrorV1> {
    require_exact_header(
        binding.trace_decoder_types_header,
        ROCPROFILER_SDK_7_2_4_TRACE_DECODER_TYPES_HEADER_SHA256_V1,
        ROCPROFILER_SDK_7_2_4_TRACE_DECODER_TYPES_HEADER_BYTES_V1,
    )?;
    require_exact_header(
        binding.trace_decoder_api_header,
        ROCPROFILER_SDK_7_2_4_TRACE_DECODER_API_HEADER_SHA256_V1,
        ROCPROFILER_SDK_7_2_4_TRACE_DECODER_API_HEADER_BYTES_V1,
    )?;
    validate_content_identity(binding.decoder_library)?;
    validate_content_identity(binding.exporter_tool)
}

fn validate_decoder_identity(value: DecodedAttDecoderIdentityV1) -> Result<(), DecodedAttErrorV1> {
    validate_decoder_binding(DecodedAttImportBindingV1 {
        trace_decoder_types_header: value.trace_decoder_types_header,
        trace_decoder_api_header: value.trace_decoder_api_header,
        decoder_library: value.decoder_library,
        exporter_tool: value.exporter_tool,
    })?;
    if value.identity_origin != TruthOriginV1::Declared
        || value.authenticity != DecodedAttAuthenticityV1::UnavailableSelfClaimedExternalDecoder
    {
        return Err(DecodedAttErrorV1::InvalidDecoderIdentity);
    }
    Ok(())
}

fn require_exact_header(
    value: ContentIdentityRecordV1,
    digest: [u8; 32],
    len: u64,
) -> Result<(), DecodedAttErrorV1> {
    if value.scheme != ContentSchemeV1::RawCanonicalSha256
        || value.format_version != 1
        || value.digest.as_bytes() != digest
        || value.canonical_len != len
    {
        return Err(DecodedAttErrorV1::UnsupportedDecoderAbi);
    }
    Ok(())
}

fn bind_raw_references(
    raw: &[RawReferenceV1],
    expected: &[crate::AttArtifactReferenceV4],
) -> Result<(Vec<DecodedAttRawReferenceV1>, DecodedAttRawDecodeRelationV1), DecodedAttErrorV1> {
    if raw.len() != expected.len() || raw.is_empty() {
        return Err(DecodedAttErrorV1::RawReferenceMismatch);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(raw.len())
        .map_err(|_| DecodedAttErrorV1::AllocationFailure)?;
    let mut complete = true;
    for (source, target) in raw.iter().zip(expected) {
        if source.ordinal != target.ordinal
            || source.kind != target.kind
            || source.reference != target.reference
            || source.content != target.content.value
        {
            return Err(DecodedAttErrorV1::RawReferenceMismatch);
        }
        let content = match target.content.value {
            Some(content) => {
                validate_content_identity(content)?;
                IdentityFactV1::declared(content)
            }
            None => {
                complete = false;
                IdentityFactV1::unavailable(CaptureUnavailableReasonV1::NotProvided)
            }
        };
        output.push(DecodedAttRawReferenceV1 {
            ordinal: target.ordinal,
            kind: target.kind,
            reference: copy_string(&target.reference)?,
            content,
        });
    }
    Ok((
        output,
        DecodedAttRawDecodeRelationV1 {
            origin: if complete {
                TruthOriginV1::Declared
            } else {
                TruthOriginV1::Unavailable
            },
            state: if complete {
                DecodedAttRawRelationV1::ExternalDecoderDeclaredCompleteExactWaveInputs
            } else {
                DecodedAttRawRelationV1::UnavailableMissingExactRawReferenceIdentity
            },
        },
    ))
}

struct BoundCodeObjectsV1 {
    output: Vec<DecodedAttCodeObjectV1>,
    raw_to_identity: Vec<(u64, CaptureIdentityV1)>,
}

fn bind_code_objects(
    raw: &[RawCodeObjectV1],
    export: ContentIdentityRecordV1,
) -> Result<BoundCodeObjectsV1, DecodedAttErrorV1> {
    if raw.len() > MAX_DECODED_ATT_CODE_OBJECTS_V1 {
        return Err(DecodedAttErrorV1::CodeObjectLimitExceeded);
    }
    let mut output = Vec::new();
    let mut mapping = Vec::new();
    output
        .try_reserve_exact(raw.len())
        .map_err(|_| DecodedAttErrorV1::AllocationFailure)?;
    mapping
        .try_reserve_exact(raw.len())
        .map_err(|_| DecodedAttErrorV1::AllocationFailure)?;
    let mut prior_load_id = None;
    for value in raw {
        validate_content_identity(value.artifact)?;
        if value.load_id == 0
            || value.load_size == 0
            || value.load_address.checked_add(value.load_size).is_none()
            || prior_load_id.is_some_and(|prior| prior >= value.load_id)
        {
            return Err(DecodedAttErrorV1::InvalidCodeObject);
        }
        let selector = derive_selector(export, value.load_id)?;
        let identity = derive_code_object_identity(selector, value.artifact, value.load_size)?;
        prior_load_id = Some(value.load_id);
        mapping.push((value.load_id, identity));
        output.push(DecodedAttCodeObjectV1 {
            identity,
            selector,
            origin: DecodedAttRecordOriginV1::ExternalDecoderDeclared,
            artifact: value.artifact,
            load_size: value.load_size,
        });
    }
    output.sort_by_key(|value| value.identity);
    Ok(BoundCodeObjectsV1 {
        output,
        raw_to_identity: mapping,
    })
}

fn normalize_callbacks(
    callbacks: Vec<RawCallbackV1>,
    code_objects: &[(u64, CaptureIdentityV1)],
    export: ContentIdentityRecordV1,
    output: &mut SemanticDecodedAttV1,
) -> Result<(), DecodedAttErrorV1> {
    if callbacks.is_empty() || callbacks.len() > MAX_DECODED_ATT_CALLBACKS_V1 {
        return Err(DecodedAttErrorV1::CallbackLimitExceeded);
    }
    let callback_count =
        u64::try_from(callbacks.len()).map_err(|_| DecodedAttErrorV1::SizeOverflow)?;
    let mut total = 0_usize;
    let mut decoded_references = Vec::new();
    decoded_references
        .try_reserve_exact(output.raw_references.len())
        .map_err(|_| DecodedAttErrorV1::AllocationFailure)?;
    let mut active_reference = None;
    let mut frequency_seen_for_reference = false;
    let mut wave_states = 0_usize;
    let mut instructions = 0_usize;
    output
        .callbacks
        .try_reserve_exact(callbacks.len())
        .map_err(|_| DecodedAttErrorV1::AllocationFailure)?;
    for (callback_ordinal, callback) in callbacks.into_iter().enumerate() {
        if callback.len() == 0 {
            return Err(DecodedAttErrorV1::EmptyCallback);
        }
        total = total
            .checked_add(callback.len())
            .ok_or(DecodedAttErrorV1::SizeOverflow)?;
        if total > MAX_DECODED_ATT_RECORDS_V1 {
            return Err(DecodedAttErrorV1::RecordLimitExceeded);
        }
        let source_reference_ordinal = callback.source_reference_ordinal();
        let reference_index = usize::try_from(source_reference_ordinal)
            .map_err(|_| DecodedAttErrorV1::SizeOverflow)?;
        let reference = output
            .raw_references
            .get(reference_index)
            .filter(|value| {
                value.ordinal == source_reference_ordinal
                    && value.kind == AttReferenceKindV4::WaveTimeline
            })
            .ok_or(DecodedAttErrorV1::RawReferenceMismatch)?;
        let first_for_reference = active_reference != Some(reference.ordinal);
        if first_for_reference {
            if active_reference.is_some_and(|prior| prior >= reference.ordinal) {
                return Err(DecodedAttErrorV1::RawReferenceMismatch);
            }
            decoded_references.push(reference.ordinal);
            active_reference = Some(reference.ordinal);
            frequency_seen_for_reference = false;
        }
        let callback_ordinal =
            u64::try_from(callback_ordinal).map_err(|_| DecodedAttErrorV1::SizeOverflow)?;
        output.callbacks.push(DecodedAttCallbackSummaryV1 {
            ordinal: callback_ordinal,
            source_reference_ordinal,
            kind: callback.kind(),
            record_count: u64::try_from(callback.len())
                .map_err(|_| DecodedAttErrorV1::SizeOverflow)?,
        });
        match callback {
            RawCallbackV1::Gfxip { records, .. } => {
                if !first_for_reference
                    || records.len() != 1
                    || records[0] == 0
                    || (output.gfxip_major != 0 && output.gfxip_major != records[0])
                {
                    return Err(DecodedAttErrorV1::InvalidGfxip);
                }
                output.gfxip_major = records[0];
            }
            RawCallbackV1::Occupancy { records, .. } => {
                if first_for_reference {
                    return Err(DecodedAttErrorV1::InvalidGfxip);
                }
                reserve(&mut output.occupancy, records.len())?;
                for (record_ordinal, value) in records.into_iter().enumerate() {
                    if value.reserved != 0 || value.start_reserved != 0 || value.simd > 3 {
                        return Err(DecodedAttErrorV1::InvalidOccupancy);
                    }
                    let record_ordinal = u64::try_from(record_ordinal)
                        .map_err(|_| DecodedAttErrorV1::SizeOverflow)?;
                    output.occupancy.push(DecodedAttOccupancyV1 {
                        identity: derive_record_identity(
                            export,
                            b"occupancy",
                            callback_ordinal,
                            record_ordinal,
                            source_reference_ordinal,
                        )?,
                        origin: DecodedAttRecordOriginV1::ExternalDecoderDeclared,
                        source_callback_ordinal: callback_ordinal,
                        source_record_ordinal: record_ordinal,
                        source_reference_ordinal,
                        pc: normalize_pc(value.pc, code_objects)?,
                        time: value.time,
                        cu_or_wgp: value.cu,
                        simd: value.simd,
                        wave_slot: value.wave_id,
                        started: value.start,
                    });
                }
            }
            RawCallbackV1::Perfevent { records, .. } => {
                if first_for_reference {
                    return Err(DecodedAttErrorV1::InvalidGfxip);
                }
                reserve(&mut output.perf_events, records.len())?;
                for (record_ordinal, value) in records.into_iter().enumerate() {
                    if value.bank > 1 {
                        return Err(DecodedAttErrorV1::InvalidPerfEvent);
                    }
                    let record_ordinal = u64::try_from(record_ordinal)
                        .map_err(|_| DecodedAttErrorV1::SizeOverflow)?;
                    output.perf_events.push(DecodedAttPerfEventV1 {
                        identity: derive_record_identity(
                            export,
                            b"perfevent",
                            callback_ordinal,
                            record_ordinal,
                            source_reference_ordinal,
                        )?,
                        origin: DecodedAttRecordOriginV1::ExternalDecoderDeclared,
                        source_callback_ordinal: callback_ordinal,
                        source_record_ordinal: record_ordinal,
                        source_reference_ordinal,
                        time: value.time,
                        events: [value.events0, value.events1, value.events2, value.events3],
                        cu_or_wgp: value.cu,
                        bank: value.bank,
                    });
                }
            }
            RawCallbackV1::Wave { records, .. } => {
                if first_for_reference {
                    return Err(DecodedAttErrorV1::InvalidGfxip);
                }
                reserve(&mut output.waves, records.len())?;
                for (record_ordinal, value) in records.into_iter().enumerate() {
                    wave_states = wave_states
                        .checked_add(value.timeline.len())
                        .ok_or(DecodedAttErrorV1::SizeOverflow)?;
                    instructions = instructions
                        .checked_add(value.instructions.len())
                        .ok_or(DecodedAttErrorV1::SizeOverflow)?;
                    if wave_states > MAX_DECODED_ATT_WAVE_STATES_V1
                        || instructions > MAX_DECODED_ATT_INSTRUCTIONS_V1
                    {
                        return Err(DecodedAttErrorV1::RecordLimitExceeded);
                    }
                    output.waves.push(normalize_wave(
                        value,
                        export,
                        code_objects,
                        callback_ordinal,
                        u64::try_from(record_ordinal)
                            .map_err(|_| DecodedAttErrorV1::SizeOverflow)?,
                        source_reference_ordinal,
                    )?);
                }
            }
            RawCallbackV1::Info { records, .. } => {
                if first_for_reference {
                    return Err(DecodedAttErrorV1::InvalidGfxip);
                }
                reserve(&mut output.info, records.len())?;
                for (record_ordinal, kind) in records.into_iter().enumerate() {
                    let record_ordinal = u64::try_from(record_ordinal)
                        .map_err(|_| DecodedAttErrorV1::SizeOverflow)?;
                    output.info.push(DecodedAttInfoV1 {
                        identity: derive_record_identity(
                            export,
                            b"info",
                            callback_ordinal,
                            record_ordinal,
                            source_reference_ordinal,
                        )?,
                        origin: DecodedAttRecordOriginV1::ExternalDecoderDeclared,
                        source_callback_ordinal: callback_ordinal,
                        source_record_ordinal: record_ordinal,
                        source_reference_ordinal,
                        kind,
                    });
                }
            }
            RawCallbackV1::Shaderdata { records, .. } => {
                if first_for_reference {
                    return Err(DecodedAttErrorV1::InvalidGfxip);
                }
                reserve(&mut output.shader_data, records.len())?;
                for (record_ordinal, value) in records.into_iter().enumerate() {
                    if value.flags > 1 || value.reserved != 0 || value.simd > 3 {
                        return Err(DecodedAttErrorV1::InvalidShaderData);
                    }
                    let record_ordinal = u64::try_from(record_ordinal)
                        .map_err(|_| DecodedAttErrorV1::SizeOverflow)?;
                    output.shader_data.push(DecodedAttShaderDataV1 {
                        identity: derive_record_identity(
                            export,
                            b"shaderdata",
                            callback_ordinal,
                            record_ordinal,
                            source_reference_ordinal,
                        )?,
                        origin: DecodedAttRecordOriginV1::ExternalDecoderDeclared,
                        source_callback_ordinal: callback_ordinal,
                        source_record_ordinal: record_ordinal,
                        source_reference_ordinal,
                        time: value.time,
                        value: value.value,
                        cu_or_wgp: value.cu,
                        simd: value.simd,
                        wave_slot: value.wave_id,
                        private_trap_handler: value.flags == 1,
                    });
                }
            }
            RawCallbackV1::Realtime { records, .. } => {
                if first_for_reference {
                    return Err(DecodedAttErrorV1::InvalidGfxip);
                }
                reserve(&mut output.realtime, records.len())?;
                for (record_ordinal, value) in records.into_iter().enumerate() {
                    if value.reserved != 0 {
                        return Err(DecodedAttErrorV1::InvalidRealtime);
                    }
                    let record_ordinal = u64::try_from(record_ordinal)
                        .map_err(|_| DecodedAttErrorV1::SizeOverflow)?;
                    output.realtime.push(DecodedAttRealtimeV1 {
                        identity: derive_record_identity(
                            export,
                            b"realtime",
                            callback_ordinal,
                            record_ordinal,
                            source_reference_ordinal,
                        )?,
                        origin: DecodedAttRecordOriginV1::ExternalDecoderDeclared,
                        source_callback_ordinal: callback_ordinal,
                        source_record_ordinal: record_ordinal,
                        source_reference_ordinal,
                        shader_clock: value.shader_clock,
                        realtime_clock: value.realtime_clock,
                    });
                }
            }
            RawCallbackV1::RtFrequency { records, .. } => {
                if first_for_reference {
                    return Err(DecodedAttErrorV1::InvalidGfxip);
                }
                if frequency_seen_for_reference
                    || records.len() != 1
                    || records[0] == 0
                    || output
                        .realtime_frequency_hz
                        .is_some_and(|frequency| frequency != records[0])
                {
                    return Err(DecodedAttErrorV1::InvalidRealtimeFrequency);
                }
                frequency_seen_for_reference = true;
                output.realtime_frequency_hz = Some(records[0]);
            }
        }
    }
    if decoded_references.is_empty() {
        return Err(DecodedAttErrorV1::InvalidGfxip);
    }
    let wave_reference_count = output
        .raw_references
        .iter()
        .filter(|reference| reference.kind == AttReferenceKindV4::WaveTimeline)
        .count();
    let decoded_reference_count = decoded_references.len();
    output.coverage.callback_count = callback_count;
    output.decoded_wave_references = decoded_references;
    if decoded_reference_count != wave_reference_count
        && output.raw_decode_relation.origin != TruthOriginV1::Unavailable
    {
        output.raw_decode_relation = DecodedAttRawDecodeRelationV1 {
            origin: TruthOriginV1::Unavailable,
            state: DecodedAttRawRelationV1::UnavailableIncompleteWaveReferenceCoverage,
        };
    }
    Ok(())
}

fn normalize_wave(
    raw: RawWaveV1,
    export: ContentIdentityRecordV1,
    code_objects: &[(u64, CaptureIdentityV1)],
    callback: u64,
    record: u64,
    source_reference_ordinal: u32,
) -> Result<DecodedAttWaveV1, DecodedAttErrorV1> {
    if raw.simd > 3 || raw.reserved != [0; 3] || raw.begin_time > raw.end_time {
        return Err(DecodedAttErrorV1::InvalidWave);
    }
    let wave_identity =
        derive_record_identity(export, b"wave", callback, record, source_reference_ordinal)?;
    let mut timeline = Vec::new();
    timeline
        .try_reserve_exact(raw.timeline.len())
        .map_err(|_| DecodedAttErrorV1::AllocationFailure)?;
    for (ordinal, value) in raw.timeline.into_iter().enumerate() {
        if value.duration < 0 {
            return Err(DecodedAttErrorV1::InvalidWaveState);
        }
        let ordinal = u64::try_from(ordinal).map_err(|_| DecodedAttErrorV1::SizeOverflow)?;
        timeline.push(DecodedAttWaveStateV1 {
            identity: derive_child_identity(wave_identity, b"state", ordinal)?,
            origin: DecodedAttRecordOriginV1::ExternalDecoderDeclared,
            ordinal,
            state: value.state,
            duration_cycles: value.duration,
        });
    }
    let mut instructions = Vec::new();
    instructions
        .try_reserve_exact(raw.instructions.len())
        .map_err(|_| DecodedAttErrorV1::AllocationFailure)?;
    let mut prior_time = None;
    for (ordinal, value) in raw.instructions.into_iter().enumerate() {
        if value.stall > 0x00ff_ffff
            || value.duration < 0
            || value.stall > value.duration as u32
            || value.time.checked_add(i64::from(value.duration)).is_none()
            || prior_time.is_some_and(|prior| value.time < prior)
        {
            return Err(DecodedAttErrorV1::InvalidInstruction);
        }
        prior_time = Some(value.time);
        let ordinal = u64::try_from(ordinal).map_err(|_| DecodedAttErrorV1::SizeOverflow)?;
        instructions.push(DecodedAttInstructionV1 {
            identity: derive_child_identity(wave_identity, b"instruction", ordinal)?,
            origin: DecodedAttRecordOriginV1::ExternalDecoderDeclared,
            ordinal,
            category: value.category,
            stall_cycles: value.stall,
            duration_cycles: value.duration,
            time: value.time,
            pc: normalize_pc(value.pc, code_objects)?,
        });
    }
    Ok(DecodedAttWaveV1 {
        identity: wave_identity,
        origin: DecodedAttRecordOriginV1::ExternalDecoderDeclared,
        source_callback_ordinal: callback,
        source_record_ordinal: record,
        source_reference_ordinal,
        cu_or_wgp: raw.cu,
        simd: raw.simd,
        wave_slot: raw.wave_id,
        contexts: raw.contexts,
        begin_time: raw.begin_time,
        end_time: raw.end_time,
        timeline,
        instructions,
    })
}

fn normalize_pc(
    raw: RawPcV1,
    code_objects: &[(u64, CaptureIdentityV1)],
) -> Result<DecodedAttPcV1, DecodedAttErrorV1> {
    if raw.code_object_id == 0 {
        return Ok(DecodedAttPcV1 {
            origin: DecodedAttRecordOriginV1::ExternalDecoderDeclared,
            availability: DecodedAttPcAvailabilityV1::UnavailableNativeVirtualAddressRedacted,
            code_object: None,
            elf_virtual_address: None,
            unavailable_reason: Some(CaptureUnavailableReasonV1::NotRepresented),
        });
    }
    Ok(DecodedAttPcV1 {
        origin: DecodedAttRecordOriginV1::ExternalDecoderDeclared,
        availability: DecodedAttPcAvailabilityV1::ElfVirtualAddress,
        code_object: Some(
            code_objects
                .binary_search_by_key(&raw.code_object_id, |(load_id, _)| *load_id)
                .ok()
                .and_then(|index| code_objects.get(index))
                .map(|(_, identity)| *identity)
                .ok_or(DecodedAttErrorV1::UnknownCodeObject)?,
        ),
        elf_virtual_address: Some(raw.address),
        unavailable_reason: None,
    })
}

fn derive_coverage(
    value: &SemanticDecodedAttV1,
) -> Result<DecodedAttCoverageV1, DecodedAttErrorV1> {
    let count = |kind| usize_to_u64(value.info.iter().filter(|value| value.kind == kind).count());
    let data_lost = count(DecodedAttInfoKindV1::DataLost)?;
    let stitch_incomplete = count(DecodedAttInfoKindV1::StitchIncomplete)?;
    let wave_incomplete = count(DecodedAttInfoKindV1::WaveIncomplete)?;
    let wave_state_count = value.waves.iter().try_fold(0_u64, |total, wave| {
        total.checked_add(usize_to_u64(wave.timeline.len()).ok()?)
    });
    let instruction_count = value.waves.iter().try_fold(0_u64, |total, wave| {
        total.checked_add(usize_to_u64(wave.instructions.len()).ok()?)
    });
    Ok(DecodedAttCoverageV1 {
        origin: DecodedAttRecordOriginV1::ExternalDecoderDeclared,
        completeness: if data_lost + stitch_incomplete + wave_incomplete > 0 {
            DecodedAttCompletenessV1::IncompleteInfoReported
        } else {
            DecodedAttCompletenessV1::UnknownNoPositiveDecoderCompletenessSignal
        },
        loss: if data_lost > 0 {
            DecodedAttLossStateV1::ExternalDecoderReportedDataLost
        } else {
            DecodedAttLossStateV1::Unknown
        },
        callback_count: u64::try_from(value.callbacks.len())
            .map_err(|_| DecodedAttErrorV1::SizeOverflow)?,
        wave_reference_count: u64::try_from(
            value
                .raw_references
                .iter()
                .filter(|reference| reference.kind == AttReferenceKindV4::WaveTimeline)
                .count(),
        )
        .map_err(|_| DecodedAttErrorV1::SizeOverflow)?,
        decoded_wave_reference_count: u64::try_from(value.decoded_wave_references.len())
            .map_err(|_| DecodedAttErrorV1::SizeOverflow)?,
        occupancy_count: usize_to_u64(value.occupancy.len())?,
        perf_event_count: usize_to_u64(value.perf_events.len())?,
        wave_count: usize_to_u64(value.waves.len())?,
        wave_state_count: wave_state_count.ok_or(DecodedAttErrorV1::SizeOverflow)?,
        instruction_count: instruction_count.ok_or(DecodedAttErrorV1::SizeOverflow)?,
        shader_data_count: usize_to_u64(value.shader_data.len())?,
        realtime_count: usize_to_u64(value.realtime.len())?,
        info_count: usize_to_u64(value.info.len())?,
        data_lost_info_count: data_lost,
        stitch_incomplete_info_count: stitch_incomplete,
        wave_incomplete_info_count: wave_incomplete,
        debug_record_support: TruthOriginV1::Unavailable,
        full_grid_wave_coverage: TruthOriginV1::Unavailable,
    })
}

fn empty_coverage() -> DecodedAttCoverageV1 {
    DecodedAttCoverageV1 {
        origin: DecodedAttRecordOriginV1::ExternalDecoderDeclared,
        completeness: DecodedAttCompletenessV1::UnknownNoPositiveDecoderCompletenessSignal,
        loss: DecodedAttLossStateV1::Unknown,
        callback_count: 0,
        wave_reference_count: 0,
        decoded_wave_reference_count: 0,
        occupancy_count: 0,
        perf_event_count: 0,
        wave_count: 0,
        wave_state_count: 0,
        instruction_count: 0,
        shader_data_count: 0,
        realtime_count: 0,
        info_count: 0,
        data_lost_info_count: 0,
        stitch_incomplete_info_count: 0,
        wave_incomplete_info_count: 0,
        debug_record_support: TruthOriginV1::Unavailable,
        full_grid_wave_coverage: TruthOriginV1::Unavailable,
    }
}

fn validate_reference_catalog(
    references: &[DecodedAttRawReferenceV1],
    decoded_wave_references: &[u32],
    relation: DecodedAttRawDecodeRelationV1,
) -> Result<(), DecodedAttErrorV1> {
    if references.is_empty() || references.len() > crate::MAX_PROFILER_ATT_REFERENCES_V4 {
        return Err(DecodedAttErrorV1::RawReferenceMismatch);
    }
    let mut complete = true;
    for (ordinal, reference) in (0_u32..).zip(references) {
        if reference.ordinal != ordinal || !safe_reference(&reference.reference) {
            return Err(DecodedAttErrorV1::RawReferenceMismatch);
        }
        match (
            reference.content.origin,
            reference.content.value,
            reference.content.unavailable_reason,
        ) {
            (TruthOriginV1::Declared, Some(value), None) => validate_content_identity(value)?,
            (TruthOriginV1::Unavailable, None, Some(CaptureUnavailableReasonV1::NotProvided)) => {
                complete = false
            }
            _ => return Err(DecodedAttErrorV1::RawReferenceMismatch),
        }
    }
    if decoded_wave_references.is_empty()
        || decoded_wave_references.len() > crate::MAX_PROFILER_ATT_REFERENCES_V4
        || !decoded_wave_references
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        || decoded_wave_references.iter().any(|ordinal| {
            usize::try_from(*ordinal)
                .ok()
                .and_then(|index| references.get(index))
                .is_none_or(|reference| {
                    reference.ordinal != *ordinal
                        || reference.kind != AttReferenceKindV4::WaveTimeline
                })
        })
    {
        return Err(DecodedAttErrorV1::InvalidRawDecodeRelation);
    }
    let wave_reference_count = references
        .iter()
        .filter(|reference| reference.kind == AttReferenceKindV4::WaveTimeline)
        .count();
    let valid_relation = if !complete {
        relation
            == DecodedAttRawDecodeRelationV1 {
                origin: TruthOriginV1::Unavailable,
                state: DecodedAttRawRelationV1::UnavailableMissingExactRawReferenceIdentity,
            }
    } else if decoded_wave_references.len() == wave_reference_count {
        relation
            == DecodedAttRawDecodeRelationV1 {
                origin: TruthOriginV1::Declared,
                state: DecodedAttRawRelationV1::ExternalDecoderDeclaredCompleteExactWaveInputs,
            }
    } else {
        relation
            == DecodedAttRawDecodeRelationV1 {
                origin: TruthOriginV1::Unavailable,
                state: DecodedAttRawRelationV1::UnavailableIncompleteWaveReferenceCoverage,
            }
    };
    if !valid_relation {
        return Err(DecodedAttErrorV1::InvalidRawDecodeRelation);
    }
    Ok(())
}

fn validate_record_source(
    source_reference_ordinal: u32,
    decoded_wave_references: &[u32],
) -> Result<(), DecodedAttErrorV1> {
    if decoded_wave_references
        .binary_search(&source_reference_ordinal)
        .is_ok()
    {
        Ok(())
    } else {
        Err(DecodedAttErrorV1::RawReferenceMismatch)
    }
}

fn validate_callback_summaries(
    callbacks: &[DecodedAttCallbackSummaryV1],
    decoded_wave_references: &[u32],
) -> Result<(), DecodedAttErrorV1> {
    if callbacks.is_empty() || callbacks.len() > MAX_DECODED_ATT_CALLBACKS_V1 {
        return Err(DecodedAttErrorV1::CallbackLimitExceeded);
    }
    let mut source_index = 0_usize;
    let mut active_source = None;
    let mut frequency_seen_for_source = false;
    let mut total = 0_u64;
    for (ordinal, callback) in (0_u64..).zip(callbacks) {
        if callback.ordinal != ordinal
            || callback.record_count == 0
            || decoded_wave_references
                .binary_search(&callback.source_reference_ordinal)
                .is_err()
        {
            return Err(DecodedAttErrorV1::InvalidInterchange);
        }
        total = total
            .checked_add(callback.record_count)
            .ok_or(DecodedAttErrorV1::SizeOverflow)?;
        let first = active_source != Some(callback.source_reference_ordinal);
        if first {
            if active_source.is_some_and(|prior| prior >= callback.source_reference_ordinal)
                || decoded_wave_references.get(source_index)
                    != Some(&callback.source_reference_ordinal)
            {
                return Err(DecodedAttErrorV1::InvalidInterchange);
            }
            source_index = source_index
                .checked_add(1)
                .ok_or(DecodedAttErrorV1::SizeOverflow)?;
            active_source = Some(callback.source_reference_ordinal);
            frequency_seen_for_source = false;
        }
        if first != (callback.kind == DecodedAttCallbackKindV1::Gfxip)
            || (callback.kind == DecodedAttCallbackKindV1::Gfxip && callback.record_count != 1)
            || (callback.kind == DecodedAttCallbackKindV1::RtFrequency
                && (callback.record_count != 1 || frequency_seen_for_source))
        {
            return Err(DecodedAttErrorV1::InvalidInterchange);
        }
        if callback.kind == DecodedAttCallbackKindV1::RtFrequency {
            frequency_seen_for_source = true;
        }
    }
    if total > usize_to_u64(MAX_DECODED_ATT_RECORDS_V1)?
        || source_index != decoded_wave_references.len()
    {
        return Err(DecodedAttErrorV1::InvalidInterchange);
    }
    Ok(())
}

fn validate_record_sequence(
    callbacks: &[DecodedAttCallbackSummaryV1],
    kind: DecodedAttCallbackKindV1,
    actual: impl Iterator<Item = (u64, u64, u32)>,
) -> Result<(), DecodedAttErrorV1> {
    let mut actual = actual;
    for callback in callbacks.iter().filter(|value| value.kind == kind) {
        for record in 0..callback.record_count {
            if actual.next() != Some((callback.ordinal, record, callback.source_reference_ordinal))
            {
                return Err(DecodedAttErrorV1::InvalidInterchange);
            }
        }
    }
    if actual.next().is_some() {
        return Err(DecodedAttErrorV1::InvalidInterchange);
    }
    Ok(())
}

fn validate_callback_record(
    callbacks: &[DecodedAttCallbackSummaryV1],
    kind: DecodedAttCallbackKindV1,
    callback_ordinal: u64,
    record_ordinal: u64,
    source_reference_ordinal: u32,
) -> Result<(), DecodedAttErrorV1> {
    let callback_index =
        usize::try_from(callback_ordinal).map_err(|_| DecodedAttErrorV1::InvalidInterchange)?;
    if callbacks.get(callback_index).is_some_and(|callback| {
        callback.ordinal == callback_ordinal
            && callback.kind == kind
            && callback.source_reference_ordinal == source_reference_ordinal
            && record_ordinal < callback.record_count
    }) {
        Ok(())
    } else {
        Err(DecodedAttErrorV1::InvalidInterchange)
    }
}

fn validate_output_code_objects(
    values: &[DecodedAttCodeObjectV1],
) -> Result<(), DecodedAttErrorV1> {
    if values.len() > MAX_DECODED_ATT_CODE_OBJECTS_V1 {
        return Err(DecodedAttErrorV1::CodeObjectLimitExceeded);
    }
    let mut selectors = Vec::new();
    selectors
        .try_reserve_exact(values.len())
        .map_err(|_| DecodedAttErrorV1::AllocationFailure)?;
    for value in values {
        validate_content_identity(value.artifact)?;
        if value.origin != DecodedAttRecordOriginV1::ExternalDecoderDeclared
            || value.load_size == 0
            || value.identity
                != derive_code_object_identity(value.selector, value.artifact, value.load_size)?
        {
            return Err(DecodedAttErrorV1::InvalidCodeObject);
        }
        selectors.push(value.selector);
    }
    if !values
        .windows(2)
        .all(|pair| pair[0].identity < pair[1].identity)
    {
        return Err(DecodedAttErrorV1::InvalidCodeObject);
    }
    selectors.sort_unstable();
    if !selectors.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(DecodedAttErrorV1::InvalidCodeObject);
    }
    Ok(())
}

fn validate_wave_children(
    export: ContentIdentityRecordV1,
    wave: &DecodedAttWaveV1,
    code_objects: &[DecodedAttCodeObjectV1],
) -> Result<(), DecodedAttErrorV1> {
    for (ordinal, value) in (0_u64..).zip(&wave.timeline) {
        if value.ordinal != ordinal
            || value.origin != DecodedAttRecordOriginV1::ExternalDecoderDeclared
            || value.duration_cycles < 0
            || value.identity != derive_child_identity(wave.identity, b"state", ordinal)?
        {
            return Err(DecodedAttErrorV1::InvalidWaveState);
        }
    }
    let mut prior = None;
    for (ordinal, value) in (0_u64..).zip(&wave.instructions) {
        if value.ordinal != ordinal
            || value.origin != DecodedAttRecordOriginV1::ExternalDecoderDeclared
            || value.stall_cycles > 0x00ff_ffff
            || value.duration_cycles < 0
            || value.stall_cycles > value.duration_cycles as u32
            || value
                .time
                .checked_add(i64::from(value.duration_cycles))
                .is_none()
            || prior.is_some_and(|time| value.time < time)
            || value.identity != derive_child_identity(wave.identity, b"instruction", ordinal)?
        {
            return Err(DecodedAttErrorV1::InvalidInstruction);
        }
        validate_pc(value.pc, code_objects)?;
        prior = Some(value.time);
    }
    validate_record_identity(
        export,
        b"wave",
        wave.source_callback_ordinal,
        wave.source_record_ordinal,
        wave.source_reference_ordinal,
        wave.identity,
    )
}

fn validate_pc(
    value: DecodedAttPcV1,
    code_objects: &[DecodedAttCodeObjectV1],
) -> Result<(), DecodedAttErrorV1> {
    let valid = match value.availability {
        DecodedAttPcAvailabilityV1::ElfVirtualAddress => {
            value.origin == DecodedAttRecordOriginV1::ExternalDecoderDeclared
                && value.code_object.is_some_and(|identity| {
                    code_objects
                        .binary_search_by_key(&identity, |value| value.identity)
                        .is_ok()
                })
                && value.elf_virtual_address.is_some()
                && value.unavailable_reason.is_none()
        }
        DecodedAttPcAvailabilityV1::UnavailableNativeVirtualAddressRedacted => {
            value.origin == DecodedAttRecordOriginV1::ExternalDecoderDeclared
                && value.code_object.is_none()
                && value.elf_virtual_address.is_none()
                && value.unavailable_reason == Some(CaptureUnavailableReasonV1::NotRepresented)
        }
    };
    if valid {
        Ok(())
    } else {
        Err(DecodedAttErrorV1::InvalidPc)
    }
}

fn validate_record_identity(
    export: ContentIdentityRecordV1,
    kind: &[u8],
    callback: u64,
    record: u64,
    source_reference_ordinal: u32,
    actual: CaptureIdentityV1,
) -> Result<(), DecodedAttErrorV1> {
    if actual == derive_record_identity(export, kind, callback, record, source_reference_ordinal)? {
        Ok(())
    } else {
        Err(DecodedAttErrorV1::StaleRecordIdentity)
    }
}

fn derive_selector(
    export: ContentIdentityRecordV1,
    raw_load_id: u64,
) -> Result<CaptureIdentityV1, DecodedAttErrorV1> {
    derive_identity(
        DECODED_ATT_CODE_OBJECT_SELECTOR_DOMAIN_V1,
        &[&export.digest.as_bytes(), &raw_load_id.to_le_bytes()],
    )
}

fn derive_code_object_identity(
    selector: CaptureIdentityV1,
    artifact: ContentIdentityRecordV1,
    load_size: u64,
) -> Result<CaptureIdentityV1, DecodedAttErrorV1> {
    derive_identity(
        DECODED_ATT_CODE_OBJECT_IDENTITY_DOMAIN_V1,
        &[
            &selector.as_bytes(),
            &artifact.digest.as_bytes(),
            &artifact.canonical_len.to_le_bytes(),
            &load_size.to_le_bytes(),
        ],
    )
}

fn derive_record_identity(
    export: ContentIdentityRecordV1,
    kind: &[u8],
    callback: u64,
    record: u64,
    source_reference_ordinal: u32,
) -> Result<CaptureIdentityV1, DecodedAttErrorV1> {
    derive_identity(
        DECODED_ATT_RECORD_IDENTITY_DOMAIN_V1,
        &[
            &export.digest.as_bytes(),
            kind,
            &callback.to_le_bytes(),
            &record.to_le_bytes(),
            &source_reference_ordinal.to_le_bytes(),
        ],
    )
}

fn derive_child_identity(
    parent: CaptureIdentityV1,
    kind: &[u8],
    ordinal: u64,
) -> Result<CaptureIdentityV1, DecodedAttErrorV1> {
    derive_identity(
        DECODED_ATT_RECORD_IDENTITY_DOMAIN_V1,
        &[&parent.as_bytes(), kind, &ordinal.to_le_bytes()],
    )
}

fn derive_identity(
    domain: &[u8],
    fields: &[&[u8]],
) -> Result<CaptureIdentityV1, DecodedAttErrorV1> {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for field in fields {
        hasher.update(usize_to_u64(field.len())?.to_le_bytes());
        hasher.update(field);
    }
    CaptureIdentityV1::new(hasher.finalize().into()).map_err(|_| DecodedAttErrorV1::IdentityFailure)
}

fn domain_identity(
    domain: &[u8],
    version: u16,
    bytes: &[u8],
) -> Result<ContentIdentityRecordV1, DecodedAttErrorV1> {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: version,
        digest: CaptureIdentityV1::new(hasher.finalize().into())
            .map_err(|_| DecodedAttErrorV1::IdentityFailure)?,
        canonical_len: u64::try_from(bytes.len()).map_err(|_| DecodedAttErrorV1::SizeOverflow)?,
    })
}

fn validate_content_identity(value: ContentIdentityRecordV1) -> Result<(), DecodedAttErrorV1> {
    if value.format_version == 0 || value.canonical_len == 0 {
        return Err(DecodedAttErrorV1::InvalidContentIdentity);
    }
    Ok(())
}

fn bounded_input(bytes: &[u8], maximum: u64) -> Result<(), DecodedAttErrorV1> {
    let len = u64::try_from(bytes.len()).map_err(|_| DecodedAttErrorV1::SizeOverflow)?;
    if len == 0 || len > maximum {
        return Err(DecodedAttErrorV1::InputTooLarge);
    }
    Ok(())
}

fn reserve<T>(output: &mut Vec<T>, additional: usize) -> Result<(), DecodedAttErrorV1> {
    output
        .try_reserve_exact(additional)
        .map_err(|_| DecodedAttErrorV1::AllocationFailure)
}

fn usize_to_u64(value: usize) -> Result<u64, DecodedAttErrorV1> {
    u64::try_from(value).map_err(|_| DecodedAttErrorV1::SizeOverflow)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BoundedJsonEncodeErrorV1 {
    TooLarge,
    Allocation,
    Json,
}

fn encode_json_bounded(
    value: &impl Serialize,
    maximum: u64,
) -> Result<Vec<u8>, BoundedJsonEncodeErrorV1> {
    let mut output = Vec::new();
    let initial =
        usize::try_from(maximum.min(64 * 1024)).map_err(|_| BoundedJsonEncodeErrorV1::TooLarge)?;
    output
        .try_reserve_exact(initial)
        .map_err(|_| BoundedJsonEncodeErrorV1::Allocation)?;
    let mut writer = BoundedJsonWriterV1 {
        output: &mut output,
        maximum,
        too_large: false,
        allocation_failed: false,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| {
        if writer.too_large {
            BoundedJsonEncodeErrorV1::TooLarge
        } else if writer.allocation_failed {
            BoundedJsonEncodeErrorV1::Allocation
        } else {
            BoundedJsonEncodeErrorV1::Json
        }
    })?;
    Ok(output)
}

struct BoundedJsonWriterV1<'a> {
    output: &'a mut Vec<u8>,
    maximum: u64,
    too_large: bool,
    allocation_failed: bool,
}

impl Write for BoundedJsonWriterV1<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let next = u64::try_from(self.output.len()).ok().and_then(|value| {
            u64::try_from(bytes.len())
                .ok()
                .and_then(|bytes| value.checked_add(bytes))
        });
        if next.is_none_or(|value| value > self.maximum) {
            self.too_large = true;
            return Err(std::io::Error::other("decoded ATT JSON limit exceeded"));
        }
        if self.output.try_reserve_exact(bytes.len()).is_err() {
            self.allocation_failed = true;
            return Err(std::io::Error::other("decoded ATT JSON allocation failed"));
        }
        self.output.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn copy_string(value: &str) -> Result<String, DecodedAttErrorV1> {
    let mut output = String::new();
    output
        .try_reserve_exact(value.len())
        .map_err(|_| DecodedAttErrorV1::AllocationFailure)?;
    output.push_str(value);
    Ok(output)
}

fn safe_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_DECODED_ATT_REFERENCE_BYTES_V1
        && value.is_ascii()
        && !value.starts_with('/')
        && !value.starts_with('\\')
        && !value.contains('\\')
        && !value.contains(':')
        && value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn deserialize_safe_reference<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    struct ReferenceVisitor;
    impl Visitor<'_> for ReferenceVisitor {
        type Value = String;
        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a bounded safe relative ASCII reference")
        }
        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if !safe_reference(value) {
                return Err(E::custom("unsafe decoded ATT reference"));
            }
            let mut output = String::new();
            output
                .try_reserve_exact(value.len())
                .map_err(|_| E::custom("decoded ATT reference allocation failed"))?;
            output.push_str(value);
            Ok(output)
        }
    }
    deserializer.deserialize_str(ReferenceVisitor)
}

fn deserialize_bounded_vec<'de, D, T>(
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
    impl<'de, T> Visitor<'de> for BoundedVisitor<T>
    where
        T: Deserialize<'de>,
    {
        type Value = Vec<T>;
        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "at most {} {} records", self.maximum, self.label)
        }
        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            if sequence.size_hint().is_some_and(|hint| hint > self.maximum) {
                return Err(serde::de::Error::custom("decoded ATT array bound exceeded"));
            }
            let mut output = Vec::new();
            while let Some(value) = sequence.next_element()? {
                if output.len() == self.maximum {
                    return Err(serde::de::Error::custom("decoded ATT array bound exceeded"));
                }
                output
                    .try_reserve_exact(1)
                    .map_err(|_| serde::de::Error::custom("decoded ATT allocation failed"))?;
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

macro_rules! bounded_deserializer {
    ($name:ident, $ty:ty, $max:expr, $label:literal) => {
        fn $name<'de, D>(deserializer: D) -> Result<Vec<$ty>, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserialize_bounded_vec(deserializer, $max, $label)
        }
    };
}

bounded_deserializer!(
    deserialize_raw_export_references,
    RawReferenceV1,
    crate::MAX_PROFILER_ATT_REFERENCES_V4,
    "raw reference"
);
bounded_deserializer!(
    deserialize_raw_code_objects,
    RawCodeObjectV1,
    MAX_DECODED_ATT_CODE_OBJECTS_V1,
    "code object"
);
bounded_deserializer!(
    deserialize_raw_callbacks,
    RawCallbackV1,
    MAX_DECODED_ATT_CALLBACKS_V1,
    "callback"
);
bounded_deserializer!(
    deserialize_callback_summaries,
    DecodedAttCallbackSummaryV1,
    MAX_DECODED_ATT_CALLBACKS_V1,
    "callback summary"
);
bounded_deserializer!(deserialize_gfxip_records, u64, 1, "gfxip");
bounded_deserializer!(
    deserialize_raw_occupancy,
    RawOccupancyV1,
    MAX_DECODED_ATT_RECORDS_V1,
    "occupancy"
);
bounded_deserializer!(
    deserialize_raw_perf_events,
    RawPerfEventV1,
    MAX_DECODED_ATT_RECORDS_V1,
    "performance event"
);
bounded_deserializer!(
    deserialize_raw_waves,
    RawWaveV1,
    MAX_DECODED_ATT_WAVES_V1,
    "wave"
);
bounded_deserializer!(
    deserialize_info_kinds,
    DecodedAttInfoKindV1,
    MAX_DECODED_ATT_RECORDS_V1,
    "info"
);
bounded_deserializer!(
    deserialize_raw_shader_data,
    RawShaderDataV1,
    MAX_DECODED_ATT_RECORDS_V1,
    "shader data"
);
bounded_deserializer!(
    deserialize_raw_realtime,
    RawRealtimeV1,
    MAX_DECODED_ATT_RECORDS_V1,
    "realtime"
);
bounded_deserializer!(deserialize_frequency_records, u64, 1, "realtime frequency");
bounded_deserializer!(
    deserialize_raw_wave_states,
    RawWaveStateV1,
    MAX_DECODED_ATT_WAVE_STATES_V1,
    "wave state"
);
bounded_deserializer!(
    deserialize_raw_instructions,
    RawInstructionV1,
    MAX_DECODED_ATT_INSTRUCTIONS_V1,
    "instruction"
);
bounded_deserializer!(
    deserialize_raw_references,
    DecodedAttRawReferenceV1,
    crate::MAX_PROFILER_ATT_REFERENCES_V4,
    "raw reference"
);
bounded_deserializer!(
    deserialize_decoded_wave_references,
    u32,
    crate::MAX_PROFILER_ATT_REFERENCES_V4,
    "decoded wave reference"
);
bounded_deserializer!(
    deserialize_code_objects,
    DecodedAttCodeObjectV1,
    MAX_DECODED_ATT_CODE_OBJECTS_V1,
    "code object"
);
bounded_deserializer!(
    deserialize_occupancy,
    DecodedAttOccupancyV1,
    MAX_DECODED_ATT_RECORDS_V1,
    "occupancy"
);
bounded_deserializer!(
    deserialize_perf_events,
    DecodedAttPerfEventV1,
    MAX_DECODED_ATT_RECORDS_V1,
    "performance event"
);
bounded_deserializer!(
    deserialize_waves,
    DecodedAttWaveV1,
    MAX_DECODED_ATT_WAVES_V1,
    "wave"
);
bounded_deserializer!(
    deserialize_wave_states,
    DecodedAttWaveStateV1,
    MAX_DECODED_ATT_WAVE_STATES_V1,
    "wave state"
);
bounded_deserializer!(
    deserialize_instructions,
    DecodedAttInstructionV1,
    MAX_DECODED_ATT_INSTRUCTIONS_V1,
    "instruction"
);
bounded_deserializer!(
    deserialize_shader_data,
    DecodedAttShaderDataV1,
    MAX_DECODED_ATT_RECORDS_V1,
    "shader data"
);
bounded_deserializer!(
    deserialize_realtime,
    DecodedAttRealtimeV1,
    MAX_DECODED_ATT_RECORDS_V1,
    "realtime"
);
bounded_deserializer!(
    deserialize_info,
    DecodedAttInfoV1,
    MAX_DECODED_ATT_RECORDS_V1,
    "info"
);

#[derive(Debug)]
#[non_exhaustive]
pub enum DecodedAttErrorV1 {
    LimitOutOfRange,
    InputTooLarge,
    InterchangeTooLarge,
    UnsupportedExportSchema,
    UnsupportedDecoderAbi,
    InvalidDecoderIdentity,
    InvalidAttBundle,
    InvalidContentIdentity,
    RawReferenceMismatch,
    InvalidRawDecodeRelation,
    CallbackLimitExceeded,
    RecordLimitExceeded,
    CodeObjectLimitExceeded,
    EmptyCallback,
    InvalidGfxip,
    InvalidCodeObject,
    UnknownCodeObject,
    InvalidPc,
    InvalidOccupancy,
    InvalidPerfEvent,
    InvalidWave,
    InvalidWaveState,
    InvalidInstruction,
    InvalidShaderData,
    InvalidRealtime,
    InvalidRealtimeFrequency,
    InvalidInfo,
    InvalidCoverage,
    InvalidInterchange,
    StaleRecordIdentity,
    StaleInterchange,
    IdentityFailure,
    SizeOverflow,
    AllocationFailure,
    JsonEncode,
    JsonDecode,
    NonCanonicalExport,
    NonCanonicalInterchange,
}

impl fmt::Display for DecodedAttErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "decoded ATT evidence rejected: {self:?}")
    }
}

impl Error for DecodedAttErrorV1 {}
