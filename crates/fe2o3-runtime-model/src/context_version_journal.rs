//! Bounded writer issuance and allocation membership over Context projections.
//!
//! Model values are not runtime authority. The caller models one journal per
//! fresh Context, configured before any local ID is minted. This module neither
//! authenticates that premise nor allocates Context IDs. Settlement, production
//! tickets and cross-run reuse remain separate transitions.

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

/// An inert projection of the complete existing allocation identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ContextAllocationKeyV1 {
    pub context_generation: u64,
    pub local: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextJournalDeviceKeyV1 {
    pub context_generation: u64,
    pub local: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextAllocationReferenceV1 {
    pub slot: usize,
    pub key: ContextAllocationKeyV1,
}

/// One canonical whole-allocation destination, not an aliased input view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextAllocationWriteV1 {
    pub allocation: ContextAllocationReferenceV1,
    pub device: ContextJournalDeviceKeyV1,
    pub byte_extent: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextWriterStateV1 {
    Reserved,
    Pending { member_count: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextAllocationStateV1 {
    pub device: ContextJournalDeviceKeyV1,
    pub byte_extent: u64,
    pub attempt_epoch: u64,
    pub content_lineage: u64,
    pub pending_writer: Option<ContextWriterReferenceV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WriterEntryV1 {
    Reserved(ContextWriterKeyV1),
    Pending {
        key: ContextWriterKeyV1,
        head: Option<usize>,
        count: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AllocationEntryV1 {
    key: ContextAllocationKeyV1,
    device: ContextJournalDeviceKeyV1,
    byte_extent: u64,
    attempt_epoch: u64,
    content_lineage: u64,
    pending_member: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MemberEntryV1 {
    writer: ContextWriterReferenceV1,
    allocation: ContextAllocationReferenceV1,
    prior_lineage: u64,
    attempt_epoch: u64,
    next: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BeginMemberPlanV1 {
    member_slot: usize,
    allocation: ContextAllocationReferenceV1,
    prior_lineage: u64,
    attempt_epoch: u64,
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
    InvalidAllocationId,
    InvalidDeviceId,
    InvalidExtent,
    AllocationReplay,
    AllocationCapacity,
    InvalidAllocationReference,
    AllocationDeviceMismatch,
    AllocationExtentMismatch,
    AllocationBusy,
    RosterCapacity,
    NonCanonicalRoster,
    MemberCapacity,
    EpochExhausted,
}

/// Construction is O(A + W), enrollment O(A), and canonical Begin O(k).
/// Writer issuance, exact lookup and pre-effect abort remain O(1).
#[derive(Debug)]
pub struct ContextVersionJournalV1 {
    context_generation: u64,
    allocation_capacity: usize,
    writer_capacity: usize,
    registration_watermark: u64,
    reserved_count: usize,
    writers: Vec<Option<WriterEntryV1>>,
    free: Vec<usize>,
    allocations: Vec<Option<AllocationEntryV1>>,
    allocation_free: Vec<usize>,
    members: Vec<Option<MemberEntryV1>>,
    member_free: Vec<usize>,
    scratch: Vec<Option<BeginMemberPlanV1>>,
    #[cfg(test)]
    indexed_accesses: Cell<usize>,
}

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
            None => return Err(ContextVersionJournalErrorV1::InvalidReference),
        };
        if key != reference.key || key.context_generation != self.context_generation {
            return Err(ContextVersionJournalErrorV1::InvalidReference);
        }
        Ok(state)
    }

    /// The caller supplies an already canonical whole-allocation roster.
    /// Full preflight precedes even scratch mutation; commit keeps the same borrow.
    pub fn begin_write(
        &mut self,
        writer: ContextWriterReferenceV1,
        canonical: &[ContextAllocationWriteV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.lookup_reserved(writer)?;
        let reserved_count = self
            .reserved_count
            .checked_sub(1)
            .ok_or(ContextVersionJournalErrorV1::InvalidState)?;
        let count = canonical.len();
        if count > self.allocation_capacity {
            return Err(ContextVersionJournalErrorV1::RosterCapacity);
        }
        for pair in canonical.windows(2) {
            if pair[0].allocation.key >= pair[1].allocation.key {
                return Err(ContextVersionJournalErrorV1::NonCanonicalRoster);
            }
        }
        for destination in canonical {
            let entry = self.exact_allocation(destination.allocation)?;
            if entry.device != destination.device {
                return Err(ContextVersionJournalErrorV1::AllocationDeviceMismatch);
            }
            if entry.byte_extent != destination.byte_extent {
                return Err(ContextVersionJournalErrorV1::AllocationExtentMismatch);
            }
            if entry.pending_member.is_some() {
                return Err(ContextVersionJournalErrorV1::AllocationBusy);
            }
            entry
                .attempt_epoch
                .checked_add(1)
                .ok_or(ContextVersionJournalErrorV1::EpochExhausted)?;
        }
        if count > self.member_free.len() {
            return Err(ContextVersionJournalErrorV1::MemberCapacity);
        }
        if count > self.scratch.len() {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        // Free-slot uniqueness is a preserved global invariant, not an arena scan.
        for index in 0..count {
            let member = self.free_member_slot(index);
            self.count_indexed_access();
            if self.members.get(member) != Some(&None) {
                return Err(ContextVersionJournalErrorV1::InvalidState);
            }
            self.count_indexed_access();
            if self.scratch[index].is_some() {
                return Err(ContextVersionJournalErrorV1::InvalidState);
            }
        }
        for (index, destination) in canonical.iter().enumerate() {
            let entry = self
                .read_allocation(destination.allocation.slot)
                .expect("whole-roster preflight retains exact allocations");
            let plan = BeginMemberPlanV1 {
                member_slot: self.free_member_slot(index),
                allocation: destination.allocation,
                prior_lineage: entry.content_lineage,
                attempt_epoch: entry.attempt_epoch + 1,
            };
            self.store_plan(index, plan);
        }
        let head = if count == 0 {
            None
        } else {
            self.count_indexed_access();
            Some(self.scratch[0].expect("complete member plan").member_slot)
        };
        for index in 0..count {
            self.count_indexed_access();
            let plan = self.scratch[index].take().expect("complete member plan");
            let next = if index + 1 == count {
                None
            } else {
                self.count_indexed_access();
                Some(
                    self.scratch[index + 1]
                        .expect("complete next member plan")
                        .member_slot,
                )
            };
            self.count_indexed_access();
            let _ = self.member_free.pop();
            self.count_indexed_access();
            self.members[plan.member_slot] = Some(MemberEntryV1 {
                writer,
                allocation: plan.allocation,
                prior_lineage: plan.prior_lineage,
                attempt_epoch: plan.attempt_epoch,
                next,
            });
            self.count_indexed_access();
            let entry = self.allocations[plan.allocation.slot]
                .as_mut()
                .expect("retained exact allocation");
            entry.attempt_epoch = plan.attempt_epoch;
            entry.pending_member = Some(plan.member_slot);
        }
        self.store_slot(
            writer.slot,
            Some(WriterEntryV1::Pending {
                key: writer.key,
                head,
                count,
            }),
        );
        self.reserved_count = reserved_count;
        Ok(())
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
