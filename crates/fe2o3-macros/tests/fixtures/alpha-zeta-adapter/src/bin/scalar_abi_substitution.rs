use alpha_zeta_adapter_fixture::{alpha_gpu, scalar_gemm_v1_gpu};

fn require_scalar_adapter<T>()
where
    T: gpu_host::__generated::CompilerGeneratedWorkerV3ArgumentsV1<
        'static,
        scalar_gemm_v1_gpu::Marker,
    >,
{
}

fn main() {
    require_scalar_adapter::<alpha_gpu::Arguments<'static>>();
}
