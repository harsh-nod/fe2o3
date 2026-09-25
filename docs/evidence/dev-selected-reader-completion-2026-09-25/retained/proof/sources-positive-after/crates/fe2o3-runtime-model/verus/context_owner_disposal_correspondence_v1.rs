verus! {

proof fn disposal_roster_correspondence_v1(actual: JournalContentsV1, model: logical::JournalContentsV1,
    roster: Seq<AllocationWriteV1>, cursor: Option<usize>, index: nat)
    requires represents(actual, model),
    ensures disposal_roster_scan_v1(actual, roster, cursor, index)
        == begin_result_from(logical::disposal_roster_scan_v1(model, begin_roster_view(roster), cursor, index)),
    decreases roster.len() - index,
{
    if index < roster.len() {
        if let Some(slot) = cursor {
            if slot < actual.members@.len() {
                if let Some(member) = actual.members@[slot as int] {
                    begin_allocation_correspondence(actual, model, member.allocation);
                    allocation_reference_round_trip(member.allocation, allocation_reference_view(member.allocation));
                    allocation_reference_round_trip(roster[index as int].allocation,
                        allocation_reference_view(roster[index as int].allocation));
                    device_key_round_trip(roster[index as int].device, device_key_view(roster[index as int].device));
                    if let Ok(allocation) = begin_exact_allocation_v1(actual, member.allocation) {
                        allocation_entry_round_trip(allocation, allocation_entry_view(allocation));
                        device_key_round_trip(allocation.device, device_key_view(allocation.device));
                    }
                    disposal_roster_correspondence_v1(actual, model, roster, member.next, index + 1);
                }
            }
        }
    }
}

proof fn disposal_return_correspondence_v1(actual: JournalContentsV1, model: logical::JournalContentsV1,
    count: usize, writer_storage: usize, member_storage: usize, allocation_storage: usize)
    requires represents(actual, model),
    ensures disposal_return_decision_v1(actual, count, writer_storage, member_storage, allocation_storage)
        == begin_result_from(logical::disposal_return_decision_v1(model, count, writer_storage, member_storage, allocation_storage)),
{
    if count <= actual.scratch@.len() { owner_settlement_scratch_correspondence_v1(actual, model, count, 0); }
}

proof fn disposal_plan_correspondence_v1(actual: JournalContentsV1, model: logical::JournalContentsV1,
    writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
    writer_storage: usize, member_storage: usize, allocation_storage: usize)
    requires represents(actual, model),
    ensures disposal_plan_decision_v1(actual, writer, roster, writer_storage, member_storage, allocation_storage)
        == begin_result_from(logical::disposal_plan_decision_v1(model, writer_reference_view(writer), begin_roster_view(roster),
            writer_storage, member_storage, allocation_storage)),
        disposal_validate_decision_v1(actual, writer, roster, writer_storage, member_storage, allocation_storage)
        == begin_result_from(logical::disposal_validate_decision_v1(model, writer_reference_view(writer), begin_roster_view(roster),
            writer_storage, member_storage, allocation_storage)),
{
    owner_header_correspondence(actual, model, writer, true);
    if let Ok((head, count, _)) = owner_retained_header_v1(actual, writer, true) {
        owner_chain_correspondence(actual, model, writer, head, count);
        disposal_roster_correspondence_v1(actual, model, roster, head, 0);
        disposal_return_correspondence_v1(actual, model, count, writer_storage, member_storage, allocation_storage);
    }
}

proof fn disposal_execute_correspondence_v1(actual: JournalContentsV1, model: logical::JournalContentsV1,
    writer: WriterReferenceV1, evidence: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
    writer_storage: usize, member_storage: usize, allocation_storage: usize)
    requires represents(actual, model),
    ensures disposal_execute_decision_v1(actual, writer, evidence, roster, writer_storage, member_storage, allocation_storage)
        == begin_result_from(logical::disposal_execute_decision_v1(model, writer_reference_view(writer), writer_reference_view(evidence),
            begin_roster_view(roster), writer_storage, member_storage, allocation_storage)),
{
    owner_header_correspondence(actual, model, writer, true);
    writer_reference_round_trip(writer, writer_reference_view(writer));
    writer_reference_round_trip(evidence, writer_reference_view(evidence));
    disposal_plan_correspondence_v1(actual, model, writer, roster, writer_storage, member_storage, allocation_storage);
}

proof fn disposal_stable_correspondence_v1(actual: ContextReadLeasedJournalV1, model: logical::ReadContentsV1,
    roster: Seq<AllocationWriteV1>, index: nat)
    requires stable_represents(actual, model),
    ensures stable_unread_writes_v1(actual, roster, index)
        == begin_result_from(logical::disposal_stable_unread_v1(model, begin_roster_view(roster), index)),
        disposal_stable_safe_v1(actual, roster, index)
        == logical::disposal_stable_safe_v1(model, begin_roster_view(roster), index),
    decreases roster.len() - index,
{
    if index < roster.len() {
        allocation_lookup_correspondence(actual.journal, model.journal, roster[index as int].allocation);
        disposal_stable_correspondence_v1(actual, model, roster, index + 1);
    }
}

proof fn disposal_producer_correspondence_v1(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    roster: Seq<AllocationWriteV1>, index: nat)
    requires producer_represents(actual, model),
    ensures producer_unread_writes_v1(actual, roster, index)
        == begin_result_from(logical::disposal_producer_unread_v1(model, begin_roster_view(roster), index)),
        disposal_producer_safe_v1(actual, roster, index)
        == logical::disposal_producer_safe_v1(model, begin_roster_view(roster), index),
    decreases roster.len() - index,
{
    if index < roster.len() {
        allocation_lookup_correspondence(actual.stable.journal, model.stable.journal, roster[index as int].allocation);
        producer_count_correspondence(actual, model, roster[index as int].allocation);
        disposal_producer_correspondence_v1(actual, model, roster, index + 1);
    }
}

proof fn disposal_owner_validate_correspondence_v1(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
    writer_storage: usize, member_storage: usize, allocation_storage: usize)
    requires producer_represents(actual, model),
    ensures disposal_stable_validate_v1(actual.stable, writer, roster, writer_storage, member_storage, allocation_storage)
        == begin_result_from(logical::disposal_stable_validate_v1(model.stable, writer_reference_view(writer), begin_roster_view(roster),
            writer_storage, member_storage, allocation_storage)),
        disposal_producer_validate_v1(actual, writer, roster, writer_storage, member_storage, allocation_storage)
        == begin_result_from(logical::disposal_producer_validate_v1(model, writer_reference_view(writer), begin_roster_view(roster),
            writer_storage, member_storage, allocation_storage)),
{
    disposal_producer_correspondence_v1(actual, model, roster, 0);
    disposal_stable_correspondence_v1(actual.stable, model.stable, roster, 0);
    disposal_plan_correspondence_v1(actual.stable.journal, model.stable.journal, writer, roster,
        writer_storage, member_storage, allocation_storage);
}

proof fn disposal_refs_correspondence_v1(actual: JournalContentsV1, model: logical::JournalContentsV1,
    head: Option<usize>, count: usize)
    requires represents(actual, model), disposal_ready_v1(actual, head, count),
    ensures retirement_roster_view_v1(disposal_refs_v1(actual, head, count as nat))
        == logical::disposal_refs_v1(model, head, count as nat),
        forall|i: int| 0 <= i < count ==> (#[trigger] disposal_refs_v1(actual, head, count as nat)[i]).slot < actual.allocations@.len(),
{
    assert forall|i: int| 0 <= i < count implies
        allocation_reference_view(disposal_refs_v1(actual, head, count as nat)[i])
            == logical::disposal_refs_v1(model, head, count as nat)[i]
        && disposal_refs_v1(actual, head, count as nat)[i].slot < actual.allocations@.len() by {
        owner_settlement_plan_correspondence_v1(actual, model, head, count, i as nat);
        assert(settlement_plan_ready_v1(actual, head, i as nat));
    }
    assert(retirement_roster_view_v1(disposal_refs_v1(actual, head, count as nat))
        =~= logical::disposal_refs_v1(model, head, count as nat));
    assert forall|i: int| 0 <= i < count implies
        (#[trigger] disposal_refs_v1(actual, head, count as nat)[i]).slot < actual.allocations@.len() by {
        assert(settlement_plan_ready_v1(actual, head, i as nat));
    }
}

proof fn disposal_paired_transition_v1(before: JournalContentsV1, after: JournalContentsV1,
    model_before: logical::JournalContentsV1, model_after: logical::JournalContentsV1,
    writer: WriterReferenceV1, evidence: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
    writer_storage: usize, member_storage: usize, allocation_storage: usize,
    result: Result<(), ReadErrorV1>, model_result: Result<(), logical::ReadErrorV1>)
    requires represents(before, model_before),
        disposal_relation_v1(before, after, writer, evidence, roster, writer_storage, member_storage, allocation_storage, result),
        logical::disposal_relation_v1(model_before, model_after, writer_reference_view(writer), writer_reference_view(evidence),
            begin_roster_view(roster), writer_storage, member_storage, allocation_storage, model_result),
    ensures result == begin_result_from(model_result), represents(after, model_after),
{
    disposal_execute_correspondence_v1(before, model_before, writer, evidence, roster, writer_storage, member_storage, allocation_storage);
    if let Ok((head, count)) = disposal_execute_decision_v1(before, writer, evidence, roster,
        writer_storage, member_storage, allocation_storage) {
        disposal_preflight_ready_v1(before, writer, roster, writer_storage, member_storage, allocation_storage, head, count);
        disposal_refs_correspondence_v1(before, model_before, head, count);
        retirement_prefix_correspondence_v1(before, model_before, disposal_refs_v1(before, head, count as nat), count as nat);
        owner_settlement_prefix_correspondence_v1(before, model_before, head, count, count as nat, false);
        owner_settlement_slots_correspondence_v1(before, model_before, head, count);
        assert(journal_view(after).writers =~= model_after.writers@);
    }
}

proof fn disposal_owner_paired_transition_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    model_before: logical::ProducerReadContentsV1, model_after: logical::ProducerReadContentsV1,
    writer: WriterReferenceV1, evidence: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
    writer_storage: usize, member_storage: usize, allocation_storage: usize,
    result: Result<(), ReadErrorV1>, model_result: Result<(), logical::ReadErrorV1>)
    requires producer_represents(before, model_before),
        disposal_producer_relation_v1(before, after, writer, evidence, roster, writer_storage, member_storage, allocation_storage, result),
        logical::disposal_producer_relation_v1(model_before, model_after, writer_reference_view(writer), writer_reference_view(evidence),
            begin_roster_view(roster), writer_storage, member_storage, allocation_storage, model_result),
    ensures result == begin_result_from(model_result), producer_represents(after, model_after),
{
    disposal_owner_validate_correspondence_v1(before, model_before, writer, roster, writer_storage, member_storage, allocation_storage);
    if disposal_producer_validate_v1(before, writer, roster, writer_storage, member_storage, allocation_storage).is_ok() {
        disposal_paired_transition_v1(before.stable.journal, after.stable.journal, model_before.stable.journal, model_after.stable.journal,
            writer, evidence, roster, writer_storage, member_storage, allocation_storage, result, model_result);
    }
}

// Each side executes independently; this raw bridge requires only reached-prefix count safety.
fn owner_disposal_paired_exec_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    writer: WriterReferenceV1, model_writer: logical::WriterReferenceV1,
    evidence: WriterReferenceV1, model_evidence: logical::WriterReferenceV1,
    roster: &[AllocationWriteV1], model_roster: &[logical::AllocationWriteV1],
    writer_storage: usize, member_storage: usize, allocation_storage: usize,
    Ghost(storage): Ghost<logical::StorageCapacitiesV1>, Ghost(history): Ghost<Seq<logical::WriterReferenceV1>>)
    -> (results: (Result<(), ReadErrorV1>, Result<(), logical::ReadErrorV1>))
    requires producer_represents(*old(actual), *old(model)), writer_reference_view(writer) == model_writer,
        writer_reference_view(evidence) == model_evidence, begin_roster_view(roster@) == model_roster@,
        disposal_producer_safe_v1(*old(actual), roster@, 0),
    ensures results.0 == begin_result_from(results.1), producer_represents(*final(actual), *final(model)),
        disposal_producer_relation_v1(*old(actual), *final(actual), writer, evidence, roster@,
            writer_storage, member_storage, allocation_storage, results.0),
        logical::disposal_producer_relation_v1(*old(model), *final(model), model_writer, model_evidence, model_roster@,
            writer_storage, member_storage, allocation_storage, results.1),
        results.0.is_err() ==> *final(actual) == *old(actual), results.1.is_err() ==> *final(model) == *old(model),
        logical::producer_invariant_v1(*old(model)) ==> logical::producer_invariant_v1(*final(model)),
        logical::issued_producer_v1(*old(model), storage, history) ==> logical::issued_producer_v1(*final(model), storage, history),
        logical::producer_invariant_v1(*old(model)) ==> {
            &&& forall|reference: logical::ReadReferenceV1| #[trigger] logical::lease_decision_v1(final(model).stable, reference)
                == logical::lease_decision_v1(old(model).stable, reference)
            &&& forall|reference: logical::ProducerReadReferenceV1| #[trigger] logical::producer_lookup_decision_v1(*final(model), reference)
                == logical::producer_lookup_decision_v1(*old(model), reference)
            &&& forall|s: int| 0 <= s < old(model).reservations@.len() && (#[trigger] old(model).reservations@[s]).is_some()
                ==> logical::producer_status_decision_v1(final(model).stable.journal, old(model).reservations@[s].unwrap().request)
                    == logical::producer_status_decision_v1(old(model).stable.journal, old(model).reservations@[s].unwrap().request)
        },
{
    let ghost before = *actual;
    let ghost model_before = *model;
    proof { disposal_producer_correspondence_v1(before, model_before, roster@, 0); }
    let result = actual.dispose_unknown_observed_v1(writer, &ContextWriterDisposalEvidenceV1 { writer: evidence, allocations: roster },
        writer_storage, member_storage, allocation_storage);
    let model_result = logical::disposal_producer_exec_v1(model, model_writer, model_evidence, model_roster,
        writer_storage, member_storage, allocation_storage);
    proof {
        disposal_owner_paired_transition_v1(before, *actual, model_before, *model, writer, evidence, roster@,
            writer_storage, member_storage, allocation_storage, result, model_result);
        logical::disposal_producer_preserves_v1(model_before, *model, model_writer, model_evidence, model_roster@,
            writer_storage, member_storage, allocation_storage, model_result, storage, history);
    }
    (result, model_result)
}

proof fn owner_disposal_invariant_domain_v1(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    roster: Seq<AllocationWriteV1>)
    requires producer_represents(actual, model), logical::producer_invariant_v1(model),
    ensures disposal_producer_safe_v1(actual, roster, 0),
{
    logical::disposal_producer_safe_from_invariant_v1(model, begin_roster_view(roster), 0);
    disposal_producer_correspondence_v1(actual, model, roster, 0);
}

}
