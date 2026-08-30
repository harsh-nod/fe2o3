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
        current: DiagnosisFactV2::Inferred {
            value: 0,
            basis: DiagnosisInferenceBasisV2::BarrierPhase,
        },
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

fn expected_participant(local: u32) -> DiagnosisBarrierParticipantV2 {
    let mut participant = participant(local);
    participant.local_workitem = DiagnosisFactV2::Inferred {
        value: [local, 0, 0],
        basis: DiagnosisInferenceBasisV2::LaunchGeometry,
    };
    participant
}

fn divergence_response() -> DiagnosisResponseV2 {
    seal_response(DiagnosisResponseV2::Ok {
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
            site: DiagnosisFactV2::Observed { value: kir_site(2) },
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
                    expected_participant_set: DiagnosisFactV2::Inferred {
                        value: (0..4).map(expected_participant).collect(),
                        basis: DiagnosisInferenceBasisV2::LaunchGeometry,
                    },
                    arrived_participants: DiagnosisFactV2::Observed {
                        value: vec![participant(1)],
                    },
                    waiting_participants: DiagnosisFactV2::Observed {
                        value: vec![participant(1)],
                    },
                    exited_participants: DiagnosisFactV2::Observed {
                        value: vec![participant(0), participant(2), participant(3)],
                    },
                },
            },
            evidence: DiagnosisEvidenceManifestV2::unsealed().unwrap(),
        }],
        next_cursor: None,
    })
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
    seal_response(DiagnosisResponseV2::Ok {
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
                    expected_participant_set: DiagnosisFactV2::Inferred {
                        value: (0..4).map(expected_participant).collect(),
                        basis: DiagnosisInferenceBasisV2::LaunchGeometry,
                    },
                    mismatch: DiagnosisFactV2::Observed { value: mismatch },
                    expected_site: DiagnosisFactV2::Observed {
                        value: kir_site(expected_operation),
                    },
                },
            },
            evidence: DiagnosisEvidenceManifestV2::unsealed().unwrap(),
        }],
        next_cursor: None,
    })
}

fn out_of_bounds_response() -> DiagnosisResponseV2 {
    seal_response(DiagnosisResponseV2::Ok {
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
                    legal_offset: 0,
                    legal_bytes: 4,
                    allocation_bytes: 4,
                    allocation_contract: DiagnosisFactV2::Declared {
                        value: DiagnosisAllocationContractV2 {
                            address_space: AddressSpaceV1::Global,
                            access: DiagnosisAccessModeV2::ReadWrite,
                            alignment: 4,
                            allocation_bytes: 4,
                            abi_arguments: vec![DiagnosisAbiArgumentV2 {
                                ordinal: 0,
                                backing: None,
                                kind: DiagnosisAbiArgumentKindV2::Slice,
                                element: DiagnosisScalarTypeV2::U32,
                                address_space: AddressSpaceV1::Global,
                                access: DiagnosisAccessModeV2::ReadWrite,
                                supplied_access: DiagnosisAccessModeV2::ReadWrite,
                                view_offset: 0,
                                view_bytes: 4,
                            }],
                        },
                    },
                    abi_argument: DiagnosisFactV2::Declared {
                        value: DiagnosisAbiArgumentV2 {
                            ordinal: 0,
                            backing: None,
                            kind: DiagnosisAbiArgumentKindV2::Slice,
                            element: DiagnosisScalarTypeV2::U32,
                            address_space: AddressSpaceV1::Global,
                            access: DiagnosisAccessModeV2::ReadWrite,
                            supplied_access: DiagnosisAccessModeV2::ReadWrite,
                            view_offset: 0,
                            view_bytes: 4,
                        },
                    },
                    logical_element: DiagnosisFactV2::Inferred {
                        value: DiagnosisLogicalElementV2 {
                            argument_ordinal: 0,
                            element: DiagnosisScalarTypeV2::U32,
                            element_bytes: 4,
                            element_index: 1,
                        },
                        basis: DiagnosisInferenceBasisV2::AbiViewBounds,
                    },
                    legal_bounds: DiagnosisFactV2::Inferred {
                        value: DiagnosisLegalBoundsPropertyV2 {
                            argument_ordinal: 0,
                            legal_offset: 0,
                            legal_bytes: 4,
                            requested_offset: 4,
                            requested_bytes: 4,
                            satisfied: false,
                        },
                        basis: DiagnosisInferenceBasisV2::AbiViewBounds,
                    },
                },
            },
            barrier: DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::NotApplicable,
            },
            evidence: DiagnosisEvidenceManifestV2::unsealed().unwrap(),
        }],
        next_cursor: None,
    })
}

