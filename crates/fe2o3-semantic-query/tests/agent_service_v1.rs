use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Cursor, Write};
use std::process::{Command, Stdio};

use fe2o3_semantic_import::*;
use fe2o3_semantic_query::*;
use fe2o3_semantic_trace::*;
use serde_json::{Value, json};

const CSV: &[u8] =
    include_bytes!("../../fe2o3-semantic-import/tests/fixtures/rocprofv3-1.1-kernel-dispatch.csv");
const ATT: &[u8] = br#"{"counter_names":[],"gfxip":9,"gfxv":"vega","global_begin_time":0,"is_pcs_stochastic":false,"pc_sampling":false,"thread_trace":true,"version":"3.0.0","wave_filenames":{"0":{"0":{"0":{"0":["waves/se0.json",10,20]}}}},"se_filenames":["se0.json"]}"#;

fn opaque(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).unwrap()
}

fn content(byte: u8, len: u64) -> ContentIdentityRecordV1 {
    ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: 1,
        digest: CaptureIdentityV1::new([byte; 32]).unwrap(),
        canonical_len: len,
    }
}

fn binding(environment: u8) -> ProfilerDispatchBindingV4 {
    ProfilerDispatchBindingV4 {
        environment: ProfilerEnvironmentBindingV4 {
            environment: content(environment, 200),
            collector_tool: content(11, 50),
            collector_configuration: content(12, 80),
            stable_device_bindings: vec![
                ProfilerDeviceBindingV4 {
                    source_agent_id: 17,
                    stable_identity: content(20, 64),
                },
                ProfilerDeviceBindingV4 {
                    source_agent_id: 19,
                    stable_identity: content(21, 64),
                },
            ],
        },
        kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(opaque(1), 97).unwrap(),
        artifact: Some(ArtifactClaimV1 {
            identity: opaque(2),
            canonical_len: 4_096,
            format_version: 1,
        }),
        source_map: None,
        wave_width: WaveWidthV1::Wave64,
    }
}

fn bundle(environment: u8) -> Vec<u8> {
    encode_profiler_bundle_v4(
        &import_rocprofv3_csv_profiler_bundle_v4(CSV, binding(environment)).unwrap(),
    )
    .unwrap()
}

fn att_bundle() -> Vec<u8> {
    encode_profiler_bundle_v4(
        &import_rocprofv3_att_profiler_bundle_v4(
            ATT,
            ProfilerAttBindingV4 {
                environment: ProfilerEnvironmentBindingV4 {
                    environment: content(10, 200),
                    collector_tool: content(11, 50),
                    collector_configuration: content(12, 80),
                    stable_device_bindings: vec![ProfilerDeviceBindingV4 {
                        source_agent_id: 17,
                        stable_identity: content(20, 64),
                    }],
                },
                source_agent_id: 17,
                referenced_artifacts: vec![ProfilerAttArtifactBindingV4 {
                    reference: "se0.json".to_owned(),
                    content: content(30, 400),
                }],
            },
        )
        .unwrap(),
    )
    .unwrap()
}

fn homogeneous(origin: TruthOriginV1) -> AgentProfilerAggregateOriginV1 {
    AgentProfilerAggregateOriginV1::Homogeneous { origin }
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").unwrap();
    }
    output
}

fn open(
    service: &mut AgentProfilerServiceV1,
    request_id: u64,
    bytes: &[u8],
) -> (AgentProfilerResponseV1, ContentIdentityRecordV1) {
    let response = service.handle(AgentProfilerRequestV1::OpenCapture {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id,
        bundle_hex: lower_hex(bytes),
    });
    let AgentProfilerResponseV1::Ok { value, .. } = &response else {
        panic!("expected opened capture: {response:?}")
    };
    let AgentProfilerResultV1::CaptureOpened {
        context, evidence, ..
    } = value.as_ref()
    else {
        panic!("expected opened capture value")
    };
    assert_eq!(evidence.captures, [context.bundle_identity]);
    (response.clone(), context.bundle_identity)
}

fn assert_error(response: AgentProfilerResponseV1, expected: AgentProfilerErrorCodeV1) {
    assert!(matches!(
        response,
        AgentProfilerResponseV1::Error {
            code,
            terminal: false,
            ..
        } if code == expected
    ));
}

