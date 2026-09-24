//! Opt-in allocation and host-write metadata. No lineage or reuse authority yet.

use super::*;
#[cfg(test)]
use fe2o3_runtime_model::ContextVersionJournalV1;
use fe2o3_runtime_model::{
    ContextAllocationEnrollmentV1, ContextAllocationKeyV1, ContextAllocationReferenceV1,
    ContextJournalDeviceKeyV1, ContextProducerReadJournalV1, ContextVersionJournalErrorV1,
};

mod generated;
mod readers;
mod submission_disposal;
mod submissions;
mod writers;
pub(super) use readers::{ContextReadSourceV1, SubmissionReaderMarkerV1};
pub(super) use submissions::{SubmissionWriterDomainV1, SubmissionWriterOutcomeV1};

/// Bounded journal metadata counts, not residency, initializedness or data versions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeContextJournalUsageV1 {
    pub allocation_capacity: usize,
    pub writer_capacity: usize,
    pub allocation_records: usize,
    pub provisional_records: usize,
}

/// Failed construction returns the still-owning backend without cleanup.
/// Enumeration may already have run; this is not a pristine-state guarantee.
#[must_use = "construction failure retains the original backend"]
pub struct RuntimeContextOpenFailureV1<B: RuntimeBackendV1> {
    pub(super) backend: B,
    pub(super) error: RuntimeErrorV1<B::Error>,
}

impl<B: RuntimeBackendV1> fmt::Debug for RuntimeContextOpenFailureV1<B> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeContextOpenFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl<B: RuntimeBackendV1> RuntimeContextOpenFailureV1<B> {
    pub fn error(&self) -> &RuntimeErrorV1<B::Error> {
        &self.error
    }