fn aliasing_view_out_of_bounds_response() -> DiagnosisResponseV2 {
    let mut response = out_of_bounds_response();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut response else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed { value: region } = &mut diagnoses[0].memory_region else {
        unreachable!()
    };
    let narrow = DiagnosisAbiArgumentV2 {
        ordinal: 0,
        backing: Some(9),
        kind: DiagnosisAbiArgumentKindV2::Slice,
        element: DiagnosisScalarTypeV2::U32,
        address_space: AddressSpaceV1::Global,
        access: DiagnosisAccessModeV2::ReadWrite,
        supplied_access: DiagnosisAccessModeV2::ReadWrite,
        view_offset: 4,
        view_bytes: 4,
    };
    let alias = DiagnosisAbiArgumentV2 {
        ordinal: 1,
        backing: Some(9),
        kind: DiagnosisAbiArgumentKindV2::Slice,
        element: DiagnosisScalarTypeV2::U32,
        address_space: AddressSpaceV1::Global,
        access: DiagnosisAccessModeV2::ReadWrite,
        supplied_access: DiagnosisAccessModeV2::ReadWrite,
        view_offset: 0,
        view_bytes: 12,
    };
    region.requested_offset = 8;
    region.legal_offset = 4;
    region.legal_bytes = 4;
    region.allocation_bytes = 12;
    region.allocation_contract = DiagnosisFactV2::Declared {
        value: DiagnosisAllocationContractV2 {
            address_space: AddressSpaceV1::Global,
            access: DiagnosisAccessModeV2::ReadWrite,
            alignment: 4,
            allocation_bytes: 12,
            abi_arguments: vec![narrow, alias],
        },
    };
    region.abi_argument = DiagnosisFactV2::Declared { value: narrow };
    region.logical_element = DiagnosisFactV2::Inferred {
        value: DiagnosisLogicalElementV2 {
            argument_ordinal: 0,
            element: DiagnosisScalarTypeV2::U32,
            element_bytes: 4,
            element_index: 1,
        },
        basis: DiagnosisInferenceBasisV2::AbiViewBounds,
    };
    region.legal_bounds = DiagnosisFactV2::Inferred {
        value: DiagnosisLegalBoundsPropertyV2 {
            argument_ordinal: 0,
            legal_offset: 4,
            legal_bytes: 4,
            requested_offset: 8,
            requested_bytes: 4,
            satisfied: false,
        },
        basis: DiagnosisInferenceBasisV2::AbiViewBounds,
    };
    reseal_response(&mut response);
    response
}

fn bundle_and_source_mapped_out_of_bounds_response() -> DiagnosisResponseV2 {
    let mut response = out_of_bounds_response();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut response else {
        unreachable!()
    };
    let diagnosis = &mut diagnoses[0];
    let DiagnosisFactV2::Observed { value: site } = diagnosis.site else {
        unreachable!()
    };
    let bundle_subject = identity(22);
    let location = SourceLocationV1 {
        map_identity: identity(27),
        provenance: SourceMapProvenanceV1::CompilerBundleBound,
        file_identity: identity(28),
        byte_start: 100,
        byte_end: 120,
    };
    let member = diagnosis_source_map_member_identity_v2(bundle_subject, site, location).unwrap();
    let root = diagnosis_source_map_membership_root_v2(&[member]).unwrap();
    let membership = diagnosis_source_map_membership_proof_v2(&[member], 0).unwrap();
    diagnosis.input.simulation_bundle = DiagnosisFactV2::Declared {
        value: DiagnosisBundleReferenceV2 {
            envelope_version: 2,
            identity: identity(21),
            subject_identity: bundle_subject,
        },
    };
    diagnosis.input.production_kir = DiagnosisFactV2::Declared {
        value: DiagnosisVersionedContentReferenceV2 {
            version: 8,
            content: content(23, 1_024),
        },
    };
    diagnosis.input.kernel_abi_identity = DiagnosisFactV2::Declared {
        value: identity(24),
    };
    diagnosis.input.source_lineage = DiagnosisFactV2::Declared {
        value: DiagnosisSourceLineageV2 {
            identity_inventory_receipt: content(25, 256),
            preflight_plan_receipt: content(26, 512),
        },
    };
    diagnosis.input.source_map_v2 = DiagnosisFactV2::Declared {
        value: DiagnosisSourceMapReferenceV2 {
            identity: location.map_identity,
            bundle_subject_identity: bundle_subject,
            provenance: location.provenance,
            operation_membership_root: root,
            operation_members: 1,
        },
    };
    diagnosis.source_operation = DiagnosisFactV2::Declared {
        value: DiagnosisSourceOperationV2 {
            bundle_subject_identity: bundle_subject,
            kir_site: site,
            location,
            membership,
        },
    };
    reseal_response(&mut response);
    response
}

