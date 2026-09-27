//! Accepted copy descriptors stay indexed through preparation and native handoff.

use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

impl KfdRuntimeBackendV1 {
    pub(super) fn publish_sdma_copy_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        match catch_unwind(AssertUnwindSafe(|| {
            self.publish_sdma_copy_inner_v1(submission)
        })) {
            Ok(Err(failure @ RuntimeBackendFailureV1::Terminal(_))) => {
                self.seal_sdma_publication_v1(submission);
                Err(failure)
            }
            Ok(result) => result,
            Err(payload) => sdma_host_write::resume_sdma_owner_panic_v1(payload, || {
                self.seal_sdma_publication_v1(submission);
            }),
        }
    }

    fn seal_sdma_publication_v1(&mut self, submission: u64) {
        if let Some(active) = self.active_sdma.get_mut(&submission)
            && matches!(active.phase, ActiveSdmaPhaseV1::Ready)
        {
            active.phase = ActiveSdmaPhaseV1::Quarantined;
        }
        // A returned Published owner, if any, is already rooted and must not be
        // overwritten even if a later bookkeeping operation unwinds.
        self.poison_terminal_v1();
    }

    fn publish_sdma_copy_inner_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let active = &self.active_sdma[&submission];
        if !self.peer_dma_access_is_intact_v1(active) {
            return Err(
                self.terminal_error("private peer DMA lost its retained predecessor access")
            );
        }
        if !matches!(active.phase, ActiveSdmaPhaseV1::Ready) {
            return Err(self.terminal_error("SDMA publication requires retained Ready custody"));
        }
        if self.persistent_compute_is_active_v1()
            && !self.sdma_can_coexist_with_persistent_compute_v1(active)
        {
            return Ok(BackendPollV1::Pending);
        }
        let copy_kind = match self.direct_sdma_copy_kind_for_active_v1(active) {
            Ok(kind) => kind,
            Err(detail) => return Err(self.terminal_error(detail)),
        };
        let endpoints = [active.source, active.destination];
        self.active_sdma
            .get_mut(&submission)
            .expect("retained SDMA copy")
            .phase = ActiveSdmaPhaseV1::Quarantined;
        for allocation in endpoints {
            if let Err(failure) = self
                .normalize_h2d_ready_v1(allocation)
                .and_then(|()| self.synchronize_native_allocation_v1(allocation))
            {
                return self.fail_sdma_publication_v1(submission, failure);
            }
        }
        #[cfg(test)]
        let window = match self
            .scripted_sdma
            .as_ref()
            .and_then(|driver| driver.publication_byte_limit)
        {
            Some(limit) => direct_sdma_window_plan_with_limit_v1(
                &self.active_sdma[&submission],
                u64::from(limit),
            ),
            None => direct_sdma_window_plan_v1(&self.active_sdma[&submission]),
        };
        #[cfg(not(test))]
        let window = direct_sdma_window_plan_v1(&self.active_sdma[&submission]);
        let Some(window) = window else {
            return self.fail_sdma_publication_v1(
                submission,
                Self::capacity("KFD SDMA window planning failed"),
            );
        };
        let publication = match copy_kind {
            DirectSdmaCopyKindV1::Directional(direction) => {
                directional_sdma_requests_v1(&window.requests, direction)
                    .map(|requests| EitherSdmaWindowRequestsV1::Directional(direction, requests))
            }
            DirectSdmaCopyKindV1::SameDevice => same_device_sdma_requests_v1(&window.requests)
                .map(EitherSdmaWindowRequestsV1::SameDevice),
        };
        // Record attempted request intent before any move of native buffer owners.
        let active = self
            .active_sdma
            .get_mut(&submission)
            .expect("retained SDMA copy");
        active.window_bytes = window.copy_bytes;
        active.window_requests = Some(window.requests);
        match publication {
            Some(EitherSdmaWindowRequestsV1::Directional(direction, requests)) => {
                self.publish_directional_sdma_window_v1(submission, direction, requests)
            }
            Some(EitherSdmaWindowRequestsV1::SameDevice(requests)) => {
                self.publish_same_device_sdma_window_v1(submission, requests)
            }
            None => self.fail_sdma_publication_v1(
                submission,
                Self::capacity("KFD SDMA window metadata allocation failed"),
            ),
        }
    }

    fn fail_sdma_publication_v1(
        &mut self,
        submission: u64,
        failure: RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        // Terminal is never evidence of quiescence, even after a completed prefix.
        if matches!(failure, RuntimeBackendFailureV1::Terminal(_)) {
            return Err(failure);
        }
        if self.active_sdma[&submission].completed_bytes != 0 {
            self.fail_quiescent_sdma_copy_v1(submission)?;
            Err(match failure {
                RuntimeBackendFailureV1::Rejected(error) => {
                    RuntimeBackendFailureV1::Quiescent(error)
                }
                failure => failure,
            })
        } else {
            self.fail_unpublished_sdma_copy_v1(submission)
        }
    }

    fn publish_directional_sdma_window_v1(
        &mut self,
        submission: u64,
        direction: Gfx942PersistentSdmaDirectionV1,
        requests: DirectionalSdmaRequestPlanV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let binding = SdmaStorageBindingV1::from(&self.active_sdma[&submission]);
        let pair = match self.take_directional_sdma_storage_v1(
            binding,
            direction,
            KfdRuntimeSdmaInFlightV1::Async(submission),
        ) {
            Ok(pair) => pair,
            Err(failure) => return self.fail_sdma_publication_v1(submission, failure),
        };
        match self
            .directional_sdma_ops_v1()
            .submit(pair, direction, requests)
        {
            Ok(native) => {
                self.active_sdma
                    .get_mut(&submission)
                    .expect("retained SDMA copy")
                    .phase = ActiveSdmaPhaseV1::DirectionalPublished(Box::new(native));
                self.index_published_sdma_v1(submission);
                #[cfg(feature = "hardware-qualification")]
                self.record_drain_capture_publication_v1(submission);
                Ok(BackendPollV1::Pending)
            }
            Err(SdmaTransitionFailureV1::Retryable { detail, custody }) => {
                self.restore_directional_sdma_storage_v1(
                    binding,
                    direction,
                    KfdRuntimeSdmaInFlightV1::Async(submission),
                    custody,
                    false,
                )?;
                self.fail_sdma_publication_v1(
                    submission,
                    Self::rejected(
                        KfdRuntimeBackendErrorKindV1::Native,
                        format!("KFD directional SDMA publication: {detail}"),
                    ),
                )
            }
            Err(SdmaTransitionFailureV1::ProcessTeardown { detail, custody }) => {
                self.retain_sdma_seam_terminal_v1(custody);
                Err(self.terminal_error(format!("KFD directional SDMA publication: {detail}")))
            }
        }
    }

    fn publish_same_device_sdma_window_v1(
        &mut self,
        submission: u64,
        requests: Box<[SameDeviceSdmaCopyRequestV1]>,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let binding = SdmaStorageBindingV1::from(&self.active_sdma[&submission]);
        let pair = match self
            .take_same_device_sdma_storage_v1(binding, KfdRuntimeSdmaInFlightV1::Async(submission))
        {
            Ok(pair) => pair,
            Err(failure) => return self.fail_sdma_publication_v1(submission, failure),
        };
        match self
            .directional_sdma_ops_v1()
            .submit_same_device(pair, requests)
        {
            Ok(native) => {
                self.active_sdma
                    .get_mut(&submission)
                    .expect("retained SDMA copy")
                    .phase = ActiveSdmaPhaseV1::SameDevicePublished(Box::new(native));
                self.index_published_sdma_v1(submission);
                #[cfg(feature = "hardware-qualification")]
                self.record_drain_capture_publication_v1(submission);
                Ok(BackendPollV1::Pending)
            }
            Err(SdmaTransitionFailureV1::Retryable { detail, custody }) => {
                self.restore_same_device_sdma_storage_v1(
                    binding,
                    KfdRuntimeSdmaInFlightV1::Async(submission),
                    custody,
                    false,
                )?;
                self.fail_sdma_publication_v1(
                    submission,
                    Self::rejected(
                        KfdRuntimeBackendErrorKindV1::Native,
                        format!("KFD same-device SDMA publication: {detail}"),
                    ),
                )
            }
            Err(SdmaTransitionFailureV1::ProcessTeardown { detail, custody }) => {
                self.retain_sdma_seam_terminal_v1(custody);
                Err(self.terminal_error(format!("KFD same-device SDMA publication: {detail}")))
            }
        }
    }
}
