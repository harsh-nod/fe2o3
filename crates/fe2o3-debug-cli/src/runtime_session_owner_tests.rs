//! Actual interpreter/CLI controls over the retained raw-KIR tutorial.
//! These are not ordinary-source, browser, hardware, or proof qualifications.
use super::*;

fn backend(observed: bool) -> SimulatorBackendV1 {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let input = load_debug_simulation_input_v1(
        &root.join("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir"),
        &root.join("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/request.json"),
    )
    .unwrap();
    SimulatorBackendV1::new_with_maps_schedule_and_observations(
        input,
        DebugWaveWidthV1::Wave32,
        None,
        None,
        None,
        observed,
    )
    .unwrap()
}
fn query(
    backend: &SimulatorBackendV1,
    owner: Option<RuntimeObservationOwnerV1>,
) -> RuntimeObservationRequestV1 {
    RuntimeObservationRequestV1::InspectCurrentRecord {
        schema: RuntimeObservationRequestSchemaV1::V1,
        request_id: backend.command_count + 1,
        expected_revision: backend.revision,
        expected_cursor: backend.session_view().cursor,
        expected_owner: owner,
    }
}
fn select(backend: &mut SimulatorBackendV1, index: usize) {
    let cursor = DebugCursorV1 {
        event_sequence: index as u64 + 1,
        ..backend.session_view().cursor
    };
    let response = backend.handle(DebugRequestV1::Seek {
        schema: RequestSchemaV1::V1,
        request_id: backend.command_count + 1,
        expected_revision: backend.revision,
        cursor,
    });
    assert!(
        matches!(response, DebugResponseV1::Ok { .. }),
        "{response:?}"
    );
}
fn last_checkpoint(backend: &SimulatorBackendV1) -> usize {
    backend
        .session
        .transcript()
        .records()
        .iter()
        .rposition(|row| matches!(row.kind, SimulationDebugRecordKindV1::Checkpoint { .. }))
        .unwrap()
}
fn current(backend: &mut SimulatorBackendV1) -> RuntimeObservationBindingV1 {
    let request = query(backend, None);
    let response = backend.handle_runtime_observation_v1(request.clone());
    response
        .validate_for_request(&request, backend.protocol_limits)
        .unwrap();
    match response {
        RuntimeObservationResponseV1::Ok { binding, .. } => binding,
        value => panic!("actual checkpoint must expose runtime metadata: {value:?}"),
    }
}
fn page() -> ResourceQueryPageV1 {
    ResourceQueryPageV1 {
        max_items: 16,
        max_scanned: 64,
        token: None,
    }
}
fn inventory(
    backend: &mut SimulatorBackendV1,
    binding: RuntimeObservationBindingV1,
) -> ResourceStorageIdentityV2 {
    let request = ResourceRequestV2::QueryAllocations {
        schema: ResourceRequestSchemaV2::V2,
        request_id: backend.command_count + 1,
        expected_revision: backend.revision,
        expected_binding: binding,
        address_space: None,
        page: page(),
    };
    let response = backend.handle_resource_queries_v2(request.clone());
    response
        .validate_for_request(&request, backend.protocol_limits)
        .unwrap();
    match response {
        ResourceResponseV2::Ok {
            result: ResourceQueryResultV2::Allocations { allocations },
            ..
        } => {
            assert_eq!(allocations.len(), 1);
            assert_eq!(
                allocations[0].descriptor.address_space,
                AddressSpaceV1::Global
            );
            allocations[0].descriptor.identity
        }
        value => panic!("actual current inventory required: {value:?}"),
    }
}

#[test]
fn observed_option_is_explicit_closed_and_diagnostic_profiles_remain_separate() {
    let parse = |args: &[&str]| parse_options(args.iter().map(OsString::from));
    assert!(
        !parse(&["sim", "--kir-v7", "k", "--request", "r"])
            .unwrap()
            .runtime_observations
    );
    assert!(
        parse(&[
            "sim",
            "--kir-v7",
            "k",
            "--request",
            "r",
            "--runtime-observations",
            "v1"
        ])
        .unwrap()
        .runtime_observations
    );
    for args in [
        vec![
            "sim",
            "--kir-v7",
            "k",
            "--request",
            "r",
            "--runtime-observations",
            "v2",
        ],
        vec![
            "sim",
            "--kir-v7",
            "k",
            "--request",
            "r",
            "--runtime-observations",
            "v1",
            "--runtime-observations",
            "v1",
        ],
        vec![
            "sim",
            "--diagnostic-kir-v16",
            "k",
            "--request",
            "r",
            "--runtime-observations",
            "v1",
        ],
        vec![
            "sim",
            "--diagnostic-kir-v17",
            "k",
            "--request",
            "r",
            "--runtime-observations",
            "v1",
        ],
    ] {
        assert!(parse(&args).is_err());
    }
}

