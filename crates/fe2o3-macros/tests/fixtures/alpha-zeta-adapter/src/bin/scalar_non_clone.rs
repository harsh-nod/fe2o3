use alpha_zeta_adapter_fixture::scalar_gemm_v1_gpu;

fn duplicate(arguments: scalar_gemm_v1_gpu::Arguments<'_>) {
    let _ = arguments.clone();
}

fn main() {
    let _ = duplicate;
}
