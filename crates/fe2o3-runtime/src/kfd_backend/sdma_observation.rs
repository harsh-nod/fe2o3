//! Observation never detaches the accepted copy descriptor from its custody index.

use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

impl KfdRuntimeBackendV1 {
    pub(super) fn sdma_completion_custody_is_intact_v1(&self, submission: u64) -> bool {
        let active = &self.active_sdma[&submission];
        matches!(active.phase, ActiveSdmaPhaseV1::Quarantined)
            && active.window_bytes != 0
            && active.window_requests.is_some()
            && active.dependencies.iter().all(|id| {
                self.submissions
                    .get(id)
                    .is_some_and(|record| record.status == BackendPollV1::Succeeded)
            })
            && self.sdma_release_custody_is_intact_v1(submission)
    }

    pub(super) fn observe_sdma_copy_v1(
        &mut self,
        submission: u64,
        timeout: Option<Duration>,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        match catch_unwind(AssertUnwindSafe(|| {
            self.observe_sdma_copy_inner_v1(submission, timeout)
        })) {
            Ok(result) => result,
            Err(payload) => super::sdma_host_write::resume_sdma_owner_panic_v1(payload, || {
                self.poison_terminal_v1();
            }),
        }
    }

    fn observe_sdma_copy_inner_v1(
        &mut self,
        submission: u64,
        timeout: Option<Duration>,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if matches!(
            self.active_sdma[&submission].phase,
            ActiveSdmaPhaseV1::Ready
        ) {
            return self.observe_unpublished_sdma_copy_v1(submission);
        }
        let phase = std::mem::replace(
            &mut self
                .active_sdma
                .get_mut(&submission)
                .expect("indexed SDMA copy")
                .phase,
            ActiveSdmaPhaseV1::Quarantined,
        );
        self.unindex_published_sdma_v1(submission);
        match phase {
            ActiveSdmaPhaseV1::DirectionalPublished(native) => {
                self.observe_directional_sdma_copy_v1(submission, *native, timeout)
            }
            ActiveSdmaPhaseV1::SameDevicePublished(native) => {
                self.observe_same_device_sdma_copy_v1(submission, *native, timeout)
            }
            ActiveSdmaPhaseV1::Ready | ActiveSdmaPhaseV1::Quarantined => {
                Err(self.terminal_error("SDMA observation found quarantined custody"))
            }
        }
    }

