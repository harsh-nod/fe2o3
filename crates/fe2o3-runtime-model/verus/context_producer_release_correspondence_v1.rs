verus! {

spec fn producer_references_view(references: Seq<ContextProducerReadReferenceV1>) -> Seq<logical::ProducerReadReferenceV1> {
    references.map(|_i, reference| producer_reference_view(reference))
}

spec fn producer_release_scan_view(state: ProducerReadReleaseScanV1) -> logical::ReadScanV1 {
    logical::ReadScanV1 { previous: state.previous, group: state.group }
}

proof fn producer_release_header_correspondence(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    consumer: WriterKeyV1, evidence: WriterKeyV1, count: usize, observed_free_capacity: usize)
    requires producer_represents(actual, model),
    ensures producer_release_header_v1(actual, consumer, evidence, count, observed_free_capacity)
        == begin_result_from(logical::producer_release_header_v1(model, writer_key_view(consumer), writer_key_view(evidence),
            count, observed_free_capacity)),
{}

proof fn producer_release_item_correspondence(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    consumer: WriterKeyV1, reference: ContextProducerReadReferenceV1, state: ProducerReadReleaseScanV1)
    requires producer_represents(actual, model),
    ensures match logical::producer_release_item_v1(model, writer_key_view(consumer), producer_reference_view(reference), producer_release_scan_view(state)) {
        Err(error) => producer_release_item_v1(actual, consumer, reference, state) == Err(read_error_embed(error)),
        Ok(next) => producer_release_item_v1(actual, consumer, reference, state).is_ok()
            && producer_release_scan_view(producer_release_item_v1(actual, consumer, reference, state).unwrap()) == next,
    },
{
    producer_lookup_correspondence(actual, model, reference);
}

proof fn producer_release_scan_correspondence(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    consumer: WriterKeyV1, references: Seq<ContextProducerReadReferenceV1>, index: nat, state: ProducerReadReleaseScanV1)
    requires producer_represents(actual, model),
    ensures producer_release_scan_v1(actual, consumer, references, index, state)
        == begin_result_from(logical::producer_release_scan_v1(model, writer_key_view(consumer), producer_references_view(references),
            index, producer_release_scan_view(state))),
    decreases references.len() - index,
{
    if index < references.len() {
        producer_release_item_correspondence(actual, model, consumer, references[index as int], state);
        if let Ok(next) = producer_release_item_v1(actual, consumer, references[index as int], state) {
            producer_release_scan_correspondence(actual, model, consumer, references, index + 1, next);
        }
    }
}

proof fn producer_release_decision_correspondence(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    consumer: WriterKeyV1, references: Seq<ContextProducerReadReferenceV1>, evidence: WriterKeyV1, observed_free_capacity: usize)
    requires producer_represents(actual, model),
    ensures producer_release_decision_v1(actual, consumer, references, evidence, observed_free_capacity)
        == begin_result_from(logical::producer_release_decision_v1(model, writer_key_view(consumer), producer_references_view(references),
            writer_key_view(evidence), observed_free_capacity)),
{
    producer_release_header_correspondence(actual, model, consumer, evidence, references.len() as usize, observed_free_capacity);
    producer_release_scan_correspondence(actual, model, consumer, references, 0, ProducerReadReleaseScanV1 { previous: None, group: 0 });
}

proof fn producer_released_requests_correspondence(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    references: Seq<ContextProducerReadReferenceV1>)
    requires producer_represents(actual, model), producer_release_commit_ready_v1(actual, references),
    ensures producer_requests_view(producer_released_requests_v1(actual, references))
        == logical::producer_released_requests_v1(model, producer_references_view(references)),
{
    assert(producer_requests_view(producer_released_requests_v1(actual, references))
        =~= logical::producer_released_requests_v1(model, producer_references_view(references)));
}

proof fn producer_released_reservations_correspondence(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    references: Seq<ContextProducerReadReferenceV1>, count: nat)
    requires producer_represents(actual, model), producer_release_commit_ready_v1(actual, references), count <= references.len(),
    ensures producer_released_reservations_v1(actual, references, count).map(|_i, entry| producer_reservation_slot_view(entry))
        == logical::producer_released_reservations_v1(model, producer_references_view(references), count),
        producer_released_reservations_v1(actual, references, count).len() == actual.reservations@.len(),
    decreases count,
{
    if count > 0 {
        producer_released_reservations_correspondence(actual, model, references, (count - 1) as nat);
        assert(producer_released_reservations_v1(actual, references, count).map(|_i, entry| producer_reservation_slot_view(entry))
            =~= logical::producer_released_reservations_v1(model, producer_references_view(references), count));
    }
}

proof fn producer_release_paired_transition(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    model_before: logical::ProducerReadContentsV1, model_after: logical::ProducerReadContentsV1, consumer: WriterKeyV1,
    references: Seq<ContextProducerReadReferenceV1>, evidence: WriterKeyV1, observed_free_capacity: usize,
    result: Result<(), ReadErrorV1>, model_result: Result<(), logical::ReadErrorV1>)
    requires producer_represents(before, model_before), producer_release_domain_v1(before, consumer, evidence, references.len() as usize, observed_free_capacity), references.len() <= usize::MAX,
        producer_release_execution_relation_v1(before, after, consumer, references, evidence, observed_free_capacity, result),
        logical::producer_release_execution_relation_v1(model_before, model_after, writer_key_view(consumer),
            producer_references_view(references), writer_key_view(evidence), observed_free_capacity, model_result),
    ensures result == begin_result_from(model_result), producer_represents(after, model_after),
{
    producer_release_decision_correspondence(before, model_before, consumer, references, evidence, observed_free_capacity);
    if result == Ok(()) {
        producer_release_preflight_implies_commit_ready_v1(before, consumer, references, evidence, observed_free_capacity);
        producer_released_requests_correspondence(before, model_before, references);
        producer_released_reservations_correspondence(before, model_before, references, references.len());
        assert(Seq::new(references.len(), |i: int| references[i].slot)
            =~= Seq::new(producer_references_view(references).len(), |i: int| producer_references_view(references)[i].slot));
        assert(after.counts@ =~= model_after.counts@) by {
            assert forall|a: int| 0 <= a < after.counts@.len() implies after.counts@[a] == model_after.counts@[a] by {
                assert(producer_released_requests_v1(before, references).take(references.len() as int)
                    =~= producer_released_requests_v1(before, references));
                assert(logical::producer_released_requests_v1(model_before, producer_references_view(references)).take(references.len() as int)
                    =~= logical::producer_released_requests_v1(model_before, producer_references_view(references)));
                producer_slot_count_correspondence(producer_released_requests_v1(before, references), a as usize);
            }
        }
    }
}

// Both executions consume the same explicit observation; neither controls the other's admission.
fn producer_release_historical_exec_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    consumer: WriterKeyV1, model_consumer: logical::WriterKeyV1,
    references: &[ContextProducerReadReferenceV1], model_references: &[logical::ProducerReadReferenceV1],
    evidence: &ContextReadQuiescenceEvidenceV1, model_evidence: logical::WriterKeyV1, observed_free_capacity: usize)
    -> (results: (Result<(), ReadErrorV1>, Result<(), logical::ReadErrorV1>))
    requires producer_represents(*old(actual), *old(model)), logical::producer_invariant_v1(*old(model)),
        writer_key_view(consumer) == model_consumer, producer_references_view(references@) == model_references@,
        writer_key_view(evidence.consumer) == model_evidence,
    ensures results.0 == begin_result_from(results.1), producer_represents(*final(actual), *final(model)),
        logical::producer_invariant_v1(*final(model)),
        producer_release_execution_relation_v1(*old(actual), *final(actual), consumer, references@,
            evidence.consumer, observed_free_capacity, results.0),
        logical::producer_release_execution_relation_v1(*old(model), *final(model), model_consumer, model_references@,
            model_evidence, observed_free_capacity, results.1),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let _count = references.len();
    let result = actual.release_producer_reads_observed_v1(consumer, references, evidence, observed_free_capacity);
    let model_result = logical::producer_release_exec_v1(model, model_consumer, model_references, model_evidence, observed_free_capacity);
    proof { producer_release_paired_transition(before, *actual, model_before, *model, consumer, references@,
        evidence.consumer, observed_free_capacity, result, model_result); }
    (result, model_result)
}

}
