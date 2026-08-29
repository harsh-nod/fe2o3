use fe2o3_debug_protocol::*;

fn identity(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).unwrap()
}

fn session() -> SessionViewV1 {
    let configuration = identity(1);
    SessionViewV1 {
        backend: DebugBackendV1::CpuKirSimulator,
        execution_kind: ExecutionKindV1::CpuKirSimulation,
        state: SessionStateV1::Created,
        revision: 0,
        configuration_identity: configuration,
        cursor: DebugCursorV1 {
            configuration_identity: configuration,
            event_sequence: 0,
            state_revision: 0,
        },
        simulated: true,
        hardware_observed: false,
        performance_prediction: false,
    }
}

fn context() -> DiagnosisExecutionContextV2 {
    DiagnosisExecutionContextV2 {
        dispatch: DiagnosisFactV2::Declared {
            value: DiagnosisDispatchV2 {
                launch_extent: [4, 1, 1],
                workgroup_size: [4, 1, 1],
            },
        },
        workgroup: DiagnosisFactV2::Observed { value: [0, 0, 0] },
        workitem: DiagnosisFactV2::Observed {
            value: DiagnosisWorkitemV2 {
                global: [1, 0, 0],
                local: [1, 0, 0],
            },
        },
        wave: DiagnosisFactV2::Inferred {
            value: DiagnosisLogicalWaveV2 {
                wave: 0,
                width: 32,
                active_mask: 0b1111,
            },
            basis: DiagnosisInferenceBasisV2::LogicalWavePartition,
        },
        lane: DiagnosisFactV2::Inferred {
            value: 1,
            basis: DiagnosisInferenceBasisV2::LogicalWavePartition,
        },
    }
}

fn out_of_bounds_response() -> DiagnosisResponseV2 {
    DiagnosisResponseV2::Ok {
        schema: DiagnosisResponseSchemaV2::V2,
        request_id: 7,
        operation: DiagnosisOperationV2::Diagnose,
        session: session(),
        completeness: CaptureCompletenessV1::Complete,
        diagnoses: vec![DiagnosisViewV2 {
            sequence: 11,
            class: DiagnosisClassV2::MemoryOutOfBounds,
            context: context(),
            site: DiagnosisFactV2::Observed {
                value: KirSiteV1 {
                    function_ordinal: 0,
                    block_ordinal: 0,
                    point: KirSitePointV1::Operation {
                        operation_ordinal: 2,
                    },
                },
            },
            memory_region: DiagnosisFactV2::Observed {
                value: DiagnosisMemoryRegionV2 {
                    allocation: AllocationIdentityV1 {
                        ordinal: 1,
                        generation: 0,
                    },
                    requested_offset: 4,
                    requested_bytes: 4,
                    allocation_bytes: 4,
                },
            },
            barrier: DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::NotApplicable,
            },
        }],
        next_cursor: None,
    }
}

fn reencode(response: &DiagnosisResponseV2) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(response).unwrap();
    bytes.push(b'\n');
    bytes
}

