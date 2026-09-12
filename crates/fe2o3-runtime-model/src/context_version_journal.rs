//! Bounded Reserved-writer issuance over existing Context identity projections.
//!
//! Model values are not runtime authority. The caller models one journal per
//! fresh Context, configured before any local ID is minted. This module neither
//! authenticates that premise nor allocates Context IDs. Membership, Begin,
//! settlement, production tickets and cross-run reuse are separate transitions.

use alloc::vec::Vec;
#[cfg(test)]
use core::cell::Cell;

pub const CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1: usize = 1_048_576;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextWriterKindV1 {
    Synchronous,
    Submission,
}

/// Inert projection of an existing identity, not an identity-issuance API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextWriterKeyV1 {
    pub context_generation: u64,
    pub local: u64,
    pub kind: ContextWriterKindV1,
}

/// A descriptive slot reference; discarding it does not abort its reservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use = "discarding a reference does not release its retained writer slot"]
pub struct ContextWriterReferenceV1 {
    pub slot: usize,
    pub key: ContextWriterKeyV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextVersionJournalErrorV1 {
    InvalidContextGeneration,
    InvalidCapacity,
    StorageAllocationFailed,
    ForeignContext,
    InvalidWriterId,
    WriterReplay,
    WriterCapacity,
    InvalidReference,
    InvalidState,
}

/// Construction is O(W); registration, lookup and pre-effect abort are O(1).
/// A is retained configuration only until allocation membership is implemented.
#[derive(Debug)]
pub struct ContextVersionJournalV1 {
    context_generation: u64,
    allocation_capacity: usize,
    writer_capacity: usize,
    registration_watermark: u64,
    reserved: Vec<Option<ContextWriterKeyV1>>,
    free: Vec<usize>,
    #[cfg(test)]
    indexed_accesses: Cell<usize>,
}

fn issuable_context_id(value: u64) -> bool {
    value != 0 && value != u64::MAX
}

impl ContextVersionJournalV1 {
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
        let mut reserved = Vec::new();
        reserved
            .try_reserve_exact(writer_capacity)
            .map_err(|_| ContextVersionJournalErrorV1::StorageAllocationFailed)?;
        let mut free = Vec::new();
        free.try_reserve_exact(writer_capacity)
            .map_err(|_| ContextVersionJournalErrorV1::StorageAllocationFailed)?;
        reserved.resize(writer_capacity, None);
        free.extend((0..writer_capacity).rev());
        Ok(Self {
            context_generation,
            allocation_capacity,
            writer_capacity,
            registration_watermark: 0,
            reserved,
            free,
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
        self.writer_capacity - self.free.len()
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
        // Exclusive preflight fixes the free slot; commit has no fallible work.
        self.pop_free();
        self.store_slot(slot, Some(key));
        self.registration_watermark = key.local;
        Ok(ContextWriterReferenceV1 { slot, key })
    }

    /// Validates exact Reserved identity, not the latest registration order.
    pub fn lookup_reserved(
        &self,
        reference: ContextWriterReferenceV1,
    ) -> Result<ContextWriterKeyV1, ContextVersionJournalErrorV1> {
        match self.read_slot(reference.slot).copied().flatten() {
            Some(key)
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
        self.store_slot(reference.slot, None);
        self.push_free(reference.slot);
        Ok(())
    }

    fn count_indexed_access(&self) {
        #[cfg(test)]
        self.indexed_accesses.set(self.indexed_accesses.get() + 1);
    }

    fn read_slot(&self, slot: usize) -> Option<&Option<ContextWriterKeyV1>> {
        self.count_indexed_access();
        self.reserved.get(slot)
    }

    fn store_slot(&mut self, slot: usize, value: Option<ContextWriterKeyV1>) {
        self.count_indexed_access();
        self.reserved[slot] = value;
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
