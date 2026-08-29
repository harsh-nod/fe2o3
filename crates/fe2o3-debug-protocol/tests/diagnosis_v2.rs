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

fn content(byte: u8, canonical_bytes: u64) -> DiagnosisContentReferenceV2 {
    DiagnosisContentReferenceV2 {
        sha256: identity(byte),
        canonical_bytes,
    }
}

fn input_evidence() -> DiagnosisInputEvidenceV2 {
    let dispatch_request = content(2, 128);
    let canonical_kir_v7 = content(3, 512);
    DiagnosisInputEvidenceV2 {
        configuration_identity: identity(1),
        dispatch_identity: diagnosis_dispatch_input_identity_v2(dispatch_request, canonical_kir_v7)
            .unwrap(),
        dispatch_request: DiagnosisFactV2::Declared {
            value: dispatch_request,
        },
        canonical_kir_v7: DiagnosisFactV2::Declared {
            value: canonical_kir_v7,
        },
        simulation_bundle: DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::InputNotProvided,
        },
        production_kir: DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::InputNotProvided,
        },
        kernel_abi_identity: DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::InputNotProvided,
        },
        source_lineage: DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::InputNotProvided,
        },
        source_map_v2: DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::InputNotProvided,
        },
        finalized_artifact: DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::NoArtifactAuthority,
        },
        property_proof: DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::NoProofAuthority,
        },
    }
}

fn no_source_operation() -> DiagnosisFactV2<DiagnosisSourceOperationV2> {
    DiagnosisFactV2::Unavailable {
        reason: DiagnosisUnavailableReasonV2::InputNotProvided,
    }
}

fn barrier_semantics(ordering: DiagnosisMemoryOrderingV2) -> DiagnosisBarrierSemanticsV2 {
    DiagnosisBarrierSemanticsV2 {
        memory_scope: DiagnosisSynchronizationScopeV2::Workgroup,
        ordering,
        address_spaces: vec![AddressSpaceV1::Workgroup],
    }
}

fn lds_epoch() -> DiagnosisLdsEpochV2 {
    DiagnosisLdsEpochV2 {
        current: DiagnosisFactV2::Observed { value: 0 },
        after_release: DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::BarrierNotReleased,
        },
    }
}

fn participant(local: u32) -> DiagnosisBarrierParticipantV2 {
    DiagnosisBarrierParticipantV2 {
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
    }
}

