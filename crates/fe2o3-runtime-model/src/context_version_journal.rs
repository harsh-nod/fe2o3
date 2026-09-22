//! Bounded writer issuance and allocation membership over Context projections.
//!
//! Model values are not runtime authority. The caller models one journal per
//! fresh Context, configured before any local ID is minted. This module neither
//! authenticates that premise nor allocates Context IDs. Production
//! tickets and cross-run reuse remain separate from these model transitions.

use alloc::vec::Vec;
#[cfg(test)]
use core::cell::Cell;

mod allocation_lifecycle;
mod begin;
mod retained;
mod settlement;
mod settlement_commit;
mod settlement_scratch;
mod settlement_storage;
pub use allocation_lifecycle::ContextAllocationEnrollmentV1;
use settlement_storage::SettlementReturnStorageV1;

pub const CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1: usize = 1_048_576;

macro_rules! context_journal_declarations_v1 {
    ($($items:tt)*) => { $($items)* };
}

include!("context_version_journal/declarations.rs");

fn issuable_context_id(value: u64) -> bool {
    value != 0 && value != u64::MAX
}

fn vacant_slots<T>(capacity: usize) -> Result<Vec<Option<T>>, ContextVersionJournalErrorV1> {
    let mut slots = Vec::new();
    slots
        .try_reserve_exact(capacity)
        .map_err(|_| ContextVersionJournalErrorV1::StorageAllocationFailed)?;
    slots.resize_with(capacity, || None);
    Ok(slots)
}

fn free_slots(capacity: usize) -> Result<Vec<usize>, ContextVersionJournalErrorV1> {
    let mut free = Vec::new();
    free.try_reserve_exact(capacity)
        .map_err(|_| ContextVersionJournalErrorV1::StorageAllocationFailed)?;
    free.extend((0..capacity).rev());
    Ok(free)
}

impl ContextVersionJournalV1 {
    #[cfg(test)]
    pub(crate) fn reset_access_count_for_test_v1(&self) {
        self.indexed_accesses.set(0);
    }

    /// Models construction-only opt-in, with a fixed zero initial watermark.
    pub fn new(
        context_generation: u64,
        allocation_capacity: usize,
        writer_capacity: usize,
    ) -> Result<Self, ContextVersionJournalErrorV1> {
        if !issuable_context_id(context_generation) {
            return Err(ContextVersionJournalErrorV1::InvalidContextGeneration);
        }
        if !(1..=CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1).contains(&allocation_capacity)
            || !(1..=CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1).contains(&writer_capacity)
        {
            return Err(ContextVersionJournalErrorV1::InvalidCapacity);
        }
        Ok(Self {
            context_generation,
            allocation_capacity,
            writer_capacity,
            registration_watermark: 0,
            reserved_count: 0,
            writers: vacant_slots(writer_capacity)?,
            free: free_slots(writer_capacity)?,
            allocations: vacant_slots(allocation_capacity)?,
            allocation_free: free_slots(allocation_capacity)?,
            members: vacant_slots(allocation_capacity)?,
            member_free: free_slots(allocation_capacity)?,
            scratch: vacant_slots(allocation_capacity)?,
            #[cfg(test)]
            indexed_accesses: Cell::new(0),
        })
    }

    pub const fn context_generation(&self) -> u64 {
        self.context_generation
    }

    pub const fn allocation_capacity(&self) -> usize {
        self.allocation_capacity
    }

    pub const fn writer_capacity(&self) -> usize {
        self.writer_capacity
    }

    pub const fn registration_watermark(&self) -> u64 {
        self.registration_watermark
    }

    /// The future exclusive Context owner checks this before minting an ID.
    pub fn remaining_writer_slots(&self) -> usize {
        self.free.len()
    }

    pub fn reserved_writer_count(&self) -> usize {
        self.reserved_count
    }

