verus! {

proof fn mixed_acquire_storage_from_model_v1(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1)
    requires producer_represents(actual, model), logical::producer_invariant_v1(model),
    ensures mixed_acquire_storage_v1(actual),
{
    logical::producer_capacity_arithmetic_v1(model);
}

proof fn mixed_acquire_decision_correspondence_v1(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    consumer: WriterKeyV1, stable: Seq<ContextAllocationReadV1>, stable_output: Seq<Option<ContextReadLeaseReferenceV1>>,
    pending: Seq<ContextProducerReadV1>, producer_output: Seq<Option<ContextProducerReadReferenceV1>>)
    requires producer_represents(actual, model),
    ensures mixed_acquire_decision_v1(actual, consumer, stable, stable_output, pending, producer_output)
        == begin_result_from(logical::mixed_acquire_decision_v1(model, writer_key_view(consumer),
            stable_requests_view(stable), stable_output_view(stable_output),
            producer_requests_view(pending), producer_output_view(producer_output))),
{
    stable_acquire_decision_correspondence(actual.stable, model.stable, consumer, stable, stable_output);
    producer_acquire_decision_correspondence(actual, model, consumer, pending, producer_output);
}

proof fn mixed_acquire_paired_transition_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    model_before: logical::ProducerReadContentsV1, model_after: logical::ProducerReadContentsV1,
    consumer: WriterKeyV1, stable: Seq<ContextAllocationReadV1>, pending: Seq<ContextProducerReadV1>,
    stable_before: Seq<Option<ContextReadLeaseReferenceV1>>, stable_after: Seq<Option<ContextReadLeaseReferenceV1>>,
    producer_before: Seq<Option<ContextProducerReadReferenceV1>>, producer_after: Seq<Option<ContextProducerReadReferenceV1>>,
    model_stable_after: Seq<Option<logical::ReadReferenceV1>>, model_producer_after: Seq<Option<logical::ProducerReadReferenceV1>>,
    result: Result<(), ReadErrorV1>, model_result: Result<(), logical::ReadErrorV1>)
    requires producer_represents(before, model_before), logical::producer_invariant_v1(model_before),
        stable.len() <= usize::MAX, stable.len() <= u64::MAX, pending.len() <= usize::MAX, pending.len() <= u64::MAX,
        mixed_acquire_relation_v1(before, after, consumer, stable, pending,
            stable_before, stable_after, producer_before, producer_after, result),
        logical::mixed_acquire_relation_v1(model_before, model_after, writer_key_view(consumer),
            stable_requests_view(stable), producer_requests_view(pending),
            stable_output_view(stable_before), model_stable_after, producer_output_view(producer_before), model_producer_after, model_result),
    ensures result == begin_result_from(model_result), producer_represents(after, model_after),
        stable_output_view(stable_after) == model_stable_after, producer_output_view(producer_after) == model_producer_after,
        logical::producer_invariant_v1(model_after),
{
    mixed_acquire_storage_from_model_v1(before, model_before);
    mixed_acquire_decision_correspondence_v1(before, model_before, consumer, stable, stable_before, pending, producer_before);
    logical::mixed_acquire_preserves_v1(model_before, model_after, writer_key_view(consumer),
        stable_requests_view(stable), producer_requests_view(pending), stable_output_view(stable_before), model_stable_after,
        producer_output_view(producer_before), model_producer_after, model_result);
    if result.is_ok() {
        let middle = choose|middle: ContextProducerReadJournalV1|
            mixed_stable_commit_v1(before, middle, consumer, stable, stable_before, stable_after)
            && mixed_producer_commit_v1(middle, after, consumer, pending, producer_before, producer_after);
        let model_middle = choose|middle: logical::ProducerReadContentsV1|
            logical::mixed_stable_step_v1(model_before, middle, writer_key_view(consumer),
                stable_requests_view(stable), stable_output_view(stable_before), model_stable_after)
            && logical::mixed_pending_step_v1(middle, model_after, writer_key_view(consumer),
                producer_requests_view(pending), producer_output_view(producer_before), model_producer_after);
        if stable.len() > 0 {
            stable_acquire_decision_correspondence(before.stable, model_before.stable, consumer, stable, stable_before);
            stable_acquire_paired_transition(before.stable, middle.stable, model_before.stable, model_middle.stable,
                consumer, stable, stable_before, stable_after, model_stable_after, Ok(()), Ok(()));
        }
        assert(producer_represents(middle, model_middle));
        logical::mixed_stable_preserves_v1(model_before, model_middle, writer_key_view(consumer),
            stable_requests_view(stable), pending.len(), stable_output_view(stable_before), model_stable_after);
        mixed_acquire_storage_from_model_v1(middle, model_middle);
        if pending.len() > 0 {
            logical::mixed_pending_decision_frame_v1(model_before, model_middle, writer_key_view(consumer),
                producer_requests_view(pending), producer_output_view(producer_before));
            producer_acquire_decision_correspondence(middle, model_middle, consumer, pending, producer_before);
            producer_acquire_paired_transition(middle, after, model_middle, model_after, consumer, pending,
                producer_before, producer_after, model_producer_after, Ok(()), Ok(()));
        }
    }
}

