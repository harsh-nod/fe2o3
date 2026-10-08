//! A private phase rooted by the original Context attempt and lexical carrier.

use super::*;

pub(super) enum CompletionDispositionV1 {
    Retained(RetainedProducerV1),
    Settled(NativeSettlementV1),
}

pub(super) struct RetainedProducerV1 {
    pub(super) submission: RuntimeSubmissionIdV1,
    pub(super) backend_submission: u64,
    pub(super) stream: RuntimeStreamIdV1,
    pub(super) hold: u64,
    pub(super) expected_writer: Option<fe2o3_runtime_model::ContextWriterReferenceV1>,
    pub(super) expected_reader: Option<SubmissionReaderMarkerV1>,
}

macro_rules! impl_retained_producer_context {
    ($backend:ty) => {
        impl RuntimeContextV1<$backend> {
            pub(in crate::context::generated_issue::completion) fn retain_completed_gfx942_context_v1(
                &mut self,
                hold: &ContextUnpublishedHoldV1,
                original: RetainedProducerV1,
            ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
                self.validate_unpublished_hold_v1(hold)?;
                let attempt = self
                    .generated_issues
                    .get(&hold.stream())
                    .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?;
                let token = self.generated_issue_token_v1(hold, &attempt.plan)?;
                if attempt.phase != PhaseV1::Unknown
                    || original.submission != token.id
                    || original.backend_submission != token.backend_submission
                    || original.stream != hold.stream()
                    || original.hold != hold.identity()
                    || original.expected_writer != attempt.expected_writer
                    || original.expected_reader != attempt.expected_reader
                {
                    return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
                }
                // No Context writer success, hold release, decoder or graph
                // success occurs here. The next step must borrow the original
                // source again before consuming the retained DATA for disposal.
                self.generated_issues
                    .get_mut(&hold.stream())
                    .expect("retained original attempt")
                    .phase = PhaseV1::RetainedProducer;
                Ok(())
            }
        }
    };
}

impl_retained_producer_context!(KfdRuntimeBackendV1);
impl_retained_producer_context!(KfdMultiDeviceRuntimeBackendV1);

#[cfg(test)]
mod tests;
