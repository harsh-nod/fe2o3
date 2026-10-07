verus! {

proof fn producer_stable_header_correspondence(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    consumer: WriterKeyV1, count: usize, output: Seq<Option<ContextReadLeaseReferenceV1>>)
    requires producer_represents(actual, model),
    ensures producer_stable_header_v1(actual, consumer, count, output)
        == begin_result_from(logical::stable_wrapper_acquire_header_v1(model, writer_key_view(consumer), count, stable_output_view(output))),
{
    stable_capacity_correspondence(actual.stable, model.stable, count);
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

proof fn producer_stable_acquire_paired_transition(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    model_before: logical::ProducerReadContentsV1, model_after: logical::ProducerReadContentsV1, consumer: WriterKeyV1,
    requests: Seq<ContextAllocationReadV1>, output_before: Seq<Option<ContextReadLeaseReferenceV1>>,
    output_after: Seq<Option<ContextReadLeaseReferenceV1>>, model_output_after: Seq<Option<logical::ReadReferenceV1>>,
    result: Result<(), ReadErrorV1>, model_result: Result<(), logical::ReadErrorV1>)
    requires producer_represents(before, model_before), producer_stable_acquire_domain_v1(before, consumer, requests.len() as usize, output_before),
        requests.len() <= usize::MAX, requests.len() <= u64::MAX,
        producer_stable_acquire_relation_v1(before, after, consumer, requests, output_before, output_after, result),
        logical::stable_wrapper_acquire_relation_v1(model_before, model_after, writer_key_view(consumer),
            stable_requests_view(requests), stable_output_view(output_before), model_output_after, model_result),
    ensures result == begin_result_from(model_result), producer_represents(after, model_after),
        stable_output_view(output_after) == model_output_after,
{
    producer_stable_header_correspondence(before, model_before, consumer, requests.len() as usize, output_before);
    if producer_stable_header_v1(before, consumer, requests.len() as usize, output_before).is_ok() {
        stable_acquire_paired_transition(before.stable, after.stable, model_before.stable, model_after.stable, consumer,
            requests, output_before, output_after, model_output_after, result, model_result);
    }
}

// Execute both outer wrappers independently, preserving each one's header and delegation.
fn producer_stable_acquire_historical_exec_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    consumer: WriterKeyV1, model_consumer: logical::WriterKeyV1,
    requests: &[ContextAllocationReadV1], model_requests: &[logical::AllocationReadV1],
    output: &mut [Option<ContextReadLeaseReferenceV1>], model_output: &mut Vec<Option<logical::ReadReferenceV1>>)
    -> (results: (Result<(), ReadErrorV1>, Result<(), logical::ReadErrorV1>))
    requires producer_represents(*old(actual), *old(model)), logical::producer_invariant_v1(*old(model)),
        writer_key_view(consumer) == model_consumer, stable_requests_view(requests@) == model_requests@,
        stable_output_view(old(output)@) == old(model_output)@, requests@.len() <= u64::MAX,
    ensures results.0 == begin_result_from(results.1), producer_represents(*final(actual), *final(model)),
        stable_output_view(final(output)@) == final(model_output)@, logical::producer_invariant_v1(*final(model)),
        producer_stable_acquire_relation_v1(*old(actual), *final(actual), consumer, requests@, old(output)@, final(output)@, results.0),
        logical::stable_wrapper_acquire_relation_v1(*old(model), *final(model), model_consumer, model_requests@,
            old(model_output)@, final(model_output)@, results.1),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let ghost output_before = output@;
    let _count = requests.len();
    proof { logical::producer_capacity_arithmetic_v1(*model); }
    let result = actual.acquire_reads(consumer, requests, output);
    let model_result = logical::stable_wrapper_acquire_exec_v1(model, model_consumer, model_requests, model_output);
    proof {
        producer_stable_acquire_paired_transition(before, *actual, model_before, *model, consumer, requests@,
            output_before, output@, model_output@, result, model_result);
    }
    (result, model_result)
}

fn producer_stable_release_historical_exec_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    consumer: WriterKeyV1, model_consumer: logical::WriterKeyV1,
    references: &[ContextReadLeaseReferenceV1], model_references: &[logical::ReadReferenceV1],
    evidence: &ContextReadQuiescenceEvidenceV1, model_evidence: logical::WriterKeyV1, observed_free_capacity: usize)
    -> (results: (Result<(), ReadErrorV1>, Result<(), logical::ReadErrorV1>))
    requires producer_represents(*old(actual), *old(model)), logical::producer_invariant_v1(*old(model)),
        writer_key_view(consumer) == model_consumer, stable_references_view(references@) == model_references@,
        writer_key_view(evidence.consumer) == model_evidence,
    ensures results.0 == begin_result_from(results.1), producer_represents(*final(actual), *final(model)),
        logical::producer_invariant_v1(*final(model)),
        producer_stable_release_relation_v1(*old(actual), *final(actual), consumer, references@,
            evidence.consumer, observed_free_capacity, results.0),
        logical::producer_outer_frame_v1(*old(model), *final(model)),
        logical::release_execution_relation_v1(old(model).stable, final(model).stable, model_consumer, model_references@,
            model_evidence, observed_free_capacity, results.1),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let _count = references.len();
    let result = actual.release_reads_observed_v1(consumer, references, evidence, observed_free_capacity);
    let model_result = logical::stable_wrapper_release_exec_v1(model, model_consumer, model_references, model_evidence, observed_free_capacity);
    proof { stable_release_paired_transition(before.stable, actual.stable, model_before.stable, model.stable,
        consumer, references@, evidence.consumer, observed_free_capacity, result, model_result); }
    (result, model_result)
}

}
