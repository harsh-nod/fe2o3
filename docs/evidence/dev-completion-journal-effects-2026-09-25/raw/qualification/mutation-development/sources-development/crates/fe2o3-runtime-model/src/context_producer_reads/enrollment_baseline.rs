use super::*;

// Frozen ea7d4e0f2 adapter, redirected through the frozen inner adapters.
impl ContextProducerReadJournalV1 {
    pub(crate) fn baseline_enroll_allocations_v1(
        &mut self,
        entries: &[ContextAllocationEnrollmentV1],
        output: &mut [Option<ContextAllocationReferenceV1>],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.stable.baseline_enroll_allocations_v1(entries, output)
    }
}
