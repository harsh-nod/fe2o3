// Independent logical retirement decisions and executor; not physical storage refinement.
use super::*;

verus! {

pub open spec fn retirement_journal_frame_v1(before: JournalContentsV1, after: JournalContentsV1) -> bool {
    &&& enrollment_untouched_journal_v1(before, after)
    &&& after.writers == before.writers && after.free == before.free
    &&& after.members == before.members && after.member_free == before.member_free && after.scratch == before.scratch
}

pub open spec fn retirement_stable_frame_v1(before: ReadContentsV1, after: ReadContentsV1) -> bool {
    after.leases == before.leases && after.free_reads == before.free_reads
        && after.readers == before.readers && after.next_incarnation == before.next_incarnation
}

pub open spec fn retirement_producer_frame_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1) -> bool {
    retirement_stable_frame_v1(before.stable, after.stable) && after.reservations == before.reservations
        && after.free == before.free && after.counts == before.counts && after.next_incarnation == before.next_incarnation
}

pub open spec fn retirement_stable_count_decision_v1(owner: ReadContentsV1, reference: AllocationReferenceV1)
    -> Result<usize, ReadErrorV1>
{
    match allocation_decision_v1(owner.journal, reference) {
        Err(error) => Err(error), Ok(_) => Ok(owner.readers@[reference.slot as int]),
    }
}

pub open spec fn retirement_stable_count_safe_v1(owner: ReadContentsV1, reference: AllocationReferenceV1) -> bool {
    allocation_decision_v1(owner.journal, reference).is_ok() ==> reference.slot < owner.readers@.len()
}

pub open spec fn retirement_producer_count_safe_v1(owner: ProducerReadContentsV1, reference: AllocationReferenceV1) -> bool {
    &&& retirement_stable_count_safe_v1(owner.stable, reference)
    &&& allocation_decision_v1(owner.stable.journal, reference).is_ok() ==> reference.slot < owner.counts@.len()
        && owner.stable.readers@[reference.slot as int] + owner.counts@[reference.slot as int] <= usize::MAX
}

pub open spec fn retirement_item_v1(journal: JournalContentsV1, reference: AllocationReferenceV1,
    previous: Option<AllocationKeyV1>) -> Result<(), ReadErrorV1>
{
    match begin_exact_allocation_v1(journal, reference) {
        Err(error) => Err(error),
        Ok(entry) => if previous.is_some() && !enrollment_key_less_v1(previous.unwrap(), reference.key) {
            Err(ReadErrorV1::NonCanonicalRoster)
        } else if entry.pending_member.is_some() { Err(ReadErrorV1::AllocationBusy) }
        else { Ok(()) },
    }
}

pub open spec fn retirement_scan_v1(journal: JournalContentsV1, roster: Seq<AllocationReferenceV1>,
    index: nat, previous: Option<AllocationKeyV1>) -> Result<(), ReadErrorV1>
    decreases roster.len() - index,
{
    if index >= roster.len() { Ok(()) }
    else { match retirement_item_v1(journal, roster[index as int], previous) {
        Err(error) => Err(error),
        Ok(()) => retirement_scan_v1(journal, roster, index + 1, Some(roster[index as int].key)),
    } }
}

pub open spec fn retirement_decision_v1(journal: JournalContentsV1, roster: Seq<AllocationReferenceV1>, capacity: usize)
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

pub open spec fn retirement_slots_v1(roster: Seq<AllocationReferenceV1>, count: nat) -> Seq<usize> {
    Seq::new(count, |i: int| roster[i].slot)
}

pub open spec fn retirement_prefix_v1(allocations: Seq<Option<AllocationEntryV1>>,
    roster: Seq<AllocationReferenceV1>, count: nat) -> Seq<Option<AllocationEntryV1>>
    decreases count,
{
    if count == 0 { allocations }
    else { retirement_prefix_v1(allocations, roster, (count - 1) as nat).update(roster[count - 1].slot as int, None) }
}

pub open spec fn retirement_relation_v1(before: JournalContentsV1, after: JournalContentsV1,
    roster: Seq<AllocationReferenceV1>, capacity: usize, result: Result<(), ReadErrorV1>) -> bool
{
    &&& result == retirement_decision_v1(before, roster, capacity)
    &&& retirement_journal_frame_v1(before, after)
    &&& match result {
        Err(_) => after == before,
        Ok(()) => after.allocations@ == retirement_prefix_v1(before.allocations@, roster, roster.len())
            && after.allocation_free@ == before.allocation_free@ + retirement_slots_v1(roster, roster.len()),
    }
}

