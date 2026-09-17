//! Built-in copy sources retain custody, not initializedness or input authority.

use super::*;
use fe2o3_runtime_model::{
    ContextAllocationReadV1, ContextReadLeaseReferenceV1, ContextReadQuiescenceEvidenceV1,
    ContextWriterKeyV1, ContextWriterKindV1,
};

pub(in crate::context) struct PreparedCopySourceV1 {
    source: ContextCopySourceV1,
    request: ContextAllocationReadV1,
}

pub(super) struct RetainedCopySourceV1 {
    pub(super) source: ContextCopySourceV1,
    request: ContextAllocationReadV1,
    reference: ContextReadLeaseReferenceV1,
}

#[cfg(test)]
impl ContextVersionsV1 {
    pub(in crate::context) fn read_leases_for_test_v1(
        &mut self,
    ) -> &mut ContextReadLeasedJournalV1 {
        &mut self.journal
    }
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    /// Retained copy-source pins, not data availability or reuse authority.
    pub fn version_journal_read_records_v1(&self) -> Option<usize> {
        self.versions
            .as_ref()
            .map(|versions| versions.journal.retained_read_count())
    }

    pub(in crate::context) fn prepare_copy_source_v1(
        &mut self,
        source: Option<ContextCopySourceV1>,
    ) -> Result<Option<PreparedCopySourceV1>, RuntimeValidationErrorV1> {
        let Some(source) = source else {
            return Ok(None);
        };
        let id = source.region.allocation;
        let actual = self
            .allocations
            .get(&id)
            .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?;
        if *actual != source.record
            || !self
                .backend_allocations
                .contains(&source.record.backend_allocation)
        {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
        }
        let Some(versions) = self.versions.as_ref() else {
            return Ok(None);
        };
        let result = (|| {
            let allocation = versions.validate_live(id, &source.record)?;
            let state = versions.journal.lookup_allocation(allocation)?;
            let request = ContextAllocationReadV1 {
                allocation,
                device: state.device,
                byte_extent: state.byte_extent,
                byte_offset: source.region.byte_offset,
                byte_len: source.region.byte_len,
                attempt_epoch: state.attempt_epoch,
                content_lineage: state.content_lineage,
            };
            versions.journal.validate_read(&request)?;
            versions.journal.validate_read_capacity(1)?;
            Ok(request)
        })();
        let request = match result {
            Err(ContextVersionJournalErrorV1::AllocationBusy) => {
                return Err(RuntimeValidationErrorV1::ContextReserved);
            }
            Err(
                ContextVersionJournalErrorV1::MemberCapacity
                | ContextVersionJournalErrorV1::EpochExhausted,
            ) => return Err(RuntimeValidationErrorV1::Capacity),
            result => self.journal_result_v1(result)?,
        };
        Ok(Some(PreparedCopySourceV1 { source, request }))
    }

    pub(in crate::context) fn begin_copy_source_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        prepared: Option<PreparedCopySourceV1>,
    ) -> Result<Option<ContextReadLeaseReferenceV1>, RuntimeValidationErrorV1> {
        let Some(prepared) = prepared else {
            return Ok(None);
        };
        self.guard_journal_unwind_v1(|context| {
            let result = (|| {
                let versions = context
                    .versions
                    .as_mut()
                    .ok_or(ContextVersionJournalErrorV1::InvalidState)?;
                let root = versions
                    .submission_writers
                    .get(&id)
                    .ok_or(ContextVersionJournalErrorV1::InvalidReference)?;
                if root.domain != SubmissionWriterDomainV1::Ordinary || root.copy_source.is_some() {
                    return Err(ContextVersionJournalErrorV1::InvalidState);
                }
                let consumer = ContextWriterKeyV1 {
                    context_generation: id.context_generation,
                    local: id.local,
                    kind: ContextWriterKindV1::Submission,
                };
                let mut output = [None];
                versions
                    .journal
                    .acquire_reads(consumer, &[prepared.request], &mut output)?;
                let reference = output[0].expect("complete read roster");
                versions
                    .submission_writers
                    .get_mut(&id)
                    .expect("retained writer")
                    .copy_source = Some(RetainedCopySourceV1 {
                    source: prepared.source,
                    request: prepared.request,
                    reference,
                });
                Ok(Some(reference))
            })();
            context.journal_result_v1(result)
        })
    }

    // Only exact submission quiescence or definite initial rejection calls here.
    pub(in crate::context) fn release_copy_source_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        self.require_ordinary_submission_v1(id)?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            use ContextVersionJournalErrorV1 as E;
            let record = self.submissions.get(&id);
            let expected = record.and_then(|record| record.journal_read);
            let absent = if expected.is_some() {
                Err(E::InvalidReference)
            } else {
                Ok(())
            };
            let Some(versions) = self.versions.as_mut() else {
                return absent;
            };
            let Some(root) = versions.submission_writers.get_mut(&id) else {
                return absent;
            };
            let Some(read) = root.copy_source.as_ref() else {
                return absent;
            };
            let consumer = ContextWriterKeyV1 {
                context_generation: id.context_generation,
                local: id.local,
                kind: ContextWriterKindV1::Submission,
            };
            if root.domain != SubmissionWriterDomainV1::Ordinary
                || read.reference.consumer != consumer
                || record.is_some() && expected != Some(read.reference)
                || self.allocations.get(&read.source.region.allocation) != Some(&read.source.record)
                || !self
                    .backend_allocations
                    .contains(&read.source.record.backend_allocation)
                || !self
                    .allocation_admission
                    .has_expected_credit(read.source.region.allocation, read.source.record.device)
                || versions.journal.lookup_read(read.reference)? != read.request
            {
                return Err(E::InvalidReference);
            }
            versions.journal.release_reads(
                consumer,
                &[read.reference],
                &ContextReadQuiescenceEvidenceV1 { consumer },
            )?;
            root.copy_source = None;
            if let Some(record) = self.submissions.get_mut(&id) {
                record.journal_read = None;
            }
            Ok(())
        }));
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => {
                self.quarantine_after_async_command_panic_v1();
                Err(RuntimeValidationErrorV1::InvalidBackendDescription)
            }
            Err(payload) => {
                self.quarantine_after_async_command_panic_v1();
                core::mem::forget(payload);
                Err(RuntimeValidationErrorV1::InvalidBackendDescription)
            }
        }
    }
}
