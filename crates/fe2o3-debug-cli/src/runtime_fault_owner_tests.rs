//! Real opt-in CLI capture of the unchanged raw-KIR tutorial's bounds fault.
//! This is CPU/raw-KIR acceptance, not ordinary-source, GPU, or proof qualification.
use super::*;

const REQUEST: &[u8] = br#"{"schema":"fe2o3-simulation-request-v1","kernel":"fill","grid":[4,1,1],"workgroup":[64,1,1],"arguments":[{"kind":"buffer","element":"u32","access":"read_write","alignment":4,"bytes":"0x00000000"}]}"#;

fn line(value: &impl Serialize) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).unwrap();
    bytes.push(b'\n');
    bytes
}

fn control(backend: &mut SimulatorBackendV1, request: DebugRequestV1) -> DebugResponseV1 {
    let limits = backend.protocol_limits;
    let request = decode_request_line_v1(&line(&request), limits).unwrap();
    let response = backend.handle(request);
    let encoded = encode_response_line_v1(&response, limits).unwrap();
    assert_eq!(decode_response_line_v1(&encoded, limits).unwrap(), response);
    response
}

fn runtime(backend: &mut SimulatorBackendV1) -> RuntimeObservationResponseV1 {
    let limits = backend.protocol_limits;
    let request = RuntimeObservationRequestV1::InspectCurrentRecord {
        schema: RuntimeObservationRequestSchemaV1::V1,
        request_id: backend.command_count + 1,
        expected_revision: backend.revision,
        expected_cursor: backend.session_view().cursor,
        expected_owner: None,
    };
    let request = decode_runtime_observation_request_line_v1(&line(&request), limits).unwrap();
    let response = backend.handle_runtime_observation_v1(request.clone());
    response.validate_for_request(&request, limits).unwrap();
    let encoded = encode_runtime_observation_response_line_v1(&response, limits).unwrap();
    assert_eq!(
        decode_runtime_observation_response_line_v1(&encoded, limits).unwrap(),
        response
    );
    response
}

fn resource(backend: &mut SimulatorBackendV1, request: ResourceRequestV2) -> ResourceResponseV2 {
    let limits = backend.protocol_limits;
    let request = decode_resource_request_line_v2(&line(&request), limits).unwrap();
    let response = backend.handle_resource_queries_v2(request.clone());
    response.validate_for_request(&request, limits).unwrap();
    let encoded = encode_resource_response_line_v2(&response, limits).unwrap();
    assert_eq!(
        decode_resource_response_line_v2(&encoded, limits).unwrap(),
        response
    );
    response
}

fn select(backend: &mut SimulatorBackendV1, index: usize) {
    let request = DebugRequestV1::Seek {
        schema: RequestSchemaV1::V1,
        request_id: backend.command_count + 1,
        expected_revision: backend.revision,
        cursor: DebugCursorV1 {
            event_sequence: index as u64 + 1,
            ..backend.session_view().cursor
        },
    };
    let response = control(backend, request);
    assert!(
        matches!(response, DebugResponseV1::Ok { .. }),
        "{response:?}"
    );
}

fn page() -> ResourceQueryPageV1 {
    ResourceQueryPageV1 {
        max_items: 16,
        max_scanned: 64,
        token: None,
    }
}

fn allocations(
    backend: &SimulatorBackendV1,
    binding: RuntimeObservationBindingV1,
) -> ResourceRequestV2 {
    ResourceRequestV2::QueryAllocations {
        schema: ResourceRequestSchemaV2::V2,
        request_id: backend.command_count + 1,
        expected_revision: backend.revision,
        expected_binding: binding,
        address_space: None,
        page: page(),
    }
}

fn memory(
    backend: &SimulatorBackendV1,
    binding: RuntimeObservationBindingV1,
    allocation: ResourceStorageIdentityV2,
) -> ResourceRequestV2 {
    ResourceRequestV2::ReadAllocationMemory {
        schema: ResourceRequestSchemaV2::V2,
        request_id: backend.command_count + 1,
        expected_revision: backend.revision,
        expected_binding: binding,
        allocation,
        range: ResourceMemoryRangeV1 {
            byte_offset: ResourceDecimalU64V1::new(0),
            byte_len: ResourceDecimalU64V1::new(4),
        },
    }
}

