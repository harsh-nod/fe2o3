use generic_worker_v3_adapter_fixture::transform_gpu;

fn escape(source: &[f32], output: &mut [f32]) -> transform_gpu::RuntimeArguments {
    transform_gpu::RuntimeArguments::new(
        2.0,
        gpu_host::GeneratedKfdReadSlice::new(source),
        gpu_host::GeneratedKfdReadWriteSlice::new(output),
    )
}

fn main() {}
