use generic_worker_v3_adapter_fixture::transform_gpu;

fn reuse(
    executable: gpu_host::AuthenticatedWorkerV3ExecutableV1<transform_gpu::Marker>,
    arguments: transform_gpu::RuntimeArguments,
    device: gpu_host::CheckedGfx942XnackMinusDevice,
    geometry: gpu_host::AqlDispatchGeometryV1,
    budget: &gpu_host::GeneratedRuntimeResultBudgetV1,
) {
    let _ = executable.prepare_generated_runtime_invocation(
        arguments, device, geometry, 0, 1000,
        gpu_host::GeneratedRuntimeArgumentLimitsV1::new(4096, 4096, 3), budget,
    );
    let _ = executable.revalidate_currentness();
}

fn main() {}
