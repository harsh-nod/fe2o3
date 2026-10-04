use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::io::{self, Write};

use fe2o3_semantic_import::{
    CaptureIdentityV1, ContentIdentityRecordV1, MAX_PC_SAMPLE_CAPTURE_BYTES_V3,
    PcInstructionTypeV3, PcSampleCaptureCoverageV3, PcSampleDispatchV3, PcSampleRecordV3,
    SemanticPcSampleCaptureV3, TruthOriginV1, decode_pc_sample_capture_v3,
    pc_sample_capture_content_identity_v3,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const MAX_PC_SAMPLE_QUERY_RESPONSE_BYTES_V3: u64 = 1024 * 1024;
pub const MAX_PC_SAMPLE_QUERY_PAGE_ITEMS_V3: u16 = 4096;
pub const MAX_PC_SAMPLE_HOTSPOT_GROUPS_V3: usize = 65_536;
const MIN_RESPONSE_BYTES_V3: u64 = 4096;
const CONSERVATIVE_ITEM_BYTES_V3: u64 = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PcSampleQueryLimitsV3 {
    pub max_input_bytes: u64,
    pub max_response_bytes: u64,
    pub max_page_items: u16,
}

impl Default for PcSampleQueryLimitsV3 {
    fn default() -> Self {
        Self {
            max_input_bytes: MAX_PC_SAMPLE_CAPTURE_BYTES_V3,
            max_response_bytes: MAX_PC_SAMPLE_QUERY_RESPONSE_BYTES_V3,
            max_page_items: MAX_PC_SAMPLE_QUERY_PAGE_ITEMS_V3,
        }
    }
}

impl PcSampleQueryLimitsV3 {
    pub fn new(input: u64, response: u64, page: u16) -> Result<Self, PcSampleQueryErrorV3> {
        if input == 0 || input > MAX_PC_SAMPLE_CAPTURE_BYTES_V3 {
            return Err(PcSampleQueryErrorV3::LimitOutOfRange);
        }
        if !(MIN_RESPONSE_BYTES_V3..=MAX_PC_SAMPLE_QUERY_RESPONSE_BYTES_V3).contains(&response)
            || page == 0
            || page > MAX_PC_SAMPLE_QUERY_PAGE_ITEMS_V3
        {
            return Err(PcSampleQueryErrorV3::LimitOutOfRange);
        }
        Ok(Self {
            max_input_bytes: input,
            max_response_bytes: response,
            max_page_items: page,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcSampleListKindV3 {
    Dispatches,
    Samples,
    PcHotspots,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PcSampleCursorV3 {
    pub query_binding: CaptureIdentityV1,
    pub position: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PcSamplePageRequestV3 {
    pub limit: u16,
    pub cursor: Option<PcSampleCursorV3>,
    pub dispatch_filter: Option<CaptureIdentityV1>,
    pub code_object_filter: Option<CaptureIdentityV1>,
}

impl Default for PcSamplePageRequestV3 {
    fn default() -> Self {
        Self {
            limit: 128,
            cursor: None,
            dispatch_filter: None,
            code_object_filter: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcSampleCapabilityNameV3 {
    Open,
    DispatchEnvelopes,
    RawPcSamples,
    SampledWaveLocations,
    PcHotspots,
    SourceCorrelation,
    IsaCorrelation,
    ClockConversion,
    AttWaveTimeline,
    CompleteInstructionTimeline,
    CrossCaptureComparison,
    ExecutionControl,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcSampleCapabilityAvailabilityV3 {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PcSampleCapabilityV3 {
    pub name: PcSampleCapabilityNameV3,
    pub availability: PcSampleCapabilityAvailabilityV3,
    pub reason: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PcSampleQueryContextV3 {
    pub capture_identity: ContentIdentityRecordV1,
    pub schema_version: u16,
    pub dispatch_count: u64,
    pub raw_sample_count: u64,
    pub relative_pc_unavailable_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PcSampleDispatchSummaryV3 {
    pub identity: CaptureIdentityV1,
    pub run_identity: CaptureIdentityV1,
    pub device_identity: CaptureIdentityV1,
    pub process_index: u32,
    pub dispatch_index: u32,
    pub source_dispatch_ordinal: u64,
    pub start_timestamp: u64,
    pub end_timestamp: u64,
    pub duration_ticks: u64,
    pub sample_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PcSampleHotspotV3 {
    pub rank: u64,
    pub dispatch_identity: CaptureIdentityV1,
    pub code_object_identity: CaptureIdentityV1,
    pub code_object_offset: u64,
    pub instruction_type: PcInstructionTypeV3,
    pub raw_sample_count: u64,
    pub origin: TruthOriginV1,
    pub aggregation: &'static str,
    pub limitation: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "item", rename_all = "snake_case")]
pub enum PcSampleQueryItemV3 {
    Dispatch { dispatch: PcSampleDispatchSummaryV3 },
    Sample { sample: PcSampleRecordV3 },
    PcHotspot { hotspot: PcSampleHotspotV3 },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PcSamplePageV3 {
    pub context: PcSampleQueryContextV3,
    pub kind: PcSampleListKindV3,
    pub returned: u16,
    pub next_cursor: Option<PcSampleCursorV3>,
    pub items: Vec<PcSampleQueryItemV3>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "response", rename_all = "snake_case")]
pub enum PcSampleQueryResponseV3 {
    Capabilities {
        context: PcSampleQueryContextV3,
        capabilities: Vec<PcSampleCapabilityV3>,
    },
    Open {
        context: PcSampleQueryContextV3,
        coverage: PcSampleCaptureCoverageV3,
    },
    Page {
        page: PcSamplePageV3,
    },
    InspectDispatch {
        context: PcSampleQueryContextV3,
        dispatch: Box<PcSampleDispatchV3>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PcSampleQueryRequestV3 {
    Capabilities,
    Open,
    List {
        kind: PcSampleListKindV3,
        page: PcSamplePageRequestV3,
    },
    InspectDispatch {
        identity: CaptureIdentityV1,
    },
}

pub struct PcSampleQuerySessionV3 {
    capture: SemanticPcSampleCaptureV3,
    context: PcSampleQueryContextV3,
    limits: PcSampleQueryLimitsV3,
}

impl PcSampleQuerySessionV3 {
    pub fn open(bytes: &[u8], limits: PcSampleQueryLimitsV3) -> Result<Self, PcSampleQueryErrorV3> {
        let actual = u64::try_from(bytes.len()).map_err(|_| PcSampleQueryErrorV3::SizeOverflow)?;
        if actual > limits.max_input_bytes {
            return Err(PcSampleQueryErrorV3::InputTooLarge);
        }
        let capture = decode_pc_sample_capture_v3(bytes).map_err(PcSampleQueryErrorV3::Capture)?;
        let relative_pc_unavailable_count = capture
            .samples
            .iter()
            .try_fold(0_u64, |count, sample| {
                count.checked_add(u64::from(sample.pc.code_object_identity.is_none()))
            })
            .ok_or(PcSampleQueryErrorV3::SizeOverflow)?;
        let context = PcSampleQueryContextV3 {
            capture_identity: pc_sample_capture_content_identity_v3(bytes)
                .map_err(PcSampleQueryErrorV3::Capture)?,
            schema_version: capture.schema_version,
            dispatch_count: u64::try_from(capture.dispatches.len())
                .map_err(|_| PcSampleQueryErrorV3::SizeOverflow)?,
            raw_sample_count: u64::try_from(capture.samples.len())
                .map_err(|_| PcSampleQueryErrorV3::SizeOverflow)?,
            relative_pc_unavailable_count,
        };
        Ok(Self {
            capture,
            context,
            limits,
        })
    }

    pub fn query(
        &self,
        request: PcSampleQueryRequestV3,
    ) -> Result<PcSampleQueryResponseV3, PcSampleQueryErrorV3> {
        match request {
            PcSampleQueryRequestV3::Capabilities => Ok(PcSampleQueryResponseV3::Capabilities {
                context: self.context,
                capabilities: capabilities(),
            }),
            PcSampleQueryRequestV3::Open => Ok(PcSampleQueryResponseV3::Open {
                context: self.context,
                coverage: self.capture.coverage,
            }),
            PcSampleQueryRequestV3::InspectDispatch { identity } => {
                let dispatch = self
                    .capture
                    .dispatches
                    .iter()
                    .find(|dispatch| dispatch.identity == identity)
                    .cloned()
                    .ok_or(PcSampleQueryErrorV3::DispatchNotFound)?;
                Ok(PcSampleQueryResponseV3::InspectDispatch {
                    context: self.context,
                    dispatch: Box::new(dispatch),
                })
            }
            PcSampleQueryRequestV3::List { kind, page } => Ok(PcSampleQueryResponseV3::Page {
                page: self.page(kind, page)?,
            }),
        }
    }

    pub fn query_json(
        &self,
        request: PcSampleQueryRequestV3,
    ) -> Result<Vec<u8>, PcSampleQueryErrorV3> {
        let response = self.query(request)?;
        let max = usize::try_from(self.limits.max_response_bytes)
            .map_err(|_| PcSampleQueryErrorV3::SizeOverflow)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(max)
            .map_err(|_| PcSampleQueryErrorV3::AllocationFailure)?;
        let mut writer = BoundedWriterV3 {
            output: &mut output,
            max: max.saturating_sub(1),
            exceeded: false,
        };
        if serde_json::to_writer(&mut writer, &response).is_err() {
            return Err(if writer.exceeded {
                PcSampleQueryErrorV3::ResponseTooLarge
            } else {
                PcSampleQueryErrorV3::JsonEncode
            });
        }
        output.push(b'\n');
        Ok(output)
    }

    fn page(
        &self,
        kind: PcSampleListKindV3,
        page: PcSamplePageRequestV3,
    ) -> Result<PcSamplePageV3, PcSampleQueryErrorV3> {
        if page.limit == 0
            || page.limit > self.limits.max_page_items
            || u64::from(page.limit).saturating_mul(CONSERVATIVE_ITEM_BYTES_V3)
                > self.limits.max_response_bytes
        {
            return Err(PcSampleQueryErrorV3::LimitOutOfRange);
        }
        if kind == PcSampleListKindV3::Dispatches
            && (page.dispatch_filter.is_some() || page.code_object_filter.is_some())
        {
            return Err(PcSampleQueryErrorV3::FilterNotSupported);
        }
        let binding = query_binding(
            self.context.capture_identity.digest,
            kind,
            page.dispatch_filter,
            page.code_object_filter,
        )?;
        let start = match page.cursor {
            None => 0,
            Some(cursor) if cursor.query_binding == binding => usize::try_from(cursor.position)
                .map_err(|_| PcSampleQueryErrorV3::CursorOutOfRange)?,
            Some(_) => return Err(PcSampleQueryErrorV3::CursorQueryMismatch),
        };
        let take = usize::from(page.limit)
            .checked_add(1)
            .ok_or(PcSampleQueryErrorV3::SizeOverflow)?;
        let mut items = self.items(
            kind,
            page.dispatch_filter,
            page.code_object_filter,
            start,
            take,
        )?;
        let has_more = items.len() > usize::from(page.limit);
        if has_more {
            items.pop();
        }
        if start != 0 && items.is_empty() {
            return Err(PcSampleQueryErrorV3::CursorOutOfRange);
        }
        let end = start
            .checked_add(items.len())
            .ok_or(PcSampleQueryErrorV3::SizeOverflow)?;
        let next_cursor = has_more.then_some(PcSampleCursorV3 {
            query_binding: binding,
            position: u64::try_from(end).map_err(|_| PcSampleQueryErrorV3::SizeOverflow)?,
        });
        Ok(PcSamplePageV3 {
            context: self.context,
            kind,
            returned: u16::try_from(items.len()).map_err(|_| PcSampleQueryErrorV3::SizeOverflow)?,
            next_cursor,
            items,
        })
    }

    fn items(
        &self,
        kind: PcSampleListKindV3,
        dispatch_filter: Option<CaptureIdentityV1>,
        code_object_filter: Option<CaptureIdentityV1>,
        start: usize,
        take: usize,
    ) -> Result<Vec<PcSampleQueryItemV3>, PcSampleQueryErrorV3> {
        match kind {
            PcSampleListKindV3::Dispatches => self
                .capture
                .dispatches
                .iter()
                .skip(start)
                .take(take)
                .map(|dispatch| {
                    Ok(PcSampleQueryItemV3::Dispatch {
                        dispatch: PcSampleDispatchSummaryV3 {
                            identity: dispatch.identity,
                            run_identity: dispatch.run_identity,
                            device_identity: dispatch.device_identity,
                            process_index: dispatch.process_index,
                            dispatch_index: dispatch.dispatch_index,
                            source_dispatch_ordinal: dispatch.source_dispatch_ordinal,
                            start_timestamp: dispatch.start_timestamp,
                            end_timestamp: dispatch.end_timestamp,
                            duration_ticks: dispatch.duration_ticks,
                            sample_count: dispatch.sample_count,
                        },
                    })
                })
                .collect(),
            PcSampleListKindV3::Samples => Ok(self
                .capture
                .samples
                .iter()
                .filter(|sample| {
                    dispatch_filter.is_none_or(|identity| identity == sample.dispatch_identity)
                        && code_object_filter
                            .is_none_or(|identity| sample.pc.code_object_identity == Some(identity))
                })
                .skip(start)
                .take(take)
                .copied()
                .map(|sample| PcSampleQueryItemV3::Sample { sample })
                .collect()),
            PcSampleListKindV3::PcHotspots => {
                let mut groups = BTreeMap::<
                    (
                        CaptureIdentityV1,
                        CaptureIdentityV1,
                        u64,
                        PcInstructionTypeV3,
                    ),
                    u64,
                >::new();
                for sample in &self.capture.samples {
                    if dispatch_filter.is_some_and(|identity| identity != sample.dispatch_identity)
                    {
                        continue;
                    }
                    let (Some(code_object), Some(offset)) =
                        (sample.pc.code_object_identity, sample.pc.code_object_offset)
                    else {
                        continue;
                    };
                    if code_object_filter.is_some_and(|identity| identity != code_object) {
                        continue;
                    }
                    let key = (
                        sample.dispatch_identity,
                        code_object,
                        offset,
                        sample.instruction_type,
                    );
                    if !groups.contains_key(&key) && groups.len() == MAX_PC_SAMPLE_HOTSPOT_GROUPS_V3
                    {
                        return Err(PcSampleQueryErrorV3::TooManyHotspotGroups);
                    }
                    let count = groups.entry(key).or_default();
                    *count = count
                        .checked_add(1)
                        .ok_or(PcSampleQueryErrorV3::SizeOverflow)?;
                }
                let mut groups: Vec<_> = groups.into_iter().collect();
                groups
                    .sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
                groups
                    .into_iter()
                    .enumerate()
                    .skip(start)
                    .take(take)
                    .map(|(rank, (key, count))| {
                        Ok(PcSampleQueryItemV3::PcHotspot {
                            hotspot: PcSampleHotspotV3 {
                                rank: u64::try_from(
                                    rank.checked_add(1)
                                        .ok_or(PcSampleQueryErrorV3::SizeOverflow)?,
                                )
                                .map_err(|_| PcSampleQueryErrorV3::SizeOverflow)?,
                                dispatch_identity: key.0,
                                code_object_identity: key.1,
                                code_object_offset: key.2,
                                instruction_type: key.3,
                                raw_sample_count: count,
                                origin: TruthOriginV1::Inferred,
                                aggregation:
                                    "count_stochastic_records_by_dispatch_code_object_pc_and_instruction_type",
                                limitation:
                                    "sample_count_is_not_instruction_count_or_complete_execution_coverage",
                            },
                        })
                    })
                    .collect()
            }
        }
    }
}

fn query_binding(
    capture: CaptureIdentityV1,
    kind: PcSampleListKindV3,
    dispatch: Option<CaptureIdentityV1>,
    code_object: Option<CaptureIdentityV1>,
) -> Result<CaptureIdentityV1, PcSampleQueryErrorV3> {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3.semantic-pc-sample-capture.query.v3\0");
    digest.update(capture.as_bytes());
    digest.update([kind as u8]);
    digest.update(dispatch.map(CaptureIdentityV1::as_bytes).unwrap_or([0; 32]));
    digest.update(
        code_object
            .map(CaptureIdentityV1::as_bytes)
            .unwrap_or([0; 32]),
    );
    CaptureIdentityV1::new(digest.finalize().into())
        .map_err(|_| PcSampleQueryErrorV3::IdentityFailure)
}

fn capabilities() -> Vec<PcSampleCapabilityV3> {
    use PcSampleCapabilityAvailabilityV3::{Available, Unavailable};
    use PcSampleCapabilityNameV3::*;
    [
        (Open, Available, None),
        (DispatchEnvelopes, Available, None),
        (RawPcSamples, Available, None),
        (SampledWaveLocations, Available, None),
        (PcHotspots, Available, None),
        (
            SourceCorrelation,
            Unavailable,
            Some("no authenticated PC-to-source map correlation"),
        ),
        (
            IsaCorrelation,
            Unavailable,
            Some("only code-object-relative PC evidence is admitted"),
        ),
        (
            ClockConversion,
            Unavailable,
            Some("rocprofiler timestamp unit and cross-capture clock identity are unavailable"),
        ),
        (
            AttWaveTimeline,
            Unavailable,
            Some(
                "ATT requires a separate capture and a decoder available on the collection host; decoded events are not in PC Capture V3",
            ),
        ),
        (
            CompleteInstructionTimeline,
            Unavailable,
            Some("stochastic PC records are samples, not a complete instruction timeline"),
        ),
        (
            CrossCaptureComparison,
            Unavailable,
            Some("run, device, code-object, and dispatch identities are source-bound"),
        ),
        (
            ExecutionControl,
            Unavailable,
            Some("read-only query surface"),
        ),
    ]
    .into_iter()
    .map(|(name, availability, reason)| PcSampleCapabilityV3 {
        name,
        availability,
        reason,
    })
    .collect()
}

struct BoundedWriterV3<'a> {
    output: &'a mut Vec<u8>,
    max: usize,
    exceeded: bool,
}

impl Write for BoundedWriterV3<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.write_all(bytes)?;
        Ok(bytes.len())
    }

    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self
            .output
            .len()
            .checked_add(bytes.len())
            .is_none_or(|length| length > self.max)
        {
            self.exceeded = true;
            return Err(io::Error::other("PC sample query response limit exceeded"));
        }
        self.output.extend_from_slice(bytes);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub enum PcSampleQueryErrorV3 {
    LimitOutOfRange,
    InputTooLarge,
    CursorQueryMismatch,
    CursorOutOfRange,
    FilterNotSupported,
    DispatchNotFound,
    TooManyHotspotGroups,
    SizeOverflow,
    IdentityFailure,
    AllocationFailure,
    JsonEncode,
    ResponseTooLarge,
    Capture(fe2o3_semantic_import::PcSampleCaptureErrorV3),
}

impl fmt::Display for PcSampleQueryErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "semantic PC sample capture query rejected: {self:?}"
        )
    }
}

impl Error for PcSampleQueryErrorV3 {}
