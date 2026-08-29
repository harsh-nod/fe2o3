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

fn planning(
    goal: AgentProfilerPlanGoalV1,
    ambiguity: AgentProfilerAmbiguityV1,
    missing_evidence: Vec<AgentProfilerPlanEvidenceClassV1>,
    dispatch: Option<CaptureIdentityV1>,
    kernel_ir: Option<CaptureIdentityV1>,
) -> AgentProfilerPlanRequestV1 {
    AgentProfilerPlanRequestV1 {
        schema: AGENT_PROFILER_PLAN_REQUEST_SCHEMA_V1.into(),
        goal,
        ambiguity,
        missing_evidence,
        target: AgentProfilerPlanTargetV1 {
            compute_units: vec![3, 1],
            kernel_ir,
            dispatch,
        },
        constraints: AgentProfilerPlanConstraintsV1 {
            maximum_overhead_basis_points: MAX_AGENT_PROFILER_PLAN_OVERHEAD_BASIS_POINTS_V1,
            maximum_storage_bytes: 16 * 1024 * 1024,
            maximum_records: 100_000,
        },
    }
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

fn dispatch_target(
    service: &mut AgentProfilerServiceV1,
    request_id: u64,
    capture: ContentIdentityRecordV1,
) -> (CaptureIdentityV1, CaptureIdentityV1) {
    let page = service.handle(AgentProfilerRequestV1::ListDispatches {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id,
        capture,
        page: ProfilerPageRequestV4 {
            limit: 1,
            cursor: None,
        },
    });
    let AgentProfilerResponseV1::Ok { value, .. } = page else {
        panic!("expected dispatch page")
    };
    let AgentProfilerResultV1::Page { page, .. } = value.as_ref() else {
        panic!("expected dispatch page value")
    };
    let ProfilerQueryItemV4::Dispatch { dispatch } = &page.items[0] else {
        panic!("expected dispatch")
    };
    let dispatch_identity = dispatch.identity;
    let kernel = service.handle(AgentProfilerRequestV1::InspectKernel {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: request_id + 1,
        capture,
        dispatch: dispatch_identity,
    });
    let AgentProfilerResponseV1::Ok { value, .. } = kernel else {
        panic!("expected kernel inspection")
    };
    let AgentProfilerResultV1::Kernel { inspection, .. } = value.as_ref() else {
        panic!("expected kernel inspection value")
    };
    (dispatch_identity, inspection.kernel_ir.digest)
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
            && capability.state == AgentProfilerCapabilityStateV1::CaptureDependent
            && capability.unavailable_reason.is_none()
            && capability.request_contract_schema == Some(AGENT_PROFILER_PLAN_REQUEST_SCHEMA_V1)
            && capability.result_contract_schema == Some(AGENT_PROFILER_PLAN_SCHEMA_V1)
    }));
    assert_eq!(limits.max_requests, MAX_AGENT_PROFILER_REQUESTS_V1);
    assert_eq!(
        usize::from(limits.max_plan_missing_facts),
        MAX_AGENT_PROFILER_PLAN_MISSING_FACTS_V1
    );
    assert_eq!(
        usize::from(limits.max_plan_compute_units),
        MAX_AGENT_PROFILER_PLAN_COMPUTE_UNITS_V1
    );
    assert_eq!(
        limits.max_plan_storage_bytes,
        MAX_AGENT_PROFILER_PLAN_STORAGE_BYTES_V1
    );
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
        planning: planning(
            AgentProfilerPlanGoalV1::ExplainWaits,
            AgentProfilerAmbiguityV1::UnknownWaitCause,
            vec![
                AgentProfilerPlanEvidenceClassV1::AttManifest,
                AgentProfilerPlanEvidenceClassV1::DecodedWaitEvents,
            ],
            None,
            None,
        ),
    });
    let AgentProfilerResponseV1::Ok { value, .. } = &plan else {
        panic!("expected capture plan response")
    };
    assert!(matches!(
        value.as_ref(),
        AgentProfilerResultV1::CapturePlan { plan, evidence }
        if plan.goal == AgentProfilerPlanGoalV1::ExplainWaits
            && plan.disposition
                == AgentProfilerPlanDispositionV1::AdditionalCaptureRequiredWithUnavailableConfigurationOrPostprocessing
            && plan.minimum_additional_captures == 1
            && evidence.origin == (AgentProfilerAggregateOriginV1::Mixed {
                origins: vec![
                    TruthOriginV1::Declared,
                    TruthOriginV1::Observed,
                    TruthOriginV1::Inferred,
                ],
            })
    ));
    assert!(service.encode_response(&plan).is_ok());

    let explanation = service.handle(AgentProfilerRequestV1::ExplainRegression {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 8,
        baseline,
        candidate,
    });
    assert!(matches!(
        &explanation,
        AgentProfilerResponseV1::Ok { value, .. }
        if matches!(
            value.as_ref(),
            AgentProfilerResultV1::Unavailable {
                operation: AgentProfilerOperationV1::ExplainRegression,
                reason: AgentProfilerUnavailableReasonV1::RankedExplanationRequiresCausalCounterOrDecodedEventEvidence,
                ..
            }
        )
    ));
    assert!(service.encode_response(&explanation).is_ok());
}

