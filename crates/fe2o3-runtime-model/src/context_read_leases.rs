//! Bounded source custody and version stability, not initializedness or reuse authority.

use crate::context_version_journal::*;
use alloc::vec::Vec;
use core::ops::Deref;

macro_rules! context_read_declarations_v1 {
    ($($items:tt)*) => { $($items)* };
}

include!("context_read_leases/declarations.rs");
include!("context_read_leases/count_bodies.rs");

#[cfg(test)]
mod acquire_count_baseline;

#[cfg(test)]
pub(crate) use guard_test_support::{StableReadFaultV1, StableReadResetV1};

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

impl Deref for ContextReadLeasedJournalV1 {
    type Target = ContextVersionJournalV1;

    fn deref(&self) -> &Self::Target {
        &self.journal
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
fn read_key(request: &ContextAllocationReadV1) -> (u64, u64, u64) {
    (
        request.allocation.key.local,
        request.byte_offset,
        request.byte_len,
    )
}

impl ContextReadLeasedJournalV1 {
    pub fn new(
        generation: u64,
        allocations: usize,
        writers: usize,
        reads: usize,
    ) -> Result<Self, ContextVersionJournalErrorV1> {
        if !(1..=CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1).contains(&reads) {
            return Err(ContextVersionJournalErrorV1::InvalidCapacity);
        }
        let journal = ContextVersionJournalV1::new(generation, allocations, writers)?;
        let mut leases = storage(reads)?;
        leases.resize(reads, None);
        let mut free_reads = storage(reads)?;
        free_reads.extend((0..reads).rev());
        let mut readers = storage(allocations)?;
        readers.resize(allocations, 0);
        Ok(Self {
            journal,
            leases,
            free_reads,
            readers,
            next_incarnation: 1,
        })
    }

    pub fn remaining_read_slots(&self) -> usize {
        self.free_reads.len()
    }

    pub fn retained_read_count(&self) -> usize {
        stable_retained_read_count_body!(self)
    }

    pub fn validate_read_capacity(&self, count: usize) -> Result<(), ContextVersionJournalErrorV1> {
        stable_read_capacity_body!(self, count)
    }

    #[allow(clippy::question_mark)] // Share explicit early exits with Verus.
    pub fn reader_count(
        &self,
        allocation: ContextAllocationReferenceV1,
    ) -> Result<usize, ContextVersionJournalErrorV1> {
        stable_reader_count_body!(self, allocation)
    }

    #[allow(clippy::question_mark)]
    pub fn validate_read(
        &self,
        request: &ContextAllocationReadV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        stable_read_validate_body!(self, request)
    }

    /// Acquires a canonical (allocation ID, offset, length) roster atomically.
    /// Overlapping readers are permitted. Both output and all model state are
    /// unchanged on error. Work is O(k); storage is acquired only by construction.
    #[allow(clippy::question_mark)]
    pub fn acquire_reads(
        &mut self,
        consumer: ContextWriterKeyV1,
        requests: &[ContextAllocationReadV1],
        output: &mut [Option<ContextReadLeaseReferenceV1>],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        stable_acquire_execution_body!(
            reader_rust_expr,
            self,
            consumer,
            requests,
            output,
            acquire::stable_acquire_preflight_exec_v1,
            acquire::stable_acquire_commit_exec_v1,
            _value,
            [],
            [],
            []
        )
    }

    #[allow(clippy::question_mark)]
    pub fn lookup_read(
        &self,
        reference: ContextReadLeaseReferenceV1,
    ) -> Result<ContextAllocationReadV1, ContextVersionJournalErrorV1> {
        stable_lease_lookup_body!(self, reference, release::stable_read_reference_same_exec_v1)
    }

    /// Release order is canonical by request key, then incarnation. The caller
    /// authenticates the inert quiescence premise; failure never implies release.
    #[allow(clippy::question_mark)]
    pub fn release_reads(
        &mut self,
        consumer: ContextWriterKeyV1,
        references: &[ContextReadLeaseReferenceV1],
        evidence: &ContextReadQuiescenceEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        stable_release_execution_body!(
            reader_rust_expr,
            self,
            consumer,
            references,
            evidence,
            release::stable_release_preflight_exec_v1,
            release::stable_release_commit_exec_v1,
            [],
            _value,
            [],
            [],
            []
        )
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
        stable_begin_body!(self, writer, members)
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
            journal,
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
            journal,
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
            journal,
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
            journal,
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
        writer_owner_forward_body!(self, journal, register_writer, key, [])
    }
    pub fn abort_reserved(
        &mut self,
        writer: ContextWriterReferenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        writer_owner_forward_body!(self, journal, abort_reserved, writer, [])
    }
    pub fn enroll_allocation(
        &mut self,
        key: ContextAllocationKeyV1,
        device: ContextJournalDeviceKeyV1,
        extent: u64,
    ) -> Result<ContextAllocationReferenceV1, ContextVersionJournalErrorV1> {
        scalar_enrollment_forward_body!(self, journal, key, device, extent)
    }
    pub fn enroll_allocations(
        &mut self,
        entries: &[ContextAllocationEnrollmentV1],
        output: &mut [Option<ContextAllocationReferenceV1>],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        stable_enrollment_wrapper_body!(self, entries, output)
    }
    pub fn settle_success(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterSuccessEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        settlement_owner_forward_body!(self, journal, settle_success, writer, evidence, [])
    }
    pub fn settle_no_effect(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterNoEffectEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        settlement_owner_forward_body!(self, journal, settle_no_effect, writer, evidence, [])
    }
    pub fn mark_unknown(
        &mut self,
        writer: ContextWriterReferenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        stable_unknown_wrapper_body!(self, writer)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod guard_baseline;

#[cfg(test)]
mod guard_test_support;

#[cfg(test)]
mod acquire_baseline;

#[cfg(test)]
mod release_baseline;

#[cfg(test)]
mod unknown_baseline;

#[cfg(test)]
mod enrollment_baseline;
