// Raw normal-content admission; duplicate selected slots retain sequential overwrite semantics.
verus! {

type ProducerReadOrderV1 = (u64, u64, u64);

spec fn producer_retained_v1(contents: ContextProducerReadJournalV1) -> int {
    contents.reservations@.len() - contents.free@.len()
}

spec fn producer_stable_retained_v1(contents: ContextProducerReadJournalV1) -> int {
    contents.stable.leases@.len() - contents.stable.free_reads@.len()
}

spec fn producer_total_retained_v1(contents: ContextProducerReadJournalV1) -> int {
    producer_stable_retained_v1(contents) + producer_retained_v1(contents)
}

spec fn producer_budget_domain_v1(contents: ContextProducerReadJournalV1) -> bool {
    &&& contents.stable.free_reads@.len() <= contents.stable.leases@.len()
    &&& contents.free@.len() <= contents.reservations@.len()
    &&& producer_stable_retained_v1(contents) <= contents.free@.len()
}

spec fn producer_remaining_v1(contents: ContextProducerReadJournalV1) -> int {
    contents.reservations@.len() - producer_total_retained_v1(contents)
}

proof fn producer_budget_arithmetic_v1(contents: ContextProducerReadJournalV1)
    requires producer_budget_domain_v1(contents),
    ensures 0 <= producer_total_retained_v1(contents) <= contents.reservations@.len(),
        0 <= producer_remaining_v1(contents) <= contents.free@.len(),
{}

spec fn producer_capacity_decision_v1(contents: ContextProducerReadJournalV1, count: usize) -> Result<(), ReadErrorV1> {
    if count > producer_remaining_v1(contents) { Err(ReadErrorV1::MemberCapacity) }
    else if contents.next_incarnation == 0 || contents.next_incarnation + count > u64::MAX {
        Err(ReadErrorV1::EpochExhausted)
    } else { Ok(()) }
}

spec fn producer_stable_capacity_decision_v1(contents: ContextProducerReadJournalV1, count: usize) -> Result<(), ReadErrorV1> {
    if count > producer_remaining_v1(contents) { Err(ReadErrorV1::MemberCapacity) }
    else { stable_capacity_decision_v1(contents.stable, count) }
}

spec fn producer_read_order_v1(request: ContextProducerReadV1) -> ProducerReadOrderV1 {
    (request.read.allocation.key.local, request.read.byte_offset, request.read.byte_len)
}

spec fn producer_order_lt_v1(left: ProducerReadOrderV1, right: ProducerReadOrderV1) -> bool {
    left.0 < right.0 || (left.0 == right.0 && (left.1 < right.1
        || (left.1 == right.1 && (left.2 < right.2))))
}

spec fn producer_next_group_v1(previous: Option<ProducerReadOrderV1>, group: usize, key: ProducerReadOrderV1) -> usize {
    if previous.is_some() && previous.unwrap().0 == key.0 { (group + 1) as usize } else { 1 }
}

spec fn producer_acquire_prefix_v1(
    contents: ContextProducerReadJournalV1, consumer: WriterKeyV1, count: usize, output: Seq<Option<ContextProducerReadReferenceV1>>,
) -> Result<(), ReadErrorV1> {
    if consumer.context_generation != contents.stable.journal.context_generation { Err(ReadErrorV1::ForeignContext) }
    else if consumer.local == 0 || consumer.local == u64::MAX || consumer.kind != WriterKindV1::Submission {
        Err(ReadErrorV1::InvalidWriterId)
    }
    else if count == 0 || count != output.len() { Err(ReadErrorV1::RosterCapacity) }
    else if exists|i: int| 0 <= i < output.len() && output[i].is_some() { Err(ReadErrorV1::InvalidState) }
    else { Ok(()) }
}

spec fn producer_acquire_header_v1(
    contents: ContextProducerReadJournalV1, consumer: WriterKeyV1, count: usize, output: Seq<Option<ContextProducerReadReferenceV1>>,
) -> Result<(), ReadErrorV1> {
    match producer_acquire_prefix_v1(contents, consumer, count, output) {
        Err(error) => Err(error), Ok(()) => producer_capacity_decision_v1(contents, count),
    }
}

spec fn producer_acquire_domain_v1(
    contents: ContextProducerReadJournalV1, consumer: WriterKeyV1, count: usize, output: Seq<Option<ContextProducerReadReferenceV1>>,
) -> bool {
    &&& producer_budget_domain_v1(contents)
    &&& producer_acquire_header_v1(contents, consumer, count, output).is_ok() ==>
        contents.counts@.len() >= contents.stable.journal.allocations@.len()
}

spec fn producer_acquire_item_v1(
    contents: ContextProducerReadJournalV1, consumer: WriterKeyV1, request: ContextProducerReadV1, index: usize, state: ProducerReadAcquireScanV1,
) -> Result<ProducerReadAcquireScanV1, ReadErrorV1> {
    match producer_validate_decision_v1(contents, request) {
        Err(error) => Err(error),
        Ok(()) => {
            let key = producer_read_order_v1(request);
            if request.producer.key.local >= consumer.local { Err(ReadErrorV1::InvalidWriterId) }
            else if state.previous.is_some() && !producer_order_lt_v1(state.previous.unwrap(), key) {
                Err(ReadErrorV1::NonCanonicalRoster)
            } else {
                let group = producer_next_group_v1(state.previous, state.group, key);
                let count = contents.counts@[request.read.allocation.slot as int] + group;
                if count > usize::MAX || count > contents.reservations@.len() { Err(ReadErrorV1::InvalidState) }
                else {
                    let slot = contents.free@[contents.free@.len() - index - 1];
                    if slot >= contents.reservations@.len() || contents.reservations@[slot as int].is_some() {
                        Err(ReadErrorV1::InvalidState)
                    } else { Ok(ProducerReadAcquireScanV1 { previous: Some(key), group }) }
                }
            }
        },
    }
}

spec fn producer_acquire_scan_v1(
    contents: ContextProducerReadJournalV1, consumer: WriterKeyV1, requests: Seq<ContextProducerReadV1>, index: nat, state: ProducerReadAcquireScanV1,
) -> Result<(), ReadErrorV1>
    decreases requests.len() - index,
{
    if index >= requests.len() { Ok(()) }
    else { match producer_acquire_item_v1(contents, consumer, requests[index as int], index as usize, state) {
        Err(error) => Err(error),
        Ok(next) => producer_acquire_scan_v1(contents, consumer, requests, index + 1, next),
    } }
}

spec fn producer_acquire_decision_v1(
    contents: ContextProducerReadJournalV1, consumer: WriterKeyV1, requests: Seq<ContextProducerReadV1>,
    output: Seq<Option<ContextProducerReadReferenceV1>>,
) -> Result<(), ReadErrorV1> {
    match producer_acquire_header_v1(contents, consumer, requests.len() as usize, output) {
        Err(error) => Err(error),
        Ok(()) => producer_acquire_scan_v1(contents, consumer, requests, 0, ProducerReadAcquireScanV1 { previous: None, group: 0 }),
    }
}

spec fn producer_request_count_v1(requests: Seq<ContextProducerReadV1>, slot: usize) -> nat
    decreases requests.len(),
{
    if requests.len() == 0 { 0 }
    else { producer_request_count_v1(requests.drop_last(), slot)
        + if requests.last().read.allocation.slot == slot { 1nat } else { 0nat } }
}

proof fn producer_request_count_bound_v1(requests: Seq<ContextProducerReadV1>, slot: usize)
    ensures producer_request_count_v1(requests, slot) <= requests.len(),
    decreases requests.len(),
{
    if requests.len() > 0 { producer_request_count_bound_v1(requests.drop_last(), slot); }
}

proof fn producer_request_count_push_v1(requests: Seq<ContextProducerReadV1>, request: ContextProducerReadV1, slot: usize)
    ensures producer_request_count_v1(requests.push(request), slot)
        == producer_request_count_v1(requests, slot) + if request.read.allocation.slot == slot { 1nat } else { 0nat },
{
    assert(requests.push(request).drop_last() =~= requests);
}

spec fn producer_stable_frame_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1) -> bool {
    after.stable == before.stable
}

