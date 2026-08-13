use alpha_zeta_adapter_fixture::scalar_gemm_v1_gpu;

fn escape<'short>(
    a: gpu_host::__generated::GeneratedScalarGemmV1ReadDeviceSlice<'short>,
    b: gpu_host::__generated::GeneratedScalarGemmV1ReadDeviceSlice<'short>,
    c: gpu_host::__generated::GeneratedScalarGemmV1ReadWriteDeviceSlice<'short>,
) -> scalar_gemm_v1_gpu::Arguments<'static> {
    scalar_gemm_v1_gpu::Arguments::new(a, b, c, 1, 1, 1)
}

fn main() {
    let _ = escape;
}
