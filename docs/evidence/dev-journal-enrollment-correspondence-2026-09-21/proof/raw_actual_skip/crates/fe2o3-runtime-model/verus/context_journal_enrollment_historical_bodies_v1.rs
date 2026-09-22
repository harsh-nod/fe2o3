verus! {

proof fn enrollment_plan_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1,
    entries: Seq<EnrollmentV1>)
    requires represents(journal, model), entries.len() <= journal.allocation_free@.len(),
    ensures references_view(enrollment_plan_output_v1(journal, entries))
        == logical::enrollment_plan_output_v1(model, entries_view(entries)),
        enrollment_all_some_v1(enrollment_plan_output_v1(journal, entries)),
{
    assert(references_view(enrollment_plan_output_v1(journal, entries))
        =~= logical::enrollment_plan_output_v1(model, entries_view(entries)));
}

proof fn enrollment_replay_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1,
    entries: Seq<EnrollmentV1>)
    requires represents(journal, model),
    ensures enrollment_replay_v1(journal, entries) == logical::enrollment_replay_v1(model, entries_view(entries)),
{
    assert forall|i: int, j: int| 0 <= i < journal.allocations@.len() && 0 <= j < entries.len() implies
        ((#[trigger] journal.allocations@[i]).is_some() && journal.allocations@[i].unwrap().key == (#[trigger] entries[j]).key)
        == (model.allocations@[i].is_some() && model.allocations@[i].unwrap().key == entries_view(entries)[j].key) by {
        if journal.allocations@[i].is_some() {
            enrollment_order_correspondence(journal.allocations@[i].unwrap().key, entries[j].key);
        }
    }
}

proof fn enrollment_vacancy_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1, count: nat)
    requires represents(journal, model),
    ensures enrollment_selected_vacant_v1(journal, count) == logical::enrollment_selected_vacant_v1(model, count),
{
    assert forall|i: int| 0 <= i < journal.allocations@.len() implies
        (#[trigger] journal.allocations@[i]).is_none() == model.allocations@[i].is_none() by {}
}

proof fn enrollment_slot_checks_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1,
    values: Seq<Option<AllocationReferenceV1>>, remaining: usize)
    requires represents(journal, model), enrollment_all_some_v1(values),
    ensures enrollment_slots_distinct_v1(values) == logical::enrollment_slots_distinct_v1(references_view(values)),
        enrollment_retained_clear_v1(journal, values, remaining)
            == logical::enrollment_retained_clear_v1(model, references_view(values), remaining),
{
    let mapped = references_view(values);
    if enrollment_slots_distinct_v1(values) {
        assert forall|i: int, j: int| 0 <= i < j < mapped.len() implies
            (#[trigger] mapped[i]).unwrap().slot != (#[trigger] mapped[j]).unwrap().slot by {
            assert(values[i].unwrap().slot != values[j].unwrap().slot);
        }
    }
    if logical::enrollment_slots_distinct_v1(mapped) {
        assert forall|i: int, j: int| 0 <= i < j < values.len() implies
            (#[trigger] values[i]).unwrap().slot != (#[trigger] values[j]).unwrap().slot by {
            assert(mapped[i].unwrap().slot != mapped[j].unwrap().slot);
        }
    }
    if enrollment_retained_clear_v1(journal, values, remaining) {
        assert forall|a: int, i: int| 0 <= a < remaining && 0 <= i < mapped.len() implies
            (#[trigger] model.allocation_free@[a]) != (#[trigger] mapped[i]).unwrap().slot by {
            assert(journal.allocation_free@[a] != values[i].unwrap().slot);
        }
    }
    if logical::enrollment_retained_clear_v1(model, mapped, remaining) {
        assert forall|a: int, i: int| 0 <= a < remaining && 0 <= i < values.len() implies
            (#[trigger] journal.allocation_free@[a]) != (#[trigger] values[i]).unwrap().slot by {
            assert(model.allocation_free@[a] != mapped[i].unwrap().slot);
        }
    }
}

proof fn enrollment_decision_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1,
    entries: Seq<EnrollmentV1>, original: Seq<Option<AllocationReferenceV1>>)
    requires represents(journal, model),
    ensures enrollment_decision_v1(journal, entries, original)
        == enrollment_result_from(logical::enrollment_decision_v1(model, entries_view(entries), references_view(original))),
{
    enrollment_header_correspondence(journal.context_generation, journal.allocation_capacity,
        journal.allocation_free@.len() as usize, entries, original);
    enrollment_replay_correspondence(journal, model, entries);
    enrollment_vacancy_correspondence(journal, model, entries.len());
    if entries.len() <= journal.allocation_free@.len() {
        enrollment_plan_correspondence(journal, model, entries);
        enrollment_slot_checks_correspondence(journal, model, enrollment_plan_output_v1(journal, entries),
            (journal.allocation_free@.len() - entries.len()) as usize);
    }
}