#[test]
fn ambiguous_correctness_plan_selects_one_bounded_targeted_att_capture() {
    let mut service = AgentProfilerServiceV1::new(AgentProfilerServiceLimitsV1::default()).unwrap();
    let (_, capture) = open(&mut service, 1, &bundle(10));
    let (dispatch, kernel_ir) = dispatch_target(&mut service, 2, capture);
    let request = planning(
        AgentProfilerPlanGoalV1::AmbiguousCorrectnessDiagnosis,
        AgentProfilerAmbiguityV1::MemoryFaultVsBarrierDivergence,
        vec![
            AgentProfilerPlanEvidenceClassV1::AttManifest,
            AgentProfilerPlanEvidenceClassV1::DecodedBarrierEvents,
            AgentProfilerPlanEvidenceClassV1::DecodedMemoryEvents,
        ],
        Some(dispatch),
        Some(kernel_ir),
    );
    let response = service.handle(AgentProfilerRequestV1::PlanNextCapture {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 4,
        capture,
        planning: request.clone(),
    });
    let AgentProfilerResponseV1::Ok { value, .. } = &response else {
        panic!("expected correctness capture plan: {response:?}")
    };
    let AgentProfilerResultV1::CapturePlan { plan, evidence } = value.as_ref() else {
        panic!("expected correctness capture plan value")
    };
    assert_eq!(plan.schema, AGENT_PROFILER_PLAN_SCHEMA_V1);
    assert_eq!(
        plan.discrimination_method,
        AgentProfilerDiscriminationMethodV1::DecodedMemoryVsBarrierEventClassification
    );
    assert_eq!(
        plan.disposition,
        AgentProfilerPlanDispositionV1::AdditionalCaptureRequiredWithUnavailableConfigurationOrPostprocessing
    );
    assert_eq!(plan.minimum_additional_captures, 1);
    assert_eq!(plan.target.compute_units, [1, 3]);
    assert_eq!(plan.target.dispatch, Some(dispatch));
    assert_eq!(plan.target.kernel_ir, Some(kernel_ir));
    assert_eq!(
        plan.selected_missing_evidence,
        [
            AgentProfilerPlanEvidenceClassV1::AttManifest,
            AgentProfilerPlanEvidenceClassV1::DecodedMemoryEvents,
            AgentProfilerPlanEvidenceClassV1::DecodedBarrierEvents,
        ]
    );
    assert!(
        plan.already_available_evidence
            .contains(&AgentProfilerPlanEvidenceClassV1::DispatchEnvelope)
    );
    let recipe = plan.recipe.as_ref().unwrap();
    assert_eq!(
        recipe.requested_data_classes,
        [
            AgentProfilerCaptureDataClassV1::AttThreadTrace,
            AgentProfilerCaptureDataClassV1::DecodedMemoryEvents,
            AgentProfilerCaptureDataClassV1::DecodedBarrierEvents,
        ]
    );
    assert!(recipe.requested_logical_counters.is_empty());
    assert_eq!(
        recipe.target_validation.compute_units,
        AgentProfilerSelectorValidationV1::CallerDeclaredNotValidatedByBundle
    );
    assert_eq!(
        recipe.target_validation.kernel_ir,
        AgentProfilerSelectorValidationV1::ValidatedAgainstCapture
    );
    assert!(recipe.collector_requirements.iter().any(|requirement| {
        requirement.tool == AgentProfilerCollectorToolV1::Fe2o3SemanticImporter
            && requirement.capability
                == AgentProfilerCollectorCapabilityV1::StrictDecodedEventImport
            && requirement.status
                == AgentProfilerCollectorCapabilityStatusV1::RequiredUnavailableInCurrentBuild
    }));
    assert_eq!(recipe.expected_overhead.origin, TruthOriginV1::Declared);
    assert_eq!(
        recipe
            .expected_overhead
            .additional_runtime_basis_points
            .maximum,
        MAX_AGENT_PROFILER_PLAN_OVERHEAD_BASIS_POINTS_V1
    );
    assert!(
        recipe
            .expected_overhead
            .limitations
            .contains(&AgentProfilerOverheadLimitationV1::NotMeasured)
    );
    assert_eq!(
        recipe.required_privilege,
        AgentProfilerPrivilegeRequirementV1::ProfilerAccess
    );
    assert_eq!(
        recipe.authorization,
        AgentProfilerAuthorizationBoundaryV1 {
            service_authority: AgentProfilerServiceAuthorityV1::ReadOnlyPlanningOnly,
            stateful_execution:
                AgentProfilerExecutionAuthorizationV1::SeparateExplicitAuthorizationRequired,
            attach_authority: AgentProfilerAttachAuthorityV1::NotAvailableToService,
        }
    );
    assert_eq!(recipe.storage.maximum_bytes, 16 * 1024 * 1024);
    assert_eq!(recipe.storage.estimate_scale_multiplier, 64);
    assert!(recipe.storage.estimated_bytes.maximum <= recipe.storage.maximum_bytes);
    assert_eq!(evidence.captures, [capture]);
    assert_eq!(evidence.records, [dispatch]);
    assert!(plan.provenance.iter().any(|entry| {
        entry.kind == AgentProfilerPlanProvenanceKindV1::PlanningRequest
            && entry.identity == plan.request_identity
            && entry.origin == TruthOriginV1::Declared
    }));
    assert!(service.encode_response(&response).is_ok());

    let repeated = service.handle(AgentProfilerRequestV1::PlanNextCapture {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 5,
        capture,
        planning: request,
    });
    let AgentProfilerResponseV1::Ok { value, .. } = repeated else {
        panic!("expected repeated plan")
    };
    let AgentProfilerResultV1::CapturePlan {
        plan: repeated_plan,
        ..
    } = value.as_ref()
    else {
        panic!("expected repeated capture plan")
    };
    assert_eq!(plan.as_ref(), repeated_plan.as_ref());
}

