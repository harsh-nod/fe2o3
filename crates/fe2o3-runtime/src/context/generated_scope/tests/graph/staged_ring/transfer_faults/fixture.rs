#![cfg(test)]
use super::*;

pub(super) fn branch(index: usize) -> [CompletionNodeIdV1; 6] {
    core::array::from_fn(|offset| id((index * 6 + offset + 1) as u32))
}

pub(super) struct Fixture {
    pub context: RuntimeContextV1<FaultBackend>,
    pub metadata: ResourceCreditAccountV1,
    pub compute: [RuntimeStreamIdV1; 3],
    pub transfer: [RuntimeStreamIdV1; 3],
    pub sources: [RuntimeAllocationIdV1; 3],
    pub destinations: [RuntimeAllocationIdV1; 3],
    pub consumers: [RuntimeAllocationIdV1; 3],
}

impl Fixture {
    pub fn new(fault: Fault) -> Self {
        let mut context =
            RuntimeContextV1::open_with_version_journal_v1(FaultBackend::new(fault), 32, 32)
                .unwrap();
        assert_eq!(context.devices().len(), 3);
        let devices: [_; 3] = core::array::from_fn(|i| context.devices()[i].id());
        let compute = devices.map(|device| context.create_stream(device).unwrap());
        let transfer =
            core::array::from_fn(|i| context.create_stream(devices[(i + 1) % 3]).unwrap());
        let metadata = ResourceCreditAccountV1::new(
            ResourceVectorV1::ZERO.with(
                ResourceKindV1::ControlResidentBytes,
                RuntimeReplicaStorageV1::required_payload_bytes_v1(3).unwrap(),
            ),
            1,
        )
        .unwrap();
        context
            .configure_replica_registry_v1(
                RuntimeReplicaStorageV1::preallocate(&metadata, 3).unwrap(),
            )
            .unwrap();
        let sources = devices.map(|device| allocate(&mut context, device, 0xa5));
        let destinations =
            core::array::from_fn(|i| allocate(&mut context, devices[(i + 1) % 3], 0x3c));
        let consumers =
            core::array::from_fn(|i| allocate(&mut context, devices[(i + 1) % 3], 0x7e));
        context.backend.target = Some(context.allocations[&destinations[0]].backend_allocation);
        Self {
            context,
            metadata,
            compute,
            transfer,
            sources,
            destinations,
            consumers,
        }
    }

    pub fn request(&mut self) -> RuntimeGraphRequestV1<FaultBackend> {
        let streams: Vec<_> = self.compute.into_iter().chain(self.transfer).collect();
        let group = self
            .context
            .create_graph_group_v1(&streams, RuntimeGraphDeviceCoverageV1::AllAdmitted)
            .unwrap();
        assert_eq!(group.devices().len(), 3);
        let identities = streams
            .iter()
            .map(|&s| group.stream_identity(s).unwrap())
            .collect::<Vec<_>>();
        let mut list = Vec::new();
        for index in 0..3 {
            let ids = branch(index);
            let event = EventIdentityV1::new(group.context_identity(), [index as u8 + 1; 32]);
            let future = |offset: usize, stream, previous| {
                CompletionNodeV1::future(
                    ids[offset],
                    FutureIdentityV1::new(stream, [ids[offset].get() as u8; 32]),
                    previous,
                )
            };
            list.extend([
                future(0, identities[index], None),
                future(1, identities[index], Some(ids[0])),
                CompletionNodeV1::record_event(ids[2], identities[index], event, Some(ids[1])),
                CompletionNodeV1::wait_event(ids[3], identities[3 + index], event, ids[2], None),
                future(4, identities[3 + index], Some(ids[3])),
                future(5, identities[3 + index], Some(ids[4])),
            ]);
        }
        let graph = CompletionGraphV1::new(group.context_identity(), identities, list).unwrap();
        let mut request = RuntimeGraphRequestV1::new_group_v1(graph, group).unwrap();
        for index in 0..3 {
            let ids = branch(index);
            request
                .bind_host_staging_v1(
                    ids[1],
                    ids[0],
                    region(self.sources[index], RuntimeAccessV1::Write),
                )
                .unwrap();
            request
                .bind_tracked_replica_copy_v1(
                    ids[4],
                    region(self.sources[index], RuntimeAccessV1::Read),
                    region(self.destinations[index], RuntimeAccessV1::Write),
                )
                .unwrap();
            request
                .expect_input_version(
                    ids[4],
                    region(self.sources[index], RuntimeAccessV1::Read),
                    RuntimeGraphVersionSourceV1::ProducedBy(ids[1]),
                )
                .unwrap();
            request
                .bind_copy(
                    ids[5],
                    region(self.destinations[index], RuntimeAccessV1::Read),
                    region(self.consumers[index], RuntimeAccessV1::Write),
                )
                .unwrap();
            request
                .expect_input_version(
                    ids[5],
                    region(self.destinations[index], RuntimeAccessV1::Read),
                    RuntimeGraphVersionSourceV1::ProducedBy(ids[4]),
                )
                .unwrap();
        }
        request
    }
}

fn allocate(
    context: &mut RuntimeContextV1<FaultBackend>,
    device: RuntimeDeviceIdV1,
    fill: u8,
) -> RuntimeAllocationIdV1 {
    let allocation = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 16, 8)
        .unwrap();
    context
        .write_allocation(allocation, 0, &[fill; 16])
        .unwrap();
    allocation
}
