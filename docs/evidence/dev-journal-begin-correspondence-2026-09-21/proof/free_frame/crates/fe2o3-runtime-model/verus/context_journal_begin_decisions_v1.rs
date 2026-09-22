// Raw actual-type decisions and sequential updates; no custody precondition.
verus! {

spec fn begin_key_less_v1(left: AllocationKeyV1, right: AllocationKeyV1) -> bool {
    left.context_generation < right.context_generation
        || (left.context_generation == right.context_generation && left.local < right.local)
}

spec fn begin_reserved_decision_v1(journal: JournalContentsV1, writer: WriterReferenceV1) -> Result<WriterKeyV1, ReadErrorV1> {
    if writer.slot >= journal.writers@.len() { Err(ReadErrorV1::InvalidReference) }
    else { match journal.writers@[writer.slot as int] {
        Some(WriterEntryV1::Reserved(key)) => if key == writer.key && key.context_generation == journal.context_generation {
            Ok(key)
        } else { Err(ReadErrorV1::InvalidReference) },
        _ => Err(ReadErrorV1::InvalidReference),
    } }
}

spec fn begin_exact_allocation_v1(journal: JournalContentsV1, reference: AllocationReferenceV1)
    -> Result<AllocationEntryV1, ReadErrorV1>
{
    if reference.slot >= journal.allocations@.len() { Err(ReadErrorV1::InvalidAllocationReference) }
    else { match journal.allocations@[reference.slot as int] {
        Some(entry) => if entry.key == reference.key && entry.key.context_generation == journal.context_generation {
            Ok(entry)
        } else { Err(ReadErrorV1::InvalidAllocationReference) },
        None => Err(ReadErrorV1::InvalidAllocationReference),
    } }
}

spec fn begin_destination_decision_v1(journal: JournalContentsV1, destination: AllocationWriteV1)
    -> Result<(), ReadErrorV1>
{
    match begin_exact_allocation_v1(journal, destination.allocation) {
        Err(error) => Err(error),
        Ok(entry) => {
            if entry.device != destination.device { Err(ReadErrorV1::AllocationDeviceMismatch) }
            else if entry.byte_extent != destination.byte_extent { Err(ReadErrorV1::AllocationExtentMismatch) }
            else if entry.pending_member.is_some() { Err(ReadErrorV1::AllocationBusy) }
            else if entry.attempt_epoch == u64::MAX { Err(ReadErrorV1::EpochExhausted) }
            else { Ok(()) }
        },
    }
}

spec fn begin_canonical_scan_v1(roster: Seq<AllocationWriteV1>, index: nat) -> Result<(), ReadErrorV1>
    decreases roster.len() - index,
{
    if index >= roster.len() { Ok(()) }
    else if index > 0 && !begin_key_less_v1(roster[index - 1].allocation.key, roster[index as int].allocation.key) {
        Err(ReadErrorV1::NonCanonicalRoster)
    } else { begin_canonical_scan_v1(roster, index + 1) }
}

spec fn begin_destination_scan_v1(journal: JournalContentsV1, roster: Seq<AllocationWriteV1>, index: nat)
    -> Result<(), ReadErrorV1>
    decreases roster.len() - index,
{
    if index >= roster.len() { Ok(()) }
    else { match begin_destination_decision_v1(journal, roster[index as int]) {
        Err(error) => Err(error),
        Ok(()) => begin_destination_scan_v1(journal, roster, index + 1),
    } }
}

spec fn begin_slot_scan_v1(journal: JournalContentsV1, count: nat, index: nat) -> Result<(), ReadErrorV1>
    recommends count <= journal.member_free@.len(), count <= journal.scratch@.len(),
    decreases count - index,
{
    if index >= count { Ok(()) }
    else {
        let slot = journal.member_free@[journal.member_free@.len() - 1 - index];
        if slot >= journal.members@.len() || journal.members@[slot as int].is_some()
            || journal.scratch@[index as int].is_some() { Err(ReadErrorV1::InvalidState) }
        else { begin_slot_scan_v1(journal, count, index + 1) }
    }
}

spec fn begin_preflight_decision_v1(journal: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>)
    -> Result<usize, ReadErrorV1>
{
    match begin_reserved_decision_v1(journal, writer) {
        Err(error) => Err(error),
        Ok(_) => {
            if journal.reserved_count == 0 { Err(ReadErrorV1::InvalidState) }
            else if roster.len() > journal.allocation_capacity { Err(ReadErrorV1::RosterCapacity) }
            else { match begin_canonical_scan_v1(roster, 0) {
                Err(error) => Err(error),
                Ok(()) => match begin_destination_scan_v1(journal, roster, 0) {
                    Err(error) => Err(error),
                    Ok(()) => {
                        if roster.len() > journal.member_free@.len() { Err(ReadErrorV1::MemberCapacity) }
                        else if roster.len() > journal.scratch@.len() { Err(ReadErrorV1::InvalidState) }
                        else { match begin_slot_scan_v1(journal, roster.len(), 0) {
                            Err(error) => Err(error),
                            Ok(()) => Ok((journal.reserved_count - 1) as usize),
                        } }
                    },
                },
            } }
        },
    }
}

proof fn begin_destination_scan_at_v1(journal: JournalContentsV1, roster: Seq<AllocationWriteV1>, index: nat, at: nat)
    requires index <= at < roster.len(), begin_destination_scan_v1(journal, roster, index) == Ok(()),
    ensures begin_destination_decision_v1(journal, roster[at as int]) == Ok(()),
    decreases at - index,
{
    if index < at { begin_destination_scan_at_v1(journal, roster, index + 1, at); }
}

proof fn begin_slot_scan_at_v1(journal: JournalContentsV1, count: nat, index: nat, at: nat)
    requires index <= at < count, count <= journal.member_free@.len(), count <= journal.scratch@.len(),
        begin_slot_scan_v1(journal, count, index) == Ok(()),
    ensures journal.member_free@[journal.member_free@.len() - 1 - at] < journal.members@.len(),
        journal.members@[journal.member_free@[journal.member_free@.len() - 1 - at] as int].is_none(),
        journal.scratch@[at as int].is_none(),
    decreases at - index,
{
    if index < at { begin_slot_scan_at_v1(journal, count, index + 1, at); }
}

spec fn begin_storage_ready_v1(before: JournalContentsV1, roster: Seq<AllocationWriteV1>) -> bool {
    &&& roster.len() <= before.member_free@.len()
    &&& roster.len() <= before.scratch@.len()
    &&& forall|i: int| 0 <= i < roster.len() ==> #[trigger] begin_destination_decision_v1(before, roster[i]) == Ok(())
    &&& forall|i: int| 0 <= i < roster.len() ==> {
        &&& before.member_free@[before.member_free@.len() - 1 - i] < before.members@.len()
        &&& before.members@[before.member_free@[before.member_free@.len() - 1 - i] as int].is_none()
        &&& before.scratch@[i].is_none()
    }
}

proof fn begin_preflight_ready_v1(before: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>)
    requires begin_preflight_decision_v1(before, writer, roster).is_ok(),
    ensures begin_storage_ready_v1(before, roster), writer.slot < before.writers@.len(), before.reserved_count > 0,
{
    let canonical = begin_canonical_scan_v1(roster, 0);
    assert(canonical == Ok(())) by {
        match canonical { Ok(value) => { assert(value == ()); }, Err(_) => {}, }
    }
    let destinations = begin_destination_scan_v1(before, roster, 0);
    assert(destinations == Ok(())) by {
        match destinations { Ok(value) => { assert(value == ()); }, Err(_) => {}, }
    }
    let slots = begin_slot_scan_v1(before, roster.len(), 0);
    assert(slots == Ok(())) by {
        match slots { Ok(value) => { assert(value == ()); }, Err(_) => {}, }
    }
    assert forall|i: int| 0 <= i < roster.len() implies #[trigger] begin_destination_decision_v1(before, roster[i]) == Ok(()) by {
        begin_destination_scan_at_v1(before, roster, 0, i as nat);
    }
    assert forall|i: int| 0 <= i < roster.len() implies {
        &&& before.member_free@[before.member_free@.len() - 1 - i] < before.members@.len()
        &&& before.members@[before.member_free@[before.member_free@.len() - 1 - i] as int].is_none()
        &&& before.scratch@[i].is_none()
    } by { begin_slot_scan_at_v1(before, roster.len(), 0, i as nat); }
}