#[test]
fn schedule_resource_plan_uses_logical_counters_without_causal_overclaim() {
    let mut service = AgentProfilerServiceV1::new(AgentProfilerServiceLimitsV1::default()).unwrap();
    let (_, capture) = open(&mut service, 1, &bundle(10));
    let (dispatch, kernel_ir) = dispatch_target(&mut service, 2, capture);
    let response = service.handle(AgentProfilerRequestV1::PlanNextCapture {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 4,
        capture,
        planning: planning(
            AgentProfilerPlanGoalV1::ScheduleResourceRegression,
            AgentProfilerAmbiguityV1::SchedulingDelayVsResourcePressure,
            vec![AgentProfilerPlanEvidenceClassV1::HardwareCounterMeasurements],
            Some(dispatch),
            Some(kernel_ir),
        ),
    });
    let AgentProfilerResponseV1::Ok { value, .. } = &response else {
        panic!("expected schedule/resource capture plan: {response:?}")
    };
    let AgentProfilerResultV1::CapturePlan { plan, .. } = value.as_ref() else {
        panic!("expected schedule/resource capture plan value")
    };
    assert!(
        plan.already_available_evidence
            .contains(&AgentProfilerPlanEvidenceClassV1::DispatchTiming)
    );
    assert_eq!(
        plan.selected_missing_evidence,
        [AgentProfilerPlanEvidenceClassV1::HardwareCounterMeasurements]
    );
    let recipe = plan.recipe.as_ref().unwrap();
    assert_eq!(
        plan.discrimination_method,
        AgentProfilerDiscriminationMethodV1::AggregateSchedulerVsResourceCounterContrast
    );
    assert_eq!(
        plan.disposition,
        AgentProfilerPlanDispositionV1::AdditionalCaptureRequiredWithUnavailableConfigurationOrPostprocessing
    );
    assert_eq!(
        recipe.requested_data_classes,
        [AgentProfilerCaptureDataClassV1::DispatchHardwareCounters]
    );
    assert_eq!(recipe.requested_logical_counters.len(), 5);
    assert!(recipe.collector_requirements.iter().any(|requirement| {
        requirement.capability == AgentProfilerCollectorCapabilityV1::LogicalCounterResolution
            && requirement.status
                == AgentProfilerCollectorCapabilityStatusV1::RequiredUnavailableInCurrentBuild
    }));
    assert!(recipe.collector_requirements.iter().any(|requirement| {
        requirement.capability == AgentProfilerCollectorCapabilityV1::DispatchCounterCollection
            && requirement.status
                == AgentProfilerCollectorCapabilityStatusV1::RequiredNotVerifiedByCapture
    }));
    assert_eq!(
        recipe.target_validation.compute_units,
        AgentProfilerSelectorValidationV1::CallerDeclaredNotValidatedByBundle
    );
    assert_eq!(
        recipe.mutual_exclusions,
        [AgentProfilerMutualExclusionV1 {
            excluded_data_class: AgentProfilerCaptureDataClassV1::AttThreadTrace,
            reason: AgentProfilerMutualExclusionReasonV1::SeparateInstrumentationCaptureRequired,
        }]
    );
    assert!(
        recipe
            .sampling_and_completeness
            .limitations
            .contains(&AgentProfilerCompletenessLimitV1::AggregateCountersDoNotEstablishCausality)
    );
    assert_eq!(
        recipe.expected_overhead.additional_runtime_basis_points,
        AgentProfilerBoundedU32RangeV1 {
            minimum: 0,
            maximum: 50_000,
        }
    );
    assert!(service.encode_response(&response).is_ok());
}

