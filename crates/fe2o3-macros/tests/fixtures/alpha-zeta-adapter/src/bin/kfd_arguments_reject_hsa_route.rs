use alpha_zeta_adapter_fixture::scalar_gemm_v1_gpu;
use gpu_host::__generated::{GeneratedKfdReadSlice, GeneratedKfdReadWriteSlice};

type KfdArguments = scalar_gemm_v1_gpu::Arguments<
    'static,
    GeneratedKfdReadSlice<'static, f32>,
    GeneratedKfdReadSlice<'static, f32>,
    GeneratedKfdReadWriteSlice<'static, f32>,
>;

fn require_hsa<T>()
where
    T: gpu_host::__generated::CompilerGeneratedWorkerV3ArgumentsV1<
            'static,
            scalar_gemm_v1_gpu::Marker,
        >,
{
}

fn main() {
    require_hsa::<KfdArguments>();
}
