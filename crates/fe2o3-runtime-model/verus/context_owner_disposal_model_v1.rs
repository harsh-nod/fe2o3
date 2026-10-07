// Independent logical disposal contracts and execution; no runtime-body macros.
use super::*;

verus! {

pub open spec fn disposal_stable_unread_v1(owner: ReadContentsV1, roster: Seq<AllocationWriteV1>, index: nat)
    -> Result<(), ReadErrorV1>
    decreases roster.len() - index,
{
    if index >= roster.len() { Ok(()) }
    else { match retirement_stable_count_decision_v1(owner, roster[index as int].allocation) {
        Err(error) => Err(error),
        Ok(count) => if count != 0 { Err(ReadErrorV1::AllocationBusy) }
            else { disposal_stable_unread_v1(owner, roster, index + 1) },
    } }
}

pub open spec fn disposal_producer_unread_v1(owner: ProducerReadContentsV1, roster: Seq<AllocationWriteV1>, index: nat)
    -> Result<(), ReadErrorV1>
    decreases roster.len() - index,
{
    if index >= roster.len() { Ok(()) }
    else { match producer_reader_count_decision_v1(owner, roster[index as int].allocation) {
        Err(error) => Err(error),
        Ok(count) => if count != 0 { Err(ReadErrorV1::AllocationBusy) }
            else { disposal_producer_unread_v1(owner, roster, index + 1) },
    } }
}

pub fn disposal_return_exec_v1(journal: &JournalContentsV1, count: usize,
    writer_storage: usize, member_storage: usize, allocation_storage: usize) -> (result: Result<(), ReadErrorV1>)
    ensures result == disposal_return_decision_v1(*journal, count, writer_storage, member_storage, allocation_storage),
{
    let writers = match journal.free.len().checked_add(1) { Some(n) => n, None => return Err(ReadErrorV1::InvalidState) };
    let members = match journal.member_free.len().checked_add(count) { Some(n) => n, None => return Err(ReadErrorV1::InvalidState) };
    let allocations = match journal.allocation_free.len().checked_add(count) { Some(n) => n, None => return Err(ReadErrorV1::InvalidState) };
    if writers > journal.writer_capacity || writers > writer_storage
        || members > journal.allocation_capacity || members > member_storage
        || allocations > journal.allocation_capacity || allocations > allocation_storage || count > journal.scratch.len() {
        return Err(ReadErrorV1::InvalidState);
    }
    let mut index = 0usize;
    while index < count
        invariant index <= count, count <= journal.scratch@.len(),
            journal.free@.len() + 1 <= journal.writer_capacity,
            journal.free@.len() + 1 <= writer_storage,
            journal.member_free@.len() + count <= journal.allocation_capacity,
            journal.member_free@.len() + count <= member_storage,
            journal.allocation_free@.len() + count <= journal.allocation_capacity,
            journal.allocation_free@.len() + count <= allocation_storage,
            settlement_scratch_scan_v1(*journal, count, 0) == settlement_scratch_scan_v1(*journal, count, index as nat),
        decreases count - index,
    {
        if journal.scratch[index].is_some() { return Err(ReadErrorV1::InvalidState); }
        index += 1;
    }
    Ok(())
}

pub fn disposal_plan_exec_v1(journal: &JournalContentsV1, writer: WriterReferenceV1, roster: &[AllocationWriteV1],
    writer_storage: usize, member_storage: usize, allocation_storage: usize)
    -> (result: Result<(Option<usize>, usize), ReadErrorV1>)
    ensures result == disposal_plan_decision_v1(*journal, writer, roster@, writer_storage, member_storage, allocation_storage),
{
    let (head, count, unknown) = match retained_header_exec_v1(journal, writer, true) {
        Ok(value) => value, Err(error) => return Err(error),
    };
    if !unknown { return Err(ReadErrorV1::InvalidState); }
    if roster.len() != count { return Err(ReadErrorV1::SettlementEvidenceMismatch); }
    match retained_chain_exec_v1(journal, writer, head, count) {
        Ok(value) => { assert(value == ()); }, Err(error) => return Err(error),
    }
    let mut cursor = head;
    let mut index = 0usize;
    while index < count
        invariant index <= count, roster.len() == count,
            retained_header_decision_v1(*journal, writer, true) == Ok((head, count, true)),
            retained_chain_decision_v1(*journal, writer, head, count) == Ok(()),
            retained_scan_v1(*journal, writer, head, count as nat, None) == Ok(()),
            cursor == settlement_cursor_v1(*journal, head, index as nat),
            disposal_roster_scan_v1(*journal, roster@, head, 0) == disposal_roster_scan_v1(*journal, roster@, cursor, index as nat),
        decreases count - index,
    {
        proof { settlement_scan_plan_v1(*journal, writer, head, count, index as nat); }
        let slot = cursor.unwrap();
        let member = journal.members[slot].unwrap();
        let allocation = match begin_exact_allocation_exec_v1(journal, member.allocation) {
            Ok(value) => value, Err(error) => return Err(error),
        };
        let entry = roster[index];
        if !same_allocation_exec_v1(entry.allocation, member.allocation)
            || entry.device.context_generation != allocation.device.context_generation
            || entry.device.local != allocation.device.local || entry.byte_extent != allocation.byte_extent {
            return Err(ReadErrorV1::SettlementEvidenceMismatch);
        }
        cursor = member.next;
        index += 1;
    }
    match disposal_return_exec_v1(journal, count, writer_storage, member_storage, allocation_storage) {
        Ok(value) => { assert(value == ()); }, Err(error) => return Err(error),
    }
    Ok((head, count))
}

pub fn disposal_commit_exec_v1(journal: &mut JournalContentsV1, writer: WriterReferenceV1,
    count: usize, head: Option<usize>, Ghost(before): Ghost<JournalContentsV1>)
    requires disposal_ready_v1(before, head, count), writer.slot < before.writers@.len(),
        begin_stage_frame_v1(before, *old(journal)),
        old(journal).scratch@ == settlement_scratch_v1(before, head, count, count as nat, 0),
    ensures disposal_success_v1(before, *final(journal), writer, head, count),
{
    proof {
        disposal_prefix_shapes_v1(before, head, count, 0);
        assert(journal.member_free@ =~= before.member_free@ + settlement_slots_v1(before, head, 0));
        assert(journal.allocation_free@ =~= before.allocation_free@ + retirement_slots_v1(disposal_refs_v1(before, head, count as nat), 0));
        if count > 0 { assert(journal.scratch@[0] == Some(settlement_plan_at_v1(before, head, 0))); }
    }
    let mut index = 0usize;
    while index < count
        invariant index <= count, disposal_ready_v1(before, head, count), writer.slot < before.writers@.len(),
            disposal_frame_v1(before, *journal), journal.writers == before.writers, journal.free == before.free,
            forall|i: int| 0 <= i < journal.scratch@.len() ==> (#[trigger] journal.scratch@[i])
                == settlement_scratch_v1(before, head, count, count as nat, index as nat)[i],
            index < count ==> journal.scratch@[index as int] == Some(settlement_plan_at_v1(before, head, index as nat)),
            journal.allocations@ =~= retirement_prefix_v1(before.allocations@, disposal_refs_v1(before, head, count as nat), index as nat),
            journal.allocation_free@ =~= before.allocation_free@ + retirement_slots_v1(disposal_refs_v1(before, head, count as nat), index as nat),
            journal.members@ =~= settlement_members_prefix_v1(before, head, index as nat),
            journal.member_free@ =~= before.member_free@ + settlement_slots_v1(before, head, index as nat),
            journal.members@.len() == before.members@.len(), journal.allocations@.len() == before.allocations@.len(),
            journal.scratch@.len() == before.scratch@.len(),
            index < count ==> settlement_plan_ready_v1(before, head, index as nat),
        decreases count - index,
    {
        let plan = journal.scratch[index].take().unwrap();
        journal.allocations[plan.allocation.slot] = None;
        journal.allocation_free.push(plan.allocation.slot);
        journal.members[plan.member_slot] = None;
        journal.member_free.push(plan.member_slot);
        index += 1;
    }
    journal.writers[writer.slot] = None;
    journal.free.push(writer.slot);
    proof { assert(journal.scratch@ =~= before.scratch@); }
}

pub fn disposal_journal_exec_v1(journal: &mut JournalContentsV1, writer: WriterReferenceV1,
    evidence: WriterReferenceV1, roster: &[AllocationWriteV1],
    writer_storage: usize, member_storage: usize, allocation_storage: usize) -> (result: Result<(), ReadErrorV1>)
    ensures disposal_relation_v1(*old(journal), *final(journal), writer, evidence, roster@,
        writer_storage, member_storage, allocation_storage, result),
{
    let ghost before = *journal;
    if let Err(error) = retained_header_exec_v1(journal, writer, true) { return Err(error); }
    if !same_producer_exec_v1(writer, evidence) { return Err(ReadErrorV1::SettlementEvidenceMismatch); }
    let (head, count) = match disposal_plan_exec_v1(journal, writer, roster, writer_storage, member_storage, allocation_storage) {
        Ok(value) => value, Err(error) => return Err(error),
    };
    proof { disposal_preflight_ready_v1(before, writer, roster@, writer_storage, member_storage, allocation_storage, head, count); }
    settlement_stage_exec_v1(journal, head, count);
    disposal_commit_exec_v1(journal, writer, count, head, Ghost(before));
    Ok(())
}

pub fn disposal_stable_unread_exec_v1(owner: &ReadContentsV1, roster: &[AllocationWriteV1]) -> (result: Result<(), ReadErrorV1>)
    requires disposal_stable_safe_v1(*owner, roster@, 0),
    ensures result == disposal_stable_unread_v1(*owner, roster@, 0),
{
    let mut index = 0usize;
    while index < roster.len()
        invariant index <= roster.len(), disposal_stable_safe_v1(*owner, roster@, index as nat),
            disposal_stable_unread_v1(*owner, roster@, 0) == disposal_stable_unread_v1(*owner, roster@, index as nat),
        decreases roster.len() - index,
    {
        let reference = roster[index].allocation;
        if let Err(error) = allocation_lookup_exec_v1(&owner.journal, reference) { return Err(error); }
        if owner.readers[reference.slot] != 0 { return Err(ReadErrorV1::AllocationBusy); }
        index += 1;
    }
    Ok(())
}

pub fn disposal_producer_unread_exec_v1(owner: &ProducerReadContentsV1, roster: &[AllocationWriteV1]) -> (result: Result<(), ReadErrorV1>)
    requires disposal_producer_safe_v1(*owner, roster@, 0),
    ensures result == disposal_producer_unread_v1(*owner, roster@, 0),
{
    let mut index = 0usize;
    while index < roster.len()
        invariant index <= roster.len(), disposal_producer_safe_v1(*owner, roster@, index as nat),
            disposal_producer_unread_v1(*owner, roster@, 0) == disposal_producer_unread_v1(*owner, roster@, index as nat),
        decreases roster.len() - index,
    {
        let reference = roster[index].allocation;
        if let Err(error) = allocation_lookup_exec_v1(&owner.stable.journal, reference) { return Err(error); }
        let count = owner.stable.readers[reference.slot] + owner.counts[reference.slot];
        if count != 0 { return Err(ReadErrorV1::AllocationBusy); }
        index += 1;
    }
    Ok(())
}

pub fn disposal_stable_validate_exec_v1(owner: &ReadContentsV1, writer: WriterReferenceV1, roster: &[AllocationWriteV1],
    writer_storage: usize, member_storage: usize, allocation_storage: usize) -> (result: Result<(), ReadErrorV1>)
    requires disposal_stable_safe_v1(*owner, roster@, 0),
    ensures result == disposal_stable_validate_v1(*owner, writer, roster@, writer_storage, member_storage, allocation_storage),
{
    if let Err(error) = disposal_stable_unread_exec_v1(owner, roster) { return Err(error); }
    match disposal_plan_exec_v1(&owner.journal, writer, roster, writer_storage, member_storage, allocation_storage) {
        Ok(_) => Ok(()), Err(error) => Err(error),
    }
}

pub fn disposal_producer_validate_exec_v1(owner: &ProducerReadContentsV1, writer: WriterReferenceV1, roster: &[AllocationWriteV1],
    writer_storage: usize, member_storage: usize, allocation_storage: usize) -> (result: Result<(), ReadErrorV1>)
    requires disposal_producer_safe_v1(*owner, roster@, 0),
    ensures result == disposal_producer_validate_v1(*owner, writer, roster@, writer_storage, member_storage, allocation_storage),
{
    if let Err(error) = disposal_producer_unread_exec_v1(owner, roster) { return Err(error); }
    proof { disposal_producer_to_stable_v1(*owner, roster@, 0); }
    disposal_stable_validate_exec_v1(&owner.stable, writer, roster, writer_storage, member_storage, allocation_storage)
}

pub fn disposal_stable_exec_v1(owner: &mut ReadContentsV1, writer: WriterReferenceV1, evidence: WriterReferenceV1,
    roster: &[AllocationWriteV1], writer_storage: usize, member_storage: usize, allocation_storage: usize)
    -> (result: Result<(), ReadErrorV1>)
    requires disposal_stable_safe_v1(*old(owner), roster@, 0),
    ensures disposal_stable_relation_v1(*old(owner), *final(owner), writer, evidence, roster@,
        writer_storage, member_storage, allocation_storage, result),
{
    if let Err(error) = disposal_stable_validate_exec_v1(owner, writer, roster, writer_storage, member_storage, allocation_storage) { return Err(error); }
    disposal_journal_exec_v1(&mut owner.journal, writer, evidence, roster, writer_storage, member_storage, allocation_storage)
}

pub fn disposal_producer_exec_v1(owner: &mut ProducerReadContentsV1, writer: WriterReferenceV1, evidence: WriterReferenceV1,
    roster: &[AllocationWriteV1], writer_storage: usize, member_storage: usize, allocation_storage: usize)
    -> (result: Result<(), ReadErrorV1>)
    requires disposal_producer_safe_v1(*old(owner), roster@, 0),
    ensures disposal_producer_relation_v1(*old(owner), *final(owner), writer, evidence, roster@,
        writer_storage, member_storage, allocation_storage, result),
{
    if let Err(error) = disposal_producer_validate_exec_v1(owner, writer, roster, writer_storage, member_storage, allocation_storage) { return Err(error); }
    proof { disposal_producer_to_stable_v1(*owner, roster@, 0); }
    disposal_stable_exec_v1(&mut owner.stable, writer, evidence, roster, writer_storage, member_storage, allocation_storage)
}

pub open spec fn disposal_roster_scan_v1(journal: JournalContentsV1, roster: Seq<AllocationWriteV1>,
    cursor: Option<usize>, index: nat) -> Result<(), ReadErrorV1>
    decreases roster.len() - index,
{
    if index >= roster.len() { Ok(()) }
    else { match cursor {
        None => Err(ReadErrorV1::InvalidState),
        Some(slot) => if slot >= journal.members@.len() { Err(ReadErrorV1::InvalidState) }
            else { match journal.members@[slot as int] {
                None => Err(ReadErrorV1::InvalidState),
                Some(member) => match begin_exact_allocation_v1(journal, member.allocation) {
                    Err(error) => Err(error),
                    Ok(allocation) => if roster[index as int].allocation != member.allocation
                        || roster[index as int].device != allocation.device
                        || roster[index as int].byte_extent != allocation.byte_extent {
                        Err(ReadErrorV1::SettlementEvidenceMismatch)
                    } else { disposal_roster_scan_v1(journal, roster, member.next, index + 1) },
                },
            } },
    } }
}

pub open spec fn disposal_return_decision_v1(journal: JournalContentsV1, count: usize,
    writer_storage: usize, member_storage: usize, allocation_storage: usize) -> Result<(), ReadErrorV1>
{
    let writers = journal.free@.len() + 1;
    let members = journal.member_free@.len() + count;
    let allocations = journal.allocation_free@.len() + count;
    if writers > usize::MAX || members > usize::MAX || allocations > usize::MAX
        || writers > journal.writer_capacity || writers > writer_storage
        || members > journal.allocation_capacity || members > member_storage
        || allocations > journal.allocation_capacity || allocations > allocation_storage
        || count > journal.scratch@.len() { Err(ReadErrorV1::InvalidState) }
    else { settlement_scratch_scan_v1(journal, count, 0) }
}

pub open spec fn disposal_plan_decision_v1(journal: JournalContentsV1, writer: WriterReferenceV1,
    roster: Seq<AllocationWriteV1>, writer_storage: usize, member_storage: usize, allocation_storage: usize)
    -> Result<(Option<usize>, usize), ReadErrorV1>
{
    match retained_header_decision_v1(journal, writer, true) {
        Err(error) => Err(error),
        Ok((head, count, unknown)) => if !unknown { Err(ReadErrorV1::InvalidState) }
            else if roster.len() != count { Err(ReadErrorV1::SettlementEvidenceMismatch) }
            else { match retained_chain_decision_v1(journal, writer, head, count) {
                Err(error) => Err(error),
                Ok(()) => match disposal_roster_scan_v1(journal, roster, head, 0) {
                    Err(error) => Err(error),
                    Ok(()) => match disposal_return_decision_v1(journal, count, writer_storage, member_storage, allocation_storage) {
                        Err(error) => Err(error), Ok(()) => Ok((head, count)),
                    },
                },
            } },
    }
}

pub open spec fn disposal_validate_decision_v1(journal: JournalContentsV1, writer: WriterReferenceV1,
    roster: Seq<AllocationWriteV1>, writer_storage: usize, member_storage: usize, allocation_storage: usize)
    -> Result<(), ReadErrorV1>
{
    match disposal_plan_decision_v1(journal, writer, roster, writer_storage, member_storage, allocation_storage) {
        Err(error) => Err(error), Ok(_) => Ok(()),
    }
}

pub open spec fn disposal_execute_decision_v1(journal: JournalContentsV1, writer: WriterReferenceV1,
    evidence: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
    writer_storage: usize, member_storage: usize, allocation_storage: usize)
    -> Result<(Option<usize>, usize), ReadErrorV1>
{
    match retained_header_decision_v1(journal, writer, true) {
        Err(error) => Err(error),
        Ok(_) => if evidence != writer { Err(ReadErrorV1::SettlementEvidenceMismatch) }
            else { disposal_plan_decision_v1(journal, writer, roster, writer_storage, member_storage, allocation_storage) },
    }
}

pub open spec fn disposal_ready_v1(before: JournalContentsV1, head: Option<usize>, count: usize) -> bool {
    &&& settlement_storage_ready_v1(before, head, count)
    &&& before.allocation_free@.len() + count <= usize::MAX
}

pub proof fn disposal_preflight_ready_v1(before: JournalContentsV1, writer: WriterReferenceV1,
    roster: Seq<AllocationWriteV1>, writer_storage: usize, member_storage: usize, allocation_storage: usize,
    head: Option<usize>, count: usize)
    requires disposal_plan_decision_v1(before, writer, roster, writer_storage, member_storage, allocation_storage) == Ok((head, count)),
    ensures disposal_ready_v1(before, head, count), writer.slot < before.writers@.len(),
        retained_header_decision_v1(before, writer, true) == Ok((head, count, true)),
        retained_scan_v1(before, writer, head, count as nat, None) == Ok(()),
        roster.len() == count, disposal_roster_scan_v1(before, roster, head, 0) == Ok(()),
        before.free@.len() < before.writer_capacity,
        before.member_free@.len() + count <= before.allocation_capacity,
        before.allocation_free@.len() + count <= before.allocation_capacity,
{
    assert(retained_header_decision_v1(before, writer, true) == Ok((head, count, true)));
    match retained_chain_decision_v1(before, writer, head, count) {
        Ok(value) => { assert(value == ()); }, Err(_) => {},
    }
    match disposal_roster_scan_v1(before, roster, head, 0) {
        Ok(value) => { assert(value == ()); }, Err(_) => {},
    }
    match disposal_return_decision_v1(before, count, writer_storage, member_storage, allocation_storage) {
        Ok(value) => { assert(value == ()); }, Err(_) => {},
    }
    assert forall|i: nat| i < count implies #[trigger] settlement_plan_ready_v1(before, head, i) by {
        settlement_scan_plan_v1(before, writer, head, count, i);
    }
    assert forall|i: int| 0 <= i < count implies (#[trigger] before.scratch@[i]).is_none() by {
        settlement_scratch_at_v1(before, count, 0, i as nat);
    }
}

pub open spec fn disposal_refs_v1(before: JournalContentsV1, head: Option<usize>, count: nat) -> Seq<AllocationReferenceV1> {
    Seq::new(count, |i: int| settlement_plan_at_v1(before, head, i as nat).allocation)
}

pub proof fn disposal_prefix_shapes_v1(before: JournalContentsV1, head: Option<usize>, total: usize, count: nat)
    requires disposal_ready_v1(before, head, total), count <= total,
    ensures settlement_members_prefix_v1(before, head, count).len() == before.members@.len(),
        retirement_prefix_v1(before.allocations@, disposal_refs_v1(before, head, total as nat), count).len()
            == before.allocations@.len(),
    decreases count,
{
    if count > 0 {
        disposal_prefix_shapes_v1(before, head, total, (count - 1) as nat);
        assert(settlement_plan_ready_v1(before, head, (count - 1) as nat));
    }
}

pub open spec fn disposal_frame_v1(before: JournalContentsV1, after: JournalContentsV1) -> bool {
    &&& after.context_generation == before.context_generation
    &&& after.allocation_capacity == before.allocation_capacity
    &&& after.writer_capacity == before.writer_capacity
    &&& after.registration_watermark == before.registration_watermark
    &&& after.reserved_count == before.reserved_count
}

pub open spec fn disposal_success_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, head: Option<usize>, count: usize) -> bool
{
    &&& disposal_frame_v1(before, after)
    &&& after.writers@ == before.writers@.update(writer.slot as int, None)
    &&& after.free@ == before.free@.push(writer.slot)
    &&& after.allocations@ == retirement_prefix_v1(before.allocations@, disposal_refs_v1(before, head, count as nat), count as nat)
    &&& after.allocation_free@ == before.allocation_free@ + retirement_slots_v1(disposal_refs_v1(before, head, count as nat), count as nat)
    &&& after.members@ == settlement_members_prefix_v1(before, head, count as nat)
    &&& after.member_free@ == before.member_free@ + settlement_slots_v1(before, head, count as nat)
    &&& after.scratch@ == before.scratch@
}

