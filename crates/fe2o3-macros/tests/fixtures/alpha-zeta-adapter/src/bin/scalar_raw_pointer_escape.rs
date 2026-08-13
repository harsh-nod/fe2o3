use alpha_zeta_adapter_fixture::scalar_gemm_v1_gpu;

fn escape(arguments: scalar_gemm_v1_gpu::Arguments<'_>) -> *const () {
    arguments.a.device_pointer()
}

fn main() {
    let _ = escape;
}
