use std::io::Write;
use std::process::{Command, Stdio};

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, Constant, Function, Kernel, LaunchDomain, LaunchExtent,
    MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature, Terminator, Type,
    ValueDef, ValueId, VerifiedCanonicalKernelIrV7,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationFailureReductionLimitsV1, SimulationFailureReductionReportV1,
    SimulationFailureScheduleV1, SimulationLimitsV1, SimulationRaceAssessmentV1,
    SimulationRequestV1, SimulationScheduleRequestV1, SimulationTargetV1,
};
use fe2o3_runtime_model::{IdentityDigestV1, TransitionErrorV1};
use fe2o3_virtual_runtime::{
    VirtualArgumentV1, VirtualBufferAccessV1, VirtualDispatchInputBindingV1,
    VirtualDispatchRequestV1, VirtualEvidenceIdentityV1, VirtualHostLifetimeCompletenessV1,
    VirtualHostLifetimeEvidenceLimitsV1, VirtualHostLifetimeEvidenceV1,
    VirtualHostLifetimeOperationV1, VirtualKirEvidenceReferenceV1, VirtualRuntimeConfigV1,
    VirtualRuntimeErrorV1, VirtualRuntimeLimitsV1, VirtualRuntimeV1, VirtualTargetProfileV1,
};
use fe2o3_virtual_runtime_cli::agent_c5_v1::*;

fn op(result: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(result), ty), kind)
}

