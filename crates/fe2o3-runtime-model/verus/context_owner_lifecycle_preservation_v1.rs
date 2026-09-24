use super::*;

verus! {

pub proof fn lifecycle_settlement_preserves_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    writer: WriterReferenceV1, evidence: WriterReferenceV1, writer_storage: usize, member_storage: usize,
    success: bool, result: Result<(), ReadErrorV1>, storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires issued_producer_v1(before, storage, history),
        lifecycle_settlement_relation_v1(before, after, writer, evidence, writer_storage, member_storage, success, result),
    ensures issued_producer_v1(after, storage, history),
{
    if result.is_ok() {
        let (head, count) = match settlement_preflight_decision_v1(before.stable.journal, writer, evidence,
            writer_storage, member_storage) { Ok(value) => value, Err(_) => (None, 0usize) };
        settlement_raw_refines_chain_v1(before.stable.journal, after.stable.journal, writer, evidence,
            writer_storage, member_storage, head, count, success);
        let chain = choose|chain: Seq<usize>| #[trigger] settle_chain_relation_v1(before.stable.journal,
            after.stable.journal, writer, chain, success);
        settle_preserves_issued_producer_v1(before, after, writer, chain, success, storage, history);
    }
}

#[verifier::spinoff_prover]
pub proof fn lifecycle_step_issued_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    step: LifecycleStepV1, storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires issued_producer_v1(before, storage, history), lifecycle_step_relation_v1(before, after, step),
    ensures issued_producer_v1(after, storage, lifecycle_writer_history_v1(history, step)),
{
    match step {
        LifecycleStepV1::EnrollScalar { entry, result } => {
            scalar_enrollment_preserves_v1(before, after, entry, result, storage, history);
        },
        LifecycleStepV1::EnrollBatch { entries, original, output, result } => {
            lifecycle_batch_preserves_v1(before, after, entries, original, output, result, storage, history);
        },
        LifecycleStepV1::Retire { roster, capacity, result } => {
            retirement_producer_preserves_v1(before, after, roster, capacity, result, storage, history);
        },
        LifecycleStepV1::Register { key, result } => {
            register_issued_custody_v1(before.stable.journal, after.stable.journal, key, result, storage, history);
            producer_issuance_frame_v1(before, after, storage, registration_history_v1(history, result));
        },
        LifecycleStepV1::Abort { reference, capacity, result } => {
            lifecycle_abort_preserves_v1(before, after, reference, capacity, result, storage, history);
        },
        LifecycleStepV1::Begin { writer, roster, result } => {
            lifecycle_begin_preserves_v1(before, after, writer, roster, result, storage, history);
        },
        LifecycleStepV1::AcquireStable { consumer, requests, original, output, result } => {
            if stable_wrapper_acquire_header_v1(before, consumer, requests.len() as usize, original).is_ok() {
                stable_acquire_preserves_producer_invariant_v1(before, after, consumer, requests, original, output, result);
            } else {
                reader_allocation_frame_preserves_v1(before.stable, after.stable);
                producer_custody_frame_v1(before.stable, after.stable);
                producer_arena_journal_frame_v1(before, after);
            }
            lifecycle_read_issuance_frame_v1(before, after, storage, history);
        },
        LifecycleStepV1::ReleaseStable { consumer, references, evidence, capacity, result } => {
            stable_release_preserves_producer_invariant_v1(before, after, consumer, references, evidence, capacity, result);
            lifecycle_read_issuance_frame_v1(before, after, storage, history);
        },
        LifecycleStepV1::AcquireProducer { consumer, requests, original, output, result } => {
            producer_acquire_preserves_v1(before, after, consumer, requests, original, output, result);
            lifecycle_read_issuance_frame_v1(before, after, storage, history);
        },
        LifecycleStepV1::ReleaseProducer { consumer, references, evidence, capacity, result } => {
            producer_release_preserves_v1(before, after, consumer, references, evidence, capacity, result);
            lifecycle_read_issuance_frame_v1(before, after, storage, history);
        },
        LifecycleStepV1::SettleSuccess { writer, evidence, writer_storage, member_storage, result } => {
            lifecycle_settlement_preserves_v1(before, after, writer, evidence, writer_storage, member_storage,
                true, result, storage, history);
        },
        LifecycleStepV1::SettleNoEffect { writer, evidence, writer_storage, member_storage, result } => {
            lifecycle_settlement_preserves_v1(before, after, writer, evidence, writer_storage, member_storage,
                false, result, storage, history);
        },
        LifecycleStepV1::Unknown { writer, result } => {
            lifecycle_unknown_preserves_v1(before, after, writer, result, storage, history);
        },
        LifecycleStepV1::Dispose { writer, evidence, roster, writer_storage, member_storage, allocation_storage, result } => {
            disposal_producer_preserves_v1(before, after, writer, evidence, roster, writer_storage,
                member_storage, allocation_storage, result, storage, history);
        },
    }
}

