use generic_worker_v3_adapter_fixture::multi_argument_kernel_gpu;

fn launch_twice<'loaded, 'allocation, A>(
    prepared: gpu_host::__generated::GeneratedWorkerV3PreparedInvocationV1<
        'loaded,
        'allocation,
        multi_argument_kernel_gpu::Marker,
        A,
        multi_argument_kernel_gpu::Arguments<'allocation>,
    >,
) where
    A: gpu_host::ReviewedHsaImplicitKernargAdapterV1,
{
    let _ = prepared.dispatch();
    let _ = prepared.dispatch();
}

fn main() {
}