#[test]
fn actual_opt_in_preserves_legacy_records_and_separates_configuration_and_owner() {
    let mut legacy = backend(false);
    let mut first = backend(true);
    let mut second = backend(true);
    assert_eq!(legacy.session.transcript(), first.session.transcript());
    assert_eq!(first.session.transcript(), second.session.transcript());
    assert_ne!(legacy.configuration_identity, first.configuration_identity);
    assert_eq!(first.configuration_identity, second.configuration_identity);
    let request = query(&legacy, None);
    assert!(matches!(
        legacy.handle_runtime_observation_v1(request),
        RuntimeObservationResponseV1::Unavailable {
            reason: RuntimeObservationUnavailableV1::NotRequested,
            binding: None,
            ..
        }
    ));
    let first_index = last_checkpoint(&first);
    let second_index = last_checkpoint(&second);
    select(&mut first, first_index);
    select(&mut second, second_index);
    let one = current(&mut first);
    let two = current(&mut second);
    assert_ne!(one.owner, two.owner);
    let request = query(&second, Some(one.owner));
    assert!(matches!(
        second.handle_runtime_observation_v1(request),
        RuntimeObservationResponseV1::Error {
            error: DebugErrorV1 {
                code: DebugErrorCodeV1::InvalidCursor,
                state_changed: false,
                ..
            },
            ..
        }
    ));
}

#[test]
fn actual_runtime_inventory_memory_and_lifecycle_share_exact_cursor_and_slot() {
    let mut backend = backend(true);
    let index = last_checkpoint(&backend);
    select(&mut backend, index);
    let binding = current(&mut backend);
    let identity = inventory(&mut backend, binding);
    assert_eq!(identity.allocation.get(), 1);
    assert_eq!(identity.storage_slot.get(), 1);
    assert_eq!(identity.generation.get(), 1);
    let memory = ResourceRequestV2::ReadAllocationMemory {
        schema: ResourceRequestSchemaV2::V2,
        request_id: backend.command_count + 1,
        expected_revision: backend.revision,
        expected_binding: binding,
        allocation: identity,
        range: ResourceMemoryRangeV1 {
            byte_offset: ResourceDecimalU64V1::new(0),
            byte_len: ResourceDecimalU64V1::new(16),
        },
    };
    let response = backend.handle_resource_queries_v2(memory.clone());
    response
        .validate_for_request(&memory, backend.protocol_limits)
        .unwrap();
    assert!(
        matches!(response, ResourceResponseV2::Ok { result: ResourceQueryResultV2::AllocationMemory { memory }, .. }
        if memory.bytes == "0x11000000110000001100000011000000" && memory.initialized == "0xffff")
    );
    let lifecycle = ResourceRequestV2::QueryAllocationLifecycle {
        schema: ResourceRequestSchemaV2::V2,
        request_id: backend.command_count + 1,
        expected_revision: backend.revision,
        expected_binding: binding,
        page: page(),
    };
    let response = backend.handle_resource_queries_v2(lifecycle.clone());
    response
        .validate_for_request(&lifecycle, backend.protocol_limits)
        .unwrap();
    assert!(matches!(response, ResourceResponseV2::Ok {
        result: ResourceQueryResultV2::AllocationLifecycle { transitions }, ..
    } if transitions.len() == 1 && transitions[0].descriptor.identity == identity
        && transitions[0].kind == ResourceAllocationTransitionKindV2::Preexisting));
    let mut wrong = memory;
    if let ResourceRequestV2::ReadAllocationMemory { allocation, .. } = &mut wrong {
        allocation.generation = ResourceDecimalU64V1::new(2);
    }
    assert!(matches!(
        backend.handle_resource_queries_v2(wrong),
        ResourceResponseV2::Error {
            error: DebugErrorV1 {
                code: DebugErrorCodeV1::InvalidCursor,
                state_changed: false,
                ..
            },
            ..
        }
    ));
}

