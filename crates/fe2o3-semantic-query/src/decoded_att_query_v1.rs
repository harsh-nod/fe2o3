//! Bounded, read-only queries over admitted external ROCprofiler ATT decoder output.

use std::error::Error;
use std::fmt;
use std::io::{BufRead, Read, Write};

use fe2o3_semantic_import::{
    CaptureIdentityV1, ContentIdentityRecordV1, DecodedAttCodeObjectV1, DecodedAttCompletenessV1,
    DecodedAttCoverageV1, DecodedAttInfoV1, DecodedAttInstructionCategoryV1,
    DecodedAttInstructionV1, DecodedAttLossStateV1, DecodedAttOccupancyV1, DecodedAttPerfEventV1,
    DecodedAttRawDecodeRelationV1, DecodedAttRawReferenceV1, DecodedAttRealtimeV1,
    DecodedAttShaderDataV1, DecodedAttSourceCorrelationV1, DecodedAttWaveStateKindV1,
    DecodedAttWaveStateV1, MAX_DECODED_ATT_INTERCHANGE_BYTES_V1, SemanticDecodedAttV1,
    decode_decoded_att_v1, decoded_att_content_identity_v1,
};
use serde::de::Visitor;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAX_DECODED_ATT_QUERY_PAGE_ITEMS_V1: u16 = 4_096;
pub const MAX_DECODED_ATT_QUERY_RESPONSE_BYTES_V1: u64 = 2 * 1024 * 1024;
pub const MAX_AGENT_DECODED_ATT_REQUEST_BYTES_V1: u64 =
    MAX_DECODED_ATT_INTERCHANGE_BYTES_V1 * 2 + 16 * 1024;
pub const MAX_AGENT_DECODED_ATT_RESPONSE_BYTES_V1: u64 =
    MAX_DECODED_ATT_QUERY_RESPONSE_BYTES_V1 + 16 * 1024;
pub const MAX_AGENT_DECODED_ATT_REQUEST_ATTEMPTS_V1: u64 = 1_024;
const MIN_DECODED_ATT_QUERY_RESPONSE_BYTES_V1: u64 = 4 * 1024;
const DECODED_ATT_CURSOR_DOMAIN_V1: &[u8] = b"fe2o3.decoded-att-query.cursor.v1\0";
const AGENT_DECODED_ATT_REQUEST_SCHEMA_V1: &str = "fe2o3-decoded-att-agent-request-v1";
const AGENT_DECODED_ATT_RESPONSE_SCHEMA_V1: &str = "fe2o3-decoded-att-agent-response-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedAttQueryLimitsV1 {
    pub max_input_bytes: u64,
    pub max_response_bytes: u64,
    pub max_page_items: u16,
}

impl Default for DecodedAttQueryLimitsV1 {
    fn default() -> Self {
        Self {
            max_input_bytes: MAX_DECODED_ATT_INTERCHANGE_BYTES_V1,
            max_response_bytes: MAX_DECODED_ATT_QUERY_RESPONSE_BYTES_V1,
            max_page_items: 128,
        }
    }
}