#[test]
fn capability_inventory_is_complete_read_only_and_evidence_bound() {
    let mut service = AgentProfilerServiceV1::new(AgentProfilerServiceLimitsV1::default()).unwrap();
    let response = service.handle(AgentProfilerRequestV1::DiscoverCapabilities {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 1,
    });
    let AgentProfilerResponseV1::Ok { value, .. } = &response else {
        panic!("expected capabilities")
    };
    let AgentProfilerResultV1::Capabilities {
        capabilities,
        limits,
        evidence,
    } = value.as_ref()
    else {
        panic!("expected capability value")
    };
    assert_eq!(capabilities.len(), AgentProfilerOperationV1::ALL.len());
    assert_eq!(
        capabilities
            .iter()
            .map(|capability| capability.operation)
            .collect::<BTreeSet<_>>()
            .len(),
        AgentProfilerOperationV1::ALL.len()
    );
    assert!(capabilities.iter().all(|capability| {
        capability.state != AgentProfilerCapabilityStateV1::Unavailable
            || capability.unavailable_reason.is_some()
    }));
    assert!(capabilities.iter().any(|capability| {
        capability.operation == AgentProfilerOperationV1::InspectLane
            && capability.state == AgentProfilerCapabilityStateV1::Unavailable
    }));
    assert!(capabilities.iter().any(|capability| {
        capability.operation == AgentProfilerOperationV1::PlanNextCapture
            && capability.state == AgentProfilerCapabilityStateV1::Unavailable
            && capability.unavailable_reason
                == Some(AgentProfilerUnavailableReasonV1::CapturePlanRequirementsNotRepresented)
    }));
    assert_eq!(limits.max_requests, MAX_AGENT_PROFILER_REQUESTS_V1);
    assert!(evidence.captures.is_empty());
    assert!(evidence.records.is_empty());
    assert_eq!(evidence.origin, homogeneous(TruthOriginV1::Declared));

    let first = service.encode_response(&response).unwrap();
    assert_eq!(first, service.encode_response(&response).unwrap());
    let text = String::from_utf8(first).unwrap();
    for forbidden_key in [
        "\"pid\"",
        "\"path\"",
        "\"address\"",
        "\"command\"",
        "\"execution_authority\"",
    ] {
        assert!(!text.contains(forbidden_key), "leaked {forbidden_key}");
    }
}

