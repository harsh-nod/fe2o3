use super::*;

verus! {

pub open spec fn disposal_chain_refs_v1(before: JournalContentsV1, chain: Seq<usize>) -> Seq<AllocationReferenceV1> {
    Seq::new(chain.len(), |i: int| before.members@[chain[i] as int].unwrap().allocation)
}

pub open spec fn disposal_chain_relation_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, chain: Seq<usize>) -> bool
{
    &&& retained_chain_v1(before, writer, chain)
    &&& match before.writers@[writer.slot as int] { Some(WriterEntryV1::Unknown { .. }) => true, _ => false }
    &&& before.free@.len() < before.writer_capacity
    &&& before.member_free@.len() + chain.len() <= before.allocation_capacity
    &&& before.allocation_free@.len() + chain.len() <= before.allocation_capacity
    &&& disposal_frame_v1(before, after)
    &&& after.writers@ == before.writers@.update(writer.slot as int, None)
    &&& after.free@ == before.free@.push(writer.slot)
    &&& after.scratch@ == before.scratch@
    &&& after.members@ == Seq::new(before.members@.len(), |m: int|
        if chain.contains(m as usize) { None } else { before.members@[m] })
    &&& after.member_free@ == before.member_free@ + chain
    &&& after.allocations@ == Seq::new(before.allocations@.len(), |a: int|
        if selected_allocation_v1(before, chain, a) { None } else { before.allocations@[a] })
    &&& after.allocation_free@ == before.allocation_free@
        + retirement_slots_v1(disposal_chain_refs_v1(before, chain), chain.len())
}

pub proof fn disposal_cursor_canonical_v1(before: JournalContentsV1, writer: WriterReferenceV1,
    chain: Seq<usize>, head: Option<usize>, count: usize, index: nat)
    requires retained_chain_v1(before, writer, chain),
        retained_header_decision_v1(before, writer, true) == Ok((head, count, true)), index <= count,
    ensures count == chain.len(), settlement_cursor_v1(before, head, index)
        == if index == count { None } else { Some(chain[index as int]) },
    decreases index,
{
    if index > 0 {
        disposal_cursor_canonical_v1(before, writer, chain, head, count, (index - 1) as nat);
        assert(chain_link_v1(before, writer, chain, index - 1));
        reveal(chain_link_v1);
    }
}

