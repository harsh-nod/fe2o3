
verus! {

pub struct SettlementReturnStorageV1 {
    pub writer_free_len: usize,
    pub member_free_len: usize,
    pub writer_limit: usize,
    pub writer_storage: usize,
    pub member_limit: usize,
    pub member_storage: usize,
    pub scratch_len: usize,
}

pub open spec fn shared_settlement_storage_decision_v1(storage: SettlementReturnStorageV1, count: usize) -> Result<(), ReadErrorV1> {
    let writers = storage.writer_free_len as int + 1;
    let members = storage.member_free_len as int + count;
    if writers > usize::MAX || members > usize::MAX || writers > storage.writer_limit || writers > storage.writer_storage
        || members > storage.member_limit || members > storage.member_storage || count > storage.scratch_len {
        Err(ReadErrorV1::InvalidState)
    } else { Ok(()) }
}

pub fn shared_settlement_storage_exec_v1(storage: &SettlementReturnStorageV1, count: usize) -> (result: Result<(), ReadErrorV1>)
    ensures result == shared_settlement_storage_decision_v1(*storage, count),
{
    settlement_return_admission_body!(storage, count, ReadErrorV1::InvalidState)
}

pub fn shared_settlement_return_exec_v1(journal: &JournalContentsV1, count: usize,
    free_storage: usize, member_free_storage: usize) -> (result: Result<(), ReadErrorV1>)
    ensures result == settlement_return_decision_v1(*journal, count, free_storage, member_free_storage),
{
    let storage = SettlementReturnStorageV1 {
        writer_free_len: journal.free.len(), member_free_len: journal.member_free.len(),
        writer_limit: journal.writer_capacity, writer_storage: free_storage,
        member_limit: journal.allocation_capacity, member_storage: member_free_storage,
        scratch_len: journal.scratch.len(),
    };
    let wrong_observation = SettlementReturnStorageV1 {
        writer_free_len: journal.free.len(), member_free_len: journal.member_free.len(),
        writer_limit: journal.writer_capacity, writer_storage: member_free_storage,
        member_limit: journal.allocation_capacity, member_storage: member_free_storage,
        scratch_len: journal.scratch.len(),
    };
    match shared_settlement_storage_exec_v1(&wrong_observation, count) {
        Ok(_) => {}, Err(error) => return Err(error),
    };
    match shared_settlement_storage_exec_v1(&storage, count) {
        Ok(value) => { assert(value == ()); }, Err(error) => return Err(error),
    }
    let mut index = 0usize;
    while index < count
        invariant index <= count, count <= journal.scratch@.len(),
            settlement_return_decision_v1(*journal, count, free_storage, member_free_storage)
                == settlement_scratch_scan_v1(*journal, count, index as nat),
        decreases count - index,
    {
        if journal.scratch[index].is_some() { return Err(ReadErrorV1::InvalidState); }
        index += 1;
    }
    Ok(())
}

pub fn shared_settlement_preflight_exec_v1(journal: &JournalContentsV1, writer: WriterReferenceV1,
    evidence: WriterReferenceV1, free_storage: usize, member_free_storage: usize)
    -> (result: Result<(Option<usize>, usize), ReadErrorV1>)
    ensures result == settlement_preflight_decision_v1(*journal, writer, evidence, free_storage, member_free_storage),
{
    let (head, count, _) = match retained_header_exec_v1(journal, writer, false) {
        Ok(value) => value, Err(error) => return Err(error),
    };
    if evidence.slot != writer.slot || !same_key_exec_v1(evidence.key, writer.key) { return Err(ReadErrorV1::SettlementEvidenceMismatch); }
    match retained_chain_exec_v1(journal, writer, head, count) { Ok(_) => {}, Err(error) => return Err(error) };
    match shared_settlement_return_exec_v1(journal, count, free_storage, member_free_storage) { Ok(_) => {}, Err(error) => return Err(error) };
    Ok((head, count))
}

pub fn shared_settlement_exec_v1(journal: &mut JournalContentsV1, writer: WriterReferenceV1, evidence: WriterReferenceV1,
    free_storage: usize, member_free_storage: usize, success: bool) -> (result: Result<(), ReadErrorV1>)
    ensures settlement_execution_relation_v1(*old(journal), *final(journal), writer, evidence,
        free_storage, member_free_storage, success, result),
{
    let ghost before = *journal;
    let (head, count) = match shared_settlement_preflight_exec_v1(journal, writer, evidence, free_storage, member_free_storage) {
        Ok(value) => value, Err(error) => return Err(error),
    };
    proof { settlement_preflight_ready_v1(before, writer, evidence, free_storage, member_free_storage, head, count); }
    settlement_stage_exec_v1(journal, head, count);
    settlement_commit_exec_v1(journal, writer, head, count, success, Ghost(before));
    Ok(())
}

pub fn shared_settlement_issued_exec_v1(contents: &mut ProducerReadContentsV1, writer: WriterReferenceV1,
    evidence: WriterReferenceV1, free_storage: usize, member_free_storage: usize, success: bool,
    Ghost(storage): Ghost<StorageCapacitiesV1>, Ghost(history): Ghost<Seq<WriterReferenceV1>>)
    -> (result: Result<(), ReadErrorV1>)
    requires issued_producer_v1(*old(contents), storage, history),
    ensures settlement_issued_execution_v1(*old(contents), *final(contents), writer, evidence, free_storage,
        member_free_storage, success, result, storage, history),
{
    let ghost before = *contents;
    let result = shared_settlement_exec_v1(&mut contents.stable.journal, writer, evidence, free_storage, member_free_storage, success);
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
pub fn shared_settlement_constructor_witness_v1(empty: bool, success: bool) -> (result: bool)
    ensures result,
{
    let (mut contents, writer) = match settlement_witness_setup_v1(empty) {
        Ok(value) => value, Err(_) => return false,
    };
    let ghost before = contents;
    let ghost storage = witness_storage_v1(3, 2);
    let ghost history = seq![writer];
    let rejected = shared_settlement_issued_exec_v1(&mut contents, writer, writer, 0, 3, success, Ghost(storage), Ghost(history));
    assert(rejected == Err(ReadErrorV1::InvalidState) && contents == before);
    let settled = shared_settlement_issued_exec_v1(&mut contents, writer, writer, 3, 3, success, Ghost(storage), Ghost(history));
    assert(settled == Ok(()));
    assert(contents.stable.journal.writers@[writer.slot as int].is_none());
    assert(issued_producer_v1(contents, storage, history));
    true
}

}
