// Separate typed histories retain released references and never reset their frontiers.
use super::*;

verus! {

pub proof fn lifecycle_concat_contains_v1<T>(left: Seq<T>, right: Seq<T>, value: T)
    ensures (left + right).contains(value) == (left.contains(value) || right.contains(value)),
{
    if left.contains(value) {
        let i = choose|i: int| 0 <= i < left.len() && left[i] == value;
        assert((left + right)[i] == value);
    }
    if right.contains(value) {
        let i = choose|i: int| 0 <= i < right.len() && right[i] == value;
        assert((left + right)[left.len() + i] == value);
    }
    if (left + right).contains(value) {
        let i = choose|i: int| 0 <= i < (left + right).len() && (left + right)[i] == value;
        if i < left.len() { assert(left[i] == value); }
        else { assert(right[i - left.len()] == value); }
    }
}

pub proof fn lifecycle_stable_insert_history_v1(before: ReadContentsV1, consumer: WriterKeyV1,
    requests: Seq<AllocationReadV1>, count: nat, history: Seq<ReadReferenceV1>)
    requires acquire_commit_ready_v1(before, requests), count <= requests.len(),
        forall|s: int| 0 <= s < before.leases@.len() && (#[trigger] before.leases@[s]).is_some()
            ==> history.contains(before.leases@[s].unwrap().reference),
        forall|i: int| 0 <= i < count ==> history.contains(#[trigger] acquired_reference_v1(before, consumer, i)),
    ensures acquired_leases_v1(before, consumer, requests, count).len() == before.leases@.len(),
        forall|s: int| 0 <= s < before.leases@.len()
            && (#[trigger] acquired_leases_v1(before, consumer, requests, count)[s]).is_some()
            ==> history.contains(acquired_leases_v1(before, consumer, requests, count)[s].unwrap().reference),
    decreases count,
{
    if count > 0 {
        lifecycle_stable_insert_history_v1(before, consumer, requests, (count - 1) as nat, history);
        assert(acquire_prefix_entry_v1(before, requests, count - 1));
    }
}

pub proof fn lifecycle_producer_insert_history_v1(before: ProducerReadContentsV1, consumer: WriterKeyV1,
    requests: Seq<ProducerReadV1>, count: nat, history: Seq<ProducerReadReferenceV1>)
    requires producer_acquire_commit_ready_v1(before, requests), count <= requests.len(),
        forall|s: int| 0 <= s < before.reservations@.len() && (#[trigger] before.reservations@[s]).is_some()
            ==> history.contains(before.reservations@[s].unwrap().reference),
        forall|i: int| 0 <= i < count ==> history.contains(#[trigger] producer_acquired_reference_v1(before, consumer, i)),
    ensures producer_acquired_reservations_v1(before, consumer, requests, count).len() == before.reservations@.len(),
        forall|s: int| 0 <= s < before.reservations@.len()
            && (#[trigger] producer_acquired_reservations_v1(before, consumer, requests, count)[s]).is_some()
            ==> history.contains(producer_acquired_reservations_v1(before, consumer, requests, count)[s].unwrap().reference),
    decreases count,
{
    if count > 0 {
        lifecycle_producer_insert_history_v1(before, consumer, requests, (count - 1) as nat, history);
        assert(requests[count - 1].read.allocation.slot < before.counts@.len());
    }
}

pub proof fn lifecycle_stable_remove_history_v1(before: ReadContentsV1, references: Seq<ReadReferenceV1>,
    count: nat, history: Seq<ReadReferenceV1>)
    requires release_commit_ready_v1(before, references), count <= references.len(),
        forall|s: int| 0 <= s < before.leases@.len() && (#[trigger] before.leases@[s]).is_some()
            ==> history.contains(before.leases@[s].unwrap().reference),
    ensures released_leases_v1(before, references, count).len() == before.leases@.len(),
        forall|s: int| 0 <= s < before.leases@.len() && (#[trigger] released_leases_v1(before, references, count)[s]).is_some()
            ==> history.contains(released_leases_v1(before, references, count)[s].unwrap().reference),
    decreases count,
{
    if count > 0 { lifecycle_stable_remove_history_v1(before, references, (count - 1) as nat, history); }
}

pub proof fn lifecycle_producer_remove_history_v1(before: ProducerReadContentsV1, references: Seq<ProducerReadReferenceV1>,
    count: nat, history: Seq<ProducerReadReferenceV1>)
    requires producer_release_commit_ready_v1(before, references), count <= references.len(),
        forall|s: int| 0 <= s < before.reservations@.len() && (#[trigger] before.reservations@[s]).is_some()
            ==> history.contains(before.reservations@[s].unwrap().reference),
    ensures producer_released_reservations_v1(before, references, count).len() == before.reservations@.len(),
        forall|s: int| 0 <= s < before.reservations@.len()
            && (#[trigger] producer_released_reservations_v1(before, references, count)[s]).is_some()
            ==> history.contains(producer_released_reservations_v1(before, references, count)[s].unwrap().reference),
    decreases count,
{
    if count > 0 { lifecycle_producer_remove_history_v1(before, references, (count - 1) as nat, history); }
}

pub proof fn lifecycle_stable_acquire_history_v1(before: ReadContentsV1, after: ReadContentsV1,
    consumer: WriterKeyV1, requests: Seq<AllocationReadV1>, original: Seq<Option<ReadReferenceV1>>,
    output: Seq<Option<ReadReferenceV1>>, result: Result<(), ReadErrorV1>, history: Seq<ReadReferenceV1>)
    requires reader_invariant_v1(before), lifecycle_stable_history_v1(before, history),
        requests.len() <= usize::MAX, requests.len() <= u64::MAX,
        acquire_execution_relation_v1(before, after, consumer, requests, original, output, result),
    ensures lifecycle_stable_history_v1(after, history +
        if result.is_ok() { Seq::new(output.len(), |i: int| output[i].unwrap()) } else { Seq::empty() }),
{
    acquire_epoch_projection_v1(before, after, consumer, requests, original, output, result);
    if let Ok(value) = result {
        assert(value == ());
        acquire_preflight_implies_commit_ready_v1(before, consumer, requests, original);
        let minted = Seq::new(output.len(), |i: int| output[i].unwrap());
        let next = history + minted;
        assert forall|s: int| 0 <= s < before.leases@.len() && (#[trigger] before.leases@[s]).is_some()
            implies next.contains(before.leases@[s].unwrap().reference) by {
            lifecycle_concat_contains_v1(history, minted, before.leases@[s].unwrap().reference);
        }
        assert forall|i: int| 0 <= i < requests.len() implies next.contains(#[trigger] acquired_reference_v1(before, consumer, i)) by {
            assert(minted[i] == acquired_reference_v1(before, consumer, i));
            lifecycle_concat_contains_v1(history, minted, minted[i]);
        }
        lifecycle_stable_insert_history_v1(before, consumer, requests, requests.len(), next);
        assert forall|i: int| 0 <= i < next.len() implies (#[trigger] next[i]).incarnation == i + 1 by {
            if i < history.len() { assert(next[i] == history[i]); }
            else { assert(next[i] == minted[i - history.len()]); }
        }
    } else { assert(history + Seq::<ReadReferenceV1>::empty() =~= history); }
}

pub proof fn lifecycle_producer_acquire_history_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    consumer: WriterKeyV1, requests: Seq<ProducerReadV1>, original: Seq<Option<ProducerReadReferenceV1>>,
    output: Seq<Option<ProducerReadReferenceV1>>, result: Result<(), ReadErrorV1>, history: Seq<ProducerReadReferenceV1>)
    requires producer_invariant_v1(before), lifecycle_producer_history_v1(before, history),
        requests.len() <= usize::MAX, requests.len() <= u64::MAX,
        producer_acquire_execution_relation_v1(before, after, consumer, requests, original, output, result),
    ensures lifecycle_producer_history_v1(after, history +
        if result.is_ok() { Seq::new(output.len(), |i: int| output[i].unwrap()) } else { Seq::empty() }),
{
    lifecycle_producer_acquire_epoch_v1(before, after, consumer, requests, original, output, result);
    if let Ok(value) = result {
        assert(value == ());
        producer_acquire_preflight_ready_v1(before, consumer, requests, original);
        let minted = Seq::new(output.len(), |i: int| output[i].unwrap());
        let next = history + minted;
        assert forall|s: int| 0 <= s < before.reservations@.len() && (#[trigger] before.reservations@[s]).is_some()
            implies next.contains(before.reservations@[s].unwrap().reference) by {
            lifecycle_concat_contains_v1(history, minted, before.reservations@[s].unwrap().reference);
        }
        assert forall|i: int| 0 <= i < requests.len() implies next.contains(#[trigger] producer_acquired_reference_v1(before, consumer, i)) by {
            assert(minted[i] == producer_acquired_reference_v1(before, consumer, i));
            lifecycle_concat_contains_v1(history, minted, minted[i]);
        }
        lifecycle_producer_insert_history_v1(before, consumer, requests, requests.len(), next);
        assert forall|i: int| 0 <= i < next.len() implies (#[trigger] next[i]).incarnation == i + 1 by {
            if i < history.len() { assert(next[i] == history[i]); }
            else { assert(next[i] == minted[i - history.len()]); }
        }
    } else { assert(history + Seq::<ProducerReadReferenceV1>::empty() =~= history); }
}

pub proof fn lifecycle_stable_release_history_v1(before: ReadContentsV1, after: ReadContentsV1,
    consumer: WriterKeyV1, references: Seq<ReadReferenceV1>, evidence: WriterKeyV1, capacity: usize,
    result: Result<(), ReadErrorV1>, history: Seq<ReadReferenceV1>)
    requires reader_invariant_v1(before), lifecycle_stable_history_v1(before, history), references.len() <= usize::MAX,
        release_execution_relation_v1(before, after, consumer, references, evidence, capacity, result),
    ensures lifecycle_stable_history_v1(after, history),
{
    if let Ok(value) = result {
        assert(value == ());
        release_preflight_implies_commit_ready_v1(before, consumer, references, evidence, capacity);
        lifecycle_stable_remove_history_v1(before, references, references.len(), history);
    }
}

pub proof fn lifecycle_producer_release_history_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    consumer: WriterKeyV1, references: Seq<ProducerReadReferenceV1>, evidence: WriterKeyV1, capacity: usize,
    result: Result<(), ReadErrorV1>, history: Seq<ProducerReadReferenceV1>)
    requires producer_invariant_v1(before), lifecycle_producer_history_v1(before, history), references.len() <= usize::MAX,
        producer_release_execution_relation_v1(before, after, consumer, references, evidence, capacity, result),
    ensures lifecycle_producer_history_v1(after, history),
{
    if let Ok(value) = result {
        assert(value == ());
        producer_release_preflight_ready_v1(before, consumer, references, evidence, capacity);
        lifecycle_producer_remove_history_v1(before, references, references.len(), history);
    }
}

}
