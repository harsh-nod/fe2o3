//! Borrowed native custody checks for primary persistent compute and the exact
//! directional SDMA pair. No result survives a mutation or authorizes retirement.

use arrayvec::ArrayVec;
use fe2o3_runtime_model::{
    R66DeviceDomainV1, R66DeviceStorageV1, r66_device_storage_rosters_disjoint_v1,
};

use super::super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1;
use super::*;

impl ComputeAqlQueueSessionV1 {
    fn coexistence_domain_v1(&self) -> R66DeviceDomainV1 {
        R66DeviceDomainV1 {
            physical_device: self.key.vm.device.physical.0,
            device_generation: self.key.vm.device.generation.0,
            vm_id: self.key.vm.id.0,
        }
    }

    fn coexistence_primary_profile_v1(&self) -> bool {
        !self.terminal_poisoned
            && self.key == self.compute_lane_session
            && self.striped_sdma.is_none()
            && auxiliary_compute_lanes_are_quiescent_v1(&self.auxiliary_compute_lanes)
    }

    fn coexistence_allocation_facts_v1(
        &self,
        allocation: &Gfx942DirectionalQueuePersistentAllocationV1,
    ) -> Option<R66DeviceStorageV1> {
        if !self.directional_persistent_sdma_attachment_is_current(&allocation.attachment)
            || allocation.attachment.pool_generation == 0
            || allocation.byte_len() == 0
            || allocation.byte_len() > allocation.physical_byte_len()
            || allocation.owner.byte_len() != allocation.physical_byte_len()
            || allocation.owner.quarantine_reason().is_some()
        {
            return None;
        }
        let native = allocation.owner.local_native_for_sdma()?;
        if allocation.attachment.storage_identity
            != Gfx942SdmaBufferStorageIdentityV1::Device(native.storage_identity())
            || native.layout().requested_bytes() != allocation.physical_byte_len()
        {
            return None;
        }
        native.storage_identity().coexistence_facts_v1()
    }

    pub(super) fn persistent_inputs_coexist_with_directional_sdma_v1(
        &self,
        inputs: &[&Gfx942PersistentComputeInputV1],
    ) -> bool {
        if !self.coexistence_primary_profile_v1() || self.persistent_compute.is_some() {
            return false;
        }
        let Some(copy_devices) = self
            .sdma
            .as_ref()
            .and_then(|sdma| sdma.compute_coexistence_endpoints_v1(self.key))
        else {
            return false;
        };
        let mut compute = ArrayVec::<R66DeviceStorageV1, MAX_DISPATCH_DATA_LEASES_V1>::new();
        for input in inputs {
            let allocation = match input {
                Gfx942PersistentComputeInputV1::Uninitialized(allocation)
                | Gfx942PersistentComputeInputV1::InitializedAfterDispatch(allocation) => {
                    allocation
                }
                Gfx942PersistentComputeInputV1::Initialized(ready) => &ready.allocation,
            };
            if allocation.owner.live_use_count() != 0
                || allocation.owner.retained_settled_use_count() != 0
                || allocation.byte_len() != allocation.physical_byte_len()
            {
                return false;
            }
            let Some(identity) = self.coexistence_allocation_facts_v1(allocation) else {
                return false;
            };
            if compute.try_push(identity).is_err() {
                return false;
            }
        }
        r66_device_storage_rosters_disjoint_v1(
            self.coexistence_domain_v1(),
            &compute,
            &copy_devices,
        )
    }

    pub(super) fn directional_sdma_coexists_with_persistent_compute_v1(
        &self,
        allocation: &Gfx942DirectionalQueuePersistentAllocationV1,
        host: &Gfx942SdmaBufferV1,
    ) -> bool {
        let Some(attachment) = &self.persistent_compute else {
            return true;
        };
        if !self.coexistence_primary_profile_v1()
            || attachment.terminal_custody.is_some()
            || attachment.binding.queue != self.key
            || attachment.binding.attachment_generation.checked_add(1)
                != Some(self.next_persistent_compute_generation)
            || (!attachment.is_single() && !attachment.is_three())
        {
            return false;
        }
        let Some(sdma) = self.sdma.as_ref() else {
            return false;
        };
        let Some(mut copy_devices) = sdma.compute_coexistence_endpoints_v1(self.key) else {
            return false;
        };
        if !sdma.compute_coexistence_host_is_current_v1(self.key, host) {
            return false;
        }
        let Some(copy_device) = self.coexistence_allocation_facts_v1(allocation) else {
            return false;
        };
        if copy_devices.try_push(copy_device).is_err() {
            return false;
        }
        let mut identities =
            ArrayVec::<Gfx942DeviceMemoryIdentityV1, MAX_DISPATCH_DATA_LEASES_V1>::new();
        let mut compute = ArrayVec::<R66DeviceStorageV1, MAX_DISPATCH_DATA_LEASES_V1>::new();
        for entry in &attachment.entries {
            if entry.allocation.owner.quarantine_reason().is_some()
                || !self
                    .directional_persistent_sdma_attachment_is_current(&entry.allocation.attachment)
                || entry.allocation.attachment.pool_generation == 0
                || entry.allocation.byte_len() == 0
                || entry.allocation.byte_len() != entry.allocation.physical_byte_len()
                || entry.allocation.owner.byte_len() != entry.allocation.physical_byte_len()
                || entry.allocation.owner.local_native_for_sdma().is_some()
                || entry.allocation.owner.live_use_count() != 1
                || entry.allocation.owner.retained_settled_use_count() != 0
                || !matches!(
                    entry.state,
                    PersistentComputeUseStateV1::Prepared(_)
                        | PersistentComputeUseStateV1::Published(_)
                        | PersistentComputeUseStateV1::Completed(_)
                        | PersistentComputeUseStateV1::Recycled(_)
                )
            {
                return false;
            }
            let Some(identity) = entry.storage_identity else {
                return false;
            };
            if entry.allocation.attachment.storage_identity
                != Gfx942SdmaBufferStorageIdentityV1::Device(identity)
            {
                return false;
            }
            let Some(facts) = identity.coexistence_facts_v1() else {
                return false;
            };
            identities.push(identity);
            compute.push(facts);
        }
        self.dispatch
            .as_ref()
            .is_some_and(|dispatch| dispatch.persistent_device_roster_matches_v1(&identities))
            && r66_device_storage_rosters_disjoint_v1(
                self.coexistence_domain_v1(),
                &compute,
                &copy_devices,
            )
    }
}
