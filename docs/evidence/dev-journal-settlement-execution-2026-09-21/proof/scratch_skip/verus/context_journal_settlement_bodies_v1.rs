verus! {

fn settlement_scratch_access_v1(journal: &ContextVersionJournalV1) {}
fn settlement_commit_access_v1(journal: &ContextVersionJournalV1) {}

fn shared_settlement_storage_exec_v1(storage: &SettlementReturnStorageV1, count: usize)
    -> (result: Result<(), ReadErrorV1>)
    ensures result == read_result_from(shared_settlement_storage_decision_v1(*storage, count)),
{
    settlement_return_admission_body!(storage, count, ReadErrorV1::InvalidState)
}

fn shared_settlement_scratch_scan_v1(journal: &ContextVersionJournalV1, count: usize)
    -> (result: Result<(), ReadErrorV1>)
    requires count <= journal.scratch@.len(),
    ensures result == read_result_from(settlement_scratch_scan_v1(journal_view(*journal), count, 0)),
{
    settlement_scratch_scan_body!(verus_exec_expr, journal, count, index, [
        invariant index <= count, count <= journal.scratch@.len(),
            settlement_scratch_scan_v1(journal_view(*journal), count, 0)
                == settlement_scratch_scan_v1(journal_view(*journal), count, index as nat),
        decreases count - index,
    ])
}

fn shared_settlement_scratch_stage_v1(journal: &mut ContextVersionJournalV1, initial: Option<usize>, count: usize)
    requires settlement_storage_ready_v1(journal_view(*old(journal)), initial, count),
    ensures begin_stage_frame_v1(journal_view(*old(journal)), journal_view(*final(journal))),
        journal_view(*final(journal)).scratch == settlement_scratch_v1(journal_view(*old(journal)), initial, count, count as nat, 0),
{
    let ghost before = journal_view(*journal);
    proof {
        journal_projection_exact(*journal);
        assert(journal_view(*journal).scratch =~= settlement_scratch_v1(before, initial, count, 0, 0));
    }
    settlement_scratch_stage_body!(verus_exec_expr, journal, initial, count, head, index, [
        invariant index <= count, before == journal_view(*old(journal)), settlement_storage_ready_v1(before, initial, count),
            begin_stage_frame_v1(before, journal_view(*journal)), head == settlement_cursor_v1(before, initial, index as nat),
            journal.scratch@.len() == before.scratch.len(), journal.members@.len() == before.members.len(),
            index < count ==> settlement_plan_ready_v1(before, initial, index as nat),
            forall|i: int| 0 <= i < journal.scratch@.len() ==> begin_plan_slot_view(#[trigger] journal.scratch@[i])
                == settlement_scratch_v1(before, initial, count, index as nat, 0)[i],
        decreases count - index,
    ]);
    proof { assert(journal_view(*journal).scratch =~= settlement_scratch_v1(before, initial, count, count as nat, 0)); }
}

fn shared_settlement_commit_v1(journal: &mut ContextVersionJournalV1, writer: ContextWriterReferenceV1,
    head: Option<usize>, count: usize, success: bool, Ghost(before): Ghost<LogicalJournalViewV1>)
    requires settlement_storage_ready_v1(before, head, count), writer.slot < before.writers.len(),
        begin_stage_frame_v1(before, journal_view(*old(journal))),
        journal_view(*old(journal)).scratch == settlement_scratch_v1(before, head, count, count as nat, 0),
    ensures settlement_raw_success_relation_v1(before, journal_view(*final(journal)), writer_reference_view(writer), head, count, success),
{
    proof {
        journal_projection_exact(*journal);
        settlement_prefix_shapes_v1(before, head, count, 0, success);
        assert(journal.member_free@ =~= before.member_free + settlement_slots_v1(before, head, 0));
        if count > 0 {
            assert(journal_view(*journal).scratch[0] == Some(settlement_plan_at_v1(before, head, 0)));
            assert(begin_plan_slot_view(journal.scratch@[0]) == Some(settlement_plan_at_v1(before, head, 0)));
        }
    }
    settlement_commit_body!(verus_exec_expr, journal, writer, count, success, index, [
        invariant index <= count, settlement_storage_ready_v1(before, head, count), writer.slot < before.writers.len(),
            settlement_commit_frame_v1(before, journal_view(*journal)),
            journal_view(*journal).writers == before.writers, journal.free@ == before.free,
            forall|i: int| 0 <= i < journal.scratch@.len() ==> begin_plan_slot_view(#[trigger] journal.scratch@[i])
                == settlement_scratch_v1(before, head, count, count as nat, index as nat)[i],
            index < count ==> journal.scratch@[index as int].is_some()
                && begin_plan_view(journal.scratch@[index as int].unwrap()) == settlement_plan_at_v1(before, head, index as nat),
            journal_view(*journal).allocations =~= settlement_allocations_prefix_v1(before, head, success, index as nat),
            journal_view(*journal).members =~= settlement_members_prefix_v1(before, head, index as nat),
            journal.member_free@ =~= before.member_free + settlement_slots_v1(before, head, index as nat),
            journal.members@.len() == before.members.len(), journal.allocations@.len() == before.allocations.len(),
            journal.scratch@.len() == before.scratch.len(), journal.writers@.len() == before.writers.len(),
            forall|a: int| 0 <= a < before.allocations.len() ==>
                (#[trigger] journal.allocations@[a]).is_some() == before.allocations[a].is_some(),
            index < count ==> settlement_plan_ready_v1(before, head, index as nat),
        decreases count - index,
    ]);
    proof {
        assert(journal_view(*journal).scratch =~= before.scratch);
        assert(journal_view(*journal).writers =~= before.writers.update(writer.slot as int, None));
    }
}

// Composition uses explicit key comparison and supplied capacity observations.
// This is not a proof of the public wrapper's derived Eq or Vec::capacity calls.
fn shared_settlement_return_v1(journal: &ContextVersionJournalV1, count: usize,
    free_storage: usize, member_free_storage: usize) -> (result: Result<(), ReadErrorV1>)
    ensures result == read_result_from(settlement_return_decision_v1(journal_view(*journal), count, free_storage, member_free_storage)),
{
    let storage = SettlementReturnStorageV1 {
        writer_free_len: journal.free.len(), member_free_len: journal.member_free.len(),
        writer_limit: journal.writer_capacity, writer_storage: free_storage,
        member_limit: journal.allocation_capacity, member_storage: member_free_storage,
        scratch_len: journal.scratch.len(),
    };
    match shared_settlement_storage_exec_v1(&storage, count) {
        Ok(value) => { assert(value == ()); }, Err(error) => return Err(error),
    }
    shared_settlement_scratch_scan_v1(journal, count)
}

fn shared_settlement_preflight_v1(journal: &ContextVersionJournalV1, writer: ContextWriterReferenceV1,
    evidence: ContextWriterReferenceV1, free_storage: usize, member_free_storage: usize)
    -> (result: Result<(Option<usize>, usize), ReadErrorV1>)
    ensures result == read_result_from(settlement_preflight_decision_v1(journal_view(*journal), writer_reference_view(writer),
        writer_reference_view(evidence), free_storage, member_free_storage)),
{
    let (head, count, _) = match shared_retained_header_v1(journal, writer, false) {
        Ok(value) => value, Err(error) => return Err(error),
    };
    if evidence.slot != writer.slot || !shared_retained_writer_key_v1(evidence.key, writer.key) {
        return Err(ReadErrorV1::SettlementEvidenceMismatch);
    }
    match shared_retained_chain_v1(journal, writer, head, count) {
        Ok(value) => { assert(value == ()); }, Err(error) => return Err(error),
    };
    match shared_settlement_return_v1(journal, count, free_storage, member_free_storage) {
        Ok(value) => { assert(value == ()); }, Err(error) => return Err(error),
    };
    Ok((head, count))
}

spec fn settlement_execution_view(before: LogicalJournalViewV1, after: LogicalJournalViewV1,
    writer: logical::WriterReferenceV1, evidence: logical::WriterReferenceV1,
    free_storage: usize, member_free_storage: usize, success: bool, result: Result<(), ReadErrorV1>) -> bool
{
    match settlement_preflight_decision_v1(before, writer, evidence, free_storage, member_free_storage) {
        Err(error) => result == Err(read_error_embed(error)) && after == before,
        Ok((head, count)) => result == Ok(()) && settlement_raw_success_relation_v1(before, after, writer, head, count, success),
    }
}

fn shared_settlement_execution_v1(journal: &mut ContextVersionJournalV1, writer: ContextWriterReferenceV1,
    evidence: ContextWriterReferenceV1, free_storage: usize, member_free_storage: usize, success: bool)
    -> (result: Result<(), ReadErrorV1>)
    ensures settlement_execution_view(journal_view(*old(journal)), journal_view(*final(journal)), writer_reference_view(writer),
        writer_reference_view(evidence), free_storage, member_free_storage, success, result),
{
    let ghost before = journal_view(*journal);
    let (head, count) = match shared_settlement_preflight_v1(journal, writer, evidence, free_storage, member_free_storage) {
        Ok(value) => value, Err(error) => return Err(error),
    };
    proof { settlement_preflight_ready_v1(before, writer_reference_view(writer), writer_reference_view(evidence),
        free_storage, member_free_storage, head, count); }
    shared_settlement_scratch_stage_v1(journal, head, count);
    shared_settlement_commit_v1(journal, writer, head, count, success, Ghost(before));
    Ok(())
}

}
