use alloc::vec;

use crate::*;

fn geometry() -> R11LaunchGeometryV1 {
    R11LaunchGeometryV1 {
        grid: [256, 1, 1],
        workgroup: [64, 1, 1],
    }
}

#[test]
fn events_and_submissions_share_one_completion_and_callbacks_discharge_once() {
    let mut model = R11CompletionModelV1::default();
    model.register_submission_model_only(1).unwrap();
    model.record_event_model_only(10, 1).unwrap();
    model.register_callback_model_only(100, 1).unwrap();
    model.register_callback_model_only(101, 1).unwrap();
    assert_eq!(
        model.query_event_model_only(10),
        Ok(R11CompletionStatusV1::Pending)
    );

    model
        .observe_completion_model_only(1, R11CompletionStatusV1::Succeeded)
        .unwrap();
    assert_eq!(
        model.query_submission_model_only(1),
        model.query_event_model_only(10)
    );
    for callback in [100, 101] {
        let callback = model.callback(callback).unwrap();
        assert!(callback.discharged());
        assert_eq!(
            callback.observed_status(),
            Some(R11CompletionStatusV1::Succeeded)
        );
    }
    assert_eq!(
        model.observe_completion_model_only(1, R11CompletionStatusV1::Cancelled),
        Err(R11RuntimeModelErrorV1::IllegalTransition)
    );
    model.validate_global_invariants().unwrap();
}

#[test]
fn terminal_registration_is_immediate_and_event_custody_blocks_release() {
    let mut model = R11CompletionModelV1::default();
    model.register_submission_model_only(1).unwrap();
    model.record_event_model_only(10, 1).unwrap();
    model
        .observe_completion_model_only(1, R11CompletionStatusV1::Cancelled)
        .unwrap();
    model.register_callback_model_only(100, 1).unwrap();
    assert_eq!(
        model.callback(100).unwrap().observed_status(),
        Some(R11CompletionStatusV1::Cancelled)
    );
    assert_eq!(
        model.release_submission_model_only(1),
        Err(R11RuntimeModelErrorV1::RetainedByEvent)
    );
    model.release_event_model_only(10).unwrap();
    model.release_submission_model_only(1).unwrap();
    assert_eq!(
        model.query_submission_model_only(1),
        Err(R11RuntimeModelErrorV1::UnknownSubmission)
    );
    model.validate_global_invariants().unwrap();
}

#[test]
fn atomic_admission_matches_all_labels_and_both_capability_layers() {
    let contract = R11AtomicLaunchContractV1 {
        operation: R11AtomicOperationV1::Add,
        scope: R11MemoryScopeV1::Device,
        order: R11MemoryOrderV1::AcquireRelease,
        failure_order: None,
        weak: false,
        geometry: geometry(),
    };
    let supported = R11ExecutionCapabilitiesV1 {
        stable: true,
        execution_detail: true,
    };
    assert_eq!(
        admit_atomic_launch_model_only(contract, contract, supported),
        Ok(contract)
    );
    for grid in [[65, 1, 1], [32, 1, 1]] {
        let partial = R11AtomicLaunchContractV1 {
            geometry: R11LaunchGeometryV1 {
                grid,
                workgroup: [64, 1, 1],
            },
            ..contract
        };
        assert_eq!(
            admit_atomic_launch_model_only(partial, partial, supported),
            Ok(partial)
        );
    }
    assert_eq!(
        admit_atomic_launch_model_only(
            contract,
            R11AtomicLaunchContractV1 {
                scope: R11MemoryScopeV1::System,
                ..contract
            },
            supported,
        ),
        Err(R11RuntimeModelErrorV1::InvalidContract)
    );
    assert_eq!(
        admit_atomic_launch_model_only(
            contract,
            contract,
            R11ExecutionCapabilitiesV1 {
                stable: true,
                execution_detail: false,
            },
        ),
        Err(R11RuntimeModelErrorV1::Unsupported)
    );
}

#[test]
fn compare_exchange_admission_matches_failure_order_lattice_and_weakness() {
    use R11MemoryOrderV1::{Acquire, AcquireRelease, Relaxed, Release, SequentiallyConsistent};

    let orders = [
        Relaxed,
        Acquire,
        Release,
        AcquireRelease,
        SequentiallyConsistent,
    ];
    let legal_pairs = [
        (Relaxed, Relaxed),
        (Acquire, Relaxed),
        (Acquire, Acquire),
        (Release, Relaxed),
        (AcquireRelease, Relaxed),
        (AcquireRelease, Acquire),
        (SequentiallyConsistent, Relaxed),
        (SequentiallyConsistent, Acquire),
        (SequentiallyConsistent, SequentiallyConsistent),
    ];
    let supported = R11ExecutionCapabilitiesV1 {
        stable: true,
        execution_detail: true,
    };
    for success in orders {
        for failure in orders {
            for weak in [false, true] {
                let contract = R11AtomicLaunchContractV1 {
                    operation: R11AtomicOperationV1::CompareExchange,
                    scope: R11MemoryScopeV1::Device,
                    order: success,
                    failure_order: Some(failure),
                    weak,
                    geometry: geometry(),
                };
                assert_eq!(
                    admit_atomic_launch_model_only(contract, contract, supported).is_ok(),
                    legal_pairs.contains(&(success, failure)),
                    "unexpected compare-exchange order pair: {success:?}/{failure:?}",
                );
            }
        }
    }

    let missing_failure = R11AtomicLaunchContractV1 {
        operation: R11AtomicOperationV1::CompareExchange,
        scope: R11MemoryScopeV1::Device,
        order: AcquireRelease,
        failure_order: None,
        weak: false,
        geometry: geometry(),
    };
    assert_eq!(
        admit_atomic_launch_model_only(missing_failure, missing_failure, supported),
        Err(R11RuntimeModelErrorV1::InvalidContract)
    );
    for non_cas in [
        R11AtomicLaunchContractV1 {
            operation: R11AtomicOperationV1::Add,
            failure_order: Some(Relaxed),
            ..missing_failure
        },
        R11AtomicLaunchContractV1 {
            operation: R11AtomicOperationV1::Add,
            weak: true,
            ..missing_failure
        },
    ] {
        assert_eq!(
            admit_atomic_launch_model_only(non_cas, non_cas, supported),
            Err(R11RuntimeModelErrorV1::InvalidContract)
        );
    }
}

