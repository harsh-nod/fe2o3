
verus! {

// Production instrumentation is empty outside tests and cannot decide admission.
pub fn retained_indexed_access_v1(journal: &JournalContentsV1) {}

pub fn shared_retained_writer_key_v1(left: WriterKeyV1, right: WriterKeyV1) -> (result: bool)
    ensures result == (left == right),
{
    retained_writer_key_body!(left, right)
}

pub fn shared_retained_allocation_less_v1(left: AllocationKeyV1, right: AllocationKeyV1) -> (result: bool)
    ensures result == enrollment_key_less_v1(left, right),
{
    retained_allocation_less_body!(left, right)
}

pub fn shared_retained_allocation_v1(journal: &JournalContentsV1, reference: AllocationReferenceV1)
    -> (result: Result<AllocationEntryV1, ReadErrorV1>)
    ensures result == begin_exact_allocation_v1(*journal, reference),
{
    retained_allocation_body!(journal, reference)
}

pub fn shared_retained_header_v1(journal: &JournalContentsV1, writer: WriterReferenceV1, allow_unknown: bool)
    -> (result: Result<(Option<usize>, usize, bool), ReadErrorV1>)
    ensures result == retained_header_decision_v1(*journal, writer, allow_unknown),
{
    retained_header_body!(journal, writer, allow_unknown)
}

pub fn shared_retained_member_v1(journal: &JournalContentsV1, writer: WriterReferenceV1,
    head: Option<usize>, previous: Option<AllocationKeyV1>) -> (result: Result<MemberEntryV1, ReadErrorV1>)
    ensures result == retained_member_decision_v1(*journal, writer, head, previous),
{
    retained_member_body!(journal, writer, head, previous)
}

pub fn shared_retained_chain_v1(journal: &JournalContentsV1, writer: WriterReferenceV1, initial: Option<usize>, count: usize)
    -> (result: Result<(), ReadErrorV1>)
    ensures result == retained_chain_decision_v1(*journal, writer, initial, count),
{
    retained_chain_body!(verus_exec_expr, journal, writer, initial, count, head, previous, index, [
        invariant index <= count, count <= journal.allocation_capacity, (count == 0) == initial.is_none(),
            retained_scan_v1(*journal, writer, initial, count as nat, None)
                == retained_scan_v1(*journal, writer, head, (count - index) as nat, previous),
        decreases count - index,
    ])
}

pub fn shared_retained_unknown_v1(journal: &mut JournalContentsV1, writer: WriterReferenceV1)
    -> (result: Result<(), ReadErrorV1>)
    ensures unknown_execution_relation_v1(*old(journal), *final(journal), writer, result),
{
    let ghost before = *journal;
    let result = retained_unknown_body!(journal, writer);
    proof {
        assert(result == unknown_decision_v1(before, writer));
        assert(result.is_err() ==> *journal == before);
        assert(match retained_header_decision_v1(before, writer, true) {
            Ok((_, _, true)) => *journal == before, _ => true,
        });
        if result.is_ok() {
            assert(journal.writers@ =~= before.writers@.update(writer.slot as int,
                Some(unknown_writer_v1(before.writers@[writer.slot as int].unwrap()))));
            assert(mark_unknown_relation_v1(before, *journal, writer));
        }
    }
    result
}

pub fn shared_retained_unknown_issued_v1(contents: &mut ProducerReadContentsV1, writer: WriterReferenceV1,
    Ghost(storage): Ghost<StorageCapacitiesV1>, Ghost(history): Ghost<Seq<WriterReferenceV1>>)
    -> (result: Result<(), ReadErrorV1>)
    requires issued_producer_v1(*old(contents), storage, history),
    ensures unknown_issued_relation_v1(*old(contents), *final(contents), writer, result, storage, history),
        match retained_header_decision_v1(old(contents).stable.journal, writer, true) {
            Ok((_, _, true)) => *final(contents) == *old(contents), _ => true,
        },
{
    let ghost before = *contents;
    let result = shared_retained_unknown_v1(&mut contents.stable.journal, writer);
    proof { if result.is_ok() { unknown_preserves_issued_producer_v1(before, *contents, writer, storage, history); } }
    result
}

#[verifier::spinoff_prover]
pub fn shared_retained_unknown_witness_v1(empty: bool) -> (result: bool)
    ensures result,
{
    proof { reveal_with_fuel(retained_scan_v1, 4); }
    let (mut contents, writer) = match settlement_witness_setup_v1(empty) {
        Ok(value) => value, Err(_) => return false,
    };
    let ghost storage = witness_storage_v1(3, 2);
    let ghost history = seq![writer];
    let marked = shared_retained_unknown_issued_v1(&mut contents, writer, Ghost(storage), Ghost(history));
    assert(marked == Ok(()));
    let ghost head = if empty { None } else { Some(0usize) };
    let ghost count = if empty { 0usize } else { 2usize };
    assert(contents.stable.journal.writers@[writer.slot as int] == Some(WriterEntryV1::Unknown { key: writer.key, head, count }));
    assert(retained_header_decision_v1(contents.stable.journal, writer, true) == Ok((head, count, true)));
    let ghost before_repeat = contents;
    let repeated = shared_retained_unknown_issued_v1(&mut contents, writer, Ghost(storage), Ghost(history));
    assert(repeated == Ok(()));
    assert(contents == before_repeat);
    assert(issued_producer_v1(contents, storage, history));
    true
}

pub fn shared_retained_preflight_v1(journal: &JournalContentsV1, writer: WriterReferenceV1,
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
    match shared_settlement_return_exec_v1(journal, count, free_storage, member_free_storage) {
        Ok(value) => { assert(value == ()); }, Err(error) => return Err(error),
    };
    Ok((head, count))
}

pub fn shared_retained_settlement_v1(journal: &mut JournalContentsV1, writer: WriterReferenceV1, evidence: WriterReferenceV1,
    free_storage: usize, member_free_storage: usize, success: bool) -> (result: Result<(), ReadErrorV1>)
    ensures settlement_execution_relation_v1(*old(journal), *final(journal), writer, evidence,
        free_storage, member_free_storage, success, result),
{
    let ghost before = *journal;
    let (head, count) = match shared_retained_preflight_v1(journal, writer, evidence, free_storage, member_free_storage) {
        Ok(value) => value, Err(error) => return Err(error),
    };
    proof { settlement_preflight_ready_v1(before, writer, evidence, free_storage, member_free_storage, head, count); }
    settlement_stage_exec_v1(journal, head, count);
    settlement_commit_exec_v1(journal, writer, head, count, success, Ghost(before));
    Ok(())
}

pub fn shared_retained_settlement_issued_v1(contents: &mut ProducerReadContentsV1, writer: WriterReferenceV1,
    evidence: WriterReferenceV1, free_storage: usize, member_free_storage: usize, success: bool,
    Ghost(storage): Ghost<StorageCapacitiesV1>, Ghost(history): Ghost<Seq<WriterReferenceV1>>)
    -> (result: Result<(), ReadErrorV1>)
    requires issued_producer_v1(*old(contents), storage, history),
    ensures settlement_issued_execution_v1(*old(contents), *final(contents), writer, evidence, free_storage,
        member_free_storage, success, result, storage, history),
{
    let ghost before = *contents;
    let result = shared_retained_settlement_v1(&mut contents.stable.journal, writer, evidence, free_storage, member_free_storage, success);
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
pub fn shared_retained_settlement_witness_v1(empty: bool, success: bool) -> (result: bool)
    ensures result,
{
    let (mut contents, writer) = match settlement_witness_setup_v1(empty) {
        Ok(value) => value, Err(_) => return false,
    };
    let ghost storage = witness_storage_v1(3, 2);
    let ghost history = seq![writer];
    let settled = shared_retained_settlement_issued_v1(&mut contents, writer, writer, 3, 3, success, Ghost(storage), Ghost(history));
    assert(settled == Ok(()));
    assert(contents.stable.journal.writers@[writer.slot as int].is_none());
    assert(issued_producer_v1(contents, storage, history));
    true
}

}
