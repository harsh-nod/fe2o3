//! Private first-run publication and physical settlement, without output delivery.

use super::*;
use fe2o3_kfd::Gfx942CompletedDispatchBatchV1;

pub(super) struct GeneratedSubmissionV1 {
    pub(super) id: u64,
    pub(super) roster: GeneratedHostRosterV1,
    pub(super) receipt: ReceiptV1<Gfx942DispatchBatchV1<1>, Gfx942CompletedDispatchBatchV1<1>>,
}

impl KfdRuntimeBackendV1 {
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
                Arc::ptr_eq(&record.source_identity, &roster.source_identity)
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
            receipt: ReceiptV1::Ready,
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
                        && native
                            .submission
                            .as_ref()
                            .is_some_and(|owner| owner.id == submission)
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
        if self.generated_submission_plan_v1(submission)? != *plan {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "generated issue plan mismatch",
            ));
        }
        let ready = matches!(
            self.generated_shells[&plan.key]
                .native
                .as_ref()
                .expect("native owner")
                .submission
                .as_ref()
                .expect("submission")
                .receipt,
            ReceiptV1::Ready
        );
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
                    receipt.issue(|| lane.submit_fixed_dispatch_classified_v1::<1>())
                })
                .and_then(core::convert::identity);
            result.map_err(|error| self.generated_native_error_v1("issue", error))?;
            self.check_generated_device_v1(plan)
        }));
        self.finish_generated_native_call_v1(result)?;
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
                .with_compute_lane_v1(handle, |lane| match receipt {
                    ReceiptV1::Ready | ReceiptV1::Recycled => Ok(()),
                    ReceiptV1::Published(_) => receipt.poll(|batch| {
                        lane.poll_fixed_dispatch(batch).map(|poll| match poll {
                            Gfx942DispatchPollV1::Pending(batch) => receipt::PollV1::Pending(batch),
                            Gfx942DispatchPollV1::Ready(completed) => {
                                receipt::PollV1::Completed(completed)
                            }
                        })
                    }),
                    ReceiptV1::Completed(_) => receipt
                        .recycle(|completed| {
                            lane.recycle_fixed_dispatch(completed)
                                .map(|_| ())
                                .map_err(|failure| failure.into_parts())
                        })
                        .map(|_| ()),
                    ReceiptV1::HandedToLower(stage) => {
                        let _ = stage;
                        Err(fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract(
                            "generated consuming handoff cannot retry",
                        ))
                    }
                })
                .and_then(core::convert::identity);
            result.map_err(|error| self.generated_native_error_v1("completion", error))?;
            self.check_generated_device_v1(&plan)
        }));
        self.finish_generated_native_call_v1(result)?;
        Ok(matches!(
            self.generated_shells[&plan.key]
                .native
                .as_ref()
                .expect("native owner")
                .submission
                .as_ref()
                .expect("submission")
                .receipt,
            ReceiptV1::Recycled
        ))
    }

    pub(crate) fn generated_submission_can_retire_v1(&self, submission: u64) -> bool {
        self.generated_submissions
            .get(&submission)
            .and_then(|key| self.generated_shells.get(key))
            .and_then(|record| record.native.as_ref())
            .is_some_and(|native| {
                native.phase == PhaseV1::Adopted
                    && native.submission.as_ref().is_some_and(|owner| {
                        owner.id == submission
                            && matches!(owner.receipt, ReceiptV1::Ready | ReceiptV1::Recycled)
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
        let native = self
            .generated_shells
            .get_mut(&key)
            .and_then(|record| record.native.as_mut())
            .expect("indexed native owner");
        if !native.is_retired()
            || !native.submission.as_ref().is_some_and(|owner| {
                owner.id == submission
                    && matches!(owner.receipt, ReceiptV1::Ready | ReceiptV1::Recycled)
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
