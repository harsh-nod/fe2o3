use alloc::{vec, vec::Vec};

use crate::*;

fn digest(seed: u8) -> IdentityDigestV1 {
    IdentityDigestV1::from_untrusted_bytes([seed; IDENTITY_DIGEST_BYTES_V1])
}

fn device(physical: u64) -> DeviceKeyV1 {
    DeviceKeyV1 {
        physical: PhysicalDeviceIdV1(physical),
        generation: DeviceGenerationV1(1),
    }
}

fn stream(device: DeviceKeyV1, stream_id: u64) -> ClosedStreamKeyV1 {
    ClosedStreamKeyV1 {
        device,
        stream_id,
        generation: 1,
    }
}

fn pool(device: DeviceKeyV1, pool_id: u64) -> ClosedPoolKeyV1 {
    ClosedPoolKeyV1 { device, pool_id }
}

fn operation(stream: ClosedStreamKeyV1, sequence: u64) -> ClosedOperationKeyV1 {
    ClosedOperationKeyV1 { stream, sequence }
}

fn model_with_two_devices() -> (
    ClosedExecutionModelV1,
    ClosedStreamKeyV1,
    ClosedStreamKeyV1,
    ClosedPoolKeyV1,
    ClosedPoolKeyV1,
) {
    let left = device(1);
    let right = device(2);
    let left_stream = stream(left, 1);
    let right_stream = stream(right, 2);
    let left_pool = pool(left, 1);
    let right_pool = pool(right, 2);
    let mut model = ClosedExecutionModelV1::new_model_only(digest(1)).unwrap();
    model.register_stream_model_only(left_stream).unwrap();
    model.register_stream_model_only(right_stream).unwrap();
    model
        .register_pool_model_only(left_pool, 64 * 1024, 16)
        .unwrap();
    model
        .register_pool_model_only(right_pool, 64 * 1024, 16)
        .unwrap();
    (model, left_stream, right_stream, left_pool, right_pool)
}

#[test]
fn multiple_streams_retain_in_flight_compute_and_gate_cross_stream_dependency() {
    let (mut model, left_stream, right_stream, left_pool, right_pool) = model_with_two_devices();
    let left_lease = model.lease_model_only(left_pool, 1024, 256).unwrap();
    let right_lease = model.lease_model_only(right_pool, 1024, 256).unwrap();
    let producer = operation(left_stream, 1);
    let consumer = operation(right_stream, 1);
    model
        .prepare_operation_model_only(
            producer,
            ClosedOperationKindV1::Compute {
                execution_device: left_stream.device,
            },
            Vec::new(),
            vec![left_lease],
        )
        .unwrap();
    model
        .prepare_operation_model_only(
            consumer,
            ClosedOperationKindV1::Compute {
                execution_device: right_stream.device,
            },
            vec![producer],
            vec![right_lease],
        )
        .unwrap();
    let producer_batch = model
        .form_prepared_batch_model_only(left_stream, vec![producer])
        .unwrap();
    model
        .publish_prepared_batch_model_only(&producer_batch)
        .unwrap();
    let consumer_batch = model
        .form_prepared_batch_model_only(right_stream, vec![consumer])
        .unwrap();
    assert_eq!(
        model.publish_prepared_batch_model_only(&consumer_batch),
        Err(ClosedExecutionErrorV1::DependencyNotCompleted)
    );
    assert_eq!(model.retained_operation_count(), 2);
    assert_eq!(
        model.operation(consumer).unwrap().phase(),
        ClosedOperationPhaseV1::Prepared
    );

    model.observe_completion_model_only(producer).unwrap();
    model
        .publish_prepared_batch_model_only(&consumer_batch)
        .unwrap();
    assert_eq!(
        model.operation(producer).unwrap().phase(),
        ClosedOperationPhaseV1::CompletionObserved
    );
    assert!(matches!(
        model.operation(consumer).unwrap().phase(),
        ClosedOperationPhaseV1::Published { .. }
    ));
    model.validate_global_invariants().unwrap();
}

