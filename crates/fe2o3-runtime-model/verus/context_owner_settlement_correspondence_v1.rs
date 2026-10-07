verus! {

proof fn owner_settlement_scratch_correspondence_v1(before: JournalContentsV1, model: logical::JournalContentsV1,
    count: usize, index: nat)
    requires represents(before, model), count <= before.scratch@.len(),
    ensures settlement_scratch_scan_v1(before, count, index)
        == begin_result_from(logical::settlement_scratch_scan_v1(model, count, index)),
    decreases count - index,
{
    if index < count && before.scratch@[index as int].is_none() {
        owner_settlement_scratch_correspondence_v1(before, model, count, index + 1);
    }
}

proof fn owner_settlement_return_correspondence_v1(before: JournalContentsV1, model: logical::JournalContentsV1,
    count: usize, free_storage: usize, member_free_storage: usize)
    requires represents(before, model),
    ensures settlement_return_decision_v1(before, count, free_storage, member_free_storage)
        == begin_result_from(logical::settlement_return_decision_v1(model, count, free_storage, member_free_storage)),
{
    if count <= before.scratch@.len() { owner_settlement_scratch_correspondence_v1(before, model, count, 0); }
}

proof fn owner_settlement_preflight_correspondence_v1(before: JournalContentsV1, model: logical::JournalContentsV1,
    writer: WriterReferenceV1, evidence: WriterReferenceV1, free_storage: usize, member_free_storage: usize)
    requires represents(before, model),
    ensures settlement_preflight_decision_v1(before, writer, evidence, free_storage, member_free_storage)
        == begin_result_from(logical::settlement_preflight_decision_v1(model, writer_reference_view(writer),
            writer_reference_view(evidence), free_storage, member_free_storage)),
{
    writer_reference_round_trip(writer, writer_reference_view(writer));
    writer_reference_round_trip(evidence, writer_reference_view(evidence));
    owner_header_correspondence(before, model, writer, false);
    if let Ok((head, count, _)) = owner_retained_header_v1(before, writer, false) {
        owner_chain_correspondence(before, model, writer, head, count);
        owner_settlement_return_correspondence_v1(before, model, count, free_storage, member_free_storage);
    }
}

proof fn owner_settlement_plan_correspondence_v1(before: JournalContentsV1, model: logical::JournalContentsV1,
    head: Option<usize>, total: usize, index: nat)
    requires represents(before, model), settlement_storage_ready_v1(before, head, total), index <= total,
    ensures settlement_cursor_v1(before, head, index) == logical::settlement_cursor_v1(model, head, index),
        index < total ==> begin_plan_view(settlement_plan_at_v1(before, head, index))
            == logical::settlement_plan_at_v1(model, head, index),
        index < total ==> logical::settlement_plan_ready_v1(model, head, index),
    decreases index,
{
    if index > 0 { owner_settlement_plan_correspondence_v1(before, model, head, total, (index - 1) as nat); }
    if index < total { assert(settlement_plan_ready_v1(before, head, index)); }
}

proof fn owner_settlement_prefix_correspondence_v1(before: JournalContentsV1, model: logical::JournalContentsV1,
    head: Option<usize>, total: usize, count: nat, success: bool)
    requires represents(before, model), settlement_storage_ready_v1(before, head, total), count <= total,
    ensures settlement_members_prefix_v1(before, head, count).map(|_i, value| member_entry_slot_view(value))
            == logical::settlement_members_prefix_v1(model, head, count),
        settlement_allocations_prefix_v1(before, head, success, count).map(|_i, value| allocation_entry_slot_view(value))
            == logical::settlement_allocations_prefix_v1(model, head, success, count),
    decreases count,
{
    if count > 0 {
        owner_settlement_prefix_correspondence_v1(before, model, head, total, (count - 1) as nat, success);
        owner_settlement_plan_correspondence_v1(before, model, head, total, (count - 1) as nat);
        settlement_prefix_shapes_v1(before, head, total, (count - 1) as nat, success);
        assert(settlement_plan_ready_v1(before, head, (count - 1) as nat));
        assert(settlement_members_prefix_v1(before, head, count).map(|_i, value| member_entry_slot_view(value))
            =~= logical::settlement_members_prefix_v1(model, head, count));
        assert(settlement_allocations_prefix_v1(before, head, success, count).map(|_i, value| allocation_entry_slot_view(value))
            =~= logical::settlement_allocations_prefix_v1(model, head, success, count));
    }
}

proof fn owner_settlement_slots_correspondence_v1(before: JournalContentsV1, model: logical::JournalContentsV1,
    head: Option<usize>, count: usize)
    requires represents(before, model), settlement_storage_ready_v1(before, head, count),
    ensures settlement_slots_v1(before, head, count as nat) == logical::settlement_slots_v1(model, head, count as nat),
{
    assert forall|i: int| 0 <= i < count implies settlement_slots_v1(before, head, count as nat)[i]
        == logical::settlement_slots_v1(model, head, count as nat)[i] by {
        owner_settlement_plan_correspondence_v1(before, model, head, count, i as nat);
    }
    assert(settlement_slots_v1(before, head, count as nat) =~= logical::settlement_slots_v1(model, head, count as nat));
}

proof fn owner_settlement_paired_transition_v1(before: JournalContentsV1, after: JournalContentsV1,
    model_before: logical::JournalContentsV1, model_after: logical::JournalContentsV1,
    writer: WriterReferenceV1, evidence: WriterReferenceV1, free_storage: usize, member_free_storage: usize,
    success: bool, result: Result<(), ReadErrorV1>, model_result: Result<(), logical::ReadErrorV1>)
    requires represents(before, model_before),
        settlement_execution_relation_v1(before, after, writer, evidence, free_storage, member_free_storage, success, result),
        logical::settlement_execution_relation_v1(model_before, model_after, writer_reference_view(writer),
            writer_reference_view(evidence), free_storage, member_free_storage, success, model_result),
    ensures result == begin_result_from(model_result), represents(after, model_after),
{
    owner_settlement_preflight_correspondence_v1(before, model_before, writer, evidence, free_storage, member_free_storage);
    if let Ok((head, count)) = settlement_preflight_decision_v1(before, writer, evidence, free_storage, member_free_storage) {
        settlement_preflight_ready_v1(before, writer, evidence, free_storage, member_free_storage, head, count);
        owner_settlement_prefix_correspondence_v1(before, model_before, head, count, count as nat, success);
        owner_settlement_slots_correspondence_v1(before, model_before, head, count);
        assert(journal_view(after).writers =~= model_after.writers@);
    }
}

spec fn owner_settlement_status_transition_v1(before: logical::JournalContentsV1, after: logical::JournalContentsV1,
    writer: logical::WriterReferenceV1, success: bool) -> bool
{
    forall|request: logical::ProducerReadV1| logical::producer_status_v1(before, request).is_some() ==>
        #[trigger] logical::producer_status_v1(after, request) ==
            if logical::producer_status_v1(before, request) == Some(logical::ProducerStatusV1::Pending)
                && logical::same_producer_v1(request.producer, writer) {
                Some(if success { logical::ProducerStatusV1::Success } else { logical::ProducerStatusV1::NoEffect })
            } else { logical::producer_status_v1(before, request) }
}

proof fn owner_settlement_retained_reference_transition_v1(before: logical::ProducerReadContentsV1,
    after: logical::ProducerReadContentsV1, writer: logical::WriterReferenceV1, success: bool,
    reference: logical::ProducerReadReferenceV1)
    requires after.reservations@ == before.reservations@,
        owner_settlement_status_transition_v1(before.stable.journal, after.stable.journal, writer, success),
        logical::producer_lookup_decision_v1(before, reference).is_ok(),
    ensures logical::producer_lookup_decision_v1(after, reference) == logical::producer_lookup_decision_v1(before, reference),
        historical_producer_query_decision_v1(before, reference).is_ok(),
        historical_producer_query_decision_v1(after, reference).is_ok(),
        historical_producer_query_decision_v1(after, reference) ==
            if historical_producer_query_decision_v1(before, reference) == Ok(logical::ProducerStatusV1::Pending)
                && logical::same_producer_v1(before.reservations@[reference.slot as int].unwrap().request.producer, writer) {
                Ok(if success { logical::ProducerStatusV1::Success } else { logical::ProducerStatusV1::NoEffect })
            } else { historical_producer_query_decision_v1(before, reference) },
{
    let entry = before.reservations@[reference.slot as int].unwrap();
    let request = entry.request;
    assert(logical::producer_lookup_decision_v1(before, reference) == Ok(request));
    let prior = match logical::producer_status_decision_v1(before.stable.journal, request) {
        Ok(status) => status, Err(_) => logical::ProducerStatusV1::Pending,
    };
    let next = if prior == logical::ProducerStatusV1::Pending && logical::same_producer_v1(request.producer, writer) {
        if success { logical::ProducerStatusV1::Success } else { logical::ProducerStatusV1::NoEffect }
    } else { prior };
    logical::producer_status_projection_v1(before.stable.journal, request);
    logical::producer_status_projection_v1(after.stable.journal, request);
    assert(logical::producer_status_v1(after.stable.journal, request) == Some(next));
    assert(logical::producer_status_decision_v1(after.stable.journal, request) == Ok(next));
    assert(logical::producer_lookup_decision_v1(after, reference) == Ok(request));
}

proof fn owner_settlement_preservation_v1(before: logical::ProducerReadContentsV1, after: logical::ProducerReadContentsV1,
    writer: logical::WriterReferenceV1, evidence: logical::WriterReferenceV1, free_storage: usize, member_free_storage: usize,
    success: bool, result: Result<(), logical::ReadErrorV1>, storage: logical::StorageCapacitiesV1,
    history: Seq<logical::WriterReferenceV1>)
    requires owner_model_outer_frame_v1(before, after),
        logical::settlement_execution_relation_v1(before.stable.journal, after.stable.journal, writer, evidence,
            free_storage, member_free_storage, success, result),
    ensures logical::producer_invariant_v1(before) ==> logical::producer_invariant_v1(after),
        logical::issued_producer_v1(before, storage, history) ==> logical::issued_producer_v1(after, storage, history),
        logical::pending_custody_v1(before.stable.journal) && result.is_ok() ==>
            owner_settlement_status_transition_v1(before.stable.journal, after.stable.journal, writer, success),
{
    if logical::pending_custody_v1(before.stable.journal) && result.is_ok() {
        let (head, count) = match logical::settlement_preflight_decision_v1(before.stable.journal, writer, evidence,
            free_storage, member_free_storage) { Ok(value) => value, Err(_) => (None, 0usize) };
        logical::settlement_raw_refines_chain_v1(before.stable.journal, after.stable.journal, writer, evidence,
            free_storage, member_free_storage, head, count, success);
        let chain = choose|chain: Seq<usize>| #[trigger] logical::settle_chain_relation_v1(
            before.stable.journal, after.stable.journal, writer, chain, success);
        if logical::producer_invariant_v1(before) {
            logical::settle_preserves_producer_invariant_v1(before, after, writer, chain, success);
        }
        if logical::issued_producer_v1(before, storage, history) {
            logical::settle_preserves_issued_producer_v1(before, after, writer, chain, success, storage, history);
        }
        assert forall|request: logical::ProducerReadV1| logical::producer_status_v1(before.stable.journal, request).is_some() implies
            #[trigger] logical::producer_status_v1(after.stable.journal, request) ==
                if logical::producer_status_v1(before.stable.journal, request) == Some(logical::ProducerStatusV1::Pending)
                    && logical::same_producer_v1(request.producer, writer) {
                    Some(if success { logical::ProducerStatusV1::Success } else { logical::ProducerStatusV1::NoEffect })
                } else { logical::producer_status_v1(before.stable.journal, request) } by {
            logical::settled_producer_status_v1(before.stable.journal, after.stable.journal, writer, chain, success, request);
        }
    }
}

