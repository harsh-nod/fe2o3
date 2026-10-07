//! Device demotion retains input, diagnostics and allocation-free retry restoration.

#![forbid(unsafe_code)]

use super::kfd_backend_sdma_seam::{DirectionalSdmaOpsV1, SdmaOwnerDiagnosticV1};
use super::sdma_host_write::resume_sdma_owner_panic_v1;
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

impl KfdRuntimeBackendV1 {
    #[allow(clippy::result_large_err)]
    pub(super) fn demote_sdma_device_v1(
        &mut self,
        allocation: u64,
        device: Box<DirectionalSdmaDeviceOwnerV1>,
    ) -> Result<SdmaBufferOwnerV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let (device, shell) = take_restore_shell_v1(device);
        self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Device(device));
        let result = catch_unwind(AssertUnwindSafe(|| {
            #[cfg(test)]
            let mut ops = if let Some(driver) = self.scripted_sdma.as_mut() {
                DirectionalSdmaOpsV1::Scripted(driver)
            } else {
                DirectionalSdmaOpsV1::Native(self.queue.as_mut().expect("demotion retains queue"))
            };
            #[cfg(not(test))]
            let mut ops =
                DirectionalSdmaOpsV1::Native(self.queue.as_mut().expect("demotion retains queue"));
            let Some(KfdRuntimeTerminalSdmaCustodyV1::Device(device)) =
                self.terminal_sdma_custody.take()
            else {
                unreachable!("installed demotion input");
            };
            ops.demote(device)
        }));
        match result {
            Ok(Ok(buffer)) => Ok(buffer),
            Ok(Err(failure)) => Err(self.settle_sdma_demotion_failure_v1(
                allocation,
                shell,
                failure,
                |detail| format!("KFD persistent device demotion: {detail}"),
            )),
            Err(payload) => resume_sdma_owner_panic_v1(payload, || self.poison_terminal_v1()),
        }
    }

    pub(super) fn settle_sdma_demotion_failure_v1(
        &mut self,
        allocation: u64,
        shell: Box<MaybeUninit<DirectionalSdmaDeviceOwnerV1>>,
        failure: SdmaTransitionFailureV1<DirectionalSdmaDeviceOwnerV1, SdmaOwnerDiagnosticV1>,
        format: impl FnOnce(&SdmaOwnerDiagnosticV1) -> String,
    ) -> RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1> {
        let (detail, terminal) = match failure {
            SdmaTransitionFailureV1::Retryable { detail, custody } => {
                self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Device(
                    custody,
                ));
                (detail, false)
            }
            SdmaTransitionFailureV1::ProcessTeardown { detail, custody } => {
                self.retain_sdma_seam_terminal_v1(custody);
                (detail, true)
            }
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            if terminal {
                self.poison_terminal_v1();
            }
            let message = format(&detail);
            if terminal {
                return RuntimeBackendFailureV1::Terminal(KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    message,
                ));
            }
            if !self.allocations.get(&allocation).is_some_and(|record| {
                record.kind == RuntimeMemoryKindV1::DeviceLocal
                    && matches!(
                        record.sdma_storage,
                        KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous)
                    )
            }) {
                return self
                    .terminal_error("recovered demotion allocation slot changed unexpectedly");
            }
            let Some(KfdRuntimeTerminalSdmaCustodyV1::Device(device)) =
                self.terminal_sdma_custody.take()
            else {
                unreachable!("recovered device stays rooted through diagnostics");
            };
            self.allocations
                .get_mut(&allocation)
                .expect("validated demotion allocation slot")
                .sdma_storage =
                KfdRuntimeSdmaStorageV1::Device(fill_restore_shell_v1(shell, device));
            Self::quiescent_error(KfdRuntimeBackendErrorKindV1::Native, message)
        }));
        match result {
            Ok(failure) => failure,
            Err(payload) => resume_sdma_owner_panic_v1(payload, || self.poison_terminal_v1()),
        }
    }
}
