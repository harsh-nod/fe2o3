use super::*;
use crate::persistent_allocation::Gfx942PersistentOperationV1;
use crate::sdma::{
    GFX942_SDMA_MAX_IN_FLIGHT_V1, GFX942_SDMA_RING_BYTES_V1, Gfx942SdmaQueueObservationV1,
    persistent_sdma_buffers_for_test, persistent_sdma_ticket_coordinates_for_test,
};
use fe2o3_runtime_model::{
    DeviceGenerationV1, DeviceKeyV1, PhysicalDeviceIdV1, QueueGenerationV1, QueueInstanceIdV1,
    VmIdV1, VmKeyV1,
};
use sha2::{Digest, Sha256};

fn queue_key() -> QueueKeyV1 {
    queue_key_with_generation(1)
}

fn queue_key_with_generation(generation: u64) -> QueueKeyV1 {
    let device = DeviceKeyV1 {
        physical: PhysicalDeviceIdV1(7),
        generation: DeviceGenerationV1(1),
    };
    QueueKeyV1 {
        vm: VmKeyV1 {
            device,
            id: VmIdV1(1),
        },
        id: QueueInstanceIdV1(3),
        generation: QueueGenerationV1(generation),
    }
}

fn queue_observation(queue_id: u32, engine_index: u32) -> Gfx942SdmaQueueObservationV1 {
    Gfx942SdmaQueueObservationV1 {
        queue_id,
        ring_bytes: GFX942_SDMA_RING_BYTES_V1,
        maximum_in_flight: GFX942_SDMA_MAX_IN_FLIGHT_V1 as u16,
        engine_index: Some(engine_index),
    }
}

fn pair_observation(h2d_queue: u32, d2h_queue: u32) -> Gfx942DirectionalSdmaQueueObservationV1 {
    Gfx942DirectionalSdmaQueueObservationV1 {
        host_to_device: queue_observation(h2d_queue, GFX942_SDMA_H2D_ENGINE_INDEX_V1),
        device_to_host: queue_observation(d2h_queue, GFX942_SDMA_D2H_ENGINE_INDEX_V1),
        admitted_engine_count: 2,
        admitted_queues_per_engine: 8,
    }
}

fn promoted_fixture(
    id: u64,
    logical_bytes: u64,
) -> (
    Gfx942DirectionalQueuePersistentAllocationV1,
    Gfx942SdmaBufferV1,
) {
    let (mut device, host) = persistent_sdma_buffers_for_test(queue_key(), id);
    device.set_logical_bytes(logical_bytes);
    let pair = admit_persistent_directional_sdma_pair_v1(pair_observation(17, 23)).unwrap();
    let (allocation, outstanding) =
        promote_directional_persistent_sdma_custody_v1(device, pair, 2).unwrap();
    assert_eq!(outstanding, 2);
    (allocation, host)
}

fn prepared_fixture(
    allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    host: Gfx942SdmaBufferV1,
    dependency: Option<&Gfx942PersistentDependencyFrontierV1>,
    direction: Gfx942PersistentSdmaDirectionV1,
    ticket_generation: u32,
) -> (
    DirectionalPersistentSdmaPreparedCustodyV1,
    Gfx942SdmaCopyRequestV1,
    Gfx942SdmaCopyTicketV1,
) {
    let mut allocation = allocation;
    let operation = match direction {
        Gfx942PersistentSdmaDirectionV1::HostToDevice => {
            Gfx942PersistentOperationV1::LocalSdmaDestination
        }
        Gfx942PersistentSdmaDirectionV1::DeviceToHost => {
            Gfx942PersistentOperationV1::LocalSdmaSource
        }
    };
    let reserved = allocation
        .owner
        .reserve(
            Gfx942PersistentUseRequestV1::new(operation, 16, 32).unwrap(),
            dependency,
        )
        .unwrap();
    let prepared = allocation.owner.prepare(reserved).unwrap();
    let lease = allocation.owner.detach_local_native_for_sdma().unwrap();
    let device = Gfx942SdmaBufferV1::from_bridge_parts(
        Gfx942SdmaBufferStorageV1::Device(lease),
        allocation.attachment.queue,
        allocation.attachment.pool_generation,
        allocation.attachment.logical_bytes,
    );
    let host_binding = Gfx942PersistentDirectionalSdmaHostBindingV1::capture(&host, queue_key());
    let request = directional_persistent_sdma_request_v1(direction, host, 8, device, 16, 32);
    let ticket = persistent_sdma_ticket_coordinates_for_test(
        queue_key(),
        allocation.attachment.pair.queue_id(direction),
        (ticket_generation as u16) % GFX942_SDMA_MAX_IN_FLIGHT_V1 as u16,
        ticket_generation,
    );
    (
        DirectionalPersistentSdmaPreparedCustodyV1 {
            allocation,
            prepared,
            planned_ticket: ticket,
            host_binding,
            direction,
            host_offset: 8,
            device_offset: 16,
            copy_bytes: 32,
        },
        request,
        ticket,
    )
}

fn published_fixture(
    id: u64,
    direction: Gfx942PersistentSdmaDirectionV1,
) -> (
    Gfx942DirectionalPersistentSdmaSubmissionV1,
    Gfx942SdmaCopyRequestV1,
) {
    let (allocation, host) = promoted_fixture(id, 2048);
    let (prepared, request, ticket) = prepared_fixture(allocation, host, None, direction, 1);
    let DirectionalPersistentSdmaPublicationTransitionV1::Published(submission) =
        transition_directional_persistent_sdma_publication_v1(
            prepared,
            DirectionalPersistentSdmaPublicationObservationV1::Confirmed(ticket),
            true,
            true,
        )
    else {
        unreachable!()
    };
    (submission, request)
}

