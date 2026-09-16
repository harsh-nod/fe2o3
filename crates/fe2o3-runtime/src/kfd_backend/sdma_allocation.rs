//! Native allocation disposition survives driver selection and diagnostic unwind.

#![forbid(unsafe_code)]

use super::kfd_backend_sdma_seam::{SdmaAllocationFailureV1, SdmaOwnerDiagnosticV1};
use super::sdma_host_write::resume_sdma_owner_panic_v1;
use super::*;
use fe2o3_kfd::Gfx942SdmaAllocationDispositionV1;
use std::panic::{AssertUnwindSafe, catch_unwind};

impl KfdRuntimeBackendV1 {
    pub(super) fn sdma_allocation_ready_v1(&self) -> bool {
        let ready = self.queue.is_some();
        #[cfg(test)]
        let ready = ready || self.scripted_sdma.is_some();
        ready && self.sdma_enabled
    }

    pub(super) fn allocate_sdma_owner_v1(
        &mut self,
        kind: RuntimeMemoryKindV1,
        byte_len: usize,
        alignment: u64,
        ready_before: bool,
        operation: &'static str,
    ) -> Result<SdmaBufferOwnerV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let result = match kind {
                RuntimeMemoryKindV1::HostVisible => {
                    self.directional_sdma_ops_v1().allocate_host(byte_len)
                }
                RuntimeMemoryKindV1::DeviceLocal => self
                    .directional_sdma_ops_v1()
                    .allocate_device_buffer(byte_len as u64, alignment),
            };
            result.map_err(|failure| {
                self.settle_sdma_allocation_failure_v1(failure, ready_before, operation, |detail| {
                    detail.to_string()
                })
            })
        }));
        match result {
            Ok(result) => result,
            Err(payload) => resume_sdma_owner_panic_v1(payload, || {
                if !self.terminal {
                    self.poison_terminal_v1();
                }
            }),
        }
    }

    pub(super) fn settle_sdma_allocation_failure_v1(
        &mut self,
        failure: SdmaAllocationFailureV1,
        ready_before: bool,
        operation: &'static str,
        diagnostic: impl FnOnce(&SdmaOwnerDiagnosticV1) -> String,
    ) -> RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1> {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let retryable =
                failure.disposition == Gfx942SdmaAllocationDispositionV1::RetryableCapacity;
            if !retryable {
                self.poison_terminal_v1();
            }
            let message = format!("{operation}: {}", diagnostic(&failure.detail));
            if !retryable {
                return RuntimeBackendFailureV1::Terminal(KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    message,
                ));
            }
            if ready_before {
                Self::rejected(KfdRuntimeBackendErrorKindV1::Capacity, message)
            } else {
                // Queue creation was visible, but the admitted empty queues are idle.
                Self::quiescent_error(KfdRuntimeBackendErrorKindV1::Capacity, message)
            }
        }));
        match result {
            Ok(failure) => failure,
            Err(payload) => resume_sdma_owner_panic_v1(payload, || self.poison_terminal_v1()),
        }
    }
}