fn mixed_acquire_paired_exec_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    consumer: WriterKeyV1, model_consumer: logical::WriterKeyV1,
    stable: &[ContextAllocationReadV1], model_stable: &[logical::AllocationReadV1],
    pending: &[ContextProducerReadV1], model_pending: &[logical::ProducerReadV1],
    stable_output: &mut [Option<ContextReadLeaseReferenceV1>], model_stable_output: &mut Vec<Option<logical::ReadReferenceV1>>,
    producer_output: &mut [Option<ContextProducerReadReferenceV1>], model_producer_output: &mut Vec<Option<logical::ProducerReadReferenceV1>>,
) -> (results: (Result<(), ReadErrorV1>, Result<(), logical::ReadErrorV1>))
    requires producer_represents(*old(actual), *old(model)), logical::producer_invariant_v1(*old(model)),
        writer_key_view(consumer) == model_consumer, stable_requests_view(stable@) == model_stable@,
        producer_requests_view(pending@) == model_pending@,
        stable_output_view(old(stable_output)@) == old(model_stable_output)@,
        producer_output_view(old(producer_output)@) == old(model_producer_output)@,
        stable@.len() <= u64::MAX, pending@.len() <= u64::MAX,
    ensures results.0 == begin_result_from(results.1), producer_represents(*final(actual), *final(model)),
        stable_output_view(final(stable_output)@) == final(model_stable_output)@,
        producer_output_view(final(producer_output)@) == final(model_producer_output)@,
        logical::producer_invariant_v1(*final(model)),
        mixed_acquire_relation_v1(*old(actual), *final(actual), consumer, stable@, pending@,
            old(stable_output)@, final(stable_output)@, old(producer_output)@, final(producer_output)@, results.0),
        logical::mixed_acquire_relation_v1(*old(model), *final(model), model_consumer, model_stable@, model_pending@,
            old(model_stable_output)@, final(model_stable_output)@, old(model_producer_output)@, final(model_producer_output)@, results.1),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let ghost stable_before = stable_output@;
    let ghost producer_before = producer_output@;
    let _stable_count = stable.len();
    let _pending_count = pending.len();
    proof { mixed_acquire_storage_from_model_v1(*actual, *model); }
    let result = actual.acquire_mixed_reads(consumer, stable, stable_output, pending, producer_output);
    let model_result = logical::mixed_acquire_exec_v1(model, model_consumer, model_stable, model_stable_output,
        model_pending, model_producer_output);
    proof {
        mixed_acquire_paired_transition_v1(before, *actual, model_before, *model, consumer, stable@, pending@,
            stable_before, stable_output@, producer_before, producer_output@,
            model_stable_output@, model_producer_output@, result, model_result);
    }
    (result, model_result)
}

}
