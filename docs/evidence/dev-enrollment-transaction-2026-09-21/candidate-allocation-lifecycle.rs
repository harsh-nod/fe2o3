//! Allocation metadata transitions; callers authenticate admission and disposal.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextAllocationEnrollmentV1 {
    pub key: ContextAllocationKeyV1,
    pub device: ContextJournalDeviceKeyV1,
    pub byte_extent: u64,
}

impl ContextVersionJournalV1 {
    pub fn remaining_allocation_slots(&self) -> usize {
        self.allocation_free.len()
    }

    /// Enrolls a canonical batch atomically, using initially empty caller storage.
    /// Both journal and output are unchanged on rejection. Enrollment costs
    /// O(A log(k + 1) + k); it performs no allocation or callback.
    /// Canonical ordering applies within this batch, not across enrollments.
    ///
    /// Callers must supply fresh identities after retirement: this bounded model
    /// does not retain a history of disposed allocation keys.
    pub fn enroll_allocations(
        &mut self,
        canonical: &[ContextAllocationEnrollmentV1],
        output: &mut [Option<ContextAllocationReferenceV1>],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        if canonical.len() > self.allocation_capacity || output.len() != canonical.len() {
            return Err(E::RosterCapacity);
        }
        if output.iter().any(Option::is_some) {
            return Err(E::InvalidState);
        }
        let mut previous = None;
        for entry in canonical {
            if entry.key.context_generation != self.context_generation
                || entry.device.context_generation != self.context_generation
            {
                return Err(E::ForeignContext);
            }
            if !issuable_context_id(entry.key.local) {
                return Err(E::InvalidAllocationId);
            }
            if !issuable_context_id(entry.device.local) {
                return Err(E::InvalidDeviceId);
            }
            if entry.byte_extent == 0 {
                return Err(E::InvalidExtent);
            }
            if previous.is_some_and(|key| key >= entry.key) {
                return Err(E::NonCanonicalRoster);
            }
            previous = Some(entry.key);
        }
        if canonical.is_empty() {
            return Ok(());
        }
        if self.allocation_free.len() > self.allocation_capacity {
            return Err(E::InvalidState);
        }
        for index in 0..self.allocations.len() {
            if self.read_allocation(index).is_some_and(|entry| {
                canonical
                    .binary_search_by_key(&entry.key, |new| new.key)
                    .is_ok()
            }) {
                return Err(E::AllocationReplay);
            }
        }
        let remaining = self
            .allocation_free
            .len()
            .checked_sub(canonical.len())
            .ok_or(E::AllocationCapacity)?;
        for &slot in &self.allocation_free[remaining..] {
            if self.allocations.get(slot) != Some(&None) {
                return Err(E::InvalidState);
            }
        }
        // The selected slots are all initially vacant. Keep their coordinates
        // fixed until this inert metadata transaction commits or rolls back.
        for ((entry, out), &slot) in canonical
            .iter()
            .zip(output.iter_mut())
            .zip(self.allocation_free[remaining..].iter().rev())
        {
            *out = Some(ContextAllocationReferenceV1 {
                slot,
                key: entry.key,
            });
        }
        for (installed, (entry, reference)) in canonical.iter().zip(output.iter()).enumerate() {
            let slot = reference.unwrap().slot;
            if self.allocations[slot].is_some() {
                self.rollback_enrollment(output, installed);
                return Err(E::InvalidState);
            }
            self.allocations[slot] = Some(AllocationEntryV1 {
                key: entry.key,
                device: entry.device,
                byte_extent: entry.byte_extent,
                attempt_epoch: 0,
                content_lineage: 0,
                pending_member: None,
            });
        }
        // Replay preflight excluded every old occupied canonical key. A retained
        // slot containing one now is therefore exactly a selected-slot overlap.
        if self.allocation_free[..remaining].iter().any(|&slot| {
            self.allocations.get(slot).is_some_and(|value| {
                value.is_some_and(|entry| {
                    canonical
                        .binary_search_by_key(&entry.key, |new| new.key)
                        .is_ok()
                })
            })
        }) {
            self.rollback_enrollment(output, canonical.len());
            return Err(E::InvalidState);
        }
        self.allocation_free.truncate(remaining);
        Ok(())
    }

    fn rollback_enrollment(
        &mut self,
        output: &mut [Option<ContextAllocationReferenceV1>],
        installed: usize,
    ) {
        // Only this call installed these entries, into prevalidated vacant slots.
        for reference in output[..installed].iter().rev() {
            self.allocations[reference.unwrap().slot] = None;
        }
        output.fill(None);
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
