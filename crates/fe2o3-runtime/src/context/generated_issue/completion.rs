//! Original readback remains borrowed until native and Context settlement finish.

use super::*;
use crate::{RuntimeGfx942GeneratedCompletionCarrierV1, RuntimeGfx942GeneratedCompletionViewV1};
mod cohort3;
mod retained_producer;
use retained_producer::{CompletionDispositionV1, RetainedProducerV1};

pub(super) struct NativeSettlementV1 {
    submission: RuntimeSubmissionIdV1,
    backend_submission: u64,
    stream: RuntimeStreamIdV1,
    hold: u64,
    expected_writer: Option<fe2o3_runtime_model::ContextWriterReferenceV1>,
    expected_reader: Option<SubmissionReaderMarkerV1>,
}

#[cfg(test)]
pub(super) fn assume_native_settlement_for_context_test_v1<B: RuntimeBackendV1>(
    context: &RuntimeContextV1<B>,
    hold: &ContextUnpublishedHoldV1,
) -> NativeSettlementV1 {
    let attempt = &context.generated_issues[&hold.stream()];
    NativeSettlementV1 {
        submission: attempt.id,
        backend_submission: attempt.submission.as_ref().unwrap().backend_submission,
        stream: hold.stream(),
        hold: hold.identity(),
        expected_writer: attempt.expected_writer,
        expected_reader: attempt.expected_reader,
    }
}

pub(super) fn require_completion_view_v1<T: RuntimeGfx942GeneratedCompletionCarrierV1>(
    carrier: &mut T,
    complete: impl for<'a> FnOnce(
        RuntimeGfx942GeneratedCompletionViewV1<'a, T::CurrentnessError>,
    ) -> Result<
        NativeSettlementV1,
        RuntimeErrorV1<KfdRuntimeBackendErrorV1>,
    >,
) -> Result<NativeSettlementV1, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
    with_completion_view_result_v1(carrier, complete)
}

fn with_completion_view_result_v1<T: RuntimeGfx942GeneratedCompletionCarrierV1, O>(
    carrier: &mut T,
    complete: impl for<'a> FnOnce(
        RuntimeGfx942GeneratedCompletionViewV1<'a, T::CurrentnessError>,
    ) -> Result<O, RuntimeErrorV1<KfdRuntimeBackendErrorV1>>,
) -> Result<O, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
    let mut settled = None;
    carrier.with_completion_view_v1(|view| {
        settled = Some(complete(view)?);
        Ok(())
    })?;
    settled.ok_or_else(|| RuntimeValidationErrorV1::InvalidBackendDescription.into())
}

