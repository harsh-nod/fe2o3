use gpu_device::{DisjointSlice, kernel};

#[kernel(typed)]
pub fn renamed_typed(a: &[f32], b: &[f32], mut c: DisjointSlice<f32>) {
    let _ = (a, b, &mut c);
}

fn assert_contract<T: gpu_host::__generated::CompilerGeneratedKernelContractV1>() {}

fn main() {
    assert_contract::<__fe2o3_kernel_marker_renamed_typed>();
    assert_eq!(
        <__fe2o3_kernel_marker_renamed_typed as gpu_device::KernelMarkerV1>::REGISTRATION.2,
        2
    );
}