pub proof fn disposal_selection_canonical_v1(before: JournalContentsV1, writer: WriterReferenceV1,
    chain: Seq<usize>, a: int)
    requires pending_custody_v1(before), retained_chain_v1(before, writer, chain),
        0 <= a < before.allocations@.len(),
    ensures retirement_selected_v1(disposal_chain_refs_v1(before, chain), chain.len(), a)
        == selected_allocation_v1(before, chain, a),
{
    if retirement_selected_v1(disposal_chain_refs_v1(before, chain), chain.len(), a) {
        let i = choose|i: int| 0 <= i < chain.len()
            && (#[trigger] disposal_chain_refs_v1(before, chain)[i]).slot == a;
        assert(chain.contains(chain[i]));
        selected_member_v1(before, writer, chain, chain[i]);
    }
    if selected_allocation_v1(before, chain, a) {
        let m = before.allocations@[a].unwrap().pending_member.unwrap();
        let i = choose|i: int| 0 <= i < chain.len() && chain[i] == m;
        assert(allocation_custody_v1(before, a));
        reveal(allocation_custody_v1);
        assert(disposal_chain_refs_v1(before, chain)[i].slot == a);
    }
}

pub proof fn disposal_raw_refines_chain_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
    writer_storage: usize, member_storage: usize, allocation_storage: usize, head: Option<usize>, count: usize)
    requires pending_custody_v1(before),
        disposal_plan_decision_v1(before, writer, roster, writer_storage, member_storage, allocation_storage) == Ok((head, count)),
        disposal_success_v1(before, after, writer, head, count),
    ensures exists|chain: Seq<usize>| #[trigger] disposal_chain_relation_v1(before, after, writer, chain),
{
    disposal_preflight_ready_v1(before, writer, roster, writer_storage, member_storage, allocation_storage, head, count);
    assert(writer_custody_v1(before, writer.slot as int));
    reveal(writer_custody_v1);
    let chain = choose|chain: Seq<usize>| #[trigger] retained_chain_v1(before, writer, chain);
    assert forall|i: int| 0 <= i < count implies {
        &&& settlement_slots_v1(before, head, count as nat)[i] == chain[i]
        &&& disposal_refs_v1(before, head, count as nat)[i] == disposal_chain_refs_v1(before, chain)[i]
    } by {
        disposal_cursor_canonical_v1(before, writer, chain, head, count, i as nat);
    }
    assert(settlement_slots_v1(before, head, count as nat) =~= chain);
    assert(disposal_refs_v1(before, head, count as nat) =~= disposal_chain_refs_v1(before, chain));
    assert forall|i: int| 0 <= i < chain.len() implies
        (#[trigger] disposal_chain_refs_v1(before, chain)[i]).slot < before.allocations@.len() by {
        selected_member_v1(before, writer, chain, chain[i]);
        assert(member_custody_v1(before, chain[i] as int));
        reveal(member_custody_v1);
    }
    retirement_prefix_shape_v1(before.allocations@, disposal_chain_refs_v1(before, chain), chain.len());
    disposal_prefix_shapes_v1(before, head, count, count as nat);
    assert forall|m: int| 0 <= m < before.members@.len() implies after.members@[m]
        == if chain.contains(m as usize) { None } else { before.members@[m] } by {
        settlement_member_prefix_pointwise_v1(before, head, count, count as nat, m);
    }
    assert(after.members@ =~= Seq::new(before.members@.len(), |m: int|
        if chain.contains(m as usize) { None } else { before.members@[m] }));
    assert forall|a: int| 0 <= a < before.allocations@.len() implies after.allocations@[a]
        == if selected_allocation_v1(before, chain, a) { None } else { before.allocations@[a] } by {
        disposal_selection_canonical_v1(before, writer, chain, a);
    }
    assert(after.allocations@ =~= Seq::new(before.allocations@.len(), |a: int|
        if selected_allocation_v1(before, chain, a) { None } else { before.allocations@[a] }));
    assert(disposal_chain_relation_v1(before, after, writer, chain));
}