// Both executors run independently; custody is a conditional postcondition only.
fn owner_settlement_historical_exec_v1(actual: &mut ContextProducerReadJournalV1,
    model: &mut logical::ProducerReadContentsV1, writer: WriterReferenceV1, model_writer: logical::WriterReferenceV1,
    evidence: WriterReferenceV1, model_evidence: logical::WriterReferenceV1, free_storage: usize, member_free_storage: usize,
    success: bool, Ghost(storage): Ghost<logical::StorageCapacitiesV1>, Ghost(history): Ghost<Seq<logical::WriterReferenceV1>>)
    -> (results: (Result<(), ReadErrorV1>, Result<(), logical::ReadErrorV1>))
    requires producer_represents(*old(actual), *old(model)), writer_reference_view(writer) == model_writer,
        writer_reference_view(evidence) == model_evidence,
    ensures results.0 == begin_result_from(results.1), producer_represents(*final(actual), *final(model)),
        settlement_execution_relation_v1(old(actual).stable.journal, final(actual).stable.journal, writer, evidence,
            free_storage, member_free_storage, success, results.0),
        logical::settlement_execution_relation_v1(old(model).stable.journal, final(model).stable.journal, model_writer, model_evidence,
            free_storage, member_free_storage, success, results.1),
        owner_writer_producer_frame_v1(*old(actual), *final(actual)), owner_model_outer_frame_v1(*old(model), *final(model)),
        results.0.is_err() ==> *final(actual) == *old(actual), results.1.is_err() ==> *final(model) == *old(model),
        logical::producer_invariant_v1(*old(model)) ==> logical::producer_invariant_v1(*final(model)),
        logical::issued_producer_v1(*old(model), storage, history) ==> logical::issued_producer_v1(*final(model), storage, history),
        logical::pending_custody_v1(old(model).stable.journal) && results.1.is_ok() ==>
            owner_settlement_status_transition_v1(old(model).stable.journal, final(model).stable.journal, model_writer, success),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let result = if success {
        actual.settle_success_observed_v1(writer, &ContextWriterSuccessEvidenceV1 { writer: evidence }, free_storage, member_free_storage)
    } else {
        actual.settle_no_effect_observed_v1(writer, &ContextWriterNoEffectEvidenceV1 { writer: evidence }, free_storage, member_free_storage)
    };
    let model_result = logical::settlement_exec_v1(&mut model.stable.journal, model_writer, model_evidence,
        free_storage, member_free_storage, success);
    proof {
        owner_settlement_paired_transition_v1(before.stable.journal, actual.stable.journal, model_before.stable.journal,
            model.stable.journal, writer, evidence, free_storage, member_free_storage, success, result, model_result);
        owner_settlement_preservation_v1(model_before, *model, model_writer, model_evidence, free_storage, member_free_storage,
            success, model_result, storage, history);
    }
    (result, model_result)
}

}
