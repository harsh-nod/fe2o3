verus! {

proof fn begin_member_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1,
    writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, index: int)
    requires represents(journal, model), begin_storage_ready_v1(journal, roster), 0 <= index < roster.len(),
    ensures member_entry_view(begin_member_v1(journal, writer, roster, index))
        == logical::begin_member_v1(model, writer_reference_view(writer), begin_roster_view(roster), index),
{
    begin_plan_correspondence(journal, model, roster, index);
    if index + 1 < roster.len() { begin_plan_correspondence(journal, model, roster, index + 1); }
}

proof fn begin_prefix_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1,
    writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, count: nat)
    requires represents(journal, model), begin_storage_ready_v1(journal, roster), count <= roster.len(),
    ensures begin_members_prefix_v1(journal, writer, roster, count).map(|_i, value| member_entry_slot_view(value))
        == logical::begin_members_prefix_v1(model, writer_reference_view(writer), begin_roster_view(roster), count),
        begin_allocations_prefix_v1(journal, roster, count).map(|_i, value| allocation_entry_slot_view(value))
        == logical::begin_allocations_prefix_v1(model, begin_roster_view(roster), count),
    decreases count,
{
    if count > 0 {
        begin_prefix_correspondence(journal, model, writer, roster, (count - 1) as nat);
        begin_plan_correspondence(journal, model, roster, count - 1);
        begin_member_correspondence(journal, model, writer, roster, count - 1);
        begin_prefix_shapes_v1(journal, writer, roster, (count - 1) as nat);
        assert(begin_destination_decision_v1(journal, roster[count - 1]) == Ok(()));
        assert(begin_members_prefix_v1(journal, writer, roster, count).map(|_i, value| member_entry_slot_view(value))
            =~= logical::begin_members_prefix_v1(model, writer_reference_view(writer), begin_roster_view(roster), count));
        assert(begin_allocations_prefix_v1(journal, roster, count).map(|_i, value| allocation_entry_slot_view(value))
            =~= logical::begin_allocations_prefix_v1(model, begin_roster_view(roster), count));
    }
}

proof fn begin_paired_transition(before: JournalContentsV1, after: JournalContentsV1,
    model_before: logical::JournalContentsV1, model_after: logical::JournalContentsV1,
    writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
    result: Result<(), ReadErrorV1>, model_result: Result<(), logical::ReadErrorV1>)
    requires represents(before, model_before),
        begin_execution_relation_v1(before, after, writer, roster, result),
        logical::begin_execution_relation_v1(model_before, model_after, writer_reference_view(writer),
            begin_roster_view(roster), model_result),
    ensures result == begin_result_from(model_result), represents(after, model_after),
{
    begin_preflight_correspondence(before, model_before, writer, roster);
    if result.is_ok() {
        begin_preflight_ready_v1(before, writer, roster);
        begin_prefix_correspondence(before, model_before, writer, roster, roster.len());
        if roster.len() > 0 { begin_plan_correspondence(before, model_before, roster, 0); }
        assert(journal_view(after).writers =~= model_after.writers@);
    }
    // Each executable contract supplies its own rejection identity.
}

fn begin_historical_exec_v1(actual: &mut JournalContentsV1, model: &mut logical::JournalContentsV1,
    writer: WriterReferenceV1, model_writer: logical::WriterReferenceV1,
    roster: &[AllocationWriteV1], model_roster: &[logical::AllocationWriteV1])
    -> (results: (Result<(), ReadErrorV1>, Result<(), logical::ReadErrorV1>))
    requires represents(*old(actual), *old(model)), writer_reference_view(writer) == model_writer,
        begin_roster_view(roster@) == model_roster@,
    ensures results.0 == begin_result_from(results.1), represents(*final(actual), *final(model)),
        begin_execution_relation_v1(*old(actual), *final(actual), writer, roster@, results.0),
        logical::begin_execution_relation_v1(*old(model), *final(model), model_writer, model_roster@, results.1),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let result = Ok(());
    let model_result = logical::begin_exec_v1(model, model_writer, model_roster);
    proof { begin_paired_transition(before, *actual, model_before, *model, writer, roster@, result, model_result); }
    (result, model_result)
}

}
