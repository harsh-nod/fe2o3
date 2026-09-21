//! Producer-bound input custody. Reservations are not evidence of available data.

use crate::context_read_leases::*;
use crate::context_version_journal::*;
use alloc::vec::Vec;
use core::ops::Deref;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextProducerReadV1 {
    pub read: ContextAllocationReadV1,
    pub producer: ContextWriterReferenceV1,
}

/// Descriptive identity; discarding it does not release consumer custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextProducerReadReferenceV1 {
    pub slot: usize,
    pub incarnation: u64,
    pub consumer: ContextWriterKeyV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextProducerReadStatusV1 {
    Pending,
    Success,
    NoEffect,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReservationV1 {
    reference: ContextProducerReadReferenceV1,
    request: ContextProducerReadV1,
}

/// Owns the stable-reader journal without mutable extraction. The two arenas
/// each have `reads` slots, but share one total limit of `reads` live records.
/// Construction allocates O(allocations + writers + reads) metadata. Producer
/// acquisition/release is O(k), lookup O(1), and settlement needs no arena scan.
/// Existing stable-reader rules remain unchanged. This composition requires
/// separate verification from the stable-reader journal's original artifact.
///
/// ```compile_fail
/// use fe2o3_runtime_model::{ContextProducerReadJournalV1, ContextReadLeasedJournalV1};
/// let mut journal = ContextProducerReadJournalV1::new(1, 4, 4, 4).unwrap();
/// let bypass: &mut ContextReadLeasedJournalV1 = &mut *journal;
/// ```
#[derive(Debug)]
pub struct ContextProducerReadJournalV1 {
    stable: ContextReadLeasedJournalV1,
    reservations: Vec<Option<ReservationV1>>,
    free: Vec<usize>,
    counts: Vec<usize>,
    next_incarnation: u64,
}

impl Deref for ContextProducerReadJournalV1 {
    type Target = ContextReadLeasedJournalV1;

    fn deref(&self) -> &Self::Target {
        &self.stable
    }
}

fn storage<T>(capacity: usize) -> Result<Vec<T>, ContextVersionJournalErrorV1> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(capacity)
        .map_err(|_| ContextVersionJournalErrorV1::StorageAllocationFailed)?;
    Ok(result)
}

fn read_key(request: &ContextProducerReadV1) -> (u64, u64, u64) {
    (
        request.read.allocation.key.local,
        request.read.byte_offset,
        request.read.byte_len,
    )
}

impl ContextProducerReadJournalV1 {
    pub fn new(
        generation: u64,
        allocations: usize,
        writers: usize,
        reads: usize,
    ) -> Result<Self, ContextVersionJournalErrorV1> {
        let stable = ContextReadLeasedJournalV1::new(generation, allocations, writers, reads)?;
        let mut reservations = storage(reads)?;
        reservations.resize(reads, None);
        let mut free = storage(reads)?;
        free.extend((0..reads).rev());
        let mut counts = storage(allocations)?;
        counts.resize(allocations, 0);
        Ok(Self {
            stable,
            reservations,
            free,
            counts,
            next_incarnation: 1,
        })
    }

    pub fn retained_producer_read_count(&self) -> usize {
        self.reservations.len() - self.free.len()
    }

    pub fn retained_read_count(&self) -> usize {
        self.stable.retained_read_count() + self.retained_producer_read_count()
    }

    pub fn remaining_read_slots(&self) -> usize {
        self.reservations.len() - self.retained_read_count()
    }

    pub fn reader_count(
        &self,
        allocation: ContextAllocationReferenceV1,
    ) -> Result<usize, ContextVersionJournalErrorV1> {
        Ok(self.stable.reader_count(allocation)? + self.counts[allocation.slot])
    }

    pub fn validate_read_capacity(&self, count: usize) -> Result<(), ContextVersionJournalErrorV1> {
        if count > self.remaining_read_slots() {
            return Err(ContextVersionJournalErrorV1::MemberCapacity);
        }
        self.stable.validate_read_capacity(count)
    }

    pub fn validate_producer_read_capacity(
        &self,
        count: usize,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        if count > self.remaining_read_slots() {
            return Err(ContextVersionJournalErrorV1::MemberCapacity);
        }
        if self.next_incarnation == 0 || self.next_incarnation.checked_add(count as u64).is_none() {
            return Err(ContextVersionJournalErrorV1::EpochExhausted);
        }
        Ok(())
    }