#[test]
fn diagnosis_request_is_closed_bounded_and_separately_versioned() {
    let valid = br#"{"operation":"diagnose","schema":"fe2o3-debug-diagnosis-request-v2","request_id":7,"expected_revision":3,"filter":{"class":"memory_out_of_bounds","scope":{"level":"lane","workgroup":[0,0,0],"wave":0,"lane":1}},"page":{"limit":8}}
"#;
    let decoded = decode_diagnosis_request_line_v2(valid, ProtocolLimitsV1::default()).unwrap();
    assert_eq!(decoded.request_id(), 7);
    assert_eq!(decoded.expected_revision(), 3);
    assert!(matches!(
        decode_request_line_v1(valid, ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::InvalidJson)
    ));

    for hostile in [
        br#"{"operation":"diagnose","schema":"fe2o3-debug-diagnosis-request-v2","request_id":7,"request_id":8,"expected_revision":3,"page":{"limit":8}}
"#.as_slice(),
        br#"{"operation":"diagnose","schema":"fe2o3-debug-diagnosis-request-v2","request_id":0,"expected_revision":3,"page":{"limit":8}}
"#.as_slice(),
        br#"{"operation":"diagnose","schema":"fe2o3-debug-diagnosis-request-v2","request_id":7,"expected_revision":3,"page":{"limit":0}}
"#.as_slice(),
        br#"{"operation":"diagnose","schema":"fe2o3-debug-diagnosis-request-v2","request_id":7,"expected_revision":3,"filter":{"scope":{"level":"lane","workgroup":[0,0,0],"wave":0,"lane":64}},"page":{"limit":8}}
"#.as_slice(),
        br#"{"operation":"diagnose","schema":"fe2o3-debug-diagnosis-request-v2","request_id":7,"expected_revision":3,"filter":{},"page":{"limit":8},"native_address":1}
"#.as_slice(),
    ] {
        assert!(
            decode_diagnosis_request_line_v2(hostile, ProtocolLimitsV1::default()).is_err(),
            "hostile request was accepted: {}",
            String::from_utf8_lossy(hostile)
        );
    }
}

#[test]
fn out_of_bounds_response_round_trips_with_explicit_fact_origins() {
    let response = out_of_bounds_response();
    let encoded =
        encode_diagnosis_response_line_v2(&response, ProtocolLimitsV1::default()).unwrap();
    let text = std::str::from_utf8(&encoded).unwrap();
    assert!(text.contains("\"origin\":\"declared\""));
    assert!(text.contains("\"origin\":\"observed\""));
    assert!(text.contains("\"origin\":\"inferred\""));
    assert!(text.contains("\"origin\":\"unavailable\""));
    assert_eq!(
        decode_diagnosis_response_line_v2(&encoded, ProtocolLimitsV1::default()).unwrap(),
        response
    );
}

#[test]
fn response_rejects_truth_range_hierarchy_and_cursor_substitution() {
    let mut hardware = out_of_bounds_response();
    let DiagnosisResponseV2::Ok { session, .. } = &mut hardware else {
        unreachable!()
    };
    session.backend = DebugBackendV1::KfdHardware;
    session.execution_kind = ExecutionKindV1::KfdHardware;
    session.simulated = false;
    session.hardware_observed = true;
    assert!(matches!(
        decode_diagnosis_response_line_v2(&reencode(&hardware), ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::InvalidTruthClassification
        ))
    ));

    let mut forged_origin = out_of_bounds_response();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut forged_origin else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed { value } = &diagnoses[0].memory_region else {
        unreachable!()
    };
    diagnoses[0].memory_region = DiagnosisFactV2::Inferred {
        value: *value,
        basis: DiagnosisInferenceBasisV2::LaunchGeometry,
    };
    assert!(
        decode_diagnosis_response_line_v2(&reencode(&forged_origin), ProtocolLimitsV1::default())
            .is_err()
    );

    let mut in_bounds = out_of_bounds_response();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut in_bounds else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed { value } = &mut diagnoses[0].memory_region else {
        unreachable!()
    };
    value.allocation_bytes = 8;
    assert!(matches!(
        decode_diagnosis_response_line_v2(&reencode(&in_bounds), ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::InvalidRange("diagnosis out-of-bounds region")
        ))
    ));

    let mut hierarchy = out_of_bounds_response();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut hierarchy else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed { value } = &mut diagnoses[0].context.workitem else {
        unreachable!()
    };
    value.global[0] = 2;
    assert!(matches!(
        decode_diagnosis_response_line_v2(&reencode(&hierarchy), ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::IdentityMismatch("diagnosis workitem hierarchy")
        ))
    ));

    let mut forged_mask = out_of_bounds_response();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut forged_mask else {
        unreachable!()
    };
    let DiagnosisFactV2::Inferred { value: wave, .. } = &mut diagnoses[0].context.wave else {
        unreachable!()
    };
    wave.active_mask = 0b0011;
    assert!(matches!(
        decode_diagnosis_response_line_v2(&reencode(&forged_mask), ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::IdentityMismatch("diagnosis logical hierarchy")
        ))
    ));

    let mut cursor = out_of_bounds_response();
    let DiagnosisResponseV2::Ok { next_cursor, .. } = &mut cursor else {
        unreachable!()
    };
    *next_cursor = Some(PageCursorV1 {
        query_identity: identity(2),
        position: 0,
    });
    assert!(
        decode_diagnosis_response_line_v2(&reencode(&cursor), ProtocolLimitsV1::default()).is_err()
    );
}

