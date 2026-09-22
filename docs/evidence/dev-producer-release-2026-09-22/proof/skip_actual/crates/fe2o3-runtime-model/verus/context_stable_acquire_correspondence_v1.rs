verus! {

spec fn stable_requests_view(requests: Seq<ContextAllocationReadV1>) -> Seq<logical::AllocationReadV1> {
    requests.map(|_i, request| allocation_read_view(request))
}

spec fn stable_output_slot_view(value: Option<ContextReadLeaseReferenceV1>) -> Option<logical::ReadReferenceV1> {
    match value { None => None, Some(reference) => Some(read_reference_view(reference)) }
}

spec fn stable_output_view(output: Seq<Option<ContextReadLeaseReferenceV1>>) -> Seq<Option<logical::ReadReferenceV1>> {
    output.map(|_i, value| stable_output_slot_view(value))
}

spec fn stable_scan_view(state: StableReadAcquireScanV1) -> logical::ReadScanV1 {
    logical::ReadScanV1 { group: state.group, previous: match state.previous {
        None => None, Some(key) => Some((key.0, key.1, key.2, 0)),
    } }
}

proof fn stable_read_correspondence(actual: JournalContentsV1, model: logical::JournalContentsV1,
    request: ContextAllocationReadV1)
    requires represents(actual, model),
    ensures stable_read_decision_v1(actual, request)
        == begin_result_from(logical::read_decision_v1(model, allocation_read_view(request))),
{
    allocation_lookup_correspondence(actual, model, request.allocation);
}

proof fn stable_capacity_correspondence(actual: ContextReadLeasedJournalV1, model: logical::ReadContentsV1, count: usize)
    requires stable_represents(actual, model),
    ensures stable_capacity_decision_v1(actual, count) == begin_result_from(logical::capacity_decision_v1(model, count)),
{}

proof fn stable_acquire_header_correspondence(actual: ContextReadLeasedJournalV1, model: logical::ReadContentsV1,
    consumer: WriterKeyV1, count: usize, output: Seq<Option<ContextReadLeaseReferenceV1>>)
    requires stable_represents(actual, model),
    ensures stable_acquire_header_v1(actual, consumer, count, output)
        == begin_result_from(logical::acquire_header_v1(model, writer_key_view(consumer), count, stable_output_view(output))),
{
    stable_capacity_correspondence(actual, model, count);
    assert forall|i: int| 0 <= i < output.len() implies
        output[i].is_some() == stable_output_view(output)[i].is_some() by {
        match output[i] { None => {}, Some(_) => {} }
    }
    if exists|i: int| 0 <= i < output.len() && output[i].is_some() {
        let i = choose|i: int| 0 <= i < output.len() && output[i].is_some();
        assert(stable_output_view(output)[i].is_some());
    }
    if exists|i: int| 0 <= i < stable_output_view(output).len() && stable_output_view(output)[i].is_some() {
        let i = choose|i: int| 0 <= i < stable_output_view(output).len() && stable_output_view(output)[i].is_some();
        assert(output[i].is_some());
    }
}

proof fn stable_acquire_item_correspondence(actual: ContextReadLeasedJournalV1, model: logical::ReadContentsV1,
    request: ContextAllocationReadV1, index: usize, state: StableReadAcquireScanV1)
    requires stable_represents(actual, model),
    ensures match logical::acquire_item_v1(model, allocation_read_view(request), index, stable_scan_view(state)) {
        Err(error) => stable_acquire_item_v1(actual, request, index, state) == Err(read_error_embed(error)),
        Ok(next) => stable_acquire_item_v1(actual, request, index, state).is_ok()
            && stable_scan_view(stable_acquire_item_v1(actual, request, index, state).unwrap()) == next,
    },
{
    stable_read_correspondence(actual.journal, model.journal, request);
}

proof fn stable_acquire_scan_correspondence(actual: ContextReadLeasedJournalV1, model: logical::ReadContentsV1,
    requests: Seq<ContextAllocationReadV1>, index: nat, state: StableReadAcquireScanV1)
    requires stable_represents(actual, model),
    ensures stable_acquire_scan_v1(actual, requests, index, state)
        == begin_result_from(logical::acquire_scan_v1(model, stable_requests_view(requests), index, stable_scan_view(state))),
    decreases requests.len() - index,
{
    if index < requests.len() {
        stable_acquire_item_correspondence(actual, model, requests[index as int], index as usize, state);
        if let Ok(next) = stable_acquire_item_v1(actual, requests[index as int], index as usize, state) {
            stable_acquire_scan_correspondence(actual, model, requests, index + 1, next);
        }
    }
}

proof fn stable_acquire_decision_correspondence(actual: ContextReadLeasedJournalV1, model: logical::ReadContentsV1,
    consumer: WriterKeyV1, requests: Seq<ContextAllocationReadV1>, output: Seq<Option<ContextReadLeaseReferenceV1>>)
    requires stable_represents(actual, model),
    ensures stable_acquire_decision_v1(actual, consumer, requests, output)
        == begin_result_from(logical::acquire_decision_v1(model, writer_key_view(consumer), stable_requests_view(requests), stable_output_view(output))),
{
    stable_acquire_header_correspondence(actual, model, consumer, requests.len() as usize, output);
    stable_acquire_scan_correspondence(actual, model, requests, 0, StableReadAcquireScanV1 { previous: None, group: 0 });
}

proof fn stable_slot_count_correspondence(requests: Seq<ContextAllocationReadV1>, slot: usize)
    ensures stable_read_slot_count_v1(requests, slot) == logical::read_slot_count_v1(stable_requests_view(requests), slot),
    decreases requests.len(),
{
    if requests.len() > 0 {
        assert(stable_requests_view(requests).drop_last() =~= stable_requests_view(requests.drop_last()));
        stable_slot_count_correspondence(requests.drop_last(), slot);
    }
}

proof fn stable_acquired_reference_correspondence(actual: ContextReadLeasedJournalV1, model: logical::ReadContentsV1,
    consumer: WriterKeyV1, index: int)
    requires stable_represents(actual, model),
    ensures read_reference_view(stable_acquired_reference_v1(actual, consumer, index))
        == logical::acquired_reference_v1(model, writer_key_view(consumer), index),
{}

proof fn stable_acquired_leases_correspondence(actual: ContextReadLeasedJournalV1, model: logical::ReadContentsV1,
    consumer: WriterKeyV1, requests: Seq<ContextAllocationReadV1>, count: nat)
    requires stable_represents(actual, model), stable_acquire_commit_ready_v1(actual, requests), count <= requests.len(),
    ensures stable_acquired_leases_v1(actual, consumer, requests, count).map(|_i, entry| read_lease_slot_view(entry))
        == logical::acquired_leases_v1(model, writer_key_view(consumer), stable_requests_view(requests), count),
        stable_acquired_leases_v1(actual, consumer, requests, count).len() == actual.leases@.len(),
    decreases count,
{
    if count > 0 {
        stable_acquired_leases_correspondence(actual, model, consumer, requests, (count - 1) as nat);
        stable_acquired_reference_correspondence(actual, model, consumer, count - 1);
        let reference = stable_acquired_reference_v1(actual, consumer, count - 1);
        assert(reference.slot < actual.leases@.len());
        assert(stable_acquired_leases_v1(actual, consumer, requests, count).map(|_i, entry| read_lease_slot_view(entry))
            =~= logical::acquired_leases_v1(model, writer_key_view(consumer), stable_requests_view(requests), count));
    }
}

proof fn stable_acquire_paired_transition(before: ContextReadLeasedJournalV1, after: ContextReadLeasedJournalV1,
    model_before: logical::ReadContentsV1, model_after: logical::ReadContentsV1, consumer: WriterKeyV1,
    requests: Seq<ContextAllocationReadV1>, output_before: Seq<Option<ContextReadLeaseReferenceV1>>,
    output_after: Seq<Option<ContextReadLeaseReferenceV1>>, model_output_after: Seq<Option<logical::ReadReferenceV1>>,
    result: Result<(), ReadErrorV1>, model_result: Result<(), logical::ReadErrorV1>)
    requires stable_represents(before, model_before), stable_guard_storage_v1(before),
        requests.len() <= usize::MAX, requests.len() <= u64::MAX,
        stable_acquire_execution_relation_v1(before, after, consumer, requests, output_before, output_after, result),
        logical::acquire_execution_relation_v1(model_before, model_after, writer_key_view(consumer),
            stable_requests_view(requests), stable_output_view(output_before), model_output_after, model_result),
    ensures result == begin_result_from(model_result), stable_represents(after, model_after),
        stable_output_view(output_after) == model_output_after,
{
    stable_acquire_decision_correspondence(before, model_before, consumer, requests, output_before);
    if result == Ok(()) {
        stable_acquire_preflight_implies_commit_ready_v1(before, consumer, requests, output_before);
        stable_acquired_leases_correspondence(before, model_before, consumer, requests, requests.len());
        assert(after.readers@ =~= model_after.readers@) by {
            assert forall|a: int| 0 <= a < after.readers@.len() implies after.readers@[a] == model_after.readers@[a] by {
                stable_slot_count_correspondence(requests, a as usize);
            }
        }
        assert(stable_output_view(output_after) =~= model_output_after) by {
            assert forall|i: int| 0 <= i < output_after.len() implies
                stable_output_view(output_after)[i] == model_output_after[i] by {
                stable_acquired_reference_correspondence(before, model_before, consumer, i);
            }
        }
    }
}

// Actual execution derives its own preflight/commit premises, including aliased-slot semantics.
fn stable_acquire_historical_exec_v1(actual: &mut ContextReadLeasedJournalV1, model: &mut logical::ReadContentsV1,
    consumer: WriterKeyV1, model_consumer: logical::WriterKeyV1,
    requests: &[ContextAllocationReadV1], model_requests: &[logical::AllocationReadV1],
    output: &mut [Option<ContextReadLeaseReferenceV1>], model_output: &mut Vec<Option<logical::ReadReferenceV1>>)
    -> (results: (Result<(), ReadErrorV1>, Result<(), logical::ReadErrorV1>))
    requires stable_represents(*old(actual), *old(model)), logical::reader_invariant_v1(*old(model)),
        writer_key_view(consumer) == model_consumer, stable_requests_view(requests@) == model_requests@,
        stable_output_view(old(output)@) == old(model_output)@, requests@.len() <= u64::MAX,
    ensures results.0 == begin_result_from(results.1), stable_represents(*final(actual), *final(model)),
        stable_output_view(final(output)@) == final(model_output)@,
        logical::reader_invariant_v1(*final(model)),
        stable_acquire_execution_relation_v1(*old(actual), *final(actual), consumer, requests@, old(output)@, final(output)@, results.0),
        logical::acquire_execution_relation_v1(*old(model), *final(model), model_consumer, model_requests@,
            old(model_output)@, final(model_output)@, results.1),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let ghost output_before = output@;
    let _count = requests.len();
    let result = actual.acquire_reads(consumer, requests, output);
    let model_result = logical::acquire_verified_v1(model, model_consumer, model_requests, model_output);
    proof {
        stable_acquire_paired_transition(before, *actual, model_before, *model, consumer, requests@, output_before,
            output@, model_output@, result, model_result);
    }
    (result, model_result)
}

}
