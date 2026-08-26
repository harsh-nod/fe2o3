use fe2o3_device::{DisjointSlice, kernel, thread};
use fe2o3_host::CompilerGeneratedKernelExpectationV1;

#[kernel(
    typed,
    namespace = "0000000000000000000000000000000000000000000000000000000000000002"
)]
pub fn vecadd(a: &[f32], b: &[f32], mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    let offset = index.get();
    if let Some(value) = output.get_mut(index) {
        *value = a[offset] + b[offset];
    }
}

pub fn binding_and_registration() -> ([u8; 32], usize) {
    let registration =
        <__fe2o3_kernel_marker_vecadd as fe2o3_device::KernelMarkerV1>::REGISTRATION;
    (
        <vecadd_gpu::Marker as CompilerGeneratedKernelExpectationV1>::KERNEL_BINDING_ID_V1,
        registration as *const _ as usize,
    )
}