#[test]
fn open_page_inspect_compare_plan_and_unavailable_are_state_validated() {
    let mut service = AgentProfilerServiceV1::new(AgentProfilerServiceLimitsV1::default()).unwrap();
    let baseline_bytes = bundle(10);
    let candidate_bytes = bundle(30);
    let (opened, baseline) = open(&mut service, 1, &baseline_bytes);
    assert!(service.encode_response(&opened).is_ok());
    let (_, candidate) = open(&mut service, 2, &candidate_bytes);

    let page_response = service.handle(AgentProfilerRequestV1::ListDispatches {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 3,
        capture: baseline,
        page: ProfilerPageRequestV4 {
            limit: 1,
            cursor: None,
        },
    });
    let AgentProfilerResponseV1::Ok { value, .. } = &page_response else {
        panic!("expected page")
    };
    let AgentProfilerResultV1::Page { page, evidence } = value.as_ref() else {
        panic!("expected page value")
    };
    assert_eq!(page.returned, 1);
    assert_eq!(evidence.origin, homogeneous(TruthOriginV1::Observed));
    let ProfilerQueryItemV4::Dispatch { dispatch } = &page.items[0] else {
        panic!("expected dispatch")
    };
    let dispatch_identity = dispatch.identity;
    assert!(service.encode_response(&page_response).is_ok());

    let kernel = service.handle(AgentProfilerRequestV1::InspectKernel {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 4,
        capture: baseline,
        dispatch: dispatch_identity,
    });
    let AgentProfilerResponseV1::Ok { value, .. } = &kernel else {
        panic!("expected kernel response")
    };
    assert!(matches!(
        value.as_ref(),
        AgentProfilerResultV1::Kernel { inspection, evidence }
            if inspection.dispatch_identity == dispatch_identity
                && inspection.scope == AgentProfilerKernelScopeV1::DispatchBindingOnly
                && evidence.origin == homogeneous(TruthOriginV1::Declared)
    ));
    assert!(service.encode_response(&kernel).is_ok());

    let unavailable = service.handle(AgentProfilerRequestV1::InspectWave {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 5,
        capture: baseline,
        dispatch: dispatch_identity,
        workgroup: [0, 0, 0],
        wave: 0,
    });
    let AgentProfilerResponseV1::Ok { value, .. } = &unavailable else {
        panic!("expected unavailable response")
    };
    assert!(matches!(
        value.as_ref(),
        AgentProfilerResultV1::Unavailable {
                operation: AgentProfilerOperationV1::InspectWave,
                reason: AgentProfilerUnavailableReasonV1::WorkgroupWaveLaneHierarchyNotCaptured,
                evidence,
            } if evidence.origin == homogeneous(TruthOriginV1::Unavailable)
    ));
    assert!(service.encode_response(&unavailable).is_ok());

    let comparison = service.handle(AgentProfilerRequestV1::CompareCaptures {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 6,
        baseline,
        candidate,
    });
    let AgentProfilerResponseV1::Ok { value, .. } = &comparison else {
        panic!("expected comparison response")
    };
    assert!(matches!(
        value.as_ref(),
        AgentProfilerResultV1::Comparison { comparison, evidence }
        if comparison.baseline == baseline
            && comparison.candidate == candidate
            && !comparison.comparable
            && evidence.captures == [baseline, candidate]
    ));
    assert!(service.encode_response(&comparison).is_ok());

    let plan = service.handle(AgentProfilerRequestV1::PlanNextCapture {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 7,
        capture: baseline,
        goal: ProfilerCaptureGoalV4::ExplainWaits,
    });
    let AgentProfilerResponseV1::Ok { value, .. } = &plan else {
        panic!("expected typed unavailable plan response")
    };
    assert!(matches!(
        value.as_ref(),
        AgentProfilerResultV1::Unavailable {
            operation: AgentProfilerOperationV1::PlanNextCapture,
            reason: AgentProfilerUnavailableReasonV1::CapturePlanRequirementsNotRepresented,
            evidence,
        } if evidence.origin == homogeneous(TruthOriginV1::Unavailable)
    ));
    assert!(service.encode_response(&plan).is_ok());
}

#[test]
fn page_evidence_aggregates_homogeneous_mixed_and_unavailable_item_origins() {
    let mut service = AgentProfilerServiceV1::new(AgentProfilerServiceLimitsV1::default()).unwrap();
    let (_, dispatch_capture) = open(&mut service, 1, &bundle(10));
    let (_, att_capture) = open(&mut service, 2, &att_bundle());

    let runs = service.handle(AgentProfilerRequestV1::ListRuns {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 3,
        capture: dispatch_capture,
        page: ProfilerPageRequestV4::default(),
    });
    let AgentProfilerResponseV1::Ok { value, .. } = &runs else {
        panic!("expected run page")
    };
    assert!(matches!(
        value.as_ref(),
        AgentProfilerResultV1::Page { evidence, .. }
            if evidence.origin == homogeneous(TruthOriginV1::Inferred)
    ));
    assert!(service.encode_response(&runs).is_ok());

    let devices = service.handle(AgentProfilerRequestV1::ListDevices {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 4,
        capture: dispatch_capture,
        page: ProfilerPageRequestV4::default(),
    });
    let AgentProfilerResponseV1::Ok { value, .. } = &devices else {
        panic!("expected device page")
    };
    assert!(matches!(
        value.as_ref(),
        AgentProfilerResultV1::Page { evidence, .. }
            if evidence.origin == homogeneous(TruthOriginV1::Declared)
    ));
    assert!(service.encode_response(&devices).is_ok());

    let references = service.handle(AgentProfilerRequestV1::ListAttReferences {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 5,
        capture: att_capture,
        page: ProfilerPageRequestV4::default(),
    });
    let AgentProfilerResponseV1::Ok { value, .. } = &references else {
        panic!("expected ATT reference page")
    };
    assert!(matches!(
        value.as_ref(),
        AgentProfilerResultV1::Page { evidence, .. }
            if evidence.origin == AgentProfilerAggregateOriginV1::Mixed {
                origins: vec![TruthOriginV1::Declared, TruthOriginV1::Unavailable],
            }
    ));
    assert!(service.encode_response(&references).is_ok());

    let unavailable_dispatches = service.handle(AgentProfilerRequestV1::ListDispatches {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 6,
        capture: att_capture,
        page: ProfilerPageRequestV4::default(),
    });
    let AgentProfilerResponseV1::Ok { value, .. } = &unavailable_dispatches else {
        panic!("expected typed unavailable dispatch page")
    };
    assert!(matches!(
        value.as_ref(),
        AgentProfilerResultV1::Page { evidence, .. }
            if evidence.origin == homogeneous(TruthOriginV1::Unavailable)
    ));
    assert!(service.encode_response(&unavailable_dispatches).is_ok());
}

