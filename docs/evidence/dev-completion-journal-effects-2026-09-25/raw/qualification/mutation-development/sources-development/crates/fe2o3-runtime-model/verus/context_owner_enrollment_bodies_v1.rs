verus! {

spec fn stable_enrollment_relation_v1(before: ContextReadLeasedJournalV1, after: ContextReadLeasedJournalV1,
    entries: Seq<EnrollmentV1>, original: Seq<Option<AllocationReferenceV1>>,
    output: Seq<Option<AllocationReferenceV1>>, result: Result<(), EnrollmentErrorV1>) -> bool {
    &&& enrollment_execution_relation_v1(before.journal, after.journal, entries, original, output, result)
    &&& after.leases == before.leases
    &&& after.free_reads == before.free_reads
    &&& after.readers == before.readers
    &&& after.next_incarnation == before.next_incarnation
}

spec fn producer_enrollment_relation_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    entries: Seq<EnrollmentV1>, original: Seq<Option<AllocationReferenceV1>>,
    output: Seq<Option<AllocationReferenceV1>>, result: Result<(), EnrollmentErrorV1>) -> bool {
    &&& stable_enrollment_relation_v1(before.stable, after.stable, entries, original, output, result)
    &&& after.reservations == before.reservations
    &&& after.free == before.free
    &&& after.counts == before.counts
    &&& after.next_incarnation == before.next_incarnation
}

impl ContextVersionJournalV1 {
    fn enroll_allocations(&mut self, canonical: &[EnrollmentV1], output: &mut [Option<AllocationReferenceV1>])
        -> (result: Result<(), EnrollmentErrorV1>)
        ensures enrollment_execution_relation_v1(*old(self), *final(self), canonical@, old(output)@, final(output)@, result),
    {
        journal_enrollment_wrapper_body!(self, canonical, output, enrollment_journal_exec_v1)
    }
}

impl ContextReadLeasedJournalV1 {
    fn enroll_allocations(&mut self, entries: &[EnrollmentV1], output: &mut [Option<AllocationReferenceV1>])
        -> (result: Result<(), EnrollmentErrorV1>)
        ensures stable_enrollment_relation_v1(*old(self), *final(self), entries@, old(output)@, final(output)@, result),
    {
        stable_enrollment_wrapper_body!(self, entries, output)
    }
}

impl ContextProducerReadJournalV1 {
    fn enroll_allocations(&mut self, entries: &[EnrollmentV1], output: &mut [Option<AllocationReferenceV1>])
        -> (result: Result<(), EnrollmentErrorV1>)
        ensures producer_enrollment_relation_v1(*old(self), *final(self), entries@, old(output)@, final(output)@, result),
    {
        producer_enrollment_wrapper_body!(self, entries, output)
    }
}

}
