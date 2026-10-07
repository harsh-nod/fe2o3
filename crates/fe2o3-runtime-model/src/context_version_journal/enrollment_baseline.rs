use super::*;

// Frozen ea7d4e0f2 journal adapter; the enrollment leaf is unchanged.
impl ContextVersionJournalV1 {
    pub(crate) fn baseline_enroll_allocations_v1(
        &mut self,
        canonical: &[ContextAllocationEnrollmentV1],
        output: &mut [Option<ContextAllocationReferenceV1>],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        enrollment::enrollment_journal_exec_v1(self, canonical, output)
    }
}