#[test]
fn hostile_requests_aliases_replays_and_cross_capture_cursors_fail_closed() {
    assert!(decode_agent_profiler_request_line_v1(
        br#"{"operation":"discover_capabilities","schema":"fe2o3-agent-profiler-request-v1","request_id":1,"unknown":true}
"#
    )
    .is_err());
    assert!(decode_agent_profiler_request_line_v1(
        br#"{"operation":"discover_capabilities","schema":"fe2o3-agent-profiler-request-v1","request_id":1}
{}
"#
    )
    .is_err());

    let mut service = AgentProfilerServiceV1::new(AgentProfilerServiceLimitsV1::default()).unwrap();
    assert_error(
        service.handle(AgentProfilerRequestV1::DiscoverCapabilities {
            schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
            request_id: 0,
        }),
        AgentProfilerErrorCodeV1::InvalidRequestId,
    );
    let good = service.handle(AgentProfilerRequestV1::DiscoverCapabilities {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 1,
    });
    assert!(matches!(good, AgentProfilerResponseV1::Ok { .. }));
    assert_error(
        service.handle(AgentProfilerRequestV1::DiscoverCapabilities {
            schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
            request_id: 1,
        }),
        AgentProfilerErrorCodeV1::DuplicateRequestId,
    );
    assert_error(
        service.handle(AgentProfilerRequestV1::OpenCapture {
            schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
            request_id: 2,
            bundle_hex: "AB".into(),
        }),
        AgentProfilerErrorCodeV1::InvalidBundleEncoding,
    );

    let (_, baseline) = open(&mut service, 3, &bundle(10));
    let (_, candidate) = open(&mut service, 4, &bundle(30));
    assert_error(
        service.handle(AgentProfilerRequestV1::InspectLane {
            schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
            request_id: 8,
            capture: baseline,
            dispatch: CaptureIdentityV1::new([88; 32]).unwrap(),
            workgroup: [0, 0, 0],
            wave: 0,
            lane: 64,
        }),
        AgentProfilerErrorCodeV1::InvalidSelector,
    );
    assert_error(
        service.handle(AgentProfilerRequestV1::InspectWave {
            schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
            request_id: 9,
            capture: baseline,
            dispatch: CaptureIdentityV1::new([88; 32]).unwrap(),
            workgroup: [0, 0, 0],
            wave: 0,
        }),
        AgentProfilerErrorCodeV1::RecordNotFound,
    );
    let page = service.handle(AgentProfilerRequestV1::ListDispatches {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 5,
        capture: baseline,
        page: ProfilerPageRequestV4 {
            limit: 1,
            cursor: None,
        },
    });
    let AgentProfilerResponseV1::Ok { value, .. } = page else {
        panic!("expected page")
    };
    let AgentProfilerResultV1::Page { page, .. } = value.as_ref() else {
        panic!("expected page value")
    };
    assert_error(
        service.handle(AgentProfilerRequestV1::ListDispatches {
            schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
            request_id: 6,
            capture: candidate,
            page: ProfilerPageRequestV4 {
                limit: 1,
                cursor: page.next_cursor,
            },
        }),
        AgentProfilerErrorCodeV1::InvalidPage,
    );
    let mut alias = baseline;
    alias.canonical_len += 1;
    assert_error(
        service.handle(AgentProfilerRequestV1::ListRuns {
            schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
            request_id: 7,
            capture: alias,
            page: ProfilerPageRequestV4::default(),
        }),
        AgentProfilerErrorCodeV1::CaptureNotOpen,
    );
}