spec fn producer_acquired_reference_v1(before: ContextProducerReadJournalV1, consumer: WriterKeyV1, index: int) -> ContextProducerReadReferenceV1 {
    ContextProducerReadReferenceV1 { slot: before.free@[before.free@.len() - 1 - index],
        incarnation: (before.next_incarnation + index) as u64, consumer }
}

spec fn producer_acquire_commit_ready_v1(before: ContextProducerReadJournalV1, requests: Seq<ContextProducerReadV1>) -> bool {
    &&& requests.len() <= before.free@.len()
    &&& requests.len() <= u64::MAX
    &&& 0 < before.next_incarnation
    &&& before.next_incarnation + requests.len() <= u64::MAX
    &&& producer_acquire_prefix_ready_v1(before, requests, requests.len())
}

spec fn producer_acquire_prefix_entry_v1(before: ContextProducerReadJournalV1, requests: Seq<ContextProducerReadV1>, i: int) -> bool {
    let slot = before.free@[before.free@.len() - 1 - i];
    let allocation = requests[i].read.allocation.slot;
    &&& slot < before.reservations@.len()
    &&& before.reservations@[slot as int].is_none()
    &&& allocation < before.counts@.len()
    &&& before.counts@[allocation as int]
        + producer_request_count_v1(requests.take(i + 1), allocation) <= usize::MAX
    &&& before.counts@[allocation as int]
        + producer_request_count_v1(requests.take(i + 1), allocation) <= before.reservations@.len()
}