impl DecodedAttQueryLimitsV1 {
    pub fn new(input: u64, response: u64, page: u16) -> Result<Self, DecodedAttQueryErrorV1> {
        if input == 0
            || input > MAX_DECODED_ATT_INTERCHANGE_BYTES_V1
            || !(MIN_DECODED_ATT_QUERY_RESPONSE_BYTES_V1..=MAX_DECODED_ATT_QUERY_RESPONSE_BYTES_V1)
                .contains(&response)
            || page == 0
            || page > MAX_DECODED_ATT_QUERY_PAGE_ITEMS_V1
        {
            return Err(DecodedAttQueryErrorV1::LimitOutOfRange);
        }
        Ok(Self {
            max_input_bytes: input,
            max_response_bytes: response,
            max_page_items: page,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodedAttListKindV1 {
    RawReferences,
    CodeObjects,
    Occupancy,
    Waves,
    WaveStates,
    Instructions,
    PerfEvents,
    ShaderData,
    Realtime,
    Info,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttCursorV1 {
    pub query_binding: CaptureIdentityV1,
    pub position: u64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttFilterV1 {
    pub cu_or_wgp: Option<u8>,
    pub simd: Option<u8>,
    pub wave_slot: Option<u8>,
    pub code_object: Option<CaptureIdentityV1>,
    pub instruction_category: Option<DecodedAttInstructionCategoryV1>,
    pub wave_state: Option<DecodedAttWaveStateKindV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttPageRequestV1 {
    pub limit: u16,
    pub cursor: Option<DecodedAttCursorV1>,
    pub filter: DecodedAttFilterV1,
}

impl Default for DecodedAttPageRequestV1 {
    fn default() -> Self {
        Self {
            limit: 128,
            cursor: None,
            filter: DecodedAttFilterV1::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodedAttCapabilityNameV1 {
    RawReferenceCatalog,
    CodeObjects,
    Occupancy,
    WaveLifetimes,
    WaveStateTimelines,
    Instructions,
    PerformanceEvents,
    ShaderData,
    RealtimeCorrelation,
    LossAndCompleteness,
    SourceMirKirLlvmIsaCorrelation,
    AuthenticatedDecoderCustody,
    RawAttDecode,
    Collection,
    ExecutionControl,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodedAttAvailabilityV1 {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttCapabilityV1 {
    pub name: DecodedAttCapabilityNameV1,
    pub availability: DecodedAttAvailabilityV1,
    pub reason: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttQueryContextV1 {
    pub interchange: ContentIdentityRecordV1,
    pub export_source: ContentIdentityRecordV1,
    pub att_bundle: ContentIdentityRecordV1,
    pub att_manifest: ContentIdentityRecordV1,
    pub raw_decode_relation: DecodedAttRawDecodeRelationV1,
    pub source_correlation: DecodedAttSourceCorrelationV1,
    pub completeness: DecodedAttCompletenessV1,
    pub loss: DecodedAttLossStateV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttWaveSummaryV1 {
    pub identity: CaptureIdentityV1,
    pub source_callback_ordinal: u64,
    pub source_record_ordinal: u64,
    pub source_reference_ordinal: u32,
    pub cu_or_wgp: u8,
    pub simd: u8,
    pub wave_slot: u8,
    pub contexts: u8,
    pub begin_time: i64,
    pub end_time: i64,
    pub state_count: u64,
    pub instruction_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "item", rename_all = "snake_case")]
pub enum DecodedAttQueryItemV1 {
    RawReference {
        reference: DecodedAttRawReferenceV1,
    },
    CodeObject {
        code_object: DecodedAttCodeObjectV1,
    },
    Occupancy {
        occupancy: DecodedAttOccupancyV1,
    },
    Wave {
        wave: DecodedAttWaveSummaryV1,
    },
    WaveState {
        wave_identity: CaptureIdentityV1,
        cu_or_wgp: u8,
        simd: u8,
        wave_slot: u8,
        state: DecodedAttWaveStateV1,
    },
    Instruction {
        wave_identity: CaptureIdentityV1,
        cu_or_wgp: u8,
        simd: u8,
        wave_slot: u8,
        instruction: DecodedAttInstructionV1,
    },
    PerfEvent {
        event: DecodedAttPerfEventV1,
    },
    ShaderData {
        data: DecodedAttShaderDataV1,
    },
    Realtime {
        correlation: DecodedAttRealtimeV1,
    },
    Info {
        info: DecodedAttInfoV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttPageV1 {
    pub context: DecodedAttQueryContextV1,
    pub kind: DecodedAttListKindV1,
    pub filter: DecodedAttFilterV1,
    pub returned: u16,
    pub next_cursor: Option<DecodedAttCursorV1>,
    pub items: Vec<DecodedAttQueryItemV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "response", rename_all = "snake_case")]
pub enum DecodedAttQueryResponseV1 {
    Capabilities {
        context: DecodedAttQueryContextV1,
        capabilities: Vec<DecodedAttCapabilityV1>,
    },
    Open {
        context: DecodedAttQueryContextV1,
        coverage: DecodedAttCoverageV1,
        realtime_frequency_hz: Option<u64>,
    },
    Page {
        page: DecodedAttPageV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedAttQueryRequestV1 {
    Capabilities,
    Open,
    List {
        kind: DecodedAttListKindV1,
        page: DecodedAttPageRequestV1,
    },
}

pub struct DecodedAttQuerySessionV1 {
    capture: SemanticDecodedAttV1,
    context: DecodedAttQueryContextV1,
    limits: DecodedAttQueryLimitsV1,
}

impl DecodedAttQuerySessionV1 {
    pub fn open(
        bytes: &[u8],
        limits: DecodedAttQueryLimitsV1,
    ) -> Result<Self, DecodedAttQueryErrorV1> {
        let actual =
            u64::try_from(bytes.len()).map_err(|_| DecodedAttQueryErrorV1::SizeOverflow)?;
        if actual == 0 || actual > limits.max_input_bytes {
            return Err(DecodedAttQueryErrorV1::InputTooLarge);
        }
        let capture = decode_decoded_att_v1(bytes).map_err(DecodedAttQueryErrorV1::Capture)?;
        let identity =
            decoded_att_content_identity_v1(bytes).map_err(DecodedAttQueryErrorV1::Capture)?;
        let context = DecodedAttQueryContextV1 {
            interchange: identity,
            export_source: capture.export_source,
            att_bundle: capture.att_bundle,
            att_manifest: capture.att_manifest,
            raw_decode_relation: capture.raw_decode_relation,
            source_correlation: capture.source_correlation,
            completeness: capture.coverage.completeness,
            loss: capture.coverage.loss,
        };
        Ok(Self {
            capture,
            context,
            limits,
        })
    }

    pub const fn context(&self) -> DecodedAttQueryContextV1 {
        self.context
    }

    pub fn query(
        &self,
        request: DecodedAttQueryRequestV1,
    ) -> Result<DecodedAttQueryResponseV1, DecodedAttQueryErrorV1> {
        match request {
            DecodedAttQueryRequestV1::Capabilities => Ok(DecodedAttQueryResponseV1::Capabilities {
                context: self.context,
                capabilities: self.capabilities()?,
            }),
            DecodedAttQueryRequestV1::Open => Ok(DecodedAttQueryResponseV1::Open {
                context: self.context,
                coverage: self.capture.coverage,
                realtime_frequency_hz: self.capture.realtime_frequency_hz,
            }),
            DecodedAttQueryRequestV1::List { kind, page } => Ok(DecodedAttQueryResponseV1::Page {
                page: self.page(kind, page)?,
            }),
        }
    }

    pub fn encode_response(
        &self,
        response: &DecodedAttQueryResponseV1,
    ) -> Result<Vec<u8>, DecodedAttQueryErrorV1> {
        let mut output = Vec::new();
        let reserve = usize::try_from(self.limits.max_response_bytes.min(64 * 1024))
            .map_err(|_| DecodedAttQueryErrorV1::SizeOverflow)?;
        output
            .try_reserve_exact(reserve)
            .map_err(|_| DecodedAttQueryErrorV1::AllocationFailure)?;
        let mut writer = DecodedAttBoundedWriterV1 {
            output: &mut output,
            maximum: self.limits.max_response_bytes,
            exceeded: false,
            allocation_failed: false,
        };
        serde_json::to_writer(&mut writer, response).map_err(|_| {
            if writer.exceeded {
                DecodedAttQueryErrorV1::ResponseTooLarge
            } else if writer.allocation_failed {
                DecodedAttQueryErrorV1::AllocationFailure
            } else {
                DecodedAttQueryErrorV1::Json
            }
        })?;
        if u64::try_from(output.len()).map_err(|_| DecodedAttQueryErrorV1::SizeOverflow)?
            == self.limits.max_response_bytes
        {
            return Err(DecodedAttQueryErrorV1::ResponseTooLarge);
        }
        output
            .try_reserve_exact(1)
            .map_err(|_| DecodedAttQueryErrorV1::AllocationFailure)?;
        output.push(b'\n');
        Ok(output)
    }

    fn capabilities(&self) -> Result<Vec<DecodedAttCapabilityV1>, DecodedAttQueryErrorV1> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(15)
            .map_err(|_| DecodedAttQueryErrorV1::AllocationFailure)?;
        let present = |count: usize, reason| capability(count != 0, reason);
        values.push(named_capability(
            DecodedAttCapabilityNameV1::RawReferenceCatalog,
            present(
                self.capture.raw_references.len(),
                "export contains no raw references",
            ),
        ));
        values.push(named_capability(
            DecodedAttCapabilityNameV1::CodeObjects,
            present(
                self.capture.code_objects.len(),
                "admitted export contains no code objects",
            ),
        ));
        values.push(named_capability(
            DecodedAttCapabilityNameV1::Occupancy,
            present(
                self.capture.occupancy.len(),
                "admitted export contains no occupancy records",
            ),
        ));
        values.push(named_capability(
            DecodedAttCapabilityNameV1::WaveLifetimes,
            present(
                self.capture.waves.len(),
                "admitted export contains no wave records",
            ),
        ));
        values.push(named_capability(
            DecodedAttCapabilityNameV1::WaveStateTimelines,
            capability(
                self.capture.coverage.wave_state_count != 0,
                "admitted export contains no wave-state records",
            ),
        ));
        values.push(named_capability(
            DecodedAttCapabilityNameV1::Instructions,
            capability(
                self.capture.coverage.instruction_count != 0,
                "admitted export contains no instruction records",
            ),
        ));
        values.push(named_capability(
            DecodedAttCapabilityNameV1::PerformanceEvents,
            present(
                self.capture.perf_events.len(),
                "admitted export contains no performance events",
            ),
        ));
        values.push(named_capability(
            DecodedAttCapabilityNameV1::ShaderData,
            present(
                self.capture.shader_data.len(),
                "admitted export contains no shaderdata records",
            ),
        ));
        values.push(named_capability(
            DecodedAttCapabilityNameV1::RealtimeCorrelation,
            capability(
                !self.capture.realtime.is_empty() && self.capture.realtime_frequency_hz.is_some(),
                "both realtime records and a nonzero RT frequency are required",
            ),
        ));
        values.push(named_capability(
            DecodedAttCapabilityNameV1::LossAndCompleteness,
            capability(true, ""),
        ));
        values.push(unavailable(
            DecodedAttCapabilityNameV1::SourceMirKirLlvmIsaCorrelation,
            "no independently admitted exact artifact-to-characteristic relation is present",
        ));
        values.push(unavailable(
            DecodedAttCapabilityNameV1::AuthenticatedDecoderCustody,
            "header, library, exporter, and export hashes pin bytes but do not authenticate custody",
        ));
        values.push(unavailable(
            DecodedAttCapabilityNameV1::RawAttDecode,
            "this authority-free service admits decoder callbacks and does not decode raw ATT",
        ));
        values.push(unavailable(
            DecodedAttCapabilityNameV1::Collection,
            "this read-only service has no ATT collection authority",
        ));
        values.push(unavailable(
            DecodedAttCapabilityNameV1::ExecutionControl,
            "this read-only service has no execution-control authority",
        ));
        Ok(values)
    }

    fn page(
        &self,
        kind: DecodedAttListKindV1,
        page: DecodedAttPageRequestV1,
    ) -> Result<DecodedAttPageV1, DecodedAttQueryErrorV1> {
        if page.limit == 0 || page.limit > self.limits.max_page_items {
            return Err(DecodedAttQueryErrorV1::PageLimit);
        }
        validate_filter(kind, page.filter)?;
        let binding = cursor_binding(self.context.interchange.digest, kind, page.filter)?;
        let start = match page.cursor {
            Some(cursor) if cursor.query_binding == binding => usize::try_from(cursor.position)
                .map_err(|_| DecodedAttQueryErrorV1::CursorOutOfRange)?,
            Some(_) => return Err(DecodedAttQueryErrorV1::CursorMismatch),
            None => 0,
        };
        if matches!(
            kind,
            DecodedAttListKindV1::WaveStates | DecodedAttListKindV1::Instructions
        ) {
            return self.child_page(kind, page, binding, start);
        }
        let capacity = usize::from(page.limit)
            .checked_add(1)
            .ok_or(DecodedAttQueryErrorV1::SizeOverflow)?;
        let mut items = Vec::new();
        items
            .try_reserve_exact(capacity)
            .map_err(|_| DecodedAttQueryErrorV1::AllocationFailure)?;
        let mut matched = 0_usize;
        self.visit_items(kind, page.filter, |item| {
            if matched >= start && items.len() < capacity {
                items.push(item);
            }
            matched = matched.saturating_add(1);
        })?;
        if start > matched {
            return Err(DecodedAttQueryErrorV1::CursorOutOfRange);
        }
        let has_more = items.len() > usize::from(page.limit);
        if has_more {
            items.pop();
        }
        let end = start
            .checked_add(items.len())
            .ok_or(DecodedAttQueryErrorV1::SizeOverflow)?;
        let next_cursor = has_more.then_some(DecodedAttCursorV1 {
            query_binding: binding,
            position: u64::try_from(end).map_err(|_| DecodedAttQueryErrorV1::SizeOverflow)?,
        });
        Ok(DecodedAttPageV1 {
            context: self.context,
            kind,
            filter: page.filter,
            returned: u16::try_from(items.len())
                .map_err(|_| DecodedAttQueryErrorV1::SizeOverflow)?,
            next_cursor,
            items,
        })
    }

    fn child_page(
        &self,
        kind: DecodedAttListKindV1,
        page: DecodedAttPageRequestV1,
        binding: CaptureIdentityV1,
        start: usize,
    ) -> Result<DecodedAttPageV1, DecodedAttQueryErrorV1> {
        let total = match kind {
            DecodedAttListKindV1::WaveStates => self.capture.coverage.wave_state_count,
            DecodedAttListKindV1::Instructions => self.capture.coverage.instruction_count,
            _ => return Err(DecodedAttQueryErrorV1::InvalidFilter),
        };
        if u64::try_from(start).map_err(|_| DecodedAttQueryErrorV1::CursorOutOfRange)? > total {
            return Err(DecodedAttQueryErrorV1::CursorOutOfRange);
        }
        let mut items = Vec::new();
        items
            .try_reserve_exact(usize::from(page.limit))
            .map_err(|_| DecodedAttQueryErrorV1::AllocationFailure)?;
        let mut base = 0_usize;
        let mut next_position = None;
        'waves: for wave in &self.capture.waves {
            let children = match kind {
                DecodedAttListKindV1::WaveStates => wave.timeline.len(),
                DecodedAttListKindV1::Instructions => wave.instructions.len(),
                _ => unreachable!(),
            };
            let end = base
                .checked_add(children)
                .ok_or(DecodedAttQueryErrorV1::SizeOverflow)?;
            if end <= start {
                base = end;
                continue;
            }
            if !location_matches(page.filter, wave.cu_or_wgp, wave.simd, wave.wave_slot) {
                base = end;
                continue;
            }
            let local_start = start.saturating_sub(base);
            match kind {
                DecodedAttListKindV1::WaveStates => {
                    for (local, state) in wave.timeline.iter().enumerate().skip(local_start) {
                        if page
                            .filter
                            .wave_state
                            .is_some_and(|kind| kind != state.state)
                        {
                            continue;
                        }
                        if items.len() == usize::from(page.limit) {
                            next_position = Some(
                                base.checked_add(local)
                                    .ok_or(DecodedAttQueryErrorV1::SizeOverflow)?,
                            );
                            break 'waves;
                        }
                        items.push(DecodedAttQueryItemV1::WaveState {
                            wave_identity: wave.identity,
                            cu_or_wgp: wave.cu_or_wgp,
                            simd: wave.simd,
                            wave_slot: wave.wave_slot,
                            state: *state,
                        });
                    }
                }
                DecodedAttListKindV1::Instructions => {
                    for (local, instruction) in
                        wave.instructions.iter().enumerate().skip(local_start)
                    {
                        if page
                            .filter
                            .instruction_category
                            .is_some_and(|category| category != instruction.category)
                            || !pc_matches(page.filter, instruction.pc.code_object)
                        {
                            continue;
                        }
                        if items.len() == usize::from(page.limit) {
                            next_position = Some(
                                base.checked_add(local)
                                    .ok_or(DecodedAttQueryErrorV1::SizeOverflow)?,
                            );
                            break 'waves;
                        }
                        items.push(DecodedAttQueryItemV1::Instruction {
                            wave_identity: wave.identity,
                            cu_or_wgp: wave.cu_or_wgp,
                            simd: wave.simd,
                            wave_slot: wave.wave_slot,
                            instruction: *instruction,
                        });
                    }
                }
                _ => unreachable!(),
            }
            base = end;
        }
        Ok(DecodedAttPageV1 {
            context: self.context,
            kind,
            filter: page.filter,
            returned: u16::try_from(items.len())
                .map_err(|_| DecodedAttQueryErrorV1::SizeOverflow)?,
            next_cursor: next_position
                .map(|position| {
                    Ok(DecodedAttCursorV1 {
                        query_binding: binding,
                        position: u64::try_from(position)
                            .map_err(|_| DecodedAttQueryErrorV1::SizeOverflow)?,
                    })
                })
                .transpose()?,
            items,
        })
    }

    fn visit_items(
        &self,
        kind: DecodedAttListKindV1,
        filter: DecodedAttFilterV1,
        mut emit: impl FnMut(DecodedAttQueryItemV1),
    ) -> Result<(), DecodedAttQueryErrorV1> {
        match kind {
            DecodedAttListKindV1::RawReferences => {
                for value in &self.capture.raw_references {
                    emit(DecodedAttQueryItemV1::RawReference {
                        reference: value.clone(),
                    });
                }
            }
            DecodedAttListKindV1::CodeObjects => {
                for value in &self.capture.code_objects {
                    if filter
                        .code_object
                        .is_none_or(|identity| identity == value.identity)
                    {
                        emit(DecodedAttQueryItemV1::CodeObject {
                            code_object: *value,
                        });
                    }
                }
            }
            DecodedAttListKindV1::Occupancy => {
                for value in &self.capture.occupancy {
                    if location_matches(filter, value.cu_or_wgp, value.simd, value.wave_slot)
                        && pc_matches(filter, value.pc.code_object)
                    {
                        emit(DecodedAttQueryItemV1::Occupancy { occupancy: *value });
                    }
                }
            }
            DecodedAttListKindV1::Waves => {
                for value in &self.capture.waves {
                    if location_matches(filter, value.cu_or_wgp, value.simd, value.wave_slot) {
                        emit(DecodedAttQueryItemV1::Wave {
                            wave: DecodedAttWaveSummaryV1 {
                                identity: value.identity,
                                source_callback_ordinal: value.source_callback_ordinal,
                                source_record_ordinal: value.source_record_ordinal,
                                source_reference_ordinal: value.source_reference_ordinal,
                                cu_or_wgp: value.cu_or_wgp,
                                simd: value.simd,
                                wave_slot: value.wave_slot,
                                contexts: value.contexts,
                                begin_time: value.begin_time,
                                end_time: value.end_time,
                                state_count: u64::try_from(value.timeline.len())
                                    .map_err(|_| DecodedAttQueryErrorV1::SizeOverflow)?,
                                instruction_count: u64::try_from(value.instructions.len())
                                    .map_err(|_| DecodedAttQueryErrorV1::SizeOverflow)?,
                            },
                        });
                    }
                }
            }
            DecodedAttListKindV1::WaveStates => {
                for wave in &self.capture.waves {
                    if !location_matches(filter, wave.cu_or_wgp, wave.simd, wave.wave_slot) {
                        continue;
                    }
                    for state in &wave.timeline {
                        if filter.wave_state.is_none_or(|kind| kind == state.state) {
                            emit(DecodedAttQueryItemV1::WaveState {
                                wave_identity: wave.identity,
                                cu_or_wgp: wave.cu_or_wgp,
                                simd: wave.simd,
                                wave_slot: wave.wave_slot,
                                state: *state,
                            });
                        }
                    }
                }
            }
            DecodedAttListKindV1::Instructions => {
                for wave in &self.capture.waves {
                    if !location_matches(filter, wave.cu_or_wgp, wave.simd, wave.wave_slot) {
                        continue;
                    }
                    for instruction in &wave.instructions {
                        if filter
                            .instruction_category
                            .is_none_or(|category| category == instruction.category)
                            && pc_matches(filter, instruction.pc.code_object)
                        {
                            emit(DecodedAttQueryItemV1::Instruction {
                                wave_identity: wave.identity,
                                cu_or_wgp: wave.cu_or_wgp,
                                simd: wave.simd,
                                wave_slot: wave.wave_slot,
                                instruction: *instruction,
                            });
                        }
                    }
                }
            }
            DecodedAttListKindV1::PerfEvents => {
                for value in &self.capture.perf_events {
                    if filter.cu_or_wgp.is_none_or(|cu| cu == value.cu_or_wgp) {
                        emit(DecodedAttQueryItemV1::PerfEvent { event: *value });
                    }
                }
            }
            DecodedAttListKindV1::ShaderData => {
                for value in &self.capture.shader_data {
                    if location_matches(filter, value.cu_or_wgp, value.simd, value.wave_slot) {
                        emit(DecodedAttQueryItemV1::ShaderData { data: *value });
                    }
                }
            }
            DecodedAttListKindV1::Realtime => {
                for value in &self.capture.realtime {
                    emit(DecodedAttQueryItemV1::Realtime {
                        correlation: *value,
                    });
                }
            }
            DecodedAttListKindV1::Info => {
                for value in &self.capture.info {
                    emit(DecodedAttQueryItemV1::Info { info: *value });
                }
            }
        }
        Ok(())
    }
}

fn validate_filter(
    kind: DecodedAttListKindV1,
    filter: DecodedAttFilterV1,
) -> Result<(), DecodedAttQueryErrorV1> {
    if filter.simd.is_some_and(|simd| simd > 3) {
        return Err(DecodedAttQueryErrorV1::InvalidFilter);
    }
    let location =
        filter.cu_or_wgp.is_some() || filter.simd.is_some() || filter.wave_slot.is_some();
    let valid = match kind {
        DecodedAttListKindV1::RawReferences
        | DecodedAttListKindV1::Realtime
        | DecodedAttListKindV1::Info => {
            !location
                && filter.code_object.is_none()
                && filter.instruction_category.is_none()
                && filter.wave_state.is_none()
        }
        DecodedAttListKindV1::CodeObjects => {
            !location && filter.instruction_category.is_none() && filter.wave_state.is_none()
        }
        DecodedAttListKindV1::Occupancy => {
            filter.instruction_category.is_none() && filter.wave_state.is_none()
        }
        DecodedAttListKindV1::Waves | DecodedAttListKindV1::ShaderData => {
            filter.code_object.is_none()
                && filter.instruction_category.is_none()
                && filter.wave_state.is_none()
        }
        DecodedAttListKindV1::WaveStates => {
            filter.code_object.is_none() && filter.instruction_category.is_none()
        }
        DecodedAttListKindV1::Instructions => filter.wave_state.is_none(),
        DecodedAttListKindV1::PerfEvents => {
            filter.simd.is_none()
                && filter.wave_slot.is_none()
                && filter.code_object.is_none()
                && filter.instruction_category.is_none()
                && filter.wave_state.is_none()
        }
    };
    if valid {
        Ok(())
    } else {
        Err(DecodedAttQueryErrorV1::InvalidFilter)
    }
}

fn location_matches(filter: DecodedAttFilterV1, cu: u8, simd: u8, wave: u8) -> bool {
    filter.cu_or_wgp.is_none_or(|value| value == cu)
        && filter.simd.is_none_or(|value| value == simd)
        && filter.wave_slot.is_none_or(|value| value == wave)
}

fn pc_matches(filter: DecodedAttFilterV1, code_object: Option<CaptureIdentityV1>) -> bool {
    filter
        .code_object
        .is_none_or(|value| code_object == Some(value))
}

fn capability(
    available: bool,
    reason: &'static str,
) -> (DecodedAttAvailabilityV1, Option<&'static str>) {
    if available {
        (DecodedAttAvailabilityV1::Available, None)
    } else {
        (DecodedAttAvailabilityV1::Unavailable, Some(reason))
    }
}

fn named_capability(
    name: DecodedAttCapabilityNameV1,
    value: (DecodedAttAvailabilityV1, Option<&'static str>),
) -> DecodedAttCapabilityV1 {
    DecodedAttCapabilityV1 {
        name,
        availability: value.0,
        reason: value.1,
    }
}

fn unavailable(name: DecodedAttCapabilityNameV1, reason: &'static str) -> DecodedAttCapabilityV1 {
    DecodedAttCapabilityV1 {
        name,
        availability: DecodedAttAvailabilityV1::Unavailable,
        reason: Some(reason),
    }
}

fn cursor_binding(
    capture: CaptureIdentityV1,
    kind: DecodedAttListKindV1,
    filter: DecodedAttFilterV1,
) -> Result<CaptureIdentityV1, DecodedAttQueryErrorV1> {
    let mut hasher = Sha256::new();
    hasher.update(DECODED_ATT_CURSOR_DOMAIN_V1);
    hasher.update(capture.as_bytes());
    hasher.update([list_kind_tag(kind)]);
    hash_optional_u8(&mut hasher, filter.cu_or_wgp);
    hash_optional_u8(&mut hasher, filter.simd);
    hash_optional_u8(&mut hasher, filter.wave_slot);
    match filter.code_object {
        Some(identity) => {
            hasher.update([1]);
            hasher.update(identity.as_bytes());
        }
        None => hasher.update([0]),
    }
    hash_optional_u8(
        &mut hasher,
        filter.instruction_category.map(instruction_category_tag),
    );
    hash_optional_u8(&mut hasher, filter.wave_state.map(wave_state_tag));
    CaptureIdentityV1::new(hasher.finalize().into()).map_err(|_| DecodedAttQueryErrorV1::Identity)
}

fn hash_optional_u8(hasher: &mut Sha256, value: Option<u8>) {
    match value {
        Some(value) => hasher.update([1, value]),
        None => hasher.update([0, 0]),
    }
}

const fn list_kind_tag(kind: DecodedAttListKindV1) -> u8 {
    match kind {
        DecodedAttListKindV1::RawReferences => 0,
        DecodedAttListKindV1::CodeObjects => 1,
        DecodedAttListKindV1::Occupancy => 2,
        DecodedAttListKindV1::Waves => 3,
        DecodedAttListKindV1::WaveStates => 4,
        DecodedAttListKindV1::Instructions => 5,
        DecodedAttListKindV1::PerfEvents => 6,
        DecodedAttListKindV1::ShaderData => 7,
        DecodedAttListKindV1::Realtime => 8,
        DecodedAttListKindV1::Info => 9,
    }
}

const fn instruction_category_tag(kind: DecodedAttInstructionCategoryV1) -> u8 {
    match kind {
        DecodedAttInstructionCategoryV1::None => 0,
        DecodedAttInstructionCategoryV1::Smem => 1,
        DecodedAttInstructionCategoryV1::Salu => 2,
        DecodedAttInstructionCategoryV1::Vmem => 3,
        DecodedAttInstructionCategoryV1::Flat => 4,
        DecodedAttInstructionCategoryV1::Lds => 5,
        DecodedAttInstructionCategoryV1::Valu => 6,
        DecodedAttInstructionCategoryV1::Jump => 7,
        DecodedAttInstructionCategoryV1::Next => 8,
        DecodedAttInstructionCategoryV1::Immed => 9,
        DecodedAttInstructionCategoryV1::Context => 10,
        DecodedAttInstructionCategoryV1::Message => 11,
        DecodedAttInstructionCategoryV1::Bvh => 12,
    }
}

const fn wave_state_tag(kind: DecodedAttWaveStateKindV1) -> u8 {
    match kind {
        DecodedAttWaveStateKindV1::Empty => 0,
        DecodedAttWaveStateKindV1::Idle => 1,
        DecodedAttWaveStateKindV1::Exec => 2,
        DecodedAttWaveStateKindV1::Wait => 3,
        DecodedAttWaveStateKindV1::Stall => 4,
    }
}

struct DecodedAttBoundedWriterV1<'a> {
    output: &'a mut Vec<u8>,
    maximum: u64,
    exceeded: bool,
    allocation_failed: bool,
}

impl Write for DecodedAttBoundedWriterV1<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let next = u64::try_from(self.output.len()).ok().and_then(|value| {
            u64::try_from(bytes.len())
                .ok()
                .and_then(|bytes| value.checked_add(bytes))
        });
        if next.is_none_or(|value| value > self.maximum) {
            self.exceeded = true;
            return Err(std::io::Error::other("decoded ATT response limit exceeded"));
        }
        if self.output.try_reserve_exact(bytes.len()).is_err() {
            self.allocation_failed = true;
            return Err(std::io::Error::other(
                "decoded ATT response allocation failed",
            ));
        }
        self.output.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum DecodedAttQueryErrorV1 {
    LimitOutOfRange,
    InputTooLarge,
    PageLimit,
    InvalidFilter,
    CursorMismatch,
    CursorOutOfRange,
    ResponseTooLarge,
    AllocationFailure,
    SizeOverflow,
    Identity,
    Json,
    Capture(fe2o3_semantic_import::DecodedAttErrorV1),
}

impl fmt::Display for DecodedAttQueryErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "decoded ATT query rejected: {self:?}")
    }
}
impl Error for DecodedAttQueryErrorV1 {}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentDecodedAttRequestV1 {
    Open {
        #[serde(deserialize_with = "deserialize_agent_schema")]
        schema: String,
        request_id: u64,
        revision: u64,
        #[serde(deserialize_with = "deserialize_interchange_hex")]
        interchange_hex: String,
    },
    Capabilities {
        #[serde(deserialize_with = "deserialize_agent_schema")]
        schema: String,
        request_id: u64,
        revision: u64,
    },
    Query {
        #[serde(deserialize_with = "deserialize_agent_schema")]
        schema: String,
        request_id: u64,
        revision: u64,
        kind: DecodedAttListKindV1,
        page: DecodedAttPageRequestV1,
    },
    Close {
        #[serde(deserialize_with = "deserialize_agent_schema")]
        schema: String,
        request_id: u64,
        revision: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum AgentDecodedAttResultV1 {
    Open { response: DecodedAttQueryResponseV1 },
    Capabilities { response: DecodedAttQueryResponseV1 },
    Query { response: DecodedAttQueryResponseV1 },
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentDecodedAttErrorCodeV1 {
    InvalidRequest,
    InvalidSchema,
    RevisionMismatch,
    SessionNotOpen,
    SessionAlreadyOpen,
    EvidenceRejected,
    QueryRejected,
    RequestTooLarge,
    ResponseTooLarge,
    InvalidRequestId,
    DuplicateRequestId,
    RequestAttemptLimit,
    RevisionExhausted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "response", rename_all = "snake_case")]
pub enum AgentDecodedAttResponseV1 {
    Success {
        schema: &'static str,
        request_id: u64,
        revision: u64,
        terminal: bool,
        value: Box<AgentDecodedAttResultV1>,
    },
    Error {
        schema: &'static str,
        request_id: Option<u64>,
        revision: u64,
        terminal: bool,
        code: AgentDecodedAttErrorCodeV1,
    },
}

struct AgentDecodedAttServiceV1 {
    revision: u64,
    request_attempts: u64,
    request_ids: Vec<u64>,
    session: Option<DecodedAttQuerySessionV1>,
    terminal: bool,
}

impl AgentDecodedAttServiceV1 {
    fn new() -> Self {
        Self {
            revision: 0,
            request_attempts: 0,
            request_ids: Vec::new(),
            session: None,
            terminal: false,
        }
    }

    fn handle(&mut self, request: AgentDecodedAttRequestV1) -> AgentDecodedAttResponseV1 {
        let (schema, request_id, revision) = request_header(&request);
        if request_id == 0 {
            return self.error(
                Some(request_id),
                AgentDecodedAttErrorCodeV1::InvalidRequestId,
                false,
            );
        }
        if self.request_ids.contains(&request_id) {
            return self.error(
                Some(request_id),
                AgentDecodedAttErrorCodeV1::DuplicateRequestId,
                false,
            );
        }
        if self.request_ids.try_reserve_exact(1).is_err() {
            return self.error(
                Some(request_id),
                AgentDecodedAttErrorCodeV1::ResponseTooLarge,
                true,
            );
        }
        self.request_ids.push(request_id);
        if schema != AGENT_DECODED_ATT_REQUEST_SCHEMA_V1 {
            return self.error(
                Some(request_id),
                AgentDecodedAttErrorCodeV1::InvalidSchema,
                false,
            );
        }
        if revision != self.revision {
            return self.error(
                Some(request_id),
                AgentDecodedAttErrorCodeV1::RevisionMismatch,
                false,
            );
        }
        match request {
            AgentDecodedAttRequestV1::Open {
                interchange_hex, ..
            } => {
                if self.session.is_some() {
                    return self.error(
                        Some(request_id),
                        AgentDecodedAttErrorCodeV1::SessionAlreadyOpen,
                        false,
                    );
                }
                let bytes = match decode_lower_hex(&interchange_hex) {
                    Ok(bytes) => bytes,
                    Err(()) => {
                        return self.error(
                            Some(request_id),
                            AgentDecodedAttErrorCodeV1::EvidenceRejected,
                            false,
                        );
                    }
                };
                let session = match DecodedAttQuerySessionV1::open(
                    &bytes,
                    DecodedAttQueryLimitsV1::default(),
                ) {
                    Ok(session) => session,
                    Err(_) => {
                        return self.error(
                            Some(request_id),
                            AgentDecodedAttErrorCodeV1::EvidenceRejected,
                            false,
                        );
                    }
                };
                let response =
                    match session
                        .query(DecodedAttQueryRequestV1::Open)
                        .and_then(|response| {
                            session.encode_response(&response)?;
                            Ok(response)
                        }) {
                        Ok(response) => response,
                        Err(DecodedAttQueryErrorV1::ResponseTooLarge) => {
                            return self.error(
                                Some(request_id),
                                AgentDecodedAttErrorCodeV1::ResponseTooLarge,
                                true,
                            );
                        }
                        Err(_) => {
                            return self.error(
                                Some(request_id),
                                AgentDecodedAttErrorCodeV1::QueryRejected,
                                false,
                            );
                        }
                    };
                self.session = Some(session);
                self.success(
                    request_id,
                    false,
                    AgentDecodedAttResultV1::Open { response },
                )
            }
            AgentDecodedAttRequestV1::Capabilities { .. } => {
                let Some(session) = &self.session else {
                    return self.error(
                        Some(request_id),
                        AgentDecodedAttErrorCodeV1::SessionNotOpen,
                        false,
                    );
                };
                match session
                    .query(DecodedAttQueryRequestV1::Capabilities)
                    .and_then(|response| {
                        session.encode_response(&response)?;
                        Ok(response)
                    }) {
                    Ok(response) => self.success(
                        request_id,
                        false,
                        AgentDecodedAttResultV1::Capabilities { response },
                    ),
                    Err(DecodedAttQueryErrorV1::ResponseTooLarge) => self.error(
                        Some(request_id),
                        AgentDecodedAttErrorCodeV1::ResponseTooLarge,
                        true,
                    ),
                    Err(_) => self.error(
                        Some(request_id),
                        AgentDecodedAttErrorCodeV1::QueryRejected,
                        false,
                    ),
                }
            }
            AgentDecodedAttRequestV1::Query { kind, page, .. } => {
                let Some(session) = &self.session else {
                    return self.error(
                        Some(request_id),
                        AgentDecodedAttErrorCodeV1::SessionNotOpen,
                        false,
                    );
                };
                match session
                    .query(DecodedAttQueryRequestV1::List { kind, page })
                    .and_then(|response| {
                        session.encode_response(&response)?;
                        Ok(response)
                    }) {
                    Ok(response) => self.success(
                        request_id,
                        false,
                        AgentDecodedAttResultV1::Query { response },
                    ),
                    Err(DecodedAttQueryErrorV1::ResponseTooLarge) => self.error(
                        Some(request_id),
                        AgentDecodedAttErrorCodeV1::ResponseTooLarge,
                        true,
                    ),
                    Err(_) => self.error(
                        Some(request_id),
                        AgentDecodedAttErrorCodeV1::QueryRejected,
                        false,
                    ),
                }
            }
            AgentDecodedAttRequestV1::Close { .. } => {
                if self.session.is_none() {
                    return self.error(
                        Some(request_id),
                        AgentDecodedAttErrorCodeV1::SessionNotOpen,
                        false,
                    );
                }
                self.session = None;
                self.success(request_id, true, AgentDecodedAttResultV1::Closed)
            }
        }
    }

    fn success(
        &mut self,
        request_id: u64,
        terminal: bool,
        value: AgentDecodedAttResultV1,
    ) -> AgentDecodedAttResponseV1 {
        let Some(revision) = self.revision.checked_add(1) else {
            return self.error(
                Some(request_id),
                AgentDecodedAttErrorCodeV1::RevisionExhausted,
                true,
            );
        };
        self.revision = revision;
        self.terminal = terminal;
        AgentDecodedAttResponseV1::Success {
            schema: AGENT_DECODED_ATT_RESPONSE_SCHEMA_V1,
            request_id,
            revision: self.revision,
            terminal,
            value: Box::new(value),
        }
    }

    fn error(
        &mut self,
        request_id: Option<u64>,
        code: AgentDecodedAttErrorCodeV1,
        terminal: bool,
    ) -> AgentDecodedAttResponseV1 {
        self.terminal |= terminal;
        AgentDecodedAttResponseV1::Error {
            schema: AGENT_DECODED_ATT_RESPONSE_SCHEMA_V1,
            request_id,
            revision: self.revision,
            terminal,
            code,
        }
    }

    fn begin_attempt(&mut self) -> Result<(), AgentDecodedAttResponseV1> {
        if self.request_attempts >= MAX_AGENT_DECODED_ATT_REQUEST_ATTEMPTS_V1 {
            return Err(self.error(None, AgentDecodedAttErrorCodeV1::RequestAttemptLimit, true));
        }
        self.request_attempts = self.request_attempts.checked_add(1).ok_or_else(|| {
            self.error(None, AgentDecodedAttErrorCodeV1::RequestAttemptLimit, true)
        })?;
        Ok(())
    }
}

fn request_header(request: &AgentDecodedAttRequestV1) -> (&str, u64, u64) {
    match request {
        AgentDecodedAttRequestV1::Open {
            schema,
            request_id,
            revision,
            ..
        }
        | AgentDecodedAttRequestV1::Capabilities {
            schema,
            request_id,
            revision,
        }
        | AgentDecodedAttRequestV1::Query {
            schema,
            request_id,
            revision,
            ..
        }
        | AgentDecodedAttRequestV1::Close {
            schema,
            request_id,
            revision,
        } => (schema, *request_id, *revision),
    }
}

fn decode_lower_hex(value: &str) -> Result<Vec<u8>, ()> {
    if !value.len().is_multiple_of(2)
        || u64::try_from(value.len()).map_err(|_| ())? > MAX_DECODED_ATT_INTERCHANGE_BYTES_V1 * 2
    {
        return Err(());
    }
    let mut output = Vec::new();
    output.try_reserve_exact(value.len() / 2).map_err(|_| ())?;
    for pair in value.as_bytes().chunks_exact(2) {
        let high = lower_hex_nibble(pair[0]).ok_or(())?;
        let low = lower_hex_nibble(pair[1]).ok_or(())?;
        output.push((high << 4) | low);
    }
    Ok(output)
}

fn lower_hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn deserialize_agent_schema<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_string(deserializer, 128, "agent schema")
}

fn deserialize_interchange_hex<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_string(
        deserializer,
        usize::try_from(MAX_DECODED_ATT_INTERCHANGE_BYTES_V1 * 2).unwrap_or(usize::MAX),
        "decoded ATT interchange hex",
    )
}

fn deserialize_bounded_string<'de, D>(
    deserializer: D,
    maximum: usize,
    label: &'static str,
) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct BoundedStringVisitorV1 {
        maximum: usize,
        label: &'static str,
    }
    impl Visitor<'_> for BoundedStringVisitorV1 {
        type Value = String;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "a {} no longer than {} bytes",
                self.label, self.maximum
            )
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if value.len() > self.maximum || !value.is_ascii() {
                return Err(E::custom("bounded agent string rejected"));
            }
            let mut output = String::new();
            output
                .try_reserve_exact(value.len())
                .map_err(|_| E::custom("bounded agent string allocation failed"))?;
            output.push_str(value);
            Ok(output)
        }
    }
    deserializer.deserialize_str(BoundedStringVisitorV1 { maximum, label })
}

pub fn run_agent_decoded_att_jsonl_v1<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
) -> Result<(), AgentDecodedAttServiceErrorV1> {
    run_agent_decoded_att_jsonl_with_limit_v1(input, output, MAX_AGENT_DECODED_ATT_REQUEST_BYTES_V1)
}

fn run_agent_decoded_att_jsonl_with_limit_v1<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    request_bytes: u64,
) -> Result<(), AgentDecodedAttServiceErrorV1> {
    let mut service = AgentDecodedAttServiceV1::new();
    let mut line = Vec::new();
    loop {
        line.clear();
        let mut bounded = Read::take(&mut *input, request_bytes.saturating_add(2));
        let read = bounded
            .read_until(b'\n', &mut line)
            .map_err(|_| AgentDecodedAttServiceErrorV1::Io)?;
        if read == 0 {
            return Ok(());
        }
        if let Err(response) = service.begin_attempt() {
            write_agent_response(output, &response)?;
            return Ok(());
        }
        if line.last() != Some(&b'\n')
            || u64::try_from(line.len()).unwrap_or(u64::MAX) > request_bytes.saturating_add(1)
        {
            let response = service.error(None, AgentDecodedAttErrorCodeV1::RequestTooLarge, true);
            write_agent_response(output, &response)?;
            return Ok(());
        }
        line.pop();
        let request: AgentDecodedAttRequestV1 = match serde_json::from_slice(&line) {
            Ok(request)
                if encode_agent_json(&request, MAX_AGENT_DECODED_ATT_REQUEST_BYTES_V1)
                    .ok()
                    .as_deref()
                    == Some(line.as_slice()) =>
            {
                request
            }
            _ => {
                let response =
                    service.error(None, AgentDecodedAttErrorCodeV1::InvalidRequest, false);
                write_agent_response(output, &response)?;
                continue;
            }
        };
        let response = service.handle(request);
        if write_agent_response_or_terminal(output, &mut service, &response)? {
            return Ok(());
        }
    }
}

fn write_agent_response_or_terminal(
    output: &mut impl Write,
    service: &mut AgentDecodedAttServiceV1,
    response: &AgentDecodedAttResponseV1,
) -> Result<bool, AgentDecodedAttServiceErrorV1> {
    match write_agent_response(output, response) {
        Ok(()) => Ok(service.terminal),
        Err(AgentDecodedAttServiceErrorV1::ResponseTooLarge)
        | Err(AgentDecodedAttServiceErrorV1::AllocationFailure) => {
            let terminal = service.error(None, AgentDecodedAttErrorCodeV1::ResponseTooLarge, true);
            write_agent_response(output, &terminal)?;
            Ok(true)
        }
        Err(error) => Err(error),
    }
}

fn write_agent_response(
    output: &mut impl Write,
    response: &AgentDecodedAttResponseV1,
) -> Result<(), AgentDecodedAttServiceErrorV1> {
    let bytes = encode_agent_json(response, MAX_AGENT_DECODED_ATT_RESPONSE_BYTES_V1 - 1)?;
    output
        .write_all(&bytes)
        .map_err(|_| AgentDecodedAttServiceErrorV1::Io)?;
    output
        .write_all(b"\n")
        .map_err(|_| AgentDecodedAttServiceErrorV1::Io)?;
    output
        .flush()
        .map_err(|_| AgentDecodedAttServiceErrorV1::Io)
}

fn encode_agent_json(
    value: &impl Serialize,
    maximum: u64,
) -> Result<Vec<u8>, AgentDecodedAttServiceErrorV1> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(
            usize::try_from(maximum.min(64 * 1024))
                .map_err(|_| AgentDecodedAttServiceErrorV1::ResponseTooLarge)?,
        )
        .map_err(|_| AgentDecodedAttServiceErrorV1::AllocationFailure)?;
    let mut writer = DecodedAttBoundedWriterV1 {
        output: &mut output,
        maximum,
        exceeded: false,
        allocation_failed: false,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| {
        if writer.exceeded {
            AgentDecodedAttServiceErrorV1::ResponseTooLarge
        } else if writer.allocation_failed {
            AgentDecodedAttServiceErrorV1::AllocationFailure
        } else {
            AgentDecodedAttServiceErrorV1::Json
        }
    })?;
    Ok(output)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentDecodedAttServiceErrorV1 {
    Io,
    Json,
    ResponseTooLarge,
    AllocationFailure,
}
impl fmt::Display for AgentDecodedAttServiceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "decoded ATT agent service failed: {self:?}")
    }
}
impl Error for AgentDecodedAttServiceErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_semantic_import::{
        ContentSchemeV1, DecodedAttAuthenticityV1, DecodedAttImportBindingV1,
        DecodedAttImportLimitsV1, ProfilerAttArtifactBindingV4, ProfilerAttBindingV4,
        ProfilerDeviceBindingV4, ProfilerEnvironmentBindingV4,
        ROCPROFILER_SDK_7_2_4_TRACE_DECODER_API_HEADER_BYTES_V1,
        ROCPROFILER_SDK_7_2_4_TRACE_DECODER_API_HEADER_SHA256_V1,
        ROCPROFILER_SDK_7_2_4_TRACE_DECODER_TYPES_HEADER_BYTES_V1,
        ROCPROFILER_SDK_7_2_4_TRACE_DECODER_TYPES_HEADER_SHA256_V1, encode_decoded_att_v1,
        encode_profiler_bundle_v4, import_rocprofiler_sdk_decoded_att_v1,
        import_rocprofv3_att_profiler_bundle_v4,
    };

