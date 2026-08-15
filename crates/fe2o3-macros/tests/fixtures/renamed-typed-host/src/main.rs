use gpu_device::{DeviceGlobalMutPtr, DisjointSlice, kernel};

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn renamed_typed(a: &[f32], b: &[f32], mut c: DisjointSlice<f32>) {
    let _ = (a, b, &mut c);
}

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn renamed_general(scale: f32, input: &[f32], output: DisjointSlice<f32>) {
    let _ = (scale, input, output);
}

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d",
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn renamed_global_mut(target: DeviceGlobalMutPtr<u32>) {
    let _ = target;
}

fn assert_contract<T: gpu_host::__generated::CompilerGeneratedKernelContractV1>() {}
fn assert_expectation<T: gpu_host::__generated::CompilerGeneratedKernelExpectationV1>() {}

fn assert_safe_aliases(
    _kernel: Option<renamed_typed_gpu::Kernel>,
    _prepared: Option<renamed_typed_gpu::Prepared<'static, 'static>>,
) {
}

fn assert_general_arguments<'allocation>(
    observed: &gpu_host::__generated::ObservedContext,
    input: &'allocation gpu_host::__generated::DeviceBuffer<f32>,
    output: &'allocation mut gpu_host::__generated::DeviceBuffer<f32>,
) {
    let input = gpu_host::__generated::GeneratedReadDeviceSlice::new(observed, input).unwrap();
    let output =
        gpu_host::__generated::GeneratedReadWriteDeviceSlice::new(observed, output).unwrap();
    let _arguments: renamed_general_gpu::Arguments<'allocation> =
        renamed_general_gpu::Arguments::new(2.0_f32, input, output);
}

fn assert_global_mut_argument<'allocation>(
    observed: &gpu_host::__generated::ObservedContext,
    target: &'allocation mut gpu_host::__generated::DeviceBuffer<u32>,
) {
    let target = gpu_host::__generated::GeneratedReadWriteDeviceSlice::new(observed, target)
        .unwrap();
    let target = renamed_global_mut_gpu::GlobalMut::new(target).unwrap();
    assert_eq!(target.len(), 1);
    assert!(!target.is_empty());
    let _arguments: renamed_global_mut_gpu::Arguments<'allocation> =
        renamed_global_mut_gpu::Arguments::new(target);
}

fn main() {
    assert_contract::<__fe2o3_kernel_marker_renamed_typed>();
    assert_expectation::<renamed_general_gpu::Marker>();
    assert_safe_aliases(None, None);
    let _ = assert_general_arguments;
    let _ = assert_global_mut_argument;
    assert_eq!(
        <__fe2o3_kernel_marker_renamed_typed as gpu_host::__generated::CompilerGeneratedKernelContractV1>::PROFILE,
        gpu_host::__generated::CompilerGeneratedKernelProfileV1::TypedVecAddF32RustcLayoutV2
    );
    assert_ne!(
        <__fe2o3_kernel_marker_renamed_typed as gpu_host::__generated::CompilerGeneratedKernelContractV1>::KERNEL_BINDING_ID_V1,
        [0; 32]
    );
    assert_eq!(
        <__fe2o3_kernel_marker_renamed_typed as gpu_device::KernelMarkerV1>::REGISTRATION.2,
        2
    );
    assert_eq!(
        <renamed_general_gpu::Marker as gpu_device::KernelMarkerV1>::REGISTRATION.1,
        3
    );
    assert_eq!(
        <renamed_general_gpu::Marker as gpu_device::KernelMarkerV1>::REGISTRATION.2,
        4
    );
    assert!(matches!(
        <renamed_general_gpu::Marker as gpu_host::__generated::CompilerGeneratedKernelExpectationV1>::PROFILE,
        gpu_host::__generated::CompilerGeneratedKernelProfileV1::ManifestDerivedScalarSliceV1 {
            generated_host_contract_identity
        } if generated_host_contract_identity != [0; 32]
    ));
}
