use generic_worker_v3_adapter_fixture::{context_map_gpu, transform_gpu};
fn takes(_: gpu_runtime::TypedRuntimeKernelV1<context_map_gpu::ContextArguments>) {}
fn wrong(kernel: gpu_runtime::TypedRuntimeKernelV1<transform_gpu::ContextArguments>) {
    takes(kernel);
}
fn main() {}
