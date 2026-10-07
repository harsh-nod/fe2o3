//! Explicit owner surface: no inherited availability or mutation bypass.

use super::*;

impl ContextQueuedWriterJournalV1 {
    pub fn context_generation(&self) -> u64 {
        self.inner.context_generation()
    }
    pub fn allocation_capacity(&self) -> usize {
        self.inner.allocation_capacity()
    }
    pub fn writer_capacity(&self) -> usize {
        self.inner.writer_capacity()
    }
    pub fn registration_watermark(&self) -> u64 {
        self.inner.registration_watermark()
    }
    pub fn remaining_allocation_slots(&self) -> usize {
        self.inner.remaining_allocation_slots()
    }
    pub fn remaining_writer_slots(&self) -> usize {
        self.inner.remaining_writer_slots()
    }
    pub fn reserved_writer_count(&self) -> usize {
        self.inner.reserved_writer_count()
    }
    pub fn remaining_read_slots(&self) -> usize {
        self.inner.remaining_read_slots() - self.retained_queued_read_count()
    }
    pub fn retained_read_count(&self) -> usize {
        self.inner.retained_read_count() + self.retained_queued_read_count()
    }
    /// Retained active-producer and queued-producer leases combined.
    pub fn retained_producer_read_count(&self) -> usize {
        self.inner.retained_producer_read_count() + self.retained_queued_read_count()
    }
    pub fn member_capacity(&self) -> usize {
        self.members.len()
    }
    pub fn remaining_member_slots(&self) -> usize {
        self.free.len()
    }

    /// Exact inner version metadata, not an availability receipt. Queued writers
    /// remain Reserved there; use validate_no_queued_writer before outside access.
    /// Diagnostic lookups/counts remain available after terminal disposal failure;
    /// they cannot authorize reuse or further owner mutation.
    pub fn lookup_allocation(
        &self,
        allocation: ContextAllocationReferenceV1,
    ) -> Result<ContextAllocationStateV1, Error> {
        queued_allocation_lookup_body_v1!(self, allocation)
    }
    pub fn lookup_writer(
        &self,
        writer: ContextWriterReferenceV1,
    ) -> Result<ContextWriterStateV1, Error> {
        self.inner.lookup_writer(writer)
    }
    pub fn lookup_reserved(
        &self,
        writer: ContextWriterReferenceV1,
    ) -> Result<ContextWriterKeyV1, Error> {
        self.inner.lookup_reserved(writer)
    }
    pub fn reader_count(&self, allocation: ContextAllocationReferenceV1) -> Result<usize, Error> {
        self.inner
            .reader_count(allocation)?
            .checked_add(self.read_counts[allocation.slot])
            .ok_or(Error::InvalidState)
    }

    pub fn validate_no_queued_writer(
        &self,
        allocation: ContextAllocationReferenceV1,
    ) -> Result<(), Error> {
        self.require_unqueued(allocation)
    }

    /// Descriptive tail identity, not dependency authentication.
    pub fn latest_writer(
        &self,
        allocation: ContextAllocationReferenceV1,
    ) -> Result<Option<ContextWriterReferenceV1>, Error> {
        self.ensure_usable()?;
        let state = self.inner.lookup_allocation(allocation)?;
        match self.tails[allocation.slot] {
            Some(index) => {
                let member = self.member(index)?;
                if member.request.destination.allocation != allocation {
                    return Err(Error::InvalidAllocationReference);
                }
                self.validate_links(index, member)?;
                self.root(member.writer)?;
                Ok(Some(member.writer))
            }
            None if self.heads[allocation.slot].is_some()
                || self.queued_counts[allocation.slot] != 0
                || state.pending_writer.is_some() =>
            {
                Err(Error::InvalidState)
            }
            None => Ok(None),
        }
    }

    pub fn validate_read_capacity(&self, count: usize) -> Result<(), Error> {
        self.ensure_usable()?;
        self.inner.validate_read_capacity(count)?;
        self.validate_total_read_capacity(count)
    }
    pub fn validate_producer_read_capacity(&self, count: usize) -> Result<(), Error> {
        self.ensure_usable()?;
        self.inner.validate_producer_read_capacity(count)?;
        self.validate_total_read_capacity(count)
    }
    pub fn validate_read(&self, read: &ContextAllocationReadV1) -> Result<(), Error> {
        self.ensure_usable()?;
        self.inner.validate_read(read)?;
        self.require_unqueued(read.allocation)
    }
    pub fn validate_producer_read(&self, read: &ContextProducerReadV1) -> Result<(), Error> {
        self.ensure_usable()?;
        self.inner.validate_producer_read(read)?;
        self.require_unqueued(read.read.allocation)
    }

