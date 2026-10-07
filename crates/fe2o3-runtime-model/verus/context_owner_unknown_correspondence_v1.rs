verus! {

spec fn owner_previous_view(value: Option<AllocationKeyV1>) -> Option<logical::AllocationKeyV1> {
    match value { None => None, Some(key) => Some(allocation_key_view(key)) }
}

spec fn owner_member_result_from(value: Result<logical::MemberEntryV1, logical::ReadErrorV1>)
    -> Result<MemberEntryV1, ReadErrorV1>
{
    match value { Ok(member) => Ok(member_entry_from(member)), Err(error) => Err(read_error_embed(error)) }
}

proof fn owner_header_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1,
    writer: WriterReferenceV1, allow_unknown: bool)
    requires represents(journal, model),
    ensures owner_retained_header_v1(journal, writer, allow_unknown)
        == begin_result_from(logical::retained_header_decision_v1(model, writer_reference_view(writer), allow_unknown)),
{
    writer_key_round_trip(writer.key, writer_key_view(writer.key));
    if writer.slot < journal.writers@.len() {
        match journal.writers@[writer.slot as int] {
            Some(WriterEntryV1::Pending { key, .. }) | Some(WriterEntryV1::Unknown { key, .. }) => {
                writer_key_round_trip(key, writer_key_view(key));
            },
            _ => {},
        }
    }
}

proof fn owner_member_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1,
    writer: WriterReferenceV1, head: Option<usize>, previous: Option<AllocationKeyV1>)
    requires represents(journal, model),
    ensures owner_retained_member_v1(journal, writer, head, previous)
        == owner_member_result_from(logical::retained_member_decision_v1(model, writer_reference_view(writer), head, owner_previous_view(previous))),
{
    writer_reference_round_trip(writer, writer_reference_view(writer));
    if let Some(slot) = head {
        if slot < journal.members@.len() {
            if let Some(member) = journal.members@[slot as int] {
                member_entry_round_trip(member, member_entry_view(member));
                writer_reference_round_trip(member.writer, writer_reference_view(member.writer));
                begin_allocation_correspondence(journal, model, member.allocation);
            }
        }
    }
}

proof fn owner_scan_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1,
    writer: WriterReferenceV1, head: Option<usize>, remaining: nat, previous: Option<AllocationKeyV1>)
    requires represents(journal, model),
    ensures owner_retained_scan_v1(journal, writer, head, remaining, previous)
        == begin_result_from(logical::retained_scan_v1(model, writer_reference_view(writer), head, remaining, owner_previous_view(previous))),
    decreases remaining,
{
    if remaining > 0 {
        owner_member_correspondence(journal, model, writer, head, previous);
        if let Ok(member) = logical::retained_member_decision_v1(model, writer_reference_view(writer), head, owner_previous_view(previous)) {
            let actual_member = member_entry_from(member);
            member_entry_round_trip(actual_member, member);
            owner_scan_correspondence(journal, model, writer, actual_member.next, (remaining - 1) as nat, Some(actual_member.allocation.key));
        }
    }
}

proof fn owner_chain_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1,
    writer: WriterReferenceV1, head: Option<usize>, count: usize)
    requires represents(journal, model),
    ensures owner_retained_chain_v1(journal, writer, head, count)
        == begin_result_from(logical::retained_chain_decision_v1(model, writer_reference_view(writer), head, count)),
{
    owner_scan_correspondence(journal, model, writer, head, count as nat, None);
}

proof fn owner_unknown_decision_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1,
    writer: WriterReferenceV1)
    requires represents(journal, model),
    ensures owner_unknown_decision_v1(journal, writer)
        == begin_result_from(logical::unknown_decision_v1(model, writer_reference_view(writer))),
{
    owner_header_correspondence(journal, model, writer, true);
    if let Ok((head, count, _)) = owner_retained_header_v1(journal, writer, true) {
        owner_chain_correspondence(journal, model, writer, head, count);
    }
}

proof fn owner_unknown_paired_transition(before: JournalContentsV1, after: JournalContentsV1,
    model_before: logical::JournalContentsV1, model_after: logical::JournalContentsV1,
    writer: WriterReferenceV1, result: Result<(), ReadErrorV1>, model_result: Result<(), logical::ReadErrorV1>)
    requires represents(before, model_before), owner_unknown_relation_v1(before, after, writer, result),
        logical::unknown_execution_relation_v1(model_before, model_after, writer_reference_view(writer), model_result),
    ensures result == begin_result_from(model_result), represents(after, model_after),
{
    owner_unknown_decision_correspondence(before, model_before, writer);
    owner_header_correspondence(before, model_before, writer, true);
    if result.is_ok() {
        assert(journal_view(after).writers =~= model_after.writers@);
    }
}

// The outer actual method and historical journal execute independently. The latter
// lifts through unchanged owner fields without an issuance or storage premise.
fn producer_unknown_historical_exec_v1(actual: &mut ContextProducerReadJournalV1,
    model: &mut logical::ProducerReadContentsV1, writer: WriterReferenceV1, model_writer: logical::WriterReferenceV1)
    -> (results: (Result<(), ReadErrorV1>, Result<(), logical::ReadErrorV1>))
    requires producer_represents(*old(actual), *old(model)), writer_reference_view(writer) == model_writer,
    ensures results.0 == begin_result_from(results.1), producer_represents(*final(actual), *final(model)),
        producer_unknown_relation_v1(*old(actual), *final(actual), writer, results.0),
        logical::unknown_execution_relation_v1(old(model).stable.journal, final(model).stable.journal, model_writer, results.1),
        logical::producer_outer_frame_v1(*old(model), *final(model)),
        final(model).reservations == old(model).reservations,
        final(model).free == old(model).free,
        final(model).counts == old(model).counts,
        final(model).next_incarnation == old(model).next_incarnation,
        final(model).stable.leases == old(model).stable.leases,
        final(model).stable.free_reads == old(model).stable.free_reads,
        final(model).stable.readers == old(model).stable.readers,
        final(model).stable.next_incarnation == old(model).stable.next_incarnation,
        results.0.is_err() ==> *final(actual) == *old(actual),
        results.1.is_err() ==> *final(model) == *old(model),
        match owner_retained_header_v1(old(actual).stable.journal, writer, true) {
            Ok((_, _, true)) => *final(actual) == *old(actual) && *final(model) == *old(model), _ => true,
        },
        logical::producer_invariant_v1(*old(model)) ==> logical::producer_invariant_v1(*final(model)),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let result = actual.mark_unknown(writer);
    let model_result = logical::unknown_exec_v1(&mut model.stable.journal, model_writer);
    proof {
        owner_unknown_paired_transition(before.stable.journal, actual.stable.journal,
            model_before.stable.journal, model.stable.journal, writer, result, model_result);
        if logical::producer_invariant_v1(model_before) && model_result.is_ok() {
            logical::unknown_preserves_producer_invariant_v1(model_before, *model, model_writer);
        }
    }
    (result, model_result)
}

}
