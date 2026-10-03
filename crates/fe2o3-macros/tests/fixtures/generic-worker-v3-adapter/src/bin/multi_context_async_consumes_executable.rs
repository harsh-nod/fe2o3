use generic_worker_v3_adapter_fixture::transform_gpu;

fn reuse(
    executable: gpu_host::AuthenticatedWorkerV3ExecutableV1<transform_gpu::Marker>,
    arguments: transform_gpu::RuntimeArguments,
    owner: &gpu_runtime::RuntimeAsyncProgressHandleV1<gpu_runtime::KfdMultiDeviceRuntimeBackendV1>,
    device: gpu_runtime::RuntimeDeviceIdV1,
    geometry: gpu_host::AqlDispatchGeometryV1,
    budget: &gpu_host::GeneratedRuntimeResultBudgetV1,
) {
    let _ = executable.prepare_generated_multi_context_invocation_async(
        arguments,
        owner,
        device,
        geometry,
        0,
        1000,
        gpu_host::GeneratedRuntimeArgumentLimitsV1::new(4096, 4096, 3),
        budget,
    );
    let _ = executable.revalidate_currentness();
}

fn main() {}
