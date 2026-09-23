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

#[allow(unused_macros)]
#[macro_use]
mod enrollment_templates {
    include!("context_version_journal/enrollment_wrapper_bodies.rs");
}

#[allow(unused_macros)]
#[macro_use]
mod writer_lifecycle_templates {
    include!("context_version_journal/writer_lifecycle_bodies.rs");
}

#[allow(unused_macros)]
#[macro_use]
mod settlement_templates {
    include!("context_version_journal/settlement_wrapper_bodies.rs");
}

#[allow(unused_macros)]
#[macro_use]
mod scalar_enrollment_templates {
    include!("context_version_journal/scalar_enrollment_bodies.rs");
}

#[cfg(test)]
mod scalar_enrollment_baseline;

#[cfg(test)]
mod construction_baseline;

#[allow(unused_macros)]
#[macro_use]
mod constructor_templates {
    include!("context_version_journal/constructor_bodies.rs");
}

#[allow(unused_macros)]
#[macro_use]
mod retirement_templates {
    include!("context_version_journal/retirement_bodies.rs");
}

#[allow(unused_macros)]
#[macro_use]
mod disposal_templates {
    include!("context_version_journal/disposal_bodies.rs");
}

#[cfg(test)]
mod disposal_baseline;

#[cfg(test)]
mod retirement_baseline;

#[cfg(test)]
mod writer_lifecycle_baseline;

#[cfg(test)]
mod settlement_baseline;

impl Deref for ContextProducerReadJournalV1 {
    type Target = ContextReadLeasedJournalV1;

    fn deref(&self) -> &Self::Target {
        &self.stable
    }
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
        constructor_entry_body!(
            Self::new_with_allocator_v1,
            &mut construction::NativeConstructorAllocatorV1,
            generation,
            allocations,
            writers,
            reads
        )
    }

    #[allow(clippy::question_mark)]
    pub(crate) fn new_with_allocator_v1(
        generation: u64,
        allocations: usize,
        writers: usize,
        reads: usize,
        allocator: &mut impl construction::ConstructorAllocatorV1,
    ) -> Result<Self, ContextVersionJournalErrorV1> {
        constructor_producer_body!(
            reader_rust_expr,
            generation,
            allocations,
            writers,
            reads,
            allocator,
            ContextReadLeasedJournalV1::new_with_allocator_v1,
            construction::vacant,
            construction::free,
            construction::zero,
            result,
            [],
            [],
            []
        )
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

    #[cfg(test)]
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

    #[allow(clippy::question_mark)]
    fn require_unread_allocations(
        &self,
        references: &[ContextAllocationReferenceV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        retirement_unread_body!(reader_rust_expr, self, references, index, [])
    }

    #[allow(clippy::question_mark)]
    pub fn validate_allocation_retirement(
        &self,
        references: &[ContextAllocationReferenceV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        retirement_owner_validate_body!(
            reader_rust_expr,
            self,
            stable,
            references,
            validate_allocation_retirement,
            [],
            []
        )
    }

    #[allow(clippy::question_mark)]
    pub fn retire_allocations(
        &mut self,
        references: &[ContextAllocationReferenceV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        retirement_owner_execute_body!(
            reader_rust_expr,
            self,
            stable,
            references,
            validate_allocation_retirement,
            retire_allocations,
            [],
            []
        )
    }

    #[allow(clippy::question_mark)]
    pub fn validate_unknown_disposal(
        &self,
        writer: ContextWriterReferenceV1,
        members: &[ContextAllocationWriteV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        disposal_owner_validate_body!(
            reader_rust_expr,
            self,
            stable,
            writer,
            members,
            require_unread_writes,
            validate_unknown_disposal,
            [],
            []
        )
    }

    #[allow(clippy::question_mark)]
    pub fn dispose_unknown(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterDisposalEvidenceV1<'_>,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        disposal_owner_execute_body!(
            reader_rust_expr,
            self,
            stable,
            writer,
            evidence,
            validate_unknown_disposal,
            dispose_unknown,
            [],
            []
        )
    }

    pub fn register_writer(
        &mut self,
        key: ContextWriterKeyV1,
    ) -> Result<ContextWriterReferenceV1, ContextVersionJournalErrorV1> {
        writer_owner_forward_body!(self, stable, register_writer, key, [])
    }

    pub fn abort_reserved(
        &mut self,
        writer: ContextWriterReferenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        writer_owner_forward_body!(self, stable, abort_reserved, writer, [])
    }

    pub fn enroll_allocation(
        &mut self,
        key: ContextAllocationKeyV1,
        device: ContextJournalDeviceKeyV1,
        extent: u64,
    ) -> Result<ContextAllocationReferenceV1, ContextVersionJournalErrorV1> {
        scalar_enrollment_forward_body!(self, stable, key, device, extent)
    }

    pub fn enroll_allocations(
        &mut self,
        entries: &[ContextAllocationEnrollmentV1],
        output: &mut [Option<ContextAllocationReferenceV1>],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        producer_enrollment_wrapper_body!(self, entries, output)
    }

    pub fn settle_success(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterSuccessEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        settlement_owner_forward_body!(self, stable, settle_success, writer, evidence, [])
    }

    pub fn settle_no_effect(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterNoEffectEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        settlement_owner_forward_body!(self, stable, settle_no_effect, writer, evidence, [])
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

#[cfg(test)]
mod enrollment_baseline;
