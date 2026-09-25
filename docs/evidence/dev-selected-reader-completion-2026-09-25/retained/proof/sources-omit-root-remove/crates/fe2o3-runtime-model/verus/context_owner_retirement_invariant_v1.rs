use super::*;

verus! {

pub proof fn retirement_scan_item_at_v1(journal: JournalContentsV1, roster: Seq<AllocationReferenceV1>,
    from: nat, previous: Option<AllocationKeyV1>, at: nat)
    requires from <= at < roster.len(), retirement_scan_v1(journal, roster, from, previous).is_ok(),
    ensures retirement_item_v1(journal, roster[at as int],
        if at == from { previous } else { Some(roster[at - 1].key) }).is_ok(),
    decreases at - from,
{
    if from < at { retirement_scan_item_at_v1(journal, roster, from + 1, Some(roster[from as int].key), at); }
}

pub proof fn retirement_order_v1(journal: JournalContentsV1, roster: Seq<AllocationReferenceV1>, i: int, k: int)
    requires retirement_scan_v1(journal, roster, 0, None).is_ok(), 0 <= i < k < roster.len(),
    ensures roster[i].key.local < roster[k].key.local, roster[i].slot != roster[k].slot,
    decreases k - i,
{
    retirement_scan_at_v1(journal, roster, 0, None, i as nat);
    retirement_scan_at_v1(journal, roster, 0, None, k as nat);
    retirement_scan_at_v1(journal, roster, 0, None, (k - 1) as nat);
    retirement_scan_item_at_v1(journal, roster, 0, None, k as nat);
    if i < k - 1 { retirement_order_v1(journal, roster, i, k - 1); }
}

pub open spec fn retirement_selected_v1(roster: Seq<AllocationReferenceV1>, count: nat, a: int) -> bool {
    exists|i: int| 0 <= i < count && (#[trigger] roster[i]).slot == a
}

pub proof fn retirement_prefix_shape_v1(allocations: Seq<Option<AllocationEntryV1>>,
    roster: Seq<AllocationReferenceV1>, count: nat)
    requires count <= roster.len(),
        forall|i: int| 0 <= i < roster.len() ==> (#[trigger] roster[i]).slot < allocations.len(),
    ensures retirement_prefix_v1(allocations, roster, count).len() == allocations.len(),
        forall|a: int| 0 <= a < allocations.len() ==> (#[trigger] retirement_prefix_v1(allocations, roster, count)[a])
            == if retirement_selected_v1(roster, count, a) { None } else { allocations[a] },
    decreases count,
{
    if count > 0 {
        retirement_prefix_shape_v1(allocations, roster, (count - 1) as nat);
        assert forall|a: int| 0 <= a < allocations.len() implies
            retirement_selected_v1(roster, count, a)
                == (retirement_selected_v1(roster, (count - 1) as nat, a) || roster[count - 1].slot == a) by {
            if retirement_selected_v1(roster, count, a) {
                let i = choose|i: int| 0 <= i < count && roster[i].slot == a;
                if i < count - 1 { assert(retirement_selected_v1(roster, (count - 1) as nat, a)); }
            }
        }
    }
}

pub proof fn retirement_remove_partition_v1<T>(entries: Seq<Option<T>>, free: Seq<usize>, slot: usize)
    requires slot_partition_v1(entries, free), slot < entries.len(), entries[slot as int].is_some(),
        free.len() < entries.len() <= usize::MAX,
    ensures slot_partition_v1(entries.update(slot as int, None), free.push(slot)),
{
    assert(!free.contains(slot));
    assert(free.push(slot).no_duplicates());
    assert forall|s: int| 0 <= s < entries.len() implies
        (#[trigger] entries.update(slot as int, None)[s]).is_none() == free.push(slot).contains(s as usize) by {
        vstd::seq_lib::lemma_seq_contains_after_push(free, slot, s as usize);
        assert(free.push(slot).contains(s as usize) == (free.contains(s as usize) || s == slot));
    }
}

pub proof fn retirement_prefix_partition_v1(journal: JournalContentsV1, roster: Seq<AllocationReferenceV1>,
    capacity: usize, count: nat)
    requires pending_custody_v1(journal), retirement_decision_v1(journal, roster, capacity).is_ok(), count <= roster.len(),
    ensures slot_partition_v1(retirement_prefix_v1(journal.allocations@, roster, count),
        journal.allocation_free@ + retirement_slots_v1(roster, count)),
    decreases count,
{
    retirement_ready_v1(journal, roster, capacity);
    if count == 0 {
        assert(retirement_slots_v1(roster, 0) =~= Seq::<usize>::empty());
        assert(journal.allocation_free@ + retirement_slots_v1(roster, 0) =~= journal.allocation_free@);
    } else {
        retirement_prefix_partition_v1(journal, roster, capacity, (count - 1) as nat);
        retirement_prefix_shape_v1(journal.allocations@, roster, (count - 1) as nat);
        retirement_scan_at_v1(journal, roster, 0, None, (count - 1) as nat);
        let slot = roster[count - 1].slot;
        assert forall|i: int| 0 <= i < count - 1 implies (#[trigger] roster[i]).slot != slot by {
            retirement_order_v1(journal, roster, i, count - 1);
        }
        retirement_remove_partition_v1(retirement_prefix_v1(journal.allocations@, roster, (count - 1) as nat),
            journal.allocation_free@ + retirement_slots_v1(roster, (count - 1) as nat), slot);
        assert((journal.allocation_free@ + retirement_slots_v1(roster, (count - 1) as nat)).push(slot)
            =~= journal.allocation_free@ + retirement_slots_v1(roster, count));
    }
}

pub proof fn retirement_pending_preserves_v1(before: JournalContentsV1, after: JournalContentsV1,
    roster: Seq<AllocationReferenceV1>, capacity: usize)
    requires pending_custody_v1(before), retirement_relation_v1(before, after, roster, capacity, Ok(())),
    ensures pending_custody_v1(after),
{
    retirement_ready_v1(before, roster, capacity);
    retirement_prefix_shape_v1(before.allocations@, roster, roster.len());
    retirement_prefix_partition_v1(before, roster, capacity, roster.len());
    assert forall|a: int| 0 <= a < after.allocations@.len() implies #[trigger] allocation_custody_v1(after, a) by {
        assert(allocation_custody_v1(before, a));
        reveal(allocation_custody_v1);
    }
    assert forall|m: int| 0 <= m < after.members@.len() implies #[trigger] member_custody_v1(after, m) by {
        assert(member_custody_v1(before, m));
        reveal(member_custody_v1);
        if before.members@[m].is_some() {
            let a = before.members@[m].unwrap().allocation.slot;
            assert forall|i: int| 0 <= i < roster.len() implies (#[trigger] roster[i]).slot != a by {
                retirement_scan_at_v1(before, roster, 0, None, i as nat);
            }
            assert(after.allocations@[a as int] == before.allocations@[a as int]);
        }
    }
    assert forall|w: int| 0 <= w < after.writers@.len() implies #[trigger] writer_custody_v1(after, w) by {
        assert(writer_custody_v1(before, w));
        reveal(writer_custody_v1);
        if before.writers@[w].is_some() {
            let entry = before.writers@[w].unwrap();
            let writer = WriterReferenceV1 { slot: w as usize, key: writer_key_v1(entry) };
            if !matches!(entry, WriterEntryV1::Reserved(_)) {
                let chain = choose|chain: Seq<usize>| retained_chain_v1(before, writer, chain);
                retained_chain_enrollment_frame_v1(before, after, writer, chain);
            }
        }
    }
    assert forall|a: int, b: int| 0 <= a < b < after.allocations@.len()
        && (#[trigger] after.allocations@[a]).is_some() && (#[trigger] after.allocations@[b]).is_some()
        implies after.allocations@[a].unwrap().key != after.allocations@[b].unwrap().key by {
        assert(before.allocations@[a].is_some() && before.allocations@[b].is_some());
    }
}

pub proof fn retirement_issuance_frame_v1(before: JournalContentsV1, after: JournalContentsV1,
    storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires issuance_invariant_v1(issuance_contents_projection_v1(before, storage, history)),
        retirement_journal_frame_v1(before, after), after.allocations@.len() == before.allocations@.len(),
        after.allocation_free@.len() <= before.allocation_capacity,
    ensures issuance_invariant_v1(issuance_contents_projection_v1(after, storage, history)),
{
}

pub proof fn retirement_producer_safe_from_invariant_v1(owner: ProducerReadContentsV1,
    roster: Seq<AllocationReferenceV1>, index: nat)
    requires producer_invariant_v1(owner),
    ensures retirement_producer_safe_v1(owner, roster, index),
    decreases roster.len() - index,
{
    if index < roster.len() {
        if allocation_decision_v1(owner.stable.journal, roster[index as int]).is_ok() {
            reader_count_addition_fits_usize_v1(owner, roster[index as int].slot);
        }
        retirement_producer_safe_from_invariant_v1(owner, roster, index + 1);
    }
}

pub proof fn retirement_stable_zero_at_v1(owner: ReadContentsV1, roster: Seq<AllocationReferenceV1>, from: nat, at: nat)
    requires from <= at < roster.len(), retirement_stable_safe_v1(owner, roster, from),
        retirement_stable_unread_v1(owner, roster, from).is_ok(),
    ensures roster[at as int].slot < owner.readers@.len(), owner.readers@[roster[at as int].slot as int] == 0,
    decreases at - from,
{
    if from < at { retirement_stable_zero_at_v1(owner, roster, from + 1, at); }
}

pub proof fn retirement_producer_zero_at_v1(owner: ProducerReadContentsV1, roster: Seq<AllocationReferenceV1>, from: nat, at: nat)
    requires from <= at < roster.len(), retirement_producer_safe_v1(owner, roster, from),
        retirement_producer_unread_v1(owner, roster, from).is_ok(),
    ensures roster[at as int].slot < owner.stable.readers@.len(), roster[at as int].slot < owner.counts@.len(),
        owner.stable.readers@[roster[at as int].slot as int] == 0, owner.counts@[roster[at as int].slot as int] == 0,
    decreases at - from,
{
    if from < at { retirement_producer_zero_at_v1(owner, roster, from + 1, at); }
}

pub proof fn retirement_stable_preserves_v1(before: ReadContentsV1, after: ReadContentsV1,
    roster: Seq<AllocationReferenceV1>, capacity: usize, result: Result<(), ReadErrorV1>)
    requires retirement_stable_relation_v1(before, after, roster, capacity, result),
        retirement_stable_safe_v1(before, roster, 0),
    ensures reader_invariant_v1(before) ==> reader_invariant_v1(after),
        pending_custody_v1(before.journal) ==> pending_custody_v1(after.journal),
{
    if result.is_ok() {
        retirement_ready_v1(before.journal, roster, capacity);
        retirement_prefix_shape_v1(before.journal.allocations@, roster, roster.len());
        if pending_custody_v1(before.journal) { retirement_pending_preserves_v1(before.journal, after.journal, roster, capacity); }
        if reader_invariant_v1(before) {
            assert forall|a: int| 0 <= a < before.readers@.len() && before.readers@[a] > 0 implies
                after.journal.allocations@[a] == before.journal.allocations@[a] by {
                assert forall|i: int| 0 <= i < roster.len() implies (#[trigger] roster[i]).slot != a by {
                    retirement_stable_zero_at_v1(before, roster, 0, i as nat);
                }
            }
            reader_allocation_frame_preserves_v1(before, after);
        }
    }
}

pub proof fn retirement_observation_frame_v1(before: JournalContentsV1, after: JournalContentsV1, request: ProducerReadV1)
    requires retirement_journal_frame_v1(before, after), after.allocations@.len() == before.allocations@.len(),
        request.read.allocation.slot < before.allocations@.len()
            ==> after.allocations@[request.read.allocation.slot as int] == before.allocations@[request.read.allocation.slot as int],
    ensures producer_status_v1(after, request) == producer_status_v1(before, request),
        producer_status_decision_v1(after, request) == producer_status_decision_v1(before, request),
        read_decision_v1(after, request.read) == read_decision_v1(before, request.read),
        allocation_decision_v1(after, request.read.allocation) == allocation_decision_v1(before, request.read.allocation),
{
}

pub proof fn retirement_retained_producer_frame_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    roster: Seq<AllocationReferenceV1>, capacity: usize)
    requires producer_invariant_v1(before), retirement_producer_relation_v1(before, after, roster, capacity, Ok(())),
    ensures forall|s: int| 0 <= s < before.reservations@.len() && (#[trigger] before.reservations@[s]).is_some() ==> {
        let request = before.reservations@[s].unwrap().request;
        &&& producer_status_v1(after.stable.journal, request) == producer_status_v1(before.stable.journal, request)
        &&& producer_status_decision_v1(after.stable.journal, request) == producer_status_decision_v1(before.stable.journal, request)
    },
{
    retirement_ready_v1(before.stable.journal, roster, capacity);
    retirement_prefix_shape_v1(before.stable.journal.allocations@, roster, roster.len());
    retirement_producer_safe_from_invariant_v1(before, roster, 0);
    assert forall|s: int| 0 <= s < before.reservations@.len() && (#[trigger] before.reservations@[s]).is_some() implies {
        let request = before.reservations@[s].unwrap().request;
        &&& producer_status_v1(after.stable.journal, request) == producer_status_v1(before.stable.journal, request)
        &&& producer_status_decision_v1(after.stable.journal, request) == producer_status_decision_v1(before.stable.journal, request)
    } by {
        let entry = before.reservations@[s].unwrap();
        assert(producer_entry_valid_v1(before.stable.journal, entry, s, before.next_incarnation));
        reveal(producer_entry_valid_v1);
        retained_producer_positive_count_v1(before, s as usize);
        let a = entry.request.read.allocation.slot;
        assert forall|i: int| 0 <= i < roster.len() implies (#[trigger] roster[i]).slot != a by {
            retirement_producer_zero_at_v1(before, roster, 0, i as nat);
        }
        retirement_observation_frame_v1(before.stable.journal, after.stable.journal, entry.request);
    }
}

pub proof fn retirement_producer_preserves_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    roster: Seq<AllocationReferenceV1>, capacity: usize, result: Result<(), ReadErrorV1>,
    storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires retirement_producer_relation_v1(before, after, roster, capacity, result),
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
        retirement_producer_safe_from_invariant_v1(before, roster, 0);
        retirement_producer_to_stable_v1(before, roster, 0);
        retirement_stable_preserves_v1(before.stable, after.stable, roster, capacity, result);
        retirement_retained_producer_frame_v1(before, after, roster, capacity);
        retirement_ready_v1(before.stable.journal, roster, capacity);
        retirement_prefix_shape_v1(before.stable.journal.allocations@, roster, roster.len());
        assert forall|s: int| 0 <= s < after.reservations@.len() && (#[trigger] after.reservations@[s]).is_some()
            implies producer_entry_valid_v1(after.stable.journal, after.reservations@[s].unwrap(), s, after.next_incarnation) by {
            assert(producer_entry_valid_v1(before.stable.journal, before.reservations@[s].unwrap(), s, before.next_incarnation));
            reveal(producer_entry_valid_v1);
        }
        assert(producer_invariant_v1(after));
        if issued_producer_v1(before, storage, history) {
            retirement_issuance_frame_v1(before.stable.journal, after.stable.journal, storage, history);
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
