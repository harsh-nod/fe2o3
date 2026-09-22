verus! {

type ProducerReadReleaseOrderV1 = (u64, u64, u64, u64);

spec fn producer_release_consumer_same_v1(left: WriterKeyV1, right: WriterKeyV1) -> bool { left == right }

spec fn producer_release_read_order_v1(request: ContextProducerReadV1, incarnation: u64) -> ProducerReadReleaseOrderV1 {
    (request.read.allocation.key.local, request.read.byte_offset, request.read.byte_len, incarnation)
}

spec fn producer_release_order_lt_v1(left: ProducerReadReleaseOrderV1, right: ProducerReadReleaseOrderV1) -> bool {
    left.0 < right.0 || (left.0 == right.0 && (left.1 < right.1
        || (left.1 == right.1 && (left.2 < right.2 || (left.2 == right.2 && left.3 < right.3)))))
}

spec fn producer_release_next_group_v1(previous: Option<ProducerReadReleaseOrderV1>, group: usize, key: ProducerReadReleaseOrderV1) -> usize {
    if previous.is_some() && previous.unwrap().0 == key.0 { (group + 1) as usize } else { 1 }
}

spec fn producer_release_header_v1(
    contents: ContextProducerReadJournalV1, consumer: WriterKeyV1, evidence_consumer: WriterKeyV1,
    count: usize, observed_free_capacity: usize,
) -> Result<(), ReadErrorV1> {
    if !producer_release_consumer_same_v1(evidence_consumer, consumer) { Err(ReadErrorV1::SettlementEvidenceMismatch) }
    else if count == 0 { Err(ReadErrorV1::RosterCapacity) }
    else if contents.free@.len() + count > usize::MAX
        || contents.free@.len() + count > contents.reservations@.len()
        || contents.free@.len() + count > observed_free_capacity { Err(ReadErrorV1::InvalidState) }
    else { Ok(()) }
}

// Storage coverage is needed only when the header admits per-reference validation.
spec fn producer_release_domain_v1(contents: ContextProducerReadJournalV1, consumer: WriterKeyV1,
    evidence_consumer: WriterKeyV1, count: usize, observed_free_capacity: usize) -> bool {
    producer_release_header_v1(contents, consumer, evidence_consumer, count, observed_free_capacity).is_ok()
        ==> contents.counts@.len() >= contents.stable.journal.allocations@.len()
}

spec fn producer_release_item_v1(
    contents: ContextProducerReadJournalV1, consumer: WriterKeyV1, reference: ContextProducerReadReferenceV1, state: ProducerReadReleaseScanV1,
) -> Result<ProducerReadReleaseScanV1, ReadErrorV1> {
    if !producer_release_consumer_same_v1(reference.consumer, consumer) { Err(ReadErrorV1::InvalidReference) }
    else { match producer_lookup_decision_v1(contents, reference) {
        Err(error) => Err(error),
        Ok(request) => {
            let key = producer_release_read_order_v1(request, reference.incarnation);
            if state.previous.is_some() && !producer_release_order_lt_v1(state.previous.unwrap(), key) {
                Err(ReadErrorV1::NonCanonicalRoster)
            } else {
                let group = producer_release_next_group_v1(state.previous, state.group, key);
                if contents.counts@[request.read.allocation.slot as int] < group { Err(ReadErrorV1::InvalidState) }
                else { Ok(ProducerReadReleaseScanV1 { previous: Some(key), group }) }
            }
        },
    } }
}

spec fn producer_release_scan_v1(
    contents: ContextProducerReadJournalV1, consumer: WriterKeyV1, references: Seq<ContextProducerReadReferenceV1>, index: nat, state: ProducerReadReleaseScanV1,
) -> Result<(), ReadErrorV1>
    decreases references.len() - index,
{
    if index >= references.len() { Ok(()) }
    else { match producer_release_item_v1(contents, consumer, references[index as int], state) {
        Err(error) => Err(error),
        Ok(next) => producer_release_scan_v1(contents, consumer, references, index + 1, next),
    } }
}

spec fn producer_release_decision_v1(
    contents: ContextProducerReadJournalV1, consumer: WriterKeyV1, references: Seq<ContextProducerReadReferenceV1>,
    evidence_consumer: WriterKeyV1, observed_free_capacity: usize,
) -> Result<(), ReadErrorV1> {
    match producer_release_header_v1(contents, consumer, evidence_consumer, references.len() as usize, observed_free_capacity) {
        Err(error) => Err(error),
        Ok(()) => producer_release_scan_v1(contents, consumer, references, 0, ProducerReadReleaseScanV1 { previous: None, group: 0 }),
    }
}

spec fn producer_released_requests_v1(before: ContextProducerReadJournalV1, references: Seq<ContextProducerReadReferenceV1>) -> Seq<ContextProducerReadV1> {
    Seq::new(references.len(), |i: int| before.reservations@[references[i].slot as int].unwrap().request)
}

spec fn producer_release_commit_ready_v1(before: ContextProducerReadJournalV1, references: Seq<ContextProducerReadReferenceV1>) -> bool {
    &&& before.free@.len() + references.len() <= usize::MAX
    &&& before.free@.len() + references.len() <= before.reservations@.len()
    &&& forall|i: int, j: int| 0 <= i < j < references.len() ==> references[i].slot != references[j].slot
    &&& producer_release_prefix_ready_v1(before, references, references.len())
}

spec fn producer_release_prefix_ready_v1(before: ContextProducerReadJournalV1, references: Seq<ContextProducerReadReferenceV1>, end: nat) -> bool {
    forall|i: int| 0 <= i < end ==> {
        let slot = (#[trigger] references[i]).slot;
        let entry = before.reservations@[slot as int].unwrap();
        &&& slot < before.reservations@.len()
        &&& before.reservations@[slot as int].is_some()
        &&& entry.reference == references[i]
        &&& entry.request.read.allocation.slot < before.counts@.len()
        &&& producer_request_count_v1(producer_released_requests_v1(before, references).take(i + 1), entry.request.read.allocation.slot)
            <= before.counts@[entry.request.read.allocation.slot as int]
    }
}

spec fn producer_released_reservations_v1(before: ContextProducerReadJournalV1, references: Seq<ContextProducerReadReferenceV1>, count: nat)
    -> Seq<Option<ReservationV1>>
    decreases count,
{
    if count == 0 { before.reservations@ }
    else { producer_released_reservations_v1(before, references, (count - 1) as nat).update(references[count - 1].slot as int, None) }
}

proof fn producer_released_reservations_unselected_v1(before: ContextProducerReadJournalV1, references: Seq<ContextProducerReadReferenceV1>, count: nat, slot: int)
    requires count <= references.len(), 0 <= slot < before.reservations@.len(),
        forall|i: int| 0 <= i < count ==> references[i].slot != slot,
        forall|i: int| 0 <= i < count ==> references[i].slot < before.reservations@.len(),
    ensures producer_released_reservations_v1(before, references, count)[slot] == before.reservations@[slot],
        producer_released_reservations_v1(before, references, count).len() == before.reservations@.len(),
    decreases count,
{
    if count > 0 { producer_released_reservations_unselected_v1(before, references, (count - 1) as nat, slot); }
}

spec fn producer_release_commit_prefix_v1(
    before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1, references: Seq<ContextProducerReadReferenceV1>, count: nat,
) -> bool {
    &&& producer_stable_frame_v1(before, after)
    &&& after.next_incarnation == before.next_incarnation
    &&& after.free@ == before.free@ + Seq::new(count, |i: int| references[i].slot)
    &&& after.reservations@ == producer_released_reservations_v1(before, references, count)
    &&& after.reservations@.len() == before.reservations@.len()
    &&& after.counts@.len() == before.counts@.len()
    &&& forall|a: int| 0 <= a < after.counts@.len() ==>
        after.counts@[a] == before.counts@[a]
            - producer_request_count_v1(producer_released_requests_v1(before, references).take(count as int), a as usize)
}

spec fn producer_release_order_local_count_v1(keys: Seq<ProducerReadReleaseOrderV1>, local: u64) -> nat
    decreases keys.len(),
{
    if keys.len() == 0 { 0 }
    else { producer_release_order_local_count_v1(keys.drop_last(), local) + if keys.last().0 == local { 1nat } else { 0nat } }
}

proof fn producer_release_order_local_count_zero_v1(keys: Seq<ProducerReadReleaseOrderV1>, local: u64)
    requires forall|i: int| 0 <= i < keys.len() ==> (#[trigger] keys[i]).0 != local,
    ensures producer_release_order_local_count_v1(keys, local) == 0,
    decreases keys.len(),
{
    if keys.len() > 0 { producer_release_order_local_count_zero_v1(keys.drop_last(), local); }
}

proof fn producer_release_order_transitive_v1(left: ProducerReadReleaseOrderV1, middle: ProducerReadReleaseOrderV1, right: ProducerReadReleaseOrderV1)
    requires producer_release_order_lt_v1(left, middle), producer_release_order_lt_v1(middle, right),
    ensures producer_release_order_lt_v1(left, right),
{}

spec fn producer_release_canonical_prefix_v1(keys: Seq<ProducerReadReleaseOrderV1>, end: nat, state: ProducerReadReleaseScanV1) -> bool {
    &&& end <= keys.len()
    &&& state.group <= end
    &&& forall|i: int, j: int| 0 <= i < j < end ==> producer_release_order_lt_v1(#[trigger] keys[i], #[trigger] keys[j])
    &&& if end == 0 { state.previous.is_none() && state.group == 0 }
        else { state.previous == Some(keys[end - 1])
            && state.group == producer_release_order_local_count_v1(keys.take(end as int), keys[end - 1].0) }
}

proof fn producer_release_canonical_prefix_extend_v1(keys: Seq<ProducerReadReleaseOrderV1>, end: nat, state: ProducerReadReleaseScanV1)
    requires producer_release_canonical_prefix_v1(keys, end, state), end < keys.len(), end < usize::MAX,
        state.previous.is_some() ==> producer_release_order_lt_v1(state.previous.unwrap(), keys[end as int]),
    ensures producer_release_canonical_prefix_v1(keys, end + 1, ProducerReadReleaseScanV1 { previous: Some(keys[end as int]),
        group: producer_release_next_group_v1(state.previous, state.group, keys[end as int]) }),
{
    let key = keys[end as int];
    assert forall|i: int, j: int| 0 <= i < j < end + 1 implies producer_release_order_lt_v1(keys[i], keys[j]) by {
        if j == end && i < end - 1 { producer_release_order_transitive_v1(keys[i], keys[end - 1], key); }
    }
    assert(keys.take((end + 1) as int).drop_last() =~= keys.take(end as int));
    assert(keys.take((end + 1) as int).last() == key);
    if end == 0 {
        assert(keys.take(0) =~= Seq::<ProducerReadReleaseOrderV1>::empty());
    } else if keys[end - 1].0 != key.0 {
        assert forall|i: int| 0 <= i < end implies (#[trigger] keys.take(end as int)[i]).0 != key.0 by {
            if i < end - 1 { assert(producer_release_order_lt_v1(keys[i], keys[end - 1])); }
        }
        producer_release_order_local_count_zero_v1(keys.take(end as int), key.0);
    }
    assert(producer_release_order_local_count_v1(keys.take((end + 1) as int), key.0)
        == producer_release_order_local_count_v1(keys.take(end as int), key.0) + 1);
}

proof fn producer_release_same_slot_local_v1(journal: JournalContentsV1, left: ContextProducerReadV1, right: ContextProducerReadV1)
    requires producer_status_decision_v1(journal, left).is_ok(), producer_status_decision_v1(journal, right).is_ok(),
        left.read.allocation.slot == right.read.allocation.slot,
    ensures left.read.allocation.key.local == right.read.allocation.key.local,
{}

proof fn producer_release_read_count_local_dominance_v1(
    journal: JournalContentsV1, requests: Seq<ContextProducerReadV1>, keys: Seq<ProducerReadReleaseOrderV1>, target: ContextProducerReadV1,
)
    requires requests.len() == keys.len(), producer_status_decision_v1(journal, target).is_ok(),
        forall|i: int| 0 <= i < requests.len() ==> producer_status_decision_v1(journal, #[trigger] requests[i]).is_ok(),
        forall|i: int| 0 <= i < requests.len() ==> (#[trigger] keys[i]).0 == requests[i].read.allocation.key.local,
    ensures producer_request_count_v1(requests, target.read.allocation.slot)
        <= producer_release_order_local_count_v1(keys, target.read.allocation.key.local),
    decreases requests.len(),
{
    if requests.len() > 0 {
        producer_release_read_count_local_dominance_v1(journal, requests.drop_last(), keys.drop_last(), target);
        if requests.last().read.allocation.slot == target.read.allocation.slot {
            producer_release_same_slot_local_v1(journal, requests.last(), target);
        }
    }
}

proof fn producer_release_scan_commit_ready_v1(
    before: ContextProducerReadJournalV1, consumer: WriterKeyV1, references: Seq<ContextProducerReadReferenceV1>,
    evidence_consumer: WriterKeyV1, observed_free_capacity: usize,
    index: nat, state: ProducerReadReleaseScanV1,
)
    requires before.counts@.len() >= before.stable.journal.allocations@.len(), references.len() <= usize::MAX,
        producer_release_header_v1(before, consumer, evidence_consumer, references.len() as usize, observed_free_capacity) == Ok(()),
        index <= references.len(), producer_release_scan_v1(before, consumer, references, index, state) == Ok(()),
        producer_release_canonical_prefix_v1(Seq::new(references.len(), |i: int|
            producer_release_read_order_v1(producer_released_requests_v1(before, references)[i], references[i].incarnation)), index, state),
        forall|i: int| 0 <= i < index ==>
            producer_status_decision_v1(before.stable.journal, #[trigger] producer_released_requests_v1(before, references)[i]).is_ok(),
        producer_release_prefix_ready_v1(before, references, index),
    ensures producer_release_commit_ready_v1(before, references),
    decreases references.len() - index,
{
    let requests = producer_released_requests_v1(before, references);
    let keys = Seq::new(references.len(), |i: int| producer_release_read_order_v1(requests[i], references[i].incarnation));
    if index < references.len() {
        let reference = references[index as int];
        let next = match producer_release_item_v1(before, consumer, reference, state) { Ok(next) => next, Err(_) => state };
        producer_release_canonical_prefix_extend_v1(keys, index, state);
        producer_release_read_count_local_dominance_v1(before.stable.journal, requests.take((index + 1) as int), keys.take((index + 1) as int), requests[index as int]);
        assert(next == ProducerReadReleaseScanV1 { previous: Some(keys[index as int]),
            group: producer_release_next_group_v1(state.previous, state.group, keys[index as int]) });
        producer_release_scan_commit_ready_v1(before, consumer, references, evidence_consumer, observed_free_capacity, index + 1, next);
    } else {
        assert forall|i: int, j: int| 0 <= i < j < references.len() implies references[i].slot != references[j].slot by {
            if references[i].slot == references[j].slot {
                assert(keys[i] == keys[j]);
                assert(producer_release_order_lt_v1(keys[i], keys[j]));
            }
        }
    }
}

proof fn producer_release_preflight_implies_commit_ready_v1(
    before: ContextProducerReadJournalV1, consumer: WriterKeyV1, references: Seq<ContextProducerReadReferenceV1>,
    evidence_consumer: WriterKeyV1, observed_free_capacity: usize,
)
    requires before.counts@.len() >= before.stable.journal.allocations@.len(), references.len() <= usize::MAX,
        producer_release_decision_v1(before, consumer, references, evidence_consumer, observed_free_capacity) == Ok(()),
    ensures producer_release_commit_ready_v1(before, references),
{
    producer_release_scan_commit_ready_v1(before, consumer, references, evidence_consumer, observed_free_capacity,
        0, ProducerReadReleaseScanV1 { previous: None, group: 0 });
}

spec fn producer_release_execution_relation_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    consumer: WriterKeyV1, references: Seq<ContextProducerReadReferenceV1>, evidence_consumer: WriterKeyV1,
    observed_free_capacity: usize, result: Result<(), ReadErrorV1>) -> bool {
    &&& result == producer_release_decision_v1(before, consumer, references, evidence_consumer, observed_free_capacity)
    &&& match result {
        Err(_) => after == before,
        Ok(()) => producer_release_commit_prefix_v1(before, after, references, references.len()),
    }
}

}
