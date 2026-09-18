//! Readback retains borrowed staging until copying and recycling have settled.

#![forbid(unsafe_code)]

use super::kfd_backend_sdma_seam::DirectionalSdmaOpsV1;
use super::sdma_host_write::resume_sdma_owner_panic_v1;
use super::sdma_recycle::SdmaRecycleTargetV1;
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

impl KfdRuntimeBackendV1 {
    pub(super) fn read_indexed_sdma_host_into_v1(
        &mut self,
        allocation: u64,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.access_indexed_sdma_host_v1(allocation, "KFD persistent host read", |ops, buffer| {
            ops.read_host_into(buffer, offset, destination)
        })
    }

    pub(super) fn readback_and_recycle_transient_sdma_v1(
        &mut self,
        buffer: SdmaBufferOwnerV1,
        destination: &mut [u8],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Buffer(buffer));
        let result = catch_unwind(AssertUnwindSafe(|| {
            let Some(KfdRuntimeTerminalSdmaCustodyV1::Buffer(buffer)) = &self.terminal_sdma_custody
            else {
                unreachable!("readback retains the staging buffer");
            };
            #[cfg(test)]
            let mut ops = if let Some(driver) = self.scripted_sdma.as_mut() {
                DirectionalSdmaOpsV1::Scripted(driver)
            } else {
                DirectionalSdmaOpsV1::Native(
                    self.queue.as_mut().expect("SDMA readback retains queue"),
                )
            };
            #[cfg(not(test))]
            let mut ops = DirectionalSdmaOpsV1::Native(
                self.queue.as_mut().expect("SDMA readback retains queue"),
            );
            ops.read_host_into(buffer, 0, destination)
        }));
        let readback = match result {
            Ok(result) => result,
            Err(payload) => resume_sdma_owner_panic_v1(payload, || self.poison_terminal_v1()),
        };
        // Ordinary read failures still attempt cleanup; cleanup failure has precedence.
        self.recycle_rooted_sdma_owner_v1(SdmaRecycleTargetV1::Transient("download"))?;
        readback.map_err(|error| self.terminal_error(format!("KFD download readback: {error}")))
    }
}
