use generic_worker_v3_adapter_fixture::multi_argument_kernel_gpu;

fn escape(arguments: multi_argument_kernel_gpu::Arguments<'_>) -> *const () {
    arguments.first.device_pointer()
}

fn main() {
    let _ = escape;
}
