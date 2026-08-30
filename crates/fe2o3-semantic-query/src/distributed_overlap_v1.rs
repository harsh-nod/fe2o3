use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_semantic_import::{CaptureIdentityV1, ContentIdentityRecordV1, ContentSchemeV1};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const AGENT_PROFILER_DISTRIBUTED_OVERLAP_DEPENDENCY_SCHEMA_V1: &str =
    "fe2o3-agent-profiler-distributed-overlap-dependency-v1";
pub const AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUEST_SCHEMA_V1: &str =
    "fe2o3-agent-profiler-distributed-overlap-request-v1";
pub const AGENT_PROFILER_DISTRIBUTED_OVERLAP_RESULT_SCHEMA_V1: &str =
    "fe2o3-agent-profiler-distributed-overlap-consumer-requirements-v1";
pub const AGENT_PROFILER_DISTRIBUTED_OVERLAP_RESPONSE_SCHEMA_V1: &str =
    "fe2o3-agent-profiler-distributed-overlap-response-v1";
pub const AGENT_PROFILER_DISTRIBUTED_OVERLAP_CONTRACT_VERSION_V1: u16 = 1;
pub const AGENT_PROFILER_DISTRIBUTED_OVERLAP_OWNER_REPOSITORY_V1: &str = "harsh-nod/fe2o3";
pub const AGENT_PROFILER_DISTRIBUTED_OVERLAP_OWNER_ISSUE_V1: u64 = 182;
pub const MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUIRED_AXES_V1: usize = 18;
pub const MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_CONTRACT_BYTES_V1: u64 = 16 * 1024;
pub const MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUEST_BYTES_V1: u64 = 16 * 1024;
pub const MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_RESPONSE_BYTES_V1: u64 = 4 * 1024;
pub const MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUESTS_V1: u32 = 4_096;

const DISTRIBUTED_OVERLAP_DEPENDENCY_DOMAIN_V1: &[u8] =
    b"fe2o3.agent-profiler.distributed-overlap-dependency.v1\0";
const DISTRIBUTED_OVERLAP_SERVICE_CONTRACT_DOMAIN_V1: &[u8] =
    b"fe2o3.agent-profiler.distributed-overlap-service.v1\0";
const DISTRIBUTED_OVERLAP_SERVICE_CONTRACT_BYTES_V1: &[u8] = b"extension-v1;discover-capabilities;explain-distributed-overlap;dependency-contract-v1;metadata-only;4096-byte-response;no-execution-authority;no-t5-exit";
const DISTRIBUTED_OVERLAP_RESPONSE_BINDING_DOMAIN_V1: &[u8] =
    b"fe2o3.agent-profiler.distributed-overlap-response-binding.v1\0";

