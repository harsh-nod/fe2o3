// V4-J3 development: reader commit contents; no native authority claim.


verus! {

pub open spec fn read_slot_count_v1(requests: Seq<AllocationReadV1>, slot: usize) -> nat
    decreases requests.len(),
{
    if requests.len() == 0 { 0 }
    else { read_slot_count_v1(requests.drop_last(), slot)
        + if requests.last().allocation.slot == slot { 1nat } else { 0nat } }
}

pub proof fn read_slot_count_bound_v1(requests: Seq<AllocationReadV1>, slot: usize)
    ensures read_slot_count_v1(requests, slot) <= requests.len(),
    decreases requests.len(),
{
    if requests.len() > 0 { read_slot_count_bound_v1(requests.drop_last(), slot); }
}

pub proof fn read_slot_count_push_v1(requests: Seq<AllocationReadV1>, request: AllocationReadV1, slot: usize)
    ensures read_slot_count_v1(requests.push(request), slot)
        == read_slot_count_v1(requests, slot) + if request.allocation.slot == slot { 1nat } else { 0nat },
{
    assert(requests.push(request).drop_last() =~= requests);
}

pub open spec fn reader_journal_frame_v1(before: ReadContentsV1, after: ReadContentsV1) -> bool {
    &&& issuance_contents_frame_v1(before.journal, after.journal)
    &&& after.journal.registration_watermark == before.journal.registration_watermark
    &&& after.journal.reserved_count == before.journal.reserved_count
    &&& after.journal.writers@ == before.journal.writers@
    &&& after.journal.free@ == before.journal.free@
}

pub open spec fn acquired_reference_v1(before: ReadContentsV1, consumer: WriterKeyV1, index: int) -> ReadReferenceV1 {
    ReadReferenceV1 { slot: before.free_reads@[before.free_reads@.len() - 1 - index],
        incarnation: (before.next_incarnation + index) as u64, consumer }
}

pub open spec fn selected_free_unique_v1(before: ReadContentsV1, count: nat) -> bool {
    forall|i: int, j: int| 0 <= i < j < count ==>
        #[trigger] before.free_reads@[before.free_reads@.len() - 1 - i]
            != #[trigger] before.free_reads@[before.free_reads@.len() - 1 - j]
}

// These are explicit commit safety premises, not an assumed reachable-state invariant.
pub open spec fn acquire_commit_ready_v1(before: ReadContentsV1, requests: Seq<AllocationReadV1>) -> bool {
    &&& requests.len() <= before.free_reads@.len()
    &&& requests.len() <= u64::MAX
    &&& 0 < before.next_incarnation
    &&& before.next_incarnation + requests.len() <= u64::MAX
    &&& selected_free_unique_v1(before, requests.len())
    &&& acquire_prefix_ready_v1(before, requests, requests.len())
}

pub open spec fn acquire_prefix_ready_v1(before: ReadContentsV1, requests: Seq<AllocationReadV1>, end: nat) -> bool {
    forall|i: int| 0 <= i < end ==> {
        let slot = before.free_reads@[before.free_reads@.len() - 1 - i];
        let allocation = (#[trigger] requests[i]).allocation.slot;
        &&& slot < before.leases@.len()
        &&& before.leases@[slot as int].is_none()
        &&& allocation < before.readers@.len()
        &&& before.readers@[allocation as int]
            + read_slot_count_v1(requests.take(i + 1), allocation) <= usize::MAX
        &&& before.readers@[allocation as int]
            + read_slot_count_v1(requests.take(i + 1), allocation) <= before.leases@.len()
    }
}

pub open spec fn acquired_leases_v1(
    before: ReadContentsV1, consumer: WriterKeyV1, requests: Seq<AllocationReadV1>, count: nat,
) -> Seq<Option<ReadLeaseV1>>
    decreases count,
{
    if count == 0 { before.leases@ }
    else {
        let reference = acquired_reference_v1(before, consumer, count - 1);
        acquired_leases_v1(before, consumer, requests, (count - 1) as nat)
            .update(reference.slot as int, Some(ReadLeaseV1 { reference, request: requests[count - 1] }))
    }
}

pub open spec fn acquire_commit_prefix_v1(
    before: ReadContentsV1, after: ReadContentsV1, consumer: WriterKeyV1,
    requests: Seq<AllocationReadV1>, output_before: Seq<Option<ReadReferenceV1>>,
    output_after: Seq<Option<ReadReferenceV1>>, count: nat,
) -> bool {
    &&& reader_journal_frame_v1(before, after)
    &&& after.next_incarnation == before.next_incarnation
    &&& after.free_reads@ == before.free_reads@.take(before.free_reads@.len() - count)
    &&& after.leases@ == acquired_leases_v1(before, consumer, requests, count)
    &&& after.leases@.len() == before.leases@.len()
    &&& after.readers@.len() == before.readers@.len()
    &&& forall|a: int| 0 <= a < after.readers@.len() ==>
        after.readers@[a] == before.readers@[a] + read_slot_count_v1(requests.take(count as int), a as usize)
    &&& output_after.len() == output_before.len()
    &&& forall|i: int| 0 <= i < output_after.len() ==> output_after[i]
        == if i < count { Some(acquired_reference_v1(before, consumer, i)) } else { output_before[i] }
}

pub open spec fn acquire_commit_relation_v1(
    before: ReadContentsV1, after: ReadContentsV1, consumer: WriterKeyV1,
    requests: Seq<AllocationReadV1>, output: Seq<Option<ReadReferenceV1>>,
) -> bool {
    &&& reader_journal_frame_v1(before, after)
    &&& after.next_incarnation == before.next_incarnation + requests.len()
    &&& after.free_reads@ == before.free_reads@.take(before.free_reads@.len() - requests.len())
    &&& after.leases@ == acquired_leases_v1(before, consumer, requests, requests.len())
    &&& after.leases@.len() == before.leases@.len()
    &&& after.readers@.len() == before.readers@.len()
    &&& forall|a: int| 0 <= a < after.readers@.len() ==>
        after.readers@[a] == before.readers@[a] + read_slot_count_v1(requests, a as usize)
    &&& output.len() == requests.len()
    &&& forall|i: int| 0 <= i < output.len() ==> output[i] == Some(acquired_reference_v1(before, consumer, i))
}

pub proof fn acquired_lease_installed_v1(
    before: ReadContentsV1, consumer: WriterKeyV1, requests: Seq<AllocationReadV1>, count: nat, index: nat,
)
    requires acquire_commit_ready_v1(before, requests), index < count <= requests.len(),
    ensures acquired_leases_v1(before, consumer, requests, count)
        [acquired_reference_v1(before, consumer, index as int).slot as int]
        == Some(ReadLeaseV1 { reference: acquired_reference_v1(before, consumer, index as int), request: requests[index as int] }),
        acquired_leases_v1(before, consumer, requests, count).len() == before.leases@.len(),
    decreases count,
{
    if index + 1 < count {
        acquired_lease_installed_v1(before, consumer, requests, (count - 1) as nat, index);
    } else if count > 1 {
        acquired_lease_installed_v1(before, consumer, requests, (count - 1) as nat, 0);
    }
}

pub proof fn acquired_outputs_are_installed_v1(
    before: ReadContentsV1, after: ReadContentsV1, consumer: WriterKeyV1,
    requests: Seq<AllocationReadV1>, output: Seq<Option<ReadReferenceV1>>,
)
    requires acquire_commit_ready_v1(before, requests), acquire_commit_relation_v1(before, after, consumer, requests, output),
    ensures forall|i: int| 0 <= i < requests.len() ==> {
        let reference = (#[trigger] output[i]).unwrap();
        &&& output[i].is_some()
        &&& reference.slot < after.leases@.len()
        &&& after.leases@[reference.slot as int] == Some(ReadLeaseV1 { reference, request: requests[i] })
        &&& before.next_incarnation <= reference.incarnation < after.next_incarnation
        &&& 0 < reference.incarnation
    },
{
    assert forall|i: int| 0 <= i < requests.len() implies {
        let reference = (#[trigger] output[i]).unwrap();
        &&& output[i].is_some()
        &&& reference.slot < after.leases@.len()
        &&& after.leases@[reference.slot as int] == Some(ReadLeaseV1 { reference, request: requests[i] })
        &&& before.next_incarnation <= reference.incarnation < after.next_incarnation
        &&& 0 < reference.incarnation
    } by {
        acquired_lease_installed_v1(before, consumer, requests, requests.len(), i as nat);
    }
}

pub fn acquire_commit_exec_v1(
    contents: &mut ReadContentsV1, consumer: WriterKeyV1, requests: &[AllocationReadV1],
    output: &mut Vec<Option<ReadReferenceV1>>,
)
    requires acquire_commit_ready_v1(*old(contents), requests@), old(output)@.len() == requests@.len(),
    ensures acquire_commit_relation_v1(*old(contents), *final(contents), consumer, requests@, final(output)@),
{
    let ghost before = *contents;
    let ghost output_before = output@;
    let reader_capacity = contents.readers.len();
    let mut index = 0;
    proof {
        assert(before.free_reads@.take(before.free_reads@.len() as int) =~= before.free_reads@);
        assert(requests@.take(0) =~= Seq::<AllocationReadV1>::empty());
    }
    while index < requests.len()
        invariant index <= requests.len(), output_before.len() == requests@.len(),
            before.readers@.len() == reader_capacity,
            acquire_commit_ready_v1(before, requests@),
            acquire_commit_prefix_v1(before, *contents, consumer, requests@, output_before, output@, index as nat),
        decreases requests.len() - index,
    {
        let ghost previous_readers = contents.readers@;
        let ghost previous_output = output@;
        let ghost previous_leases = contents.leases@;
        let slot = contents.free_reads.pop().unwrap();
        let reference = ReadReferenceV1 { slot, incarnation: contents.next_incarnation + index as u64, consumer };
        contents.leases.set(slot, Some(ReadLeaseV1 { reference, request: requests[index] }));
        let allocation = requests[index].allocation.slot;
        proof {
            assert(requests@.take(index + 1) =~= requests@.take(index as int).push(requests@[index as int]));
            read_slot_count_push_v1(requests@.take(index as int), requests@[index as int], allocation);
        }
        let count = contents.readers[allocation] + 1;
        contents.readers.set(allocation, count);
        output.set(index, Some(reference));
        proof {
            assert forall|a: int| 0 <= a < contents.readers@.len() implies
                contents.readers@[a] == before.readers@[a] + read_slot_count_v1(requests@.take(index + 1), a as usize) by {
                read_slot_count_push_v1(requests@.take(index as int), requests@[index as int], a as usize);
            }
            assert(contents.free_reads@ =~= before.free_reads@.take(before.free_reads@.len() - index - 1));
        }
        index += 1;
    }
    proof { assert(requests@.take(requests@.len() as int) =~= requests@); }
    contents.next_incarnation += requests.len() as u64;
}

pub open spec fn released_requests_v1(before: ReadContentsV1, references: Seq<ReadReferenceV1>) -> Seq<AllocationReadV1> {
    Seq::new(references.len(), |i: int| before.leases@[references[i].slot as int].unwrap().request)
}

pub open spec fn release_commit_ready_v1(before: ReadContentsV1, references: Seq<ReadReferenceV1>) -> bool {
    &&& before.free_reads@.len() + references.len() <= usize::MAX
    &&& before.free_reads@.len() + references.len() <= before.leases@.len()
    &&& forall|i: int, j: int| 0 <= i < j < references.len() ==> references[i].slot != references[j].slot
    &&& release_prefix_ready_v1(before, references, references.len())
}

pub open spec fn release_prefix_ready_v1(before: ReadContentsV1, references: Seq<ReadReferenceV1>, end: nat) -> bool {
    forall|i: int| 0 <= i < end ==> {
        let slot = (#[trigger] references[i]).slot;
        let entry = before.leases@[slot as int].unwrap();
        &&& slot < before.leases@.len()
        &&& before.leases@[slot as int].is_some()
        &&& same_read_reference_v1(entry.reference, references[i])
        &&& entry.request.allocation.slot < before.readers@.len()
        &&& read_slot_count_v1(released_requests_v1(before, references).take(i + 1), entry.request.allocation.slot)
            <= before.readers@[entry.request.allocation.slot as int]
    }
}

pub open spec fn released_leases_v1(before: ReadContentsV1, references: Seq<ReadReferenceV1>, count: nat)
    -> Seq<Option<ReadLeaseV1>>
    decreases count,
{
    if count == 0 { before.leases@ }
    else { released_leases_v1(before, references, (count - 1) as nat).update(references[count - 1].slot as int, None) }
}

pub proof fn released_leases_unselected_v1(before: ReadContentsV1, references: Seq<ReadReferenceV1>, count: nat, slot: int)
    requires count <= references.len(), 0 <= slot < before.leases@.len(),
        forall|i: int| 0 <= i < count ==> references[i].slot != slot,
        forall|i: int| 0 <= i < count ==> references[i].slot < before.leases@.len(),
    ensures released_leases_v1(before, references, count)[slot] == before.leases@[slot],
        released_leases_v1(before, references, count).len() == before.leases@.len(),
    decreases count,
{
    if count > 0 { released_leases_unselected_v1(before, references, (count - 1) as nat, slot); }
}

pub open spec fn release_commit_prefix_v1(
    before: ReadContentsV1, after: ReadContentsV1, references: Seq<ReadReferenceV1>, count: nat,
) -> bool {
    &&& reader_journal_frame_v1(before, after)
    &&& after.next_incarnation == before.next_incarnation
    &&& after.free_reads@ == before.free_reads@ + Seq::new(count, |i: int| references[i].slot)
    &&& after.leases@ == released_leases_v1(before, references, count)
    &&& after.leases@.len() == before.leases@.len()
    &&& after.readers@.len() == before.readers@.len()
    &&& forall|a: int| 0 <= a < after.readers@.len() ==>
        after.readers@[a] == before.readers@[a]
            - read_slot_count_v1(released_requests_v1(before, references).take(count as int), a as usize)
}

pub fn release_commit_exec_v1(contents: &mut ReadContentsV1, references: &[ReadReferenceV1])
    requires release_commit_ready_v1(*old(contents), references@),
    ensures release_commit_prefix_v1(*old(contents), *final(contents), references@, references@.len()),
{
    let ghost before = *contents;
    let ghost requests = released_requests_v1(before, references@);
    let reader_capacity = contents.readers.len();
    let mut index = 0;
    proof {
        assert(Seq::new(0, |i: int| references@[i].slot) =~= Seq::<usize>::empty());
        assert(before.free_reads@ + Seq::<usize>::empty() =~= before.free_reads@);
        assert(requests.take(0) =~= Seq::<AllocationReadV1>::empty());
    }
    while index < references.len()
        invariant index <= references.len(), release_commit_ready_v1(before, references@),
            before.readers@.len() == reader_capacity,
            requests == released_requests_v1(before, references@),
            release_commit_prefix_v1(before, *contents, references@, index as nat),
        decreases references.len() - index,
    {
        let reference = references[index];
        let ghost previous_readers = contents.readers@;
        proof { released_leases_unselected_v1(before, references@, index as nat, reference.slot as int); }
        let entry = contents.leases[reference.slot].unwrap();
        contents.leases.set(reference.slot, None);
        let allocation = entry.request.allocation.slot;
        proof {
            assert(requests.take(index + 1) =~= requests.take(index as int).push(requests[index as int]));
            read_slot_count_push_v1(requests.take(index as int), requests[index as int], allocation);
        }
        let count = contents.readers[allocation] - 1;
        contents.readers.set(allocation, count);
        contents.free_reads.push(reference.slot);
        proof {
            assert forall|a: int| 0 <= a < contents.readers@.len() implies
                contents.readers@[a] == before.readers@[a] - read_slot_count_v1(requests.take(index + 1), a as usize) by {
                assert(previous_readers[a] == before.readers@[a] - read_slot_count_v1(requests.take(index as int), a as usize));
                assert(requests[index as int].allocation.slot == allocation);
                assert(contents.readers@[a] == if a == allocation { previous_readers[a] - 1 } else { previous_readers[a] as int });
                read_slot_count_push_v1(requests.take(index as int), requests[index as int], a as usize);
            }
            assert(contents.free_reads@ =~= before.free_reads@ + Seq::new((index + 1) as nat, |i: int| references@[i].slot));
        }
        index += 1;
    }
}

pub open spec fn order_local_count_v1(keys: Seq<ReadOrderV1>, local: u64) -> nat
    decreases keys.len(),
{
    if keys.len() == 0 { 0 }
    else { order_local_count_v1(keys.drop_last(), local) + if keys.last().0 == local { 1nat } else { 0nat } }
}

pub proof fn order_local_count_zero_v1(keys: Seq<ReadOrderV1>, local: u64)
    requires forall|i: int| 0 <= i < keys.len() ==> (#[trigger] keys[i]).0 != local,
    ensures order_local_count_v1(keys, local) == 0,
    decreases keys.len(),
{
    if keys.len() > 0 { order_local_count_zero_v1(keys.drop_last(), local); }
}

pub proof fn order_transitive_v1(left: ReadOrderV1, middle: ReadOrderV1, right: ReadOrderV1)
    requires order_lt_v1(left, middle), order_lt_v1(middle, right),
    ensures order_lt_v1(left, right),
{}

pub open spec fn canonical_prefix_v1(keys: Seq<ReadOrderV1>, end: nat, state: ReadScanV1) -> bool {
    &&& end <= keys.len()
    &&& state.group <= end
    &&& forall|i: int, j: int| 0 <= i < j < end ==> order_lt_v1(#[trigger] keys[i], #[trigger] keys[j])
    &&& if end == 0 { state.previous.is_none() && state.group == 0 }
        else { state.previous == Some(keys[end - 1])
            && state.group == order_local_count_v1(keys.take(end as int), keys[end - 1].0) }
}

pub proof fn canonical_prefix_extend_v1(keys: Seq<ReadOrderV1>, end: nat, state: ReadScanV1)
    requires canonical_prefix_v1(keys, end, state), end < keys.len(), end < usize::MAX,
        state.previous.is_some() ==> order_lt_v1(state.previous.unwrap(), keys[end as int]),
    ensures canonical_prefix_v1(keys, end + 1, ReadScanV1 { previous: Some(keys[end as int]),
        group: next_group_v1(state.previous, state.group, keys[end as int]) }),
{
    let key = keys[end as int];
    assert forall|i: int, j: int| 0 <= i < j < end + 1 implies order_lt_v1(keys[i], keys[j]) by {
        if j == end && i < end - 1 { order_transitive_v1(keys[i], keys[end - 1], key); }
    }
    assert(keys.take((end + 1) as int).drop_last() =~= keys.take(end as int));
    assert(keys.take((end + 1) as int).last() == key);
    if end == 0 {
        assert(keys.take(0) =~= Seq::<ReadOrderV1>::empty());
    } else if keys[end - 1].0 != key.0 {
        assert forall|i: int| 0 <= i < end implies (#[trigger] keys.take(end as int)[i]).0 != key.0 by {
            if i < end - 1 { assert(order_lt_v1(keys[i], keys[end - 1])); }
        }
        order_local_count_zero_v1(keys.take(end as int), key.0);
    }
    assert(order_local_count_v1(keys.take((end + 1) as int), key.0)
        == order_local_count_v1(keys.take(end as int), key.0) + 1);
}

pub proof fn validated_same_slot_local_v1(journal: JournalContentsV1, left: AllocationReadV1, right: AllocationReadV1)
    requires read_decision_v1(journal, left) == Ok(()), read_decision_v1(journal, right) == Ok(()),
        left.allocation.slot == right.allocation.slot,
    ensures left.allocation.key.local == right.allocation.key.local,
{}

pub proof fn read_count_local_dominance_v1(
    journal: JournalContentsV1, requests: Seq<AllocationReadV1>, keys: Seq<ReadOrderV1>, target: AllocationReadV1,
)
    requires requests.len() == keys.len(), read_decision_v1(journal, target) == Ok(()),
        forall|i: int| 0 <= i < requests.len() ==> read_decision_v1(journal, #[trigger] requests[i]) == Ok(()),
        forall|i: int| 0 <= i < requests.len() ==> (#[trigger] keys[i]).0 == requests[i].allocation.key.local,
    ensures read_slot_count_v1(requests, target.allocation.slot)
        <= order_local_count_v1(keys, target.allocation.key.local),
    decreases requests.len(),
{
    if requests.len() > 0 {
        read_count_local_dominance_v1(journal, requests.drop_last(), keys.drop_last(), target);
        if requests.last().allocation.slot == target.allocation.slot {
            validated_same_slot_local_v1(journal, requests.last(), target);
        }
    }
}

pub proof fn acquire_scan_commit_ready_v1(
    before: ReadContentsV1, consumer: WriterKeyV1, requests: Seq<AllocationReadV1>, output: Seq<Option<ReadReferenceV1>>,
    index: nat, state: ReadScanV1,
)
    requires before.readers@.len() == before.journal.allocations@.len(), requests.len() <= usize::MAX,
        requests.len() <= u64::MAX,
        acquire_header_v1(before, consumer, requests.len() as usize, output) == Ok(()),
        selected_free_unique_v1(before, requests.len()),
        index <= requests.len(), acquire_scan_v1(before, requests, index, state) == Ok(()),
        canonical_prefix_v1(Seq::new(requests.len(), |i: int| read_order_v1(requests[i], 0)), index, state),
        forall|i: int| 0 <= i < index ==> read_decision_v1(before.journal, #[trigger] requests[i]) == Ok(()),
        acquire_prefix_ready_v1(before, requests, index),
    ensures acquire_commit_ready_v1(before, requests), output.len() == requests.len(),
        forall|i: int| 0 <= i < requests.len() ==> read_decision_v1(before.journal, #[trigger] requests[i]) == Ok(()),
    decreases requests.len() - index,
{
    let keys = Seq::new(requests.len(), |i: int| read_order_v1(requests[i], 0));
    if index < requests.len() {
        let request = requests[index as int];
        let next = match acquire_item_v1(before, request, index as usize, state) { Ok(next) => next, Err(_) => state };
        canonical_prefix_extend_v1(keys, index, state);
        read_count_local_dominance_v1(before.journal, requests.take((index + 1) as int), keys.take((index + 1) as int), request);
        assert(next == ReadScanV1 { previous: Some(keys[index as int]),
            group: next_group_v1(state.previous, state.group, keys[index as int]) });
        assert(canonical_prefix_v1(keys, index + 1, next));
        assert(read_slot_count_v1(requests.take((index + 1) as int), request.allocation.slot) <= next.group);
        assert(before.readers@[request.allocation.slot as int] + next.group <= before.leases@.len());
        assert(acquire_prefix_ready_v1(before, requests, index + 1));
        acquire_scan_commit_ready_v1(before, consumer, requests, output, index + 1, next);
    }
}

pub proof fn acquire_preflight_implies_commit_ready_v1(
    before: ReadContentsV1, consumer: WriterKeyV1, requests: Seq<AllocationReadV1>, output: Seq<Option<ReadReferenceV1>>,
)
    requires before.readers@.len() == before.journal.allocations@.len(), requests.len() <= usize::MAX,
        requests.len() <= u64::MAX, acquire_decision_v1(before, consumer, requests, output) == Ok(()),
        selected_free_unique_v1(before, requests.len()),
    ensures acquire_commit_ready_v1(before, requests), output.len() == requests.len(),
        forall|i: int| 0 <= i < requests.len() ==> read_decision_v1(before.journal, #[trigger] requests[i]) == Ok(()),
{
    acquire_scan_commit_ready_v1(before, consumer, requests, output, 0, ReadScanV1 { previous: None, group: 0 });
}

pub proof fn release_scan_commit_ready_v1(
    before: ReadContentsV1, consumer: WriterKeyV1, references: Seq<ReadReferenceV1>,
    evidence_consumer: WriterKeyV1, observed_free_capacity: usize,
    index: nat, state: ReadScanV1,
)
    requires before.readers@.len() == before.journal.allocations@.len(), references.len() <= usize::MAX,
        release_header_v1(before, consumer, evidence_consumer, references.len() as usize, observed_free_capacity) == Ok(()),
        index <= references.len(), release_scan_v1(before, consumer, references, index, state) == Ok(()),
        canonical_prefix_v1(Seq::new(references.len(), |i: int|
            read_order_v1(released_requests_v1(before, references)[i], references[i].incarnation)), index, state),
        forall|i: int| 0 <= i < index ==>
            read_decision_v1(before.journal, #[trigger] released_requests_v1(before, references)[i]) == Ok(()),
        release_prefix_ready_v1(before, references, index),
    ensures release_commit_ready_v1(before, references),
    decreases references.len() - index,
{
    let requests = released_requests_v1(before, references);
    let keys = Seq::new(references.len(), |i: int| read_order_v1(requests[i], references[i].incarnation));
    if index < references.len() {
        let reference = references[index as int];
        let next = match release_item_v1(before, consumer, reference, state) { Ok(next) => next, Err(_) => state };
        canonical_prefix_extend_v1(keys, index, state);
        read_count_local_dominance_v1(before.journal, requests.take((index + 1) as int), keys.take((index + 1) as int), requests[index as int]);
        assert(next == ReadScanV1 { previous: Some(keys[index as int]),
            group: next_group_v1(state.previous, state.group, keys[index as int]) });
        release_scan_commit_ready_v1(before, consumer, references, evidence_consumer, observed_free_capacity, index + 1, next);
    } else {
        assert forall|i: int, j: int| 0 <= i < j < references.len() implies references[i].slot != references[j].slot by {
            if references[i].slot == references[j].slot {
                assert(keys[i] == keys[j]);
                assert(order_lt_v1(keys[i], keys[j]));
            }
        }
    }
}

pub proof fn release_preflight_implies_commit_ready_v1(
    before: ReadContentsV1, consumer: WriterKeyV1, references: Seq<ReadReferenceV1>,
    evidence_consumer: WriterKeyV1, observed_free_capacity: usize,
)
    requires before.readers@.len() == before.journal.allocations@.len(), references.len() <= usize::MAX,
        release_decision_v1(before, consumer, references, evidence_consumer, observed_free_capacity) == Ok(()),
    ensures release_commit_ready_v1(before, references),
{
    release_scan_commit_ready_v1(before, consumer, references, evidence_consumer, observed_free_capacity,
        0, ReadScanV1 { previous: None, group: 0 });
}

pub open spec fn acquire_execution_relation_v1(
    before: ReadContentsV1, after: ReadContentsV1, consumer: WriterKeyV1, requests: Seq<AllocationReadV1>,
    output_before: Seq<Option<ReadReferenceV1>>, output_after: Seq<Option<ReadReferenceV1>>, result: Result<(), ReadErrorV1>,
) -> bool {
    &&& result == acquire_decision_v1(before, consumer, requests, output_before)
    &&& match result {
        Err(_) => reader_contents_frame_v1(before, after) && output_after == output_before,
        Ok(()) => acquire_commit_relation_v1(before, after, consumer, requests, output_after),
    }
}

pub fn acquire_contents_exec_v1(
    contents: &mut ReadContentsV1, consumer: WriterKeyV1, requests: &[AllocationReadV1], output: &mut Vec<Option<ReadReferenceV1>>,
) -> (result: Result<(), ReadErrorV1>)
    requires old(contents).readers@.len() == old(contents).journal.allocations@.len(), requests@.len() <= u64::MAX,
        acquire_decision_v1(*old(contents), consumer, requests@, old(output)@) == Ok(())
            ==> selected_free_unique_v1(*old(contents), requests@.len()),
    ensures acquire_execution_relation_v1(*old(contents), *final(contents), consumer, requests@, old(output)@, final(output)@, result),
{
    let ghost before = *contents;
    let ghost output_before = output@;
    let _count = requests.len();
    let decision = acquire_preflight_exec_v1(contents, consumer, requests, output.as_slice());
    proof {
        assert(decision == acquire_decision_v1(before, consumer, requests@, output_before));
    }
    match decision {
        Err(error) => return Err(error), Ok(value) => {
            // Materialize extensional equality of the unit payload for the scan bridge.
            proof { assert(value =~= ()); }
        },
    }
    proof { acquire_preflight_implies_commit_ready_v1(*contents, consumer, requests@, output@); }
    acquire_commit_exec_v1(contents, consumer, requests, output);
    contents.next_incarnation = 0;
    Ok(())
}

pub open spec fn release_execution_relation_v1(
    before: ReadContentsV1, after: ReadContentsV1, consumer: WriterKeyV1, references: Seq<ReadReferenceV1>,
    evidence_consumer: WriterKeyV1, observed_free_capacity: usize, result: Result<(), ReadErrorV1>,
) -> bool {
    &&& result == release_decision_v1(before, consumer, references, evidence_consumer, observed_free_capacity)
    &&& match result {
        Err(_) => reader_contents_frame_v1(before, after),
        Ok(()) => release_commit_prefix_v1(before, after, references, references.len()),
    }
}

pub fn release_contents_exec_v1(
    contents: &mut ReadContentsV1, consumer: WriterKeyV1, references: &[ReadReferenceV1],
    evidence_consumer: WriterKeyV1, observed_free_capacity: usize,
) -> (result: Result<(), ReadErrorV1>)
    requires old(contents).readers@.len() == old(contents).journal.allocations@.len(),
    ensures release_execution_relation_v1(*old(contents), *final(contents), consumer, references@,
        evidence_consumer, observed_free_capacity, result),
{
    let ghost before = *contents;
    let _count = references.len();
    let decision = release_preflight_exec_v1(contents, consumer, references, evidence_consumer, observed_free_capacity);
    proof {
        assert(decision == release_decision_v1(before, consumer, references@, evidence_consumer, observed_free_capacity));
    }
    match decision {
        Err(error) => return Err(error), Ok(value) => {
            proof { assert(value =~= ()); }
        },
    }
    proof { release_preflight_implies_commit_ready_v1(*contents, consumer, references@, evidence_consumer, observed_free_capacity); }
    release_commit_exec_v1(contents, references);
    Ok(())
}

}
