use alpha_zeta_adapter_fixture::alpha_gpu;

fn construct<'allocation>(
    input: gpu_host::__generated::GeneratedReadDeviceSlice<'allocation, f32>,
    output: gpu_host::__generated::GeneratedReadWriteDeviceSlice<'allocation, f32>,
) -> alpha_gpu::Arguments<'allocation> {
    alpha_gpu::Arguments {
        scale: 2.0,
        input,
        output,
    }
}

fn main() {
    let _ = construct;
}
