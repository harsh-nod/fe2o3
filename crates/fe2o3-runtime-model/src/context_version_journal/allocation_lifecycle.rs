//! Allocation metadata transitions; callers authenticate admission and disposal.

use super::*;

mod enrollment;
mod ordering;

#[cfg(test)]
#[path = "enrollment_baseline.rs"]
mod enrollment_baseline;

#[allow(unused_macros)]
#[macro_use]
mod enrollment_templates {
    include!("enrollment_wrapper_bodies.rs");
}

macro_rules! enrollment_declarations_v1 {
    ($($declaration:tt)*) => { $($declaration)* };
}
include!("enrollment_declarations.rs");

impl ContextVersionJournalV1 {
    pub fn remaining_allocation_slots(&self) -> usize {
        self.allocation_free.len()
    }

    /// Enrolls a canonical batch atomically, using initially empty caller storage.
    /// Both journal and output are unchanged on rejection. Enrollment costs
    /// O(A log(k + 1) + k log(k + 1)); it performs no allocation or callback.
    /// Canonical ordering applies within this batch, not across enrollments.
    ///
    /// Callers must supply fresh identities after retirement: this bounded model
    /// does not retain a history of disposed allocation keys.
    pub fn enroll_allocations(
        &mut self,
        canonical: &[ContextAllocationEnrollmentV1],
        output: &mut [Option<ContextAllocationReferenceV1>],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        journal_enrollment_wrapper_body!(
            self,
            canonical,
            output,
            enrollment::enrollment_journal_exec_v1
        )
    }

    /// Preflights the entire exact roster in O(k), without mutating any state.
    /// Pending and Unknown writer membership both prevent retirement.
    /// Partition preservation assumes a valid prestate; unrelated arena
    /// corruption is not comprehensively detected by these local checks.
    pub fn validate_allocation_retirement(
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

    /// Removes metadata after externally authenticated disposal (or no owner).
    /// This model call is not disposal authority and cannot recover Unknown.
    pub fn retire_allocations(
        &mut self,
        canonical: &[ContextAllocationReferenceV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.validate_allocation_retirement(canonical)?;
        for reference in canonical {
            self.allocations[reference.slot] = None;
            self.allocation_free.push(reference.slot);
        }
        Ok(())
    }
}
