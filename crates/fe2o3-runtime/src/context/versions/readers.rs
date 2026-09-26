//! Domain-bound input custody, not initializedness or executable/reuse authority.

use super::*;
use fe2o3_runtime_model::{
    ContextAllocationReadV1, ContextReadLeaseReferenceV1, ContextReadQuiescenceEvidenceV1,
    ContextWriterKeyV1, ContextWriterKindV1,
};

#[derive(Clone, Copy, Debug)]
pub(in crate::context) struct ContextReadSourceV1 {
    pub(in crate::context) region: RuntimeMemoryRegionV1,
    pub(in crate::context) record: AllocationRecordV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::context) struct SubmissionReaderMarkerV1 {
    pub(in crate::context) first: ContextReadLeaseReferenceV1,
    pub(in crate::context) count: usize,
}

pub(in crate::context) struct PreparedSubmissionReadersV1 {
    pub(super) root: RetainedSubmissionReadersV1,
    pub(super) output: Vec<Option<ContextReadLeaseReferenceV1>>,
}

pub(super) struct RetainedSubmissionReadersV1 {
    pub(super) domain: SubmissionWriterDomainV1,
    pub(super) sources: Vec<ContextReadSourceV1>,
    pub(super) requests: Vec<ContextAllocationReadV1>,
    pub(super) references: Vec<ContextReadLeaseReferenceV1>,
    pub(super) marker: Option<SubmissionReaderMarkerV1>,
}

impl ContextVersionsV1 {
    pub(in crate::context) fn retained_readers(&self) -> usize {
        self.journal.retained_read_count()
            + self
                .submission_readers
                .values()
                .filter(|root| root.marker.is_none())
                .count()
            + self
                .producer_readers
                .values()
                .filter(|root| root.marker.is_none())
                .count()
    }

    #[cfg(test)]
    pub(in crate::context) fn read_leases_for_test_v1(
        &mut self,
    ) -> &mut ContextProducerReadJournalV1 {
        &mut self.journal
    }

