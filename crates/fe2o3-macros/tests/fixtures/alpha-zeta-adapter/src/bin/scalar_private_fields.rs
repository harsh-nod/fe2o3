use alpha_zeta_adapter_fixture::scalar_gemm_v1_gpu;

fn replace_dimensions<'allocation>(
    a: gpu_host::__generated::GeneratedScalarGemmV1ReadDeviceSlice<'allocation>,
    b: gpu_host::__generated::GeneratedScalarGemmV1ReadDeviceSlice<'allocation>,
    c: gpu_host::__generated::GeneratedScalarGemmV1ReadWriteDeviceSlice<'allocation>,
) -> scalar_gemm_v1_gpu::Arguments<'allocation> {
    scalar_gemm_v1_gpu::Arguments { a, b, c, m: 1, n: 1, k: 1 }
}

fn main() {
    let _ = replace_dimensions;
}
