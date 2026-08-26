use alpha_zeta_adapter_fixture::scalar_gemm_v1_gpu;

fn launch_twice<'loaded, 'allocation, A>(
    prepared: gpu_host::__generated::GeneratedWorkerV3PreparedInvocationV1<
        'loaded,
        'allocation,
        scalar_gemm_v1_gpu::Marker,
        A,
        scalar_gemm_v1_gpu::Arguments<'allocation>,
    >,
) where
    A: gpu_host::ReviewedHsaImplicitKernargAdapterV1,
{
    let _ = prepared.dispatch();
    let _ = prepared.dispatch();
}

fn main() {
}