    const EXPORT: &[u8] = include_bytes!(
        "../../fe2o3-semantic-import/tests/fixtures/rocprofiler-sdk-7.2.4-decoded-att-v1.json"
    );
    const MANIFEST: &[u8] = br#"{"counter_names":[],"gfxip":9,"gfxv":"vega","global_begin_time":0,"is_pcs_stochastic":false,"pc_sampling":false,"thread_trace":true,"version":"3.0.0","wave_filenames":{"0":{"0":{"0":{"0":["waves/se0.json",10,20]}}}},"se_filenames":["se0.json"]}"#;

    fn identity(byte: u8, len: u64, scheme: ContentSchemeV1) -> ContentIdentityRecordV1 {
        ContentIdentityRecordV1 {
            scheme,
            format_version: 1,
            digest: CaptureIdentityV1::new([byte; 32]).unwrap(),
            canonical_len: len,
        }
    }

    fn evidence_from_export(export: &[u8]) -> Vec<u8> {
        let bundle = import_rocprofv3_att_profiler_bundle_v4(
            MANIFEST,
            ProfilerAttBindingV4 {
                environment: ProfilerEnvironmentBindingV4 {
                    environment: identity(10, 200, ContentSchemeV1::DomainSeparatedSha256),
                    collector_tool: identity(11, 50, ContentSchemeV1::DomainSeparatedSha256),
                    collector_configuration: identity(
                        12,
                        80,
                        ContentSchemeV1::DomainSeparatedSha256,
                    ),
                    stable_device_bindings: vec![ProfilerDeviceBindingV4 {
                        source_agent_id: 17,
                        stable_identity: identity(20, 64, ContentSchemeV1::DomainSeparatedSha256),
                    }],
                },
                source_agent_id: 17,
                referenced_artifacts: vec![
                    ProfilerAttArtifactBindingV4 {
                        reference: "waves/se0.json".to_owned(),
                        content: identity(31, 401, ContentSchemeV1::DomainSeparatedSha256),
                    },
                    ProfilerAttArtifactBindingV4 {
                        reference: "se0.json".to_owned(),
                        content: identity(32, 402, ContentSchemeV1::DomainSeparatedSha256),
                    },
                ],
            },
        )
        .unwrap();
        let bundle = encode_profiler_bundle_v4(&bundle).unwrap();
        let decoded = import_rocprofiler_sdk_decoded_att_v1(
            export,
            &bundle,
            DecodedAttImportBindingV1 {
                trace_decoder_types_header: ContentIdentityRecordV1 {
                    scheme: ContentSchemeV1::RawCanonicalSha256,
                    format_version: 1,
                    digest: CaptureIdentityV1::new(
                        ROCPROFILER_SDK_7_2_4_TRACE_DECODER_TYPES_HEADER_SHA256_V1,
                    )
                    .unwrap(),
                    canonical_len: ROCPROFILER_SDK_7_2_4_TRACE_DECODER_TYPES_HEADER_BYTES_V1,
                },
                trace_decoder_api_header: ContentIdentityRecordV1 {
                    scheme: ContentSchemeV1::RawCanonicalSha256,
                    format_version: 1,
                    digest: CaptureIdentityV1::new(
                        ROCPROFILER_SDK_7_2_4_TRACE_DECODER_API_HEADER_SHA256_V1,
                    )
                    .unwrap(),
                    canonical_len: ROCPROFILER_SDK_7_2_4_TRACE_DECODER_API_HEADER_BYTES_V1,
                },
                decoder_library: identity(50, 50_000, ContentSchemeV1::RawCanonicalSha256),
                exporter_tool: identity(51, 25_000, ContentSchemeV1::RawCanonicalSha256),
            },
            DecodedAttImportLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(
            decoded.decoder.authenticity,
            DecodedAttAuthenticityV1::UnavailableSelfClaimedExternalDecoder
        );
        encode_decoded_att_v1(&decoded).unwrap()
    }

    fn evidence() -> Vec<u8> {
        let mut export = EXPORT.to_vec();
        assert_eq!(export.pop(), Some(b'\n'));
        evidence_from_export(&export)
    }

    #[test]
    fn pages_instruction_coordinates_with_content_bound_cursors() {
        let bytes = evidence();
        let session =
            DecodedAttQuerySessionV1::open(&bytes, DecodedAttQueryLimitsV1::default()).unwrap();
        let first = session
            .query(DecodedAttQueryRequestV1::List {
                kind: DecodedAttListKindV1::Instructions,
                page: DecodedAttPageRequestV1 {
                    limit: 2,
                    cursor: None,
                    filter: DecodedAttFilterV1 {
                        cu_or_wgp: Some(3),
                        ..DecodedAttFilterV1::default()
                    },
                },
            })
            .unwrap();
        let DecodedAttQueryResponseV1::Page { page: first } = first else {
            panic!("expected page");
        };
        assert_eq!(first.returned, 2);
        let cursor = first.next_cursor.unwrap();
        let DecodedAttQueryItemV1::Instruction { instruction, .. } = &first.items[0] else {
            panic!("expected instruction");
        };
        assert!(instruction.pc.code_object.is_some());
        assert_eq!(instruction.pc.elf_virtual_address, Some(256));

        let second = session
            .query(DecodedAttQueryRequestV1::List {
                kind: DecodedAttListKindV1::Instructions,
                page: DecodedAttPageRequestV1 {
                    limit: 2,
                    cursor: Some(cursor),
                    filter: DecodedAttFilterV1 {
                        cu_or_wgp: Some(3),
                        ..DecodedAttFilterV1::default()
                    },
                },
            })
            .unwrap();
        let DecodedAttQueryResponseV1::Page { page: second } = second else {
            panic!("expected page");
        };
        assert_eq!(second.returned, 2);
        assert_ne!(first.items, second.items);

        assert!(matches!(
            session.query(DecodedAttQueryRequestV1::List {
                kind: DecodedAttListKindV1::Instructions,
                page: DecodedAttPageRequestV1 {
                    limit: 2,
                    cursor: Some(cursor),
                    filter: DecodedAttFilterV1 {
                        simd: Some(2),
                        ..DecodedAttFilterV1::default()
                    },
                },
            }),
            Err(DecodedAttQueryErrorV1::CursorMismatch)
        ));

        let filter = DecodedAttFilterV1::default();
        let late = session
            .query(DecodedAttQueryRequestV1::List {
                kind: DecodedAttListKindV1::Instructions,
                page: DecodedAttPageRequestV1 {
                    limit: 1,
                    cursor: Some(DecodedAttCursorV1 {
                        query_binding: cursor_binding(
                            session.context().interchange.digest,
                            DecodedAttListKindV1::Instructions,
                            filter,
                        )
                        .unwrap(),
                        position: 12,
                    }),
                    filter,
                },
            })
            .unwrap();
        let DecodedAttQueryResponseV1::Page { page: late } = late else {
            panic!("expected late page");
        };
        assert_eq!(late.returned, 1);
        assert!(late.next_cursor.is_none());
        let DecodedAttQueryItemV1::Instruction { instruction, .. } = &late.items[0] else {
            panic!("expected late instruction");
        };
        assert_eq!(instruction.pc.elf_virtual_address, Some(304));

        let waves = session
            .query(DecodedAttQueryRequestV1::List {
                kind: DecodedAttListKindV1::Waves,
                page: DecodedAttPageRequestV1::default(),
            })
            .unwrap();
        let encoded = session.encode_response(&waves).unwrap();
        let text = std::str::from_utf8(&encoded).unwrap();
        assert!(!text.contains("timeline"));
        assert!(!text.contains("instructions"));
    }

    #[test]
    fn reports_unavailable_authority_and_rejects_inapplicable_filters() {
        let bytes = evidence();
        let session =
            DecodedAttQuerySessionV1::open(&bytes, DecodedAttQueryLimitsV1::default()).unwrap();
        let response = session
            .query(DecodedAttQueryRequestV1::Capabilities)
            .unwrap();
        let DecodedAttQueryResponseV1::Capabilities { capabilities, .. } = response else {
            panic!("expected capabilities");
        };
        for name in [
            DecodedAttCapabilityNameV1::SourceMirKirLlvmIsaCorrelation,
            DecodedAttCapabilityNameV1::AuthenticatedDecoderCustody,
            DecodedAttCapabilityNameV1::RawAttDecode,
            DecodedAttCapabilityNameV1::Collection,
            DecodedAttCapabilityNameV1::ExecutionControl,
        ] {
            assert!(capabilities.iter().any(|value| {
                value.name == name && value.availability == DecodedAttAvailabilityV1::Unavailable
            }));
        }
        assert!(matches!(
            session.query(DecodedAttQueryRequestV1::List {
                kind: DecodedAttListKindV1::Realtime,
                page: DecodedAttPageRequestV1 {
                    filter: DecodedAttFilterV1 {
                        cu_or_wgp: Some(1),
                        ..DecodedAttFilterV1::default()
                    },
                    ..DecodedAttPageRequestV1::default()
                },
            }),
            Err(DecodedAttQueryErrorV1::InvalidFilter)
        ));
    }

    #[test]
    fn absent_callback_class_is_unavailable_without_a_completeness_claim() {
        let mut export = EXPORT.to_vec();
        assert_eq!(export.pop(), Some(b'\n'));
        let export = String::from_utf8(export)
            .unwrap()
            .replacen(
                r#",{"record_type":"shaderdata","source_reference_ordinal":0,"records":[{"time":120,"value":4660,"cu":3,"simd":2,"wave_id":7,"flags":1,"reserved":0}]}"#,
                "",
                1,
            );
        assert!(!export.contains("shaderdata"));
        let session = DecodedAttQuerySessionV1::open(
            &evidence_from_export(export.as_bytes()),
            DecodedAttQueryLimitsV1::default(),
        )
        .unwrap();
        let DecodedAttQueryResponseV1::Capabilities {
            context,
            capabilities,
        } = session
            .query(DecodedAttQueryRequestV1::Capabilities)
            .unwrap()
        else {
            panic!("expected capabilities");
        };
        assert_eq!(
            context.completeness,
            DecodedAttCompletenessV1::IncompleteInfoReported
        );
        let shaderdata = capabilities
            .iter()
            .find(|value| value.name == DecodedAttCapabilityNameV1::ShaderData)
            .unwrap();
        assert_eq!(
            shaderdata.availability,
            DecodedAttAvailabilityV1::Unavailable
        );
        assert_eq!(
            shaderdata.reason,
            Some("admitted export contains no shaderdata records")
        );
    }

    fn lower_hex(bytes: &[u8]) -> String {
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use std::fmt::Write as _;
            write!(&mut output, "{byte:02x}").unwrap();
        }
        output
    }

