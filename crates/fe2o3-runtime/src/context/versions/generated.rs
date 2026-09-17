//! Protected completion and whole-shell disposal use their own writer domain.

use super::*;
use crate::generated_source::GeneratedHostRosterV1;
use crate::kfd_backend::GeneratedShellPlanV1;
use fe2o3_runtime_model::{ContextWriterKindV1, ContextWriterReferenceV1, ContextWriterStateV1};

pub(in crate::context) struct GeneratedShellRetirementV1 {
    plan: GeneratedShellPlanV1,
    writer: Option<RuntimeSubmissionIdV1>,
    references: [ContextAllocationReferenceV1; fe2o3_kfd::GFX942_MAX_FIXED_DISPATCH_DATA_V1],
    count: usize,
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    pub(in crate::context) fn validate_generated_writer_v1(
        &self,
        id: RuntimeSubmissionIdV1,
        domain: SubmissionWriterDomainV1,
        expected: Option<ContextWriterReferenceV1>,
        plan: &GeneratedShellPlanV1,
        roster: &GeneratedHostRosterV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        if domain
            != (SubmissionWriterDomainV1::Generated {
                stream: plan.binding.stream,
                hold: plan.binding.hold,
                shell_key: plan.key,
            })
            || plan.count != roster.count
        {
            return Err(E::InvalidReference);
        }
        if self
            .submissions
            .get(&id)
            .is_some_and(|record| record.journal_writer != expected)
        {
            return Err(E::InvalidReference);
        }
        let Some(versions) = self.versions.as_ref() else {
            return if expected.is_none() {
                Ok(())
            } else {
                Err(E::InvalidState)
            };
        };
        let writable =
            roster.buffers[..roster.count]
                .iter()
                .try_fold(0, |count, slot| {
                    let slot = slot.ok_or(E::InvalidReference)?;
                    Ok(count
                        + usize::from(slot.access != crate::Gfx942RuntimeBufferAccessV1::ReadOnly))
                })?;
        let Some(writer) = expected else {
            return if writable == 0 && !versions.submission_writers.contains_key(&id) {
                Ok(())
            } else {
                Err(E::InvalidReference)
            };
        };
        let root = versions
            .submission_writers
            .get(&id)
            .ok_or(E::InvalidReference)?;
        if root.writer != writer
            || root.domain != domain
            || root.disposal_started
            || root.disposed_count != 0
            || root.journal_disposed
            || root.allocations.len() != writable
            || root.members.len() != writable
            || writer.key.context_generation != id.context_generation
            || writer.key.local != id.local
            || writer.key.kind != ContextWriterKindV1::Submission
        {
            return Err(E::InvalidReference);
        }
        let count = match versions.retained_writer(writer)? {
            ContextWriterStateV1::Pending { member_count }
            | ContextWriterStateV1::Unknown { member_count } => member_count,
            _ => return Err(E::InvalidState),
        };
        if count != writable {
            return Err(E::InvalidState);
        }
        for (ordinal, (member, slot)) in plan.members[..plan.count]
            .iter()
            .zip(&roster.buffers[..roster.count])
            .enumerate()
        {
            let member = member.ok_or(E::InvalidAllocationReference)?;
            let slot = slot.ok_or(E::InvalidAllocationReference)?;
            if slot.ordinal != ordinal || slot.bytes != member.description.byte_len {
                return Err(E::InvalidAllocationReference);
            }
            if slot.access == crate::Gfx942RuntimeBufferAccessV1::ReadOnly {
                continue;
            }
            let index = root
                .allocations
                .binary_search_by_key(&member.logical, |entry| entry.id)
                .map_err(|_| E::InvalidAllocationReference)?;
            let entry = &root.allocations[index];
            if entry.disposed
                || entry.record.backend_allocation != member.backend
                || self.allocations.get(&entry.id) != Some(&entry.record)
                || !self.backend_allocations.contains(&member.backend)
                || !self
                    .allocation_admission
                    .has_expected_credit(entry.id, entry.record.device)
                || versions.whole_allocation(entry.id, &entry.record)? != root.members[index]
                || versions
                    .journal
                    .lookup_allocation(root.members[index].allocation)?
                    .pending_writer
                    != Some(writer)
            {
                return Err(E::InvalidAllocationReference);
            }
        }
        Ok(())
    }

