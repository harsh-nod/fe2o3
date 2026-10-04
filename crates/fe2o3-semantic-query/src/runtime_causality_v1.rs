//! Bounded runtime lifecycle and causality queries over direct-KFD evidence.
//!
//! KFD Runtime Profile V1 records producer order, queue/stream membership,
//! host staging, dispatch publication/completion, and resource release. It does
//! not record device-copy engines or dispatch dependency edges. Bundle V4 can
//! be retained beside the runtime profile, but has no admitted common dispatch
//! identity or clock relation. This module keeps those boundaries explicit.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::io::{BufRead, Read, Write};

use fe2o3_profiler_protocol::{
    KfdProfileBindingV1, KfdProfileHostContentV1, KfdProfileHostTimingV1, KfdProfileLaunchV1,
    KfdRuntimeProfileEventKindV1, KfdRuntimeProfileV1, MAX_KFD_RUNTIME_PROFILE_BYTES_V1,
    ProfileContentIdentityV1, ProfileIdentityV1, decode_kfd_runtime_profile_v1,
    kfd_runtime_profile_content_identity_v1,
};
use fe2o3_semantic_import::{
    CaptureIdentityV1, ContentIdentityRecordV1, LossStatusV1, MAX_PROFILER_BUNDLE_BYTES_V4,
    SemanticProfilerBundleV4, TruthOriginV1, decode_profiler_bundle_v4,
    profiler_bundle_content_identity_v4,
};
use serde::de::Visitor;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const AGENT_RUNTIME_CAUSALITY_REQUEST_SCHEMA_V1: &str =
    "fe2o3-agent-runtime-causality-request-v1";
pub const AGENT_RUNTIME_CAUSALITY_RESPONSE_SCHEMA_V1: &str =
    "fe2o3-agent-runtime-causality-response-v1";
pub const MAX_AGENT_RUNTIME_CAUSALITY_REQUEST_ATTEMPTS_V1: u64 = 64;
pub const MAX_AGENT_RUNTIME_CAUSALITY_PAGE_ITEMS_V1: u16 = 4_096;
pub const MAX_RUNTIME_CAUSALITY_LIFECYCLE_EDGES_V1: u64 = 1_114_112;
pub const MAX_AGENT_RUNTIME_CAUSALITY_REQUEST_BYTES_V1: u64 =
    2 * (MAX_KFD_RUNTIME_PROFILE_BYTES_V1 + MAX_PROFILER_BUNDLE_BYTES_V4) + 64 * 1024;
pub const MAX_AGENT_RUNTIME_CAUSALITY_RESPONSE_BYTES_V1: u64 = 2 * 1024 * 1024;