spec fn begin_plan_v1(before: JournalContentsV1, roster: Seq<AllocationWriteV1>, i: int) -> BeginMemberPlanV1 {
    let entry = before.allocations@[roster[i].allocation.slot as int].unwrap();
    BeginMemberPlanV1 {
        member_slot: before.member_free@[before.member_free@.len() - 1 - i],
        allocation: roster[i].allocation, prior_lineage: entry.content_lineage,
        attempt_epoch: (entry.attempt_epoch + 1) as u64,
    }
}

spec fn begin_scratch_v1(before: JournalContentsV1, roster: Seq<AllocationWriteV1>, filled: nat, cleared: nat)
    -> Seq<Option<BeginMemberPlanV1>>
{
    Seq::new(before.scratch@.len(), |i: int|
        if cleared <= i < filled { Some(begin_plan_v1(before, roster, i)) } else { before.scratch@[i] })
}

spec fn begin_stage_frame_v1(before: JournalContentsV1, after: JournalContentsV1) -> bool {
    &&& after.context_generation == before.context_generation
    &&& after.allocation_capacity == before.allocation_capacity
    &&& after.writer_capacity == before.writer_capacity
    &&& after.registration_watermark == before.registration_watermark
    &&& after.reserved_count == before.reserved_count
    &&& after.writers == before.writers
    &&& after.free == before.free
    &&& after.allocations == before.allocations
    &&& after.allocation_free == before.allocation_free
    &&& after.members == before.members
    &&& after.member_free == before.member_free
}

