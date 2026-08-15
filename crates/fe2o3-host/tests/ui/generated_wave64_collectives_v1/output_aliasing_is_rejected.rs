use fe2o3_core::DeviceBuffer;
use fe2o3_host::{GeneratedWave64CollectivesV1HostAdapterV1, ObservedContext};

fn alias<'a>(
    observed: &ObservedContext,
    input: &'a DeviceBuffer<f32>,
    output: &'a mut DeviceBuffer<f32>,
) {
    let _ = GeneratedWave64CollectivesV1HostAdapterV1::prepare(
        observed,
        input.view(..).unwrap(),
        !0,
        output.view_mut(..).unwrap(),
        output.view_mut(..).unwrap(),
        output.view_mut(..).unwrap(),
    );
}

fn main() {}
