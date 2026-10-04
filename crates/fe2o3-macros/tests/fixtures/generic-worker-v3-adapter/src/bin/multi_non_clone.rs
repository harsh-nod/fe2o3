use generic_worker_v3_adapter_fixture::multi_argument_kernel_gpu;

fn duplicate(arguments: multi_argument_kernel_gpu::Arguments<'_>) {
    let _ = arguments.clone();
}

fn main() {
    let _ = duplicate;
}
