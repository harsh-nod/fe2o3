use fe2o3_core::DeviceBuffer;
use fe2o3_host::{GeneratedReadWriteDeviceSlice, ObservedContext};

fn reject_shared_view(observed: &ObservedContext, output: &DeviceBuffer<f32>) {
    let shared = output.view(..).unwrap();
    let _write = GeneratedReadWriteDeviceSlice::from_view_mut(observed, shared);

    let shared = output.view(..).unwrap();
    let _read_write = GeneratedReadWriteDeviceSlice::from_view_mut(observed, shared);
}

fn main() {}
