//! Stateful, authority-free JSONL service for Profiler Variant V3 archives.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::io::{BufRead, Write};

use fe2o3_hsaco_finalize::{
    AdmittedProductionProfilerKirArchiveV1, InertProductionProfilerKirArchiveV1,
    MAX_PRODUCTION_PROFILER_KIR_ARCHIVE_BYTES_V1, PRODUCTION_PROFILER_KIR_ARCHIVE_VERSION_V1,
    ProductionProfilerKirArchiveAdmissionV1, ProductionProfilerKirArchiveUnavailableClassV1,
    ProductionProfilerKirArchiveUnavailableV1,
};
use fe2o3_semantic_import::{CaptureIdentityV1, ContentIdentityRecordV1, ContentSchemeV1};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    AgentProfilerVariantErrorCodeV2, AgentProfilerVariantTreatmentHexV2,
    MAX_PROFILER_COMPLETE_STRUCTURAL_RESULT_BYTES_V1, ProfilerCompleteStructuralComparisonV1,
    ProfilerCompleteStructuralTreatmentInputV1, ProfilerVariantComparisonV3,
    ProfilerVariantProductionKirEvidenceV3, ProfilerVariantStructuralContentIdentityV3,
    ProfilerVariantTreatmentInputV3, agent_variant_service_v2::OwnedTreatmentV2,
    build_profiler_complete_structural_request_v1, build_profiler_variant_request_v3,
    compare_profiler_complete_structural_v1, compare_profiler_variants_v3,
};

pub const AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V3: &str =
    "fe2o3-agent-profiler-variant-request-v3";
pub const AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V3: &str =
    "fe2o3-agent-profiler-variant-response-v3";
pub const AGENT_PROFILER_VARIANT_COMPARISON_SCHEMA_V3: &str = "fe2o3-profiler-variant-v3";
pub const MAX_AGENT_PROFILER_VARIANT_REQUESTS_V3: u32 = 64;
pub const MAX_AGENT_PROFILER_VARIANT_ARCHIVES_V3: usize = 2;
pub const MAX_AGENT_PROFILER_VARIANT_REQUEST_BYTES_V3: u64 =
    (MAX_PRODUCTION_PROFILER_KIR_ARCHIVE_BYTES_V1 as u64 * 2) + (64 * 1024);
pub const MAX_AGENT_PROFILER_VARIANT_RESPONSE_BYTES_V3: u64 =
    MAX_PROFILER_COMPLETE_STRUCTURAL_RESULT_BYTES_V1 + (32 * 1024);