#[test]
fn prepared_batch_publication_is_all_or_nothing_and_preserves_roster() {
    let (mut model, left_stream, _, left_pool, _) = model_with_two_devices();
    let first_lease = model.lease_model_only(left_pool, 256, 64).unwrap();
    let second_lease = model.lease_model_only(left_pool, 256, 64).unwrap();
    let first = operation(left_stream, 1);
    let second = operation(left_stream, 2);
    for (key, lease) in [(first, first_lease), (second, second_lease)] {
        model
            .prepare_operation_model_only(
                key,
                ClosedOperationKindV1::Compute {
                    execution_device: left_stream.device,
                },
                Vec::new(),
                vec![lease],
            )
            .unwrap();
    }
    assert!(matches!(
        model.form_prepared_batch_model_only(left_stream, vec![second]),
        Err(ClosedExecutionErrorV1::InvalidSequence)
    ));
    let batch = model
        .form_prepared_batch_model_only(left_stream, vec![first, second])
        .unwrap();
    let epoch = model.publish_prepared_batch_model_only(&batch).unwrap();
    for key in [first, second] {
        assert_eq!(
            model.operation(key).unwrap().phase(),
            ClosedOperationPhaseV1::Published {
                batch_id: batch.batch_id(),
                publication_epoch: epoch,
            }
        );
    }
    assert_eq!(
        model.publish_prepared_batch_model_only(&batch),
        Err(ClosedExecutionErrorV1::InvalidSequence)
    );
    model.validate_global_invariants().unwrap();
}

#[test]
fn dependency_failure_cannot_partially_publish_a_prepared_batch() {
    let (mut model, left_stream, right_stream, left_pool, right_pool) = model_with_two_devices();
    let blocker_lease = model.lease_model_only(right_pool, 64, 64).unwrap();
    let blocker = operation(right_stream, 1);
    model
        .prepare_operation_model_only(
            blocker,
            ClosedOperationKindV1::Compute {
                execution_device: right_stream.device,
            },
            Vec::new(),
            vec![blocker_lease],
        )
        .unwrap();
    let blocker_batch = model
        .form_prepared_batch_model_only(right_stream, vec![blocker])
        .unwrap();
    model
        .publish_prepared_batch_model_only(&blocker_batch)
        .unwrap();

    let first = operation(left_stream, 1);
    let second = operation(left_stream, 2);
    let first_lease = model.lease_model_only(left_pool, 64, 64).unwrap();
    let second_lease = model.lease_model_only(left_pool, 64, 64).unwrap();
    model
        .prepare_operation_model_only(
            first,
            ClosedOperationKindV1::Compute {
                execution_device: left_stream.device,
            },
            Vec::new(),
            vec![first_lease],
        )
        .unwrap();
    model
        .prepare_operation_model_only(
            second,
            ClosedOperationKindV1::Compute {
                execution_device: left_stream.device,
            },
            vec![blocker],
            vec![second_lease],
        )
        .unwrap();
    let batch = model
        .form_prepared_batch_model_only(left_stream, vec![first, second])
        .unwrap();
    assert_eq!(
        model.publish_prepared_batch_model_only(&batch),
        Err(ClosedExecutionErrorV1::DependencyNotCompleted)
    );
    assert_eq!(
        model.operation(first).unwrap().phase(),
        ClosedOperationPhaseV1::Prepared
    );
    assert_eq!(
        model.operation(second).unwrap().phase(),
        ClosedOperationPhaseV1::Prepared
    );

    model.observe_completion_model_only(blocker).unwrap();
    model.publish_prepared_batch_model_only(&batch).unwrap();
    assert!(matches!(
        model.operation(first).unwrap().phase(),
        ClosedOperationPhaseV1::Published { .. }
    ));
    assert!(matches!(
        model.operation(second).unwrap().phase(),
        ClosedOperationPhaseV1::Published { .. }
    ));
    model.validate_global_invariants().unwrap();
}

#[test]
fn completion_and_cancel_advance_pool_generations_before_reuse() {
    let (mut model, left_stream, _, left_pool, _) = model_with_two_devices();
    let first_lease = model.lease_model_only(left_pool, 4096, 4096).unwrap();
    let first = operation(left_stream, 1);
    model
        .prepare_operation_model_only(
            first,
            ClosedOperationKindV1::Compute {
                execution_device: left_stream.device,
            },
            Vec::new(),
            vec![first_lease],
        )
        .unwrap();
    let batch = model
        .form_prepared_batch_model_only(left_stream, vec![first])
        .unwrap();
    model.publish_prepared_batch_model_only(&batch).unwrap();
    assert_eq!(
        model.release_completed_model_only(first),
        Err(ClosedExecutionErrorV1::IllegalTransition)
    );
    model.observe_completion_model_only(first).unwrap();
    model.release_completed_model_only(first).unwrap();
    let reused = model.lease_model_only(left_pool, 1024, 256).unwrap();
    assert_eq!(reused.block_id, first_lease.block_id);
    assert_eq!(reused.generation, first_lease.generation + 1);

    let second = operation(left_stream, 2);
    model
        .prepare_operation_model_only(
            second,
            ClosedOperationKindV1::Compute {
                execution_device: left_stream.device,
            },
            Vec::new(),
            vec![reused],
        )
        .unwrap();
    model.cancel_before_publication_model_only(second).unwrap();
    let reused_again = model.lease_model_only(left_pool, 512, 128).unwrap();
    assert_eq!(reused_again.block_id, reused.block_id);
    assert_eq!(reused_again.generation, reused.generation + 1);
    model.validate_global_invariants().unwrap();
}