fn seal_response(mut response: DiagnosisResponseV2) -> DiagnosisResponseV2 {
    let DiagnosisResponseV2::Ok {
        session,
        completeness,
        diagnoses,
        ..
    } = &mut response
    else {
        return response;
    };
    for diagnosis in diagnoses {
        let retained = retained_evidence(diagnosis, *completeness).unwrap();
        diagnosis
            .seal_evidence_v2(*session, *completeness, retained)
            .unwrap();
    }
    response
}

fn retained_evidence(
    diagnosis: &DiagnosisViewV2,
    completeness: CaptureCompletenessV1,
) -> Option<DiagnosisRetainedEvidenceV2> {
    let invocation = match (
        &diagnosis.context.dispatch,
        &diagnosis.context.workgroup,
        &diagnosis.context.workitem,
    ) {
        (
            DiagnosisFactV2::Declared { value: dispatch },
            DiagnosisFactV2::Observed { value: workgroup },
            DiagnosisFactV2::Observed { value: workitem },
        ) => Some(DiagnosisTerminalInvocationRecordV2 {
            global: workitem.global,
            workgroup: *workgroup,
            local: workitem.local,
            workgroup_size: dispatch.workgroup_size,
            launch_extent: dispatch.launch_extent,
        }),
        _ => None,
    };
    let site = match &diagnosis.site {
        DiagnosisFactV2::Observed { value } => Some(*value),
        _ => None,
    };
    let (payload, barrier) = match (&diagnosis.memory_region, &diagnosis.barrier) {
        (DiagnosisFactV2::Observed { value: memory }, _) => {
            let DiagnosisFactV2::Declared { value: argument } = &memory.abi_argument else {
                return None;
            };
            let DiagnosisFactV2::Declared { value: contract } = &memory.allocation_contract else {
                return None;
            };
            (
                DiagnosisTerminalPayloadRecordV2::MemoryOutOfBounds {
                    allocation: memory.allocation,
                    requested_offset: memory.requested_offset,
                    requested_bytes: memory.requested_bytes,
                    allocation_bytes: memory.allocation_bytes,
                    abi_view: Some(DiagnosisTerminalAbiViewRecordV2 {
                        allocation_contract: contract.clone(),
                        abi_argument: *argument,
                        legal_offset: memory.legal_offset,
                        legal_bytes: memory.legal_bytes,
                    }),
                },
                None,
            )
        }
        (
            _,
            DiagnosisFactV2::Observed {
                value:
                    DiagnosisBarrierV2::Divergence {
                        phase: DiagnosisFactV2::Observed { value: phase },
                        arrived_participants,
                        waiting_participants,
                        exited_participants,
                        ..
                    },
            },
        ) => {
            let locals = |fact: &DiagnosisFactV2<Vec<DiagnosisBarrierParticipantV2>>| {
                let DiagnosisFactV2::Observed { value } = fact else {
                    return None;
                };
                value
                    .iter()
                    .map(|participant| match &participant.local_workitem {
                        DiagnosisFactV2::Observed { value } => Some(*value),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()
            };
            let waiting = locals(waiting_participants)?;
            let exited = locals(exited_participants)?;
            let arrived = locals(arrived_participants)?;
            let mut arrivals = Vec::new();
            for (index, local) in arrived.into_iter().enumerate() {
                arrivals.push(DiagnosisBarrierArrivalEvidenceRecordV2 {
                    sequence: u64::try_from(index).ok()?.checked_add(1)?,
                    local,
                    site,
                });
            }
            (
                DiagnosisTerminalPayloadRecordV2::WorkgroupBarrierDivergence {
                    phase: *phase,
                    waiting_representative: *waiting.first()?,
                    exited_representative: *exited.first()?,
                    waiting: Some(waiting),
                    exited: Some(exited),
                },
                Some(DiagnosisBarrierTranscriptEvidenceV2 {
                    phase: *phase,
                    workgroup: invocation?.workgroup,
                    arrivals,
                }),
            )
        }
        (
            _,
            DiagnosisFactV2::Observed {
                value:
                    DiagnosisBarrierV2::Mismatch {
                        phase: DiagnosisFactV2::Observed { value: phase },
                        mismatch: DiagnosisFactV2::Observed { value: mismatch },
                        expected_site,
                        ..
                    },
            },
        ) => (
            DiagnosisTerminalPayloadRecordV2::WorkgroupBarrierMismatch {
                phase: *phase,
                mismatch: *mismatch,
                expected_site: match expected_site {
                    DiagnosisFactV2::Observed { value } => Some(*value),
                    _ => None,
                },
            },
            None,
        ),
        _ => return None,
    };
    Some(DiagnosisRetainedEvidenceV2 {
        terminal: DiagnosisTerminalEvidenceRecordV2 {
            sequence: diagnosis.sequence,
            class: diagnosis.class,
            invocation,
            site,
            payload,
        },
        transcript: DiagnosisTranscriptEvidenceRecordV2 {
            completeness,
            barrier,
        },
    })
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
fn evidence_manifest_binds_retained_records_claims_session_and_completeness() {
    let response = out_of_bounds_response();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &response else {
        unreachable!()
    };
    assert!(
        diagnoses[0]
            .evidence
            .citations
            .iter()
            .all(|citation| citation.source_record_identity != citation.claim_identity)
    );

    let mut source_record = response.clone();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut source_record else {
        unreachable!()
    };
    diagnoses[0].evidence.citations[0].source_record_identity = identity(71);
    assert!(
        decode_diagnosis_response_line_v2(&reencode(&source_record), ProtocolLimitsV1::default())
            .is_err()
    );

    let mut claim = response.clone();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut claim else {
        unreachable!()
    };
    diagnoses[0].evidence.citations[0].claim_identity = identity(72);
    assert!(
        decode_diagnosis_response_line_v2(&reencode(&claim), ProtocolLimitsV1::default()).is_err()
    );

    let mut revision = response.clone();
    let DiagnosisResponseV2::Ok { session, .. } = &mut revision else {
        unreachable!()
    };
    session.revision = 1;
    session.cursor.state_revision = 1;
    assert!(matches!(
        decode_diagnosis_response_line_v2(&reencode(&revision), ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::IdentityMismatch("diagnosis evidence manifest")
        ))
    ));

    let mut truncated_response = response;
    let DiagnosisResponseV2::Ok { completeness, .. } = &mut truncated_response else {
        unreachable!()
    };
    *completeness = CaptureCompletenessV1::Truncated {
        reason: CaptureTruncationReasonV1::EventLimit,
        emitted_events: 1,
        dropped_events: None,
    };
    assert!(
        decode_diagnosis_response_line_v2(
            &reencode(&truncated_response),
            ProtocolLimitsV1::default()
        )
        .is_err()
    );
}

fn capture_binding(response: &DiagnosisResponseV2) -> DiagnosisCaptureBindingV2 {
    let DiagnosisResponseV2::Ok {
        schema,
        request_id,
        operation,
        session,
        completeness,
        diagnoses,
        next_cursor,
        ..
    } = response
    else {
        unreachable!()
    };
    diagnoses[0]
        .evidence
        .retained
        .as_ref()
        .unwrap()
        .capture_binding_v2(
            &diagnoses[0].input,
            *session,
            *completeness,
            DiagnosisResponseEnvelopeBindingV2 {
                schema: *schema,
                request_id: *request_id,
                operation: *operation,
                next_cursor: *next_cursor,
            },
        )
        .unwrap()
}

#[test]
fn coherent_reseal_has_content_integrity_but_not_original_capture_authenticity() {
    let original = bundle_and_source_mapped_out_of_bounds_response();
    let expected_capture = capture_binding(&original);
    let mut substituted = original;
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut substituted else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed { value: region } = &mut diagnoses[0].memory_region else {
        unreachable!()
    };
    region.requested_offset += region.requested_bytes;
    let DiagnosisFactV2::Inferred { value: logical, .. } = &mut region.logical_element else {
        unreachable!()
    };
    logical.element_index += 1;
    let DiagnosisFactV2::Inferred { value: bounds, .. } = &mut region.legal_bounds else {
        unreachable!()
    };
    bounds.requested_offset = region.requested_offset;
    reseal_response(&mut substituted);

    let decoded =
        decode_diagnosis_response_line_v2(&reencode(&substituted), ProtocolLimitsV1::default())
            .expect("coherently resealed content remains structurally valid");
    assert!(matches!(
        decoded.validate_against_capture_v2(ProtocolLimitsV1::default(), expected_capture),
        Err(ProtocolValidationErrorV1::IdentityMismatch(
            "diagnosis capture binding"
        ))
    ));
}

fn assert_resealed_capture_substitution_is_rejected(
    substituted: &DiagnosisResponseV2,
    expected_capture: DiagnosisCaptureBindingV2,
) {
    let decoded =
        decode_diagnosis_response_line_v2(&reencode(substituted), ProtocolLimitsV1::default())
            .expect("coherently resealed content remains structurally valid");
    assert!(matches!(
        decoded.validate_against_capture_v2(ProtocolLimitsV1::default(), expected_capture),
        Err(ProtocolValidationErrorV1::IdentityMismatch(
            "diagnosis capture binding"
        ))
    ));
}

#[test]
fn capture_binding_rejects_coherently_resealed_session_revision_and_cursor() {
    let original = bundle_and_source_mapped_out_of_bounds_response();
    let expected_capture = capture_binding(&original);
    let mut substituted = original;
    let DiagnosisResponseV2::Ok { session, .. } = &mut substituted else {
        unreachable!()
    };
    session.state = SessionStateV1::Stopped;
    session.revision = 1;
    session.cursor.event_sequence = 1;
    session.cursor.state_revision = 1;
    reseal_response(&mut substituted);
    assert_resealed_capture_substitution_is_rejected(&substituted, expected_capture);
}

#[test]
fn capture_binding_rejects_cursor_only_response_substitution() {
    let original = bundle_and_source_mapped_out_of_bounds_response();
    let expected_capture = capture_binding(&original);
    let mut substituted = original;
    let DiagnosisResponseV2::Ok { next_cursor, .. } = &mut substituted else {
        unreachable!()
    };
    *next_cursor = Some(PageCursorV1 {
        query_identity: identity(33),
        position: 1,
    });
    reseal_response(&mut substituted);
    assert_resealed_capture_substitution_is_rejected(&substituted, expected_capture);
}

#[test]
fn capture_binding_rejects_request_id_only_response_substitution() {
    let original = bundle_and_source_mapped_out_of_bounds_response();
    let expected_capture = capture_binding(&original);
    let mut substituted = original;
    let DiagnosisResponseV2::Ok { request_id, .. } = &mut substituted else {
        unreachable!()
    };
    *request_id += 1;
    reseal_response(&mut substituted);
    assert_resealed_capture_substitution_is_rejected(&substituted, expected_capture);
}

#[test]
fn capture_binding_rejects_coherently_resealed_configuration_and_input() {
    let original = bundle_and_source_mapped_out_of_bounds_response();
    let expected_capture = capture_binding(&original);
    let mut substituted = original;
    let DiagnosisResponseV2::Ok {
        session, diagnoses, ..
    } = &mut substituted
    else {
        unreachable!()
    };
    let replacement = identity(31);
    session.configuration_identity = replacement;
    session.cursor.configuration_identity = replacement;
    diagnoses[0].input.configuration_identity = replacement;
    reseal_response(&mut substituted);
    assert_resealed_capture_substitution_is_rejected(&substituted, expected_capture);
}

#[test]
fn capture_binding_rejects_coherently_resealed_kernel_abi_identity() {
    let original = bundle_and_source_mapped_out_of_bounds_response();
    let expected_capture = capture_binding(&original);
    let mut substituted = original;
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut substituted else {
        unreachable!()
    };
    diagnoses[0].input.kernel_abi_identity = DiagnosisFactV2::Declared {
        value: identity(32),
    };
    reseal_response(&mut substituted);
    assert_resealed_capture_substitution_is_rejected(&substituted, expected_capture);
}

#[test]
fn capture_binding_rejects_same_bundle_with_coherently_resealed_source_member() {
    let original = bundle_and_source_mapped_out_of_bounds_response();
    let expected_capture = capture_binding(&original);
    let mut substituted = original;
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut substituted else {
        unreachable!()
    };
    let original_bundle = diagnoses[0].input.simulation_bundle.clone();
    let DiagnosisFactV2::Declared { value: source } = &mut diagnoses[0].source_operation else {
        unreachable!()
    };
    source.location.byte_start += 1;
    source.location.byte_end += 1;
    let leaf = diagnosis_source_map_member_identity_v2(
        source.bundle_subject_identity,
        source.kir_site,
        source.location,
    )
    .unwrap();
    source.membership = diagnosis_source_map_membership_proof_v2(&[leaf], 0).unwrap();
    let DiagnosisFactV2::Declared { value: map } = &mut diagnoses[0].input.source_map_v2 else {
        unreachable!()
    };
    map.operation_membership_root = diagnosis_source_map_membership_root_v2(&[leaf]).unwrap();
    assert_eq!(diagnoses[0].input.simulation_bundle, original_bundle);
    reseal_response(&mut substituted);
    assert_resealed_capture_substitution_is_rejected(&substituted, expected_capture);
}

#[test]
fn coordinated_oob_contract_substitutions_require_original_capture_binding() {
    for mutate in [
        0_u8, // address space
        1_u8, // access mode
        2_u8, // shared-backing identity
    ] {
        let original = aliasing_view_out_of_bounds_response();
        let expected_capture = capture_binding(&original);
        let mut substituted = original;
        let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut substituted else {
            unreachable!()
        };
        let DiagnosisFactV2::Observed { value: region } = &mut diagnoses[0].memory_region else {
            unreachable!()
        };
        let DiagnosisFactV2::Declared { value: contract } = &mut region.allocation_contract else {
            unreachable!()
        };
        let DiagnosisFactV2::Declared { value: argument } = &mut region.abi_argument else {
            unreachable!()
        };
        match mutate {
            0 => {
                contract.address_space = AddressSpaceV1::Constant;
                for alias in &mut contract.abi_arguments {
                    alias.address_space = AddressSpaceV1::Constant;
                }
                argument.address_space = AddressSpaceV1::Constant;
            }
            1 => {
                contract.access = DiagnosisAccessModeV2::ReadOnly;
                for alias in &mut contract.abi_arguments {
                    alias.access = DiagnosisAccessModeV2::ReadOnly;
                    alias.supplied_access = DiagnosisAccessModeV2::ReadOnly;
                }
                argument.access = DiagnosisAccessModeV2::ReadOnly;
                argument.supplied_access = DiagnosisAccessModeV2::ReadOnly;
            }
            _ => {
                for alias in &mut contract.abi_arguments {
                    alias.backing = Some(10);
                }
                argument.backing = Some(10);
            }
        }
        reseal_response(&mut substituted);
        let decoded =
            decode_diagnosis_response_line_v2(&reencode(&substituted), ProtocolLimitsV1::default())
                .expect("coordinated substitution is a new internally consistent story");
        assert!(
            decoded
                .validate_against_capture_v2(ProtocolLimitsV1::default(), expected_capture)
                .is_err()
        );
    }

    let mut wrong_element_width = out_of_bounds_response();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut wrong_element_width else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed { value: region } = &mut diagnoses[0].memory_region else {
        unreachable!()
    };
    region.requested_bytes = 8;
    let DiagnosisFactV2::Inferred { value: bounds, .. } = &mut region.legal_bounds else {
        unreachable!()
    };
    bounds.requested_bytes = 8;
    reseal_response(&mut wrong_element_width);
    assert!(
        decode_diagnosis_response_line_v2(
            &reencode(&wrong_element_width),
            ProtocolLimitsV1::default(),
        )
        .is_err()
    );
}

#[test]
fn abi_view_and_backing_access_capabilities_use_monotonic_admission() {
    for (required, supplied, backing) in [
        (
            DiagnosisAccessModeV2::ReadOnly,
            DiagnosisAccessModeV2::ReadWrite,
            DiagnosisAccessModeV2::ReadWrite,
        ),
        (
            DiagnosisAccessModeV2::ReadOnly,
            DiagnosisAccessModeV2::ReadOnly,
            DiagnosisAccessModeV2::ReadWrite,
        ),
    ] {
        let mut response = aliasing_view_out_of_bounds_response();
        let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut response else {
            unreachable!()
        };
        let DiagnosisFactV2::Observed { value: region } = &mut diagnoses[0].memory_region else {
            unreachable!()
        };
        let DiagnosisFactV2::Declared { value: contract } = &mut region.allocation_contract else {
            unreachable!()
        };
        contract.access = backing;
        contract.abi_arguments[0].access = required;
        contract.abi_arguments[0].supplied_access = supplied;
        let DiagnosisFactV2::Declared { value: argument } = &mut region.abi_argument else {
            unreachable!()
        };
        *argument = contract.abi_arguments[0];
        reseal_response(&mut response);
        assert!(
            decode_diagnosis_response_line_v2(&reencode(&response), ProtocolLimitsV1::default())
                .is_ok()
        );
    }

    for (required, supplied, backing) in [
        (
            DiagnosisAccessModeV2::ReadWrite,
            DiagnosisAccessModeV2::ReadOnly,
            DiagnosisAccessModeV2::ReadWrite,
        ),
        (
            DiagnosisAccessModeV2::ReadOnly,
            DiagnosisAccessModeV2::ReadWrite,
            DiagnosisAccessModeV2::ReadOnly,
        ),
    ] {
        let mut response = aliasing_view_out_of_bounds_response();
        let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut response else {
            unreachable!()
        };
        let DiagnosisFactV2::Observed { value: region } = &mut diagnoses[0].memory_region else {
            unreachable!()
        };
        let DiagnosisFactV2::Declared { value: contract } = &mut region.allocation_contract else {
            unreachable!()
        };
        contract.access = backing;
        contract.abi_arguments[0].access = required;
        contract.abi_arguments[0].supplied_access = supplied;
        let DiagnosisFactV2::Declared { value: argument } = &mut region.abi_argument else {
            unreachable!()
        };
        *argument = contract.abi_arguments[0];
        reseal_response(&mut response);
        assert!(
            decode_diagnosis_response_line_v2(&reencode(&response), ProtocolLimitsV1::default())
                .is_err()
        );
    }

    let mut ordinary = out_of_bounds_response();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut ordinary else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed { value: region } = &mut diagnoses[0].memory_region else {
        unreachable!()
    };
    let DiagnosisFactV2::Declared { value: contract } = &mut region.allocation_contract else {
        unreachable!()
    };
    contract.abi_arguments[0].access = DiagnosisAccessModeV2::ReadOnly;
    contract.abi_arguments[0].supplied_access = DiagnosisAccessModeV2::ReadOnly;
    let DiagnosisFactV2::Declared { value: argument } = &mut region.abi_argument else {
        unreachable!()
    };
    *argument = contract.abi_arguments[0];
    reseal_response(&mut ordinary);
    assert!(
        decode_diagnosis_response_line_v2(&reencode(&ordinary), ProtocolLimitsV1::default())
            .is_err()
    );
}

#[test]
fn oversized_declared_workgroup_is_rejected_before_participant_reconstruction() {
    let mut response = divergence_response();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut response else {
        unreachable!()
    };
    let DiagnosisFactV2::Declared { value: dispatch } = &mut diagnoses[0].context.dispatch else {
        unreachable!()
    };
    dispatch.workgroup_size = [u32::MAX, 1, 1];
    assert!(matches!(
        decode_diagnosis_response_line_v2(&reencode(&response), ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::CountOutOfRange("diagnosis workgroup volume")
        ))
    ));
}

#[test]
fn pointer_view_bounds_and_aliases_do_not_collapse_to_the_backing_allocation() {
    let response = aliasing_view_out_of_bounds_response();
    assert!(
        decode_diagnosis_response_line_v2(&reencode(&response), ProtocolLimitsV1::default())
            .is_ok()
    );
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &response else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed { value: region } = &diagnoses[0].memory_region else {
        unreachable!()
    };
    assert!(region.requested_offset + region.requested_bytes <= region.allocation_bytes);
    let DiagnosisFactV2::Declared { value: contract } = &region.allocation_contract else {
        unreachable!()
    };
    assert_eq!(contract.abi_arguments.len(), 2);
    assert_eq!(contract.abi_arguments[0].backing, Some(9));
    assert_eq!(contract.abi_arguments[1].backing, Some(9));
    assert_ne!(
        contract.abi_arguments[0].view_bytes,
        contract.abi_arguments[1].view_bytes
    );

    let mut substituted = response;
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut substituted else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed { value: region } = &mut diagnoses[0].memory_region else {
        unreachable!()
    };
    let DiagnosisFactV2::Declared { value: argument } = &mut region.abi_argument else {
        unreachable!()
    };
    argument.backing = None;
    assert!(matches!(
        decode_diagnosis_response_line_v2(&reencode(&substituted), ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::IdentityMismatch("diagnosis faulting ABI argument")
        ))
    ));
}

