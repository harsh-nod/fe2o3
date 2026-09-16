//! Promotion errors retain typed diagnostics until their owner is rooted.

#![forbid(unsafe_code)]

use super::kfd_backend_sdma_seam::{DirectionalSdmaOpsV1, SdmaPromotionDiagnosticV1};
use super::sdma_host_write::resume_sdma_owner_panic_v1;
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

impl KfdRuntimeBackendV1 {
    // Failure custody stays inline; allocating a box on the failure path is not required.
    #[allow(clippy::result_large_err)]
    pub(super) fn promote_sdma_buffer_v1(
        &mut self,
        buffer: SdmaBufferOwnerV1,
    ) -> Result<DirectionalSdmaDeviceOwnerV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Buffer(buffer));
        let result = catch_unwind(AssertUnwindSafe(|| {
            // Select the driver before transferring custody; a missing queue cannot lose it.
            #[cfg(test)]
            let mut ops = if let Some(driver) = self.scripted_sdma.as_mut() {
                DirectionalSdmaOpsV1::Scripted(driver)
            } else {
                DirectionalSdmaOpsV1::Native(self.queue.as_mut().expect("promotion retains queue"))
            };
            #[cfg(not(test))]
            let mut ops =
                DirectionalSdmaOpsV1::Native(self.queue.as_mut().expect("promotion retains queue"));
            let Some(KfdRuntimeTerminalSdmaCustodyV1::Buffer(buffer)) =
                self.terminal_sdma_custody.take()
            else {
                unreachable!("installed promotion input");
            };
            ops.promote(buffer)
        }));
        match result {
            Ok(Ok(allocation)) => Ok(allocation),
            Ok(Err(failure)) => Err(self.settle_sdma_promotion_failure_v1(failure, |detail| {
                format!("KFD persistent device promotion: {detail}")
            })),
            Err(payload) => resume_sdma_owner_panic_v1(payload, || self.poison_terminal_v1()),
        }
    }

    pub(super) fn settle_sdma_promotion_failure_v1(
        &mut self,
        failure: SdmaTransitionFailureV1<SdmaBufferOwnerV1, SdmaPromotionDiagnosticV1>,
        format: impl FnOnce(&SdmaPromotionDiagnosticV1) -> String,
    ) -> RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1> {
        let (detail, terminal) = match failure {
            SdmaTransitionFailureV1::Retryable { detail, custody } => {
                self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Buffer(
                    custody,
                ));
                (detail, false)
            }
            SdmaTransitionFailureV1::ProcessTeardown { detail, custody } => {
                self.retain_sdma_seam_terminal_v1(custody);
                (detail, true)
            }
        };
        let formatted = catch_unwind(AssertUnwindSafe(|| {
            if terminal {
                self.poison_terminal_v1();
            }
            format(&detail)
        }));
        let message = match formatted {
            Ok(message) => message,
            Err(payload) => resume_sdma_owner_panic_v1(payload, || self.poison_terminal_v1()),
        };
        if terminal {
            return RuntimeBackendFailureV1::Terminal(KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Terminal,
                message,
            ));
        }
        let Some(KfdRuntimeTerminalSdmaCustodyV1::Buffer(buffer)) =
            self.terminal_sdma_custody.take()
        else {
            unreachable!("retryable promotion retains buffer until diagnostics settle");
        };
        match self.recycle_transient_sdma_buffer_v1(buffer, "promotion") {
            Ok(()) => Self::rejected(KfdRuntimeBackendErrorKindV1::Native, message),
            Err(failure) => failure,
        }
    }
}
