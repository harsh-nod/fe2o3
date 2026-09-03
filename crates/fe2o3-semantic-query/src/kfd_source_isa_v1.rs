//! Exact artifact-level correlation between direct-KFD runtime observations and
//! admitted source/IR/ISA summary evidence.
//!
//! A runtime dispatch has no observed PC in the direct-KFD capture. This module
//! therefore binds a dispatch to every matching compilation-unit summary in
//! its exact loaded artifact, while keeping site attribution explicitly
//! unavailable. It never guesses a source site from a kernel name.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::io::{self, BufRead, Write};

use fe2o3_profiler_protocol::{
    KfdProfileLaunchV1, KfdRuntimeProfileEventKindV1, MAX_KFD_RUNTIME_PROFILE_BYTES_V1,
    ProfileContentIdentityV1, ProfileIdentityV1, decode_kfd_runtime_profile_v1,
    kfd_runtime_profile_content_identity_v1,
};
use fe2o3_source_isa_observation::wire_v1::{
    AdmittedSourceIsaObservationV1, MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1,
    SourceIsaObservationCollectionV1, SourceIsaObservationFrameV1,
    SourceIsaObservationKirVersionV1, SourceIsaObservationOutcomeV1,
    SourceIsaObservationTargetProfileV1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const AGENT_KFD_SOURCE_ISA_REQUEST_SCHEMA_V1: &str = "fe2o3-agent-kfd-source-isa-request-v1";
pub const AGENT_KFD_SOURCE_ISA_RESPONSE_SCHEMA_V1: &str = "fe2o3-agent-kfd-source-isa-response-v1";
pub const MAX_AGENT_KFD_SOURCE_ISA_REQUESTS_V1: u32 = 64;
pub const MAX_AGENT_KFD_SOURCE_ISA_PAGE_ITEMS_V1: u16 = 4_096;
pub const MAX_AGENT_KFD_SOURCE_ISA_REQUEST_BYTES_V1: u64 = (MAX_KFD_RUNTIME_PROFILE_BYTES_V1 * 2)
    + ((MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1 as u64) * 2)
    + (64 * 1024);
pub const MAX_AGENT_KFD_SOURCE_ISA_RESPONSE_BYTES_V1: u64 = 4 * 1024 * 1024;

const INPUT_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.kfd-source-isa.input.v1\0";
const BINDING_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.kfd-source-isa.binding.v1\0";
const CURSOR_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.kfd-source-isa.cursor.v1\0";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentKfdSourceIsaRequestV1 {
    DiscoverCapabilities {
        schema: String,
        request_id: u64,
        expected_revision: u64,
    },
    OpenEvidence {
        schema: String,
        request_id: u64,
        expected_revision: u64,
        capture_hex: String,
        source_isa_collection_hex: String,
    },
    InspectBinding {
        schema: String,
        request_id: u64,
        expected_revision: u64,
    },
    ListDispatches {
        schema: String,
        request_id: u64,
        expected_revision: u64,
        #[serde(default)]
        cursor: Option<KfdSourceIsaCursorV1>,
        limit: u16,
    },
    InspectDispatch {
        schema: String,
        request_id: u64,
        expected_revision: u64,
        dispatch_identity: String,
    },
}

impl AgentKfdSourceIsaRequestV1 {
    fn schema(&self) -> &str {
        match self {
            Self::DiscoverCapabilities { schema, .. }
            | Self::OpenEvidence { schema, .. }
            | Self::InspectBinding { schema, .. }
            | Self::ListDispatches { schema, .. }
            | Self::InspectDispatch { schema, .. } => schema,
        }
    }

    fn request_id(&self) -> u64 {
        match self {
            Self::DiscoverCapabilities { request_id, .. }
            | Self::OpenEvidence { request_id, .. }
            | Self::InspectBinding { request_id, .. }
            | Self::ListDispatches { request_id, .. }
            | Self::InspectDispatch { request_id, .. } => *request_id,
        }
    }

