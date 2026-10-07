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

#[allow(unused_macros)]
#[macro_use]
mod retirement_templates {
    include!("retirement_bodies.rs");
}

macro_rules! retirement_rust_expr {
    ($body:expr) => {
        $body
    };
}

macro_rules! enrollment_declarations_v1 {
    ($($declaration:tt)*) => { $($declaration)* };
}
include!("enrollment_declarations.rs");

impl ContextVersionJournalV1 {
    pub fn remaining_allocation_slots(&self) -> usize {
        owner_inspection_length_body!(self, allocation_free)
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
    #[allow(clippy::question_mark)]
    pub fn validate_allocation_retirement(
        &self,
        canonical: &[ContextAllocationReferenceV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        retirement_preflight_body!(
            retirement_rust_expr,
            self,
            canonical,
            retained::shared_retained_allocation_v1,
            retained::shared_retained_allocation_less_v1,
            self.allocation_free.capacity(),
            previous,
            index,
            []
        )
    }

    /// Removes metadata after externally authenticated disposal (or no owner).
    /// This model call is not disposal authority and cannot recover Unknown.
    #[allow(clippy::question_mark)]
    pub fn retire_allocations(
        &mut self,
        canonical: &[ContextAllocationReferenceV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        retirement_execute_body!(
            retirement_rust_expr,
            self,
            canonical,
            validate_allocation_retirement,
            [],
            index,
            [],
            [],
            []
        )
    }
}
