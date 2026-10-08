//! Destination-only journal admission for an original detached generated source.
//! No ordinary source AllocationRef or generated consumer profile is created.

use super::*;

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    pub(crate) fn advance_gfx942_data_copy_v1<T: RuntimeGfx942GeneratedCompletionCarrierV1>(
        &mut self,
        prepared: &mut RuntimeGfx942PreparedV1<T>,
        roster: &GeneratedHostRosterV1,
        hold: &ContextUnpublishedHoldV1,
        stream: RuntimeStreamIdV1,
        destination: RuntimeMemoryRegionV1,
        copy: &mut Option<RuntimeSubmissionV1<RuntimeCopyV1>>,
    ) -> Result<bool, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.require_graph_access(None)?;
        self.validate_unpublished_hold_v1(hold)?;
        if hold.graph_access().is_some() || self.versions.is_none() {
            return Err(RuntimeValidationErrorV1::Unsupported.into());
        }
        let scope = self.generated_adoption_scope_for_hold_v1(hold)?;
        let mut completed = false;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.validate_gfx942_prepared_v1(prepared)?;
            let plan = self.generated_plan_for_hold_v1(hold)?;
            if plan.profile != crate::generated_source::GeneratedProfileV1::Singleton
                || plan.count != 1
                || !self.gfx942_prepared_matches_plan_v1(prepared, &plan)
            {
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
            }
            let attempt = self
                .generated_issues
                .get(&hold.stream())
                .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?;
            if attempt.phase != PhaseV1::RetainedProducer
                || attempt.plan != plan
                || !attempt.roster.matches(roster)
            {
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
            }
            let producer = self.generated_issue_token_v1(hold, &plan)?;
            let producer_id = producer.id;
            let producer_backend = producer.backend_submission;
            let expected_writer = attempt.expected_writer;
            let expected_reader = attempt.expected_reader;
            let unread = self.generated_issue_exclusive_readers_v1(&plan);
            if !self.journal_result_v1(unread)? {
                return Err(RuntimeValidationErrorV1::ContextReserved.into());
            }
            let allocation = *self
                .allocations
                .get(&destination.allocation)
                .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?;
            let stream_record = *self.unheld_stream_v1(stream)?;
            if allocation.device != plan.binding.device
                || stream_record.device != allocation.device
                || stream == hold.stream()
            {
                return Err(RuntimeValidationErrorV1::WrongDevice.into());
            }
            if allocation.kind != RuntimeMemoryKindV1::DeviceLocal
                || destination.byte_offset != 0
                || destination.byte_len != allocation.byte_len
                || destination.byte_len == 0
                || plan.members[0].is_none_or(|m| m.description.byte_len != destination.byte_len)
                || !matches!(
                    destination.access,
                    RuntimeAccessV1::Write | RuntimeAccessV1::ReadWrite
                )
            {
                return Err(RuntimeValidationErrorV1::InvalidRange.into());
            }
            let bytes = u32::try_from(destination.byte_len)
                .map_err(|_| RuntimeValidationErrorV1::InvalidRange)?;
            if let Some(copy) = copy.as_ref() {
                let record = self.submission_record(copy)?;
                if record.stream != stream {
                    return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
                }
            } else if self.submissions.len() >= MAX_RUNTIME_SUBMISSIONS_V1
                || self
                    .submissions
                    .values()
                    .any(|r| r.stream == stream && !r.quiescent)
            {
                return Err(RuntimeValidationErrorV1::ContextReserved.into());
            }
            let uid = self.generated_issue_device_uid_v1(plan.binding.backend_device)?;
            self.generated_issues.get_mut(&hold.stream()).unwrap().phase = PhaseV1::Unknown;
            let destination_complete =
                advance_destination_after_source(self, copy, |context, copy| {
                    let mut native_complete = false;
                    with_completion_view_result_v1(prepared.value_mut_v1(), |view| {
                        let (source, destinations) = view.into_parts();
                        source
                            .with_current_source_v1(uid, roster, || {
                                source
                                    .validate_completed_readback_v1(destinations)
                                    .map_err(|_| {
                                        RuntimeErrorV1::Validation(
                                            RuntimeValidationErrorV1::InvalidBackendDescription,
                                        )
                                    })?;
                                if let Some(original) = copy.as_ref() {
                                    native_complete = context
                                        .backend
                                        .advance_retained_generated_copy_v1(
                                            &plan,
                                            producer_backend,
                                            original.backend_submission,
                                        )
                                        .map_err(map_backend_error)?;
                                } else {
                                    // Reserve the original destination writer before entering
                                    // the backend. The DATA owner is not an ordinary reader.
                                    *copy = Some(context.submit_context_operation_v1(
                                        stream,
                                        stream_record,
                                        &[destination.allocation],
                                        None,
                                        &[],
                                        |backend| {
                                            backend.begin_retained_generated_copy_v1(
                                                &plan,
                                                producer_backend,
                                                stream_record.backend_stream,
                                                allocation.backend_allocation,
                                                bytes,
                                            )
                                        },
                                    )?);
                                }
                                Ok::<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>>(())
                            })
                            .map_err(|_| {
                                RuntimeErrorV1::Validation(
                                    RuntimeValidationErrorV1::InvalidBackendDescription,
                                )
                            })??;
                        Ok(())
                    })?;
                    Ok(native_complete)
                })?;
            if !destination_complete {
                self.generated_issues.get_mut(&hold.stream()).unwrap().phase =
                    PhaseV1::RetainedProducer;
                return Ok(());
            }
            let Some(original) = copy.as_mut() else {
                std::process::abort()
            };
            self.backend
                .finish_retained_generated_copy_v1(
                    &plan,
                    producer_backend,
                    original.backend_submission,
                )
                .map_err(map_backend_error)?;
            self.backend
                .release_submission_v1(producer_backend)
                .map_err(map_backend_error)?;
            self.settle_completed_gfx942_context_v1(
                hold,
                NativeSettlementV1 {
                    submission: producer_id,
                    backend_submission: producer_backend,
                    stream: hold.stream(),
                    hold: hold.identity(),
                    expected_writer,
                    expected_reader,
                },
            )?;
            copy.take();
            completed = true;
            Ok(())
        }));
        self.finish_generated_issue_scoped_v1(hold, scope, result)?;
        Ok(completed)
    }
}

// The concrete caller retains the original source bracket and native owners.
// A false/error/unwind from that bracket must not observe or release the writer.
fn advance_destination_after_source<B: RuntimeBackendV1>(
    context: &mut RuntimeContextV1<B>,
    copy: &mut Option<RuntimeSubmissionV1<RuntimeCopyV1>>,
    step: impl FnOnce(
        &mut RuntimeContextV1<B>,
        &mut Option<RuntimeSubmissionV1<RuntimeCopyV1>>,
    ) -> Result<bool, RuntimeErrorV1<B::Error>>,
) -> Result<bool, RuntimeErrorV1<B::Error>> {
    if !step(context, copy)? {
        return Ok(false);
    }
    // Only after native source release, destination restoration and the
    // original source closing check may the ordinary writer succeed.
    let Some(original) = copy.as_mut() else {
        std::process::abort()
    };
    if context.poll(original)? != RuntimePollV1::Succeeded {
        return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
    }
    context.release_submission_ref(original, None)?;
    Ok(true)
}

#[cfg(test)]
#[path = "data_copy/tests.rs"]
mod tests;