pub proof fn retirement_scan_at_v1(journal: JournalContentsV1, roster: Seq<AllocationReferenceV1>,
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

pub proof fn retirement_ready_v1(journal: JournalContentsV1, roster: Seq<AllocationReferenceV1>, capacity: usize)
    requires retirement_decision_v1(journal, roster, capacity).is_ok(),
    ensures forall|i: int| 0 <= i < roster.len() ==> (#[trigger] roster[i]).slot < journal.allocations@.len(),
        journal.allocation_free@.len() + roster.len() <= usize::MAX,
{
    assert forall|i: int| 0 <= i < roster.len() implies (#[trigger] roster[i]).slot < journal.allocations@.len() by {
        retirement_scan_at_v1(journal, roster, 0, None, i as nat);
    }
}

pub open spec fn retirement_stable_unread_v1(owner: ReadContentsV1, roster: Seq<AllocationReferenceV1>, index: nat)
    -> Result<(), ReadErrorV1>
    decreases roster.len() - index,
{
    if index >= roster.len() { Ok(()) }
    else { match retirement_stable_count_decision_v1(owner, roster[index as int]) {
        Err(error) => Err(error),
        Ok(count) => if count != 0 { Err(ReadErrorV1::AllocationBusy) }
            else { retirement_stable_unread_v1(owner, roster, index + 1) },
    } }
}

pub open spec fn retirement_producer_unread_v1(owner: ProducerReadContentsV1, roster: Seq<AllocationReferenceV1>, index: nat)
    -> Result<(), ReadErrorV1>
    decreases roster.len() - index,
{
    if index >= roster.len() { Ok(()) }
    else { match producer_reader_count_decision_v1(owner, roster[index as int]) {
        Err(error) => Err(error),
        Ok(count) => if count != 0 { Err(ReadErrorV1::AllocationBusy) }
            else { retirement_producer_unread_v1(owner, roster, index + 1) },
    } }
}

pub open spec fn retirement_stable_safe_v1(owner: ReadContentsV1, roster: Seq<AllocationReferenceV1>, index: nat) -> bool
    decreases roster.len() - index,
{
    index >= roster.len() || (retirement_stable_count_safe_v1(owner, roster[index as int])
        && (retirement_stable_count_decision_v1(owner, roster[index as int]) == Ok(0)
            ==> retirement_stable_safe_v1(owner, roster, index + 1)))
}

pub open spec fn retirement_producer_safe_v1(owner: ProducerReadContentsV1, roster: Seq<AllocationReferenceV1>, index: nat) -> bool
    decreases roster.len() - index,
{
    index >= roster.len() || (retirement_producer_count_safe_v1(owner, roster[index as int])
        && (producer_reader_count_decision_v1(owner, roster[index as int]) == Ok(0)
            ==> retirement_producer_safe_v1(owner, roster, index + 1)))
}

pub proof fn retirement_producer_to_stable_v1(owner: ProducerReadContentsV1, roster: Seq<AllocationReferenceV1>, index: nat)
    requires retirement_producer_safe_v1(owner, roster, index), retirement_producer_unread_v1(owner, roster, index).is_ok(),
    ensures retirement_stable_safe_v1(owner.stable, roster, index), retirement_stable_unread_v1(owner.stable, roster, index).is_ok(),
    decreases roster.len() - index,
{
    if index < roster.len() {
        retirement_producer_to_stable_v1(owner, roster, index + 1);
    }
}

pub open spec fn retirement_stable_decision_v1(owner: ReadContentsV1, roster: Seq<AllocationReferenceV1>, capacity: usize)
    -> Result<(), ReadErrorV1>
{
    match retirement_stable_unread_v1(owner, roster, 0) {
        Err(error) => Err(error), Ok(()) => retirement_decision_v1(owner.journal, roster, capacity),
    }
}

pub open spec fn retirement_producer_decision_v1(owner: ProducerReadContentsV1, roster: Seq<AllocationReferenceV1>, capacity: usize)
    -> Result<(), ReadErrorV1>
{
    match retirement_producer_unread_v1(owner, roster, 0) {
        Err(error) => Err(error), Ok(()) => retirement_stable_decision_v1(owner.stable, roster, capacity),
    }
}

pub open spec fn retirement_stable_relation_v1(before: ReadContentsV1, after: ReadContentsV1,
    roster: Seq<AllocationReferenceV1>, capacity: usize, result: Result<(), ReadErrorV1>) -> bool
{
    &&& result == retirement_stable_decision_v1(before, roster, capacity)
    &&& retirement_stable_frame_v1(before, after)
    &&& match retirement_stable_unread_v1(before, roster, 0) {
        Err(_) => after == before,
        Ok(()) => retirement_relation_v1(before.journal, after.journal, roster, capacity, result),
    }
    &&& result.is_err() ==> after == before
}

pub open spec fn retirement_producer_relation_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    roster: Seq<AllocationReferenceV1>, capacity: usize, result: Result<(), ReadErrorV1>) -> bool
{
    &&& result == retirement_producer_decision_v1(before, roster, capacity)
    &&& retirement_producer_frame_v1(before, after)
    &&& match retirement_producer_unread_v1(before, roster, 0) {
        Err(_) => after == before,
        Ok(()) => retirement_stable_relation_v1(before.stable, after.stable, roster, capacity, result),
    }
    &&& result.is_err() ==> after == before
}

pub fn retirement_preflight_exec_v1(journal: &JournalContentsV1, roster: &[AllocationReferenceV1], capacity: usize)
    -> (result: Result<(), ReadErrorV1>)
    ensures result == retirement_decision_v1(*journal, roster@, capacity),
{
    if roster.len() > journal.allocation_capacity { return Err(ReadErrorV1::RosterCapacity); }
    let mut previous = None;
    let mut index = 0usize;
    while index < roster.len()
        invariant index <= roster.len(), roster.len() <= journal.allocation_capacity,
            retirement_scan_v1(*journal, roster@, 0, None) == retirement_scan_v1(*journal, roster@, index as nat, previous),
        decreases roster.len() - index,
    {
        let reference = roster[index];
        let entry = match begin_exact_allocation_exec_v1(journal, reference) {
            Err(error) => return Err(error), Ok(entry) => entry,
        };
        if let Some(key) = previous {
            if !enrollment_key_less_exec_v1(key, reference.key) { return Err(ReadErrorV1::NonCanonicalRoster); }
        }
        if entry.pending_member.is_some() { return Err(ReadErrorV1::AllocationBusy); }
        previous = Some(reference.key);
        index += 1;
    }
    let returned = match journal.allocation_free.len().checked_add(roster.len()) {
        Some(count) => count, None => return Err(ReadErrorV1::InvalidState),
    };
    if returned > journal.allocation_capacity || returned > capacity { return Err(ReadErrorV1::InvalidState); }
    Ok(())
}

pub fn retirement_exec_v1(journal: &mut JournalContentsV1, roster: &[AllocationReferenceV1], capacity: usize)
    -> (result: Result<(), ReadErrorV1>)
    ensures retirement_relation_v1(*old(journal), *final(journal), roster@, capacity, result),
{
    match retirement_preflight_exec_v1(journal, roster, capacity) {
        Err(error) => return Err(error), Ok(()) => {},
    }
    let ghost before = *journal;
    proof {
        retirement_ready_v1(before, roster@, capacity);
        assert(retirement_slots_v1(roster@, 0) =~= Seq::<usize>::empty());
    }
    let mut index = 0usize;
    while index < roster.len()
        invariant index <= roster.len(), before == *old(journal),
            retirement_decision_v1(before, roster@, capacity).is_ok(), retirement_journal_frame_v1(before, *journal),
            journal.allocations@.len() == before.allocations@.len(),
            journal.allocations@ == retirement_prefix_v1(before.allocations@, roster@, index as nat),
            journal.allocation_free@ == before.allocation_free@ + retirement_slots_v1(roster@, index as nat),
            forall|i: int| 0 <= i < roster.len() ==> (#[trigger] roster@[i]).slot < journal.allocations@.len(),
            before.allocation_free@.len() + roster.len() <= usize::MAX,
        decreases roster.len() - index,
    {
        proof {
            assert(retirement_slots_v1(roster@, index as nat).push(roster@[index as int].slot)
                =~= retirement_slots_v1(roster@, index as nat + 1));
            assert((before.allocation_free@ + retirement_slots_v1(roster@, index as nat)).push(roster@[index as int].slot)
                =~= before.allocation_free@ + retirement_slots_v1(roster@, index as nat + 1));
        }
        let reference = roster[index];
        journal.allocations.set(reference.slot, None);
        journal.allocation_free.push(reference.slot);
        index += 1;
    }
    Ok(())
}

pub fn retirement_stable_unread_exec_v1(owner: &ReadContentsV1, roster: &[AllocationReferenceV1])
    -> (result: Result<(), ReadErrorV1>)
    requires retirement_stable_safe_v1(*owner, roster@, 0),
    ensures result == retirement_stable_unread_v1(*owner, roster@, 0),
{
    let mut index = 0usize;
    while index < roster.len()
        invariant index <= roster.len(), retirement_stable_safe_v1(*owner, roster@, index as nat),
            retirement_stable_unread_v1(*owner, roster@, 0) == retirement_stable_unread_v1(*owner, roster@, index as nat),
        decreases roster.len() - index,
    {
        let reference = roster[index];
        match allocation_lookup_exec_v1(&owner.journal, reference) {
            Err(error) => return Err(error), Ok(_) => {},
        }
        if owner.readers[reference.slot] != 0 { return Err(ReadErrorV1::AllocationBusy); }
        index += 1;
    }
    Ok(())
}

pub fn retirement_producer_unread_exec_v1(owner: &ProducerReadContentsV1, roster: &[AllocationReferenceV1])
    -> (result: Result<(), ReadErrorV1>)
    requires retirement_producer_safe_v1(*owner, roster@, 0),
    ensures result == retirement_producer_unread_v1(*owner, roster@, 0),
{
    let mut index = 0usize;
    while index < roster.len()
        invariant index <= roster.len(), retirement_producer_safe_v1(*owner, roster@, index as nat),
            retirement_producer_unread_v1(*owner, roster@, 0) == retirement_producer_unread_v1(*owner, roster@, index as nat),
        decreases roster.len() - index,
    {
        let reference = roster[index];
        match allocation_lookup_exec_v1(&owner.stable.journal, reference) {
            Err(error) => return Err(error), Ok(_) => {},
        }
        if owner.stable.readers[reference.slot] + owner.counts[reference.slot] != 0 {
            return Err(ReadErrorV1::AllocationBusy);
        }
        index += 1;
    }
    Ok(())
}

pub fn retirement_stable_validate_exec_v1(owner: &ReadContentsV1, roster: &[AllocationReferenceV1], capacity: usize)
    -> (result: Result<(), ReadErrorV1>)
    requires retirement_stable_safe_v1(*owner, roster@, 0),
    ensures result == retirement_stable_decision_v1(*owner, roster@, capacity),
{
    match retirement_stable_unread_exec_v1(owner, roster) {
        Err(error) => return Err(error), Ok(()) => {},
    }
    retirement_preflight_exec_v1(&owner.journal, roster, capacity)
}

pub fn retirement_stable_exec_v1(owner: &mut ReadContentsV1, roster: &[AllocationReferenceV1], capacity: usize)
    -> (result: Result<(), ReadErrorV1>)
    requires retirement_stable_safe_v1(*old(owner), roster@, 0),
    ensures retirement_stable_relation_v1(*old(owner), *final(owner), roster@, capacity, result),
{
    match retirement_stable_validate_exec_v1(owner, roster, capacity) {
        Err(error) => return Err(error), Ok(()) => {},
    }
    retirement_exec_v1(&mut owner.journal, roster, capacity)
}

pub fn retirement_producer_validate_exec_v1(owner: &ProducerReadContentsV1, roster: &[AllocationReferenceV1], capacity: usize)
    -> (result: Result<(), ReadErrorV1>)
    requires retirement_producer_safe_v1(*owner, roster@, 0),
    ensures result == retirement_producer_decision_v1(*owner, roster@, capacity),
{
    match retirement_producer_unread_exec_v1(owner, roster) {
        Err(error) => return Err(error), Ok(()) => {},
    }
    proof { retirement_producer_to_stable_v1(*owner, roster@, 0); }
    retirement_stable_validate_exec_v1(&owner.stable, roster, capacity)
}

pub fn retirement_producer_exec_v1(owner: &mut ProducerReadContentsV1, roster: &[AllocationReferenceV1], capacity: usize)
    -> (result: Result<(), ReadErrorV1>)
    requires retirement_producer_safe_v1(*old(owner), roster@, 0),
    ensures retirement_producer_relation_v1(*old(owner), *final(owner), roster@, capacity, result),
{
    match retirement_producer_validate_exec_v1(owner, roster, capacity) {
        Err(error) => return Err(error), Ok(()) => {},
    }
    proof { retirement_producer_to_stable_v1(*owner, roster@, 0); }
    retirement_stable_exec_v1(&mut owner.stable, roster, capacity)
}

}