fn divergence_response() -> DiagnosisResponseV2 {
    DiagnosisResponseV2::Ok {
        schema: DiagnosisResponseSchemaV2::V2,
        request_id: 9,
        operation: DiagnosisOperationV2::Diagnose,
        session: session(),
        completeness: CaptureCompletenessV1::Complete,
        diagnoses: vec![DiagnosisViewV2 {
            sequence: 12,
            class: DiagnosisClassV2::WorkgroupBarrierDivergence,
            input: input_evidence(),
            context: context(),
            site: DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::SiteNotRepresented,
            },
            source_operation: no_source_operation(),
            memory_region: DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::NotApplicable,
            },
            barrier: DiagnosisFactV2::Observed {
                value: DiagnosisBarrierV2::Divergence {
                    phase: DiagnosisFactV2::Observed { value: 0 },
                    semantics: DiagnosisFactV2::Declared {
                        value: barrier_semantics(DiagnosisMemoryOrderingV2::AcquireRelease),
                    },
                    lds_epoch: lds_epoch(),
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
    }
}

fn kir_site(operation_ordinal: u64) -> KirSiteV1 {
    KirSiteV1 {
        function_ordinal: 0,
        block_ordinal: 0,
        point: KirSitePointV1::Operation { operation_ordinal },
    }
}

fn mismatch_response(
    mismatch: DiagnosisBarrierMismatchV2,
    actual_operation: u64,
    expected_operation: u64,
) -> DiagnosisResponseV2 {
    DiagnosisResponseV2::Ok {
        schema: DiagnosisResponseSchemaV2::V2,
        request_id: 10,
        operation: DiagnosisOperationV2::Diagnose,
        session: session(),
        completeness: CaptureCompletenessV1::Complete,
        diagnoses: vec![DiagnosisViewV2 {
            sequence: 13,
            class: DiagnosisClassV2::WorkgroupBarrierMismatch,
            input: input_evidence(),
            context: context(),
            site: DiagnosisFactV2::Observed {
                value: kir_site(actual_operation),
            },
            source_operation: no_source_operation(),
            memory_region: DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::NotApplicable,
            },
            barrier: DiagnosisFactV2::Observed {
                value: DiagnosisBarrierV2::Mismatch {
                    phase: DiagnosisFactV2::Observed { value: 0 },
                    semantics: DiagnosisFactV2::Declared {
                        value: barrier_semantics(DiagnosisMemoryOrderingV2::AcquireRelease),
                    },
                    expected_semantics: DiagnosisFactV2::Declared {
                        value: barrier_semantics(if mismatch == DiagnosisBarrierMismatchV2::Site {
                            DiagnosisMemoryOrderingV2::AcquireRelease
                        } else {
                            DiagnosisMemoryOrderingV2::SequentiallyConsistent
                        }),
                    },
                    lds_epoch: lds_epoch(),
                    expected_participants: DiagnosisFactV2::Inferred {
                        value: 4,
                        basis: DiagnosisInferenceBasisV2::LaunchGeometry,
                    },
                    mismatch: DiagnosisFactV2::Observed { value: mismatch },
                    expected_site: DiagnosisFactV2::Observed {
                        value: kir_site(expected_operation),
                    },
                },
            },
        }],
        next_cursor: None,
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
            input: input_evidence(),
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
            source_operation: no_source_operation(),
            memory_region: DiagnosisFactV2::Observed {
                value: DiagnosisMemoryRegionV2 {
                    allocation: AllocationIdentityV1 {
                        ordinal: 1,
                        generation: 0,
                    },
                    requested_offset: 4,
                    requested_bytes: 4,
                    allocation_bytes: 4,
                    allocation_contract: DiagnosisFactV2::Declared {
                        value: DiagnosisAllocationContractV2 {
                            address_space: AddressSpaceV1::Global,
                            access: DiagnosisAccessModeV2::ReadWrite,
                            alignment: 4,
                            allocation_bytes: 4,
                            abi_argument: DiagnosisFactV2::Declared {
                                value: DiagnosisAbiArgumentV2 {
                                    ordinal: 0,
                                    kind: DiagnosisAbiArgumentKindV2::Slice,
                                    element: DiagnosisScalarTypeV2::U32,
                                    address_space: AddressSpaceV1::Global,
                                    access: DiagnosisAccessModeV2::ReadWrite,
                                    view_offset: 0,
                                    view_bytes: 4,
                                },
                            },
                        },
                    },
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
        value: value.clone(),
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
fn response_rejects_input_source_abi_and_authority_substitution() {
    let mut configuration = out_of_bounds_response();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut configuration else {
        unreachable!()
    };
    diagnoses[0].input.configuration_identity = identity(9);
    assert!(matches!(
        decode_diagnosis_response_line_v2(&reencode(&configuration), ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::IdentityMismatch("diagnosis configuration")
        ))
    ));

    let mut dispatch = out_of_bounds_response();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut dispatch else {
        unreachable!()
    };
    diagnoses[0].input.dispatch_identity = identity(9);
    assert!(matches!(
        decode_diagnosis_response_line_v2(&reencode(&dispatch), ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::IdentityMismatch("diagnosis dispatch input")
        ))
    ));

    let mut source = out_of_bounds_response();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut source else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed { value: site } = diagnoses[0].site else {
        unreachable!()
    };
    diagnoses[0].input.source_map_v2 = DiagnosisFactV2::Declared {
        value: DiagnosisSourceMapReferenceV2 {
            identity: identity(7),
            bundle_subject_identity: identity(8),
            provenance: SourceMapProvenanceV1::CallerBound,
        },
    };
    diagnoses[0].source_operation = DiagnosisFactV2::Declared {
        value: DiagnosisSourceOperationV2 {
            bundle_subject_identity: identity(8),
            kir_site: site,
            location: SourceLocationV1 {
                map_identity: identity(9),
                provenance: SourceMapProvenanceV1::CallerBound,
                file_identity: identity(10),
                byte_start: 4,
                byte_end: 8,
            },
        },
    };
    assert!(matches!(
        decode_diagnosis_response_line_v2(&reencode(&source), ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::IdentityMismatch("diagnosis source operation")
        ))
    ));

    let mut bundle_subject = out_of_bounds_response();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut bundle_subject else {
        unreachable!()
    };
    diagnoses[0].input.simulation_bundle = DiagnosisFactV2::Declared {
        value: DiagnosisBundleReferenceV2 {
            identity: identity(12),
            subject_identity: identity(13),
        },
    };
    diagnoses[0].input.production_kir = DiagnosisFactV2::Declared {
        value: DiagnosisVersionedContentReferenceV2 {
            version: 8,
            content: content(14, 256),
        },
    };
    diagnoses[0].input.kernel_abi_identity = DiagnosisFactV2::Declared {
        value: identity(15),
    };
    diagnoses[0].input.source_lineage = DiagnosisFactV2::Declared {
        value: DiagnosisSourceLineageV2 {
            identity_inventory_receipt: content(16, 64),
            preflight_plan_receipt: content(17, 64),
        },
    };
    diagnoses[0].input.source_map_v2 = DiagnosisFactV2::Declared {
        value: DiagnosisSourceMapReferenceV2 {
            identity: identity(18),
            bundle_subject_identity: identity(19),
            provenance: SourceMapProvenanceV1::CompilerBundleBound,
        },
    };
    assert!(matches!(
        decode_diagnosis_response_line_v2(&reencode(&bundle_subject), ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::IdentityMismatch("diagnosis source map bundle subject")
        ))
    ));

    let mut abi = out_of_bounds_response();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut abi else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed { value: region } = &mut diagnoses[0].memory_region else {
        unreachable!()
    };
    let DiagnosisFactV2::Declared { value: allocation } = &mut region.allocation_contract else {
        unreachable!()
    };
    let DiagnosisFactV2::Declared { value: argument } = &mut allocation.abi_argument else {
        unreachable!()
    };
    argument.view_offset = 4;
    assert!(matches!(
        decode_diagnosis_response_line_v2(&reencode(&abi), ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::InvalidRange("diagnosis ABI view")
        ))
    ));

    let mut authority = out_of_bounds_response();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut authority else {
        unreachable!()
    };
    diagnoses[0].input.property_proof = DiagnosisFactV2::Declared {
        value: identity(11),
    };
    assert!(matches!(
        decode_diagnosis_response_line_v2(&reencode(&authority), ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::InvalidTruthClassification
        ))
    ));
}