spec fn begin_member_v1(before: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, i: int)
    -> MemberEntryV1
{
    let plan = begin_plan_v1(before, roster, i);
    MemberEntryV1 {
        writer, allocation: plan.allocation, prior_lineage: plan.prior_lineage, attempt_epoch: plan.attempt_epoch,
        next: if i + 1 == roster.len() { None } else { Some(begin_plan_v1(before, roster, i + 1).member_slot) },
    }
}

spec fn begin_members_prefix_v1(before: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, count: nat)
    -> Seq<Option<MemberEntryV1>>
    decreases count,
{
    if count == 0 { before.members@ }
    else { begin_members_prefix_v1(before, writer, roster, (count - 1) as nat)
        .update(begin_plan_v1(before, roster, count - 1).member_slot as int, Some(begin_member_v1(before, writer, roster, count - 1))) }
}

spec fn begin_updated_allocation_v1(entry: AllocationEntryV1, plan: BeginMemberPlanV1) -> AllocationEntryV1 {
    AllocationEntryV1 {
        key: entry.key, device: entry.device, byte_extent: entry.byte_extent,
        attempt_epoch: plan.attempt_epoch, content_lineage: entry.content_lineage, pending_member: Some(plan.member_slot),
    }
}

spec fn begin_allocations_prefix_v1(before: JournalContentsV1, roster: Seq<AllocationWriteV1>, count: nat)
    -> Seq<Option<AllocationEntryV1>>
    decreases count,
{
    if count == 0 { before.allocations@ }
    else {
        let prefix = begin_allocations_prefix_v1(before, roster, (count - 1) as nat);
        let plan = begin_plan_v1(before, roster, count - 1);
        prefix.update(plan.allocation.slot as int, Some(begin_updated_allocation_v1(prefix[plan.allocation.slot as int].unwrap(), plan)))
    }
}

proof fn begin_prefix_shapes_v1(before: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, count: nat)
    requires begin_storage_ready_v1(before, roster), count <= roster.len(),
    ensures begin_members_prefix_v1(before, writer, roster, count).len() == before.members@.len(),
        begin_allocations_prefix_v1(before, roster, count).len() == before.allocations@.len(),
        forall|a: int| 0 <= a < before.allocations@.len() ==>
            (#[trigger] begin_allocations_prefix_v1(before, roster, count)[a]).is_some() == before.allocations@[a].is_some(),
    decreases count,
{
    if count > 0 {
        begin_prefix_shapes_v1(before, writer, roster, (count - 1) as nat);
        assert(begin_destination_decision_v1(before, roster[count - 1]) == Ok(()));
    }
}

spec fn begin_commit_frame_v1(before: JournalContentsV1, after: JournalContentsV1) -> bool {
    &&& after.context_generation == before.context_generation
    &&& after.allocation_capacity == before.allocation_capacity
    &&& after.writer_capacity == before.writer_capacity
    &&& after.registration_watermark == before.registration_watermark
    &&& true
    &&& after.allocation_free == before.allocation_free
}

spec fn begin_success_relation_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>) -> bool
{
    &&& begin_commit_frame_v1(before, after)
    &&& after.reserved_count == before.reserved_count - 1
    &&& after.writers@ == before.writers@.update(writer.slot as int, Some(WriterEntryV1::Pending {
        key: writer.key, head: if roster.len() == 0 { None } else { Some(begin_plan_v1(before, roster, 0).member_slot) },
        count: roster.len() as usize,
    }))
    &&& after.allocations@ == begin_allocations_prefix_v1(before, roster, roster.len())
    &&& after.members@ == begin_members_prefix_v1(before, writer, roster, roster.len())
    &&& after.member_free@ == before.member_free@.subrange(0, before.member_free@.len() - roster.len())
    &&& after.scratch@ == before.scratch@
}

spec fn begin_execution_relation_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, result: Result<(), ReadErrorV1>) -> bool
{
    match begin_preflight_decision_v1(before, writer, roster) {
        Err(error) => result == Err(error) && after == before,
        Ok(_) => result == Ok(()) && begin_success_relation_v1(before, after, writer, roster),
    }
}

}
