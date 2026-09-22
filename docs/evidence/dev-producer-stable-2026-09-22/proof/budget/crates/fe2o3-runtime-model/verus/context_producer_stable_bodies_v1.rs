verus! {

fn producer_stable_output_vacant_exec_v1(output: &[Option<ContextReadLeaseReferenceV1>]) -> (result: bool)
    ensures result == (forall|i: int| 0 <= i < output@.len() ==> output@[i].is_none()),
{
    stable_read_output_vacant_body!(verus_exec_expr, output, index, [
        invariant index <= output.len(), forall|i: int| 0 <= i < index ==> output@[i].is_none(),
        decreases output.len() - index,
    ])
}

fn producer_stable_acquire_header_exec_v1(contents: &ContextProducerReadJournalV1, consumer: WriterKeyV1,
    count: usize, output: &[Option<ContextReadLeaseReferenceV1>]) -> (result: Result<(), ReadErrorV1>)
    requires count <= u64::MAX,
        producer_stable_prefix_v1(*contents, consumer, count, output@).is_ok() ==> producer_budget_domain_v1(*contents),
    ensures result == producer_stable_header_v1(*contents, consumer, count, output@),
{
    producer_stable_acquire_header_body!(contents, contents.stable.journal.context_generation(), consumer, count, output)
}

impl ContextProducerReadJournalV1 {
    fn acquire_reads(&mut self, consumer: WriterKeyV1, requests: &[ContextAllocationReadV1],
        output: &mut [Option<ContextReadLeaseReferenceV1>]) -> (result: Result<(), ReadErrorV1>)
        requires producer_stable_acquire_domain_v1(*old(self), consumer, requests.len(), old(output)@),
            requests@.len() <= u64::MAX,
        ensures producer_stable_acquire_relation_v1(*old(self), *final(self), consumer, requests@, old(output)@, final(output)@, result),
    {
        producer_stable_acquire_body!(verus_exec_expr, self, consumer, requests, output,
            producer_stable_acquire_header_exec_v1, value, result, [],
            [proof { assert(value =~= ()); }], [])
    }

    // Capacity remains an observation passed to the underlying stable proof adapter.
    fn release_reads_observed_v1(&mut self, consumer: WriterKeyV1, references: &[ContextReadLeaseReferenceV1],
        evidence: &ContextReadQuiescenceEvidenceV1, observed_free_capacity: usize) -> (result: Result<(), ReadErrorV1>)
        requires stable_guard_storage_v1(old(self).stable),
        ensures producer_stable_release_relation_v1(*old(self), *final(self), consumer, references@,
            evidence.consumer, observed_free_capacity, result),
    {
        producer_stable_release_body!(self, consumer, references, evidence, release_reads_observed_v1, [, observed_free_capacity])
    }
}

}
