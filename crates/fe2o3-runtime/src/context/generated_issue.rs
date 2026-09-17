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
    // The original hold/carrier remain in the owner-only async registry. Owned
    // shutdown retains that registry and this Context together on any failure.
    submission: Option<RuntimeSubmissionV1<()>>,
}

impl GeneratedIssueV1 {
    pub(super) fn owns_submission_v1(&self, id: RuntimeSubmissionIdV1) -> bool {
        self.id == id
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
        let id = RuntimeSubmissionIdV1::new(self.context_generation, self.next_id()?);
        self.generated_issues.insert(
            hold.stream(),
            GeneratedIssueV1 {
                id,
                plan,
                roster: roster.clone(),
                phase: PhaseV1::Entering,
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
                backend_submission,
                stream: hold.stream(),
                device,
                quiescent: false,
                status: RuntimeCompletionStatusV1::Pending,
                journal_writer: None,
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
            self.backend
                .retire_generated_data_v1(&plan)
                .map_err(map_backend_error)?;
            self.backend
                .release_submission_v1(backend_submission)
                .map_err(map_backend_error)?;
            self.generated_issues
                .get_mut(&hold.stream())
                .expect("retained attempt")
                .phase = PhaseV1::DisposedWithoutResult;
            self.transition_submission_status(
                id,
                RuntimeCompletionStatusV1::QuiescentWithoutResult,
            )?;
            self.submissions.remove(&id);
            self.backend_submissions.remove(&backend_submission);
            self.retire_generated_shells_v1(hold)?;
            self.require_graph_access(hold.graph_access())?;
            self.generated_issues.remove(&hold.stream());
            retired = true;
            Ok(())
        }));
        self.finish_generated_issue_v1(hold, result)?;
        Ok(retired)
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
