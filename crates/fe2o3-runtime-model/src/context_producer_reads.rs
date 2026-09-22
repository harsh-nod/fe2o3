//! Producer-bound input custody. Reservations are not evidence of available data.

use crate::context_read_leases::*;
use crate::context_version_journal::*;
use alloc::vec::Vec;
use core::ops::Deref;

macro_rules! context_producer_read_declarations_v1 {
    ($($items:tt)*) => { $($items)* };
}

include!("context_producer_reads/declarations.rs");

#[macro_use]
mod query;

#[allow(unused_macros)]
#[macro_use]
mod guard_templates {
    include!("context_read_leases/guard_bodies.rs");
}

macro_rules! reader_rust_expr {
    ($body:expr) => {
        $body
    };
}

#[macro_use]
mod acquire;

#[macro_use]
mod release;

#[macro_use]
mod stable_wrappers;

#[allow(unused_macros)]
#[macro_use]
mod unknown_templates {
    include!("context_version_journal/unknown_wrapper_bodies.rs");
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

#[cfg(test)]
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
        producer_retained_count_body!(self)
    }

    pub fn retained_read_count(&self) -> usize {
        producer_total_read_count_body!(self)
    }

    pub fn remaining_read_slots(&self) -> usize {
        producer_remaining_read_slots_body!(self)
    }

    #[allow(clippy::question_mark)] // Share explicit early exits with Verus.
    pub fn reader_count(
        &self,
        allocation: ContextAllocationReferenceV1,
    ) -> Result<usize, ContextVersionJournalErrorV1> {
        producer_reader_count_body!(self, allocation)
    }

    pub fn validate_read_capacity(&self, count: usize) -> Result<(), ContextVersionJournalErrorV1> {
        producer_stable_capacity_body!(self, count)
    }

    pub fn validate_producer_read_capacity(
        &self,
        count: usize,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        producer_capacity_body!(self, count)
    }

    #[allow(clippy::question_mark)]
    fn status(
        &self,
        request: &ContextProducerReadV1,
    ) -> Result<ContextProducerReadStatusV1, ContextVersionJournalErrorV1> {
        producer_status_body!(&self.stable, request, query::producer_writer_same_exec_v1)
    }

    pub fn validate_producer_read(
        &self,
        request: &ContextProducerReadV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        producer_validate_body!(self, request)
    }

    /// Atomic canonical-roster acquisition. The caller authenticates each exact
    /// producer/event relationship and success-gated backend execution separately.
    #[allow(clippy::question_mark)]
    pub fn acquire_producer_reads(
        &mut self,
        consumer: ContextWriterKeyV1,
        requests: &[ContextProducerReadV1],
        output: &mut [Option<ContextProducerReadReferenceV1>],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        producer_acquire_execution_body!(
            reader_rust_expr,
            self,
            consumer,
            requests,
            output,
            acquire::producer_acquire_preflight_exec_v1,
            acquire::producer_acquire_commit_exec_v1,
            _value,
            [],
            [],
            []
        )
    }

    #[allow(clippy::question_mark)]
    fn inspect_producer_read(
        &self,
        reference: ContextProducerReadReferenceV1,
    ) -> Result<(ContextProducerReadV1, ContextProducerReadStatusV1), ContextVersionJournalErrorV1>
    {
        producer_inspect_body!(self, reference, query::producer_reference_same_exec_v1)
    }

    pub fn lookup_producer_read(
        &self,
        reference: ContextProducerReadReferenceV1,
    ) -> Result<ContextProducerReadV1, ContextVersionJournalErrorV1> {
        producer_lookup_body!(self, reference)
    }

    pub fn producer_read_status(
        &self,
        reference: ContextProducerReadReferenceV1,
    ) -> Result<ContextProducerReadStatusV1, ContextVersionJournalErrorV1> {
        producer_query_body!(self, reference)
    }

    /// Quiescence releases custody even when the producer is still Pending or
    /// Unknown. It never asserts that the consumer observed successful input.
    #[allow(clippy::question_mark)]
    pub fn release_producer_reads(
        &mut self,
        consumer: ContextWriterKeyV1,
        references: &[ContextProducerReadReferenceV1],
        evidence: &ContextReadQuiescenceEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        producer_release_execution_body!(
            reader_rust_expr,
            self,
            consumer,
            references,
            evidence,
            release::producer_release_preflight_exec_v1,
            release::producer_release_commit_exec_v1,
            [],
            _value,
            [],
            [],
            []
        )
    }

    #[allow(clippy::question_mark)]
    pub fn acquire_reads(
        &mut self,
        consumer: ContextWriterKeyV1,
        requests: &[ContextAllocationReadV1],
        output: &mut [Option<ContextReadLeaseReferenceV1>],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        producer_stable_acquire_body!(
            reader_rust_expr,
            self,
            consumer,
            requests,
            output,
            stable_wrappers::producer_stable_acquire_header_exec_v1,
            _value,
            result,
            [],
            [],
            []
        )
    }

    pub fn release_reads(
        &mut self,
        consumer: ContextWriterKeyV1,
        references: &[ContextReadLeaseReferenceV1],
        evidence: &ContextReadQuiescenceEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        producer_stable_release_body!(self, consumer, references, evidence, release_reads, [])
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

    #[allow(clippy::question_mark)]
    fn require_unread_writes(
        &self,
        members: &[ContextAllocationWriteV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        unread_writes_body!(reader_rust_expr, self, members, index, [])
    }

    #[allow(clippy::question_mark)]
    pub fn begin_write(
        &mut self,
        writer: ContextWriterReferenceV1,
        members: &[ContextAllocationWriteV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        producer_begin_body!(self, writer, members)
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
        producer_unknown_wrapper_body!(self, writer)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod guard_baseline;

#[cfg(test)]
mod guard_test_support;

#[cfg(test)]
mod query_baseline;

#[cfg(test)]
mod acquire_baseline;

#[cfg(test)]
mod release_baseline;

#[cfg(test)]
mod stable_wrapper_baseline;

#[cfg(test)]
mod unknown_baseline;
