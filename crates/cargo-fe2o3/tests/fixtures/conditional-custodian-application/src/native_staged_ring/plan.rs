//! Each branch has only its own producer dependency; no compute-wide barrier.
use super::*;
use fe2o3_runtime::completion::{
    CompletionGraphV1, CompletionNodeIdV1, CompletionNodeV1, EventIdentityV1, FutureIdentityV1,
};
use fe2o3_runtime::*;

pub(super) struct Plan {
    pub devices: Vec<RuntimeDeviceIdV1>,
    pub observed: Vec<(u64, u16)>,
    pub streams: Vec<RuntimeStreamIdV1>,
    pub sources: Vec<RuntimeAllocationIdV1>,
    pub destinations: Vec<RuntimeAllocationIdV1>,
    pub nodes: Vec<[CompletionNodeIdV1; 5]>,
    pub request: Option<RuntimeGraphRequestV1<Backend>>,
}

pub(super) fn region(
    allocation: RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
    bytes: usize,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: bytes as u64,
    }
}

impl Plan {
    pub fn new(context: &mut Context, case: &Case) -> Result<Self> {
        if context.devices().len() != case.devices.len()
            || context
                .devices()
                .iter()
                .any(|device| device.target() != "gfx942:xnack-")
        {
            return Err("exact selected gfx942 Context roster required".into());
        }
        let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
        let mut observed = Vec::with_capacity(devices.len());
        let mut streams = Vec::with_capacity(devices.len() * 2);
        let mut sources = Vec::with_capacity(devices.len());
        let mut destinations = Vec::with_capacity(devices.len());
        for (index, &device) in devices.iter().enumerate() {
            let original = checked(context.with_gfx942_preparation_device_v1(device, |owner| {
                Ok::<_, String>((
                    owner.observation().unique_id(),
                    owner.observation().render_minor(),
                ))
            }))?;
            observed.push(*original.value());
            streams.push(checked(context.create_stream(device))?);
            let bytes = case.elements(index) * 4;
            let source = checked(context.allocate(
                device,
                RuntimeMemoryKindV1::HostVisible,
                bytes as u64,
                4096,
            ))?;
            checked(context.write_host_visible_allocation_v1(source, &vec![0xa5; bytes]))?;
            sources.push(source);
        }
        case.check_roster(&observed)?;
        for index in 0..devices.len() {
            let destination = devices[(index + 1) % devices.len()];
            streams.push(checked(context.create_stream(destination))?);
            let bytes = case.elements(index) * 4;
            let allocation = checked(context.allocate(
                destination,
                RuntimeMemoryKindV1::HostVisible,
                bytes as u64,
                4096,
            ))?;
            checked(context.write_host_visible_allocation_v1(allocation, &vec![0x3c; bytes]))?;
            destinations.push(allocation);
        }
        let group = checked(
            context.create_graph_group_v1(&streams, RuntimeGraphDeviceCoverageV1::AllAdmitted),
        )?;
        let identities = streams
            .iter()
            .map(|&s| checked(group.stream_identity(s)))
            .collect::<Result<Vec<_>>>()?;
        let mut nodes = Vec::with_capacity(devices.len());
        let mut graph_nodes = Vec::with_capacity(devices.len() * 5);
        for index in 0..devices.len() {
            let raw = native_staged_ring_case::nodes(index);
            let node = |value| CompletionNodeIdV1::new(value).ok_or("invalid ring node ID");
            let ids = [
                node(raw[0])?,
                node(raw[1])?,
                node(raw[2])?,
                node(raw[3])?,
                node(raw[4])?,
            ];
            let compute = identities[index];
            let transfer = identities[devices.len() + index];
            let event = EventIdentityV1::new(group.context_identity(), [index as u8 + 1; 32]);
            graph_nodes.extend([
                CompletionNodeV1::future(
                    ids[0],
                    FutureIdentityV1::new(compute, [ids[0].get() as u8; 32]),
                    None,
                ),
                CompletionNodeV1::future(
                    ids[1],
                    FutureIdentityV1::new(compute, [ids[1].get() as u8; 32]),
                    Some(ids[0]),
                ),
                CompletionNodeV1::record_event(ids[2], compute, event, Some(ids[1])),
                CompletionNodeV1::wait_event(ids[3], transfer, event, ids[2], None),
                CompletionNodeV1::future(
                    ids[4],
                    FutureIdentityV1::new(transfer, [ids[4].get() as u8; 32]),
                    Some(ids[3]),
                ),
            ]);
            nodes.push(ids);
        }
        let graph = checked(CompletionGraphV1::new(
            group.context_identity(),
            identities,
            graph_nodes,
        ))?;
        let mut request = checked(RuntimeGraphRequestV1::new_group_v1(graph, group))?;
        for index in 0..devices.len() {
            let bytes = case.elements(index) * 4;
            checked(request.bind_host_staging_v1(
                nodes[index][1],
                nodes[index][0],
                region(sources[index], RuntimeAccessV1::Write, bytes),
            ))?;
            checked(request.bind_tracked_replica_copy_v1(
                nodes[index][4],
                region(sources[index], RuntimeAccessV1::Read, bytes),
                region(destinations[index], RuntimeAccessV1::Write, bytes),
            ))?;
            checked(request.expect_input_version(
                nodes[index][4],
                region(sources[index], RuntimeAccessV1::Read, bytes),
                RuntimeGraphVersionSourceV1::ProducedBy(nodes[index][1]),
            ))?;
        }
        Ok(Self {
            devices,
            observed,
            streams,
            sources,
            destinations,
            nodes,
            request: Some(request),
        })
    }

    pub fn close(self, context: &mut Context) -> Result<()> {
        for allocation in self.destinations.into_iter().chain(self.sources).rev() {
            checked(context.release_allocation(allocation))?;
        }
        for stream in self.streams.into_iter().rev() {
            checked(context.destroy_stream(stream))?;
        }
        Ok(())
    }
}
