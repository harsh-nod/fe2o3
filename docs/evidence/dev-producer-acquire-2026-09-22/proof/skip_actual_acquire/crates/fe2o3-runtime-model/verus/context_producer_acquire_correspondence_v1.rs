verus! {

spec fn producer_requests_view(requests: Seq<ContextProducerReadV1>) -> Seq<logical::ProducerReadV1> {
    requests.map(|_i, request| producer_read_view(request))
}

spec fn producer_output_slot_view(value: Option<ContextProducerReadReferenceV1>) -> Option<logical::ProducerReadReferenceV1> {
    match value { None => None, Some(reference) => Some(producer_reference_view(reference)) }
}

spec fn producer_output_view(output: Seq<Option<ContextProducerReadReferenceV1>>) -> Seq<Option<logical::ProducerReadReferenceV1>> {
    output.map(|_i, value| producer_output_slot_view(value))
}

spec fn producer_scan_view(state: ProducerReadAcquireScanV1) -> logical::ReadScanV1 {
    logical::ReadScanV1 { group: state.group, previous: match state.previous {
        None => None, Some(key) => Some((key.0, key.1, key.2, 0)),
    } }
}

proof fn producer_capacity_correspondence(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1, count: usize)
    requires producer_represents(actual, model),
    ensures producer_capacity_decision_v1(actual, count) == begin_result_from(logical::producer_capacity_decision_v1(model, count)),
{}

fn producer_capacity_historical_exec_v1(actual: &ContextProducerReadJournalV1, model: &logical::ProducerReadContentsV1,
    count: usize) -> (results: (Result<(), ReadErrorV1>, Result<(), logical::ReadErrorV1>))
    requires producer_represents(*actual, *model), logical::producer_invariant_v1(*model), count <= u64::MAX,
    ensures results.0 == begin_result_from(results.1),
        results.0 == producer_capacity_decision_v1(*actual, count),
        results.1 == logical::producer_capacity_decision_v1(*model, count),
{
    proof { logical::producer_capacity_arithmetic_v1(*model); }
    let result = actual.validate_producer_read_capacity(count);
    let model_result = logical::producer_capacity_exec_v1(model, count);
    proof { producer_capacity_correspondence(*actual, *model, count); }
    (result, model_result)
}

proof fn producer_acquire_header_correspondence(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    consumer: WriterKeyV1, count: usize, output: Seq<Option<ContextProducerReadReferenceV1>>)
    requires producer_represents(actual, model),
    ensures producer_acquire_header_v1(actual, consumer, count, output)
        == begin_result_from(logical::producer_acquire_header_v1(model, writer_key_view(consumer), count, producer_output_view(output))),
{
    producer_capacity_correspondence(actual, model, count);
    assert forall|i: int| 0 <= i < output.len() implies
        output[i].is_some() == producer_output_view(output)[i].is_some() by {
        match output[i] { None => {}, Some(_) => {} }
    }
    if exists|i: int| 0 <= i < output.len() && output[i].is_some() {
        let i = choose|i: int| 0 <= i < output.len() && output[i].is_some();
        assert(producer_output_view(output)[i].is_some());
    }
    if exists|i: int| 0 <= i < producer_output_view(output).len() && producer_output_view(output)[i].is_some() {
        let i = choose|i: int| 0 <= i < producer_output_view(output).len() && producer_output_view(output)[i].is_some();
        assert(output[i].is_some());
    }
}

proof fn producer_acquire_item_correspondence(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    consumer: WriterKeyV1, request: ContextProducerReadV1, index: usize, state: ProducerReadAcquireScanV1)
    requires producer_represents(actual, model),
    ensures match logical::producer_acquire_item_v1(model, writer_key_view(consumer), producer_read_view(request), index, producer_scan_view(state)) {
        Err(error) => producer_acquire_item_v1(actual, consumer, request, index, state) == Err(read_error_embed(error)),
        Ok(next) => producer_acquire_item_v1(actual, consumer, request, index, state).is_ok()
            && producer_scan_view(producer_acquire_item_v1(actual, consumer, request, index, state).unwrap()) == next,
    },
{
    producer_status_correspondence(actual.stable.journal, model.stable.journal, request);
}

proof fn producer_acquire_scan_correspondence(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    consumer: WriterKeyV1, requests: Seq<ContextProducerReadV1>, index: nat, state: ProducerReadAcquireScanV1)
    requires producer_represents(actual, model),
    ensures producer_acquire_scan_v1(actual, consumer, requests, index, state)
        == begin_result_from(logical::producer_acquire_scan_v1(model, writer_key_view(consumer), producer_requests_view(requests), index, producer_scan_view(state))),
    decreases requests.len() - index,
{
    if index < requests.len() {
        producer_acquire_item_correspondence(actual, model, consumer, requests[index as int], index as usize, state);
        if let Ok(next) = producer_acquire_item_v1(actual, consumer, requests[index as int], index as usize, state) {
            producer_acquire_scan_correspondence(actual, model, consumer, requests, index + 1, next);
        }
    }
}

proof fn producer_acquire_decision_correspondence(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    consumer: WriterKeyV1, requests: Seq<ContextProducerReadV1>, output: Seq<Option<ContextProducerReadReferenceV1>>)
    requires producer_represents(actual, model),
    ensures producer_acquire_decision_v1(actual, consumer, requests, output)
        == begin_result_from(logical::producer_acquire_decision_v1(model, writer_key_view(consumer), producer_requests_view(requests), producer_output_view(output))),
{
    producer_acquire_header_correspondence(actual, model, consumer, requests.len() as usize, output);
    producer_acquire_scan_correspondence(actual, model, consumer, requests, 0, ProducerReadAcquireScanV1 { previous: None, group: 0 });
}

proof fn producer_slot_count_correspondence(requests: Seq<ContextProducerReadV1>, slot: usize)
    ensures producer_request_count_v1(requests, slot) == logical::producer_request_count_v1(producer_requests_view(requests), slot),
    decreases requests.len(),
{
    if requests.len() > 0 {
        assert(producer_requests_view(requests).drop_last() =~= producer_requests_view(requests.drop_last()));
        producer_slot_count_correspondence(requests.drop_last(), slot);
    }
}

proof fn producer_acquired_reference_correspondence(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    consumer: WriterKeyV1, index: int)
    requires producer_represents(actual, model),
    ensures producer_reference_view(producer_acquired_reference_v1(actual, consumer, index))
        == logical::producer_acquired_reference_v1(model, writer_key_view(consumer), index),
{}

proof fn producer_acquired_reservations_correspondence(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    consumer: WriterKeyV1, requests: Seq<ContextProducerReadV1>, count: nat)
    requires producer_represents(actual, model), producer_acquire_commit_ready_v1(actual, requests), count <= requests.len(),
    ensures producer_acquired_reservations_v1(actual, consumer, requests, count).map(|_i, entry| producer_reservation_slot_view(entry))
        == logical::producer_acquired_reservations_v1(model, writer_key_view(consumer), producer_requests_view(requests), count),
        producer_acquired_reservations_v1(actual, consumer, requests, count).len() == actual.reservations@.len(),
    decreases count,
{
    if count > 0 {
        producer_acquired_reservations_correspondence(actual, model, consumer, requests, (count - 1) as nat);
        producer_acquired_reference_correspondence(actual, model, consumer, count - 1);
        let reference = producer_acquired_reference_v1(actual, consumer, count - 1);
        assert(reference.slot < actual.reservations@.len());
        assert(producer_acquired_reservations_v1(actual, consumer, requests, count).map(|_i, entry| producer_reservation_slot_view(entry))
            =~= logical::producer_acquired_reservations_v1(model, writer_key_view(consumer), producer_requests_view(requests), count));
    }
}

proof fn producer_acquire_paired_transition(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    model_before: logical::ProducerReadContentsV1, model_after: logical::ProducerReadContentsV1, consumer: WriterKeyV1,
    requests: Seq<ContextProducerReadV1>, output_before: Seq<Option<ContextProducerReadReferenceV1>>,
    output_after: Seq<Option<ContextProducerReadReferenceV1>>, model_output_after: Seq<Option<logical::ProducerReadReferenceV1>>,
    result: Result<(), ReadErrorV1>, model_result: Result<(), logical::ReadErrorV1>)
    requires producer_represents(before, model_before),
        producer_acquire_domain_v1(before, consumer, requests.len() as usize, output_before),
        requests.len() <= usize::MAX, requests.len() <= u64::MAX,
        producer_acquire_execution_relation_v1(before, after, consumer, requests, output_before, output_after, result),
        logical::producer_acquire_execution_relation_v1(model_before, model_after, writer_key_view(consumer),
            producer_requests_view(requests), producer_output_view(output_before), model_output_after, model_result),
    ensures result == begin_result_from(model_result), producer_represents(after, model_after),
        producer_output_view(output_after) == model_output_after,
{
    producer_acquire_decision_correspondence(before, model_before, consumer, requests, output_before);
    if result == Ok(()) {
        producer_acquire_preflight_implies_commit_ready_v1(before, consumer, requests, output_before);
        producer_acquired_reservations_correspondence(before, model_before, consumer, requests, requests.len());
        assert(after.counts@ =~= model_after.counts@) by {
            assert forall|a: int| 0 <= a < after.counts@.len() implies after.counts@[a] == model_after.counts@[a] by {
                producer_slot_count_correspondence(requests, a as usize);
            }
        }
        assert(producer_output_view(output_after) =~= model_output_after) by {
            assert forall|i: int| 0 <= i < output_after.len() implies
                producer_output_view(output_after)[i] == model_output_after[i] by {
                producer_acquired_reference_correspondence(before, model_before, consumer, i);
            }
        }
    }
}

// Raw actual admission retains alias writes; this paired executor adds represented producer custody.
fn producer_acquire_historical_exec_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    consumer: WriterKeyV1, model_consumer: logical::WriterKeyV1,
    requests: &[ContextProducerReadV1], model_requests: &[logical::ProducerReadV1],
    output: &mut [Option<ContextProducerReadReferenceV1>], model_output: &mut Vec<Option<logical::ProducerReadReferenceV1>>)
    -> (results: (Result<(), ReadErrorV1>, Result<(), logical::ReadErrorV1>))
    requires producer_represents(*old(actual), *old(model)), logical::producer_invariant_v1(*old(model)),
        writer_key_view(consumer) == model_consumer, producer_requests_view(requests@) == model_requests@,
        producer_output_view(old(output)@) == old(model_output)@, requests@.len() <= u64::MAX,
    ensures results.0 == begin_result_from(results.1), producer_represents(*final(actual), *final(model)),
        producer_output_view(final(output)@) == final(model_output)@,
        logical::producer_invariant_v1(*final(model)),
        producer_acquire_execution_relation_v1(*old(actual), *final(actual), consumer, requests@, old(output)@, final(output)@, results.0),
        logical::producer_acquire_execution_relation_v1(*old(model), *final(model), model_consumer, model_requests@,
            old(model_output)@, final(model_output)@, results.1),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let ghost output_before = output@;
    let _count = requests.len();
    proof { logical::producer_capacity_arithmetic_v1(*model); }
    let result: Result<(), ReadErrorV1> = Err(ReadErrorV1::InvalidState);
    let model_result = logical::producer_acquire_exec_v1(model, model_consumer, model_requests, model_output);
    proof {
        producer_acquire_paired_transition(before, *actual, model_before, *model, consumer, requests@, output_before,
            output@, model_output@, result, model_result);
    }
    (result, model_result)
}

}
