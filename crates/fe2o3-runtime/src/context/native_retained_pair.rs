//! Concrete native pair facade; native authority remains in the backend.

#![forbid(unsafe_code)]

use super::*;
use crate::{KfdNativeXgmiRuntimeBackendV1, KfdRuntimeBackendErrorV1};
use fe2o3_kfd::{
    GFX942_XGMI_RETAINED_PAIR_PROFILE_V1, Gfx942XgmiRetainedPairEnvironmentAssumptionV1,
};

/// Completion under the opt-in ordinary-lifetime profile, not whole-host freshness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeNativeRetainedPeerCopyPollV1 {
    Pending,
    /// Native copy fences closed; `finish` still owns logical settlement and restoration.
    ReadyToFinish,
}

/// One bounded native roster. Finish explicitly; Drop quarantines, forget keeps the gate.
///
/// This is not a generic backend provider or a full-fresh completion witness.
/// Both endpoint sessions and the directional queue remain backend-owned.
///
/// ```compile_fail,E0599
/// use fe2o3_runtime::RuntimeNativeRetainedPeerCopyBatchV1;
/// fn cannot_clone(batch: RuntimeNativeRetainedPeerCopyBatchV1<'_, '_, '_>) {
///     let _ = batch.clone();
/// }
/// ```
///
/// ```compile_fail,E0616
/// use fe2o3_runtime::RuntimeNativeRetainedPeerCopyBatchV1;
/// fn cannot_bypass_gate(batch: RuntimeNativeRetainedPeerCopyBatchV1<'_, '_, '_>) {
///     let _ = batch.context;
/// }
/// ```
#[must_use = "finish the retained native pair before ordinary context use"]
pub struct RuntimeNativeRetainedPeerCopyBatchV1<'context, 'roster, 'handle> {
    context: &'context mut RuntimeContextV1<KfdNativeXgmiRuntimeBackendV1>,
    submissions: &'roster mut [&'handle mut RuntimeSubmissionV1<RuntimePeerCopyV1>],
    token: u64,
    finished: bool,
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    pub(in crate::context) fn retained_pair_roster_v1(
        &self,
        submissions: &[&mut RuntimeSubmissionV1<RuntimePeerCopyV1>],
    ) -> Result<Vec<u64>, RuntimeValidationErrorV1> {
        self.require_live()?;
        if submissions.is_empty()
            || submissions.len() > MAX_RUNTIME_PEER_COPY_BATCH_SUBMISSIONS_V1
            || submissions.len() != self.submissions.len()
            || !self.events.is_empty()
            || !self.completion_callbacks.is_empty()
            || self.completion_callback_count != 0
            || !self.producer_launches.is_empty()
            || !self.generated_issues.is_empty()
            || self.has_unpublished_holds_v1()
        {
            return Err(RuntimeValidationErrorV1::InvalidPeerCopyBatch);
        }
        let mut ids = Vec::new();
        ids.try_reserve_exact(submissions.len())
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        let mut device = None;
        for submission in submissions {
            let record = self.live_submission_record(submission)?;
            self.require_ordinary_submission_v1(submission.id)?;
            self.require_stream_unheld_v1(record.stream)?;
            if record.status != RuntimeCompletionStatusV1::Pending
                || !record.scalar_peer_copy
                || record.directed_peer_copy
                || record.producer_launch
                || record.dependency_retains != 0
                || ids.contains(&record.backend_submission)
                || self
                    .scalar_peer_copies
                    .get(&submission.id)
                    .is_none_or(|root| !root.dependencies.is_empty() || root.directed.is_some())
            {
                return Err(RuntimeValidationErrorV1::InvalidPeerCopyBatch);
            }
            if device.is_some_and(|device| device != record.device) {
                return Err(RuntimeValidationErrorV1::WrongDevice);
            }
            device = Some(record.device);
            ids.push(record.backend_submission);
        }
        Ok(ids)
    }

    pub(in crate::context) fn require_retained_pair_v1(
        &self,
        token: u64,
    ) -> Result<(), RuntimeValidationErrorV1> {
        if self.terminal {
            Err(RuntimeValidationErrorV1::ContextTerminal)
        } else if self.native_pair_reservation != Some(token) || self.graph_reservation.is_some() {
            Err(RuntimeValidationErrorV1::ContextReserved)
        } else {
            Ok(())
        }
    }
}

