verus! {

type StableReadReleaseOrderV1 = (u64, u64, u64, u64);

spec fn stable_read_consumer_same_v1(left: WriterKeyV1, right: WriterKeyV1) -> bool { left == right }

spec fn stable_read_reference_same_v1(left: ContextReadLeaseReferenceV1, right: ContextReadLeaseReferenceV1) -> bool {
    left.slot == right.slot && left.incarnation == right.incarnation
        && stable_read_consumer_same_v1(left.consumer, right.consumer)
}

spec fn stable_lease_decision_v1(contents: ContextReadLeasedJournalV1, reference: ContextReadLeaseReferenceV1)
    -> Result<ContextAllocationReadV1, ReadErrorV1>
{
    if reference.slot >= contents.leases@.len() { Err(ReadErrorV1::InvalidReference) }
    else { match contents.leases@[reference.slot as int] {
        None => Err(ReadErrorV1::InvalidReference),
        Some(entry) => {
            if !stable_read_reference_same_v1(entry.reference, reference) { Err(ReadErrorV1::InvalidReference) }
            else { match stable_read_decision_v1(contents.journal, entry.request) {
                Err(error) => Err(error),
                Ok(()) => Ok(entry.request),
            } }
        },
    } }
}

spec fn stable_release_read_order_v1(request: ContextAllocationReadV1, incarnation: u64) -> StableReadReleaseOrderV1 {
    (request.allocation.key.local, request.byte_offset, request.byte_len, incarnation)
}

spec fn stable_release_order_lt_v1(left: StableReadReleaseOrderV1, right: StableReadReleaseOrderV1) -> bool {
    left.0 < right.0 || (left.0 == right.0 && (left.1 < right.1
        || (left.1 == right.1 && (left.2 < right.2 || (left.2 == right.2 && left.3 < right.3)))))
}

spec fn stable_release_next_group_v1(previous: Option<StableReadReleaseOrderV1>, group: usize, key: StableReadReleaseOrderV1) -> usize {
    if previous.is_some() && previous.unwrap().0 == key.0 { (group + 1) as usize } else { 1 }
}

spec fn stable_release_header_v1(
    contents: ContextReadLeasedJournalV1, consumer: WriterKeyV1, evidence_consumer: WriterKeyV1,
    count: usize, observed_free_capacity: usize,
) -> Result<(), ReadErrorV1> {
    if !stable_read_consumer_same_v1(evidence_consumer, consumer) { Err(ReadErrorV1::SettlementEvidenceMismatch) }
    else if count == 0 { Err(ReadErrorV1::RosterCapacity) }
    else if contents.free_reads@.len() + count > usize::MAX
        || contents.free_reads@.len() + count > contents.leases@.len()
        || contents.free_reads@.len() + count > observed_free_capacity { Err(ReadErrorV1::InvalidState) }
    else { Ok(()) }
}

spec fn stable_release_item_v1(
    contents: ContextReadLeasedJournalV1, consumer: WriterKeyV1, reference: ContextReadLeaseReferenceV1, state: StableReadReleaseScanV1,
) -> Result<StableReadReleaseScanV1, ReadErrorV1> {
    if !stable_read_consumer_same_v1(reference.consumer, consumer) { Err(ReadErrorV1::InvalidReference) }
    else { match stable_lease_decision_v1(contents, reference) {
        Err(error) => Err(error),
        Ok(request) => {
            let key = stable_release_read_order_v1(request, reference.incarnation);
            if state.previous.is_some() && !stable_release_order_lt_v1(state.previous.unwrap(), key) {
                Err(ReadErrorV1::NonCanonicalRoster)
            } else {
                let group = stable_release_next_group_v1(state.previous, state.group, key);
                if contents.readers@[request.allocation.slot as int] < group { Err(ReadErrorV1::InvalidState) }
                else { Ok(StableReadReleaseScanV1 { previous: Some(key), group }) }
            }
        },
    } }
}

spec fn stable_release_scan_v1(
    contents: ContextReadLeasedJournalV1, consumer: WriterKeyV1, references: Seq<ContextReadLeaseReferenceV1>, index: nat, state: StableReadReleaseScanV1,
) -> Result<(), ReadErrorV1>
    decreases references.len() - index,
{
    if index >= references.len() { Ok(()) }
    else { match stable_release_item_v1(contents, consumer, references[index as int], state) {
        Err(error) => Err(error),
        Ok(next) => stable_release_scan_v1(contents, consumer, references, index + 1, next),
    } }
}

spec fn stable_release_decision_v1(
    contents: ContextReadLeasedJournalV1, consumer: WriterKeyV1, references: Seq<ContextReadLeaseReferenceV1>,
    evidence_consumer: WriterKeyV1, observed_free_capacity: usize,
) -> Result<(), ReadErrorV1> {
    match stable_release_header_v1(contents, consumer, evidence_consumer, references.len() as usize, observed_free_capacity) {
        Err(error) => Err(error),
        Ok(()) => stable_release_scan_v1(contents, consumer, references, 0, StableReadReleaseScanV1 { previous: None, group: 0 }),
    }
}

spec fn stable_released_requests_v1(before: ContextReadLeasedJournalV1, references: Seq<ContextReadLeaseReferenceV1>) -> Seq<ContextAllocationReadV1> {
    Seq::new(references.len(), |i: int| before.leases@[references[i].slot as int].unwrap().request)
}

spec fn stable_release_commit_ready_v1(before: ContextReadLeasedJournalV1, references: Seq<ContextReadLeaseReferenceV1>) -> bool {
    &&& before.free_reads@.len() + references.len() <= usize::MAX
    &&& before.free_reads@.len() + references.len() <= before.leases@.len()
    &&& forall|i: int, j: int| 0 <= i < j < references.len() ==> references[i].slot != references[j].slot
    &&& stable_release_prefix_ready_v1(before, references, references.len())
}

spec fn stable_release_prefix_ready_v1(before: ContextReadLeasedJournalV1, references: Seq<ContextReadLeaseReferenceV1>, end: nat) -> bool {
    forall|i: int| 0 <= i < end ==> {
        let slot = (#[trigger] references[i]).slot;
        let entry = before.leases@[slot as int].unwrap();
        &&& slot < before.leases@.len()
        &&& before.leases@[slot as int].is_some()
        &&& stable_read_reference_same_v1(entry.reference, references[i])
        &&& entry.request.allocation.slot < before.readers@.len()
        &&& stable_read_slot_count_v1(stable_released_requests_v1(before, references).take(i + 1), entry.request.allocation.slot)
            <= before.readers@[entry.request.allocation.slot as int]
    }
}

spec fn stable_released_leases_v1(before: ContextReadLeasedJournalV1, references: Seq<ContextReadLeaseReferenceV1>, count: nat)
    -> Seq<Option<ReadLeaseV1>>
    decreases count,
{
    if count == 0 { before.leases@ }
    else { stable_released_leases_v1(before, references, (count - 1) as nat).update(references[count - 1].slot as int, None) }
}

proof fn stable_released_leases_unselected_v1(before: ContextReadLeasedJournalV1, references: Seq<ContextReadLeaseReferenceV1>, count: nat, slot: int)
    requires count <= references.len(), 0 <= slot < before.leases@.len(),
        forall|i: int| 0 <= i < count ==> references[i].slot != slot,
        forall|i: int| 0 <= i < count ==> references[i].slot < before.leases@.len(),
    ensures stable_released_leases_v1(before, references, count)[slot] == before.leases@[slot],
        stable_released_leases_v1(before, references, count).len() == before.leases@.len(),
    decreases count,
{
    if count > 0 { stable_released_leases_unselected_v1(before, references, (count - 1) as nat, slot); }
}

spec fn stable_release_commit_prefix_v1(
    before: ContextReadLeasedJournalV1, after: ContextReadLeasedJournalV1, references: Seq<ContextReadLeaseReferenceV1>, count: nat,
) -> bool {
    &&& stable_reader_journal_frame_v1(before, after)
    &&& after.next_incarnation == before.next_incarnation
    &&& after.free_reads@ == before.free_reads@ + Seq::new(count, |i: int| references[i].slot)
    &&& after.leases@ == stable_released_leases_v1(before, references, count)
    &&& after.leases@.len() == before.leases@.len()
    &&& after.readers@.len() == before.readers@.len()
    &&& forall|a: int| 0 <= a < after.readers@.len() ==>
        after.readers@[a] == before.readers@[a]
            - stable_read_slot_count_v1(stable_released_requests_v1(before, references).take(count as int), a as usize)
}

spec fn stable_release_order_local_count_v1(keys: Seq<StableReadReleaseOrderV1>, local: u64) -> nat
    decreases keys.len(),
{
    if keys.len() == 0 { 0 }
    else { stable_release_order_local_count_v1(keys.drop_last(), local) + if keys.last().0 == local { 1nat } else { 0nat } }
}

proof fn stable_release_order_local_count_zero_v1(keys: Seq<StableReadReleaseOrderV1>, local: u64)
    requires forall|i: int| 0 <= i < keys.len() ==> (#[trigger] keys[i]).0 != local,
    ensures stable_release_order_local_count_v1(keys, local) == 0,
    decreases keys.len(),
{
    if keys.len() > 0 { stable_release_order_local_count_zero_v1(keys.drop_last(), local); }
}

proof fn stable_release_order_transitive_v1(left: StableReadReleaseOrderV1, middle: StableReadReleaseOrderV1, right: StableReadReleaseOrderV1)
    requires stable_release_order_lt_v1(left, middle), stable_release_order_lt_v1(middle, right),
    ensures stable_release_order_lt_v1(left, right),
{}

spec fn stable_release_canonical_prefix_v1(keys: Seq<StableReadReleaseOrderV1>, end: nat, state: StableReadReleaseScanV1) -> bool {
    &&& end <= keys.len()
    &&& state.group <= end
    &&& forall|i: int, j: int| 0 <= i < j < end ==> stable_release_order_lt_v1(#[trigger] keys[i], #[trigger] keys[j])
    &&& if end == 0 { state.previous.is_none() && state.group == 0 }
        else { state.previous == Some(keys[end - 1])
            && state.group == stable_release_order_local_count_v1(keys.take(end as int), keys[end - 1].0) }
}

proof fn stable_release_canonical_prefix_extend_v1(keys: Seq<StableReadReleaseOrderV1>, end: nat, state: StableReadReleaseScanV1)
    requires stable_release_canonical_prefix_v1(keys, end, state), end < keys.len(), end < usize::MAX,
        state.previous.is_some() ==> stable_release_order_lt_v1(state.previous.unwrap(), keys[end as int]),
    ensures stable_release_canonical_prefix_v1(keys, end + 1, StableReadReleaseScanV1 { previous: Some(keys[end as int]),
        group: stable_release_next_group_v1(state.previous, state.group, keys[end as int]) }),
{
    let key = keys[end as int];
    assert forall|i: int, j: int| 0 <= i < j < end + 1 implies stable_release_order_lt_v1(keys[i], keys[j]) by {
        if j == end && i < end - 1 { stable_release_order_transitive_v1(keys[i], keys[end - 1], key); }
    }
    assert(keys.take((end + 1) as int).drop_last() =~= keys.take(end as int));
    assert(keys.take((end + 1) as int).last() == key);
    if end == 0 {
        assert(keys.take(0) =~= Seq::<StableReadReleaseOrderV1>::empty());
    } else if keys[end - 1].0 != key.0 {
        assert forall|i: int| 0 <= i < end implies (#[trigger] keys.take(end as int)[i]).0 != key.0 by {
            if i < end - 1 { assert(stable_release_order_lt_v1(keys[i], keys[end - 1])); }
        }
        stable_release_order_local_count_zero_v1(keys.take(end as int), key.0);
    }
    assert(stable_release_order_local_count_v1(keys.take((end + 1) as int), key.0)
        == stable_release_order_local_count_v1(keys.take(end as int), key.0) + 1);
}

proof fn stable_release_read_count_local_dominance_v1(
    journal: JournalContentsV1, requests: Seq<ContextAllocationReadV1>, keys: Seq<StableReadReleaseOrderV1>, target: ContextAllocationReadV1,
)
    requires requests.len() == keys.len(), stable_read_decision_v1(journal, target) == Ok(()),
        forall|i: int| 0 <= i < requests.len() ==> stable_read_decision_v1(journal, #[trigger] requests[i]) == Ok(()),
        forall|i: int| 0 <= i < requests.len() ==> (#[trigger] keys[i]).0 == requests[i].allocation.key.local,
    ensures stable_read_slot_count_v1(requests, target.allocation.slot)
        <= stable_release_order_local_count_v1(keys, target.allocation.key.local),
    decreases requests.len(),
{
    if requests.len() > 0 {
        stable_release_read_count_local_dominance_v1(journal, requests.drop_last(), keys.drop_last(), target);
        if requests.last().allocation.slot == target.allocation.slot {
            stable_validated_same_slot_local_v1(journal, requests.last(), target);
        }
    }
}

proof fn stable_release_scan_commit_ready_v1(
    before: ContextReadLeasedJournalV1, consumer: WriterKeyV1, references: Seq<ContextReadLeaseReferenceV1>,
    evidence_consumer: WriterKeyV1, observed_free_capacity: usize,
    index: nat, state: StableReadReleaseScanV1,
)
    requires before.readers@.len() == before.journal.allocations@.len(), references.len() <= usize::MAX,
        stable_release_header_v1(before, consumer, evidence_consumer, references.len() as usize, observed_free_capacity) == Ok(()),
        index <= references.len(), stable_release_scan_v1(before, consumer, references, index, state) == Ok(()),
        stable_release_canonical_prefix_v1(Seq::new(references.len(), |i: int|
            stable_release_read_order_v1(stable_released_requests_v1(before, references)[i], references[i].incarnation)), index, state),
        forall|i: int| 0 <= i < index ==>
            stable_read_decision_v1(before.journal, #[trigger] stable_released_requests_v1(before, references)[i]) == Ok(()),
        stable_release_prefix_ready_v1(before, references, index),
    ensures stable_release_commit_ready_v1(before, references),
    decreases references.len() - index,
{
    let requests = stable_released_requests_v1(before, references);
    let keys = Seq::new(references.len(), |i: int| stable_release_read_order_v1(requests[i], references[i].incarnation));
    if index < references.len() {
        let reference = references[index as int];
        let next = match stable_release_item_v1(before, consumer, reference, state) { Ok(next) => next, Err(_) => state };
        stable_release_canonical_prefix_extend_v1(keys, index, state);
        stable_release_read_count_local_dominance_v1(before.journal, requests.take((index + 1) as int), keys.take((index + 1) as int), requests[index as int]);
        assert(next == StableReadReleaseScanV1 { previous: Some(keys[index as int]),
            group: stable_release_next_group_v1(state.previous, state.group, keys[index as int]) });
        stable_release_scan_commit_ready_v1(before, consumer, references, evidence_consumer, observed_free_capacity, index + 1, next);
    } else {
        assert forall|i: int, j: int| 0 <= i < j < references.len() implies references[i].slot != references[j].slot by {
            if references[i].slot == references[j].slot {
                assert(keys[i] == keys[j]);
                assert(stable_release_order_lt_v1(keys[i], keys[j]));
            }
        }
    }
}

proof fn stable_release_preflight_implies_commit_ready_v1(
    before: ContextReadLeasedJournalV1, consumer: WriterKeyV1, references: Seq<ContextReadLeaseReferenceV1>,
    evidence_consumer: WriterKeyV1, observed_free_capacity: usize,
)
    requires before.readers@.len() == before.journal.allocations@.len(), references.len() <= usize::MAX,
        stable_release_decision_v1(before, consumer, references, evidence_consumer, observed_free_capacity) == Ok(()),
    ensures stable_release_commit_ready_v1(before, references),
{
    stable_release_scan_commit_ready_v1(before, consumer, references, evidence_consumer, observed_free_capacity,
        0, StableReadReleaseScanV1 { previous: None, group: 0 });
}

spec fn stable_release_execution_relation_v1(before: ContextReadLeasedJournalV1, after: ContextReadLeasedJournalV1,
    consumer: WriterKeyV1, references: Seq<ContextReadLeaseReferenceV1>, evidence_consumer: WriterKeyV1,
    observed_free_capacity: usize, result: Result<(), ReadErrorV1>) -> bool {
    &&& result == stable_release_decision_v1(before, consumer, references, evidence_consumer, observed_free_capacity)
    &&& match result {
        Err(_) => true,
        Ok(()) => stable_release_commit_prefix_v1(before, after, references, references.len()),
    }
}

}
