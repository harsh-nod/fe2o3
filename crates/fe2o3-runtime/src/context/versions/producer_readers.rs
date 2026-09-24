//! One exact pending input for the opt-in directed scalar copy profile.

use super::*;
use crate::context::peer_custody::ScalarPeerDependencyV1;
use fe2o3_runtime_model::{
    ContextAllocationReadV1, ContextProducerReadReferenceV1, ContextProducerReadStatusV1,
    ContextProducerReadV1, ContextReadQuiescenceEvidenceV1, ContextWriterKeyV1,
    ContextWriterKindV1,
};

pub(super) struct RetainedProducerReadV1 {
    pub(super) source: ContextReadSourceV1,
    dependency: ScalarPeerDependencyV1,
    request: ContextProducerReadV1,
    pub(super) reference: Option<ContextProducerReadReferenceV1>,
}

#[cfg(test)]
impl ContextVersionsV1 {
    pub(in crate::context) fn remove_producer_read_root_for_test_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) {
        self.producer_readers.remove(&id);
    }
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    pub(super) fn prepare_producer_read_v1(
        &mut self,
        peer: Option<&PreparedPeerSubmissionV1>,
    ) -> Result<Option<RetainedProducerReadV1>, RuntimeValidationErrorV1> {
        self.guard_journal_unwind_v1(|context| {
            let Some(root) = peer
                .and_then(|peer| peer.scalar.as_ref())
                .filter(|root| root.directed.is_some())
            else {
                return Ok(None);
            };
            let Some(versions) = &context.versions else {
                return Ok(None);
            };
            let source = root.source;
            let result = versions
                .validate_live(source.region.allocation, &source.record)
                .and_then(|allocation| {
                    versions
                        .journal
                        .lookup_allocation(allocation)
                        .map(|state| (allocation, state))
                });
            let (allocation, state) = context.journal_result_v1(result)?;
            let Some(writer) = state.pending_writer else {
                return Ok(None);
            };
            let result = context
                .versions
                .as_ref()
                .expect("configured journal")
                .retained_writer(writer);
            if !matches!(
                context.journal_result_v1(result)?,
                fe2o3_runtime_model::ContextWriterStateV1::Pending { .. }
            ) {
                return Err(RuntimeValidationErrorV1::ContextReserved);
            }
            let dependency = root
                .dependencies
                .iter()
                .find(|dependency| {
                    writer.key.kind == ContextWriterKindV1::Submission
                        && writer.key.context_generation == dependency.submission.context_generation
                        && writer.key.local == dependency.submission.local
                })
                .copied()
                .ok_or(RuntimeValidationErrorV1::ContextReserved)?;
            let producer = context
                .scalar_peer_copies
                .get(&dependency.submission)
                .filter(|producer| producer.directed.is_some())
                .ok_or(RuntimeValidationErrorV1::ContextReserved)?;
            let covered = producer.destination;
            if covered.region.allocation != source.region.allocation
                || covered.record != source.record
                || source.region.byte_offset < covered.region.byte_offset
                || source
                    .region
                    .byte_offset
                    .checked_add(source.region.byte_len)
                    .zip(
                        covered
                            .region
                            .byte_offset
                            .checked_add(covered.region.byte_len),
                    )
                    .is_none_or(|(end, producer_end)| end > producer_end)
            {
                return Err(RuntimeValidationErrorV1::ContextReserved);
            }
            let result = context.validate_pending_peer_copy_roots_v1(dependency.submission);
            context.journal_result_v1(result)?;
            if context.submissions[&dependency.submission].journal_writer != Some(writer) {
                return Err(RuntimeValidationErrorV1::ContextReserved);
            }
            let request = ContextProducerReadV1 {
                read: ContextAllocationReadV1 {
                    allocation,
                    device: state.device,
                    byte_extent: state.byte_extent,
                    byte_offset: source.region.byte_offset,
                    byte_len: source.region.byte_len,
                    attempt_epoch: state.attempt_epoch,
                    content_lineage: state.content_lineage,
                },
                producer: writer,
            };
            let versions = context.versions.as_ref().expect("configured journal");
            let result = versions
                .journal
                .validate_producer_read_capacity(1)
                .and_then(|()| versions.journal.validate_producer_read(&request));
            match result {
                Err(
                    ContextVersionJournalErrorV1::MemberCapacity
                    | ContextVersionJournalErrorV1::EpochExhausted,
                ) => return Err(RuntimeValidationErrorV1::Capacity),
                Err(ContextVersionJournalErrorV1::AllocationBusy) => {
                    return Err(RuntimeValidationErrorV1::ContextReserved);
                }
                result => context.journal_result_v1(result)?,
            }
            context
                .versions
                .as_mut()
                .expect("configured journal")
                .producer_readers
                .try_reserve(1)
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            Ok(Some(RetainedProducerReadV1 {
                source,
                dependency,
                request,
                reference: None,
            }))
        })
    }

    pub(super) fn begin_producer_read_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        prepared: Option<RetainedProducerReadV1>,
    ) -> Result<Option<ContextProducerReadReferenceV1>, RuntimeValidationErrorV1> {
        let Some(root) = prepared else {
            return Ok(None);
        };
        self.guard_journal_unwind_v1(|context| {
            let result = (|| {
                let versions = context
                    .versions
                    .as_mut()
                    .ok_or(ContextVersionJournalErrorV1::InvalidState)?;
                if versions.producer_readers.contains_key(&id)
                    || versions.submission_readers.contains_key(&id)
                    || versions
                        .submission_writers
                        .get(&id)
                        .is_none_or(|root| root.domain != SubmissionWriterDomainV1::Ordinary)
                {
                    return Err(ContextVersionJournalErrorV1::InvalidReference);
                }
                assert!(
                    versions.producer_readers.len() < versions.producer_readers.capacity(),
                    "preallocated producer root"
                );
                versions.producer_readers.insert(id, root);
                let root = versions
                    .producer_readers
                    .get_mut(&id)
                    .expect("retained producer input");
                let consumer = ContextWriterKeyV1 {
                    context_generation: id.context_generation,
                    local: id.local,
                    kind: ContextWriterKindV1::Submission,
                };
                let mut output = [None];
                versions
                    .journal
                    .acquire_producer_reads(consumer, &[root.request], &mut output)?;
                root.reference = Some(output[0].expect("complete producer reservation"));
                Ok(root.reference)
            })();
            context.journal_result_v1(result)
        })
    }

    pub(super) fn validate_producer_read_v1(
        &self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<Option<ContextProducerReadStatusV1>, ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        let record = self.submissions.get(&id);
        let expected = record.and_then(|record| record.journal_producer_read);
        let absent = if expected.is_some() {
            Err(E::InvalidReference)
        } else {
            Ok(None)
        };
        let Some(versions) = &self.versions else {
            return absent;
        };
        let Some(root) = versions.producer_readers.get(&id) else {
            return absent;
        };
        let reference = root.reference.ok_or(E::InvalidReference)?;
        let peer = self
            .scalar_peer_copies
            .get(&id)
            .ok_or(E::InvalidReference)?;
        let source = root.source;
        let read = root.request.read;
        if versions.submission_readers.contains_key(&id)
            || record.is_some_and(|record| {
                !record.directed_peer_copy
                    || record.journal_read.is_some()
                    || expected != Some(reference)
                    || record.journal_writer
                        != versions.submission_writers.get(&id).map(|root| root.writer)
            })
            || peer.directed.is_none()
            || !peer.dependencies_held
            || peer.source.region != source.region
            || peer.source.record != source.record
            || !peer.dependencies.contains(&root.dependency)
            || reference.consumer
                != (ContextWriterKeyV1 {
                    context_generation: id.context_generation,
                    local: id.local,
                    kind: ContextWriterKindV1::Submission,
                })
            || root.request.producer.key
                != (ContextWriterKeyV1 {
                    context_generation: root.dependency.submission.context_generation,
                    local: root.dependency.submission.local,
                    kind: ContextWriterKindV1::Submission,
                })
            || root.dependency.submission.local >= id.local
            || versions
                .submission_writers
                .get(&id)
                .is_none_or(|root| root.domain != SubmissionWriterDomainV1::Ordinary)
            || self.allocations.get(&source.region.allocation) != Some(&source.record)
            || !self
                .backend_allocations
                .contains(&source.record.backend_allocation)
            || !self
                .allocation_admission
                .has_expected_credit(source.region.allocation, source.record.device)
            || versions.validate_live(source.region.allocation, &source.record)? != read.allocation
            || read.device
                != enrollment(
                    source.region.allocation,
                    source.record.device,
                    source.record.byte_len,
                )
                .device
            || read.byte_extent != source.record.byte_len
            || read.byte_offset != source.region.byte_offset
            || read.byte_len != source.region.byte_len
            || versions.journal.lookup_producer_read(reference)? != root.request
        {
            return Err(E::InvalidReference);
        }
        // Resolved reservations outlive their writer slot. Never revalidate admission here.
        Ok(Some(versions.journal.producer_read_status(reference)?))
    }

    pub(in crate::context) fn directed_input_status_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<Option<ContextProducerReadStatusV1>, RuntimeValidationErrorV1> {
        let result = self.validate_producer_read_v1(id);
        self.journal_result_v1(result)
    }

    pub(in crate::context) fn release_submission_inputs_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        self.require_ordinary_submission_v1(id)?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.validate_submission_readers_v1(id, SubmissionWriterDomainV1::Ordinary)?;
            self.validate_producer_read_v1(id)
        }));
        let producer = match result {
            Ok(result) => self.journal_result_v1(result)?.is_some(),
            Err(payload) => {
                self.quarantine_after_async_command_panic_v1();
                core::mem::forget(payload);
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
            }
        };
        if !producer {
            return self.release_submission_readers_v1(id);
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            let versions = self.versions.as_mut().expect("validated producer input");
            let reference = versions.producer_readers[&id]
                .reference
                .expect("validated reservation");
            let consumer = reference.consumer;
            versions.journal.release_producer_reads(
                consumer,
                &[reference],
                &ContextReadQuiescenceEvidenceV1 { consumer },
            )?;
            versions.producer_readers.remove(&id);
            if let Some(record) = self.submissions.get_mut(&id) {
                record.journal_producer_read = None;
            }
            Ok(())
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
}