    fn request(value: &AgentDecodedAttRequestV1, output: &mut Vec<u8>) {
        output.extend_from_slice(&serde_json::to_vec(value).unwrap());
        output.push(b'\n');
    }

    #[test]
    fn jsonl_service_is_revisioned_read_only_and_strict() {
        let bytes = evidence();
        let schema = AGENT_DECODED_ATT_REQUEST_SCHEMA_V1.to_owned();
        let mut input = Vec::new();
        request(
            &AgentDecodedAttRequestV1::Open {
                schema: schema.clone(),
                request_id: 1,
                revision: 0,
                interchange_hex: lower_hex(&bytes),
            },
            &mut input,
        );
        request(
            &AgentDecodedAttRequestV1::Capabilities {
                schema: schema.clone(),
                request_id: 2,
                revision: 1,
            },
            &mut input,
        );
        request(
            &AgentDecodedAttRequestV1::Query {
                schema: schema.clone(),
                request_id: 3,
                revision: 2,
                kind: DecodedAttListKindV1::WaveStates,
                page: DecodedAttPageRequestV1 {
                    limit: 1,
                    cursor: None,
                    filter: DecodedAttFilterV1::default(),
                },
            },
            &mut input,
        );
        request(
            &AgentDecodedAttRequestV1::Close {
                schema,
                request_id: 4,
                revision: 3,
            },
            &mut input,
        );
        let mut output = Vec::new();
        run_agent_decoded_att_jsonl_v1(&mut input.as_slice(), &mut output).unwrap();
        let lines = output
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 4);
        assert!(
            lines
                .iter()
                .all(|line| serde_json::from_slice::<serde_json::Value>(line).is_ok())
        );

