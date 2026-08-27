use alpha_zeta_adapter_fixture::scalar_gemm_v1_gpu;
use gpu_host::__generated::{GeneratedKfdReadSlice, GeneratedKfdReadWriteSlice};

fn retain_two_outputs(output: &mut [f32]) {
    let a = [0.0_f32];
    let b = [0.0_f32];
    let first = scalar_gemm_v1_gpu::Arguments::new(
        GeneratedKfdReadSlice::new(&a),
        GeneratedKfdReadSlice::new(&b),
        GeneratedKfdReadWriteSlice::new(output),
        1,
        1,
        1,
    );
    let second = scalar_gemm_v1_gpu::Arguments::new(
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