#[test]
fn barrier_divergence_requires_observed_phase_and_distinct_origin_domains() {
    let participant = |local: u32| DiagnosisBarrierParticipantV2 {
        local_workitem: DiagnosisFactV2::Observed {
            value: [local, 0, 0],
        },
        global_workitem: DiagnosisFactV2::Inferred {
            value: [u64::from(local), 0, 0],
            basis: DiagnosisInferenceBasisV2::LaunchGeometry,
        },
        wave: DiagnosisFactV2::Inferred {
            value: 0,
            basis: DiagnosisInferenceBasisV2::LogicalWavePartition,
        },
        lane: DiagnosisFactV2::Inferred {
            value: u16::try_from(local).unwrap(),
            basis: DiagnosisInferenceBasisV2::LogicalWavePartition,
        },
    };
    let response = DiagnosisResponseV2::Ok {
        schema: DiagnosisResponseSchemaV2::V2,
        request_id: 9,
        operation: DiagnosisOperationV2::Diagnose,
        session: session(),
        completeness: CaptureCompletenessV1::Complete,
        diagnoses: vec![DiagnosisViewV2 {
            sequence: 12,
            class: DiagnosisClassV2::WorkgroupBarrierDivergence,
            context: context(),
            site: DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::SiteNotRepresented,
            },
            memory_region: DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::NotApplicable,
            },
            barrier: DiagnosisFactV2::Observed {
                value: DiagnosisBarrierV2::Divergence {
                    phase: DiagnosisFactV2::Observed { value: 0 },
                    observed_arrivals: DiagnosisFactV2::Observed { value: 1 },
                    expected_participants: DiagnosisFactV2::Inferred {
                        value: 4,
                        basis: DiagnosisInferenceBasisV2::LaunchGeometry,
                    },
                    waiting: participant(1),
                    exited: participant(0),
                },
            },
        }],
        next_cursor: None,
    };
    let encoded =
        encode_diagnosis_response_line_v2(&response, ProtocolLimitsV1::default()).unwrap();
    assert!(decode_diagnosis_response_line_v2(&encoded, ProtocolLimitsV1::default()).is_ok());

    let mut forged = response.clone();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut forged else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed {
        value: DiagnosisBarrierV2::Divergence { phase, .. },
    } = &mut diagnoses[0].barrier
    else {
        unreachable!()
    };
    *phase = DiagnosisFactV2::Inferred {
        value: 0,
        basis: DiagnosisInferenceBasisV2::LaunchGeometry,
    };
    assert!(
        decode_diagnosis_response_line_v2(&reencode(&forged), ProtocolLimitsV1::default()).is_err()
    );

    let mut forged_count = response;
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut forged_count else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed {
        value:
            DiagnosisBarrierV2::Divergence {
                expected_participants,
                ..
            },
    } = &mut diagnoses[0].barrier
    else {
        unreachable!()
    };
    *expected_participants = DiagnosisFactV2::Inferred {
        value: 3,
        basis: DiagnosisInferenceBasisV2::LaunchGeometry,
    };
    assert!(matches!(
        decode_diagnosis_response_line_v2(&reencode(&forged_count), ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::IdentityMismatch("diagnosis barrier participant count")
        ))
    ));
}
