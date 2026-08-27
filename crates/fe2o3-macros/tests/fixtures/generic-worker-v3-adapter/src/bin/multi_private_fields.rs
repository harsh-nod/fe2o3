use generic_worker_v3_adapter_fixture::multi_argument_kernel_gpu;

fn forge_arguments<'allocation>(
    first: gpu_host::__generated::GeneratedReadDeviceSlice<'allocation, f32>,
    second: gpu_host::__generated::GeneratedReadDeviceSlice<'allocation, f32>,
    destination: gpu_host::__generated::GeneratedReadWriteDeviceSlice<'allocation, f32>,
) -> multi_argument_kernel_gpu::Arguments<'allocation> {
    multi_argument_kernel_gpu::Arguments {
        first,
        second,
        destination,
        extent_x: 1,
        extent_y: 1,
        extent_z: 1,
    }
}

fn main() {
    let _ = forge_arguments;
}