    fn status(
        &self,
        request: &ContextProducerReadV1,
    ) -> Result<ContextProducerReadStatusV1, ContextVersionJournalErrorV1> {
        use ContextProducerReadStatusV1 as S;
        use ContextVersionJournalErrorV1 as E;
        let read = request.read;
        let state = self.stable.lookup_allocation(read.allocation)?;
        if state.device != read.device {
            return Err(E::AllocationDeviceMismatch);
        }
        if state.byte_extent != read.byte_extent {
            return Err(E::AllocationExtentMismatch);
        }
        if read.byte_len == 0
            || read
                .byte_offset
                .checked_add(read.byte_len)
                .is_none_or(|end| end > read.byte_extent)
        {
            return Err(E::InvalidExtent);
        }
        if request.producer.key.context_generation != self.context_generation()
            || request.producer.key.kind != ContextWriterKindV1::Submission
            || read.content_lineage >= read.attempt_epoch
            || state.attempt_epoch != read.attempt_epoch
        {
            return Err(E::InvalidState);
        }
        match state.pending_writer {
            Some(writer) => {
                if writer != request.producer || state.content_lineage != read.content_lineage {
                    return Err(E::InvalidState);
                }
                match self.stable.lookup_writer(writer)? {
                    ContextWriterStateV1::Pending { .. } => Ok(S::Pending),
                    ContextWriterStateV1::Unknown { .. } => Ok(S::Unknown),
                    ContextWriterStateV1::Reserved => Err(E::InvalidState),
                }
            }
            // The old producer slot may already have been reused elsewhere.
            None if state.content_lineage == read.attempt_epoch => Ok(S::Success),
            None if state.content_lineage == read.content_lineage => Ok(S::NoEffect),
            None => Err(E::InvalidState),
        }
    }

    pub fn validate_producer_read(
        &self,
        request: &ContextProducerReadV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        if self.status(request)? != ContextProducerReadStatusV1::Pending {
            return Err(ContextVersionJournalErrorV1::AllocationBusy);
        }
        Ok(())
    }

    /// Atomic canonical-roster acquisition. The caller authenticates each exact
    /// producer/event relationship and success-gated backend execution separately.
    pub fn acquire_producer_reads(
        &mut self,
        consumer: ContextWriterKeyV1,
        requests: &[ContextProducerReadV1],
        output: &mut [Option<ContextProducerReadReferenceV1>],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        if consumer.context_generation != self.context_generation() {
            return Err(E::ForeignContext);
        }
        if consumer.local == 0
            || consumer.local == u64::MAX
            || consumer.kind != ContextWriterKindV1::Submission
        {
            return Err(E::InvalidWriterId);
        }
        if requests.is_empty() || requests.len() != output.len() {
            return Err(E::RosterCapacity);
        }
        if output.iter().any(Option::is_some) {
            return Err(E::InvalidState);
        }
        self.validate_producer_read_capacity(requests.len())?;
        let mut previous = None;
        let mut group = 0;
        for (index, request) in requests.iter().enumerate() {
            self.validate_producer_read(request)?;
            if request.producer.key.local >= consumer.local {
                return Err(E::InvalidWriterId);
            }
            let key = read_key(request);
            if previous.is_some_and(|prior| prior >= key) {
                return Err(E::NonCanonicalRoster);
            }
            group = if previous.is_some_and(|prior: (u64, u64, u64)| prior.0 == key.0) {
                group + 1
            } else {
                1
            };
            if self.counts[request.read.allocation.slot]
                .checked_add(group)
                .is_none_or(|count| count > self.reservations.len())
            {
                return Err(E::InvalidState);
            }
            let slot = self.free[self.free.len() - index - 1];
            if self.reservations.get(slot) != Some(&None) {
                return Err(E::InvalidState);
            }
            previous = Some(key);
        }
        for (index, request) in requests.iter().enumerate() {
            let slot = self.free.pop().expect("preflighted free reservation");
            let reference = ContextProducerReadReferenceV1 {
                slot,
                incarnation: self.next_incarnation + index as u64,
                consumer,
            };
            self.reservations[slot] = Some(ReservationV1 {
                reference,
                request: *request,
            });
            self.counts[request.read.allocation.slot] += 1;
            output[index] = Some(reference);
        }
        self.next_incarnation += requests.len() as u64;
        Ok(())
    }

    pub fn lookup_producer_read(
        &self,
        reference: ContextProducerReadReferenceV1,
    ) -> Result<ContextProducerReadV1, ContextVersionJournalErrorV1> {
        let entry = self
            .reservations
            .get(reference.slot)
            .copied()
            .flatten()
            .filter(|entry| entry.reference == reference)
            .ok_or(ContextVersionJournalErrorV1::InvalidReference)?;
        self.status(&entry.request)?;
        Ok(entry.request)
    }

    pub fn producer_read_status(
        &self,
        reference: ContextProducerReadReferenceV1,
    ) -> Result<ContextProducerReadStatusV1, ContextVersionJournalErrorV1> {
        self.status(&self.lookup_producer_read(reference)?)
    }