pub proof fn disposal_partition_v1<T>(entries: Seq<Option<T>>, free: Seq<usize>, removed: Seq<usize>, after: Seq<Option<T>>)
    requires slot_partition_v1(entries, free), removed.no_duplicates(), free.len() + removed.len() <= entries.len(),
        forall|i: int| 0 <= i < removed.len() ==> (#[trigger] removed[i]) < entries.len() && entries[removed[i] as int].is_some(),
        after == Seq::new(entries.len(), |s: int| if removed.contains(s as usize) { None } else { entries[s] }),
    ensures slot_partition_v1(after, free + removed),
{
    assert forall|i: int| 0 <= i < removed.len() implies !free.contains(removed[i]) by {};
    assert((free + removed).no_duplicates());
    assert forall|i: int| 0 <= i < (free + removed).len() implies (free + removed)[i] < entries.len() by {
        if i < free.len() { assert((free + removed)[i] == free[i]); }
        else { assert((free + removed)[i] == removed[i - free.len()]); }
    }
    assert forall|s: int| 0 <= s < entries.len() implies
        (#[trigger] after[s]).is_none() == (free + removed).contains(s as usize) by {
        producer_concat_contains_v1(free, removed, s as usize);
    }
}

pub proof fn disposal_chain_slots_v1(before: JournalContentsV1, writer: WriterReferenceV1, chain: Seq<usize>)
    requires pending_custody_v1(before), retained_chain_v1(before, writer, chain),
    ensures retirement_slots_v1(disposal_chain_refs_v1(before, chain), chain.len()).no_duplicates(),
        forall|i: int| 0 <= i < chain.len() ==> {
            let a = (#[trigger] disposal_chain_refs_v1(before, chain)[i]).slot;
            &&& a < before.allocations@.len()
            &&& before.allocations@[a as int].is_some()
            &&& before.allocations@[a as int].unwrap().pending_member == Some(chain[i])
        },
{
    assert forall|i: int| 0 <= i < chain.len() implies {
        let a = (#[trigger] disposal_chain_refs_v1(before, chain)[i]).slot;
        &&& a < before.allocations@.len()
        &&& before.allocations@[a as int].is_some()
        &&& before.allocations@[a as int].unwrap().pending_member == Some(chain[i])
    } by {
        selected_member_v1(before, writer, chain, chain[i]);
        assert(member_custody_v1(before, chain[i] as int));
        reveal(member_custody_v1);
    }
    assert forall|i: int, j: int| 0 <= i < j < chain.len() implies
        retirement_slots_v1(disposal_chain_refs_v1(before, chain), chain.len())[i]
            != retirement_slots_v1(disposal_chain_refs_v1(before, chain), chain.len())[j] by {
        assert(chain[i] != chain[j]);
    }
}

pub proof fn disposal_surviving_chain_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, chain: Seq<usize>, other: WriterReferenceV1, other_chain: Seq<usize>)
    requires pending_custody_v1(before), disposal_chain_relation_v1(before, after, writer, chain),
        retained_chain_v1(before, other, other_chain), other.slot != writer.slot,
    ensures retained_chain_v1(after, other, other_chain),
{
    assert forall|i: int| 0 <= i < other_chain.len() implies !chain.contains(other_chain[i]) by {
        assert(chain_link_v1(before, other, other_chain, i));
        reveal(chain_link_v1);
        if chain.contains(other_chain[i]) { selected_member_v1(before, writer, chain, other_chain[i]); }
    }
    assert forall|i: int| 0 <= i < other_chain.len() implies
        after.members@[other_chain[i] as int] == before.members@[other_chain[i] as int] by {
        assert(chain_link_v1(before, other, other_chain, i));
        reveal(chain_link_v1);
    }
    assert forall|i: int| 0 <= i < other_chain.len() implies #[trigger] chain_link_v1(after, other, other_chain, i) by {
        assert(chain_link_v1(before, other, other_chain, i));
        reveal(chain_link_v1);
    }
}

pub proof fn disposal_preserves_pending_custody_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, chain: Seq<usize>)
    requires pending_custody_v1(before), disposal_chain_relation_v1(before, after, writer, chain),
    ensures pending_custody_v1(after),
{
    disposal_chain_slots_v1(before, writer, chain);
    let slots = retirement_slots_v1(disposal_chain_refs_v1(before, chain), chain.len());
    assert forall|a: int| 0 <= a < before.allocations@.len() implies
        selected_allocation_v1(before, chain, a) == slots.contains(a as usize) by {
        disposal_selection_canonical_v1(before, writer, chain, a);
        if retirement_selected_v1(disposal_chain_refs_v1(before, chain), chain.len(), a) {
            let i = choose|i: int| 0 <= i < chain.len() && (#[trigger] disposal_chain_refs_v1(before, chain)[i]).slot == a;
            assert(slots[i] == a);
        }
        if slots.contains(a as usize) {
            let i = choose|i: int| 0 <= i < slots.len() && slots[i] == a;
            assert(disposal_chain_refs_v1(before, chain)[i].slot == a);
        }
    }
    assert(after.allocations@ =~= Seq::new(before.allocations@.len(), |a: int|
        if slots.contains(a as usize) { None } else { before.allocations@[a] }));
    disposal_partition_v1(before.allocations@, before.allocation_free@, slots, after.allocations@);
    assert forall|i: int| 0 <= i < chain.len() implies (#[trigger] chain[i]) < before.members@.len()
        && before.members@[chain[i] as int].is_some() by {
        selected_member_v1(before, writer, chain, chain[i]);
    }
    disposal_partition_v1(before.members@, before.member_free@, chain, after.members@);
    retirement_remove_partition_v1(before.writers@, before.free@, writer.slot);
    assert forall|a: int| 0 <= a < after.allocations@.len() implies #[trigger] allocation_custody_v1(after, a) by {
        assert(allocation_custody_v1(before, a));
        reveal(allocation_custody_v1);
    }
    assert forall|m: int| 0 <= m < after.members@.len() implies #[trigger] member_custody_v1(after, m) by {
        assert(member_custody_v1(before, m));
        reveal(member_custody_v1);
        if after.members@[m].is_some() {
            let member = before.members@[m].unwrap();
            if member.writer.slot == writer.slot {
                assert(same_producer_v1(member.writer, writer));
                assert(chain.contains(m as usize));
            }
        }
    }
    assert forall|w: int| 0 <= w < after.writers@.len() implies #[trigger] writer_custody_v1(after, w) by {
        assert(writer_custody_v1(before, w));
        reveal(writer_custody_v1);
        if after.writers@[w].is_some() {
            let entry = before.writers@[w].unwrap();
            if !matches!(entry, WriterEntryV1::Reserved(_)) {
                let other = WriterReferenceV1 { slot: w as usize, key: writer_key_v1(entry) };
                let other_chain = choose|other_chain: Seq<usize>| #[trigger] retained_chain_v1(before, other, other_chain);
                disposal_surviving_chain_v1(before, after, writer, chain, other, other_chain);
            }
        }
    }
    assert forall|a: int, b: int| 0 <= a < b < after.allocations@.len()
        && (#[trigger] after.allocations@[a]).is_some() && (#[trigger] after.allocations@[b]).is_some()
        implies after.allocations@[a].unwrap().key != after.allocations@[b].unwrap().key by {
        assert(before.allocations@[a].is_some() && before.allocations@[b].is_some());
    }
}

