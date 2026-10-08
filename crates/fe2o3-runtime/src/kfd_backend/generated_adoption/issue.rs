//! Private first-run publication and physical settlement, without output delivery.

use super::*;

pub(super) struct GeneratedSubmissionV1 {
    pub(super) id: u64,
    pub(super) roster: GeneratedHostRosterV1,
    pub(super) receipt: NativeReceiptV1,
}

impl KfdRuntimeBackendV1 {
    pub(in crate::kfd_backend) fn generated_shell_has_no_submission_v1(
        &self,
        plan: &GeneratedShellPlanV1,
    ) -> bool {
        self.validate_generated_shell_records_v1(plan)
            && !self
                .generated_submissions
                .values()
                .any(|key| *key == plan.key)
            && self.generated_shells.get(&plan.key).is_some_and(|record| {
                record
                    .native
                    .as_ref()
                    .is_none_or(|native| native.submission.is_none())
            })
    }

    pub(in crate::kfd_backend) fn generated_submission_owner_matches_v1(
        &self,
        submission: u64,
        plan: &GeneratedShellPlanV1,
    ) -> bool {
        submission != 0
            && self.generated_submissions.get(&submission) == Some(&plan.key)
            && self.validate_generated_shell_records_v1(plan)
            && self.generated_shells.get(&plan.key).is_some_and(|record| {
                record
                    .native
                    .as_ref()
                    .and_then(|native| native.submission.as_ref())
                    .is_some_and(|owner| {
                        owner.id == submission
                            && owner.receipt.profile() == plan.profile
                            && record
                                .source_identity
                                .matches(&owner.roster.source_identity)
                            && readback::roster_matches_plan_v1(plan, &owner.roster)
                    })
            })
    }

