use generic_worker_v3_adapter_fixture::transform_gpu;

fn escape(arguments: transform_gpu::Arguments<'_>) -> *const () {
    arguments.source.device_pointer()
}

fn main() {
    let _ = escape;
}
