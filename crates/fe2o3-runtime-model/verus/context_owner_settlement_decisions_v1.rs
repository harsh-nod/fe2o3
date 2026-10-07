// Total settlement decisions over actual owner declarations; capacities are observations.
verus! {

spec fn shared_settlement_storage_decision_v1(storage: SettlementReturnStorageV1, count: usize) -> Result<(), ReadErrorV1> {
    let writers = storage.writer_free_len as int + 1;
    let members = storage.member_free_len as int + count;
    if writers > usize::MAX || members > usize::MAX || writers > storage.writer_limit || writers > storage.writer_storage
        || members > storage.member_limit || members > storage.member_storage || count > storage.scratch_len {
        Err(ReadErrorV1::InvalidState)
    } else { Ok(()) }
}

spec fn settlement_scratch_scan_v1(journal: JournalContentsV1, count: usize, index: nat) -> Result<(), ReadErrorV1>
    recommends count <= journal.scratch@.len(),
    decreases count - index,
{
    if index >= count { Ok(()) }
    else if journal.scratch@[index as int].is_some() { Err(ReadErrorV1::InvalidState) }
    else { settlement_scratch_scan_v1(journal, count, index + 1) }
}

spec fn settlement_return_decision_v1(journal: JournalContentsV1, count: usize,
    free_storage: usize, member_free_storage: usize) -> Result<(), ReadErrorV1>
{
    let writers = journal.free@.len() + 1;
    let members = journal.member_free@.len() + count;
    if writers > usize::MAX || members > usize::MAX || writers > journal.writer_capacity || writers > free_storage
        || members > journal.allocation_capacity || members > member_free_storage || count > journal.scratch@.len() {
        Err(ReadErrorV1::InvalidState)
    } else { settlement_scratch_scan_v1(journal, count, 0) }
}

spec fn settlement_preflight_decision_v1(journal: JournalContentsV1, writer: WriterReferenceV1,
    evidence: WriterReferenceV1, free_storage: usize, member_free_storage: usize) -> Result<(Option<usize>, usize), ReadErrorV1>
{
    match owner_retained_header_v1(journal, writer, false) {
        Err(error) => Err(error),
        Ok((head, count, _)) => if evidence != writer { Err(ReadErrorV1::SettlementEvidenceMismatch) }
            else { match owner_retained_chain_v1(journal, writer, head, count) {
                Err(error) => Err(error),
                Ok(()) => match settlement_return_decision_v1(journal, count, free_storage, member_free_storage) {
                    Err(error) => Err(error), Ok(()) => Ok((head, count)),
                },
            } },
    }
}

spec fn settlement_cursor_v1(before: JournalContentsV1, head: Option<usize>, index: nat) -> Option<usize>
    decreases index,
{
    if index == 0 { head }
    else { before.members@[settlement_cursor_v1(before, head, (index - 1) as nat).unwrap() as int].unwrap().next }
}

spec fn settlement_previous_v1(before: JournalContentsV1, head: Option<usize>, index: nat) -> Option<AllocationKeyV1> {
    if index == 0 { None }
    else { Some(before.members@[settlement_cursor_v1(before, head, (index - 1) as nat).unwrap() as int].unwrap().allocation.key) }
}

proof fn settlement_scan_at_v1(before: JournalContentsV1, writer: WriterReferenceV1, head: Option<usize>, count: usize, index: nat)
    requires owner_retained_scan_v1(before, writer, head, count as nat, None) == Ok(()), index <= count,
    ensures owner_retained_scan_v1(before, writer, settlement_cursor_v1(before, head, index),
        (count - index) as nat, settlement_previous_v1(before, head, index)) == Ok(()),
    decreases index,
{
    if index > 0 { settlement_scan_at_v1(before, writer, head, count, (index - 1) as nat); }
}

spec fn settlement_plan_at_v1(before: JournalContentsV1, head: Option<usize>, index: nat) -> BeginMemberPlanV1 {
    let slot = settlement_cursor_v1(before, head, index).unwrap();
    let member = before.members@[slot as int].unwrap();
    BeginMemberPlanV1 { member_slot: slot, allocation: member.allocation,
        prior_lineage: member.prior_lineage, attempt_epoch: member.attempt_epoch }
}

spec fn settlement_plan_ready_v1(before: JournalContentsV1, head: Option<usize>, index: nat) -> bool {
    let cursor = settlement_cursor_v1(before, head, index);
    let plan = settlement_plan_at_v1(before, head, index);
    &&& cursor.is_some()
    &&& plan.member_slot < before.members@.len()
    &&& before.members@[plan.member_slot as int].is_some()
    &&& plan.allocation.slot < before.allocations@.len()
    &&& before.allocations@[plan.allocation.slot as int].is_some()
}

proof fn settlement_scan_plan_v1(before: JournalContentsV1, writer: WriterReferenceV1, head: Option<usize>, count: usize, index: nat)
    requires owner_retained_scan_v1(before, writer, head, count as nat, None) == Ok(()), index < count,
    ensures settlement_plan_ready_v1(before, head, index),
        owner_retained_member_v1(before, writer, settlement_cursor_v1(before, head, index),
            settlement_previous_v1(before, head, index))
            == Ok(before.members@[settlement_cursor_v1(before, head, index).unwrap() as int].unwrap()),
{
    settlement_scan_at_v1(before, writer, head, count, index);
}

proof fn settlement_scratch_at_v1(before: JournalContentsV1, count: usize, index: nat, at: nat)
    requires index <= at < count <= before.scratch@.len(), settlement_scratch_scan_v1(before, count, index) == Ok(()),
    ensures before.scratch@[at as int].is_none(),
    decreases at - index,
{
    if index < at { settlement_scratch_at_v1(before, count, index + 1, at); }
}

spec fn settlement_storage_ready_v1(before: JournalContentsV1, head: Option<usize>, count: usize) -> bool {
    &&& count <= before.scratch@.len()
    &&& before.free@.len() < usize::MAX
    &&& before.member_free@.len() + count <= usize::MAX
    &&& forall|i: nat| i < count ==> #[trigger] settlement_plan_ready_v1(before, head, i)
    &&& forall|i: int| 0 <= i < count ==> (#[trigger] before.scratch@[i]).is_none()
}

proof fn settlement_preflight_ready_v1(before: JournalContentsV1, writer: WriterReferenceV1,
    evidence: WriterReferenceV1, free_storage: usize, member_free_storage: usize, head: Option<usize>, count: usize)
    requires settlement_preflight_decision_v1(before, writer, evidence, free_storage, member_free_storage) == Ok((head, count)),
    ensures settlement_storage_ready_v1(before, head, count),
        owner_retained_header_v1(before, writer, false) == Ok((head, count, false)), evidence == writer,
        owner_retained_scan_v1(before, writer, head, count as nat, None) == Ok(()),
        before.free@.len() < before.writer_capacity,
        before.member_free@.len() + count <= before.allocation_capacity,
{
    assert(owner_retained_header_v1(before, writer, false) == Ok((head, count, false)));
    match owner_retained_chain_v1(before, writer, head, count) {
        Ok(value) => { assert(value == ()); }, Err(_) => {},
    }
    match settlement_return_decision_v1(before, count, free_storage, member_free_storage) {
        Ok(value) => { assert(value == ()); }, Err(_) => {},
    }
    assert forall|i: nat| i < count implies #[trigger] settlement_plan_ready_v1(before, head, i) by {
        settlement_scan_plan_v1(before, writer, head, count, i);
    }
    assert forall|i: int| 0 <= i < count implies (#[trigger] before.scratch@[i]).is_none() by {
        settlement_scratch_at_v1(before, count, 0, i as nat);
    }
}

spec fn settlement_scratch_v1(before: JournalContentsV1, head: Option<usize>, count: usize, filled: nat, cleared: nat)
    -> Seq<Option<BeginMemberPlanV1>>
{
    Seq::new(before.scratch@.len(), |i: int|
        if cleared <= i < filled { Some(settlement_plan_at_v1(before, head, i as nat)) } else { before.scratch@[i] })
}

spec fn settlement_updated_allocation_v1(entry: AllocationEntryV1, plan: BeginMemberPlanV1, success: bool) -> AllocationEntryV1 {
    AllocationEntryV1 { key: entry.key, device: entry.device, byte_extent: entry.byte_extent,
        attempt_epoch: entry.attempt_epoch,
        content_lineage: if success { plan.attempt_epoch } else { entry.content_lineage }, pending_member: None }
}

spec fn settlement_allocations_prefix_v1(before: JournalContentsV1, head: Option<usize>, success: bool, count: nat)
    -> Seq<Option<AllocationEntryV1>>
    decreases count,
{
    if count == 0 { before.allocations@ }
    else {
        let prefix = settlement_allocations_prefix_v1(before, head, success, (count - 1) as nat);
        let plan = settlement_plan_at_v1(before, head, (count - 1) as nat);
        prefix.update(plan.allocation.slot as int,
            Some(settlement_updated_allocation_v1(prefix[plan.allocation.slot as int].unwrap(), plan, success)))
    }
}

spec fn settlement_members_prefix_v1(before: JournalContentsV1, head: Option<usize>, count: nat)
    -> Seq<Option<MemberEntryV1>>
    decreases count,
{
    if count == 0 { before.members@ }
    else { settlement_members_prefix_v1(before, head, (count - 1) as nat)
        .update(settlement_plan_at_v1(before, head, (count - 1) as nat).member_slot as int, None) }
}

spec fn settlement_slots_v1(before: JournalContentsV1, head: Option<usize>, count: nat) -> Seq<usize> {
    Seq::new(count, |i: int| settlement_plan_at_v1(before, head, i as nat).member_slot)
}

proof fn settlement_prefix_shapes_v1(before: JournalContentsV1, head: Option<usize>, total: usize, count: nat, success: bool)
    requires settlement_storage_ready_v1(before, head, total), count <= total,
    ensures settlement_members_prefix_v1(before, head, count).len() == before.members@.len(),
        settlement_allocations_prefix_v1(before, head, success, count).len() == before.allocations@.len(),
        forall|a: int| 0 <= a < before.allocations@.len() ==>
            (#[trigger] settlement_allocations_prefix_v1(before, head, success, count)[a]).is_some() == before.allocations@[a].is_some(),
    decreases count,
{
    if count > 0 {
        settlement_prefix_shapes_v1(before, head, total, (count - 1) as nat, success);
        assert(settlement_plan_ready_v1(before, head, (count - 1) as nat));
    }
}

spec fn settlement_commit_frame_v1(before: JournalContentsV1, after: JournalContentsV1) -> bool {
    &&& after.context_generation == before.context_generation
    &&& after.allocation_capacity == before.allocation_capacity
    &&& after.writer_capacity == before.writer_capacity
    &&& after.registration_watermark == before.registration_watermark
    &&& after.reserved_count == before.reserved_count
    &&& after.allocation_free == before.allocation_free
}

spec fn settlement_raw_success_relation_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, head: Option<usize>, count: usize, success: bool) -> bool
{
    &&& settlement_commit_frame_v1(before, after)
    &&& after.writers@ == before.writers@.update(writer.slot as int, None)
    &&& after.free@ == before.free@.push(writer.slot)
    &&& after.members@ == settlement_members_prefix_v1(before, head, count as nat)
    &&& after.allocations@ == settlement_allocations_prefix_v1(before, head, success, count as nat)
    &&& after.member_free@ == before.member_free@ + settlement_slots_v1(before, head, count as nat)
    &&& after.scratch@ == before.scratch@
}

spec fn settlement_execution_relation_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, evidence: WriterReferenceV1, free_storage: usize, member_free_storage: usize,
    success: bool, result: Result<(), ReadErrorV1>) -> bool
{
    match settlement_preflight_decision_v1(before, writer, evidence, free_storage, member_free_storage) {
        Err(error) => result == Err(error) && after == before,
        Ok((head, count)) => result == Ok(()) && settlement_raw_success_relation_v1(before, after, writer, head, count, success),
    }
}


}