fn assert_raw_bounds_diagnosis(backend: &mut SimulatorBackendV1) {
    let limits = backend.protocol_limits;
    let request = DiagnosisRequestV2::Diagnose {
        schema: DiagnosisRequestSchemaV2::V2,
        request_id: backend.command_count + 1,
        expected_revision: backend.revision,
        filter: DiagnosisFilterV2 {
            class: Some(DiagnosisClassV2::MemoryOutOfBounds),
            scope: None,
        },
        page: PageRequestV1 {
            limit: 1,
            cursor: None,
        },
    };
    let request = decode_diagnosis_request_line_v2(&line(&request), limits).unwrap();
    let response = backend.handle_diagnosis_v2(request);
    let encoded = encode_diagnosis_response_line_v2(&response, limits).unwrap();
    assert_eq!(
        decode_diagnosis_response_line_v2(&encoded, limits).unwrap(),
        response
    );
    let DiagnosisResponseV2::Ok {
        session,
        completeness,
        diagnoses,
        next_cursor,
        ..
    } = response
    else {
        panic!("the actual bounds fault must retain its structured diagnosis");
    };
    assert!(session.simulated && !session.hardware_observed);
    assert_eq!(completeness, CaptureCompletenessV1::Complete);
    assert!(next_cursor.is_none());
    assert_eq!(diagnoses.len(), 1);
    let diagnosis = &diagnoses[0];
    assert_eq!(diagnosis.class, DiagnosisClassV2::MemoryOutOfBounds);
    assert_eq!(
        diagnosis.input.configuration_identity,
        session.configuration_identity
    );
    assert!(matches!(
        diagnosis.memory_region,
        DiagnosisFactV2::Observed {
            value: DiagnosisMemoryRegionV2 {
                requested_offset: 4,
                requested_bytes: 4,
                allocation_bytes: 4,
                ..
            }
        }
    ));
    assert!(matches!(
        diagnosis.input.dispatch_request,
        DiagnosisFactV2::Declared { .. }
    ));
    assert!(matches!(
        diagnosis.input.canonical_kir_v7,
        DiagnosisFactV2::Declared { .. }
    ));
    assert!(matches!(
        diagnosis.input.simulation_bundle,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::InputNotProvided
        }
    ));
    assert!(matches!(
        diagnosis.input.source_map_v2,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::InputNotProvided
        }
    ));
    assert!(matches!(
        diagnosis.source_operation,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::InputNotProvided
        }
    ));
    assert!(matches!(
        diagnosis.input.finalized_artifact,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::NoArtifactAuthority
        }
    ));
    assert!(matches!(
        diagnosis.input.property_proof,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::NoProofAuthority
        }
    ));
}

