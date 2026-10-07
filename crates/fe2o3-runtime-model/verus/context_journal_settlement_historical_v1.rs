// Historical correspondence keeps opaque Vec identity separate from sequence equality.
verus! {

spec fn settlement_storage_view(storage: SettlementReturnStorageV1) -> logical::SettlementReturnStorageV1 {
    logical::SettlementReturnStorageV1 {
        writer_free_len: storage.writer_free_len, member_free_len: storage.member_free_len,
        writer_limit: storage.writer_limit, writer_storage: storage.writer_storage,
        member_limit: storage.member_limit, member_storage: storage.member_storage,
        scratch_len: storage.scratch_len,
    }
}

proof fn settlement_scalar_historical(storage: SettlementReturnStorageV1, count: usize)
    ensures shared_settlement_storage_decision_v1(storage, count)
        == logical::shared_settlement_storage_decision_v1(settlement_storage_view(storage), count),
{}

proof fn settlement_scratch_scan_historical(before: logical::JournalContentsV1, count: usize, index: nat)
    ensures settlement_scratch_scan_v1(logical_contents(before), count, index)
        == logical::settlement_scratch_scan_v1(before, count, index),
    decreases count - index,
{
    if index < count && before.scratch@[index as int].is_none() {
        settlement_scratch_scan_historical(before, count, index + 1);
    }
}

proof fn settlement_preflight_historical(before: logical::JournalContentsV1, writer: logical::WriterReferenceV1,
    evidence: logical::WriterReferenceV1, free_storage: usize, member_free_storage: usize)
    ensures settlement_preflight_decision_v1(logical_contents(before), writer, evidence, free_storage, member_free_storage)
        == logical::settlement_preflight_decision_v1(before, writer, evidence, free_storage, member_free_storage),
{
    header_historical(before, writer, false);
    if let Ok((head, count, _)) = logical::retained_header_decision_v1(before, writer, false) {
        chain_historical(before, writer, head, count);
        settlement_scratch_scan_historical(before, count, 0);
    }
}

proof fn settlement_plan_historical(before: logical::JournalContentsV1, head: Option<usize>, index: nat)
    ensures settlement_cursor_v1(logical_contents(before), head, index) == logical::settlement_cursor_v1(before, head, index),
        settlement_plan_at_v1(logical_contents(before), head, index) == logical::settlement_plan_at_v1(before, head, index),
        settlement_plan_ready_v1(logical_contents(before), head, index) == logical::settlement_plan_ready_v1(before, head, index),
    decreases index,
{
    if index > 0 { settlement_plan_historical(before, head, (index - 1) as nat); }
}

proof fn settlement_ready_historical(before: logical::JournalContentsV1, head: Option<usize>, count: usize)
    ensures settlement_storage_ready_v1(logical_contents(before), head, count) == logical::settlement_storage_ready_v1(before, head, count),
{
    if settlement_storage_ready_v1(logical_contents(before), head, count) {
        assert forall|i: nat| i < count implies #[trigger] logical::settlement_plan_ready_v1(before, head, i) by {
            settlement_plan_historical(before, head, i);
        }
    }
    if logical::settlement_storage_ready_v1(before, head, count) {
        assert forall|i: nat| i < count implies #[trigger] settlement_plan_ready_v1(logical_contents(before), head, i) by {
            settlement_plan_historical(before, head, i);
        }
    }
}

proof fn settlement_scratch_historical(before: logical::JournalContentsV1, head: Option<usize>, count: usize, filled: nat, cleared: nat)
    ensures settlement_scratch_v1(logical_contents(before), head, count, filled, cleared)
        == logical::settlement_scratch_v1(before, head, count, filled, cleared),
{
    assert(settlement_scratch_v1(logical_contents(before), head, count, filled, cleared)
        =~= logical::settlement_scratch_v1(before, head, count, filled, cleared)) by {
        assert forall|i: int| 0 <= i < before.scratch@.len() implies
            settlement_scratch_v1(logical_contents(before), head, count, filled, cleared)[i]
                == logical::settlement_scratch_v1(before, head, count, filled, cleared)[i] by {
            if cleared <= i < filled { settlement_plan_historical(before, head, i as nat); }
        }
    }
}

spec fn historical_stage_identity(before: logical::JournalContentsV1, after: logical::JournalContentsV1) -> bool {
    &&& after.writers == before.writers
    &&& after.free == before.free
    &&& after.allocations == before.allocations
    &&& after.allocation_free == before.allocation_free
    &&& after.members == before.members
    &&& after.member_free == before.member_free
}

