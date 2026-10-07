// Shared settlement commit; physical storage and whole-wrapper refinement remain separate.
include!("context_shared_settlement_scratch_v1.rs");
include!("../src/context_version_journal/settlement_commit_body.rs");

verus! {

pub fn settlement_commit_access_v1(journal: &JournalContentsV1) {}

pub fn shared_settlement_commit_v1(journal: &mut JournalContentsV1, writer: WriterReferenceV1,
    head: Option<usize>, count: usize, success: bool, Ghost(before): Ghost<JournalContentsV1>)
    requires settlement_storage_ready_v1(before, head, count), writer.slot < before.writers@.len(),
        begin_stage_frame_v1(before, *old(journal)),
        old(journal).scratch@ == settlement_scratch_v1(before, head, count, count as nat, 0),
    ensures settlement_raw_success_relation_v1(before, *final(journal), writer, head, count, success),
{
    proof {
        settlement_prefix_shapes_v1(before, head, count, 0, success);
        assert(journal.member_free@ =~= before.member_free@ + settlement_slots_v1(before, head, 0));
    }
    settlement_commit_body!(verus_exec_expr, journal, writer, count, success, index, [
        invariant index <= count, settlement_storage_ready_v1(before, head, count), writer.slot < before.writers@.len(),
            settlement_commit_frame_v1(before, *journal), journal.writers == before.writers, journal.free == before.free,
            journal.scratch@ =~= settlement_scratch_v1(before, head, count, count as nat, index as nat),
            journal.allocations@ == settlement_allocations_prefix_v1(before, head, success, index as nat),
            journal.members@ == settlement_members_prefix_v1(before, head, index as nat),
            journal.member_free@ =~= before.member_free@ + settlement_slots_v1(before, head, index as nat),
            journal.members@.len() == before.members@.len(), journal.allocations@.len() == before.allocations@.len(),
            forall|a: int| 0 <= a < before.allocations@.len() ==>
                (#[trigger] journal.allocations@[a]).is_some() == before.allocations@[a].is_some(),
            index < count ==> settlement_plan_ready_v1(before, head, index as nat),
        decreases count - index,
    ]);
    proof { assert(journal.scratch@ =~= before.scratch@); }
}

pub fn shared_commit_settlement_v1(journal: &mut JournalContentsV1, writer: WriterReferenceV1, evidence: WriterReferenceV1,
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
    shared_settlement_commit_v1(journal, writer, head, count, success, Ghost(before));
    Ok(())
}

pub fn shared_commit_settlement_issued_v1(contents: &mut ProducerReadContentsV1, writer: WriterReferenceV1,
    evidence: WriterReferenceV1, free_storage: usize, member_free_storage: usize, success: bool,
    Ghost(storage): Ghost<StorageCapacitiesV1>, Ghost(history): Ghost<Seq<WriterReferenceV1>>)
    -> (result: Result<(), ReadErrorV1>)
    requires issued_producer_v1(*old(contents), storage, history),
    ensures settlement_issued_execution_v1(*old(contents), *final(contents), writer, evidence, free_storage,
        member_free_storage, success, result, storage, history),
{
    let ghost before = *contents;
    let result = shared_commit_settlement_v1(&mut contents.stable.journal, writer, evidence, free_storage, member_free_storage, success);
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
pub fn shared_commit_settlement_witness_v1(empty: bool, success: bool) -> (result: bool)
    ensures result,
{
    let (mut contents, writer) = match settlement_witness_setup_v1(empty) {
        Ok(value) => value, Err(_) => return false,
    };
    let ghost storage = witness_storage_v1(3, 2);
    let ghost history = seq![writer];
    let ghost before = contents;
    let rejected = shared_commit_settlement_issued_v1(&mut contents, writer, writer, 0, 3, success, Ghost(storage), Ghost(history));
    assert(rejected == Err(ReadErrorV1::InvalidState) && contents == before);
    let settled = shared_commit_settlement_issued_v1(&mut contents, writer, writer, 3, 3, success, Ghost(storage), Ghost(history));
    assert(settled == Ok(()));
    assert(contents.stable.journal.writers@[writer.slot as int].is_none());
    assert(issued_producer_v1(contents, storage, history));
    true
}

}
