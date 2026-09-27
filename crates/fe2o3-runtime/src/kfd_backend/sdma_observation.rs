//! Observation never detaches the accepted copy descriptor from its custody index.

use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

impl KfdRuntimeBackendV1 {
    pub(super) fn sdma_completion_custody_is_intact_v1(&self, submission: u64) -> bool {
        let active = &self.active_sdma[&submission];
        if active.id != submission
            || !matches!(active.phase, ActiveSdmaPhaseV1::Quarantined)
            || active.source == active.destination
            || active.window_bytes == 0
            || active.window_requests.is_none()
            || active.dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1
            || active.dependency_cursor > active.dependencies.len()
            || self.sdma_completion_reservations == 0
            || self.submissions.contains_key(&submission)
            || self.submissions.len() == self.submissions.capacity()
            || self.published_sdma_submissions.contains(&submission)
        {
            return false;
        }
        // The admitted roster is unique but not sorted. Check uniqueness without
        // allocating or replacing its retained backing, before decrementing counts.
        let mut sorted = [0_u64; MAX_RUNTIME_DEPENDENCIES_V1];
        let dependencies = &mut sorted[..active.dependencies.len()];
        dependencies.copy_from_slice(&active.dependencies);
        dependencies.sort_unstable();
        if dependencies.windows(2).any(|pair| pair[0] == pair[1])
            || dependencies.iter().any(|id| {
                self.sdma_dependency_retain_counts
                    .get(id)
                    .is_none_or(|count| *count == 0)
                    || self
                        .submissions
                        .get(id)
                        .is_none_or(|record| record.status != BackendPollV1::Succeeded)
            })
        {
            return false;
        }
        let expected = RuntimeAllocationCustodyOwnerV1 {
            submission,
            stream: active.stream,
            kind: RuntimeAllocationCustodyKindV1::Sdma,
        };
        // Private admission/removal preserves ordered owner and stream indexes.
        // Validate local release prerequisites without rescanning an entire queue.
        [active.source, active.destination].into_iter().all(|id| {
            self.allocation_custody.get(&id).is_some_and(|custody| {
                custody.owner_counts[RuntimeAllocationCustodyKindV1::Sdma.index()] != 0
                    && custody
                        .owners
                        .binary_search_by_key(&submission, |owner| owner.submission)
                        .is_ok_and(|index| {
                            custody.owners[index] == expected
                                && index
                                    .checked_sub(1)
                                    .and_then(|prior| custody.owners.get(prior))
                                    .is_none_or(|owner| owner.submission != submission)
                                && custody
                                    .owners
                                    .get(index + 1)
                                    .is_none_or(|owner| owner.submission != submission)
                        })
            })
        }) && self
            .active_sdma_streams
            .get(&active.stream)
            .is_some_and(|queue| {
                queue.binary_search(&submission).is_ok_and(|index| {
                    index.checked_sub(1).and_then(|prior| queue.get(prior)) != Some(&submission)
                        && queue.get(index + 1) != Some(&submission)
                })
            })
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