#[test]
fn actual_observed_bounds_fault_clears_selected_runtime_and_storage() {
    // Exactly the unchanged 245-byte fill fixture and one-u32 request used by
    // tests/diagnosis_v2.rs, with explicit observed capture enabled.
    let kernel = include_bytes!("../../fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir");
    assert_eq!(kernel.len(), 245);
    let input = load_debug_simulation_input_bytes_v1(kernel, REQUEST).unwrap();
    let mut backend = SimulatorBackendV1::new_with_maps_schedule_and_observations(
        input,
        DebugWaveWidthV1::Wave32,
        None,
        None,
        None,
        true,
    )
    .unwrap();
    assert!(backend.failed_execution);
    assert!(backend.session.observed().is_some());
    assert!(backend.session.transcript().terminal_fault().is_some());
    assert_eq!(
        backend.session.transcript().completeness(),
        DebugTranscriptCompletenessV1::Complete
    );
    let first = backend
        .session
        .transcript()
        .records()
        .iter()
        .position(|row| matches!(row.kind, SimulationDebugRecordKindV1::Checkpoint { .. }))
        .unwrap();
    select(&mut backend, first);
    let RuntimeObservationResponseV1::Ok {
        binding: before,
        origin,
        frames,
        ..
    } = runtime(&mut backend)
    else {
        panic!("the real pre-fault checkpoint must have runtime metadata");
    };
    assert!(matches!(frames, RuntimeFramesV1::Captured { .. }));
    let request = allocations(&backend, before);
    let ResourceResponseV2::Ok {
        result: ResourceQueryResultV2::Allocations { allocations: rows },
        ..
    } = resource(&mut backend, request)
    else {
        panic!("the pre-fault allocation must be inspectable");
    };
    assert_eq!(rows.len(), 1);
    let allocation = rows[0].descriptor.identity;
    assert_eq!(rows[0].descriptor.address_space, AddressSpaceV1::Global);
    assert_eq!(rows[0].descriptor.byte_len.get(), 4);
    let request = memory(&backend, before, allocation);
    assert!(
        matches!(resource(&mut backend, request), ResourceResponseV2::Ok {
        result: ResourceQueryResultV2::AllocationMemory { memory }, ..
    } if memory.bytes == "0x00000000" && memory.initialized == "0x0f")
    );

    let previous_revision = backend.revision;
    let request = DebugRequestV1::Continue {
        schema: RequestSchemaV1::V1,
        request_id: backend.command_count + 1,
        expected_revision: backend.revision,
        max_events: 1_000_000,
    };
    let response = control(&mut backend, request);
    assert!(matches!(response, DebugResponseV1::Ok { result, .. }
    if matches!(result.as_ref(), DebugResultV1::Control {
        stop: Some(StopViewV1 {
            reason: StopReasonV1::Fault, outcome: ExecutionOutcomeV1::Failed,
            exact: true, ..
        }),
        snapshot: SnapshotAvailabilityV1::Unavailable {
            reason: SnapshotUnavailableReasonV1::NotCaptured
        }, ..
    })));
    assert_eq!(backend.revision, previous_revision + 1);
    assert_eq!(
        backend.session.cursor_record_index(),
        Some(backend.session.transcript().records().len())
    );
    assert_raw_bounds_diagnosis(&mut backend);
    let RuntimeObservationResponseV1::Unavailable {
        binding: Some(terminal),
        reason: RuntimeObservationUnavailableV1::NoSelectedRecord,
        completeness: CaptureCompletenessV1::Complete,
        session,
        ..
    } = runtime(&mut backend)
    else {
        panic!("the fault cursor must not reuse the previous record's runtime metadata");
    };
    assert!(session.simulated && !session.hardware_observed);
    assert_eq!(terminal.owner, before.owner);
    assert_ne!(terminal, before);

    // An internally consistent old binding is stale; merely replacing its
    // revision cannot turn the old selected record into the terminal cursor.
    let mut stale = allocations(&backend, before);
    if let ResourceRequestV2::QueryAllocations {
        expected_revision, ..
    } = &mut stale
    {
        *expected_revision = before.cursor.state_revision;
    }
    assert!(matches!(
        resource(&mut backend, stale),
        ResourceResponseV2::Error {
            error: DebugErrorV1 {
                code: DebugErrorCodeV1::StaleRevision,
                state_changed: false,
                ..
            },
            ..
        }
    ));
    let mut wrong_cursor = before;
    wrong_cursor.cursor.state_revision = backend.revision;
    let request = allocations(&backend, wrong_cursor);
    assert!(matches!(
        resource(&mut backend, request),
        ResourceResponseV2::Error {
            error: DebugErrorV1 {
                code: DebugErrorCodeV1::InvalidCursor,
                state_changed: false,
                ..
            },
            ..
        }
    ));
    let revision = backend.revision;
    for kind in 0..4 {
        let request = match kind {
            0 => allocations(&backend, terminal),
            1 => memory(&backend, terminal, allocation),
            2 => ResourceRequestV2::QueryAllocationLifecycle {
                schema: ResourceRequestSchemaV2::V2,
                request_id: backend.command_count + 1,
                expected_revision: backend.revision,
                expected_binding: terminal,
                page: page(),
            },
            _ => ResourceRequestV2::QueryMemoryAccesses {
                schema: ResourceRequestSchemaV2::V2,
                request_id: backend.command_count + 1,
                expected_revision: backend.revision,
                expected_binding: terminal,
                allocation,
                page: page(),
            },
        };
        assert!(
            matches!(resource(&mut backend, request), ResourceResponseV2::Unavailable {
            reason: RuntimeObservationUnavailableV1::NoSelectedRecord,
            completeness: CaptureCompletenessV1::Complete, binding, ..
        } if binding == terminal)
        );
        assert_eq!(backend.revision, revision);
    }

    // Clearing the current selection does not erase immutable historical rows.
    // No terminal-memory snapshot is required or manufactured by this test.
    select(&mut backend, first);
    let RuntimeObservationResponseV1::Ok {
        binding: revisited,
        origin: again_origin,
        frames: again_frames,
        ..
    } = runtime(&mut backend)
    else {
        panic!("historical runtime metadata must remain inspectable after the fault");
    };
    assert_eq!(revisited.owner, before.owner);
    assert_eq!(again_origin, origin);
    assert_eq!(again_frames, frames);
    let request = memory(&backend, revisited, allocation);
    assert!(
        matches!(resource(&mut backend, request), ResourceResponseV2::Ok {
        result: ResourceQueryResultV2::AllocationMemory { memory }, ..
    } if memory.bytes == "0x00000000" && memory.initialized == "0x0f")
    );
}
