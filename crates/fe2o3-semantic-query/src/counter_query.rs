use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::io::{self, Write};

use fe2o3_semantic_import::{
    CaptureIdentityV1, ContentIdentityRecordV1, CounterDefinitionV2, CounterDispatchV2,
    CounterValueV2, MAX_COUNTER_CAPTURE_BYTES_V2, SemanticCounterCaptureV2, TruthOriginV1,
    counter_capture_content_identity_v2, decode_counter_capture_v2,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const MAX_COUNTER_QUERY_RESPONSE_BYTES_V2: u64 = 1024 * 1024;
pub const MAX_COUNTER_QUERY_PAGE_ITEMS_V2: u16 = 4096;
const MIN_RESPONSE_BYTES: u64 = 4096;
const CONSERVATIVE_ITEM_BYTES: u64 = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CounterQueryLimitsV2 {
    pub max_input_bytes: u64,
    pub max_response_bytes: u64,
    pub max_page_items: u16,
}

impl Default for CounterQueryLimitsV2 {
    fn default() -> Self {
        Self {
            max_input_bytes: MAX_COUNTER_CAPTURE_BYTES_V2,
            max_response_bytes: MAX_COUNTER_QUERY_RESPONSE_BYTES_V2,
            max_page_items: MAX_COUNTER_QUERY_PAGE_ITEMS_V2,
        }
    }
}

impl CounterQueryLimitsV2 {
    pub fn new(input: u64, response: u64, page: u16) -> Result<Self, CounterQueryErrorV2> {
        if input == 0 || input > MAX_COUNTER_CAPTURE_BYTES_V2 {
            return Err(CounterQueryErrorV2::LimitOutOfRange);
        }
        if !(MIN_RESPONSE_BYTES..=MAX_COUNTER_QUERY_RESPONSE_BYTES_V2).contains(&response)
            || page == 0
            || page > MAX_COUNTER_QUERY_PAGE_ITEMS_V2
        {
            return Err(CounterQueryErrorV2::LimitOutOfRange);
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
pub enum CounterListKindV2 {
    Definitions,
    Dispatches,
    Values,
    Hotspots,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CounterCursorV2 {
    pub query_binding: CaptureIdentityV1,
    pub position: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CounterPageRequestV2 {
    pub limit: u16,
    pub cursor: Option<CounterCursorV2>,
    pub dispatch_filter: Option<CaptureIdentityV1>,
    pub counter_filter: Option<CaptureIdentityV1>,
}

impl Default for CounterPageRequestV2 {
    fn default() -> Self {
        Self {
            limit: 128,
            cursor: None,
            dispatch_filter: None,
            counter_filter: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterCapabilityNameV2 {
    Open,
    CounterDefinitions,
    DispatchCounterValues,
    CounterHotspots,
    HardwareInstanceCorrelation,
    SourceCorrelation,
    IsaCorrelation,
    PcSamples,
    AttWaveEvents,
    SemanticExecutionHistory,
    ExecutionControl,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterCapabilityAvailabilityV2 {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CounterCapabilityV2 {
    pub name: CounterCapabilityNameV2,
    pub availability: CounterCapabilityAvailabilityV2,
    pub reason: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CounterQueryContextV2 {
    pub capture_identity: ContentIdentityRecordV1,
    pub schema_version: u16,
    pub definition_count: u64,
    pub dispatch_count: u64,
    pub raw_value_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CounterHotspotV2 {
    pub rank: u64,
    pub dispatch_identity: CaptureIdentityV1,
    pub counter_identity: CaptureIdentityV1,
    pub aggregate_f64_bits: u64,
    pub raw_record_count: u64,
    pub origin: TruthOriginV1,
    pub aggregation: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "item", rename_all = "snake_case")]
pub enum CounterQueryItemV2 {
    Definition {
        definition: CounterDefinitionV2,
    },
    Dispatch {
        dispatch: Box<CounterDispatchV2>,
    },
    Value {
        dispatch_identity: CaptureIdentityV1,
        value: CounterValueV2,
    },
    Hotspot {
        hotspot: CounterHotspotV2,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CounterPageV2 {
    pub context: CounterQueryContextV2,
    pub kind: CounterListKindV2,
    pub returned: u16,
    pub next_cursor: Option<CounterCursorV2>,
    pub items: Vec<CounterQueryItemV2>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "response", rename_all = "snake_case")]
pub enum CounterQueryResponseV2 {
    Capabilities {
        context: CounterQueryContextV2,
        capabilities: Vec<CounterCapabilityV2>,
    },
    Open {
        context: CounterQueryContextV2,
        coverage: fe2o3_semantic_import::CounterCaptureCoverageV2,
    },
    Page {
        page: CounterPageV2,
    },
    InspectDispatch {
        context: CounterQueryContextV2,
        dispatch: Box<CounterDispatchV2>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CounterQueryRequestV2 {
    Capabilities,
    Open,
    List {
        kind: CounterListKindV2,
        page: CounterPageRequestV2,
    },
    InspectDispatch {
        identity: CaptureIdentityV1,
    },
}

pub struct CounterQuerySessionV2 {
    capture: SemanticCounterCaptureV2,
    context: CounterQueryContextV2,
    limits: CounterQueryLimitsV2,
}

impl CounterQuerySessionV2 {
    pub fn open(bytes: &[u8], limits: CounterQueryLimitsV2) -> Result<Self, CounterQueryErrorV2> {
        let actual = u64::try_from(bytes.len()).map_err(|_| CounterQueryErrorV2::SizeOverflow)?;
        if actual > limits.max_input_bytes {
            return Err(CounterQueryErrorV2::InputTooLarge);
        }
        let capture = decode_counter_capture_v2(bytes).map_err(CounterQueryErrorV2::Capture)?;
        let raw_value_count = capture
            .dispatches
            .iter()
            .try_fold(0_u64, |n, dispatch| {
                n.checked_add(u64::try_from(dispatch.values.len()).ok()?)
            })
            .ok_or(CounterQueryErrorV2::SizeOverflow)?;
        let context = CounterQueryContextV2 {
            capture_identity: counter_capture_content_identity_v2(bytes)
                .map_err(CounterQueryErrorV2::Capture)?,
            schema_version: capture.schema_version,
            definition_count: u64::try_from(capture.counter_definitions.len())
                .map_err(|_| CounterQueryErrorV2::SizeOverflow)?,
            dispatch_count: u64::try_from(capture.dispatches.len())
                .map_err(|_| CounterQueryErrorV2::SizeOverflow)?,
            raw_value_count,
        };
        Ok(Self {
            capture,
            context,
            limits,
        })
    }

    pub fn query(
        &self,
        request: CounterQueryRequestV2,
    ) -> Result<CounterQueryResponseV2, CounterQueryErrorV2> {
        match request {
            CounterQueryRequestV2::Capabilities => Ok(CounterQueryResponseV2::Capabilities {
                context: self.context,
                capabilities: capabilities(),
            }),
            CounterQueryRequestV2::Open => Ok(CounterQueryResponseV2::Open {
                context: self.context,
                coverage: self.capture.coverage,
            }),
            CounterQueryRequestV2::InspectDispatch { identity } => {
                let dispatch = self
                    .capture
                    .dispatches
                    .iter()
                    .find(|item| item.identity == identity)
                    .cloned()
                    .ok_or(CounterQueryErrorV2::DispatchNotFound)?;
                Ok(CounterQueryResponseV2::InspectDispatch {
                    context: self.context,
                    dispatch: Box::new(dispatch),
                })
            }
            CounterQueryRequestV2::List { kind, page } => Ok(CounterQueryResponseV2::Page {
                page: self.page(kind, page)?,
            }),
        }
    }

    pub fn query_json(
        &self,
        request: CounterQueryRequestV2,
    ) -> Result<Vec<u8>, CounterQueryErrorV2> {
        let response = self.query(request)?;
        let max = usize::try_from(self.limits.max_response_bytes)
            .map_err(|_| CounterQueryErrorV2::SizeOverflow)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(max)
            .map_err(|_| CounterQueryErrorV2::AllocationFailure)?;
        let mut writer = BoundedWriter {
            output: &mut output,
            max: max.saturating_sub(1),
            exceeded: false,
        };
        if serde_json::to_writer(&mut writer, &response).is_err() {
            return Err(if writer.exceeded {
                CounterQueryErrorV2::ResponseTooLarge
            } else {
                CounterQueryErrorV2::JsonEncode
            });
        }
        output.push(b'\n');
        Ok(output)
    }

    fn page(
        &self,
        kind: CounterListKindV2,
        page: CounterPageRequestV2,
    ) -> Result<CounterPageV2, CounterQueryErrorV2> {
        if page.limit == 0
            || page.limit > self.limits.max_page_items
            || u64::from(page.limit).saturating_mul(CONSERVATIVE_ITEM_BYTES)
                > self.limits.max_response_bytes
        {
            return Err(CounterQueryErrorV2::LimitOutOfRange);
        }
        if matches!(
            kind,
            CounterListKindV2::Definitions | CounterListKindV2::Dispatches
        ) && (page.dispatch_filter.is_some() || page.counter_filter.is_some())
        {
            return Err(CounterQueryErrorV2::FilterNotSupported);
        }
        let binding = query_binding(
            self.context.capture_identity.digest,
            kind,
            page.dispatch_filter,
            page.counter_filter,
        )?;
        let start = match page.cursor {
            None => 0,
            Some(cursor) if cursor.query_binding == binding => usize::try_from(cursor.position)
                .map_err(|_| CounterQueryErrorV2::CursorOutOfRange)?,
            Some(_) => return Err(CounterQueryErrorV2::CursorQueryMismatch),
        };
        let mut all = self.items(kind, page.dispatch_filter, page.counter_filter)?;
        if start > all.len() {
            return Err(CounterQueryErrorV2::CursorOutOfRange);
        }
        let end = start.saturating_add(usize::from(page.limit)).min(all.len());
        let items: Vec<_> = all.drain(start..end).collect();
        let next_cursor = (end < all.len() + items.len()).then_some(CounterCursorV2 {
            query_binding: binding,
            position: u64::try_from(end).map_err(|_| CounterQueryErrorV2::SizeOverflow)?,
        });
        Ok(CounterPageV2 {
            context: self.context,
            kind,
            returned: u16::try_from(items.len()).map_err(|_| CounterQueryErrorV2::SizeOverflow)?,
            next_cursor,
            items,
        })
    }

    fn items(
        &self,
        kind: CounterListKindV2,
        dispatch_filter: Option<CaptureIdentityV1>,
        counter_filter: Option<CaptureIdentityV1>,
    ) -> Result<Vec<CounterQueryItemV2>, CounterQueryErrorV2> {
        match kind {
            CounterListKindV2::Definitions => Ok(self
                .capture
                .counter_definitions
                .iter()
                .cloned()
                .map(|definition| CounterQueryItemV2::Definition { definition })
                .collect()),
            CounterListKindV2::Dispatches => Ok(self
                .capture
                .dispatches
                .iter()
                .cloned()
                .map(|dispatch| CounterQueryItemV2::Dispatch {
                    dispatch: Box::new(dispatch),
                })
                .collect()),
            CounterListKindV2::Values => {
                let mut items = Vec::new();
                for dispatch in &self.capture.dispatches {
                    if dispatch_filter.is_some_and(|id| id != dispatch.identity) {
                        continue;
                    }
                    items.extend(
                        dispatch
                            .values
                            .iter()
                            .filter(|value| {
                                counter_filter.is_none_or(|id| id == value.counter_identity)
                            })
                            .copied()
                            .map(|value| CounterQueryItemV2::Value {
                                dispatch_identity: dispatch.identity,
                                value,
                            }),
                    );
                }
                Ok(items)
            }
            CounterListKindV2::Hotspots => {
                let mut groups: BTreeMap<(CaptureIdentityV1, CaptureIdentityV1), (f64, u64)> =
                    BTreeMap::new();
                for dispatch in &self.capture.dispatches {
                    if dispatch_filter.is_some_and(|id| id != dispatch.identity) {
                        continue;
                    }
                    for value in &dispatch.values {
                        if counter_filter.is_some_and(|id| id != value.counter_identity) {
                            continue;
                        }
                        let entry = groups
                            .entry((dispatch.identity, value.counter_identity))
                            .or_default();
                        entry.0 += value.value();
                        entry.1 = entry
                            .1
                            .checked_add(1)
                            .ok_or(CounterQueryErrorV2::SizeOverflow)?;
                        if !entry.0.is_finite() {
                            return Err(CounterQueryErrorV2::NonFiniteAggregate);
                        }
                    }
                }
                let mut groups: Vec<_> = groups.into_iter().collect();
                groups.sort_by(|left, right| {
                    right
                        .1
                        .0
                        .total_cmp(&left.1.0)
                        .then_with(|| left.0.cmp(&right.0))
                });
                Ok(groups
                    .into_iter()
                    .enumerate()
                    .map(
                        |(rank, ((dispatch_identity, counter_identity), (value, count)))| {
                            CounterQueryItemV2::Hotspot {
                                hotspot: CounterHotspotV2 {
                                    rank: u64::try_from(rank + 1).unwrap_or(u64::MAX),
                                    dispatch_identity,
                                    counter_identity,
                                    aggregate_f64_bits: value.to_bits(),
                                    raw_record_count: count,
                                    origin: TruthOriginV1::Inferred,
                                    aggregation: "sum_raw_records_by_dispatch_and_counter_id",
                                },
                            }
                        },
                    )
                    .collect())
            }
        }
    }
}

fn query_binding(
    capture: CaptureIdentityV1,
    kind: CounterListKindV2,
    dispatch: Option<CaptureIdentityV1>,
    counter: Option<CaptureIdentityV1>,
) -> Result<CaptureIdentityV1, CounterQueryErrorV2> {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3.semantic-counter-capture.query.v2\0");
    digest.update(capture.as_bytes());
    digest.update([kind as u8]);
    digest.update(dispatch.map(CaptureIdentityV1::as_bytes).unwrap_or([0; 32]));
    digest.update(counter.map(CaptureIdentityV1::as_bytes).unwrap_or([0; 32]));
    CaptureIdentityV1::new(digest.finalize().into())
        .map_err(|_| CounterQueryErrorV2::IdentityFailure)
}

fn capabilities() -> Vec<CounterCapabilityV2> {
    use CounterCapabilityAvailabilityV2::{Available, Unavailable};
    use CounterCapabilityNameV2::*;
    [
        (Open, Available, None),
        (CounterDefinitions, Available, None),
        (DispatchCounterValues, Available, None),
        (CounterHotspots, Available, None),
        (
            HardwareInstanceCorrelation,
            Unavailable,
            Some("counter records contain no instance identity"),
        ),
        (
            SourceCorrelation,
            Unavailable,
            Some("no authenticated source map correlation"),
        ),
        (
            IsaCorrelation,
            Unavailable,
            Some("no authenticated ISA map correlation"),
        ),
        (
            PcSamples,
            Unavailable,
            Some("not imported by counter capture v2"),
        ),
        (
            AttWaveEvents,
            Unavailable,
            Some("not imported by counter capture v2"),
        ),
        (
            SemanticExecutionHistory,
            Unavailable,
            Some("not represented by dispatch counter records"),
        ),
        (
            ExecutionControl,
            Unavailable,
            Some("read-only query surface"),
        ),
    ]
    .into_iter()
    .map(|(name, availability, reason)| CounterCapabilityV2 {
        name,
        availability,
        reason,
    })
    .collect()
}

struct BoundedWriter<'a> {
    output: &'a mut Vec<u8>,
    max: usize,
    exceeded: bool,
}
impl Write for BoundedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.write_all(bytes)?;
        Ok(bytes.len())
    }
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self
            .output
            .len()
            .checked_add(bytes.len())
            .is_none_or(|n| n > self.max)
        {
            self.exceeded = true;
            return Err(io::Error::other("counter query response limit exceeded"));
        }
        self.output.extend_from_slice(bytes);
        Ok(())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub enum CounterQueryErrorV2 {
    LimitOutOfRange,
    InputTooLarge,
    CursorQueryMismatch,
    CursorOutOfRange,
    FilterNotSupported,
    DispatchNotFound,
    NonFiniteAggregate,
    SizeOverflow,
    IdentityFailure,
    AllocationFailure,
    JsonEncode,
    ResponseTooLarge,
    Capture(fe2o3_semantic_import::CounterCaptureErrorV2),
}
impl fmt::Display for CounterQueryErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "semantic counter capture query rejected: {self:?}")
    }
}
impl Error for CounterQueryErrorV2 {}
