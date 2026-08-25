use fe2o3_core::DeviceBuffer;
use fe2o3_host::{GeneratedScalarGemmV1ReadWriteDeviceSlice, ObservedContext};

fn rejected(observed: &ObservedContext, mut guarded: DeviceBuffer<f32>) {
    let (_left, output, _right) = guarded.split_range_mut(1..3).unwrap();
    let capability =
        GeneratedScalarGemmV1ReadWriteDeviceSlice::from_view_mut(observed, output).unwrap();
    let _reuse = guarded.len();
    drop(capability);
}

fn main() {}
