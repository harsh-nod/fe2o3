use fe2o3_core::{DeviceBuffer, Stream};
use fe2o3_host::{
    CheckedMoeHostObservedRoutingOutputV1, upload_checked_moe_routing_expert_bridge_v1,
};

fn mutate(
    stream: &Stream,
    offsets: &mut DeviceBuffer<u32>,
    inverse: &mut DeviceBuffer<u32>,
    checked: CheckedMoeHostObservedRoutingOutputV1,
) {
    let bridge =
        upload_checked_moe_routing_expert_bridge_v1(stream, offsets, inverse, checked).unwrap();
    let mutable = inverse.view_mut(..).unwrap();
    drop((bridge, mutable));
}

fn main() {}
