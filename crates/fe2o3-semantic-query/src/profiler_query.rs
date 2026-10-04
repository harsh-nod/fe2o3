use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::io::{self, Write};

use fe2o3_semantic_import::{
    AttArtifactReferenceV4, CaptureDispatchV1, CaptureIdentityV1, ContentIdentityRecordV1,
    IdentityFactV1, MAX_PROFILER_BUNDLE_BYTES_V4, ProfilerCoverageV4, ProfilerDeviceV4,
    ProfilerSourceKindV4, SemanticProfilerBundleV4, TruthOriginV1, decode_profiler_bundle_v4,
    profiler_bundle_content_identity_v4,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAX_PROFILER_QUERY_PAGE_ITEMS_V4: u16 = 4_096;
pub const MAX_PROFILER_QUERY_RESPONSE_BYTES_V4: u64 = 2 * 1024 * 1024;
pub const MAX_PROFILER_CAPTURE_PLAN_STEPS_V4: usize = 8;
const MIN_PROFILER_QUERY_RESPONSE_BYTES_V4: u64 = 4 * 1024;
const PROFILER_QUERY_CURSOR_DOMAIN_V4: &[u8] = b"fe2o3.profiler-query.cursor.v4\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfilerQueryLimitsV4 {
    pub max_input_bytes: u64,
    pub max_response_bytes: u64,
    pub max_page_items: u16,
}

impl Default for ProfilerQueryLimitsV4 {
    fn default() -> Self {
        Self {
            max_input_bytes: MAX_PROFILER_BUNDLE_BYTES_V4,
            max_response_bytes: MAX_PROFILER_QUERY_RESPONSE_BYTES_V4,
            max_page_items: 128,
        }
    }
}