pub open spec fn disposal_relation_v1(before: JournalContentsV1, after: JournalContentsV1, writer: WriterReferenceV1,
    evidence: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
    writer_storage: usize, member_storage: usize, allocation_storage: usize, result: Result<(), ReadErrorV1>) -> bool
{
    match disposal_execute_decision_v1(before, writer, evidence, roster, writer_storage, member_storage, allocation_storage) {
        Err(error) => result == Err(error) && after == before,
        Ok((head, count)) => result == Ok(()) && disposal_success_v1(before, after, writer, head, count),
    }
}

pub open spec fn disposal_stable_safe_v1(owner: ReadContentsV1, roster: Seq<AllocationWriteV1>, index: nat) -> bool
    decreases roster.len() - index,
{
    index >= roster.len() || (retirement_stable_count_safe_v1(owner, roster[index as int].allocation)
        && (retirement_stable_count_decision_v1(owner, roster[index as int].allocation) == Ok(0)
            ==> disposal_stable_safe_v1(owner, roster, index + 1)))
}

pub open spec fn disposal_producer_safe_v1(owner: ProducerReadContentsV1, roster: Seq<AllocationWriteV1>, index: nat) -> bool
    decreases roster.len() - index,
{
    index >= roster.len() || (retirement_producer_count_safe_v1(owner, roster[index as int].allocation)
        && (producer_reader_count_decision_v1(owner, roster[index as int].allocation) == Ok(0)
            ==> disposal_producer_safe_v1(owner, roster, index + 1)))
}