fn completed_request(request: Gfx942SdmaCopyRequestV1) -> Gfx942SdmaCompletedCopyV1 {
    Gfx942SdmaCompletedCopyV1 {
        source: request.source,
        destination: request.destination,
        copy_bytes: request.copy_bytes,
        source_offset: request.source_offset,
        destination_offset: request.destination_offset,
    }
}

fn prepared_window_fixture(
    allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    host: Gfx942SdmaBufferV1,
    direction: Gfx942PersistentSdmaDirectionV1,
    packet_count: usize,
) -> (
    DirectionalPersistentSdmaWindowPreparedCustodyV1,
    Gfx942SdmaCopyRequestV1,
    Vec<Gfx942SdmaCopyTicketV1>,
) {
    let mut allocation = allocation;
    let operation = match direction {
        Gfx942PersistentSdmaDirectionV1::HostToDevice => {
            Gfx942PersistentOperationV1::LocalSdmaDestination
        }
        Gfx942PersistentSdmaDirectionV1::DeviceToHost => {
            Gfx942PersistentOperationV1::LocalSdmaSource
        }
    };
    let reserved = allocation
        .owner
        .reserve(
            Gfx942PersistentUseRequestV1::new(operation, 16, 32).unwrap(),
            None,
        )
        .unwrap();
    let prepared = allocation.owner.prepare(reserved).unwrap();
    let lease = allocation.owner.detach_local_native_for_sdma().unwrap();
    let device = Gfx942SdmaBufferV1::from_bridge_parts(
        Gfx942SdmaBufferStorageV1::Device(lease),
        allocation.attachment.queue,
        allocation.attachment.pool_generation,
        allocation.attachment.logical_bytes,
    );
    let host_binding = Gfx942PersistentDirectionalSdmaHostBindingV1::capture(&host, queue_key());
    let request = directional_persistent_sdma_request_v1(direction, host, 8, device, 16, 32);
    let tickets = (0..packet_count)
        .map(|index| {
            persistent_sdma_ticket_coordinates_for_test(
                queue_key(),
                allocation.attachment.pair.queue_id(direction),
                index as u16,
                1,
            )
        })
        .collect::<Vec<_>>();
    (
        DirectionalPersistentSdmaWindowPreparedCustodyV1 {
            allocation,
            prepared,
            planned_tickets: tickets.clone(),
            host_binding,
            direction,
            host_offset: 8,
            device_offset: 16,
            copy_bytes: 32,
            packet_count,
        },
        request,
        tickets,
    )
}

fn published_window_fixture(
    id: u64,
    direction: Gfx942PersistentSdmaDirectionV1,
    packet_count: usize,
) -> (
    Gfx942DirectionalPersistentSdmaWindowSubmissionV1,
    Gfx942SdmaCopyRequestV1,
) {
    let (allocation, host) = promoted_fixture(id, 2048);
    let (prepared, request, tickets) =
        prepared_window_fixture(allocation, host, direction, packet_count);
    let DirectionalPersistentSdmaWindowPublicationTransitionV1::Published(submission) =
        transition_directional_persistent_sdma_window_publication_v1(
            prepared,
            DirectionalPersistentSdmaWindowPublicationObservationV1::Confirmed(tickets),
            true,
            true,
        )
    else {
        unreachable!()
    };
    (submission, request)
}

fn exercise_sequential_directions(
    id: u64,
    direction_for_cycle: impl Fn(usize) -> Gfx942PersistentSdmaDirectionV1,
) {
    let (mut allocation, mut host) = promoted_fixture(id, 2048);
    for cycle in 0..(crate::GFX942_MAX_PERSISTENT_ALLOCATION_USES_V1 + 2) {
        let direction = direction_for_cycle(cycle);
        let generation = u32::try_from(cycle + 1).unwrap();
        let (prepared, request, ticket) =
            prepared_fixture(allocation, host, None, direction, generation);
        let DirectionalPersistentSdmaPublicationTransitionV1::Published(submission) =
            transition_directional_persistent_sdma_publication_v1(
                prepared,
                DirectionalPersistentSdmaPublicationObservationV1::Confirmed(ticket),
                true,
                true,
            )
        else {
            panic!("cycle {cycle} must publish")
        };
        assert_eq!(submission.direction(), direction);
        let DirectionalPersistentSdmaCompletionTransitionV1::Completed(completed) =
            transition_directional_persistent_sdma_completion_v1(
                submission,
                DirectionalPersistentSdmaCompletionObservationV1::Completed(completed_request(
                    request,
                )),
                true,
            )
        else {
            panic!("cycle {cycle} must complete")
        };
        let (next_allocation, next_host, frontier) = completed.into_parts();
        allocation = next_allocation
            .retire_settled_frontier_v1(frontier)
            .unwrap_or_else(|_| panic!("cycle {cycle} frontier must retire"));
        host = next_host;
    }
    assert_eq!(allocation.owner.retained_settled_use_count(), 0);
}

#[path = "tests/admission_tests.rs"]
mod admission_tests;
#[path = "tests/synchronous_tests.rs"]
mod synchronous_tests;
