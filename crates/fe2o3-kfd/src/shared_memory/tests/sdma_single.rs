//! Forward the single-copy algorithm to genuine fixture mappings and engine checks.

use super::preparation::PreparationMemoryFixtureV1;
use super::*;
use crate::sdma::{
    MappedHostBufferV1, SdmaControlAuthorityV1, SdmaRingAuthorityV1, SdmaSingleMemoryV1,
};

impl SdmaSingleMemoryV1 for PreparationMemoryFixtureV1 {
    fn check_queue_operational_currentness(&mut self) -> Result<(), MemorySessionError> {
        self.fixture.engine.require_active()?;
        self.fixture.engine.check_operational_currentness()
    }
    fn observe_aql_control_counters_in_current_scope(
        &mut self,
        control: &mut SdmaControlAuthorityV1,
    ) -> Result<(u64, u64), MemorySessionError> {
        self.fixture
            .engine
            .observe_aql_counters_in_current_scope(&mut control.token)
    }
    fn single_host_facts(
        &self,
        token: &MappedHostBufferV1,
    ) -> Result<SharedGttMappedResourceFactsV1, MemorySessionError> {
        self.fixture
            .engine
            .mapped_resource_facts_v1(token, self.fixture.vm)
    }
    fn single_device_facts(
        &self,
        lease: &Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Result<Gfx942DeviceMemoryDispatchFactsV1, MemorySessionError> {
        self.fixture.engine.mapped_device_memory_facts_v1(
            lease,
            self.fixture.device.model_key(),
            self.fixture.vm,
        )
    }
    fn overwrite_mapped_host_visible_subrange_in_current_scope(
        &mut self,
        token: &mut MappedHostBufferV1,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), MemorySessionError> {
        self.sdma_access_fault_v1("reset")?;
        self.fixture
            .engine
            .overwrite_mapped_host_visible_subrange_in_current_scope(token, offset, bytes)
    }
    fn write_sdma_ring_slot_in_current_scope(
        &mut self,
        ring: &mut SdmaRingAuthorityV1,
        slot: u32,
        packet: &[u8; 64],
    ) -> Result<(), MemorySessionError> {
        self.sdma_access_fault_v1("ring")?;
        self.fixture
            .engine
            .write_sdma_slot_in_current_scope(&mut ring.token, slot, packet)
    }
    fn publish_sdma_control_write_release_in_current_scope(
        &mut self,
        control: &mut SdmaControlAuthorityV1,
        expected: u64,
        new: u64,
    ) -> Result<(), MemorySessionError> {
        self.sdma_access_fault_v1("control")?;
        self.fixture
            .engine
            .publish_sdma_write_release_in_current_scope(&mut control.token, expected, new)
    }
    fn observe_mapped_host_visible_i64_at_in_current_scope(
        &mut self,
        token: &mut MappedHostBufferV1,
        offset: u64,
    ) -> Result<i64, MemorySessionError> {
        self.sdma_access_fault_v1("observe")?;
        let offset = usize::try_from(offset).map_err(|_| MemorySessionError::SizeOverflow)?;
        self.fixture
            .engine
            .observe_i64_acquire_in_current_scope(token, offset)
    }

    fn single_doorbell(
        &mut self,
        doorbell: &mut crate::queue_linux::LinuxDoorbellSliceV1,
        write: u64,
    ) -> Result<(), crate::sdma::Gfx942SdmaErrorV1> {
        self.sdma_access_fault_v1("doorbell")?;
        doorbell
            .store_packet_id_release(write)
            .map_err(|_| crate::sdma::Gfx942SdmaErrorV1::Contract("SDMA doorbell operation failed"))
    }
}

impl PreparationMemoryFixtureV1 {
    pub(crate) fn sdma_fixture_counters_v1(
        &mut self,
        control: &SdmaControlAuthorityV1,
        write: u64,
        read: u64,
    ) {
        let engine = &mut self.fixture.engine;
        let index = engine
            .index(
                &control.token,
                SharedAllocationPhaseV1::GpuAccessibleMutable,
            )
            .unwrap();
        let record = &mut engine.allocations[index];
        let mapping = record.mapping.as_mut().unwrap();
        for (offset, value) in [
            (
                crate::queue_resources::AMD_AQL_WRITE_DISPATCH_ID_OFFSET_V1,
                write,
            ),
            (
                crate::queue_resources::AMD_AQL_READ_DISPATCH_ID_OFFSET_V1,
                read,
            ),
        ] {
            let start = mapping.byte_offset + offset;
            mapping.bytes[start..start + 8].copy_from_slice(&value.to_le_bytes());
        }
    }

