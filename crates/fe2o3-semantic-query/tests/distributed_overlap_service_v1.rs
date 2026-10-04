use std::io::{BufRead, BufReader, Cursor, Write};
use std::process::{Command, Stdio};

use fe2o3_semantic_import::CaptureIdentityV1;
use fe2o3_semantic_query::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const PARENT_V1_CAPABILITY_BYTES: usize = 4_562;
const PARENT_V1_CAPABILITY_SHA256: &str =
    "339eb56fdd6b6ee147d2f25ab4aa66f401d0a515db3c40499ed5c1bdafa4a847";

fn lower_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").unwrap();
    }
    output
}

fn dependency_capability(
    result: &AgentProfilerDistributedOverlapServiceResultV1,
) -> AgentProfilerDistributedOverlapCapabilityV1 {
    let AgentProfilerDistributedOverlapServiceResultV1::Capabilities { capabilities, .. } = result
    else {
        panic!("expected extension capabilities")
    };
    capabilities
        .iter()
        .find(|capability| {
            capability.operation
                == AgentProfilerDistributedOverlapOperationV1::ExplainDistributedOverlap
        })
        .copied()
        .unwrap()
}

fn response_value(
    response: &AgentProfilerDistributedOverlapServiceResponseV1,
) -> &AgentProfilerDistributedOverlapServiceResultV1 {
    let AgentProfilerDistributedOverlapServiceResponseV1::Ok { value, .. } = response else {
        panic!("expected successful extension response")
    };
    value
}

fn explain_request(
    request_id: u64,
    capability: AgentProfilerDistributedOverlapCapabilityV1,
) -> AgentProfilerDistributedOverlapServiceRequestV1 {
    AgentProfilerDistributedOverlapServiceRequestV1::ExplainDistributedOverlap {
        schema: capability.request_schema.to_owned(),
        request_id,
        dependency_contract_version: capability.dependency_contract_version.unwrap(),
        dependency_contract_identity: capability.dependency_contract_identity.unwrap(),
    }
}

#[test]
fn frozen_v1_capability_wire_matches_the_independently_measured_parent() {
    assert_eq!(AgentProfilerOperationV1::ALL.len(), 23);
    let mut service = AgentProfilerServiceV1::new(AgentProfilerServiceLimitsV1::default()).unwrap();
    let response = service.handle(AgentProfilerRequestV1::DiscoverCapabilities {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.to_owned(),
        request_id: 1,
    });
    let encoded = service.encode_response(&response).unwrap();
    assert_eq!(encoded.len(), PARENT_V1_CAPABILITY_BYTES);
    let digest = Sha256::digest(&encoded);
    assert_eq!(lower_hex(&digest), PARENT_V1_CAPABILITY_SHA256);
    let wire: Value = serde_json::from_slice(&encoded).unwrap();
    assert!(
        wire["value"]["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .all(|capability| {
                capability.get("dependency_contract_version").is_none()
                    && capability.get("dependency_contract_identity").is_none()
            })
    );
}

#[test]
fn extension_discovery_constructs_a_bounded_metadata_only_explain_request() {
    let mut service = AgentProfilerDistributedOverlapServiceV1::new().unwrap();
    let discovery = service
        .handle(
            AgentProfilerDistributedOverlapServiceRequestV1::DiscoverCapabilities {
                schema: AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 1,
            },
        )
        .unwrap();
    let encoded_discovery = service.encode_response(&discovery).unwrap();
    assert!(
        encoded_discovery.len()
            <= MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_RESPONSE_BYTES_V1 as usize
    );
    let capability = dependency_capability(response_value(&discovery));
    assert_eq!(
        capability.state,
        AgentProfilerDistributedOverlapCapabilityStateV1::Unavailable
    );
    assert_eq!(
        capability.unavailable_reason,
        Some(AgentProfilerDistributedOverlapUnavailableReasonV1::Issue182InputNotAdmitted)
    );
    assert_eq!(
        capability.response_schema,
        AGENT_PROFILER_DISTRIBUTED_OVERLAP_RESPONSE_SCHEMA_V1
    );
    assert_eq!(
        capability.result_schema,
        Some(AGENT_PROFILER_DISTRIBUTED_OVERLAP_RESULT_SCHEMA_V1)
    );

    let explained = service.handle(explain_request(2, capability)).unwrap();
    let AgentProfilerDistributedOverlapServiceResultV1::ConsumerRequirements {
        requirements, ..
    } = response_value(&explained)
    else {
        panic!("expected consumer requirements")
    };
    assert_eq!(
        requirements.classification,
        AgentProfilerDistributedOverlapClassificationV1::ConsumerRequirementsMetadataOnly
    );
    assert_eq!(
        requirements.dependency_contract.owner.repository,
        AGENT_PROFILER_DISTRIBUTED_OVERLAP_OWNER_REPOSITORY_V1
    );
    assert_eq!(
        requirements.dependency_contract.owner.issue,
        AGENT_PROFILER_DISTRIBUTED_OVERLAP_OWNER_ISSUE_V1
    );
    assert_eq!(
        requirements.dependency_contract.required_axes.len(),
        MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUIRED_AXES_V1
    );
    for required in [
        AgentProfilerDistributedOverlapRequiredAxisV1::DirectedDependencyEdgeIdentity,
        AgentProfilerDistributedOverlapRequiredAxisV1::PredecessorOperationIdentity,
        AgentProfilerDistributedOverlapRequiredAxisV1::SuccessorOperationIdentity,
    ] {
        assert!(
            requirements
                .dependency_contract
                .required_axes
                .contains(&required)
        );
    }
    assert_eq!(
        requirements.dependency_contract.accepted_loss_states,
        [
            AgentProfilerDistributedOverlapLossStateV1::ReportedWithOriginAndLostRecordCount,
            AgentProfilerDistributedOverlapLossStateV1::UnknownWithOriginAndUnavailableReason,
        ]
    );
    assert_eq!(
        requirements.global_time_precision,
        AgentProfilerDistributedOverlapGlobalTimeStatusV1::UnavailableWithoutAdmittedCorrelationIntervalUncertaintyAndPrecision
    );
    assert_eq!(
        requirements.causal_localization,
        AgentProfilerDistributedOverlapCausalLocalizationStatusV1::UnavailableWithoutCompleteAdmittedDependencyAndPhaseEvidence
    );
    assert_eq!(
        requirements.t5_status,
        AgentProfilerDistributedOverlapT5StatusV1::NotClaimedBlockedOnIssue182Producer
    );
    let encoded = service.encode_response(&explained).unwrap();
    assert!(encoded.len() <= MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_RESPONSE_BYTES_V1 as usize);
    let text = String::from_utf8(encoded).unwrap();
    for forbidden in [
        "captures",
        "records",
        "pid",
        "path",
        "address",
        "command",
        "execution_authority",
    ] {
        assert!(!text.contains(forbidden), "leaked {forbidden}");
    }
}

