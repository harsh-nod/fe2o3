use alloc::vec;

use super::*;

fn geometry() -> R16LaunchGeometryV1 {
    R16LaunchGeometryV1 {
        grid: [256, 1, 1],
        workgroup: [64, 1, 1],
        dynamic_shared_bytes: 32,
    }
}

fn atomic(scope: R16MemoryScopeV1) -> R16SemanticContractV1 {
    R16SemanticContractV1::Atomic(R16AtomicContractV1 {
        operation: R16AtomicOperationV1::CompareExchange,
        scope,
        order: R16MemoryOrderV1::AcquireRelease,
        failure_order: Some(R16MemoryOrderV1::Acquire),
        weak: true,
        geometry: geometry(),
    })
}

fn collective(scope: R16MemoryScopeV1, participants: u64) -> R16SemanticContractV1 {
    R16SemanticContractV1::Collective(R16CollectiveContractV1 {
        operation: R16CollectiveOperationV1::AllReduceSum,
        scope,
        order: R16MemoryOrderV1::AcquireRelease,
        participants,
        geometry: geometry(),
    })
}

fn request(contract: R16SemanticContractV1) -> R16SemanticRequestV1 {
    let kind = match contract {
        R16SemanticContractV1::Atomic(_) => R16SemanticOperationKindV1::Atomic,
        R16SemanticContractV1::Collective(_) => R16SemanticOperationKindV1::Collective,
    };
    R16SemanticRequestV1 {
        opcode: kind,
        variant: kind,
        contract,
        launch: geometry(),
        explicit_kernarg_bytes: 16,
        bindings: vec![R16BindingSummaryV1 {
            kernarg_byte_offset: 0,
            kernarg_patch_is_zero: true,
            region_byte_offset: 8,
            region_byte_len: 64,
        }],
        dependencies: vec![7, 9],
        trailing_bytes: 0,
    }
}

fn ready_model() -> R16WorkerSemanticBoundaryModelV1 {
    let mut model = R16WorkerSemanticBoundaryModelV1::new_model_only();
    model
        .negotiate_model_only(R16WorkerHandshakeV1::ExactRuntimeV5)
        .unwrap();
    model
}

fn summary() -> R16SemanticSidecarSummaryV1 {
    R16SemanticSidecarSummaryV1 {
        schema: R16SemanticSidecarSchemaV1::ExactV1,
        schema_version: 1,
        encoded_byte_len: 4096,
        runtime_profile: 1,
        runtime_capture_scope: 2,
        runtime_profile_dispatches: 3,
        typed_semantic_contracts: 2,
        ordinary_dispatches: 1,
        complete_retained_dispatch_classification: true,
        complete_runtime_operation_history: true,
        runtime_profile_complete_runtime_operation_history: true,
    }
}

fn publication(index: u64) -> R16SemanticPublicationV1 {
    R16SemanticPublicationV1 {
        runtime_event: 30 + index,
        runtime_event_sequence: 4 + index * 3,
        dispatch: 50 + index,
        dispatch_shape: 60 + index,
        launch: geometry(),
    }
}

#[test]
fn only_the_exact_v5_profile_negotiates() {
    let mut accepted = R16WorkerSemanticBoundaryModelV1::new_model_only();
    accepted
        .negotiate_model_only(R16WorkerHandshakeV1::ExactRuntimeV5)
        .unwrap();
    assert_eq!(accepted.phase(), R16WorkerPhaseV1::ReadyV5);

    for handshake in [
        R16WorkerHandshakeV1::RuntimeV1,
        R16WorkerHandshakeV1::RuntimeV4,
        R16WorkerHandshakeV1::Other,
    ] {
        let mut rejected = R16WorkerSemanticBoundaryModelV1::new_model_only();
        assert_eq!(
            rejected.negotiate_model_only(handshake),
            Err(R16WorkerModelErrorV1::HandshakeMismatch)
        );
        assert!(rejected.is_terminal());
        assert_eq!(rejected.attempted_requests(), 0);
        assert_eq!(rejected.accepted_backend_custodies(), 0);
        rejected.validate_global_invariants().unwrap();
    }
}