    #[cfg(test)]
    pub(in crate::context) fn remove_submission_readers_for_test_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) {
        self.submission_readers.remove(&id);
    }

    #[cfg(test)]
    pub(in crate::context) fn corrupt_submission_read_reference_for_test_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        index: usize,
    ) {
        self.submission_readers.get_mut(&id).unwrap().references[index].incarnation += 1;
    }

    #[cfg(test)]
    pub(in crate::context) fn submission_reader_sources_for_test_v1(
        &self,
        id: RuntimeSubmissionIdV1,
    ) -> &[ContextReadSourceV1] {
        &self.submission_readers[&id].sources
    }

    #[cfg(test)]
    pub(in crate::context) fn submission_reader_identity_for_test_v1(
        &self,
        id: RuntimeSubmissionIdV1,
    ) -> (SubmissionWriterDomainV1, &[ContextReadLeaseReferenceV1]) {
        let root = &self.submission_readers[&id];
        (root.domain, &root.references)
    }

    #[cfg(test)]
    pub(in crate::context) fn set_submission_reader_domain_for_test_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        domain: SubmissionWriterDomainV1,
    ) {
        self.submission_readers.get_mut(&id).unwrap().domain = domain;
    }
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    /// Active input leases plus provisional batch roots, not available-data counts
    /// or reuse authority. A failed acquisition can retain a root with no lease.
    pub fn version_journal_read_records_v1(&self) -> Option<usize> {
        self.versions
            .as_ref()
            .map(ContextVersionsV1::retained_readers)
    }

    pub(in crate::context) fn prepare_submission_readers_v1(
        &mut self,
        sources: &[ContextReadSourceV1],
    ) -> Result<Option<PreparedSubmissionReadersV1>, RuntimeValidationErrorV1> {
        self.guard_journal_unwind_v1(|context| {
            let mut previous = None;
            for source in sources {
                let id = source.region.allocation;
                let actual = context
                    .allocations
                    .get(&id)
                    .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?;
                if *actual != source.record
                    || !context
                        .backend_allocations
                        .contains(&source.record.backend_allocation)
                    || !context.allocation_admission.has_expected_credit(
                        id,
                        source.record.device,
                        source.record.byte_len,
                    )
                    || previous.is_some_and(|prior| prior >= id)
                {
                    return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
                }
                previous = Some(id);
            }
            let Some(versions) = context.versions.as_ref() else {
                return Ok(None);
            };
            if sources.is_empty() {
                return Ok(None);
            }
            let mut requests = Vec::new();
            requests
                .try_reserve_exact(sources.len())
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            let result = (|| {
                versions.journal.validate_read_capacity(sources.len())?;
                for source in sources {
                    let allocation =
                        versions.validate_live(source.region.allocation, &source.record)?;
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
                    requests.push(request);
                }
                Ok(())
            })();
            match result {
                Err(ContextVersionJournalErrorV1::AllocationBusy) => {
                    return Err(RuntimeValidationErrorV1::ContextReserved);
                }
                Err(
                    ContextVersionJournalErrorV1::MemberCapacity
                    | ContextVersionJournalErrorV1::EpochExhausted,
                ) => return Err(RuntimeValidationErrorV1::Capacity),
                result => context.journal_result_v1(result)?,
            }
            let mut originals = Vec::new();
            originals
                .try_reserve_exact(sources.len())
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            originals.extend_from_slice(sources);
            let mut output = Vec::new();
            output
                .try_reserve_exact(sources.len())
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            output.resize(sources.len(), None);
            let mut references = Vec::new();
            references
                .try_reserve_exact(sources.len())
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            context
                .versions
                .as_mut()
                .expect("configured journal")
                .submission_readers
                .try_reserve(1)
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            Ok(Some(PreparedSubmissionReadersV1 {
                root: RetainedSubmissionReadersV1 {
                    domain: SubmissionWriterDomainV1::Ordinary,
                    sources: originals,
                    requests,
                    references,
                    marker: None,
                },
                output,
            }))
        })
    }

    pub(in crate::context) fn begin_submission_readers_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        prepared: Option<PreparedSubmissionReadersV1>,
        domain: SubmissionWriterDomainV1,
    ) -> Result<Option<SubmissionReaderMarkerV1>, RuntimeValidationErrorV1> {
        let Some(mut prepared) = prepared else {
            return Ok(None);
        };
        self.guard_journal_unwind_v1(|context| {
            let result = (|| {
                let versions = context
                    .versions
                    .as_mut()
                    .ok_or(ContextVersionJournalErrorV1::InvalidState)?;
                if versions.submission_readers.contains_key(&id)
                    || versions.producer_readers.contains_key(&id)
                        && !(domain == SubmissionWriterDomainV1::Ordinary
                            && context.producer_launches.contains_key(&id))
                    || versions
                        .submission_writers
                        .get(&id)
                        .is_some_and(|root| root.domain != domain)
                {
                    return Err(ContextVersionJournalErrorV1::InvalidState);
                }
                assert!(
                    versions.submission_readers.len() < versions.submission_readers.capacity(),
                    "preallocated reader root"
                );
                // Root the complete original roster before the model can acquire any lease.
                prepared.root.domain = domain;
                versions.submission_readers.insert(id, prepared.root);
                let root = versions
                    .submission_readers
                    .get_mut(&id)
                    .expect("retained readers");
                let consumer = ContextWriterKeyV1 {
                    context_generation: id.context_generation,
                    local: id.local,
                    kind: ContextWriterKindV1::Submission,
                };
                versions
                    .journal
                    .acquire_reads(consumer, &root.requests, &mut prepared.output)?;
                assert!(
                    root.references.capacity() >= prepared.output.len(),
                    "preallocated references"
                );
                for reference in prepared.output {
                    root.references
                        .push(reference.expect("complete read roster"));
                }
                let marker = SubmissionReaderMarkerV1 {
                    first: root.references[0],
                    count: root.references.len(),
                };
                root.marker = Some(marker);
                Ok(Some(marker))
            })();
            context.journal_result_v1(result)
        })
    }

    pub(super) fn validate_submission_readers_v1(
        &self,
        id: RuntimeSubmissionIdV1,
        domain: SubmissionWriterDomainV1,
    ) -> Result<Option<&RetainedSubmissionReadersV1>, ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        let record = self.submissions.get(&id);
        let expected = record.and_then(|record| record.journal_read);
        let absent = if expected.is_some() {
            Err(E::InvalidReference)
        } else {
            Ok(None)
        };
        let Some(versions) = self.versions.as_ref() else {
            return absent;
        };
        let Some(root) = versions.submission_readers.get(&id) else {
            return absent;
        };
        if (versions.producer_readers.contains_key(&id)
            || record.is_some_and(|record| record.journal_producer_read.is_some()))
            && !(domain == SubmissionWriterDomainV1::Ordinary
                && self.producer_launches.contains_key(&id)
                && record.is_none_or(|record| record.producer_launch))
        {
            return Err(E::InvalidReference);
        }
        let marker = root.marker.ok_or(E::InvalidReference)?;
        let consumer = ContextWriterKeyV1 {
            context_generation: id.context_generation,
            local: id.local,
            kind: ContextWriterKindV1::Submission,
        };
        if root.domain != domain
            || marker.count == 0
            || marker.count != root.sources.len()
            || marker.count != root.requests.len()
            || marker.count != root.references.len()
            || root.references[0] != marker.first
            || marker.first.consumer != consumer
            || record.is_some() && expected != Some(marker)
            || record.is_some_and(|record| {
                record.journal_writer
                    != versions.submission_writers.get(&id).map(|root| root.writer)
            })
            || versions
                .submission_writers
                .get(&id)
                .is_some_and(|root| root.domain != domain)
        {
            return Err(E::InvalidReference);
        }
        for (index, ((source, request), reference)) in root
            .sources
            .iter()
            .zip(&root.requests)
            .zip(&root.references)
            .enumerate()
        {
            if reference.consumer != consumer
                || marker.first.incarnation.checked_add(index as u64) != Some(reference.incarnation)
                || source.region.allocation.context_generation
                    != request.allocation.key.context_generation
                || source.region.allocation.local != request.allocation.key.local
                || source.region.byte_offset != request.byte_offset
                || source.region.byte_len != request.byte_len
                || source.record.byte_len != request.byte_extent
                || index > 0
                    && root.sources[index - 1].region.allocation >= source.region.allocation
                || self.allocations.get(&source.region.allocation) != Some(&source.record)
                || !self
                    .backend_allocations
                    .contains(&source.record.backend_allocation)
                || !self.allocation_admission.has_expected_credit(
                    source.region.allocation,
                    source.record.device,
                    source.record.byte_len,
                )
                || versions.validate_live(source.region.allocation, &source.record)?
                    != request.allocation
                || versions.journal.lookup_read(*reference)? != *request
            {
                return Err(E::InvalidReference);
            }
        }
        Ok(Some(root))
    }

    pub(super) fn release_readers_in_domain_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        domain: SubmissionWriterDomainV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        let result = catch_unwind(AssertUnwindSafe(|| {
            if self.validate_submission_readers_v1(id, domain)?.is_none() {
                return Ok(());
            }
            self.release_validated_readers_v1(id)
        }));
        match result {
            Ok(result) => self.journal_result_v1(result),
            Err(payload) => {
                self.quarantine_after_async_command_panic_v1();
                core::mem::forget(payload);
                Err(RuntimeValidationErrorV1::InvalidBackendDescription)
            }
        }
    }

    // Joint completion has validated both rosters, without intervening mutation.
    pub(super) fn release_prevalidated_submission_readers_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        let result = catch_unwind(AssertUnwindSafe(|| {
            if self
                .versions
                .as_ref()
                .is_none_or(|versions| !versions.submission_readers.contains_key(&id))
            {
                return Ok(());
            }
            self.release_validated_readers_v1(id)
        }));
        match result {
            Ok(result) => self.journal_result_v1(result),
            Err(payload) => {
                self.quarantine_after_async_command_panic_v1();
                core::mem::forget(payload);
                Err(RuntimeValidationErrorV1::InvalidBackendDescription)
            }
        }
    }

    #[allow(clippy::question_mark)] // The effect/retirement body is shared with Verus.
    fn release_validated_readers_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        let versions = self.versions.as_mut().expect("validated readers");
        #[cfg(test)]
        versions.completion_boundary_for_test_v1(
            id,
            completion_faults::CompletionJournalStageV1::Stable,
            completion_faults::CompletionJournalPointV1::BeforeEffect,
        )?;
        completion_selected_reader_release_body!(completion_journal_rust_syntax,
        versions.submission_readers, self.submissions, versions.journal,
        id, journal_read, release_reads, [
            #[cfg(test)]
            versions.completion_boundary_for_test_v1(
                id,
                completion_faults::CompletionJournalStageV1::Stable,
                completion_faults::CompletionJournalPointV1::AfterEffect,
            )?;
        ])
    }
}