#[test]
fn request_and_capture_budgets_end_or_reject_without_eviction() {
    let request_limits =
        AgentProfilerServiceLimitsV1::new(1, 1, ProfilerQueryLimitsV4::default()).unwrap();
    let mut request_limited = AgentProfilerServiceV1::new(request_limits).unwrap();
    assert!(matches!(
        request_limited.handle(AgentProfilerRequestV1::DiscoverCapabilities {
            schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
            request_id: 1,
        }),
        AgentProfilerResponseV1::Ok { .. }
    ));
    assert!(matches!(
        request_limited.handle(AgentProfilerRequestV1::DiscoverCapabilities {
            schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
            request_id: 2,
        }),
        AgentProfilerResponseV1::Error {
            code: AgentProfilerErrorCodeV1::RequestBudgetExhausted,
            terminal: true,
            ..
        }
    ));

    let capture_limits =
        AgentProfilerServiceLimitsV1::new(4, 1, ProfilerQueryLimitsV4::default()).unwrap();
    let mut capture_limited = AgentProfilerServiceV1::new(capture_limits).unwrap();
    open(&mut capture_limited, 1, &bundle(10));
    assert_error(
        capture_limited.handle(AgentProfilerRequestV1::OpenCapture {
            schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
            request_id: 2,
            bundle_hex: lower_hex(&bundle(30)),
        }),
        AgentProfilerErrorCodeV1::CaptureLimitReached,
    );
}

#[test]
fn zero_and_duplicate_ids_consume_budget_and_terminal_revision_is_stable() {
    let limits = AgentProfilerServiceLimitsV1::new(3, 1, ProfilerQueryLimitsV4::default()).unwrap();
    let mut service = AgentProfilerServiceV1::new(limits).unwrap();
    let first = service.handle(AgentProfilerRequestV1::DiscoverCapabilities {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 1,
    });
    assert!(matches!(
        first,
        AgentProfilerResponseV1::Ok {
            response_revision: 1,
            ..
        }
    ));
    let duplicate = service.handle(AgentProfilerRequestV1::DiscoverCapabilities {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 1,
    });
    assert!(matches!(
        duplicate,
        AgentProfilerResponseV1::Error {
            response_revision: 2,
            code: AgentProfilerErrorCodeV1::DuplicateRequestId,
            terminal: false,
            ..
        }
    ));
    let zero = service.handle(AgentProfilerRequestV1::DiscoverCapabilities {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 0,
    });
    assert!(matches!(
        zero,
        AgentProfilerResponseV1::Error {
            response_revision: 3,
            code: AgentProfilerErrorCodeV1::InvalidRequestId,
            terminal: false,
            ..
        }
    ));
    let terminal = service.handle(AgentProfilerRequestV1::DiscoverCapabilities {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 2,
    });
    assert!(matches!(
        terminal,
        AgentProfilerResponseV1::Error {
            response_revision: 4,
            code: AgentProfilerErrorCodeV1::RequestBudgetExhausted,
            terminal: true,
            ..
        }
    ));
    let after_terminal = service.handle(AgentProfilerRequestV1::DiscoverCapabilities {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 3,
    });
    assert_eq!(after_terminal, terminal);
    assert!(service.encode_response(&terminal).is_ok());
}

#[test]
fn configured_bundle_limit_rejects_before_capture_admission() {
    let bytes = bundle(10);
    let query = ProfilerQueryLimitsV4::new(
        u64::try_from(bytes.len() - 1).unwrap(),
        MAX_PROFILER_QUERY_RESPONSE_BYTES_V4,
        128,
    )
    .unwrap();
    let limits = AgentProfilerServiceLimitsV1::new(2, 1, query).unwrap();
    let mut service = AgentProfilerServiceV1::new(limits).unwrap();
    assert_error(
        service.handle(AgentProfilerRequestV1::OpenCapture {
            schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
            request_id: 1,
            bundle_hex: lower_hex(&bytes),
        }),
        AgentProfilerErrorCodeV1::BundleTooLarge,
    );
}

