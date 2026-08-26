use fe2o3_core::DeviceBuffer;
use fe2o3_host::{GeneratedReadDeviceSlice, GeneratedReadWriteDeviceSlice, ObservedContext};

fn shared_view_retains_parent(observed: &ObservedContext, input: DeviceBuffer<f32>) {
    let view = input.view(..).unwrap();
    let capability = GeneratedReadDeviceSlice::from_view(observed, view).unwrap();
    drop(input);
    drop(capability);
}

fn mutable_view_retains_parent(observed: &ObservedContext, mut output: DeviceBuffer<f32>) {
    let view = output.view_mut(..).unwrap();
    let capability = GeneratedReadWriteDeviceSlice::from_view_mut(observed, view).unwrap();
    let _reuse = output.len();
    drop(capability);
}

fn main() {}
