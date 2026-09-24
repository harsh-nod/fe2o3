use super::*;
use fe2o3_runtime_model::{
    ContextAllocationWriteV1, ContextWriterKeyV1, ContextWriterKindV1,
    ContextWriterNoEffectEvidenceV1, ContextWriterReferenceV1, ContextWriterSuccessEvidenceV1,
};

// Context owns this root independently of a returned backend/public handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::context) enum SubmissionWriterDomainV1 {
    Ordinary,
    Generated {
        stream: RuntimeStreamIdV1,
        hold: u64,
        shell_key: u64,
    },
}

pub(super) struct RetainedSubmissionWriterV1 {
    pub(super) writer: ContextWriterReferenceV1,
    pub(super) domain: SubmissionWriterDomainV1,
    pub(super) allocations: Vec<SubmissionWriterAllocationV1>,
    pub(super) members: Vec<ContextAllocationWriteV1>,
    pub(super) disposal_started: bool,
    pub(super) disposed_count: usize,
    pub(super) journal_disposed: bool,
}

pub(super) struct SubmissionWriterAllocationV1 {
    pub(super) id: RuntimeAllocationIdV1,
    pub(super) record: AllocationRecordV1,
    pub(super) disposed: bool,
}

pub(in crate::context) struct PreparedSubmissionWriterV1 {
    allocations: Vec<SubmissionWriterAllocationV1>,
    members: Vec<ContextAllocationWriteV1>,
}

#[derive(Clone, Copy)]
pub(in crate::context) enum SubmissionWriterOutcomeV1 {
    Success,
    NoEffect,
    Unknown,
}

#[cfg(test)]
impl ContextVersionsV1 {
    pub(in crate::context) fn submission_disposal_member_for_test_v1(
        &self,
        id: RuntimeSubmissionIdV1,
        index: usize,
    ) -> (
        RuntimeAllocationIdV1,
        AllocationRecordV1,
        ContextAllocationWriteV1,
        bool,
    ) {
        let root = &self.submission_writers[&id];
        let allocation = &root.allocations[index];
        (
            allocation.id,
            allocation.record,
            root.members[index],
            allocation.disposed,
        )
    }

    pub(in crate::context) fn submission_disposal_progress_for_test_v1(
        &self,
        id: RuntimeSubmissionIdV1,
    ) -> Option<(usize, bool)> {
        self.submission_writers
            .get(&id)
            .map(|root| (root.disposed_count, root.journal_disposed))
    }

    pub(in crate::context) fn clear_disposal_phase_for_test_v1(
        &mut self,
        reference: ContextAllocationReferenceV1,
    ) {
        self.phases[reference.slot] = None;
    }

