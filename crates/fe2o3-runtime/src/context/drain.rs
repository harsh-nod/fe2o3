//! Owner-only drain observations through the ordinary completion transition.

use super::*;
use std::collections::VecDeque;

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    pub(crate) fn snapshot_async_drain_v1(
        &self,
    ) -> Result<(VecDeque<RuntimeSubmissionIdV1>, Vec<RuntimeStreamIdV1>), RuntimeValidationErrorV1>
    {
        self.require_live()?;
        let mut submissions = Vec::new();
        submissions
            .try_reserve(self.submissions.len())
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        submissions.extend(
            self.submissions
                .iter()
                .filter_map(|(&id, record)| (!record.status.is_terminal()).then_some(id)),
        );
        submissions.sort_unstable();
        let mut streams = Vec::new();
        streams
            .try_reserve(self.streams.len())
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        streams.extend(self.streams.keys().copied());
        streams.sort_unstable();
        Ok((submissions.into(), streams))
    }

    pub(crate) fn poll_async_drain_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<RuntimeCompletionStatusV1, RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        let record = *self
            .submissions
            .get(&id)
            .ok_or(RuntimeValidationErrorV1::UnknownSubmission)?;
        if record.status.is_terminal() {
            return Ok(record.status);
        }
        let result = self.backend.poll_v1(record.backend_submission);
        self.completion_backend_result(id, result)
    }

    pub(crate) fn async_drain_counts_v1(&self) -> RuntimeStreamObservationV1 {
        let mut counts = RuntimeStreamObservationV1::default();
        let mut first_failure = None;
        for (&id, record) in &self.submissions {
            counts.total_submissions += 1;
            match record.status {
                RuntimeCompletionStatusV1::Pending => counts.pending += 1,
                RuntimeCompletionStatusV1::Succeeded => counts.succeeded += 1,
                RuntimeCompletionStatusV1::Failed(failure) => {
                    counts.failed += 1;
                    if first_failure.is_none_or(|(prior, _)| id < prior) {
                        first_failure = Some((id, failure));
                    }
                }
                RuntimeCompletionStatusV1::QuiescentWithoutResult => {
                    counts.quiescent_without_result += 1
                }
            }
        }
        counts.first_failure = first_failure.map(|(_, failure)| failure);
        counts
    }
}