    /// Registers an already-issued key. Gaps are legal; rejected keys do not
    /// advance this watermark or undo the caller's existing ID consumption.
    pub fn register_writer(
        &mut self,
        key: ContextWriterKeyV1,
    ) -> Result<ContextWriterReferenceV1, ContextVersionJournalErrorV1> {
        if key.context_generation != self.context_generation {
            return Err(ContextVersionJournalErrorV1::ForeignContext);
        }
        if !issuable_context_id(key.local) {
            return Err(ContextVersionJournalErrorV1::InvalidWriterId);
        }
        if key.local <= self.registration_watermark {
            return Err(ContextVersionJournalErrorV1::WriterReplay);
        }
        let slot = self
            .next_free()
            .ok_or(ContextVersionJournalErrorV1::WriterCapacity)?;
        if self.read_slot(slot) != Some(&None) {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        let reserved_count = self
            .reserved_count
            .checked_add(1)
            .filter(|count| *count <= self.writer_capacity)
            .ok_or(ContextVersionJournalErrorV1::InvalidState)?;
        // Exclusive preflight fixes the free slot; commit has no fallible work.
        self.pop_free();
        self.store_slot(slot, Some(WriterEntryV1::Reserved(key)));
        self.reserved_count = reserved_count;
        self.registration_watermark = key.local;
        Ok(ContextWriterReferenceV1 { slot, key })
    }

    /// Validates exact Reserved identity, not the latest registration order.
    pub fn lookup_reserved(
        &self,
        reference: ContextWriterReferenceV1,
    ) -> Result<ContextWriterKeyV1, ContextVersionJournalErrorV1> {
        match self.read_slot(reference.slot).copied().flatten() {
            Some(WriterEntryV1::Reserved(key))
                if key == reference.key && key.context_generation == self.context_generation =>
            {
                Ok(key)
            }
            _ => Err(ContextVersionJournalErrorV1::InvalidReference),
        }
    }

    /// Explicitly releases a Reserved slot before effects; never rolls back IDs.
    pub fn abort_reserved(
        &mut self,
        reference: ContextWriterReferenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.lookup_reserved(reference)?;
        if self.free.len() >= self.writer_capacity || self.free.len() >= self.free.capacity() {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        let reserved_count = self
            .reserved_count
            .checked_sub(1)
            .ok_or(ContextVersionJournalErrorV1::InvalidState)?;
        self.store_slot(reference.slot, None);
        self.push_free(reference.slot);
        self.reserved_count = reserved_count;
        Ok(())
    }

    /// Enrolls an existing allocation. No retirement or re-enrollment is implied.
    pub fn enroll_allocation(
        &mut self,
        key: ContextAllocationKeyV1,
        device: ContextJournalDeviceKeyV1,
        byte_extent: u64,
    ) -> Result<ContextAllocationReferenceV1, ContextVersionJournalErrorV1> {
        if key.context_generation != self.context_generation {
            return Err(ContextVersionJournalErrorV1::ForeignContext);
        }
        if !issuable_context_id(key.local) {
            return Err(ContextVersionJournalErrorV1::InvalidAllocationId);
        }
        if device.context_generation != self.context_generation {
            return Err(ContextVersionJournalErrorV1::ForeignContext);
        }
        if !issuable_context_id(device.local) {
            return Err(ContextVersionJournalErrorV1::InvalidDeviceId);
        }
        if byte_extent == 0 {
            return Err(ContextVersionJournalErrorV1::InvalidExtent);
        }
        for index in 0..self.allocations.len() {
            if self
                .read_allocation(index)
                .is_some_and(|entry| entry.key == key)
            {
                return Err(ContextVersionJournalErrorV1::AllocationReplay);
            }
        }
        self.count_indexed_access();
        let slot = self
            .allocation_free
            .last()
            .copied()
            .ok_or(ContextVersionJournalErrorV1::AllocationCapacity)?;
        self.count_indexed_access();
        if self.allocations.get(slot) != Some(&None) {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        self.count_indexed_access();
        let _ = self.allocation_free.pop();
        self.count_indexed_access();
        self.allocations[slot] = Some(AllocationEntryV1 {
            key,
            device,
            byte_extent,
            attempt_epoch: 0,
            content_lineage: 0,
            pending_member: None,
        });
        Ok(ContextAllocationReferenceV1 { slot, key })
    }

    pub fn lookup_allocation(
        &self,
        reference: ContextAllocationReferenceV1,
    ) -> Result<ContextAllocationStateV1, ContextVersionJournalErrorV1> {
        let entry = self.exact_allocation(reference)?;
        let pending_writer = match entry.pending_member {
            None => None,
            Some(slot) => {
                self.count_indexed_access();
                let member = self
                    .members
                    .get(slot)
                    .and_then(Option::as_ref)
                    .filter(|member| member.allocation == reference)
                    .ok_or(ContextVersionJournalErrorV1::InvalidState)?;
                Some(member.writer)
            }
        };
        Ok(ContextAllocationStateV1 {
            device: entry.device,
            byte_extent: entry.byte_extent,
            attempt_epoch: entry.attempt_epoch,
            content_lineage: entry.content_lineage,
            pending_writer,
        })
    }

    pub fn lookup_writer(
        &self,
        reference: ContextWriterReferenceV1,
    ) -> Result<ContextWriterStateV1, ContextVersionJournalErrorV1> {
        let (key, state) = match self.read_slot(reference.slot).copied().flatten() {
            Some(WriterEntryV1::Reserved(key)) => (key, ContextWriterStateV1::Reserved),
            Some(WriterEntryV1::Pending { key, count, .. }) => (
                key,
                ContextWriterStateV1::Pending {
                    member_count: count,
                },
            ),
            Some(WriterEntryV1::Unknown { key, count, .. }) => (
                key,
                ContextWriterStateV1::Unknown {
                    member_count: count,
                },
            ),
            None => return Err(ContextVersionJournalErrorV1::InvalidReference),
        };
        if key != reference.key || key.context_generation != self.context_generation {
            return Err(ContextVersionJournalErrorV1::InvalidReference);
        }
        Ok(state)
    }

    #[cfg(test)]
    fn preflight_begin_write(
        &self,
        writer: ContextWriterReferenceV1,
        canonical: &[ContextAllocationWriteV1],
    ) -> Result<usize, ContextVersionJournalErrorV1> {
        begin::begin_preflight_exec_v1(self, writer, canonical)
    }

    /// The caller supplies an already canonical whole-allocation roster.
    /// Full preflight precedes even scratch mutation; commit keeps the same borrow.
    pub fn begin_write(
        &mut self,
        writer: ContextWriterReferenceV1,
        canonical: &[ContextAllocationWriteV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        begin::begin_exec_v1(self, writer, canonical)
    }

    fn read_allocation(&self, slot: usize) -> Option<&AllocationEntryV1> {
        self.count_indexed_access();
        self.allocations.get(slot).and_then(Option::as_ref)
    }

    fn exact_allocation(
        &self,
        reference: ContextAllocationReferenceV1,
    ) -> Result<&AllocationEntryV1, ContextVersionJournalErrorV1> {
        self.read_allocation(reference.slot)
            .filter(|entry| {
                entry.key == reference.key
                    && entry.key.context_generation == self.context_generation
            })
            .ok_or(ContextVersionJournalErrorV1::InvalidAllocationReference)
    }

    #[cfg(test)]
    fn free_member_slot(&self, offset: usize) -> usize {
        self.count_indexed_access();
        self.member_free[self.member_free.len() - 1 - offset]
    }

    fn store_plan(&mut self, index: usize, plan: BeginMemberPlanV1) {
        self.count_indexed_access();
        self.scratch[index] = Some(plan);
    }

    fn count_indexed_access(&self) {
        #[cfg(test)]
        self.indexed_accesses.set(self.indexed_accesses.get() + 1);
    }

    fn read_slot(&self, slot: usize) -> Option<&Option<WriterEntryV1>> {
        self.count_indexed_access();
        self.writers.get(slot)
    }

    fn store_slot(&mut self, slot: usize, value: Option<WriterEntryV1>) {
        self.count_indexed_access();
        self.writers[slot] = value;
    }

    fn next_free(&self) -> Option<usize> {
        self.count_indexed_access();
        self.free.last().copied()
    }

    fn pop_free(&mut self) {
        self.count_indexed_access();
        let _ = self.free.pop();
    }

    fn push_free(&mut self, slot: usize) {
        self.count_indexed_access();
        self.free.push(slot);
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod declaration_tests;