#[test]
fn existing_att_manifest_requires_postprocessing_without_another_capture() {
    let mut service = AgentProfilerServiceV1::new(AgentProfilerServiceLimitsV1::default()).unwrap();
    let (_, capture) = open(&mut service, 1, &att_bundle());
    let response = service.handle(AgentProfilerRequestV1::PlanNextCapture {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 2,
        capture,
        planning: planning(
            AgentProfilerPlanGoalV1::DecodeAttCoverage,
            AgentProfilerAmbiguityV1::MissingVsUndecodedAttCoverage,
            vec![
                AgentProfilerPlanEvidenceClassV1::DecodedMemoryEvents,
                AgentProfilerPlanEvidenceClassV1::DecodedBarrierEvents,
                AgentProfilerPlanEvidenceClassV1::DecodedWaitEvents,
            ],
            None,
            None,
        ),
    });
    let AgentProfilerResponseV1::Ok { value, .. } = &response else {
        panic!("expected ATT postprocessing plan")
    };
    let AgentProfilerResultV1::CapturePlan { plan, .. } = value.as_ref() else {
        panic!("expected ATT postprocessing plan value")
    };
    assert_eq!(
        plan.disposition,
        AgentProfilerPlanDispositionV1::ExistingCaptureRequiresUnavailablePostprocessing
    );
    assert_eq!(plan.minimum_additional_captures, 0);
    assert!(
        plan.already_available_evidence
            .contains(&AgentProfilerPlanEvidenceClassV1::AttManifest)
    );
    let recipe = plan.recipe.as_ref().unwrap();
    assert_eq!(
        recipe.action,
        AgentProfilerCaptureActionV1::PostprocessExistingCapture
    );
    assert!(
        !recipe
            .requested_data_classes
            .contains(&AgentProfilerCaptureDataClassV1::AttThreadTrace)
    );
    assert!(!recipe.collector_requirements.iter().any(|requirement| {
        requirement.capability == AgentProfilerCollectorCapabilityV1::AttThreadTraceCollection
    }));
    assert_eq!(
        recipe.expected_overhead.additional_runtime_basis_points,
        AgentProfilerBoundedU32RangeV1 {
            minimum: 0,
            maximum: 0,
        }
    );
    assert_eq!(
        recipe.required_privilege,
        AgentProfilerPrivilegeRequirementV1::None
    );
    assert!(service.encode_response(&response).is_ok());
}