const INPUT_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.runtime-causality.input.v1\0";
const BINDING_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.runtime-causality.binding.v1\0";
const RULE_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.runtime-causality.rule.v1\0";
const EDGE_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.runtime-causality.edge.v1\0";
const QUERY_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.runtime-causality.query.v1\0";
const CURSOR_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.runtime-causality.cursor.v1\0";
const LIFECYCLE_RULE_V1: &[u8] = b"validated-kfd-runtime-profile-v1-lifecycle-order";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCausalityAuthorityV1 {
    ReadOnlyNoRuntimeProfilerCollectionAttachDispatchOrSchedulingAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCausalityOperationV1 {
    DiscoverCapabilities,
    Open,
    Binding,
    Page,
    InspectDispatch,
    ProfilerJuxtaposition,
    Close,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCausalityPageKindV1 {
    RuntimeEvents,
    LifecycleEdges,
    Dispatches,
    HostStaging,
    Dependencies,
    DeviceCopies,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCausalityUnavailableReasonV1 {
    NoCanonicalDependencyEvents,
    NoDeviceCopyEngineEvents,
    ProfilerBundleNotSupplied,
    ProfilerBundleHasNoDispatchEnvelopes,
    NoAdmittedCommonDispatchIdentity,
    NoAdmittedClockCorrelation,
    CaptureIncompleteOrDispatchStillLive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeCausalityAvailabilityV1 {
    Available {
        origin: TruthOriginV1,
    },
    Unavailable {
        origin: TruthOriginV1,
        reason: RuntimeCausalityUnavailableReasonV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCausalityInputIdentityV1 {
    pub digest: CaptureIdentityV1,
    pub byte_len: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCausalityCoverageV1 {
    pub origin: TruthOriginV1,
    pub observed_runtime_events: u64,
    pub dropped_runtime_events: u64,
    pub complete_runtime_operation_history: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCausalityCapabilitiesV1 {
    pub operations: Vec<RuntimeCausalityOperationV1>,
    pub page_kinds: Vec<RuntimeCausalityPageKindV1>,
    pub authority: RuntimeCausalityAuthorityV1,
    pub exact_input_encoding: &'static str,
    pub producer_order_domain: &'static str,
    pub lifecycle_edge_rule: CaptureIdentityV1,
    pub dependency_events: RuntimeCausalityAvailabilityV1,
    pub device_copy_engine_events: RuntimeCausalityAvailabilityV1,
    pub profiler_dispatch_join: RuntimeCausalityAvailabilityV1,
    pub cross_clock_relation: RuntimeCausalityAvailabilityV1,
    pub max_runtime_profile_bytes: u64,
    pub max_profiler_bundle_bytes: u64,
    pub max_lifecycle_edges: u64,
    pub max_page_items: u16,
    pub max_request_attempts: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCausalityBindingV1 {
    pub binding_identity: CaptureIdentityV1,
    pub runtime_input: RuntimeCausalityInputIdentityV1,
    pub runtime_content_identity: ProfileContentIdentityV1,
    pub profiler_input: Option<RuntimeCausalityInputIdentityV1>,
    pub profiler_content_identity: Option<ContentIdentityRecordV1>,
    pub runtime_capture_scope: ProfileIdentityV1,
    pub runtime_device: ProfileIdentityV1,
    pub coverage: RuntimeCausalityCoverageV1,
    pub authority: RuntimeCausalityAuthorityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEventKindV1 {
    NativeQueueCreated,
    NativeQueueDestroyed,
    StreamCreated,
    StreamDestroyed,
    AllocationCreated,
    HostWrite,
    HostRead,
    AllocationReleased,
    ModuleLoaded,
    KernelResolved,
    ModuleUnloaded,
    DispatchPublished,
    DispatchCompleted,
    SubmissionReleased,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeEventSummaryV1 {
    pub event_identity: ProfileIdentityV1,
    pub sequence: u64,
    pub producer_order_domain: &'static str,
    pub origin: TruthOriginV1,
    pub kind: RuntimeEventKindV1,
    pub primary_resource: ProfileIdentityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeLifecycleEdgeKindV1 {
    QueueExistsBeforeDispatchPublication,
    StreamExistsBeforeDispatchPublication,
    AllocationExistsBeforeDispatchPublication,
    DispatchPublicationBeforeCompletion,
    DispatchCompletionBeforeSubmissionRelease,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeLifecycleEdgeV1 {
    pub edge_identity: CaptureIdentityV1,
    pub kind: RuntimeLifecycleEdgeKindV1,
    pub origin: TruthOriginV1,
    pub inference_rule: CaptureIdentityV1,
    pub predecessor_event: ProfileIdentityV1,
    pub predecessor_sequence: u64,
    pub successor_event: ProfileIdentityV1,
    pub successor_sequence: u64,
    pub producer_order_domain: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeDispatchEventRefV1 {
    Observed {
        origin: TruthOriginV1,
        event_identity: ProfileIdentityV1,
        sequence: u64,
    },
    Unavailable {
        origin: TruthOriginV1,
        reason: RuntimeCausalityUnavailableReasonV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeDispatchSummaryV1 {
    pub dispatch: ProfileIdentityV1,
    pub publication_event: ProfileIdentityV1,
    pub publication_sequence: u64,
    pub queue: ProfileIdentityV1,
    pub stream: ProfileIdentityV1,
    pub kernel: ProfileIdentityV1,
    pub completion: RuntimeDispatchEventRefV1,
    pub release: RuntimeDispatchEventRefV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeDispatchDetailV1 {
    pub summary: RuntimeDispatchSummaryV1,
    pub launch: KfdProfileLaunchV1,
    pub bindings: Vec<KfdProfileBindingV1>,
    pub host_timing: Option<KfdProfileHostTimingV1>,
    pub host_timing_origin: RuntimeCausalityAvailabilityV1,
    pub dependency_events: RuntimeCausalityAvailabilityV1,
    pub device_copy_events: RuntimeCausalityAvailabilityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeHostStagingKindV1 {
    HostWrite,
    HostRead,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeHostStagingV1 {
    pub event_identity: ProfileIdentityV1,
    pub sequence: u64,
    pub producer_order_domain: &'static str,
    pub origin: TruthOriginV1,
    pub kind: RuntimeHostStagingKindV1,
    pub allocation: ProfileIdentityV1,
    pub byte_offset: u64,
    pub content: KfdProfileHostContentV1,
    pub device_copy_relation: RuntimeCausalityAvailabilityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "item", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeCausalityPageItemV1 {
    RuntimeEvent(RuntimeEventSummaryV1),
    LifecycleEdge(RuntimeLifecycleEdgeV1),
    Dispatch(RuntimeDispatchSummaryV1),
    HostStaging(RuntimeHostStagingV1),
}

impl RuntimeCausalityPageItemV1 {
    fn identity(self) -> [u8; 32] {
        match self {
            Self::RuntimeEvent(item) => item.event_identity.as_bytes(),
            Self::LifecycleEdge(item) => item.edge_identity.as_bytes(),
            Self::Dispatch(item) => item.publication_event.as_bytes(),
            Self::HostStaging(item) => item.event_identity.as_bytes(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCausalityCursorV1 {
    pub binding_identity: CaptureIdentityV1,
    pub query_identity: CaptureIdentityV1,
    pub next_ordinal: u64,
    pub preceding_item_identity: CaptureIdentityV1,
    pub cursor_identity: CaptureIdentityV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCausalityPageV1 {
    pub binding_identity: CaptureIdentityV1,
    pub kind: RuntimeCausalityPageKindV1,
    pub availability: RuntimeCausalityAvailabilityV1,
    pub coverage: RuntimeCausalityCoverageV1,
    pub total_items: u64,
    pub returned: u16,
    pub items: Vec<RuntimeCausalityPageItemV1>,
    pub next_cursor: Option<RuntimeCausalityCursorV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeProfilerJuxtapositionStateV1 {
    NotSupplied,
    BoundWithoutDispatchOrClockJoin,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeProfilerJuxtapositionV1 {
    pub binding_identity: CaptureIdentityV1,
    pub state: RuntimeProfilerJuxtapositionStateV1,
    pub runtime_input: RuntimeCausalityInputIdentityV1,
    pub profiler_input: Option<RuntimeCausalityInputIdentityV1>,
    pub profiler_content_identity: Option<ContentIdentityRecordV1>,
    pub profiler_run_identity: Option<CaptureIdentityV1>,
    pub runtime_producer_order_domain: &'static str,
    pub profiler_local_tick_coordinates: RuntimeCausalityAvailabilityV1,
    pub profiler_local_tick_description: Option<&'static str>,
    pub dispatch_relation: RuntimeCausalityAvailabilityV1,
    pub cross_clock_relation: RuntimeCausalityAvailabilityV1,
    pub cross_clock_uncertainty: RuntimeCausalityAvailabilityV1,
    pub runtime_coverage: RuntimeCausalityCoverageV1,
    pub profiler_loss: Option<LossStatusV1>,
}

#[derive(Clone, Debug)]
struct IndexedDispatchV1 {
    summary: RuntimeDispatchSummaryV1,
    launch: KfdProfileLaunchV1,
    bindings: Vec<KfdProfileBindingV1>,
    host_timing: Option<KfdProfileHostTimingV1>,
}

struct RuntimeIndexV1 {
    events: Vec<RuntimeEventSummaryV1>,
    edges: Vec<RuntimeLifecycleEdgeV1>,
    dispatches: Vec<IndexedDispatchV1>,
    host_staging: Vec<RuntimeHostStagingV1>,
}

#[derive(Clone, Debug)]
pub struct RuntimeCausalitySessionV1 {
    binding: RuntimeCausalityBindingV1,
    events: Vec<RuntimeEventSummaryV1>,
    edges: Vec<RuntimeLifecycleEdgeV1>,
    dispatches: Vec<IndexedDispatchV1>,
    host_staging: Vec<RuntimeHostStagingV1>,
    profiler: Option<SemanticProfilerBundleV4>,
}

impl RuntimeCausalitySessionV1 {
    pub fn open(
        runtime_bytes: &[u8],
        profiler_bytes: Option<&[u8]>,
    ) -> Result<Self, RuntimeCausalityErrorV1> {
        let runtime = decode_kfd_runtime_profile_v1(runtime_bytes)
            .map_err(|_| RuntimeCausalityErrorV1::RuntimeProfileRejected)?;
        let runtime_content_identity = kfd_runtime_profile_content_identity_v1(runtime_bytes)
            .map_err(|_| RuntimeCausalityErrorV1::RuntimeProfileRejected)?;
        let runtime_input = input_identity(runtime_bytes)?;
        let (profiler, profiler_input, profiler_content_identity) = match profiler_bytes {
            Some(bytes) => {
                let bundle = decode_profiler_bundle_v4(bytes)
                    .map_err(|_| RuntimeCausalityErrorV1::ProfilerBundleRejected)?;
                let content = profiler_bundle_content_identity_v4(bytes)
                    .map_err(|_| RuntimeCausalityErrorV1::ProfilerBundleRejected)?;
                (Some(bundle), Some(input_identity(bytes)?), Some(content))
            }
            None => (None, None, None),
        };
        let binding_identity =
            binding_identity(runtime_content_identity, profiler_content_identity.as_ref())?;
        let coverage = RuntimeCausalityCoverageV1 {
            origin: TruthOriginV1::Observed,
            observed_runtime_events: runtime.coverage.observed_events,
            dropped_runtime_events: runtime.coverage.dropped_events,
            complete_runtime_operation_history: runtime.coverage.complete_runtime_operation_history,
        };
        let binding = RuntimeCausalityBindingV1 {
            binding_identity,
            runtime_input,
            runtime_content_identity,
            profiler_input,
            profiler_content_identity,
            runtime_capture_scope: runtime.capture_scope,
            runtime_device: runtime.device.identity,
            coverage,
            authority: RuntimeCausalityAuthorityV1::ReadOnlyNoRuntimeProfilerCollectionAttachDispatchOrSchedulingAuthority,
        };
        let index = index_runtime(&runtime)?;
        validate_graph(&index.events, &index.edges)?;
        Ok(Self {
            binding,
            events: index.events,
            edges: index.edges,
            dispatches: index.dispatches,
            host_staging: index.host_staging,
            profiler,
        })
    }

    pub const fn binding(&self) -> &RuntimeCausalityBindingV1 {
        &self.binding
    }

    pub fn inspect_dispatch(
        &self,
        dispatch: ProfileIdentityV1,
    ) -> Result<RuntimeDispatchDetailV1, RuntimeCausalityErrorV1> {
        let record = self
            .dispatches
            .iter()
            .find(|record| record.summary.dispatch == dispatch)
            .ok_or(RuntimeCausalityErrorV1::UnknownDispatch)?;
        let host_timing_origin = if record.host_timing.is_some() {
            available_observed()
        } else {
            unavailable(RuntimeCausalityUnavailableReasonV1::CaptureIncompleteOrDispatchStillLive)
        };
        Ok(RuntimeDispatchDetailV1 {
            summary: record.summary,
            launch: record.launch,
            bindings: record.bindings.clone(),
            host_timing: record.host_timing,
            host_timing_origin,
            dependency_events: unavailable(
                RuntimeCausalityUnavailableReasonV1::NoCanonicalDependencyEvents,
            ),
            device_copy_events: unavailable(
                RuntimeCausalityUnavailableReasonV1::NoDeviceCopyEngineEvents,
            ),
        })
    }

    pub fn page(
        &self,
        kind: RuntimeCausalityPageKindV1,
        cursor: Option<RuntimeCausalityCursorV1>,
        limit: u16,
    ) -> Result<RuntimeCausalityPageV1, RuntimeCausalityErrorV1> {
        if limit == 0 || limit > MAX_AGENT_RUNTIME_CAUSALITY_PAGE_ITEMS_V1 {
            return Err(RuntimeCausalityErrorV1::PageLimitOutOfRange);
        }
        let availability = page_availability(kind);
        let query_identity = query_identity(self.binding.binding_identity, kind)?;
        let start = match cursor {
            Some(cursor) => self.validate_cursor(kind, query_identity, cursor)?,
            None => 0,
        };
        let total = self.page_item_count(kind);
        if start > total {
            return Err(RuntimeCausalityErrorV1::InvalidCursor);
        }
        let end = start.saturating_add(usize::from(limit)).min(total);
        let mut items = Vec::new();
        items
            .try_reserve_exact(end.saturating_sub(start))
            .map_err(|_| RuntimeCausalityErrorV1::AllocationFailure)?;
        for ordinal in start..end {
            items.push(
                self.page_item(kind, ordinal)
                    .ok_or(RuntimeCausalityErrorV1::InvalidCursor)?,
            );
        }
        let next_cursor = if end < total {
            let preceding = CaptureIdentityV1::new(
                items
                    .last()
                    .ok_or(RuntimeCausalityErrorV1::InvalidCursor)?
                    .identity(),
            )
            .map_err(|_| RuntimeCausalityErrorV1::IdentityFailure)?;
            let next_ordinal =
                u64::try_from(end).map_err(|_| RuntimeCausalityErrorV1::SizeOverflow)?;
            Some(make_cursor(
                self.binding.binding_identity,
                query_identity,
                next_ordinal,
                preceding,
            )?)
        } else {
            None
        };
        Ok(RuntimeCausalityPageV1 {
            binding_identity: self.binding.binding_identity,
            kind,
            availability,
            coverage: self.binding.coverage,
            total_items: u64::try_from(total).map_err(|_| RuntimeCausalityErrorV1::SizeOverflow)?,
            returned: u16::try_from(items.len())
                .map_err(|_| RuntimeCausalityErrorV1::SizeOverflow)?,
            items,
            next_cursor,
        })
    }

    pub fn profiler_juxtaposition(&self) -> RuntimeProfilerJuxtapositionV1 {
        match &self.profiler {
            None => RuntimeProfilerJuxtapositionV1 {
                binding_identity: self.binding.binding_identity,
                state: RuntimeProfilerJuxtapositionStateV1::NotSupplied,
                runtime_input: self.binding.runtime_input,
                profiler_input: None,
                profiler_content_identity: None,
                profiler_run_identity: None,
                runtime_producer_order_domain: "kfd_runtime_profile_v1_event_sequence",
                profiler_local_tick_coordinates: unavailable(
                    RuntimeCausalityUnavailableReasonV1::ProfilerBundleNotSupplied,
                ),
                profiler_local_tick_description: None,
                dispatch_relation: unavailable(
                    RuntimeCausalityUnavailableReasonV1::ProfilerBundleNotSupplied,
                ),
                cross_clock_relation: unavailable(
                    RuntimeCausalityUnavailableReasonV1::ProfilerBundleNotSupplied,
                ),
                cross_clock_uncertainty: unavailable(
                    RuntimeCausalityUnavailableReasonV1::ProfilerBundleNotSupplied,
                ),
                runtime_coverage: self.binding.coverage,
                profiler_loss: None,
            },
            Some(bundle) => {
                let clock_reason = if bundle.dispatch_capture.is_some() {
                    RuntimeCausalityUnavailableReasonV1::NoAdmittedClockCorrelation
                } else {
                    RuntimeCausalityUnavailableReasonV1::ProfilerBundleHasNoDispatchEnvelopes
                };
                RuntimeProfilerJuxtapositionV1 {
                    binding_identity: self.binding.binding_identity,
                    state: RuntimeProfilerJuxtapositionStateV1::BoundWithoutDispatchOrClockJoin,
                    runtime_input: self.binding.runtime_input,
                    profiler_input: self.binding.profiler_input,
                    profiler_content_identity: self.binding.profiler_content_identity,
                    profiler_run_identity: Some(bundle.run_identity),
                    runtime_producer_order_domain: "kfd_runtime_profile_v1_event_sequence",
                    profiler_local_tick_coordinates: if bundle.dispatch_capture.is_some() {
                        available_observed()
                    } else {
                        unavailable(clock_reason)
                    },
                    profiler_local_tick_description: bundle.dispatch_capture.as_ref().map(
                        |_| "bundle_v4_capture_local_opaque_collector_ticks_without_frequency",
                    ),
                    dispatch_relation: unavailable(if bundle.dispatch_capture.is_some() {
                        RuntimeCausalityUnavailableReasonV1::NoAdmittedCommonDispatchIdentity
                    } else {
                        RuntimeCausalityUnavailableReasonV1::ProfilerBundleHasNoDispatchEnvelopes
                    }),
                    cross_clock_relation: unavailable(
                        RuntimeCausalityUnavailableReasonV1::NoAdmittedClockCorrelation,
                    ),
                    cross_clock_uncertainty: unavailable(
                        RuntimeCausalityUnavailableReasonV1::NoAdmittedClockCorrelation,
                    ),
                    runtime_coverage: self.binding.coverage,
                    profiler_loss: Some(bundle.coverage.loss),
                }
            }
        }
    }

    fn page_item_count(&self, kind: RuntimeCausalityPageKindV1) -> usize {
        match kind {
            RuntimeCausalityPageKindV1::RuntimeEvents => self.events.len(),
            RuntimeCausalityPageKindV1::LifecycleEdges => self.edges.len(),
            RuntimeCausalityPageKindV1::Dispatches => self.dispatches.len(),
            RuntimeCausalityPageKindV1::HostStaging => self.host_staging.len(),
            RuntimeCausalityPageKindV1::Dependencies | RuntimeCausalityPageKindV1::DeviceCopies => {
                0
            }
        }
    }

    fn page_item(
        &self,
        kind: RuntimeCausalityPageKindV1,
        ordinal: usize,
    ) -> Option<RuntimeCausalityPageItemV1> {
        match kind {
            RuntimeCausalityPageKindV1::RuntimeEvents => self
                .events
                .get(ordinal)
                .copied()
                .map(RuntimeCausalityPageItemV1::RuntimeEvent),
            RuntimeCausalityPageKindV1::LifecycleEdges => self
                .edges
                .get(ordinal)
                .copied()
                .map(RuntimeCausalityPageItemV1::LifecycleEdge),
            RuntimeCausalityPageKindV1::Dispatches => self
                .dispatches
                .get(ordinal)
                .map(|record| RuntimeCausalityPageItemV1::Dispatch(record.summary)),
            RuntimeCausalityPageKindV1::HostStaging => self
                .host_staging
                .get(ordinal)
                .copied()
                .map(RuntimeCausalityPageItemV1::HostStaging),
            RuntimeCausalityPageKindV1::Dependencies | RuntimeCausalityPageKindV1::DeviceCopies => {
                None
            }
        }
    }

    fn validate_cursor(
        &self,
        kind: RuntimeCausalityPageKindV1,
        query_identity: CaptureIdentityV1,
        cursor: RuntimeCausalityCursorV1,
    ) -> Result<usize, RuntimeCausalityErrorV1> {
        if cursor.binding_identity != self.binding.binding_identity
            || cursor.query_identity != query_identity
            || cursor.cursor_identity
                != cursor_identity(
                    cursor.binding_identity,
                    cursor.query_identity,
                    cursor.next_ordinal,
                    cursor.preceding_item_identity,
                )?
        {
            return Err(RuntimeCausalityErrorV1::InvalidCursor);
        }
        let start = usize::try_from(cursor.next_ordinal)
            .map_err(|_| RuntimeCausalityErrorV1::InvalidCursor)?;
        if start == 0 {
            return Err(RuntimeCausalityErrorV1::InvalidCursor);
        }
        let preceding = self
            .page_item(kind, start - 1)
            .ok_or(RuntimeCausalityErrorV1::InvalidCursor)?;
        if preceding.identity() != cursor.preceding_item_identity.as_bytes() {
            return Err(RuntimeCausalityErrorV1::InvalidCursor);
        }
        Ok(start)
    }
}

fn index_runtime(runtime: &KfdRuntimeProfileV1) -> Result<RuntimeIndexV1, RuntimeCausalityErrorV1> {
    let mut dispatch_capacity = 0_usize;
    let mut host_staging_capacity = 0_usize;
    let mut edge_capacity = 0_usize;
    for event in &runtime.events {
        match &event.event {
            KfdRuntimeProfileEventKindV1::DispatchPublished { bindings, .. } => {
                dispatch_capacity = dispatch_capacity
                    .checked_add(1)
                    .ok_or(RuntimeCausalityErrorV1::SizeOverflow)?;
                edge_capacity = edge_capacity
                    .checked_add(bindings.len())
                    .and_then(|value| value.checked_add(4))
                    .ok_or(RuntimeCausalityErrorV1::SizeOverflow)?;
            }
            KfdRuntimeProfileEventKindV1::HostWrite { .. }
            | KfdRuntimeProfileEventKindV1::HostRead { .. } => {
                host_staging_capacity = host_staging_capacity
                    .checked_add(1)
                    .ok_or(RuntimeCausalityErrorV1::SizeOverflow)?;
            }
            _ => {}
        }
    }
    if u64::try_from(edge_capacity).unwrap_or(u64::MAX) > MAX_RUNTIME_CAUSALITY_LIFECYCLE_EDGES_V1 {
        return Err(RuntimeCausalityErrorV1::EdgeLimitExceeded);
    }
    let mut events = Vec::new();
    let mut edges = Vec::new();
    let mut dispatches = Vec::new();
    let mut host_staging = Vec::new();
    events
        .try_reserve_exact(runtime.events.len())
        .map_err(|_| RuntimeCausalityErrorV1::AllocationFailure)?;
    edges
        .try_reserve_exact(edge_capacity)
        .map_err(|_| RuntimeCausalityErrorV1::AllocationFailure)?;
    dispatches
        .try_reserve_exact(dispatch_capacity)
        .map_err(|_| RuntimeCausalityErrorV1::AllocationFailure)?;
    host_staging
        .try_reserve_exact(host_staging_capacity)
        .map_err(|_| RuntimeCausalityErrorV1::AllocationFailure)?;
    let mut queues = BTreeMap::new();
    let mut streams = BTreeMap::new();
    let mut allocations = BTreeMap::new();
    let mut dispatch_index = BTreeMap::new();
    for event in &runtime.events {
        let (kind, primary) = event_kind_and_primary(&event.event);
        events.push(RuntimeEventSummaryV1 {
            event_identity: event.identity,
            sequence: event.sequence,
            producer_order_domain: "kfd_runtime_profile_v1_event_sequence",
            origin: TruthOriginV1::Observed,
            kind,
            primary_resource: primary,
        });
        let current = OwnedEventRefV1 {
            identity: event.identity,
            sequence: event.sequence,
        };
        match &event.event {
            KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue } => {
                queues.insert(*queue, current);
            }
            KfdRuntimeProfileEventKindV1::StreamCreated { stream } => {
                streams.insert(*stream, current);
            }
            KfdRuntimeProfileEventKindV1::AllocationCreated { allocation, .. } => {
                allocations.insert(*allocation, current);
            }
            KfdRuntimeProfileEventKindV1::HostWrite {
                allocation,
                byte_offset,
                content,
            } => host_staging.push(RuntimeHostStagingV1 {
                event_identity: event.identity,
                sequence: event.sequence,
                producer_order_domain: "kfd_runtime_profile_v1_event_sequence",
                origin: TruthOriginV1::Observed,
                kind: RuntimeHostStagingKindV1::HostWrite,
                allocation: *allocation,
                byte_offset: *byte_offset,
                content: *content,
                device_copy_relation: unavailable(
                    RuntimeCausalityUnavailableReasonV1::NoDeviceCopyEngineEvents,
                ),
            }),
            KfdRuntimeProfileEventKindV1::HostRead {
                allocation,
                byte_offset,
                content,
            } => host_staging.push(RuntimeHostStagingV1 {
                event_identity: event.identity,
                sequence: event.sequence,
                producer_order_domain: "kfd_runtime_profile_v1_event_sequence",
                origin: TruthOriginV1::Observed,
                kind: RuntimeHostStagingKindV1::HostRead,
                allocation: *allocation,
                byte_offset: *byte_offset,
                content: *content,
                device_copy_relation: unavailable(
                    RuntimeCausalityUnavailableReasonV1::NoDeviceCopyEngineEvents,
                ),
            }),
            KfdRuntimeProfileEventKindV1::DispatchPublished {
                dispatch,
                queue,
                stream,
                kernel,
                launch,
                bindings,
                ..
            } => {
                let queue_event = *queues
                    .get(queue)
                    .ok_or(RuntimeCausalityErrorV1::GraphMissingEndpoint)?;
                let stream_event = *streams
                    .get(stream)
                    .ok_or(RuntimeCausalityErrorV1::GraphMissingEndpoint)?;
                push_edge(
                    &mut edges,
                    RuntimeLifecycleEdgeKindV1::QueueExistsBeforeDispatchPublication,
                    queue_event,
                    current,
                )?;
                push_edge(
                    &mut edges,
                    RuntimeLifecycleEdgeKindV1::StreamExistsBeforeDispatchPublication,
                    stream_event,
                    current,
                )?;
                let mut distinct_allocations = BTreeSet::new();
                for binding in bindings {
                    if distinct_allocations.insert(binding.allocation) {
                        let allocation_event = *allocations
                            .get(&binding.allocation)
                            .ok_or(RuntimeCausalityErrorV1::GraphMissingEndpoint)?;
                        push_edge(
                            &mut edges,
                            RuntimeLifecycleEdgeKindV1::AllocationExistsBeforeDispatchPublication,
                            allocation_event,
                            current,
                        )?;
                    }
                }
                let index = dispatches.len();
                dispatches.push(IndexedDispatchV1 {
                    summary: RuntimeDispatchSummaryV1 {
                        dispatch: *dispatch,
                        publication_event: event.identity,
                        publication_sequence: event.sequence,
                        queue: *queue,
                        stream: *stream,
                        kernel: *kernel,
                        completion: missing_dispatch_event(),
                        release: missing_dispatch_event(),
                    },
                    launch: *launch,
                    bindings: bindings.clone(),
                    host_timing: None,
                });
                dispatch_index.insert(*dispatch, index);
            }
            KfdRuntimeProfileEventKindV1::DispatchCompleted {
                dispatch,
                host_timing,
            } => {
                let record = dispatches
                    .get_mut(
                        *dispatch_index
                            .get(dispatch)
                            .ok_or(RuntimeCausalityErrorV1::GraphMissingEndpoint)?,
                    )
                    .ok_or(RuntimeCausalityErrorV1::GraphMissingEndpoint)?;
                let publication = OwnedEventRefV1 {
                    identity: record.summary.publication_event,
                    sequence: record.summary.publication_sequence,
                };
                push_edge(
                    &mut edges,
                    RuntimeLifecycleEdgeKindV1::DispatchPublicationBeforeCompletion,
                    publication,
                    current,
                )?;
                record.summary.completion = observed_dispatch_event(current);
                record.host_timing = Some(*host_timing);
            }
            KfdRuntimeProfileEventKindV1::SubmissionReleased { dispatch } => {
                let record = dispatches
                    .get_mut(
                        *dispatch_index
                            .get(dispatch)
                            .ok_or(RuntimeCausalityErrorV1::GraphMissingEndpoint)?,
                    )
                    .ok_or(RuntimeCausalityErrorV1::GraphMissingEndpoint)?;
                let completion = match record.summary.completion {
                    RuntimeDispatchEventRefV1::Observed {
                        event_identity,
                        sequence,
                        ..
                    } => OwnedEventRefV1 {
                        identity: event_identity,
                        sequence,
                    },
                    RuntimeDispatchEventRefV1::Unavailable { .. } => {
                        return Err(RuntimeCausalityErrorV1::GraphMissingEndpoint);
                    }
                };
                push_edge(
                    &mut edges,
                    RuntimeLifecycleEdgeKindV1::DispatchCompletionBeforeSubmissionRelease,
                    completion,
                    current,
                )?;
                record.summary.release = observed_dispatch_event(current);
            }
            KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue } => {
                queues.remove(queue);
            }
            KfdRuntimeProfileEventKindV1::StreamDestroyed { stream } => {
                streams.remove(stream);
            }
            KfdRuntimeProfileEventKindV1::AllocationReleased { allocation } => {
                allocations.remove(allocation);
            }
            KfdRuntimeProfileEventKindV1::ModuleLoaded { .. }
            | KfdRuntimeProfileEventKindV1::KernelResolved { .. }
            | KfdRuntimeProfileEventKindV1::ModuleUnloaded { .. } => {}
        }
    }
    Ok(RuntimeIndexV1 {
        events,
        edges,
        dispatches,
        host_staging,
    })
}

fn event_kind_and_primary(
    event: &KfdRuntimeProfileEventKindV1,
) -> (RuntimeEventKindV1, ProfileIdentityV1) {
    match event {
        KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue } => {
            (RuntimeEventKindV1::NativeQueueCreated, *queue)
        }
        KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue } => {
            (RuntimeEventKindV1::NativeQueueDestroyed, *queue)
        }
        KfdRuntimeProfileEventKindV1::StreamCreated { stream } => {
            (RuntimeEventKindV1::StreamCreated, *stream)
        }
        KfdRuntimeProfileEventKindV1::StreamDestroyed { stream } => {
            (RuntimeEventKindV1::StreamDestroyed, *stream)
        }
        KfdRuntimeProfileEventKindV1::AllocationCreated { allocation, .. } => {
            (RuntimeEventKindV1::AllocationCreated, *allocation)
        }
        KfdRuntimeProfileEventKindV1::HostWrite { allocation, .. } => {
            (RuntimeEventKindV1::HostWrite, *allocation)
        }
        KfdRuntimeProfileEventKindV1::HostRead { allocation, .. } => {
            (RuntimeEventKindV1::HostRead, *allocation)
        }
        KfdRuntimeProfileEventKindV1::AllocationReleased { allocation } => {
            (RuntimeEventKindV1::AllocationReleased, *allocation)
        }
        KfdRuntimeProfileEventKindV1::ModuleLoaded { module, .. } => {
            (RuntimeEventKindV1::ModuleLoaded, *module)
        }
        KfdRuntimeProfileEventKindV1::KernelResolved { kernel, .. } => {
            (RuntimeEventKindV1::KernelResolved, *kernel)
        }
        KfdRuntimeProfileEventKindV1::ModuleUnloaded { module } => {
            (RuntimeEventKindV1::ModuleUnloaded, *module)
        }
        KfdRuntimeProfileEventKindV1::DispatchPublished { dispatch, .. } => {
            (RuntimeEventKindV1::DispatchPublished, *dispatch)
        }
        KfdRuntimeProfileEventKindV1::DispatchCompleted { dispatch, .. } => {
            (RuntimeEventKindV1::DispatchCompleted, *dispatch)
        }
        KfdRuntimeProfileEventKindV1::SubmissionReleased { dispatch } => {
            (RuntimeEventKindV1::SubmissionReleased, *dispatch)
        }
    }
}

fn push_edge(
    edges: &mut Vec<RuntimeLifecycleEdgeV1>,
    kind: RuntimeLifecycleEdgeKindV1,
    predecessor: OwnedEventRefV1,
    successor: OwnedEventRefV1,
) -> Result<(), RuntimeCausalityErrorV1> {
    if predecessor.sequence >= successor.sequence {
        return Err(RuntimeCausalityErrorV1::GraphCycle);
    }
    let inference_rule = identity(RULE_IDENTITY_DOMAIN_V1, &[LIFECYCLE_RULE_V1])?;
    let edge_identity = expected_edge_identity(kind, predecessor, successor, inference_rule)?;
    edges
        .try_reserve_exact(1)
        .map_err(|_| RuntimeCausalityErrorV1::AllocationFailure)?;
    edges.push(RuntimeLifecycleEdgeV1 {
        edge_identity,
        kind,
        origin: TruthOriginV1::Inferred,
        inference_rule,
        predecessor_event: predecessor.identity,
        predecessor_sequence: predecessor.sequence,
        successor_event: successor.identity,
        successor_sequence: successor.sequence,
        producer_order_domain: "kfd_runtime_profile_v1_event_sequence",
    });
    Ok(())
}

fn expected_edge_identity(
    kind: RuntimeLifecycleEdgeKindV1,
    predecessor: OwnedEventRefV1,
    successor: OwnedEventRefV1,
    inference_rule: CaptureIdentityV1,
) -> Result<CaptureIdentityV1, RuntimeCausalityErrorV1> {
    let tag = [edge_tag(kind)];
    identity(
        EDGE_IDENTITY_DOMAIN_V1,
        &[
            &tag,
            &predecessor.identity.as_bytes(),
            &predecessor.sequence.to_le_bytes(),
            &successor.identity.as_bytes(),
            &successor.sequence.to_le_bytes(),
            &inference_rule.as_bytes(),
        ],
    )
}

#[derive(Clone, Copy)]
struct OwnedEventRefV1 {
    identity: ProfileIdentityV1,
    sequence: u64,
}

fn edge_tag(kind: RuntimeLifecycleEdgeKindV1) -> u8 {
    match kind {
        RuntimeLifecycleEdgeKindV1::QueueExistsBeforeDispatchPublication => 1,
        RuntimeLifecycleEdgeKindV1::StreamExistsBeforeDispatchPublication => 2,
        RuntimeLifecycleEdgeKindV1::AllocationExistsBeforeDispatchPublication => 3,
        RuntimeLifecycleEdgeKindV1::DispatchPublicationBeforeCompletion => 4,
        RuntimeLifecycleEdgeKindV1::DispatchCompletionBeforeSubmissionRelease => 5,
    }
}

fn validate_graph(
    events: &[RuntimeEventSummaryV1],
    edges: &[RuntimeLifecycleEdgeV1],
) -> Result<(), RuntimeCausalityErrorV1> {
    let mut event_sequences = BTreeMap::new();
    for event in events {
        if event_sequences
            .insert(event.event_identity, event.sequence)
            .is_some()
        {
            return Err(RuntimeCausalityErrorV1::GraphDuplicateIdentity);
        }
    }
    let expected_rule = identity(RULE_IDENTITY_DOMAIN_V1, &[LIFECYCLE_RULE_V1])?;
    let mut edge_ids = BTreeSet::new();
    for edge in edges {
        let predecessor_sequence = *event_sequences
            .get(&edge.predecessor_event)
            .ok_or(RuntimeCausalityErrorV1::GraphMissingEndpoint)?;
        let successor_sequence = *event_sequences
            .get(&edge.successor_event)
            .ok_or(RuntimeCausalityErrorV1::GraphMissingEndpoint)?;
        if predecessor_sequence >= successor_sequence
            || edge.predecessor_sequence >= edge.successor_sequence
        {
            return Err(RuntimeCausalityErrorV1::GraphCycle);
        }
        let predecessor = OwnedEventRefV1 {
            identity: edge.predecessor_event,
            sequence: edge.predecessor_sequence,
        };
        let successor = OwnedEventRefV1 {
            identity: edge.successor_event,
            sequence: edge.successor_sequence,
        };
        if edge.origin != TruthOriginV1::Inferred
            || edge.inference_rule != expected_rule
            || edge.predecessor_sequence != predecessor_sequence
            || edge.successor_sequence != successor_sequence
            || edge.edge_identity
                != expected_edge_identity(edge.kind, predecessor, successor, expected_rule)?
            || !edge_ids.insert(edge.edge_identity)
        {
            return Err(RuntimeCausalityErrorV1::GraphStaleEvidence);
        }
    }
    Ok(())
}

fn observed_dispatch_event(event: OwnedEventRefV1) -> RuntimeDispatchEventRefV1 {
    RuntimeDispatchEventRefV1::Observed {
        origin: TruthOriginV1::Observed,
        event_identity: event.identity,
        sequence: event.sequence,
    }
}

fn missing_dispatch_event() -> RuntimeDispatchEventRefV1 {
    RuntimeDispatchEventRefV1::Unavailable {
        origin: TruthOriginV1::Unavailable,
        reason: RuntimeCausalityUnavailableReasonV1::CaptureIncompleteOrDispatchStillLive,
    }
}

fn available_observed() -> RuntimeCausalityAvailabilityV1 {
    RuntimeCausalityAvailabilityV1::Available {
        origin: TruthOriginV1::Observed,
    }
}

fn unavailable(reason: RuntimeCausalityUnavailableReasonV1) -> RuntimeCausalityAvailabilityV1 {
    RuntimeCausalityAvailabilityV1::Unavailable {
        origin: TruthOriginV1::Unavailable,
        reason,
    }
}

fn page_availability(kind: RuntimeCausalityPageKindV1) -> RuntimeCausalityAvailabilityV1 {
    match kind {
        RuntimeCausalityPageKindV1::Dependencies => {
            unavailable(RuntimeCausalityUnavailableReasonV1::NoCanonicalDependencyEvents)
        }
        RuntimeCausalityPageKindV1::DeviceCopies => {
            unavailable(RuntimeCausalityUnavailableReasonV1::NoDeviceCopyEngineEvents)
        }
        RuntimeCausalityPageKindV1::RuntimeEvents
        | RuntimeCausalityPageKindV1::LifecycleEdges
        | RuntimeCausalityPageKindV1::Dispatches
        | RuntimeCausalityPageKindV1::HostStaging => available_observed(),
    }
}

fn input_identity(
    bytes: &[u8],
) -> Result<RuntimeCausalityInputIdentityV1, RuntimeCausalityErrorV1> {
    Ok(RuntimeCausalityInputIdentityV1 {
        digest: identity(INPUT_IDENTITY_DOMAIN_V1, &[bytes])?,
        byte_len: u64::try_from(bytes.len()).map_err(|_| RuntimeCausalityErrorV1::SizeOverflow)?,
    })
}

fn binding_identity(
    runtime: ProfileContentIdentityV1,
    profiler: Option<&ContentIdentityRecordV1>,
) -> Result<CaptureIdentityV1, RuntimeCausalityErrorV1> {
    match profiler {
        Some(profiler) => identity(
            BINDING_IDENTITY_DOMAIN_V1,
            &[
                &runtime.digest.as_bytes(),
                &runtime.byte_len.to_le_bytes(),
                &[match profiler.scheme {
                    fe2o3_semantic_import::ContentSchemeV1::RawCanonicalSha256 => 1,
                    fe2o3_semantic_import::ContentSchemeV1::DomainSeparatedSha256 => 2,
                }],
                &profiler.format_version.to_le_bytes(),
                &profiler.digest.as_bytes(),
                &profiler.canonical_len.to_le_bytes(),
            ],
        ),
        None => identity(
            BINDING_IDENTITY_DOMAIN_V1,
            &[
                &runtime.digest.as_bytes(),
                &runtime.byte_len.to_le_bytes(),
                &[0],
            ],
        ),
    }
}

fn query_identity(
    binding: CaptureIdentityV1,
    kind: RuntimeCausalityPageKindV1,
) -> Result<CaptureIdentityV1, RuntimeCausalityErrorV1> {
    identity(
        QUERY_IDENTITY_DOMAIN_V1,
        &[&binding.as_bytes(), &[page_tag(kind)]],
    )
}

fn page_tag(kind: RuntimeCausalityPageKindV1) -> u8 {
    match kind {
        RuntimeCausalityPageKindV1::RuntimeEvents => 1,
        RuntimeCausalityPageKindV1::LifecycleEdges => 2,
        RuntimeCausalityPageKindV1::Dispatches => 3,
        RuntimeCausalityPageKindV1::HostStaging => 4,
        RuntimeCausalityPageKindV1::Dependencies => 5,
        RuntimeCausalityPageKindV1::DeviceCopies => 6,
    }
}

fn make_cursor(
    binding_identity: CaptureIdentityV1,
    query_identity: CaptureIdentityV1,
    next_ordinal: u64,
    preceding_item_identity: CaptureIdentityV1,
) -> Result<RuntimeCausalityCursorV1, RuntimeCausalityErrorV1> {
    Ok(RuntimeCausalityCursorV1 {
        binding_identity,
        query_identity,
        next_ordinal,
        preceding_item_identity,
        cursor_identity: cursor_identity(
            binding_identity,
            query_identity,
            next_ordinal,
            preceding_item_identity,
        )?,
    })
}

fn cursor_identity(
    binding: CaptureIdentityV1,
    query: CaptureIdentityV1,
    next: u64,
    preceding: CaptureIdentityV1,
) -> Result<CaptureIdentityV1, RuntimeCausalityErrorV1> {
    identity(
        CURSOR_IDENTITY_DOMAIN_V1,
        &[
            &binding.as_bytes(),
            &query.as_bytes(),
            &next.to_le_bytes(),
            &preceding.as_bytes(),
        ],
    )
}

fn identity(domain: &[u8], parts: &[&[u8]]) -> Result<CaptureIdentityV1, RuntimeCausalityErrorV1> {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for part in parts {
        hasher.update(
            u64::try_from(part.len())
                .map_err(|_| RuntimeCausalityErrorV1::SizeOverflow)?
                .to_le_bytes(),
        );
        hasher.update(part);
    }
    CaptureIdentityV1::new(hasher.finalize().into())
        .map_err(|_| RuntimeCausalityErrorV1::IdentityFailure)
}

fn capabilities() -> Result<RuntimeCausalityCapabilitiesV1, RuntimeCausalityErrorV1> {
    Ok(RuntimeCausalityCapabilitiesV1 {
        operations: vec![
            RuntimeCausalityOperationV1::DiscoverCapabilities,
            RuntimeCausalityOperationV1::Open,
            RuntimeCausalityOperationV1::Binding,
            RuntimeCausalityOperationV1::Page,
            RuntimeCausalityOperationV1::InspectDispatch,
            RuntimeCausalityOperationV1::ProfilerJuxtaposition,
            RuntimeCausalityOperationV1::Close,
        ],
        page_kinds: vec![
            RuntimeCausalityPageKindV1::RuntimeEvents,
            RuntimeCausalityPageKindV1::LifecycleEdges,
            RuntimeCausalityPageKindV1::Dispatches,
            RuntimeCausalityPageKindV1::HostStaging,
            RuntimeCausalityPageKindV1::Dependencies,
            RuntimeCausalityPageKindV1::DeviceCopies,
        ],
        authority: RuntimeCausalityAuthorityV1::ReadOnlyNoRuntimeProfilerCollectionAttachDispatchOrSchedulingAuthority,
        exact_input_encoding: "canonical lowercase hex of KFD Runtime Profile V1 and optional Bundle V4 bytes",
        producer_order_domain: "kfd_runtime_profile_v1_event_sequence",
        lifecycle_edge_rule: identity(RULE_IDENTITY_DOMAIN_V1, &[LIFECYCLE_RULE_V1])?,
        dependency_events: unavailable(
            RuntimeCausalityUnavailableReasonV1::NoCanonicalDependencyEvents,
        ),
        device_copy_engine_events: unavailable(
            RuntimeCausalityUnavailableReasonV1::NoDeviceCopyEngineEvents,
        ),
        profiler_dispatch_join: unavailable(
            RuntimeCausalityUnavailableReasonV1::NoAdmittedCommonDispatchIdentity,
        ),
        cross_clock_relation: unavailable(
            RuntimeCausalityUnavailableReasonV1::NoAdmittedClockCorrelation,
        ),
        max_runtime_profile_bytes: MAX_KFD_RUNTIME_PROFILE_BYTES_V1,
        max_profiler_bundle_bytes: MAX_PROFILER_BUNDLE_BYTES_V4,
        max_lifecycle_edges: MAX_RUNTIME_CAUSALITY_LIFECYCLE_EDGES_V1,
        max_page_items: MAX_AGENT_RUNTIME_CAUSALITY_PAGE_ITEMS_V1,
        max_request_attempts: MAX_AGENT_RUNTIME_CAUSALITY_REQUEST_ATTEMPTS_V1,
    })
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentRuntimeCausalityRequestV1 {
    DiscoverCapabilities {
        #[serde(deserialize_with = "deserialize_schema")]
        schema: String,
        request_id: u64,
        revision: u64,
    },
    Open {
        #[serde(deserialize_with = "deserialize_schema")]
        schema: String,
        request_id: u64,
        revision: u64,
        #[serde(deserialize_with = "deserialize_runtime_hex")]
        runtime_profile_hex: String,
        #[serde(default, deserialize_with = "deserialize_optional_profiler_hex")]
        profiler_bundle_hex: Option<String>,
    },
    Binding {
        #[serde(deserialize_with = "deserialize_schema")]
        schema: String,
        request_id: u64,
        revision: u64,
    },
    Page {
        #[serde(deserialize_with = "deserialize_schema")]
        schema: String,
        request_id: u64,
        revision: u64,
        kind: AgentRuntimeCausalityPageKindV1,
        limit: u16,
        #[serde(default)]
        cursor: Option<RuntimeCausalityCursorV1>,
    },
    InspectDispatch {
        #[serde(deserialize_with = "deserialize_schema")]
        schema: String,
        request_id: u64,
        revision: u64,
        dispatch: ProfileIdentityV1,
    },
    ProfilerJuxtaposition {
        #[serde(deserialize_with = "deserialize_schema")]
        schema: String,
        request_id: u64,
        revision: u64,
    },
    Close {
        #[serde(deserialize_with = "deserialize_schema")]
        schema: String,
        request_id: u64,
        revision: u64,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRuntimeCausalityPageKindV1 {
    RuntimeEvents,
    LifecycleEdges,
    Dispatches,
    HostStaging,
    Dependencies,
    DeviceCopies,
}

impl From<AgentRuntimeCausalityPageKindV1> for RuntimeCausalityPageKindV1 {
    fn from(value: AgentRuntimeCausalityPageKindV1) -> Self {
        match value {
            AgentRuntimeCausalityPageKindV1::RuntimeEvents => Self::RuntimeEvents,
            AgentRuntimeCausalityPageKindV1::LifecycleEdges => Self::LifecycleEdges,
            AgentRuntimeCausalityPageKindV1::Dispatches => Self::Dispatches,
            AgentRuntimeCausalityPageKindV1::HostStaging => Self::HostStaging,
            AgentRuntimeCausalityPageKindV1::Dependencies => Self::Dependencies,
            AgentRuntimeCausalityPageKindV1::DeviceCopies => Self::DeviceCopies,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentRuntimeCausalityResultV1 {
    Capabilities(RuntimeCausalityCapabilitiesV1),
    Opened(RuntimeCausalityBindingV1),
    Binding(RuntimeCausalityBindingV1),
    Page(RuntimeCausalityPageV1),
    Dispatch(RuntimeDispatchDetailV1),
    ProfilerJuxtaposition(RuntimeProfilerJuxtapositionV1),
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRuntimeCausalityErrorCodeV1 {
    InvalidRequest,
    InvalidSchema,
    InvalidRequestId,
    DuplicateRequestId,
    RevisionMismatch,
    RevisionExhausted,
    RequestTooLarge,
    RequestAttemptLimit,
    SessionAlreadyOpen,
    SessionNotOpen,
    EvidenceRejected,
    QueryRejected,
    ResponseTooLarge,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentRuntimeCausalityResponseV1 {
    Ok {
        schema: &'static str,
        request_id: u64,
        revision: u64,
        terminal: bool,
        result: Box<AgentRuntimeCausalityResultV1>,
    },
    Error {
        schema: &'static str,
        request_id: Option<u64>,
        revision: u64,
        terminal: bool,
        code: AgentRuntimeCausalityErrorCodeV1,
    },
}

struct AgentRuntimeCausalityServiceV1 {
    revision: u64,
    attempts: u64,
    request_ids: Vec<u64>,
    session: Option<RuntimeCausalitySessionV1>,
    terminal: bool,
}

impl AgentRuntimeCausalityServiceV1 {
    fn new() -> Self {
        Self {
            revision: 0,
            attempts: 0,
            request_ids: Vec::new(),
            session: None,
            terminal: false,
        }
    }

    fn begin_attempt(&mut self) -> Result<(), AgentRuntimeCausalityResponseV1> {
        if self.attempts >= MAX_AGENT_RUNTIME_CAUSALITY_REQUEST_ATTEMPTS_V1 {
            return Err(self.error(
                None,
                AgentRuntimeCausalityErrorCodeV1::RequestAttemptLimit,
                true,
            ));
        }
        self.attempts = self.attempts.checked_add(1).ok_or_else(|| {
            self.error(
                None,
                AgentRuntimeCausalityErrorCodeV1::RequestAttemptLimit,
                true,
            )
        })?;
        Ok(())
    }

    fn handle(
        &mut self,
        request: AgentRuntimeCausalityRequestV1,
    ) -> AgentRuntimeCausalityResponseV1 {
        let (schema, request_id, revision) = request_header(&request);
        if request_id == 0 {
            return self.error(
                Some(request_id),
                AgentRuntimeCausalityErrorCodeV1::InvalidRequestId,
                false,
            );
        }
        if self.request_ids.contains(&request_id) {
            return self.error(
                Some(request_id),
                AgentRuntimeCausalityErrorCodeV1::DuplicateRequestId,
                false,
            );
        }
        if self.request_ids.try_reserve_exact(1).is_err() {
            return self.error(
                Some(request_id),
                AgentRuntimeCausalityErrorCodeV1::ResponseTooLarge,
                true,
            );
        }
        self.request_ids.push(request_id);
        if schema != AGENT_RUNTIME_CAUSALITY_REQUEST_SCHEMA_V1 {
            return self.error(
                Some(request_id),
                AgentRuntimeCausalityErrorCodeV1::InvalidSchema,
                false,
            );
        }
        if revision != self.revision {
            return self.error(
                Some(request_id),
                AgentRuntimeCausalityErrorCodeV1::RevisionMismatch,
                false,
            );
        }
        match request {
            AgentRuntimeCausalityRequestV1::DiscoverCapabilities { .. } => match capabilities() {
                Ok(value) => self.success(
                    request_id,
                    false,
                    AgentRuntimeCausalityResultV1::Capabilities(value),
                ),
                Err(_) => self.error(
                    Some(request_id),
                    AgentRuntimeCausalityErrorCodeV1::ResponseTooLarge,
                    true,
                ),
            },
            AgentRuntimeCausalityRequestV1::Open {
                runtime_profile_hex,
                profiler_bundle_hex,
                ..
            } => {
                if self.session.is_some() {
                    return self.error(
                        Some(request_id),
                        AgentRuntimeCausalityErrorCodeV1::SessionAlreadyOpen,
                        false,
                    );
                }
                let runtime = match decode_lower_hex(
                    &runtime_profile_hex,
                    MAX_KFD_RUNTIME_PROFILE_BYTES_V1,
                ) {
                    Ok(value) => value,
                    Err(()) => {
                        return self.error(
                            Some(request_id),
                            AgentRuntimeCausalityErrorCodeV1::EvidenceRejected,
                            false,
                        );
                    }
                };
                let profiler = match profiler_bundle_hex.as_deref() {
                    Some(value) => match decode_lower_hex(value, MAX_PROFILER_BUNDLE_BYTES_V4) {
                        Ok(value) => Some(value),
                        Err(()) => {
                            return self.error(
                                Some(request_id),
                                AgentRuntimeCausalityErrorCodeV1::EvidenceRejected,
                                false,
                            );
                        }
                    },
                    None => None,
                };
                let session = match RuntimeCausalitySessionV1::open(&runtime, profiler.as_deref()) {
                    Ok(value) => value,
                    Err(_) => {
                        return self.error(
                            Some(request_id),
                            AgentRuntimeCausalityErrorCodeV1::EvidenceRejected,
                            false,
                        );
                    }
                };
                let result = session.binding().clone();
                self.session = Some(session);
                self.success(
                    request_id,
                    false,
                    AgentRuntimeCausalityResultV1::Opened(result),
                )
            }
            AgentRuntimeCausalityRequestV1::Binding { .. } => {
                let Some(session) = &self.session else {
                    return self.error(
                        Some(request_id),
                        AgentRuntimeCausalityErrorCodeV1::SessionNotOpen,
                        false,
                    );
                };
                self.success(
                    request_id,
                    false,
                    AgentRuntimeCausalityResultV1::Binding(session.binding().clone()),
                )
            }
            AgentRuntimeCausalityRequestV1::Page {
                kind,
                limit,
                cursor,
                ..
            } => {
                let Some(session) = &self.session else {
                    return self.error(
                        Some(request_id),
                        AgentRuntimeCausalityErrorCodeV1::SessionNotOpen,
                        false,
                    );
                };
                match session.page(kind.into(), cursor, limit) {
                    Ok(page) => {
                        self.success(request_id, false, AgentRuntimeCausalityResultV1::Page(page))
                    }
                    Err(_) => self.error(
                        Some(request_id),
                        AgentRuntimeCausalityErrorCodeV1::QueryRejected,
                        false,
                    ),
                }
            }
            AgentRuntimeCausalityRequestV1::InspectDispatch { dispatch, .. } => {
                let Some(session) = &self.session else {
                    return self.error(
                        Some(request_id),
                        AgentRuntimeCausalityErrorCodeV1::SessionNotOpen,
                        false,
                    );
                };
                match session.inspect_dispatch(dispatch) {
                    Ok(dispatch) => self.success(
                        request_id,
                        false,
                        AgentRuntimeCausalityResultV1::Dispatch(dispatch),
                    ),
                    Err(_) => self.error(
                        Some(request_id),
                        AgentRuntimeCausalityErrorCodeV1::QueryRejected,
                        false,
                    ),
                }
            }
            AgentRuntimeCausalityRequestV1::ProfilerJuxtaposition { .. } => {
                let Some(session) = &self.session else {
                    return self.error(
                        Some(request_id),
                        AgentRuntimeCausalityErrorCodeV1::SessionNotOpen,
                        false,
                    );
                };
                self.success(
                    request_id,
                    false,
                    AgentRuntimeCausalityResultV1::ProfilerJuxtaposition(
                        session.profiler_juxtaposition(),
                    ),
                )
            }
            AgentRuntimeCausalityRequestV1::Close { .. } => {
                if self.session.is_none() {
                    return self.error(
                        Some(request_id),
                        AgentRuntimeCausalityErrorCodeV1::SessionNotOpen,
                        false,
                    );
                }
                self.session = None;
                self.success(request_id, true, AgentRuntimeCausalityResultV1::Closed)
            }
        }
    }

    fn success(
        &mut self,
        request_id: u64,
        terminal: bool,
        result: AgentRuntimeCausalityResultV1,
    ) -> AgentRuntimeCausalityResponseV1 {
        let Some(revision) = self.revision.checked_add(1) else {
            return self.error(
                Some(request_id),
                AgentRuntimeCausalityErrorCodeV1::RevisionExhausted,
                true,
            );
        };
        let response = AgentRuntimeCausalityResponseV1::Ok {
            schema: AGENT_RUNTIME_CAUSALITY_RESPONSE_SCHEMA_V1,
            request_id,
            revision,
            terminal,
            result: Box::new(result),
        };
        if encode_bounded(&response, MAX_AGENT_RUNTIME_CAUSALITY_RESPONSE_BYTES_V1 - 1).is_err() {
            return self.error(
                Some(request_id),
                AgentRuntimeCausalityErrorCodeV1::ResponseTooLarge,
                true,
            );
        }
        self.revision = revision;
        self.terminal = terminal;
        response
    }

    fn error(
        &mut self,
        request_id: Option<u64>,
        code: AgentRuntimeCausalityErrorCodeV1,
        terminal: bool,
    ) -> AgentRuntimeCausalityResponseV1 {
        self.terminal |= terminal;
        AgentRuntimeCausalityResponseV1::Error {
            schema: AGENT_RUNTIME_CAUSALITY_RESPONSE_SCHEMA_V1,
            request_id,
            revision: self.revision,
            terminal,
            code,
        }
    }
}

fn request_header(request: &AgentRuntimeCausalityRequestV1) -> (&str, u64, u64) {
    match request {
        AgentRuntimeCausalityRequestV1::DiscoverCapabilities {
            schema,
            request_id,
            revision,
        }
        | AgentRuntimeCausalityRequestV1::Open {
            schema,
            request_id,
            revision,
            ..
        }
        | AgentRuntimeCausalityRequestV1::Binding {
            schema,
            request_id,
            revision,
        }
        | AgentRuntimeCausalityRequestV1::Page {
            schema,
            request_id,
            revision,
            ..
        }
        | AgentRuntimeCausalityRequestV1::InspectDispatch {
            schema,
            request_id,
            revision,
            ..
        }
        | AgentRuntimeCausalityRequestV1::ProfilerJuxtaposition {
            schema,
            request_id,
            revision,
        }
        | AgentRuntimeCausalityRequestV1::Close {
            schema,
            request_id,
            revision,
        } => (schema, *request_id, *revision),
    }
}

pub fn run_agent_runtime_causality_jsonl_v1<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
) -> Result<(), AgentRuntimeCausalityServiceErrorV1> {
    run_agent_runtime_causality_jsonl_with_limit_v1(
        input,
        output,
        MAX_AGENT_RUNTIME_CAUSALITY_REQUEST_BYTES_V1,
    )
}

fn run_agent_runtime_causality_jsonl_with_limit_v1<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    request_bytes: u64,
) -> Result<(), AgentRuntimeCausalityServiceErrorV1> {
    let mut service = AgentRuntimeCausalityServiceV1::new();
    let mut line = Vec::new();
    loop {
        line.clear();
        let read_limit = request_bytes.saturating_add(2);
        let mut bounded = Read::take(&mut *input, read_limit);
        let read = bounded
            .read_until(b'\n', &mut line)
            .map_err(|_| AgentRuntimeCausalityServiceErrorV1::Io)?;
        if read == 0 {
            return Ok(());
        }
        if let Err(response) = service.begin_attempt() {
            write_response(output, &response)?;
            return Ok(());
        }
        if line.last() != Some(&b'\n')
            || u64::try_from(line.len()).unwrap_or(u64::MAX) > request_bytes.saturating_add(1)
        {
            let response = service.error(
                None,
                AgentRuntimeCausalityErrorCodeV1::RequestTooLarge,
                true,
            );
            write_response(output, &response)?;
            return Ok(());
        }
        line.pop();
        let request: AgentRuntimeCausalityRequestV1 = match serde_json::from_slice(&line) {
            Ok(request)
                if encode_bounded(&request, MAX_AGENT_RUNTIME_CAUSALITY_REQUEST_BYTES_V1)
                    .ok()
                    .as_deref()
                    == Some(line.as_slice()) =>
            {
                request
            }
            _ => {
                let response = service.error(
                    None,
                    AgentRuntimeCausalityErrorCodeV1::InvalidRequest,
                    false,
                );
                write_response(output, &response)?;
                continue;
            }
        };
        let response = service.handle(request);
        if write_response_or_terminal(output, &mut service, &response)? {
            return Ok(());
        }
    }
}

fn write_response_or_terminal(
    output: &mut impl Write,
    service: &mut AgentRuntimeCausalityServiceV1,
    response: &AgentRuntimeCausalityResponseV1,
) -> Result<bool, AgentRuntimeCausalityServiceErrorV1> {
    match write_response(output, response) {
        Ok(()) => Ok(service.terminal),
        Err(AgentRuntimeCausalityServiceErrorV1::ResponseTooLarge)
        | Err(AgentRuntimeCausalityServiceErrorV1::AllocationFailure) => {
            let terminal = service.error(
                None,
                AgentRuntimeCausalityErrorCodeV1::ResponseTooLarge,
                true,
            );
            write_response(output, &terminal)?;
            Ok(true)
        }
        Err(error) => Err(error),
    }
}

fn write_response(
    output: &mut impl Write,
    response: &AgentRuntimeCausalityResponseV1,
) -> Result<(), AgentRuntimeCausalityServiceErrorV1> {
    let bytes = encode_bounded(response, MAX_AGENT_RUNTIME_CAUSALITY_RESPONSE_BYTES_V1 - 1)?;
    output
        .write_all(&bytes)
        .and_then(|()| output.write_all(b"\n"))
        .and_then(|()| output.flush())
        .map_err(|_| AgentRuntimeCausalityServiceErrorV1::Io)
}

fn encode_bounded(
    value: &impl Serialize,
    maximum: u64,
) -> Result<Vec<u8>, AgentRuntimeCausalityServiceErrorV1> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(usize::try_from(maximum.min(64 * 1024)).unwrap_or(64 * 1024))
        .map_err(|_| AgentRuntimeCausalityServiceErrorV1::AllocationFailure)?;
    if u64::try_from(bytes.capacity()).unwrap_or(u64::MAX) > maximum {
        return Err(AgentRuntimeCausalityServiceErrorV1::AllocationFailure);
    }
    let mut failure = None;
    let serialized = {
        let mut serializer = serde_json::Serializer::new(BoundedWriterV1 {
            bytes: &mut bytes,
            maximum,
            failure: &mut failure,
        });
        value.serialize(&mut serializer)
    };
    if serialized.is_err() {
        return Err(failure.unwrap_or(AgentRuntimeCausalityServiceErrorV1::Json));
    }
    Ok(bytes)
}

struct BoundedWriterV1<'a> {
    bytes: &'a mut Vec<u8>,
    maximum: u64,
    failure: &'a mut Option<AgentRuntimeCausalityServiceErrorV1>,
}

impl Write for BoundedWriterV1<'_> {
    fn write(&mut self, input: &[u8]) -> std::io::Result<usize> {
        let maximum = usize::try_from(self.maximum).unwrap_or(usize::MAX);
        let required = self.bytes.len().checked_add(input.len()).ok_or_else(|| {
            *self.failure = Some(AgentRuntimeCausalityServiceErrorV1::ResponseTooLarge);
            std::io::Error::other("runtime causality response size overflow")
        })?;
        if required > maximum {
            *self.failure = Some(AgentRuntimeCausalityServiceErrorV1::ResponseTooLarge);
            return Err(std::io::Error::other(
                "runtime causality response limit exceeded",
            ));
        }
        if required > self.bytes.capacity() {
            let doubled = self.bytes.capacity().checked_mul(2).unwrap_or(maximum);
            let target = required.max(doubled).min(maximum);
            if self
                .bytes
                .try_reserve_exact(target.saturating_sub(self.bytes.capacity()))
                .is_err()
                || self.bytes.capacity() > maximum
            {
                *self.failure = Some(AgentRuntimeCausalityServiceErrorV1::AllocationFailure);
                return Err(std::io::Error::other(
                    "runtime causality response allocation failed",
                ));
            }
        }
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn decode_lower_hex(value: &str, maximum: u64) -> Result<Vec<u8>, ()> {
    let maximum_hex = maximum.checked_mul(2).ok_or(())?;
    if value.is_empty()
        || !value.len().is_multiple_of(2)
        || u64::try_from(value.len()).map_err(|_| ())? > maximum_hex
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

fn deserialize_schema<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_string(deserializer, 128, "schema")
}

fn deserialize_runtime_hex<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_string(
        deserializer,
        usize::try_from(MAX_KFD_RUNTIME_PROFILE_BYTES_V1.saturating_mul(2)).unwrap_or(usize::MAX),
        "runtime profile hex",
    )
}

fn deserialize_optional_profiler_hex<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    match value {
        Some(value) => {
            if value.len()
                > usize::try_from(MAX_PROFILER_BUNDLE_BYTES_V4.saturating_mul(2))
                    .unwrap_or(usize::MAX)
                || !value.is_ascii()
            {
                return Err(serde::de::Error::custom(
                    "bounded profiler bundle hex rejected",
                ));
            }
            Ok(Some(value))
        }
        None => Ok(None),
    }
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
                "an ASCII {} no longer than {} bytes",
                self.label, self.maximum
            )
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if value.len() > self.maximum || !value.is_ascii() {
                return Err(E::custom("bounded runtime causality string rejected"));
            }
            let mut output = String::new();
            output
                .try_reserve_exact(value.len())
                .map_err(|_| E::custom("bounded runtime causality allocation failed"))?;
            output.push_str(value);
            Ok(output)
        }
    }
    deserializer.deserialize_str(BoundedStringVisitorV1 { maximum, label })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCausalityErrorV1 {
    RuntimeProfileRejected,
    ProfilerBundleRejected,
    UnknownDispatch,
    InvalidCursor,
    PageLimitOutOfRange,
    GraphMissingEndpoint,
    GraphCycle,
    GraphDuplicateIdentity,
    GraphStaleEvidence,
    EdgeLimitExceeded,
    SizeOverflow,
    IdentityFailure,
    AllocationFailure,
}

impl fmt::Display for RuntimeCausalityErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "runtime causality query rejected: {self:?}")
    }
}

impl Error for RuntimeCausalityErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentRuntimeCausalityServiceErrorV1 {
    Io,
    Json,
    ResponseTooLarge,
    AllocationFailure,
}

impl fmt::Display for AgentRuntimeCausalityServiceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "runtime causality agent service failed: {self:?}"
        )
    }
}

impl Error for AgentRuntimeCausalityServiceErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_profiler_protocol::{
        KfdProfileAccessV1, KfdProfileDeviceV1, KfdProfileHostContentModeV1,
        KfdProfileMemoryKindV1, KfdProfileResourceKindV1, KfdRuntimeProfileEventV1,
        encode_kfd_runtime_profile_v1, push_observed_event_v1, resource_identity_v1,
    };
    use fe2o3_semantic_import::{
        ContentSchemeV1, ProfilerAttBindingV4, ProfilerDeviceBindingV4, ProfilerDispatchBindingV4,
        ProfilerEnvironmentBindingV4, encode_profiler_bundle_v4,
        import_rocprofv3_att_profiler_bundle_v4, import_rocprofv3_csv_profiler_bundle_v4,
    };
    use fe2o3_semantic_trace::{KernelIrIdentityClaimV1, OpaqueIdentityV1, WaveWidthV1};

    fn runtime_fixture(seed: u8, dropped: u64) -> (Vec<u8>, ProfileIdentityV1) {
        let scope = ProfileIdentityV1::new([seed; 32]).unwrap();
        let queue = resource_identity_v1(scope, KfdProfileResourceKindV1::NativeQueue, 1).unwrap();
        let stream = resource_identity_v1(scope, KfdProfileResourceKindV1::Stream, 2).unwrap();
        let allocation =
            resource_identity_v1(scope, KfdProfileResourceKindV1::Allocation, 3).unwrap();
        let module = resource_identity_v1(scope, KfdProfileResourceKindV1::Module, 4).unwrap();
        let kernel = resource_identity_v1(scope, KfdProfileResourceKindV1::Kernel, 5).unwrap();
        let dispatch = resource_identity_v1(scope, KfdProfileResourceKindV1::Dispatch, 6).unwrap();
        let mut events = Vec::<KfdRuntimeProfileEventV1>::new();
        let mut push = |event| push_observed_event_v1(scope, &mut events, event).unwrap();
        push(KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue });
        push(KfdRuntimeProfileEventKindV1::StreamCreated { stream });
        push(KfdRuntimeProfileEventKindV1::AllocationCreated {
            allocation,
            memory_kind: KfdProfileMemoryKindV1::HostVisible,
            byte_len: 64,
            alignment: 16,
        });
        push(KfdRuntimeProfileEventKindV1::HostWrite {
            allocation,
            byte_offset: 0,
            content: KfdProfileHostContentV1::RangeOnly { byte_len: 8 },
        });
        push(KfdRuntimeProfileEventKindV1::ModuleLoaded {
            module,
            artifact: ProfileContentIdentityV1::observed(b"artifact").unwrap(),
        });
        push(KfdRuntimeProfileEventKindV1::KernelResolved {
            kernel,
            module,
            name: ProfileContentIdentityV1::observed(b"kernel").unwrap(),
            signature: ProfileContentIdentityV1::observed(b"signature").unwrap(),
        });
        push(KfdRuntimeProfileEventKindV1::DispatchPublished {
            dispatch,
            queue,
            stream,
            kernel,
            dispatch_shape: ProfileContentIdentityV1::observed(b"shape").unwrap(),
            launch: KfdProfileLaunchV1 {
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_shared_bytes: 0,
            },
            bindings: vec![KfdProfileBindingV1 {
                allocation,
                access: KfdProfileAccessV1::ReadWrite,
                byte_offset: 0,
                byte_len: 64,
                kernarg_byte_offset: 0,
            }],
        });
        push(KfdRuntimeProfileEventKindV1::DispatchCompleted {
            dispatch,
            host_timing: KfdProfileHostTimingV1 {
                preparation_ns: 1,
                bound_snapshot_ns: 2,
                authority_ns: 3,
                native_binding_ns: 4,
                publication_ns: 5,
                publish_to_completion_ns: 6,
                completed_readback_ns: 7,
                recycle_ns: 8,
            },
        });
        push(KfdRuntimeProfileEventKindV1::SubmissionReleased { dispatch });
        push(KfdRuntimeProfileEventKindV1::HostRead {
            allocation,
            byte_offset: 0,
            content: KfdProfileHostContentV1::RangeOnly { byte_len: 8 },
        });
        push(KfdRuntimeProfileEventKindV1::AllocationReleased { allocation });
        push(KfdRuntimeProfileEventKindV1::StreamDestroyed { stream });
        push(KfdRuntimeProfileEventKindV1::ModuleUnloaded { module });
        push(KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue });
        let profile = KfdRuntimeProfileV1::new(
            scope,
            KfdProfileDeviceV1::observed(u64::from(seed), "gfx942:xnack-", 64).unwrap(),
            KfdProfileHostContentModeV1::RangeOnly,
            events,
            dropped,
        )
        .unwrap();
        (encode_kfd_runtime_profile_v1(&profile).unwrap(), dispatch)
    }

    fn semantic_content(byte: u8) -> ContentIdentityRecordV1 {
        ContentIdentityRecordV1 {
            scheme: ContentSchemeV1::DomainSeparatedSha256,
            format_version: 1,
            digest: CaptureIdentityV1::new([byte; 32]).unwrap(),
            canonical_len: 1,
        }
    }

    fn profiler_fixture_with_environment(environment: u8) -> Vec<u8> {
        let source = br#"{"counter_names":[],"gfxip":9,"gfxv":"vega","global_begin_time":0,"is_pcs_stochastic":false,"pc_sampling":false,"thread_trace":true,"version":"3.0.0","wave_filenames":{"0":{"0":{"0":{"0":["waves/se0.json",10,20]}}}},"se_filenames":["se0.json"]}"#;
        let bundle = import_rocprofv3_att_profiler_bundle_v4(
            source,
            ProfilerAttBindingV4 {
                environment: ProfilerEnvironmentBindingV4 {
                    environment: semantic_content(environment),
                    collector_tool: semantic_content(2),
                    collector_configuration: semantic_content(3),
                    stable_device_bindings: vec![ProfilerDeviceBindingV4 {
                        source_agent_id: 17,
                        stable_identity: semantic_content(4),
                    }],
                },
                source_agent_id: 17,
                referenced_artifacts: Vec::new(),
            },
        )
        .unwrap();
        encode_profiler_bundle_v4(&bundle).unwrap()
    }

    fn profiler_fixture() -> Vec<u8> {
        profiler_fixture_with_environment(1)
    }

    fn dispatch_profiler_fixture() -> Vec<u8> {
        let source = include_bytes!(
            "../../fe2o3-semantic-import/tests/fixtures/rocprofv3-current-kernel-dispatch.csv"
        );
        let bundle = import_rocprofv3_csv_profiler_bundle_v4(
            source,
            ProfilerDispatchBindingV4 {
                environment: ProfilerEnvironmentBindingV4 {
                    environment: semantic_content(1),
                    collector_tool: semantic_content(2),
                    collector_configuration: semantic_content(3),
                    stable_device_bindings: vec![
                        ProfilerDeviceBindingV4 {
                            source_agent_id: 17,
                            stable_identity: semantic_content(4),
                        },
                        ProfilerDeviceBindingV4 {
                            source_agent_id: 19,
                            stable_identity: semantic_content(5),
                        },
                    ],
                },
                kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(
                    OpaqueIdentityV1::new([6; 32]).unwrap(),
                    1,
                )
                .unwrap(),
                artifact: None,
                source_map: None,
                wave_width: WaveWidthV1::Wave64,
            },
        )
        .unwrap();
        encode_profiler_bundle_v4(&bundle).unwrap()
    }

    #[test]
    fn normal_lifecycle_is_paged_and_cites_exact_endpoint_evidence() {
        let (bytes, dispatch) = runtime_fixture(7, 0);
        let session = RuntimeCausalitySessionV1::open(&bytes, None).unwrap();
        let first = session
            .page(RuntimeCausalityPageKindV1::LifecycleEdges, None, 2)
            .unwrap();
        assert_eq!(first.total_items, 5);
        assert_eq!(first.returned, 2);
        let second = session
            .page(
                RuntimeCausalityPageKindV1::LifecycleEdges,
                first.next_cursor,
                3,
            )
            .unwrap();
        assert_eq!(second.returned, 3);
        assert!(second.next_cursor.is_none());
        for item in first.items.iter().chain(&second.items) {
            let RuntimeCausalityPageItemV1::LifecycleEdge(edge) = item else {
                panic!("expected lifecycle edge")
            };
            assert_eq!(edge.origin, TruthOriginV1::Inferred);
            assert!(edge.predecessor_sequence < edge.successor_sequence);
            assert_ne!(edge.predecessor_event, edge.successor_event);
        }
        let detail = session.inspect_dispatch(dispatch).unwrap();
        assert_eq!(detail.host_timing.unwrap().publish_to_completion_ns, 6);
        assert!(matches!(
            detail.dependency_events,
            RuntimeCausalityAvailabilityV1::Unavailable {
                reason: RuntimeCausalityUnavailableReasonV1::NoCanonicalDependencyEvents,
                ..
            }
        ));
        let replay_a = serde_json::to_vec(&second).unwrap();
        let replay_b = serde_json::to_vec(
            &session
                .page(
                    RuntimeCausalityPageKindV1::LifecycleEdges,
                    first.next_cursor,
                    3,
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(replay_a, replay_b);
    }

    #[test]
    fn absent_dependency_and_copy_telemetry_is_not_negative_evidence() {
        let (bytes, _) = runtime_fixture(8, 3);
        let session = RuntimeCausalitySessionV1::open(&bytes, None).unwrap();
        for (kind, reason) in [
            (
                RuntimeCausalityPageKindV1::Dependencies,
                RuntimeCausalityUnavailableReasonV1::NoCanonicalDependencyEvents,
            ),
            (
                RuntimeCausalityPageKindV1::DeviceCopies,
                RuntimeCausalityUnavailableReasonV1::NoDeviceCopyEngineEvents,
            ),
        ] {
            let page = session.page(kind, None, 1).unwrap();
            assert_eq!(page.total_items, 0);
            assert!(page.items.is_empty());
            assert_eq!(
                page.availability,
                RuntimeCausalityAvailabilityV1::Unavailable {
                    origin: TruthOriginV1::Unavailable,
                    reason,
                }
            );
            assert_eq!(page.coverage.dropped_runtime_events, 3);
            assert!(!page.coverage.complete_runtime_operation_history);
        }
    }

    #[test]
    fn incomplete_prefix_never_claims_completion_or_release() {
        let (bytes, dispatch) = runtime_fixture(18, 0);
        let profile = decode_kfd_runtime_profile_v1(&bytes).unwrap();
        let prefix = KfdRuntimeProfileV1::new(
            profile.capture_scope,
            profile.device,
            profile.host_content_mode,
            profile.events.into_iter().take(7).collect(),
            1,
        )
        .unwrap();
        let bytes = encode_kfd_runtime_profile_v1(&prefix).unwrap();
        let session = RuntimeCausalitySessionV1::open(&bytes, None).unwrap();
        let detail = session.inspect_dispatch(dispatch).unwrap();
        assert!(matches!(
            detail.summary.completion,
            RuntimeDispatchEventRefV1::Unavailable {
                reason: RuntimeCausalityUnavailableReasonV1::CaptureIncompleteOrDispatchStillLive,
                ..
            }
        ));
        assert!(matches!(
            detail.summary.release,
            RuntimeDispatchEventRefV1::Unavailable {
                reason: RuntimeCausalityUnavailableReasonV1::CaptureIncompleteOrDispatchStillLive,
                ..
            }
        ));
        assert!(detail.host_timing.is_none());
        assert_eq!(session.binding().coverage.dropped_runtime_events, 1);
    }

    #[test]
    fn profiler_is_only_content_bound_juxtaposition_with_incomparable_clocks() {
        let (runtime, _) = runtime_fixture(9, 0);
        let profiler = profiler_fixture();
        let session = RuntimeCausalitySessionV1::open(&runtime, Some(&profiler)).unwrap();
        let view = session.profiler_juxtaposition();
        assert_eq!(
            view.state,
            RuntimeProfilerJuxtapositionStateV1::BoundWithoutDispatchOrClockJoin
        );
        assert!(view.profiler_content_identity.is_some());
        assert!(matches!(
            view.dispatch_relation,
            RuntimeCausalityAvailabilityV1::Unavailable {
                reason: RuntimeCausalityUnavailableReasonV1::ProfilerBundleHasNoDispatchEnvelopes,
                ..
            }
        ));
        assert!(matches!(
            view.cross_clock_relation,
            RuntimeCausalityAvailabilityV1::Unavailable {
                reason: RuntimeCausalityUnavailableReasonV1::NoAdmittedClockCorrelation,
                ..
            }
        ));
        assert!(matches!(
            view.cross_clock_uncertainty,
            RuntimeCausalityAvailabilityV1::Unavailable {
                reason: RuntimeCausalityUnavailableReasonV1::NoAdmittedClockCorrelation,
                ..
            }
        ));
        assert!(view.profiler_loss.is_some());
        assert!(matches!(
            view.profiler_local_tick_coordinates,
            RuntimeCausalityAvailabilityV1::Unavailable {
                reason: RuntimeCausalityUnavailableReasonV1::ProfilerBundleHasNoDispatchEnvelopes,
                ..
            }
        ));
    }

    #[test]
    fn profiler_local_ticks_do_not_create_a_cross_clock_relation() {
        let (runtime, _) = runtime_fixture(19, 0);
        let profiler = dispatch_profiler_fixture();
        let session = RuntimeCausalitySessionV1::open(&runtime, Some(&profiler)).unwrap();
        let view = session.profiler_juxtaposition();
        assert_eq!(
            view.profiler_local_tick_coordinates,
            RuntimeCausalityAvailabilityV1::Available {
                origin: TruthOriginV1::Observed,
            }
        );
        assert_eq!(
            view.profiler_local_tick_description,
            Some("bundle_v4_capture_local_opaque_collector_ticks_without_frequency")
        );
        assert!(matches!(
            view.cross_clock_relation,
            RuntimeCausalityAvailabilityV1::Unavailable {
                reason: RuntimeCausalityUnavailableReasonV1::NoAdmittedClockCorrelation,
                ..
            }
        ));
    }

    #[test]
    fn canonical_inputs_and_cursors_resist_substitution() {
        let (runtime_a, _) = runtime_fixture(10, 0);
        let (runtime_b, _) = runtime_fixture(11, 0);
        let session_a = RuntimeCausalitySessionV1::open(&runtime_a, None).unwrap();
        let session_b = RuntimeCausalitySessionV1::open(&runtime_b, None).unwrap();
        let page = session_a
            .page(RuntimeCausalityPageKindV1::RuntimeEvents, None, 1)
            .unwrap();
        assert!(matches!(
            session_b.page(
                RuntimeCausalityPageKindV1::RuntimeEvents,
                page.next_cursor,
                1,
            ),
            Err(RuntimeCausalityErrorV1::InvalidCursor)
        ));
        let mut cursor = page.next_cursor.unwrap();
        cursor.next_ordinal = cursor.next_ordinal.checked_add(1).unwrap();
        assert!(matches!(
            session_a.page(RuntimeCausalityPageKindV1::RuntimeEvents, Some(cursor), 1,),
            Err(RuntimeCausalityErrorV1::InvalidCursor)
        ));
        let mut noncanonical_runtime = runtime_a;
        noncanonical_runtime.push(b'\n');
        assert!(matches!(
            RuntimeCausalitySessionV1::open(&noncanonical_runtime, None),
            Err(RuntimeCausalityErrorV1::RuntimeProfileRejected)
        ));
        let mut noncanonical_profiler = profiler_fixture();
        noncanonical_profiler.push(b'\n');
        assert!(matches!(
            RuntimeCausalitySessionV1::open(&runtime_b, Some(&noncanonical_profiler)),
            Err(RuntimeCausalityErrorV1::ProfilerBundleRejected)
        ));

        let profiler_a = profiler_fixture_with_environment(21);
        let profiler_b = profiler_fixture_with_environment(22);
        let session_a = RuntimeCausalitySessionV1::open(&runtime_b, Some(&profiler_a)).unwrap();
        let session_b = RuntimeCausalitySessionV1::open(&runtime_b, Some(&profiler_b)).unwrap();
        let cursor = session_a
            .page(RuntimeCausalityPageKindV1::RuntimeEvents, None, 1)
            .unwrap()
            .next_cursor;
        assert!(matches!(
            session_b.page(RuntimeCausalityPageKindV1::RuntimeEvents, cursor, 1),
            Err(RuntimeCausalityErrorV1::InvalidCursor)
        ));

        assert_eq!(
            session_a.page(RuntimeCausalityPageKindV1::RuntimeEvents, None, 0),
            Err(RuntimeCausalityErrorV1::PageLimitOutOfRange)
        );
    }

    #[test]
    fn graph_validation_rejects_missing_cycles_and_stale_relations() {
        let first = ProfileIdentityV1::new([1; 32]).unwrap();
        let second = ProfileIdentityV1::new([2; 32]).unwrap();
        let unknown = ProfileIdentityV1::new([3; 32]).unwrap();
        let events = vec![
            RuntimeEventSummaryV1 {
                event_identity: first,
                sequence: 0,
                producer_order_domain: "kfd_runtime_profile_v1_event_sequence",
                origin: TruthOriginV1::Observed,
                kind: RuntimeEventKindV1::NativeQueueCreated,
                primary_resource: first,
            },
            RuntimeEventSummaryV1 {
                event_identity: second,
                sequence: 1,
                producer_order_domain: "kfd_runtime_profile_v1_event_sequence",
                origin: TruthOriginV1::Observed,
                kind: RuntimeEventKindV1::DispatchPublished,
                primary_resource: second,
            },
        ];
        let mut edges = Vec::new();
        push_edge(
            &mut edges,
            RuntimeLifecycleEdgeKindV1::QueueExistsBeforeDispatchPublication,
            OwnedEventRefV1 {
                identity: first,
                sequence: 0,
            },
            OwnedEventRefV1 {
                identity: second,
                sequence: 1,
            },
        )
        .unwrap();
        assert!(validate_graph(&events, &edges).is_ok());
        let mut missing = edges.clone();
        missing[0].successor_event = unknown;
        assert_eq!(
            validate_graph(&events, &missing),
            Err(RuntimeCausalityErrorV1::GraphMissingEndpoint)
        );
        let mut cycle = edges.clone();
        cycle[0].predecessor_event = second;
        cycle[0].predecessor_sequence = 1;
        cycle[0].successor_event = first;
        cycle[0].successor_sequence = 0;
        assert_eq!(
            validate_graph(&events, &cycle),
            Err(RuntimeCausalityErrorV1::GraphCycle)
        );
        let duplicate = vec![edges[0], edges[0]];
        assert_eq!(
            validate_graph(&events, &duplicate),
            Err(RuntimeCausalityErrorV1::GraphStaleEvidence)
        );
        let mut stale = edges;
        stale[0].edge_identity = CaptureIdentityV1::new([9; 32]).unwrap();
        assert_eq!(
            validate_graph(&events, &stale),
            Err(RuntimeCausalityErrorV1::GraphStaleEvidence)
        );
    }

    fn request_line(request: &AgentRuntimeCausalityRequestV1) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(request).unwrap();
        bytes.push(b'\n');
        bytes
    }

    fn lower_hex(bytes: &[u8]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        output
    }

    #[test]
    fn jsonl_service_is_canonical_stateful_and_replayable() {
        let (runtime, _) = runtime_fixture(12, 0);
        let requests = [
            AgentRuntimeCausalityRequestV1::DiscoverCapabilities {
                schema: AGENT_RUNTIME_CAUSALITY_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 1,
                revision: 0,
            },
            AgentRuntimeCausalityRequestV1::Open {
                schema: AGENT_RUNTIME_CAUSALITY_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 2,
                revision: 1,
                runtime_profile_hex: lower_hex(&runtime),
                profiler_bundle_hex: None,
            },
            AgentRuntimeCausalityRequestV1::Page {
                schema: AGENT_RUNTIME_CAUSALITY_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 3,
                revision: 2,
                kind: AgentRuntimeCausalityPageKindV1::Dependencies,
                limit: 1,
                cursor: None,
            },
            AgentRuntimeCausalityRequestV1::Close {
                schema: AGENT_RUNTIME_CAUSALITY_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 4,
                revision: 3,
            },
        ];
        let mut input = Vec::new();
        for request in &requests {
            input.extend(request_line(request));
        }
        let mut first = Vec::new();
        run_agent_runtime_causality_jsonl_v1(&mut input.as_slice(), &mut first).unwrap();
        let mut second = Vec::new();
        run_agent_runtime_causality_jsonl_v1(&mut input.as_slice(), &mut second).unwrap();
        assert_eq!(first, second);
        let lines: Vec<_> = first
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .collect();
        assert_eq!(lines.len(), 4);
        let page: serde_json::Value = serde_json::from_slice(lines[2]).unwrap();
        assert_eq!(page["result"]["availability"]["state"], "unavailable");
        assert_eq!(page["result"]["items"], serde_json::json!([]));
    }

    #[test]
    fn jsonl_hostile_ids_attempts_and_unterminated_oversize_are_bounded() {
        let zero = AgentRuntimeCausalityRequestV1::DiscoverCapabilities {
            schema: AGENT_RUNTIME_CAUSALITY_REQUEST_SCHEMA_V1.to_owned(),
            request_id: 0,
            revision: 0,
        };
        let valid = AgentRuntimeCausalityRequestV1::DiscoverCapabilities {
            schema: AGENT_RUNTIME_CAUSALITY_REQUEST_SCHEMA_V1.to_owned(),
            request_id: 1,
            revision: 0,
        };
        let duplicate = AgentRuntimeCausalityRequestV1::DiscoverCapabilities {
            schema: AGENT_RUNTIME_CAUSALITY_REQUEST_SCHEMA_V1.to_owned(),
            request_id: 1,
            revision: 1,
        };
        let mut ids = request_line(&zero);
        ids.extend(request_line(&valid));
        ids.extend(request_line(&duplicate));
        let mut output = Vec::new();
        run_agent_runtime_causality_jsonl_v1(&mut ids.as_slice(), &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("invalid_request_id"));
        assert!(text.contains("duplicate_request_id"));

        let mut attempts = b"{}\n"
            .repeat(usize::try_from(MAX_AGENT_RUNTIME_CAUSALITY_REQUEST_ATTEMPTS_V1).unwrap());
        attempts.extend_from_slice(b"{}\n");
        let mut output = Vec::new();
        run_agent_runtime_causality_jsonl_v1(&mut attempts.as_slice(), &mut output).unwrap();
        assert_eq!(output.iter().filter(|byte| **byte == b'\n').count(), 65);
        assert!(
            String::from_utf8(output)
                .unwrap()
                .contains("request_attempt_limit")
        );

        let hostile = b"xxxxx\n{}\n";
        let mut output = Vec::new();
        run_agent_runtime_causality_jsonl_with_limit_v1(&mut hostile.as_slice(), &mut output, 4)
            .unwrap();
        assert_eq!(output.iter().filter(|byte| **byte == b'\n').count(), 1);
        assert!(
            String::from_utf8(output)
                .unwrap()
                .contains("request_too_large")
        );
    }

    #[test]
    fn bounded_encoder_fails_before_partial_publication() {
        let response = AgentRuntimeCausalityResponseV1::Ok {
            schema: AGENT_RUNTIME_CAUSALITY_RESPONSE_SCHEMA_V1,
            request_id: u64::MAX,
            revision: u64::MAX,
            terminal: false,
            result: Box::new(AgentRuntimeCausalityResultV1::Capabilities(
                capabilities().unwrap(),
            )),
        };
        assert_eq!(
            encode_bounded(&response, 8),
            Err(AgentRuntimeCausalityServiceErrorV1::ResponseTooLarge)
        );
    }
}