    pub fn acquire_reads(
        &mut self,
        consumer: ContextWriterKeyV1,
        requests: &[ContextAllocationReadV1],
        output: &mut [Option<ContextReadLeaseReferenceV1>],
    ) -> Result<(), Error> {
        self.ensure_usable()?;
        self.inner
            .preflight_acquire_reads_v1(consumer, requests, output)?;
        self.validate_total_read_capacity(requests.len())?;
        for read in requests {
            self.require_unqueued(read.allocation)?;
        }
        self.inner.acquire_reads(consumer, requests, output)
    }
    pub fn acquire_producer_reads(
        &mut self,
        consumer: ContextWriterKeyV1,
        requests: &[ContextProducerReadV1],
        output: &mut [Option<ContextProducerReadReferenceV1>],
    ) -> Result<(), Error> {
        self.ensure_usable()?;
        self.inner
            .preflight_acquire_producer_reads_v1(consumer, requests, output)?;
        self.validate_total_read_capacity(requests.len())?;
        for read in requests {
            self.require_unqueued(read.read.allocation)?;
        }
        self.inner
            .acquire_producer_reads(consumer, requests, output)
    }
    pub fn acquire_mixed_reads(
        &mut self,
        consumer: ContextWriterKeyV1,
        stable_requests: &[ContextAllocationReadV1],
        stable_output: &mut [Option<ContextReadLeaseReferenceV1>],
        producer_requests: &[ContextProducerReadV1],
        producer_output: &mut [Option<ContextProducerReadReferenceV1>],
    ) -> Result<(), Error> {
        self.ensure_usable()?;
        self.inner.preflight_acquire_mixed_reads_v1(
            consumer,
            stable_requests,
            stable_output,
            producer_requests,
            producer_output,
        )?;
        self.validate_total_read_capacity(
            stable_requests
                .len()
                .checked_add(producer_requests.len())
                .ok_or(Error::MemberCapacity)?,
        )?;
        for read in stable_requests {
            self.require_unqueued(read.allocation)?;
        }
        for read in producer_requests {
            self.require_unqueued(read.read.allocation)?;
        }
        self.inner.acquire_mixed_reads(
            consumer,
            stable_requests,
            stable_output,
            producer_requests,
            producer_output,
        )
    }

    pub fn lookup_read(
        &self,
        reference: ContextReadLeaseReferenceV1,
    ) -> Result<ContextAllocationReadV1, Error> {
        self.inner.lookup_read(reference)
    }
    pub fn lookup_producer_read(
        &self,
        reference: ContextProducerReadReferenceV1,
    ) -> Result<ContextProducerReadV1, Error> {
        queued_active_lookup_body_v1!(self, reference)
    }
    pub fn producer_read_status(
        &self,
        reference: ContextProducerReadReferenceV1,
    ) -> Result<ContextProducerReadStatusV1, Error> {
        queued_active_status_body_v1!(self, reference)
    }
    pub fn release_reads(
        &mut self,
        consumer: ContextWriterKeyV1,
        references: &[ContextReadLeaseReferenceV1],
        evidence: &ContextReadQuiescenceEvidenceV1,
    ) -> Result<(), Error> {
        self.ensure_usable()?;
        self.inner.release_reads(consumer, references, evidence)
    }
    pub fn release_producer_reads(
        &mut self,
        consumer: ContextWriterKeyV1,
        references: &[ContextProducerReadReferenceV1],
        evidence: &ContextReadQuiescenceEvidenceV1,
    ) -> Result<(), Error> {
        self.ensure_usable()?;
        self.inner
            .release_producer_reads(consumer, references, evidence)
    }

    pub fn register_writer(
        &mut self,
        key: ContextWriterKeyV1,
    ) -> Result<ContextWriterReferenceV1, Error> {
        self.ensure_usable()?;
        let writer = self.inner.register_writer(key)?;
        if self.roots[writer.slot].is_some() {
            return Err(Error::InvalidState);
        }
        Ok(writer)
    }
    pub fn abort_reserved(&mut self, writer: ContextWriterReferenceV1) -> Result<(), Error> {
        self.ensure_usable()?;
        self.inner.lookup_reserved(writer)?;
        if self.roots[writer.slot].is_some() {
            return Err(Error::AllocationBusy);
        }
        self.inner.abort_reserved(writer)
    }
    pub fn enroll_allocation(
        &mut self,
        key: ContextAllocationKeyV1,
        device: ContextJournalDeviceKeyV1,
        extent: u64,
    ) -> Result<ContextAllocationReferenceV1, Error> {
        self.ensure_usable()?;
        self.inner.enroll_allocation(key, device, extent)
    }
    pub fn enroll_allocations(
        &mut self,
        entries: &[ContextAllocationEnrollmentV1],
        output: &mut [Option<ContextAllocationReferenceV1>],
    ) -> Result<(), Error> {
        self.ensure_usable()?;
        self.inner.enroll_allocations(entries, output)
    }
    pub fn validate_allocation_retirement(
        &self,
        allocations: &[ContextAllocationReferenceV1],
    ) -> Result<(), Error> {
        self.ensure_usable()?;
        self.inner.validate_allocation_retirement(allocations)?;
        for &allocation in allocations {
            self.require_unqueued(allocation)?;
            if self.reader_count(allocation)? != 0 {
                return Err(Error::AllocationBusy);
            }
        }
        Ok(())
    }
    pub fn retire_allocations(
        &mut self,
        allocations: &[ContextAllocationReferenceV1],
    ) -> Result<(), Error> {
        self.validate_allocation_retirement(allocations)?;
        self.inner.retire_allocations(allocations)
    }
    pub fn validate_unknown_disposal(
        &self,
        writer: ContextWriterReferenceV1,
        members: &[ContextAllocationWriteV1],
    ) -> Result<(), Error> {
        self.ensure_usable()?;
        self.inner.validate_unknown_disposal(writer, members)?;
        if self.has_queued_consumer_reads_matching(|consumer| consumer == writer.key) {
            return Err(Error::AllocationBusy);
        }
        let root = self.validate_root(writer)?;
        if root.phase != Phase::Active {
            return Err(Error::InvalidState);
        }
        for member in members {
            self.require_unqueued(member.allocation)?;
            if self.reader_count(member.allocation)? != 0 {
                return Err(Error::AllocationBusy);
            }
        }
        Ok(())
    }
    pub fn dispose_unknown(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterDisposalEvidenceV1<'_>,
    ) -> Result<(), Error> {
        self.ensure_usable()?;
        if evidence.writer != writer {
            return Err(Error::SettlementEvidenceMismatch);
        }
        self.validate_unknown_disposal(writer, evidence.allocations)?;
        let root = self.validate_root(writer)?;
        self.inner.dispose_unknown(writer, evidence)?;
        self.unlink(root, false);
        Ok(())
    }
}
