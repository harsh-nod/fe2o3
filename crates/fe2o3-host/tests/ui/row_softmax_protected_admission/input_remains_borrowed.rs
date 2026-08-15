use fe2o3_core::DeviceBuffer;
use fe2o3_host::{GeneratedProtectedRowSoftmaxV1HostAdapterV1, ObservedContext};

fn release_input_while_prepared(
    observed: &ObservedContext,
    input: DeviceBuffer<f32>,
    mut output: DeviceBuffer<f32>,
) {
    let prepared = GeneratedProtectedRowSoftmaxV1HostAdapterV1::prepare(
        observed,
        input.view(..).unwrap(),
        output.view_mut(..).unwrap(),
    )
    .unwrap();
    drop(input);
    let _still_live = prepared;
}

fn main() {}
