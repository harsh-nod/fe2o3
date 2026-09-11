use gpu_device::{Blocked, DisjointSlice, Index1D, kernel};

#[kernel(
    typed,
    namespace = "8c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn transform(factor: f32, source: &[f32], destination: DisjointSlice<f32>) {
    let _ = (factor, source, destination);
}

#[kernel(
    typed,
    namespace = "8c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn combine(left: &[f32], right: &[f32], offset: f32, destination: DisjointSlice<f32>) {
    let _ = (left, right, offset, destination);
}

#[kernel(
    typed,
    namespace = "53bf3c83481a081d4ab0e2b32039f9c89be5de3937a84aca0c40800c8d6b0413",
    launch(required = [256, 1, 1], max = [256, 1, 1])
)]
pub fn multi_argument_kernel(
    first: &[f32],
    second: &[f32],
    destination: DisjointSlice<f32>,
    extent_x: u32,
    extent_y: u32,
    extent_z: u32,
) {
    let _ = (first, second, destination, extent_x, extent_y, extent_z);
}

#[kernel(
    typed,
    namespace = "d4c75aecdeaa38326f57d8deca5225f5644e9f08b31a64758ac79e06c4d082cf",
    launch(required = [256, 1, 1], max = [256, 1, 1])
)]
pub fn mapped_output_kernel(
    first: &[u16],
    second: &[u16],
    destination: DisjointSlice<u16, Blocked<Index1D, 1, 8>>,
) {
    let _ = (first, second, destination);
}

pub fn assert_generated_adapters() {
    fn assert_kfd_adapter<'allocation, K, Arguments>()
    where
        K: gpu_host::__generated::CompilerGeneratedKernelExpectationV1,
        Arguments: gpu_host::__generated::CompilerGeneratedKfdArguments<'allocation, K>,
    {
    }

    fn assert_owned_adapter<K, Arguments>()
    where
        K: gpu_host::__generated::CompilerGeneratedKernelExpectationV1,
        Arguments: gpu_host::__generated::CompilerGeneratedRuntimeArguments<K> + Send + 'static,
    {
    }

    assert_owned_adapter::<transform_gpu::Marker, transform_gpu::RuntimeArguments>();
    assert_owned_adapter::<combine_gpu::Marker, combine_gpu::RuntimeArguments>();
    assert_owned_adapter::<
        multi_argument_kernel_gpu::Marker,
        multi_argument_kernel_gpu::RuntimeArguments,
    >();
    assert_owned_adapter::<
        mapped_output_kernel_gpu::Marker,
        mapped_output_kernel_gpu::RuntimeArguments,
    >();

    assert_kfd_adapter::<transform_gpu::Marker, transform_gpu::Arguments<'static>>();
    assert_kfd_adapter::<combine_gpu::Marker, combine_gpu::Arguments<'static>>();
    assert_kfd_adapter::<
        multi_argument_kernel_gpu::Marker,
        multi_argument_kernel_gpu::Arguments<'static>,
    >();
    assert_kfd_adapter::<
        multi_argument_kernel_gpu::Marker,
        multi_argument_kernel_gpu::Arguments<
            'static,
            gpu_host::__generated::GeneratedKfdReadSlice<'static, f32>,
            gpu_host::__generated::GeneratedKfdReadSlice<'static, f32>,
            gpu_host::__generated::GeneratedKfdReadWriteSlice<'static, f32>,
        >,
    >();
    assert_kfd_adapter::<
        mapped_output_kernel_gpu::Marker,
        mapped_output_kernel_gpu::Arguments<
            'static,
            gpu_host::__generated::GeneratedKfdReadSlice<'static, u16>,
            gpu_host::__generated::GeneratedKfdReadSlice<'static, u16>,
            gpu_host::__generated::GeneratedKfdReadWriteSlice<'static, u16>,
        >,
    >();
}

pub fn owned_generated_arguments() -> transform_gpu::RuntimeArguments {
    let source = gpu_host::GeneratedRuntimeReadSlice::new(vec![1_f32, 2.0].into_boxed_slice());
    let (destination, observer) =
        gpu_host::GeneratedRuntimeReadWriteSlice::new(vec![0_f32; 2].into_boxed_slice());
    drop(observer);
    transform_gpu::RuntimeArguments::new(2.0, source, destination)
}

pub fn prepare_owned_generated_arguments(
    executable: &gpu_host::AuthenticatedWorkerV3ExecutableV1<transform_gpu::Marker>,
) -> Result<gpu_host::GeneratedRuntimePackedArgumentsV1, gpu_host::GeneratedRuntimeArgumentErrorV1>
{
    executable.prepare_generated_runtime_arguments(
        owned_generated_arguments(),
        gpu_host::GeneratedRuntimeArgumentLimitsV1::new(4096, 4096, 3),
    )
}

pub fn prepare_charged_generated_arguments(
    executable: &gpu_host::AuthenticatedWorkerV3ExecutableV1<transform_gpu::Marker>,
    budget: &gpu_host::GeneratedRuntimeResultBudgetV1,
) -> Result<(gpu_host::GeneratedRuntimeChargedArgumentsV1, gpu_host::GeneratedRuntimeChargedResultV1<f32>), gpu_host::GeneratedRuntimeArgumentErrorV1> {
    let source = gpu_host::GeneratedRuntimeReadSlice::new(vec![1f32, 2.0].into_boxed_slice());
    let (output, observer) = gpu_host::GeneratedRuntimeReadWriteSlice::new_charged(vec![0f32; 2].into_boxed_slice());
    let prepared = executable.prepare_generated_runtime_arguments_charged(
        transform_gpu::RuntimeArguments::new(2.0, source, output),
        gpu_host::GeneratedRuntimeArgumentLimitsV1::new(4096, 4096, 3),
        budget,
    )?;
    Ok((prepared, observer))
}

pub fn mapped_kfd_arguments<'allocation>(
    first: &'allocation [u16],
    second: &'allocation [u16],
    destination: &'allocation mut [u16],
) -> mapped_output_kernel_gpu::Arguments<
    'allocation,
    gpu_host::__generated::GeneratedKfdReadSlice<'allocation, u16>,
    gpu_host::__generated::GeneratedKfdReadSlice<'allocation, u16>,
    gpu_host::__generated::GeneratedKfdReadWriteSlice<'allocation, u16>,
> {
    mapped_output_kernel_gpu::Arguments::new(
        gpu_host::__generated::GeneratedKfdReadSlice::new(first),
        gpu_host::__generated::GeneratedKfdReadSlice::new(second),
        gpu_host::__generated::GeneratedKfdReadWriteSlice::new(destination),
    )
}
