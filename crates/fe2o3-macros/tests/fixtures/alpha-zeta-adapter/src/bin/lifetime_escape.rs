use alpha_zeta_adapter_fixture::alpha_gpu;

fn escape<'short>(
    input: gpu_host::__generated::GeneratedReadDeviceSlice<'short, f32>,
    output: gpu_host::__generated::GeneratedReadWriteDeviceSlice<'short, f32>,
) -> alpha_gpu::Arguments<'static> {
    alpha_gpu::Arguments::new(2.0, input, output)
}

fn main() {
    let _ = escape;
}