    pub(in crate::context) fn remove_submission_writer_root_for_test_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) {
        self.submission_writers.remove(&id);
    }
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    pub(in crate::context) fn validate_pending_peer_copy_roots_v1(
        &self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        use fe2o3_runtime_model::ContextWriterStateV1;
        let record = self.submissions.get(&id).ok_or(E::InvalidReference)?;
        self.validate_scalar_peer_custody_v1(id)
            .map_err(|_| E::InvalidReference)?;
        if !record.scalar_peer_copy {
            return Err(E::InvalidReference);
        }
        if record.quiescent || record.status != RuntimeCompletionStatusV1::Pending {
            return Err(E::InvalidState);
        }
        let Some(versions) = self.versions.as_ref() else {
            return if record.journal_writer.is_none()
                && record.journal_read.is_none()
                && record.journal_producer_read.is_none()
            {
                Ok(())
            } else {
                Err(E::InvalidReference)
            };
        };
        let readers =
            self.validate_submission_readers_v1(id, SubmissionWriterDomainV1::Ordinary)?;
        let producer = self.validate_producer_read_v1(id)?;
        let root = versions
            .submission_writers
            .get(&id)
            .ok_or(E::InvalidReference)?;
        let writer = root.writer;
        if !matches!(
            (readers.map(|readers| readers.sources.len()), producer),
            (Some(1), None) | (None, Some(_))
        ) || record.journal_writer != Some(writer)
            || root.domain != SubmissionWriterDomainV1::Ordinary
            || root.disposal_started
            || root.disposed_count != 0
            || root.journal_disposed
            || root.allocations.len() != 1
            || root.members.len() != 1
            || writer.key.context_generation != id.context_generation
            || writer.key.local != id.local
            || writer.key.kind != ContextWriterKindV1::Submission
            || versions.retained_writer(writer)?
                != (ContextWriterStateV1::Pending { member_count: 1 })
        {
            return Err(E::InvalidReference);
        }
        let allocation = &root.allocations[0];
        let member = root.members[0];
        if allocation.disposed
            || self.scalar_peer_copies.get(&id).is_none_or(|peer| {
                peer.destination.region.allocation != allocation.id
                    || peer.destination.record != allocation.record
            })
            || self.allocations.get(&allocation.id) != Some(&allocation.record)
            || !self
                .backend_allocations
                .contains(&allocation.record.backend_allocation)
            || !self
                .allocation_admission
                .has_expected_credit(allocation.id, allocation.record.device)
            || versions.whole_allocation(allocation.id, &allocation.record)? != member
            || versions
                .journal
                .lookup_allocation(member.allocation)?
                .pending_writer
                != Some(writer)
        {
            return Err(E::InvalidAllocationReference);
        }
        Ok(())
    }

    pub(in crate::context) fn prepare_submission_writer_v1(
        &mut self,
        destinations: &[RuntimeAllocationIdV1],
    ) -> Result<Option<PreparedSubmissionWriterV1>, RuntimeValidationErrorV1> {
        self.guard_journal_unwind_v1(|context| {
            let Some(versions) = context.versions.as_ref() else {
                return Ok(None);
            };
            if destinations.is_empty() {
                return Ok(None);
            }
            if versions.journal.remaining_writer_slots() == 0 {
                return Err(RuntimeValidationErrorV1::Capacity);
            }
            let mut canonical = Vec::new();
            canonical
                .try_reserve_exact(destinations.len())
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            canonical.extend_from_slice(destinations);
            canonical.sort_unstable();
            canonical.dedup();
            let mut allocations = Vec::new();
            allocations
                .try_reserve_exact(canonical.len())
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            let mut members = Vec::new();
            members
                .try_reserve_exact(canonical.len())
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            for id in canonical {
                let record = *context
                    .allocations
                    .get(&id)
                    .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?;
                let result = context
                    .versions
                    .as_ref()
                    .expect("configured journal")
                    .whole_allocation(id, &record);
                let member = context.journal_result_v1(result)?;
                let result = context
                    .versions
                    .as_ref()
                    .expect("configured journal")
                    .journal
                    .reader_count(member.allocation);
                if context.journal_result_v1(result)? != 0 {
                    return Err(RuntimeValidationErrorV1::ContextReserved);
                }
                let result = context
                    .versions
                    .as_ref()
                    .expect("configured journal")
                    .journal
                    .lookup_allocation(member.allocation);
                let state = context.journal_result_v1(result)?;
                if let Some(writer) = state.pending_writer {
                    let result = context
                        .versions
                        .as_ref()
                        .expect("configured journal")
                        .retained_writer(writer);
                    context.journal_result_v1(result)?;
                    return Err(RuntimeValidationErrorV1::ContextReserved);
                }
                if state.attempt_epoch == u64::MAX {
                    return Err(RuntimeValidationErrorV1::Capacity);
                }
                members.push(member);
                allocations.push(SubmissionWriterAllocationV1 {
                    id,
                    record,
                    disposed: false,
                });
            }
            context
                .versions
                .as_mut()
                .expect("configured journal")
                .submission_writers
                .try_reserve(1)
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            Ok(Some(PreparedSubmissionWriterV1 {
                allocations,
                members,
            }))
        })
    }

    pub(in crate::context) fn begin_submission_writer_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        prepared: Option<PreparedSubmissionWriterV1>,
        domain: SubmissionWriterDomainV1,
    ) -> Result<Option<ContextWriterReferenceV1>, RuntimeValidationErrorV1> {
        self.guard_journal_unwind_v1(|context| {
            let Some(prepared) = prepared else {
                return Ok(None);
            };
            let key = ContextWriterKeyV1 {
                context_generation: id.context_generation,
                local: id.local,
                kind: ContextWriterKindV1::Submission,
            };
            let result = context
                .versions
                .as_mut()
                .expect("configured journal")
                .journal
                .register_writer(key);
            let writer = context.journal_result_v1(result)?;
            let versions = context.versions.as_mut().expect("configured journal");
            assert!(
                !versions.submission_writers.contains_key(&id),
                "fresh submission writer identity"
            );
            assert!(
                versions.submission_writers.len() < versions.submission_writers.capacity(),
                "preallocated submission writer root"
            );
            versions.submission_writers.insert(
                id,
                RetainedSubmissionWriterV1 {
                    writer,
                    domain,
                    allocations: prepared.allocations,
                    members: prepared.members,
                    disposal_started: false,
                    disposed_count: 0,
                    journal_disposed: false,
                },
            );
            let result = versions
                .journal
                .begin_write(writer, &versions.submission_writers[&id].members);
            if result.is_err() {
                let abort = versions.journal.abort_reserved(writer);
                context.journal_result_v1(abort)?;
                context
                    .versions
                    .as_mut()
                    .expect("configured journal")
                    .submission_writers
                    .remove(&id);
            }
            context.journal_result_v1(result)?;
            Ok(Some(writer))
        })
    }

    pub(in crate::context) fn settle_submission_writer_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        outcome: SubmissionWriterOutcomeV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        self.require_ordinary_submission_v1(id)?;
        if self.versions.as_ref().is_some_and(|versions| {
            versions
                .submission_writers
                .get(&id)
                .is_some_and(|root| root.domain != SubmissionWriterDomainV1::Ordinary)
        }) {
            return Err(RuntimeValidationErrorV1::ContextReserved);
        }
        self.settle_writer_v1(id, SubmissionWriterDomainV1::Ordinary, outcome)
    }

    pub(super) fn settle_writer_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        domain: SubmissionWriterDomainV1,
        outcome: SubmissionWriterOutcomeV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let record = self.submissions.get(&id);
            let expected = record.and_then(|record| record.journal_writer);
            let absent = if expected.is_some() {
                Err(ContextVersionJournalErrorV1::InvalidReference)
            } else {
                Ok(())
            };
            let Some(versions) = self.versions.as_mut() else {
                return absent;
            };
            if versions.submission_readers.contains_key(&id)
                || versions.producer_readers.contains_key(&id)
                || record.is_some_and(|record| record.journal_read.is_some())
                || record.is_some_and(|record| record.journal_producer_read.is_some())
            {
                return Err(ContextVersionJournalErrorV1::InvalidState);
            }
            let Some(root) = versions.submission_writers.get(&id) else {
                return absent;
            };
            let writer = root.writer;
            if root.domain != domain {
                return Err(ContextVersionJournalErrorV1::InvalidReference);
            }
            if record.is_some() && expected != Some(writer) {
                return Err(ContextVersionJournalErrorV1::InvalidReference);
            }
            if writer.key.context_generation != id.context_generation
                || writer.key.local != id.local
                || writer.key.kind != ContextWriterKindV1::Submission
            {
                return Err(ContextVersionJournalErrorV1::InvalidReference);
            }
            match outcome {
                SubmissionWriterOutcomeV1::Success => versions
                    .journal
                    .settle_success(writer, &ContextWriterSuccessEvidenceV1 { writer })?,
                SubmissionWriterOutcomeV1::NoEffect => versions
                    .journal
                    .settle_no_effect(writer, &ContextWriterNoEffectEvidenceV1 { writer })?,
                SubmissionWriterOutcomeV1::Unknown => versions.journal.mark_unknown(writer)?,
            }
            if !matches!(outcome, SubmissionWriterOutcomeV1::Unknown) {
                versions.submission_writers.remove(&id);
                if let Some(record) = self.submissions.get_mut(&id) {
                    record.journal_writer = None;
                }
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

    pub(in crate::context) fn quarantine_submission_writers_v1(&mut self) {
        self.terminal = true;
        let Some(versions) = self.versions.as_mut() else {
            return;
        };
        for root in versions.submission_writers.values() {
            // Even a secondary poison/settlement panic must preserve the
            // backend's original diagnostic and all remaining roots.
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
                let _ = versions.journal.mark_unknown(root.writer);
            })) {
                core::mem::forget(payload);
            }
            for allocation in &root.allocations {
                if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
                    self.allocation_admission.quarantine(allocation.id);
                })) {
                    core::mem::forget(payload);
                }
            }
        }
        for root in versions.submission_readers.values() {
            for source in &root.sources {
                if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
                    self.allocation_admission
                        .quarantine(source.region.allocation);
                })) {
                    core::mem::forget(payload);
                }
            }
        }
        for root in versions.producer_readers.values() {
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
                self.allocation_admission
                    .quarantine(root.source.region.allocation);
            })) {
                core::mem::forget(payload);
            }
        }
    }

    pub(in crate::context) fn invoke_journal_backend_v1<T>(
        &mut self,
        call: impl FnOnce(&mut B) -> Result<T, RuntimeBackendFailureV1<B::Error>>,
    ) -> Result<T, RuntimeBackendFailureV1<B::Error>> {
        if self.versions.is_none() && self.scalar_peer_copies.is_empty() {
            return call(&mut self.backend);
        }
        match catch_unwind(AssertUnwindSafe(|| call(&mut self.backend))) {
            Ok(result) => result,
            Err(payload) => {
                self.quarantine_after_async_command_panic_v1();
                std::panic::resume_unwind(payload);
            }
        }
    }

    pub(in crate::context) fn submit_context_operation_v1<M>(
        &mut self,
        stream: RuntimeStreamIdV1,
        stream_record: StreamRecordV1,
        destinations: &[RuntimeAllocationIdV1],
        peer: Option<PreparedPeerSubmissionV1>,
        sources: &[ContextReadSourceV1],
        submit: impl FnOnce(&mut B) -> Result<u64, RuntimeBackendFailureV1<B::Error>>,
    ) -> Result<RuntimeSubmissionV1<M>, RuntimeErrorV1<B::Error>> {
        let prepared = self.prepare_submission_writer_v1(destinations)?;
        let producer = self.prepare_producer_read_v1(peer.as_ref())?;
        let reads = if producer.is_some() {
            None
        } else {
            self.prepare_submission_readers_v1(sources)?
        };
        self.submissions
            .try_reserve(1)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        self.backend_submissions
            .try_reserve(1)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        let id = RuntimeSubmissionIdV1::new(self.context_generation, self.next_id()?);
        let journal_writer =
            self.begin_submission_writer_v1(id, prepared, SubmissionWriterDomainV1::Ordinary)?;
        let journal_read =
            self.begin_submission_readers_v1(id, reads, SubmissionWriterDomainV1::Ordinary)?;
        let journal_producer_read = self.begin_producer_read_v1(id, producer)?;
        let peer_transfer = peer.as_ref().map(|peer| peer.mechanism);
        let scalar_peer_copy = peer.as_ref().is_some_and(|peer| peer.scalar.is_some());
        let directed_peer_copy = peer
            .as_ref()
            .and_then(|peer| peer.scalar.as_ref())
            .is_some_and(|root| root.directed.is_some());
        if let Some(root) = peer.and_then(|peer| peer.scalar) {
            self.begin_scalar_peer_custody_v1(id, root);
        }
        let result = self.invoke_journal_backend_v1(submit);
        let backend_submission = match result {
            Ok(handle) => handle,
            Err(failure) => {
                if matches!(&failure, RuntimeBackendFailureV1::Terminal(_)) {
                    return self.backend_result(Err(failure));
                }
                let outcome = if matches!(&failure, RuntimeBackendFailureV1::Rejected(_)) {
                    SubmissionWriterOutcomeV1::NoEffect
                } else {
                    SubmissionWriterOutcomeV1::Unknown
                };
                if self.check_scalar_peer_custody_v1(id).is_ok()
                    && self.release_submission_inputs_v1(id).is_ok()
                    && self.settle_submission_writer_v1(id, outcome).is_ok()
                    && self.release_scalar_peer_dependencies_v1(id).is_ok()
                {
                    self.scalar_peer_copies.remove(&id);
                }
                return self.backend_result(Err(failure));
            }
        };
        let protocol_error = self.backend_handle_protocol_error(
            RuntimeBackendResourceKindV1::Submission,
            backend_submission,
        );
        // Both indexes have headroom before backend entry. Root the returned
        // handle even if the backend violated zero/duplicate-handle rules.
        if let Some(root) = self.scalar_peer_copies.get_mut(&id) {
            root.backend_submission = Some(backend_submission);
        }
        self.submissions.insert(
            id,
            SubmissionRecordV1 {
                backend_submission,
                stream,
                device: stream_record.device,
                quiescent: false,
                status: RuntimeCompletionStatusV1::Pending,
                journal_writer,
                journal_read,
                journal_producer_read,
                scalar_peer_copy,
                directed_peer_copy,
                dependency_retains: 0,
            },
        );
        if protocol_error.is_none() {
            self.backend_submissions.insert(backend_submission);
        }
        let submission = RuntimeSubmissionV1 {
            id,
            backend_submission,
            stream,
            device: stream_record.device,
            completion: None,
            peer_transfer,
            marker: PhantomData,
        };
        self.seal_backend_protocol(protocol_error, submission)
    }
}