const CONTRACT_DOMAIN_V3: &[u8] = b"fe2o3.agent-profiler.variant-service.contract.v3\0";
const CONTRACT_BYTES_V3: &[u8] = b"variant-v3;strict-finalizer-replay-archive;exact-v7-v8-catalog-characteristic-owners;separate-bounded-open;strict-lowercase-hex;positive-structural-co-observation;complete-catalog-same-domain-structural-multiset-delta;typed-unavailable;no-external-provenance;read-only-no-execution-attach-scheduling-collection-decoder-publication-load-launch-dispatch-or-runtime-authority";
const RESPONSE_DOMAIN_V3: &[u8] = b"fe2o3.agent-profiler.variant-service.response.v3\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerVariantOperationV3 {
    DiscoverCapabilities,
    OpenStructuralArchive,
    CompareVariants,
    CompareCompleteStructuralCatalogs,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerVariantAuthorityV3 {
    ReadOnlyNoExecutionAttachSchedulingCollectionDecoderPublicationLoadLaunchDispatchOrRuntimeAuthority,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerVariantCapabilitiesV3 {
    pub service_contract: ContentIdentityRecordV1,
    pub operations: Vec<AgentProfilerVariantOperationV3>,
    pub authority: AgentProfilerVariantAuthorityV3,
    pub exact_input_encoding: &'static str,
    pub archive_admission: &'static str,
    pub comparison_semantics: &'static str,
    pub external_provenance: &'static str,
    pub maximum_requests: u32,
    pub maximum_request_bytes: u64,
    pub maximum_response_bytes: u64,
    pub maximum_open_archives: usize,
    pub maximum_archive_bytes: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AgentProfilerVariantTreatmentHexV3 {
    pub treatment_v2: Box<AgentProfilerVariantTreatmentHexV2>,
    #[serde(default)]
    pub structural_archive: Option<ContentIdentityRecordV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentProfilerVariantRequestV3 {
    DiscoverCapabilities {
        schema: String,
        request_id: u64,
        expected_revision: u64,
    },
    OpenStructuralArchive {
        schema: String,
        request_id: u64,
        expected_revision: u64,
        expected_archive: ContentIdentityRecordV1,
        archive_hex: String,
    },
    CompareVariants {
        schema: String,
        request_id: u64,
        expected_revision: u64,
        baseline: Box<AgentProfilerVariantTreatmentHexV3>,
        candidate: Box<AgentProfilerVariantTreatmentHexV3>,
    },
    CompareCompleteStructuralCatalogs {
        schema: String,
        request_id: u64,
        expected_revision: u64,
        baseline: Box<AgentProfilerVariantTreatmentHexV3>,
        candidate: Box<AgentProfilerVariantTreatmentHexV3>,
    },
}

impl AgentProfilerVariantRequestV3 {
    fn schema(&self) -> &str {
        match self {
            Self::DiscoverCapabilities { schema, .. }
            | Self::OpenStructuralArchive { schema, .. }
            | Self::CompareVariants { schema, .. }
            | Self::CompareCompleteStructuralCatalogs { schema, .. } => schema,
        }
    }

    fn request_id(&self) -> u64 {
        match self {
            Self::DiscoverCapabilities { request_id, .. }
            | Self::OpenStructuralArchive { request_id, .. }
            | Self::CompareVariants { request_id, .. }
            | Self::CompareCompleteStructuralCatalogs { request_id, .. } => *request_id,
        }
    }

    fn expected_revision(&self) -> u64 {
        match self {
            Self::DiscoverCapabilities {
                expected_revision, ..
            }
            | Self::OpenStructuralArchive {
                expected_revision, ..
            }
            | Self::CompareVariants {
                expected_revision, ..
            }
            | Self::CompareCompleteStructuralCatalogs {
                expected_revision, ..
            } => *expected_revision,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerVariantArchiveUnavailableKindV3 {
    SemanticDebugCarrier,
    SourceIsaCatalog,
    StructuralBridge,
    CharacteristicProjection,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerVariantArchiveUnavailableV3 {
    pub archive_identity: ContentIdentityRecordV1,
    pub kind: AgentProfilerVariantArchiveUnavailableKindV3,
    pub reason_code: &'static str,
    pub semantics: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerVariantOpenedArchiveV3 {
    pub archive_identity: ContentIdentityRecordV1,
    pub bridge_identity: CaptureIdentityV1,
    pub catalog_identity: CaptureIdentityV1,
    pub structural_identity: CaptureIdentityV1,
    pub correlation_identity: CaptureIdentityV1,
    pub semantic_map_identity: CaptureIdentityV1,
    pub source_map_v2: ProfilerVariantStructuralContentIdentityV3,
    pub artifact: ProfilerVariantStructuralContentIdentityV3,
    pub external_provenance: &'static str,
    pub authority: AgentProfilerVariantAuthorityV3,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum AgentProfilerVariantResultV3 {
    Capabilities {
        capabilities: AgentProfilerVariantCapabilitiesV3,
    },
    StructuralArchiveOpened {
        archive: AgentProfilerVariantOpenedArchiveV3,
    },
    StructuralArchiveUnavailable {
        unavailable: AgentProfilerVariantArchiveUnavailableV3,
    },
    Comparison {
        comparison_schema: &'static str,
        baseline_archive: Option<ContentIdentityRecordV1>,
        candidate_archive: Option<ContentIdentityRecordV1>,
        comparison: Box<ProfilerVariantComparisonV3>,
    },
    CompleteStructuralComparison {
        comparison_schema: &'static str,
        baseline_archive: Option<ContentIdentityRecordV1>,
        candidate_archive: Option<ContentIdentityRecordV1>,
        comparison: Box<ProfilerCompleteStructuralComparisonV1>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerVariantErrorCodeV3 {
    InvalidRequest,
    InvalidSchema,
    InvalidRequestId,
    DuplicateRequestId,
    StaleRevision,
    RequestBudgetExhausted,
    RequestTooLarge,
    InvalidEvidenceEncoding,
    EvidenceTooLarge,
    ArchiveIdentityMismatch,
    DuplicateStructuralArchive,
    StructuralArchiveBudgetExhausted,
    StructuralArchiveAdmissionFailed,
    UnknownStructuralArchive,
    EvidenceAdmissionFailed,
    ResponseTooLarge,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
#[allow(
    clippy::large_enum_variant,
    reason = "the bounded response stays allocation-free until its fallible JSON encoding"
)]
pub enum AgentProfilerVariantResponseV3 {
    Ok {
        schema: &'static str,
        request_id: u64,
        response_revision: u64,
        value: AgentProfilerVariantResultV3,
        response_identity: ContentIdentityRecordV1,
    },
    Error {
        schema: &'static str,
        request_id: Option<u64>,
        response_revision: u64,
        code: AgentProfilerVariantErrorCodeV3,
        terminal: bool,
        response_identity: ContentIdentityRecordV1,
    },
}

impl AgentProfilerVariantResponseV3 {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Error { terminal: true, .. })
    }
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum ResponsePreimageV3<'a> {
    Ok {
        schema: &'a str,
        request_id: u64,
        response_revision: u64,
        value: &'a AgentProfilerVariantResultV3,
    },
    Error {
        schema: &'a str,
        request_id: Option<u64>,
        response_revision: u64,
        code: AgentProfilerVariantErrorCodeV3,
        terminal: bool,
    },
}

pub struct AgentProfilerVariantServiceV3 {
    service_contract: ContentIdentityRecordV1,
    response_revision: u64,
    remaining_requests: u32,
    seen_request_ids: BTreeSet<u64>,
    opened_archive_ids: BTreeSet<CaptureIdentityV1>,
    archives: BTreeMap<CaptureIdentityV1, AdmittedProductionProfilerKirArchiveV1>,
}

impl AgentProfilerVariantServiceV3 {
    pub fn new() -> Result<Self, AgentProfilerVariantServiceErrorV3> {
        Ok(Self {
            service_contract: content_identity(CONTRACT_DOMAIN_V3, CONTRACT_BYTES_V3, 3)?,
            response_revision: 0,
            remaining_requests: MAX_AGENT_PROFILER_VARIANT_REQUESTS_V3,
            seen_request_ids: BTreeSet::new(),
            opened_archive_ids: BTreeSet::new(),
            archives: BTreeMap::new(),
        })
    }

    pub fn handle(
        &mut self,
        request: AgentProfilerVariantRequestV3,
    ) -> Result<AgentProfilerVariantResponseV3, AgentProfilerVariantServiceErrorV3> {
        if self.remaining_requests == 0 {
            return self.error(
                Some(request.request_id()),
                AgentProfilerVariantErrorCodeV3::RequestBudgetExhausted,
                true,
            );
        }
        self.remaining_requests -= 1;
        let request_id = request.request_id();
        if request_id == 0 {
            return self.error(
                Some(request_id),
                AgentProfilerVariantErrorCodeV3::InvalidRequestId,
                false,
            );
        }
        if !self.seen_request_ids.insert(request_id) {
            return self.error(
                Some(request_id),
                AgentProfilerVariantErrorCodeV3::DuplicateRequestId,
                false,
            );
        }
        if request.schema() != AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V3 {
            return self.error(
                Some(request_id),
                AgentProfilerVariantErrorCodeV3::InvalidSchema,
                false,
            );
        }
        if request.expected_revision() != self.response_revision {
            return self.error(
                Some(request_id),
                AgentProfilerVariantErrorCodeV3::StaleRevision,
                false,
            );
        }
        match request {
            AgentProfilerVariantRequestV3::DiscoverCapabilities { .. } => self.ok(
                request_id,
                AgentProfilerVariantResultV3::Capabilities {
                    capabilities: AgentProfilerVariantCapabilitiesV3 {
                        service_contract: self.service_contract,
                        operations: vec![
                            AgentProfilerVariantOperationV3::DiscoverCapabilities,
                            AgentProfilerVariantOperationV3::OpenStructuralArchive,
                            AgentProfilerVariantOperationV3::CompareVariants,
                            AgentProfilerVariantOperationV3::CompareCompleteStructuralCatalogs,
                        ],
                        authority: AgentProfilerVariantAuthorityV3::ReadOnlyNoExecutionAttachSchedulingCollectionDecoderPublicationLoadLaunchDispatchOrRuntimeAuthority,
                        exact_input_encoding: "canonical_lowercase_hex_of_exact_bytes",
                        archive_admission: "checksum_and_content_identity_then_complete_worker_v3_finalizer_replay_and_exact_catalog_bridge_characteristic_reconstruction",
                        comparison_semantics: "positive_exact_structural_co_observation;added_removed_only_as_exact_structural_multiset_deltas_from_two_complete_admitted_catalogs_in_one_workload_and_stable_source_mir_universe;partial_or_sampled_absence_excluded;no_schedule_execution_or_causality",
                        external_provenance: "not_authenticated_by_this_archive_or_service",
                        maximum_requests: MAX_AGENT_PROFILER_VARIANT_REQUESTS_V3,
                        maximum_request_bytes: MAX_AGENT_PROFILER_VARIANT_REQUEST_BYTES_V3,
                        maximum_response_bytes: MAX_AGENT_PROFILER_VARIANT_RESPONSE_BYTES_V3,
                        maximum_open_archives: MAX_AGENT_PROFILER_VARIANT_ARCHIVES_V3,
                        maximum_archive_bytes: MAX_PRODUCTION_PROFILER_KIR_ARCHIVE_BYTES_V1,
                    },
                },
            ),
            AgentProfilerVariantRequestV3::OpenStructuralArchive {
                expected_archive,
                archive_hex,
                ..
            } => self.open_archive(request_id, expected_archive, &archive_hex),
            AgentProfilerVariantRequestV3::CompareVariants {
                baseline,
                candidate,
                ..
            } => self.compare(request_id, *baseline, *candidate),
            AgentProfilerVariantRequestV3::CompareCompleteStructuralCatalogs {
                baseline,
                candidate,
                ..
            } => self.compare_complete_structural_catalogs(request_id, *baseline, *candidate),
        }
    }

    fn open_archive(
        &mut self,
        request_id: u64,
        expected: ContentIdentityRecordV1,
        encoded: &str,
    ) -> Result<AgentProfilerVariantResponseV3, AgentProfilerVariantServiceErrorV3> {
        let bytes = match decode_archive_hex(encoded) {
            Ok(value) => value,
            Err(code) => return self.error(Some(request_id), code, false),
        };
        let inert = match InertProductionProfilerKirArchiveV1::decode_owned_canonical(bytes) {
            Ok(value) => value,
            Err(_) => {
                return self.error(
                    Some(request_id),
                    AgentProfilerVariantErrorCodeV3::StructuralArchiveAdmissionFailed,
                    false,
                );
            }
        };
        let actual = match archive_identity(inert.identity().as_bytes(), inert.canonical_bytes()) {
            Ok(value) => value,
            Err(_) => {
                return self.error(
                    Some(request_id),
                    AgentProfilerVariantErrorCodeV3::StructuralArchiveAdmissionFailed,
                    false,
                );
            }
        };
        if actual != expected {
            return self.error(
                Some(request_id),
                AgentProfilerVariantErrorCodeV3::ArchiveIdentityMismatch,
                false,
            );
        }
        if self.opened_archive_ids.contains(&actual.digest) {
            return self.error(
                Some(request_id),
                AgentProfilerVariantErrorCodeV3::DuplicateStructuralArchive,
                false,
            );
        }
        if self.archives.len() >= MAX_AGENT_PROFILER_VARIANT_ARCHIVES_V3 {
            return self.error(
                Some(request_id),
                AgentProfilerVariantErrorCodeV3::StructuralArchiveBudgetExhausted,
                false,
            );
        }
        let admitted = match inert.admit_exact_replay_v1() {
            Ok(value) => value,
            Err(_) => {
                return self.error(
                    Some(request_id),
                    AgentProfilerVariantErrorCodeV3::StructuralArchiveAdmissionFailed,
                    false,
                );
            }
        };
        self.opened_archive_ids.insert(actual.digest);
        match admitted {
            ProductionProfilerKirArchiveAdmissionV1::Unavailable(reason) => self.ok(
                request_id,
                AgentProfilerVariantResultV3::StructuralArchiveUnavailable {
                    unavailable: archive_unavailable(actual, reason),
                },
            ),
            ProductionProfilerKirArchiveAdmissionV1::Admitted(archive) => {
                let summary = opened_archive(actual, &archive)?;
                self.archives.insert(actual.digest, archive);
                self.ok(
                    request_id,
                    AgentProfilerVariantResultV3::StructuralArchiveOpened { archive: summary },
                )
            }
        }
    }

    fn compare(
        &mut self,
        request_id: u64,
        baseline: AgentProfilerVariantTreatmentHexV3,
        candidate: AgentProfilerVariantTreatmentHexV3,
    ) -> Result<AgentProfilerVariantResponseV3, AgentProfilerVariantServiceErrorV3> {
        let baseline_archive = baseline.structural_archive;
        let candidate_archive = candidate.structural_archive;
        let baseline_treatment = match OwnedTreatmentV2::decode(*baseline.treatment_v2) {
            Ok(value) => value,
            Err(code) => return self.error(Some(request_id), map_v2_error(code), false),
        };
        let candidate_treatment = match OwnedTreatmentV2::decode(*candidate.treatment_v2) {
            Ok(value) => value,
            Err(code) => return self.error(Some(request_id), map_v2_error(code), false),
        };
        let comparison = {
            let baseline_owner = match find_archive(&self.archives, baseline_archive) {
                Ok(value) => value,
                Err(code) => return self.error(Some(request_id), code, false),
            };
            let candidate_owner = match find_archive(&self.archives, candidate_archive) {
                Ok(value) => value,
                Err(code) => return self.error(Some(request_id), code, false),
            };
            let baseline_input = ProfilerVariantTreatmentInputV3 {
                treatment: baseline_treatment.input(),
                production_kir: baseline_owner.map(production_evidence),
            };
            let candidate_input = ProfilerVariantTreatmentInputV3 {
                treatment: candidate_treatment.input(),
                production_kir: candidate_owner.map(production_evidence),
            };
            let comparison_request = match build_profiler_variant_request_v3(
                baseline_treatment.input().treatment.semantic_workload,
                baseline_input,
                candidate_input,
            ) {
                Ok(value) => value,
                Err(_) => {
                    return self.error(
                        Some(request_id),
                        AgentProfilerVariantErrorCodeV3::EvidenceAdmissionFailed,
                        false,
                    );
                }
            };
            match compare_profiler_variants_v3(comparison_request, baseline_input, candidate_input)
            {
                Ok(value) => value,
                Err(_) => {
                    return self.error(
                        Some(request_id),
                        AgentProfilerVariantErrorCodeV3::EvidenceAdmissionFailed,
                        false,
                    );
                }
            }
        };
        self.ok(
            request_id,
            AgentProfilerVariantResultV3::Comparison {
                comparison_schema: AGENT_PROFILER_VARIANT_COMPARISON_SCHEMA_V3,
                baseline_archive,
                candidate_archive,
                comparison: Box::new(comparison),
            },
        )
    }

    fn compare_complete_structural_catalogs(
        &mut self,
        request_id: u64,
        baseline: AgentProfilerVariantTreatmentHexV3,
        candidate: AgentProfilerVariantTreatmentHexV3,
    ) -> Result<AgentProfilerVariantResponseV3, AgentProfilerVariantServiceErrorV3> {
        let baseline_archive = baseline.structural_archive;
        let candidate_archive = candidate.structural_archive;
        let baseline_treatment = match OwnedTreatmentV2::decode(*baseline.treatment_v2) {
            Ok(value) => value,
            Err(code) => return self.error(Some(request_id), map_v2_error(code), false),
        };
        let candidate_treatment = match OwnedTreatmentV2::decode(*candidate.treatment_v2) {
            Ok(value) => value,
            Err(code) => return self.error(Some(request_id), map_v2_error(code), false),
        };
        let comparison = {
            let baseline_owner = match find_archive(&self.archives, baseline_archive) {
                Ok(value) => value,
                Err(code) => return self.error(Some(request_id), code, false),
            };
            let candidate_owner = match find_archive(&self.archives, candidate_archive) {
                Ok(value) => value,
                Err(code) => return self.error(Some(request_id), code, false),
            };
            let baseline_input = ProfilerCompleteStructuralTreatmentInputV1 {
                treatment: baseline_treatment.input(),
                archive: baseline_owner,
            };
            let candidate_input = ProfilerCompleteStructuralTreatmentInputV1 {
                treatment: candidate_treatment.input(),
                archive: candidate_owner,
            };
            let comparison_request = match build_profiler_complete_structural_request_v1(
                baseline_treatment.input().treatment.semantic_workload,
                baseline_input,
                candidate_input,
            ) {
                Ok(value) => value,
                Err(_) => {
                    return self.error(
                        Some(request_id),
                        AgentProfilerVariantErrorCodeV3::EvidenceAdmissionFailed,
                        false,
                    );
                }
            };
            match compare_profiler_complete_structural_v1(
                comparison_request,
                baseline_input,
                candidate_input,
            ) {
                Ok(value) => value,
                Err(_) => {
                    return self.error(
                        Some(request_id),
                        AgentProfilerVariantErrorCodeV3::EvidenceAdmissionFailed,
                        false,
                    );
                }
            }
        };
        self.ok(
            request_id,
            AgentProfilerVariantResultV3::CompleteStructuralComparison {
                comparison_schema: crate::PROFILER_COMPLETE_STRUCTURAL_SCHEMA_V1,
                baseline_archive,
                candidate_archive,
                comparison: Box::new(comparison),
            },
        )
    }

    pub fn encode_response(
        &self,
        response: &AgentProfilerVariantResponseV3,
    ) -> Result<Vec<u8>, AgentProfilerVariantServiceErrorV3> {
        validate_response_identity(response)?;
        let mut bytes = serde_json::to_vec(response)
            .map_err(|_| AgentProfilerVariantServiceErrorV3::JsonEncode)?;
        if bytes.len() as u64 + 1 > MAX_AGENT_PROFILER_VARIANT_RESPONSE_BYTES_V3 {
            return Err(AgentProfilerVariantServiceErrorV3::ResponseTooLarge);
        }
        bytes.push(b'\n');
        Ok(bytes)
    }

    fn ok(
        &mut self,
        request_id: u64,
        value: AgentProfilerVariantResultV3,
    ) -> Result<AgentProfilerVariantResponseV3, AgentProfilerVariantServiceErrorV3> {
        self.response_revision = self
            .response_revision
            .checked_add(1)
            .ok_or(AgentProfilerVariantServiceErrorV3::RevisionOverflow)?;
        let preimage = ResponsePreimageV3::Ok {
            schema: AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V3,
            request_id,
            response_revision: self.response_revision,
            value: &value,
        };
        let response_identity = response_identity(&preimage)?;
        Ok(AgentProfilerVariantResponseV3::Ok {
            schema: AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V3,
            request_id,
            response_revision: self.response_revision,
            value,
            response_identity,
        })
    }

    fn error(
        &mut self,
        request_id: Option<u64>,
        code: AgentProfilerVariantErrorCodeV3,
        terminal: bool,
    ) -> Result<AgentProfilerVariantResponseV3, AgentProfilerVariantServiceErrorV3> {
        self.response_revision = self
            .response_revision
            .checked_add(1)
            .ok_or(AgentProfilerVariantServiceErrorV3::RevisionOverflow)?;
        let preimage = ResponsePreimageV3::Error {
            schema: AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V3,
            request_id,
            response_revision: self.response_revision,
            code,
            terminal,
        };
        Ok(AgentProfilerVariantResponseV3::Error {
            schema: AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V3,
            request_id,
            response_revision: self.response_revision,
            code,
            terminal,
            response_identity: response_identity(&preimage)?,
        })
    }

    fn terminal_protocol_error(
        &mut self,
        code: AgentProfilerVariantErrorCodeV3,
    ) -> Result<AgentProfilerVariantResponseV3, AgentProfilerVariantServiceErrorV3> {
        self.error(None, code, true)
    }
}

fn production_evidence(
    archive: &AdmittedProductionProfilerKirArchiveV1,
) -> ProfilerVariantProductionKirEvidenceV3<'_> {
    ProfilerVariantProductionKirEvidenceV3 {
        bridge: archive.bridge(),
        catalog: archive.catalog(),
        characteristic: archive.characteristic(),
    }
}

fn find_archive(
    archives: &BTreeMap<CaptureIdentityV1, AdmittedProductionProfilerKirArchiveV1>,
    identity: Option<ContentIdentityRecordV1>,
) -> Result<Option<&AdmittedProductionProfilerKirArchiveV1>, AgentProfilerVariantErrorCodeV3> {
    let Some(identity) = identity else {
        return Ok(None);
    };
    let archive = archives
        .get(&identity.digest)
        .ok_or(AgentProfilerVariantErrorCodeV3::UnknownStructuralArchive)?;
    if archive_content_identity(archive)? != identity {
        return Err(AgentProfilerVariantErrorCodeV3::UnknownStructuralArchive);
    }
    Ok(Some(archive))
}

fn archive_content_identity(
    archive: &AdmittedProductionProfilerKirArchiveV1,
) -> Result<ContentIdentityRecordV1, AgentProfilerVariantErrorCodeV3> {
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: PRODUCTION_PROFILER_KIR_ARCHIVE_VERSION_V1,
        digest: CaptureIdentityV1::new(*archive.identity().as_bytes())
            .map_err(|_| AgentProfilerVariantErrorCodeV3::StructuralArchiveAdmissionFailed)?,
        canonical_len: archive.canonical_len(),
    })
}

fn archive_identity(
    digest: &[u8; 32],
    bytes: &[u8],
) -> Result<ContentIdentityRecordV1, AgentProfilerVariantServiceErrorV3> {
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: PRODUCTION_PROFILER_KIR_ARCHIVE_VERSION_V1,
        digest: CaptureIdentityV1::new(*digest)
            .map_err(|_| AgentProfilerVariantServiceErrorV3::Identity)?,
        canonical_len: bytes.len() as u64,
    })
}

fn opened_archive(
    identity: ContentIdentityRecordV1,
    archive: &AdmittedProductionProfilerKirArchiveV1,
) -> Result<AgentProfilerVariantOpenedArchiveV3, AgentProfilerVariantServiceErrorV3> {
    let bridge = archive.bridge();
    Ok(AgentProfilerVariantOpenedArchiveV3 {
        archive_identity: identity,
        bridge_identity: capture_identity(bridge.identity())?,
        catalog_identity: capture_identity(archive.catalog().identity())?,
        structural_identity: capture_identity(bridge.structural_identity())?,
        correlation_identity: capture_identity(bridge.correlation_identity())?,
        semantic_map_identity: capture_identity(bridge.semantic_map_identity())?,
        source_map_v2: ProfilerVariantStructuralContentIdentityV3 {
            digest: capture_identity(&bridge.source_map_v2_identity().sha256())?,
            canonical_len: bridge.source_map_v2_identity().byte_len(),
        },
        artifact: ProfilerVariantStructuralContentIdentityV3 {
            digest: capture_identity(&bridge.artifact_identity().sha256())?,
            canonical_len: bridge.artifact_identity().byte_len(),
        },
        external_provenance: "not_authenticated_by_this_archive_or_service",
        authority: AgentProfilerVariantAuthorityV3::ReadOnlyNoExecutionAttachSchedulingCollectionDecoderPublicationLoadLaunchDispatchOrRuntimeAuthority,
    })
}

fn archive_unavailable(
    archive_identity: ContentIdentityRecordV1,
    reason: ProductionProfilerKirArchiveUnavailableV1,
) -> AgentProfilerVariantArchiveUnavailableV3 {
    let kind = match reason.class() {
        ProductionProfilerKirArchiveUnavailableClassV1::SemanticDebugCarrier => {
            AgentProfilerVariantArchiveUnavailableKindV3::SemanticDebugCarrier
        }
        ProductionProfilerKirArchiveUnavailableClassV1::SourceIsaCatalog => {
            AgentProfilerVariantArchiveUnavailableKindV3::SourceIsaCatalog
        }
        ProductionProfilerKirArchiveUnavailableClassV1::StructuralBridge => {
            AgentProfilerVariantArchiveUnavailableKindV3::StructuralBridge
        }
        ProductionProfilerKirArchiveUnavailableClassV1::CharacteristicProjection => {
            AgentProfilerVariantArchiveUnavailableKindV3::CharacteristicProjection
        }
    };
    AgentProfilerVariantArchiveUnavailableV3 {
        archive_identity,
        kind,
        reason_code: reason.reason_code(),
        semantics: "exact finalizer replay succeeded, but the production structural projection is typed unavailable; no query owner was retained",
    }
}

fn map_v2_error(code: AgentProfilerVariantErrorCodeV2) -> AgentProfilerVariantErrorCodeV3 {
    match code {
        AgentProfilerVariantErrorCodeV2::InvalidEvidenceEncoding => {
            AgentProfilerVariantErrorCodeV3::InvalidEvidenceEncoding
        }
        AgentProfilerVariantErrorCodeV2::EvidenceTooLarge => {
            AgentProfilerVariantErrorCodeV3::EvidenceTooLarge
        }
        _ => AgentProfilerVariantErrorCodeV3::EvidenceAdmissionFailed,
    }
}

fn decode_archive_hex(value: &str) -> Result<Vec<u8>, AgentProfilerVariantErrorCodeV3> {
    if value.is_empty()
        || !value.len().is_multiple_of(2)
        || value.len() as u64 > MAX_PRODUCTION_PROFILER_KIR_ARCHIVE_BYTES_V1 as u64 * 2
    {
        return Err(AgentProfilerVariantErrorCodeV3::InvalidEvidenceEncoding);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(value.len() / 2)
        .map_err(|_| AgentProfilerVariantErrorCodeV3::EvidenceTooLarge)?;
    for pair in value.as_bytes().chunks_exact(2) {
        let high =
            hex_nibble(pair[0]).ok_or(AgentProfilerVariantErrorCodeV3::InvalidEvidenceEncoding)?;
        let low =
            hex_nibble(pair[1]).ok_or(AgentProfilerVariantErrorCodeV3::InvalidEvidenceEncoding)?;
        output.push((high << 4) | low);
    }
    Ok(output)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

pub fn decode_agent_profiler_variant_request_line_v3(
    line: &[u8],
) -> Result<AgentProfilerVariantRequestV3, AgentProfilerVariantServiceErrorV3> {
    if line.is_empty() || line.len() as u64 > MAX_AGENT_PROFILER_VARIANT_REQUEST_BYTES_V3 {
        return Err(AgentProfilerVariantServiceErrorV3::RequestTooLarge);
    }
    let payload = line.strip_suffix(b"\n").unwrap_or(line);
    if payload.is_empty() || payload.contains(&b'\n') || payload.contains(&b'\r') {
        return Err(AgentProfilerVariantServiceErrorV3::InvalidRequest);
    }
    serde_json::from_slice(payload).map_err(|_| AgentProfilerVariantServiceErrorV3::InvalidRequest)
}

pub fn run_agent_profiler_variant_jsonl_v3<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
) -> Result<(), AgentProfilerVariantServiceErrorV3> {
    let mut service = AgentProfilerVariantServiceV3::new()?;
    loop {
        let line = match read_line(input) {
            Ok(Some(line)) => line,
            Ok(None) => return Ok(()),
            Err(AgentProfilerVariantServiceErrorV3::RequestTooLarge) => {
                write_terminal(
                    output,
                    &mut service,
                    AgentProfilerVariantErrorCodeV3::RequestTooLarge,
                )?;
                return Err(AgentProfilerVariantServiceErrorV3::ProtocolTerminated);
            }
            Err(_) => {
                write_terminal(
                    output,
                    &mut service,
                    AgentProfilerVariantErrorCodeV3::InvalidRequest,
                )?;
                return Err(AgentProfilerVariantServiceErrorV3::ProtocolTerminated);
            }
        };
        let request = match decode_agent_profiler_variant_request_line_v3(&line) {
            Ok(value) => value,
            Err(_) => {
                write_terminal(
                    output,
                    &mut service,
                    AgentProfilerVariantErrorCodeV3::InvalidRequest,
                )?;
                return Err(AgentProfilerVariantServiceErrorV3::ProtocolTerminated);
            }
        };
        let response = service.handle(request)?;
        let terminal = response.is_terminal();
        output
            .write_all(&service.encode_response(&response)?)
            .map_err(|_| AgentProfilerVariantServiceErrorV3::Io)?;
        output
            .flush()
            .map_err(|_| AgentProfilerVariantServiceErrorV3::Io)?;
        if terminal {
            return Err(AgentProfilerVariantServiceErrorV3::ProtocolTerminated);
        }
    }
}

fn read_line<R: BufRead>(
    reader: &mut R,
) -> Result<Option<Vec<u8>>, AgentProfilerVariantServiceErrorV3> {
    let mut line = Vec::new();
    let maximum = usize::try_from(MAX_AGENT_PROFILER_VARIANT_REQUEST_BYTES_V3)
        .map_err(|_| AgentProfilerVariantServiceErrorV3::SizeOverflow)?;
    loop {
        let buffer = reader
            .fill_buf()
            .map_err(|_| AgentProfilerVariantServiceErrorV3::Io)?;
        if buffer.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err(AgentProfilerVariantServiceErrorV3::InvalidRequest)
            };
        }
        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let available = newline.map_or(buffer.len(), |position| position + 1);
        let consumed = available.min(maximum.saturating_add(1).saturating_sub(line.len()));
        line.extend_from_slice(&buffer[..consumed]);
        reader.consume(consumed);
        if line.len() > maximum {
            return Err(AgentProfilerVariantServiceErrorV3::RequestTooLarge);
        }
        if newline.is_some() && consumed == available {
            return Ok(Some(line));
        }
    }
}

fn write_terminal<W: Write>(
    output: &mut W,
    service: &mut AgentProfilerVariantServiceV3,
    code: AgentProfilerVariantErrorCodeV3,
) -> Result<(), AgentProfilerVariantServiceErrorV3> {
    let response = service.terminal_protocol_error(code)?;
    output
        .write_all(&service.encode_response(&response)?)
        .map_err(|_| AgentProfilerVariantServiceErrorV3::Io)?;
    output
        .flush()
        .map_err(|_| AgentProfilerVariantServiceErrorV3::Io)
}

fn validate_response_identity(
    response: &AgentProfilerVariantResponseV3,
) -> Result<(), AgentProfilerVariantServiceErrorV3> {
    let (preimage, supplied) = match response {
        AgentProfilerVariantResponseV3::Ok {
            schema,
            request_id,
            response_revision,
            value,
            response_identity,
        } => (
            ResponsePreimageV3::Ok {
                schema,
                request_id: *request_id,
                response_revision: *response_revision,
                value,
            },
            response_identity,
        ),
        AgentProfilerVariantResponseV3::Error {
            schema,
            request_id,
            response_revision,
            code,
            terminal,
            response_identity,
        } => (
            ResponsePreimageV3::Error {
                schema,
                request_id: *request_id,
                response_revision: *response_revision,
                code: *code,
                terminal: *terminal,
            },
            response_identity,
        ),
    };
    if &response_identity(&preimage)? != supplied {
        return Err(AgentProfilerVariantServiceErrorV3::InvalidResponse);
    }
    Ok(())
}

pub fn validate_agent_profiler_variant_response_line_v3(
    line: &[u8],
) -> Result<(), AgentProfilerVariantServiceErrorV3> {
    if line.is_empty() || line.len() as u64 > MAX_AGENT_PROFILER_VARIANT_RESPONSE_BYTES_V3 {
        return Err(AgentProfilerVariantServiceErrorV3::InvalidResponse);
    }
    let payload = line.strip_suffix(b"\n").unwrap_or(line);
    if payload.is_empty() || payload.contains(&b'\n') || payload.contains(&b'\r') {
        return Err(AgentProfilerVariantServiceErrorV3::InvalidResponse);
    }
    let mut value: serde_json::Value = serde_json::from_slice(payload)
        .map_err(|_| AgentProfilerVariantServiceErrorV3::InvalidResponse)?;
    let object = value
        .as_object_mut()
        .ok_or(AgentProfilerVariantServiceErrorV3::InvalidResponse)?;
    if object.get("schema").and_then(serde_json::Value::as_str)
        != Some(AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V3)
    {
        return Err(AgentProfilerVariantServiceErrorV3::InvalidResponse);
    }
    let supplied: ContentIdentityRecordV1 = serde_json::from_value(
        object
            .remove("response_identity")
            .ok_or(AgentProfilerVariantServiceErrorV3::InvalidResponse)?,
    )
    .map_err(|_| AgentProfilerVariantServiceErrorV3::InvalidResponse)?;
    let bytes =
        serde_json::to_vec(&value).map_err(|_| AgentProfilerVariantServiceErrorV3::JsonEncode)?;
    if content_identity(RESPONSE_DOMAIN_V3, &bytes, 3)? != supplied {
        return Err(AgentProfilerVariantServiceErrorV3::InvalidResponse);
    }
    Ok(())
}

fn response_identity<T: Serialize>(
    value: &T,
) -> Result<ContentIdentityRecordV1, AgentProfilerVariantServiceErrorV3> {
    let value =
        serde_json::to_value(value).map_err(|_| AgentProfilerVariantServiceErrorV3::JsonEncode)?;
    let bytes =
        serde_json::to_vec(&value).map_err(|_| AgentProfilerVariantServiceErrorV3::JsonEncode)?;
    content_identity(RESPONSE_DOMAIN_V3, &bytes, 3)
}

fn content_identity(
    domain: &[u8],
    bytes: &[u8],
    format_version: u16,
) -> Result<ContentIdentityRecordV1, AgentProfilerVariantServiceErrorV3> {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version,
        digest: CaptureIdentityV1::new(digest.finalize().into())
            .map_err(|_| AgentProfilerVariantServiceErrorV3::Identity)?,
        canonical_len: bytes.len() as u64,
    })
}