#[verifier::spinoff_prover]
pub proof fn lifecycle_step_histories_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    step: LifecycleStepV1, history: LifecycleHistoryV1)
    requires producer_invariant_v1(before), lifecycle_step_relation_v1(before, after, step),
        lifecycle_stable_history_v1(before.stable, history.stable), lifecycle_producer_history_v1(before, history.producer),
    ensures lifecycle_stable_history_v1(after.stable, lifecycle_next_history_v1(history, step).stable),
        lifecycle_producer_history_v1(after, lifecycle_next_history_v1(history, step).producer),
        reader_epoch_step_v1(before.stable.next_incarnation, after.stable.next_incarnation, lifecycle_stable_minted_v1(step)),
        lifecycle_producer_epoch_step_v1(before.next_incarnation, after.next_incarnation, lifecycle_producer_minted_v1(step)),
{
    assert(history.stable + Seq::<ReadReferenceV1>::empty() =~= history.stable);
    assert(history.producer + Seq::<ProducerReadReferenceV1>::empty() =~= history.producer);
    match step {
        LifecycleStepV1::AcquireStable { consumer, requests, original, output, result } => {
            lifecycle_stable_acquire_epoch_v1(before, after, consumer, requests, original, output, result);
            if stable_wrapper_acquire_header_v1(before, consumer, requests.len() as usize, original).is_ok() {
                lifecycle_stable_acquire_history_v1(before.stable, after.stable, consumer, requests,
                    original, output, result, history.stable);
            }
        },
        LifecycleStepV1::ReleaseStable { consumer, references, evidence, capacity, result } => {
            lifecycle_stable_release_history_v1(before.stable, after.stable, consumer, references, evidence,
                capacity, result, history.stable);
        },
        LifecycleStepV1::AcquireProducer { consumer, requests, original, output, result } => {
            lifecycle_producer_acquire_epoch_v1(before, after, consumer, requests, original, output, result);
            lifecycle_producer_acquire_history_v1(before, after, consumer, requests, original, output, result, history.producer);
        },
        LifecycleStepV1::ReleaseProducer { consumer, references, evidence, capacity, result } => {
            lifecycle_producer_release_history_v1(before, after, consumer, references, evidence, capacity, result, history.producer);
        },
        _ => {},
    }
}

pub proof fn lifecycle_step_preserves_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    step: LifecycleStepV1, storage: StorageCapacitiesV1, history: LifecycleHistoryV1)
    requires lifecycle_invariant_v1(before, storage, history), lifecycle_step_relation_v1(before, after, step),
    ensures lifecycle_invariant_v1(after, storage, lifecycle_next_history_v1(history, step)),
{
    lifecycle_step_issued_v1(before, after, step, storage, history.writers);
    lifecycle_step_histories_v1(before, after, step, history);
}

pub open spec fn lifecycle_shape_v1(contents: ProducerReadContentsV1) -> (u64, usize, usize, nat, nat) {
    (contents.stable.journal.context_generation, contents.stable.journal.allocation_capacity,
        contents.stable.journal.writer_capacity, contents.stable.leases@.len(), contents.reservations@.len())
}

pub proof fn lifecycle_step_shape_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1, step: LifecycleStepV1)
    requires lifecycle_step_relation_v1(before, after, step),
    ensures lifecycle_shape_v1(after) == lifecycle_shape_v1(before),
{
    match step {
        LifecycleStepV1::EnrollScalar { .. } => {}, LifecycleStepV1::EnrollBatch { .. } => {},
        LifecycleStepV1::Retire { .. } => {}, LifecycleStepV1::Register { .. } => {},
        LifecycleStepV1::Abort { .. } => {}, LifecycleStepV1::Begin { .. } => {},
        LifecycleStepV1::AcquireStable { .. } => {}, LifecycleStepV1::ReleaseStable { .. } => {},
        LifecycleStepV1::AcquireProducer { .. } => {}, LifecycleStepV1::ReleaseProducer { .. } => {},
        LifecycleStepV1::SettleSuccess { .. } => {}, LifecycleStepV1::SettleNoEffect { .. } => {},
        LifecycleStepV1::Unknown { .. } => {}, LifecycleStepV1::Dispose { .. } => {},
    }
}