#[test]
fn collective_admission_gates_exact_geometry_membership_and_system_scope() {
    let contract = R11CollectiveLaunchContractV1 {
        operation: R11CollectiveOperationV1::ReduceSum,
        scope: R11MemoryScopeV1::Workgroup,
        order: R11MemoryOrderV1::AcquireRelease,
        participants: 64,
        geometry: geometry(),
    };
    let supported = R11ExecutionCapabilitiesV1 {
        stable: true,
        execution_detail: true,
    };
    assert_eq!(
        admit_collective_launch_model_only(contract, contract, supported),
        Ok(contract)
    );
    for mutation in [
        R11CollectiveLaunchContractV1 {
            participants: 63,
            ..contract
        },
        R11CollectiveLaunchContractV1 {
            scope: R11MemoryScopeV1::System,
            ..contract
        },
        R11CollectiveLaunchContractV1 {
            geometry: R11LaunchGeometryV1 {
                grid: [0, 1, 1],
                ..geometry()
            },
            ..contract
        },
        R11CollectiveLaunchContractV1 {
            geometry: R11LaunchGeometryV1 {
                grid: [65, 1, 1],
                workgroup: [64, 1, 1],
            },
            ..contract
        },
        R11CollectiveLaunchContractV1 {
            geometry: R11LaunchGeometryV1 {
                grid: [32, 1, 1],
                workgroup: [64, 1, 1],
            },
            ..contract
        },
    ] {
        assert_eq!(
            admit_collective_launch_model_only(mutation, mutation, supported),
            Err(R11RuntimeModelErrorV1::InvalidContract)
        );
    }
}

#[test]
fn persistent_mappings_are_retained_for_the_complete_batch_and_reusable_after_success() {
    let first = R11PersistentMappingKeyV1 {
        mapping_id: 1,
        generation: 7,
    };
    let second = R11PersistentMappingKeyV1 {
        mapping_id: 2,
        generation: 9,
    };
    let mut model = R11PersistentBatchModelV1::default();
    model.register_mapping_model_only(first).unwrap();
    model.register_mapping_model_only(second).unwrap();
    assert_eq!(
        model.register_mapping_model_only(R11PersistentMappingKeyV1 {
            mapping_id: first.mapping_id,
            generation: first.generation + 1,
        }),
        Err(R11RuntimeModelErrorV1::DuplicateIdentity)
    );
    model
        .begin_batch_model_only(10, vec![first, second])
        .unwrap();
    for mapping in [first, second] {
        assert_eq!(
            model.mapping(mapping).unwrap().phase(),
            R11PersistentMappingPhaseV1::RetainedByBatch(10)
        );
        assert_eq!(
            model.release_mapping_model_only(mapping),
            Err(R11RuntimeModelErrorV1::IllegalTransition)
        );
    }
    model.complete_batch_model_only(10, true).unwrap();
    assert_eq!(
        model.mapping(first).unwrap().phase(),
        R11PersistentMappingPhaseV1::Active
    );
    model.begin_batch_model_only(11, vec![first]).unwrap();
    model.complete_batch_model_only(11, true).unwrap();
    assert_eq!(model.mapping(first).unwrap().key(), first);
    model.validate_global_invariants().unwrap();
}

#[test]
fn indeterminate_batch_quarantines_every_mapping_and_prevents_release() {
    let first = R11PersistentMappingKeyV1 {
        mapping_id: 1,
        generation: 1,
    };
    let second = R11PersistentMappingKeyV1 {
        mapping_id: 2,
        generation: 1,
    };
    let mut model = R11PersistentBatchModelV1::default();
    model.register_mapping_model_only(first).unwrap();
    model.register_mapping_model_only(second).unwrap();
    model
        .begin_batch_model_only(10, vec![first, second])
        .unwrap();
    model.complete_batch_model_only(10, false).unwrap();
    for mapping in [first, second] {
        assert_eq!(
            model.mapping(mapping).unwrap().phase(),
            R11PersistentMappingPhaseV1::Quarantined
        );
        assert_eq!(
            model.release_mapping_model_only(mapping),
            Err(R11RuntimeModelErrorV1::IllegalTransition)
        );
    }
    model.validate_global_invariants().unwrap();
}