pub proof fn disposal_producer_to_stable_v1(owner: ProducerReadContentsV1, roster: Seq<AllocationWriteV1>, index: nat)
    requires disposal_producer_safe_v1(owner, roster, index), disposal_producer_unread_v1(owner, roster, index).is_ok(),
    ensures disposal_stable_safe_v1(owner.stable, roster, index), disposal_stable_unread_v1(owner.stable, roster, index).is_ok(),
    decreases roster.len() - index,
{
    if index < roster.len() { disposal_producer_to_stable_v1(owner, roster, index + 1); }
}

pub open spec fn disposal_stable_validate_v1(owner: ReadContentsV1, writer: WriterReferenceV1,
    roster: Seq<AllocationWriteV1>, writer_storage: usize, member_storage: usize, allocation_storage: usize)
    -> Result<(), ReadErrorV1>
{
    match disposal_stable_unread_v1(owner, roster, 0) {
        Err(error) => Err(error),
        Ok(()) => disposal_validate_decision_v1(owner.journal, writer, roster, writer_storage, member_storage, allocation_storage),
    }
}

pub open spec fn disposal_producer_validate_v1(owner: ProducerReadContentsV1, writer: WriterReferenceV1,
    roster: Seq<AllocationWriteV1>, writer_storage: usize, member_storage: usize, allocation_storage: usize)
    -> Result<(), ReadErrorV1>
{
    match disposal_producer_unread_v1(owner, roster, 0) {
        Err(error) => Err(error),
        Ok(()) => disposal_stable_validate_v1(owner.stable, writer, roster, writer_storage, member_storage, allocation_storage),
    }
}

