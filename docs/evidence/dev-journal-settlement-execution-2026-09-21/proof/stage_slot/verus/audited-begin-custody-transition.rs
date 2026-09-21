#[verifier::spinoff_prover]
pub proof fn begin_preserves_allocation_custody_v1(before: JournalContentsV1, after: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>)
    requires pending_custody_v1(before), begin_shape_v1(before, writer, roster), begin_final_views_v1(before, after, writer, roster),
        begin_success_relation_v1(before, after, writer, roster),
    ensures forall|a: int| 0 <= a < after.allocations@.len() ==> #[trigger] allocation_custody_v1(after, a),
{
    hide(begin_members_prefix_v1);
    hide(begin_allocations_prefix_v1);
    begin_retained_storage_v1(before, after, writer, roster);
    begin_allocation_coordinates_v1(before, after, writer, roster);
    begin_new_chain_v1(before, after, writer, roster);
    assert forall|a: int| 0 <= a < after.allocations@.len() implies #[trigger] allocation_custody_v1(after, a) by {
        assert(allocation_custody_v1(before, a));
        reveal(allocation_custody_v1);
        if begin_selected_allocation_v1(roster, a) {
            let i = choose|i: int| 0 <= i < roster.len() && (#[trigger] roster[i]).allocation.slot == a;
            assert(begin_destination_decision_v1(before, roster[i]) == Ok(()));
        }
    }
}

#[verifier::spinoff_prover]
pub proof fn begin_preserves_member_custody_v1(before: JournalContentsV1, after: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>)
    requires pending_custody_v1(before), begin_shape_v1(before, writer, roster), begin_final_views_v1(before, after, writer, roster),
        begin_success_relation_v1(before, after, writer, roster),
    ensures forall|m: int| 0 <= m < after.members@.len() ==> #[trigger] member_custody_v1(after, m),
{
    hide(begin_members_prefix_v1);
    hide(begin_allocations_prefix_v1);
    begin_retained_storage_v1(before, after, writer, roster);
    begin_allocation_coordinates_v1(before, after, writer, roster);
    begin_new_chain_v1(before, after, writer, roster);
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
}

#[verifier::spinoff_prover]
pub proof fn begin_preserves_writer_custody_v1(before: JournalContentsV1, after: JournalContentsV1, writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>)
    requires pending_custody_v1(before), begin_shape_v1(before, writer, roster), begin_final_views_v1(before, after, writer, roster),
        begin_success_relation_v1(before, after, writer, roster),
    ensures forall|w: int| 0 <= w < after.writers@.len() ==> #[trigger] writer_custody_v1(after, w),
{
    hide(begin_members_prefix_v1);
    hide(begin_allocations_prefix_v1);
    begin_retained_storage_v1(before, after, writer, roster);
    begin_allocation_coordinates_v1(before, after, writer, roster);
    begin_new_chain_v1(before, after, writer, roster);
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
    assert forall|w: int| 0 <= w < after.writers@.len() implies {
        &&& (#[trigger] after.writers@[w]).is_some() == before.writers@[w].is_some()
        &&& after.writers@[w].is_some() ==>
            writer_key_v1(after.writers@[w].unwrap()) == writer_key_v1(before.writers@[w].unwrap())
    } by {
        if w == writer.slot {
            assert(before.writers@[w] == Some(WriterEntryV1::Reserved(writer.key)));
        }
    }
    assert(slot_partition_v1(after.allocations@, after.allocation_free@));
    assert(slot_partition_v1(after.writers@, after.free@));
    begin_preserves_allocation_custody_v1(before, after, writer, roster);
    begin_preserves_member_custody_v1(before, after, writer, roster);
    begin_preserves_writer_custody_v1(before, after, writer, roster);
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