//! Fresh-process, read-only diagnosis of retained simulator evidence.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use fe2o3_kernel_ir::{BlockId, FunctionId};
use fe2o3_kir_sim::{
    SimulationDataRaceV1, SimulationFailureReductionReportV1, SimulationFailureScheduleV1,
    SimulationInvocationV1, SimulationMemoryConflictV1, SimulationSiteV1,
};
use fe2o3_virtual_runtime::{
    VirtualDispatchInputBindingV1, VirtualEvidenceIdentityV1, VirtualHostLifetimeBlockerV1,
    VirtualHostLifetimeCompletenessV1, VirtualHostLifetimeEvidenceV1, VirtualHostLifetimeFindingV1,
    VirtualHostLifetimeOperationV1, VirtualKirEvidenceReferenceV1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SIMULATION_AGENT_REQUEST_SCHEMA_V1: &str = "fe2o3-sim-agent-request-v1";
pub const SIMULATION_AGENT_RESPONSE_SCHEMA_V1: &str = "fe2o3-sim-agent-response-v1";
pub const SIMULATION_AGENT_DIAGNOSIS_SCHEMA_V1: &str = "fe2o3-sim-agent-diagnosis-v1";
pub const SIMULATION_AGENT_REDUCTION_SCHEMA_V1: &str = "fe2o3-sim-agent-reduction-v1";
pub const MAX_SIMULATION_AGENT_REQUESTS_V1: u32 = 128;
pub const MAX_SIMULATION_AGENT_REQUEST_BYTES_V1: usize = 20 * 1024 * 1024;
pub const MAX_SIMULATION_AGENT_RESPONSE_BYTES_V1: usize = 4 * 1024 * 1024;
pub const MAX_SIMULATION_AGENT_EVIDENCE_BYTES_V1: usize = 8 * 1024 * 1024;
pub const MAX_SIMULATION_AGENT_PAGE_ITEMS_V1: u16 = 256;
pub const MAX_SIMULATION_AGENT_SITE_BYTES_V1: usize = 4 * 1024;

const CONTRACT_DOMAIN_V1: &[u8] = b"FE2O3/SIM-AGENT/CONTRACT/V1\0";
const CONTRACT_BYTES_V1: &[u8] = b"read-only;no-execution;no-file;no-network;no-patch;canonical-race-reduction-or-host-lifetime-evidence;bounded-jsonl";
const SESSION_DOMAIN_V1: &[u8] = b"FE2O3/SIM-AGENT/SESSION/V1\0";
const RACE_DETAIL_DOMAIN_V1: &[u8] = b"FE2O3/SIM-AGENT/RACE-DETAIL/V1\0";
const DECISION_DOMAIN_V1: &[u8] = b"FE2O3/SIM-AGENT/REDUCTION-DECISION/V1\0";
const CURSOR_DOMAIN_V1: &[u8] = b"FE2O3/SIM-AGENT/CURSOR/V1\0";
const RESPONSE_DOMAIN_V1: &[u8] = b"FE2O3/SIM-AGENT/RESPONSE/V1\0";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationAgentOperationV1 {
    DiscoverCapabilities,
    OpenRace,
    OpenHostLifetime,
    Diagnose,
    Reduce,
    Terminate,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationAgentAuthorityV1 {
    AdvisoryReadOnlyNoExecutionFileNetworkOrPatchAuthority,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationAgentEvidenceKindV1 {
    RaceReduction,
    VirtualHostLifetime,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationAgentCapabilitiesV1 {
    pub service_contract: VirtualEvidenceIdentityV1,
    pub operations: Vec<SimulationAgentOperationV1>,
    pub authority: SimulationAgentAuthorityV1,
    pub accepted_evidence: Vec<SimulationAgentEvidenceKindV1>,
    pub maximum_requests: u32,
    pub maximum_request_bytes: u64,
    pub maximum_response_bytes: u64,
    pub maximum_evidence_bytes: u64,
    pub maximum_page_items: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationAgentRaceInvocationV1 {
    pub global: [u64; 3],
    pub workgroup: [u64; 3],
    pub local: [u32; 3],
    pub workgroup_size: [u32; 3],
    pub workgroup_count: [u64; 3],
    pub launch_extent: [u64; 3],
}

impl From<SimulationInvocationV1> for SimulationAgentRaceInvocationV1 {
    fn from(value: SimulationInvocationV1) -> Self {
        Self {
            global: value.global,
            workgroup: value.workgroup,
            local: value.local,
            workgroup_size: value.workgroup_size,
            workgroup_count: value.workgroup_count,
            launch_extent: value.launch_extent,
        }
    }
}

impl From<SimulationAgentRaceInvocationV1> for SimulationInvocationV1 {
    fn from(value: SimulationAgentRaceInvocationV1) -> Self {
        Self {
            global: value.global,
            workgroup: value.workgroup,
            local: value.local,
            workgroup_size: value.workgroup_size,
            workgroup_count: value.workgroup_count,
            launch_extent: value.launch_extent,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationAgentRaceSiteV1 {
    pub function: String,
    pub block: u32,
    pub operation: Option<u32>,
}

impl From<&SimulationSiteV1> for SimulationAgentRaceSiteV1 {
    fn from(value: &SimulationSiteV1) -> Self {
        Self {
            function: value.function.as_str().to_owned(),
            block: value.block.0,
            operation: value.operation,
        }
    }
}

impl From<SimulationAgentRaceSiteV1> for SimulationSiteV1 {
    fn from(value: SimulationAgentRaceSiteV1) -> Self {
        Self {
            function: FunctionId::new(value.function),
            block: BlockId(value.block),
            operation: value.operation,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationAgentRaceEvidenceV1 {
    pub allocation: u64,
    pub byte_offset: u64,
    pub earlier: SimulationAgentRaceInvocationV1,
    pub later: SimulationAgentRaceInvocationV1,
    pub earlier_site: SimulationAgentRaceSiteV1,
    pub later_site: SimulationAgentRaceSiteV1,
    pub earlier_atomic: bool,
    pub later_atomic: bool,
}

impl SimulationAgentRaceEvidenceV1 {
    pub fn from_simulation(value: &SimulationDataRaceV1) -> Self {
        Self {
            allocation: value.conflict.allocation,
            byte_offset: value.conflict.offset as u64,
            earlier: value.conflict.earlier.into(),
            later: value.conflict.later.into(),
            earlier_site: (&value.conflict.earlier_site).into(),
            later_site: (&value.conflict.later_site).into(),
            earlier_atomic: value.earlier_atomic,
            later_atomic: value.later_atomic,
        }
    }

    fn validate(&self) -> Result<(), SimulationAgentErrorCodeV1> {
        if self.allocation == 0
            || self.earlier == self.later
            || (self.earlier_atomic && self.later_atomic)
            || self.earlier_site.operation.is_none()
            || self.later_site.operation.is_none()
            || self.earlier.workgroup_size != self.later.workgroup_size
            || self.earlier.workgroup_count != self.later.workgroup_count
            || self.earlier.launch_extent != self.later.launch_extent
        {
            return Err(SimulationAgentErrorCodeV1::InvalidEvidence);
        }
        for site in [&self.earlier_site, &self.later_site] {
            if site.function.is_empty()
                || site.function.len() > MAX_SIMULATION_AGENT_SITE_BYTES_V1
                || site.function.bytes().any(|byte| byte.is_ascii_control())
            {
                return Err(SimulationAgentErrorCodeV1::InvalidEvidence);
            }
        }
        validate_invocation(&self.earlier)?;
        validate_invocation(&self.later)?;
        usize::try_from(self.byte_offset)
            .map(|_| ())
            .map_err(|_| SimulationAgentErrorCodeV1::InvalidEvidence)
    }

    fn to_simulation(&self) -> Result<SimulationDataRaceV1, SimulationAgentErrorCodeV1> {
        self.validate()?;
        Ok(SimulationDataRaceV1 {
            conflict: SimulationMemoryConflictV1 {
                allocation: self.allocation,
                offset: usize::try_from(self.byte_offset)
                    .map_err(|_| SimulationAgentErrorCodeV1::InvalidEvidence)?,
                earlier: self.earlier.clone().into(),
                later: self.later.clone().into(),
                earlier_site: self.earlier_site.clone().into(),
                later_site: self.later_site.clone().into(),
            },
            earlier_atomic: self.earlier_atomic,
            later_atomic: self.later_atomic,
        })
    }
}

fn validate_invocation(
    invocation: &SimulationAgentRaceInvocationV1,
) -> Result<(), SimulationAgentErrorCodeV1> {
    for axis in 0..3 {
        let size = u64::from(invocation.workgroup_size[axis]);
        let extent = invocation.launch_extent[axis];
        if size == 0
            || extent == 0
            || invocation.local[axis] >= invocation.workgroup_size[axis]
            || invocation.workgroup_count[axis] != extent.div_ceil(size)
            || invocation.workgroup[axis] >= invocation.workgroup_count[axis]
        {
            return Err(SimulationAgentErrorCodeV1::InvalidEvidence);
        }
        let global = invocation.workgroup[axis]
            .checked_mul(size)
            .and_then(|base| base.checked_add(u64::from(invocation.local[axis])))
            .ok_or(SimulationAgentErrorCodeV1::InvalidEvidence)?;
        if global != invocation.global[axis] || global >= extent {
            return Err(SimulationAgentErrorCodeV1::InvalidEvidence);
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationAgentPageRequestV1 {
    pub limit: u16,
    pub cursor: Option<SimulationAgentPageCursorV1>,
}

impl SimulationAgentPageRequestV1 {
    fn validate(self) -> Result<(), SimulationAgentErrorCodeV1> {
        if self.limit == 0 || self.limit > MAX_SIMULATION_AGENT_PAGE_ITEMS_V1 {
            return Err(SimulationAgentErrorCodeV1::InvalidPage);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationAgentPageCursorV1 {
    pub start: u64,
    pub identity: VirtualEvidenceIdentityV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum SimulationAgentRequestV1 {
    DiscoverCapabilities {
        schema: String,
        request_id: u64,
        expected_revision: u64,
    },
    OpenRace {
        schema: String,
        request_id: u64,
        expected_revision: u64,
        reduction_report_hex: String,
        race: Box<SimulationAgentRaceEvidenceV1>,
        expected_kir: VirtualKirEvidenceReferenceV1,
        expected_context_identity: VirtualEvidenceIdentityV1,
        expected_report_identity: VirtualEvidenceIdentityV1,
    },
    OpenHostLifetime {
        schema: String,
        request_id: u64,
        expected_revision: u64,
        evidence_hex: String,
        expected_runtime_identity: VirtualEvidenceIdentityV1,
        expected_incident_identity: VirtualEvidenceIdentityV1,
    },
    Diagnose {
        schema: String,
        request_id: u64,
        expected_revision: u64,
        session_identity: VirtualEvidenceIdentityV1,
    },
    Reduce {
        schema: String,
        request_id: u64,
        expected_revision: u64,
        session_identity: VirtualEvidenceIdentityV1,
        page: SimulationAgentPageRequestV1,
    },
    Terminate {
        schema: String,
        request_id: u64,
        expected_revision: u64,
        session_identity: VirtualEvidenceIdentityV1,
    },
}

impl SimulationAgentRequestV1 {
    fn schema(&self) -> &str {
        match self {
            Self::DiscoverCapabilities { schema, .. }
            | Self::OpenRace { schema, .. }
            | Self::OpenHostLifetime { schema, .. }
            | Self::Diagnose { schema, .. }
            | Self::Reduce { schema, .. }
            | Self::Terminate { schema, .. } => schema,
        }
    }

    fn request_id(&self) -> u64 {
        match self {
            Self::DiscoverCapabilities { request_id, .. }
            | Self::OpenRace { request_id, .. }
            | Self::OpenHostLifetime { request_id, .. }
            | Self::Diagnose { request_id, .. }
            | Self::Reduce { request_id, .. }
            | Self::Terminate { request_id, .. } => *request_id,
        }
    }

    fn expected_revision(&self) -> u64 {
        match self {
            Self::DiscoverCapabilities {
                expected_revision, ..
            }
            | Self::OpenRace {
                expected_revision, ..
            }
            | Self::OpenHostLifetime {
                expected_revision, ..
            }
            | Self::Diagnose {
                expected_revision, ..
            }
            | Self::Reduce {
                expected_revision, ..
            }
            | Self::Terminate {
                expected_revision, ..
            } => *expected_revision,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationAgentFactOriginV1 {
    Declared,
    SimulatedObserved,
    InferredFromObservedVirtualState,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationAgentUnavailableReasonV1 {
    NoModeledHappensBeforeEdge,
    ScheduleSpaceNotExhausted,
    ProducerAuthenticationUnavailable,
    HardwareExecutionUnavailable,
    DispatchInputIdentityByteLimit,
    BlockerInventoryTruncated,
    LocalMinimumNotEstablished,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationAgentUnavailableV1 {
    pub fact: String,
    pub origin: SimulationAgentFactOriginV1,
    pub reason: SimulationAgentUnavailableReasonV1,
    pub evidence_ids: Vec<VirtualEvidenceIdentityV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationAgentRaceFindingV1 {
    UnorderedConflictingAccesses,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationAgentRaceDiagnosisV1 {
    pub finding: SimulationAgentRaceFindingV1,
    pub origin: SimulationAgentFactOriginV1,
    pub kir: VirtualKirEvidenceReferenceV1,
    pub context_identity: VirtualEvidenceIdentityV1,
    pub report_identity: VirtualEvidenceIdentityV1,
    pub reproducer_identity: VirtualEvidenceIdentityV1,
    pub race_identity: VirtualEvidenceIdentityV1,
    pub original_schedule: SimulationAgentOriginalScheduleV1,
    pub race: SimulationAgentRaceEvidenceV1,
    pub reduction: SimulationAgentReductionCoverageV1,
    pub evidence_ids: Vec<VirtualEvidenceIdentityV1>,
    pub unavailable: Vec<SimulationAgentUnavailableV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SimulationAgentOriginalScheduleV1 {
    Canonical,
    Seeded { seed: u64 },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationAgentReductionCoverageV1 {
    pub attempts: u64,
    pub matching_candidates: u64,
    pub rejected_candidates: u64,
    pub removed_decisions: u64,
    pub locally_minimal: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationAgentHostLifetimeDiagnosisV1 {
    pub finding: VirtualHostLifetimeFindingV1,
    pub finding_origin: SimulationAgentFactOriginV1,
    pub attempted_operation: VirtualHostLifetimeOperationV1,
    pub attempted_operation_origin: SimulationAgentFactOriginV1,
    pub runtime_identity: VirtualEvidenceIdentityV1,
    pub incident_identity: VirtualEvidenceIdentityV1,
    pub buffer_ordinal: u64,
    pub retained_dispatches: u64,
    pub blockers: Vec<VirtualHostLifetimeBlockerV1>,
    pub completeness: VirtualHostLifetimeCompletenessV1,
    pub evidence_ids: Vec<VirtualEvidenceIdentityV1>,
    pub unavailable: Vec<SimulationAgentUnavailableV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SimulationAgentDiagnosisV1 {
    Race {
        schema: String,
        diagnosis: Box<SimulationAgentRaceDiagnosisV1>,
    },
    HostLifetime {
        schema: String,
        diagnosis: Box<SimulationAgentHostLifetimeDiagnosisV1>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationAgentReductionCompletenessV1 {
    SimulatorVerifiedLocallyMinimal,
    SimulatorVerifiedLocalMinimumUnavailable,
    MinimumPositiveWitnessFromCompleteIncident,
    MinimumPositiveWitnessFromPartialIncident,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SimulationAgentReductionItemV1 {
    RaceDecision {
        ordinal: u64,
        workgroup: [u64; 3],
        phase: u64,
        local: [u32; 3],
        evidence_identity: VirtualEvidenceIdentityV1,
    },
    HostBlockingCompletion {
        blocker: VirtualHostLifetimeBlockerV1,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationAgentReductionPageV1 {
    pub schema: String,
    pub session_identity: VirtualEvidenceIdentityV1,
    pub source_evidence_identity: VirtualEvidenceIdentityV1,
    pub completeness: SimulationAgentReductionCompletenessV1,
    pub total_items: u64,
    pub items: Vec<SimulationAgentReductionItemV1>,
    pub next_cursor: Option<SimulationAgentPageCursorV1>,
    pub unavailable: Vec<SimulationAgentUnavailableV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub enum SimulationAgentResultV1 {
    Capabilities {
        capabilities: SimulationAgentCapabilitiesV1,
    },
    Opened {
        kind: SimulationAgentEvidenceKindV1,
        session_identity: VirtualEvidenceIdentityV1,
        evidence_identity: VirtualEvidenceIdentityV1,
        artifact_identities: Vec<VirtualEvidenceIdentityV1>,
    },
    Diagnosis {
        diagnosis: SimulationAgentDiagnosisV1,
    },
    Reduction {
        reduction: SimulationAgentReductionPageV1,
    },
    Terminated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationAgentErrorCodeV1 {
    InvalidRequest,
    InvalidSchema,
    InvalidRequestId,
    DuplicateRequestId,
    StaleRevision,
    RequestBudgetExhausted,
    RequestTooLarge,
    InvalidEvidenceEncoding,
    EvidenceTooLarge,
    InvalidEvidence,
    EvidenceIdentityMismatch,
    EvidenceAlreadyOpen,
    EvidenceNotOpen,
    SessionMismatch,
    InvalidPage,
    CursorMismatch,
    ResponseTooLarge,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SimulationAgentResponseV1 {
    Ok {
        schema: String,
        request_id: u64,
        response_revision: u64,
        value: SimulationAgentResultV1,
        response_identity: VirtualEvidenceIdentityV1,
    },
    Error {
        schema: String,
        request_id: Option<u64>,
        response_revision: u64,
        code: SimulationAgentErrorCodeV1,
        terminal: bool,
        response_identity: VirtualEvidenceIdentityV1,
    },
}

impl SimulationAgentResponseV1 {
    fn is_terminal(&self) -> bool {
        matches!(self, Self::Error { terminal: true, .. })
            || matches!(
                self,
                Self::Ok {
                    value: SimulationAgentResultV1::Terminated,
                    ..
                }
            )
    }
}

#[derive(Clone)]
enum ActiveEvidenceV1 {
    Race {
        session_identity: VirtualEvidenceIdentityV1,
        report: Box<SimulationFailureReductionReportV1>,
        race: Box<SimulationAgentRaceEvidenceV1>,
        race_identity: VirtualEvidenceIdentityV1,
    },
    HostLifetime {
        session_identity: VirtualEvidenceIdentityV1,
        evidence: VirtualHostLifetimeEvidenceV1,
    },
}

impl ActiveEvidenceV1 {
    const fn session_identity(&self) -> VirtualEvidenceIdentityV1 {
        match self {
            Self::Race {
                session_identity, ..
            }
            | Self::HostLifetime {
                session_identity, ..
            } => *session_identity,
        }
    }
}

pub struct SimulationAgentServiceV1 {
    contract_identity: VirtualEvidenceIdentityV1,
    response_revision: u64,
    remaining_requests: u32,
    seen_request_ids: BTreeSet<u64>,
    active: Option<ActiveEvidenceV1>,
}

impl SimulationAgentServiceV1 {
    pub fn new() -> Result<Self, SimulationAgentServiceErrorV1> {
        Ok(Self {
            contract_identity: content_identity(CONTRACT_DOMAIN_V1, CONTRACT_BYTES_V1)?,
            response_revision: 0,
            remaining_requests: MAX_SIMULATION_AGENT_REQUESTS_V1,
            seen_request_ids: BTreeSet::new(),
            active: None,
        })
    }

    pub fn handle(
        &mut self,
        request: SimulationAgentRequestV1,
    ) -> Result<SimulationAgentResponseV1, SimulationAgentServiceErrorV1> {
        if self.remaining_requests == 0 {
            return self.error(
                Some(request.request_id()),
                SimulationAgentErrorCodeV1::RequestBudgetExhausted,
                true,
            );
        }
        self.remaining_requests -= 1;
        let request_id = request.request_id();
        if request_id == 0 {
            return self.error(
                Some(request_id),
                SimulationAgentErrorCodeV1::InvalidRequestId,
                false,
            );
        }
        if !self.seen_request_ids.insert(request_id) {
            return self.error(
                Some(request_id),
                SimulationAgentErrorCodeV1::DuplicateRequestId,
                false,
            );
        }
        if request.schema() != SIMULATION_AGENT_REQUEST_SCHEMA_V1 {
            return self.error(
                Some(request_id),
                SimulationAgentErrorCodeV1::InvalidSchema,
                false,
            );
        }
        if request.expected_revision() != self.response_revision {
            return self.error(
                Some(request_id),
                SimulationAgentErrorCodeV1::StaleRevision,
                false,
            );
        }

        match request {
            SimulationAgentRequestV1::DiscoverCapabilities { .. } => {
                self.ok(request_id, SimulationAgentResultV1::Capabilities {
                    capabilities: SimulationAgentCapabilitiesV1 {
                        service_contract: self.contract_identity,
                        operations: vec![
                            SimulationAgentOperationV1::DiscoverCapabilities,
                            SimulationAgentOperationV1::OpenRace,
                            SimulationAgentOperationV1::OpenHostLifetime,
                            SimulationAgentOperationV1::Diagnose,
                            SimulationAgentOperationV1::Reduce,
                            SimulationAgentOperationV1::Terminate,
                        ],
                        authority: SimulationAgentAuthorityV1::AdvisoryReadOnlyNoExecutionFileNetworkOrPatchAuthority,
                        accepted_evidence: vec![
                            SimulationAgentEvidenceKindV1::RaceReduction,
                            SimulationAgentEvidenceKindV1::VirtualHostLifetime,
                        ],
                        maximum_requests: MAX_SIMULATION_AGENT_REQUESTS_V1,
                        maximum_request_bytes: MAX_SIMULATION_AGENT_REQUEST_BYTES_V1 as u64,
                        maximum_response_bytes: MAX_SIMULATION_AGENT_RESPONSE_BYTES_V1 as u64,
                        maximum_evidence_bytes: MAX_SIMULATION_AGENT_EVIDENCE_BYTES_V1 as u64,
                        maximum_page_items: MAX_SIMULATION_AGENT_PAGE_ITEMS_V1,
                    },
                })
            }
            SimulationAgentRequestV1::OpenRace {
                reduction_report_hex,
                race,
                expected_kir,
                expected_context_identity,
                expected_report_identity,
                ..
            } => {
                if self.active.is_some() {
                    return self.error(Some(request_id), SimulationAgentErrorCodeV1::EvidenceAlreadyOpen, false);
                }
                let report_bytes = match decode_hex(&reduction_report_hex) {
                    Ok(value) => value,
                    Err(code) => return self.error(Some(request_id), code, false),
                };
                let report = match SimulationFailureReductionReportV1::from_canonical_bytes(&report_bytes) {
                    Ok(value) => value,
                    Err(_) => return self.error(Some(request_id), SimulationAgentErrorCodeV1::InvalidEvidence, false),
                };
                let actual_kir = match kir_reference(&report) {
                    Ok(value) => value,
                    Err(error) => return self.error(Some(request_id), error, false),
                };
                let actual_context = match identity(*report.context_identity()) {
                    Ok(value) => value,
                    Err(_) => return self.error(Some(request_id), SimulationAgentErrorCodeV1::InvalidEvidence, false),
                };
                let actual_report = match identity(*report.report_identity()) {
                    Ok(value) => value,
                    Err(_) => return self.error(Some(request_id), SimulationAgentErrorCodeV1::InvalidEvidence, false),
                };
                let detailed = match race.to_simulation() {
                    Ok(value) => value,
                    Err(code) => return self.error(Some(request_id), code, false),
                };
                if report.fingerprint().class() != "data_race"
                    || !report.matches_data_race(&detailed)
                    || expected_kir != actual_kir
                    || expected_context_identity != actual_context
                    || expected_report_identity != actual_report
                {
                    return self.error(Some(request_id), SimulationAgentErrorCodeV1::EvidenceIdentityMismatch, false);
                }
                let race_identity = match content_identity(
                    RACE_DETAIL_DOMAIN_V1,
                    &serde_json::to_vec(&race).map_err(|_| SimulationAgentServiceErrorV1::JsonEncode)?,
                ) {
                    Ok(value) => value,
                    Err(_) => return self.error(Some(request_id), SimulationAgentErrorCodeV1::InvalidEvidence, false),
                };
                let session_identity = session_identity(
                    SimulationAgentEvidenceKindV1::RaceReduction,
                    actual_report,
                    &[actual_kir.sha256, actual_context, race_identity],
                )?;
                self.active = Some(ActiveEvidenceV1::Race {
                    session_identity,
                    report: Box::new(report),
                    race,
                    race_identity,
                });
                self.ok(request_id, SimulationAgentResultV1::Opened {
                    kind: SimulationAgentEvidenceKindV1::RaceReduction,
                    session_identity,
                    evidence_identity: actual_report,
                    artifact_identities: vec![actual_kir.sha256, actual_context],
                })
            }
            SimulationAgentRequestV1::OpenHostLifetime {
                evidence_hex,
                expected_runtime_identity,
                expected_incident_identity,
                ..
            } => {
                if self.active.is_some() {
                    return self.error(Some(request_id), SimulationAgentErrorCodeV1::EvidenceAlreadyOpen, false);
                }
                let evidence_bytes = match decode_hex(&evidence_hex) {
                    Ok(value) => value,
                    Err(code) => return self.error(Some(request_id), code, false),
                };
                let evidence = match VirtualHostLifetimeEvidenceV1::from_canonical_bytes(&evidence_bytes) {
                    Ok(value) => value,
                    Err(_) => return self.error(Some(request_id), SimulationAgentErrorCodeV1::InvalidEvidence, false),
                };
                if evidence.runtime_identity != expected_runtime_identity
                    || evidence.incident_identity != expected_incident_identity
                {
                    return self.error(Some(request_id), SimulationAgentErrorCodeV1::EvidenceIdentityMismatch, false);
                }
                let artifacts = host_artifact_identities(&evidence);
                let session_identity = session_identity(
                    SimulationAgentEvidenceKindV1::VirtualHostLifetime,
                    evidence.incident_identity,
                    &artifacts,
                )?;
                let evidence_identity = evidence.incident_identity;
                self.active = Some(ActiveEvidenceV1::HostLifetime {
                    session_identity,
                    evidence,
                });
                self.ok(request_id, SimulationAgentResultV1::Opened {
                    kind: SimulationAgentEvidenceKindV1::VirtualHostLifetime,
                    session_identity,
                    evidence_identity,
                    artifact_identities: artifacts,
                })
            }
            SimulationAgentRequestV1::Diagnose {
                session_identity, ..
            } => {
                let active = match self.active.clone() {
                    Some(value) if value.session_identity() == session_identity => value,
                    Some(_) => return self.error(Some(request_id), SimulationAgentErrorCodeV1::SessionMismatch, false),
                    None => return self.error(Some(request_id), SimulationAgentErrorCodeV1::EvidenceNotOpen, false),
                };
                let diagnosis = diagnosis(active)?;
                self.ok(request_id, SimulationAgentResultV1::Diagnosis { diagnosis })
            }
            SimulationAgentRequestV1::Reduce {
                session_identity,
                page,
                ..
            } => {
                if let Err(code) = page.validate() {
                    return self.error(Some(request_id), code, false);
                }
                let active = match self.active.clone() {
                    Some(value) if value.session_identity() == session_identity => value,
                    Some(_) => return self.error(Some(request_id), SimulationAgentErrorCodeV1::SessionMismatch, false),
                    None => return self.error(Some(request_id), SimulationAgentErrorCodeV1::EvidenceNotOpen, false),
                };
                let reduction = match reduction_page(active, page) {
                    Ok(value) => value,
                    Err(code) => return self.error(Some(request_id), code, false),
                };
                self.ok(request_id, SimulationAgentResultV1::Reduction { reduction })
            }
            SimulationAgentRequestV1::Terminate {
                session_identity, ..
            } => {
                let Some(active) = &self.active else {
                    return self.error(Some(request_id), SimulationAgentErrorCodeV1::EvidenceNotOpen, false);
                };
                if active.session_identity() != session_identity {
                    return self.error(Some(request_id), SimulationAgentErrorCodeV1::SessionMismatch, false);
                }
                self.active = None;
                self.ok(request_id, SimulationAgentResultV1::Terminated)
            }
        }
    }

    pub fn encode_response(
        &self,
        response: &SimulationAgentResponseV1,
    ) -> Result<Vec<u8>, SimulationAgentServiceErrorV1> {
        validate_response_identity(response)?;
        let mut bytes =
            serde_json::to_vec(response).map_err(|_| SimulationAgentServiceErrorV1::JsonEncode)?;
        if bytes.len() + 1 > MAX_SIMULATION_AGENT_RESPONSE_BYTES_V1 {
            return Err(SimulationAgentServiceErrorV1::ResponseTooLarge);
        }
        bytes.push(b'\n');
        Ok(bytes)
    }

    fn ok(
        &mut self,
        request_id: u64,
        value: SimulationAgentResultV1,
    ) -> Result<SimulationAgentResponseV1, SimulationAgentServiceErrorV1> {
        self.response_revision = self
            .response_revision
            .checked_add(1)
            .ok_or(SimulationAgentServiceErrorV1::RevisionOverflow)?;
        let response_identity = ok_response_identity(request_id, self.response_revision, &value)?;
        Ok(SimulationAgentResponseV1::Ok {
            schema: SIMULATION_AGENT_RESPONSE_SCHEMA_V1.to_owned(),
            request_id,
            response_revision: self.response_revision,
            value,
            response_identity,
        })
    }

    fn error(
        &mut self,
        request_id: Option<u64>,
        code: SimulationAgentErrorCodeV1,
        terminal: bool,
    ) -> Result<SimulationAgentResponseV1, SimulationAgentServiceErrorV1> {
        self.response_revision = self
            .response_revision
            .checked_add(1)
            .ok_or(SimulationAgentServiceErrorV1::RevisionOverflow)?;
        let response_identity =
            error_response_identity(request_id, self.response_revision, code, terminal)?;
        Ok(SimulationAgentResponseV1::Error {
            schema: SIMULATION_AGENT_RESPONSE_SCHEMA_V1.to_owned(),
            request_id,
            response_revision: self.response_revision,
            code,
            terminal,
            response_identity,
        })
    }
}

fn diagnosis(
    active: ActiveEvidenceV1,
) -> Result<SimulationAgentDiagnosisV1, SimulationAgentServiceErrorV1> {
    match active {
        ActiveEvidenceV1::Race {
            report,
            race,
            race_identity,
            ..
        } => {
            let report_identity = identity(*report.report_identity())?;
            let reproducer_identity = identity(*report.reproducer_identity())?;
            let detail_identity = identity(*report.fingerprint().detail_identity())?;
            let evidence_ids = sorted_identities(vec![
                report_identity,
                reproducer_identity,
                race_identity,
                detail_identity,
            ]);
            let coverage = report.coverage();
            let unavailable = vec![
                unavailable(
                    "happens_before",
                    SimulationAgentUnavailableReasonV1::NoModeledHappensBeforeEdge,
                    evidence_ids.clone(),
                ),
                unavailable(
                    "schedule_space",
                    SimulationAgentUnavailableReasonV1::ScheduleSpaceNotExhausted,
                    evidence_ids.clone(),
                ),
                unavailable(
                    "producer_authentication",
                    SimulationAgentUnavailableReasonV1::ProducerAuthenticationUnavailable,
                    vec![report_identity],
                ),
                unavailable(
                    "hardware_execution",
                    SimulationAgentUnavailableReasonV1::HardwareExecutionUnavailable,
                    vec![report_identity],
                ),
            ];
            Ok(SimulationAgentDiagnosisV1::Race {
                schema: SIMULATION_AGENT_DIAGNOSIS_SCHEMA_V1.to_owned(),
                diagnosis: Box::new(SimulationAgentRaceDiagnosisV1 {
                    finding: SimulationAgentRaceFindingV1::UnorderedConflictingAccesses,
                    origin: SimulationAgentFactOriginV1::SimulatedObserved,
                    kir: kir_reference(&report)
                        .map_err(|_| SimulationAgentServiceErrorV1::InvalidEvidence)?,
                    context_identity: identity(*report.context_identity())?,
                    report_identity,
                    reproducer_identity,
                    race_identity,
                    original_schedule: match report.original_schedule() {
                        SimulationFailureScheduleV1::Canonical => {
                            SimulationAgentOriginalScheduleV1::Canonical
                        }
                        SimulationFailureScheduleV1::Seeded { seed } => {
                            SimulationAgentOriginalScheduleV1::Seeded { seed }
                        }
                    },
                    race: *race,
                    reduction: SimulationAgentReductionCoverageV1 {
                        attempts: coverage.attempts() as u64,
                        matching_candidates: coverage.matching_candidates() as u64,
                        rejected_candidates: coverage.rejected_candidates() as u64,
                        removed_decisions: coverage.removed_decisions() as u64,
                        locally_minimal: coverage.is_locally_minimal(),
                    },
                    evidence_ids,
                    unavailable,
                }),
            })
        }
        ActiveEvidenceV1::HostLifetime { evidence, .. } => {
            let mut evidence_ids = evidence
                .blockers
                .iter()
                .map(|blocker| blocker.blocker_identity)
                .collect::<Vec<_>>();
            evidence_ids.push(evidence.incident_identity);
            let evidence_ids = sorted_identities(evidence_ids);
            let mut unavailable_facts = vec![
                unavailable(
                    "producer_authentication",
                    SimulationAgentUnavailableReasonV1::ProducerAuthenticationUnavailable,
                    vec![evidence.incident_identity],
                ),
                unavailable(
                    "hardware_execution",
                    SimulationAgentUnavailableReasonV1::HardwareExecutionUnavailable,
                    vec![evidence.incident_identity],
                ),
            ];
            if evidence.blockers.iter().any(|blocker| {
                matches!(
                    blocker.dispatch_input,
                    VirtualDispatchInputBindingV1::Unavailable { .. }
                )
            }) {
                unavailable_facts.push(unavailable(
                    "dispatch_input_identity",
                    SimulationAgentUnavailableReasonV1::DispatchInputIdentityByteLimit,
                    evidence_ids.clone(),
                ));
            }
            if matches!(
                evidence.completeness,
                VirtualHostLifetimeCompletenessV1::PartialBlockerLimit { .. }
                    | VirtualHostLifetimeCompletenessV1::PartialBlockerAndInputIdentity { .. }
            ) {
                unavailable_facts.push(unavailable(
                    "complete_blocker_inventory",
                    SimulationAgentUnavailableReasonV1::BlockerInventoryTruncated,
                    evidence_ids.clone(),
                ));
            }
            Ok(SimulationAgentDiagnosisV1::HostLifetime {
                schema: SIMULATION_AGENT_DIAGNOSIS_SCHEMA_V1.to_owned(),
                diagnosis: Box::new(SimulationAgentHostLifetimeDiagnosisV1 {
                    finding: evidence.finding,
                    finding_origin: SimulationAgentFactOriginV1::InferredFromObservedVirtualState,
                    attempted_operation: evidence.operation,
                    attempted_operation_origin: SimulationAgentFactOriginV1::Declared,
                    runtime_identity: evidence.runtime_identity,
                    incident_identity: evidence.incident_identity,
                    buffer_ordinal: evidence.buffer_ordinal,
                    retained_dispatches: evidence.retained_dispatches,
                    blockers: evidence.blockers,
                    completeness: evidence.completeness,
                    evidence_ids,
                    unavailable: unavailable_facts,
                }),
            })
        }
    }
}

fn reduction_page(
    active: ActiveEvidenceV1,
    page: SimulationAgentPageRequestV1,
) -> Result<SimulationAgentReductionPageV1, SimulationAgentErrorCodeV1> {
    let session_identity = active.session_identity();
    let (source_evidence_identity, total, completeness, unavailable) = match &active {
        ActiveEvidenceV1::Race { report, .. } => {
            let source = identity(*report.report_identity())
                .map_err(|_| SimulationAgentErrorCodeV1::InvalidEvidence)?;
            let completeness = if report.coverage().is_locally_minimal() {
                SimulationAgentReductionCompletenessV1::SimulatorVerifiedLocallyMinimal
            } else {
                SimulationAgentReductionCompletenessV1::SimulatorVerifiedLocalMinimumUnavailable
            };
            let unavailable = if report.coverage().is_locally_minimal() {
                Vec::new()
            } else {
                vec![unavailable(
                    "local_minimum",
                    SimulationAgentUnavailableReasonV1::LocalMinimumNotEstablished,
                    vec![source],
                )]
            };
            (
                source,
                report.reproducer_schedule().len(),
                completeness,
                unavailable,
            )
        }
        ActiveEvidenceV1::HostLifetime { evidence, .. } => {
            let complete = matches!(
                evidence.completeness,
                VirtualHostLifetimeCompletenessV1::Complete
            );
            let unavailable = match evidence.completeness {
                VirtualHostLifetimeCompletenessV1::Complete => Vec::new(),
                VirtualHostLifetimeCompletenessV1::PartialBlockerLimit { .. } => vec![unavailable(
                    "complete_blocker_inventory",
                    SimulationAgentUnavailableReasonV1::BlockerInventoryTruncated,
                    vec![evidence.incident_identity],
                )],
                VirtualHostLifetimeCompletenessV1::PartialInputIdentity { .. } => {
                    vec![unavailable(
                        "dispatch_input_identity",
                        SimulationAgentUnavailableReasonV1::DispatchInputIdentityByteLimit,
                        vec![evidence.incident_identity],
                    )]
                }
                VirtualHostLifetimeCompletenessV1::PartialBlockerAndInputIdentity { .. } => vec![
                    unavailable(
                        "complete_blocker_inventory",
                        SimulationAgentUnavailableReasonV1::BlockerInventoryTruncated,
                        vec![evidence.incident_identity],
                    ),
                    unavailable(
                        "dispatch_input_identity",
                        SimulationAgentUnavailableReasonV1::DispatchInputIdentityByteLimit,
                        vec![evidence.incident_identity],
                    ),
                ],
            };
            (
                evidence.incident_identity,
                1,
                if complete {
                    SimulationAgentReductionCompletenessV1::MinimumPositiveWitnessFromCompleteIncident
                } else {
                    SimulationAgentReductionCompletenessV1::MinimumPositiveWitnessFromPartialIncident
                },
                unavailable,
            )
        }
    };
    let start = match page.cursor {
        Some(cursor) => {
            let start = usize::try_from(cursor.start)
                .map_err(|_| SimulationAgentErrorCodeV1::InvalidPage)?;
            if cursor.identity
                != cursor_identity(session_identity, source_evidence_identity, start)?
            {
                return Err(SimulationAgentErrorCodeV1::CursorMismatch);
            }
            start
        }
        None => 0,
    };
    if start > total {
        return Err(SimulationAgentErrorCodeV1::InvalidPage);
    }
    let end = start.saturating_add(usize::from(page.limit)).min(total);
    let mut items = Vec::new();
    items
        .try_reserve_exact(end - start)
        .map_err(|_| SimulationAgentErrorCodeV1::ResponseTooLarge)?;
    match active {
        ActiveEvidenceV1::Race { report, .. } => {
            for (offset, decision) in report.reproducer_schedule()[start..end]
                .iter()
                .copied()
                .enumerate()
            {
                let ordinal = start + offset;
                items.push(SimulationAgentReductionItemV1::RaceDecision {
                    ordinal: ordinal as u64,
                    workgroup: decision.workgroup(),
                    phase: decision.phase(),
                    local: decision.local(),
                    evidence_identity: decision_identity(
                        source_evidence_identity,
                        ordinal,
                        decision.workgroup(),
                        decision.phase(),
                        decision.local(),
                    )?,
                });
            }
        }
        ActiveEvidenceV1::HostLifetime { evidence, .. } => {
            if start == 0 && end == 1 {
                items.push(SimulationAgentReductionItemV1::HostBlockingCompletion {
                    blocker: evidence.blockers[0].clone(),
                });
            }
        }
    }
    let next_cursor = if end < total {
        Some(SimulationAgentPageCursorV1 {
            start: end as u64,
            identity: cursor_identity(session_identity, source_evidence_identity, end)?,
        })
    } else {
        None
    };
    Ok(SimulationAgentReductionPageV1 {
        schema: SIMULATION_AGENT_REDUCTION_SCHEMA_V1.to_owned(),
        session_identity,
        source_evidence_identity,
        completeness,
        total_items: total as u64,
        items,
        next_cursor,
        unavailable,
    })
}

fn unavailable(
    fact: &str,
    reason: SimulationAgentUnavailableReasonV1,
    evidence_ids: Vec<VirtualEvidenceIdentityV1>,
) -> SimulationAgentUnavailableV1 {
    SimulationAgentUnavailableV1 {
        fact: fact.to_owned(),
        origin: SimulationAgentFactOriginV1::Unavailable,
        reason,
        evidence_ids: sorted_identities(evidence_ids),
    }
}

fn host_artifact_identities(
    evidence: &VirtualHostLifetimeEvidenceV1,
) -> Vec<VirtualEvidenceIdentityV1> {
    sorted_identities(
        evidence
            .blockers
            .iter()
            .flat_map(|blocker| {
                let input = match blocker.dispatch_input {
                    VirtualDispatchInputBindingV1::Exact { identity, .. } => Some(identity),
                    VirtualDispatchInputBindingV1::Unavailable { .. } => None,
                };
                [Some(blocker.kir.sha256), input].into_iter().flatten()
            })
            .collect(),
    )
}

fn kir_reference(
    report: &SimulationFailureReductionReportV1,
) -> Result<VirtualKirEvidenceReferenceV1, SimulationAgentErrorCodeV1> {
    Ok(VirtualKirEvidenceReferenceV1 {
        wire_version: report.kir_wire_version(),
        sha256: identity(*report.kir_sha256())
            .map_err(|_| SimulationAgentErrorCodeV1::InvalidEvidence)?,
        canonical_bytes: report.kir_canonical_bytes(),
    })
}

fn session_identity(
    kind: SimulationAgentEvidenceKindV1,
    evidence: VirtualEvidenceIdentityV1,
    bindings: &[VirtualEvidenceIdentityV1],
) -> Result<VirtualEvidenceIdentityV1, SimulationAgentServiceErrorV1> {
    let mut bytes = Vec::with_capacity(2 + 32 * (bindings.len() + 1));
    bytes.push(match kind {
        SimulationAgentEvidenceKindV1::RaceReduction => 0,
        SimulationAgentEvidenceKindV1::VirtualHostLifetime => 1,
    });
    bytes.extend_from_slice(&evidence.as_bytes());
    for binding in bindings {
        bytes.extend_from_slice(&binding.as_bytes());
    }
    content_identity(SESSION_DOMAIN_V1, &bytes)
}

fn decision_identity(
    source: VirtualEvidenceIdentityV1,
    ordinal: usize,
    workgroup: [u64; 3],
    phase: u64,
    local: [u32; 3],
) -> Result<VirtualEvidenceIdentityV1, SimulationAgentErrorCodeV1> {
    let mut bytes = Vec::with_capacity(32 + 8 + 24 + 8 + 12);
    bytes.extend_from_slice(&source.as_bytes());
    bytes.extend_from_slice(&(ordinal as u64).to_le_bytes());
    for value in workgroup {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&phase.to_le_bytes());
    for value in local {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    content_identity(DECISION_DOMAIN_V1, &bytes)
        .map_err(|_| SimulationAgentErrorCodeV1::InvalidEvidence)
}

fn cursor_identity(
    session: VirtualEvidenceIdentityV1,
    source: VirtualEvidenceIdentityV1,
    start: usize,
) -> Result<VirtualEvidenceIdentityV1, SimulationAgentErrorCodeV1> {
    let mut bytes = Vec::with_capacity(72);
    bytes.extend_from_slice(&session.as_bytes());
    bytes.extend_from_slice(&source.as_bytes());
    bytes.extend_from_slice(&(start as u64).to_le_bytes());
    content_identity(CURSOR_DOMAIN_V1, &bytes)
        .map_err(|_| SimulationAgentErrorCodeV1::InvalidEvidence)
}

fn sorted_identities(mut values: Vec<VirtualEvidenceIdentityV1>) -> Vec<VirtualEvidenceIdentityV1> {
    values.sort_unstable();
    values.dedup();
    values
}

fn identity(bytes: [u8; 32]) -> Result<VirtualEvidenceIdentityV1, SimulationAgentServiceErrorV1> {
    VirtualEvidenceIdentityV1::new(bytes).map_err(|_| SimulationAgentServiceErrorV1::Identity)
}

fn decode_hex(value: &str) -> Result<Vec<u8>, SimulationAgentErrorCodeV1> {
    if value.len() > MAX_SIMULATION_AGENT_EVIDENCE_BYTES_V1 * 2 {
        return Err(SimulationAgentErrorCodeV1::EvidenceTooLarge);
    }
    if value.is_empty() || !value.len().is_multiple_of(2) {
        return Err(SimulationAgentErrorCodeV1::InvalidEvidenceEncoding);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(value.len() / 2)
        .map_err(|_| SimulationAgentErrorCodeV1::EvidenceTooLarge)?;
    for pair in value.as_bytes().chunks_exact(2) {
        let high = nibble(pair[0]).ok_or(SimulationAgentErrorCodeV1::InvalidEvidenceEncoding)?;
        let low = nibble(pair[1]).ok_or(SimulationAgentErrorCodeV1::InvalidEvidenceEncoding)?;
        output.push((high << 4) | low);
    }
    Ok(output)
}

const fn nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

pub fn encode_evidence_hex_v1(bytes: &[u8]) -> Result<String, SimulationAgentErrorCodeV1> {
    if bytes.is_empty() || bytes.len() > MAX_SIMULATION_AGENT_EVIDENCE_BYTES_V1 {
        return Err(SimulationAgentErrorCodeV1::EvidenceTooLarge);
    }
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let output_len = bytes
        .len()
        .checked_mul(2)
        .ok_or(SimulationAgentErrorCodeV1::EvidenceTooLarge)?;
    let mut output = String::new();
    output
        .try_reserve_exact(output_len)
        .map_err(|_| SimulationAgentErrorCodeV1::EvidenceTooLarge)?;
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    Ok(output)
}

fn content_identity(
    domain: &[u8],
    bytes: &[u8],
) -> Result<VirtualEvidenceIdentityV1, SimulationAgentServiceErrorV1> {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    identity(hash.finalize().into())
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum ResponsePreimageV1<'a> {
    Ok {
        schema: &'a str,
        request_id: u64,
        response_revision: u64,
        value: &'a SimulationAgentResultV1,
    },
    Error {
        schema: &'a str,
        request_id: Option<u64>,
        response_revision: u64,
        code: SimulationAgentErrorCodeV1,
        terminal: bool,
    },
}

fn ok_response_identity(
    request_id: u64,
    revision: u64,
    value: &SimulationAgentResultV1,
) -> Result<VirtualEvidenceIdentityV1, SimulationAgentServiceErrorV1> {
    response_identity(&ResponsePreimageV1::Ok {
        schema: SIMULATION_AGENT_RESPONSE_SCHEMA_V1,
        request_id,
        response_revision: revision,
        value,
    })
}

fn error_response_identity(
    request_id: Option<u64>,
    revision: u64,
    code: SimulationAgentErrorCodeV1,
    terminal: bool,
) -> Result<VirtualEvidenceIdentityV1, SimulationAgentServiceErrorV1> {
    response_identity(&ResponsePreimageV1::Error {
        schema: SIMULATION_AGENT_RESPONSE_SCHEMA_V1,
        request_id,
        response_revision: revision,
        code,
        terminal,
    })
}

fn response_identity<T: Serialize>(
    value: &T,
) -> Result<VirtualEvidenceIdentityV1, SimulationAgentServiceErrorV1> {
    let value =
        serde_json::to_value(value).map_err(|_| SimulationAgentServiceErrorV1::JsonEncode)?;
    let bytes =
        serde_json::to_vec(&value).map_err(|_| SimulationAgentServiceErrorV1::JsonEncode)?;
    content_identity(RESPONSE_DOMAIN_V1, &bytes)
}

fn validate_response_identity(
    response: &SimulationAgentResponseV1,
) -> Result<(), SimulationAgentServiceErrorV1> {
    let schema = match response {
        SimulationAgentResponseV1::Ok { schema, .. }
        | SimulationAgentResponseV1::Error { schema, .. } => schema,
    };
    if schema != SIMULATION_AGENT_RESPONSE_SCHEMA_V1 {
        return Err(SimulationAgentServiceErrorV1::InvalidResponse);
    }
    let (actual, expected) = match response {
        SimulationAgentResponseV1::Ok {
            request_id,
            response_revision,
            value,
            response_identity,
            ..
        } => (
            ok_response_identity(*request_id, *response_revision, value)?,
            *response_identity,
        ),
        SimulationAgentResponseV1::Error {
            request_id,
            response_revision,
            code,
            terminal,
            response_identity,
            ..
        } => (
            error_response_identity(*request_id, *response_revision, *code, *terminal)?,
            *response_identity,
        ),
    };
    if actual != expected {
        return Err(SimulationAgentServiceErrorV1::InvalidResponse);
    }
    Ok(())
}

pub fn validate_simulation_agent_response_line_v1(
    line: &[u8],
) -> Result<(), SimulationAgentServiceErrorV1> {
    if line.is_empty() || line.len() > MAX_SIMULATION_AGENT_RESPONSE_BYTES_V1 {
        return Err(SimulationAgentServiceErrorV1::InvalidResponse);
    }
    let payload = line.strip_suffix(b"\n").unwrap_or(line);
    if payload.is_empty() || payload.contains(&b'\n') || payload.contains(&b'\r') {
        return Err(SimulationAgentServiceErrorV1::InvalidResponse);
    }
    let response: SimulationAgentResponseV1 = serde_json::from_slice(payload)
        .map_err(|_| SimulationAgentServiceErrorV1::InvalidResponse)?;
    validate_response_identity(&response)
}

pub fn decode_simulation_agent_request_line_v1(
    line: &[u8],
) -> Result<SimulationAgentRequestV1, SimulationAgentServiceErrorV1> {
    if line.is_empty() || line.len() > MAX_SIMULATION_AGENT_REQUEST_BYTES_V1 {
        return Err(SimulationAgentServiceErrorV1::RequestTooLarge);
    }
    let payload = line.strip_suffix(b"\n").unwrap_or(line);
    if payload.is_empty() || payload.contains(&b'\n') || payload.contains(&b'\r') {
        return Err(SimulationAgentServiceErrorV1::InvalidRequest);
    }
    serde_json::from_slice(payload).map_err(|_| SimulationAgentServiceErrorV1::InvalidRequest)
}

pub fn run_simulation_agent_jsonl_v1<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
) -> Result<(), SimulationAgentServiceErrorV1> {
    let mut service = SimulationAgentServiceV1::new()?;
    loop {
        let line = match read_line(input) {
            Ok(Some(value)) => value,
            Ok(None) => return Ok(()),
            Err(error) => {
                let code = if error == SimulationAgentServiceErrorV1::RequestTooLarge {
                    SimulationAgentErrorCodeV1::RequestTooLarge
                } else {
                    SimulationAgentErrorCodeV1::InvalidRequest
                };
                write_terminal(&mut service, output, code)?;
                return Err(error);
            }
        };
        let request = match decode_simulation_agent_request_line_v1(&line) {
            Ok(value) => value,
            Err(_) => {
                write_terminal(
                    &mut service,
                    output,
                    SimulationAgentErrorCodeV1::InvalidRequest,
                )?;
                return Err(SimulationAgentServiceErrorV1::ProtocolTerminated);
            }
        };
        let response = service.handle(request)?;
        let terminal = response.is_terminal();
        output
            .write_all(&service.encode_response(&response)?)
            .map_err(|_| SimulationAgentServiceErrorV1::Io)?;
        output
            .flush()
            .map_err(|_| SimulationAgentServiceErrorV1::Io)?;
        if terminal {
            return Ok(());
        }
    }
}

fn read_line<R: BufRead>(reader: &mut R) -> Result<Option<Vec<u8>>, SimulationAgentServiceErrorV1> {
    let mut line = Vec::new();
    loop {
        let buffer = reader
            .fill_buf()
            .map_err(|_| SimulationAgentServiceErrorV1::Io)?;
        if buffer.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err(SimulationAgentServiceErrorV1::InvalidRequest)
            };
        }
        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let available = newline.map_or(buffer.len(), |index| index + 1);
        let remaining = MAX_SIMULATION_AGENT_REQUEST_BYTES_V1
            .saturating_add(1)
            .saturating_sub(line.len());
        let consumed = available.min(remaining);
        line.extend_from_slice(&buffer[..consumed]);
        reader.consume(consumed);
        if line.len() > MAX_SIMULATION_AGENT_REQUEST_BYTES_V1 {
            return Err(SimulationAgentServiceErrorV1::RequestTooLarge);
        }
        if newline.is_some() && consumed == available {
            return Ok(Some(line));
        }
    }
}

fn write_terminal<W: Write>(
    service: &mut SimulationAgentServiceV1,
    output: &mut W,
    code: SimulationAgentErrorCodeV1,
) -> Result<(), SimulationAgentServiceErrorV1> {
    let response = service.error(None, code, true)?;
    output
        .write_all(&service.encode_response(&response)?)
        .map_err(|_| SimulationAgentServiceErrorV1::Io)?;
    output
        .flush()
        .map_err(|_| SimulationAgentServiceErrorV1::Io)
}

pub fn main() -> ExitCode {
    if std::env::args_os().len() != 1 {
        return ExitCode::FAILURE;
    }
    let stdin = io::stdin();
    let stdout = io::stdout();
    match run_simulation_agent_jsonl_v1(&mut stdin.lock(), &mut stdout.lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationAgentServiceErrorV1 {
    Identity,
    InvalidEvidence,
    InvalidRequest,
    RequestTooLarge,
    InvalidResponse,
    ResponseTooLarge,
    RevisionOverflow,
    JsonEncode,
    Io,
    ProtocolTerminated,
}

impl fmt::Display for SimulationAgentServiceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "simulation agent service failed: {self:?}")
    }
}

impl Error for SimulationAgentServiceErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_request_and_response_identity_fail_closed() {
        assert!(decode_simulation_agent_request_line_v1(
            br#"{"operation":"discover_capabilities","schema":"fe2o3-sim-agent-request-v1","request_id":1,"expected_revision":0,"extra":true}\n"#,
        )
        .is_err());
        let mut service = SimulationAgentServiceV1::new().unwrap();
        let response = service
            .handle(SimulationAgentRequestV1::DiscoverCapabilities {
                schema: SIMULATION_AGENT_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 1,
                expected_revision: 0,
            })
            .unwrap();
        let mut forged = response.clone();
        match &mut forged {
            SimulationAgentResponseV1::Ok {
                response_identity, ..
            }
            | SimulationAgentResponseV1::Error {
                response_identity, ..
            } => *response_identity = VirtualEvidenceIdentityV1::new([99; 32]).unwrap(),
        }
        assert_eq!(
            service.encode_response(&forged).unwrap_err(),
            SimulationAgentServiceErrorV1::InvalidResponse
        );
    }

    #[test]
    fn truncated_json_line_is_typed_as_invalid_not_oversized() {
        let mut input = std::io::Cursor::new(br#"{"operation":"discover_capabilities""#);
        let mut output = Vec::new();
        assert_eq!(
            run_simulation_agent_jsonl_v1(&mut input, &mut output).unwrap_err(),
            SimulationAgentServiceErrorV1::InvalidRequest
        );
        let response: SimulationAgentResponseV1 =
            serde_json::from_slice(output.strip_suffix(b"\n").unwrap()).unwrap();
        assert!(matches!(
            response,
            SimulationAgentResponseV1::Error {
                code: SimulationAgentErrorCodeV1::InvalidRequest,
                terminal: true,
                ..
            }
        ));
    }
}
