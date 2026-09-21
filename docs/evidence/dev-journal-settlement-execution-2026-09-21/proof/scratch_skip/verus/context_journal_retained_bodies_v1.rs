use self::ContextVersionJournalErrorV1 as ReadErrorV1;
use self::ContextWriterKindV1 as WriterKindV1;

verus! {

// Production instrumentation is empty outside tests; the content view excludes it.
fn retained_indexed_access_v1(journal: &ContextVersionJournalV1) {}

fn shared_retained_writer_key_v1(left: ContextWriterKeyV1, right: ContextWriterKeyV1) -> (result: bool)
    ensures result == logical::same_key_v1(writer_key_view(left), writer_key_view(right)),
{
    retained_writer_key_body!(left, right)
}

fn shared_retained_allocation_less_v1(left: ContextAllocationKeyV1, right: ContextAllocationKeyV1) -> (result: bool)
    ensures result == logical::enrollment_key_less_v1(allocation_key_view(left), allocation_key_view(right)),
{
    retained_allocation_less_body!(left, right)
}

fn shared_retained_allocation_v1(journal: &ContextVersionJournalV1, reference: ContextAllocationReferenceV1)
    -> (result: Result<AllocationEntryV1, ReadErrorV1>)
    ensures result == allocation_result_from(allocation_decision(journal_view(*journal), allocation_reference_view(reference))),
{
    retained_allocation_body!(journal, reference)
}

fn shared_retained_header_v1(journal: &ContextVersionJournalV1, writer: ContextWriterReferenceV1, allow_unknown: bool)
    -> (result: Result<(Option<usize>, usize, bool), ReadErrorV1>)
    ensures result == read_result_from(header_decision(journal_view(*journal), writer_reference_view(writer), allow_unknown)),
{
    retained_header_body!(journal, writer, allow_unknown)
}

fn shared_retained_member_v1(journal: &ContextVersionJournalV1, writer: ContextWriterReferenceV1,
    head: Option<usize>, previous: Option<ContextAllocationKeyV1>) -> (result: Result<MemberEntryV1, ReadErrorV1>)
    ensures result == member_result_from(member_decision(journal_view(*journal), writer_reference_view(writer), head, previous_view(previous))),
{
    retained_member_body!(journal, writer, head, previous)
}

fn shared_retained_chain_v1(journal: &ContextVersionJournalV1, writer: ContextWriterReferenceV1, initial: Option<usize>, count: usize)
    -> (result: Result<(), ReadErrorV1>)
    ensures result == read_result_from(chain_decision(journal_view(*journal), writer_reference_view(writer), initial, count)),
{
    retained_chain_body!(verus_exec_expr, journal, writer, initial, count, head, previous, index, [
        invariant index <= count, count <= journal.allocation_capacity, (count == 0) == initial.is_none(),
            scan_decision(journal_view(*journal), writer_reference_view(writer), initial, count as nat, None)
                == scan_decision(journal_view(*journal), writer_reference_view(writer), head, (count - index) as nat, previous_view(previous)),
        decreases count - index,
    ])
}

// Conditional historical correspondence supplements, rather than replaces, raw-state contracts.
fn retained_chain_historical(journal: &ContextVersionJournalV1, writer: ContextWriterReferenceV1,
    initial: Option<usize>, count: usize, Ghost(model): Ghost<logical::JournalContentsV1>)
    -> (result: Result<(), ReadErrorV1>)
    requires represents(*journal, model),
    ensures result == read_result_from(logical::retained_chain_decision_v1(model, writer_reference_view(writer), initial, count)),
{
    proof { chain_historical(model, writer_reference_view(writer), initial, count); }
    shared_retained_chain_v1(journal, writer, initial, count)
}

}
