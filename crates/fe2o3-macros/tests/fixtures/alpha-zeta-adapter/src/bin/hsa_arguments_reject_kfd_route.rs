use alpha_zeta_adapter_fixture::scalar_gemm_v1_gpu;

fn require_kfd<T>()
where
    T: gpu_host::__generated::CompilerGeneratedKfdArguments<
            'static,
            scalar_gemm_v1_gpu::Marker,
        >,
{
}

fn main() {
    require_kfd::<scalar_gemm_v1_gpu::Arguments<'static>>();
}