        let mut hostile = br#"{"operation":"capabilities","schema":"fe2o3-decoded-att-agent-request-v1","request_id":1,"revision":0,"unknown":true}"#.to_vec();
        hostile.push(b'\n');
        let mut rejected = Vec::new();
        run_agent_decoded_att_jsonl_v1(&mut hostile.as_slice(), &mut rejected).unwrap();
        assert!(
            std::str::from_utf8(&rejected)
                .unwrap()
                .contains("invalid_request")
        );
    }

    #[test]
    fn agent_service_rejects_zero_duplicate_and_exhausted_revisions() {
        let schema = AGENT_DECODED_ATT_REQUEST_SCHEMA_V1.to_owned();
        let mut service = AgentDecodedAttServiceV1::new();
        service.begin_attempt().unwrap();
        assert!(matches!(
            service.handle(AgentDecodedAttRequestV1::Capabilities {
                schema: schema.clone(),
                request_id: 0,
                revision: 0,
            }),
            AgentDecodedAttResponseV1::Error {
                code: AgentDecodedAttErrorCodeV1::InvalidRequestId,
                ..
            }
        ));
        service.begin_attempt().unwrap();
        assert!(matches!(
            service.handle(AgentDecodedAttRequestV1::Capabilities {
                schema: schema.clone(),
                request_id: 7,
                revision: 0,
            }),
            AgentDecodedAttResponseV1::Error {
                code: AgentDecodedAttErrorCodeV1::SessionNotOpen,
                ..
            }
        ));
        service.begin_attempt().unwrap();
        assert!(matches!(
            service.handle(AgentDecodedAttRequestV1::Capabilities {
                schema,
                request_id: 7,
                revision: 0,
            }),
            AgentDecodedAttResponseV1::Error {
                code: AgentDecodedAttErrorCodeV1::DuplicateRequestId,
                ..
            }
        ));

        let mut exhausted = AgentDecodedAttServiceV1::new();
        exhausted.revision = u64::MAX;
        assert!(matches!(
            exhausted.success(9, false, AgentDecodedAttResultV1::Closed),
            AgentDecodedAttResponseV1::Error {
                code: AgentDecodedAttErrorCodeV1::RevisionExhausted,
                terminal: true,
                ..
            }
        ));
    }

    #[test]
    fn malformed_attempts_are_charged_and_terminal_at_the_bound() {
        let mut input = Vec::new();
        for _ in 0..=MAX_AGENT_DECODED_ATT_REQUEST_ATTEMPTS_V1 {
            input.extend_from_slice(b"{}\n");
        }
        let mut output = Vec::new();
        run_agent_decoded_att_jsonl_v1(&mut input.as_slice(), &mut output).unwrap();
        let records = output
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        assert_eq!(
            records.len() as u64,
            MAX_AGENT_DECODED_ATT_REQUEST_ATTEMPTS_V1 + 1
        );
        assert!(
            std::str::from_utf8(records.last().unwrap())
                .unwrap()
                .contains("request_attempt_limit")
        );
    }

    #[test]
    fn oversize_unterminated_record_is_terminal_and_never_reparsed() {
        let mut input = b"abcdef\n{}\n".as_slice();
        let mut output = Vec::new();
        run_agent_decoded_att_jsonl_with_limit_v1(&mut input, &mut output, 4).unwrap();
        let text = std::str::from_utf8(&output).unwrap();
        assert!(text.contains("request_too_large"));
        assert_eq!(text.lines().count(), 1);
    }

    #[test]
    fn oversize_query_and_envelope_return_small_typed_terminal_errors() {
        let bytes = evidence();
        let session = DecodedAttQuerySessionV1::open(
            &bytes,
            DecodedAttQueryLimitsV1::new(
                MAX_DECODED_ATT_INTERCHANGE_BYTES_V1,
                MIN_DECODED_ATT_QUERY_RESPONSE_BYTES_V1,
                128,
            )
            .unwrap(),
        )
        .unwrap();
        let mut service = AgentDecodedAttServiceV1::new();
        service.session = Some(session);
        service.begin_attempt().unwrap();
        let response = service.handle(AgentDecodedAttRequestV1::Query {
            schema: AGENT_DECODED_ATT_REQUEST_SCHEMA_V1.to_owned(),
            request_id: 1,
            revision: 0,
            kind: DecodedAttListKindV1::Instructions,
            page: DecodedAttPageRequestV1 {
                limit: 13,
                cursor: None,
                filter: DecodedAttFilterV1::default(),
            },
        });
        assert!(matches!(
            response,
            AgentDecodedAttResponseV1::Error {
                code: AgentDecodedAttErrorCodeV1::ResponseTooLarge,
                terminal: true,
                ..
            }
        ));

        let context = service.session.as_ref().unwrap().context();
        let large = AgentDecodedAttResponseV1::Success {
            schema: AGENT_DECODED_ATT_RESPONSE_SCHEMA_V1,
            request_id: 2,
            revision: 1,
            terminal: false,
            value: Box::new(AgentDecodedAttResultV1::Capabilities {
                response: DecodedAttQueryResponseV1::Capabilities {
                    context,
                    capabilities: vec![
                        unavailable(
                            DecodedAttCapabilityNameV1::RawAttDecode,
                            "intentionally repeated bounded response fixture",
                        );
                        40_000
                    ],
                },
            }),
        };
        let mut output = Vec::new();
        service.terminal = false;
        assert!(write_agent_response_or_terminal(&mut output, &mut service, &large).unwrap());
        let text = std::str::from_utf8(&output).unwrap();
        assert!(text.contains("response_too_large"));
        assert_eq!(text.lines().count(), 1);
    }
}
