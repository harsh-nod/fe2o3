verus! {

fn shared_retained_unknown_v1(journal: &mut ContextVersionJournalV1, writer: ContextWriterReferenceV1)
    -> (result: Result<(), ReadErrorV1>)
    ensures unknown_execution_view(journal_view(*old(journal)), journal_view(*final(journal)), writer_reference_view(writer), result),
{
    let ghost before = *journal;
    let result = retained_unknown_body!(journal, writer);
    proof {
        assert(journal_view(*journal).writers =~= unknown_after(journal_view(before), writer_reference_view(writer)).writers);
    }
    result
}

fn retained_unknown_historical(journal: &mut ContextVersionJournalV1, writer: ContextWriterReferenceV1,
    Ghost(model): Ghost<logical::JournalContentsV1>) -> (result: Result<(), ReadErrorV1>)
    requires represents(*old(journal), model),
    ensures result == read_result_from(logical::unknown_decision_v1(model, writer_reference_view(writer))),
        journal_view(*final(journal)) == unknown_after(logical_contents(model), writer_reference_view(writer)),
{
    proof { unknown_decision_historical(model, writer_reference_view(writer)); }
    shared_retained_unknown_v1(journal, writer)
}

}
