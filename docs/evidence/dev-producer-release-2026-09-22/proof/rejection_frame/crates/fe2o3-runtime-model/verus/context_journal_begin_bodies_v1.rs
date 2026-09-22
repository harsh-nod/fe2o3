verus! {

// Test-only counted accesses are absent from production and the content contract.
fn begin_indexed_access_v1(journal: &JournalContentsV1) {}
fn retained_indexed_access_v1(journal: &JournalContentsV1) {}

fn shared_retained_writer_key_v1(left: WriterKeyV1, right: WriterKeyV1) -> (result: bool)
    ensures result == (left == right),
{
    retained_writer_key_body!(left, right)
}

fn shared_retained_allocation_less_v1(left: AllocationKeyV1, right: AllocationKeyV1) -> (result: bool)
    ensures result == begin_key_less_v1(left, right),
{
    retained_allocation_less_body!(left, right)
}

fn shared_retained_allocation_v1(journal: &JournalContentsV1, reference: AllocationReferenceV1)
    -> (result: Result<AllocationEntryV1, ReadErrorV1>)
    ensures result == begin_exact_allocation_v1(*journal, reference),
{
    retained_allocation_body!(journal, reference)
}

fn begin_reserved_exec_v1(journal: &JournalContentsV1, writer: WriterReferenceV1)
    -> (result: Result<WriterKeyV1, ReadErrorV1>)
    ensures result == begin_reserved_decision_v1(*journal, writer),
{
    begin_reserved_body!(journal, writer)
}

fn begin_canonical_exec_v1(roster: &[AllocationWriteV1]) -> (result: Result<(), ReadErrorV1>)
    ensures result == begin_canonical_scan_v1(roster@, 0),
{
    begin_canonical_body!(verus_exec_expr, roster, index, [
        invariant 1 <= index <= roster.len(),
            begin_canonical_scan_v1(roster@, 0) == begin_canonical_scan_v1(roster@, index as nat),
        decreases roster.len() - index,
    ])
}

fn begin_destinations_exec_v1(journal: &JournalContentsV1, roster: &[AllocationWriteV1])
    -> (result: Result<(), ReadErrorV1>)
    ensures result == begin_destination_scan_v1(*journal, roster@, 0),
{
    begin_destinations_body!(verus_exec_expr, journal, roster, index, [
        invariant index <= roster.len(),
            begin_destination_scan_v1(*journal, roster@, 0) == begin_destination_scan_v1(*journal, roster@, index as nat),
        decreases roster.len() - index,
    ])
}

fn begin_slots_exec_v1(journal: &JournalContentsV1, count: usize) -> (result: Result<(), ReadErrorV1>)
    requires count <= journal.member_free@.len(), count <= journal.scratch@.len(),
    ensures result == begin_slot_scan_v1(*journal, count as nat, 0),
{
    begin_slots_body!(verus_exec_expr, journal, count, index, [
        invariant index <= count, count <= journal.member_free@.len(), count <= journal.scratch@.len(),
            begin_slot_scan_v1(*journal, count as nat, 0) == begin_slot_scan_v1(*journal, count as nat, index as nat),
        decreases count - index,
    ])
}

fn begin_preflight_exec_v1(journal: &JournalContentsV1, writer: WriterReferenceV1, roster: &[AllocationWriteV1])
    -> (result: Result<usize, ReadErrorV1>)
    ensures result == begin_preflight_decision_v1(*journal, writer, roster@),
{
    begin_preflight_body!(journal, writer, roster)
}

fn begin_stage_exec_v1(journal: &mut JournalContentsV1, roster: &[AllocationWriteV1])
    requires begin_storage_ready_v1(*old(journal), roster@),
    ensures begin_stage_frame_v1(*old(journal), *final(journal)),
        final(journal).scratch@ == begin_scratch_v1(*old(journal), roster@, roster@.len(), 0),
{
    let ghost before = *journal;
    proof { assert(journal.scratch@ =~= begin_scratch_v1(before, roster@, 0, 0)); }
    begin_stage_body!(verus_exec_expr, journal, roster, index, [
        invariant index <= roster.len(), before == *old(journal), begin_storage_ready_v1(before, roster@),
            begin_stage_frame_v1(before, *journal),
            journal.scratch@ == begin_scratch_v1(before, roster@, index as nat, 0),
            index < roster.len() ==> begin_destination_decision_v1(before, roster@[index as int]) == Ok(()),
        decreases roster.len() - index,
    ], [proof { assert(journal.scratch@ =~= begin_scratch_v1(before, roster@, index as nat, 0)); }]);
}

fn begin_commit_exec_v1(journal: &mut JournalContentsV1, writer: WriterReferenceV1, roster: &[AllocationWriteV1],
    reserved_count: usize, Ghost(before): Ghost<JournalContentsV1>)
    requires begin_storage_ready_v1(before, roster@), writer.slot < before.writers@.len(),
        before.reserved_count > 0, reserved_count == before.reserved_count - 1,
        begin_stage_frame_v1(before, *old(journal)),
        old(journal).scratch@ == begin_scratch_v1(before, roster@, roster@.len(), 0),
    ensures begin_success_relation_v1(before, *final(journal), writer, roster@),
{
    proof {
        begin_prefix_shapes_v1(before, writer, roster@, 0);
        assert(journal.member_free@ =~= before.member_free@.subrange(0, before.member_free@.len() as int));
    }
    begin_commit_body!(verus_exec_expr, journal, writer, roster, reserved_count, index, count, head, [
        invariant index <= count, count == roster.len(), begin_storage_ready_v1(before, roster@),
            writer.slot < before.writers@.len(), before.reserved_count > 0,
            reserved_count == before.reserved_count - 1,
            head == if count == 0 { None } else { Some(begin_plan_v1(before, roster@, 0).member_slot) },
            begin_commit_frame_v1(before, *journal), journal.writers == before.writers,
            journal.reserved_count == before.reserved_count,
            journal.scratch@ == begin_scratch_v1(before, roster@, roster@.len(), index as nat),
            index < count ==> journal.scratch@[index as int].is_some()
                && journal.scratch@[index as int].unwrap() == begin_plan_v1(before, roster@, index as int),
            index < count ==> begin_destination_decision_v1(before, roster@[index as int]) == Ok(()),
            journal.allocations@ == begin_allocations_prefix_v1(before, roster@, index as nat),
            journal.members@ == begin_members_prefix_v1(before, writer, roster@, index as nat),
            journal.member_free@ == before.member_free@.subrange(0, before.member_free@.len() - index),
            journal.members@.len() == before.members@.len(), journal.allocations@.len() == before.allocations@.len(),
            forall|a: int| 0 <= a < before.allocations@.len() ==>
                (#[trigger] journal.allocations@[a]).is_some() == before.allocations@[a].is_some(),
        decreases count - index,
    ], [proof {
        assert(journal.scratch@ =~= begin_scratch_v1(before, roster@, roster@.len(), index as nat));
        assert(journal.member_free@ =~= before.member_free@.subrange(0, before.member_free@.len() - index));
        begin_prefix_shapes_v1(before, writer, roster@, index as nat);
    }]);
    proof { assert(journal.scratch@ =~= before.scratch@); }
}

fn begin_exec_v1(journal: &mut JournalContentsV1, writer: WriterReferenceV1, roster: &[AllocationWriteV1])
    -> (result: Result<(), ReadErrorV1>)
    ensures begin_execution_relation_v1(*old(journal), *final(journal), writer, roster@, result),
{
    let ghost before = *journal;
    begin_execution_body!(verus_exec_expr, journal, writer, roster, reserved_count,
        [proof { begin_preflight_ready_v1(before, writer, roster@); }], [, Ghost(before)])
}

}