spec fn producer_acquire_prefix_ready_v1(before: ContextProducerReadJournalV1, requests: Seq<ContextProducerReadV1>, end: nat) -> bool {
    forall|i: int| #![trigger requests[i]] #![trigger producer_acquire_prefix_entry_v1(before, requests, i)]
        0 <= i < end ==> producer_acquire_prefix_entry_v1(before, requests, i)
}

spec fn producer_acquired_reservations_v1(
    before: ContextProducerReadJournalV1, consumer: WriterKeyV1, requests: Seq<ContextProducerReadV1>, count: nat,
) -> Seq<Option<ReservationV1>>
    decreases count,
{
    if count == 0 { before.reservations@ }
    else {
        let reference = producer_acquired_reference_v1(before, consumer, count - 1);
        producer_acquired_reservations_v1(before, consumer, requests, (count - 1) as nat)
            .update(reference.slot as int, Some(ReservationV1 { reference, request: requests[count - 1] }))
    }
}

spec fn producer_acquire_commit_prefix_v1(
    before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1, consumer: WriterKeyV1,
    requests: Seq<ContextProducerReadV1>, output_before: Seq<Option<ContextProducerReadReferenceV1>>,
    output_after: Seq<Option<ContextProducerReadReferenceV1>>, count: nat,
) -> bool {
    &&& producer_stable_frame_v1(before, after)
    &&& after.next_incarnation == before.next_incarnation
    &&& after.free@ == before.free@.take(before.free@.len() - count)
    &&& after.reservations@ == producer_acquired_reservations_v1(before, consumer, requests, count)
    &&& after.reservations@.len() == before.reservations@.len()
    &&& after.counts@.len() == before.counts@.len()
    &&& forall|a: int| 0 <= a < after.counts@.len() ==>
        after.counts@[a] == before.counts@[a] + producer_request_count_v1(requests.take(count as int), a as usize)
    &&& output_after.len() == output_before.len()
    &&& forall|i: int| 0 <= i < output_after.len() ==> output_after[i]
        == if i < count { Some(producer_acquired_reference_v1(before, consumer, i)) } else { output_before[i] }
}

