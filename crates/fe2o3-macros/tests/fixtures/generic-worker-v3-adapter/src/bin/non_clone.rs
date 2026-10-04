use generic_worker_v3_adapter_fixture::transform_gpu;

fn duplicate(arguments: transform_gpu::Arguments<'_>) {
    let _copy = arguments.clone();
}

fn main() {
    let _ = duplicate;
}
