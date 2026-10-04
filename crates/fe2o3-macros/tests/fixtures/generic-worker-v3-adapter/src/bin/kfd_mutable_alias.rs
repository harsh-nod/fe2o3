use generic_worker_v3_adapter_fixture::multi_argument_kernel_gpu;
use gpu_host::__generated::{GeneratedKfdReadSlice, GeneratedKfdReadWriteSlice};

fn retain_two_outputs(output: &mut [f32]) {
    let a = [0.0_f32];
    let b = [0.0_f32];
    let first = multi_argument_kernel_gpu::Arguments::new(
        GeneratedKfdReadSlice::new(&a),
        GeneratedKfdReadSlice::new(&b),
        GeneratedKfdReadWriteSlice::new(output),
        1,
        1,
        1,
    );
    let second = multi_argument_kernel_gpu::Arguments::new(
        GeneratedKfdReadSlice::new(&a),
        GeneratedKfdReadSlice::new(&b),
        GeneratedKfdReadWriteSlice::new(output),
        1,
        1,
        1,
    );
    let _ = (first, second);
}

fn main() {}
