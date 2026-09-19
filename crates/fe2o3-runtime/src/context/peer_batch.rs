//! Explicit aggregate progress without replacing ordinary submission custody.

use super::*;

/// Maximum ordinary peer-copy submissions in one bounded aggregate operation.
pub const MAX_RUNTIME_PEER_COPY_BATCH_SUBMISSIONS_V1: usize = 63;

/// Aggregate observation of the exact requested peer-copy roster.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimePeerCopyBatchPollV1 {
    /// Every submission remains retained; no partial completion is published.
    Pending,
    /// Every requested copy completed under the backend's closing checks.
    Succeeded,
}

/// Optional explicit peer-copy aggregate SPI; not encoded by Worker V1-V5.
///
/// Implementations must admit the complete exact roster before effects. They
/// may reject subsets of a native scheduling domain, mixed directions or work
/// whose dependencies are not already successful. The caller's absolute
/// deadline includes preparation. Expiry still permits one completion scan;
/// mandatory closing checks may finish after it. `Pending` retains all work
/// without republishing it on retry. `Succeeded` commits every listed backend
/// submission before returning, never before closing currentness succeeds.
///
/// Rejection preserves every pending submission. Quiescent failure means the
/// entire roster is quiescent without a result. Any partial or ambiguous effect
/// that cannot satisfy those outcomes must be terminal, not partial success.
pub trait RuntimePeerCopyBatchBackendV1: RuntimeBackendV1 {
    fn progress_peer_copy_batch_v1(
        &mut self,
        submissions: &[u64],
        deadline: Instant,
    ) -> Result<RuntimePeerCopyBatchPollV1, RuntimeBackendFailureV1<Self::Error>>;
}

impl<B: RuntimePeerCopyBatchBackendV1> RuntimeContextV1<B> {
    fn validate_peer_copy_batch_roots(
        &mut self,
        submissions: &[&mut RuntimeSubmissionV1<RuntimePeerCopyV1>],
    ) -> Result<(), RuntimeValidationErrorV1> {
        let result = catch_unwind(AssertUnwindSafe(|| {
            for submission in submissions {
                self.validate_pending_peer_copy_roots_v1(submission.id)?;
            }
            Ok::<_, fe2o3_runtime_model::ContextVersionJournalErrorV1>(())
        }));
        match result {
            Ok(Ok(())) => Ok(()),
            result => {
                self.quarantine_after_async_command_panic_v1();
                if let Err(payload) = result {
                    core::mem::forget(payload);
                }
                Err(RuntimeValidationErrorV1::InvalidBackendDescription)
            }
        }
    }

    /// Publishes and waits for an explicit batch of ordinary peer-copy handles.
    ///
    /// All handles must be distinct, pending, local to this context and on the
    /// same destination device. Native backends can impose stricter exact-roster
    /// admission. Ordinary `wait` is unchanged and never widens to other work.
    /// Every handle keeps its existing journal roots, events and callback list.
    /// Success is observed member by member in caller order after the backend
    /// closes the complete batch, not as an atomic group of callback deliveries.
    pub fn wait_peer_copy_batch(
        &mut self,
        submissions: &mut [&mut RuntimeSubmissionV1<RuntimePeerCopyV1>],
        timeout: Duration,
    ) -> Result<RuntimePeerCopyBatchPollV1, RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        if submissions.is_empty() || submissions.len() > MAX_RUNTIME_PEER_COPY_BATCH_SUBMISSIONS_V1
        {
            return Err(RuntimeValidationErrorV1::InvalidPeerCopyBatch.into());
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(RuntimeValidationErrorV1::InvalidDeadline)?;
        let mut backend_ids = Vec::new();
        backend_ids
            .try_reserve_exact(submissions.len())
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        let mut device = None;
        for submission in submissions.iter() {
            let record = self.live_submission_record(submission)?;
            self.require_ordinary_submission_v1(submission.id)?;
            self.require_stream_unheld_v1(record.stream)?;
            if record.status != RuntimeCompletionStatusV1::Pending
                || backend_ids.contains(&record.backend_submission)
            {
                return Err(RuntimeValidationErrorV1::InvalidPeerCopyBatch.into());
            }
            if device.is_some_and(|device| device != record.device) {
                return Err(RuntimeValidationErrorV1::WrongDevice.into());
            }
            device = Some(record.device);
            backend_ids.push(record.backend_submission);
        }
        self.validate_peer_copy_batch_roots(submissions)?;
        let result = self.invoke_journal_backend_v1(|backend| {
            backend.progress_peer_copy_batch_v1(&backend_ids, deadline)
        });
        self.observe_peer_copy_batch_result_v1(submissions, result)
    }

    pub(super) fn observe_peer_copy_batch_result_v1(
        &mut self,
        submissions: &mut [&mut RuntimeSubmissionV1<RuntimePeerCopyV1>],
        result: Result<RuntimePeerCopyBatchPollV1, RuntimeBackendFailureV1<B::Error>>,
    ) -> Result<RuntimePeerCopyBatchPollV1, RuntimeErrorV1<B::Error>> {
        let status = match result {
            Ok(RuntimePeerCopyBatchPollV1::Pending) => {
                return Ok(RuntimePeerCopyBatchPollV1::Pending);
            }
            Ok(RuntimePeerCopyBatchPollV1::Succeeded) => RuntimeCompletionStatusV1::Succeeded,
            Err(RuntimeBackendFailureV1::Quiescent(error)) => {
                if self.validate_peer_copy_batch_roots(submissions).is_err() {
                    return Err(RuntimeErrorV1::BackendQuiescent(error));
                }
                for submission in submissions.iter_mut() {
                    let result = self.transition_submission_status(
                        submission.id,
                        RuntimeCompletionStatusV1::QuiescentWithoutResult,
                    );
                    match result {
                        Ok(status) => {
                            submission.observe_status(status);
                        }
                        Err(_) => {
                            self.quarantine_after_async_command_panic_v1();
                            break;
                        }
                    }
                }
                return Err(RuntimeErrorV1::BackendQuiescent(error));
            }
            Err(failure) => return self.backend_result(Err(failure)),
        };
        self.validate_peer_copy_batch_roots(submissions)?;
        for submission in submissions.iter_mut() {
            let status = self.transition_submission_status(submission.id, status)?;
            submission.observe_status(status);
        }
        Ok(RuntimePeerCopyBatchPollV1::Succeeded)
    }
}