pub proof fn lifecycle_constructor_preserves_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1,
    context: u64, allocations: usize, writers: usize, reads: usize, contents: ProducerReadContentsV1,
    storage: StorageCapacitiesV1)
    requires constructor_producer_relation_v1(before, after, context, allocations, writers, reads, Ok(contents)),
        storage_admission_v1(allocations, writers, storage),
    ensures lifecycle_invariant_v1(contents, storage, lifecycle_empty_history_v1()),
{
    constructor_producer_issued_v1(before, after, context, allocations, writers, reads, contents, storage);
}

pub open spec fn lifecycle_trace_history_v1(steps: Seq<LifecycleStepV1>, end: nat) -> LifecycleHistoryV1
    recommends end <= steps.len(),
    decreases end,
{
    if end == 0 { lifecycle_empty_history_v1() }
    else { lifecycle_next_history_v1(lifecycle_trace_history_v1(steps, (end - 1) as nat), steps[end - 1]) }
}

pub open spec fn lifecycle_trace_relation_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1,
    context: u64, allocations: usize, writers: usize, reads: usize,
    states: Seq<ProducerReadContentsV1>, steps: Seq<LifecycleStepV1>) -> bool
{
    &&& states.len() == steps.len() + 1
    &&& constructor_producer_relation_v1(before, after, context, allocations, writers, reads, Ok(states[0]))
    &&& forall|i: int| 0 <= i < steps.len() ==> lifecycle_step_relation_v1(states[i], states[i + 1], #[trigger] steps[i])
}

pub proof fn lifecycle_trace_prefix_preserves_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1,
    context: u64, allocations: usize, writers: usize, reads: usize, storage: StorageCapacitiesV1,
    states: Seq<ProducerReadContentsV1>, steps: Seq<LifecycleStepV1>, end: nat)
    requires lifecycle_trace_relation_v1(before, after, context, allocations, writers, reads, states, steps),
        storage_admission_v1(allocations, writers, storage), end <= steps.len(),
    ensures lifecycle_invariant_v1(states[end as int], storage, lifecycle_trace_history_v1(steps, end)),
        lifecycle_shape_v1(states[end as int]) == (context, allocations, writers, reads as nat, reads as nat),
    decreases end,
{
    if end == 0 {
        lifecycle_constructor_preserves_v1(before, after, context, allocations, writers, reads, states[0], storage);
    } else {
        lifecycle_trace_prefix_preserves_v1(before, after, context, allocations, writers, reads,
            storage, states, steps, (end - 1) as nat);
        lifecycle_step_preserves_v1(states[end - 1], states[end as int], steps[end - 1], storage,
            lifecycle_trace_history_v1(steps, (end - 1) as nat));
        lifecycle_step_shape_v1(states[end - 1], states[end as int], steps[end - 1]);
    }
}

pub proof fn lifecycle_trace_histories_never_reissue_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1,
    context: u64, allocations: usize, writers: usize, reads: usize, storage: StorageCapacitiesV1,
    states: Seq<ProducerReadContentsV1>, steps: Seq<LifecycleStepV1>, end: nat, first: int, last: int)
    requires lifecycle_trace_relation_v1(before, after, context, allocations, writers, reads, states, steps),
        storage_admission_v1(allocations, writers, storage), end <= steps.len(), 0 <= first < last,
    ensures last < lifecycle_trace_history_v1(steps, end).stable.len() ==>
            lifecycle_trace_history_v1(steps, end).stable[first].incarnation
                < lifecycle_trace_history_v1(steps, end).stable[last].incarnation,
        last < lifecycle_trace_history_v1(steps, end).producer.len() ==>
            lifecycle_trace_history_v1(steps, end).producer[first].incarnation
                < lifecycle_trace_history_v1(steps, end).producer[last].incarnation,
{
    lifecycle_trace_prefix_preserves_v1(before, after, context, allocations, writers, reads, storage, states, steps, end);
}

}
