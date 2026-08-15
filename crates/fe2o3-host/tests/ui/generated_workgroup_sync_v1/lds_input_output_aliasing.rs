use fe2o3_core::DeviceBuffer;
use fe2o3_host::{GeneratedWorkgroupLdsReductionV1HostAdapterV1, ObservedContext};

fn alias(observed: &ObservedContext, buffer: &mut DeviceBuffer<i32>) {
    let _ = GeneratedWorkgroupLdsReductionV1HostAdapterV1::prepare(
        observed,
        buffer.view(..).unwrap(),
        0,
        buffer.view_mut(0..1).unwrap(),
    );
}

fn main() {}
