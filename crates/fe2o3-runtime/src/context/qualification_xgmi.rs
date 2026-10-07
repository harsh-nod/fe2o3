//! Exact-operation hardware qualification hooks; no memory or launch authority.

use super::*;
use crate::kfd_backend::{KfdMultiDeviceRuntimeBackendV1, KfdRuntimeBackendErrorV1};

impl RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1> {
    fn qualification_xgmi_submission_v1<A>(
        &mut self,
        submission: &RuntimeSubmissionV1<A>,
    ) -> Result<SubmissionRecordV1, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        self.require_ordinary_submission_v1(submission.id)?;
        let record = self.live_submission_record(submission)?;
        self.require_retained_submission_unheld_v1(&record)?;
        self.check_operation_custody_v1(submission.id)?;
        if record.directed_peer_copy {
            return Err(RuntimeValidationErrorV1::Unsupported.into());
        }
        Ok(record)
    }

    /// Requests a real host-capacity rejection at this native peer's first queue
    /// creation, before device-visible queue creation is armed.
    ///
    /// Available only for hardware qualification. The exact ordinary peer must
    /// still be pending and retain its original local owners without extraction.
    /// Rearming, nonnative work, and already-started work are rejected without
    /// progress. Cancellation or dependency failure may leave the request unused.
    /// This does not change journal failure semantics or authorize allocation reuse.
    pub fn reject_native_xgmi_host_preparation_once_for_qualification_v1<A>(
        &mut self,
        submission: &RuntimeSubmissionV1<A>,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        let record = self.qualification_xgmi_submission_v1(submission)?;
        if record.quiescent || record.status != RuntimeCompletionStatusV1::Pending {
            return Err(RuntimeValidationErrorV1::Unsupported.into());
        }
        let result = self.invoke_journal_backend_v1(|backend| {
            backend.reject_native_xgmi_host_preparation_once_for_qualification_v1(
                record.backend_submission,
            )
        });
        self.backend_result(result)
    }

    /// Observes whether this exact retained result consumed the qualification
    /// request and received the lower host-preparation rejection classification.
    ///
    /// `true` additionally requires observed failure, quiescence, restored native
    /// owners, and cleared endpoint reservations. Pending work, cancellation,
    /// dependency failure, and scripted rejections cannot satisfy this observation.
    /// No fence is sampled and no progress or journal transition occurs. Keep the
    /// original allocations and stream live until this observation; it is a
    /// historical diagnostic, not a reusable custody or content certificate.
    pub fn native_xgmi_host_preparation_rejected_for_qualification_v1<A>(
        &mut self,
        submission: &RuntimeSubmissionV1<A>,
    ) -> Result<bool, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        let record = self.qualification_xgmi_submission_v1(submission)?;
        let result = self.invoke_journal_backend_v1(|backend| {
            backend.native_xgmi_host_preparation_rejected_for_qualification_v1(
                record.backend_submission,
            )
        });
        let classified = self.backend_result(result)?;
        Ok(classified
            && record.quiescent
            && matches!(record.status, RuntimeCompletionStatusV1::Failed(_)))
    }

    #[cfg(test)]
    fn qualification_xgmi_test_context_v1() -> Self {
        Self::open(KfdMultiDeviceRuntimeBackendV1::qualification_xgmi_test_backend_v1()).unwrap()
    }
}

#[cfg(test)]
mod tests;
