verus! {

fn disposal_access_v1(journal: &JournalContentsV1) {}

fn shared_disposal_scratch_scan_v1(journal: &JournalContentsV1, count: usize)
    -> (result: Result<(), ReadErrorV1>)
    requires count <= journal.scratch@.len(),
    ensures result == settlement_scratch_scan_v1(*journal, count, 0),
{
    disposal_scratch_scan_body!(verus_exec_expr, journal, count, index, [
        invariant index <= count, count <= journal.scratch@.len(),
            settlement_scratch_scan_v1(*journal, count, 0) == settlement_scratch_scan_v1(*journal, count, index as nat),
        decreases count - index,
    ])
}

fn shared_disposal_stage_v1(journal: &mut JournalContentsV1, initial: Option<usize>, count: usize)
    requires disposal_ready_v1(*old(journal), initial, count),
    ensures begin_stage_frame_v1(*old(journal), *final(journal)),
        final(journal).scratch@ == settlement_scratch_v1(*old(journal), initial, count, count as nat, 0),
{
    let ghost before = *journal;
    proof { assert(journal.scratch@ =~= settlement_scratch_v1(before, initial, count, 0, 0)); }
    disposal_stage_body!(verus_exec_expr, journal, initial, count, disposal_access_v1, head, index, [
        invariant index <= count, before == *old(journal), disposal_ready_v1(before, initial, count),
            begin_stage_frame_v1(before, *journal), head == settlement_cursor_v1(before, initial, index as nat),
            journal.scratch@.len() == before.scratch@.len(),
            index < count ==> settlement_plan_ready_v1(before, initial, index as nat),
            forall|i: int| 0 <= i < journal.scratch@.len() ==> (#[trigger] journal.scratch@[i])
                == settlement_scratch_v1(before, initial, count, index as nat, 0)[i],
        decreases count - index,
    ]);
    proof { assert(journal.scratch@ =~= settlement_scratch_v1(before, initial, count, count as nat, 0)); }
}

fn shared_disposal_commit_v1(journal: &mut JournalContentsV1, writer: WriterReferenceV1,
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
    disposal_commit_body!(verus_exec_expr, journal, writer, count, disposal_access_v1, index, [
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
    ]);
    proof { assert(journal.scratch@ =~= before.scratch@); }
}

impl ContextVersionJournalV1 {
    fn unknown_disposal_plan_observed_v1(&self, writer: WriterReferenceV1, roster: &[AllocationWriteV1],
        writer_storage: usize, member_storage: usize, allocation_storage: usize)
        -> (result: Result<(Option<usize>, usize), ReadErrorV1>)
        ensures result == disposal_plan_decision_v1(*self, writer, roster@, writer_storage, member_storage, allocation_storage),
    {
        disposal_plan_body!(verus_exec_expr, self, writer, roster, writer_storage, member_storage, allocation_storage,
            shared_retained_header_v1, shared_retained_chain_v1, shared_retained_allocation_v1, shared_disposal_scratch_scan_v1,
            head, count, cursor, index, [proof {
                match owner_retained_chain_v1(*self, writer, head, count) {
                    Ok(value) => { assert(value == ()); }, Err(_) => {},
                }
            }], [
                invariant index <= count, roster.len() == count,
                    owner_retained_header_v1(*self, writer, true) == Ok((head, count, true)),
                    owner_retained_chain_v1(*self, writer, head, count) == Ok(()),
                    owner_retained_scan_v1(*self, writer, head, count as nat, None) == Ok(()),
                    cursor == settlement_cursor_v1(*self, head, index as nat),
                    disposal_roster_scan_v1(*self, roster@, head, 0) == disposal_roster_scan_v1(*self, roster@, cursor, index as nat),
                decreases count - index,
            ], [proof { settlement_scan_plan_v1(*self, writer, head, count, index as nat); }])
    }

    fn validate_unknown_disposal_observed_v1(&self, writer: WriterReferenceV1, roster: &[AllocationWriteV1],
        writer_storage: usize, member_storage: usize, allocation_storage: usize) -> (result: Result<(), ReadErrorV1>)
        ensures result == disposal_validate_decision_v1(*self, writer, roster@, writer_storage, member_storage, allocation_storage),
    {
        disposal_validate_body!(self, writer, roster, unknown_disposal_plan_observed_v1, [, writer_storage, member_storage, allocation_storage])
    }

    fn dispose_unknown_observed_v1(&mut self, writer: WriterReferenceV1, evidence: &ContextWriterDisposalEvidenceV1<'_>,
        writer_storage: usize, member_storage: usize, allocation_storage: usize) -> (result: Result<(), ReadErrorV1>)
        ensures disposal_relation_v1(*old(self), *final(self), writer, evidence.writer, evidence.allocations@,
            writer_storage, member_storage, allocation_storage, result),
    {
        disposal_execute_body!(verus_exec_expr, self, writer, evidence,
            shared_retained_header_v1, shared_retained_writer_key_v1, unknown_disposal_plan_observed_v1,
            [, writer_storage, member_storage, allocation_storage], shared_disposal_stage_v1, shared_disposal_commit_v1,
            head, count, [, head, Ghost(before)], [let ghost before = *self;], [proof {
                disposal_preflight_ready_v1(before, writer, evidence.allocations@,
                    writer_storage, member_storage, allocation_storage, head, count);
            }])
    }
}

impl ContextReadLeasedJournalV1 {
    fn require_disposal_unread_writes_v1(&self, roster: &[AllocationWriteV1]) -> (result: Result<(), ReadErrorV1>)
        requires disposal_stable_safe_v1(*self, roster@, 0),
        ensures result == stable_unread_writes_v1(*self, roster@, 0),
    {
        unread_writes_body!(verus_exec_expr, self, roster, index, [
            invariant index <= roster.len(), disposal_stable_safe_v1(*self, roster@, index as nat),
                stable_unread_writes_v1(*self, roster@, 0) == stable_unread_writes_v1(*self, roster@, index as nat),
            decreases roster.len() - index,
        ])
    }

    fn validate_unknown_disposal_observed_v1(&self, writer: WriterReferenceV1, roster: &[AllocationWriteV1],
        writer_storage: usize, member_storage: usize, allocation_storage: usize) -> (result: Result<(), ReadErrorV1>)
        requires disposal_stable_safe_v1(*self, roster@, 0),
        ensures result == disposal_stable_validate_v1(*self, writer, roster@, writer_storage, member_storage, allocation_storage),
    {
        disposal_owner_validate_body!(verus_exec_expr, self, journal, writer, roster,
            require_disposal_unread_writes_v1, validate_unknown_disposal_observed_v1,
            [, writer_storage, member_storage, allocation_storage], [])
    }

    fn dispose_unknown_observed_v1(&mut self, writer: WriterReferenceV1, evidence: &ContextWriterDisposalEvidenceV1<'_>,
        writer_storage: usize, member_storage: usize, allocation_storage: usize) -> (result: Result<(), ReadErrorV1>)
        requires disposal_stable_safe_v1(*old(self), evidence.allocations@, 0),
        ensures disposal_stable_relation_v1(*old(self), *final(self), writer, evidence.writer, evidence.allocations@,
            writer_storage, member_storage, allocation_storage, result),
    {
        disposal_owner_execute_body!(verus_exec_expr, self, journal, writer, evidence,
            validate_unknown_disposal_observed_v1, dispose_unknown_observed_v1, [, writer_storage, member_storage, allocation_storage], [])
    }
}

impl ContextProducerReadJournalV1 {
    fn require_disposal_unread_writes_v1(&self, roster: &[AllocationWriteV1]) -> (result: Result<(), ReadErrorV1>)
        requires disposal_producer_safe_v1(*self, roster@, 0),
        ensures result == producer_unread_writes_v1(*self, roster@, 0),
    {
        unread_writes_body!(verus_exec_expr, self, roster, index, [
            invariant index <= roster.len(), disposal_producer_safe_v1(*self, roster@, index as nat),
                producer_unread_writes_v1(*self, roster@, 0) == producer_unread_writes_v1(*self, roster@, index as nat),
            decreases roster.len() - index,
        ])
    }

    fn validate_unknown_disposal_observed_v1(&self, writer: WriterReferenceV1, roster: &[AllocationWriteV1],
        writer_storage: usize, member_storage: usize, allocation_storage: usize) -> (result: Result<(), ReadErrorV1>)
        requires disposal_producer_safe_v1(*self, roster@, 0),
        ensures result == disposal_producer_validate_v1(*self, writer, roster@, writer_storage, member_storage, allocation_storage),
    {
        disposal_owner_validate_body!(verus_exec_expr, self, stable, writer, roster,
            require_disposal_unread_writes_v1, validate_unknown_disposal_observed_v1, [, writer_storage, member_storage, allocation_storage],
            [proof { disposal_producer_to_stable_v1(*self, roster@, 0); }])
    }

    fn dispose_unknown_observed_v1(&mut self, writer: WriterReferenceV1, evidence: &ContextWriterDisposalEvidenceV1<'_>,
        writer_storage: usize, member_storage: usize, allocation_storage: usize) -> (result: Result<(), ReadErrorV1>)
        requires disposal_producer_safe_v1(*old(self), evidence.allocations@, 0),
        ensures disposal_producer_relation_v1(*old(self), *final(self), writer, evidence.writer, evidence.allocations@,
            writer_storage, member_storage, allocation_storage, result),
    {
        disposal_owner_execute_body!(verus_exec_expr, self, stable, writer, evidence,
            validate_unknown_disposal_observed_v1, dispose_unknown_observed_v1, [, writer_storage, member_storage, allocation_storage],
            [proof { disposal_producer_to_stable_v1(*self, evidence.allocations@, 0); }])
    }
}

}