    pub fn into_parts(self) -> (B, RuntimeErrorV1<B::Error>) {
        (self.backend, self.error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AllocationPhaseV1 {
    Provisional,
    Live,
    Disposed,
}

pub(super) struct ContextVersionsV1 {
    journal: ContextProducerReadJournalV1,
    phases: Vec<Option<AllocationPhaseV1>>,
    submission_writers: HashMap<RuntimeSubmissionIdV1, submissions::RetainedSubmissionWriterV1>,
    submission_readers: HashMap<RuntimeSubmissionIdV1, readers::RetainedSubmissionReadersV1>,
}

// Dropping this move-only ticket never removes its Context-owned record.
pub(super) struct AllocationEnrollmentV1(ContextAllocationReferenceV1);

// A move-only preflight, consumed only after the exact backend disposal succeeds.
pub(super) struct AllocationDisposalV1 {
    id: RuntimeAllocationIdV1,
    record: AllocationRecordV1,
    member: fe2o3_runtime_model::ContextAllocationWriteV1,
    kind: AllocationDisposalKindV1,
}

enum AllocationDisposalKindV1 {
    Unwritten,
    Synchronous(fe2o3_runtime_model::ContextWriterReferenceV1),
    Submission {
        id: RuntimeSubmissionIdV1,
        index: usize,
    },
}

impl AllocationEnrollmentV1 {
    pub(super) fn reference(&self) -> ContextAllocationReferenceV1 {
        self.0
    }
}

pub(super) fn enrollment(
    id: RuntimeAllocationIdV1,
    device: RuntimeDeviceIdV1,
    byte_extent: u64,
) -> ContextAllocationEnrollmentV1 {
    ContextAllocationEnrollmentV1 {
        key: ContextAllocationKeyV1 {
            context_generation: id.context_generation,
            local: id.local,
        },
        device: ContextJournalDeviceKeyV1 {
            context_generation: device.context_generation,
            local: device.local,
        },
        byte_extent,
    }
}

impl ContextVersionsV1 {
    pub(super) fn new(
        generation: u64,
        allocations: usize,
        writers: usize,
    ) -> Result<Self, ContextVersionJournalErrorV1> {
        let journal = ContextProducerReadJournalV1::new(generation, allocations, writers, writers)?;
        let mut phases = Vec::new();
        phases
            .try_reserve_exact(allocations)
            .map_err(|_| ContextVersionJournalErrorV1::StorageAllocationFailed)?;
        phases.resize(allocations, None);
        let mut submission_writers = HashMap::new();
        submission_writers
            .try_reserve(writers)
            .map_err(|_| ContextVersionJournalErrorV1::StorageAllocationFailed)?;
        let mut submission_readers = HashMap::new();
        submission_readers
            .try_reserve(writers)
            .map_err(|_| ContextVersionJournalErrorV1::StorageAllocationFailed)?;
        Ok(Self {
            journal,
            phases,
            submission_writers,
            submission_readers,
        })
    }

    pub(super) fn retained_records(&self) -> usize {
        self.journal.allocation_capacity() - self.journal.remaining_allocation_slots()
    }

    pub(super) fn usage(&self) -> RuntimeContextJournalUsageV1 {
        RuntimeContextJournalUsageV1 {
            allocation_capacity: self.journal.allocation_capacity(),
            writer_capacity: self.journal.writer_capacity(),
            allocation_records: self.retained_records(),
            provisional_records: self
                .phases
                .iter()
                .filter(|phase| **phase == Some(AllocationPhaseV1::Provisional))
                .count(),
        }
    }

    pub(super) fn enroll_roster(
        &mut self,
        entries: &[ContextAllocationEnrollmentV1],
        output: &mut [Option<ContextAllocationReferenceV1>],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.journal.enroll_allocations(entries, output)?;
        for reference in output.iter().flatten() {
            if self.phases[reference.slot].is_some() {
                return Err(ContextVersionJournalErrorV1::InvalidState);
            }
        }
        for reference in output.iter().flatten() {
            self.phases[reference.slot] = Some(AllocationPhaseV1::Provisional);
        }
        Ok(())
    }

    fn validate_phase(
        &self,
        reference: ContextAllocationReferenceV1,
        phase: AllocationPhaseV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.journal.lookup_allocation(reference)?;
        if self.phases.get(reference.slot) != Some(&Some(phase)) {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        Ok(())
    }

    pub(super) fn commit_live(
        &mut self,
        reference: ContextAllocationReferenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.validate_phase(reference, AllocationPhaseV1::Provisional)?;
        self.phases[reference.slot] = Some(AllocationPhaseV1::Live);
        Ok(())
    }

    pub(super) fn validate_live(
        &self,
        id: RuntimeAllocationIdV1,
        record: &AllocationRecordV1,
    ) -> Result<ContextAllocationReferenceV1, ContextVersionJournalErrorV1> {
        let reference = record
            .journal
            .ok_or(ContextVersionJournalErrorV1::InvalidState)?;
        self.validate_phase(reference, AllocationPhaseV1::Live)?;
        let expected = enrollment(id, record.device, record.byte_len);
        let actual = self.journal.lookup_allocation(reference)?;
        if reference.key != expected.key
            || actual.device != expected.device
            || actual.byte_extent != expected.byte_extent
        {
            return Err(ContextVersionJournalErrorV1::InvalidAllocationReference);
        }
        Ok(reference)
    }

    pub(super) fn validate_retirement(
        &self,
        references: &[ContextAllocationReferenceV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.journal.validate_allocation_retirement(references)
    }

    pub(super) fn retire(
        &mut self,
        references: &[ContextAllocationReferenceV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        self.journal.retire_allocations(references)?;
        for reference in references {
            self.phases[reference.slot] = None;
        }
        Ok(())
    }
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    pub(super) fn guard_journal_unwind_v1<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> T,
    ) -> T {
        if self.versions.is_none() && self.scalar_peer_copies.is_empty() {
            return operation(self);
        }
        match catch_unwind(AssertUnwindSafe(|| operation(self))) {
            Ok(value) => value,
            Err(payload) => {
                self.quarantine_submission_writers_v1();
                std::panic::resume_unwind(payload);
            }
        }
    }

    /// Enables a bounded allocation journal before any Context-local ID exists.
    /// Each capacity must be in 1..=1_048_576; invalid bounds reject before
    /// enumeration. Other failures return the possibly enumeration-entered backend.
    ///
    /// This development profile tracks allocation custody, synchronous host
    /// writes, ordinary asynchronous launch/copy destinations, and the original
    /// writable buffers of protected generated attempts. Generated settlement
    /// requires protected completion or whole-batch Stop disposal, never generic
    /// polling. Writer capacity and unresolved writes gate subsequent writes.
    /// Built-in copies and pure Read typed bindings retain bounded source read
    /// leases. Generated ReadOnly shell members use the same reader arena with
    /// a distinct stream/hold/shell domain and protected settlement boundary.
    /// Reader capacity equals writer capacity but its occupancy is
    /// independent. A writable alias uses only exclusive writer custody.
    /// Source writes and retirement reject while any reader remains.
    /// Backend aliases and initialized-input
    /// authority remain outside this profile;
    /// no content lineage or reuse permission is exposed. Unknown async writers
    /// retain their journal metadata and credits, even after submission metadata
    /// is released, until every original destination owner has been disposed.
    /// The default `open` path has no journal exclusion. Launch preparation still
    /// retains original Read allocation identities for revalidation at issue;
    /// its bounded metadata allocation can return Capacity before submission.
    pub fn open_with_version_journal_v1(
        backend: B,
        allocation_capacity: usize,
        writer_capacity: usize,
    ) -> Result<Self, RuntimeContextOpenFailureV1<B>> {
        Self::open_configured_v1(backend, Some((allocation_capacity, writer_capacity)))
    }

    pub fn version_journal_usage_v1(&self) -> Option<RuntimeContextJournalUsageV1> {
        self.versions.as_ref().map(ContextVersionsV1::usage)
    }

    /// Retained writer metadata, not completion or resource-reuse authority.
    pub fn version_journal_writer_records_v1(&self) -> Option<usize> {
        self.versions
            .as_ref()
            .map(ContextVersionsV1::retained_writers)
    }

    pub(super) fn preflight_journal_capacity_v1(
        &self,
        count: usize,
    ) -> Result<(), RuntimeValidationErrorV1> {
        if self
            .versions
            .as_ref()
            .is_some_and(|versions| versions.journal.remaining_allocation_slots() < count)
        {
            return Err(RuntimeValidationErrorV1::Capacity);
        }
        Ok(())
    }

    pub(super) fn journal_result_v1<T>(
        &mut self,
        result: Result<T, ContextVersionJournalErrorV1>,
    ) -> Result<T, RuntimeValidationErrorV1> {
        result.map_err(|_| {
            self.quarantine_submission_writers_v1();
            RuntimeValidationErrorV1::InvalidBackendDescription
        })
    }

    pub(super) fn enroll_journal_allocation_v1(
        &mut self,
        id: RuntimeAllocationIdV1,
        device: RuntimeDeviceIdV1,
        bytes: u64,
    ) -> Result<Option<AllocationEnrollmentV1>, RuntimeValidationErrorV1> {
        self.guard_journal_unwind_v1(|context| {
            let Some(versions) = context.versions.as_mut() else {
                return Ok(None);
            };
            let mut output = [None];
            let result = versions.enroll_roster(&[enrollment(id, device, bytes)], &mut output);
            context.journal_result_v1(result)?;
            Ok(Some(AllocationEnrollmentV1(
                output[0].expect("complete enrollment"),
            )))
        })
    }

    pub(super) fn commit_journal_allocation_v1(&mut self, ticket: Option<AllocationEnrollmentV1>) {
        if let Some(ticket) = ticket {
            let result = self
                .versions
                .as_mut()
                .expect("configured journal")
                .commit_live(ticket.0);
            self.journal_result_v1(result)
                .expect("allocation journal commit invariant");
        }
    }

    pub(super) fn dispose_journal_provisional_v1(
        &mut self,
        ticket: Option<AllocationEnrollmentV1>,
    ) {
        self.guard_journal_unwind_v1(|context| {
            if let Some(ticket) = ticket {
                let versions = context.versions.as_mut().expect("configured journal");
                let result = versions
                    .validate_phase(ticket.0, AllocationPhaseV1::Provisional)
                    .and_then(|()| versions.retire(&[ticket.0]));
                context
                    .journal_result_v1(result)
                    .expect("provisional journal disposal invariant");
            }
        });
    }

    pub(super) fn finish_allocation_disposal_v1(
        &mut self,
        id: RuntimeAllocationIdV1,
        record: AllocationRecordV1,
        plan: Option<AllocationDisposalV1>,
    ) {
        self.guard_journal_unwind_v1(|context| {
            let exact = match (&plan, &context.versions) {
                (Some(plan), Some(_)) => {
                    plan.id == id
                        && plan.record == record
                        && context.allocations.get(&id) == Some(&record)
                }
                (None, None) => record.journal.is_none(),
                _ => false,
            };
            if !exact {
                context.quarantine_submission_writers_v1();
                panic!("allocation disposal plan identity invariant");
            }
            let plan = match plan {
                Some(plan) if matches!(plan.kind, AllocationDisposalKindV1::Submission { .. }) => {
                    let result = context.finish_submission_allocation_disposal_v1(plan);
                    context
                        .journal_result_v1(result)
                        .expect("submission allocation disposal invariant");
                    return;
                }
                plan => plan,
            };
            context.dispose_allocation_credits_v1(id);
            if let Some(plan) = plan {
                let result = context
                    .versions
                    .as_mut()
                    .expect("configured journal")
                    .retire_after_disposal(plan);
                context
                    .journal_result_v1(result)
                    .expect("allocation journal disposal invariant");
            }
            context.allocations.remove(&id);
            context
                .backend_allocations
                .remove(&record.backend_allocation);
        });
    }
}