pub proof fn disposal_preserves_issued_custody_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, chain: Seq<usize>, storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires issued_custody_v1(before, storage, history), disposal_chain_relation_v1(before, after, writer, chain),
    ensures issued_custody_v1(after, storage, history),
{
    disposal_preserves_pending_custody_v1(before, after, writer, chain);
    let pre = issuance_contents_projection_v1(before, storage, history);
    let post = issuance_contents_projection_v1(after, storage, history);
    prefix_update_v1(pre.writers, writer.slot as int, None, pre.writers.len() as int);
    assert(partition_v1(post));
    assert forall|w: int| #![trigger post.writers[w]] 0 <= w < post.writers.len() implies match post.writers[w] {
        Some(entry) => {
            &&& exists|i: int| 0 <= i < history.len() && history[i].slot == w
                && same_key_v1(history[i].key, writer_key_v1(entry))
            &&& forall|i: int| 0 <= i < history.len() && history[i].slot == w
                ==> history[i].key.local <= writer_key_v1(entry).local
        },
        None => true,
    } by { assert(post.writers[w] == pre.writers[w] || w == writer.slot); }
}

pub proof fn disposal_producer_safe_from_invariant_v1(owner: ProducerReadContentsV1,
    roster: Seq<AllocationWriteV1>, index: nat)
    requires producer_invariant_v1(owner),
    ensures disposal_producer_safe_v1(owner, roster, index),
    decreases roster.len() - index,
{
    if index < roster.len() {
        if allocation_decision_v1(owner.stable.journal, roster[index as int].allocation).is_ok() {
            reader_count_addition_fits_usize_v1(owner, roster[index as int].allocation.slot);
        }
        disposal_producer_safe_from_invariant_v1(owner, roster, index + 1);
    }
}

pub proof fn disposal_producer_zero_at_v1(owner: ProducerReadContentsV1, roster: Seq<AllocationWriteV1>, from: nat, at: nat)
    requires from <= at < roster.len(), disposal_producer_safe_v1(owner, roster, from),
        disposal_producer_unread_v1(owner, roster, from).is_ok(),
    ensures roster[at as int].allocation.slot < owner.stable.readers@.len(),
        roster[at as int].allocation.slot < owner.counts@.len(),
        owner.stable.readers@[roster[at as int].allocation.slot as int] == 0,
        owner.counts@[roster[at as int].allocation.slot as int] == 0,
    decreases at - from,
{
    if from < at { disposal_producer_zero_at_v1(owner, roster, from + 1, at); }
}

pub proof fn disposal_roster_member_v1(before: JournalContentsV1, roster: Seq<AllocationWriteV1>,
    head: Option<usize>, count: usize, from: nat, at: nat)
    requires disposal_ready_v1(before, head, count), roster.len() == count, from <= at < count,
        disposal_roster_scan_v1(before, roster, settlement_cursor_v1(before, head, from), from).is_ok(),
    ensures roster[at as int].allocation == settlement_plan_at_v1(before, head, at).allocation,
    decreases at - from,
{
    assert(settlement_plan_ready_v1(before, head, from));
    if from < at { disposal_roster_member_v1(before, roster, head, count, from + 1, at); }
}

