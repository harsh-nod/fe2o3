use generic_worker_v3_adapter_fixture::multi_argument_kernel_gpu;

fn escape<'short>(
    first: gpu_host::__generated::GeneratedKfdReadSlice<'short, f32>,
    second: gpu_host::__generated::GeneratedKfdReadSlice<'short, f32>,
    destination: gpu_host::__generated::GeneratedKfdReadWriteSlice<'short, f32>,
) -> multi_argument_kernel_gpu::Arguments<'static> {
    multi_argument_kernel_gpu::Arguments::new(first, second, destination, 1, 1, 1)
}

fn main() {
    let _ = escape;
}