fn capture_identity(bytes: &[u8]) -> Result<CaptureIdentityV1, AgentProfilerVariantServiceErrorV3> {
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| AgentProfilerVariantServiceErrorV3::Identity)?;
    CaptureIdentityV1::new(bytes).map_err(|_| AgentProfilerVariantServiceErrorV3::Identity)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentProfilerVariantServiceErrorV3 {
    Identity,
    InvalidRequest,
    RequestTooLarge,
    InvalidResponse,
    ResponseTooLarge,
    JsonEncode,
    RevisionOverflow,
    SizeOverflow,
    Io,
    ProtocolTerminated,
}

impl fmt::Display for AgentProfilerVariantServiceErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "profiler Variant V3 service rejected input: {self:?}"
        )
    }
}

impl Error for AgentProfilerVariantServiceErrorV3 {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_are_authority_free_and_request_ids_are_unique() {
        let mut service = AgentProfilerVariantServiceV3::new().unwrap();
        let request = || AgentProfilerVariantRequestV3::DiscoverCapabilities {
            schema: AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V3.to_owned(),
            request_id: 7,
            expected_revision: 0,
        };
        let response = service.handle(request()).unwrap();
        let AgentProfilerVariantResponseV3::Ok {
            value: AgentProfilerVariantResultV3::Capabilities { capabilities },
            ..
        } = response
        else {
            panic!("capabilities request did not succeed")
        };
        assert_eq!(capabilities.operations.len(), 4);
        assert!(
            capabilities
                .operations
                .contains(&AgentProfilerVariantOperationV3::CompareCompleteStructuralCatalogs)
        );
        assert_eq!(capabilities.maximum_open_archives, 2);
        assert_eq!(
            capabilities.external_provenance,
            "not_authenticated_by_this_archive_or_service"
        );
        assert!(matches!(
            service.handle(request()).unwrap(),
            AgentProfilerVariantResponseV3::Error {
                code: AgentProfilerVariantErrorCodeV3::DuplicateRequestId,
                ..
            }
        ));
    }