impl RuntimeContextV1<KfdNativeXgmiRuntimeBackendV1> {
    /// Adopts the complete pending ordinary native-XGMI roster under explicit opt-in.
    ///
    /// The first slice excludes events, callbacks, dependencies, directed copies,
    /// segmented copies and every other pending submission. Entry fixes an absolute
    /// deadline including preparation; subsequent waits never republish or extend it.
    /// Entry prepares and retains native owners but does not publish packets. The
    /// first `wait` publishes once; later waits only observe those exact tickets.
    /// No success is exposed on the ordinary handles until explicit `finish`.
    pub fn begin_retained_peer_copy_batch_v1<'context, 'roster, 'handle>(
        &'context mut self,
        submissions: &'roster mut [&'handle mut RuntimeSubmissionV1<RuntimePeerCopyV1>],
        timeout: Duration,
        environment: Gfx942XgmiRetainedPairEnvironmentAssumptionV1,
    ) -> Result<
        RuntimeNativeRetainedPeerCopyBatchV1<'context, 'roster, 'handle>,
        RuntimeErrorV1<KfdRuntimeBackendErrorV1>,
    > {
        self.require_live()?;
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(RuntimeValidationErrorV1::InvalidDeadline)?;
        let backend_ids = self.retained_pair_roster_v1(submissions)?;
        self.validate_peer_copy_batch_roots(submissions)?;
        let token = self.next_id()?;
        self.native_pair_reservation = Some(token);
        let result = self.invoke_journal_backend_v1(|backend| {
            backend.begin_retained_peer_batch_v1(&backend_ids, deadline, environment)
        });
        if let Err(failure) = result {
            let error = self.observe_peer_copy_batch_result_v1(submissions, Err(failure));
            if !self.terminal {
                self.native_pair_reservation = None;
            }
            return Err(error.err().unwrap_or_else(|| std::process::abort()));
        }
        Ok(RuntimeNativeRetainedPeerCopyBatchV1 {
            context: self,
            submissions,
            token,
            finished: false,
        })
    }
}

impl RuntimeNativeRetainedPeerCopyBatchV1<'_, '_, '_> {
    pub const fn profile(&self) -> &'static str {
        GFX942_XGMI_RETAINED_PAIR_PROFILE_V1
    }

    pub fn wait(
        &mut self,
    ) -> Result<RuntimeNativeRetainedPeerCopyPollV1, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.context.require_retained_pair_v1(self.token)?;
        let result = self
            .context
            .invoke_journal_backend_v1(|backend| backend.wait_retained_peer_batch_v1());
        self.context.backend_result(result)
    }

    /// Restores native owners and settles existing handles after successful close.
    /// Calling this while pending is a terminal refusal, never cancellation.
    pub fn finish(mut self) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.context.require_retained_pair_v1(self.token)?;
        let result = self
            .context
            .invoke_journal_backend_v1(|backend| backend.finish_retained_peer_batch_v1());
        self.context.backend_result(result)?;
        self.context.observe_peer_copy_batch_result_v1(
            self.submissions,
            Ok(RuntimePeerCopyBatchPollV1::Succeeded),
        )?;
        self.context.native_pair_reservation = None;
        self.finished = true;
        Ok(())
    }
}

impl Drop for RuntimeNativeRetainedPeerCopyBatchV1<'_, '_, '_> {
    fn drop(&mut self) {
        if !self.finished {
            self.context.quarantine_submission_writers_v1();
            self.context.backend.abandon_retained_peer_batch_v1();
        }
    }
}
