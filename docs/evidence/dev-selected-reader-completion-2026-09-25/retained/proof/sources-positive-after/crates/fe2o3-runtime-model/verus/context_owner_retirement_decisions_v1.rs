verus! {

spec fn retirement_item_v1(journal: JournalContentsV1, reference: AllocationReferenceV1,
    previous: Option<AllocationKeyV1>) -> Result<(), ReadErrorV1>
{
    match begin_exact_allocation_v1(journal, reference) {
        Err(error) => Err(error),
        Ok(entry) => if previous.is_some() && !begin_key_less_v1(previous.unwrap(), reference.key) {
            Err(ReadErrorV1::NonCanonicalRoster)
        } else if entry.pending_member.is_some() { Err(ReadErrorV1::AllocationBusy) }
        else { Ok(()) },
    }
}

spec fn retirement_scan_v1(journal: JournalContentsV1, roster: Seq<AllocationReferenceV1>,
    index: nat, previous: Option<AllocationKeyV1>) -> Result<(), ReadErrorV1>
    decreases roster.len() - index,
{
    if index >= roster.len() { Ok(()) }
    else { match retirement_item_v1(journal, roster[index as int], previous) {
        Err(error) => Err(error),
        Ok(()) => retirement_scan_v1(journal, roster, index + 1, Some(roster[index as int].key)),
    } }
}

spec fn retirement_decision_v1(journal: JournalContentsV1, roster: Seq<AllocationReferenceV1>, capacity: usize)
    -> Result<(), ReadErrorV1>
{
    if roster.len() > journal.allocation_capacity { Err(ReadErrorV1::RosterCapacity) }
    else { match retirement_scan_v1(journal, roster, 0, None) {
        Err(error) => Err(error),
        Ok(()) => if journal.allocation_free@.len() + roster.len() > usize::MAX
            || journal.allocation_free@.len() + roster.len() > journal.allocation_capacity
            || journal.allocation_free@.len() + roster.len() > capacity { Err(ReadErrorV1::InvalidState) }
            else { Ok(()) },
    } }
}

spec fn retirement_slots_v1(roster: Seq<AllocationReferenceV1>, count: nat) -> Seq<usize> {
    Seq::new(count, |i: int| roster[i].slot)
}

spec fn retirement_prefix_v1(allocations: Seq<Option<AllocationEntryV1>>,
    roster: Seq<AllocationReferenceV1>, count: nat) -> Seq<Option<AllocationEntryV1>>
    decreases count,
{
    if count == 0 { allocations }
    else { retirement_prefix_v1(allocations, roster, (count - 1) as nat).update(roster[count - 1].slot as int, None) }
}

spec fn retirement_relation_v1(before: JournalContentsV1, after: JournalContentsV1,
    roster: Seq<AllocationReferenceV1>, capacity: usize, result: Result<(), ReadErrorV1>) -> bool
{
    &&& result == retirement_decision_v1(before, roster, capacity)
    &&& enrollment_untouched_journal_v1(before, after)
    &&& match result {
        Err(_) => after == before,
        Ok(()) => after.allocations@ == retirement_prefix_v1(before.allocations@, roster, roster.len())
            && after.allocation_free@ == before.allocation_free@ + retirement_slots_v1(roster, roster.len()),
    }
}

proof fn retirement_scan_at_v1(journal: JournalContentsV1, roster: Seq<AllocationReferenceV1>,
    from: nat, previous: Option<AllocationKeyV1>, at: nat)
    requires from <= at < roster.len(), retirement_scan_v1(journal, roster, from, previous).is_ok(),
    ensures begin_exact_allocation_v1(journal, roster[at as int]).is_ok(),
        journal.allocations@[roster[at as int].slot as int].is_some(),
        roster[at as int].slot < journal.allocations@.len(),
        journal.allocations@[roster[at as int].slot as int].unwrap().pending_member.is_none(),
    decreases at - from,
{
    if from < at { retirement_scan_at_v1(journal, roster, from + 1, Some(roster[from as int].key), at); }
}

proof fn retirement_ready_v1(journal: JournalContentsV1, roster: Seq<AllocationReferenceV1>, capacity: usize)
    requires retirement_decision_v1(journal, roster, capacity).is_ok(),
    ensures forall|i: int| 0 <= i < roster.len() ==> (#[trigger] roster[i]).slot < journal.allocations@.len(),
        journal.allocation_free@.len() + roster.len() <= usize::MAX,
{
    assert forall|i: int| 0 <= i < roster.len() implies (#[trigger] roster[i]).slot < journal.allocations@.len() by {
        retirement_scan_at_v1(journal, roster, 0, None, i as nat);
    }
}

spec fn retirement_stable_unread_v1(owner: ContextReadLeasedJournalV1, roster: Seq<AllocationReferenceV1>, index: nat)
    -> Result<(), ReadErrorV1>
    decreases roster.len() - index,
{
    if index >= roster.len() { Ok(()) }
    else { match stable_count_decision_v1(owner, roster[index as int]) {
        Err(error) => Err(error),
        Ok(count) => if count != 0 { Err(ReadErrorV1::AllocationBusy) }
            else { retirement_stable_unread_v1(owner, roster, index + 1) },
    } }
}

spec fn retirement_producer_unread_v1(owner: ContextProducerReadJournalV1, roster: Seq<AllocationReferenceV1>, index: nat)
    -> Result<(), ReadErrorV1>
    decreases roster.len() - index,
{
    if index >= roster.len() { Ok(()) }
    else { match producer_count_decision_v1(owner, roster[index as int]) {
        Err(error) => Err(error),
        Ok(count) => if count != 0 { Err(ReadErrorV1::AllocationBusy) }
            else { retirement_producer_unread_v1(owner, roster, index + 1) },
    } }
}

spec fn retirement_stable_safe_v1(owner: ContextReadLeasedJournalV1, roster: Seq<AllocationReferenceV1>, index: nat) -> bool
    decreases roster.len() - index,
{
    index >= roster.len() || (stable_count_safe_v1(owner, roster[index as int])
        && (stable_count_decision_v1(owner, roster[index as int]) == Ok(0)
            ==> retirement_stable_safe_v1(owner, roster, index + 1)))
}

spec fn retirement_producer_safe_v1(owner: ContextProducerReadJournalV1, roster: Seq<AllocationReferenceV1>, index: nat) -> bool
    decreases roster.len() - index,
{
    index >= roster.len() || (producer_count_safe_v1(owner, roster[index as int])
        && (producer_count_decision_v1(owner, roster[index as int]) == Ok(0)
            ==> retirement_producer_safe_v1(owner, roster, index + 1)))
}

proof fn retirement_producer_to_stable_v1(owner: ContextProducerReadJournalV1, roster: Seq<AllocationReferenceV1>, index: nat)
    requires retirement_producer_safe_v1(owner, roster, index), retirement_producer_unread_v1(owner, roster, index).is_ok(),
    ensures retirement_stable_safe_v1(owner.stable, roster, index), retirement_stable_unread_v1(owner.stable, roster, index).is_ok(),
    decreases roster.len() - index,
{
    if index < roster.len() {
        retirement_producer_to_stable_v1(owner, roster, index + 1);
    }
}

spec fn retirement_stable_decision_v1(owner: ContextReadLeasedJournalV1, roster: Seq<AllocationReferenceV1>, capacity: usize)
    -> Result<(), ReadErrorV1>
{
    match retirement_stable_unread_v1(owner, roster, 0) {
        Err(error) => Err(error), Ok(()) => retirement_decision_v1(owner.journal, roster, capacity),
    }
}

spec fn retirement_producer_decision_v1(owner: ContextProducerReadJournalV1, roster: Seq<AllocationReferenceV1>, capacity: usize)
    -> Result<(), ReadErrorV1>
{
    match retirement_producer_unread_v1(owner, roster, 0) {
        Err(error) => Err(error), Ok(()) => retirement_stable_decision_v1(owner.stable, roster, capacity),
    }
}

spec fn retirement_stable_relation_v1(before: ContextReadLeasedJournalV1, after: ContextReadLeasedJournalV1,
    roster: Seq<AllocationReferenceV1>, capacity: usize, result: Result<(), ReadErrorV1>) -> bool
{
    &&& result == retirement_stable_decision_v1(before, roster, capacity)
    &&& owner_writer_stable_frame_v1(before, after)
    &&& match retirement_stable_unread_v1(before, roster, 0) {
        Err(_) => after == before,
        Ok(()) => retirement_relation_v1(before.journal, after.journal, roster, capacity, result),
    }
    &&& result.is_err() ==> after == before
}

spec fn retirement_producer_relation_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    roster: Seq<AllocationReferenceV1>, capacity: usize, result: Result<(), ReadErrorV1>) -> bool
{
    &&& result == retirement_producer_decision_v1(before, roster, capacity)
    &&& owner_writer_producer_frame_v1(before, after)
    &&& match retirement_producer_unread_v1(before, roster, 0) {
        Err(_) => after == before,
        Ok(()) => retirement_stable_relation_v1(before.stable, after.stable, roster, capacity, result),
    }
    &&& result.is_err() ==> after == before
}

}
