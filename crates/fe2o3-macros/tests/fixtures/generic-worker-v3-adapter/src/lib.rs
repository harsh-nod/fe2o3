use gpu_device::{DisjointSlice, kernel};

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
pub fn combine(
    left: &[f32],
    right: &[f32],
    offset: f32,
    destination: DisjointSlice<f32>,
) {
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
    let _ = (
        first,
        second,
        destination,
        extent_x,
        extent_y,
        extent_z,
    );
}

pub fn assert_generated_adapters() {
    fn assert_adapter<'allocation, K, Arguments>()
    where
        K: gpu_host::__generated::CompilerGeneratedKernelExpectationV1,
        Arguments: gpu_host::__generated::CompilerGeneratedWorkerV3ArgumentsV1<'allocation, K>,
    {
    }

    assert_adapter::<transform_gpu::Marker, transform_gpu::Arguments<'static>>();
    assert_adapter::<combine_gpu::Marker, combine_gpu::Arguments<'static>>();
    assert_adapter::<
        multi_argument_kernel_gpu::Marker,
        multi_argument_kernel_gpu::Arguments<'static>,
    >();
}

pub fn prepare_transform<'loaded, 'allocation, A>(
    executable: &'loaded mut gpu_host::LoadedWorkerV3HsaExecutableV1<transform_gpu::Marker, A>,
    observed: &gpu_host::ObservedContext,
    geometry: gpu_host::HsaLaunchGeometryV1,
    arguments: transform_gpu::Arguments<'allocation>,
) where
    A: gpu_host::ReviewedHsaImplicitKernargAdapterV1,
{
    let _prepared = arguments.prepare_worker_v3(executable, observed, geometry);
}

pub fn prepare_combine<'loaded, 'allocation, A>(
    executable: &'loaded mut gpu_host::LoadedWorkerV3HsaExecutableV1<combine_gpu::Marker, A>,
    observed: &gpu_host::ObservedContext,
    geometry: gpu_host::HsaLaunchGeometryV1,
    arguments: combine_gpu::Arguments<'allocation>,
) where
    A: gpu_host::ReviewedHsaImplicitKernargAdapterV1,
{
    let _prepared = arguments.prepare_worker_v3(executable, observed, geometry);
}

pub fn prepare_multi_argument<'loaded, 'allocation, A>(
    executable: &'loaded mut gpu_host::LoadedWorkerV3HsaExecutableV1<
        multi_argument_kernel_gpu::Marker,
        A,
    >,
    observed: &gpu_host::ObservedContext,
    geometry: gpu_host::HsaLaunchGeometryV1,
    arguments: multi_argument_kernel_gpu::Arguments<'allocation>,
) where
    A: gpu_host::ReviewedHsaImplicitKernargAdapterV1,
{
    let _prepared = arguments.prepare_worker_v3(executable, observed, geometry);
}
