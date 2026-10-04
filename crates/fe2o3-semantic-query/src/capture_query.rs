use std::error::Error;
use std::fmt;
use std::io::{self, Write};

use fe2o3_semantic_import::{
    CAPTURE_IDENTITY_DOMAIN_V1, CAPTURE_SCHEMA_VERSION_V1, CaptureCoverageV1, CaptureDeviceV1,
    CaptureDispatchV1, CaptureIdentityV1, CaptureRunV1, ContentIdentityRecordV1, ContentSchemeV1,
    MAX_CAPTURE_BYTES_V1, SemanticCaptureV1, TruthOriginV1, decode_capture_v1,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const MAX_CAPTURE_QUERY_RESPONSE_BYTES_V1: u64 = 1024 * 1024;
pub const MIN_CAPTURE_QUERY_RESPONSE_BYTES_V1: u64 = 4 * 1024;
pub const MAX_CAPTURE_QUERY_PAGE_ITEMS_V1: u16 = 4_096;
const MAX_CONSERVATIVE_ITEM_BYTES_V1: u64 = 2_048;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureQueryLimitsV1 {
    max_input_bytes: u64,
    max_response_bytes: u64,
    max_page_items: u16,
}

impl CaptureQueryLimitsV1 {
    pub fn new(
        max_input_bytes: u64,
        max_response_bytes: u64,
        max_page_items: u16,
    ) -> Result<Self, CaptureQueryErrorV1> {
        if max_input_bytes == 0 || max_input_bytes > MAX_CAPTURE_BYTES_V1 {
            return Err(CaptureQueryErrorV1::InputLimitOutOfRange);
        }
        if !(MIN_CAPTURE_QUERY_RESPONSE_BYTES_V1..=MAX_CAPTURE_QUERY_RESPONSE_BYTES_V1)
            .contains(&max_response_bytes)
        {
            return Err(CaptureQueryErrorV1::ResponseLimitOutOfRange);
        }
        if max_page_items == 0 || max_page_items > MAX_CAPTURE_QUERY_PAGE_ITEMS_V1 {
            return Err(CaptureQueryErrorV1::PageLimitOutOfRange);
        }
        Ok(Self {
            max_input_bytes,
            max_response_bytes,
            max_page_items,
        })
    }

    pub const fn max_input_bytes(self) -> u64 {
        self.max_input_bytes
    }
    pub const fn max_response_bytes(self) -> u64 {
        self.max_response_bytes
    }
    pub const fn max_page_items(self) -> u16 {
        self.max_page_items
    }
}

impl Default for CaptureQueryLimitsV1 {
    fn default() -> Self {
        Self {
            max_input_bytes: MAX_CAPTURE_BYTES_V1,
            max_response_bytes: MAX_CAPTURE_QUERY_RESPONSE_BYTES_V1,
            max_page_items: MAX_CAPTURE_QUERY_PAGE_ITEMS_V1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureListKindV1 {
    Runs,
    Devices,
    Dispatches,
    Hotspots,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CaptureCursorV1 {
    pub query_binding: CaptureIdentityV1,
    pub position: u64,
}

impl CaptureCursorV1 {
    pub const fn new(query_binding: CaptureIdentityV1, position: u64) -> Self {
        Self {
            query_binding,
            position,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CapturePageRequestV1 {
    pub limit: u16,
    pub cursor: Option<CaptureCursorV1>,
}

impl Default for CapturePageRequestV1 {
    fn default() -> Self {
        Self {
            limit: 128,
            cursor: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureQueryRequestV1 {
    Capabilities,
    Open,
    List {
        kind: CaptureListKindV1,
        page: CapturePageRequestV1,
    },
    InspectDispatch {
        identity: CaptureIdentityV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "response", rename_all = "snake_case")]
pub enum CaptureQueryResponseV1 {
    Capabilities {
        context: CaptureContextV1,
        capabilities: Vec<CaptureCapabilityV1>,
    },
    Open {
        context: CaptureContextV1,
        coverage: CaptureCoverageV1,
    },
    Page {
        page: CapturePageV1,
    },
    InspectDispatch {
        context: CaptureContextV1,
        dispatch: Box<CaptureDispatchV1>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CaptureContextV1 {
    pub capture_identity: ContentIdentityRecordV1,
    pub schema_version: u16,
    pub run_count: u64,
    pub device_count: u64,
    pub dispatch_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureCapabilityNameV1 {
    Open,
    ListRuns,
    ListDevices,
    ListDispatches,
    InspectDispatch,
    DurationHotspots,
    KernelDispatchEnvelopes,
    CounterRecords,
    PcSamples,
    AttWaveEvents,
    SemanticExecutionHistory,
    ExecutionControl,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureCapabilityAvailabilityV1 {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureCapabilityReasonV1 {
    NotImportedByCaptureV1,
    NotRepresentedByStructuredDispatchRecords,
    ReadOnlySurface,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CaptureCapabilityV1 {
    pub name: CaptureCapabilityNameV1,
    pub availability: CaptureCapabilityAvailabilityV1,
    pub reason: Option<CaptureCapabilityReasonV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CapturePageV1 {
    pub context: CaptureContextV1,
    pub kind: CaptureListKindV1,
    pub returned: u16,
    pub next_cursor: Option<CaptureCursorV1>,
    pub items: Vec<CaptureQueryItemV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "item", rename_all = "snake_case")]
pub enum CaptureQueryItemV1 {
    Run { run: CaptureRunV1 },
    Device { device: CaptureDeviceV1 },
    Dispatch { dispatch: Box<CaptureDispatchV1> },
    Hotspot { hotspot: DurationHotspotV1 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct DurationHotspotV1 {
    pub rank: u64,
    pub dispatch_identity: CaptureIdentityV1,
    pub duration_ticks: u64,
    pub origin: TruthOriginV1,
    pub comparison_scope: &'static str,
}

pub struct CaptureQuerySessionV1 {
    capture: SemanticCaptureV1,
    context: CaptureContextV1,
    limits: CaptureQueryLimitsV1,
}

impl CaptureQuerySessionV1 {
    pub fn open(bytes: &[u8], limits: CaptureQueryLimitsV1) -> Result<Self, CaptureQueryErrorV1> {
        let actual = u64::try_from(bytes.len()).map_err(|_| CaptureQueryErrorV1::SizeOverflow)?;
        if actual > limits.max_input_bytes {
            return Err(CaptureQueryErrorV1::InputTooLarge {
                actual,
                max: limits.max_input_bytes,
            });
        }
        let capture = decode_capture_v1(bytes).map_err(CaptureQueryErrorV1::Capture)?;
        let mut hasher = Sha256::new();
        hasher.update(CAPTURE_IDENTITY_DOMAIN_V1);
        hasher.update(bytes);
        let capture_identity = ContentIdentityRecordV1 {
            scheme: ContentSchemeV1::DomainSeparatedSha256,
            format_version: CAPTURE_SCHEMA_VERSION_V1,
            digest: CaptureIdentityV1::new(hasher.finalize().into())
                .map_err(|_| CaptureQueryErrorV1::IdentityFailure)?,
            canonical_len: actual,
        };
        let context = CaptureContextV1 {
            capture_identity,
            schema_version: capture.schema_version,
            run_count: count(capture.runs.len())?,
            device_count: count(capture.devices.len())?,
            dispatch_count: count(capture.dispatches.len())?,
        };
        Ok(Self {
            capture,
            context,
            limits,
        })
    }

    pub const fn limits(&self) -> CaptureQueryLimitsV1 {
        self.limits
    }

    pub fn query(
        &self,
        request: CaptureQueryRequestV1,
    ) -> Result<CaptureQueryResponseV1, CaptureQueryErrorV1> {
        match request {
            CaptureQueryRequestV1::Capabilities => Ok(CaptureQueryResponseV1::Capabilities {
                context: self.context,
                capabilities: capabilities(),
            }),
            CaptureQueryRequestV1::Open => Ok(CaptureQueryResponseV1::Open {
                context: self.context,
                coverage: self.capture.coverage,
            }),
            CaptureQueryRequestV1::List { kind, page } => {
                self.validate_page(page)?;
                Ok(CaptureQueryResponseV1::Page {
                    page: self.page(kind, page)?,
                })
            }
            CaptureQueryRequestV1::InspectDispatch { identity } => {
                let dispatch = self
                    .capture
                    .dispatches
                    .iter()
                    .find(|dispatch| dispatch.identity == identity)
                    .cloned()
                    .ok_or(CaptureQueryErrorV1::DispatchNotFound)?;
                Ok(CaptureQueryResponseV1::InspectDispatch {
                    context: self.context,
                    dispatch: Box::new(dispatch),
                })
            }
        }
    }

    pub fn query_json(
        &self,
        request: CaptureQueryRequestV1,
    ) -> Result<Vec<u8>, CaptureQueryErrorV1> {
        let response = self.query(request)?;
        let max = usize::try_from(self.limits.max_response_bytes)
            .map_err(|_| CaptureQueryErrorV1::SizeOverflow)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(max)
            .map_err(|_| CaptureQueryErrorV1::AllocationFailure)?;
        let mut writer = BoundedWriterV1 {
            output: &mut output,
            max: max.saturating_sub(1),
            exceeded: false,
        };
        if serde_json::to_writer(&mut writer, &response).is_err() {
            return Err(if writer.exceeded {
                CaptureQueryErrorV1::ResponseTooLarge
            } else {
                CaptureQueryErrorV1::JsonEncode
            });
        }
        if output.len() == max {
            return Err(CaptureQueryErrorV1::ResponseTooLarge);
        }
        output.push(b'\n');
        Ok(output)
    }

    fn validate_page(&self, page: CapturePageRequestV1) -> Result<(), CaptureQueryErrorV1> {
        if page.limit == 0 || page.limit > self.limits.max_page_items {
            return Err(CaptureQueryErrorV1::PageLimitOutOfRange);
        }
        if u64::from(page.limit).saturating_mul(MAX_CONSERVATIVE_ITEM_BYTES_V1)
            > self.limits.max_response_bytes
        {
            return Err(CaptureQueryErrorV1::PageExceedsResponseBudget);
        }
        Ok(())
    }

    fn page(
        &self,
        kind: CaptureListKindV1,
        page: CapturePageRequestV1,
    ) -> Result<CapturePageV1, CaptureQueryErrorV1> {
        let binding = query_binding(self.context.capture_identity.digest, kind)?;
        let start = match page.cursor {
            Some(cursor) if cursor.query_binding == binding => usize::try_from(cursor.position)
                .map_err(|_| CaptureQueryErrorV1::CursorOutOfRange)?,
            Some(_) => return Err(CaptureQueryErrorV1::CursorQueryMismatch),
            None => 0,
        };
        let total = match kind {
            CaptureListKindV1::Runs => self.capture.runs.len(),
            CaptureListKindV1::Devices => self.capture.devices.len(),
            CaptureListKindV1::Dispatches | CaptureListKindV1::Hotspots => {
                self.capture.dispatches.len()
            }
        };
        if start > total {
            return Err(CaptureQueryErrorV1::CursorOutOfRange);
        }
        let end = start.saturating_add(usize::from(page.limit)).min(total);
        let mut items = Vec::new();
        items
            .try_reserve_exact(end - start)
            .map_err(|_| CaptureQueryErrorV1::AllocationFailure)?;
        match kind {
            CaptureListKindV1::Runs => items.extend(
                self.capture.runs[start..end]
                    .iter()
                    .cloned()
                    .map(|run| CaptureQueryItemV1::Run { run }),
            ),
            CaptureListKindV1::Devices => items.extend(
                self.capture.devices[start..end]
                    .iter()
                    .cloned()
                    .map(|device| CaptureQueryItemV1::Device { device }),
            ),
            CaptureListKindV1::Dispatches => items.extend(
                self.capture.dispatches[start..end]
                    .iter()
                    .cloned()
                    .map(|dispatch| CaptureQueryItemV1::Dispatch {
                        dispatch: Box::new(dispatch),
                    }),
            ),
            CaptureListKindV1::Hotspots => {
                let mut ranked = Vec::new();
                ranked
                    .try_reserve_exact(self.capture.dispatches.len())
                    .map_err(|_| CaptureQueryErrorV1::AllocationFailure)?;
                ranked.extend(self.capture.dispatches.iter());
                ranked.sort_by(|left, right| {
                    right
                        .duration_ticks
                        .cmp(&left.duration_ticks)
                        .then_with(|| left.identity.cmp(&right.identity))
                });
                items.extend(
                    ranked[start..end]
                        .iter()
                        .enumerate()
                        .map(|(offset, dispatch)| CaptureQueryItemV1::Hotspot {
                            hotspot: DurationHotspotV1 {
                                rank: u64::try_from(start + offset + 1).unwrap_or(u64::MAX),
                                dispatch_identity: dispatch.identity,
                                duration_ticks: dispatch.duration_ticks,
                                origin: dispatch.timing_origin,
                                comparison_scope: "captured_dispatch_envelopes_only",
                            },
                        }),
                );
            }
        }
        let next_cursor = (end < total).then(|| CaptureCursorV1::new(binding, end as u64));
        Ok(CapturePageV1 {
            context: self.context,
            kind,
            returned: u16::try_from(items.len()).map_err(|_| CaptureQueryErrorV1::SizeOverflow)?,
            next_cursor,
            items,
        })
    }
}

fn capabilities() -> Vec<CaptureCapabilityV1> {
    use CaptureCapabilityAvailabilityV1::{Available, Unavailable};
    use CaptureCapabilityNameV1::*;
    use CaptureCapabilityReasonV1::*;
    vec![
        CaptureCapabilityV1 {
            name: Open,
            availability: Available,
            reason: None,
        },
        CaptureCapabilityV1 {
            name: ListRuns,
            availability: Available,
            reason: None,
        },
        CaptureCapabilityV1 {
            name: ListDevices,
            availability: Available,
            reason: None,
        },
        CaptureCapabilityV1 {
            name: ListDispatches,
            availability: Available,
            reason: None,
        },
        CaptureCapabilityV1 {
            name: InspectDispatch,
            availability: Available,
            reason: None,
        },
        CaptureCapabilityV1 {
            name: DurationHotspots,
            availability: Available,
            reason: None,
        },
        CaptureCapabilityV1 {
            name: KernelDispatchEnvelopes,
            availability: Available,
            reason: None,
        },
        CaptureCapabilityV1 {
            name: CounterRecords,
            availability: Unavailable,
            reason: Some(NotImportedByCaptureV1),
        },
        CaptureCapabilityV1 {
            name: PcSamples,
            availability: Unavailable,
            reason: Some(NotImportedByCaptureV1),
        },
        CaptureCapabilityV1 {
            name: AttWaveEvents,
            availability: Unavailable,
            reason: Some(NotImportedByCaptureV1),
        },
        CaptureCapabilityV1 {
            name: SemanticExecutionHistory,
            availability: Unavailable,
            reason: Some(NotRepresentedByStructuredDispatchRecords),
        },
        CaptureCapabilityV1 {
            name: ExecutionControl,
            availability: Unavailable,
            reason: Some(ReadOnlySurface),
        },
    ]
}

fn count(value: usize) -> Result<u64, CaptureQueryErrorV1> {
    u64::try_from(value).map_err(|_| CaptureQueryErrorV1::SizeOverflow)
}

fn query_binding(
    capture: CaptureIdentityV1,
    kind: CaptureListKindV1,
) -> Result<CaptureIdentityV1, CaptureQueryErrorV1> {
    let mut hasher = Sha256::new();
    hasher.update(b"fe2o3.semantic-capture.query.v1\0");
    hasher.update(capture.as_bytes());
    hasher.update([match kind {
        CaptureListKindV1::Runs => 0,
        CaptureListKindV1::Devices => 1,
        CaptureListKindV1::Dispatches => 2,
        CaptureListKindV1::Hotspots => 3,
    }]);
    CaptureIdentityV1::new(hasher.finalize().into())
        .map_err(|_| CaptureQueryErrorV1::IdentityFailure)
}

struct BoundedWriterV1<'a> {
    output: &'a mut Vec<u8>,
    max: usize,
    exceeded: bool,
}

impl Write for BoundedWriterV1<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.write_all(bytes)?;
        Ok(bytes.len())
    }

    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self
            .output
            .len()
            .checked_add(bytes.len())
            .is_none_or(|required| required > self.max)
        {
            self.exceeded = true;
            return Err(io::Error::other("capture query response limit exceeded"));
        }
        self.output.extend_from_slice(bytes);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub enum CaptureQueryErrorV1 {
    InputLimitOutOfRange,
    ResponseLimitOutOfRange,
    PageLimitOutOfRange,
    InputTooLarge { actual: u64, max: u64 },
    PageExceedsResponseBudget,
    CursorQueryMismatch,
    CursorOutOfRange,
    DispatchNotFound,
    SizeOverflow,
    IdentityFailure,
    AllocationFailure,
    JsonEncode,
    ResponseTooLarge,
    Capture(fe2o3_semantic_import::CaptureErrorV1),
}

impl fmt::Display for CaptureQueryErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "semantic capture query rejected: {self:?}")
    }
}

impl Error for CaptureQueryErrorV1 {}
