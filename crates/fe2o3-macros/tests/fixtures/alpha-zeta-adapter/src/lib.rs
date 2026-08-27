use gpu_device::{DisjointSlice, kernel};

#[kernel(
    typed,
    namespace = "8c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn alpha(scale: f32, input: &[f32], output: DisjointSlice<f32>) {
    let _ = (scale, input, output);
}

#[kernel(
    typed,
    namespace = "8c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn zeta(a: &[f32], b: &[f32], bias: f32, output: DisjointSlice<f32>) {
    let _ = (a, b, bias, output);
}

#[kernel(
    typed,
    namespace = "53bf3c83481a081d4ab0e2b32039f9c89be5de3937a84aca0c40800c8d6b0413",
    launch(required = [256, 1, 1], max = [256, 1, 1])
)]
pub fn scalar_gemm_v1(
    a: &[f32],
    b: &[f32],
    c: DisjointSlice<f32>,
    m: u32,
    n: u32,
    k: u32,
) {
    let _ = (a, b, c, m, n, k);
}

pub fn assert_generated_adapters() {
    fn assert_adapter<'allocation, K, Arguments>()
    where
        K: gpu_host::__generated::CompilerGeneratedKernelExpectationV1,
        Arguments: gpu_host::__generated::CompilerGeneratedWorkerV3ArgumentsV1<'allocation, K>,
    {
    }

    fn assert_kfd_adapter<'allocation, K, Arguments>()
    where
        K: gpu_host::__generated::CompilerGeneratedKernelExpectationV1,
        Arguments: gpu_host::__generated::CompilerGeneratedKfdArguments<'allocation, K>,
    {
    }

    assert_adapter::<alpha_gpu::Marker, alpha_gpu::Arguments<'static>>();
    assert_adapter::<zeta_gpu::Marker, zeta_gpu::Arguments<'static>>();
    assert_adapter::<
        scalar_gemm_v1_gpu::Marker,
        scalar_gemm_v1_gpu::Arguments<'static>,
    >();
    assert_kfd_adapter::<
        alpha_gpu::Marker,
        alpha_gpu::Arguments<
            'static,
            gpu_host::__generated::GeneratedKfdReadSlice<'static, f32>,
            gpu_host::__generated::GeneratedKfdReadWriteSlice<'static, f32>,
        >,
    >();
    assert_kfd_adapter::<
        scalar_gemm_v1_gpu::Marker,
        scalar_gemm_v1_gpu::Arguments<
            'static,
            gpu_host::__generated::GeneratedKfdReadSlice<'static, f32>,
            gpu_host::__generated::GeneratedKfdReadSlice<'static, f32>,
            gpu_host::__generated::GeneratedKfdReadWriteSlice<'static, f32>,
        >,
    >();
}

pub fn prepare_alpha<'loaded, 'allocation, A>(
    executable: &'loaded mut gpu_host::LoadedWorkerV3HsaExecutableV1<alpha_gpu::Marker, A>,
    observed: &gpu_host::ObservedContext,
    geometry: gpu_host::HsaLaunchGeometryV1,
    arguments: alpha_gpu::Arguments<'allocation>,
) where
    A: gpu_host::ReviewedHsaImplicitKernargAdapterV1,
{
    let _prepared = arguments.prepare_worker_v3(executable, observed, geometry);
}

pub fn prepare_zeta<'loaded, 'allocation, A>(
    executable: &'loaded mut gpu_host::LoadedWorkerV3HsaExecutableV1<zeta_gpu::Marker, A>,
    observed: &gpu_host::ObservedContext,
    geometry: gpu_host::HsaLaunchGeometryV1,
    arguments: zeta_gpu::Arguments<'allocation>,
) where
    A: gpu_host::ReviewedHsaImplicitKernargAdapterV1,
{
    let _prepared = arguments.prepare_worker_v3(executable, observed, geometry);
}

pub fn prepare_scalar_gemm<'loaded, 'allocation, A>(
    executable: &'loaded mut gpu_host::LoadedWorkerV3HsaExecutableV1<
        scalar_gemm_v1_gpu::Marker,
        A,
    >,
    observed: &gpu_host::ObservedContext,
    geometry: gpu_host::HsaLaunchGeometryV1,
    arguments: scalar_gemm_v1_gpu::Arguments<'allocation>,
) where
    A: gpu_host::ReviewedHsaImplicitKernargAdapterV1,
{
    let _prepared = arguments.prepare_worker_v3(executable, observed, geometry);
}

pub fn zero_shape_arguments<'allocation>(
    a: gpu_host::__generated::GeneratedReadDeviceSlice<'allocation, f32>,
    b: gpu_host::__generated::GeneratedReadDeviceSlice<'allocation, f32>,
    c: gpu_host::__generated::GeneratedReadWriteDeviceSlice<'allocation, f32>,
) -> scalar_gemm_v1_gpu::Arguments<'allocation> {
    scalar_gemm_v1_gpu::Arguments::new(a, b, c, 0, u32::MAX, 1)
}

pub fn overflow_candidate_arguments<'allocation>(
    a: gpu_host::__generated::GeneratedReadDeviceSlice<'allocation, f32>,
    b: gpu_host::__generated::GeneratedReadDeviceSlice<'allocation, f32>,
    c: gpu_host::__generated::GeneratedReadWriteDeviceSlice<'allocation, f32>,
) -> scalar_gemm_v1_gpu::Arguments<'allocation> {
    scalar_gemm_v1_gpu::Arguments::new(a, b, c, u32::MAX, u32::MAX, u32::MAX)
}
