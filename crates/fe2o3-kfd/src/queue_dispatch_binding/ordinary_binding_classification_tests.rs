//! Genuine resource binding through the Ordinary classifier, with CPU no-effect receipts.

use super::*;
use crate::queue::live::{
    ComputeAqlQueueSessionErrorV1, ComputeAqlQueueSessionV1, Gfx942FixedDispatchSubmissionFailureV1,
};
use crate::queue::submit::{NativeAqlSubmissionErrorV1, NativeAqlSubmissionFailureV1};

fn refused<const N: usize>(
    owner: DispatchResourceOwnerV1,
    memory: &Memory,
    queue: QueueKeyV1,
    expected_error: &str,
) -> DispatchResourceOwnerV1 {
    let before_generation = owner.source_failure_snapshot_v1();
    let before_resources = resources(&owner, memory);
    let before_memory = memory.observation();
    let (result, owner, terminal, completion) =
        ComputeAqlQueueSessionV1::with_ordinary_binding_session_v1(queue, owner, |session| {
            session.submit_ordinary_binding_for_test::<N>(|_, _| {
                panic!("resource-binding refusal must not reach native submission")
            })
        });
    let Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
        ComputeAqlQueueSessionErrorV1::DispatchBinding(error),
    ) = result.unwrap_err()
    else {
        panic!("Ordinary binding refusal changed its classification or error wrapper")
    };
    assert_eq!(error.to_string(), expected_error);
    assert!(!terminal);
    assert_eq!(completion[0], completion[1]);
    assert_eq!(owner.source_failure_snapshot_v1(), before_generation);
    assert_eq!(resources(&owner, memory), before_resources);
    assert_eq!(memory.observation(), before_memory);
    owner
}

fn corrected_retry(
    owner: DispatchResourceOwnerV1,
    memory: &Memory,
    queue: QueueKeyV1,
    selected_slot: usize,
) -> DispatchResourceOwnerV1 {
    let mut expected = owner.source_failure_snapshot_v1();
    assert_eq!(
        expected.slots[selected_slot].phase,
        DispatchEpochPhaseV1::Vacant
    );
    expected.recipe_queue = Some(queue);
    expected.next_generation += 1;
    expected.slots[selected_slot].slot_generation += 1;
    let before_resources = resources(&owner, memory);
    let before_memory = memory.observation();
    let mut native_calls = 0;
    let (result, owner, terminal, completion) =
        ComputeAqlQueueSessionV1::with_ordinary_binding_session_v1(queue, owner, |session| {
            session.submit_ordinary_binding_for_test::<3>(|_, packets| {
                native_calls += 1;
                assert_eq!(packets.packet_count(), 3);
                Err(NativeAqlSubmissionFailureV1::RetryableBeforeSideEffect(
                    NativeAqlSubmissionErrorV1::Ring(
                        fe2o3_aql::AqlRingReservationError::InsufficientSpace {
                            requested: 3,
                            available: 0,
                        },
                    ),
                ))
            })
        });
    assert!(matches!(
        result,
        Err(
            Gfx942FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(
                ComputeAqlQueueSessionErrorV1::Native("submission ring occupancy")
            )
        )
    ));
    assert_eq!(native_calls, 1);
    assert!(!terminal);
    // Completion identities burn even though cancellation restores every signal.
    // The fixture independently requires the completion owner to be releasable.
    assert_ne!(completion[0], completion[1]);
    assert_eq!(owner.source_failure_snapshot_v1(), expected);
    assert_eq!(resources(&owner, memory), before_resources);
    assert_eq!(memory.observation(), before_memory);
    owner
}

