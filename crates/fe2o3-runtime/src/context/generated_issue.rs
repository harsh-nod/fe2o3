//! First-run mutation attempts and completion. No journal-backed reuse.

use super::*;
use crate::generated_source::GeneratedHostRosterV1;
use crate::kfd_backend::GeneratedShellPlanV1;
use crate::{KfdRuntimeBackendErrorV1, KfdRuntimeBackendV1, RuntimeGfx942GeneratedCarrierV1};

mod completion;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PhaseV1 {
    Entering,
    Active,
    PhysicallyComplete,
    DisposedWithoutResult,
    Unknown,
}

pub(super) struct GeneratedIssueV1 {
    id: RuntimeSubmissionIdV1,
    plan: GeneratedShellPlanV1,
    roster: GeneratedHostRosterV1,
    phase: PhaseV1,
    pub(super) expected_writer: Option<fe2o3_runtime_model::ContextWriterReferenceV1>,
    pub(super) expected_reader: Option<SubmissionReaderMarkerV1>,
    // The original hold/carrier remain in the owner-only async registry. Owned
    // shutdown retains that registry and this Context together on any failure.
    submission: Option<RuntimeSubmissionV1<()>>,
}

impl GeneratedIssueV1 {
    pub(super) fn owns_submission_v1(&self, id: RuntimeSubmissionIdV1) -> bool {
        self.id == id
    }

    pub(super) fn writer_binding_v1(&self) -> (RuntimeSubmissionIdV1, SubmissionWriterDomainV1) {
        (self.id, generated_writer_domain_v1(&self.plan))
    }

    pub(super) fn writer_roster_v1(&self) -> (&GeneratedShellPlanV1, &GeneratedHostRosterV1) {
        (&self.plan, &self.roster)
    }
}