#[test]
fn peer_copy_indeterminate_failure_retains_both_device_owners() {
    let (mut model, _, right_stream, left_pool, right_pool) = model_with_two_devices();
    let source = model.lease_model_only(left_pool, 2048, 256).unwrap();
    let destination = model.lease_model_only(right_pool, 2048, 256).unwrap();
    let copy = operation(right_stream, 1);
    model
        .prepare_operation_model_only(
            copy,
            ClosedOperationKindV1::PeerCopy {
                source_device: left_pool.device,
                destination_device: right_pool.device,
                execution_device: right_pool.device,
            },
            Vec::new(),
            vec![source, destination],
        )
        .unwrap();
    let batch = model
        .form_prepared_batch_model_only(right_stream, vec![copy])
        .unwrap();
    model.publish_prepared_batch_model_only(&batch).unwrap();
    assert!(model.request_cancellation_model_only(copy).unwrap());
    assert!(!model.request_cancellation_model_only(copy).unwrap());
    assert_eq!(model.observe_timeout_model_only(copy).unwrap(), 1);
    model.quarantine_published_model_only(copy).unwrap();
    assert_eq!(
        model.release_completed_model_only(copy),
        Err(ClosedExecutionErrorV1::IllegalTransition)
    );
    assert_eq!(
        model
            .blocks()
            .iter()
            .filter(|block| block.phase() == ClosedPoolBlockPhaseV1::Quarantined(copy))
            .count(),
        2
    );
    let replacement_source = model.lease_model_only(left_pool, 2048, 256).unwrap();
    let replacement_destination = model.lease_model_only(right_pool, 2048, 256).unwrap();
    assert_ne!(replacement_source.block_id, source.block_id);
    assert_ne!(replacement_destination.block_id, destination.block_id);
    model.validate_global_invariants().unwrap();
}

#[test]
fn peer_copy_rejects_reversed_device_ownership_before_mutation() {
    let (mut model, _, right_stream, left_pool, right_pool) = model_with_two_devices();
    let source = model.lease_model_only(left_pool, 64, 64).unwrap();
    let destination = model.lease_model_only(right_pool, 64, 64).unwrap();
    assert_eq!(
        model.prepare_operation_model_only(
            operation(right_stream, 1),
            ClosedOperationKindV1::PeerCopy {
                source_device: right_pool.device,
                destination_device: left_pool.device,
                execution_device: left_pool.device,
            },
            Vec::new(),
            vec![source, destination],
        ),
        Err(ClosedExecutionErrorV1::InvalidDeviceOwnership)
    );
    assert!(model.operations().is_empty());
    assert!(
        model
            .blocks()
            .iter()
            .all(|block| block.phase() == ClosedPoolBlockPhaseV1::Leased)
    );
}

fn atomic_step(
    operation: ClosedAtomicOperationV1,
    order: ClosedAtomicOrderV1,
    scope: ClosedAtomicScopeV1,
    old_value: u64,
    operand: u64,
) -> UntrustedClosedAtomicStepV1 {
    let fences = ClosedAtomicFencePlanV1 {
        pre_release: matches!(
            order,
            ClosedAtomicOrderV1::Release
                | ClosedAtomicOrderV1::AcquireRelease
                | ClosedAtomicOrderV1::SequentiallyConsistent
        ),
        post_acquire: matches!(
            order,
            ClosedAtomicOrderV1::Acquire
                | ClosedAtomicOrderV1::AcquireRelease
                | ClosedAtomicOrderV1::SequentiallyConsistent
        ),
        sequentially_consistent: order == ClosedAtomicOrderV1::SequentiallyConsistent,
    };
    let (new_value, returned_value) = match operation {
        ClosedAtomicOperationV1::Load => (old_value, Some(old_value)),
        ClosedAtomicOperationV1::Store => (operand, None),
        ClosedAtomicOperationV1::Exchange => (operand, Some(old_value)),
        ClosedAtomicOperationV1::FetchAdd => (old_value.wrapping_add(operand), Some(old_value)),
    };
    UntrustedClosedAtomicStepV1 {
        operation,
        declared_order: order,
        declared_scope: scope,
        observed_operation: operation,
        observed_order: order,
        observed_scope: scope,
        fences,
        old_value,
        operand,
        new_value,
        returned_value,
    }
}