#[test]
fn barrier_response_rejects_semantics_and_lds_epoch_substitution() {
    let mut lds = divergence_response();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut lds else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed {
        value: DiagnosisBarrierV2::Divergence { lds_epoch, .. },
    } = &mut diagnoses[0].barrier
    else {
        unreachable!()
    };
    lds_epoch.current = DiagnosisFactV2::Observed { value: 1 };
    assert!(matches!(
        decode_diagnosis_response_line_v2(&reencode(&lds), ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::InvalidTruthClassification
        ))
    ));

    let mut mismatch = mismatch_response(DiagnosisBarrierMismatchV2::Semantics, 2, 2);
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut mismatch else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed {
        value: DiagnosisBarrierV2::Mismatch {
            expected_semantics, ..
        },
    } = &mut diagnoses[0].barrier
    else {
        unreachable!()
    };
    *expected_semantics = DiagnosisFactV2::Declared {
        value: barrier_semantics(DiagnosisMemoryOrderingV2::AcquireRelease),
    };
    assert!(matches!(
        decode_diagnosis_response_line_v2(&reencode(&mismatch), ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::IdentityMismatch("diagnosis barrier mismatch semantics")
        ))
    ));
}

#[test]
fn barrier_divergence_requires_observed_phase_and_distinct_origin_domains() {
    let response = divergence_response();
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

#[test]
fn divergence_rejects_participant_context_and_completeness_substitution() {
    let response = divergence_response();

    let mut same_participant = response.clone();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut same_participant else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed {
        value: DiagnosisBarrierV2::Divergence {
            waiting, exited, ..
        },
    } = &mut diagnoses[0].barrier
    else {
        unreachable!()
    };
    *exited = waiting.clone();
    assert!(matches!(
        decode_diagnosis_response_line_v2(
            &reencode(&same_participant),
            ProtocolLimitsV1::default()
        ),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::IdentityMismatch("diagnosis barrier participants")
        ))
    ));

    let mut third_lane_context = response.clone();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut third_lane_context else {
        unreachable!()
    };
    diagnoses[0].context.workitem = DiagnosisFactV2::Observed {
        value: DiagnosisWorkitemV2 {
            global: [2, 0, 0],
            local: [2, 0, 0],
        },
    };
    diagnoses[0].context.wave = DiagnosisFactV2::Inferred {
        value: DiagnosisLogicalWaveV2 {
            wave: 0,
            width: 32,
            active_mask: 0b1111,
        },
        basis: DiagnosisInferenceBasisV2::LogicalWavePartition,
    };
    diagnoses[0].context.lane = DiagnosisFactV2::Inferred {
        value: 2,
        basis: DiagnosisInferenceBasisV2::LogicalWavePartition,
    };
    assert!(matches!(
        decode_diagnosis_response_line_v2(
            &reencode(&third_lane_context),
            ProtocolLimitsV1::default()
        ),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::IdentityMismatch("diagnosis barrier waiting context")
        ))
    ));

    for reason in [
        DiagnosisUnavailableReasonV2::NotCaptured,
        DiagnosisUnavailableReasonV2::TranscriptTruncated,
    ] {
        let mut complete_unavailable = response.clone();
        set_arrivals(
            &mut complete_unavailable,
            DiagnosisFactV2::Unavailable { reason },
        );
        assert!(
            decode_diagnosis_response_line_v2(
                &reencode(&complete_unavailable),
                ProtocolLimitsV1::default()
            )
            .is_err()
        );
    }

    let truncated = CaptureCompletenessV1::Truncated {
        reason: CaptureTruncationReasonV1::EventLimit,
        emitted_events: 1,
        dropped_events: None,
    };
    for arrivals in [
        DiagnosisFactV2::Observed { value: 1 },
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::NotCaptured,
        },
    ] {
        let mut truncated_invalid = response.clone();
        let DiagnosisResponseV2::Ok { completeness, .. } = &mut truncated_invalid else {
            unreachable!()
        };
        *completeness = truncated;
        set_arrivals(&mut truncated_invalid, arrivals);
        assert!(
            decode_diagnosis_response_line_v2(
                &reencode(&truncated_invalid),
                ProtocolLimitsV1::default()
            )
            .is_err()
        );
    }

    let mut truncated_valid = response;
    let DiagnosisResponseV2::Ok { completeness, .. } = &mut truncated_valid else {
        unreachable!()
    };
    *completeness = truncated;
    set_arrivals(
        &mut truncated_valid,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::TranscriptTruncated,
        },
    );
    assert!(
        decode_diagnosis_response_line_v2(&reencode(&truncated_valid), ProtocolLimitsV1::default())
            .is_ok()
    );
}

