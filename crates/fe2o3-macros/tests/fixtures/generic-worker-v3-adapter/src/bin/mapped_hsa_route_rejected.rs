use generic_worker_v3_adapter_fixture::mapped_output_kernel_gpu;

fn require_hsa<T>()
where
    T: gpu_host::__generated::CompilerGeneratedWorkerV3ArgumentsV1<
            'static,
            mapped_output_kernel_gpu::Marker,
        >,
{
}

fn main() {
    require_hsa::<mapped_output_kernel_gpu::Arguments<'static>>();
}
