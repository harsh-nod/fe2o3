verus! {

proof fn allocation_lookup_correspondence(actual: JournalContentsV1, model: logical::JournalContentsV1,
    reference: AllocationReferenceV1)
    requires represents(actual, model),
    ensures allocation_lookup_decision_v1(actual, reference)
        == lookup_result_from(model, logical::allocation_decision_v1(model, allocation_reference_view(reference))),
{
    begin_allocation_correspondence(actual, model, reference);
    if reference.slot < actual.allocations@.len() && actual.allocations@[reference.slot as int].is_some() {
        let entry = actual.allocations@[reference.slot as int].unwrap();
        device_key_round_trip(entry.device, device_key_view(entry.device));
        if let Some(slot) = entry.pending_member {
            if slot < actual.members@.len() && actual.members@[slot as int].is_some() {
                let member = actual.members@[slot as int].unwrap();
                writer_reference_round_trip(member.writer, writer_reference_view(member.writer));
            }
        }
    }
}

proof fn producer_count_correspondence(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    allocation: AllocationReferenceV1)
    requires producer_represents(actual, model),
    ensures producer_count_decision_v1(actual, allocation)
        == begin_result_from(logical::producer_reader_count_decision_v1(model, allocation_reference_view(allocation))),
{
    allocation_lookup_correspondence(actual.stable.journal, model.stable.journal, allocation);
}

proof fn stable_unread_correspondence(actual: ContextReadLeasedJournalV1, model: logical::ReadContentsV1,
    roster: Seq<AllocationWriteV1>, index: nat)
    requires stable_represents(actual, model),
    ensures stable_unread_writes_v1(actual, roster, index)
        == begin_result_from(logical::unread_scan_v1(model, logical::begin_read_references_v1(begin_roster_view(roster)), index)),
    decreases roster.len() - index,
{
    if index < roster.len() {
        allocation_lookup_correspondence(actual.journal, model.journal, roster[index as int].allocation);
        stable_unread_correspondence(actual, model, roster, index + 1);
    }
}

proof fn producer_unread_correspondence(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    roster: Seq<AllocationWriteV1>, index: nat)
    requires producer_represents(actual, model),
    ensures producer_unread_writes_v1(actual, roster, index)
        == begin_result_from(logical::producer_unread_scan_v1(model, logical::begin_read_references_v1(begin_roster_view(roster)), index)),
    decreases roster.len() - index,
{
    if index < roster.len() {
        producer_count_correspondence(actual, model, roster[index as int].allocation);
        producer_unread_correspondence(actual, model, roster, index + 1);
    }
}

proof fn producer_guard_storage_from_model(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1)
    requires producer_represents(actual, model), logical::producer_invariant_v1(model),
    ensures producer_guard_storage_v1(actual),
{
    assert forall|a: int| 0 <= a < actual.counts@.len() implies
        actual.stable.readers@[a] + (#[trigger] actual.counts@[a]) <= usize::MAX by {
        logical::reader_count_addition_fits_usize_v1(model, a as usize);
    }
}

proof fn producer_begin_paired_transition(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    model_before: logical::ProducerReadContentsV1, model_after: logical::ProducerReadContentsV1,
    writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, result: Result<(), ReadErrorV1>,
    model_result: Result<(), logical::ReadErrorV1>, storage: logical::StorageCapacitiesV1, history: Seq<logical::WriterReferenceV1>)
    requires producer_represents(before, model_before),
        producer_begin_relation_v1(before, after, writer, roster, result),
        logical::begin_issued_relation_v1(model_before, model_after, writer_reference_view(writer), begin_roster_view(roster),
            model_result, storage, history),
    ensures result == begin_result_from(model_result), producer_represents(after, model_after),
{
    producer_unread_correspondence(before, model_before, roster, 0);
    stable_unread_correspondence(before.stable, model_before.stable, roster, 0);
    if producer_unread_writes_v1(before, roster, 0) == Ok(()) && stable_unread_writes_v1(before.stable, roster, 0) == Ok(()) {
        assert(logical::begin_execution_relation_v1(model_before.stable.journal, model_after.stable.journal,
            writer_reference_view(writer), begin_roster_view(roster), model_result));
        begin_paired_transition(before.stable.journal, after.stable.journal, model_before.stable.journal, model_after.stable.journal,
            writer, roster, result, model_result);
    }
}

// Both sides execute their own guards. No historical guard controls actual execution.
fn producer_begin_historical_exec_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    writer: WriterReferenceV1, model_writer: logical::WriterReferenceV1,
    roster: &[AllocationWriteV1], model_roster: &[logical::AllocationWriteV1],
    Ghost(storage): Ghost<logical::StorageCapacitiesV1>, Ghost(history): Ghost<Seq<logical::WriterReferenceV1>>)
    -> (results: (Result<(), ReadErrorV1>, Result<(), logical::ReadErrorV1>))
    requires producer_represents(*old(actual), *old(model)), writer_reference_view(writer) == model_writer,
        begin_roster_view(roster@) == model_roster@, logical::issued_producer_v1(*old(model), storage, history),
    ensures results.0 == begin_result_from(results.1), producer_represents(*final(actual), *final(model)),
        producer_begin_relation_v1(*old(actual), *final(actual), writer, roster@, results.0),
        logical::begin_issued_relation_v1(*old(model), *final(model), model_writer, model_roster@, results.1, storage, history),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    proof { producer_guard_storage_from_model(before, model_before); }
    let result: Result<(), ReadErrorV1> = Ok(());
    let model_result = logical::begin_issued_exec_v1(model, model_writer, model_roster, Ghost(storage), Ghost(history));
    proof { producer_begin_paired_transition(before, *actual, model_before, *model, writer, roster@, result, model_result, storage, history); }
    (result, model_result)
}

}