fn store_module(module_name: &str, kernel_name: &str) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
    block.operations = vec![
        op(1, scalar, OperationKind::Constant(Constant::U32(42))),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let function_name = format!("{kernel_name}_impl");
    let entry = Function::kernel_entry(
        function_name.clone(),
        Signature::new(vec![pointer], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    let mut module = Module::new(module_name);
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        kernel_name,
        function_name,
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn admitted(module_name: &str, kernel_name: &str) -> AdmittedSimulationModuleV1 {
    AdmittedSimulationModuleV1::admit(
        VerifiedCanonicalKernelIrV7::from_module(store_module(module_name, kernel_name)).unwrap(),
        SimulationLimitsV1::default(),
    )
    .unwrap()
}

fn buffer() -> BufferArgumentV1 {
    BufferArgumentV1::from_scalars(
        AccessMode::ReadWrite,
        4,
        &[ScalarBitsV1::u32(0)],
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap()
}

struct RaceFixture {
    report: SimulationFailureReductionReportV1,
    race: SimulationAgentRaceEvidenceV1,
}

fn race_fixture() -> RaceFixture {
    let module = admitted("agent-c5::race", "generic_race");
    let request = SimulationRequestV1::new(
        "generic_race",
        [2, 1, 1],
        [2, 1, 1],
        vec![SimulationArgumentV1::Buffer(buffer())],
    );
    let limits = SimulationLimitsV1::default();
    let execution = module
        .simulate_scheduled(
            &request,
            SimulationTargetV1::amdgpu_64(),
            limits,
            SimulationScheduleRequestV1::RecordSeeded {
                seed: 29,
                max_decisions: 16,
            },
        )
        .unwrap();
    let SimulationRaceAssessmentV1::RacesObserved { first, .. } = execution.race_assessment()
    else {
        panic!("seeded fixture must retain its first exact race")
    };
    let report = module
        .reduce_simulation_failure(
            &request,
            SimulationTargetV1::amdgpu_64(),
            limits,
            SimulationFailureScheduleV1::Seeded { seed: 29 },
            SimulationFailureReductionLimitsV1::new(18, 16, 48).unwrap(),
        )
        .unwrap();
    assert!(report.matches_data_race(first));
    RaceFixture {
        report,
        race: SimulationAgentRaceEvidenceV1::from_simulation(first),
    }
}

fn race_open_request(
    request_id: u64,
    expected_revision: u64,
    fixture: &RaceFixture,
) -> SimulationAgentRequestV1 {
    let report_bytes = fixture.report.to_canonical_bytes().unwrap();
    SimulationAgentRequestV1::OpenRace {
        schema: SIMULATION_AGENT_REQUEST_SCHEMA_V1.to_owned(),
        request_id,
        expected_revision,
        reduction_report_hex: encode_evidence_hex_v1(&report_bytes).unwrap(),
        race: Box::new(fixture.race.clone()),
        expected_kir: VirtualKirEvidenceReferenceV1 {
            wire_version: fixture.report.kir_wire_version(),
            sha256: VirtualEvidenceIdentityV1::new(*fixture.report.kir_sha256()).unwrap(),
            canonical_bytes: fixture.report.kir_canonical_bytes(),
        },
        expected_context_identity: VirtualEvidenceIdentityV1::new(
            *fixture.report.context_identity(),
        )
        .unwrap(),
        expected_report_identity: VirtualEvidenceIdentityV1::new(*fixture.report.report_identity())
            .unwrap(),
    }
}

fn runtime(seed: u8) -> VirtualRuntimeV1 {
    VirtualRuntimeV1::new(VirtualRuntimeConfigV1 {
        runtime_identity: IdentityDigestV1::from_untrusted_bytes([seed; 32]),
        target: VirtualTargetProfileV1::Amdgpu64TargetNeutral,
        runtime_limits: VirtualRuntimeLimitsV1::default(),
        simulation_limits: SimulationLimitsV1::default(),
    })
    .unwrap()
}

fn host_evidence(
    blockers: usize,
    max_blockers: usize,
    input_bytes: usize,
) -> VirtualHostLifetimeEvidenceV1 {
    let mut runtime = runtime(61);
    let module = runtime
        .register_module(admitted("agent-c5::host", "generic_host"))
        .unwrap();
    let queue = runtime.create_queue(8).unwrap();
    let buffer = runtime
        .allocate_buffer(4, VirtualBufferAccessV1::ReadWrite)
        .unwrap();
    runtime.copy_from_host(buffer, 0, &[3; 4]).unwrap();
    for _ in 0..blockers {
        runtime
            .submit(
                queue,
                module,
                VirtualDispatchRequestV1 {
                    kernel: "generic_host".into(),
                    grid: [1, 1, 1],
                    workgroup: [1, 1, 1],
                    arguments: vec![VirtualArgumentV1::Buffer {
                        buffer,
                        element: ScalarType::U32,
                        access: AccessMode::ReadWrite,
                        alignment: 4,
                        byte_offset: 0,
                        elements: 1,
                    }],
                    dependencies: vec![],
                },
            )
            .unwrap();
    }
    assert!(matches!(
        runtime.release_buffer(buffer),
        Err(VirtualRuntimeErrorV1::Model(
            TransitionErrorV1::ResourceInUse(_)
        ))
    ));
    runtime
        .capture_host_lifetime_evidence_v1(
            buffer,
            VirtualHostLifetimeOperationV1::ReleaseBuffer,
            VirtualHostLifetimeEvidenceLimitsV1::new(max_blockers, input_bytes).unwrap(),
        )
        .unwrap()
}

fn host_open_request(
    request_id: u64,
    expected_revision: u64,
    evidence: &VirtualHostLifetimeEvidenceV1,
) -> SimulationAgentRequestV1 {
    SimulationAgentRequestV1::OpenHostLifetime {
        schema: SIMULATION_AGENT_REQUEST_SCHEMA_V1.to_owned(),
        request_id,
        expected_revision,
        evidence_hex: encode_evidence_hex_v1(&evidence.to_canonical_bytes().unwrap()).unwrap(),
        expected_runtime_identity: evidence.runtime_identity,
        expected_incident_identity: evidence.incident_identity,
    }
}

fn opened_session(response: &SimulationAgentResponseV1) -> VirtualEvidenceIdentityV1 {
    match response {
        SimulationAgentResponseV1::Ok {
            value:
                SimulationAgentResultV1::Opened {
                    session_identity, ..
                },
            ..
        } => *session_identity,
        other => panic!("expected opened response, got {other:?}"),
    }
}

fn run_process(requests: &[SimulationAgentRequestV1]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-sim-agent"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for request in requests {
        serde_json::to_writer(&mut input, request).unwrap();
        input.write_all(b"\n").unwrap();
    }
    drop(input);
    child.wait_with_output().unwrap()
}

#[test]
fn fresh_process_agent_diagnoses_and_pages_a_seeded_race_reduction() {
    let fixture = race_fixture();
    let mut service = SimulationAgentServiceV1::new().unwrap();
    let discover = SimulationAgentRequestV1::DiscoverCapabilities {
        schema: SIMULATION_AGENT_REQUEST_SCHEMA_V1.to_owned(),
        request_id: 1,
        expected_revision: 0,
    };
    service.handle(discover.clone()).unwrap();
    let open = race_open_request(2, 1, &fixture);
    let session = opened_session(&service.handle(open.clone()).unwrap());
    let diagnose = SimulationAgentRequestV1::Diagnose {
        schema: SIMULATION_AGENT_REQUEST_SCHEMA_V1.to_owned(),
        request_id: 3,
        expected_revision: 2,
        session_identity: session,
    };
    let response = service.handle(diagnose.clone()).unwrap();
    let SimulationAgentResponseV1::Ok {
        value: SimulationAgentResultV1::Diagnosis { diagnosis },
        ..
    } = response
    else {
        panic!("expected race diagnosis")
    };
    let SimulationAgentDiagnosisV1::Race { diagnosis, .. } = diagnosis else {
        panic!("expected race diagnosis kind")
    };
    assert_eq!(
        diagnosis.finding,
        SimulationAgentRaceFindingV1::UnorderedConflictingAccesses
    );
    assert_eq!(
        diagnosis.original_schedule,
        SimulationAgentOriginalScheduleV1::Seeded { seed: 29 }
    );
    assert!(diagnosis.reduction.locally_minimal);
    assert!(diagnosis.evidence_ids.len() >= 3);
    assert!(diagnosis.unavailable.iter().any(|fact| {
        fact.reason == SimulationAgentUnavailableReasonV1::ScheduleSpaceNotExhausted
    }));

    let reduce = SimulationAgentRequestV1::Reduce {
        schema: SIMULATION_AGENT_REQUEST_SCHEMA_V1.to_owned(),
        request_id: 4,
        expected_revision: 3,
        session_identity: session,
        page: SimulationAgentPageRequestV1 {
            limit: 1,
            cursor: None,
        },
    };
    let reduced = service.handle(reduce.clone()).unwrap();
    assert!(matches!(
        &reduced,
        SimulationAgentResponseV1::Ok {
            value: SimulationAgentResultV1::Reduction {
                reduction: SimulationAgentReductionPageV1 {
                    completeness:
                        SimulationAgentReductionCompletenessV1::SimulatorVerifiedLocallyMinimal,
                    ..
                }
            },
            ..
        }
    ));
    let next_cursor = match &reduced {
        SimulationAgentResponseV1::Ok {
            value: SimulationAgentResultV1::Reduction { reduction },
            ..
        } => {
            assert!(reduction.total_items >= 2);
            reduction.next_cursor.expect("one-item page must continue")
        }
        _ => unreachable!(),
    };
    assert!(matches!(
        service
            .handle(SimulationAgentRequestV1::Reduce {
                schema: SIMULATION_AGENT_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 40,
                expected_revision: 4,
                session_identity: session,
                page: SimulationAgentPageRequestV1 {
                    limit: MAX_SIMULATION_AGENT_PAGE_ITEMS_V1,
                    cursor: Some(next_cursor),
                },
            })
            .unwrap(),
        SimulationAgentResponseV1::Ok {
            value: SimulationAgentResultV1::Reduction { .. },
            ..
        }
    ));
    let terminate = SimulationAgentRequestV1::Terminate {
        schema: SIMULATION_AGENT_REQUEST_SCHEMA_V1.to_owned(),
        request_id: 5,
        expected_revision: 4,
        session_identity: session,
    };
    let requests = [discover, open, diagnose, reduce, terminate];
    let first = run_process(&requests);
    let second = run_process(&requests);
    assert!(first.status.success());
    assert!(first.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);
    for line in first.stdout.split_inclusive(|byte| *byte == b'\n') {
        validate_simulation_agent_response_line_v1(line).unwrap();
    }
}

#[test]
fn fresh_process_agent_diagnoses_and_reduces_host_lifetime_misuse() {
    let evidence = host_evidence(1, 8, 1 << 20);
    let mut service = SimulationAgentServiceV1::new().unwrap();
    let discover = SimulationAgentRequestV1::DiscoverCapabilities {
        schema: SIMULATION_AGENT_REQUEST_SCHEMA_V1.to_owned(),
        request_id: 10,
        expected_revision: 0,
    };
    service.handle(discover.clone()).unwrap();
    let open = host_open_request(11, 1, &evidence);
    let session = opened_session(&service.handle(open.clone()).unwrap());
    let diagnose = SimulationAgentRequestV1::Diagnose {
        schema: SIMULATION_AGENT_REQUEST_SCHEMA_V1.to_owned(),
        request_id: 12,
        expected_revision: 2,
        session_identity: session,
    };
    let diagnosed = service.handle(diagnose.clone()).unwrap();
    let SimulationAgentResponseV1::Ok {
        value:
            SimulationAgentResultV1::Diagnosis {
                diagnosis: SimulationAgentDiagnosisV1::HostLifetime { diagnosis, .. },
            },
        ..
    } = diagnosed
    else {
        panic!("expected host-lifetime diagnosis")
    };
    assert_eq!(diagnosis.retained_dispatches, 1);
    let reduce = SimulationAgentRequestV1::Reduce {
        schema: SIMULATION_AGENT_REQUEST_SCHEMA_V1.to_owned(),
        request_id: 13,
        expected_revision: 3,
        session_identity: session,
        page: SimulationAgentPageRequestV1 {
            limit: 1,
            cursor: None,
        },
    };
    let reduced = service.handle(reduce.clone()).unwrap();
    assert!(matches!(
        reduced,
        SimulationAgentResponseV1::Ok {
            value: SimulationAgentResultV1::Reduction {
                reduction: SimulationAgentReductionPageV1 {
                    completeness: SimulationAgentReductionCompletenessV1::MinimumPositiveWitnessFromCompleteIncident,
                    total_items: 1,
                    ..
                }
            },
            ..
        }
    ));
    let terminate = SimulationAgentRequestV1::Terminate {
        schema: SIMULATION_AGENT_REQUEST_SCHEMA_V1.to_owned(),
        request_id: 14,
        expected_revision: 4,
        session_identity: session,
    };
    let output = run_process(&[discover, open, diagnose, reduce, terminate]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    for line in output.stdout.split_inclusive(|byte| *byte == b'\n') {
        validate_simulation_agent_response_line_v1(line).unwrap();
    }
}

#[test]
fn hostile_identity_cursor_encoding_and_partial_evidence_fail_closed() {
    assert!(decode_simulation_agent_request_line_v1(
        br#"{"operation":"discover_capabilities","schema":"fe2o3-sim-agent-request-v1","request_id":1,"expected_revision":0,"unknown":true}\n"#,
    )
    .is_err());
    let fixture = race_fixture();
    let mut mismatched = race_open_request(1, 0, &fixture);
    let SimulationAgentRequestV1::OpenRace {
        expected_context_identity,
        ..
    } = &mut mismatched
    else {
        unreachable!()
    };
    *expected_context_identity = VirtualEvidenceIdentityV1::new([88; 32]).unwrap();
    let mut service = SimulationAgentServiceV1::new().unwrap();
    assert!(matches!(
        service.handle(mismatched).unwrap(),
        SimulationAgentResponseV1::Error {
            code: SimulationAgentErrorCodeV1::EvidenceIdentityMismatch,
            ..
        }
    ));

    let mut uppercase = race_open_request(2, 1, &fixture);
    let SimulationAgentRequestV1::OpenRace {
        reduction_report_hex,
        ..
    } = &mut uppercase
    else {
        unreachable!()
    };
    reduction_report_hex.make_ascii_uppercase();
    assert!(matches!(
        service.handle(uppercase).unwrap(),
        SimulationAgentResponseV1::Error {
            code: SimulationAgentErrorCodeV1::InvalidEvidenceEncoding,
            ..
        }
    ));

    let mut corrupt_race = race_open_request(3, 2, &fixture);
    let SimulationAgentRequestV1::OpenRace { race, .. } = &mut corrupt_race else {
        unreachable!()
    };
    race.allocation = race.allocation.saturating_add(1);
    assert!(matches!(
        service.handle(corrupt_race).unwrap(),
        SimulationAgentResponseV1::Error {
            code: SimulationAgentErrorCodeV1::EvidenceIdentityMismatch,
            ..
        }
    ));

    let partial = host_evidence(2, 1, 0);
    assert!(matches!(
        partial.completeness,
        VirtualHostLifetimeCompletenessV1::PartialBlockerAndInputIdentity { .. }
    ));
    assert!(matches!(
        partial.blockers[0].dispatch_input,
        VirtualDispatchInputBindingV1::Unavailable { .. }
    ));
    let mut partial_service = SimulationAgentServiceV1::new().unwrap();
    let mut substituted_host = host_open_request(40, 0, &partial);
    let SimulationAgentRequestV1::OpenHostLifetime {
        expected_incident_identity,
        ..
    } = &mut substituted_host
    else {
        unreachable!()
    };
    *expected_incident_identity = VirtualEvidenceIdentityV1::new([66; 32]).unwrap();
    assert!(matches!(
        partial_service.handle(substituted_host).unwrap(),
        SimulationAgentResponseV1::Error {
            code: SimulationAgentErrorCodeV1::EvidenceIdentityMismatch,
            ..
        }
    ));
    let session = opened_session(
        &partial_service
            .handle(host_open_request(1, 1, &partial))
            .unwrap(),
    );
    let diagnosed = partial_service
        .handle(SimulationAgentRequestV1::Diagnose {
            schema: SIMULATION_AGENT_REQUEST_SCHEMA_V1.to_owned(),
            request_id: 2,
            expected_revision: 2,
            session_identity: session,
        })
        .unwrap();
    let SimulationAgentResponseV1::Ok {
        value:
            SimulationAgentResultV1::Diagnosis {
                diagnosis: SimulationAgentDiagnosisV1::HostLifetime { diagnosis, .. },
            },
        ..
    } = diagnosed
    else {
        panic!("expected partial host diagnosis")
    };
    assert!(diagnosis.unavailable.iter().any(|fact| {
        fact.reason == SimulationAgentUnavailableReasonV1::DispatchInputIdentityByteLimit
    }));
    assert!(diagnosis.unavailable.iter().any(|fact| {
        fact.reason == SimulationAgentUnavailableReasonV1::BlockerInventoryTruncated
    }));

    let wrong_cursor = SimulationAgentPageCursorV1 {
        start: 1,
        identity: VirtualEvidenceIdentityV1::new([77; 32]).unwrap(),
    };
    assert!(matches!(
        partial_service
            .handle(SimulationAgentRequestV1::Reduce {
                schema: SIMULATION_AGENT_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 3,
                expected_revision: 3,
                session_identity: session,
                page: SimulationAgentPageRequestV1 {
                    limit: 1,
                    cursor: Some(wrong_cursor),
                },
            })
            .unwrap(),
        SimulationAgentResponseV1::Error {
            code: SimulationAgentErrorCodeV1::CursorMismatch,
            ..
        }
    ));
    assert!(matches!(
        partial_service
            .handle(SimulationAgentRequestV1::Reduce {
                schema: SIMULATION_AGENT_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 4,
                expected_revision: 4,
                session_identity: session,
                page: SimulationAgentPageRequestV1 {
                    limit: 0,
                    cursor: None,
                },
            })
            .unwrap(),
        SimulationAgentResponseV1::Error {
            code: SimulationAgentErrorCodeV1::InvalidPage,
            ..
        }
    ));
}

#[test]
fn additive_agent_contract_leaves_existing_virtual_runtime_schema_unchanged() {
    assert_ne!(
        SIMULATION_AGENT_REQUEST_SCHEMA_V1,
        fe2o3_virtual_runtime::VIRTUAL_RUNTIME_SCHEMA_V1
    );
    assert_ne!(
        SIMULATION_AGENT_RESPONSE_SCHEMA_V1,
        fe2o3_virtual_runtime::VIRTUAL_RUNTIME_OUTCOME_SCHEMA_V1
    );
    let mut service = SimulationAgentServiceV1::new().unwrap();
    let response = service
        .handle(SimulationAgentRequestV1::DiscoverCapabilities {
            schema: SIMULATION_AGENT_REQUEST_SCHEMA_V1.to_owned(),
            request_id: 1,
            expected_revision: 0,
        })
        .unwrap();
    assert!(matches!(
        response,
        SimulationAgentResponseV1::Ok {
            value: SimulationAgentResultV1::Capabilities {
                capabilities: SimulationAgentCapabilitiesV1 {
                    authority: SimulationAgentAuthorityV1::AdvisoryReadOnlyNoExecutionFileNetworkOrPatchAuthority,
                    ..
                }
            },
            ..
        }
    ));
}
