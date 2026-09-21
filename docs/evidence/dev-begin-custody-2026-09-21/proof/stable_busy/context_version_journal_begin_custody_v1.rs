// Begin custody composition candidate; production/native refinement remains separate.
include!("context_version_journal_begin_v1.rs");
#[path = "context_begin_reader_guards_v1.rs"]
mod begin_reader_guards;
use begin_reader_guards::*;

verus! {

pub proof fn begin_canonical_at_v1(roster: Seq<AllocationWriteV1>, index: nat, at: nat)
    requires index <= at < roster.len(), begin_canonical_scan_v1(roster, index) == Ok(()),
    ensures at > 0 ==> enrollment_key_less_v1(roster[at - 1].allocation.key, roster[at as int].allocation.key),
    decreases at - index,
{
    if index < at { begin_canonical_at_v1(roster, index + 1, at); }
}

pub proof fn begin_canonical_order_v1(roster: Seq<AllocationWriteV1>, i: int, j: int)
    requires 0 <= i < j < roster.len(), begin_canonical_scan_v1(roster, 0) == Ok(()),
    ensures enrollment_key_less_v1(roster[i].allocation.key, roster[j].allocation.key),
    decreases j - i,
{
    begin_canonical_at_v1(roster, 0, j as nat);
    if i + 1 < j { begin_canonical_order_v1(roster, i, j - 1); }
}

pub open spec fn begin_chain_v1(before: JournalContentsV1, roster: Seq<AllocationWriteV1>) -> Seq<usize> {
    Seq::new(roster.len(), |i: int| begin_plan_v1(before, roster, i).member_slot)
}

pub open spec fn begin_shape_v1(before: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>) -> bool {
    &&& begin_storage_ready_v1(before, roster)
    &&& roster.len() <= before.allocation_capacity
    &&& writer.slot < before.writers@.len()
    &&& before.writers@[writer.slot as int] == Some(WriterEntryV1::Reserved(writer.key))
    &&& forall|i: int, j: int| 0 <= i < j < roster.len() ==> {
        &&& (#[trigger] roster[i]).allocation.slot != (#[trigger] roster[j]).allocation.slot
        &&& roster[i].allocation.key.local < roster[j].allocation.key.local
        &&& begin_plan_v1(before, roster, i).member_slot != begin_plan_v1(before, roster, j).member_slot
    }
    &&& begin_chain_v1(before, roster).no_duplicates()
}

#[verifier::spinoff_prover]
pub proof fn begin_selected_shape_v1(before: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>)
    requires pending_custody_v1(before), begin_preflight_decision_v1(before, writer, roster).is_ok(),
    ensures begin_shape_v1(before, writer, roster),
{
    begin_preflight_ready_v1(before, writer, roster);
    let canonical = begin_canonical_scan_v1(roster, 0);
    assert(canonical == Ok(())) by { match canonical { Ok(value) => { assert(value == ()); }, Err(_) => {}, } }
    assert forall|i: int, j: int| 0 <= i < j < roster.len() implies {
        &&& (#[trigger] roster[i]).allocation.slot != (#[trigger] roster[j]).allocation.slot
        &&& roster[i].allocation.key.local < roster[j].allocation.key.local
        &&& begin_plan_v1(before, roster, i).member_slot != begin_plan_v1(before, roster, j).member_slot
    } by {
        begin_canonical_order_v1(roster, i, j);
        assert(begin_destination_decision_v1(before, roster[i]) == Ok(()));
        assert(begin_destination_decision_v1(before, roster[j]) == Ok(()));
        let p = before.member_free@.len() - 1 - i;
        let q = before.member_free@.len() - 1 - j;
        assert(0 <= q < p < before.member_free@.len());
        assert(before.member_free@[p] != before.member_free@[q]);
    }
    assert forall|i: int, j: int| 0 <= i < j < roster.len() implies
        begin_chain_v1(before, roster)[i] != begin_chain_v1(before, roster)[j] by {
        assert(roster[i].allocation.slot != roster[j].allocation.slot);
    }
}

pub open spec fn begin_selected_allocation_v1(roster: Seq<AllocationWriteV1>, a: int) -> bool {
    exists|i: int| 0 <= i < roster.len() && (#[trigger] roster[i]).allocation.slot == a
}

pub proof fn begin_member_prefix_frame_v1(before: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, count: nat, m: int)
    requires begin_storage_ready_v1(before, roster), count <= roster.len(), 0 <= m < before.members@.len(),
        forall|i: int| 0 <= i < count ==> (#[trigger] begin_plan_v1(before, roster, i)).member_slot != m,
    ensures begin_members_prefix_v1(before, writer, roster, count)[m] == before.members@[m],
    decreases count,
{
    if count > 0 { begin_member_prefix_frame_v1(before, writer, roster, (count - 1) as nat, m); }
}

pub proof fn begin_allocation_prefix_frame_v1(before: JournalContentsV1, roster: Seq<AllocationWriteV1>, count: nat, a: int)
    requires begin_storage_ready_v1(before, roster), count <= roster.len(), 0 <= a < before.allocations@.len(),
        forall|i: int| 0 <= i < count ==> (#[trigger] roster[i]).allocation.slot != a,
    ensures begin_allocations_prefix_v1(before, roster, count)[a] == before.allocations@[a],
    decreases count,
{
    if count > 0 { begin_allocation_prefix_frame_v1(before, roster, (count - 1) as nat, a); }
}

pub proof fn begin_member_prefix_selected_v1(before: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, count: nat, i: int)
    requires begin_shape_v1(before, writer, roster), 0 <= i < count <= roster.len(),
    ensures begin_members_prefix_v1(before, writer, roster, count)[begin_plan_v1(before, roster, i).member_slot as int]
        == Some(begin_member_v1(before, writer, roster, i)),
    decreases count,
{
    begin_prefix_shapes_v1(before, writer, roster, count);
    begin_prefix_shapes_v1(before, writer, roster, (count - 1) as nat);
    assert(before.scratch@[i].is_none());
    assert(before.scratch@[count - 1].is_none());
    assert(begin_plan_v1(before, roster, i).member_slot < before.members@.len());
    assert(begin_plan_v1(before, roster, count - 1).member_slot < before.members@.len());
    if i < count - 1 {
        assert(roster[i].allocation.slot != roster[count - 1].allocation.slot);
        begin_member_prefix_selected_v1(before, writer, roster, (count - 1) as nat, i);
    }
}

pub proof fn begin_allocation_prefix_selected_v1(before: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, count: nat, i: int)
    requires begin_shape_v1(before, writer, roster), 0 <= i < count <= roster.len(),
    ensures begin_allocations_prefix_v1(before, roster, count)[roster[i].allocation.slot as int]
        == Some(begin_updated_allocation_v1(before.allocations@[roster[i].allocation.slot as int].unwrap(), begin_plan_v1(before, roster, i))),
    decreases count,
{
    if i < count - 1 { begin_allocation_prefix_selected_v1(before, writer, roster, (count - 1) as nat, i); }
    else {
        assert(begin_destination_decision_v1(before, roster[i]) == Ok(()));
        begin_prefix_shapes_v1(before, writer, roster, (count - 1) as nat);
        assert forall|j: int| 0 <= j < count - 1 implies (#[trigger] roster[j]).allocation.slot != roster[i].allocation.slot by {}
        begin_allocation_prefix_frame_v1(before, roster, (count - 1) as nat, roster[i].allocation.slot as int);
    }
}

pub open spec fn begin_final_views_v1(before: JournalContentsV1, after: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>) -> bool {
    &&& after.allocations@.len() == before.allocations@.len()
    &&& after.members@.len() == before.members@.len()
    &&& forall|i: int| 0 <= i < roster.len() ==> {
        &&& after.members@[begin_plan_v1(before, roster, i).member_slot as int] == Some(begin_member_v1(before, writer, roster, i))
        &&& after.allocations@[(#[trigger] roster[i]).allocation.slot as int]
            == Some(begin_updated_allocation_v1(before.allocations@[roster[i].allocation.slot as int].unwrap(), begin_plan_v1(before, roster, i)))
    }
    &&& forall|m: int| 0 <= m < before.members@.len() && !begin_chain_v1(before, roster).contains(m as usize)
        ==> (#[trigger] after.members@[m]) == before.members@[m]
    &&& forall|a: int| 0 <= a < before.allocations@.len() && !begin_selected_allocation_v1(roster, a)
        ==> (#[trigger] after.allocations@[a]) == before.allocations@[a]
}

#[verifier::spinoff_prover]
pub proof fn begin_final_views_proved_v1(before: JournalContentsV1, after: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>)
    requires begin_shape_v1(before, writer, roster), begin_success_relation_v1(before, after, writer, roster),
    ensures begin_final_views_v1(before, after, writer, roster),
{
    begin_prefix_shapes_v1(before, writer, roster, roster.len());
    assert forall|i: int| 0 <= i < roster.len() implies {
        &&& after.members@[begin_plan_v1(before, roster, i).member_slot as int] == Some(begin_member_v1(before, writer, roster, i))
        &&& after.allocations@[(#[trigger] roster[i]).allocation.slot as int]
            == Some(begin_updated_allocation_v1(before.allocations@[roster[i].allocation.slot as int].unwrap(), begin_plan_v1(before, roster, i)))
    } by {
        begin_member_prefix_selected_v1(before, writer, roster, roster.len(), i);
        begin_allocation_prefix_selected_v1(before, writer, roster, roster.len(), i);
    }
    assert forall|m: int| 0 <= m < before.members@.len() && !begin_chain_v1(before, roster).contains(m as usize)
        implies (#[trigger] after.members@[m]) == before.members@[m] by {
        assert forall|i: int| 0 <= i < roster.len() implies (#[trigger] begin_plan_v1(before, roster, i)).member_slot != m by {
            assert(begin_chain_v1(before, roster)[i] == begin_plan_v1(before, roster, i).member_slot);
        }
        begin_member_prefix_frame_v1(before, writer, roster, roster.len(), m);
    }
    assert forall|a: int| 0 <= a < before.allocations@.len() && !begin_selected_allocation_v1(roster, a)
        implies (#[trigger] after.allocations@[a]) == before.allocations@[a] by {
        begin_allocation_prefix_frame_v1(before, roster, roster.len(), a);
    }
}

#[verifier::spinoff_prover]
pub proof fn begin_retained_storage_v1(before: JournalContentsV1, after: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>)
    requires pending_custody_v1(before), begin_shape_v1(before, writer, roster), begin_final_views_v1(before, after, writer, roster),
    ensures forall|m: int| 0 <= m < before.members@.len() && (#[trigger] before.members@[m]).is_some()
            ==> after.members@[m] == before.members@[m],
        forall|a: int| 0 <= a < before.allocations@.len() && (#[trigger] before.allocations@[a]).is_some()
            && before.allocations@[a].unwrap().pending_member.is_some() ==> after.allocations@[a] == before.allocations@[a],
{
    assert forall|m: int| 0 <= m < before.members@.len() && (#[trigger] before.members@[m]).is_some()
        implies after.members@[m] == before.members@[m] by {
        if begin_chain_v1(before, roster).contains(m as usize) {
            let i = choose|i: int| 0 <= i < roster.len() && begin_chain_v1(before, roster)[i] == m;
            assert(begin_plan_v1(before, roster, i).member_slot == m);
        }
    }
    assert forall|a: int| 0 <= a < before.allocations@.len() && (#[trigger] before.allocations@[a]).is_some()
        && before.allocations@[a].unwrap().pending_member.is_some() implies after.allocations@[a] == before.allocations@[a] by {
        if begin_selected_allocation_v1(roster, a) {
            let i = choose|i: int| 0 <= i < roster.len() && (#[trigger] roster[i]).allocation.slot == a;
            assert(begin_destination_decision_v1(before, roster[i]) == Ok(()));
        }
    }
}

#[verifier::spinoff_prover]
pub proof fn begin_new_chain_v1(before: JournalContentsV1, after: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>)
    requires pending_custody_v1(before), begin_shape_v1(before, writer, roster), begin_final_views_v1(before, after, writer, roster),
        begin_success_relation_v1(before, after, writer, roster),
    ensures retained_chain_v1(after, writer, begin_chain_v1(before, roster)),
{
    hide(begin_members_prefix_v1);
    hide(begin_allocations_prefix_v1);
    let chain = begin_chain_v1(before, roster);
    assert forall|i: int| 0 <= i < chain.len() implies #[trigger] chain_link_v1(after, writer, chain, i) by {
        reveal(chain_link_v1);
        assert(after.members@[chain[i] as int] == Some(begin_member_v1(before, writer, roster, i)));
        if i > 0 { assert(after.members@[chain[i - 1] as int] == Some(begin_member_v1(before, writer, roster, i - 1))); }
    }
    assert forall|m: int| 0 <= m < after.members@.len() && (#[trigger] after.members@[m]).is_some()
        && same_producer_v1(after.members@[m].unwrap().writer, writer) implies chain.contains(m as usize) by {
        if !chain.contains(m as usize) {
            assert(before.members@[m].is_some());
            assert(member_custody_v1(before, m));
            reveal(member_custody_v1);
        }
    }
}

#[verifier::spinoff_prover]
pub proof fn begin_surviving_chain_v1(before: JournalContentsV1, after: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
    other: WriterReferenceV1, chain: Seq<usize>)
    requires pending_custody_v1(before), begin_shape_v1(before, writer, roster), begin_final_views_v1(before, after, writer, roster),
        begin_success_relation_v1(before, after, writer, roster), retained_chain_v1(before, other, chain),
    ensures retained_chain_v1(after, other, chain),
{
    hide(begin_members_prefix_v1);
    hide(begin_allocations_prefix_v1);
    begin_retained_storage_v1(before, after, writer, roster);
    assert(other.slot != writer.slot);
    assert(after.writers@[other.slot as int] == before.writers@[other.slot as int]);
    assert forall|i: int| 0 <= i < chain.len() implies
        after.members@[chain[i] as int] == before.members@[chain[i] as int] by {
        assert(chain_link_v1(before, other, chain, i));
        reveal(chain_link_v1);
    }
    assert forall|i: int| 0 <= i < chain.len() implies #[trigger] chain_link_v1(after, other, chain, i) by {
        assert(chain_link_v1(before, other, chain, i));
        reveal(chain_link_v1);
        if i > 0 { assert(chain_link_v1(before, other, chain, i - 1)); }
    }
    assert forall|m: int| 0 <= m < after.members@.len() && (#[trigger] after.members@[m]).is_some()
        && same_producer_v1(after.members@[m].unwrap().writer, other) implies chain.contains(m as usize) by {
        if begin_chain_v1(before, roster).contains(m as usize) {
            let i = choose|i: int| 0 <= i < roster.len() && begin_chain_v1(before, roster)[i] == m;
            assert(after.members@[m] == Some(begin_member_v1(before, writer, roster, i)));
        } else {
            assert(before.members@[m] == after.members@[m]);
            assert(same_producer_v1(before.members@[m].unwrap().writer, other));
        }
    }
}

#[verifier::spinoff_prover]
pub proof fn begin_member_partition_v1(before: JournalContentsV1, after: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>)
    requires pending_custody_v1(before), begin_shape_v1(before, writer, roster), begin_final_views_v1(before, after, writer, roster),
        begin_success_relation_v1(before, after, writer, roster),
    ensures slot_partition_v1(after.members@, after.member_free@),
{
    let free = before.member_free@;
    let kept = after.member_free@;
    let remaining = free.len() - roster.len();
    assert(kept.no_duplicates());
    assert forall|m: int| 0 <= m < after.members@.len() implies
        (#[trigger] after.members@[m]).is_none() == kept.contains(m as usize) by {
        if kept.contains(m as usize) {
            let p = choose|p: int| 0 <= p < kept.len() && kept[p] == m;
            assert(free[p] == m);
            assert(free.contains(m as usize));
            assert(!begin_chain_v1(before, roster).contains(m as usize)) by {
                if begin_chain_v1(before, roster).contains(m as usize) {
                    let i = choose|i: int| 0 <= i < roster.len() && begin_chain_v1(before, roster)[i] == m;
                    let q = free.len() - 1 - i;
                    assert(p < remaining <= q < free.len());
                    assert(free[p] != free[q]);
                }
            }
        } else if after.members@[m].is_none() {
            if begin_chain_v1(before, roster).contains(m as usize) {
                let i = choose|i: int| 0 <= i < roster.len() && begin_chain_v1(before, roster)[i] == m;
                assert(after.members@[m] == Some(begin_member_v1(before, writer, roster, i)));
            } else {
                assert(free.contains(m as usize));
                let p = choose|p: int| 0 <= p < free.len() && free[p] == m;
                if p < remaining { assert(kept[p] == m); assert(kept.contains(m as usize)); }
                else {
                    let i = free.len() - 1 - p;
                    assert(0 <= i < roster.len());
                    assert(begin_chain_v1(before, roster)[i] == m);
                }
            }
        }
    }
}

#[verifier::spinoff_prover]
pub proof fn begin_allocation_coordinates_v1(before: JournalContentsV1, after: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>)
    requires begin_shape_v1(before, writer, roster), begin_final_views_v1(before, after, writer, roster),
    ensures forall|a: int| 0 <= a < before.allocations@.len() ==> {
        &&& (#[trigger] after.allocations@[a]).is_some() == before.allocations@[a].is_some()
        &&& after.allocations@[a].is_some() ==> {
            &&& after.allocations@[a].unwrap().key == before.allocations@[a].unwrap().key
            &&& after.allocations@[a].unwrap().device == before.allocations@[a].unwrap().device
            &&& after.allocations@[a].unwrap().byte_extent == before.allocations@[a].unwrap().byte_extent
            &&& after.allocations@[a].unwrap().content_lineage == before.allocations@[a].unwrap().content_lineage
        }
    },
{
    assert forall|a: int| 0 <= a < before.allocations@.len() implies {
        &&& (#[trigger] after.allocations@[a]).is_some() == before.allocations@[a].is_some()
        &&& after.allocations@[a].is_some() ==> {
            &&& after.allocations@[a].unwrap().key == before.allocations@[a].unwrap().key
            &&& after.allocations@[a].unwrap().device == before.allocations@[a].unwrap().device
            &&& after.allocations@[a].unwrap().byte_extent == before.allocations@[a].unwrap().byte_extent
            &&& after.allocations@[a].unwrap().content_lineage == before.allocations@[a].unwrap().content_lineage
        }
    } by {
        if begin_selected_allocation_v1(roster, a) {
            let i = choose|i: int| 0 <= i < roster.len() && (#[trigger] roster[i]).allocation.slot == a;
            assert(begin_destination_decision_v1(before, roster[i]) == Ok(()));
        }
    }
}

#[verifier::spinoff_prover]
pub proof fn begin_preserves_pending_custody_v1(before: JournalContentsV1, after: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>)
    requires pending_custody_v1(before), begin_preflight_decision_v1(before, writer, roster).is_ok(),
        begin_success_relation_v1(before, after, writer, roster),
    ensures pending_custody_v1(after),
{
    hide(begin_members_prefix_v1);
    hide(begin_allocations_prefix_v1);
    begin_selected_shape_v1(before, writer, roster);
    begin_final_views_proved_v1(before, after, writer, roster);
    begin_retained_storage_v1(before, after, writer, roster);
    begin_allocation_coordinates_v1(before, after, writer, roster);
    begin_new_chain_v1(before, after, writer, roster);
    begin_member_partition_v1(before, after, writer, roster);
    assert(slot_partition_v1(after.allocations@, after.allocation_free@));
    assert(slot_partition_v1(after.writers@, after.free@));
    assert forall|a: int| 0 <= a < after.allocations@.len() implies #[trigger] allocation_custody_v1(after, a) by {
        assert(allocation_custody_v1(before, a));
        reveal(allocation_custody_v1);
        if begin_selected_allocation_v1(roster, a) {
            let i = choose|i: int| 0 <= i < roster.len() && (#[trigger] roster[i]).allocation.slot == a;
            assert(begin_destination_decision_v1(before, roster[i]) == Ok(()));
        }
    }
    assert forall|m: int| 0 <= m < after.members@.len() implies #[trigger] member_custody_v1(after, m) by {
        reveal(member_custody_v1);
        if begin_chain_v1(before, roster).contains(m as usize) {
            let i = choose|i: int| 0 <= i < roster.len() && begin_chain_v1(before, roster)[i] == m;
            assert(begin_destination_decision_v1(before, roster[i]) == Ok(()));
            let a = roster[i].allocation.slot as int;
            assert(allocation_custody_v1(before, a));
            reveal(allocation_custody_v1);
            assert(after.members@[m] == Some(begin_member_v1(before, writer, roster, i)));
        } else if after.members@[m].is_some() {
            assert(member_custody_v1(before, m));
            let member = before.members@[m].unwrap();
            assert(member.writer.slot != writer.slot);
            assert(before.allocations@[member.allocation.slot as int].unwrap().pending_member.is_some());
        }
    }
    assert forall|w: int| 0 <= w < after.writers@.len() implies #[trigger] writer_custody_v1(after, w) by {
        assert(writer_custody_v1(before, w));
        reveal(writer_custody_v1);
        if w == writer.slot {
            assert(retained_chain_v1(after, writer, begin_chain_v1(before, roster)));
        } else if before.writers@[w].is_some() {
            let entry = before.writers@[w].unwrap();
            if !matches!(entry, WriterEntryV1::Reserved(_)) {
                let other = WriterReferenceV1 { slot: w as usize, key: writer_key_v1(entry) };
                let chain = choose|chain: Seq<usize>| #[trigger] retained_chain_v1(before, other, chain);
                begin_surviving_chain_v1(before, after, writer, roster, other, chain);
            }
        }
    }
    assert forall|a: int, b: int| 0 <= a < b < after.allocations@.len()
        && (#[trigger] after.allocations@[a]).is_some() && (#[trigger] after.allocations@[b]).is_some()
        implies after.allocations@[a].unwrap().key != after.allocations@[b].unwrap().key by {
        assert(before.allocations@[a].unwrap().key != before.allocations@[b].unwrap().key);
    }
    assert forall|w: int, v: int| 0 <= w < v < after.writers@.len()
        && (#[trigger] after.writers@[w]).is_some() && (#[trigger] after.writers@[v]).is_some()
        implies writer_key_v1(after.writers@[w].unwrap()).local != writer_key_v1(after.writers@[v].unwrap()).local by {
        assert(writer_key_v1(before.writers@[w].unwrap()).local != writer_key_v1(before.writers@[v].unwrap()).local);
    }
}

pub open spec fn begin_selected_unread_v1(contents: ProducerReadContentsV1, roster: Seq<AllocationWriteV1>) -> bool {
    forall|i: int| 0 <= i < roster.len() ==> {
        &&& contents.stable.readers@[(#[trigger] roster[i]).allocation.slot as int] == 0
        &&& contents.counts@[roster[i].allocation.slot as int] == 0
    }
}

pub proof fn begin_preserves_stable_readers_v1(before: ReadContentsV1, after: ReadContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>)
    requires reader_invariant_v1(before), reader_storage_frame_v1(before, after),
        begin_shape_v1(before.journal, writer, roster), begin_final_views_v1(before.journal, after.journal, writer, roster),
        begin_success_relation_v1(before.journal, after.journal, writer, roster),
        forall|i: int| 0 <= i < roster.len() ==> before.readers@[(#[trigger] roster[i]).allocation.slot as int] == 0,
    ensures reader_invariant_v1(after),
{
    hide(begin_members_prefix_v1);
    hide(begin_allocations_prefix_v1);
    assert forall|a: int| 0 <= a < before.readers@.len() && before.readers@[a] > 0 implies
        after.journal.allocations@[a] == before.journal.allocations@[a] by {
        if begin_selected_allocation_v1(roster, a) {
            let i = choose|i: int| 0 <= i < roster.len() && (#[trigger] roster[i]).allocation.slot == a;
            assert(before.readers@[roster[i].allocation.slot as int] == 0);
        }
    }
    reader_allocation_frame_preserves_v1(before, after);
}

#[verifier::spinoff_prover]
pub proof fn begin_producer_status_frame_v1(before: JournalContentsV1, after: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>,
    request: ProducerReadV1)
    requires pending_custody_v1(before), begin_shape_v1(before, writer, roster), begin_final_views_v1(before, after, writer, roster),
        begin_success_relation_v1(before, after, writer, roster), producer_status_v1(before, request).is_some(),
        !begin_selected_allocation_v1(roster, request.read.allocation.slot as int),
    ensures producer_status_v1(after, request) == producer_status_v1(before, request),
{
    hide(begin_members_prefix_v1);
    hide(begin_allocations_prefix_v1);
    begin_retained_storage_v1(before, after, writer, roster);
    let a = request.read.allocation.slot as int;
    assert(after.allocations@[a] == before.allocations@[a]);
    if let Some(m) = before.allocations@[a].unwrap().pending_member {
        assert(before.members@[m as int].is_some());
        assert(after.members@[m as int] == before.members@[m as int]);
        assert(request.producer.slot != writer.slot);
        assert(after.writers@[request.producer.slot as int] == before.writers@[request.producer.slot as int]);
    } else {
        resolved_producer_storage_frame_v1(before, after, request);
    }
}

#[verifier::spinoff_prover]
pub proof fn begin_preserves_producer_invariant_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>)
    requires producer_invariant_v1(before), producer_storage_frame_v1(before, after),
        begin_preflight_decision_v1(before.stable.journal, writer, roster).is_ok(),
        begin_success_relation_v1(before.stable.journal, after.stable.journal, writer, roster),
        begin_selected_unread_v1(before, roster),
    ensures producer_invariant_v1(after),
{
    hide(begin_members_prefix_v1);
    hide(begin_allocations_prefix_v1);
    begin_selected_shape_v1(before.stable.journal, writer, roster);
    begin_final_views_proved_v1(before.stable.journal, after.stable.journal, writer, roster);
    begin_preserves_pending_custody_v1(before.stable.journal, after.stable.journal, writer, roster);
    begin_preserves_stable_readers_v1(before.stable, after.stable, writer, roster);
    assert forall|s: int| 0 <= s < after.reservations@.len() && (#[trigger] after.reservations@[s]).is_some() implies
        producer_entry_valid_v1(after.stable.journal, after.reservations@[s].unwrap(), s, after.next_incarnation) by {
        let entry = before.reservations@[s].unwrap();
        assert(producer_entry_valid_v1(before.stable.journal, entry, s, before.next_incarnation));
        reveal(producer_entry_valid_v1);
        let a = entry.request.read.allocation.slot as int;
        assert(!begin_selected_allocation_v1(roster, a)) by {
            if begin_selected_allocation_v1(roster, a) {
                let i = choose|i: int| 0 <= i < roster.len() && (#[trigger] roster[i]).allocation.slot == a;
                assert(before.counts@[a] == 0);
                producer_count_zero_v1(before.reservations@, a as usize);
                assert(producer_weight_v1(before.reservations@[s], a as usize) == 1);
            }
        }
        begin_producer_status_frame_v1(before.stable.journal, after.stable.journal, writer, roster, entry.request);
    }
}

pub proof fn begin_preserves_issued_producer_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, result: Result<(), ReadErrorV1>,
    storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires issued_producer_v1(before, storage, history), producer_storage_frame_v1(before, after),
        begin_execution_relation_v1(before.stable.journal, after.stable.journal, writer, roster, result),
        begin_selected_unread_v1(before, roster),
    ensures issued_producer_v1(after, storage, history),
{
    if result.is_ok() {
        begin_preserves_producer_invariant_v1(before, after, writer, roster);
    }
    begin_preserves_issuance_v1(before.stable.journal, after.stable.journal, writer, roster, result, storage, history);
}

pub open spec fn begin_read_references_v1(roster: Seq<AllocationWriteV1>) -> Seq<AllocationReferenceV1> {
    Seq::new(roster.len(), |i: int| roster[i].allocation)
}

// Production projects references lazily from the write slice; no temporary Vec.
pub fn begin_combined_unread_exec_v1(contents: &ProducerReadContentsV1, roster: &[AllocationWriteV1])
    -> (result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(*contents),
    ensures result == producer_unread_scan_v1(*contents, begin_read_references_v1(roster@), 0),
{
    let mut index = 0usize;
    while index < roster.len()
        invariant index <= roster.len(), producer_invariant_v1(*contents),
            producer_unread_scan_v1(*contents, begin_read_references_v1(roster@), 0)
                == producer_unread_scan_v1(*contents, begin_read_references_v1(roster@), index as nat),
        decreases roster.len() - index,
    {
        let count = match producer_reader_count_exec_v1(contents, roster[index].allocation) {
            Ok(count) => count, Err(error) => return Err(error),
        };
        if count != 0 { return Err(ReadErrorV1::AllocationBusy); }
        index += 1;
    }
    Ok(())
}

pub fn begin_stable_unread_exec_v1(contents: &ReadContentsV1, roster: &[AllocationWriteV1])
    -> (result: Result<(), ReadErrorV1>)
    requires contents.readers@.len() == contents.journal.allocations@.len(),
    ensures result == unread_scan_v1(*contents, begin_read_references_v1(roster@), 0),
{
    let mut index = 0usize;
    while index < roster.len()
        invariant index <= roster.len(), contents.readers@.len() == contents.journal.allocations@.len(),
            unread_scan_v1(*contents, begin_read_references_v1(roster@), 0)
                == unread_scan_v1(*contents, begin_read_references_v1(roster@), index as nat),
        decreases roster.len() - index,
    {
        let reference = roster[index].allocation;
        match allocation_lookup_exec_v1(&contents.journal, reference) {
            Ok(_) => {}, Err(error) => return Err(error),
        }
        if contents.readers[reference.slot] != 0 { return Ok(()); }
        index += 1;
    }
    Ok(())
}

pub proof fn begin_unread_scan_at_v1(contents: ProducerReadContentsV1, references: Seq<AllocationReferenceV1>, index: nat, at: nat)
    requires index <= at < references.len(), producer_unread_scan_v1(contents, references, index) == Ok(()),
    ensures producer_reader_count_decision_v1(contents, references[at as int]) == Ok(0),
    decreases at - index,
{
    if index < at { begin_unread_scan_at_v1(contents, references, index + 1, at); }
}

pub proof fn begin_unread_scan_selected_v1(contents: ProducerReadContentsV1, roster: Seq<AllocationWriteV1>)
    requires producer_invariant_v1(contents), producer_unread_scan_v1(contents, begin_read_references_v1(roster), 0) == Ok(()),
    ensures begin_selected_unread_v1(contents, roster),
{
    assert forall|i: int| 0 <= i < roster.len() implies {
        &&& contents.stable.readers@[(#[trigger] roster[i]).allocation.slot as int] == 0
        &&& contents.counts@[roster[i].allocation.slot as int] == 0
    } by {
        begin_unread_scan_at_v1(contents, begin_read_references_v1(roster), 0, i as nat);
        let slot = roster[i].allocation.slot;
        assert(allocation_decision_v1(contents.stable.journal, roster[i].allocation).is_ok());
        reader_count_addition_fits_usize_v1(contents, slot);
    }
}

pub open spec fn begin_issued_decision_v1(before: ProducerReadContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>)
    -> Result<(), ReadErrorV1>
{
    match producer_unread_scan_v1(before, begin_read_references_v1(roster), 0) {
        Err(error) => Err(error),
        Ok(()) => match unread_scan_v1(before.stable, begin_read_references_v1(roster), 0) {
            Err(error) => Err(error),
            Ok(()) => match begin_preflight_decision_v1(before.stable.journal, writer, roster) {
                Err(error) => Err(error), Ok(_) => Ok(()),
            },
        },
    }
}

pub open spec fn begin_issued_relation_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, result: Result<(), ReadErrorV1>,
    storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>) -> bool
{
    &&& result == begin_issued_decision_v1(before, writer, roster)
    &&& issued_producer_v1(after, storage, history)
    &&& producer_storage_frame_v1(before, after)
    &&& result.is_err() ==> after == before
    &&& result.is_ok() ==> begin_success_relation_v1(before.stable.journal, after.stable.journal, writer, roster)
}

#[verifier::spinoff_prover]
pub fn begin_issued_exec_v1(contents: &mut ProducerReadContentsV1, writer: WriterReferenceV1, roster: &[AllocationWriteV1],
    Ghost(storage): Ghost<StorageCapacitiesV1>, Ghost(history): Ghost<Seq<WriterReferenceV1>>)
    -> (result: Result<(), ReadErrorV1>)
    requires issued_producer_v1(*old(contents), storage, history),
    ensures begin_issued_relation_v1(*old(contents), *final(contents), writer, roster@, result, storage, history),
{
    let ghost before = *contents;
    match begin_combined_unread_exec_v1(contents, roster) {
        Err(error) => return Err(error), Ok(value) => { assert(value == ()); },
    }
    assert(producer_unread_scan_v1(before, begin_read_references_v1(roster@), 0) == Ok(()));
    match begin_stable_unread_exec_v1(&contents.stable, roster) {
        Err(error) => return Err(error), Ok(value) => { assert(value == ()); },
    }
    let result = begin_exec_v1(&mut contents.stable.journal, writer, roster);
    proof {
        begin_unread_scan_selected_v1(before, roster@);
        begin_preserves_issued_producer_v1(before, *contents, writer, roster@, result, storage, history);
    }
    result
}

// Keep reachability setup separate from exact commit inspection under the default solver budget.
#[verifier::spinoff_prover]
pub fn begin_issued_witness_commit_v1(contents: &mut ProducerReadContentsV1, writer: WriterReferenceV1,
    roster: &[AllocationWriteV1], empty: bool)
    -> (result: bool)
    requires issued_producer_v1(*old(contents), witness_storage_v1(3, 2), seq![writer]),
        begin_issued_decision_v1(*old(contents), writer, roster@) == Ok(()),
        old(contents).stable.journal.reserved_count == 1,
        old(contents).stable.journal.member_free@ == seq![2usize, 1usize, 0usize],
        roster.len() == if empty { 0usize } else { 2usize },
        !empty ==> roster@[0].allocation.slot == 0 && roster@[1].allocation.slot == 1,
        !empty ==> old(contents).stable.journal.allocations@[0].unwrap().attempt_epoch == 0
            && old(contents).stable.journal.allocations@[1].unwrap().attempt_epoch == 0
            && old(contents).stable.journal.allocations@[0].unwrap().content_lineage == 0
            && old(contents).stable.journal.allocations@[1].unwrap().content_lineage == 0,
    ensures result,
{
    proof {
        reveal_with_fuel(producer_unread_scan_v1, 4);
        reveal_with_fuel(unread_scan_v1, 4);
        reveal_with_fuel(begin_allocations_prefix_v1, 4);
        reveal_with_fuel(begin_members_prefix_v1, 4);
    }
    let ghost storage = witness_storage_v1(3, 2);
    let ghost history = seq![writer];
    let ghost before = contents.stable.journal;
    let result = begin_issued_exec_v1(contents, writer, roster, Ghost(storage), Ghost(history));
    assert(result == Ok(()));
    assert(issued_producer_v1(*contents, storage, history));
    assert(contents.stable.journal.reserved_count == 0);
    if empty {
        assert(contents.stable.journal.writers@[writer.slot as int]
            == Some(WriterEntryV1::Pending { key: writer.key, head: None, count: 0 }));
        assert(contents.stable.journal.allocations@ == before.allocations@);
        assert(contents.stable.journal.members@ == before.members@);
    } else {
        assert(contents.stable.journal.writers@[writer.slot as int]
            == Some(WriterEntryV1::Pending { key: writer.key, head: Some(0), count: 2 }));
        assert(contents.stable.journal.members@[0].unwrap().next == Some(1));
        assert(contents.stable.journal.members@[1].unwrap().next == None);
        assert(contents.stable.journal.member_free@ == seq![2usize]);
        assert(contents.stable.journal.allocations@[0].unwrap().attempt_epoch == 1);
        assert(contents.stable.journal.allocations@[1].unwrap().attempt_epoch == 1);
        assert(contents.stable.journal.allocations@[0].unwrap().content_lineage == 0);
        assert(contents.stable.journal.allocations@[1].unwrap().content_lineage == 0);
    }
    let ghost pending = *contents;
    let again = begin_issued_exec_v1(contents, writer, roster, Ghost(storage), Ghost(history));
    assert(again == Err(ReadErrorV1::InvalidReference) && *contents == pending);
    true
}

// Constructor reachability through both ordered guards and the issued wrapper.
#[verifier::spinoff_prover]
pub fn begin_issued_constructor_witness_v1(empty: bool) -> (result: bool)
    ensures result,
{
    proof {
        reveal_with_fuel(producer_unread_scan_v1, 4);
        reveal_with_fuel(unread_scan_v1, 4);
        reveal_with_fuel(enrollment_roster_scan_v1, 4);
        reveal_with_fuel(enrollment_prefix_v1, 4);
        reveal_with_fuel(begin_canonical_scan_v1, 4);
        reveal_with_fuel(begin_destination_scan_v1, 4);
        reveal_with_fuel(begin_slot_scan_v1, 4);
    }
    let ghost storage = witness_storage_v1(3, 2);
    let ghost history = Seq::<WriterReferenceV1>::empty();
    let mut contents = match issued_producer_constructor_exec_v1(7, 3, 2, 2, Ghost(storage)) {
        Ok(contents) => contents, Err(_) => return false,
    };
    let entries = vec![EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 20 },
        device: DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 },
        EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 30 },
        device: DeviceKeyV1 { context_generation: 7, local: 3 }, byte_extent: 32 }];
    let mut output = vec![None, None];
    let enrolled = enrollment_issued_exec_v1(&mut contents, &entries, &mut output, Ghost(storage), Ghost(history));
    assert(enrolled == Ok(()));
    assert(output@[0] == Some(AllocationReferenceV1 { slot: 0, key: entries@[0].key }));
    assert(output@[1] == Some(AllocationReferenceV1 { slot: 1, key: entries@[1].key }));
    let key = WriterKeyV1 { context_generation: 7, local: 40, kind: WriterKindV1::Submission };
    let writer = match register_issued_producer_exec_v1(&mut contents, key, Ghost(storage), Ghost(history)) {
        Ok(reference) => reference, Err(_) => return false,
    };
    let mut roster = vec![];
    if !empty {
        roster.push(AllocationWriteV1 { allocation: output[0].unwrap(), device: entries[0].device, byte_extent: 16 });
        roster.push(AllocationWriteV1 { allocation: output[1].unwrap(), device: entries[1].device, byte_extent: 32 });
    }
    assert(contents.stable.journal.member_free@ =~= seq![2usize, 1usize, 0usize]);
    begin_issued_witness_commit_v1(&mut contents, writer, &roster, empty)
}

}