#[test]
fn bundle_axes_and_exact_source_map_membership_reject_one_field_substitution() {
    let response = bundle_and_source_mapped_out_of_bounds_response();
    assert!(
        decode_diagnosis_response_line_v2(&reencode(&response), ProtocolLimitsV1::default())
            .is_ok()
    );

    let mut envelope = response.clone();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut envelope else {
        unreachable!()
    };
    let DiagnosisFactV2::Declared { value: bundle } = &mut diagnoses[0].input.simulation_bundle
    else {
        unreachable!()
    };
    bundle.identity = identity(29);
    assert!(matches!(
        decode_diagnosis_response_line_v2(&reencode(&envelope), ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::IdentityMismatch("diagnosis evidence manifest")
        ))
    ));

    let mut envelope_version = response.clone();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut envelope_version else {
        unreachable!()
    };
    let DiagnosisFactV2::Declared { value: bundle } = &mut diagnoses[0].input.simulation_bundle
    else {
        unreachable!()
    };
    bundle.envelope_version = 3;
    assert!(matches!(
        decode_diagnosis_response_line_v2(
            &reencode(&envelope_version),
            ProtocolLimitsV1::default()
        ),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::InvalidTruthClassification
        ))
    ));

    let mut abi = response.clone();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut abi else {
        unreachable!()
    };
    diagnoses[0].input.kernel_abi_identity = DiagnosisFactV2::Declared {
        value: identity(30),
    };
    assert!(
        decode_diagnosis_response_line_v2(&reencode(&abi), ProtocolLimitsV1::default()).is_err()
    );

    let mut same_map_span = response;
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut same_map_span else {
        unreachable!()
    };
    let DiagnosisFactV2::Declared { value: source } = &mut diagnoses[0].source_operation else {
        unreachable!()
    };
    source.location.byte_end = 121;
    assert!(matches!(
        decode_diagnosis_response_line_v2(&reencode(&same_map_span), ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::IdentityMismatch("diagnosis source map member")
        ))
    ));
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
    value.requested_offset = 0;
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
            operation_membership_root: identity(11),
            operation_members: 1,
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
            membership: DiagnosisSourceMapMembershipProofV2 {
                member_identity: identity(12),
                member_index: 0,
                member_count: 1,
                siblings: Vec::new(),
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
            envelope_version: 2,
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
            operation_membership_root: identity(20),
            operation_members: 1,
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
    let argument = &mut allocation.abi_arguments[0];
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

    let mut expected_observed = response.clone();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut expected_observed else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed {
        value:
            DiagnosisBarrierV2::Divergence {
                expected_participant_set,
                ..
            },
    } = &mut diagnoses[0].barrier
    else {
        unreachable!()
    };
    let DiagnosisFactV2::Inferred { value, .. } = expected_participant_set else {
        unreachable!()
    };
    value[0].local_workitem = DiagnosisFactV2::Observed { value: [0, 0, 0] };
    assert!(
        decode_diagnosis_response_line_v2(
            &reencode(&expected_observed),
            ProtocolLimitsV1::default()
        )
        .is_err()
    );

    let mut waiting_inferred = response.clone();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut waiting_inferred else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed {
        value:
            DiagnosisBarrierV2::Divergence {
                waiting_participants,
                ..
            },
    } = &mut diagnoses[0].barrier
    else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed { value } = waiting_participants else {
        unreachable!()
    };
    value[0].local_workitem = DiagnosisFactV2::Inferred {
        value: [1, 0, 0],
        basis: DiagnosisInferenceBasisV2::LaunchGeometry,
    };
    assert!(
        decode_diagnosis_response_line_v2(
            &reencode(&waiting_inferred),
            ProtocolLimitsV1::default()
        )
        .is_err()
    );

    let mut same_participant = response.clone();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = &mut same_participant else {
        unreachable!()
    };
    let DiagnosisFactV2::Observed {
        value:
            DiagnosisBarrierV2::Divergence {
                waiting_participants,
                exited_participants,
                ..
            },
    } = &mut diagnoses[0].barrier
    else {
        unreachable!()
    };
    *exited_participants = waiting_participants.clone();
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
    reseal_response(&mut truncated_valid);
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

fn reseal_response(response: &mut DiagnosisResponseV2) {
    let DiagnosisResponseV2::Ok {
        session,
        completeness,
        diagnoses,
        ..
    } = response
    else {
        return;
    };
    for diagnosis in diagnoses {
        if let Some(retained) = retained_evidence(diagnosis, *completeness) {
            let _ = diagnosis.seal_evidence_v2(*session, *completeness, retained);
        }
    }
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