// Resolved reads do not consult the writer slot; it may belong to a newer writer.
pub proof fn disposal_surviving_observation_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, chain: Seq<usize>, request: ProducerReadV1)
    requires pending_custody_v1(before), disposal_chain_relation_v1(before, after, writer, chain),
        producer_status_v1(before, request).is_some(),
        !selected_allocation_v1(before, chain, request.read.allocation.slot as int),
    ensures producer_status_v1(after, request) == producer_status_v1(before, request),
        producer_status_decision_v1(after, request) == producer_status_decision_v1(before, request),
{
    let a = request.read.allocation.slot as int;
    let entry = before.allocations@[a].unwrap();
    assert(after.allocations@[a] == before.allocations@[a]);
    if let Some(m) = entry.pending_member {
        assert(!chain.contains(m));
        assert(member_custody_v1(before, m as int));
        reveal(member_custody_v1);
        let member = before.members@[m as int].unwrap();
        if member.writer.slot == writer.slot {
            assert(same_producer_v1(member.writer, writer));
            assert(chain.contains(m));
        }
        assert(request.producer.slot != writer.slot);
        assert(after.writers@[request.producer.slot as int] == before.writers@[request.producer.slot as int]);
        assert(after.members@[m as int] == before.members@[m as int]);
    }
}

pub proof fn disposal_preserves_stable_readers_v1(before: ReadContentsV1, after: ReadContentsV1,
    writer: WriterReferenceV1, chain: Seq<usize>)
    requires reader_invariant_v1(before), disposal_chain_relation_v1(before.journal, after.journal, writer, chain),
        retirement_stable_frame_v1(before, after),
    ensures reader_invariant_v1(after),
{
    assert forall|a: int| 0 <= a < before.readers@.len() && before.readers@[a] > 0 implies
        after.journal.allocations@[a] == before.journal.allocations@[a] by {
        reader_count_consequences_v1(before, a as usize);
    }
    reader_allocation_frame_preserves_v1(before, after);
}

pub proof fn disposal_retained_producer_frame_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    writer: WriterReferenceV1, chain: Seq<usize>, roster: Seq<AllocationWriteV1>,
    writer_storage: usize, member_storage: usize, allocation_storage: usize, head: Option<usize>, count: usize)
    requires producer_invariant_v1(before), disposal_chain_relation_v1(before.stable.journal, after.stable.journal, writer, chain),
        disposal_plan_decision_v1(before.stable.journal, writer, roster, writer_storage, member_storage, allocation_storage) == Ok((head, count)),
        disposal_producer_unread_v1(before, roster, 0).is_ok(),
    ensures forall|s: int| 0 <= s < before.reservations@.len() && (#[trigger] before.reservations@[s]).is_some() ==> {
        let request = before.reservations@[s].unwrap().request;
        &&& producer_status_v1(after.stable.journal, request) == producer_status_v1(before.stable.journal, request)
        &&& producer_status_decision_v1(after.stable.journal, request) == producer_status_decision_v1(before.stable.journal, request)
    },
{
    let journal = before.stable.journal;
    disposal_preflight_ready_v1(journal, writer, roster, writer_storage, member_storage, allocation_storage, head, count);
    disposal_producer_safe_from_invariant_v1(before, roster, 0);
    assert forall|s: int| 0 <= s < before.reservations@.len() && (#[trigger] before.reservations@[s]).is_some() implies {
        let request = before.reservations@[s].unwrap().request;
        &&& producer_status_v1(after.stable.journal, request) == producer_status_v1(before.stable.journal, request)
        &&& producer_status_decision_v1(after.stable.journal, request) == producer_status_decision_v1(before.stable.journal, request)
    } by {
        let entry = before.reservations@[s].unwrap();
        assert(producer_entry_valid_v1(journal, entry, s, before.next_incarnation));
        reveal(producer_entry_valid_v1);
        retained_producer_positive_count_v1(before, s as usize);
        let a = entry.request.read.allocation.slot as int;
        if selected_allocation_v1(journal, chain, a) {
            let m = journal.allocations@[a].unwrap().pending_member.unwrap();
            let i = choose|i: int| 0 <= i < chain.len() && chain[i] == m;
            assert(allocation_custody_v1(journal, a));
            reveal(allocation_custody_v1);
            disposal_cursor_canonical_v1(journal, writer, chain, head, count, i as nat);
            disposal_roster_member_v1(journal, roster, head, count, 0, i as nat);
            disposal_producer_zero_at_v1(before, roster, 0, i as nat);
        }
        disposal_surviving_observation_v1(journal, after.stable.journal, writer, chain, entry.request);
    }
}