    pub(in crate::context) fn settle_generated_submission_writer_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        domain: SubmissionWriterDomainV1,
        outcome: SubmissionWriterOutcomeV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        if !matches!(domain, SubmissionWriterDomainV1::Generated { .. })
            || matches!(outcome, SubmissionWriterOutcomeV1::NoEffect)
        {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
        }
        self.settle_writer_v1(id, domain, outcome)
    }

    pub(in crate::context) fn prepare_generated_shell_retirement_v1(
        &self,
        plan: GeneratedShellPlanV1,
    ) -> Result<GeneratedShellRetirementV1, ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        let mut ticket = GeneratedShellRetirementV1 {
            plan,
            writer: None,
            references: [ContextAllocationReferenceV1 {
                slot: 0,
                key: ContextAllocationKeyV1 {
                    context_generation: 0,
                    local: 0,
                },
            }; fe2o3_kfd::GFX942_MAX_FIXED_DISPATCH_DATA_V1],
            count: 0,
        };
        let attempt = self.generated_issues.get(&plan.binding.stream);
        if let Some(attempt) = attempt {
            let (id, domain) = attempt.writer_binding_v1();
            if let Some(writer) = attempt.expected_writer {
                let (original_plan, roster) = attempt.writer_roster_v1();
                if *original_plan != plan {
                    return Err(E::InvalidReference);
                }
                self.validate_generated_writer_v1(id, domain, Some(writer), &plan, roster)?;
                let versions = self.versions.as_ref().ok_or(E::InvalidState)?;
                let root = versions
                    .submission_writers
                    .get(&id)
                    .ok_or(E::InvalidReference)?;
                self.validate_disposal_submission_v1(id, writer)?;
                versions
                    .journal
                    .validate_unknown_disposal(writer, &root.members)?;
                ticket.writer = Some(id);
            } else if self
                .versions
                .as_ref()
                .is_some_and(|versions| versions.submission_writers.contains_key(&id))
            {
                return Err(E::InvalidReference);
            }
        }
        for (ordinal, member) in plan.members[..plan.count].iter().enumerate() {
            let member = member.ok_or(E::InvalidAllocationReference)?;
            let record = self
                .allocations
                .get(&member.logical)
                .ok_or(E::InvalidAllocationReference)?;
            if record.backend_allocation != member.backend
                || record.device != plan.binding.device
                || record.kind != member.description.kind
                || record.byte_len != member.description.byte_len
                || !self.backend_allocations.contains(&member.backend)
                || !self
                    .allocation_admission
                    .has_expected_credit(member.logical, record.device)
            {
                return Err(E::InvalidAllocationReference);
            }
            if let Some(versions) = self.versions.as_ref() {
                let reference = versions.validate_live(member.logical, record)?;
                let writer = ticket
                    .writer
                    .map(|id| versions.submission_writers[&id].writer);
                let pending = versions
                    .journal
                    .lookup_allocation(reference)?
                    .pending_writer;
                let writable = ticket.writer.is_some()
                    && attempt
                        .and_then(|attempt| attempt.writer_roster_v1().1.buffers[ordinal])
                        .is_some_and(|slot| {
                            slot.access != crate::Gfx942RuntimeBufferAccessV1::ReadOnly
                        });
                if pending != if writable { writer } else { None } {
                    return Err(E::InvalidAllocationReference);
                }
                if !writable {
                    ticket.references[ticket.count] = reference;
                    ticket.count += 1;
                }
            } else if record.journal.is_some() {
                return Err(E::InvalidState);
            }
        }
        if let Some(versions) = self.versions.as_ref() {
            versions.validate_retirement(&ticket.references[..ticket.count])?;
        }
        Ok(ticket)
    }

    pub(in crate::context) fn finish_generated_shell_retirement_v1(
        &mut self,
        ticket: GeneratedShellRetirementV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        use ContextVersionJournalErrorV1 as E;
        // The validated backend operation disposes the complete original batch.
        // Record all receipts before performing any fallible model/credit commit.
        if let Some(id) = ticket.writer {
            let versions = self.versions.as_mut().ok_or(E::InvalidState)?;
            let root = versions
                .submission_writers
                .get_mut(&id)
                .ok_or(E::InvalidReference)?;
            root.disposal_started = true;
            for (entry, member) in root.allocations.iter_mut().zip(&root.members) {
                entry.disposed = true;
                root.disposed_count += 1;
                versions.phases[member.allocation.slot] = Some(AllocationPhaseV1::Disposed);
                if self.allocations.remove(&entry.id) != Some(entry.record)
                    || !self
                        .backend_allocations
                        .remove(&entry.record.backend_allocation)
                {
                    return Err(E::InvalidAllocationReference);
                }
            }
            self.commit_submission_writer_disposal_v1(id)?;
        }
        if let Some(versions) = self.versions.as_mut() {
            versions.retire(&ticket.references[..ticket.count])?;
        }
        for member in ticket.plan.members[..ticket.plan.count].iter().flatten() {
            if self.allocations.contains_key(&member.logical) {
                self.dispose_allocation_credits_v1(member.logical);
                self.allocations.remove(&member.logical);
                self.backend_allocations.remove(&member.backend);
            }
        }
        if let Some(attempt) = self.generated_issues.get_mut(&ticket.plan.binding.stream) {
            attempt.expected_writer = None;
        }
        Ok(())
    }
}