    /// Quiescence releases custody even when the producer is still Pending or
    /// Unknown. It never asserts that the consumer observed successful input.
    pub fn release_producer_reads(
        &mut self,
        consumer: ContextWriterKeyV1,
        references: &[ContextProducerReadReferenceV1],
        evidence: &ContextReadQuiescenceEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        if evidence.consumer != consumer {
            return Err(E::SettlementEvidenceMismatch);
        }
        if references.is_empty() {
            return Err(E::RosterCapacity);
        }
        if self
            .free
            .len()
            .checked_add(references.len())
            .is_none_or(|count| count > self.reservations.len() || count > self.free.capacity())
        {
            return Err(E::InvalidState);
        }
        let mut previous = None;
        let mut group = 0;
        for reference in references {
            if reference.consumer != consumer {
                return Err(E::InvalidReference);
            }
            let request = self.lookup_producer_read(*reference)?;
            let key = (read_key(&request), reference.incarnation);
            if previous.is_some_and(|prior| prior >= key) {
                return Err(E::NonCanonicalRoster);
            }
            group = if previous.is_some_and(|prior: ((u64, u64, u64), u64)| prior.0.0 == key.0.0) {
                group + 1
            } else {
                1
            };
            if self.counts[request.read.allocation.slot] < group {
                return Err(E::InvalidState);
            }
            previous = Some(key);
        }
        for reference in references {
            let entry = self.reservations[reference.slot]
                .take()
                .expect("preflighted reservation");
            self.counts[entry.request.read.allocation.slot] -= 1;
            self.free.push(reference.slot);
        }
        Ok(())
    }

    pub fn acquire_reads(
        &mut self,
        consumer: ContextWriterKeyV1,
        requests: &[ContextAllocationReadV1],
        output: &mut [Option<ContextReadLeaseReferenceV1>],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        // Preserve stable-reader header error precedence before the shared budget.
        use ContextVersionJournalErrorV1 as E;
        if consumer.context_generation != self.context_generation() {
            return Err(E::ForeignContext);
        }
        if consumer.local == 0 || consumer.local == u64::MAX {
            return Err(E::InvalidWriterId);
        }
        if requests.is_empty() || requests.len() != output.len() {
            return Err(E::RosterCapacity);
        }
        if output.iter().any(Option::is_some) {
            return Err(E::InvalidState);
        }
        self.validate_read_capacity(requests.len())?;
        self.stable.acquire_reads(consumer, requests, output)
    }

    pub fn release_reads(
        &mut self,
        consumer: ContextWriterKeyV1,
        references: &[ContextReadLeaseReferenceV1],
        evidence: &ContextReadQuiescenceEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.stable.release_reads(consumer, references, evidence)
    }

    fn require_unread(
        &self,
        references: impl IntoIterator<Item = ContextAllocationReferenceV1>,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        for reference in references {
            if self.reader_count(reference)? != 0 {
                return Err(ContextVersionJournalErrorV1::AllocationBusy);
            }
        }
        Ok(())
    }

    pub fn begin_write(
        &mut self,
        writer: ContextWriterReferenceV1,
        members: &[ContextAllocationWriteV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.require_unread(members.iter().map(|member| member.allocation))?;
        self.stable.begin_write(writer, members)
    }

    pub fn validate_allocation_retirement(
        &self,
        references: &[ContextAllocationReferenceV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.require_unread(references.iter().copied())?;
        self.stable.validate_allocation_retirement(references)
    }

    pub fn retire_allocations(
        &mut self,
        references: &[ContextAllocationReferenceV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.validate_allocation_retirement(references)?;
        self.stable.retire_allocations(references)
    }

    pub fn validate_unknown_disposal(
        &self,
        writer: ContextWriterReferenceV1,
        members: &[ContextAllocationWriteV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.require_unread(members.iter().map(|member| member.allocation))?;
        self.stable.validate_unknown_disposal(writer, members)
    }

    pub fn dispose_unknown(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterDisposalEvidenceV1<'_>,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.validate_unknown_disposal(writer, evidence.allocations)?;
        self.stable.dispose_unknown(writer, evidence)
    }

    pub fn register_writer(
        &mut self,
        key: ContextWriterKeyV1,
    ) -> Result<ContextWriterReferenceV1, ContextVersionJournalErrorV1> {
        self.stable.register_writer(key)
    }

    pub fn abort_reserved(
        &mut self,
        writer: ContextWriterReferenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.stable.abort_reserved(writer)
    }

    pub fn enroll_allocation(
        &mut self,
        key: ContextAllocationKeyV1,
        device: ContextJournalDeviceKeyV1,
        extent: u64,
    ) -> Result<ContextAllocationReferenceV1, ContextVersionJournalErrorV1> {
        self.stable.enroll_allocation(key, device, extent)
    }

    pub fn enroll_allocations(
        &mut self,
        entries: &[ContextAllocationEnrollmentV1],
        output: &mut [Option<ContextAllocationReferenceV1>],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.stable.enroll_allocations(entries, output)
    }

    pub fn settle_success(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterSuccessEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.stable.settle_success(writer, evidence)
    }

    pub fn settle_no_effect(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterNoEffectEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.stable.settle_no_effect(writer, evidence)
    }

    pub fn mark_unknown(
        &mut self,
        writer: ContextWriterReferenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.stable.mark_unknown(writer)
    }
}

#[cfg(test)]
mod tests;