#[test]
fn ordinary_genuine_template_refusals_preserve_exact_error_owner_and_completion() {
    let (mut memory, mut owner, queue) = fixture();
    owner.generation.poisoned = true;
    owner = refused::<0>(owner, &memory, queue, "Poisoned");
    owner.generation.poisoned = false;
    owner = refused::<0>(owner, &memory, queue, "ZeroPacketCount");
    owner = refused::<8193>(
        owner,
        &memory,
        queue,
        "PacketCountExceedsMaximum { requested: 8193, maximum: 8192 }",
    );
    owner = refused::<2>(owner, &memory, queue, "WrongQueueGeneration");

    let original = owner.packets.clone();
    owner.packets[0].code_index = usize::MAX;
    owner.packets[2].ordering = AqlDispatchOrderingV1::Independent;
    owner = refused::<3>(
        owner,
        &memory,
        queue,
        "InvalidKernarg { packet: 2, detail: \"multi-inflight recipe requires wait-for-prior ordering\" }",
    );
    owner.packets[2].ordering = AqlDispatchOrderingV1::WaitForPrior;
    owner = refused::<3>(
        owner,
        &memory,
        queue,
        "InvalidCode(\"packet program index\")",
    );
    owner.packets.copy_from_slice(&original);
    owner.packets[2].code_bound_kernarg_layout = true;
    owner.packets[2].kernarg_layout_identity = [0xff; 32];
    owner = refused::<3>(
        owner,
        &memory,
        queue,
        "InvalidKernarg { packet: 2, detail: \"prepared kernarg dispatch ABI identity\" }",
    );
    owner.packets.copy_from_slice(&original);
    owner = corrected_retry(owner, &memory, queue, 0);
    cleanup(owner, &mut memory);
}

#[test]
fn ordinary_genuine_vm_refusal_precedes_late_packet_errors_without_binding() {
    let (mut memory, mut owner, queue) = fixture();
    let original = owner.packets.clone();
    // Leave one retained program unused: its VM must still be checked.
    owner.packets[2].code_index = 0;
    owner.packets[0].code_bound_kernarg_layout = true;
    owner.packets[0].kernarg_layout_identity = [0xff; 32];
    let original_code = owner.code_identity[1];
    owner.code_identity[1].mapping.allocation.vm.id.0 += 1;
    owner = refused::<3>(owner, &memory, queue, "WrongQueueGeneration");
    owner.code_identity[1] = original_code;
    owner = refused::<3>(
        owner,
        &memory,
        queue,
        "InvalidKernarg { packet: 0, detail: \"prepared kernarg dispatch ABI identity\" }",
    );
    owner.packets.copy_from_slice(&original);
    owner = corrected_retry(owner, &memory, queue, 0);
    cleanup(owner, &mut memory);
}

#[test]
fn ordinary_genuine_generation_refusals_preserve_live_neighbors_and_corrected_retry() {
    let (mut memory, mut owner, queue) = fixture();
    let (_, left) = owner.bind_templates::<3>(queue).unwrap();
    let (_, right) = owner.bind_templates::<3>(queue).unwrap();
    let next_generation = owner.generation.next_generation;
    owner.generation.next_generation = u64::MAX;
    owner = refused::<3>(owner, &memory, queue, "GenerationExhausted");
    // The raw epoch leaf accepts zero; the complete template path must reject
    // its completion roster without reserving an epoch or entering native work.
    owner.generation.next_generation = 0;
    owner = refused::<3>(owner, &memory, queue, "Completion(StaleBatchGeneration)");
    owner.generation.next_generation = next_generation;
    owner = corrected_retry(owner, &memory, queue, 2);
    owner.cancel_binding(left).unwrap();
    owner.cancel_binding(right).unwrap();
    cleanup(owner, &mut memory);
}

#[test]
fn ordinary_genuine_capacity_refusal_precedes_late_abi_and_reuses_only_freed_epoch() {
    let (mut memory, mut owner, queue) = fixture();
    let mut epochs = Vec::new();
    for _ in 0..64 {
        epochs.push(owner.bind_templates::<3>(queue).unwrap().1);
    }
    let original = owner.packets[2];
    owner.packets[2].code_bound_kernarg_layout = true;
    owner.packets[2].kernarg_layout_identity = [0xff; 32];
    owner = refused::<3>(
        owner,
        &memory,
        queue,
        "DispatchEpochCapacity { maximum: 64 }",
    );
    owner.cancel_binding(epochs.pop().unwrap()).unwrap();
    owner = refused::<3>(
        owner,
        &memory,
        queue,
        "InvalidKernarg { packet: 2, detail: \"prepared kernarg dispatch ABI identity\" }",
    );
    owner.packets[2] = original;
    owner = corrected_retry(owner, &memory, queue, 63);
    for identity in epochs {
        owner.cancel_binding(identity).unwrap();
    }
    cleanup(owner, &mut memory);
}
