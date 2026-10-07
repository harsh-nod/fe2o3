use super::*;

verus! {

pub open spec fn mixed_acquire_decision_v1(contents: ProducerReadContentsV1, consumer: WriterKeyV1,
    stable: Seq<AllocationReadV1>, stable_output: Seq<Option<ReadReferenceV1>>,
    pending: Seq<ProducerReadV1>, producer_output: Seq<Option<ProducerReadReferenceV1>>,
) -> Result<(), ReadErrorV1> {
    if consumer.context_generation != contents.stable.journal.context_generation {
        Err(ReadErrorV1::ForeignContext)
    } else if !issuable_id_v1(consumer.local) || consumer.kind != WriterKindV1::Submission {
        Err(ReadErrorV1::InvalidWriterId)
    } else if stable.len() != stable_output.len() || pending.len() != producer_output.len() {
        Err(ReadErrorV1::RosterCapacity)
    } else if stable.len() + pending.len() > usize::MAX
        || stable.len() + pending.len() > contents.reservations@.len() - producer_live_reads_v1(contents) {
        Err(ReadErrorV1::MemberCapacity)
    } else if stable.len() > 0 && acquire_decision_v1(contents.stable, consumer, stable, stable_output).is_err() {
        acquire_decision_v1(contents.stable, consumer, stable, stable_output)
    } else if pending.len() > 0 {
        producer_acquire_decision_v1(contents, consumer, pending, producer_output)
    } else {
        Ok(())
    }
}

pub open spec fn mixed_stable_step_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    consumer: WriterKeyV1, requests: Seq<AllocationReadV1>,
    output_before: Seq<Option<ReadReferenceV1>>, output_after: Seq<Option<ReadReferenceV1>>,
) -> bool {
    &&& producer_outer_frame_v1(before, after)
    &&& if requests.len() == 0 { after == before && output_after == output_before }
        else { acquire_execution_relation_v1(before.stable, after.stable, consumer, requests,
            output_before, output_after, Ok(())) }
}

pub open spec fn mixed_pending_step_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    consumer: WriterKeyV1, requests: Seq<ProducerReadV1>,
    output_before: Seq<Option<ProducerReadReferenceV1>>, output_after: Seq<Option<ProducerReadReferenceV1>>,
) -> bool {
    if requests.len() == 0 { after == before && output_after == output_before }
    else { producer_acquire_execution_relation_v1(before, after, consumer, requests,
        output_before, output_after, Ok(())) }
}

// Independent component transitions, not an assumed equality to production outputs.
pub open spec fn mixed_acquire_relation_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    consumer: WriterKeyV1, stable: Seq<AllocationReadV1>, pending: Seq<ProducerReadV1>,
    stable_before: Seq<Option<ReadReferenceV1>>, stable_after: Seq<Option<ReadReferenceV1>>,
    producer_before: Seq<Option<ProducerReadReferenceV1>>, producer_after: Seq<Option<ProducerReadReferenceV1>>,
    result: Result<(), ReadErrorV1>,
) -> bool {
    &&& result == mixed_acquire_decision_v1(before, consumer, stable, stable_before, pending, producer_before)
    &&& if result.is_err() {
        producer_contents_frame_v1(before, after) && stable_after == stable_before && producer_after == producer_before
    } else {
        exists|middle: ProducerReadContentsV1|
            mixed_stable_step_v1(before, middle, consumer, stable, stable_before, stable_after)
            && mixed_pending_step_v1(middle, after, consumer, pending, producer_before, producer_after)
    }
}

pub proof fn mixed_pending_item_frame_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    consumer: WriterKeyV1, request: ProducerReadV1, index: usize, state: ReadScanV1)
    requires producer_outer_frame_v1(before, after), reader_journal_frame_v1(before.stable, after.stable),
    ensures producer_acquire_item_v1(before, consumer, request, index, state)
        == producer_acquire_item_v1(after, consumer, request, index, state),
{
    assert(allocation_decision_v1(before.stable.journal, request.read.allocation)
        == allocation_decision_v1(after.stable.journal, request.read.allocation));
    assert(producer_status_decision_v1(before.stable.journal, request)
        == producer_status_decision_v1(after.stable.journal, request));
    assert(producer_validate_decision_v1(before.stable.journal, request)
        == producer_validate_decision_v1(after.stable.journal, request));
}

