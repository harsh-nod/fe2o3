//! Shared CPU fixture setup for original mapped allocations and completion templates.

use super::*;

pub(super) fn public_sdma_pool_buffer_for_test(
    owner: QueueKeyV1,
    id: u64,
    bytes: u64,
) -> Gfx942SdmaBufferV1 {
    Gfx942SdmaBufferV1::from_bridge_parts(
        Gfx942SdmaBufferStorageV1::Device(
            crate::shared_memory::local_mapping_with_extent_for_persistent_sdma_test(id, bytes),
        ),
        owner,
        1,
        bytes,
    )
}

pub(super) fn test_completion_mapping(
    queue: QueueKeyV1,
    id: u64,
) -> fe2o3_runtime_model::MemoryMappingKeyV1 {
    fe2o3_runtime_model::MemoryMappingKeyV1 {
        allocation: fe2o3_runtime_model::MemoryAllocationKeyV1 {
            vm: queue.vm,
            id: fe2o3_runtime_model::AllocationIdV1(id),
            generation: fe2o3_runtime_model::AllocationGenerationV1(1),
        },
        id: fe2o3_runtime_model::MappingIdV1(id),
    }
}

pub(super) fn test_completion_template(
    queue: QueueKeyV1,
    dispatch_generation: u64,
) -> CompletionPacketTemplateV1 {
    CompletionPacketTemplateV1::new(
        fe2o3_aql::AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        fe2o3_aql::AqlDispatchOrderingV1::WaitForPrior,
        0,
        0,
        fe2o3_aql::ObservedGpuAddressV1::new(0x40_0000).unwrap(),
        fe2o3_aql::ObservedGpuAddressV1::new(0x50_0000).unwrap(),
        16,
        super::super::super::completion::CompletionDispatchGenerationBindingV1::new(
            queue,
            test_completion_mapping(queue, 30),
            test_completion_mapping(queue, 31),
            dispatch_generation,
        ),
    )
}

pub(super) fn persistent_restore_fixture(
    direction: Gfx942PersistentSdmaDirectionV1,
    id: u64,
) -> (
    Gfx942QueuePersistentAllocationV1,
    Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    Gfx942SdmaCopyRequestV1,
    Gfx942PersistentSdmaHostBindingV1,
) {
    let queue = test_queue_key(21, 1);
    let (device, host) = crate::sdma::persistent_sdma_buffers_for_test(queue, id);
    let host_binding = Gfx942PersistentSdmaHostBindingV1::capture(&host, queue);
    let storage_identity = device.storage_identity();
    let (storage, _, pool_generation, logical_bytes) = device.into_bridge_parts();
    let Gfx942SdmaBufferStorageV1::Device(lease) = storage else {
        unreachable!()
    };
    let mut owner = Gfx942PersistentDeviceAllocationV1::from_local_mapping(lease);
    let operation = match direction {
        Gfx942PersistentSdmaDirectionV1::HostToDevice => {
            Gfx942PersistentOperationV1::LocalSdmaDestination
        }
        Gfx942PersistentSdmaDirectionV1::DeviceToHost => {
            Gfx942PersistentOperationV1::LocalSdmaSource
        }
    };
    let reserved = owner
        .reserve(
            Gfx942PersistentUseRequestV1::new(operation, 16, 32).unwrap(),
            None,
        )
        .unwrap();
    let prepared = owner.prepare(reserved).unwrap();
    let lease = owner.detach_local_native_for_sdma().unwrap();
    let device = Gfx942SdmaBufferV1::from_bridge_parts(
        Gfx942SdmaBufferStorageV1::Device(lease),
        queue,
        pool_generation,
        logical_bytes,
    );
    (
        Gfx942QueuePersistentAllocationV1 {
            owner,
            attachment: Gfx942PersistentSdmaAttachmentV1 {
                queue,
                native_queue_id: 17,
                engine_index: direction.engine_index(),
                pool_generation,
                logical_bytes,
                physical_bytes: logical_bytes,
                storage_identity,
            },
        },
        prepared,
        persistent_sdma_request(direction, host, 8, device, 16, 32),
        host_binding,
    )
}