pub open spec fn disposal_stable_relation_v1(before: ReadContentsV1, after: ReadContentsV1,
    writer: WriterReferenceV1, evidence: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
    writer_storage: usize, member_storage: usize, allocation_storage: usize, result: Result<(), ReadErrorV1>) -> bool
{
    &&& retirement_stable_frame_v1(before, after)
    &&& match disposal_stable_validate_v1(before, writer, roster, writer_storage, member_storage, allocation_storage) {
        Err(error) => result == Err(error) && after == before,
        Ok(()) => disposal_relation_v1(before.journal, after.journal, writer, evidence, roster,
            writer_storage, member_storage, allocation_storage, result),
    }
    &&& result.is_err() ==> after == before
}

pub open spec fn disposal_producer_relation_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    writer: WriterReferenceV1, evidence: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
    writer_storage: usize, member_storage: usize, allocation_storage: usize, result: Result<(), ReadErrorV1>) -> bool
{
    &&& retirement_producer_frame_v1(before, after)
    &&& match disposal_producer_validate_v1(before, writer, roster, writer_storage, member_storage, allocation_storage) {
        Err(error) => result == Err(error) && after == before,
        Ok(()) => disposal_stable_relation_v1(before.stable, after.stable, writer, evidence, roster,
            writer_storage, member_storage, allocation_storage, result),
    }
    &&& result.is_err() ==> after == before
}

}