#[test]
fn exact_worker_frame_formulas_and_caps_are_checked() {
    let mut atomic_request = request(atomic(R16MemoryScopeV1::Device));
    assert_eq!(
        atomic_request.encoded_frame_bytes(),
        Some(63 + 16 + 29 + 16)
    );
    assert!(atomic_request.is_worker_wire_valid());
    assert!(atomic_request.is_composed_pre_custody_valid());

    let collective_request = request(collective(R16MemoryScopeV1::Workgroup, 64));
    assert_eq!(
        collective_request.encoded_frame_bytes(),
        Some(69 + 16 + 29 + 16)
    );
    assert!(collective_request.is_worker_wire_valid());
    assert!(collective_request.is_composed_pre_custody_valid());

    atomic_request.explicit_kernarg_bytes = MAX_R16_EXPLICIT_KERNARG_BYTES_V1;
    atomic_request.bindings = (0..MAX_R16_BINDINGS_V1)
        .map(|index| R16BindingSummaryV1 {
            kernarg_byte_offset: (index * R16_DEVICE_POINTER_BYTES_V1) as u32,
            kernarg_patch_is_zero: true,
            region_byte_offset: index as u64,
            region_byte_len: 1,
        })
        .collect();
    atomic_request.dependencies = (0..MAX_R16_DEPENDENCIES_V1 as u64).collect();
    assert_eq!(atomic_request.encoded_frame_bytes(), Some(1_054_399));
    assert!(atomic_request.is_worker_wire_valid());

    let mut over = atomic_request.clone();
    over.explicit_kernarg_bytes += 1;
    assert!(!over.is_worker_wire_valid());
    over = atomic_request.clone();
    over.bindings.push(R16BindingSummaryV1 {
        kernarg_byte_offset: 2048,
        kernarg_patch_is_zero: true,
        region_byte_offset: 0,
        region_byte_len: 1,
    });
    assert!(!over.is_worker_wire_valid());
    over = atomic_request;
    over.dependencies.push(257);
    assert!(!over.is_worker_wire_valid());

    let overflowing_grid = R16LaunchGeometryV1 {
        grid: [u32::MAX; 3],
        workgroup: [1; 3],
        dynamic_shared_bytes: u32::MAX,
    };
    let overflowing_collective = R16SemanticContractV1::Collective(R16CollectiveContractV1 {
        operation: R16CollectiveOperationV1::AllReduceSum,
        scope: R16MemoryScopeV1::Device,
        order: R16MemoryOrderV1::AcquireRelease,
        participants: u64::MAX,
        geometry: overflowing_grid,
    });
    assert!(!overflowing_collective.is_worker_wire_valid_for(overflowing_grid));
}

#[test]
fn malformed_semantics_are_rejected_before_composed_custody() {
    let base = request(atomic(R16MemoryScopeV1::Device));
    let mut backend_admission_failure = base.clone();
    backend_admission_failure.bindings[0].kernarg_patch_is_zero = false;
    assert!(backend_admission_failure.is_worker_wire_valid());
    assert!(!backend_admission_failure.is_composed_pre_custody_valid());

    let mut malformed = vec![];
    let mut value = base.clone();
    value.variant = R16SemanticOperationKindV1::Collective;
    malformed.push(value);
    let mut value = base.clone();
    value.trailing_bytes = 1;
    malformed.push(value);
    let mut value = base.clone();
    value.dependencies.push(7);
    malformed.push(value);
    let mut value = base.clone();
    value.bindings[0].kernarg_patch_is_zero = false;
    malformed.push(value);
    let mut value = base;
    let R16SemanticContractV1::Atomic(ref mut contract) = value.contract else {
        unreachable!();
    };
    contract.failure_order = Some(R16MemoryOrderV1::Release);
    malformed.push(value);

    for request in malformed {
        let mut model = ready_model();
        assert_eq!(
            model.receive_request_model_only(request),
            Err(R16WorkerModelErrorV1::InvalidRequestBeforeCustody)
        );
        assert_eq!(model.attempted_requests(), 0);
        assert_eq!(model.accepted_backend_custodies(), 0);
        assert!(model.pending_request().is_none());
        assert!(model.is_terminal());
        model.validate_global_invariants().unwrap();
    }
}

#[test]
fn valid_request_and_success_preserve_the_exact_contract() {
    let exact = request(atomic(R16MemoryScopeV1::Device));
    let mut model = ready_model();
    model.receive_request_model_only(exact.clone()).unwrap();
    assert_eq!(model.attempted_requests(), 1);
    assert_eq!(model.accepted_backend_custodies(), 0);
    assert_eq!(model.pending_request(), Some(&exact));
    assert_eq!(
        model.observe_response_model_only(R16WorkerResponseV1::Success { handle: 17 }),
        Ok(R16WorkerOutcomeV1::Success { handle: 17 })
    );
    assert_eq!(model.accepted_backend_custodies(), 1);
    assert_eq!(model.last_successful_request(), Some(&exact));
    assert_eq!(model.phase(), R16WorkerPhaseV1::ReadyV5);
    model.validate_global_invariants().unwrap();
}