fn generated_writer_domain_v1(plan: &GeneratedShellPlanV1) -> SubmissionWriterDomainV1 {
    SubmissionWriterDomainV1::Generated {
        stream: plan.binding.stream,
        hold: plan.binding.hold,
        shell_key: plan.key,
    }
}

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    fn generated_issue_token_v1(
        &self,
        hold: &ContextUnpublishedHoldV1,
        plan: &GeneratedShellPlanV1,
    ) -> Result<&RuntimeSubmissionV1<()>, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.validate_unpublished_hold_v1(hold)?;
        let attempt = self
            .generated_issues
            .get(&hold.stream())
            .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?;
        let token = attempt
            .submission
            .as_ref()
            .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?;
        if attempt.plan != *plan
            || token.id != attempt.id
            || token.stream != hold.stream()
            || token.stream != plan.binding.stream
            || token.device != plan.binding.device
        {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
        }
        self.live_submission_record(token)?;
        self.validate_generated_writer_v1(
            attempt.id,
            generated_writer_domain_v1(plan),
            attempt.expected_writer,
            plan,
            &attempt.roster,
        )
        .map_err(|_| RuntimeValidationErrorV1::InvalidBackendDescription)?;
        self.validate_generated_readers_v1(
            attempt.id,
            generated_writer_domain_v1(plan),
            attempt.expected_reader,
            plan,
            &attempt.roster,
        )
        .map_err(|_| RuntimeValidationErrorV1::InvalidBackendDescription)?;
        Ok(token)
    }

    fn begin_generated_issue_v1(
        &mut self,
        hold: &ContextUnpublishedHoldV1,
        plan: GeneratedShellPlanV1,
        roster: &GeneratedHostRosterV1,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        if self.generated_plan_for_hold_v1(hold)? != plan
            || self.generated_issues.contains_key(&hold.stream())
            || plan.count != roster.count
            || plan.members[..plan.count]
                .iter()
                .enumerate()
                .any(|(index, member)| {
                    member.is_none_or(|member| {
                        roster.buffers[index].is_none_or(|slot| {
                            slot.ordinal != index || slot.bytes != member.description.byte_len
                        })
                    })
                })
        {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
        }
        if self.next_identity == 0 || self.submissions.len() >= MAX_RUNTIME_SUBMISSIONS_V1 {
            return Err(RuntimeValidationErrorV1::Capacity.into());
        }
        self.generated_issues
            .try_reserve(1)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        self.submissions
            .try_reserve(1)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        self.backend_submissions
            .try_reserve(1)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        let mut destinations =
            [RuntimeAllocationIdV1::new(0, 0); fe2o3_kfd::GFX942_MAX_FIXED_DISPATCH_DATA_V1];
        let mut count = 0;
        let mut sources = Vec::new();
        sources
            .try_reserve_exact(
                roster.buffers[..roster.count]
                    .iter()
                    .filter(|slot| {
                        slot.is_some_and(|slot| {
                            slot.access == crate::Gfx942RuntimeBufferAccessV1::ReadOnly
                        })
                    })
                    .count(),
            )
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        for (member, slot) in plan.members[..plan.count]
            .iter()
            .zip(&roster.buffers[..roster.count])
        {
            if slot.expect("validated roster").access
                != crate::Gfx942RuntimeBufferAccessV1::ReadOnly
            {
                destinations[count] = member.expect("validated plan").logical;
                count += 1;
            } else {
                let member = member.expect("validated plan");
                let record = self.allocations[&member.logical];
                sources.push(ContextReadSourceV1 {
                    region: RuntimeMemoryRegionV1 {
                        allocation: member.logical,
                        access: RuntimeAccessV1::Read,
                        byte_offset: 0,
                        byte_len: record.byte_len,
                    },
                    record,
                });
            }
        }
        sources.sort_unstable_by_key(|source| source.region.allocation);
        let prepared = self.prepare_submission_writer_v1(&destinations[..count])?;
        let readers = self.prepare_submission_readers_v1(&sources)?;
        let id = RuntimeSubmissionIdV1::new(self.context_generation, self.next_id()?);
        let domain = generated_writer_domain_v1(&plan);
        let expected_writer = self.begin_submission_writer_v1(id, prepared, domain)?;
        let expected_reader = self.begin_submission_readers_v1(id, readers, domain)?;
        self.generated_issues.insert(
            hold.stream(),
            GeneratedIssueV1 {
                id,
                plan,
                roster: roster.clone(),
                phase: PhaseV1::Entering,
                expected_writer,
                expected_reader,
                submission: None,
            },
        );
        Ok(())
    }

    fn install_generated_submission_v1(
        &mut self,
        hold: &ContextUnpublishedHoldV1,
        backend_submission: u64,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        let protocol = self.backend_handle_protocol_error(
            RuntimeBackendResourceKindV1::Submission,
            backend_submission,
        );
        let attempt = self
            .generated_issues
            .get_mut(&hold.stream())
            .expect("rooted mutation attempt");
        assert!(attempt.submission.is_none());
        let id = attempt.id;
        let device = attempt.plan.binding.device;
        let journal_writer = attempt.expected_writer;
        let journal_read = attempt.expected_reader;
        attempt.submission = Some(RuntimeSubmissionV1 {
            id,
            backend_submission,
            stream: hold.stream(),
            device,
            completion: None,
            peer_transfer: None,
            marker: PhantomData,
        });
        // Returned identity is rooted even when protocol validation rejects it.
        self.seal_backend_protocol(protocol, ())?;
        self.submissions.insert(
            id,
            SubmissionRecordV1 {
                scalar_peer_copy: false,
                directed_peer_copy: false,
                dependency_retains: 0,
                backend_submission,
                stream: hold.stream(),
                device,
                quiescent: false,
                status: RuntimeCompletionStatusV1::Pending,
                journal_writer,
                journal_read,
                journal_producer_read: None,
            },
        );
        assert!(self.backend_submissions.insert(backend_submission));
        Ok(())
    }

    pub(crate) fn progress_gfx942_issue_v1<T: RuntimeGfx942GeneratedCarrierV1>(
        &mut self,
        prepared: &mut RuntimeGfx942PreparedV1<T>,
        roster: &GeneratedHostRosterV1,
        hold: &ContextUnpublishedHoldV1,
    ) -> Result<bool, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.validate_unpublished_hold_v1(hold)?;
        let mut complete = false;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.validate_gfx942_prepared_v1(prepared)?;
            let plan = self.generated_plan_for_hold_v1(hold)?;
            if !self.gfx942_prepared_matches_plan_v1(prepared, &plan) {
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
            }
            let uid = self.generated_issue_device_uid_v1(plan.binding.backend_device)?;
            prepared
                .value()
                .source()
                .with_current_source_v1(
                    uid,
                    roster,
                    || -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
                        if let Some(attempt) = self.generated_issues.get(&hold.stream()) {
                            if attempt.plan != plan
                                || !attempt.roster.matches(roster)
                                || !matches!(
                                    attempt.phase,
                                    PhaseV1::Active | PhaseV1::PhysicallyComplete
                                )
                            {
                                return Err(
                                    RuntimeValidationErrorV1::InvalidBackendDescription.into()
                                );
                            }
                            self.generated_issue_token_v1(hold, &plan)?;
                            if attempt.phase == PhaseV1::PhysicallyComplete {
                                complete = true;
                                return Ok(());
                            }
                        } else {
                            self.begin_generated_issue_v1(hold, plan, roster)?;
                            let handle = self
                                .backend
                                .prepare_generated_issue_v1(&plan, roster)
                                .map_err(map_backend_error)?;
                            self.install_generated_submission_v1(hold, handle)?;
                        }
                        let attempt = self
                            .generated_issues
                            .get_mut(&hold.stream())
                            .expect("retained attempt");
                        // This is the first non-reusing mutation hook. It remains Unknown
                        // until closing authority checks succeed; physical completion
                        // alone never settles a successful output version.
                        attempt.phase = PhaseV1::Unknown;
                        let handle = attempt
                            .submission
                            .as_ref()
                            .expect("rooted token")
                            .backend_submission;
                        complete = self
                            .backend
                            .advance_generated_issue_v1(&plan, handle)
                            .map_err(map_backend_error)?;
                        Ok(())
                    },
                )
                .map_err(|_| {
                    RuntimeErrorV1::Validation(RuntimeValidationErrorV1::InvalidBackendDescription)
                })??;
            self.generated_issues
                .get_mut(&hold.stream())
                .expect("retained attempt")
                .phase = if complete {
                PhaseV1::PhysicallyComplete
            } else {
                PhaseV1::Active
            };
            Ok(())
        }));
        self.finish_generated_issue_v1(hold, result)?;
        Ok(complete)
    }

    // Called only after the async engine stops observers. It never retries ISSUE.
    pub(crate) fn retire_gfx942_issued_v1(
        &mut self,
        hold: &ContextUnpublishedHoldV1,
    ) -> Result<bool, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.validate_unpublished_hold_v1(hold)?;
        let mut retired = false;
        let result = catch_unwind(AssertUnwindSafe(|| {
            let plan = self.generated_plan_for_hold_v1(hold)?;
            let attempt = self
                .generated_issues
                .get(&hold.stream())
                .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?;
            if attempt.plan != plan
                || !matches!(attempt.phase, PhaseV1::Active | PhaseV1::PhysicallyComplete)
            {
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
            }
            let token = self.generated_issue_token_v1(hold, &plan)?;
            let id = token.id;
            let backend_submission = token.backend_submission;
            // At most two lower steps: observe and recycle, with no waiting or
            // publication. A still-pending packet remains owned after Stop.
            for _ in 0..2 {
                if self
                    .backend
                    .generated_submission_can_retire_v1(backend_submission)
                {
                    break;
                }
                self.backend
                    .progress_generated_submission_v1(backend_submission)
                    .map_err(map_backend_error)?;
            }
            if !self
                .backend
                .generated_submission_can_retire_v1(backend_submission)
            {
                return Ok(());
            }
            let unread = self.generated_issue_exclusive_readers_v1(&plan);
            if !self.journal_result_v1(unread)? {
                return Ok(());
            }
            self.backend
                .retire_generated_data_v1(&plan)
                .map_err(map_backend_error)?;
            self.backend
                .release_submission_v1(backend_submission)
                .map_err(map_backend_error)?;
            self.settle_stopped_gfx942_context_v1(hold, id, backend_submission)?;
            retired = true;
            Ok(())
        }));
        self.finish_generated_issue_v1(hold, result)?;
        Ok(retired)
    }

    // Native DATA and submission disposal precede this owner-only metadata tail.
    fn settle_stopped_gfx942_context_v1(
        &mut self,
        hold: &ContextUnpublishedHoldV1,
        id: RuntimeSubmissionIdV1,
        backend_submission: u64,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        let plan = self.generated_plan_for_hold_v1(hold)?;
        let token = self.generated_issue_token_v1(hold, &plan)?;
        if token.id != id || token.backend_submission != backend_submission {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
        }
        self.settle_generated_custody_v1(hold.stream(), SubmissionWriterOutcomeV1::Unknown)?;
        self.generated_issues
            .get_mut(&hold.stream())
            .expect("retained attempt")
            .phase = PhaseV1::DisposedWithoutResult;
        self.publish_submission_status_v1(id, RuntimeCompletionStatusV1::QuiescentWithoutResult)?;
        self.submissions.remove(&id);
        self.backend_submissions.remove(&backend_submission);
        self.retire_generated_shells_v1(hold)?;
        self.require_graph_access(hold.graph_access())?;
        self.generated_issues.remove(&hold.stream());
        Ok(())
    }

    fn settle_generated_custody_v1(
        &mut self,
        stream: RuntimeStreamIdV1,
        outcome: SubmissionWriterOutcomeV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        if matches!(outcome, SubmissionWriterOutcomeV1::NoEffect) {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
        }
        let attempt = self
            .generated_issues
            .get(&stream)
            .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?;
        let (id, domain) = attempt.writer_binding_v1();
        let result = self.generated_issue_exclusive_readers_v1(&attempt.plan);
        if !self.journal_result_v1(result)? {
            return Err(RuntimeValidationErrorV1::ContextReserved);
        }
        self.release_generated_submission_readers_v1(id, domain)?;
        self.generated_issues
            .get_mut(&stream)
            .expect("retained attempt")
            .expected_reader = None;
        self.settle_generated_submission_writer_v1(id, domain, outcome)?;
        if matches!(outcome, SubmissionWriterOutcomeV1::Success) {
            self.generated_issues
                .get_mut(&stream)
                .expect("retained attempt")
                .expected_writer = None;
        }
        Ok(())
    }

    fn generated_issue_device_uid_v1(
        &mut self,
        device: u64,
    ) -> Result<u64, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.backend
            .with_retained_preparation_device_v1(device, |owner| owner.observation().unique_id())
            .map_err(map_backend_error)
    }

    fn finish_generated_issue_v1(
        &mut self,
        hold: &ContextUnpublishedHoldV1,
        result: std::thread::Result<Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>>>,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        if !matches!(result, Ok(Ok(())))
            && let Some(attempt) = self.generated_issues.get_mut(&hold.stream())
        {
            attempt.phase = PhaseV1::Unknown;
        }
        self.finish_gfx942_adoption_v1(result)
    }
}

#[cfg(test)]
mod tests;
