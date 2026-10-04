use generic_worker_v3_adapter_fixture::{multi_argument_kernel_gpu, transform_gpu};

fn require_multi_argument_adapter<T>()
where
    T: gpu_host::__generated::CompilerGeneratedKfdArguments<
        'static,
        multi_argument_kernel_gpu::Marker,
    >,
{
}

fn main() {
    require_multi_argument_adapter::<transform_gpu::Arguments<'static>>();
}