spec fn producer_acquire_commit_relation_v1(
    before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1, consumer: WriterKeyV1,
    requests: Seq<ContextProducerReadV1>, output: Seq<Option<ContextProducerReadReferenceV1>>,
) -> bool {
    &&& producer_stable_frame_v1(before, after)
    &&& after.next_incarnation == before.next_incarnation + requests.len()
    &&& after.free@ == before.free@.take(before.free@.len() - requests.len())
    &&& after.reservations@ == producer_acquired_reservations_v1(before, consumer, requests, requests.len())
    &&& after.reservations@.len() == before.reservations@.len()
    &&& after.counts@.len() == before.counts@.len()
    &&& forall|a: int| 0 <= a < after.counts@.len() ==>
        after.counts@[a] == before.counts@[a] + producer_request_count_v1(requests, a as usize)
    &&& output.len() == requests.len()
    &&& forall|i: int| 0 <= i < output.len() ==> output[i] == Some(producer_acquired_reference_v1(before, consumer, i))
}

spec fn producer_order_local_count_v1(keys: Seq<ProducerReadOrderV1>, local: u64) -> nat
    decreases keys.len(),
{
    if keys.len() == 0 { 0 }
    else { producer_order_local_count_v1(keys.drop_last(), local) + if keys.last().0 == local { 1nat } else { 0nat } }
}

