use gpu_device::{DisjointSlice, kernel};
use gpu_host::__generated::{DeviceBuffer, GeneratedReadWriteDeviceSlice, ObservedContext};

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn two_outputs(left: DisjointSlice<f32>, right: DisjointSlice<f32>) {
    let _ = (left, right);
}

fn alias<'allocation>(observed: &ObservedContext, output: &'allocation mut DeviceBuffer<f32>) {
    let left = GeneratedReadWriteDeviceSlice::new(observed, output).unwrap();
    let right = GeneratedReadWriteDeviceSlice::new(observed, output).unwrap();
    let _arguments: two_outputs_gpu::Arguments<'allocation> =
        two_outputs_gpu::Arguments::new(left, right);
}

fn main() {
    let _ = alias;
}