#[test]
fn semantic_validation_rejects_axis_owner_and_contract_identity_substitution() {
    let mut service = AgentProfilerDistributedOverlapServiceV1::new().unwrap();
    let discovery = service
        .handle(
            AgentProfilerDistributedOverlapServiceRequestV1::DiscoverCapabilities {
                schema: AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 1,
            },
        )
        .unwrap();
    let capability = dependency_capability(response_value(&discovery));
    let explained = service.handle(explain_request(2, capability)).unwrap();
    let expected = response_value(&explained);
    assert!(service.validate_result(expected).is_ok());

    let mut missing_axis = expected.clone();
    let AgentProfilerDistributedOverlapServiceResultV1::ConsumerRequirements {
        requirements, ..
    } = &mut missing_axis
    else {
        unreachable!()
    };
    requirements.dependency_contract.required_axes.pop();
    assert_eq!(
        service.validate_result(&missing_axis),
        Err(AgentProfilerDistributedOverlapServiceErrorV1::InvalidResponse)
    );

    let mut substituted_owner = expected.clone();
    let AgentProfilerDistributedOverlapServiceResultV1::ConsumerRequirements {
        requirements, ..
    } = &mut substituted_owner
    else {
        unreachable!()
    };
    requirements.dependency_contract.owner.issue += 1;
    assert_eq!(
        service.validate_result(&substituted_owner),
        Err(AgentProfilerDistributedOverlapServiceErrorV1::InvalidResponse)
    );

    let mut aliased_identity = expected.clone();
    let AgentProfilerDistributedOverlapServiceResultV1::ConsumerRequirements {
        requirements, ..
    } = &mut aliased_identity
    else {
        unreachable!()
    };
    requirements.dependency_contract.identity.canonical_len += 1;
    assert_eq!(
        service.validate_result(&aliased_identity),
        Err(AgentProfilerDistributedOverlapServiceErrorV1::InvalidResponse)
    );

    let other_service = AgentProfilerDistributedOverlapServiceV1::new().unwrap();
    assert_eq!(
        other_service.encode_response(&explained),
        Err(AgentProfilerDistributedOverlapServiceErrorV1::InvalidResponse)
    );
}

