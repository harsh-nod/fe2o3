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
        Arguments: gpu_host::__generated::CompilerGeneratedAlphaZetaCov6ArgumentsV1<'allocation, K>,
    {
    }

    assert_adapter::<alpha_gpu::Marker, alpha_gpu::Arguments<'static>>();
    assert_adapter::<zeta_gpu::Marker, zeta_gpu::Arguments<'static>>();

    fn assert_scalar_gemm_adapter<'allocation, K, Arguments>()
    where
        K: gpu_host::__generated::CompilerGeneratedKernelExpectationV1,
        Arguments:
            gpu_host::__generated::CompilerGeneratedScalarGemmV1Arguments<'allocation, K>,
    {
    }
    assert_scalar_gemm_adapter::<
        scalar_gemm_v1_gpu::Marker,
        scalar_gemm_v1_gpu::Arguments<'static>,
    >();
}

pub fn prepare_alpha<'loaded, 'allocation, P, A, Authenticator>(
    executable: &'loaded mut gpu_host::LoadedHsaExecutableV1<P, A>,
    observed: &gpu_host::ObservedContext,
    authenticator: &mut Authenticator,
    arguments: alpha_gpu::Arguments<'allocation>,
) where
    A: gpu_host::ReviewedHsaImplicitKernargAdapterV1,
    Authenticator: gpu_host::WorkerV2PrerequisiteAuthenticatorV1<alpha_gpu::Marker>,
{
    let _prepared = arguments.prepare(executable, observed, authenticator);
}

pub fn prepare_zeta<'loaded, 'allocation, P, A, Authenticator>(
    executable: &'loaded mut gpu_host::LoadedHsaExecutableV1<P, A>,
    observed: &gpu_host::ObservedContext,
    authenticator: &mut Authenticator,
    arguments: zeta_gpu::Arguments<'allocation>,
) where
    A: gpu_host::ReviewedHsaImplicitKernargAdapterV1,
    Authenticator: gpu_host::WorkerV2PrerequisiteAuthenticatorV1<zeta_gpu::Marker>,
{
    let _prepared = arguments.prepare(executable, observed, authenticator);
}

pub fn prepare_scalar_gemm<'loaded, 'allocation, P, A, Authenticator>(
    executable: &'loaded mut gpu_host::LoadedHsaExecutableV1<P, A>,
    observed: &gpu_host::ObservedContext,
    authenticator: &mut Authenticator,
    arguments: scalar_gemm_v1_gpu::Arguments<'allocation>,
) where
    A: gpu_host::ReviewedHsaImplicitKernargAdapterV1,
    Authenticator: gpu_host::WorkerV2PrerequisiteAuthenticatorV1<scalar_gemm_v1_gpu::Marker>,
{
    let _prepared = arguments.prepare(executable, observed, authenticator);
}

pub fn zero_shape_arguments<'allocation>(
    a: gpu_host::__generated::GeneratedScalarGemmV1ReadDeviceSlice<'allocation>,
    b: gpu_host::__generated::GeneratedScalarGemmV1ReadDeviceSlice<'allocation>,
    c: gpu_host::__generated::GeneratedScalarGemmV1ReadWriteDeviceSlice<'allocation>,
) -> scalar_gemm_v1_gpu::Arguments<'allocation> {
    scalar_gemm_v1_gpu::Arguments::new(a, b, c, 0, u32::MAX, 1)
}

pub fn overflow_candidate_arguments<'allocation>(
    a: gpu_host::__generated::GeneratedScalarGemmV1ReadDeviceSlice<'allocation>,
    b: gpu_host::__generated::GeneratedScalarGemmV1ReadDeviceSlice<'allocation>,
    c: gpu_host::__generated::GeneratedScalarGemmV1ReadWriteDeviceSlice<'allocation>,
) -> scalar_gemm_v1_gpu::Arguments<'allocation> {
    scalar_gemm_v1_gpu::Arguments::new(a, b, c, u32::MAX, u32::MAX, u32::MAX)
}
