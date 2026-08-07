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

pub fn assert_generated_adapters() {
    fn assert_adapter<'allocation, K, Arguments>()
    where
        K: gpu_host::__generated::CompilerGeneratedKernelExpectationV1,
        Arguments: gpu_host::__generated::CompilerGeneratedAlphaZetaCov6ArgumentsV1<'allocation, K>,
    {
    }

    assert_adapter::<alpha_gpu::Marker, alpha_gpu::Arguments<'static>>();
    assert_adapter::<zeta_gpu::Marker, zeta_gpu::Arguments<'static>>();
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