proof fn producer_order_local_count_zero_v1(keys: Seq<ProducerReadOrderV1>, local: u64)
    requires forall|i: int| 0 <= i < keys.len() ==> (#[trigger] keys[i]).0 != local,
    ensures producer_order_local_count_v1(keys, local) == 0,
    decreases keys.len(),
{
    if keys.len() > 0 { producer_order_local_count_zero_v1(keys.drop_last(), local); }
}

proof fn producer_order_transitive_v1(left: ProducerReadOrderV1, middle: ProducerReadOrderV1, right: ProducerReadOrderV1)
    requires producer_order_lt_v1(left, middle), producer_order_lt_v1(middle, right),
    ensures producer_order_lt_v1(left, right),
{}

spec fn producer_canonical_prefix_v1(keys: Seq<ProducerReadOrderV1>, end: nat, state: ProducerReadAcquireScanV1) -> bool {
    &&& end <= keys.len()
    &&& state.group <= end
    &&& forall|i: int, j: int| 0 <= i < j < end ==> producer_order_lt_v1(#[trigger] keys[i], #[trigger] keys[j])
    &&& if end == 0 { state.previous.is_none() && state.group == 0 }
        else { state.previous == Some(keys[end - 1])
            && state.group == producer_order_local_count_v1(keys.take(end as int), keys[end - 1].0) }
}

proof fn producer_canonical_prefix_extend_v1(keys: Seq<ProducerReadOrderV1>, end: nat, state: ProducerReadAcquireScanV1)
    requires producer_canonical_prefix_v1(keys, end, state), end < keys.len(), end < usize::MAX,
        state.previous.is_some() ==> producer_order_lt_v1(state.previous.unwrap(), keys[end as int]),
    ensures producer_canonical_prefix_v1(keys, end + 1, ProducerReadAcquireScanV1 { previous: Some(keys[end as int]),
        group: producer_next_group_v1(state.previous, state.group, keys[end as int]) }),
{
    let key = keys[end as int];
    assert forall|i: int, j: int| 0 <= i < j < end + 1 implies producer_order_lt_v1(keys[i], keys[j]) by {
        if j == end && i < end - 1 { producer_order_transitive_v1(keys[i], keys[end - 1], key); }
    }
    assert(keys.take((end + 1) as int).drop_last() =~= keys.take(end as int));
    assert(keys.take((end + 1) as int).last() == key);
    if end == 0 {
        assert(keys.take(0) =~= Seq::<ProducerReadOrderV1>::empty());
    } else if keys[end - 1].0 != key.0 {
        assert forall|i: int| 0 <= i < end implies (#[trigger] keys.take(end as int)[i]).0 != key.0 by {
            if i < end - 1 { assert(producer_order_lt_v1(keys[i], keys[end - 1])); }
        }
        producer_order_local_count_zero_v1(keys.take(end as int), key.0);
    }
    assert(producer_order_local_count_v1(keys.take((end + 1) as int), key.0)
        == producer_order_local_count_v1(keys.take(end as int), key.0) + 1);
}

proof fn producer_validated_same_slot_local_v1(journal: JournalContentsV1, left: ContextProducerReadV1, right: ContextProducerReadV1)
    requires producer_status_decision_v1(journal, left) == Ok(ContextProducerReadStatusV1::Pending), producer_status_decision_v1(journal, right) == Ok(ContextProducerReadStatusV1::Pending),
        left.read.allocation.slot == right.read.allocation.slot,
    ensures left.read.allocation.key.local == right.read.allocation.key.local,
{}

proof fn producer_read_count_local_dominance_v1(
    journal: JournalContentsV1, requests: Seq<ContextProducerReadV1>, keys: Seq<ProducerReadOrderV1>, target: ContextProducerReadV1,
)
    requires requests.len() == keys.len(), producer_status_decision_v1(journal, target) == Ok(ContextProducerReadStatusV1::Pending),
        forall|i: int| 0 <= i < requests.len() ==> producer_status_decision_v1(journal, #[trigger] requests[i]) == Ok(ContextProducerReadStatusV1::Pending),
        forall|i: int| 0 <= i < requests.len() ==> (#[trigger] keys[i]).0 == requests[i].read.allocation.key.local,
    ensures producer_request_count_v1(requests, target.read.allocation.slot)
        <= producer_order_local_count_v1(keys, target.read.allocation.key.local),
    decreases requests.len(),
{
    if requests.len() > 0 {
        producer_read_count_local_dominance_v1(journal, requests.drop_last(), keys.drop_last(), target);
        if requests.last().read.allocation.slot == target.read.allocation.slot {
            producer_validated_same_slot_local_v1(journal, requests.last(), target);
        }
    }
}

#[verifier::spinoff_prover]
proof fn producer_acquire_prefix_extend_v1(before: ContextProducerReadJournalV1, requests: Seq<ContextProducerReadV1>, index: nat)
    requires producer_acquire_prefix_ready_v1(before, requests, index), index < requests.len(),
        index < before.free@.len(),
        before.free@[before.free@.len() - 1 - index] < before.reservations@.len(),
        before.reservations@[before.free@[before.free@.len() - 1 - index] as int].is_none(),
        requests[index as int].read.allocation.slot < before.counts@.len(),
        before.counts@[requests[index as int].read.allocation.slot as int]
            + producer_request_count_v1(requests.take((index + 1) as int), requests[index as int].read.allocation.slot) <= usize::MAX,
        before.counts@[requests[index as int].read.allocation.slot as int]
            + producer_request_count_v1(requests.take((index + 1) as int), requests[index as int].read.allocation.slot) <= before.reservations@.len(),
    ensures producer_acquire_prefix_ready_v1(before, requests, index + 1),
{
    assert forall|i: int| #![trigger requests[i]] #![trigger producer_acquire_prefix_entry_v1(before, requests, i)] 0 <= i < index + 1
        implies producer_acquire_prefix_entry_v1(before, requests, i) by {
        if i < index { assert(producer_acquire_prefix_entry_v1(before, requests, i)); }
        else { assert(i == index); }
    }
}

#[verifier::spinoff_prover]
proof fn producer_acquire_scan_commit_ready_v1(
    before: ContextProducerReadJournalV1, consumer: WriterKeyV1, requests: Seq<ContextProducerReadV1>, output: Seq<Option<ContextProducerReadReferenceV1>>,
    index: nat, state: ProducerReadAcquireScanV1,
)
    requires before.counts@.len() >= before.stable.journal.allocations@.len(), requests.len() <= usize::MAX,
        requests.len() <= u64::MAX,
        producer_budget_domain_v1(before),
        producer_acquire_header_v1(before, consumer, requests.len() as usize, output) == Ok(()),
        index <= requests.len(), producer_acquire_scan_v1(before, consumer, requests, index, state) == Ok(()),
        producer_canonical_prefix_v1(Seq::new(requests.len(), |i: int| producer_read_order_v1(requests[i])), index, state),
        forall|i: int| 0 <= i < index ==> producer_status_decision_v1(before.stable.journal, #[trigger] requests[i]) == Ok(ContextProducerReadStatusV1::Pending),
        producer_acquire_prefix_ready_v1(before, requests, index),
    ensures producer_acquire_commit_ready_v1(before, requests), output.len() == requests.len(),
        forall|i: int| 0 <= i < requests.len() ==> producer_status_decision_v1(before.stable.journal, #[trigger] requests[i]) == Ok(ContextProducerReadStatusV1::Pending),
    decreases requests.len() - index,
{
    let keys = Seq::new(requests.len(), |i: int| producer_read_order_v1(requests[i]));
    if index < requests.len() {
        let request = requests[index as int];
        let next = match producer_acquire_item_v1(before, consumer, request, index as usize, state) { Ok(next) => next, Err(_) => state };
        producer_canonical_prefix_extend_v1(keys, index, state);
        producer_read_count_local_dominance_v1(before.stable.journal, requests.take((index + 1) as int), keys.take((index + 1) as int), request);
        assert(next == ProducerReadAcquireScanV1 { previous: Some(keys[index as int]),
            group: producer_next_group_v1(state.previous, state.group, keys[index as int]) });
        assert(producer_canonical_prefix_v1(keys, index + 1, next));
        assert(producer_request_count_v1(requests.take((index + 1) as int), request.read.allocation.slot) <= next.group);
        assert(before.counts@[request.read.allocation.slot as int] + next.group <= before.reservations@.len());
        producer_acquire_prefix_extend_v1(before, requests, index);
        producer_acquire_scan_commit_ready_v1(before, consumer, requests, output, index + 1, next);
    }
}

proof fn producer_acquire_preflight_implies_commit_ready_v1(
    before: ContextProducerReadJournalV1, consumer: WriterKeyV1, requests: Seq<ContextProducerReadV1>, output: Seq<Option<ContextProducerReadReferenceV1>>,
)
    requires before.counts@.len() >= before.stable.journal.allocations@.len(), requests.len() <= usize::MAX,
        requests.len() <= u64::MAX, producer_budget_domain_v1(before),
        producer_acquire_decision_v1(before, consumer, requests, output) == Ok(()),
    ensures producer_acquire_commit_ready_v1(before, requests), output.len() == requests.len(),
        forall|i: int| 0 <= i < requests.len() ==> producer_status_decision_v1(before.stable.journal, #[trigger] requests[i]) == Ok(ContextProducerReadStatusV1::Pending),
{
    producer_acquire_scan_commit_ready_v1(before, consumer, requests, output, 0, ProducerReadAcquireScanV1 { previous: None, group: 0 });
}

spec fn producer_acquire_execution_relation_v1(
    before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1, consumer: WriterKeyV1, requests: Seq<ContextProducerReadV1>,
    output_before: Seq<Option<ContextProducerReadReferenceV1>>, output_after: Seq<Option<ContextProducerReadReferenceV1>>, result: Result<(), ReadErrorV1>,
) -> bool {
    &&& result == producer_acquire_decision_v1(before, consumer, requests, output_before)
    &&& match result {
        Err(_) => after == before && output_after == output_before,
        Ok(()) => producer_acquire_commit_relation_v1(before, after, consumer, requests, output_after),
    }
}

}