#[test]
fn recoverable_responses_do_not_seal_or_fabricate_success() {
    for (response, expected) in [
        (R16WorkerResponseV1::Rejected, R16WorkerOutcomeV1::Rejected),
        (
            R16WorkerResponseV1::Quiescent,
            R16WorkerOutcomeV1::Quiescent,
        ),
    ] {
        let mut model = ready_model();
        model
            .receive_request_model_only(request(atomic(R16MemoryScopeV1::Device)))
            .unwrap();
        assert_eq!(model.observe_response_model_only(response), Ok(expected));
        assert_eq!(model.phase(), R16WorkerPhaseV1::ReadyV5);
        assert_eq!(model.attempted_requests(), 1);
        assert_eq!(model.accepted_backend_custodies(), 0);
        assert!(model.pending_request().is_none());
        assert!(model.last_successful_request().is_none());
        model.validate_global_invariants().unwrap();
    }
}

#[test]
fn every_ambiguous_response_seals_and_terminal_is_absorbing() {
    for response in [
        R16WorkerResponseV1::Success { handle: 0 },
        R16WorkerResponseV1::Terminal,
        R16WorkerResponseV1::Malformed,
        R16WorkerResponseV1::Timeout,
        R16WorkerResponseV1::EndOfFile,
    ] {
        let exact = request(atomic(R16MemoryScopeV1::Device));
        let mut model = ready_model();
        model.receive_request_model_only(exact.clone()).unwrap();
        assert_eq!(
            model.observe_response_model_only(response),
            Err(R16WorkerModelErrorV1::Terminal)
        );
        assert!(model.is_terminal());
        assert!(model.pending_request().is_none());
        assert_eq!(model.indeterminate_request(), Some(&exact));
        assert_eq!(model.attempted_requests(), 1);
        assert_eq!(model.accepted_backend_custodies(), 0);
        let frozen = model.clone();
        assert_eq!(
            model.receive_request_model_only(request(collective(R16MemoryScopeV1::Workgroup, 64,))),
            Err(R16WorkerModelErrorV1::Terminal)
        );
        assert_eq!(model, frozen);
        model.validate_global_invariants().unwrap();
    }
}

#[test]
fn worker_and_direct_kfd_sidecar_validity_are_distinct() {
    let system_atomic = atomic(R16MemoryScopeV1::System);
    assert!(system_atomic.is_worker_wire_valid_for(geometry()));
    assert!(!system_atomic.is_direct_kfd_sidecar_valid_for(geometry()));

    let device_collective = collective(R16MemoryScopeV1::Device, 256);
    assert!(device_collective.is_worker_wire_valid_for(geometry()));
    assert!(!device_collective.is_direct_kfd_sidecar_valid_for(geometry()));

    let system_collective = collective(R16MemoryScopeV1::System, 64);
    assert!(!system_collective.is_worker_wire_valid_for(geometry()));
    assert!(!system_collective.is_direct_kfd_sidecar_valid_for(geometry()));

    for contract in [
        atomic(R16MemoryScopeV1::Device),
        collective(R16MemoryScopeV1::Workgroup, 64),
    ] {
        assert!(contract.is_worker_wire_valid_for(geometry()));
        assert!(contract.is_direct_kfd_sidecar_valid_for(geometry()));
    }
}

