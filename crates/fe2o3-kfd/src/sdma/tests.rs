#[path = "tests/xgmi_batch_wait.rs"]
mod xgmi_batch_wait;
#[path = "tests/xgmi_single_poll.rs"]
mod xgmi_single_poll;
#[path = "tests/xgmi_single_wait.rs"]
mod xgmi_single_wait;
use super::*;
use fe2o3_runtime_model::{
    DeviceGenerationV1, DeviceKeyV1, PhysicalDeviceIdV1, QueueGenerationV1, QueueInstanceIdV1,
    VmIdV1, VmKeyV1,
};
use sha2::{Digest, Sha256};

fn word(packet: &Gfx942SdmaCopySubmissionV1, index: usize) -> u32 {
    let offset = index * 4;
    u32::from_le_bytes(packet.bytes[offset..offset + 4].try_into().unwrap())
}

pub(super) fn queue_key(physical: u64, queue: u64, generation: u64) -> QueueKeyV1 {
    QueueKeyV1 {
        vm: VmKeyV1 {
            device: DeviceKeyV1 {
                physical: PhysicalDeviceIdV1(physical),
                generation: DeviceGenerationV1(1),
            },
            id: VmIdV1(1),
        },
        id: QueueInstanceIdV1(queue),
        generation: QueueGenerationV1(generation),
    }
}

fn compute_coexistence_owner_for_test(engine: u32) -> Gfx942SdmaQueueOwnerV1 {
    Gfx942SdmaQueueOwnerV1 {
        owner: queue_key(7, 11, 13),
        queue_id: engine + 20,
        engine_index: Some(engine),
        ring: None,
        control: None,
        completions: Some(crate::shared_memory::mapped_host_for_persistent_sdma_test(
            900, 4096,
        )),
        doorbell: None,
        records: (0..64).map(|_| None).collect(),
        xgmi_records: (0..64).map(|_| None).collect(),
        persistent_window_slots: (0..64).map(|_| None).collect(),
        persistent_window_records: (0..64).map(|_| None).collect(),
        uncertain_xgmi_ticket: None,
        generations: [0; 64],
        destroyed: false,
        poisoned: false,
    }
}

fn compute_coexistence_add_single(owner: &mut Gfx942SdmaQueueOwnerV1, id: u64) {
    let (device, host) = persistent_sdma_buffers_for_test(owner.owner, id);
    let (source, destination) = if owner.engine_index == Some(GFX942_SDMA_D2H_ENGINE_INDEX_V1) {
        (device, host)
    } else {
        (host, device)
    };
    owner.generations[7] = 3;
    owner.records[7] = Some(SdmaCopyRecordV1 {
        directional_persistent: true,
        generation: 3,
        completion_value: 3,
        fence_header: 0,
        completion_observed: false,
        source,
        destination,
        copy_bytes: 4096,
        source_offset: 0,
        destination_offset: 0,
    });
}

fn compute_coexistence_add_window(owner: &mut Gfx942SdmaQueueOwnerV1, id: u64) {
    let bytes = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1;
    let device = Gfx942SdmaBufferV1::from_bridge_parts(
        Gfx942SdmaBufferStorageV1::Device(
            crate::shared_memory::local_mapping_with_extent_for_persistent_sdma_test(
                id,
                u64::from(bytes),
            ),
        ),
        owner.owner,
        1,
        u64::from(bytes),
    );
    let host = Gfx942SdmaBufferV1::from_bridge_parts(
        Gfx942SdmaBufferStorageV1::Host(
            crate::shared_memory::mapped_host_for_persistent_sdma_test(id + 1000, bytes as usize),
        ),
        owner.owner,
        1,
        u64::from(bytes),
    );
    let request = if owner.engine_index == Some(GFX942_SDMA_D2H_ENGINE_INDEX_V1) {
        Gfx942SdmaCopyRequestV1::new(device, 0, host, 0, bytes)
    } else {
        Gfx942SdmaCopyRequestV1::new(host, 0, device, 0, bytes)
    };
    owner.persistent_window_records[63] = Some(PersistentSdmaWindowRecordV1 {
        request,
        packet_count: 2,
    });
    for (slot, generation) in [(63, 4), (0, 5)] {
        owner.generations[slot] = generation;
        owner.persistent_window_slots[slot] = Some(PersistentSdmaWindowSlotV1 {
            anchor_slot: 63,
            generation,
            completion_value: generation,
        });
    }
}

fn compute_coexistence_collect_for_test(
    owner: &Gfx942SdmaQueueOwnerV1,
) -> Option<arrayvec::ArrayVec<fe2o3_runtime_model::R66DeviceStorageV1, 258>> {
    let mut endpoints = arrayvec::ArrayVec::new();
    owner.collect_compute_coexistence_endpoints_v1(
        owner.completions.as_ref()?.storage_identity(),
        &mut endpoints,
    )?;
    Some(endpoints)
}

#[path = "tests/allocation_tests.rs"]
mod allocation_tests;
#[path = "tests/wait_profile_tests.rs"]
mod wait_profile_tests;
