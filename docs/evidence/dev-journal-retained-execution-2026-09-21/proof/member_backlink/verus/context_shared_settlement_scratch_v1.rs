// Shared scratch scanning/staging; physical storage and commit refinement remain separate.
include!("context_shared_retained_v1.rs");
include!("../src/context_version_journal/settlement_scratch_bodies.rs");

verus! {

pub fn settlement_scratch_access_v1(journal: &JournalContentsV1) {}

pub fn shared_settlement_scratch_scan_v1(journal: &JournalContentsV1, count: usize)
    -> (result: Result<(), ReadErrorV1>)
    requires count <= journal.scratch@.len(),
    ensures result == settlement_scratch_scan_v1(*journal, count, 0),
{
    settlement_scratch_scan_body!(verus_exec_expr, journal, count, index, [
        invariant index <= count, count <= journal.scratch@.len(),
            settlement_scratch_scan_v1(*journal, count, 0) == settlement_scratch_scan_v1(*journal, count, index as nat),
        decreases count - index,
    ])
}

pub fn shared_settlement_scratch_stage_v1(journal: &mut JournalContentsV1, initial: Option<usize>, count: usize)
    requires settlement_storage_ready_v1(*old(journal), initial, count),
    ensures begin_stage_frame_v1(*old(journal), *final(journal)),
        final(journal).scratch@ == settlement_scratch_v1(*old(journal), initial, count, count as nat, 0),
{
    let ghost before = *journal;
    proof { assert(journal.scratch@ =~= settlement_scratch_v1(before, initial, count, 0, 0)); }
    settlement_scratch_stage_body!(verus_exec_expr, journal, initial, count, head, index, [
        invariant index <= count, before == *old(journal), settlement_storage_ready_v1(before, initial, count),
            begin_stage_frame_v1(before, *journal), head == settlement_cursor_v1(before, initial, index as nat),
            index < count ==> settlement_plan_ready_v1(before, initial, index as nat),
            journal.scratch@ =~= settlement_scratch_v1(before, initial, count, index as nat, 0),
        decreases count - index,
    ]);
}

pub fn shared_scratch_return_v1(journal: &JournalContentsV1, count: usize,
    free_storage: usize, member_free_storage: usize) -> (result: Result<(), ReadErrorV1>)
    ensures result == settlement_return_decision_v1(*journal, count, free_storage, member_free_storage),
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

pub fn shared_scratch_preflight_v1(journal: &JournalContentsV1, writer: WriterReferenceV1,
    evidence: WriterReferenceV1, free_storage: usize, member_free_storage: usize)
    -> (result: Result<(Option<usize>, usize), ReadErrorV1>)
    ensures result == settlement_preflight_decision_v1(*journal, writer, evidence, free_storage, member_free_storage),
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
    match shared_scratch_return_v1(journal, count, free_storage, member_free_storage) {
        Ok(value) => { assert(value == ()); }, Err(error) => return Err(error),
    };
    Ok((head, count))
}

pub fn shared_scratch_settlement_v1(journal: &mut JournalContentsV1, writer: WriterReferenceV1, evidence: WriterReferenceV1,
    free_storage: usize, member_free_storage: usize, success: bool) -> (result: Result<(), ReadErrorV1>)
    ensures settlement_execution_relation_v1(*old(journal), *final(journal), writer, evidence,
        free_storage, member_free_storage, success, result),
{
    let ghost before = *journal;
    let (head, count) = match shared_scratch_preflight_v1(journal, writer, evidence, free_storage, member_free_storage) {
        Ok(value) => value, Err(error) => return Err(error),
    };
    proof { settlement_preflight_ready_v1(before, writer, evidence, free_storage, member_free_storage, head, count); }
    shared_settlement_scratch_stage_v1(journal, head, count);
    settlement_commit_exec_v1(journal, writer, head, count, success, Ghost(before));
    Ok(())
}

pub fn shared_scratch_settlement_issued_v1(contents: &mut ProducerReadContentsV1, writer: WriterReferenceV1,
    evidence: WriterReferenceV1, free_storage: usize, member_free_storage: usize, success: bool,
    Ghost(storage): Ghost<StorageCapacitiesV1>, Ghost(history): Ghost<Seq<WriterReferenceV1>>)
    -> (result: Result<(), ReadErrorV1>)
    requires issued_producer_v1(*old(contents), storage, history),
    ensures settlement_issued_execution_v1(*old(contents), *final(contents), writer, evidence, free_storage,
        member_free_storage, success, result, storage, history),
{
    let ghost before = *contents;
    let result = shared_scratch_settlement_v1(&mut contents.stable.journal, writer, evidence, free_storage, member_free_storage, success);
    proof {
        if result.is_ok() {
            let (head, count) = match settlement_preflight_decision_v1(before.stable.journal, writer, evidence, free_storage, member_free_storage) {
                Ok(value) => value, Err(_) => (None, 0usize),
            };
            settlement_raw_refines_chain_v1(before.stable.journal, contents.stable.journal, writer, evidence,
                free_storage, member_free_storage, head, count, success);
            let chain = choose|chain: Seq<usize>| #[trigger] settle_chain_relation_v1(before.stable.journal, contents.stable.journal, writer, chain, success);
            settle_preserves_issued_producer_v1(before, *contents, writer, chain, success, storage, history);
        }
    }
    result
}

#[verifier::spinoff_prover]
pub fn shared_scratch_settlement_witness_v1(empty: bool, success: bool) -> (result: bool)
    ensures result,
{
    let (mut contents, writer) = match settlement_witness_setup_v1(empty) {
        Ok(value) => value, Err(_) => return false,
    };
    let ghost storage = witness_storage_v1(3, 2);
    let ghost history = seq![writer];
    let ghost before = contents;
    let rejected = shared_scratch_settlement_issued_v1(&mut contents, writer, writer, 0, 3, success, Ghost(storage), Ghost(history));
    assert(rejected == Err(ReadErrorV1::InvalidState) && contents == before);
    let settled = shared_scratch_settlement_issued_v1(&mut contents, writer, writer, 3, 3, success, Ghost(storage), Ghost(history));
    assert(settled == Ok(()));
    assert(contents.stable.journal.writers@[writer.slot as int].is_none());
    assert(issued_producer_v1(contents, storage, history));
    true
}

}
