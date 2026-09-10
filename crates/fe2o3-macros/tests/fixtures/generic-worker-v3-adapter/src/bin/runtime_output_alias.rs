use generic_worker_v3_adapter_fixture::transform_gpu;

fn main() {
    let (output, _observer) =
        gpu_host::GeneratedRuntimeReadWriteSlice::new(vec![0_f32; 2].into_boxed_slice());
    let first = transform_gpu::RuntimeArguments::new(
        1.0,
        gpu_host::GeneratedRuntimeReadSlice::new(vec![1_f32; 2].into_boxed_slice()),
        output,
    );
    let second = transform_gpu::RuntimeArguments::new(
        2.0,
        gpu_host::GeneratedRuntimeReadSlice::new(vec![2_f32; 2].into_boxed_slice()),
        output,
    );
    let _ = (first, second);
}
