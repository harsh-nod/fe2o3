//! Shared submission identity checks and Generated batch disposal.

use super::*;
use fe2o3_runtime_model::{ContextWriterDisposalEvidenceV1, ContextWriterReferenceV1};

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    pub(super) fn validate_disposal_submission_v1(
        &self,
        id: RuntimeSubmissionIdV1,
        writer: ContextWriterReferenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        let root = self
            .versions
            .as_ref()
            .and_then(|versions| versions.submission_writers.get(&id))
            .ok_or(ContextVersionJournalErrorV1::InvalidReference)?;
        if root.writer != writer
            || (root.domain == SubmissionWriterDomainV1::Ordinary && !root.disposal_quiescent)
        {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        if self.submissions.get(&id).is_some_and(|record| {
            !record.quiescent
                || record.journal_writer != Some(writer)
                || record.journal_read.is_some()
                || record.journal_producer_read.is_some()
        }) || self.versions.as_ref().is_some_and(|versions| {
            versions.submission_readers.contains_key(&id)
                || versions.producer_readers.contains_key(&id)
        }) {
            return Err(ContextVersionJournalErrorV1::InvalidReference);
        }
        Ok(())
    }

    pub(super) fn commit_submission_writer_disposal_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        let root = self
            .versions
            .as_ref()
            .ok_or(E::InvalidState)?
            .submission_writers
            .get(&id)
            .ok_or(E::InvalidReference)?;
        if root.journal_disposed
            || root.disposed_count != root.members.len()
            || root.allocations.len() != root.members.len()
        {
            return Err(E::InvalidState);
        }
        let writer = root.writer;
        self.validate_disposal_submission_v1(id, writer)?;
        let versions = self.versions.as_mut().expect("configured journal");
        let root = versions
            .submission_writers
            .get_mut(&id)
            .expect("retained root");
        for (entry, member) in root.allocations.iter().zip(&root.members) {
            if !entry.disposed
                || self.allocations.contains_key(&entry.id)
                || versions.phases.get(member.allocation.slot)
                    != Some(&Some(AllocationPhaseV1::Disposed))
                || !self.allocation_admission.has_expected_credit(
                    entry.id,
                    entry.record.device,
                    entry.record.byte_len,
                )
            {
                return Err(E::InvalidState);
            }
        }
        let evidence = ContextWriterDisposalEvidenceV1 {
            writer,
            allocations: &root.members,
        };
        if !matches!(root.domain, SubmissionWriterDomainV1::Generated { .. })
            || root.disposal_group.is_some()
        {
            return Err(E::InvalidState);
        }
        versions.journal.dispose_unknown(writer, &evidence)?;
        root.journal_disposed = true;
        for member in &root.members {
            versions.phases[member.allocation.slot] = None;
        }
        let count = root.allocations.len();
        for index in 0..count {
            let allocation = self
                .versions
                .as_ref()
                .expect("configured journal")
                .submission_writers[&id]
                .allocations[index]
                .id;
            self.dispose_allocation_credits_v1(allocation);
        }
        if let Some(record) = self.submissions.get_mut(&id) {
            record.journal_writer = None;
        }
        self.versions
            .as_mut()
            .expect("configured journal")
            .submission_writers
            .remove(&id)
            .ok_or(E::InvalidReference)?;
        Ok(())
    }
}