pub proof fn mixed_pending_scan_frame_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    consumer: WriterKeyV1, requests: Seq<ProducerReadV1>, index: nat, state: ReadScanV1)
    requires producer_outer_frame_v1(before, after), reader_journal_frame_v1(before.stable, after.stable),
    ensures producer_acquire_scan_v1(before, consumer, requests, index, state)
        == producer_acquire_scan_v1(after, consumer, requests, index, state),
    decreases requests.len() - index,
{
    if index < requests.len() {
        mixed_pending_item_frame_v1(before, after, consumer, requests[index as int], index as usize, state);
        if let Ok(next) = producer_acquire_item_v1(before, consumer, requests[index as int], index as usize, state) {
            mixed_pending_scan_frame_v1(before, after, consumer, requests, index + 1, next);
        }
    }
}

pub proof fn mixed_pending_decision_frame_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    consumer: WriterKeyV1, requests: Seq<ProducerReadV1>, output: Seq<Option<ProducerReadReferenceV1>>)
    requires producer_outer_frame_v1(before, after), reader_journal_frame_v1(before.stable, after.stable),
        requests.len() <= usize::MAX,
        requests.len() <= before.reservations@.len() - producer_live_reads_v1(before),
        requests.len() <= after.reservations@.len() - producer_live_reads_v1(after),
    ensures producer_acquire_decision_v1(before, consumer, requests, output)
        == producer_acquire_decision_v1(after, consumer, requests, output),
{
    let count = requests.len() as usize;
    assert(producer_capacity_decision_v1(before, count) == producer_capacity_decision_v1(after, count));
    assert(producer_acquire_header_v1(before, consumer, count, output)
        == producer_acquire_header_v1(after, consumer, count, output));
    mixed_pending_scan_frame_v1(before, after, consumer, requests, 0, ReadScanV1 { previous: None, group: 0 });
}

pub proof fn mixed_stable_preserves_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    consumer: WriterKeyV1, stable: Seq<AllocationReadV1>, pending_count: nat,
    output_before: Seq<Option<ReadReferenceV1>>, output_after: Seq<Option<ReadReferenceV1>>)
    requires producer_invariant_v1(before), stable.len() <= usize::MAX, stable.len() <= u64::MAX,
        stable.len() + pending_count <= before.reservations@.len() - producer_live_reads_v1(before),
        mixed_stable_step_v1(before, after, consumer, stable, output_before, output_after),
    ensures producer_invariant_v1(after), reader_journal_frame_v1(before.stable, after.stable),
        producer_live_reads_v1(after) == producer_live_reads_v1(before) + stable.len(),
        pending_count <= after.reservations@.len() - producer_live_reads_v1(after),
{
    if stable.len() > 0 {
        stable_acquire_preserves_producer_invariant_v1(before, after, consumer, stable, output_before, output_after, Ok(()));
    }
}