proof fn settlement_stage_historical_factorization(before: logical::JournalContentsV1, after: logical::JournalContentsV1,
    head: Option<usize>, count: usize)
    ensures (logical::begin_stage_frame_v1(before, after)
        && after.scratch@ == logical::settlement_scratch_v1(before, head, count, count as nat, 0)) <==>
        (begin_stage_frame_v1(logical_contents(before), logical_contents(after))
            && logical_contents(after).scratch == settlement_scratch_v1(logical_contents(before), head, count, count as nat, 0)
            && historical_stage_identity(before, after)),
{
    settlement_scratch_historical(before, head, count, count as nat, 0);
}

proof fn settlement_prefix_historical(before: logical::JournalContentsV1, head: Option<usize>, count: nat, success: bool)
    ensures settlement_allocations_prefix_v1(logical_contents(before), head, success, count)
            == logical::settlement_allocations_prefix_v1(before, head, success, count),
        settlement_members_prefix_v1(logical_contents(before), head, count)
            == logical::settlement_members_prefix_v1(before, head, count),
    decreases count,
{
    if count > 0 {
        settlement_prefix_historical(before, head, (count - 1) as nat, success);
        settlement_plan_historical(before, head, (count - 1) as nat);
    }
}

proof fn settlement_slots_historical(before: logical::JournalContentsV1, head: Option<usize>, count: nat)
    ensures settlement_slots_v1(logical_contents(before), head, count) == logical::settlement_slots_v1(before, head, count),
{
    assert(settlement_slots_v1(logical_contents(before), head, count) =~= logical::settlement_slots_v1(before, head, count)) by {
        assert forall|i: int| 0 <= i < count implies settlement_slots_v1(logical_contents(before), head, count)[i]
            == logical::settlement_slots_v1(before, head, count)[i] by {
            settlement_plan_historical(before, head, i as nat);
        }
    }
}

proof fn settlement_raw_historical_factorization(before: logical::JournalContentsV1, after: logical::JournalContentsV1,
    writer: logical::WriterReferenceV1, head: Option<usize>, count: usize, success: bool)
    ensures logical::settlement_raw_success_relation_v1(before, after, writer, head, count, success) <==>
        (settlement_raw_success_relation_v1(logical_contents(before), logical_contents(after), writer, head, count, success)
            && after.allocation_free == before.allocation_free),
{
    settlement_prefix_historical(before, head, count as nat, success);
    settlement_slots_historical(before, head, count as nat);
}

spec fn historical_settlement_identity(before: logical::JournalContentsV1, after: logical::JournalContentsV1,
    writer: logical::WriterReferenceV1, evidence: logical::WriterReferenceV1, free_storage: usize, member_free_storage: usize) -> bool
{
    match logical::settlement_preflight_decision_v1(before, writer, evidence, free_storage, member_free_storage) {
        Err(_) => after == before,
        Ok(_) => after.allocation_free == before.allocation_free,
    }
}

proof fn settlement_historical_factorization(before: logical::JournalContentsV1, after: logical::JournalContentsV1,
    writer: logical::WriterReferenceV1, evidence: logical::WriterReferenceV1, free_storage: usize, member_free_storage: usize,
    success: bool, result: Result<(), logical::ReadErrorV1>)
    ensures logical::settlement_execution_relation_v1(before, after, writer, evidence, free_storage, member_free_storage, success, result) <==>
        (settlement_execution_view(logical_contents(before), logical_contents(after), writer, evidence,
            free_storage, member_free_storage, success, read_result_from(result))
            && historical_settlement_identity(before, after, writer, evidence, free_storage, member_free_storage)),
{
    settlement_preflight_historical(before, writer, evidence, free_storage, member_free_storage);
    match logical::settlement_preflight_decision_v1(before, writer, evidence, free_storage, member_free_storage) {
        Err(error) => { unit_result_embedding_injective(result, Err(error)); },
        Ok((head, count)) => { settlement_raw_historical_factorization(before, after, writer, head, count, success); },
    }
}

proof fn settlement_historical_to_view(before: logical::JournalContentsV1, after: logical::JournalContentsV1,
    writer: logical::WriterReferenceV1, evidence: logical::WriterReferenceV1, free_storage: usize, member_free_storage: usize,
    success: bool, result: Result<(), logical::ReadErrorV1>)
    requires logical::settlement_execution_relation_v1(before, after, writer, evidence, free_storage, member_free_storage, success, result),
    ensures settlement_execution_view(logical_contents(before), logical_contents(after), writer, evidence,
        free_storage, member_free_storage, success, read_result_from(result)),
{
    settlement_historical_factorization(before, after, writer, evidence, free_storage, member_free_storage, success, result);
}
}
