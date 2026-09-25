verus! {

spec fn owner_register_result_from_v1(result: Result<logical::WriterReferenceV1, logical::JournalErrorV1>)
    -> Result<WriterReferenceV1, ReadErrorV1>
{
    match result { Ok(reference) => Ok(writer_reference_from(reference)), Err(error) => Err(journal_error_embed(error)) }
}

spec fn owner_abort_result_from_v1(result: Result<(), logical::JournalErrorV1>) -> Result<(), ReadErrorV1> {
    match result { Ok(()) => Ok(()), Err(error) => Err(journal_error_embed(error)) }
}

spec fn owner_model_register_relation_v1(before: logical::JournalContentsV1, after: logical::JournalContentsV1,
    key: logical::WriterKeyV1, result: Result<logical::WriterReferenceV1, logical::JournalErrorV1>) -> bool
{
    &&& logical::issuance_contents_frame_v1(before, after)
    &&& logical::register_execution_relation_v1(before.context_generation, before.writer_capacity, key,
        before.writers@, before.free@, before.registration_watermark, before.reserved_count,
        after.writers@, after.free@, after.registration_watermark, after.reserved_count, result)
}

spec fn owner_model_abort_relation_v1(before: logical::JournalContentsV1, after: logical::JournalContentsV1,
    reference: logical::WriterReferenceV1, capacity: usize, result: Result<(), logical::JournalErrorV1>) -> bool
{
    &&& logical::issuance_contents_frame_v1(before, after)
    &&& logical::abort_execution_relation_v1(before.context_generation, before.writer_capacity, capacity, reference,
        before.writers@, before.free@, before.registration_watermark, before.reserved_count,
        after.writers@, after.free@, after.registration_watermark, after.reserved_count, result)
}

spec fn owner_model_outer_frame_v1(before: logical::ProducerReadContentsV1, after: logical::ProducerReadContentsV1) -> bool {
    &&& after.reservations == before.reservations
    &&& after.free == before.free
    &&& after.counts == before.counts
    &&& after.next_incarnation == before.next_incarnation
    &&& after.stable.leases == before.stable.leases
    &&& after.stable.free_reads == before.stable.free_reads
    &&& after.stable.readers == before.stable.readers
    &&& after.stable.next_incarnation == before.stable.next_incarnation
}

proof fn owner_register_paired_transition_v1(before: JournalContentsV1, after: JournalContentsV1,
    model_before: logical::JournalContentsV1, model_after: logical::JournalContentsV1,
    key: WriterKeyV1, result: Result<WriterReferenceV1, ReadErrorV1>,
    model_result: Result<logical::WriterReferenceV1, logical::JournalErrorV1>)
    requires represents(before, model_before), owner_register_relation_v1(before, after, key, result),
        owner_model_register_relation_v1(model_before, model_after, writer_key_view(key), model_result),
    ensures result == owner_register_result_from_v1(model_result), represents(after, model_after),
        logical::retaining_writers_frame_v1(model_before, model_after),
{
    writer_key_round_trip(key, writer_key_view(key));
    if let Ok(reference) = result {
        writer_reference_round_trip(reference, writer_reference_view(reference));
        assert(journal_view(after).writers =~= model_after.writers@);
    }
    assert(logical::retaining_writers_frame_v1(model_before, model_after));
}

proof fn owner_abort_paired_transition_v1(before: JournalContentsV1, after: JournalContentsV1,
    model_before: logical::JournalContentsV1, model_after: logical::JournalContentsV1,
    reference: WriterReferenceV1, capacity: usize, result: Result<(), ReadErrorV1>,
    model_result: Result<(), logical::JournalErrorV1>)
    requires represents(before, model_before), owner_abort_relation_v1(before, after, reference, capacity, result),
        owner_model_abort_relation_v1(model_before, model_after, writer_reference_view(reference), capacity, model_result),
    ensures result == owner_abort_result_from_v1(model_result), represents(after, model_after),
        logical::retaining_writers_frame_v1(model_before, model_after),
{
    vstd::std_specs::vec::axiom_spec_len(&model_before.free);
    begin_reserved_correspondence(before, model_before, reference);
    if result.is_ok() { assert(journal_view(after).writers =~= model_after.writers@); }
    assert(logical::retaining_writers_frame_v1(model_before, model_after));
}

fn owner_reserved_lookup_historical_exec_v1(actual: &JournalContentsV1, model: &logical::JournalContentsV1,
    reference: WriterReferenceV1, model_reference: logical::WriterReferenceV1)
    -> (results: (Result<WriterKeyV1, ReadErrorV1>, Result<logical::WriterKeyV1, logical::JournalErrorV1>))
    requires represents(*actual, *model), writer_reference_view(reference) == model_reference,
    ensures results.0 == begin_reserved_result_from(results.1),
        results.0 == begin_reserved_decision_v1(*actual, reference),
        results.1 == logical::lookup_decision_v1(model.context_generation, model.writers@, model_reference),
{
    let result = actual.lookup_reserved(reference);
    let model_result = logical::lookup_reserved_v1(model.context_generation, model.writers.as_slice(), model_reference);
    proof { begin_reserved_correspondence(*actual, *model, reference); }
    (result, model_result)
}

fn owner_register_historical_exec_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    key: WriterKeyV1, model_key: logical::WriterKeyV1,
    Ghost(storage): Ghost<logical::StorageCapacitiesV1>, Ghost(history): Ghost<Seq<logical::WriterReferenceV1>>)
    -> (results: (Result<WriterReferenceV1, ReadErrorV1>, Result<logical::WriterReferenceV1, logical::JournalErrorV1>))
    requires producer_represents(*old(actual), *old(model)), writer_key_view(key) == model_key,
    ensures results.0 == owner_register_result_from_v1(results.1), producer_represents(*final(actual), *final(model)),
        owner_register_relation_v1(old(actual).stable.journal, final(actual).stable.journal, key, results.0),
        owner_writer_producer_frame_v1(*old(actual), *final(actual)),
        owner_model_register_relation_v1(old(model).stable.journal, final(model).stable.journal, model_key, results.1),
        owner_model_outer_frame_v1(*old(model), *final(model)),
        logical::producer_storage_frame_v1(*old(model), *final(model)),
        results.0.is_err() ==> *final(actual) == *old(actual),
        logical::issued_producer_v1(*old(model), storage, history) ==>
            logical::issued_producer_v1(*final(model), storage, logical::registration_history_v1(history, results.1)),
        forall|request: logical::ProducerReadV1| #[trigger] logical::producer_status_v1(final(model).stable.journal, request)
            == logical::producer_status_v1(old(model).stable.journal, request),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let result = actual.register_writer(key);
    let model_result = logical::register_contents_exec_v1(&mut model.stable.journal, model_key);
    proof {
        owner_register_paired_transition_v1(before.stable.journal, actual.stable.journal,
            model_before.stable.journal, model.stable.journal, key, result, model_result);
        if logical::issued_producer_v1(model_before, storage, history) {
            logical::register_issued_custody_v1(model_before.stable.journal, model.stable.journal, model_key,
                model_result, storage, history);
            logical::producer_issuance_frame_v1(model_before, *model, storage, logical::registration_history_v1(history, model_result));
        }
        assert forall|request: logical::ProducerReadV1| #[trigger] logical::producer_status_v1(model.stable.journal, request)
            == logical::producer_status_v1(model_before.stable.journal, request) by {
            logical::producer_status_issuance_frame_v1(model_before.stable.journal, model.stable.journal, request);
        }
    }
    (result, model_result)
}

