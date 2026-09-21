// Compose exact logical settlement with issued custody; native admission remains separate.
include!("context_version_journal_settlement_commit_v1.rs");

verus! {

pub proof fn settlement_cursor_canonical_v1(before: JournalContentsV1, writer: WriterReferenceV1,
    chain: Seq<usize>, head: Option<usize>, count: usize, index: nat)
    requires retained_chain_v1(before, writer, chain),
        retained_header_decision_v1(before, writer, false) == Ok((head, count, false)), index <= count,
    ensures count == chain.len(), settlement_cursor_v1(before, head, index)
        == if index == count { None } else { Some(chain[index as int]) },
    decreases index,
{
    if index > 0 {
        settlement_cursor_canonical_v1(before, writer, chain, head, count, (index - 1) as nat);
        assert(chain_link_v1(before, writer, chain, index - 1));
        reveal(chain_link_v1);
    }
}

pub proof fn settlement_slots_canonical_v1(before: JournalContentsV1, writer: WriterReferenceV1,
    chain: Seq<usize>, head: Option<usize>, count: usize)
    requires retained_chain_v1(before, writer, chain),
        retained_header_decision_v1(before, writer, false) == Ok((head, count, false)),
    ensures settlement_slots_v1(before, head, count as nat) == chain,
{
    assert forall|i: int| 0 <= i < count implies settlement_slots_v1(before, head, count as nat)[i] == chain[i] by {
        settlement_cursor_canonical_v1(before, writer, chain, head, count, i as nat);
    }
    assert(settlement_slots_v1(before, head, count as nat) =~= chain);
}

pub proof fn settlement_member_prefix_pointwise_v1(before: JournalContentsV1, head: Option<usize>, total: usize, count: nat, m: int)
    requires settlement_storage_ready_v1(before, head, total), count <= total, 0 <= m < before.members@.len(), m <= usize::MAX,
    ensures settlement_members_prefix_v1(before, head, count)[m]
        == if settlement_slots_v1(before, head, count).contains(m as usize) { None } else { before.members@[m] },
    decreases count,
{
    if count > 0 {
        settlement_member_prefix_pointwise_v1(before, head, total, (count - 1) as nat, m);
        let slot = settlement_plan_at_v1(before, head, (count - 1) as nat).member_slot;
        assert(settlement_slots_v1(before, head, count) =~= settlement_slots_v1(before, head, (count - 1) as nat).push(slot));
        assert(settlement_plan_ready_v1(before, head, (count - 1) as nat));
        settlement_prefix_shapes_v1(before, head, total, (count - 1) as nat, false);
        assert(settlement_members_prefix_v1(before, head, count)[m]
            == if m == slot { None } else { settlement_members_prefix_v1(before, head, (count - 1) as nat)[m] });
        vstd::seq_lib::lemma_seq_contains_after_push(settlement_slots_v1(before, head, (count - 1) as nat), slot, m as usize);
        assert(settlement_slots_v1(before, head, count).contains(m as usize)
            == (settlement_slots_v1(before, head, (count - 1) as nat).contains(m as usize) || slot == m));
    } else {
        assert(!settlement_slots_v1(before, head, count).contains(m as usize));
    }
}

pub open spec fn settlement_plan_selection_v1(before: JournalContentsV1, head: Option<usize>, count: nat, a: int) -> bool {
    exists|i: nat| i < count && (#[trigger] settlement_plan_at_v1(before, head, i)).allocation.slot == a
}

pub proof fn settlement_selection_step_v1(before: JournalContentsV1, head: Option<usize>, count: nat, a: int)
    requires count > 0,
    ensures settlement_plan_selection_v1(before, head, count, a)
        == (settlement_plan_selection_v1(before, head, (count - 1) as nat, a)
            || settlement_plan_at_v1(before, head, (count - 1) as nat).allocation.slot == a),
{
    if settlement_plan_selection_v1(before, head, count, a) {
        let i = choose|i: nat| i < count && (#[trigger] settlement_plan_at_v1(before, head, i)).allocation.slot == a;
        if i < count - 1 { assert(settlement_plan_selection_v1(before, head, (count - 1) as nat, a)); }
    } else if settlement_plan_selection_v1(before, head, (count - 1) as nat, a) {
        let i = choose|i: nat| i < count - 1 && (#[trigger] settlement_plan_at_v1(before, head, i)).allocation.slot == a;
        assert(settlement_plan_selection_v1(before, head, count, a));
    }
}

pub open spec fn settlement_plan_epochs_v1(before: JournalContentsV1, head: Option<usize>, total: usize) -> bool {
    forall|i: nat| i < total ==> {
        let plan = #[trigger] settlement_plan_at_v1(before, head, i);
        plan.attempt_epoch == before.allocations@[plan.allocation.slot as int].unwrap().attempt_epoch
    }
}

pub proof fn settlement_allocation_prefix_pointwise_v1(before: JournalContentsV1, head: Option<usize>, total: usize,
    count: nat, success: bool, a: int)
    requires settlement_storage_ready_v1(before, head, total), settlement_plan_epochs_v1(before, head, total),
        count <= total, 0 <= a < before.allocations@.len(),
    ensures settlement_allocations_prefix_v1(before, head, success, count)[a]
        == if settlement_plan_selection_v1(before, head, count, a) {
            Some(settled_allocation_v1(before.allocations@[a].unwrap(), success))
        } else { before.allocations@[a] },
    decreases count,
{
    if count > 0 {
        settlement_allocation_prefix_pointwise_v1(before, head, total, (count - 1) as nat, success, a);
        settlement_selection_step_v1(before, head, count, a);
        assert(settlement_plan_ready_v1(before, head, (count - 1) as nat));
        let plan = settlement_plan_at_v1(before, head, (count - 1) as nat);
        assert(plan.attempt_epoch == before.allocations@[plan.allocation.slot as int].unwrap().attempt_epoch);
        settlement_prefix_shapes_v1(before, head, total, (count - 1) as nat, success);
    }
}

pub proof fn settlement_selection_canonical_v1(before: JournalContentsV1, writer: WriterReferenceV1,
    chain: Seq<usize>, head: Option<usize>, count: usize, a: int)
    requires pending_custody_v1(before), retained_chain_v1(before, writer, chain),
        retained_header_decision_v1(before, writer, false) == Ok((head, count, false)),
        0 <= a < before.allocations@.len(),
    ensures settlement_plan_selection_v1(before, head, count as nat, a) == selected_allocation_v1(before, chain, a),
{
    if settlement_plan_selection_v1(before, head, count as nat, a) {
        let i = choose|i: nat| i < count && (#[trigger] settlement_plan_at_v1(before, head, i)).allocation.slot == a;
        settlement_cursor_canonical_v1(before, writer, chain, head, count, i);
        assert(chain.contains(chain[i as int]));
        selected_member_v1(before, writer, chain, chain[i as int]);
    }
    if selected_allocation_v1(before, chain, a) {
        let m = before.allocations@[a].unwrap().pending_member.unwrap();
        let i = choose|i: int| 0 <= i < chain.len() && chain[i] == m;
        assert(allocation_custody_v1(before, a));
        reveal(allocation_custody_v1);
        settlement_cursor_canonical_v1(before, writer, chain, head, count, i as nat);
        assert(settlement_plan_at_v1(before, head, i as nat).allocation.slot == a);
        assert(settlement_plan_selection_v1(before, head, count as nat, a));
    }
}

#[verifier::spinoff_prover]
pub proof fn settlement_raw_refines_chain_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, evidence: WriterReferenceV1, free_storage: usize, member_free_storage: usize,
    head: Option<usize>, count: usize, success: bool)
    requires pending_custody_v1(before),
        settlement_preflight_decision_v1(before, writer, evidence, free_storage, member_free_storage) == Ok((head, count)),
        settlement_raw_success_relation_v1(before, after, writer, head, count, success),
    ensures exists|chain: Seq<usize>| #[trigger] settle_chain_relation_v1(before, after, writer, chain, success),
{
    settlement_preflight_ready_v1(before, writer, evidence, free_storage, member_free_storage, head, count);
    assert(writer_custody_v1(before, writer.slot as int));
    reveal(writer_custody_v1);
    let chain = choose|chain: Seq<usize>| #[trigger] retained_chain_v1(before, writer, chain);
    settlement_slots_canonical_v1(before, writer, chain, head, count);
    assert forall|i: nat| i < count implies {
        let plan = #[trigger] settlement_plan_at_v1(before, head, i);
        plan.attempt_epoch == before.allocations@[plan.allocation.slot as int].unwrap().attempt_epoch
    } by {
        settlement_scan_plan_v1(before, writer, head, count, i);
    }
    settlement_prefix_shapes_v1(before, head, count, count as nat, success);
    assert forall|m: int| 0 <= m < before.members@.len() implies after.members@[m]
        == if chain.contains(m as usize) { None } else { before.members@[m] } by {
        settlement_member_prefix_pointwise_v1(before, head, count, count as nat, m);
    }
    assert(after.members@ =~= Seq::new(before.members@.len(), |m: int|
        if chain.contains(m as usize) { None } else { before.members@[m] }));
    assert forall|a: int| 0 <= a < before.allocations@.len() implies after.allocations@[a]
        == if selected_allocation_v1(before, chain, a) {
            Some(settled_allocation_v1(before.allocations@[a].unwrap(), success))
        } else { before.allocations@[a] } by {
        settlement_allocation_prefix_pointwise_v1(before, head, count, count as nat, success, a);
        settlement_selection_canonical_v1(before, writer, chain, head, count, a);
    }
    assert(after.allocations@ =~= Seq::new(before.allocations@.len(), |a: int|
        if selected_allocation_v1(before, chain, a) {
            Some(settled_allocation_v1(before.allocations@[a].unwrap(), success))
        } else { before.allocations@[a] }));
    assert(settle_chain_relation_v1(before, after, writer, chain, success));
}

pub open spec fn settlement_issued_execution_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    writer: WriterReferenceV1, evidence: WriterReferenceV1, free_storage: usize, member_free_storage: usize,
    success: bool, result: Result<(), ReadErrorV1>, storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>) -> bool
{
    &&& settlement_execution_relation_v1(before.stable.journal, after.stable.journal, writer, evidence,
        free_storage, member_free_storage, success, result)
    &&& producer_storage_frame_v1(before, after)
    &&& issued_producer_v1(after, storage, history)
    &&& result.is_err() ==> after == before
}

pub fn settlement_issued_exec_v1(contents: &mut ProducerReadContentsV1, writer: WriterReferenceV1,
    evidence: WriterReferenceV1, free_storage: usize, member_free_storage: usize, success: bool,
    Ghost(storage): Ghost<StorageCapacitiesV1>, Ghost(history): Ghost<Seq<WriterReferenceV1>>)
    -> (result: Result<(), ReadErrorV1>)
    requires issued_producer_v1(*old(contents), storage, history),
    ensures settlement_issued_execution_v1(*old(contents), *final(contents), writer, evidence, free_storage,
        member_free_storage, success, result, storage, history),
{
    let ghost before = *contents;
    let result = settlement_exec_v1(&mut contents.stable.journal, writer, evidence, free_storage, member_free_storage, success);
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
    if result.is_ok() { return Err(ReadErrorV1::InvalidState); }
    result
}

#[verifier::spinoff_prover]
pub fn settlement_commit_constructor_witness_v1(empty: bool, success: bool) -> (result: bool)
    ensures result,
{
    let (mut contents, writer) = match settlement_witness_setup_v1(empty) {
        Ok(value) => value, Err(_) => return false,
    };
    let ghost before = contents;
    let ghost storage = witness_storage_v1(3, 2);
    let ghost history = seq![writer];
    let wrong = WriterReferenceV1 { slot: usize::MAX, key: writer.key };
    let rejected = settlement_issued_exec_v1(&mut contents, writer, wrong, 0, 0, success, Ghost(storage), Ghost(history));
    assert(rejected == Err(ReadErrorV1::SettlementEvidenceMismatch) && contents == before);
    let rejected = settlement_issued_exec_v1(&mut contents, writer, writer, 0, 3, success, Ghost(storage), Ghost(history));
    assert(rejected == Err(ReadErrorV1::InvalidState) && contents == before);
    let settled = settlement_issued_exec_v1(&mut contents, writer, writer, 3, 3, success, Ghost(storage), Ghost(history));
    assert(settled == Ok(()));
    assert(contents.stable.journal.writers@[writer.slot as int].is_none());
    assert(contents.stable.journal.free@ == before.stable.journal.free@.push(writer.slot));
    assert(issued_producer_v1(contents, storage, history));
    let ghost settled_state = contents;
    let repeated = settlement_issued_exec_v1(&mut contents, writer, wrong, 0, 0, success, Ghost(storage), Ghost(history));
    assert(repeated == Err(ReadErrorV1::InvalidReference) && contents == settled_state);
    true
}

}
