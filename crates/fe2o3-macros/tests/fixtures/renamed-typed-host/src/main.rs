use gpu_device::{DisjointSlice, kernel};

#[kernel(typed)]
pub fn renamed_typed(a: &[f32], b: &[f32], mut c: DisjointSlice<f32>) {
    let _ = (a, b, &mut c);
}

fn assert_contract<T: gpu_host::__generated::CompilerGeneratedKernelContractV1>() {}

fn assert_safe_aliases(
    _kernel: Option<renamed_typed_gpu::Kernel>,
    _prepared: Option<renamed_typed_gpu::Prepared<'static, 'static>>,
) {
}

fn main() {
    assert_contract::<__fe2o3_kernel_marker_renamed_typed>();
    assert_safe_aliases(None, None);
    assert_eq!(
        <__fe2o3_kernel_marker_renamed_typed as gpu_host::__generated::CompilerGeneratedKernelContractV1>::PROFILE,
        gpu_host::__generated::CompilerGeneratedKernelProfileV1::TypedVecAddF32V1
    );
    assert_eq!(
        <__fe2o3_kernel_marker_renamed_typed as gpu_device::KernelMarkerV1>::REGISTRATION.2,
        2
    );
}