#[test]
fn sidecar_bounds_and_complete_classification_fail_closed() {
    let publications = [publication(0), publication(1), publication(2)];
    let observations = [
        R16SemanticObservationV1 {
            dispatch: publications[0].dispatch,
            semantic_contract: Some(atomic(R16MemoryScopeV1::Device)),
        },
        R16SemanticObservationV1 {
            dispatch: publications[1].dispatch,
            semantic_contract: None,
        },
        R16SemanticObservationV1 {
            dispatch: publications[2].dispatch,
            semantic_contract: Some(collective(R16MemoryScopeV1::Workgroup, 64)),
        },
    ];
    let records = observations
        .iter()
        .zip(publications)
        .map(|(observation, publication)| {
            R16SemanticSidecarRecordV1::from_publication(publication, observation.semantic_contract)
        })
        .collect::<alloc::vec::Vec<_>>();
    assert!(summary().is_valid_for(&publications, &observations, &records));
    assert!(
        R16SemanticSidecarSummaryV1 {
            complete_runtime_operation_history: false,
            runtime_profile_complete_runtime_operation_history: false,
            ..summary()
        }
        .is_valid_for(&publications, &observations, &records)
    );
    for invalid in [
        R16SemanticSidecarSummaryV1 {
            schema: R16SemanticSidecarSchemaV1::Other,
            ..summary()
        },
        R16SemanticSidecarSummaryV1 {
            schema_version: 2,
            ..summary()
        },
        R16SemanticSidecarSummaryV1 {
            encoded_byte_len: MAX_R16_SEMANTIC_SIDECAR_BYTES_V1 + 1,
            ..summary()
        },
        R16SemanticSidecarSummaryV1 {
            runtime_profile: 0,
            ..summary()
        },
        R16SemanticSidecarSummaryV1 {
            ordinary_dispatches: 2,
            ..summary()
        },
        R16SemanticSidecarSummaryV1 {
            complete_retained_dispatch_classification: false,
            ..summary()
        },
        R16SemanticSidecarSummaryV1 {
            complete_runtime_operation_history: false,
            ..summary()
        },
    ] {
        assert!(!invalid.is_valid_for(&publications, &observations, &records));
    }

    let oversized_publications = vec![publication(0); MAX_R16_SEMANTIC_SIDECAR_RECORDS_V1 + 1];
    let oversized_observations = vec![
        R16SemanticObservationV1 {
            dispatch: publication(0).dispatch,
            semantic_contract: None,
        };
        MAX_R16_SEMANTIC_SIDECAR_RECORDS_V1 + 1
    ];
    let oversized_records = vec![
        R16SemanticSidecarRecordV1::from_publication(publication(0), None);
        MAX_R16_SEMANTIC_SIDECAR_RECORDS_V1 + 1
    ];
    assert!(
        !R16SemanticSidecarSummaryV1 {
            runtime_profile_dispatches: MAX_R16_SEMANTIC_SIDECAR_RECORDS_V1 + 1,
            typed_semantic_contracts: 0,
            ordinary_dispatches: MAX_R16_SEMANTIC_SIDECAR_RECORDS_V1 + 1,
            ..summary()
        }
        .is_valid_for(
            &oversized_publications,
            &oversized_observations,
            &oversized_records,
        )
    );
}

#[test]
fn exact_ordered_sidecar_join_rejects_omission_reorder_and_substitution() {
    let contracts = [
        Some(atomic(R16MemoryScopeV1::Device)),
        None,
        Some(collective(R16MemoryScopeV1::Workgroup, 64)),
    ];
    let publications = [publication(0), publication(1), publication(2)];
    let observations = [
        R16SemanticObservationV1 {
            dispatch: publications[0].dispatch,
            semantic_contract: contracts[0],
        },
        R16SemanticObservationV1 {
            dispatch: publications[1].dispatch,
            semantic_contract: contracts[1],
        },
        R16SemanticObservationV1 {
            dispatch: publications[2].dispatch,
            semantic_contract: contracts[2],
        },
    ];
    let records = observations
        .iter()
        .zip(publications)
        .map(|(observation, publication)| {
            R16SemanticSidecarRecordV1::from_publication(publication, observation.semantic_contract)
        })
        .collect::<alloc::vec::Vec<_>>();
    assert!(semantic_observation_matches_request_model_only(
        &request(contracts[0].unwrap()),
        publications[0],
        observations[0],
    ));
    assert!(semantic_sidecar_sequence_joins_exactly_model_only(
        summary(),
        &publications,
        &observations,
        &records,
    ));

    assert!(!semantic_sidecar_sequence_joins_exactly_model_only(
        summary(),
        &publications[..2],
        &observations,
        &records,
    ));
    let mut reordered = records.clone();
    reordered.swap(0, 1);
    assert!(!semantic_sidecar_sequence_joins_exactly_model_only(
        summary(),
        &publications,
        &observations,
        &reordered,
    ));
    let mut unordered_publications = publications;
    unordered_publications[2].runtime_event_sequence = 1;
    let mut unordered_records = records.clone();
    unordered_records[2].runtime_event_sequence = 1;
    assert!(!semantic_sidecar_sequence_joins_exactly_model_only(
        summary(),
        &unordered_publications,
        &observations,
        &unordered_records,
    ));
    let mut substituted = records.clone();
    substituted[0].semantic_contract = None;
    assert!(!semantic_sidecar_sequence_joins_exactly_model_only(
        summary(),
        &publications,
        &observations,
        &substituted,
    ));
    let mut duplicate_dispatch = publications;
    duplicate_dispatch[2].dispatch = duplicate_dispatch[0].dispatch;
    let mut duplicate_observations = observations;
    duplicate_observations[2].dispatch = duplicate_dispatch[2].dispatch;
    let mut duplicate_records = records;
    duplicate_records[2].dispatch = duplicate_dispatch[2].dispatch;
    assert!(!semantic_sidecar_sequence_joins_exactly_model_only(
        summary(),
        &duplicate_dispatch,
        &duplicate_observations,
        &duplicate_records,
    ));
}
