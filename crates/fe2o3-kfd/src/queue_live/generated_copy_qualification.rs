//! Immutable joins of original receipts and disjoint retained storage identities.
use super::*;

fn disjoint_host_identity_v1(
    generated: crate::shared_memory::SharedGttAllocationIdentityV1,
    copy: crate::shared_memory::SharedGttAllocationIdentityV1,
) -> bool {
    generated.same_retained_session_v1(copy) && !generated.same_retained_allocation_v1(copy)
}

impl ComputeAqlQueueSessionV1 {
    /// Qualification only: both original receipts remain retained, and the
    /// generated single write-only host DATA and directional-copy owners are
    /// distinct. No polling, I/O, mutation, or physical-overlap claim occurs.
    pub fn observe_generated_single_copy_disjoint_custody_v1(
        &self,
        lane: ComputeAqlQueueLaneV1,
        batch: &Gfx942DispatchBatchV1<1>,
        copy: &Gfx942DirectionalPersistentSdmaSubmissionV1,
    ) -> bool {
        if self.diagnose_r66_retained_single_copy_v1(copy).is_err() {
            return false;
        }
        self.sdma
            .as_ref()
            .and_then(|sdma| sdma.observe_directional_retained_request_v1(self.key, &[copy.ticket]))
            .is_some_and(|observed| {
                self.generated_copy_storage_is_disjoint_v1(lane, batch, observed)
            })
    }

    /// The same qualification boundary for one retained complete SDMA window.
    pub fn observe_generated_window_copy_disjoint_custody_v1(
        &self,
        lane: ComputeAqlQueueLaneV1,
        batch: &Gfx942DispatchBatchV1<1>,
        copy: &Gfx942DirectionalPersistentSdmaWindowSubmissionV1,
    ) -> bool {
        if self.diagnose_r66_retained_window_copy_v1(copy).is_err() {
            return false;
        }
        self.sdma
            .as_ref()
            .and_then(|sdma| sdma.observe_directional_retained_request_v1(self.key, &copy.tickets))
            .is_some_and(|observed| {
                self.generated_copy_storage_is_disjoint_v1(lane, batch, observed)
            })
    }

    fn generated_copy_storage_is_disjoint_v1(
        &self,
        lane: ComputeAqlQueueLaneV1,
        batch: &Gfx942DispatchBatchV1<1>,
        copy: crate::sdma::RetainedDirectionalSdmaObservationV1<'_>,
    ) -> bool {
        if lane.ordinal != 0
            || self
                .observe_retained_fixed_dispatch_v1(lane, batch)
                .is_none()
        {
            return false;
        }
        let Some(generated) = self.dispatch.as_ref().and_then(|dispatch| {
            dispatch.qualification_single_write_host_data_identity_v1(self.key.vm)
        }) else {
            return false;
        };
        let (host, device) = match (
            copy.source.storage_identity(),
            copy.destination.storage_identity(),
        ) {
            (
                Gfx942SdmaBufferStorageIdentityV1::Host(host),
                Gfx942SdmaBufferStorageIdentityV1::Device(device),
            )
            | (
                Gfx942SdmaBufferStorageIdentityV1::Device(device),
                Gfx942SdmaBufferStorageIdentityV1::Host(host),
            ) => (host, device),
            _ => return false,
        };
        // A different generation of the same host allocation is not disjoint.
        // Device-local and shared-GTT allocations have separate owning types;
        // both retained copy endpoints must still name this exact device/VM.
        disjoint_host_identity_v1(generated, host)
            && device.coexistence_facts_v1().is_some_and(|facts| {
                facts.physical_device == self.key.vm.device.physical.0
                    && facts.device_generation == self.key.vm.device.generation.0
                    && facts.vm_id == self.key.vm.id.0
                    && facts.allocation_id != 0
                    && facts.generation != 0
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_host_owner_disjointness_rejects_same_allocation() {
        let first = crate::shared_memory::mapped_host_for_persistent_sdma_test(1, 4096);
        let other = crate::shared_memory::mapped_host_for_persistent_sdma_test(2, 4096);
        let first = first.storage_identity();
        let other = other.storage_identity();
        assert!(!disjoint_host_identity_v1(first, first));
        assert!(disjoint_host_identity_v1(first, other));
    }
}