#[test]
fn jsonl_reader_rejects_oversize_and_unterminated_frames() {
    let mut oversized = Cursor::new(vec![b'x'; MAX_AGENT_PROFILER_REQUEST_BYTES_V1 as usize + 1]);
    assert!(matches!(
        read_agent_profiler_request_line_v1(&mut oversized),
        Err(AgentProfilerServiceErrorV1::RequestTooLarge)
    ));

    let mut unterminated = Cursor::new(br#"{"operation":"discover_capabilities"}"#.to_vec());
    assert!(matches!(
        read_agent_profiler_request_line_v1(&mut unterminated),
        Err(AgentProfilerServiceErrorV1::InvalidRequest)
    ));
}

#[test]
fn state_encoder_rejects_forged_ids_revisions_audits_and_evidence() {
    let mut service = AgentProfilerServiceV1::new(AgentProfilerServiceLimitsV1::default()).unwrap();
    let (opened, capture) = open(&mut service, 1, &bundle(10));
    open(&mut service, 2, &bundle(30));
    assert!(service.encode_response(&opened).is_ok());

    let mut forged_request = opened.clone();
    let AgentProfilerResponseV1::Ok { request_id, .. } = &mut forged_request else {
        unreachable!()
    };
    *request_id = 99;
    assert!(matches!(
        service.encode_response(&forged_request),
        Err(AgentProfilerServiceErrorV1::InvalidResponse)
    ));

    let mut forged_revision = opened.clone();
    let AgentProfilerResponseV1::Ok {
        response_revision, ..
    } = &mut forged_revision
    else {
        unreachable!()
    };
    *response_revision = 99;
    assert!(matches!(
        service.encode_response(&forged_revision),
        Err(AgentProfilerServiceErrorV1::InvalidResponse)
    ));

    let mut forged_audit = opened;
    let AgentProfilerResponseV1::Ok { value, .. } = &mut forged_audit else {
        unreachable!()
    };
    let AgentProfilerResultV1::CaptureOpened { audit, .. } = value.as_mut() else {
        unreachable!()
    };
    audit.before_open_captures = 1;
    audit.after_open_captures = 2;
    assert!(matches!(
        service.encode_response(&forged_audit),
        Err(AgentProfilerServiceErrorV1::InvalidResponse)
    ));

    let mut response = service.handle(AgentProfilerRequestV1::ListRuns {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 3,
        capture,
        page: ProfilerPageRequestV4::default(),
    });
    let AgentProfilerResponseV1::Ok { value, .. } = &mut response else {
        panic!("expected page")
    };
    let AgentProfilerResultV1::Page { evidence, .. } = value.as_mut() else {
        panic!("expected page value")
    };
    evidence.service_contract.digest = CaptureIdentityV1::new([99; 32]).unwrap();
    assert!(matches!(
        service.encode_response(&response),
        Err(AgentProfilerServiceErrorV1::InvalidResponse)
    ));
}

#[test]
fn jsonl_binary_keeps_state_across_requests_and_terminates_on_malformed_input() {
    let executable = env!("CARGO_BIN_EXE_fe2o3-profiler-service");
    let mut child = Command::new(executable)
        .arg("jsonl")
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
            "operation": "open_capture",
            "schema": AGENT_PROFILER_REQUEST_SCHEMA_V1,
            "request_id": 1,
            "bundle_hex": lower_hex(&bundle(10)),
        })
    )
    .unwrap();
    input.flush().unwrap();
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    let opened: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(opened["status"], "ok");
    let capture = opened["value"]["context"]["bundle_identity"].clone();

    writeln!(
        input,
        "{}",
        json!({
            "operation": "list_dispatches",
            "schema": AGENT_PROFILER_REQUEST_SCHEMA_V1,
            "request_id": 2,
            "capture": capture,
            "page": { "limit": 1, "cursor": null },
        })
    )
    .unwrap();
    input.flush().unwrap();
    line.clear();
    output.read_line(&mut line).unwrap();
    let page: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(page["status"], "ok");
    assert_eq!(page["value"]["page"]["returned"], 1);

    input.write_all(b"not-json\n").unwrap();
    input.flush().unwrap();
    line.clear();
    output.read_line(&mut line).unwrap();
    let terminal: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(terminal["status"], "error");
    assert_eq!(terminal["code"], "invalid_request");
    assert_eq!(terminal["terminal"], true);
    assert_eq!(terminal["response_revision"], 3);
    drop(input);
    let status = child.wait().unwrap();
    assert_eq!(status.code(), Some(1));
}