    fn observe_directional_sdma_copy_v1(
        &mut self,
        submission: u64,
        native: DirectionalSdmaSubmissionOwnerV1,
        timeout: Option<Duration>,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        #[cfg(feature = "hardware-diagnostic")]
        let mut diagnostic = None;
        let outcome = if let Some(timeout) = timeout {
            #[cfg(feature = "hardware-diagnostic")]
            let outcome = if let Some(policy) = self
                .directional_wait_diagnostic
                .as_ref()
                .map(|r| r.policy())
            {
                let (outcome, observed) = self
                    .directional_sdma_ops_v1()
                    .wait_with_diagnostics_v1(native, timeout, policy);
                diagnostic = observed;
                outcome
            } else {
                self.directional_sdma_ops_v1().wait(native, timeout)
            };
            #[cfg(not(feature = "hardware-diagnostic"))]
            let outcome = self.directional_sdma_ops_v1().wait(native, timeout);
            outcome.map(|outcome| match outcome {
                DirectionalSdmaWaitV1::Timeout(native) => DirectionalSdmaPollV1::Pending(native),
                DirectionalSdmaWaitV1::Completed(completed) => {
                    DirectionalSdmaPollV1::Completed(completed)
                }
            })
        } else {
            self.directional_sdma_ops_v1().poll(native)
        };
        match outcome {
            Ok(DirectionalSdmaPollV1::Pending(native)) => {
                self.active_sdma
                    .get_mut(&submission)
                    .expect("retained SDMA copy")
                    .phase = ActiveSdmaPhaseV1::DirectionalPublished(Box::new(native));
                self.index_published_sdma_v1(submission);
                Ok(BackendPollV1::Pending)
            }
            Ok(DirectionalSdmaPollV1::Completed(completed)) => {
                #[cfg(feature = "hardware-diagnostic")]
                let observation = diagnostic.map(|observed| {
                    KfdRuntimeDirectionalWaitObservationV1::new(
                        &self.active_sdma[&submission],
                        &completed,
                        observed,
                    )
                });
                let result = self.finish_sdma_copy_v1(submission, completed);
                #[cfg(feature = "hardware-diagnostic")]
                self.record_directional_wait_settlement_v1(result.is_ok(), observation);
                result
            }
            Err(DirectionalSdmaExecutionFailureV1::Retryable {
                detail,
                submission: native,
            }) => {
                self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Pending(
                    native,
                ));
                let operation = if timeout.is_some() {
                    "bounded wait returned non-timeout retryable custody"
                } else {
                    "completion observation returned foreign retryable custody"
                };
                Err(self.terminal_error(format!("KFD directional SDMA {operation}: {detail}")))
            }
            Err(DirectionalSdmaExecutionFailureV1::ProcessTeardown { detail, custody }) => {
                self.retain_sdma_seam_terminal_v1(custody);
                let operation = if timeout.is_some() {
                    "bounded wait"
                } else {
                    "completion observation"
                };
                Err(self.terminal_error(format!("KFD directional SDMA {operation}: {detail}")))
            }
        }
    }

    fn observe_same_device_sdma_copy_v1(
        &mut self,
        submission: u64,
        native: SameDeviceSdmaSubmissionOwnerV1,
        timeout: Option<Duration>,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let outcome = if let Some(timeout) = timeout {
            self.directional_sdma_ops_v1()
                .wait_same_device(native, timeout)
                .map(|outcome| match outcome {
                    SameDeviceSdmaWaitV1::Timeout(native) => SameDeviceSdmaPollV1::Pending(native),
                    SameDeviceSdmaWaitV1::Completed(completed) => {
                        SameDeviceSdmaPollV1::Completed(completed)
                    }
                })
        } else {
            self.directional_sdma_ops_v1().poll_same_device(native)
        };
        match outcome {
            Ok(SameDeviceSdmaPollV1::Pending(native)) => {
                self.active_sdma
                    .get_mut(&submission)
                    .expect("retained SDMA copy")
                    .phase = ActiveSdmaPhaseV1::SameDevicePublished(Box::new(native));
                self.index_published_sdma_v1(submission);
                Ok(BackendPollV1::Pending)
            }
            Ok(SameDeviceSdmaPollV1::Completed(completed)) => {
                let result = self.finish_same_device_sdma_copy_v1(submission, completed);
                #[cfg(feature = "hardware-diagnostic")]
                self.record_directional_wait_settlement_v1(result.is_ok(), None);
                result
            }
            Err(SameDeviceSdmaExecutionFailureV1::Retryable {
                detail,
                submission: native,
            }) => {
                self.retain_terminal_sdma_custody_v1(
                    KfdRuntimeTerminalSdmaCustodyV1::SameDevicePending(native),
                );
                let operation = if timeout.is_some() {
                    "bounded wait returned non-timeout retryable custody"
                } else {
                    "completion observation returned foreign retryable custody"
                };
                Err(self.terminal_error(format!("KFD same-device SDMA {operation}: {detail}")))
            }
            Err(SameDeviceSdmaExecutionFailureV1::ProcessTeardown { detail, custody }) => {
                self.retain_sdma_seam_terminal_v1(custody);
                let operation = if timeout.is_some() {
                    "bounded wait"
                } else {
                    "completion observation"
                };
                Err(self.terminal_error(format!("KFD same-device SDMA {operation}: {detail}")))
            }
        }
    }
}
