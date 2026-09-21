use vstd::prelude::*;

verus! {

#[derive(Clone, Copy)]
pub struct AllocationWriteV1 {
    pub allocation: AllocationReferenceV1,
    pub device: DeviceKeyV1,
    pub byte_extent: u64,
}

// Unlike public allocation lookup, raw Begin does not validate a pending backlink.
pub open spec fn begin_exact_allocation_v1(journal: JournalContentsV1, reference: AllocationReferenceV1)
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

pub fn begin_exact_allocation_exec_v1(journal: &JournalContentsV1, reference: AllocationReferenceV1)
    -> (result: Result<AllocationEntryV1, ReadErrorV1>)
    ensures result == begin_exact_allocation_v1(*journal, reference),
{
    if reference.slot >= journal.allocations.len() { return Err(ReadErrorV1::InvalidAllocationReference); }
    match journal.allocations[reference.slot] {
        Some(entry) => {
            if entry.key.context_generation == reference.key.context_generation
                && entry.key.local == reference.key.local
                && entry.key.context_generation == journal.context_generation { Ok(entry) }
            else { Err(ReadErrorV1::InvalidAllocationReference) }
        },
        None => Err(ReadErrorV1::InvalidAllocationReference),
    }
}

pub open spec fn begin_destination_decision_v1(journal: JournalContentsV1, destination: AllocationWriteV1)
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

pub fn begin_destination_exec_v1(journal: &JournalContentsV1, destination: AllocationWriteV1)
    -> (result: Result<(), ReadErrorV1>)
    ensures result == begin_destination_decision_v1(*journal, destination),
{
    let entry = match begin_exact_allocation_exec_v1(journal, destination.allocation) {
        Ok(entry) => entry,
        Err(error) => return Err(error),
    };
    if entry.device.context_generation != destination.device.context_generation
        || entry.device.local != destination.device.local { return Err(ReadErrorV1::AllocationDeviceMismatch); }
    if entry.byte_extent != destination.byte_extent { return Err(ReadErrorV1::AllocationExtentMismatch); }
    if entry.pending_member.is_some() { return Err(ReadErrorV1::AllocationBusy); }
    match entry.attempt_epoch.checked_add(1) {
        Some(_) => Ok(()),
        None => Err(ReadErrorV1::EpochExhausted),
    }
}

pub open spec fn begin_canonical_scan_v1(roster: Seq<AllocationWriteV1>, index: nat) -> Result<(), ReadErrorV1>
    decreases roster.len() - index,
{
    if index >= roster.len() { Ok(()) }
    else if index > 0 && !enrollment_key_less_v1(roster[index - 1].allocation.key, roster[index as int].allocation.key) {
        Err(ReadErrorV1::NonCanonicalRoster)
    } else { begin_canonical_scan_v1(roster, index + 1) }
}

pub fn begin_canonical_exec_v1(roster: &[AllocationWriteV1]) -> (result: Result<(), ReadErrorV1>)
    ensures result == begin_canonical_scan_v1(roster@, 0),
{
    let mut index = 0usize;
    while index < roster.len()
        invariant index <= roster.len(),
            begin_canonical_scan_v1(roster@, 0) == begin_canonical_scan_v1(roster@, index as nat),
        decreases roster.len() - index,
    {
        if index > 0 && !enrollment_key_less_exec_v1(roster[index - 1].allocation.key, roster[index].allocation.key) {
            return Err(ReadErrorV1::NonCanonicalRoster);
        }
        index += 1;
    }
    Ok(())
}

pub open spec fn begin_destination_scan_v1(journal: JournalContentsV1, roster: Seq<AllocationWriteV1>, index: nat)
    -> Result<(), ReadErrorV1>
    decreases roster.len() - index,
{
    if index >= roster.len() { Ok(()) }
    else { match begin_destination_decision_v1(journal, roster[index as int]) {
        Err(error) => Err(error),
        Ok(()) => begin_destination_scan_v1(journal, roster, index + 1),
    } }
}

pub fn begin_destinations_exec_v1(journal: &JournalContentsV1, roster: &[AllocationWriteV1])
    -> (result: Result<(), ReadErrorV1>)
    ensures result == begin_destination_scan_v1(*journal, roster@, 0),
{
    let mut index = 0usize;
    while index < roster.len()
        invariant index <= roster.len(),
            begin_destination_scan_v1(*journal, roster@, 0) == begin_destination_scan_v1(*journal, roster@, index as nat),
        decreases roster.len() - index,
    {
        match begin_destination_exec_v1(journal, roster[index]) {
            Err(error) => return Err(error),
            Ok(()) => {},
        }
        index += 1;
    }
    Ok(())
}

pub open spec fn begin_slot_scan_v1(journal: JournalContentsV1, count: nat, index: nat) -> Result<(), ReadErrorV1>
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

pub fn begin_slots_exec_v1(journal: &JournalContentsV1, count: usize) -> (result: Result<(), ReadErrorV1>)
    requires count <= journal.member_free@.len(), count <= journal.scratch@.len(),
    ensures result == begin_slot_scan_v1(*journal, count as nat, 0),
{
    let mut index = 0usize;
    while index < count
        invariant index <= count, count <= journal.member_free@.len(), count <= journal.scratch@.len(),
            begin_slot_scan_v1(*journal, count as nat, 0) == begin_slot_scan_v1(*journal, count as nat, index as nat),
        decreases count - index,
    {
        let member = journal.member_free[journal.member_free.len() - 1 - index];
        if member >= journal.members.len() || journal.members[member].is_some() { return Err(ReadErrorV1::InvalidState); }
        if journal.scratch[index].is_some() { return Err(ReadErrorV1::InvalidState); }
        index += 1;
    }
    Ok(())
}

pub open spec fn begin_preflight_decision_v1(journal: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>)
    -> Result<usize, ReadErrorV1>
{
    match lookup_decision_v1(journal.context_generation, journal.writers@, writer) {
        Err(error) => Err(lift_error_v1(error)),
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

// No custody, length-equality, uniqueness or well-formed-journal precondition.
pub fn begin_preflight_exec_v1(journal: &JournalContentsV1, writer: WriterReferenceV1, roster: &[AllocationWriteV1])
    -> (result: Result<usize, ReadErrorV1>)
    ensures result == begin_preflight_decision_v1(*journal, writer, roster@),
{
    match lookup_reserved_v1(journal.context_generation, journal.writers.as_slice(), writer) {
        Err(error) => return Err(lift_error_exec_v1(error)),
        Ok(_) => {},
    }
    let reserved_count = match journal.reserved_count.checked_sub(1) {
        None => return Err(ReadErrorV1::InvalidState),
        Some(count) => count,
    };
    let count = roster.len();
    if count > journal.allocation_capacity { return Err(ReadErrorV1::RosterCapacity); }
    match begin_canonical_exec_v1(roster) {
        Err(error) => return Err(error),
        Ok(()) => {},
    }
    match begin_destinations_exec_v1(journal, roster) {
        Err(error) => return Err(error),
        Ok(()) => {},
    }
    if count > journal.member_free.len() { return Err(ReadErrorV1::MemberCapacity); }
    if count > journal.scratch.len() { return Err(ReadErrorV1::InvalidState); }
    match begin_slots_exec_v1(journal, count) {
        Err(error) => Err(error),
        Ok(()) => Ok(reserved_count),
    }
}

pub proof fn begin_destination_scan_at_v1(journal: JournalContentsV1, roster: Seq<AllocationWriteV1>, index: nat, at: nat)
    requires index <= at < roster.len(), begin_destination_scan_v1(journal, roster, index) == Ok(()),
    ensures begin_destination_decision_v1(journal, roster[at as int]) == Ok(()),
    decreases at - index,
{
    if index < at { begin_destination_scan_at_v1(journal, roster, index + 1, at); }
}

pub proof fn begin_slot_scan_at_v1(journal: JournalContentsV1, count: nat, index: nat, at: nat)
    requires index <= at < count, count <= journal.member_free@.len(), count <= journal.scratch@.len(),
        begin_slot_scan_v1(journal, count, index) == Ok(()),
    ensures journal.member_free@[journal.member_free@.len() - 1 - at] < journal.members@.len(),
        journal.members@[journal.member_free@[journal.member_free@.len() - 1 - at] as int].is_none(),
        journal.scratch@[at as int].is_none(),
    decreases at - index,
{
    if index < at { begin_slot_scan_at_v1(journal, count, index + 1, at); }
}

pub open spec fn begin_storage_ready_v1(before: JournalContentsV1, roster: Seq<AllocationWriteV1>) -> bool {
    &&& roster.len() <= before.member_free@.len()
    &&& roster.len() <= before.scratch@.len()
    &&& forall|i: int| 0 <= i < roster.len() ==> #[trigger] begin_destination_decision_v1(before, roster[i]) == Ok(())
    &&& forall|i: int| 0 <= i < roster.len() ==> {
        &&& before.member_free@[before.member_free@.len() - 1 - i] < before.members@.len()
        &&& before.members@[before.member_free@[before.member_free@.len() - 1 - i] as int].is_none()
        &&& before.scratch@[i].is_none()
    }
}

pub proof fn begin_preflight_ready_v1(before: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>)
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

pub open spec fn begin_plan_v1(before: JournalContentsV1, roster: Seq<AllocationWriteV1>, i: int) -> BeginMemberPlanV1 {
    let entry = before.allocations@[roster[i].allocation.slot as int].unwrap();
    BeginMemberPlanV1 {
        member_slot: before.member_free@[before.member_free@.len() - 1 - i],
        allocation: roster[i].allocation, prior_lineage: entry.content_lineage,
        attempt_epoch: (entry.attempt_epoch + 1) as u64,
    }
}

pub open spec fn begin_scratch_v1(before: JournalContentsV1, roster: Seq<AllocationWriteV1>, filled: nat, cleared: nat)
    -> Seq<Option<BeginMemberPlanV1>>
{
    Seq::new(before.scratch@.len(), |i: int|
        if cleared <= i < filled { Some(begin_plan_v1(before, roster, i)) } else { before.scratch@[i] })
}

pub open spec fn begin_stage_frame_v1(before: JournalContentsV1, after: JournalContentsV1) -> bool {
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

pub fn begin_stage_exec_v1(journal: &mut JournalContentsV1, roster: &[AllocationWriteV1])
    requires begin_storage_ready_v1(*old(journal), roster@),
    ensures begin_stage_frame_v1(*old(journal), *final(journal)),
        final(journal).scratch@ == begin_scratch_v1(*old(journal), roster@, roster@.len(), 0),
{
    let ghost before = *journal;
    let mut index = 0usize;
    proof { assert(journal.scratch@ =~= begin_scratch_v1(before, roster@, 0, 0)); }
    while index < roster.len()
        invariant index <= roster.len(), before == *old(journal), begin_storage_ready_v1(before, roster@),
            begin_stage_frame_v1(before, *journal),
            journal.scratch@ == begin_scratch_v1(before, roster@, index as nat, 0),
        decreases roster.len() - index,
    {
        let destination = roster[index];
        assert(begin_destination_decision_v1(before, destination) == Ok(()));
        let entry = journal.allocations[destination.allocation.slot].unwrap();
        let plan = BeginMemberPlanV1 {
            member_slot: journal.member_free[journal.member_free.len() - 1 - index],
            allocation: destination.allocation, prior_lineage: entry.content_lineage,
            attempt_epoch: entry.attempt_epoch + 1,
        };
        journal.scratch.set(index, Some(plan));
        index += 1;
        proof { assert(journal.scratch@ =~= begin_scratch_v1(before, roster@, index as nat, 0)); }
    }
}

pub open spec fn begin_member_v1(before: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, i: int)
    -> MemberEntryV1
{
    let plan = begin_plan_v1(before, roster, i);
    MemberEntryV1 {
        writer, allocation: plan.allocation, prior_lineage: plan.prior_lineage, attempt_epoch: plan.attempt_epoch,
        next: if i + 1 == roster.len() { None } else { Some(begin_plan_v1(before, roster, i + 1).member_slot) },
    }
}

pub open spec fn begin_members_prefix_v1(before: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, count: nat)
    -> Seq<Option<MemberEntryV1>>
    decreases count,
{
    if count == 0 { before.members@ }
    else { begin_members_prefix_v1(before, writer, roster, (count - 1) as nat)
        .update(begin_plan_v1(before, roster, count - 1).member_slot as int, Some(begin_member_v1(before, writer, roster, count - 1))) }
}

pub open spec fn begin_updated_allocation_v1(entry: AllocationEntryV1, plan: BeginMemberPlanV1) -> AllocationEntryV1 {
    AllocationEntryV1 {
        key: entry.key, device: entry.device, byte_extent: entry.byte_extent,
        attempt_epoch: plan.attempt_epoch, content_lineage: entry.content_lineage, pending_member: Some(plan.member_slot),
    }
}

pub open spec fn begin_allocations_prefix_v1(before: JournalContentsV1, roster: Seq<AllocationWriteV1>, count: nat)
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

pub proof fn begin_prefix_shapes_v1(before: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, count: nat)
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

pub open spec fn begin_commit_frame_v1(before: JournalContentsV1, after: JournalContentsV1) -> bool {
    &&& after.context_generation == before.context_generation
    &&& after.allocation_capacity == before.allocation_capacity
    &&& after.writer_capacity == before.writer_capacity
    &&& after.registration_watermark == before.registration_watermark
    &&& after.free == before.free
    &&& after.allocation_free == before.allocation_free
}

pub open spec fn begin_success_relation_v1(before: JournalContentsV1, after: JournalContentsV1,
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

pub fn begin_commit_exec_v1(journal: &mut JournalContentsV1, writer: WriterReferenceV1, roster: &[AllocationWriteV1],
    reserved_count: usize, Ghost(before): Ghost<JournalContentsV1>)
    requires begin_storage_ready_v1(before, roster@), writer.slot < before.writers@.len(),
        before.reserved_count > 0, reserved_count == before.reserved_count - 1,
        begin_stage_frame_v1(before, *old(journal)),
        old(journal).scratch@ == begin_scratch_v1(before, roster@, roster@.len(), 0),
    ensures begin_success_relation_v1(before, *final(journal), writer, roster@),
{
    let count = roster.len();
    let head = if count == 0 { None } else { Some(journal.scratch[0].unwrap().member_slot) };
    let mut index = 0usize;
    proof {
        begin_prefix_shapes_v1(before, writer, roster@, 0);
        assert(journal.member_free@ =~= before.member_free@.subrange(0, before.member_free@.len() as int));
    }
    while index < count
        invariant index <= count, count == roster.len(), begin_storage_ready_v1(before, roster@),
            writer.slot < before.writers@.len(), before.reserved_count > 0,
            reserved_count == before.reserved_count - 1,
            head == if count == 0 { None } else { Some(begin_plan_v1(before, roster@, 0).member_slot) },
            begin_commit_frame_v1(before, *journal), journal.writers == before.writers,
            journal.reserved_count == before.reserved_count,
            journal.scratch@ == begin_scratch_v1(before, roster@, roster@.len(), index as nat),
            journal.allocations@ == begin_allocations_prefix_v1(before, roster@, index as nat),
            journal.members@ == begin_members_prefix_v1(before, writer, roster@, index as nat),
            journal.member_free@ == before.member_free@.subrange(0, before.member_free@.len() - index),
            journal.members@.len() == before.members@.len(), journal.allocations@.len() == before.allocations@.len(),
            forall|a: int| 0 <= a < before.allocations@.len() ==>
                (#[trigger] journal.allocations@[a]).is_some() == before.allocations@[a].is_some(),
        decreases count - index,
    {
        let plan = journal.scratch[index].unwrap();
        journal.scratch.set(index, None);
        let next = if index + 1 == count { None } else { Some(journal.scratch[index + 1].unwrap().member_slot) };
        let _ = journal.member_free.pop();
        journal.members.set(plan.member_slot, Some(MemberEntryV1 {
            writer, allocation: plan.allocation, prior_lineage: plan.prior_lineage,
            attempt_epoch: plan.attempt_epoch, next,
        }));
        assert(begin_destination_decision_v1(before, roster@[index as int]) == Ok(()));
        let entry = journal.allocations[plan.allocation.slot].unwrap();
        journal.allocations.set(plan.allocation.slot, Some(AllocationEntryV1 {
            key: entry.key, device: entry.device, byte_extent: entry.byte_extent,
            attempt_epoch: plan.attempt_epoch, content_lineage: entry.content_lineage, pending_member: Some(plan.member_slot),
        }));
        index += 1;
        proof {
            assert(journal.scratch@ =~= begin_scratch_v1(before, roster@, roster@.len(), index as nat));
            assert(journal.member_free@ =~= before.member_free@.subrange(0, before.member_free@.len() - index));
            begin_prefix_shapes_v1(before, writer, roster@, index as nat);
        }
    }
    journal.writers.set(writer.slot, Some(WriterEntryV1::Pending { key: writer.key, head, count }));
    journal.reserved_count = reserved_count;
    proof { assert(journal.scratch@ =~= before.scratch@); }
}

pub open spec fn begin_execution_relation_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, result: Result<(), ReadErrorV1>) -> bool
{
    match begin_preflight_decision_v1(before, writer, roster) {
        Err(error) => result == Err(error) && after == before,
        Ok(_) => result == Ok(()) && begin_success_relation_v1(before, after, writer, roster),
    }
}

pub fn begin_exec_v1(journal: &mut JournalContentsV1, writer: WriterReferenceV1, roster: &[AllocationWriteV1])
    -> (result: Result<(), ReadErrorV1>)
    ensures begin_execution_relation_v1(*old(journal), *final(journal), writer, roster@, result),
{
    let ghost before = *journal;
    let reserved_count = match begin_preflight_exec_v1(journal, writer, roster) {
        Err(error) => return Err(error),
        Ok(count) => count,
    };
    proof { begin_preflight_ready_v1(before, writer, roster@); }
    begin_stage_exec_v1(journal, roster);
    begin_commit_exec_v1(journal, writer, roster, reserved_count, Ghost(before));
    Ok(())
}

#[verifier::spinoff_prover]
pub proof fn begin_preserves_issuance_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, result: Result<(), ReadErrorV1>,
    storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires issuance_invariant_v1(issuance_contents_projection_v1(before, storage, history)),
        begin_execution_relation_v1(before, after, writer, roster, result),
    ensures issuance_invariant_v1(issuance_contents_projection_v1(after, storage, history)),
{
    if result.is_ok() {
        begin_preflight_ready_v1(before, writer, roster);
        begin_prefix_shapes_v1(before, writer, roster, roster.len());
        let pre = issuance_contents_projection_v1(before, storage, history);
        let post = issuance_contents_projection_v1(after, storage, history);
        prefix_update_v1(pre.writers, writer.slot as int, post.writers[writer.slot as int], pre.writers.len() as int);
        assert forall|w: int| 0 <= w < post.writers.len() implies {
            &&& post.writers[w].is_some() == pre.writers[w].is_some()
            &&& post.writers[w].is_some() ==> writer_key_v1(post.writers[w].unwrap()) == writer_key_v1(pre.writers[w].unwrap())
        } by {}
        assert forall|w: int| #![trigger post.writers[w]] 0 <= w < post.writers.len() implies match post.writers[w] {
            Some(entry) => {
                &&& exists|i: int| 0 <= i < history.len() && history[i].slot == w
                    && same_key_v1(history[i].key, writer_key_v1(entry))
                &&& forall|i: int| 0 <= i < history.len() && history[i].slot == w
                    ==> history[i].key.local <= writer_key_v1(entry).local
            },
            None => true,
        } by { assert(pre.writers[w].is_some() == post.writers[w].is_some()); }
    }
}

// A real constructor/enroll/register/Begin trace, not direct Pending-state initialization.
// General pending-custody/reader preservation is a separate obligation.
#[verifier::spinoff_prover]
pub fn begin_constructor_witness_v1(empty: bool) -> (result: bool)
    ensures result,
{
    proof {
        reveal_with_fuel(enrollment_roster_scan_v1, 4);
        reveal_with_fuel(enrollment_prefix_v1, 4);
        reveal_with_fuel(begin_canonical_scan_v1, 4);
        reveal_with_fuel(begin_destination_scan_v1, 4);
        reveal_with_fuel(begin_slot_scan_v1, 4);
        reveal_with_fuel(begin_allocations_prefix_v1, 4);
        reveal_with_fuel(begin_members_prefix_v1, 4);
    }
    let ghost storage = witness_storage_v1(3, 2);
    let ghost history = Seq::<WriterReferenceV1>::empty();
    let mut contents = match issued_producer_constructor_exec_v1(7, 3, 2, 2, Ghost(storage)) {
        Ok(contents) => contents, Err(_) => return false,
    };
    let entries = vec![EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 20 },
        device: DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 },
        EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 30 },
        device: DeviceKeyV1 { context_generation: 7, local: 3 }, byte_extent: 32 }];
    let mut output = vec![None, None];
    let enrolled = enrollment_issued_exec_v1(&mut contents, &entries, &mut output, Ghost(storage), Ghost(history));
    assert(enrolled == Ok(()));
    assert(output@[0] == Some(AllocationReferenceV1 { slot: 0, key: entries@[0].key }));
    assert(output@[1] == Some(AllocationReferenceV1 { slot: 1, key: entries@[1].key }));
    let key = WriterKeyV1 { context_generation: 7, local: 40, kind: WriterKindV1::Submission };
    let writer = match register_issued_producer_exec_v1(&mut contents, key, Ghost(storage), Ghost(history)) {
        Ok(reference) => reference, Err(_) => return false,
    };
    let ghost history = seq![writer];
    let ghost before = contents.stable.journal;
    let mut roster = vec![];
    if !empty {
        roster.push(AllocationWriteV1 { allocation: output[0].unwrap(), device: entries[0].device, byte_extent: 16 });
        roster.push(AllocationWriteV1 { allocation: output[1].unwrap(), device: entries[1].device, byte_extent: 32 });
    }
    let result = begin_exec_v1(&mut contents.stable.journal, writer, &roster);
    assert(result == Ok(()));
    proof { begin_preserves_issuance_v1(before, contents.stable.journal, writer, roster@, result, storage, history); }
    assert(contents.stable.journal.reserved_count == 0);
    if empty {
        assert(contents.stable.journal.writers@[writer.slot as int]
            == Some(WriterEntryV1::Pending { key, head: None, count: 0 }));
        assert(contents.stable.journal.allocations@ == before.allocations@);
        assert(contents.stable.journal.members@ == before.members@);
    } else {
        assert(contents.stable.journal.writers@[writer.slot as int]
            == Some(WriterEntryV1::Pending { key, head: Some(0), count: 2 }));
        assert(contents.stable.journal.members@[0].unwrap().next == Some(1));
        assert(contents.stable.journal.members@[1].unwrap().next == None);
        assert(contents.stable.journal.member_free@ == seq![2usize]);
        assert(contents.stable.journal.allocations@[0].unwrap().attempt_epoch == 1);
        assert(contents.stable.journal.allocations@[1].unwrap().attempt_epoch == 1);
        assert(contents.stable.journal.allocations@[0].unwrap().content_lineage == 0);
        assert(contents.stable.journal.allocations@[1].unwrap().content_lineage == 0);
    }
    let ghost pending = contents.stable.journal;
    let again = begin_exec_v1(&mut contents.stable.journal, writer, &roster);
    assert(again == Err(ReadErrorV1::InvalidReference) && contents.stable.journal == pending);
    true
}

}
