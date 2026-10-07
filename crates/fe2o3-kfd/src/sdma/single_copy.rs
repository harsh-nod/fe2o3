//! Caller-owned single-copy phases survive borrowed preparation and publication.

#![forbid(unsafe_code)]

use super::*;

pub(crate) trait SdmaSingleMemoryV1 {
    fn check_queue_operational_currentness(&mut self) -> Result<(), MemorySessionError>;
    fn observe_aql_control_counters_in_current_scope(
        &mut self,
        control: &mut SdmaControlAuthorityV1,
    ) -> Result<(u64, u64), MemorySessionError>;
    fn single_host_facts(
        &self,
        token: &MappedHostBufferV1,
    ) -> Result<crate::shared_memory::SharedGttMappedResourceFactsV1, MemorySessionError>;
    fn single_device_facts(
        &self,
        lease: &Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Result<crate::shared_memory::Gfx942DeviceMemoryDispatchFactsV1, MemorySessionError>;
    fn overwrite_mapped_host_visible_subrange_in_current_scope(
        &mut self,
        token: &mut MappedHostBufferV1,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), MemorySessionError>;
    fn write_sdma_ring_slot_in_current_scope(
        &mut self,
        ring: &mut SdmaRingAuthorityV1,
        slot: u32,
        packet: &[u8; 64],
    ) -> Result<(), MemorySessionError>;
    fn publish_sdma_control_write_release_in_current_scope(
        &mut self,
        control: &mut SdmaControlAuthorityV1,
        expected: u64,
        new: u64,
    ) -> Result<(), MemorySessionError>;
    fn observe_mapped_host_visible_i64_at_in_current_scope(
        &mut self,
        token: &mut MappedHostBufferV1,
        offset: u64,
    ) -> Result<i64, MemorySessionError>;
    fn single_doorbell(
        &mut self,
        doorbell: &mut LinuxDoorbellSliceV1,
        write: u64,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        doorbell
            .store_packet_id_release(write)
            .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA doorbell operation failed"))
    }
}

impl SdmaSingleMemoryV1 for SharedGttMemorySessionV1 {
    fn check_queue_operational_currentness(&mut self) -> Result<(), MemorySessionError> {
        Self::check_queue_operational_currentness(self)
    }
    fn observe_aql_control_counters_in_current_scope(
        &mut self,
        control: &mut SdmaControlAuthorityV1,
    ) -> Result<(u64, u64), MemorySessionError> {
        Self::observe_aql_control_counters_in_current_scope(self, control)
    }
    fn single_host_facts(
        &self,
        token: &MappedHostBufferV1,
    ) -> Result<crate::shared_memory::SharedGttMappedResourceFactsV1, MemorySessionError> {
        self.mapped_resource_facts(token)
    }
    fn single_device_facts(
        &self,
        lease: &Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Result<crate::shared_memory::Gfx942DeviceMemoryDispatchFactsV1, MemorySessionError> {
        self.mapped_gfx942_device_memory_facts(lease)
    }
    fn overwrite_mapped_host_visible_subrange_in_current_scope(
        &mut self,
        token: &mut MappedHostBufferV1,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), MemorySessionError> {
        Self::overwrite_mapped_host_visible_subrange_in_current_scope(self, token, offset, bytes)
    }
    fn write_sdma_ring_slot_in_current_scope(
        &mut self,
        ring: &mut SdmaRingAuthorityV1,
        slot: u32,
        packet: &[u8; 64],
    ) -> Result<(), MemorySessionError> {
        Self::write_sdma_ring_slot_in_current_scope(self, ring, slot, packet)
    }
    fn publish_sdma_control_write_release_in_current_scope(
        &mut self,
        control: &mut SdmaControlAuthorityV1,
        expected: u64,
        new: u64,
    ) -> Result<(), MemorySessionError> {
        Self::publish_sdma_control_write_release_in_current_scope(self, control, expected, new)
    }
    fn observe_mapped_host_visible_i64_at_in_current_scope(
        &mut self,
        token: &mut MappedHostBufferV1,
        offset: u64,
    ) -> Result<i64, MemorySessionError> {
        Self::observe_mapped_host_visible_i64_at_in_current_scope(self, token, offset)
    }
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum SingleSdmaCopyCustodyV1 {
    Request(Gfx942SdmaCopyRequestV1),
    Prepared(PreparedSingleSdmaV1),
    QueueRetained(Gfx942SdmaCopyTicketV1),
    Completed(Gfx942SdmaCompletedCopyV1),
}

impl Gfx942SdmaQueueSetV1 {
    pub(crate) fn prepare_directional_single_in_place(
        &mut self,
        memory: &mut impl SdmaSingleMemoryV1,
        custody: &mut Option<SingleSdmaCopyCustodyV1>,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        if !matches!(self, Self::Directional(_)) {
            return Err(Gfx942SdmaErrorV1::Contract(
                "directional persistent single profile",
            ));
        }
        let Some(SingleSdmaCopyCustodyV1::Request(request)) = custody.as_ref() else {
            return Err(Gfx942SdmaErrorV1::Contract("single SDMA request custody"));
        };
        let plan = self
            .owner_for_copy(request.source.kind(), request.destination.kind())?
            .prepare_single_plan(memory, request)?;
        let Some(SingleSdmaCopyCustodyV1::Request(request)) = custody.take() else {
            unreachable!("borrowed preparation preserves request");
        };
        let mut prepared = plan.attach(request);
        prepared.directional_persistent = true;
        *custody = Some(SingleSdmaCopyCustodyV1::Prepared(prepared));
        Ok(())
    }

    pub(crate) fn publish_single_in_place(
        &mut self,
        memory: &mut impl SdmaSingleMemoryV1,
        custody: &mut Option<SingleSdmaCopyCustodyV1>,
    ) -> Result<Gfx942SdmaCopyTicketV1, Gfx942SdmaErrorV1> {
        let Some(SingleSdmaCopyCustodyV1::Prepared(prepared)) = custody.as_ref() else {
            return Err(Gfx942SdmaErrorV1::Contract("single SDMA prepared custody"));
        };
        self.owner_for_ticket(prepared.ticket())?
            .publish_single_in_place(memory, custody)
    }
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SingleQueueSnapshotV1 {
    pub(crate) queue_id: u32,
    pub(crate) poisoned: bool,
    pub(crate) generations: Vec<u32>,
    #[allow(clippy::type_complexity)]
    pub(crate) records: Vec<(
        Gfx942SdmaCopyTicketV1,
        Gfx942SdmaBufferStorageIdentityV1,
        Gfx942SdmaBufferStorageIdentityV1,
        u64,
        u64,
        u32,
        bool,
    )>,
    pub(crate) ring: Vec<u8>,
    pub(crate) control: Vec<u8>,
    pub(crate) completions: Vec<u8>,
}

#[cfg(test)]
impl Gfx942SdmaQueueSetV1 {
    pub(crate) fn single_completion_address_for_test_v1(
        &self,
        memory: &impl SdmaSingleMemoryV1,
        queue_id: u32,
    ) -> u64 {
        let Self::Directional(owners) = self else {
            panic!("directional fixture")
        };
        let owner = owners
            .iter()
            .find(|owner| owner.queue_id == queue_id)
            .unwrap();
        memory
            .single_host_facts(owner.completions.as_ref().unwrap())
            .unwrap()
            .gpu_va()
    }

    pub(crate) fn single_snapshots_for_test_v1(
        &self,
        memory: &crate::shared_memory::PreparationMemoryFixtureV1,
    ) -> Vec<SingleQueueSnapshotV1> {
        let Self::Directional(owners) = self else {
            panic!("directional fixture")
        };
        owners
            .iter()
            .map(|owner| SingleQueueSnapshotV1 {
                queue_id: owner.queue_id,
                poisoned: owner.poisoned,
                generations: owner.generations.to_vec(),
                records: owner
                    .records
                    .iter()
                    .enumerate()
                    .filter_map(|(slot, record)| {
                        record.as_ref().map(|record| {
                            (
                                Gfx942SdmaCopyTicketV1 {
                                    owner: owner.owner,
                                    queue_id: owner.queue_id,
                                    slot: slot as u16,
                                    generation: record.generation,
                                },
                                record.source.storage_identity(),
                                record.destination.storage_identity(),
                                record.source_offset,
                                record.destination_offset,
                                record.copy_bytes,
                                record.directional_persistent,
                            )
                        })
                    })
                    .collect(),
                ring: memory.sdma_resource_bytes_v1(owner.ring.as_ref().unwrap()),
                control: memory.sdma_resource_bytes_v1(owner.control.as_ref().unwrap()),
                completions: memory.sdma_mapped_bytes_v1(owner.completions.as_ref().unwrap()),
            })
            .collect()
    }

    pub(crate) fn seed_single_bytes_for_test_v1(&mut self, memory: &mut impl SdmaSingleMemoryV1) {
        let Self::Directional(owners) = self else {
            panic!("directional fixture")
        };
        for owner in owners {
            memory
                .write_sdma_ring_slot_in_current_scope(owner.ring.as_mut().unwrap(), 0, &[0x5a; 64])
                .unwrap();
            memory
                .overwrite_mapped_host_visible_subrange_in_current_scope(
                    owner.completions.as_mut().unwrap(),
                    0,
                    &13i64.to_le_bytes(),
                )
                .unwrap();
        }
    }
}
