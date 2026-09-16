//! Recycle diagnostics never outlive unrooted input or recovered buffer custody.

#![forbid(unsafe_code)]

use super::kfd_backend_sdma_seam::{DirectionalSdmaOpsV1, SdmaOwnerDiagnosticV1};
use super::sdma_host_write::resume_sdma_owner_panic_v1;
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(Clone, Copy)]
pub(super) enum SdmaRecycleTargetV1 {
    Transient(&'static str),
    Indexed {
        allocation: u64,
        kind: RuntimeMemoryKindV1,
    },
}

impl KfdRuntimeBackendV1 {
    #[allow(
        clippy::result_large_err,
        reason = "returned-owner failures cross the unwind boundary without allocating"
    )]
    pub(super) fn recycle_sdma_owner_v1(
        &mut self,
        buffer: SdmaBufferOwnerV1,
        target: SdmaRecycleTargetV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Buffer(buffer));
        let result = catch_unwind(AssertUnwindSafe(|| {
            #[cfg(test)]
            let mut ops = if let Some(driver) = self.scripted_sdma.as_mut() {
                DirectionalSdmaOpsV1::Scripted(driver)
            } else {
                DirectionalSdmaOpsV1::Native(self.queue.as_mut().expect("recycle retains queue"))
            };
            #[cfg(not(test))]
            let mut ops =
                DirectionalSdmaOpsV1::Native(self.queue.as_mut().expect("recycle retains queue"));
            let Some(KfdRuntimeTerminalSdmaCustodyV1::Buffer(buffer)) =
                self.terminal_sdma_custody.take()
            else {
                unreachable!("installed recycle input");
            };
            ops.recycle(buffer)
        }));
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(failure)) => Err(self.settle_sdma_recycle_failure_v1(
                target,
                failure,
                |detail, recovered| match target {
                    SdmaRecycleTargetV1::Transient(operation) => {
                        format!("KFD {operation} transient release became ambiguous: {detail}")
                    }
                    SdmaRecycleTargetV1::Indexed { .. } if recovered => {
                        format!("KFD persistent allocation recycle rejected: {detail}")
                    }
                    SdmaRecycleTargetV1::Indexed { .. } => {
                        format!("KFD persistent allocation recycle became ambiguous: {detail}")
                    }
                },
            )),
            Err(payload) => resume_sdma_owner_panic_v1(payload, || self.poison_terminal_v1()),
        }
    }

    pub(super) fn settle_sdma_recycle_failure_v1(
        &mut self,
        target: SdmaRecycleTargetV1,
        failure: SdmaRecycleFailureV1,
        format: impl FnOnce(&SdmaOwnerDiagnosticV1, bool) -> String,
    ) -> RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1> {
        let (detail, recovered) = match failure {
            SdmaRecycleFailureV1::Recovered { detail, buffer } => {
                self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Buffer(
                    buffer,
                ));
                (detail, true)
            }
            SdmaRecycleFailureV1::Ambiguous { detail } => (detail, false),
            #[cfg(test)]
            SdmaRecycleFailureV1::ProcessTeardown { detail, custody } => {
                self.retain_sdma_seam_terminal_v1(custody);
                (detail, false)
            }
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            let terminal = !recovered || matches!(target, SdmaRecycleTargetV1::Transient(_));
            if terminal {
                self.poison_terminal_v1();
            }
            let message = format(&detail, recovered);
            if terminal {
                return RuntimeBackendFailureV1::Terminal(KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    message,
                ));
            }
            let SdmaRecycleTargetV1::Indexed { allocation, kind } = target else {
                unreachable!("retryable indexed recycle");
            };
            if !self.allocations.get(&allocation).is_some_and(|record| {
                record.kind == kind
                    && matches!(
                        record.sdma_storage,
                        KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous)
                    )
            }) {
                return self
                    .terminal_error("recovered recycle allocation slot changed unexpectedly");
            }
            let Some(KfdRuntimeTerminalSdmaCustodyV1::Buffer(buffer)) =
                self.terminal_sdma_custody.take()
            else {
                unreachable!("recovered buffer stays rooted through diagnostics");
            };
            self.allocations
                .get_mut(&allocation)
                .expect("validated recycle allocation slot")
                .sdma_storage = match kind {
                RuntimeMemoryKindV1::HostVisible => KfdRuntimeSdmaStorageV1::Host(buffer),
                RuntimeMemoryKindV1::DeviceLocal => KfdRuntimeSdmaStorageV1::DemotedDevice(buffer),
            };
            Self::quiescent_error(KfdRuntimeBackendErrorKindV1::Native, message)
        }));
        match result {
            Ok(failure) => failure,
            Err(payload) => resume_sdma_owner_panic_v1(payload, || self.poison_terminal_v1()),
        }
    }
}
