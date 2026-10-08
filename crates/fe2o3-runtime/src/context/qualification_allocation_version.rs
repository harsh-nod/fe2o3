//! Qualification-only, historical journal observations; never data authority.
use super::*;

/// Scalar snapshot of an original Context-owned, whole allocation at one instant.
/// This is not a reader, writer, replica, native receipt, or device-currentness proof.
/// It cannot authorize a later read or release; those operations revalidate owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAllocationVersionObservationV1 {
    allocation: RuntimeAllocationIdV1,
    device: RuntimeDeviceIdV1,
    byte_extent: u64,
    attempt_epoch: u64,
    content_lineage: u64,
}

impl RuntimeAllocationVersionObservationV1 {
    pub fn allocation(&self) -> RuntimeAllocationIdV1 {
        self.allocation
    }
    pub fn device(&self) -> RuntimeDeviceIdV1 {
        self.device
    }
    pub fn byte_extent(&self) -> u64 {
        self.byte_extent
    }
    pub fn attempt_epoch(&self) -> u64 {
        self.attempt_epoch
    }
    pub fn content_lineage(&self) -> u64 {
        self.content_lineage
    }
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    /// Observes only a live whole-region journal entry with no outstanding writer.
    /// Existing Context, scope, graph, allocation and original debit checks apply.
    /// No native operation or proof of hardware currentness is performed here.
    /// Success says nothing about initialization or the validity of any bytes.
    pub fn observe_allocation_version_qualification_v1(
        &self,
        region: RuntimeMemoryRegionV1,
    ) -> Result<RuntimeAllocationVersionObservationV1, RuntimeValidationErrorV1> {
        let stamp = self.replica_stamp_v1(region)?;
        Ok(RuntimeAllocationVersionObservationV1 {
            allocation: stamp.region.allocation,
            device: stamp.record.device,
            byte_extent: stamp.read.byte_extent,
            attempt_epoch: stamp.read.attempt_epoch,
            content_lineage: stamp.read.content_lineage,
        })
    }
}

#[cfg(test)]
mod tests;
