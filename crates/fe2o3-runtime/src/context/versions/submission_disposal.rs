//! Per-allocation disposal receipts compose only at the complete Unknown roster.

use super::*;
use fe2o3_runtime_model::{
    ContextAllocationWriteV1, ContextWriterDisposalEvidenceV1, ContextWriterReferenceV1,
    ContextWriterStateV1,
};

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    pub(super) fn validate_disposal_submission_v1(
        &self,
        id: RuntimeSubmissionIdV1,
        writer: ContextWriterReferenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        if self
            .submissions
            .get(&id)
            .is_some_and(|record| !record.quiescent || record.journal_writer != Some(writer))
        {
            return Err(ContextVersionJournalErrorV1::InvalidReference);
        }
        Ok(())
    }

    pub(super) fn prepare_submission_allocation_disposal_v1(
        &mut self,
        allocation: RuntimeAllocationIdV1,
        record: AllocationRecordV1,
        member: ContextAllocationWriteV1,
        writer: ContextWriterReferenceV1,
    ) -> Result<AllocationDisposalV1, ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        let id = RuntimeSubmissionIdV1::new(writer.key.context_generation, writer.key.local);
        self.validate_disposal_submission_v1(id, writer)?;
        let versions = self.versions.as_mut().ok_or(E::InvalidState)?;
        let root = versions
            .submission_writers
            .get(&id)
            .ok_or(E::InvalidReference)?;
        if root.writer != writer
            || root.domain != SubmissionWriterDomainV1::Ordinary
            || root.copy_source.is_some()
            || root.journal_disposed
            || root.allocations.len() != root.members.len()
            || root.disposed_count >= root.allocations.len()
            || versions.retained_writer(writer)?
                != (ContextWriterStateV1::Unknown {
                    member_count: root.allocations.len(),
                })
        {
            return Err(E::InvalidState);
        }
        let index = root
            .allocations
            .binary_search_by_key(&allocation, |entry| entry.id)
            .map_err(|_| E::InvalidAllocationReference)?;
        let entry = &root.allocations[index];
        if entry.disposed || entry.record != record || root.members[index] != member {
            return Err(E::InvalidAllocationReference);
        }
        if !self
            .backend_allocations
            .contains(&record.backend_allocation)
            || !self
                .allocation_admission
                .has_expected_credit(allocation, record.device)
        {
            return Err(E::InvalidAllocationReference);
        }
        if !root.disposal_started {
            // Freeze the complete original roster before the first owner
            // release. Later per-ID releases cannot mutate its identities.
            if root.disposed_count != 0 {
                return Err(E::InvalidState);
            }
            versions
                .journal
                .validate_unknown_disposal(writer, &root.members)?;
            for (entry, member) in root.allocations.iter().zip(&root.members) {
                if entry.disposed
                    || self.allocations.get(&entry.id) != Some(&entry.record)
                    || !self
                        .backend_allocations
                        .contains(&entry.record.backend_allocation)
                    || versions.whole_allocation(entry.id, &entry.record)? != *member
                    || !self
                        .allocation_admission
                        .has_expected_credit(entry.id, entry.record.device)
                {
                    return Err(E::InvalidAllocationReference);
                }
            }
            versions
                .submission_writers
                .get_mut(&id)
                .expect("validated root")
                .disposal_started = true;
        }
        Ok(AllocationDisposalV1 {
            id: allocation,
            record,
            member,
            kind: AllocationDisposalKindV1::Submission { id, index },
        })
    }

    pub(super) fn finish_submission_allocation_disposal_v1(
        &mut self,
        plan: AllocationDisposalV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        let AllocationDisposalKindV1::Submission { id, index } = plan.kind else {
            return Err(E::InvalidState);
        };
        let versions = self.versions.as_mut().ok_or(E::InvalidState)?;
        let root = versions
            .submission_writers
            .get_mut(&id)
            .ok_or(E::InvalidReference)?;
        let entry = root.allocations.get_mut(index).ok_or(E::InvalidState)?;
        if !root.disposal_started
            || root.journal_disposed
            || entry.disposed
            || entry.id != plan.id
            || entry.record != plan.record
            || root.members.get(index) != Some(&plan.member)
            || root.disposed_count >= root.members.len()
            || versions.phases.get(plan.member.allocation.slot)
                != Some(&Some(AllocationPhaseV1::Live))
        {
            return Err(E::InvalidState);
        }

        // Record native success before any fallible whole-writer settlement.
        // Removing this exact handle here prevents both use and double release.
        entry.disposed = true;
        root.disposed_count += 1;
        versions.phases[plan.member.allocation.slot] = Some(AllocationPhaseV1::Disposed);
        let removed = self.allocations.remove(&plan.id);
        let indexed = self
            .backend_allocations
            .remove(&plan.record.backend_allocation);
        if removed != Some(plan.record) || !indexed {
            return Err(E::InvalidAllocationReference);
        }
        if root.disposed_count != root.members.len() {
            return Ok(());
        }
        self.commit_submission_writer_disposal_v1(id)
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
                || !self
                    .allocation_admission
                    .has_expected_credit(entry.id, entry.record.device)
            {
                return Err(E::InvalidState);
            }
        }
        versions.journal.dispose_unknown(
            writer,
            &ContextWriterDisposalEvidenceV1 {
                writer,
                allocations: &root.members,
            },
        )?;
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