    fn sdma_access_fault_v1(&self, operation: &'static str) -> Result<(), MemorySessionError> {
        if let Some((selected, panic)) = self.sdma_access_fault
            && selected == operation
        {
            if panic {
                std::panic::panic_any(("compute-XGMI SDMA access panic", operation));
            }
            return Err(MemorySessionError::Injected(operation));
        }
        Ok(())
    }

    pub(crate) fn sdma_arm_access_fault_v1(&mut self, operation: &'static str, panic: bool) {
        assert!(matches!(
            operation,
            "reset" | "ring" | "control" | "doorbell" | "observe"
        ));
        self.sdma_access_fault = Some((operation, panic));
    }

    pub(crate) fn sdma_fail_operational_currentness_v1(&mut self, ordinal: usize, panic: bool) {
        let backend = &mut self.fixture.engine.backend;
        let selected = backend.operational_currentness_calls + ordinal;
        if panic {
            backend.panic_operational_currentness_at = Some(selected);
        } else {
            backend.fail_operational_currentness_at = Some(selected);
        }
    }

    pub(crate) fn enable_sdma_mapped_bytes_v1(&mut self) {
        for record in &mut self.fixture.engine.allocations {
            if let Some(mapping) = record.mapping.as_mut() {
                mapping.sdma_bytes = true;
            }
        }
    }

    pub(crate) fn sdma_resource_bytes_v1<R: SharedGttQueueResourceRoleV1, P: GttProfileV1>(
        &self,
        authority: &SharedGttQueueResourceAuthorityV1<R, P, GttGpuAccessibleMutableV1>,
    ) -> Vec<u8> {
        self.sdma_mapped_bytes_v1(&authority.token)
    }

    pub(crate) fn sdma_mapped_bytes_v1<P: GttProfileV1>(
        &self,
        token: &SharedGttAllocationV1<P, GttGpuAccessibleMutableV1>,
    ) -> Vec<u8> {
        // Observation only: authenticate the original record even after terminal quarantine.
        let engine = &self.fixture.engine;
        let record = engine
            .allocations
            .iter()
            .find(|record| {
                engine.shared_allocation_record_matches(
                    record,
                    token,
                    SharedAllocationPhaseV1::GpuAccessibleMutable,
                )
            })
            .unwrap();
        let mapping = record.mapping.as_ref().unwrap();
        mapping.bytes[mapping.byte_offset..mapping.byte_offset + record.layout.requested_bytes]
            .to_vec()
    }

    pub(crate) fn sdma_mapping_panic_v1<P: GttProfileV1>(
        &mut self,
        token: &SharedGttAllocationV1<P, GttGpuAccessibleMutableV1>,
        operation: &'static str,
    ) {
        let engine = &mut self.fixture.engine;
        let index = engine
            .index(token, SharedAllocationPhaseV1::GpuAccessibleMutable)
            .unwrap();
        engine.allocations[index]
            .mapping
            .as_mut()
            .unwrap()
            .panic_access = Some(operation);
    }

    pub(crate) fn sdma_resource_panic_v1<R: SharedGttQueueResourceRoleV1, P: GttProfileV1>(
        &mut self,
        resource: &SharedGttQueueResourceAuthorityV1<R, P, GttGpuAccessibleMutableV1>,
        operation: &'static str,
    ) {
        self.sdma_mapping_panic_v1(&resource.token, operation);
    }
}
