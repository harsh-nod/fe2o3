use fe2o3_device::{DisjointSlice, kernel, thread};
use fe2o3_host::CompilerGeneratedKernelContractV1;

#[kernel(typed)]
pub fn vecadd(a: &[f32], b: &[f32], mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    let offset = index.get();
    if let Some(value) = output.get_mut(index) {
        *value = a[offset] + b[offset];
    }
}

pub fn binding_and_artifact() -> ([u8; 32], usize, usize) {
    let bytes =
        <__fe2o3_kernel_marker_vecadd as CompilerGeneratedKernelContractV1>::artifact_container_bytes();
    (
        <__fe2o3_kernel_marker_vecadd as CompilerGeneratedKernelContractV1>::KERNEL_BINDING_ID_V1,
        bytes.as_ptr() as usize,
        bytes.len(),
    )
}