pub proof fn disposal_producer_preserves_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    writer: WriterReferenceV1, evidence: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
    writer_storage: usize, member_storage: usize, allocation_storage: usize, result: Result<(), ReadErrorV1>,
    storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires disposal_producer_relation_v1(before, after, writer, evidence, roster,
        writer_storage, member_storage, allocation_storage, result),
    ensures producer_invariant_v1(before) ==> producer_invariant_v1(after),
        issued_producer_v1(before, storage, history) ==> issued_producer_v1(after, storage, history),
        producer_invariant_v1(before) ==> {
            &&& forall|reference: ReadReferenceV1| #[trigger] lease_decision_v1(after.stable, reference)
                == lease_decision_v1(before.stable, reference)
            &&& forall|reference: ProducerReadReferenceV1| #[trigger] producer_lookup_decision_v1(after, reference)
                == producer_lookup_decision_v1(before, reference)
            &&& forall|s: int| 0 <= s < before.reservations@.len() && (#[trigger] before.reservations@[s]).is_some()
                ==> producer_status_decision_v1(after.stable.journal, before.reservations@[s].unwrap().request)
                    == producer_status_decision_v1(before.stable.journal, before.reservations@[s].unwrap().request)
        },
{
    if result.is_ok() && producer_invariant_v1(before) {
        let (head, count) = match disposal_plan_decision_v1(before.stable.journal, writer, roster,
            writer_storage, member_storage, allocation_storage) {
            Ok(value) => value, Err(_) => { assert(false); return; },
        };
        disposal_raw_refines_chain_v1(before.stable.journal, after.stable.journal, writer, roster,
            writer_storage, member_storage, allocation_storage, head, count);
        let chain = choose|chain: Seq<usize>|
            #[trigger] disposal_chain_relation_v1(before.stable.journal, after.stable.journal, writer, chain);
        disposal_preserves_pending_custody_v1(before.stable.journal, after.stable.journal, writer, chain);
        disposal_preserves_stable_readers_v1(before.stable, after.stable, writer, chain);
        disposal_retained_producer_frame_v1(before, after, writer, chain, roster,
            writer_storage, member_storage, allocation_storage, head, count);
        assert forall|s: int| 0 <= s < after.reservations@.len() && (#[trigger] after.reservations@[s]).is_some()
            implies producer_entry_valid_v1(after.stable.journal, after.reservations@[s].unwrap(), s, after.next_incarnation) by {
            assert(producer_entry_valid_v1(before.stable.journal, before.reservations@[s].unwrap(), s, before.next_incarnation));
            reveal(producer_entry_valid_v1);
        }
        assert(producer_invariant_v1(after));
        if issued_producer_v1(before, storage, history) {
            disposal_preserves_issued_custody_v1(before.stable.journal, after.stable.journal, writer, chain, storage, history);
        }
        assert forall|reference: ReadReferenceV1| #[trigger] lease_decision_v1(after.stable, reference)
            == lease_decision_v1(before.stable, reference) by {
            if reference.slot < before.stable.leases@.len() && before.stable.leases@[reference.slot as int].is_some() {
                let entry = before.stable.leases@[reference.slot as int].unwrap();
                assert(read_decision_v1(before.stable.journal, entry.request) == Ok(()));
                assert(read_decision_v1(after.stable.journal, entry.request) == Ok(()));
            }
        }
        assert forall|reference: ProducerReadReferenceV1| #[trigger] producer_lookup_decision_v1(after, reference)
            == producer_lookup_decision_v1(before, reference) by {
            if reference.slot < before.reservations@.len() && before.reservations@[reference.slot as int].is_some() {
                let entry = before.reservations@[reference.slot as int].unwrap();
                assert(producer_status_decision_v1(after.stable.journal, entry.request)
                    == producer_status_decision_v1(before.stable.journal, entry.request));
            }
        }
    }
}

}
