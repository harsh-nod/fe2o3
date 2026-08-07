use fe2o3_core::DeviceBuffer;
use fe2o3_host::{GeneratedReadDeviceSlice, GeneratedReadWriteDeviceSlice, ObservedContext};

fn rejected(observed: &ObservedContext, input: &DeviceBuffer<f32>, state: &mut DeviceBuffer<f32>) {
    let input = GeneratedReadDeviceSlice::new(observed, input).unwrap();
    let state = GeneratedReadWriteDeviceSlice::new(observed, state).unwrap();
    let _ = input.device_pointer();
    let _ = state.device_pointer();
}

fn main() {}
