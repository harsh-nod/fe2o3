use alpha_zeta_adapter_fixture::scalar_gemm_v1_gpu;

fn launch_twice<'loaded, 'allocation, P, A>(
    prepared: scalar_gemm_v1_gpu::Prepared<'loaded, 'allocation, P, A>,
) where
    A: gpu_host::ReviewedHsaImplicitKernargAdapterV1,
{
    let _ = prepared.dispatch();
    let _ = prepared.dispatch();
}

fn main() {
}