macro_rules! impl_generated_completion_context {
    ($backend:ty) => {
        impl RuntimeContextV1<$backend> {
            pub(crate) fn complete_gfx942_issue_v1<T: RuntimeGfx942GeneratedCompletionCarrierV1>(
                &mut self,
                prepared: &mut RuntimeGfx942PreparedV1<T>,
                roster: &GeneratedHostRosterV1,
                hold: &ContextUnpublishedHoldV1,
            ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
                self.complete_gfx942_issue_mode_v1(prepared, roster, hold, false)
                    .map(|_| ())
            }

            pub(crate) fn complete_gfx942_scoped_issue_v1<
                T: RuntimeGfx942GeneratedCompletionCarrierV1,
            >(
                &mut self,
                prepared: &mut RuntimeGfx942PreparedV1<T>,
                roster: &GeneratedHostRosterV1,
                hold: &ContextUnpublishedHoldV1,
            ) -> Result<bool, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
                self.complete_gfx942_issue_mode_v1(prepared, roster, hold, true)
            }

            fn complete_gfx942_issue_mode_v1<T: RuntimeGfx942GeneratedCompletionCarrierV1>(
                &mut self,
                prepared: &mut RuntimeGfx942PreparedV1<T>,
                roster: &GeneratedHostRosterV1,
                hold: &ContextUnpublishedHoldV1,
                retain_producer: bool,
            ) -> Result<bool, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
                self.validate_unpublished_hold_v1(hold)?;
                let scope = self.generated_adoption_scope_for_hold_v1(hold)?;
                let mut completed = false;
                let result = catch_unwind(AssertUnwindSafe(|| {
                    self.validate_gfx942_prepared_v1(prepared)?;
                    let plan = self.generated_plan_for_hold_v1(hold)?;
                    if !self.gfx942_prepared_matches_plan_v1(prepared, &plan) {
                        return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
                    }
                    let attempt = self
                        .generated_issues
                        .get(&hold.stream())
                        .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?;
                    let retained = retain_producer && attempt.phase == PhaseV1::RetainedProducer;
                    if attempt.plan != plan
                        || !attempt.roster.matches(roster)
                        || !(attempt.phase == PhaseV1::PhysicallyComplete || retained)
                    {
                        return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
                    }
                    let token = self.generated_issue_token_v1(hold, &plan)?;
                    let id = token.id;
                    let backend_submission = token.backend_submission;
                    let expected_writer = attempt.expected_writer;
                    let expected_reader = attempt.expected_reader;
                    let unread = self.generated_issue_exclusive_readers_v1(&plan);
                    if !self.journal_result_v1(unread)? {
                        return Err(RuntimeValidationErrorV1::ContextReserved.into());
                    }
                    let uid = self.generated_issue_device_uid_v1(plan.binding.backend_device)?;
                    self.generated_issues
                        .get_mut(&hold.stream())
                        .expect("retained attempt")
                        .phase = PhaseV1::Unknown;
                    let disposition =
                        with_completion_view_result_v1(prepared.value_mut_v1(), |view| {
                            let (source, destinations) = view.into_parts();
                            let mut retained_now = false;
                            source
                                .with_current_source_v1(uid, roster, || {
                                    if !retained {
                                        self.backend
                                            .read_generated_submission_v1(
                                                &plan,
                                                backend_submission,
                                                roster,
                                                destinations,
                                            )
                                            .map_err(map_backend_error)?;
                                    }
                                    source
                                        .validate_completed_readback_v1(destinations)
                                        .map_err(|_| {
                                            RuntimeErrorV1::Validation(
                                                RuntimeValidationErrorV1::InvalidBackendDescription,
                                            )
                                        })?;
                                    if retain_producer
                                        && !retained
                                        && self
                                            .backend
                                            .retain_scoped_completed_producer_v1(
                                                &plan,
                                                backend_submission,
                                            )
                                            .map_err(map_backend_error)?
                                    {
                                        retained_now = true;
                                        return Ok(());
                                    }
                                    self.backend
                                        .retire_generated_data_v1(&plan)
                                        .map_err(map_backend_error)?;
                                    self.backend
                                        .release_submission_v1(backend_submission)
                                        .map_err(map_backend_error)?;
                                    Ok::<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>>(())
                                })
                                .map_err(|_| {
                                    RuntimeErrorV1::Validation(
                                        RuntimeValidationErrorV1::InvalidBackendDescription,
                                    )
                                })??;
                            Ok(if retained_now {
                                CompletionDispositionV1::Retained(RetainedProducerV1 {
                                    submission: id,
                                    backend_submission,
                                    stream: hold.stream(),
                                    hold: hold.identity(),
                                    expected_writer,
                                    expected_reader,
                                })
                            } else {
                                CompletionDispositionV1::Settled(NativeSettlementV1 {
                                    submission: id,
                                    backend_submission,
                                    stream: hold.stream(),
                                    hold: hold.identity(),
                                    expected_writer,
                                    expected_reader,
                                })
                            })
                        })?;
                    match disposition {
                        CompletionDispositionV1::Retained(original) => {
                            self.retain_completed_gfx942_context_v1(hold, original)?;
                            Ok(())
                        }
                        CompletionDispositionV1::Settled(settled) => {
                            self.settle_completed_gfx942_context_v1(hold, settled)?;
                            completed = true;
                            Ok(())
                        }
                    }
                }));
                self.finish_generated_issue_scoped_v1(hold, scope, result)?;
                Ok(completed)
            }

            // Called only after required lending, closing currentness and native
            // settlement. This tail releases Context metadata, not native authority.
            pub(super) fn settle_completed_gfx942_context_v1(
                &mut self,
                hold: &ContextUnpublishedHoldV1,
                settled: NativeSettlementV1,
            ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
                self.validate_unpublished_hold_v1(hold)?;
                let attempt = self
                    .generated_issues
                    .get(&hold.stream())
                    .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?;
                if attempt.phase != PhaseV1::Unknown {
                    return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
                }
                let token = self.generated_issue_token_v1(hold, &attempt.plan)?;
                let (id, backend_submission) = (token.id, token.backend_submission);
                if settled.submission != id
                    || settled.backend_submission != backend_submission
                    || settled.stream != hold.stream()
                    || settled.hold != hold.identity()
                    || settled.expected_writer != attempt.expected_writer
                    || settled.expected_reader != attempt.expected_reader
                {
                    return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
                }
                self.settle_generated_custody_v1(
                    hold.stream(),
                    SubmissionWriterOutcomeV1::Success,
                )?;
                self.retire_generated_shells_v1(hold)?;
                self.require_graph_access(hold.graph_access())?;
                self.release_unpublished_hold_v1(hold)?;
                // This status describes native execution. Typed output stays behind
                // its original gate until the async driver consumes the decoder.
                self.publish_submission_status_v1(id, RuntimeCompletionStatusV1::Succeeded)?;
                self.submissions.remove(&id);
                self.backend_submissions.remove(&backend_submission);
                self.generated_issues.remove(&hold.stream());
                Ok(())
            }
        }
    };
}
impl_generated_completion_context!(KfdRuntimeBackendV1);
impl_generated_completion_context!(KfdMultiDeviceRuntimeBackendV1);