static NEXT_DISTRIBUTED_OVERLAP_SERVICE_INSTANCE_V1: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerDistributedOverlapOwnerV1 {
    pub repository: &'static str,
    pub issue: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerDistributedOverlapRequiredAxisV1 {
    OperationIdentity,
    DirectedDependencyEdgeIdentity,
    PredecessorOperationIdentity,
    SuccessorOperationIdentity,
    NodeIdentity,
    DeviceIdentity,
    QueueIdentity,
    ComputeIntervalOrPhase,
    CopyIntervalOrPhase,
    TransferIntervalOrPhase,
    CollectiveIntervalOrPhase,
    LocalClockDomainIdentity,
    ClockCorrelationInterval,
    ClockCorrelationUncertaintyAndPrecision,
    LossStatus,
    CompletenessStatus,
    EvidenceContentIdentity,
    EvidenceSchemaVersion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerDistributedOverlapTruthBoundaryV1 {
    MeasuredIntervalsRequireObservedOrigin,
    ProducerInferencesRequireRuleIdentityAndInputEvidence,
    OverlapQuantificationWouldBeInferredFromAdmittedInputs,
    MissingEventsDoNotEstablishIdleCompletionOrCausality,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerDistributedOverlapLossStateV1 {
    ReportedWithOriginAndLostRecordCount,
    UnknownWithOriginAndUnavailableReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerDistributedOverlapCompletenessStateV1 {
    CompleteWithOriginAndScope,
    PartialWithOriginAndScope,
    UnknownWithOriginAndUnavailableReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerDistributedOverlapClockFieldV1 {
    RequiredFromIssue182Producer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerDistributedOverlapGlobalTimeStatusV1 {
    UnavailableWithoutAdmittedCorrelationIntervalUncertaintyAndPrecision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerDistributedOverlapClockRequirementsV1 {
    pub local_clock_domain: AgentProfilerDistributedOverlapClockFieldV1,
    pub correlation_interval: AgentProfilerDistributedOverlapClockFieldV1,
    pub correlation_uncertainty_and_precision: AgentProfilerDistributedOverlapClockFieldV1,
    pub global_time_precision: AgentProfilerDistributedOverlapGlobalTimeStatusV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerDistributedOverlapCausalLocalizationStatusV1 {
    UnavailableWithoutCompleteAdmittedDependencyAndPhaseEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerDistributedOverlapAdmissionStatusV1 {
    AwaitingIssue182TypedProducer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerDistributedOverlapClassificationV1 {
    ConsumerRequirementsMetadataOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerDistributedOverlapAuthorityV1 {
    ReadOnlyNoExecutionAttachSchedulingOrCollectionAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerDistributedOverlapT5StatusV1 {
    NotClaimedBlockedOnIssue182Producer,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerDistributedOverlapDependencyContractV1 {
    pub schema: &'static str,
    pub contract_version: u16,
    pub owner: AgentProfilerDistributedOverlapOwnerV1,
    pub identity: ContentIdentityRecordV1,
    pub admission: AgentProfilerDistributedOverlapAdmissionStatusV1,
    pub required_axes: Vec<AgentProfilerDistributedOverlapRequiredAxisV1>,
    pub truth_boundaries: Vec<AgentProfilerDistributedOverlapTruthBoundaryV1>,
    pub clock_requirements: AgentProfilerDistributedOverlapClockRequirementsV1,
    pub accepted_loss_states: Vec<AgentProfilerDistributedOverlapLossStateV1>,
    pub accepted_completeness_states: Vec<AgentProfilerDistributedOverlapCompletenessStateV1>,
    pub authority: AgentProfilerDistributedOverlapAuthorityV1,
}

#[derive(Serialize)]
struct AgentProfilerDistributedOverlapDependencyPreimageV1<'a> {
    schema: &'a str,
    contract_version: u16,
    owner: &'a AgentProfilerDistributedOverlapOwnerV1,
    admission: AgentProfilerDistributedOverlapAdmissionStatusV1,
    required_axes: &'a [AgentProfilerDistributedOverlapRequiredAxisV1],
    truth_boundaries: &'a [AgentProfilerDistributedOverlapTruthBoundaryV1],
    clock_requirements: AgentProfilerDistributedOverlapClockRequirementsV1,
    accepted_loss_states: &'a [AgentProfilerDistributedOverlapLossStateV1],
    accepted_completeness_states: &'a [AgentProfilerDistributedOverlapCompletenessStateV1],
    authority: AgentProfilerDistributedOverlapAuthorityV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerDistributedOverlapConsumerRequirementsV1 {
    pub schema: &'static str,
    pub classification: AgentProfilerDistributedOverlapClassificationV1,
    pub dependency_contract: AgentProfilerDistributedOverlapDependencyContractV1,
    pub admission: AgentProfilerDistributedOverlapAdmissionStatusV1,
    pub global_time_precision: AgentProfilerDistributedOverlapGlobalTimeStatusV1,
    pub causal_localization: AgentProfilerDistributedOverlapCausalLocalizationStatusV1,
    pub t5_status: AgentProfilerDistributedOverlapT5StatusV1,
}

impl AgentProfilerDistributedOverlapConsumerRequirementsV1 {
    pub(crate) fn new(
        dependency_contract: AgentProfilerDistributedOverlapDependencyContractV1,
    ) -> Self {
        Self {
            schema: AGENT_PROFILER_DISTRIBUTED_OVERLAP_RESULT_SCHEMA_V1,
            classification:
                AgentProfilerDistributedOverlapClassificationV1::ConsumerRequirementsMetadataOnly,
            admission: dependency_contract.admission,
            global_time_precision: dependency_contract.clock_requirements.global_time_precision,
            causal_localization: AgentProfilerDistributedOverlapCausalLocalizationStatusV1::UnavailableWithoutCompleteAdmittedDependencyAndPhaseEvidence,
            dependency_contract,
            t5_status:
                AgentProfilerDistributedOverlapT5StatusV1::NotClaimedBlockedOnIssue182Producer,
        }
    }
}

pub fn agent_profiler_distributed_overlap_dependency_contract_v1() -> Result<
    AgentProfilerDistributedOverlapDependencyContractV1,
    AgentProfilerDistributedOverlapContractErrorV1,
> {
    let owner = AgentProfilerDistributedOverlapOwnerV1 {
        repository: AGENT_PROFILER_DISTRIBUTED_OVERLAP_OWNER_REPOSITORY_V1,
        issue: AGENT_PROFILER_DISTRIBUTED_OVERLAP_OWNER_ISSUE_V1,
    };
    let required_axes = vec![
        AgentProfilerDistributedOverlapRequiredAxisV1::OperationIdentity,
        AgentProfilerDistributedOverlapRequiredAxisV1::DirectedDependencyEdgeIdentity,
        AgentProfilerDistributedOverlapRequiredAxisV1::PredecessorOperationIdentity,
        AgentProfilerDistributedOverlapRequiredAxisV1::SuccessorOperationIdentity,
        AgentProfilerDistributedOverlapRequiredAxisV1::NodeIdentity,
        AgentProfilerDistributedOverlapRequiredAxisV1::DeviceIdentity,
        AgentProfilerDistributedOverlapRequiredAxisV1::QueueIdentity,
        AgentProfilerDistributedOverlapRequiredAxisV1::ComputeIntervalOrPhase,
        AgentProfilerDistributedOverlapRequiredAxisV1::CopyIntervalOrPhase,
        AgentProfilerDistributedOverlapRequiredAxisV1::TransferIntervalOrPhase,
        AgentProfilerDistributedOverlapRequiredAxisV1::CollectiveIntervalOrPhase,
        AgentProfilerDistributedOverlapRequiredAxisV1::LocalClockDomainIdentity,
        AgentProfilerDistributedOverlapRequiredAxisV1::ClockCorrelationInterval,
        AgentProfilerDistributedOverlapRequiredAxisV1::ClockCorrelationUncertaintyAndPrecision,
        AgentProfilerDistributedOverlapRequiredAxisV1::LossStatus,
        AgentProfilerDistributedOverlapRequiredAxisV1::CompletenessStatus,
        AgentProfilerDistributedOverlapRequiredAxisV1::EvidenceContentIdentity,
        AgentProfilerDistributedOverlapRequiredAxisV1::EvidenceSchemaVersion,
    ];
    if required_axes.len() != MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUIRED_AXES_V1 {
        return Err(AgentProfilerDistributedOverlapContractErrorV1::ResourceLimit);
    }
    let truth_boundaries = vec![
        AgentProfilerDistributedOverlapTruthBoundaryV1::MeasuredIntervalsRequireObservedOrigin,
        AgentProfilerDistributedOverlapTruthBoundaryV1::ProducerInferencesRequireRuleIdentityAndInputEvidence,
        AgentProfilerDistributedOverlapTruthBoundaryV1::OverlapQuantificationWouldBeInferredFromAdmittedInputs,
        AgentProfilerDistributedOverlapTruthBoundaryV1::MissingEventsDoNotEstablishIdleCompletionOrCausality,
    ];
    let clock_requirements = AgentProfilerDistributedOverlapClockRequirementsV1 {
        local_clock_domain:
            AgentProfilerDistributedOverlapClockFieldV1::RequiredFromIssue182Producer,
        correlation_interval:
            AgentProfilerDistributedOverlapClockFieldV1::RequiredFromIssue182Producer,
        correlation_uncertainty_and_precision:
            AgentProfilerDistributedOverlapClockFieldV1::RequiredFromIssue182Producer,
        global_time_precision: AgentProfilerDistributedOverlapGlobalTimeStatusV1::UnavailableWithoutAdmittedCorrelationIntervalUncertaintyAndPrecision,
    };
    let accepted_loss_states = vec![
        AgentProfilerDistributedOverlapLossStateV1::ReportedWithOriginAndLostRecordCount,
        AgentProfilerDistributedOverlapLossStateV1::UnknownWithOriginAndUnavailableReason,
    ];
    let accepted_completeness_states = vec![
        AgentProfilerDistributedOverlapCompletenessStateV1::CompleteWithOriginAndScope,
        AgentProfilerDistributedOverlapCompletenessStateV1::PartialWithOriginAndScope,
        AgentProfilerDistributedOverlapCompletenessStateV1::UnknownWithOriginAndUnavailableReason,
    ];
    let admission = AgentProfilerDistributedOverlapAdmissionStatusV1::AwaitingIssue182TypedProducer;
    let authority = AgentProfilerDistributedOverlapAuthorityV1::ReadOnlyNoExecutionAttachSchedulingOrCollectionAuthority;
    let preimage = AgentProfilerDistributedOverlapDependencyPreimageV1 {
        schema: AGENT_PROFILER_DISTRIBUTED_OVERLAP_DEPENDENCY_SCHEMA_V1,
        contract_version: AGENT_PROFILER_DISTRIBUTED_OVERLAP_CONTRACT_VERSION_V1,
        owner: &owner,
        admission,
        required_axes: &required_axes,
        truth_boundaries: &truth_boundaries,
        clock_requirements,
        accepted_loss_states: &accepted_loss_states,
        accepted_completeness_states: &accepted_completeness_states,
        authority,
    };
    let canonical = serde_json::to_vec(&preimage)
        .map_err(|_| AgentProfilerDistributedOverlapContractErrorV1::Encoding)?;
    if canonical.len() as u64 > MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_CONTRACT_BYTES_V1 {
        return Err(AgentProfilerDistributedOverlapContractErrorV1::ResourceLimit);
    }
    let mut hasher = Sha256::new();
    hasher.update(DISTRIBUTED_OVERLAP_DEPENDENCY_DOMAIN_V1);
    hasher.update(&canonical);
    let digest = CaptureIdentityV1::new(hasher.finalize().into())
        .map_err(|_| AgentProfilerDistributedOverlapContractErrorV1::Identity)?;
    Ok(AgentProfilerDistributedOverlapDependencyContractV1 {
        schema: AGENT_PROFILER_DISTRIBUTED_OVERLAP_DEPENDENCY_SCHEMA_V1,
        contract_version: AGENT_PROFILER_DISTRIBUTED_OVERLAP_CONTRACT_VERSION_V1,
        owner,
        identity: ContentIdentityRecordV1 {
            scheme: ContentSchemeV1::DomainSeparatedSha256,
            format_version: AGENT_PROFILER_DISTRIBUTED_OVERLAP_CONTRACT_VERSION_V1,
            digest,
            canonical_len: canonical.len() as u64,
        },
        admission,
        required_axes,
        truth_boundaries,
        clock_requirements,
        accepted_loss_states,
        accepted_completeness_states,
        authority,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentProfilerDistributedOverlapContractErrorV1 {
    Encoding,
    Identity,
    ResourceLimit,
}

impl fmt::Display for AgentProfilerDistributedOverlapContractErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "distributed-overlap dependency contract construction failed: {self:?}"
        )
    }
}

impl Error for AgentProfilerDistributedOverlapContractErrorV1 {}

pub const AGENT_PROFILER_DISTRIBUTED_OVERLAP_CAPABILITY_SCHEMA_V1: &str =
    "fe2o3-agent-profiler-distributed-overlap-capability-v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerDistributedOverlapOperationV1 {
    DiscoverCapabilities,
    ExplainDistributedOverlap,
}

impl AgentProfilerDistributedOverlapOperationV1 {
    pub const ALL: [Self; 2] = [Self::DiscoverCapabilities, Self::ExplainDistributedOverlap];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerDistributedOverlapCapabilityStateV1 {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerDistributedOverlapUnavailableReasonV1 {
    Issue182InputNotAdmitted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerDistributedOverlapCapabilityV1 {
    pub schema: &'static str,
    pub operation: AgentProfilerDistributedOverlapOperationV1,
    pub state: AgentProfilerDistributedOverlapCapabilityStateV1,
    pub unavailable_reason: Option<AgentProfilerDistributedOverlapUnavailableReasonV1>,
    pub request_schema: &'static str,
    pub response_schema: &'static str,
    pub result_schema: Option<&'static str>,
    pub dependency_contract_version: Option<u16>,
    pub dependency_contract_identity: Option<ContentIdentityRecordV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerDistributedOverlapLimitsV1 {
    pub max_request_bytes: u64,
    pub max_response_bytes: u64,
    pub max_requests: u32,
    pub max_required_axes: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentProfilerDistributedOverlapServiceRequestV1 {
    DiscoverCapabilities {
        schema: String,
        request_id: u64,
    },
    ExplainDistributedOverlap {
        schema: String,
        request_id: u64,
        dependency_contract_version: u16,
        dependency_contract_identity: ContentIdentityRecordV1,
    },
}

impl AgentProfilerDistributedOverlapServiceRequestV1 {
    pub fn request_id(&self) -> u64 {
        match self {
            Self::DiscoverCapabilities { request_id, .. }
            | Self::ExplainDistributedOverlap { request_id, .. } => *request_id,
        }
    }

    fn schema(&self) -> &str {
        match self {
            Self::DiscoverCapabilities { schema, .. }
            | Self::ExplainDistributedOverlap { schema, .. } => schema,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum AgentProfilerDistributedOverlapServiceResultV1 {
    Capabilities {
        capabilities: Vec<AgentProfilerDistributedOverlapCapabilityV1>,
        limits: AgentProfilerDistributedOverlapLimitsV1,
        service_contract: ContentIdentityRecordV1,
    },
    ConsumerRequirements {
        requirements: Box<AgentProfilerDistributedOverlapConsumerRequirementsV1>,
        service_contract: ContentIdentityRecordV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerDistributedOverlapErrorCodeV1 {
    InvalidRequest,
    InvalidSchema,
    InvalidRequestId,
    DuplicateRequestId,
    RequestBudgetExhausted,
    RequestTooLarge,
    InvalidDependencyContract,
    ResponseTooLarge,
    InternalContractMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentProfilerDistributedOverlapResponseBindingV1([u8; 32]);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AgentProfilerDistributedOverlapServiceResponseV1 {
    Ok {
        schema: &'static str,
        request_id: u64,
        response_revision: u64,
        value: Box<AgentProfilerDistributedOverlapServiceResultV1>,
        #[serde(skip)]
        state_binding: AgentProfilerDistributedOverlapResponseBindingV1,
    },
    Error {
        schema: &'static str,
        request_id: Option<u64>,
        response_revision: u64,
        code: AgentProfilerDistributedOverlapErrorCodeV1,
        terminal: bool,
        #[serde(skip)]
        state_binding: AgentProfilerDistributedOverlapResponseBindingV1,
    },
}

impl AgentProfilerDistributedOverlapServiceResponseV1 {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Error { terminal: true, .. })
    }
}

pub struct AgentProfilerDistributedOverlapServiceV1 {
    dependency_contract: AgentProfilerDistributedOverlapDependencyContractV1,
    service_contract: ContentIdentityRecordV1,
    instance: u64,
    request_ids: BTreeSet<u64>,
    request_count: u32,
    response_revision: u64,
    response_bindings: BTreeMap<u64, AgentProfilerDistributedOverlapResponseBindingV1>,
    terminal_response: Option<AgentProfilerDistributedOverlapServiceResponseV1>,
}

impl AgentProfilerDistributedOverlapServiceV1 {
    pub fn new() -> Result<Self, AgentProfilerDistributedOverlapServiceErrorV1> {
        let instance = NEXT_DISTRIBUTED_OVERLAP_SERVICE_INSTANCE_V1
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| AgentProfilerDistributedOverlapServiceErrorV1::InstanceExhausted)?;
        if instance == 0 {
            return Err(AgentProfilerDistributedOverlapServiceErrorV1::InstanceExhausted);
        }
        Ok(Self {
            dependency_contract: agent_profiler_distributed_overlap_dependency_contract_v1()
                .map_err(|_| AgentProfilerDistributedOverlapServiceErrorV1::Contract)?,
            service_contract: distributed_overlap_service_contract_identity_v1()?,
            instance,
            request_ids: BTreeSet::new(),
            request_count: 0,
            response_revision: 0,
            response_bindings: BTreeMap::new(),
            terminal_response: None,
        })
    }

    pub fn handle(
        &mut self,
        request: AgentProfilerDistributedOverlapServiceRequestV1,
    ) -> Result<
        AgentProfilerDistributedOverlapServiceResponseV1,
        AgentProfilerDistributedOverlapServiceErrorV1,
    > {
        if let Some(response) = &self.terminal_response {
            return Ok(response.clone());
        }
        let request_id = request.request_id();
        if self.request_count >= MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUESTS_V1 {
            return self.error(
                Some(request_id),
                AgentProfilerDistributedOverlapErrorCodeV1::RequestBudgetExhausted,
                true,
            );
        }
        self.request_count = self
            .request_count
            .checked_add(1)
            .ok_or(AgentProfilerDistributedOverlapServiceErrorV1::RevisionOverflow)?;
        if request_id == 0 {
            return self.error(
                None,
                AgentProfilerDistributedOverlapErrorCodeV1::InvalidRequestId,
                false,
            );
        }
        if !self.request_ids.insert(request_id) {
            return self.error(
                Some(request_id),
                AgentProfilerDistributedOverlapErrorCodeV1::DuplicateRequestId,
                false,
            );
        }
        if request.schema() != AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUEST_SCHEMA_V1 {
            return self.error(
                Some(request_id),
                AgentProfilerDistributedOverlapErrorCodeV1::InvalidSchema,
                false,
            );
        }
        let result = match request {
            AgentProfilerDistributedOverlapServiceRequestV1::DiscoverCapabilities { .. } => {
                AgentProfilerDistributedOverlapServiceResultV1::Capabilities {
                    capabilities: self.capabilities(),
                    limits: distributed_overlap_limits_v1(),
                    service_contract: self.service_contract,
                }
            }
            AgentProfilerDistributedOverlapServiceRequestV1::ExplainDistributedOverlap {
                dependency_contract_version,
                dependency_contract_identity,
                ..
            } => {
                if dependency_contract_version != self.dependency_contract.contract_version
                    || dependency_contract_identity != self.dependency_contract.identity
                {
                    return self.error(
                        Some(request_id),
                        AgentProfilerDistributedOverlapErrorCodeV1::InvalidDependencyContract,
                        false,
                    );
                }
                AgentProfilerDistributedOverlapServiceResultV1::ConsumerRequirements {
                    requirements: Box::new(
                        AgentProfilerDistributedOverlapConsumerRequirementsV1::new(
                            self.dependency_contract.clone(),
                        ),
                    ),
                    service_contract: self.service_contract,
                }
            }
        };
        self.ok(request_id, result)
    }

    pub fn validate_result(
        &self,
        result: &AgentProfilerDistributedOverlapServiceResultV1,
    ) -> Result<(), AgentProfilerDistributedOverlapServiceErrorV1> {
        match result {
            AgentProfilerDistributedOverlapServiceResultV1::Capabilities {
                capabilities,
                limits,
                service_contract,
            } => {
                if *capabilities != self.capabilities()
                    || *limits != distributed_overlap_limits_v1()
                    || *service_contract != self.service_contract
                {
                    return Err(AgentProfilerDistributedOverlapServiceErrorV1::InvalidResponse);
                }
            }
            AgentProfilerDistributedOverlapServiceResultV1::ConsumerRequirements {
                requirements,
                service_contract,
            } => {
                let expected = AgentProfilerDistributedOverlapConsumerRequirementsV1::new(
                    self.dependency_contract.clone(),
                );
                if requirements.as_ref() != &expected || *service_contract != self.service_contract
                {
                    return Err(AgentProfilerDistributedOverlapServiceErrorV1::InvalidResponse);
                }
            }
        }
        Ok(())
    }

    pub fn encode_response(
        &self,
        response: &AgentProfilerDistributedOverlapServiceResponseV1,
    ) -> Result<Vec<u8>, AgentProfilerDistributedOverlapServiceErrorV1> {
        self.validate_response(response)?;
        encode_distributed_overlap_response_bounded_v1(response)
    }

    pub fn terminal_protocol_error(
        &mut self,
        code: AgentProfilerDistributedOverlapErrorCodeV1,
    ) -> Result<
        AgentProfilerDistributedOverlapServiceResponseV1,
        AgentProfilerDistributedOverlapServiceErrorV1,
    > {
        if let Some(response) = &self.terminal_response {
            return Ok(response.clone());
        }
        if !matches!(
            code,
            AgentProfilerDistributedOverlapErrorCodeV1::InvalidRequest
                | AgentProfilerDistributedOverlapErrorCodeV1::RequestTooLarge
                | AgentProfilerDistributedOverlapErrorCodeV1::ResponseTooLarge
                | AgentProfilerDistributedOverlapErrorCodeV1::InternalContractMismatch
        ) {
            return Err(AgentProfilerDistributedOverlapServiceErrorV1::InvalidResponse);
        }
        self.error(None, code, true)
    }

    fn capabilities(&self) -> Vec<AgentProfilerDistributedOverlapCapabilityV1> {
        AgentProfilerDistributedOverlapOperationV1::ALL
            .into_iter()
            .map(|operation| {
                let explain =
                    operation == AgentProfilerDistributedOverlapOperationV1::ExplainDistributedOverlap;
                AgentProfilerDistributedOverlapCapabilityV1 {
                    schema: AGENT_PROFILER_DISTRIBUTED_OVERLAP_CAPABILITY_SCHEMA_V1,
                    operation,
                    state: if explain {
                        AgentProfilerDistributedOverlapCapabilityStateV1::Unavailable
                    } else {
                        AgentProfilerDistributedOverlapCapabilityStateV1::Available
                    },
                    unavailable_reason: explain.then_some(
                        AgentProfilerDistributedOverlapUnavailableReasonV1::Issue182InputNotAdmitted,
                    ),
                    request_schema: AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUEST_SCHEMA_V1,
                    response_schema: AGENT_PROFILER_DISTRIBUTED_OVERLAP_RESPONSE_SCHEMA_V1,
                    result_schema: explain
                        .then_some(AGENT_PROFILER_DISTRIBUTED_OVERLAP_RESULT_SCHEMA_V1),
                    dependency_contract_version: explain.then_some(
                        AGENT_PROFILER_DISTRIBUTED_OVERLAP_CONTRACT_VERSION_V1,
                    ),
                    dependency_contract_identity: explain
                        .then_some(self.dependency_contract.identity),
                }
            })
            .collect()
    }

    fn ok(
        &mut self,
        request_id: u64,
        value: AgentProfilerDistributedOverlapServiceResultV1,
    ) -> Result<
        AgentProfilerDistributedOverlapServiceResponseV1,
        AgentProfilerDistributedOverlapServiceErrorV1,
    > {
        let revision = self.next_revision()?;
        let mut response = AgentProfilerDistributedOverlapServiceResponseV1::Ok {
            schema: AGENT_PROFILER_DISTRIBUTED_OVERLAP_RESPONSE_SCHEMA_V1,
            request_id,
            response_revision: revision,
            value: Box::new(value),
            state_binding: AgentProfilerDistributedOverlapResponseBindingV1([0; 32]),
        };
        self.bind_response(&mut response);
        Ok(response)
    }

    fn error(
        &mut self,
        request_id: Option<u64>,
        code: AgentProfilerDistributedOverlapErrorCodeV1,
        terminal: bool,
    ) -> Result<
        AgentProfilerDistributedOverlapServiceResponseV1,
        AgentProfilerDistributedOverlapServiceErrorV1,
    > {
        let revision = self.next_revision()?;
        let mut response = AgentProfilerDistributedOverlapServiceResponseV1::Error {
            schema: AGENT_PROFILER_DISTRIBUTED_OVERLAP_RESPONSE_SCHEMA_V1,
            request_id,
            response_revision: revision,
            code,
            terminal,
            state_binding: AgentProfilerDistributedOverlapResponseBindingV1([0; 32]),
        };
        self.bind_response(&mut response);
        if terminal {
            self.terminal_response = Some(response.clone());
        }
        Ok(response)
    }

    fn next_revision(&mut self) -> Result<u64, AgentProfilerDistributedOverlapServiceErrorV1> {
        self.response_revision = self
            .response_revision
            .checked_add(1)
            .ok_or(AgentProfilerDistributedOverlapServiceErrorV1::RevisionOverflow)?;
        Ok(self.response_revision)
    }

    fn bind_response(&mut self, response: &mut AgentProfilerDistributedOverlapServiceResponseV1) {
        let binding = self.calculate_response_binding(response);
        let (revision, state_binding) = match response {
            AgentProfilerDistributedOverlapServiceResponseV1::Ok {
                response_revision,
                state_binding,
                ..
            }
            | AgentProfilerDistributedOverlapServiceResponseV1::Error {
                response_revision,
                state_binding,
                ..
            } => (*response_revision, state_binding),
        };
        *state_binding = binding;
        let previous = self.response_bindings.insert(revision, binding);
        debug_assert!(previous.is_none());
    }

    fn calculate_response_binding(
        &self,
        response: &AgentProfilerDistributedOverlapServiceResponseV1,
    ) -> AgentProfilerDistributedOverlapResponseBindingV1 {
        let mut hasher = Sha256::new();
        hasher.update(DISTRIBUTED_OVERLAP_RESPONSE_BINDING_DOMAIN_V1);
        hasher.update(self.instance.to_le_bytes());
        hasher.update(self.service_contract.digest.as_bytes());
        serde_json::to_writer(DistributedOverlapDigestWriterV1(&mut hasher), response)
            .expect("hash-backed distributed-overlap writer cannot fail");
        AgentProfilerDistributedOverlapResponseBindingV1(hasher.finalize().into())
    }

    fn validate_response(
        &self,
        response: &AgentProfilerDistributedOverlapServiceResponseV1,
    ) -> Result<(), AgentProfilerDistributedOverlapServiceErrorV1> {
        let (revision, binding) = match response {
            AgentProfilerDistributedOverlapServiceResponseV1::Ok {
                response_revision,
                state_binding,
                ..
            }
            | AgentProfilerDistributedOverlapServiceResponseV1::Error {
                response_revision,
                state_binding,
                ..
            } => (*response_revision, *state_binding),
        };
        if self.response_bindings.get(&revision) != Some(&binding)
            || self.calculate_response_binding(response) != binding
        {
            return Err(AgentProfilerDistributedOverlapServiceErrorV1::InvalidResponse);
        }
        match response {
            AgentProfilerDistributedOverlapServiceResponseV1::Ok {
                schema,
                request_id,
                response_revision,
                value,
                ..
            } => {
                if *schema != AGENT_PROFILER_DISTRIBUTED_OVERLAP_RESPONSE_SCHEMA_V1
                    || *request_id == 0
                    || *response_revision == 0
                {
                    return Err(AgentProfilerDistributedOverlapServiceErrorV1::InvalidResponse);
                }
                self.validate_result(value)?;
            }
            AgentProfilerDistributedOverlapServiceResponseV1::Error {
                schema,
                request_id,
                response_revision,
                code,
                terminal,
                ..
            } => {
                let terminal_valid = !*terminal
                    || matches!(
                        code,
                        AgentProfilerDistributedOverlapErrorCodeV1::RequestBudgetExhausted
                            | AgentProfilerDistributedOverlapErrorCodeV1::InvalidRequest
                            | AgentProfilerDistributedOverlapErrorCodeV1::RequestTooLarge
                            | AgentProfilerDistributedOverlapErrorCodeV1::ResponseTooLarge
                            | AgentProfilerDistributedOverlapErrorCodeV1::InternalContractMismatch
                    );
                if *schema != AGENT_PROFILER_DISTRIBUTED_OVERLAP_RESPONSE_SCHEMA_V1
                    || *response_revision == 0
                    || request_id.is_some_and(|id| id == 0)
                    || !terminal_valid
                {
                    return Err(AgentProfilerDistributedOverlapServiceErrorV1::InvalidResponse);
                }
            }
        }
        Ok(())
    }
}

impl Default for AgentProfilerDistributedOverlapServiceV1 {
    fn default() -> Self {
        Self::new().expect("fixed distributed-overlap contracts are valid")
    }
}

fn distributed_overlap_limits_v1() -> AgentProfilerDistributedOverlapLimitsV1 {
    AgentProfilerDistributedOverlapLimitsV1 {
        max_request_bytes: MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUEST_BYTES_V1,
        max_response_bytes: MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_RESPONSE_BYTES_V1,
        max_requests: MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUESTS_V1,
        max_required_axes: MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUIRED_AXES_V1 as u8,
    }
}

fn distributed_overlap_service_contract_identity_v1()
-> Result<ContentIdentityRecordV1, AgentProfilerDistributedOverlapServiceErrorV1> {
    let mut hasher = Sha256::new();
    hasher.update(DISTRIBUTED_OVERLAP_SERVICE_CONTRACT_DOMAIN_V1);
    hasher.update(DISTRIBUTED_OVERLAP_SERVICE_CONTRACT_BYTES_V1);
    let digest = CaptureIdentityV1::new(hasher.finalize().into())
        .map_err(|_| AgentProfilerDistributedOverlapServiceErrorV1::Identity)?;
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: 1,
        digest,
        canonical_len: DISTRIBUTED_OVERLAP_SERVICE_CONTRACT_BYTES_V1.len() as u64,
    })
}

pub fn decode_agent_profiler_distributed_overlap_request_line_v1(
    line: &[u8],
) -> Result<
    AgentProfilerDistributedOverlapServiceRequestV1,
    AgentProfilerDistributedOverlapServiceErrorV1,
> {
    if line.is_empty()
        || line.len() as u64 > MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUEST_BYTES_V1
    {
        return Err(AgentProfilerDistributedOverlapServiceErrorV1::RequestTooLarge);
    }
    let payload = line.strip_suffix(b"\n").unwrap_or(line);
    if payload.is_empty()
        || payload.contains(&b'\n')
        || payload.contains(&b'\r')
        || payload.len() as u64 >= MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUEST_BYTES_V1
    {
        return Err(AgentProfilerDistributedOverlapServiceErrorV1::InvalidRequest);
    }
    serde_json::from_slice(payload)
        .map_err(|_| AgentProfilerDistributedOverlapServiceErrorV1::InvalidRequest)
}

pub fn read_agent_profiler_distributed_overlap_request_line_v1<R: BufRead>(
    reader: &mut R,
) -> Result<Option<Vec<u8>>, AgentProfilerDistributedOverlapServiceErrorV1> {
    let mut line = Vec::new();
    let max = usize::try_from(MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUEST_BYTES_V1)
        .map_err(|_| AgentProfilerDistributedOverlapServiceErrorV1::SizeOverflow)?;
    loop {
        let buffer = reader
            .fill_buf()
            .map_err(|_| AgentProfilerDistributedOverlapServiceErrorV1::Io)?;
        if buffer.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err(AgentProfilerDistributedOverlapServiceErrorV1::InvalidRequest)
            };
        }
        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let available = newline.map_or(buffer.len(), |position| position + 1);
        let remaining = max.saturating_add(1).saturating_sub(line.len());
        let consumed = available.min(remaining);
        line.extend_from_slice(&buffer[..consumed]);
        reader.consume(consumed);
        if line.len() > max {
            return Err(AgentProfilerDistributedOverlapServiceErrorV1::RequestTooLarge);
        }
        if newline.is_some() && consumed == available {
            return Ok(Some(line));
        }
    }
}

pub fn run_agent_profiler_distributed_overlap_jsonl_v1<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
) -> Result<(), AgentProfilerDistributedOverlapServiceErrorV1> {
    let mut service = AgentProfilerDistributedOverlapServiceV1::new()?;
    loop {
        let line = match read_agent_profiler_distributed_overlap_request_line_v1(input) {
            Ok(Some(line)) => line,
            Ok(None) => return Ok(()),
            Err(AgentProfilerDistributedOverlapServiceErrorV1::RequestTooLarge) => {
                write_distributed_overlap_terminal_v1(
                    output,
                    &mut service,
                    AgentProfilerDistributedOverlapErrorCodeV1::RequestTooLarge,
                )?;
                return Err(AgentProfilerDistributedOverlapServiceErrorV1::ProtocolTerminated);
            }
            Err(_) => {
                write_distributed_overlap_terminal_v1(
                    output,
                    &mut service,
                    AgentProfilerDistributedOverlapErrorCodeV1::InvalidRequest,
                )?;
                return Err(AgentProfilerDistributedOverlapServiceErrorV1::ProtocolTerminated);
            }
        };
        let request = match decode_agent_profiler_distributed_overlap_request_line_v1(&line) {
            Ok(request) => request,
            Err(_) => {
                write_distributed_overlap_terminal_v1(
                    output,
                    &mut service,
                    AgentProfilerDistributedOverlapErrorCodeV1::InvalidRequest,
                )?;
                return Err(AgentProfilerDistributedOverlapServiceErrorV1::ProtocolTerminated);
            }
        };
        let response = service.handle(request)?;
        let terminal = response.is_terminal();
        let encoded = service.encode_response(&response)?;
        output
            .write_all(&encoded)
            .map_err(|_| AgentProfilerDistributedOverlapServiceErrorV1::Io)?;
        output
            .flush()
            .map_err(|_| AgentProfilerDistributedOverlapServiceErrorV1::Io)?;
        if terminal {
            return Err(AgentProfilerDistributedOverlapServiceErrorV1::ProtocolTerminated);
        }
    }
}

fn write_distributed_overlap_terminal_v1<W: Write>(
    output: &mut W,
    service: &mut AgentProfilerDistributedOverlapServiceV1,
    code: AgentProfilerDistributedOverlapErrorCodeV1,
) -> Result<(), AgentProfilerDistributedOverlapServiceErrorV1> {
    let response = service.terminal_protocol_error(code)?;
    let encoded = service.encode_response(&response)?;
    output
        .write_all(&encoded)
        .map_err(|_| AgentProfilerDistributedOverlapServiceErrorV1::Io)?;
    output
        .flush()
        .map_err(|_| AgentProfilerDistributedOverlapServiceErrorV1::Io)
}

fn encode_distributed_overlap_response_bounded_v1(
    response: &AgentProfilerDistributedOverlapServiceResponseV1,
) -> Result<Vec<u8>, AgentProfilerDistributedOverlapServiceErrorV1> {
    let mut output = Vec::new();
    let mut writer = DistributedOverlapBoundedWriterV1 {
        output: &mut output,
        max: MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_RESPONSE_BYTES_V1 - 1,
        exceeded: false,
    };
    serde_json::to_writer(&mut writer, response).map_err(|_| {
        if writer.exceeded {
            AgentProfilerDistributedOverlapServiceErrorV1::ResponseTooLarge
        } else {
            AgentProfilerDistributedOverlapServiceErrorV1::JsonEncode
        }
    })?;
    output.push(b'\n');
    Ok(output)
}

struct DistributedOverlapDigestWriterV1<'a>(&'a mut Sha256);

impl Write for DistributedOverlapDigestWriterV1<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct DistributedOverlapBoundedWriterV1<'a> {
    output: &'a mut Vec<u8>,
    max: u64,
    exceeded: bool,
}

impl Write for DistributedOverlapBoundedWriterV1<'_> {
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
            .is_none_or(|len| len > max)
        {
            self.exceeded = true;
            return Err(io::Error::other(
                "distributed-overlap response limit exceeded",
            ));
        }
        self.output.extend_from_slice(bytes);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentProfilerDistributedOverlapServiceErrorV1 {
    Contract,
    Identity,
    RequestTooLarge,
    InvalidRequest,
    InvalidResponse,
    ResponseTooLarge,
    JsonEncode,
    RevisionOverflow,
    SizeOverflow,
    InstanceExhausted,
    Io,
    ProtocolTerminated,
}

impl fmt::Display for AgentProfilerDistributedOverlapServiceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "distributed-overlap extension service rejected input: {self:?}"
        )
    }
}

impl Error for AgentProfilerDistributedOverlapServiceErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maximum_request_id_and_revision_fit_the_production_wire_bound() {
        let mut service = AgentProfilerDistributedOverlapServiceV1::new().unwrap();
        service.response_revision = u64::MAX - 1;
        let response = service
            .handle(
                AgentProfilerDistributedOverlapServiceRequestV1::DiscoverCapabilities {
                    schema: AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUEST_SCHEMA_V1.to_owned(),
                    request_id: u64::MAX,
                },
            )
            .unwrap();
        assert!(matches!(
            response,
            AgentProfilerDistributedOverlapServiceResponseV1::Ok {
                request_id: u64::MAX,
                response_revision: u64::MAX,
                ..
            }
        ));
        let encoded = service.encode_response(&response).unwrap();
        assert!(encoded.len() <= MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_RESPONSE_BYTES_V1 as usize);
    }
}