#[test]
fn actual_reverse_revisit_retains_keys_but_old_full_binding_is_stale() {
    let mut backend = backend(true);
    let index = last_checkpoint(&backend);
    select(&mut backend, index);
    let old_request = query(&backend, None);
    let old = backend.handle_runtime_observation_v1(old_request.clone());
    let (old_binding, old_origin) = match old {
        RuntimeObservationResponseV1::Ok {
            binding, origin, ..
        } => (binding, origin),
        value => panic!("{value:?}"),
    };
    select(&mut backend, 0);
    select(&mut backend, index);
    assert!(matches!(
        backend.handle_runtime_observation_v1(old_request),
        RuntimeObservationResponseV1::Error {
            error: DebugErrorV1 {
                code: DebugErrorCodeV1::StaleRevision,
                ..
            },
            ..
        }
    ));
    let refreshed = query(&backend, Some(old_binding.owner));
    assert!(
        matches!(backend.handle_runtime_observation_v1(refreshed), RuntimeObservationResponseV1::Ok {
        binding, origin, ..
    } if binding.owner == old_binding.owner && binding.cursor.event_sequence == old_binding.cursor.event_sequence
        && binding.cursor.state_revision > old_binding.cursor.state_revision && origin == old_origin)
    );
}

#[test]
fn actual_memory_record_never_borrows_previous_checkpoint_frames() {
    let mut backend = backend(true);
    let index = backend
        .session
        .transcript()
        .records()
        .iter()
        .position(|row| matches!(row.kind, SimulationDebugRecordKindV1::Memory { .. }))
        .unwrap();
    select(&mut backend, index);
    let request = query(&backend, None);
    let response = backend.handle_runtime_observation_v1(request.clone());
    response
        .validate_for_request(&request, backend.protocol_limits)
        .unwrap();
    assert!(matches!(
        response,
        RuntimeObservationResponseV1::Ok {
            origin: RuntimeOperationObservationV1::Available { .. },
            frames: RuntimeFramesV1::Unavailable {
                reason: RuntimeObservationUnavailableV1::NotCheckpoint
            },
            ..
        }
    ));
}

#[test]
fn exhausted_owned_control_budget_reports_failure_not_an_invented_stop() {
    let mut backend = backend(true);
    let before = backend.cursor_sequence();
    let revision = backend.revision;
    backend.session.work = RuntimeReplayWorkV1::new(0).unwrap();
    let navigation = backend.session.seek_record_index(0);
    assert!(matches!(navigation, DebugNavigationV1::Unavailable(_)));
    let placeholder = backend.error(
        Some(1),
        Some(DebugOperationNameV1::Seek),
        DebugErrorStageV1::Backend,
        DebugErrorCodeV1::BackendFailure,
        "placeholder never escapes",
    );
    let response = backend.finish_observed_command(
        1,
        DebugOperationNameV1::Seek,
        before,
        revision,
        placeholder,
    );
    assert!(matches!(
        response,
        DebugResponseV1::Error {
            error: DebugErrorV1 {
                code: DebugErrorCodeV1::ResourceLimit,
                state_changed: false,
                ..
            },
            ..
        }
    ));
    assert_eq!(backend.cursor_sequence(), before);
    assert_eq!(backend.revision, revision);
}

#[test]
fn legacy_backend_keeps_existing_step_and_default_wire_result() {
    let mut first = backend(false);
    let mut second = backend(false);
    let request = DebugRequestV1::Step {
        schema: RequestSchemaV1::V1,
        request_id: 1,
        expected_revision: 0,
        direction: StepDirectionV1::Forward,
        granularity: StepGranularityV1::Operation,
        count: 1,
        focus: None,
    };
    assert_eq!(first.handle(request.clone()), second.handle(request));
    assert!(first.session.observed().is_none());
}

