// Actual contents retain sequential alias writes; custody reachability is a separate relation.
verus! {

type StableReadOrderV1 = (u64, u64, u64);

spec fn stable_read_decision_v1(journal: JournalContentsV1, request: ContextAllocationReadV1)
    -> Result<(), ReadErrorV1>
{
    match allocation_lookup_decision_v1(journal, request.allocation) {
        Err(error) => Err(error),
        Ok(entry) => {
            if entry.device.context_generation != request.device.context_generation
                || entry.device.local != request.device.local { Err(ReadErrorV1::AllocationDeviceMismatch) }
            else if entry.byte_extent != request.byte_extent { Err(ReadErrorV1::AllocationExtentMismatch) }
            else if request.byte_len == 0 || request.byte_offset + request.byte_len > u64::MAX
                || request.byte_offset + request.byte_len > request.byte_extent { Err(ReadErrorV1::InvalidExtent) }
            else if entry.pending_writer.is_some() { Err(ReadErrorV1::AllocationBusy) }
            else if entry.attempt_epoch != request.attempt_epoch
                || entry.content_lineage != request.content_lineage { Err(ReadErrorV1::InvalidState) }
            else { Ok(()) }
        },
    }
}

spec fn stable_capacity_decision_v1(contents: ContextReadLeasedJournalV1, count: usize) -> Result<(), ReadErrorV1> {
    if count > contents.free_reads@.len() { Err(ReadErrorV1::MemberCapacity) }
    else if contents.next_incarnation == 0 || contents.next_incarnation + count > u64::MAX {
        Err(ReadErrorV1::EpochExhausted)
    } else { Ok(()) }
}

spec fn stable_read_order_v1(request: ContextAllocationReadV1) -> StableReadOrderV1 {
    (request.allocation.key.local, request.byte_offset, request.byte_len)
}

spec fn stable_order_lt_v1(left: StableReadOrderV1, right: StableReadOrderV1) -> bool {
    left.0 < right.0 || (left.0 == right.0 && (left.1 < right.1
        || (left.1 == right.1 && (left.2 < right.2))))
}

spec fn stable_next_group_v1(previous: Option<StableReadOrderV1>, group: usize, key: StableReadOrderV1) -> usize {
    if previous.is_some() && previous.unwrap().0 == key.0 { (group + 1) as usize } else { 1 }
}

spec fn stable_acquire_header_v1(
    contents: ContextReadLeasedJournalV1, consumer: WriterKeyV1, count: usize, output: Seq<Option<ContextReadLeaseReferenceV1>>,
) -> Result<(), ReadErrorV1> {
    if consumer.context_generation != contents.journal.context_generation { Err(ReadErrorV1::ForeignContext) }
    else if consumer.local == 0 || consumer.local == u64::MAX { Err(ReadErrorV1::InvalidWriterId) }
    else if count == 0 || count != output.len() { Err(ReadErrorV1::RosterCapacity) }
    else if exists|i: int| 0 <= i < output.len() && output[i].is_some() { Err(ReadErrorV1::InvalidState) }
    else { stable_capacity_decision_v1(contents, count) }
}

spec fn stable_acquire_item_v1(
    contents: ContextReadLeasedJournalV1, request: ContextAllocationReadV1, index: usize, state: StableReadAcquireScanV1,
) -> Result<StableReadAcquireScanV1, ReadErrorV1> {
    match stable_read_decision_v1(contents.journal, request) {
        Err(error) => Err(error),
        Ok(()) => {
            let key = stable_read_order_v1(request);
            if state.previous.is_some() && !stable_order_lt_v1(state.previous.unwrap(), key) {
                Err(ReadErrorV1::NonCanonicalRoster)
            } else {
                let group = stable_next_group_v1(state.previous, state.group, key);
                let count = contents.readers@[request.allocation.slot as int] + group;
                if count > usize::MAX || count > contents.leases@.len() { Err(ReadErrorV1::InvalidState) }
                else {
                    let slot = contents.free_reads@[contents.free_reads@.len() - index - 1];
                    if slot >= contents.leases@.len() || contents.leases@[slot as int].is_some() {
                        Err(ReadErrorV1::InvalidState)
                    } else { Ok(StableReadAcquireScanV1 { previous: Some(key), group }) }
                }
            }
        },
    }
}

spec fn stable_acquire_scan_v1(
    contents: ContextReadLeasedJournalV1, requests: Seq<ContextAllocationReadV1>, index: nat, state: StableReadAcquireScanV1,
) -> Result<(), ReadErrorV1>
    decreases requests.len() - index,
{
    if index >= requests.len() { Ok(()) }
    else { match stable_acquire_item_v1(contents, requests[index as int], index as usize, state) {
        Err(error) => Err(error),
        Ok(next) => stable_acquire_scan_v1(contents, requests, index + 1, next),
    } }
}

spec fn stable_acquire_decision_v1(
    contents: ContextReadLeasedJournalV1, consumer: WriterKeyV1, requests: Seq<ContextAllocationReadV1>,
    output: Seq<Option<ContextReadLeaseReferenceV1>>,
) -> Result<(), ReadErrorV1> {
    match stable_acquire_header_v1(contents, consumer, requests.len() as usize, output) {
        Err(error) => Err(error),
        Ok(()) => stable_acquire_scan_v1(contents, requests, 0, StableReadAcquireScanV1 { previous: None, group: 0 }),
    }
}

spec fn stable_read_slot_count_v1(requests: Seq<ContextAllocationReadV1>, slot: usize) -> nat
    decreases requests.len(),
{
    if requests.len() == 0 { 0 }
    else { stable_read_slot_count_v1(requests.drop_last(), slot)
        + if requests.last().allocation.slot == slot { 1nat } else { 0nat } }
}

proof fn stable_read_slot_count_bound_v1(requests: Seq<ContextAllocationReadV1>, slot: usize)
    ensures stable_read_slot_count_v1(requests, slot) <= requests.len(),
    decreases requests.len(),
{
    if requests.len() > 0 { stable_read_slot_count_bound_v1(requests.drop_last(), slot); }
}

proof fn stable_read_slot_count_push_v1(requests: Seq<ContextAllocationReadV1>, request: ContextAllocationReadV1, slot: usize)
    ensures stable_read_slot_count_v1(requests.push(request), slot)
        == stable_read_slot_count_v1(requests, slot) + if request.allocation.slot == slot { 1nat } else { 0nat },
{
    assert(requests.push(request).drop_last() =~= requests);
}

spec fn stable_reader_journal_frame_v1(before: ContextReadLeasedJournalV1, after: ContextReadLeasedJournalV1) -> bool {
    after.journal == before.journal
}

spec fn stable_acquired_reference_v1(before: ContextReadLeasedJournalV1, consumer: WriterKeyV1, index: int) -> ContextReadLeaseReferenceV1 {
    ContextReadLeaseReferenceV1 { slot: before.free_reads@[before.free_reads@.len() - 1 - index],
        incarnation: (before.next_incarnation + index) as u64, consumer }
}

spec fn stable_acquire_commit_ready_v1(before: ContextReadLeasedJournalV1, requests: Seq<ContextAllocationReadV1>) -> bool {
    &&& requests.len() <= before.free_reads@.len()
    &&& requests.len() <= u64::MAX
    &&& 0 < before.next_incarnation
    &&& before.next_incarnation + requests.len() <= u64::MAX
    &&& stable_acquire_prefix_ready_v1(before, requests, requests.len())
}

spec fn stable_acquire_prefix_entry_v1(before: ContextReadLeasedJournalV1, requests: Seq<ContextAllocationReadV1>, i: int) -> bool {
    let slot = before.free_reads@[before.free_reads@.len() - 1 - i];
    let allocation = requests[i].allocation.slot;
    &&& slot < before.leases@.len()
    &&& before.leases@[slot as int].is_none()
    &&& allocation < before.readers@.len()
    &&& before.readers@[allocation as int]
        + stable_read_slot_count_v1(requests.take(i + 1), allocation) <= usize::MAX
    &&& before.readers@[allocation as int]
        + stable_read_slot_count_v1(requests.take(i + 1), allocation) <= before.leases@.len()
}

spec fn stable_acquire_prefix_ready_v1(before: ContextReadLeasedJournalV1, requests: Seq<ContextAllocationReadV1>, end: nat) -> bool {
    forall|i: int| #![trigger requests[i]] #![trigger stable_acquire_prefix_entry_v1(before, requests, i)]
        0 <= i < end ==> stable_acquire_prefix_entry_v1(before, requests, i)
}

spec fn stable_acquired_leases_v1(
    before: ContextReadLeasedJournalV1, consumer: WriterKeyV1, requests: Seq<ContextAllocationReadV1>, count: nat,
) -> Seq<Option<ReadLeaseV1>>
    decreases count,
{
    if count == 0 { before.leases@ }
    else {
        let reference = stable_acquired_reference_v1(before, consumer, count - 1);
        stable_acquired_leases_v1(before, consumer, requests, (count - 1) as nat)
            .update(reference.slot as int, Some(ReadLeaseV1 { reference, request: requests[count - 1] }))
    }
}

spec fn stable_acquire_commit_prefix_v1(
    before: ContextReadLeasedJournalV1, after: ContextReadLeasedJournalV1, consumer: WriterKeyV1,
    requests: Seq<ContextAllocationReadV1>, output_before: Seq<Option<ContextReadLeaseReferenceV1>>,
    output_after: Seq<Option<ContextReadLeaseReferenceV1>>, count: nat,
) -> bool {
    &&& stable_reader_journal_frame_v1(before, after)
    &&& after.next_incarnation == before.next_incarnation
    &&& after.free_reads@ == before.free_reads@.take(before.free_reads@.len() - count)
    &&& after.leases@ == stable_acquired_leases_v1(before, consumer, requests, count)
    &&& after.leases@.len() == before.leases@.len()
    &&& after.readers@.len() == before.readers@.len()
    &&& forall|a: int| 0 <= a < after.readers@.len() ==>
        after.readers@[a] == before.readers@[a] + stable_read_slot_count_v1(requests.take(count as int), a as usize)
    &&& output_after.len() == output_before.len()
    &&& forall|i: int| 0 <= i < output_after.len() ==> output_after[i]
        == if i < count { Some(stable_acquired_reference_v1(before, consumer, i)) } else { output_before[i] }
}

spec fn stable_acquire_commit_relation_v1(
    before: ContextReadLeasedJournalV1, after: ContextReadLeasedJournalV1, consumer: WriterKeyV1,
    requests: Seq<ContextAllocationReadV1>, output: Seq<Option<ContextReadLeaseReferenceV1>>,
) -> bool {
    &&& stable_reader_journal_frame_v1(before, after)
    &&& after.next_incarnation == before.next_incarnation + requests.len()
    &&& after.free_reads@ == before.free_reads@.take(before.free_reads@.len() - requests.len())
    &&& after.leases@ == stable_acquired_leases_v1(before, consumer, requests, requests.len())
    &&& after.leases@.len() == before.leases@.len()
    &&& after.readers@.len() == before.readers@.len()
    &&& forall|a: int| 0 <= a < after.readers@.len() ==>
        after.readers@[a] == before.readers@[a] + stable_read_slot_count_v1(requests, a as usize)
    &&& output.len() == requests.len()
    &&& forall|i: int| 0 <= i < output.len() ==> output[i] == Some(stable_acquired_reference_v1(before, consumer, i))
}

spec fn stable_order_local_count_v1(keys: Seq<StableReadOrderV1>, local: u64) -> nat
    decreases keys.len(),
{
    if keys.len() == 0 { 0 }
    else { stable_order_local_count_v1(keys.drop_last(), local) + if keys.last().0 == local { 1nat } else { 0nat } }
}

proof fn stable_order_local_count_zero_v1(keys: Seq<StableReadOrderV1>, local: u64)
    requires forall|i: int| 0 <= i < keys.len() ==> (#[trigger] keys[i]).0 != local,
    ensures stable_order_local_count_v1(keys, local) == 0,
    decreases keys.len(),
{
    if keys.len() > 0 { stable_order_local_count_zero_v1(keys.drop_last(), local); }
}

proof fn stable_order_transitive_v1(left: StableReadOrderV1, middle: StableReadOrderV1, right: StableReadOrderV1)
    requires stable_order_lt_v1(left, middle), stable_order_lt_v1(middle, right),
    ensures stable_order_lt_v1(left, right),
{}

spec fn stable_canonical_prefix_v1(keys: Seq<StableReadOrderV1>, end: nat, state: StableReadAcquireScanV1) -> bool {
    &&& end <= keys.len()
    &&& state.group <= end
    &&& forall|i: int, j: int| 0 <= i < j < end ==> stable_order_lt_v1(#[trigger] keys[i], #[trigger] keys[j])
    &&& if end == 0 { state.previous.is_none() && state.group == 0 }
        else { state.previous == Some(keys[end - 1])
            && state.group == stable_order_local_count_v1(keys.take(end as int), keys[end - 1].0) }
}

proof fn stable_canonical_prefix_extend_v1(keys: Seq<StableReadOrderV1>, end: nat, state: StableReadAcquireScanV1)
    requires stable_canonical_prefix_v1(keys, end, state), end < keys.len(), end < usize::MAX,
        state.previous.is_some() ==> stable_order_lt_v1(state.previous.unwrap(), keys[end as int]),
    ensures stable_canonical_prefix_v1(keys, end + 1, StableReadAcquireScanV1 { previous: Some(keys[end as int]),
        group: stable_next_group_v1(state.previous, state.group, keys[end as int]) }),
{
    let key = keys[end as int];
    assert forall|i: int, j: int| 0 <= i < j < end + 1 implies stable_order_lt_v1(keys[i], keys[j]) by {
        if j == end && i < end - 1 { stable_order_transitive_v1(keys[i], keys[end - 1], key); }
    }
    assert(keys.take((end + 1) as int).drop_last() =~= keys.take(end as int));
    assert(keys.take((end + 1) as int).last() == key);
    if end == 0 {
        assert(keys.take(0) =~= Seq::<StableReadOrderV1>::empty());
    } else if keys[end - 1].0 != key.0 {
        assert forall|i: int| 0 <= i < end implies (#[trigger] keys.take(end as int)[i]).0 != key.0 by {
            if i < end - 1 { assert(stable_order_lt_v1(keys[i], keys[end - 1])); }
        }
        stable_order_local_count_zero_v1(keys.take(end as int), key.0);
    }
    assert(stable_order_local_count_v1(keys.take((end + 1) as int), key.0)
        == stable_order_local_count_v1(keys.take(end as int), key.0) + 1);
}

proof fn stable_validated_same_slot_local_v1(journal: JournalContentsV1, left: ContextAllocationReadV1, right: ContextAllocationReadV1)
    requires stable_read_decision_v1(journal, left) == Ok(()), stable_read_decision_v1(journal, right) == Ok(()),
        left.allocation.slot == right.allocation.slot,
    ensures left.allocation.key.local == right.allocation.key.local,
{}

proof fn stable_read_count_local_dominance_v1(
    journal: JournalContentsV1, requests: Seq<ContextAllocationReadV1>, keys: Seq<StableReadOrderV1>, target: ContextAllocationReadV1,
)
    requires requests.len() == keys.len(), stable_read_decision_v1(journal, target) == Ok(()),
        forall|i: int| 0 <= i < requests.len() ==> stable_read_decision_v1(journal, #[trigger] requests[i]) == Ok(()),
        forall|i: int| 0 <= i < requests.len() ==> (#[trigger] keys[i]).0 == requests[i].allocation.key.local,
    ensures stable_read_slot_count_v1(requests, target.allocation.slot)
        <= stable_order_local_count_v1(keys, target.allocation.key.local),
    decreases requests.len(),
{
    if requests.len() > 0 {
        stable_read_count_local_dominance_v1(journal, requests.drop_last(), keys.drop_last(), target);
        if requests.last().allocation.slot == target.allocation.slot {
            stable_validated_same_slot_local_v1(journal, requests.last(), target);
        }
    }
}

#[verifier::spinoff_prover]
proof fn stable_acquire_prefix_extend_v1(before: ContextReadLeasedJournalV1, requests: Seq<ContextAllocationReadV1>, index: nat)
    requires stable_acquire_prefix_ready_v1(before, requests, index), index < requests.len(),
        index < before.free_reads@.len(),
        before.free_reads@[before.free_reads@.len() - 1 - index] < before.leases@.len(),
        before.leases@[before.free_reads@[before.free_reads@.len() - 1 - index] as int].is_none(),
        requests[index as int].allocation.slot < before.readers@.len(),
        before.readers@[requests[index as int].allocation.slot as int]
            + stable_read_slot_count_v1(requests.take((index + 1) as int), requests[index as int].allocation.slot) <= usize::MAX,
        before.readers@[requests[index as int].allocation.slot as int]
            + stable_read_slot_count_v1(requests.take((index + 1) as int), requests[index as int].allocation.slot) <= before.leases@.len(),
    ensures stable_acquire_prefix_ready_v1(before, requests, index + 1),
{
    assert forall|i: int| #![trigger requests[i]] #![trigger stable_acquire_prefix_entry_v1(before, requests, i)] 0 <= i < index + 1
        implies stable_acquire_prefix_entry_v1(before, requests, i) by {
        if i < index { assert(stable_acquire_prefix_entry_v1(before, requests, i)); }
        else { assert(i == index); }
    }
}

#[verifier::spinoff_prover]
proof fn stable_acquire_scan_commit_ready_v1(
    before: ContextReadLeasedJournalV1, consumer: WriterKeyV1, requests: Seq<ContextAllocationReadV1>, output: Seq<Option<ContextReadLeaseReferenceV1>>,
    index: nat, state: StableReadAcquireScanV1,
)
    requires before.readers@.len() == before.journal.allocations@.len(), requests.len() <= usize::MAX,
        requests.len() <= u64::MAX,
        stable_acquire_header_v1(before, consumer, requests.len() as usize, output) == Ok(()),
        index <= requests.len(), stable_acquire_scan_v1(before, requests, index, state) == Ok(()),
        stable_canonical_prefix_v1(Seq::new(requests.len(), |i: int| stable_read_order_v1(requests[i])), index, state),
        forall|i: int| 0 <= i < index ==> stable_read_decision_v1(before.journal, #[trigger] requests[i]) == Ok(()),
        stable_acquire_prefix_ready_v1(before, requests, index),
    ensures stable_acquire_commit_ready_v1(before, requests), output.len() == requests.len(),
        forall|i: int| 0 <= i < requests.len() ==> stable_read_decision_v1(before.journal, #[trigger] requests[i]) == Ok(()),
    decreases requests.len() - index,
{
    let keys = Seq::new(requests.len(), |i: int| stable_read_order_v1(requests[i]));
    if index < requests.len() {
        let request = requests[index as int];
        let next = match stable_acquire_item_v1(before, request, index as usize, state) { Ok(next) => next, Err(_) => state };
        stable_canonical_prefix_extend_v1(keys, index, state);
        stable_read_count_local_dominance_v1(before.journal, requests.take((index + 1) as int), keys.take((index + 1) as int), request);
        assert(next == StableReadAcquireScanV1 { previous: Some(keys[index as int]),
            group: stable_next_group_v1(state.previous, state.group, keys[index as int]) });
        assert(stable_canonical_prefix_v1(keys, index + 1, next));
        assert(stable_read_slot_count_v1(requests.take((index + 1) as int), request.allocation.slot) <= next.group);
        assert(before.readers@[request.allocation.slot as int] + next.group <= before.leases@.len());
        stable_acquire_prefix_extend_v1(before, requests, index);
        stable_acquire_scan_commit_ready_v1(before, consumer, requests, output, index + 1, next);
    }
}

proof fn stable_acquire_preflight_implies_commit_ready_v1(
    before: ContextReadLeasedJournalV1, consumer: WriterKeyV1, requests: Seq<ContextAllocationReadV1>, output: Seq<Option<ContextReadLeaseReferenceV1>>,
)
    requires before.readers@.len() == before.journal.allocations@.len(), requests.len() <= usize::MAX,
        requests.len() <= u64::MAX, stable_acquire_decision_v1(before, consumer, requests, output) == Ok(()),
    ensures stable_acquire_commit_ready_v1(before, requests), output.len() == requests.len(),
        forall|i: int| 0 <= i < requests.len() ==> stable_read_decision_v1(before.journal, #[trigger] requests[i]) == Ok(()),
{
    stable_acquire_scan_commit_ready_v1(before, consumer, requests, output, 0, StableReadAcquireScanV1 { previous: None, group: 0 });
}

spec fn stable_acquire_execution_relation_v1(
    before: ContextReadLeasedJournalV1, after: ContextReadLeasedJournalV1, consumer: WriterKeyV1, requests: Seq<ContextAllocationReadV1>,
    output_before: Seq<Option<ContextReadLeaseReferenceV1>>, output_after: Seq<Option<ContextReadLeaseReferenceV1>>, result: Result<(), ReadErrorV1>,
) -> bool {
    &&& result == stable_acquire_decision_v1(before, consumer, requests, output_before)
    &&& match result {
        Err(_) => after == before && output_after == output_before,
        Ok(()) => stable_acquire_commit_relation_v1(before, after, consumer, requests, output_after),
    }
}

}
