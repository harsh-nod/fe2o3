// Frozen at 3d686c5ca4bbf8a3a58cb9de357f00aab2db7116; only names and visibility are redirected.
use super::*;

impl ContextReadLeasedJournalV1 {
    pub(crate) fn baseline_validate_allocation_retirement_v1(
        &self,
        references: &[ContextAllocationReferenceV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.require_unread(references.iter().copied())?;
        self.journal
            .baseline_validate_allocation_retirement_v1(references)
    }

    pub(crate) fn baseline_retire_allocations_v1(
        &mut self,
        references: &[ContextAllocationReferenceV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.baseline_validate_allocation_retirement_v1(references)?;
        self.journal.baseline_retire_allocations_v1(references)
    }
}
