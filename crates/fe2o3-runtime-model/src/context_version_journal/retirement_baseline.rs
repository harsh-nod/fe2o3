// Frozen at 3d686c5ca4bbf8a3a58cb9de357f00aab2db7116; only names and visibility are redirected.
use super::*;

impl ContextVersionJournalV1 {
    pub(crate) fn baseline_validate_allocation_retirement_v1(
        &self,
        canonical: &[ContextAllocationReferenceV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        if canonical.len() > self.allocation_capacity {
            return Err(E::RosterCapacity);
        }
        let mut previous = None;
        for &reference in canonical {
            let entry = self.exact_allocation(reference)?;
            if previous.is_some_and(|key| key >= reference.key) {
                return Err(E::NonCanonicalRoster);
            }
            if entry.pending_member.is_some() {
                return Err(E::AllocationBusy);
            }
            previous = Some(reference.key);
        }
        let returned = self
            .allocation_free
            .len()
            .checked_add(canonical.len())
            .ok_or(E::InvalidState)?;
        if returned > self.allocation_capacity || returned > self.allocation_free.capacity() {
            return Err(E::InvalidState);
        }
        Ok(())
    }

    pub(crate) fn baseline_retire_allocations_v1(
        &mut self,
        canonical: &[ContextAllocationReferenceV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.baseline_validate_allocation_retirement_v1(canonical)?;
        for reference in canonical {
            self.allocations[reference.slot] = None;
            self.allocation_free.push(reference.slot);
        }
        Ok(())
    }
}
