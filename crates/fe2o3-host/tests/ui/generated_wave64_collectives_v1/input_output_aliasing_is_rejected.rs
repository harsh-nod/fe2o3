use fe2o3_core::DeviceBuffer;
use fe2o3_host::{GeneratedWave64CollectivesV1HostAdapterV1, ObservedContext};

fn alias<'a>(
    observed: &ObservedContext,
    shared: &'a mut DeviceBuffer<f32>,
    inclusive: &'a mut DeviceBuffer<f32>,
    exclusive: &'a mut DeviceBuffer<f32>,
) {
    let _ = GeneratedWave64CollectivesV1HostAdapterV1::prepare(
        observed,
        shared.view(..).unwrap(),
        !0,
        shared.view_mut(..).unwrap(),
        inclusive.view_mut(..).unwrap(),
        exclusive.view_mut(..).unwrap(),
    );
}

fn main() {}