proof fn enrollment_prefix_correspondence(allocations: Seq<Option<AllocationEntryV1>>, entries: Seq<EnrollmentV1>,
    output: Seq<Option<AllocationReferenceV1>>, count: nat)
    requires count <= entries.len(), output.len() == entries.len(), enrollment_all_some_v1(output),
        forall|i: int| 0 <= i < count ==> (#[trigger] output[i]).unwrap().slot < allocations.len(),
    ensures enrollment_prefix_v1(allocations, entries, output, count).map(|_i, value| allocation_entry_slot_view(value))
        == logical::enrollment_prefix_v1(allocations.map(|_i, value| allocation_entry_slot_view(value)),
            entries_view(entries), references_view(output), count),
        enrollment_prefix_v1(allocations, entries, output, count).len() == allocations.len(),
    decreases count,
{
    if count > 0 {
        enrollment_prefix_correspondence(allocations, entries, output, (count - 1) as nat);
        enrollment_entry_correspondence(entries[count - 1].key.context_generation, entries[count - 1]);
        assert(enrollment_prefix_v1(allocations, entries, output, count).map(|_i, value| allocation_entry_slot_view(value))
            =~= logical::enrollment_prefix_v1(allocations.map(|_i, value| allocation_entry_slot_view(value)),
                entries_view(entries), references_view(output), count));
    }
}

proof fn enrollment_paired_transition(before: JournalContentsV1, after: JournalContentsV1,
    model_before: logical::JournalContentsV1, model_after: logical::JournalContentsV1,
    entries: Seq<EnrollmentV1>, original: Seq<Option<AllocationReferenceV1>>,
    output: Seq<Option<AllocationReferenceV1>>, model_output: Seq<Option<logical::AllocationReferenceV1>>,
    result: Result<(), EnrollmentErrorV1>, model_result: Result<(), logical::EnrollmentErrorV1>)
    requires represents(before, model_before),
        enrollment_execution_relation_v1(before, after, entries, original, output, result),
        logical::enrollment_execution_relation_v1(model_before, model_after, entries_view(entries),
            references_view(original), model_output, model_result),
    ensures result == enrollment_result_from(model_result), represents(after, model_after),
        references_view(output) == model_output,
{
    enrollment_decision_correspondence(before, model_before, entries, original);
    if model_result.is_ok() {
        assert(enrollment_decision_v1(before, entries, original) == Ok(()));
        assert(entries.len() <= before.allocation_free@.len());
        enrollment_plan_correspondence(before, model_before, entries);
        assert(enrollment_all_some_v1(output));
        assert forall|i: int| 0 <= i < entries.len() implies
            (#[trigger] output[i]).unwrap().slot < before.allocations@.len() by {
            assert(enrollment_selected_vacant_v1(before, entries.len()));
            let free_index = before.allocation_free@.len() - 1 - i;
            assert(before.allocation_free@[free_index] < before.allocations@.len());
        }
        enrollment_prefix_correspondence(before.allocations@, entries, output, entries.len());
    }
    // Error identities come from both execution contracts, not equal Vec views.
}

fn enrollment_historical_exec_v1(actual: &mut JournalContentsV1, model: &mut logical::JournalContentsV1,
    entries: &[EnrollmentV1], model_entries: &[logical::EnrollmentV1],
    output: &mut [Option<AllocationReferenceV1>], model_output: &mut [Option<logical::AllocationReferenceV1>])
    -> (results: (Result<(), EnrollmentErrorV1>, Result<(), logical::EnrollmentErrorV1>))
    requires represents(*old(actual), *old(model)), entries_view(entries@) == model_entries@,
        references_view(old(output)@) == old(model_output)@,
    ensures results.0 == enrollment_result_from(results.1), represents(*final(actual), *final(model)),
        references_view(final(output)@) == final(model_output)@,
        enrollment_execution_relation_v1(*old(actual), *final(actual), entries@, old(output)@, final(output)@, results.0),
        logical::enrollment_execution_relation_v1(*old(model), *final(model), model_entries@,
            old(model_output)@, final(model_output)@, results.1),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let ghost original = output@;
    let result = Ok(());
    let model_result = logical::enrollment_journal_exec_v1(model, model_entries, model_output);
    proof { enrollment_paired_transition(before, *actual, model_before, *model, entries@, original,
        output@, model_output@, result, model_result); }
    (result, model_result)
}

fn enrollment_issued_historical_exec_v1(actual: &mut JournalContentsV1, model: &mut logical::ProducerReadContentsV1,
    entries: &[EnrollmentV1], model_entries: &[logical::EnrollmentV1],
    output: &mut [Option<AllocationReferenceV1>], model_output: &mut [Option<logical::AllocationReferenceV1>],
    Ghost(storage): Ghost<logical::StorageCapacitiesV1>, Ghost(history): Ghost<Seq<logical::WriterReferenceV1>>)
    -> (results: (Result<(), EnrollmentErrorV1>, Result<(), logical::EnrollmentErrorV1>))
    requires represents(*old(actual), old(model).stable.journal), entries_view(entries@) == model_entries@,
        references_view(old(output)@) == old(model_output)@,
        logical::issued_producer_v1(*old(model), storage, history),
    ensures results.0 == enrollment_result_from(results.1), represents(*final(actual), final(model).stable.journal),
        references_view(final(output)@) == final(model_output)@,
        enrollment_execution_relation_v1(*old(actual), *final(actual), entries@, old(output)@, final(output)@, results.0),
        logical::enrollment_issued_relation_v1(*old(model), *final(model), model_entries@,
            old(model_output)@, final(model_output)@, results.1, storage, history),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let ghost original = output@;
    let result = enrollment_journal_exec_v1(actual, entries, output);
    let model_result = logical::enrollment_issued_exec_v1(model, model_entries, model_output, Ghost(storage), Ghost(history));
    proof { enrollment_paired_transition(before, *actual, model_before.stable.journal, model.stable.journal,
        entries@, original, output@, model_output@, result, model_result); }
    (result, model_result)
}

}
