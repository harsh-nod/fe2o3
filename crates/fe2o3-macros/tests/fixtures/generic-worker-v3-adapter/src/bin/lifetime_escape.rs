use generic_worker_v3_adapter_fixture::transform_gpu;

fn escape<'short>(
    source: gpu_host::__generated::GeneratedKfdReadSlice<'short, f32>,
    destination: gpu_host::__generated::GeneratedKfdReadWriteSlice<'short, f32>,
) -> transform_gpu::Arguments<'static> {
    transform_gpu::Arguments::new(2.0, source, destination)
}

fn main() {
    let _ = escape;
}