#[test]
fn hostile_plan_bounds_stale_facts_and_substitutions_fail_closed() {
    let mut service = AgentProfilerServiceV1::new(AgentProfilerServiceLimitsV1::default()).unwrap();
    let (_, capture) = open(&mut service, 1, &bundle(10));
    let (dispatch, kernel_ir) = dispatch_target(&mut service, 2, capture);

    let stale = service.handle(AgentProfilerRequestV1::PlanNextCapture {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 4,
        capture,
        planning: planning(
            AgentProfilerPlanGoalV1::ScheduleResourceRegression,
            AgentProfilerAmbiguityV1::SchedulingDelayVsResourcePressure,
            vec![AgentProfilerPlanEvidenceClassV1::DispatchTiming],
            Some(dispatch),
            Some(kernel_ir),
        ),
    });
    assert_error(stale, AgentProfilerErrorCodeV1::InvalidPlanRequest);

    let mut duplicate_selector = planning(
        AgentProfilerPlanGoalV1::ScheduleResourceRegression,
        AgentProfilerAmbiguityV1::SchedulingDelayVsResourcePressure,
        vec![AgentProfilerPlanEvidenceClassV1::HardwareCounterMeasurements],
        Some(dispatch),
        Some(kernel_ir),
    );
    duplicate_selector.target.compute_units = vec![2, 2];
    assert_error(
        service.handle(AgentProfilerRequestV1::PlanNextCapture {
            schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
            request_id: 5,
            capture,
            planning: duplicate_selector,
        }),
        AgentProfilerErrorCodeV1::InvalidPlanRequest,
    );

    let mut oversized = planning(
        AgentProfilerPlanGoalV1::ScheduleResourceRegression,
        AgentProfilerAmbiguityV1::SchedulingDelayVsResourcePressure,
        vec![AgentProfilerPlanEvidenceClassV1::HardwareCounterMeasurements],
        Some(dispatch),
        Some(kernel_ir),
    );
    oversized.constraints.maximum_storage_bytes = MAX_AGENT_PROFILER_PLAN_STORAGE_BYTES_V1 + 1;
    assert_error(
        service.handle(AgentProfilerRequestV1::PlanNextCapture {
            schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
            request_id: 6,
            capture,
            planning: oversized,
        }),
        AgentProfilerErrorCodeV1::InvalidPlanRequest,
    );

    let mut overhead = planning(
        AgentProfilerPlanGoalV1::ScheduleResourceRegression,
        AgentProfilerAmbiguityV1::SchedulingDelayVsResourcePressure,
        vec![AgentProfilerPlanEvidenceClassV1::HardwareCounterMeasurements],
        Some(dispatch),
        Some(kernel_ir),
    );
    overhead.constraints.maximum_overhead_basis_points =
        MAX_AGENT_PROFILER_PLAN_OVERHEAD_BASIS_POINTS_V1 + 1;
    assert_error(
        service.handle(AgentProfilerRequestV1::PlanNextCapture {
            schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
            request_id: 7,
            capture,
            planning: overhead,
        }),
        AgentProfilerErrorCodeV1::InvalidPlanRequest,
    );

    let mut records = planning(
        AgentProfilerPlanGoalV1::ScheduleResourceRegression,
        AgentProfilerAmbiguityV1::SchedulingDelayVsResourcePressure,
        vec![AgentProfilerPlanEvidenceClassV1::HardwareCounterMeasurements],
        Some(dispatch),
        Some(kernel_ir),
    );
    records.constraints.maximum_records = MAX_AGENT_PROFILER_PLAN_RECORDS_V1 + 1;
    assert_error(
        service.handle(AgentProfilerRequestV1::PlanNextCapture {
            schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
            request_id: 8,
            capture,
            planning: records,
        }),
        AgentProfilerErrorCodeV1::InvalidPlanRequest,
    );

    let mut compute_units = planning(
        AgentProfilerPlanGoalV1::ScheduleResourceRegression,
        AgentProfilerAmbiguityV1::SchedulingDelayVsResourcePressure,
        vec![AgentProfilerPlanEvidenceClassV1::HardwareCounterMeasurements],
        Some(dispatch),
        Some(kernel_ir),
    );
    compute_units.target.compute_units =
        (0..=u32::try_from(MAX_AGENT_PROFILER_PLAN_COMPUTE_UNITS_V1).unwrap()).collect();
    assert_error(
        service.handle(AgentProfilerRequestV1::PlanNextCapture {
            schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
            request_id: 9,
            capture,
            planning: compute_units,
        }),
        AgentProfilerErrorCodeV1::InvalidPlanRequest,
    );

    let mut missing_facts = planning(
        AgentProfilerPlanGoalV1::ScheduleResourceRegression,
        AgentProfilerAmbiguityV1::SchedulingDelayVsResourcePressure,
        vec![AgentProfilerPlanEvidenceClassV1::HardwareCounterMeasurements],
        Some(dispatch),
        Some(kernel_ir),
    );
    missing_facts.missing_evidence = vec![
        AgentProfilerPlanEvidenceClassV1::HardwareCounterMeasurements;
        MAX_AGENT_PROFILER_PLAN_MISSING_FACTS_V1 + 1
    ];
    assert_error(
        service.handle(AgentProfilerRequestV1::PlanNextCapture {
            schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
            request_id: 10,
            capture,
            planning: missing_facts,
        }),
        AgentProfilerErrorCodeV1::InvalidPlanRequest,
    );

    let valid = service.handle(AgentProfilerRequestV1::PlanNextCapture {
        schema: AGENT_PROFILER_REQUEST_SCHEMA_V1.into(),
        request_id: 11,
        capture,
        planning: planning(
            AgentProfilerPlanGoalV1::ScheduleResourceRegression,
            AgentProfilerAmbiguityV1::SchedulingDelayVsResourcePressure,
            vec![AgentProfilerPlanEvidenceClassV1::HardwareCounterMeasurements],
            Some(dispatch),
            Some(kernel_ir),
        ),
    });
    assert!(service.encode_response(&valid).is_ok());
    let mut substituted = valid;
    let AgentProfilerResponseV1::Ok { value, .. } = &mut substituted else {
        unreachable!()
    };
    let AgentProfilerResultV1::CapturePlan { plan, .. } = value.as_mut() else {
        unreachable!()
    };
    plan.declared_constraints.maximum_storage_bytes -= 1;
    assert!(matches!(
        service.encode_response(&substituted),
        Err(AgentProfilerServiceErrorV1::InvalidResponse)
    ));
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