#[test]
fn request_contract_aliases_and_unknown_fields_fail_closed() {
    let mut service = AgentProfilerDistributedOverlapServiceV1::new().unwrap();
    let discovery = service
        .handle(
            AgentProfilerDistributedOverlapServiceRequestV1::DiscoverCapabilities {
                schema: AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 1,
            },
        )
        .unwrap();
    let capability = dependency_capability(response_value(&discovery));

    let mut version = explain_request(2, capability);
    let AgentProfilerDistributedOverlapServiceRequestV1::ExplainDistributedOverlap {
        dependency_contract_version,
        ..
    } = &mut version
    else {
        unreachable!()
    };
    *dependency_contract_version += 1;
    assert!(matches!(
        service.handle(version).unwrap(),
        AgentProfilerDistributedOverlapServiceResponseV1::Error {
            code: AgentProfilerDistributedOverlapErrorCodeV1::InvalidDependencyContract,
            ..
        }
    ));

    let mut length = explain_request(3, capability);
    let AgentProfilerDistributedOverlapServiceRequestV1::ExplainDistributedOverlap {
        dependency_contract_identity,
        ..
    } = &mut length
    else {
        unreachable!()
    };
    dependency_contract_identity.canonical_len += 1;
    assert!(matches!(
        service.handle(length).unwrap(),
        AgentProfilerDistributedOverlapServiceResponseV1::Error {
            code: AgentProfilerDistributedOverlapErrorCodeV1::InvalidDependencyContract,
            ..
        }
    ));

    let mut digest = explain_request(4, capability);
    let AgentProfilerDistributedOverlapServiceRequestV1::ExplainDistributedOverlap {
        dependency_contract_identity,
        ..
    } = &mut digest
    else {
        unreachable!()
    };
    dependency_contract_identity.digest = CaptureIdentityV1::new([99; 32]).unwrap();
    assert!(matches!(
        service.handle(digest).unwrap(),
        AgentProfilerDistributedOverlapServiceResponseV1::Error {
            code: AgentProfilerDistributedOverlapErrorCodeV1::InvalidDependencyContract,
            ..
        }
    ));

    assert!(decode_agent_profiler_distributed_overlap_request_line_v1(
        br#"{"operation":"discover_capabilities","schema":"fe2o3-agent-profiler-distributed-overlap-request-v1","request_id":5,"dependency":{}}
"#
    )
    .is_err());
    let mut oversized = Cursor::new(vec![
        b'x';
        MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUEST_BYTES_V1
            as usize
            + 1
    ]);
    assert_eq!(
        read_agent_profiler_distributed_overlap_request_line_v1(&mut oversized),
        Err(AgentProfilerDistributedOverlapServiceErrorV1::RequestTooLarge)
    );
}

#[test]
fn jsonl_extension_discovery_is_sufficient_for_a_fresh_client() {
    let executable = env!("CARGO_BIN_EXE_fe2o3-profiler-service");
    let mut child = Command::new(executable)
        .arg("distributed-overlap-jsonl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());

    writeln!(
        input,
        "{}",
        json!({
            "operation": "discover_capabilities",
            "schema": AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUEST_SCHEMA_V1,
            "request_id": 1,
        })
    )
    .unwrap();
    input.flush().unwrap();
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    assert!(line.len() <= MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_RESPONSE_BYTES_V1 as usize);
    let discovered: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(
        discovered["schema"],
        AGENT_PROFILER_DISTRIBUTED_OVERLAP_RESPONSE_SCHEMA_V1
    );
    let capability = discovered["value"]["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capability| capability["operation"] == "explain_distributed_overlap")
        .unwrap();

    writeln!(
        input,
        "{}",
        json!({
            "operation": "explain_distributed_overlap",
            "schema": capability["request_schema"],
            "request_id": 2,
            "dependency_contract_version": capability["dependency_contract_version"],
            "dependency_contract_identity": capability["dependency_contract_identity"],
        })
    )
    .unwrap();
    input.flush().unwrap();
    line.clear();
    output.read_line(&mut line).unwrap();
    assert!(line.len() <= MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_RESPONSE_BYTES_V1 as usize);
    let explained: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(explained["status"], "ok");
    assert_eq!(explained["value"]["result"], "consumer_requirements");
    assert_eq!(
        explained["value"]["requirements"]["classification"],
        "consumer_requirements_metadata_only"
    );
    assert_eq!(
        explained["value"]["requirements"]["dependency_contract"]["required_axes"]
            .as_array()
            .unwrap()
            .len(),
        MAX_AGENT_PROFILER_DISTRIBUTED_OVERLAP_REQUIRED_AXES_V1
    );
    assert!(explained["value"].get("evidence").is_none());

    drop(input);
    let status = child.wait().unwrap();
    assert_eq!(status.code(), Some(0));
}

#[test]
fn truth_contract_preserves_measured_inferred_and_clock_boundaries() {
    let contract = agent_profiler_distributed_overlap_dependency_contract_v1().unwrap();
    assert!(contract.truth_boundaries.contains(
        &AgentProfilerDistributedOverlapTruthBoundaryV1::MeasuredIntervalsRequireObservedOrigin
    ));
    assert!(contract.truth_boundaries.contains(
        &AgentProfilerDistributedOverlapTruthBoundaryV1::ProducerInferencesRequireRuleIdentityAndInputEvidence
    ));
    assert!(contract.truth_boundaries.contains(
        &AgentProfilerDistributedOverlapTruthBoundaryV1::OverlapQuantificationWouldBeInferredFromAdmittedInputs
    ));
    assert_eq!(
        contract.clock_requirements.correlation_interval,
        AgentProfilerDistributedOverlapClockFieldV1::RequiredFromIssue182Producer
    );
    assert_eq!(
        contract
            .clock_requirements
            .correlation_uncertainty_and_precision,
        AgentProfilerDistributedOverlapClockFieldV1::RequiredFromIssue182Producer
    );
}
