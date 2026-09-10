use generic_worker_v3_adapter_fixture::{combine_gpu, transform_gpu};

fn exact_kernel<T: gpu_host::CompilerGeneratedRuntimeArguments<combine_gpu::Marker>>() {}

fn main() {
    exact_kernel::<transform_gpu::RuntimeArguments>();
}
