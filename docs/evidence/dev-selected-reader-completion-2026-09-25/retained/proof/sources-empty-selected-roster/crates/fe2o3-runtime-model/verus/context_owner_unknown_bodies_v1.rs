verus! {

fn shared_retained_header_v1(journal: &JournalContentsV1, writer: WriterReferenceV1, allow_unknown: bool)
    -> (result: Result<(Option<usize>, usize, bool), ReadErrorV1>)
    ensures result == owner_retained_header_v1(*journal, writer, allow_unknown),
{
    retained_header_body!(journal, writer, allow_unknown)
}

fn shared_retained_member_v1(journal: &JournalContentsV1, writer: WriterReferenceV1,
    head: Option<usize>, previous: Option<AllocationKeyV1>) -> (result: Result<MemberEntryV1, ReadErrorV1>)
    ensures result == owner_retained_member_v1(*journal, writer, head, previous),
{
    retained_member_body!(journal, writer, head, previous)
}

fn shared_retained_chain_v1(journal: &JournalContentsV1, writer: WriterReferenceV1, initial: Option<usize>, count: usize)
    -> (result: Result<(), ReadErrorV1>)
    ensures result == owner_retained_chain_v1(*journal, writer, initial, count),
{
    retained_chain_body!(verus_exec_expr, journal, writer, initial, count, head, previous, index, [
        invariant index <= count, count <= journal.allocation_capacity, (count == 0) == initial.is_none(),
            owner_retained_scan_v1(*journal, writer, initial, count as nat, None)
                == owner_retained_scan_v1(*journal, writer, head, (count - index) as nat, previous),
        decreases count - index,
    ])
}

fn shared_retained_unknown_v1(journal: &mut JournalContentsV1, writer: WriterReferenceV1)
    -> (result: Result<(), ReadErrorV1>)
    ensures owner_unknown_relation_v1(*old(journal), *final(journal), writer, result),
{
    retained_unknown_body!(journal, writer)
}

impl ContextVersionJournalV1 {
    fn mark_unknown(&mut self, writer: WriterReferenceV1) -> (result: Result<(), ReadErrorV1>)
        ensures owner_unknown_relation_v1(*old(self), *final(self), writer, result),
    {
        journal_unknown_wrapper_body!(self, writer, shared_retained_unknown_v1)
    }
}

impl ContextReadLeasedJournalV1 {
    fn mark_unknown(&mut self, writer: WriterReferenceV1) -> (result: Result<(), ReadErrorV1>)
        ensures stable_unknown_relation_v1(*old(self), *final(self), writer, result),
    {
        stable_unknown_wrapper_body!(self, writer)
    }
}

impl ContextProducerReadJournalV1 {
    fn mark_unknown(&mut self, writer: WriterReferenceV1) -> (result: Result<(), ReadErrorV1>)
        ensures producer_unknown_relation_v1(*old(self), *final(self), writer, result),
    {
        producer_unknown_wrapper_body!(self, writer)
    }
}

}