pub proof fn mixed_acquire_preserves_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    consumer: WriterKeyV1, stable: Seq<AllocationReadV1>, pending: Seq<ProducerReadV1>,
    stable_before: Seq<Option<ReadReferenceV1>>, stable_after: Seq<Option<ReadReferenceV1>>,
    producer_before: Seq<Option<ProducerReadReferenceV1>>, producer_after: Seq<Option<ProducerReadReferenceV1>>,
    result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(before), stable.len() <= usize::MAX, stable.len() <= u64::MAX,
        pending.len() <= usize::MAX, pending.len() <= u64::MAX,
        mixed_acquire_relation_v1(before, after, consumer, stable, pending,
            stable_before, stable_after, producer_before, producer_after, result),
    ensures producer_invariant_v1(after),
        producer_live_reads_v1(after) == producer_live_reads_v1(before)
            + if result.is_ok() { (stable.len() + pending.len()) as int } else { 0 },
{
    if result.is_ok() {
        let middle = choose|middle: ProducerReadContentsV1|
            mixed_stable_step_v1(before, middle, consumer, stable, stable_before, stable_after)
            && mixed_pending_step_v1(middle, after, consumer, pending, producer_before, producer_after);
        mixed_stable_preserves_v1(before, middle, consumer, stable, pending.len(), stable_before, stable_after);
        if pending.len() > 0 {
            producer_acquire_preserves_v1(middle, after, consumer, pending, producer_before, producer_after, Ok(()));
        }
    }
}

// This model reuses independently verified logical executors. Production commits only once.
pub fn mixed_acquire_exec_v1(contents: &mut ProducerReadContentsV1, consumer: WriterKeyV1,
    stable: &[AllocationReadV1], stable_output: &mut Vec<Option<ReadReferenceV1>>,
    pending: &[ProducerReadV1], producer_output: &mut Vec<Option<ProducerReadReferenceV1>>,
) -> (result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(*old(contents)), stable@.len() <= u64::MAX, pending@.len() <= u64::MAX,
    ensures mixed_acquire_relation_v1(*old(contents), *final(contents), consumer, stable@, pending@,
        old(stable_output)@, final(stable_output)@, old(producer_output)@, final(producer_output)@, result),
        producer_invariant_v1(*final(contents)),
{
    let ghost before = *contents;
    let ghost stable_before = stable_output@;
    let ghost producer_before = producer_output@;
    if consumer.context_generation != contents.stable.journal.context_generation { return Err(ReadErrorV1::ForeignContext); }
    let submission = match consumer.kind { WriterKindV1::Submission => true, _ => false };
    if !issuable_id_exec_v1(consumer.local) || !submission {
        return Err(ReadErrorV1::InvalidWriterId);
    }
    if stable.len() != stable_output.len() || pending.len() != producer_output.len() { return Err(ReadErrorV1::RosterCapacity); }
    proof { producer_capacity_arithmetic_v1(*contents); }
    let count = match stable.len().checked_add(pending.len()) {
        Some(count) => count, None => return Err(ReadErrorV1::MemberCapacity),
    };
    let stable_live = contents.stable.leases.len() - contents.stable.free_reads.len();
    let producer_live = contents.reservations.len() - contents.free.len();
    if count > contents.reservations.len() - (stable_live + producer_live) { return Err(ReadErrorV1::MemberCapacity); }
    if stable.len() > 0 {
        match acquire_preflight_exec_v1(&contents.stable, consumer, stable, stable_output.as_slice()) {
            Err(error) => return Err(error), Ok(value) => { proof { assert(value =~= ()); } },
        }
    }
    if pending.len() > 0 {
        match producer_acquire_preflight_exec_v1(contents, consumer, pending, producer_output.as_slice()) {
            Err(error) => return Err(error), Ok(value) => { proof { assert(value =~= ()); } },
        }
    }
    if stable.len() > 0 {
        let decision = acquire_verified_v1(&mut contents.stable, consumer, stable, stable_output);
        assert(decision == Ok(()));
    }
    let ghost middle = *contents;
    proof {
        mixed_stable_preserves_v1(before, middle, consumer, stable@, pending@.len(), stable_before, stable_output@);
        mixed_pending_decision_frame_v1(before, middle, consumer, pending@, producer_before);
    }
    if pending.len() > 0 {
        let decision = producer_acquire_exec_v1(contents, consumer, pending, producer_output);
        assert(decision == Ok(()));
    }
    proof {
        assert(mixed_stable_step_v1(before, middle, consumer, stable@, stable_before, stable_output@));
        assert(mixed_pending_step_v1(middle, *contents, consumer, pending@, producer_before, producer_output@));
    }
    Ok(())
}

}
