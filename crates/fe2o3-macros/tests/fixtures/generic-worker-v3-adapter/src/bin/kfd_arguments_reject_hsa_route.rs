use generic_worker_v3_adapter_fixture::multi_argument_kernel_gpu;
use gpu_host::__generated::{GeneratedKfdReadSlice, GeneratedKfdReadWriteSlice};

type KfdArguments = multi_argument_kernel_gpu::Arguments<
    'static,
    GeneratedKfdReadSlice<'static, f32>,
    GeneratedKfdReadSlice<'static, f32>,
    GeneratedKfdReadWriteSlice<'static, f32>,
>;

fn require_hsa<T>()
where
    T: gpu_host::__generated::CompilerGeneratedWorkerV3ArgumentsV1<
            'static,
            multi_argument_kernel_gpu::Marker,
        >,
{
}

fn main() {
    require_hsa::<KfdArguments>();
}