fn owner_abort_historical_exec_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    reference: WriterReferenceV1, model_reference: logical::WriterReferenceV1, observed_free_capacity: usize,
    Ghost(storage): Ghost<logical::StorageCapacitiesV1>, Ghost(history): Ghost<Seq<logical::WriterReferenceV1>>)
    -> (results: (Result<(), ReadErrorV1>, Result<(), logical::JournalErrorV1>))
    requires producer_represents(*old(actual), *old(model)), writer_reference_view(reference) == model_reference,
    ensures results.0 == owner_abort_result_from_v1(results.1), producer_represents(*final(actual), *final(model)),
        owner_abort_relation_v1(old(actual).stable.journal, final(actual).stable.journal, reference, observed_free_capacity, results.0),
        owner_writer_producer_frame_v1(*old(actual), *final(actual)),
        owner_model_abort_relation_v1(old(model).stable.journal, final(model).stable.journal, model_reference, observed_free_capacity, results.1),
        owner_model_outer_frame_v1(*old(model), *final(model)),
        logical::producer_storage_frame_v1(*old(model), *final(model)),
        results.0.is_err() ==> *final(actual) == *old(actual),
        logical::issued_producer_v1(*old(model), storage, history) && observed_free_capacity == storage.free ==>
            logical::issued_producer_v1(*final(model), storage, history),
        forall|request: logical::ProducerReadV1| #[trigger] logical::producer_status_v1(final(model).stable.journal, request)
            == logical::producer_status_v1(old(model).stable.journal, request),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let result = actual.abort_reserved_observed_v1(reference, observed_free_capacity);
    let model_result = logical::abort_contents_exec_v1(&mut model.stable.journal, observed_free_capacity, model_reference);
    proof {
        owner_abort_paired_transition_v1(before.stable.journal, actual.stable.journal,
            model_before.stable.journal, model.stable.journal, reference, observed_free_capacity, result, model_result);
        if logical::issued_producer_v1(model_before, storage, history) && observed_free_capacity == storage.free {
            logical::abort_issued_custody_v1(model_before.stable.journal, model.stable.journal, model_reference,
                model_result, storage, history);
            logical::producer_issuance_frame_v1(model_before, *model, storage, history);
        }
        assert forall|request: logical::ProducerReadV1| #[trigger] logical::producer_status_v1(model.stable.journal, request)
            == logical::producer_status_v1(model_before.stable.journal, request) by {
            logical::producer_status_issuance_frame_v1(model_before.stable.journal, model.stable.journal, request);
        }
    }
    (result, model_result)
}

}
