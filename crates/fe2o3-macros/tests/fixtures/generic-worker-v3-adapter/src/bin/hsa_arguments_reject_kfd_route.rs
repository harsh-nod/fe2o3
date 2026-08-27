use generic_worker_v3_adapter_fixture::multi_argument_kernel_gpu;

fn require_kfd<T>()
where
    T: gpu_host::__generated::CompilerGeneratedKfdArguments<
            'static,
            multi_argument_kernel_gpu::Marker,
        >,
{
}

fn main() {
    require_kfd::<multi_argument_kernel_gpu::Arguments<'static>>();
}