    pub(crate) fn prepare_generated_issue_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        roster: &GeneratedHostRosterV1,
    ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if !self.validate_generated_shell_records_v1(plan)
            || !readback::roster_matches_plan_v1(plan, roster)
            || !self.generated_lease_matches_v1(plan)
            || !self.generated_shells.get(&plan.key).is_some_and(|record| {
                record.source_identity.matches(&roster.source_identity)
                    && record.native.as_ref().is_some_and(|native| {
                        native.phase == PhaseV1::Adopted && native.submission.is_none()
                    })
            })
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "generated issue identity or phase mismatch",
            ));
        }
        let receipt = NativeReceiptV1::ready(plan.profile).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "registry members require the original registry receipt owner",
            )
        })?;
        self.require_submission_capacity_v1()?;
        self.generated_submissions
            .try_reserve(1)
            .map_err(|_| Self::capacity("generated submission index capacity"))?;
        let id = self.next_handle;
        let next = id
            .checked_add(1)
            .filter(|_| id != 0)
            .ok_or_else(|| Self::capacity("generated submission identity exhausted"))?;
        self.next_handle = next;
        let native = self
            .generated_shells
            .get_mut(&plan.key)
            .expect("validated shell")
            .native
            .as_mut()
            .expect("adopted native owner");
        native.submission = Some(GeneratedSubmissionV1 {
            id,
            roster: roster.clone(),
            receipt,
        });
        assert!(self.generated_submissions.insert(id, plan.key).is_none());
        Ok(id)
    }

    pub(super) fn generated_submission_plan_v1(
        &self,
        submission: u64,
    ) -> Result<GeneratedShellPlanV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        let plan = self
            .generated_submissions
            .get(&submission)
            .and_then(|key| self.generated_shells.get(key))
            .filter(|record| {
                record.native.as_ref().is_some_and(|native| {
                    native.phase == PhaseV1::Adopted
                        && native.submission.as_ref().is_some_and(|owner| {
                            owner.id == submission && owner.receipt.profile() == record.plan.profile
                        })
                })
            })
            .map(|record| record.plan)
            .ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown generated submission",
                )
            })?;
        if !self.validate_generated_shell_records_v1(&plan)
            || !self.generated_lease_matches_v1(&plan)
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "generated submission owner mismatch",
            ));
        }
        Ok(plan)
    }

    // Only the authority-bracketed Context driver calls this. Generic poll,
    // cleanup, wait and Stop are observation-only, including a Ready receipt.
    pub(crate) fn advance_generated_issue_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        submission: u64,
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.advance_generated_issue_with_rejection_v1(plan, submission, false)
    }

    pub(crate) fn advance_generated_issue_preserving_rejection_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        submission: u64,
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.advance_generated_issue_with_rejection_v1(plan, submission, true)
    }

    fn advance_generated_issue_with_rejection_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        submission: u64,
        preserve_rejection: bool,
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if preserve_rejection
            && plan.profile != crate::generated_source::GeneratedProfileV1::Singleton
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "local rejected publication is singleton-only",
            ));
        }
        if self.generated_submission_plan_v1(submission)? != *plan {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "generated issue plan mismatch",
            ));
        }
        if preserve_rejection && self.generated_rejected_publication_v1(plan, submission) {
            // Observe retained classification only; never retry its packet.
            let result = catch_unwind(AssertUnwindSafe(|| self.check_generated_device_v1(plan)));
            self.finish_generated_native_call_v1(result)?;
            return Ok(false);
        }
        let ready = self.generated_shells[&plan.key]
            .native
            .as_ref()
            .expect("native owner")
            .submission
            .as_ref()
            .expect("submission")
            .receipt
            .issue_ready();
        if !ready {
            return self.progress_generated_submission_v1(submission);
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.check_generated_device_v1(plan)?;
            let native = self
                .generated_shells
                .get_mut(&plan.key)
                .expect("rooted shell")
                .native
                .as_mut()
                .expect("native owner");
            let handle = native.native_lane.expect("exact lane");
            let receipt = &mut native.submission.as_mut().expect("submission").receipt;
            let result = self
                .queue
                .as_mut()
                .expect("retained queue")
                .with_compute_lane_v1(handle, |lane| {
                    if preserve_rejection {
                        receipt.issue_preserving_rejection(lane)
                    } else {
                        receipt.issue(lane)
                    }
                })
                .and_then(core::convert::identity);
            result.map_err(|error| self.generated_native_error_v1("issue", error))?;
            self.check_generated_device_v1(plan)
        }));
        self.finish_generated_native_call_v1(result)?;
        #[cfg(feature = "hardware-qualification")]
        if self.generated_shells[&plan.key]
            .native
            .as_ref()
            .unwrap()
            .submission
            .as_ref()
            .unwrap()
            .receipt
            .published()
        {
            self.record_generated_copy_publication_v1(
                qualification::KfdGeneratedCopyPublicationKindV1::GeneratedCompute,
                submission,
            );
        }
        Ok(false)
    }

    #[allow(
        clippy::result_large_err,
        reason = "returned linear completion custody must stay inline without post-effect allocation"
    )]
    pub(crate) fn progress_generated_submission_v1(
        &mut self,
        submission: u64,
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let plan = self.generated_submission_plan_v1(submission)?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.check_generated_device_v1(&plan)?;
            let native = self
                .generated_shells
                .get_mut(&plan.key)
                .expect("rooted shell")
                .native
                .as_mut()
                .expect("native owner");
            let handle = native.native_lane.expect("exact lane");
            let receipt = &mut native.submission.as_mut().expect("submission").receipt;
            let result = self
                .queue
                .as_mut()
                .expect("retained queue")
                .with_compute_lane_v1(handle, |lane| receipt.progress(lane))
                .and_then(core::convert::identity);
            result.map_err(|error| self.generated_native_error_v1("completion", error))?;
            self.check_generated_device_v1(&plan)
        }));
        self.finish_generated_native_call_v1(result)?;
        Ok(self.generated_shells[&plan.key]
            .native
            .as_ref()
            .expect("native owner")
            .submission
            .as_ref()
            .expect("submission")
            .receipt
            .recycled())
    }

    pub(crate) fn generated_submission_can_retire_v1(&self, submission: u64) -> bool {
        self.generated_submissions
            .get(&submission)
            .and_then(|key| self.generated_shells.get(key))
            .and_then(|record| {
                record
                    .native
                    .as_ref()
                    .map(|native| (native, record.plan.profile))
            })
            .is_some_and(|(native, profile)| {
                native.phase == PhaseV1::Adopted
                    && native.submission.as_ref().is_some_and(|owner| {
                        owner.id == submission
                            && owner.receipt.profile() == profile
                            && owner.receipt.retirement().is_some()
                    })
            })
    }

    pub(in crate::kfd_backend) fn poll_generated_submission_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.progress_generated_submission_v1(submission)? {
            // Physical retirement is not successful generated readback/decoding.
            Err(Self::quiescent_error(
                KfdRuntimeBackendErrorKindV1::Native,
                "generated dispatch physically complete without delivered result",
            ))
        } else {
            Ok(BackendPollV1::Pending)
        }
    }

    pub(in crate::kfd_backend) fn release_generated_submission_v1(
        &mut self,
        submission: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        let key = self.generated_submissions[&submission];
        let profile = self.generated_shells[&key].plan.profile;
        let native = self
            .generated_shells
            .get_mut(&key)
            .and_then(|record| record.native.as_mut())
            .expect("indexed native owner");
        if !native.is_retired()
            || !native.submission.as_ref().is_some_and(|owner| {
                owner.id == submission
                    && owner.receipt.profile() == profile
                    && (owner.receipt.retirement().is_some() || owner.receipt.rejected_disposed())
            })
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "generated submission retains native DATA",
            ));
        }
        native.submission = None;
        self.generated_submissions.remove(&submission);
        Ok(())
    }

    pub(crate) fn generated_rejected_publication_v1(
        &self,
        plan: &GeneratedShellPlanV1,
        submission: u64,
    ) -> bool {
        self.require_live().is_ok()
            && plan.profile == crate::generated_source::GeneratedProfileV1::Singleton
            && self.generated_lease_matches_v1(plan)
            && self.generated_submission_owner_matches_v1(submission, plan)
            && self.generated_shells[&plan.key]
                .native
                .as_ref()
                .is_some_and(|native| {
                    native.phase == PhaseV1::Adopted
                        && native
                            .submission
                            .as_ref()
                            .is_some_and(|owner| owner.receipt.rejected_retirement().is_some())
                })
    }

    pub(super) fn check_generated_device_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let current = self
            .with_retained_preparation_device_v1(plan.binding.backend_device, |device| {
                device.model_admission()
            })?;
        if current != plan.binding.native_device || !self.generated_lease_matches_v1(plan) {
            return Err(self.terminal_error("generated issue/completion device or lease mismatch"));
        }
        Ok(())
    }
}