    fn expected_revision(&self) -> u64 {
        match self {
            Self::DiscoverCapabilities {
                expected_revision, ..
            }
            | Self::OpenEvidence {
                expected_revision, ..
            }
            | Self::InspectBinding {
                expected_revision, ..
            }
            | Self::ListDispatches {
                expected_revision, ..
            }
            | Self::InspectDispatch {
                expected_revision, ..
            } => *expected_revision,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KfdSourceIsaOperationV1 {
    DiscoverCapabilities,
    OpenEvidence,
    InspectBinding,
    ListDispatches,
    InspectDispatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KfdSourceIsaAuthorityV1 {
    ReadOnlyNoCompilerProofLoadDispatchAttachOrCollectionAuthority,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct KfdSourceIsaCapabilitiesV1 {
    pub operations: Vec<KfdSourceIsaOperationV1>,
    pub authority: KfdSourceIsaAuthorityV1,
    pub exact_input_encoding: &'static str,
    pub correlation_scope: &'static str,
    pub unavailable_without_observed_pc: Vec<&'static str>,
    pub max_capture_bytes: u64,
    pub max_source_isa_collection_bytes: u64,
    pub max_page_items: u16,
    pub max_requests: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct KfdSourceIsaContentIdentityV1 {
    pub scheme: &'static str,
    pub digest: String,
    pub byte_len: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KfdSourceIsaCollectionCoverageV1 {
    Complete,
    Incomplete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KfdSourceIsaArtifactRelationV1 {
    Exact,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KfdSourceIsaUnavailableReasonV1 {
    NoMatchingArtifact,
    TargetProfileMismatch,
    NoAdmittedCompilationUnit,
    SiteRequiresObservedPcOrSemanticEvent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct KfdSourceIsaFrameV1 {
    pub frame_identity: String,
    pub compilation_unit_identity: String,
    pub correlation_identity: String,
    pub artifact_sha256: String,
    pub artifact_byte_len: u64,
    pub structural_map_identity: String,
    pub target_profile: &'static str,
    pub kir_version: u16,
    pub neutral_kir: KfdSourceIsaContentIdentityV1,
    pub target_kir: KfdSourceIsaContentIdentityV1,
    pub source_anchored_records: u64,
    pub eliminated_records: u64,
    pub records_without_source: u64,
    pub isa_references: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct KfdSourceIsaArtifactBindingV1 {
    pub relation: KfdSourceIsaArtifactRelationV1,
    pub unavailable_reason: Option<KfdSourceIsaUnavailableReasonV1>,
    pub collection_coverage: KfdSourceIsaCollectionCoverageV1,
    pub matching_compilation_units: Vec<KfdSourceIsaFrameV1>,
    pub site_attribution: KfdSourceIsaArtifactRelationV1,
    pub site_unavailable_reason: KfdSourceIsaUnavailableReasonV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct KfdSourceIsaDispatchSummaryV1 {
    pub dispatch_identity: String,
    pub dispatch_event_identity: String,
    pub kernel_identity: String,
    pub module_identity: String,
    pub artifact: KfdSourceIsaContentIdentityV1,
    pub artifact_relation: KfdSourceIsaArtifactRelationV1,
    pub unavailable_reason: Option<KfdSourceIsaUnavailableReasonV1>,
    pub matching_compilation_unit_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct KfdSourceIsaDispatchV1 {
    pub summary: KfdSourceIsaDispatchSummaryV1,
    pub launch: KfdProfileLaunchV1,
    pub binding_count: u64,
    pub semantic_binding: KfdSourceIsaArtifactBindingV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct KfdSourceIsaBindingSummaryV1 {
    pub binding_identity: String,
    pub capture_input: KfdSourceIsaContentIdentityV1,
    pub capture_content_identity: KfdSourceIsaContentIdentityV1,
    pub source_isa_collection_input: KfdSourceIsaContentIdentityV1,
    pub target_profile: String,
    pub collection_coverage: KfdSourceIsaCollectionCoverageV1,
    pub observed_dispatch_count: u64,
    pub exact_artifact_dispatch_count: u64,
    pub unavailable_dispatch_count: u64,
    pub authority: KfdSourceIsaAuthorityV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KfdSourceIsaCursorV1 {
    pub binding_identity: String,
    pub next_ordinal: u64,
    pub cursor_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum AgentKfdSourceIsaResultV1 {
    Capabilities {
        capabilities: KfdSourceIsaCapabilitiesV1,
    },
    Opened {
        binding: KfdSourceIsaBindingSummaryV1,
    },
    Binding {
        binding: KfdSourceIsaBindingSummaryV1,
    },
    DispatchPage {
        binding_identity: String,
        items: Vec<KfdSourceIsaDispatchSummaryV1>,
        next_cursor: Option<KfdSourceIsaCursorV1>,
    },
    Dispatch {
        binding_identity: String,
        dispatch: KfdSourceIsaDispatchV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKfdSourceIsaErrorCodeV1 {
    InvalidRequest,
    InvalidSchema,
    InvalidRequestId,
    DuplicateRequestId,
    StaleRevision,
    RequestBudgetExhausted,
    EvidenceNotOpen,
    EvidenceEncoding,
    CaptureAdmission,
    SourceIsaAdmission,
    PageLimit,
    CursorMismatch,
    InvalidDispatchIdentity,
    UnknownDispatch,
    ResponseTooLarge,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AgentKfdSourceIsaResponseV1 {
    Ok {
        schema: &'static str,
        request_id: u64,
        response_revision: u64,
        value: Box<AgentKfdSourceIsaResultV1>,
    },
    Error {
        schema: &'static str,
        request_id: Option<u64>,
        response_revision: u64,
        code: AgentKfdSourceIsaErrorCodeV1,
        terminal: bool,
    },
}

impl AgentKfdSourceIsaResponseV1 {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Error { terminal: true, .. })
    }
}

struct AdmittedKfdSourceIsaV1 {
    target_profile: String,
    collection_coverage: KfdSourceIsaCollectionCoverageV1,
    admitted_compilation_units: bool,
    source_artifacts: BTreeMap<ProfileIdentityV1, IndexedSourceArtifactV1>,
    dispatches: Vec<IndexedKfdDispatchV1>,
    capture_input: KfdSourceIsaContentIdentityV1,
    capture_content: KfdSourceIsaContentIdentityV1,
    collection_input: KfdSourceIsaContentIdentityV1,
    binding_identity: [u8; 32],
}

struct IndexedSourceArtifactV1 {
    matching_compilation_units: Vec<KfdSourceIsaFrameV1>,
    observed_with_other_target: bool,
}

#[derive(Clone, Copy)]
struct IndexedKfdDispatchV1 {
    dispatch: ProfileIdentityV1,
    event: ProfileIdentityV1,
    kernel: ProfileIdentityV1,
    module: ProfileIdentityV1,
    artifact: ProfileContentIdentityV1,
    launch: KfdProfileLaunchV1,
    binding_count: u64,
}

impl AdmittedKfdSourceIsaV1 {
    fn new(
        capture_bytes: &[u8],
        collection_bytes: &[u8],
    ) -> Result<Self, AgentKfdSourceIsaErrorCodeV1> {
        let capture = decode_kfd_runtime_profile_v1(capture_bytes)
            .map_err(|_| AgentKfdSourceIsaErrorCodeV1::CaptureAdmission)?;
        let collection = SourceIsaObservationCollectionV1::decode_canonical(collection_bytes)
            .map_err(|_| AgentKfdSourceIsaErrorCodeV1::SourceIsaAdmission)?;
        let capture_input = input_identity(capture_bytes);
        let collection_input = input_identity(collection_bytes);
        let protocol_identity = kfd_runtime_profile_content_identity_v1(capture_bytes)
            .map_err(|_| AgentKfdSourceIsaErrorCodeV1::CaptureAdmission)?;
        let capture_content = KfdSourceIsaContentIdentityV1 {
            scheme: "fe2o3_kfd_runtime_profile_content_v1",
            digest: hex(&protocol_identity.digest.as_bytes()),
            byte_len: protocol_identity.byte_len,
        };
        let mut hasher = Sha256::new();
        hasher.update(BINDING_IDENTITY_DOMAIN_V1);
        hasher.update(capture_input.digest.as_bytes());
        hasher.update(collection_input.digest.as_bytes());
        let binding_identity = hasher.finalize().into();

        let collection_coverage =
            if collection.failure().is_none() && collection.missing_units().is_empty() {
                KfdSourceIsaCollectionCoverageV1::Complete
            } else {
                KfdSourceIsaCollectionCoverageV1::Incomplete
            };
        let mut source_artifacts: BTreeMap<ProfileIdentityV1, IndexedSourceArtifactV1> =
            BTreeMap::new();
        let mut admitted_compilation_units = false;
        for frame in collection.frames() {
            let SourceIsaObservationOutcomeV1::Admitted(admitted) = frame.outcome() else {
                continue;
            };
            admitted_compilation_units = true;
            let source_artifact = admitted.artifact();
            let expected = ProfileContentIdentityV1::observed_sha256(
                source_artifact.byte_len(),
                source_artifact.sha256(),
            )
            .map_err(|_| AgentKfdSourceIsaErrorCodeV1::SourceIsaAdmission)?;
            let indexed = source_artifacts.entry(expected.digest).or_insert_with(|| {
                IndexedSourceArtifactV1 {
                    matching_compilation_units: Vec::new(),
                    observed_with_other_target: false,
                }
            });
            if target_matches(
                &capture.device.target_profile,
                capture.device.wave_width,
                admitted.structural().target_profile(),
            ) {
                indexed
                    .matching_compilation_units
                    .push(source_frame(frame, &admitted));
            } else {
                indexed.observed_with_other_target = true;
            }
        }
        for artifact in source_artifacts.values_mut() {
            artifact.matching_compilation_units.sort_by(|left, right| {
                left.compilation_unit_identity
                    .cmp(&right.compilation_unit_identity)
            });
        }

        let mut modules = BTreeMap::new();
        let mut kernels = BTreeMap::new();
        let mut dispatches = Vec::new();
        dispatches
            .try_reserve_exact(capture.events.len())
            .map_err(|_| AgentKfdSourceIsaErrorCodeV1::CaptureAdmission)?;
        for event in &capture.events {
            match &event.event {
                KfdRuntimeProfileEventKindV1::ModuleLoaded { module, artifact } => {
                    modules.insert(*module, *artifact);
                }
                KfdRuntimeProfileEventKindV1::KernelResolved { kernel, module, .. } => {
                    let artifact = modules
                        .get(module)
                        .copied()
                        .ok_or(AgentKfdSourceIsaErrorCodeV1::CaptureAdmission)?;
                    kernels.insert(*kernel, (*module, artifact));
                }
                KfdRuntimeProfileEventKindV1::DispatchPublished {
                    dispatch,
                    kernel,
                    launch,
                    bindings,
                    ..
                } => {
                    let (module, artifact) = kernels
                        .get(kernel)
                        .copied()
                        .ok_or(AgentKfdSourceIsaErrorCodeV1::CaptureAdmission)?;
                    dispatches.push(IndexedKfdDispatchV1 {
                        dispatch: *dispatch,
                        event: event.identity,
                        kernel: *kernel,
                        module,
                        artifact,
                        launch: *launch,
                        binding_count: bindings.len() as u64,
                    });
                }
                _ => {}
            }
        }
        Ok(Self {
            target_profile: capture.device.target_profile,
            collection_coverage,
            admitted_compilation_units,
            source_artifacts,
            dispatches,
            capture_input,
            capture_content,
            collection_input,
            binding_identity,
        })
    }

    fn collection_coverage(&self) -> KfdSourceIsaCollectionCoverageV1 {
        self.collection_coverage
    }

    fn artifact_relation(
        &self,
        artifact: ProfileContentIdentityV1,
    ) -> (
        KfdSourceIsaArtifactRelationV1,
        Option<KfdSourceIsaUnavailableReasonV1>,
        usize,
    ) {
        let indexed = self.source_artifacts.get(&artifact.digest);
        let matching = indexed.map_or(0, |item| item.matching_compilation_units.len());
        let unavailable = if matching != 0 {
            None
        } else if indexed.is_some_and(|item| item.observed_with_other_target) {
            Some(KfdSourceIsaUnavailableReasonV1::TargetProfileMismatch)
        } else if !self.admitted_compilation_units {
            Some(KfdSourceIsaUnavailableReasonV1::NoAdmittedCompilationUnit)
        } else {
            Some(KfdSourceIsaUnavailableReasonV1::NoMatchingArtifact)
        };
        (
            if unavailable.is_none() {
                KfdSourceIsaArtifactRelationV1::Exact
            } else {
                KfdSourceIsaArtifactRelationV1::Unavailable
            },
            unavailable,
            matching,
        )
    }

    fn artifact_binding(
        &self,
        artifact: ProfileContentIdentityV1,
    ) -> KfdSourceIsaArtifactBindingV1 {
        let (relation, unavailable_reason, _) = self.artifact_relation(artifact);
        let matching_compilation_units = self
            .source_artifacts
            .get(&artifact.digest)
            .map_or_else(Vec::new, |item| item.matching_compilation_units.clone());
        KfdSourceIsaArtifactBindingV1 {
            relation,
            unavailable_reason,
            collection_coverage: self.collection_coverage(),
            matching_compilation_units,
            site_attribution: KfdSourceIsaArtifactRelationV1::Unavailable,
            site_unavailable_reason:
                KfdSourceIsaUnavailableReasonV1::SiteRequiresObservedPcOrSemanticEvent,
        }
    }

    fn dispatch_summary(&self, dispatch: &IndexedKfdDispatchV1) -> KfdSourceIsaDispatchSummaryV1 {
        let (artifact_relation, unavailable_reason, matching) =
            self.artifact_relation(dispatch.artifact);
        KfdSourceIsaDispatchSummaryV1 {
            dispatch_identity: profile_identity(dispatch.dispatch),
            dispatch_event_identity: profile_identity(dispatch.event),
            kernel_identity: profile_identity(dispatch.kernel),
            module_identity: profile_identity(dispatch.module),
            artifact: profile_content(dispatch.artifact),
            artifact_relation,
            unavailable_reason,
            matching_compilation_unit_count: matching as u64,
        }
    }

    fn dispatch(&self, dispatch: &IndexedKfdDispatchV1) -> KfdSourceIsaDispatchV1 {
        KfdSourceIsaDispatchV1 {
            summary: self.dispatch_summary(dispatch),
            launch: dispatch.launch,
            binding_count: dispatch.binding_count,
            semantic_binding: self.artifact_binding(dispatch.artifact),
        }
    }

    fn summary(&self) -> KfdSourceIsaBindingSummaryV1 {
        let exact = self
            .dispatches
            .iter()
            .filter(|dispatch| {
                self.artifact_relation(dispatch.artifact).0 == KfdSourceIsaArtifactRelationV1::Exact
            })
            .count() as u64;
        KfdSourceIsaBindingSummaryV1 {
            binding_identity: hex(&self.binding_identity),
            capture_input: self.capture_input.clone(),
            capture_content_identity: self.capture_content.clone(),
            source_isa_collection_input: self.collection_input.clone(),
            target_profile: self.target_profile.clone(),
            collection_coverage: self.collection_coverage(),
            observed_dispatch_count: self.dispatches.len() as u64,
            exact_artifact_dispatch_count: exact,
            unavailable_dispatch_count: self.dispatches.len() as u64 - exact,
            authority:
                KfdSourceIsaAuthorityV1::ReadOnlyNoCompilerProofLoadDispatchAttachOrCollectionAuthority,
        }
    }
}

pub struct AgentKfdSourceIsaServiceV1 {
    evidence: Option<AdmittedKfdSourceIsaV1>,
    revision: u64,
    remaining_requests: u32,
    seen_request_ids: BTreeSet<u64>,
}

impl Default for AgentKfdSourceIsaServiceV1 {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentKfdSourceIsaServiceV1 {
    pub fn new() -> Self {
        Self {
            evidence: None,
            revision: 0,
            remaining_requests: MAX_AGENT_KFD_SOURCE_ISA_REQUESTS_V1,
            seen_request_ids: BTreeSet::new(),
        }
    }

    pub fn handle(&mut self, request: AgentKfdSourceIsaRequestV1) -> AgentKfdSourceIsaResponseV1 {
        let request_id = request.request_id();
        if self.remaining_requests == 0 {
            return self.error(
                Some(request_id),
                AgentKfdSourceIsaErrorCodeV1::RequestBudgetExhausted,
                true,
            );
        }
        self.remaining_requests -= 1;
        if request_id == 0 {
            return self.error(
                Some(request_id),
                AgentKfdSourceIsaErrorCodeV1::InvalidRequestId,
                false,
            );
        }
        if !self.seen_request_ids.insert(request_id) {
            return self.error(
                Some(request_id),
                AgentKfdSourceIsaErrorCodeV1::DuplicateRequestId,
                false,
            );
        }
        if request.schema() != AGENT_KFD_SOURCE_ISA_REQUEST_SCHEMA_V1 {
            return self.error(
                Some(request_id),
                AgentKfdSourceIsaErrorCodeV1::InvalidSchema,
                false,
            );
        }
        if request.expected_revision() != self.revision {
            return self.error(
                Some(request_id),
                AgentKfdSourceIsaErrorCodeV1::StaleRevision,
                false,
            );
        }

        let value = match request {
            AgentKfdSourceIsaRequestV1::DiscoverCapabilities { .. } => {
                AgentKfdSourceIsaResultV1::Capabilities {
                    capabilities: KfdSourceIsaCapabilitiesV1 {
                        operations: vec![
                            KfdSourceIsaOperationV1::DiscoverCapabilities,
                            KfdSourceIsaOperationV1::OpenEvidence,
                            KfdSourceIsaOperationV1::InspectBinding,
                            KfdSourceIsaOperationV1::ListDispatches,
                            KfdSourceIsaOperationV1::InspectDispatch,
                        ],
                        authority: KfdSourceIsaAuthorityV1::ReadOnlyNoCompilerProofLoadDispatchAttachOrCollectionAuthority,
                        exact_input_encoding: "canonical_lowercase_hex_of_exact_bytes",
                        correlation_scope: "exact_loaded_artifact_to_all_matching_admitted_compilation_units",
                        unavailable_without_observed_pc: vec![
                            "source_site",
                            "mir_location",
                            "kir_operation",
                            "schedule_operation",
                            "llvm_location",
                            "isa_interval",
                        ],
                        max_capture_bytes: MAX_KFD_RUNTIME_PROFILE_BYTES_V1,
                        max_source_isa_collection_bytes: MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1 as u64,
                        max_page_items: MAX_AGENT_KFD_SOURCE_ISA_PAGE_ITEMS_V1,
                        max_requests: MAX_AGENT_KFD_SOURCE_ISA_REQUESTS_V1,
                    },
                }
            }
            AgentKfdSourceIsaRequestV1::OpenEvidence {
                capture_hex,
                source_isa_collection_hex,
                ..
            } => {
                let capture = match decode_hex(&capture_hex, MAX_KFD_RUNTIME_PROFILE_BYTES_V1) {
                    Ok(bytes) => bytes,
                    Err(code) => return self.error(Some(request_id), code, false),
                };
                let collection = match decode_hex(
                    &source_isa_collection_hex,
                    MAX_SOURCE_ISA_OBSERVATION_COLLECTION_BYTES_V1 as u64,
                ) {
                    Ok(bytes) => bytes,
                    Err(code) => return self.error(Some(request_id), code, false),
                };
                let evidence = match AdmittedKfdSourceIsaV1::new(&capture, &collection) {
                    Ok(evidence) => evidence,
                    Err(code) => return self.error(Some(request_id), code, false),
                };
                let binding = evidence.summary();
                self.evidence = Some(evidence);
                AgentKfdSourceIsaResultV1::Opened { binding }
            }
            AgentKfdSourceIsaRequestV1::InspectBinding { .. } => {
                let Some(evidence) = &self.evidence else {
                    return self.error(Some(request_id), AgentKfdSourceIsaErrorCodeV1::EvidenceNotOpen, false);
                };
                AgentKfdSourceIsaResultV1::Binding { binding: evidence.summary() }
            }
            AgentKfdSourceIsaRequestV1::ListDispatches { cursor, limit, .. } => {
                let Some(evidence) = &self.evidence else {
                    return self.error(Some(request_id), AgentKfdSourceIsaErrorCodeV1::EvidenceNotOpen, false);
                };
                if limit == 0 || limit > MAX_AGENT_KFD_SOURCE_ISA_PAGE_ITEMS_V1 {
                    return self.error(Some(request_id), AgentKfdSourceIsaErrorCodeV1::PageLimit, false);
                }
                let start = match cursor {
                    None => 0,
                    Some(cursor) => match validate_cursor(evidence.binding_identity, &cursor) {
                        Some(value) => value,
                        None => return self.error(Some(request_id), AgentKfdSourceIsaErrorCodeV1::CursorMismatch, false),
                    },
                };
                if start > evidence.dispatches.len() {
                    return self.error(Some(request_id), AgentKfdSourceIsaErrorCodeV1::CursorMismatch, false);
                }
                let end = start.saturating_add(limit as usize).min(evidence.dispatches.len());
                let items = evidence.dispatches[start..end]
                    .iter()
                    .map(|dispatch| evidence.dispatch_summary(dispatch))
                    .collect();
                let next_cursor = (end < evidence.dispatches.len())
                    .then(|| cursor_for(evidence.binding_identity, end));
                AgentKfdSourceIsaResultV1::DispatchPage {
                    binding_identity: hex(&evidence.binding_identity),
                    items,
                    next_cursor,
                }
            }
            AgentKfdSourceIsaRequestV1::InspectDispatch { dispatch_identity, .. } => {
                let Some(evidence) = &self.evidence else {
                    return self.error(Some(request_id), AgentKfdSourceIsaErrorCodeV1::EvidenceNotOpen, false);
                };
                let Some(dispatch_identity) = parse_identity(&dispatch_identity) else {
                    return self.error(Some(request_id), AgentKfdSourceIsaErrorCodeV1::InvalidDispatchIdentity, false);
                };
                let dispatch = evidence
                    .dispatches
                    .iter()
                    .find(|dispatch| dispatch.dispatch.as_bytes() == dispatch_identity);
                let Some(dispatch) = dispatch else {
                    return self.error(Some(request_id), AgentKfdSourceIsaErrorCodeV1::UnknownDispatch, false);
                };
                AgentKfdSourceIsaResultV1::Dispatch {
                    binding_identity: hex(&evidence.binding_identity),
                    dispatch: evidence.dispatch(dispatch),
                }
            }
        };
        self.ok(request_id, value)
    }

    fn reject_record(&mut self, request_id: Option<u64>) -> AgentKfdSourceIsaResponseV1 {
        if self.remaining_requests == 0 {
            return self.error(
                request_id,
                AgentKfdSourceIsaErrorCodeV1::RequestBudgetExhausted,
                true,
            );
        }
        self.remaining_requests -= 1;
        let code = match request_id {
            Some(request_id) if request_id != 0 && !self.seen_request_ids.insert(request_id) => {
                AgentKfdSourceIsaErrorCodeV1::DuplicateRequestId
            }
            _ => AgentKfdSourceIsaErrorCodeV1::InvalidRequest,
        };
        self.error(request_id, code, false)
    }

    fn ok(
        &mut self,
        request_id: u64,
        value: AgentKfdSourceIsaResultV1,
    ) -> AgentKfdSourceIsaResponseV1 {
        self.revision = self.revision.saturating_add(1);
        AgentKfdSourceIsaResponseV1::Ok {
            schema: AGENT_KFD_SOURCE_ISA_RESPONSE_SCHEMA_V1,
            request_id,
            response_revision: self.revision,
            value: Box::new(value),
        }
    }

    fn error(
        &mut self,
        request_id: Option<u64>,
        code: AgentKfdSourceIsaErrorCodeV1,
        terminal: bool,
    ) -> AgentKfdSourceIsaResponseV1 {
        self.revision = self.revision.saturating_add(1);
        AgentKfdSourceIsaResponseV1::Error {
            schema: AGENT_KFD_SOURCE_ISA_RESPONSE_SCHEMA_V1,
            request_id,
            response_revision: self.revision,
            code,
            terminal,
        }
    }
}

pub fn run_agent_kfd_source_isa_jsonl_v1<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
) -> Result<(), AgentKfdSourceIsaServiceErrorV1> {
    let mut service = AgentKfdSourceIsaServiceV1::new();
    let mut line = Vec::new();
    loop {
        line.clear();
        let mut bounded =
            std::io::Read::take(&mut *input, MAX_AGENT_KFD_SOURCE_ISA_REQUEST_BYTES_V1 + 2);
        let read = bounded
            .read_until(b'\n', &mut line)
            .map_err(|_| AgentKfdSourceIsaServiceErrorV1::Io)?;
        if read == 0 {
            return Ok(());
        }
        if line.last() != Some(&b'\n')
            || line.len() as u64 > MAX_AGENT_KFD_SOURCE_ISA_REQUEST_BYTES_V1 + 1
        {
            let mut response = service.reject_record(None);
            if let AgentKfdSourceIsaResponseV1::Error { terminal, .. } = &mut response {
                *terminal = true;
            }
            write_response_with_size_fallback(output, &mut service, &response)?;
            return Ok(());
        }
        line.pop();
        if line.last() == Some(&b'\r') || line.is_empty() {
            let response = service.reject_record(None);
            let terminal = write_response_with_size_fallback(output, &mut service, &response)?;
            if terminal {
                return Ok(());
            }
            continue;
        }
        let request: AgentKfdSourceIsaRequestV1 = match serde_json::from_slice(&line) {
            Ok(request) => request,
            Err(_) => {
                let response = service.reject_record(None);
                let terminal = write_response_with_size_fallback(output, &mut service, &response)?;
                if terminal {
                    return Ok(());
                }
                continue;
            }
        };
        if serde_json::to_vec(&request).map_err(|_| AgentKfdSourceIsaServiceErrorV1::Json)? != line
        {
            let response = service.reject_record(Some(request.request_id()));
            let terminal = write_response_with_size_fallback(output, &mut service, &response)?;
            if terminal {
                return Ok(());
            }
            continue;
        }
        let response = service.handle(request);
        let terminal = write_response_with_size_fallback(output, &mut service, &response)?;
        if terminal {
            return Ok(());
        }
    }
}

fn write_response(
    output: &mut impl Write,
    response: &AgentKfdSourceIsaResponseV1,
) -> Result<(), AgentKfdSourceIsaServiceErrorV1> {
    let bytes = encode_response(response)?;
    output
        .write_all(&bytes)
        .map_err(|_| AgentKfdSourceIsaServiceErrorV1::Io)?;
    output
        .flush()
        .map_err(|_| AgentKfdSourceIsaServiceErrorV1::Io)
}

fn write_response_with_size_fallback(
    output: &mut impl Write,
    service: &mut AgentKfdSourceIsaServiceV1,
    response: &AgentKfdSourceIsaResponseV1,
) -> Result<bool, AgentKfdSourceIsaServiceErrorV1> {
    match write_response(output, response) {
        Ok(()) => Ok(response.is_terminal()),
        Err(AgentKfdSourceIsaServiceErrorV1::ResponseTooLarge) => {
            let terminal =
                service.error(None, AgentKfdSourceIsaErrorCodeV1::ResponseTooLarge, true);
            write_response(output, &terminal)?;
            Ok(true)
        }
        Err(error) => Err(error),
    }
}

fn encode_response(
    response: &AgentKfdSourceIsaResponseV1,
) -> Result<Vec<u8>, AgentKfdSourceIsaServiceErrorV1> {
    let mut output = Vec::new();
    let mut writer = BoundedResponseWriterV1 {
        output: &mut output,
        max: MAX_AGENT_KFD_SOURCE_ISA_RESPONSE_BYTES_V1 - 1,
        exceeded: false,
    };
    serde_json::to_writer(&mut writer, response).map_err(|_| {
        if writer.exceeded {
            AgentKfdSourceIsaServiceErrorV1::ResponseTooLarge
        } else {
            AgentKfdSourceIsaServiceErrorV1::Json
        }
    })?;
    output.push(b'\n');
    Ok(output)
}

struct BoundedResponseWriterV1<'a> {
    output: &'a mut Vec<u8>,
    max: u64,
    exceeded: bool,
}

impl Write for BoundedResponseWriterV1<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let byte_len = u64::try_from(bytes.len())
            .map_err(|_| io::Error::other("response write length is not representable"))?;
        let current = self.output.len() as u64;
        if current
            .checked_add(byte_len)
            .is_none_or(|total| total > self.max)
        {
            self.exceeded = true;
            return Err(io::Error::other("response exceeded its byte bound"));
        }
        self.output.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn decode_hex(value: &str, max_bytes: u64) -> Result<Vec<u8>, AgentKfdSourceIsaErrorCodeV1> {
    if value.is_empty() || !value.len().is_multiple_of(2) || value.len() as u64 > max_bytes * 2 {
        return Err(AgentKfdSourceIsaErrorCodeV1::EvidenceEncoding);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(value.len() / 2)
        .map_err(|_| AgentKfdSourceIsaErrorCodeV1::EvidenceEncoding)?;
    for pair in value.as_bytes().chunks_exact(2) {
        let high = hex_nibble(pair[0]).ok_or(AgentKfdSourceIsaErrorCodeV1::EvidenceEncoding)?;
        let low = hex_nibble(pair[1]).ok_or(AgentKfdSourceIsaErrorCodeV1::EvidenceEncoding)?;
        output.push((high << 4) | low);
    }
    Ok(output)
}

fn parse_identity(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let bytes = decode_hex(value, 32).ok()?;
    let identity: [u8; 32] = bytes.try_into().ok()?;
    (identity != [0; 32]).then_some(identity)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn input_identity(bytes: &[u8]) -> KfdSourceIsaContentIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(INPUT_IDENTITY_DOMAIN_V1);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    KfdSourceIsaContentIdentityV1 {
        scheme: "domain_separated_sha256",
        digest: hex(&hasher.finalize()),
        byte_len: bytes.len() as u64,
    }
}

fn profile_identity(identity: ProfileIdentityV1) -> String {
    hex(&identity.as_bytes())
}

fn profile_content(content: ProfileContentIdentityV1) -> KfdSourceIsaContentIdentityV1 {
    KfdSourceIsaContentIdentityV1 {
        scheme: "fe2o3_profile_content_identity_v1",
        digest: profile_identity(content.digest),
        byte_len: content.byte_len,
    }
}

fn source_content(
    content: fe2o3_source_isa_observation::wire_v1::SourceIsaObservationContentIdentityV1,
) -> KfdSourceIsaContentIdentityV1 {
    KfdSourceIsaContentIdentityV1 {
        scheme: "sha256",
        digest: hex(&content.sha256()),
        byte_len: content.byte_len(),
    }
}

fn source_frame(
    frame: &SourceIsaObservationFrameV1,
    admitted: &AdmittedSourceIsaObservationV1,
) -> KfdSourceIsaFrameV1 {
    let source_artifact = admitted.artifact();
    let structural = admitted.structural();
    let counts = admitted.counts().records();
    KfdSourceIsaFrameV1 {
        frame_identity: hex(&frame.identity()),
        compilation_unit_identity: hex(&frame.context().unit()),
        correlation_identity: hex(&admitted.correlation()),
        artifact_sha256: hex(&source_artifact.sha256()),
        artifact_byte_len: source_artifact.byte_len(),
        structural_map_identity: hex(&structural.identity()),
        target_profile: source_target(structural.target_profile()),
        kir_version: source_kir_version(structural.kir_version()),
        neutral_kir: source_content(structural.neutral_kir()),
        target_kir: source_content(structural.target_kir()),
        source_anchored_records: counts.source_anchored,
        eliminated_records: counts.eliminated,
        records_without_source: counts.no_source,
        isa_references: counts.isa_references,
    }
}

fn source_target(target: SourceIsaObservationTargetProfileV1) -> &'static str {
    match target {
        SourceIsaObservationTargetProfileV1::Gfx942 => "gfx942:xnack-",
        SourceIsaObservationTargetProfileV1::Gfx950 => "gfx950:xnack-",
    }
}

fn source_kir_version(version: SourceIsaObservationKirVersionV1) -> u16 {
    match version {
        SourceIsaObservationKirVersionV1::V8 => 8,
        SourceIsaObservationKirVersionV1::V9 => 9,
    }
}

fn target_matches(
    runtime: &str,
    runtime_wave_width: u16,
    source: SourceIsaObservationTargetProfileV1,
) -> bool {
    runtime_wave_width == 64 && runtime == source_target(source)
}

fn cursor_for(binding: [u8; 32], next_ordinal: usize) -> KfdSourceIsaCursorV1 {
    let next_ordinal = next_ordinal as u64;
    let mut hasher = Sha256::new();
    hasher.update(CURSOR_IDENTITY_DOMAIN_V1);
    hasher.update(binding);
    hasher.update(next_ordinal.to_le_bytes());
    KfdSourceIsaCursorV1 {
        binding_identity: hex(&binding),
        next_ordinal,
        cursor_identity: hex(&hasher.finalize()),
    }
}

fn validate_cursor(binding: [u8; 32], cursor: &KfdSourceIsaCursorV1) -> Option<usize> {
    let expected = cursor_for(binding, usize::try_from(cursor.next_ordinal).ok()?);
    (cursor == &expected).then_some(cursor.next_ordinal as usize)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentKfdSourceIsaServiceErrorV1 {
    Io,
    Json,
    ResponseTooLarge,
}

impl fmt::Display for AgentKfdSourceIsaServiceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "KFD/source-ISA query service failed: {self:?}")
    }
}

impl Error for AgentKfdSourceIsaServiceErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_profiler_protocol::{
        KfdProfileDeviceV1, KfdProfileHostContentModeV1, KfdProfileResourceKindV1,
        KfdRuntimeProfileEventKindV1, KfdRuntimeProfileV1, push_observed_event_v1,
        resource_identity_v1,
    };
    use fe2o3_source_isa_observation::wire_v1::{
        SourceIsaObservationAttemptV1, SourceIsaObservationContentIdentityV1,
        SourceIsaObservationContextV1, SourceIsaObservationCountsV1,
        SourceIsaObservationInvocationV1, SourceIsaObservationQueryCountsV1,
        SourceIsaObservationRecordCountsV1, SourceIsaObservationSessionV1,
        SourceIsaObservationStructuralBindingV1, SourceIsaObservationStructuralCountsV1,
    };

    fn fixture(
        artifact_byte: u8,
        source_target: SourceIsaObservationTargetProfileV1,
    ) -> (Vec<u8>, Vec<u8>, String) {
        fixture_with_runtime(artifact_byte, source_target, "gfx942:xnack-", 64)
    }

    fn fixture_with_runtime(
        artifact_byte: u8,
        source_target: SourceIsaObservationTargetProfileV1,
        runtime_target: &str,
        wave_width: u16,
    ) -> (Vec<u8>, Vec<u8>, String) {
        let scope = ProfileIdentityV1::new([1; 32]).unwrap();
        let queue = resource_identity_v1(scope, KfdProfileResourceKindV1::NativeQueue, 1).unwrap();
        let stream = resource_identity_v1(scope, KfdProfileResourceKindV1::Stream, 2).unwrap();
        let module = resource_identity_v1(scope, KfdProfileResourceKindV1::Module, 3).unwrap();
        let kernel = resource_identity_v1(scope, KfdProfileResourceKindV1::Kernel, 4).unwrap();
        let dispatch = resource_identity_v1(scope, KfdProfileResourceKindV1::Dispatch, 5).unwrap();
        let artifact = SourceIsaObservationContentIdentityV1::new([artifact_byte; 32], 64).unwrap();
        let profile_artifact =
            ProfileContentIdentityV1::observed_sha256(artifact.byte_len(), artifact.sha256())
                .unwrap();
        let name = ProfileContentIdentityV1::observed(b"kernel").unwrap();
        let signature = ProfileContentIdentityV1::observed(b"signature").unwrap();
        let mut events = Vec::new();
        for event in [
            KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue },
            KfdRuntimeProfileEventKindV1::StreamCreated { stream },
            KfdRuntimeProfileEventKindV1::ModuleLoaded {
                module,
                artifact: profile_artifact,
            },
            KfdRuntimeProfileEventKindV1::KernelResolved {
                kernel,
                module,
                name,
                signature,
            },
            KfdRuntimeProfileEventKindV1::DispatchPublished {
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
                bindings: Vec::new(),
            },
            KfdRuntimeProfileEventKindV1::DispatchCompleted {
                dispatch,
                host_timing: Default::default(),
            },
            KfdRuntimeProfileEventKindV1::SubmissionReleased { dispatch },
            KfdRuntimeProfileEventKindV1::ModuleUnloaded { module },
            KfdRuntimeProfileEventKindV1::StreamDestroyed { stream },
            KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue },
        ] {
            push_observed_event_v1(scope, &mut events, event).unwrap();
        }
        let capture = KfdRuntimeProfileV1::new(
            scope,
            KfdProfileDeviceV1::observed(7, runtime_target, wave_width).unwrap(),
            KfdProfileHostContentModeV1::RangeOnly,
            events,
            0,
        )
        .unwrap();
        let capture = fe2o3_profiler_protocol::encode_kfd_runtime_profile_v1(&capture).unwrap();

        let structural = SourceIsaObservationStructuralBindingV1::new(
            [9; 32],
            source_target,
            SourceIsaObservationKirVersionV1::V9,
            SourceIsaObservationContentIdentityV1::new([10; 32], 40).unwrap(),
            SourceIsaObservationContentIdentityV1::new([11; 32], 44).unwrap(),
            SourceIsaObservationStructuralCountsV1 {
                functions: 1,
                defined_bodies: 1,
                blocks: 1,
                operations: 1,
            },
        )
        .unwrap();
        let counts = SourceIsaObservationCountsV1::new(
            SourceIsaObservationRecordCountsV1 {
                records: 1,
                source_anchored: 1,
                eliminated: 0,
                no_source: 0,
                source_anchored_without_isa: 1,
                isa_references: 0,
            },
            SourceIsaObservationQueryCountsV1 {
                distinct_source_nodes: 1,
                distinct_source_spans: 1,
                distinct_isa_points: 0,
                max_source_node_cardinality: 1,
                max_source_span_cardinality: 1,
                max_exact_pc_cardinality: 0,
            },
        )
        .unwrap();
        let admitted =
            AdmittedSourceIsaObservationV1::new([12; 32], artifact, structural, counts, None)
                .unwrap();
        let session = SourceIsaObservationSessionV1::from_bytes([13; 16]);
        let context = SourceIsaObservationContextV1::new(
            [14; 32],
            [15; 32],
            SourceIsaObservationAttemptV1::new(
                1,
                session,
                SourceIsaObservationInvocationV1::from_bytes([16; 32]),
            )
            .unwrap(),
            [17; 32],
        )
        .unwrap();
        let frame = SourceIsaObservationFrameV1::new(
            context,
            SourceIsaObservationOutcomeV1::Admitted(admitted),
        );
        let collection = SourceIsaObservationCollectionV1::from_collected(
            [14; 32],
            session,
            vec![([15; 32], frame)],
            Vec::new(),
            None,
        )
        .encode_canonical()
        .unwrap();
        (capture, collection, profile_identity(dispatch))
    }

    fn near_limit_fixture() -> (Vec<u8>, Vec<u8>) {
        const DISPATCHES: usize = 4_096;
        const SOURCE_UNITS: usize = 1_024;

        let scope = ProfileIdentityV1::new([31; 32]).unwrap();
        let queue = resource_identity_v1(scope, KfdProfileResourceKindV1::NativeQueue, 1).unwrap();
        let stream = resource_identity_v1(scope, KfdProfileResourceKindV1::Stream, 2).unwrap();
        let module = resource_identity_v1(scope, KfdProfileResourceKindV1::Module, 3).unwrap();
        let kernel = resource_identity_v1(scope, KfdProfileResourceKindV1::Kernel, 4).unwrap();
        let artifact = SourceIsaObservationContentIdentityV1::new([32; 32], 65_536).unwrap();
        let profile_artifact =
            ProfileContentIdentityV1::observed_sha256(artifact.byte_len(), artifact.sha256())
                .unwrap();
        let name = ProfileContentIdentityV1::observed(b"near-limit-kernel").unwrap();
        let signature = ProfileContentIdentityV1::observed(b"near-limit-signature").unwrap();
        let shape = ProfileContentIdentityV1::observed(b"near-limit-shape").unwrap();
        let mut events = Vec::new();
        for event in [
            KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue },
            KfdRuntimeProfileEventKindV1::StreamCreated { stream },
            KfdRuntimeProfileEventKindV1::ModuleLoaded {
                module,
                artifact: profile_artifact,
            },
            KfdRuntimeProfileEventKindV1::KernelResolved {
                kernel,
                module,
                name,
                signature,
            },
        ] {
            push_observed_event_v1(scope, &mut events, event).unwrap();
        }
        for ordinal in 0..DISPATCHES {
            let dispatch = resource_identity_v1(
                scope,
                KfdProfileResourceKindV1::Dispatch,
                10_000 + ordinal as u64,
            )
            .unwrap();
            for event in [
                KfdRuntimeProfileEventKindV1::DispatchPublished {
                    dispatch,
                    queue,
                    stream,
                    kernel,
                    dispatch_shape: shape,
                    launch: KfdProfileLaunchV1 {
                        grid: [64, 1, 1],
                        workgroup: [64, 1, 1],
                        dynamic_shared_bytes: 0,
                    },
                    bindings: Vec::new(),
                },
                KfdRuntimeProfileEventKindV1::DispatchCompleted {
                    dispatch,
                    host_timing: Default::default(),
                },
                KfdRuntimeProfileEventKindV1::SubmissionReleased { dispatch },
            ] {
                push_observed_event_v1(scope, &mut events, event).unwrap();
            }
        }
        for event in [
            KfdRuntimeProfileEventKindV1::ModuleUnloaded { module },
            KfdRuntimeProfileEventKindV1::StreamDestroyed { stream },
            KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue },
        ] {
            push_observed_event_v1(scope, &mut events, event).unwrap();
        }
        let capture = KfdRuntimeProfileV1::new(
            scope,
            KfdProfileDeviceV1::observed(33, "gfx942:xnack-", 64).unwrap(),
            KfdProfileHostContentModeV1::RangeOnly,
            events,
            0,
        )
        .unwrap();
        let capture = fe2o3_profiler_protocol::encode_kfd_runtime_profile_v1(&capture).unwrap();

        let structural = SourceIsaObservationStructuralBindingV1::new(
            [34; 32],
            SourceIsaObservationTargetProfileV1::Gfx942,
            SourceIsaObservationKirVersionV1::V9,
            SourceIsaObservationContentIdentityV1::new([35; 32], 40).unwrap(),
            SourceIsaObservationContentIdentityV1::new([36; 32], 44).unwrap(),
            SourceIsaObservationStructuralCountsV1 {
                functions: 1,
                defined_bodies: 1,
                blocks: 1,
                operations: 1,
            },
        )
        .unwrap();
        let counts = SourceIsaObservationCountsV1::new(
            SourceIsaObservationRecordCountsV1 {
                records: 1,
                source_anchored: 1,
                eliminated: 0,
                no_source: 0,
                source_anchored_without_isa: 1,
                isa_references: 0,
            },
            SourceIsaObservationQueryCountsV1 {
                distinct_source_nodes: 1,
                distinct_source_spans: 1,
                distinct_isa_points: 0,
                max_source_node_cardinality: 1,
                max_source_span_cardinality: 1,
                max_exact_pc_cardinality: 0,
            },
        )
        .unwrap();
        let config = [37; 32];
        let session = SourceIsaObservationSessionV1::from_bytes([38; 16]);
        let attempt = SourceIsaObservationAttemptV1::new(
            1,
            session,
            SourceIsaObservationInvocationV1::from_bytes([39; 32]),
        )
        .unwrap();
        let mut frames = Vec::new();
        for ordinal in 0..SOURCE_UNITS {
            let mut unit = [0; 32];
            unit[24..].copy_from_slice(&(ordinal as u64 + 1).to_be_bytes());
            let context =
                SourceIsaObservationContextV1::new(config, unit, attempt, [40; 32]).unwrap();
            let admitted =
                AdmittedSourceIsaObservationV1::new([41; 32], artifact, structural, counts, None)
                    .unwrap();
            frames.push((
                unit,
                SourceIsaObservationFrameV1::new(
                    context,
                    SourceIsaObservationOutcomeV1::Admitted(admitted),
                ),
            ));
        }
        let collection = SourceIsaObservationCollectionV1::from_collected(
            config,
            session,
            frames,
            Vec::new(),
            None,
        )
        .encode_canonical()
        .unwrap();
        (capture, collection)
    }

    #[test]
    fn exact_artifact_join_preserves_site_unavailability() {
        let (capture, collection, dispatch) =
            fixture(7, SourceIsaObservationTargetProfileV1::Gfx942);
        let evidence = AdmittedKfdSourceIsaV1::new(&capture, &collection).unwrap();
        let item = evidence.dispatch(evidence.dispatches.last().unwrap());
        assert_eq!(item.summary.dispatch_identity, dispatch);
        assert_eq!(
            item.summary.artifact_relation,
            KfdSourceIsaArtifactRelationV1::Exact
        );
        assert_eq!(item.summary.matching_compilation_unit_count, 1);
        assert_eq!(
            item.semantic_binding.site_attribution,
            KfdSourceIsaArtifactRelationV1::Unavailable
        );
        assert_eq!(
            item.semantic_binding.site_unavailable_reason,
            KfdSourceIsaUnavailableReasonV1::SiteRequiresObservedPcOrSemanticEvent
        );
    }

    #[test]
    fn near_limit_admission_and_summary_do_not_expand_dispatch_frame_products() {
        let (capture, collection) = near_limit_fixture();
        let evidence = AdmittedKfdSourceIsaV1::new(&capture, &collection).unwrap();
        let binding = evidence.summary();
        assert_eq!(binding.observed_dispatch_count, 4_096);
        assert_eq!(binding.exact_artifact_dispatch_count, 4_096);
        assert_eq!(evidence.source_artifacts.len(), 1);
        assert_eq!(
            evidence
                .source_artifacts
                .values()
                .next()
                .unwrap()
                .matching_compilation_units
                .len(),
            1_024
        );

        let summaries = evidence
            .dispatches
            .iter()
            .map(|dispatch| evidence.dispatch_summary(dispatch))
            .collect::<Vec<_>>();
        assert_eq!(summaries.len(), 4_096);
        assert!(
            summaries
                .iter()
                .all(|summary| summary.matching_compilation_unit_count == 1_024)
        );
        let page = AgentKfdSourceIsaResponseV1::Ok {
            schema: AGENT_KFD_SOURCE_ISA_RESPONSE_SCHEMA_V1,
            request_id: 1,
            response_revision: 1,
            value: Box::new(AgentKfdSourceIsaResultV1::DispatchPage {
                binding_identity: hex(&evidence.binding_identity),
                items: summaries,
                next_cursor: None,
            }),
        };
        assert!(encode_response(&page).is_ok());

        let inspected = evidence.dispatch(evidence.dispatches.last().unwrap());
        assert_eq!(
            inspected.semantic_binding.matching_compilation_units.len(),
            1_024
        );
    }

    #[test]
    fn artifact_and_target_substitution_fail_closed() {
        let (capture, _, _) = fixture(7, SourceIsaObservationTargetProfileV1::Gfx942);
        let (_, other_artifact, _) = fixture(8, SourceIsaObservationTargetProfileV1::Gfx942);
        let (_, wrong_target, _) = fixture(7, SourceIsaObservationTargetProfileV1::Gfx950);
        let artifact = AdmittedKfdSourceIsaV1::new(&capture, &other_artifact).unwrap();
        let artifact = artifact.dispatch(artifact.dispatches.last().unwrap());
        assert_eq!(
            artifact.summary.unavailable_reason,
            Some(KfdSourceIsaUnavailableReasonV1::NoMatchingArtifact)
        );
        let target = AdmittedKfdSourceIsaV1::new(&capture, &wrong_target).unwrap();
        let target = target.dispatch(target.dispatches.last().unwrap());
        assert_eq!(
            target.summary.unavailable_reason,
            Some(KfdSourceIsaUnavailableReasonV1::TargetProfileMismatch)
        );
    }

    #[test]
    fn target_feature_and_wave_substitution_fail_closed() {
        for (source_target, runtime_target) in [
            (SourceIsaObservationTargetProfileV1::Gfx942, "gfx942:xnack-"),
            (SourceIsaObservationTargetProfileV1::Gfx950, "gfx950:xnack-"),
        ] {
            let (capture, collection, _) =
                fixture_with_runtime(7, source_target, runtime_target, 64);
            let evidence = AdmittedKfdSourceIsaV1::new(&capture, &collection).unwrap();
            let dispatch = evidence.dispatch(evidence.dispatches.last().unwrap());
            assert_eq!(
                dispatch.summary.artifact_relation,
                KfdSourceIsaArtifactRelationV1::Exact
            );
        }

        let (_, collection, _) = fixture(7, SourceIsaObservationTargetProfileV1::Gfx942);
        for (runtime_target, wave_width) in [
            ("gfx942:xnack+", 64),
            ("gfx942:unknown+", 64),
            ("gfx942:", 64),
            ("gfx942:xnack-", 32),
        ] {
            let (capture, _, _) = fixture_with_runtime(
                7,
                SourceIsaObservationTargetProfileV1::Gfx942,
                runtime_target,
                wave_width,
            );
            let evidence = AdmittedKfdSourceIsaV1::new(&capture, &collection).unwrap();
            let dispatch = evidence.dispatch(evidence.dispatches.last().unwrap());
            assert_eq!(
                dispatch.summary.unavailable_reason,
                Some(KfdSourceIsaUnavailableReasonV1::TargetProfileMismatch),
                "runtime target {runtime_target:?} wave width {wave_width} was not rejected"
            );
        }
    }

    #[test]
    fn service_requires_open_and_binds_cursors() {
        let mut service = AgentKfdSourceIsaServiceV1::new();
        let response = service.handle(AgentKfdSourceIsaRequestV1::InspectBinding {
            schema: AGENT_KFD_SOURCE_ISA_REQUEST_SCHEMA_V1.to_owned(),
            request_id: 1,
            expected_revision: 0,
        });
        assert!(matches!(
            response,
            AgentKfdSourceIsaResponseV1::Error {
                code: AgentKfdSourceIsaErrorCodeV1::EvidenceNotOpen,
                ..
            }
        ));

        let (capture, collection, _) = fixture(7, SourceIsaObservationTargetProfileV1::Gfx942);
        let response = service.handle(AgentKfdSourceIsaRequestV1::OpenEvidence {
            schema: AGENT_KFD_SOURCE_ISA_REQUEST_SCHEMA_V1.to_owned(),
            request_id: 2,
            expected_revision: 1,
            capture_hex: hex(&capture),
            source_isa_collection_hex: hex(&collection),
        });
        assert!(matches!(
            response,
            AgentKfdSourceIsaResponseV1::Ok { ref value, .. }
                if matches!(value.as_ref(), AgentKfdSourceIsaResultV1::Opened { .. })
        ));
        let response = service.handle(AgentKfdSourceIsaRequestV1::ListDispatches {
            schema: AGENT_KFD_SOURCE_ISA_REQUEST_SCHEMA_V1.to_owned(),
            request_id: 3,
            expected_revision: 2,
            cursor: Some(KfdSourceIsaCursorV1 {
                binding_identity: "00".repeat(32),
                next_ordinal: 0,
                cursor_identity: "00".repeat(32),
            }),
            limit: 1,
        });
        assert!(matches!(
            response,
            AgentKfdSourceIsaResponseV1::Error {
                code: AgentKfdSourceIsaErrorCodeV1::CursorMismatch,
                ..
            }
        ));
    }

    #[test]
    fn jsonl_rejects_noncanonical_requests_without_desynchronizing() {
        let first = b"{ \"operation\":\"discover_capabilities\",\"schema\":\"fe2o3-agent-kfd-source-isa-request-v1\",\"request_id\":1,\"expected_revision\":0}\n";
        let second = serde_json::to_vec(&AgentKfdSourceIsaRequestV1::DiscoverCapabilities {
            schema: AGENT_KFD_SOURCE_ISA_REQUEST_SCHEMA_V1.to_owned(),
            request_id: 2,
            expected_revision: 1,
        })
        .unwrap();
        let mut input = first.to_vec();
        input.extend_from_slice(&second);
        input.push(b'\n');
        let mut output = Vec::new();
        run_agent_kfd_source_isa_jsonl_v1(&mut std::io::Cursor::new(input), &mut output).unwrap();
        assert_eq!(
            output
                .split(|byte| *byte == b'\n')
                .filter(|row| !row.is_empty())
                .count(),
            2
        );
    }

    #[test]
    fn malformed_records_exhaust_the_same_bounded_request_budget() {
        let input = b"{}\n".repeat(MAX_AGENT_KFD_SOURCE_ISA_REQUESTS_V1 as usize + 1);
        let mut output = Vec::new();
        run_agent_kfd_source_isa_jsonl_v1(&mut std::io::Cursor::new(input), &mut output).unwrap();
        let responses = output
            .split(|byte| *byte == b'\n')
            .filter(|row| !row.is_empty())
            .map(|row| serde_json::from_slice::<serde_json::Value>(row).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            responses.len(),
            MAX_AGENT_KFD_SOURCE_ISA_REQUESTS_V1 as usize + 1
        );
        assert_eq!(responses[0]["code"], "invalid_request");
        assert_eq!(responses[0]["response_revision"], 1);
        assert_eq!(responses[0]["terminal"], false);
        assert_eq!(responses[63]["code"], "invalid_request");
        assert_eq!(responses[63]["response_revision"], 64);
        assert_eq!(responses[63]["terminal"], false);
        assert_eq!(responses[64]["code"], "request_budget_exhausted");
        assert_eq!(responses[64]["response_revision"], 65);
        assert_eq!(responses[64]["terminal"], true);
    }

    #[test]
    fn noncanonical_records_exhaust_the_same_bounded_request_budget() {
        let mut input = Vec::new();
        for request_id in 1..=u64::from(MAX_AGENT_KFD_SOURCE_ISA_REQUESTS_V1) + 1 {
            input.push(b' ');
            input.extend_from_slice(
                &serde_json::to_vec(&AgentKfdSourceIsaRequestV1::DiscoverCapabilities {
                    schema: AGENT_KFD_SOURCE_ISA_REQUEST_SCHEMA_V1.to_owned(),
                    request_id,
                    expected_revision: request_id - 1,
                })
                .unwrap(),
            );
            input.push(b'\n');
        }
        let mut output = Vec::new();
        run_agent_kfd_source_isa_jsonl_v1(&mut std::io::Cursor::new(input), &mut output).unwrap();
        let responses = output
            .split(|byte| *byte == b'\n')
            .filter(|row| !row.is_empty())
            .map(|row| serde_json::from_slice::<serde_json::Value>(row).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            responses.len(),
            MAX_AGENT_KFD_SOURCE_ISA_REQUESTS_V1 as usize + 1
        );
        assert_eq!(responses[63]["code"], "invalid_request");
        assert_eq!(responses[63]["terminal"], false);
        assert_eq!(responses[64]["code"], "request_budget_exhausted");
        assert_eq!(responses[64]["response_revision"], 65);
        assert_eq!(responses[64]["terminal"], true);
    }

    #[test]
    fn oversized_response_is_replaced_by_a_small_typed_terminal_record() {
        let (capture, collection, _) = fixture(7, SourceIsaObservationTargetProfileV1::Gfx942);
        let evidence = AdmittedKfdSourceIsaV1::new(&capture, &collection).unwrap();
        let summary = evidence.dispatch_summary(evidence.dispatches.last().unwrap());
        let response = AgentKfdSourceIsaResponseV1::Ok {
            schema: AGENT_KFD_SOURCE_ISA_RESPONSE_SCHEMA_V1,
            request_id: 1,
            response_revision: 1,
            value: Box::new(AgentKfdSourceIsaResultV1::DispatchPage {
                binding_identity: hex(&evidence.binding_identity),
                items: vec![summary; MAX_AGENT_KFD_SOURCE_ISA_PAGE_ITEMS_V1 as usize * 4],
                next_cursor: None,
            }),
        };
        assert_eq!(
            encode_response(&response),
            Err(AgentKfdSourceIsaServiceErrorV1::ResponseTooLarge)
        );
        let mut service = AgentKfdSourceIsaServiceV1::new();
        let mut output = Vec::new();
        assert!(write_response_with_size_fallback(&mut output, &mut service, &response).unwrap());
        assert!(output.len() < 512);
        let decoded: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(decoded["status"], "error");
        assert_eq!(decoded["code"], "response_too_large");
        assert_eq!(decoded["terminal"], true);
    }
}
