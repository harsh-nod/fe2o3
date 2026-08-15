use fe2o3_core::DeviceBuffer;
use fe2o3_host::{GeneratedWorkgroupScopedAtomicV1HostAdapterV1, ObservedContext};

fn alias(
    observed: &ObservedContext,
    shared: &mut DeviceBuffer<u32>,
    eligible: &DeviceBuffer<u32>,
) {
    let _ = GeneratedWorkgroupScopedAtomicV1HostAdapterV1::prepare(
        observed,
        shared.view(..).unwrap(),
        eligible.view(..).unwrap(),
        shared.view_mut(0..1).unwrap(),
    );
}

fn main() {}
