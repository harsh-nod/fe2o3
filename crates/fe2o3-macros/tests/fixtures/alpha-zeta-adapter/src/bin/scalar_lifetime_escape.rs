use alpha_zeta_adapter_fixture::scalar_gemm_v1_gpu;

fn escape<'short>(
    a: gpu_host::__generated::GeneratedReadDeviceSlice<'short, f32>,
    b: gpu_host::__generated::GeneratedReadDeviceSlice<'short, f32>,
    c: gpu_host::__generated::GeneratedReadWriteDeviceSlice<'short, f32>,
) -> scalar_gemm_v1_gpu::Arguments<'static> {
    scalar_gemm_v1_gpu::Arguments::new(a, b, c, 1, 1, 1)
}

fn main() {
    let _ = escape;
}