fn set_arrivals(response: &mut DiagnosisResponseV2, arrivals: DiagnosisFactV2<u32>) {
    let DiagnosisResponseV2::Ok { diagnoses, .. } = response else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed {
        value: DiagnosisBarrierV2::Divergence {
            observed_arrivals, ..
        },
    } = &mut diagnoses[0].barrier
    else {
        unreachable!()
    };
    *observed_arrivals = arrivals;
}

#[test]
fn barrier_mismatch_kind_is_joined_to_actual_and_expected_sites() {
    for response in [
        mismatch_response(DiagnosisBarrierMismatchV2::Semantics, 2, 2),
        mismatch_response(DiagnosisBarrierMismatchV2::Site, 2, 3),
        mismatch_response(DiagnosisBarrierMismatchV2::SiteAndSemantics, 2, 3),
    ] {
        assert!(
            decode_diagnosis_response_line_v2(&reencode(&response), ProtocolLimitsV1::default())
                .is_ok()
        );
    }

    for response in [
        mismatch_response(DiagnosisBarrierMismatchV2::Semantics, 2, 3),
        mismatch_response(DiagnosisBarrierMismatchV2::Site, 2, 2),
        mismatch_response(DiagnosisBarrierMismatchV2::SiteAndSemantics, 2, 2),
    ] {
        assert!(matches!(
            decode_diagnosis_response_line_v2(&reencode(&response), ProtocolLimitsV1::default()),
            Err(ProtocolCodecErrorV1::Validation(
                ProtocolValidationErrorV1::IdentityMismatch("diagnosis barrier mismatch site")
            ))
        ));
    }
}
