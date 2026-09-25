verus! {

spec fn producer_stable_prefix_v1(contents: ContextProducerReadJournalV1, consumer: WriterKeyV1,
    count: usize, output: Seq<Option<ContextReadLeaseReferenceV1>>) -> Result<(), ReadErrorV1> {
    if consumer.context_generation != contents.stable.journal.context_generation { Err(ReadErrorV1::ForeignContext) }
    else if consumer.local == 0 || consumer.local == u64::MAX { Err(ReadErrorV1::InvalidWriterId) }
    else if count == 0 || count != output.len() { Err(ReadErrorV1::RosterCapacity) }
    else if exists|i: int| 0 <= i < output.len() && output[i].is_some() { Err(ReadErrorV1::InvalidState) }
    else { Ok(()) }
}

spec fn producer_stable_header_v1(contents: ContextProducerReadJournalV1, consumer: WriterKeyV1,
    count: usize, output: Seq<Option<ContextReadLeaseReferenceV1>>) -> Result<(), ReadErrorV1> {
    match producer_stable_prefix_v1(contents, consumer, count, output) {
        Err(error) => Err(error), Ok(()) => producer_stable_capacity_decision_v1(contents, count),
    }
}

spec fn producer_stable_acquire_domain_v1(contents: ContextProducerReadJournalV1, consumer: WriterKeyV1,
    count: usize, output: Seq<Option<ContextReadLeaseReferenceV1>>) -> bool {
    &&& producer_stable_prefix_v1(contents, consumer, count, output).is_ok() ==> producer_budget_domain_v1(contents)
    &&& producer_stable_header_v1(contents, consumer, count, output).is_ok() ==> stable_guard_storage_v1(contents.stable)
}

spec fn producer_reservations_frame_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1) -> bool {
    &&& after.reservations == before.reservations
    &&& after.free == before.free
    &&& after.counts == before.counts
    &&& after.next_incarnation == before.next_incarnation
}

spec fn producer_stable_acquire_decision_v1(contents: ContextProducerReadJournalV1, consumer: WriterKeyV1,
    requests: Seq<ContextAllocationReadV1>, output: Seq<Option<ContextReadLeaseReferenceV1>>) -> Result<(), ReadErrorV1> {
    match producer_stable_header_v1(contents, consumer, requests.len() as usize, output) {
        Err(error) => Err(error), Ok(()) => stable_acquire_decision_v1(contents.stable, consumer, requests, output),
    }
}

spec fn producer_stable_acquire_relation_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    consumer: WriterKeyV1, requests: Seq<ContextAllocationReadV1>, output_before: Seq<Option<ContextReadLeaseReferenceV1>>,
    output_after: Seq<Option<ContextReadLeaseReferenceV1>>, result: Result<(), ReadErrorV1>) -> bool {
    &&& producer_reservations_frame_v1(before, after)
    &&& result == producer_stable_acquire_decision_v1(before, consumer, requests, output_before)
    &&& match producer_stable_header_v1(before, consumer, requests.len() as usize, output_before) {
        Err(_) => after == before && output_after == output_before,
        Ok(()) => stable_acquire_execution_relation_v1(before.stable, after.stable, consumer, requests, output_before, output_after, result),
    }
}

spec fn producer_stable_release_relation_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    consumer: WriterKeyV1, references: Seq<ContextReadLeaseReferenceV1>, evidence: WriterKeyV1,
    observed_free_capacity: usize, result: Result<(), ReadErrorV1>) -> bool {
    &&& producer_reservations_frame_v1(before, after)
    &&& stable_release_execution_relation_v1(before.stable, after.stable, consumer, references, evidence, observed_free_capacity, result)
}

}
