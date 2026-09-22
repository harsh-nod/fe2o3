verus! {

spec fn allocation_lookup_decision_v1(journal: JournalContentsV1, reference: AllocationReferenceV1)
    -> Result<ContextAllocationStateV1, ReadErrorV1>
{
    match begin_exact_allocation_v1(journal, reference) {
        Err(error) => Err(error),
        Ok(entry) => {
            let state = ContextAllocationStateV1 { device: entry.device, byte_extent: entry.byte_extent,
                attempt_epoch: entry.attempt_epoch, content_lineage: entry.content_lineage, pending_writer: None };
            match entry.pending_member {
                None => Ok(state),
                Some(slot) => if slot >= journal.members@.len() { Err(ReadErrorV1::InvalidState) }
                    else { match journal.members@[slot as int] {
                        None => Err(ReadErrorV1::InvalidState),
                        Some(member) => if member.allocation != reference { Err(ReadErrorV1::InvalidState) }
                            else { Ok(ContextAllocationStateV1 { pending_writer: Some(member.writer), ..state }) },
                    } },
            }
        },
    }
}

spec fn stable_count_safe_v1(contents: ContextReadLeasedJournalV1, allocation: AllocationReferenceV1) -> bool {
    allocation_lookup_decision_v1(contents.journal, allocation).is_ok() ==> allocation.slot < contents.readers@.len()
}

spec fn stable_count_decision_v1(contents: ContextReadLeasedJournalV1, allocation: AllocationReferenceV1)
    -> Result<usize, ReadErrorV1>
{
    match allocation_lookup_decision_v1(contents.journal, allocation) {
        Err(error) => Err(error), Ok(_) => Ok(contents.readers@[allocation.slot as int]),
    }
}

spec fn producer_count_safe_v1(contents: ContextProducerReadJournalV1, allocation: AllocationReferenceV1) -> bool {
    &&& stable_count_safe_v1(contents.stable, allocation)
    &&& stable_count_decision_v1(contents.stable, allocation).is_ok() ==>
        allocation.slot < contents.counts@.len()
        && contents.stable.readers@[allocation.slot as int] + contents.counts@[allocation.slot as int] <= usize::MAX
}

spec fn producer_count_decision_v1(contents: ContextProducerReadJournalV1, allocation: AllocationReferenceV1)
    -> Result<usize, ReadErrorV1>
{
    match stable_count_decision_v1(contents.stable, allocation) {
        Err(error) => Err(error), Ok(stable) => Ok((stable + contents.counts@[allocation.slot as int]) as usize),
    }
}

spec fn stable_guard_storage_v1(contents: ContextReadLeasedJournalV1) -> bool {
    contents.readers@.len() == contents.journal.allocations@.len()
}

spec fn producer_guard_storage_v1(contents: ContextProducerReadJournalV1) -> bool {
    &&& stable_guard_storage_v1(contents.stable)
    &&& contents.counts@.len() == contents.stable.readers@.len()
    &&& forall|a: int| 0 <= a < contents.counts@.len() ==>
        contents.stable.readers@[a] + (#[trigger] contents.counts@[a]) <= usize::MAX
}

spec fn stable_unread_writes_v1(contents: ContextReadLeasedJournalV1, roster: Seq<AllocationWriteV1>, index: nat)
    -> Result<(), ReadErrorV1>
    decreases roster.len() - index,
{
    if index >= roster.len() { Ok(()) }
    else { match stable_count_decision_v1(contents, roster[index as int].allocation) {
        Err(error) => Err(error),
        Ok(count) => if count != 0 { Err(ReadErrorV1::AllocationBusy) }
            else { stable_unread_writes_v1(contents, roster, index + 1) },
    } }
}

spec fn producer_unread_writes_v1(contents: ContextProducerReadJournalV1, roster: Seq<AllocationWriteV1>, index: nat)
    -> Result<(), ReadErrorV1>
    decreases roster.len() - index,
{
    if index >= roster.len() { Ok(()) }
    else { match producer_count_decision_v1(contents, roster[index as int].allocation) {
        Err(error) => Err(error),
        Ok(count) => if count != 0 { Err(ReadErrorV1::AllocationBusy) }
            else { producer_unread_writes_v1(contents, roster, index + 1) },
    } }
}

spec fn stable_guard_frame_v1(before: ContextReadLeasedJournalV1, after: ContextReadLeasedJournalV1) -> bool {
    &&& true
    &&& after.free_reads == before.free_reads
    &&& after.readers == before.readers
    &&& after.next_incarnation == before.next_incarnation
}

spec fn producer_guard_frame_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1) -> bool {
    &&& stable_guard_frame_v1(before.stable, after.stable)
    &&& after.reservations == before.reservations
    &&& after.free == before.free
    &&& after.counts == before.counts
    &&& after.next_incarnation == before.next_incarnation
}

spec fn stable_begin_relation_v1(before: ContextReadLeasedJournalV1, after: ContextReadLeasedJournalV1,
    writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, result: Result<(), ReadErrorV1>) -> bool
{
    &&& stable_guard_frame_v1(before, after)
    &&& match stable_unread_writes_v1(before, roster, 0) {
        Err(error) => result == Err(error) && after == before,
        Ok(()) => begin_execution_relation_v1(before.journal, after.journal, writer, roster, result),
    }
    &&& result.is_err() ==> after == before
}

spec fn producer_begin_relation_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, result: Result<(), ReadErrorV1>) -> bool
{
    &&& producer_guard_frame_v1(before, after)
    &&& match producer_unread_writes_v1(before, roster, 0) {
        Err(error) => result == Err(error) && after == before,
        Ok(()) => stable_begin_relation_v1(before.stable, after.stable, writer, roster, result),
    }
    &&& result.is_err() ==> after == before
}

}