    #[test]
    fn hostile_archive_encoding_and_unknown_fields_fail_closed() {
        assert_eq!(
            decode_archive_hex("AA").unwrap_err(),
            AgentProfilerVariantErrorCodeV3::InvalidEvidenceEncoding
        );
        assert!(decode_agent_profiler_variant_request_line_v3(
            br#"{"operation":"discover_capabilities","schema":"fe2o3-agent-profiler-variant-request-v3","request_id":1,"expected_revision":0,"unknown":true}\n"#,
        )
        .is_err());
        let mut service = AgentProfilerVariantServiceV3::new().unwrap();
        let response = service
            .handle(AgentProfilerVariantRequestV3::OpenStructuralArchive {
                schema: AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V3.to_owned(),
                request_id: 1,
                expected_revision: 0,
                expected_archive: ContentIdentityRecordV1 {
                    scheme: ContentSchemeV1::DomainSeparatedSha256,
                    format_version: 1,
                    digest: CaptureIdentityV1::new([1; 32]).unwrap(),
                    canonical_len: 1,
                },
                archive_hex: "00".to_owned(),
            })
            .unwrap();
        assert!(matches!(
            response,
            AgentProfilerVariantResponseV3::Error {
                code: AgentProfilerVariantErrorCodeV3::StructuralArchiveAdmissionFailed,
                ..
            }
        ));
    }

    #[test]
    fn encoded_response_identity_is_independently_validated() {
        let mut service = AgentProfilerVariantServiceV3::new().unwrap();
        let response = service
            .handle(AgentProfilerVariantRequestV3::DiscoverCapabilities {
                schema: AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V3.to_owned(),
                request_id: 1,
                expected_revision: 0,
            })
            .unwrap();
        let encoded = service.encode_response(&response).unwrap();
        validate_agent_profiler_variant_response_line_v3(&encoded).unwrap();

        let mut value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        value["response_revision"] = serde_json::Value::from(2_u64);
        let substituted = serde_json::to_vec(&value).unwrap();
        assert_eq!(
            validate_agent_profiler_variant_response_line_v3(&substituted).unwrap_err(),
            AgentProfilerVariantServiceErrorV3::InvalidResponse
        );
    }
}