#[test]
fn atomic_load_store_and_rmw_bind_order_scope_fences_and_values() {
    for step in [
        atomic_step(
            ClosedAtomicOperationV1::Load,
            ClosedAtomicOrderV1::Acquire,
            ClosedAtomicScopeV1::Device,
            7,
            0,
        ),
        atomic_step(
            ClosedAtomicOperationV1::Store,
            ClosedAtomicOrderV1::Release,
            ClosedAtomicScopeV1::System,
            7,
            9,
        ),
        atomic_step(
            ClosedAtomicOperationV1::Exchange,
            ClosedAtomicOrderV1::AcquireRelease,
            ClosedAtomicScopeV1::Workgroup,
            7,
            9,
        ),
        atomic_step(
            ClosedAtomicOperationV1::FetchAdd,
            ClosedAtomicOrderV1::SequentiallyConsistent,
            ClosedAtomicScopeV1::System,
            u64::MAX,
            2,
        ),
    ] {
        assert_eq!(
            admit_closed_atomic_step_model_only_v1(step).unwrap().step(),
            step
        );
    }

    let mut wrong_scope = atomic_step(
        ClosedAtomicOperationV1::Load,
        ClosedAtomicOrderV1::Acquire,
        ClosedAtomicScopeV1::System,
        1,
        0,
    );
    wrong_scope.observed_scope = ClosedAtomicScopeV1::Device;
    assert_eq!(
        admit_closed_atomic_step_model_only_v1(wrong_scope),
        Err(ClosedAtomicCorrespondenceErrorV1::ScopeMismatch)
    );
    let mut missing_fence = atomic_step(
        ClosedAtomicOperationV1::Store,
        ClosedAtomicOrderV1::Release,
        ClosedAtomicScopeV1::Device,
        1,
        2,
    );
    missing_fence.fences.pre_release = false;
    assert_eq!(
        admit_closed_atomic_step_model_only_v1(missing_fence),
        Err(ClosedAtomicCorrespondenceErrorV1::FenceMismatch)
    );
}

fn completed_wave(
    operation: Wave64OperationV1,
    inputs: [u64; WAVE64_LANE_COUNT_V1],
) -> Wave64ConvergenceModelV1 {
    let mut wave =
        Wave64ConvergenceModelV1::new_model_only(operation, inputs, u64::MAX, true).unwrap();
    for lane in 0..WAVE64_LANE_COUNT_V1 {
        wave.arrive_model_only(lane).unwrap();
    }
    assert_eq!(wave.phase(), Wave64PhaseV1::Ready);
    wave.publish_model_only().unwrap();
    wave
}

#[test]
fn wave64_barrier_reduction_and_scans_publish_only_after_exact_convergence() {
    let inputs = core::array::from_fn(|lane| lane as u64 + 1);
    assert_eq!(
        completed_wave(Wave64OperationV1::Barrier, inputs)
            .outputs()
            .unwrap(),
        &inputs
    );
    assert!(
        completed_wave(Wave64OperationV1::ReduceSumWrappingU64, inputs)
            .outputs()
            .unwrap()
            .iter()
            .all(|value| *value == 2080)
    );
    let inclusive = completed_wave(Wave64OperationV1::InclusiveScanSumWrappingU64, inputs);
    assert_eq!(inclusive.outputs().unwrap()[0], 1);
    assert_eq!(inclusive.outputs().unwrap()[63], 2080);
    let exclusive = completed_wave(Wave64OperationV1::ExclusiveScanSumWrappingU64, inputs);
    assert_eq!(exclusive.outputs().unwrap()[0], 0);
    assert_eq!(exclusive.outputs().unwrap()[63], 2016);
}

#[test]
fn wave64_rejects_divergence_partial_masks_duplicates_and_early_publication() {
    let inputs = [1_u64; WAVE64_LANE_COUNT_V1];
    assert_eq!(
        Wave64ConvergenceModelV1::new_model_only(
            Wave64OperationV1::Barrier,
            inputs,
            u64::MAX,
            false,
        ),
        Err(Wave64ConvergenceErrorV1::DivergentControlFlow)
    );
    assert_eq!(
        Wave64ConvergenceModelV1::new_model_only(
            Wave64OperationV1::Barrier,
            inputs,
            u64::MAX - 1,
            true,
        ),
        Err(Wave64ConvergenceErrorV1::IncompletePhysicalMask)
    );
    let mut wave = Wave64ConvergenceModelV1::new_model_only(
        Wave64OperationV1::ReduceSumWrappingU64,
        inputs,
        u64::MAX,
        true,
    )
    .unwrap();
    wave.arrive_model_only(0).unwrap();
    assert_eq!(
        wave.arrive_model_only(0),
        Err(Wave64ConvergenceErrorV1::DuplicateArrival)
    );
    assert_eq!(
        wave.publish_model_only(),
        Err(Wave64ConvergenceErrorV1::IncompleteArrival)
    );
}
