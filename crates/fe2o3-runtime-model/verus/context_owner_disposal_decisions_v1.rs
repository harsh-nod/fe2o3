verus! {

spec fn disposal_roster_scan_v1(journal: JournalContentsV1, roster: Seq<AllocationWriteV1>,
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

spec fn disposal_return_decision_v1(journal: JournalContentsV1, count: usize,
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

spec fn disposal_plan_decision_v1(journal: JournalContentsV1, writer: WriterReferenceV1,
    roster: Seq<AllocationWriteV1>, writer_storage: usize, member_storage: usize, allocation_storage: usize)
    -> Result<(Option<usize>, usize), ReadErrorV1>
{
    match owner_retained_header_v1(journal, writer, true) {
        Err(error) => Err(error),
        Ok((head, count, unknown)) => if !unknown { Err(ReadErrorV1::InvalidState) }
            else if roster.len() != count { Err(ReadErrorV1::SettlementEvidenceMismatch) }
            else { match owner_retained_chain_v1(journal, writer, head, count) {
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

spec fn disposal_validate_decision_v1(journal: JournalContentsV1, writer: WriterReferenceV1,
    roster: Seq<AllocationWriteV1>, writer_storage: usize, member_storage: usize, allocation_storage: usize)
    -> Result<(), ReadErrorV1>
{
    match disposal_plan_decision_v1(journal, writer, roster, writer_storage, member_storage, allocation_storage) {
        Err(error) => Err(error), Ok(_) => Ok(()),
    }
}

spec fn disposal_execute_decision_v1(journal: JournalContentsV1, writer: WriterReferenceV1,
    evidence: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
    writer_storage: usize, member_storage: usize, allocation_storage: usize)
    -> Result<(Option<usize>, usize), ReadErrorV1>
{
    match owner_retained_header_v1(journal, writer, true) {
        Err(error) => Err(error),
        Ok(_) => if evidence != writer { Err(ReadErrorV1::SettlementEvidenceMismatch) }
            else { disposal_plan_decision_v1(journal, writer, roster, writer_storage, member_storage, allocation_storage) },
    }
}

spec fn disposal_ready_v1(before: JournalContentsV1, head: Option<usize>, count: usize) -> bool {
    &&& settlement_storage_ready_v1(before, head, count)
    &&& before.allocation_free@.len() + count <= usize::MAX
}

proof fn disposal_preflight_ready_v1(before: JournalContentsV1, writer: WriterReferenceV1,
    roster: Seq<AllocationWriteV1>, writer_storage: usize, member_storage: usize, allocation_storage: usize,
    head: Option<usize>, count: usize)
    requires disposal_plan_decision_v1(before, writer, roster, writer_storage, member_storage, allocation_storage) == Ok((head, count)),
    ensures disposal_ready_v1(before, head, count), writer.slot < before.writers@.len(),
        owner_retained_header_v1(before, writer, true) == Ok((head, count, true)),
        owner_retained_scan_v1(before, writer, head, count as nat, None) == Ok(()),
        roster.len() == count, disposal_roster_scan_v1(before, roster, head, 0) == Ok(()),
        before.free@.len() < before.writer_capacity,
        before.member_free@.len() + count <= before.allocation_capacity,
        before.allocation_free@.len() + count <= before.allocation_capacity,
{
    assert(owner_retained_header_v1(before, writer, true) == Ok((head, count, true)));
    match owner_retained_chain_v1(before, writer, head, count) {
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

spec fn disposal_refs_v1(before: JournalContentsV1, head: Option<usize>, count: nat) -> Seq<AllocationReferenceV1> {
    Seq::new(count, |i: int| settlement_plan_at_v1(before, head, i as nat).allocation)
}

proof fn disposal_prefix_shapes_v1(before: JournalContentsV1, head: Option<usize>, total: usize, count: nat)
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

spec fn disposal_frame_v1(before: JournalContentsV1, after: JournalContentsV1) -> bool {
    &&& after.context_generation == before.context_generation
    &&& after.allocation_capacity == before.allocation_capacity
    &&& after.writer_capacity == before.writer_capacity
    &&& after.registration_watermark == before.registration_watermark
    &&& after.reserved_count == before.reserved_count
}

spec fn disposal_success_v1(before: JournalContentsV1, after: JournalContentsV1,
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

spec fn disposal_relation_v1(before: JournalContentsV1, after: JournalContentsV1, writer: WriterReferenceV1,
    evidence: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
    writer_storage: usize, member_storage: usize, allocation_storage: usize, result: Result<(), ReadErrorV1>) -> bool
{
    match disposal_execute_decision_v1(before, writer, evidence, roster, writer_storage, member_storage, allocation_storage) {
        Err(error) => result == Err(error) && after == before,
        Ok((head, count)) => result == Ok(()) && disposal_success_v1(before, after, writer, head, count),
    }
}

spec fn disposal_stable_safe_v1(owner: ContextReadLeasedJournalV1, roster: Seq<AllocationWriteV1>, index: nat) -> bool
    decreases roster.len() - index,
{
    index >= roster.len() || (stable_count_safe_v1(owner, roster[index as int].allocation)
        && (stable_count_decision_v1(owner, roster[index as int].allocation) == Ok(0)
            ==> disposal_stable_safe_v1(owner, roster, index + 1)))
}

spec fn disposal_producer_safe_v1(owner: ContextProducerReadJournalV1, roster: Seq<AllocationWriteV1>, index: nat) -> bool
    decreases roster.len() - index,
{
    index >= roster.len() || (producer_count_safe_v1(owner, roster[index as int].allocation)
        && (producer_count_decision_v1(owner, roster[index as int].allocation) == Ok(0)
            ==> disposal_producer_safe_v1(owner, roster, index + 1)))
}

proof fn disposal_producer_to_stable_v1(owner: ContextProducerReadJournalV1, roster: Seq<AllocationWriteV1>, index: nat)
    requires disposal_producer_safe_v1(owner, roster, index), producer_unread_writes_v1(owner, roster, index).is_ok(),
    ensures disposal_stable_safe_v1(owner.stable, roster, index), stable_unread_writes_v1(owner.stable, roster, index).is_ok(),
    decreases roster.len() - index,
{
    if index < roster.len() { disposal_producer_to_stable_v1(owner, roster, index + 1); }
}

spec fn disposal_stable_validate_v1(owner: ContextReadLeasedJournalV1, writer: WriterReferenceV1,
    roster: Seq<AllocationWriteV1>, writer_storage: usize, member_storage: usize, allocation_storage: usize)
    -> Result<(), ReadErrorV1>
{
    match stable_unread_writes_v1(owner, roster, 0) {
        Err(error) => Err(error),
        Ok(()) => disposal_validate_decision_v1(owner.journal, writer, roster, writer_storage, member_storage, allocation_storage),
    }
}

spec fn disposal_producer_validate_v1(owner: ContextProducerReadJournalV1, writer: WriterReferenceV1,
    roster: Seq<AllocationWriteV1>, writer_storage: usize, member_storage: usize, allocation_storage: usize)
    -> Result<(), ReadErrorV1>
{
    match producer_unread_writes_v1(owner, roster, 0) {
        Err(error) => Err(error),
        Ok(()) => disposal_stable_validate_v1(owner.stable, writer, roster, writer_storage, member_storage, allocation_storage),
    }
}

spec fn disposal_stable_relation_v1(before: ContextReadLeasedJournalV1, after: ContextReadLeasedJournalV1,
    writer: WriterReferenceV1, evidence: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
    writer_storage: usize, member_storage: usize, allocation_storage: usize, result: Result<(), ReadErrorV1>) -> bool
{
    &&& owner_writer_stable_frame_v1(before, after)
    &&& match disposal_stable_validate_v1(before, writer, roster, writer_storage, member_storage, allocation_storage) {
        Err(error) => result == Err(error) && after == before,
        Ok(()) => disposal_relation_v1(before.journal, after.journal, writer, evidence, roster,
            writer_storage, member_storage, allocation_storage, result),
    }
    &&& result.is_err() ==> after == before
}

spec fn disposal_producer_relation_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    writer: WriterReferenceV1, evidence: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
    writer_storage: usize, member_storage: usize, allocation_storage: usize, result: Result<(), ReadErrorV1>) -> bool
{
    &&& owner_writer_producer_frame_v1(before, after)
    &&& match disposal_producer_validate_v1(before, writer, roster, writer_storage, member_storage, allocation_storage) {
        Err(error) => result == Err(error) && after == before,
        Ok(()) => disposal_stable_relation_v1(before.stable, after.stable, writer, evidence, roster,
            writer_storage, member_storage, allocation_storage, result),
    }
    &&& result.is_err() ==> after == before
}

}