#[test]
fn partially_progressed_owned_control_reports_exact_cursor_and_nonexact_resource_stop() {
    let mut backend = backend(true);
    select(&mut backend, 0);
    let before_cursor = backend.cursor_sequence();
    let before_revision = backend.revision;
    backend.session.begin_command();
    let progressed = backend.session.seek_record_index(1);
    assert!(!matches!(progressed, DebugNavigationV1::Unavailable(_)));
    assert_ne!(backend.cursor_sequence(), before_cursor);
    backend.session.work = RuntimeReplayWorkV1::new(0).unwrap();
    let failed = backend.session.seek_record_index(2);
    assert!(matches!(failed, DebugNavigationV1::Unavailable(_)));
    let placeholder = backend.finish_control(2, DebugOperationNameV1::Seek, before_cursor, failed);
    assert_eq!(backend.revision, before_revision);
    let response = backend.finish_observed_command(
        2,
        DebugOperationNameV1::Seek,
        before_cursor,
        before_revision,
        placeholder,
    );
    response.validate(backend.protocol_limits).unwrap();
    let DebugResponseV1::Ok { result, .. } = response else {
        panic!("real replay progress must be a valid bounded control result");
    };
    assert!(matches!(
        *result,
        DebugResultV1::Control {
            stop: Some(StopViewV1 {
                reason: StopReasonV1::ResourceExhaustion,
                outcome: ExecutionOutcomeV1::Active,
                exact: false,
                ..
            }),
            events_advanced: 1,
            ..
        }
    ));
    assert_eq!(backend.revision, before_revision + 1);
    assert_eq!(backend.cursor_sequence(), 2);
    assert_eq!(
        backend.last_stop.as_ref().unwrap().reason,
        StopReasonV1::ResourceExhaustion
    );
}

#[test]
fn failed_multi_over_rolls_back_without_claiming_progress() {
    let mut backend = backend(true);
    let index = backend
        .session
        .transcript()
        .records()
        .iter()
        .position(|record| {
            matches!(
                record.kind,
                SimulationDebugRecordKindV1::Checkpoint {
                    phase: SimulationDebugCheckpointPhaseV1::BeforeOperation,
                    ..
                }
            )
        })
        .unwrap();
    select(&mut backend, index);
    let before = backend.session_view();
    let stop = backend.last_stop.clone();
    let response = backend.handle(DebugRequestV1::Step {
        schema: RequestSchemaV1::V1,
        request_id: backend.command_count + 1,
        expected_revision: backend.revision,
        direction: StepDirectionV1::Forward,
        granularity: StepGranularityV1::Over,
        count: 2,
        focus: None,
    });
    response.validate(backend.protocol_limits).unwrap();
    assert!(matches!(
        response,
        DebugResponseV1::Error {
            error: DebugErrorV1 {
                state_changed: false,
                ..
            },
            ..
        }
    ));
    assert_eq!(backend.session_view(), before);
    assert_eq!(backend.last_stop, stop);
}

#[test]
fn failed_rollback_preserves_resource_cause_after_semantic_refusal() {
    let mut backend = backend(true);
    select(&mut backend, 0);
    let before = backend.cursor_sequence();
    let revision = backend.revision;
    backend.session.begin_command();
    let _ = backend.session.seek_record_index(1);
    backend.session.remember(RuntimeSessionErrorV1::WrongPhase);
    backend.session.work = RuntimeReplayWorkV1::new(0).unwrap();
    restore_cursor(&mut backend.session, Some(0));
    assert!(matches!(
        backend.session.failure,
        Some(RuntimeSessionErrorV1::WorkLimit)
    ));
    let placeholder = backend.error(
        Some(2),
        Some(DebugOperationNameV1::Step),
        DebugErrorStageV1::Backend,
        DebugErrorCodeV1::BackendFailure,
        "internal placeholder",
    );
    let response = backend.finish_observed_command(
        2,
        DebugOperationNameV1::Step,
        before,
        revision,
        placeholder,
    );
    response.validate(backend.protocol_limits).unwrap();
    assert!(matches!(response, DebugResponseV1::Ok { .. }));
    assert!(!backend.terminated);
    assert_eq!(backend.revision, revision + 1);
    assert_eq!(
        backend.last_stop.as_ref().unwrap().reason,
        StopReasonV1::ResourceExhaustion
    );
}

#[test]
fn actual_no_progress_continue_refusal_preserves_revision_and_prior_stop() {
    let mut backend = backend(true);
    select(&mut backend, 0);
    let before = backend.session_view();
    let stop = backend.last_stop.clone();
    let response = backend.handle(DebugRequestV1::Continue {
        schema: RequestSchemaV1::V1,
        request_id: backend.command_count + 1,
        expected_revision: backend.revision,
        max_events: 1_000_001,
    });
    response.validate(backend.protocol_limits).unwrap();
    assert!(matches!(
        response,
        DebugResponseV1::Error {
            error: DebugErrorV1 {
                code: DebugErrorCodeV1::ResourceLimit,
                state_changed: false,
                ..
            },
            ..
        }
    ));
    assert_eq!(backend.session_view(), before);
    assert_eq!(backend.last_stop, stop);
}