impl ProfilerQueryLimitsV4 {
    pub fn new(input: u64, response: u64, page: u16) -> Result<Self, ProfilerQueryErrorV4> {
        if input == 0
            || input > MAX_PROFILER_BUNDLE_BYTES_V4
            || !(MIN_PROFILER_QUERY_RESPONSE_BYTES_V4..=MAX_PROFILER_QUERY_RESPONSE_BYTES_V4)
                .contains(&response)
            || page == 0
            || page > MAX_PROFILER_QUERY_PAGE_ITEMS_V4
        {
            return Err(ProfilerQueryErrorV4::LimitOutOfRange);
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
pub enum ProfilerCapabilityNameV4 {
    Runs,
    Devices,
    DispatchEnvelopes,
    DurationHotspots,
    AttArtifactReferences,
    DecodedAttEvents,
    WaitAnalysis,
    CaptureComparison,
    NextCapturePlanning,
    ExecutionControl,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerAvailabilityV4 {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ProfilerCapabilityV4 {
    pub name: ProfilerCapabilityNameV4,
    pub availability: ProfilerAvailabilityV4,
    pub reason: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerQueryContextV4 {
    pub bundle_identity: ContentIdentityRecordV1,
    pub run_identity: CaptureIdentityV1,
    pub source_kind: ProfilerSourceKindV4,
    pub device_count: u64,
    pub dispatch_count: u64,
    pub att_reference_count: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerListKindV4 {
    Runs,
    Devices,
    Dispatches,
    DurationHotspots,
    AttReferences,
    Waits,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerCursorV4 {
    pub query_binding: CaptureIdentityV1,
    pub position: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerPageRequestV4 {
    pub limit: u16,
    pub cursor: Option<ProfilerCursorV4>,
}

impl Default for ProfilerPageRequestV4 {
    fn default() -> Self {
        Self {
            limit: 128,
            cursor: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerEvidenceV4 {
    pub origin: TruthOriginV1,
    pub bundle: CaptureIdentityV1,
    pub record: Option<CaptureIdentityV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerDispatchSummaryV4 {
    pub identity: CaptureIdentityV1,
    pub device_identity: CaptureIdentityV1,
    pub process_index: u32,
    pub dispatch_index: u32,
    pub launch: fe2o3_semantic_import::LaunchRecordV1,
    pub start_timestamp: u64,
    pub end_timestamp: u64,
    pub duration_ticks: u64,
    pub evidence: ProfilerEvidenceV4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ProfilerHotspotV4 {
    pub rank: u64,
    pub dispatch_identity: CaptureIdentityV1,
    pub duration_ticks: u64,
    pub origin: TruthOriginV1,
    pub evidence: ProfilerEvidenceV4,
    pub limitation: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "item", rename_all = "snake_case")]
pub enum ProfilerQueryItemV4 {
    Run {
        run_identity: CaptureIdentityV1,
        evidence: ProfilerEvidenceV4,
    },
    Device {
        device: ProfilerDeviceV4,
        evidence: ProfilerEvidenceV4,
    },
    Dispatch {
        dispatch: ProfilerDispatchSummaryV4,
    },
    DurationHotspot {
        hotspot: ProfilerHotspotV4,
    },
    AttReference {
        reference: AttArtifactReferenceV4,
        evidence: ProfilerEvidenceV4,
    },
    Unavailable {
        subject: &'static str,
        reason: &'static str,
        evidence: ProfilerEvidenceV4,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProfilerPageV4 {
    pub context: ProfilerQueryContextV4,
    pub kind: ProfilerListKindV4,
    pub returned: u16,
    pub next_cursor: Option<ProfilerCursorV4>,
    pub items: Vec<ProfilerQueryItemV4>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerCaptureGoalV4 {
    ExplainWaits,
    DecodeAttCoverage,
    RankDispatchDurations,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerCaptureStepV4 {
    CollectKernelDispatches,
    CollectAttManifestAndReferencedArtifacts,
    DecodeAttWithSupportedRocprofComputeViewer,
    ImportDecodedAttWhenSchemaIsSupported,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProfilerNextCapturePlanV4 {
    pub goal: ProfilerCaptureGoalV4,
    pub origin: TruthOriginV1,
    pub steps: Vec<ProfilerCaptureStepV4>,
    pub evidence: Vec<CaptureIdentityV1>,
    pub limitations: Vec<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfilerQueryRequestV4 {
    Capabilities,
    Open,
    List {
        kind: ProfilerListKindV4,
        page: ProfilerPageRequestV4,
    },
    InspectDispatch {
        identity: CaptureIdentityV1,
    },
    PlanNextCapture {
        goal: ProfilerCaptureGoalV4,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "response", rename_all = "snake_case")]
pub enum ProfilerQueryResponseV4 {
    Capabilities {
        context: ProfilerQueryContextV4,
        capabilities: Vec<ProfilerCapabilityV4>,
    },
    Open {
        context: ProfilerQueryContextV4,
        coverage: ProfilerCoverageV4,
    },
    Page {
        page: ProfilerPageV4,
    },
    InspectDispatch {
        context: ProfilerQueryContextV4,
        dispatch: Box<CaptureDispatchV1>,
        evidence: ProfilerEvidenceV4,
    },
    PlanNextCapture {
        context: ProfilerQueryContextV4,
        plan: ProfilerNextCapturePlanV4,
    },
}

pub struct ProfilerQuerySessionV4 {
    bundle: SemanticProfilerBundleV4,
    context: ProfilerQueryContextV4,
    limits: ProfilerQueryLimitsV4,
}

impl ProfilerQuerySessionV4 {
    pub fn open(bytes: &[u8], limits: ProfilerQueryLimitsV4) -> Result<Self, ProfilerQueryErrorV4> {
        let actual = u64::try_from(bytes.len()).map_err(|_| ProfilerQueryErrorV4::SizeOverflow)?;
        if actual > limits.max_input_bytes {
            return Err(ProfilerQueryErrorV4::InputTooLarge);
        }
        let bundle = decode_profiler_bundle_v4(bytes).map_err(|_| ProfilerQueryErrorV4::Bundle)?;
        let identity =
            profiler_bundle_content_identity_v4(bytes).map_err(|_| ProfilerQueryErrorV4::Bundle)?;
        let context = ProfilerQueryContextV4 {
            bundle_identity: identity,
            run_identity: bundle.run_identity,
            source_kind: bundle.source_kind,
            device_count: u64::try_from(bundle.devices.len())
                .map_err(|_| ProfilerQueryErrorV4::SizeOverflow)?,
            dispatch_count: bundle.coverage.imported_dispatches,
            att_reference_count: bundle.coverage.att_references,
        };
        Ok(Self {
            bundle,
            context,
            limits,
        })
    }

    pub fn query(
        &self,
        request: ProfilerQueryRequestV4,
    ) -> Result<ProfilerQueryResponseV4, ProfilerQueryErrorV4> {
        match request {
            ProfilerQueryRequestV4::Capabilities => Ok(ProfilerQueryResponseV4::Capabilities {
                context: self.context,
                capabilities: self.capabilities(),
            }),
            ProfilerQueryRequestV4::Open => Ok(ProfilerQueryResponseV4::Open {
                context: self.context,
                coverage: self.bundle.coverage,
            }),
            ProfilerQueryRequestV4::List { kind, page } => Ok(ProfilerQueryResponseV4::Page {
                page: self.page(kind, page)?,
            }),
            ProfilerQueryRequestV4::InspectDispatch { identity } => {
                let dispatch = self
                    .bundle
                    .dispatch_capture
                    .as_ref()
                    .and_then(|capture| {
                        capture
                            .dispatches
                            .iter()
                            .find(|dispatch| dispatch.identity == identity)
                    })
                    .ok_or(ProfilerQueryErrorV4::DispatchNotFound)?;
                Ok(ProfilerQueryResponseV4::InspectDispatch {
                    context: self.context,
                    dispatch: Box::new(dispatch.clone()),
                    evidence: self.evidence(TruthOriginV1::Observed, Some(identity)),
                })
            }
            ProfilerQueryRequestV4::PlanNextCapture { goal } => {
                Ok(ProfilerQueryResponseV4::PlanNextCapture {
                    context: self.context,
                    plan: self.plan(goal),
                })
            }
        }
    }

    pub(crate) fn admitted_bundle(&self) -> &SemanticProfilerBundleV4 {
        &self.bundle
    }

    pub fn encode_response(
        &self,
        response: &ProfilerQueryResponseV4,
    ) -> Result<Vec<u8>, ProfilerQueryErrorV4> {
        self.validate_response(response)?;
        let mut output = Vec::new();
        let mut writer = BoundedWriterV4 {
            output: &mut output,
            max: self.limits.max_response_bytes,
            exceeded: false,
        };
        serde_json::to_writer(&mut writer, response).map_err(|_| {
            if writer.exceeded {
                ProfilerQueryErrorV4::ResponseTooLarge
            } else {
                ProfilerQueryErrorV4::JsonEncode
            }
        })?;
        output.push(b'\n');
        Ok(output)
    }

    fn validate_response(
        &self,
        response: &ProfilerQueryResponseV4,
    ) -> Result<(), ProfilerQueryErrorV4> {
        let valid = match response {
            ProfilerQueryResponseV4::Capabilities {
                context,
                capabilities,
            } => *context == self.context && *capabilities == self.capabilities(),
            ProfilerQueryResponseV4::Open { context, coverage } => {
                *context == self.context && *coverage == self.bundle.coverage
            }
            ProfilerQueryResponseV4::Page { page } => self.validate_page(page)?,
            ProfilerQueryResponseV4::InspectDispatch {
                context,
                dispatch,
                evidence,
            } => {
                *context == self.context
                    && self
                        .bundle
                        .dispatch_capture
                        .as_ref()
                        .and_then(|capture| {
                            capture
                                .dispatches
                                .iter()
                                .find(|candidate| candidate.identity == dispatch.identity)
                        })
                        .is_some_and(|candidate| candidate == dispatch.as_ref())
                    && *evidence == self.evidence(TruthOriginV1::Observed, Some(dispatch.identity))
            }
            ProfilerQueryResponseV4::PlanNextCapture { context, plan } => {
                *context == self.context && *plan == self.plan(plan.goal)
            }
        };
        if valid {
            Ok(())
        } else {
            Err(ProfilerQueryErrorV4::InvalidResponse)
        }
    }

    fn validate_page(&self, page: &ProfilerPageV4) -> Result<bool, ProfilerQueryErrorV4> {
        if page.context != self.context
            || usize::from(page.returned) != page.items.len()
            || page.returned > self.limits.max_page_items
        {
            return Ok(false);
        }
        let all = self.items(page.kind)?;
        let end = match page.next_cursor {
            Some(cursor)
                if cursor.query_binding
                    == cursor_binding(self.context.bundle_identity.digest, page.kind)? =>
            {
                let end = usize::try_from(cursor.position)
                    .map_err(|_| ProfilerQueryErrorV4::InvalidResponse)?;
                if end >= all.len() {
                    return Ok(false);
                }
                end
            }
            Some(_) => return Ok(false),
            None => all.len(),
        };
        let Some(start) = end.checked_sub(page.items.len()) else {
            return Ok(false);
        };
        Ok(all.get(start..end) == Some(page.items.as_slice()))
    }

    fn evidence(
        &self,
        origin: TruthOriginV1,
        record: Option<CaptureIdentityV1>,
    ) -> ProfilerEvidenceV4 {
        ProfilerEvidenceV4 {
            origin,
            bundle: self.context.bundle_identity.digest,
            record,
        }
    }

    fn capabilities(&self) -> Vec<ProfilerCapabilityV4> {
        let dispatches = self.bundle.dispatch_capture.is_some();
        let att = self.bundle.att.is_some();
        vec![
            capability(ProfilerCapabilityNameV4::Runs, true, None),
            capability(ProfilerCapabilityNameV4::Devices, true, None),
            capability(
                ProfilerCapabilityNameV4::DispatchEnvelopes,
                dispatches,
                Some("capture contains ATT references, not dispatch envelopes"),
            ),
            capability(
                ProfilerCapabilityNameV4::DurationHotspots,
                dispatches,
                Some("duration ranking requires structured dispatch timestamps"),
            ),
            capability(
                ProfilerCapabilityNameV4::AttArtifactReferences,
                att,
                Some("capture contains dispatch metadata, not ATT references"),
            ),
            capability(
                ProfilerCapabilityNameV4::DecodedAttEvents,
                false,
                Some("the bundle preserves references and does not decode ATT payloads"),
            ),
            capability(
                ProfilerCapabilityNameV4::WaitAnalysis,
                false,
                Some("no supported decoded ATT wait-event input is present"),
            ),
            capability(ProfilerCapabilityNameV4::CaptureComparison, true, None),
            capability(ProfilerCapabilityNameV4::NextCapturePlanning, true, None),
            capability(
                ProfilerCapabilityNameV4::ExecutionControl,
                false,
                Some("read-only evidence query grants no execution authority"),
            ),
        ]
    }

    fn page(
        &self,
        kind: ProfilerListKindV4,
        page: ProfilerPageRequestV4,
    ) -> Result<ProfilerPageV4, ProfilerQueryErrorV4> {
        if page.limit == 0 || page.limit > self.limits.max_page_items {
            return Err(ProfilerQueryErrorV4::PageLimitOutOfRange);
        }
        let binding = cursor_binding(self.context.bundle_identity.digest, kind)?;
        let start = match page.cursor {
            Some(cursor) if cursor.query_binding == binding => usize::try_from(cursor.position)
                .map_err(|_| ProfilerQueryErrorV4::CursorOutOfRange)?,
            Some(_) => return Err(ProfilerQueryErrorV4::CursorMismatch),
            None => 0,
        };
        let items = self.items(kind)?;
        if start > items.len() {
            return Err(ProfilerQueryErrorV4::CursorOutOfRange);
        }
        let end = start
            .saturating_add(usize::from(page.limit))
            .min(items.len());
        let returned =
            u16::try_from(end - start).map_err(|_| ProfilerQueryErrorV4::SizeOverflow)?;
        let next_cursor = (end < items.len()).then_some(ProfilerCursorV4 {
            query_binding: binding,
            position: end as u64,
        });
        Ok(ProfilerPageV4 {
            context: self.context,
            kind,
            returned,
            next_cursor,
            items: items[start..end].to_vec(),
        })
    }

    fn items(
        &self,
        kind: ProfilerListKindV4,
    ) -> Result<Vec<ProfilerQueryItemV4>, ProfilerQueryErrorV4> {
        let unavailable = |subject, reason| ProfilerQueryItemV4::Unavailable {
            subject,
            reason,
            evidence: self.evidence(TruthOriginV1::Unavailable, None),
        };
        match kind {
            ProfilerListKindV4::Runs => Ok(vec![ProfilerQueryItemV4::Run {
                run_identity: self.bundle.run_identity,
                evidence: self.evidence(TruthOriginV1::Inferred, None),
            }]),
            ProfilerListKindV4::Devices => Ok(self
                .bundle
                .devices
                .iter()
                .cloned()
                .map(|device| ProfilerQueryItemV4::Device {
                    evidence: self.evidence(
                        TruthOriginV1::Declared,
                        device.stable_identity.value.map(|value| value.digest),
                    ),
                    device,
                })
                .collect()),
            ProfilerListKindV4::Dispatches => Ok(match &self.bundle.dispatch_capture {
                Some(capture) => capture
                    .dispatches
                    .iter()
                    .map(|dispatch| ProfilerQueryItemV4::Dispatch {
                        dispatch: dispatch_summary(dispatch, self.context.bundle_identity.digest),
                    })
                    .collect(),
                None => vec![unavailable(
                    "dispatches",
                    "ATT reference manifest has no structured dispatch envelopes",
                )],
            }),
            ProfilerListKindV4::DurationHotspots => Ok(match &self.bundle.dispatch_capture {
                Some(capture) => {
                    let mut dispatches = capture.dispatches.iter().collect::<Vec<_>>();
                    dispatches.sort_by(|left, right| {
                        right
                            .duration_ticks
                            .cmp(&left.duration_ticks)
                            .then_with(|| left.identity.cmp(&right.identity))
                    });
                    dispatches
                        .into_iter()
                        .enumerate()
                        .map(|(rank, dispatch)| ProfilerQueryItemV4::DurationHotspot {
                            hotspot: ProfilerHotspotV4 {
                                rank: rank as u64,
                                dispatch_identity: dispatch.identity,
                                duration_ticks: dispatch.duration_ticks,
                                origin: TruthOriginV1::Inferred,
                                evidence: self.evidence(
                                    TruthOriginV1::Observed,
                                    Some(dispatch.identity),
                                ),
                                limitation: "ranked collector-clock duration; not a causal diagnosis",
                            },
                        })
                        .collect()
                }
                None => vec![unavailable(
                    "duration_hotspots",
                    "ATT reference manifest has no structured dispatch timestamps",
                )],
            }),
            ProfilerListKindV4::AttReferences => Ok(match &self.bundle.att {
                Some(att) => att
                    .references
                    .iter()
                    .cloned()
                    .map(|reference| ProfilerQueryItemV4::AttReference {
                        evidence: self.evidence(
                            reference.content.origin,
                            reference.content.value.map(|value| value.digest),
                        ),
                        reference,
                    })
                    .collect(),
                None => vec![unavailable(
                    "att_references",
                    "dispatch capture contains no ATT manifest",
                )],
            }),
            ProfilerListKindV4::Waits => Ok(vec![unavailable(
                "waits",
                "no supported decoded ATT wait-event input is present; missing events are not treated as absence",
            )]),
        }
    }

    fn plan(&self, goal: ProfilerCaptureGoalV4) -> ProfilerNextCapturePlanV4 {
        let (steps, limitations) = match goal {
            ProfilerCaptureGoalV4::RankDispatchDurations
                if self.bundle.dispatch_capture.is_some() =>
            {
                (
                    Vec::new(),
                    vec![
                        "structured dispatch durations are already available; ranking is not a causal diagnosis",
                    ],
                )
            }
            ProfilerCaptureGoalV4::RankDispatchDurations => (
                vec![ProfilerCaptureStepV4::CollectKernelDispatches],
                vec!["ATT references do not contain structured dispatch timestamps"],
            ),
            ProfilerCaptureGoalV4::DecodeAttCoverage | ProfilerCaptureGoalV4::ExplainWaits => {
                let mut steps = Vec::new();
                if self.bundle.att.is_none() {
                    steps.push(ProfilerCaptureStepV4::CollectAttManifestAndReferencedArtifacts);
                }
                steps.extend([
                    ProfilerCaptureStepV4::DecodeAttWithSupportedRocprofComputeViewer,
                    ProfilerCaptureStepV4::ImportDecodedAttWhenSchemaIsSupported,
                ]);
                (
                    steps,
                    vec![
                        "ATT references are not decoded wave events",
                        "thread trace cannot establish full-grid wave coverage unless the collector reports it",
                        "wait analysis remains unavailable until decoded events have a supported strict schema",
                    ],
                )
            }
        };
        debug_assert!(steps.len() <= MAX_PROFILER_CAPTURE_PLAN_STEPS_V4);
        ProfilerNextCapturePlanV4 {
            goal,
            origin: TruthOriginV1::Inferred,
            steps,
            evidence: vec![self.context.bundle_identity.digest],
            limitations,
        }
    }
}

fn capability(
    name: ProfilerCapabilityNameV4,
    available: bool,
    unavailable_reason: Option<&'static str>,
) -> ProfilerCapabilityV4 {
    ProfilerCapabilityV4 {
        name,
        availability: if available {
            ProfilerAvailabilityV4::Available
        } else {
            ProfilerAvailabilityV4::Unavailable
        },
        reason: (!available).then_some(unavailable_reason).flatten(),
    }
}

fn dispatch_summary(
    dispatch: &CaptureDispatchV1,
    bundle: CaptureIdentityV1,
) -> ProfilerDispatchSummaryV4 {
    ProfilerDispatchSummaryV4 {
        identity: dispatch.identity,
        device_identity: dispatch.device_identity,
        process_index: dispatch.process_index,
        dispatch_index: dispatch.dispatch_index,
        launch: dispatch.launch,
        start_timestamp: dispatch.start_timestamp,
        end_timestamp: dispatch.end_timestamp,
        duration_ticks: dispatch.duration_ticks,
        evidence: ProfilerEvidenceV4 {
            origin: TruthOriginV1::Observed,
            bundle,
            record: Some(dispatch.identity),
        },
    }
}

fn cursor_binding(
    bundle: CaptureIdentityV1,
    kind: ProfilerListKindV4,
) -> Result<CaptureIdentityV1, ProfilerQueryErrorV4> {
    let mut hasher = Sha256::new();
    hasher.update(PROFILER_QUERY_CURSOR_DOMAIN_V4);
    hasher.update(bundle.as_bytes());
    hasher.update([kind as u8]);
    CaptureIdentityV1::new(hasher.finalize().into()).map_err(|_| ProfilerQueryErrorV4::Identity)
}

struct BoundedWriterV4<'a> {
    output: &'a mut Vec<u8>,
    max: u64,
    exceeded: bool,
}

impl Write for BoundedWriterV4<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.write_all(bytes)?;
        Ok(bytes.len())
    }

    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        let max = usize::try_from(self.max).unwrap_or(usize::MAX);
        if self
            .output
            .len()
            .checked_add(bytes.len())
            .is_none_or(|len| len >= max)
        {
            self.exceeded = true;
            return Err(io::Error::other("profiler response limit exceeded"));
        }
        self.output.extend_from_slice(bytes);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub enum ProfilerQueryErrorV4 {
    LimitOutOfRange,
    InputTooLarge,
    Bundle,
    PageLimitOutOfRange,
    CursorMismatch,
    CursorOutOfRange,
    DispatchNotFound,
    InvalidResponse,
    InvalidComparison,
    ResponseTooLarge,
    JsonEncode,
    Identity,
    SizeOverflow,
}

impl fmt::Display for ProfilerQueryErrorV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "semantic profiler query rejected: {self:?}")
    }
}

impl Error for ProfilerQueryErrorV4 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerCompatibilityRequirementV4 {
    Environment,
    CollectorTool,
    CollectorConfiguration,
    StableDevices,
    DispatchWorkload,
    KernelIr,
    Artifact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerCompatibilityStatusV4 {
    Exact,
    Mismatch,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ProfilerCompatibilityFactV4 {
    pub requirement: ProfilerCompatibilityRequirementV4,
    pub status: ProfilerCompatibilityStatusV4,
    pub origin: TruthOriginV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProfilerNumericDeltaV4 {
    pub metric: String,
    pub dimension: String,
    pub baseline_f64_bits: u64,
    pub candidate_f64_bits: u64,
    pub delta_f64_bits: u64,
    pub origin: TruthOriginV1,
    pub baseline_evidence: Vec<CaptureIdentityV1>,
    pub candidate_evidence: Vec<CaptureIdentityV1>,
    pub limitation: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProfilerBundleComparisonV4 {
    pub baseline: ContentIdentityRecordV1,
    pub candidate: ContentIdentityRecordV1,
    pub comparable: bool,
    pub compatibility: Vec<ProfilerCompatibilityFactV4>,
    pub deltas: Vec<ProfilerNumericDeltaV4>,
    pub unavailable: Vec<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerNumericCaptureKindV4 {
    DispatchCountersV2,
    StochasticPcSamplesV3,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProfilerNumericComparisonV4 {
    pub kind: ProfilerNumericCaptureKindV4,
    pub baseline: ContentIdentityRecordV1,
    pub candidate: ContentIdentityRecordV1,
    pub stable_environment: ProfilerCompatibilityStatusV4,
    pub numeric_dimensions_comparable: bool,
    pub deltas: Vec<ProfilerNumericDeltaV4>,
    pub unavailable: Vec<&'static str>,
}

pub fn compare_profiler_bundles_v4(
    baseline_bytes: &[u8],
    candidate_bytes: &[u8],
) -> Result<ProfilerBundleComparisonV4, ProfilerQueryErrorV4> {
    let baseline =
        decode_profiler_bundle_v4(baseline_bytes).map_err(|_| ProfilerQueryErrorV4::Bundle)?;
    let candidate =
        decode_profiler_bundle_v4(candidate_bytes).map_err(|_| ProfilerQueryErrorV4::Bundle)?;
    let baseline_id = profiler_bundle_content_identity_v4(baseline_bytes)
        .map_err(|_| ProfilerQueryErrorV4::Bundle)?;
    let candidate_id = profiler_bundle_content_identity_v4(candidate_bytes)
        .map_err(|_| ProfilerQueryErrorV4::Bundle)?;
    let facts = vec![
        comparison_fact(
            ProfilerCompatibilityRequirementV4::Environment,
            baseline.environment.value == candidate.environment.value,
            TruthOriginV1::Declared,
        ),
        comparison_fact(
            ProfilerCompatibilityRequirementV4::CollectorTool,
            baseline.collector_tool.value == candidate.collector_tool.value,
            TruthOriginV1::Declared,
        ),
        comparison_fact(
            ProfilerCompatibilityRequirementV4::CollectorConfiguration,
            baseline.collector_configuration.value == candidate.collector_configuration.value,
            TruthOriginV1::Declared,
        ),
        comparison_fact(
            ProfilerCompatibilityRequirementV4::StableDevices,
            baseline
                .devices
                .iter()
                .map(|device| device.stable_identity.value)
                .eq(candidate
                    .devices
                    .iter()
                    .map(|device| device.stable_identity.value)),
            TruthOriginV1::Declared,
        ),
        dispatch_workload_comparison_fact(&baseline, &candidate),
        dispatch_comparison_fact(
            ProfilerCompatibilityRequirementV4::KernelIr,
            &baseline,
            &candidate,
            |left, right| left.kernel_ir == right.kernel_ir,
        ),
        dispatch_identity_comparison_fact(
            ProfilerCompatibilityRequirementV4::Artifact,
            &baseline,
            &candidate,
            |dispatch| dispatch.artifact,
        ),
    ];
    let comparable = facts
        .iter()
        .all(|fact| fact.status == ProfilerCompatibilityStatusV4::Exact)
        && baseline.dispatch_capture.is_some()
        && candidate.dispatch_capture.is_some();
    let mut deltas = Vec::new();
    let mut unavailable = Vec::new();
    if comparable {
        let left = baseline.dispatch_capture.as_ref().unwrap();
        let right = candidate.dispatch_capture.as_ref().unwrap();
        let left_total = left
            .dispatches
            .iter()
            .try_fold(0_u64, |sum, dispatch| {
                sum.checked_add(dispatch.duration_ticks)
            })
            .ok_or(ProfilerQueryErrorV4::SizeOverflow)?;
        let right_total = right
            .dispatches
            .iter()
            .try_fold(0_u64, |sum, dispatch| {
                sum.checked_add(dispatch.duration_ticks)
            })
            .ok_or(ProfilerQueryErrorV4::SizeOverflow)?;
        if left_total <= MAX_EXACT_F64_INTEGER_V4 && right_total <= MAX_EXACT_F64_INTEGER_V4 {
            deltas.push(numeric_delta(
                "dispatch_total_duration_ticks",
                "all_dispatches",
                left_total as f64,
                right_total as f64,
                left.dispatches
                    .iter()
                    .map(|dispatch| dispatch.identity)
                    .collect(),
                right
                    .dispatches
                    .iter()
                    .map(|dispatch| dispatch.identity)
                    .collect(),
                "exact integer delta encoded as binary64 in an opaque collector clock; no wall-clock conversion or causal claim",
            )?);
        } else {
            unavailable.push(
                "dispatch duration total exceeds the exact binary64 integer range; no rounded delta is emitted",
            );
        }
    } else {
        unavailable.push("numeric dispatch deltas require exact environment, tool, configuration, stable device, dispatch workload, kernel IR, and artifact comparability");
    }
    unavailable.push(
        "decoded ATT and wait-event deltas are unavailable without supported decoded ATT evidence",
    );
    unavailable.push(
        "environment, tool, configuration, and stable-device comparability is exact equality of caller-declared content identities, not runtime authentication",
    );
    unavailable.push(
        "dispatch workload comparison covers sequence, device assignment, and launch geometry; argument and input-content identities are not represented",
    );
    Ok(ProfilerBundleComparisonV4 {
        baseline: baseline_id,
        candidate: candidate_id,
        comparable,
        compatibility: facts,
        deltas,
        unavailable,
    })
}

pub fn encode_profiler_bundle_comparison_v4(
    comparison: &ProfilerBundleComparisonV4,
) -> Result<Vec<u8>, ProfilerQueryErrorV4> {
    validate_bundle_comparison_v4(comparison)?;
    encode_bounded_profiler_value_v4(comparison)
}

pub fn encode_profiler_numeric_comparison_v4(
    comparison: &ProfilerNumericComparisonV4,
) -> Result<Vec<u8>, ProfilerQueryErrorV4> {
    validate_numeric_comparison_v4(comparison)?;
    encode_bounded_profiler_value_v4(comparison)
}

fn validate_bundle_comparison_v4(
    comparison: &ProfilerBundleComparisonV4,
) -> Result<(), ProfilerQueryErrorV4> {
    let requirements = [
        ProfilerCompatibilityRequirementV4::Environment,
        ProfilerCompatibilityRequirementV4::CollectorTool,
        ProfilerCompatibilityRequirementV4::CollectorConfiguration,
        ProfilerCompatibilityRequirementV4::StableDevices,
        ProfilerCompatibilityRequirementV4::DispatchWorkload,
        ProfilerCompatibilityRequirementV4::KernelIr,
        ProfilerCompatibilityRequirementV4::Artifact,
    ];
    let facts_are_valid = comparison.compatibility.len() == requirements.len()
        && comparison
            .compatibility
            .iter()
            .zip(requirements)
            .all(|(fact, requirement)| {
                fact.requirement == requirement
                    && match fact.status {
                        ProfilerCompatibilityStatusV4::Exact
                        | ProfilerCompatibilityStatusV4::Mismatch => {
                            fact.origin == TruthOriginV1::Declared
                        }
                        ProfilerCompatibilityStatusV4::Unavailable => {
                            fact.origin == TruthOriginV1::Unavailable
                        }
                    }
            });
    let derived_comparable = comparison
        .compatibility
        .iter()
        .all(|fact| fact.status == ProfilerCompatibilityStatusV4::Exact);
    if !facts_are_valid
        || comparison.comparable != derived_comparable
        || (!comparison.comparable && !comparison.deltas.is_empty())
        || comparison.unavailable.is_empty()
        || !comparison
            .unavailable
            .iter()
            .all(|value| valid_comparison_text(value))
        || !comparison.deltas.iter().all(valid_numeric_delta_v4)
    {
        return Err(ProfilerQueryErrorV4::InvalidComparison);
    }
    Ok(())
}

fn validate_numeric_comparison_v4(
    comparison: &ProfilerNumericComparisonV4,
) -> Result<(), ProfilerQueryErrorV4> {
    let kind_is_valid = match comparison.kind {
        ProfilerNumericCaptureKindV4::DispatchCountersV2 => {
            comparison.stable_environment == ProfilerCompatibilityStatusV4::Unavailable
                && (comparison.numeric_dimensions_comparable || comparison.deltas.is_empty())
        }
        ProfilerNumericCaptureKindV4::StochasticPcSamplesV3 => {
            comparison.stable_environment == ProfilerCompatibilityStatusV4::Unavailable
                && !comparison.numeric_dimensions_comparable
                && comparison.deltas.is_empty()
        }
    };
    if !kind_is_valid
        || comparison.unavailable.is_empty()
        || !comparison
            .unavailable
            .iter()
            .all(|value| valid_comparison_text(value))
        || !comparison.deltas.iter().all(valid_numeric_delta_v4)
    {
        return Err(ProfilerQueryErrorV4::InvalidComparison);
    }
    Ok(())
}

fn valid_numeric_delta_v4(delta: &ProfilerNumericDeltaV4) -> bool {
    let baseline = f64::from_bits(delta.baseline_f64_bits);
    let candidate = f64::from_bits(delta.candidate_f64_bits);
    let difference = f64::from_bits(delta.delta_f64_bits);
    !delta.metric.is_empty()
        && valid_comparison_text(&delta.metric)
        && !delta.dimension.is_empty()
        && valid_comparison_text(&delta.dimension)
        && valid_comparison_text(delta.limitation)
        && delta.origin == TruthOriginV1::Inferred
        && !delta.baseline_evidence.is_empty()
        && !delta.candidate_evidence.is_empty()
        && baseline.is_finite()
        && candidate.is_finite()
        && difference.is_finite()
        && (candidate - baseline).to_bits() == delta.delta_f64_bits
}

fn valid_comparison_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= 4_096 && !value.contains('\0')
}

fn encode_bounded_profiler_value_v4(
    value: &impl Serialize,
) -> Result<Vec<u8>, ProfilerQueryErrorV4> {
    let mut output = Vec::new();
    let mut writer = BoundedWriterV4 {
        output: &mut output,
        max: MAX_PROFILER_QUERY_RESPONSE_BYTES_V4,
        exceeded: false,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| {
        if writer.exceeded {
            ProfilerQueryErrorV4::ResponseTooLarge
        } else {
            ProfilerQueryErrorV4::JsonEncode
        }
    })?;
    output.push(b'\n');
    Ok(output)
}

fn comparison_fact(
    requirement: ProfilerCompatibilityRequirementV4,
    exact: bool,
    origin: TruthOriginV1,
) -> ProfilerCompatibilityFactV4 {
    ProfilerCompatibilityFactV4 {
        requirement,
        status: if exact {
            ProfilerCompatibilityStatusV4::Exact
        } else {
            ProfilerCompatibilityStatusV4::Mismatch
        },
        origin,
    }
}

fn dispatch_comparison_fact(
    requirement: ProfilerCompatibilityRequirementV4,
    left: &SemanticProfilerBundleV4,
    right: &SemanticProfilerBundleV4,
    predicate: impl Fn(&CaptureDispatchV1, &CaptureDispatchV1) -> bool,
) -> ProfilerCompatibilityFactV4 {
    if left.dispatch_capture.is_none() || right.dispatch_capture.is_none() {
        return ProfilerCompatibilityFactV4 {
            requirement,
            status: ProfilerCompatibilityStatusV4::Unavailable,
            origin: TruthOriginV1::Unavailable,
        };
    }
    comparison_fact(
        requirement,
        dispatch_pairs_match(left, right, predicate),
        TruthOriginV1::Declared,
    )
}

fn dispatch_identity_comparison_fact(
    requirement: ProfilerCompatibilityRequirementV4,
    left: &SemanticProfilerBundleV4,
    right: &SemanticProfilerBundleV4,
    identity: impl Fn(&CaptureDispatchV1) -> IdentityFactV1,
) -> ProfilerCompatibilityFactV4 {
    let (Some(left), Some(right)) = (&left.dispatch_capture, &right.dispatch_capture) else {
        return ProfilerCompatibilityFactV4 {
            requirement,
            status: ProfilerCompatibilityStatusV4::Unavailable,
            origin: TruthOriginV1::Unavailable,
        };
    };
    let available = left
        .dispatches
        .iter()
        .chain(&right.dispatches)
        .all(|dispatch| {
            let fact = identity(dispatch);
            fact.origin != TruthOriginV1::Unavailable && fact.value.is_some()
        });
    if !available {
        return ProfilerCompatibilityFactV4 {
            requirement,
            status: ProfilerCompatibilityStatusV4::Unavailable,
            origin: TruthOriginV1::Unavailable,
        };
    }
    comparison_fact(
        requirement,
        left.dispatches.len() == right.dispatches.len()
            && left
                .dispatches
                .iter()
                .zip(&right.dispatches)
                .all(|(left, right)| identity(left) == identity(right)),
        TruthOriginV1::Declared,
    )
}

fn dispatch_workload_comparison_fact(
    left: &SemanticProfilerBundleV4,
    right: &SemanticProfilerBundleV4,
) -> ProfilerCompatibilityFactV4 {
    if left.dispatch_capture.is_none() || right.dispatch_capture.is_none() {
        return ProfilerCompatibilityFactV4 {
            requirement: ProfilerCompatibilityRequirementV4::DispatchWorkload,
            status: ProfilerCompatibilityStatusV4::Unavailable,
            origin: TruthOriginV1::Unavailable,
        };
    }
    comparison_fact(
        ProfilerCompatibilityRequirementV4::DispatchWorkload,
        dispatch_workloads_match(left, right),
        TruthOriginV1::Declared,
    )
}

fn dispatch_workloads_match(
    left: &SemanticProfilerBundleV4,
    right: &SemanticProfilerBundleV4,
) -> bool {
    let (Some(left_capture), Some(right_capture)) =
        (&left.dispatch_capture, &right.dispatch_capture)
    else {
        return false;
    };
    left_capture.dispatches.len() == right_capture.dispatches.len()
        && left_capture
            .dispatches
            .iter()
            .zip(&right_capture.dispatches)
            .all(|(left_dispatch, right_dispatch)| {
                let left_device = stable_dispatch_device(left, left_dispatch);
                let right_device = stable_dispatch_device(right, right_dispatch);
                left_dispatch.process_index == right_dispatch.process_index
                    && left_dispatch.dispatch_index == right_dispatch.dispatch_index
                    && left_dispatch.source_record_ordinal == right_dispatch.source_record_ordinal
                    && left_dispatch.launch == right_dispatch.launch
                    && left_device.is_some()
                    && left_device == right_device
            })
}

fn stable_dispatch_device(
    bundle: &SemanticProfilerBundleV4,
    dispatch: &CaptureDispatchV1,
) -> Option<ContentIdentityRecordV1> {
    let capture = bundle.dispatch_capture.as_ref()?;
    let ordinal = capture
        .devices
        .iter()
        .position(|device| device.identity == dispatch.device_identity)?;
    bundle.devices.get(ordinal)?.stable_identity.value
}

fn dispatch_pairs_match(
    left: &SemanticProfilerBundleV4,
    right: &SemanticProfilerBundleV4,
    predicate: impl Fn(&CaptureDispatchV1, &CaptureDispatchV1) -> bool,
) -> bool {
    let (Some(left), Some(right)) = (&left.dispatch_capture, &right.dispatch_capture) else {
        return false;
    };
    left.dispatches.len() == right.dispatches.len()
        && left
            .dispatches
            .iter()
            .zip(&right.dispatches)
            .all(|(left, right)| predicate(left, right))
}

fn numeric_delta(
    metric: &str,
    dimension: &str,
    baseline: f64,
    candidate: f64,
    baseline_evidence: Vec<CaptureIdentityV1>,
    candidate_evidence: Vec<CaptureIdentityV1>,
    limitation: &'static str,
) -> Result<ProfilerNumericDeltaV4, ProfilerQueryErrorV4> {
    let delta = candidate - baseline;
    if !baseline.is_finite() || !candidate.is_finite() || !delta.is_finite() {
        return Err(ProfilerQueryErrorV4::SizeOverflow);
    }
    Ok(ProfilerNumericDeltaV4 {
        metric: metric.to_owned(),
        dimension: dimension.to_owned(),
        baseline_f64_bits: baseline.to_bits(),
        candidate_f64_bits: candidate.to_bits(),
        delta_f64_bits: delta.to_bits(),
        origin: TruthOriginV1::Inferred,
        baseline_evidence,
        candidate_evidence,
        limitation,
    })
}

const MAX_EXACT_F64_INTEGER_V4: u64 = 1_u64 << 53;

pub fn compare_counter_values_v2(
    baseline_bytes: &[u8],
    candidate_bytes: &[u8],
) -> Result<ProfilerNumericComparisonV4, ProfilerQueryErrorV4> {
    let baseline = fe2o3_semantic_import::decode_counter_capture_v2(baseline_bytes)
        .map_err(|_| ProfilerQueryErrorV4::Bundle)?;
    let candidate = fe2o3_semantic_import::decode_counter_capture_v2(candidate_bytes)
        .map_err(|_| ProfilerQueryErrorV4::Bundle)?;
    let baseline_id = fe2o3_semantic_import::counter_capture_content_identity_v2(baseline_bytes)
        .map_err(|_| ProfilerQueryErrorV4::Bundle)?;
    let candidate_id = fe2o3_semantic_import::counter_capture_content_identity_v2(candidate_bytes)
        .map_err(|_| ProfilerQueryErrorV4::Bundle)?;
    let left_definitions = baseline
        .counter_definitions
        .iter()
        .map(|definition| {
            (
                &definition.name,
                definition.is_constant,
                definition.is_derived,
            )
        })
        .collect::<Vec<_>>();
    let right_definitions = candidate
        .counter_definitions
        .iter()
        .map(|definition| {
            (
                &definition.name,
                definition.is_constant,
                definition.is_derived,
            )
        })
        .collect::<Vec<_>>();
    if left_definitions != right_definitions
        || baseline.dispatches.len() != candidate.dispatches.len()
        || !baseline
            .dispatches
            .iter()
            .zip(&candidate.dispatches)
            .all(|(left, right)| {
                left.kernel_ir == right.kernel_ir
                    && left.artifact == right.artifact
                    && left.launch == right.launch
            })
    {
        return Ok(ProfilerNumericComparisonV4 {
            kind: ProfilerNumericCaptureKindV4::DispatchCountersV2,
            baseline: baseline_id,
            candidate: candidate_id,
            stable_environment: ProfilerCompatibilityStatusV4::Unavailable,
            numeric_dimensions_comparable: false,
            deltas: Vec::new(),
            unavailable: vec![
                "counter deltas require exact counter definitions and matching dispatch declarations",
                "Semantic Counter Capture V2 has no stable environment identity",
            ],
        });
    }
    let left_by_id = baseline
        .counter_definitions
        .iter()
        .enumerate()
        .map(|(index, definition)| (definition.identity, index))
        .collect::<BTreeMap<_, _>>();
    let right_by_id = candidate
        .counter_definitions
        .iter()
        .enumerate()
        .map(|(index, definition)| (definition.identity, index))
        .collect::<BTreeMap<_, _>>();
    let left = aggregate_counters(
        &baseline.dispatches,
        &left_by_id,
        baseline.counter_definitions.len(),
    )?;
    let right = aggregate_counters(
        &candidate.dispatches,
        &right_by_id,
        candidate.counter_definitions.len(),
    )?;
    let mut deltas = Vec::new();
    for index in 0..left.len() {
        let baseline_evidence = counter_evidence(&baseline.dispatches, &left_by_id, index);
        let candidate_evidence = counter_evidence(&candidate.dispatches, &right_by_id, index);
        if baseline_evidence.is_empty() || candidate_evidence.is_empty() {
            continue;
        }
        deltas.push(numeric_delta(
            "counter_total_raw_value",
            &baseline.counter_definitions[index].name,
            left[index],
            right[index],
            baseline_evidence,
            candidate_evidence,
            "deterministic binary64 sum of observed raw values for exact matching definitions and dispatch declarations; stable environment identity is unavailable in V2",
        )?);
    }
    Ok(ProfilerNumericComparisonV4 {
        kind: ProfilerNumericCaptureKindV4::DispatchCountersV2,
        baseline: baseline_id,
        candidate: candidate_id,
        stable_environment: ProfilerCompatibilityStatusV4::Unavailable,
        numeric_dimensions_comparable: true,
        deltas,
        unavailable: vec![
            "Semantic Counter Capture V2 has no stable environment identity; numeric equality is not an environment-controlled performance conclusion",
            "counter dimensions without observed records in both captures remain unavailable and are not treated as zero",
        ],
    })
}

fn counter_evidence(
    dispatches: &[fe2o3_semantic_import::CounterDispatchV2],
    definitions: &BTreeMap<CaptureIdentityV1, usize>,
    index: usize,
) -> Vec<CaptureIdentityV1> {
    dispatches
        .iter()
        .flat_map(|dispatch| &dispatch.values)
        .filter(|value| definitions.get(&value.counter_identity) == Some(&index))
        .map(|value| value.identity)
        .collect()
}

fn aggregate_counters(
    dispatches: &[fe2o3_semantic_import::CounterDispatchV2],
    definitions: &BTreeMap<CaptureIdentityV1, usize>,
    count: usize,
) -> Result<Vec<f64>, ProfilerQueryErrorV4> {
    let mut totals = vec![0.0_f64; count];
    for value in dispatches.iter().flat_map(|dispatch| &dispatch.values) {
        let index = *definitions
            .get(&value.counter_identity)
            .ok_or(ProfilerQueryErrorV4::Bundle)?;
        totals[index] += value.value();
        if !totals[index].is_finite() {
            return Err(ProfilerQueryErrorV4::SizeOverflow);
        }
    }
    Ok(totals)
}

pub fn compare_pc_sample_counts_v3(
    baseline_bytes: &[u8],
    candidate_bytes: &[u8],
) -> Result<ProfilerNumericComparisonV4, ProfilerQueryErrorV4> {
    let baseline = fe2o3_semantic_import::decode_pc_sample_capture_v3(baseline_bytes)
        .map_err(|_| ProfilerQueryErrorV4::Bundle)?;
    let candidate = fe2o3_semantic_import::decode_pc_sample_capture_v3(candidate_bytes)
        .map_err(|_| ProfilerQueryErrorV4::Bundle)?;
    let baseline_id = fe2o3_semantic_import::pc_sample_capture_content_identity_v3(baseline_bytes)
        .map_err(|_| ProfilerQueryErrorV4::Bundle)?;
    let candidate_id =
        fe2o3_semantic_import::pc_sample_capture_content_identity_v3(candidate_bytes)
            .map_err(|_| ProfilerQueryErrorV4::Bundle)?;
    let mut unavailable = vec![
        "Semantic PC Sample Capture V3 has no stable environment identity",
        "V3 code-object identities are capture-local and cannot establish a stable relative-PC join across runs",
    ];
    if baseline.coverage.sampling != candidate.coverage.sampling
        || baseline.dispatches.len() != candidate.dispatches.len()
        || !baseline
            .dispatches
            .iter()
            .zip(&candidate.dispatches)
            .all(|(left, right)| {
                left.kernel_ir == right.kernel_ir
                    && left.artifact == right.artifact
                    && left.launch == right.launch
            })
    {
        unavailable.push(
            "PC sample deltas also require exact sampling configuration and matching dispatch declarations",
        );
    }
    Ok(ProfilerNumericComparisonV4 {
        kind: ProfilerNumericCaptureKindV4::StochasticPcSamplesV3,
        baseline: baseline_id,
        candidate: candidate_id,
        stable_environment: ProfilerCompatibilityStatusV4::Unavailable,
        numeric_dimensions_comparable: false,
        deltas: Vec::new(),
        unavailable,
    })
}
