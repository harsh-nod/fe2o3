use generic_worker_v3_adapter_fixture::transform_gpu;

fn construct<'allocation>(
    source: gpu_host::__generated::GeneratedReadDeviceSlice<'allocation, f32>,
    destination: gpu_host::__generated::GeneratedReadWriteDeviceSlice<'allocation, f32>,
) -> transform_gpu::Arguments<'allocation> {
    transform_gpu::Arguments {
        factor: 2.0,
        source,
        destination,
    }
}

fn main() {
    let _ = construct;
}
